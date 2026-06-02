//! Form.io verb integration routes.
//!
//! Two endpoints bridge the JS/React side of the dsl.form verb to the
//! bpmn-runtime's HumanTaskComplete event:
//!
//!   GET  /api/forms/:ref          — serve form schema JSON from form_schemas table
//!   POST /api/forms/:token_id/submit — accept submission, drain engine, return 200
//!
//! The Rust verb (DslFormHandler) is form.io-agnostic: it emits {form_ref, mode,
//! prefill_data} and parks the fiber with correlation_key = token_id.to_string().
//! These endpoints complete the JS→Rust return path.
//!
//! Submit wiring: enqueue HumanTaskComplete into dsl_event_queue, then call
//! ProcessRegistry::run_instance to drain synchronously before returning 200.
//! Returns 200 (not 202) because the drain is inline and the caller knows the
//! engine has quiesced when the response arrives.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde_json::Value;

use crate::process_registry::ProcessRegistry;

/// State for the forms router — pool + process registry.
#[derive(Clone)]
pub(crate) struct FormsState {
    pool: sqlx::PgPool,
    process_registry: std::sync::Arc<ProcessRegistry>,
}

/// Build the forms router.
pub(crate) fn create_forms_router(
    pool: sqlx::PgPool,
    process_registry: std::sync::Arc<ProcessRegistry>,
) -> Router {
    Router::new()
        .route("/api/forms/:form_ref", get(get_form_schema))
        .route("/api/forms/:token_id/submit", post(submit_form))
        .with_state(FormsState {
            pool,
            process_registry,
        })
}

/// `GET /api/forms/:form_ref`
///
/// Queries form_schemas table by ref slug. Returns 404 if not found.
async fn get_form_schema(
    State(state): State<FormsState>,
    Path(form_ref): Path<String>,
) -> impl IntoResponse {
    let row = sqlx::query_scalar!("SELECT schema FROM form_schemas WHERE ref = $1", form_ref)
        .fetch_optional(&state.pool)
        .await;

    match row {
        Ok(Some(schema)) => Json(schema).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            format!("form schema not found: {}", form_ref),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("db error: {}", e),
        )
            .into_response(),
    }
}

/// `POST /api/forms/:token_id/submit`
///
/// Accepts a form submission from the React FormioForm component.
/// Looks up the parked fiber by correlation_key = token_id, enqueues
/// HumanTaskComplete, then drains the engine synchronously (returns 200
/// once the engine has quiesced, not 202).
///
/// Response body: `{ accepted, token_id, instance_id, next_form? }`
/// where `next_form` is populated if the engine immediately re-parks at
/// another human task (chained form flow).
async fn submit_form(
    State(state): State<FormsState>,
    Path(token_id): Path<String>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    // Look up the pending human-task wait by correlation key (= token_id string).
    let wait = sqlx::query!(
        r#"SELECT instance_id, node_name
           FROM dsl_pending_wait
           WHERE wait_kind = 'human_task' AND correlation_key = $1
           LIMIT 1"#,
        token_id
    )
    .fetch_optional(&state.pool)
    .await;

    let wait = match wait {
        Ok(Some(w)) => w,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                format!("no parked human-task for token_id: {}", token_id),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("db error looking up pending wait: {}", e),
            )
                .into_response();
        }
    };

    // Enqueue HumanTaskComplete into dsl_event_queue.
    let event_payload = serde_json::json!({
        "node_name":   wait.node_name,
        "token_id":    token_id,
        "output_data": payload,
    });

    let enqueue = sqlx::query!(
        r#"INSERT INTO dsl_event_queue (instance_id, event_kind, payload)
           VALUES ($1, 'HumanTaskComplete', $2)"#,
        wait.instance_id,
        event_payload
    )
    .execute(&state.pool)
    .await;

    if let Err(e) = enqueue {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to enqueue HumanTaskComplete: {}", e),
        )
            .into_response();
    }

    // Drain the engine synchronously — run forward until quiesced.
    // Returns the next pending human task if the process re-parks.
    let next_form = match state.process_registry.run_instance(wait.instance_id).await {
        Ok(pending) => pending,
        Err(e) => {
            tracing::warn!(
                instance_id = %wait.instance_id,
                error = %e,
                "ProcessRegistry::run_instance failed after HumanTaskComplete"
            );
            None
        }
    };

    tracing::info!(
        token_id = %token_id,
        instance_id = %wait.instance_id,
        node_name = %wait.node_name,
        has_next_form = next_form.is_some(),
        "HumanTaskComplete processed, engine quiesced"
    );

    Json(serde_json::json!({
        "accepted":    true,
        "token_id":    token_id,
        "instance_id": wait.instance_id,
        "next_form":   next_form,
    }))
    .into_response()
}
