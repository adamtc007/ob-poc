//! Form.io verb integration routes.
//!
//! Two endpoints bridge the JS/React side of the dsl.form verb to the
//! bpmn-runtime's HumanTaskComplete event:
//!
//!   GET  /api/forms/:ref          — serve form schema JSON from form_schemas table
//!   POST /api/forms/:token_id/submit — accept submission, enqueue HumanTaskComplete
//!
//! The Rust verb (DslFormHandler) is form.io-agnostic: it emits {form_ref, mode,
//! prefill_data} and parks the fiber with correlation_key = token_id.to_string().
//! These endpoints complete the JS→Rust return path.
//!
//! Submit wiring: rather than coupling to a RuntimeEngine instance, the endpoint
//! writes directly to dsl_event_queue. The runtime's next tick drains the queue
//! and advances the parked token.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

/// Build the forms router, sharing the application PgPool.
pub(crate) fn create_forms_router(pool: PgPool) -> Router {
    Router::new()
        .route("/api/forms/:form_ref", get(get_form_schema))
        .route("/api/forms/:token_id/submit", post(submit_form))
        .with_state(pool)
}

/// `GET /api/forms/:form_ref`
///
/// Queries form_schemas table by ref slug. Returns 404 if not found.
async fn get_form_schema(
    State(pool): State<PgPool>,
    Path(form_ref): Path<String>,
) -> impl IntoResponse {
    let row = sqlx::query_scalar!(
        "SELECT schema FROM form_schemas WHERE ref = $1",
        form_ref
    )
    .fetch_optional(&pool)
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
/// Looks up the parked fiber by correlation_key = token_id, then enqueues
/// a HumanTaskComplete event into dsl_event_queue for the runtime to drain.
async fn submit_form(
    State(pool): State<PgPool>,
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
    .fetch_optional(&pool)
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
    // The bpmn-runtime drains this on its next tick.
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
    .execute(&pool)
    .await;

    match enqueue {
        Ok(_) => {
            tracing::info!(
                token_id = %token_id,
                instance_id = %wait.instance_id,
                node_name = %wait.node_name,
                "HumanTaskComplete enqueued for parked fiber"
            );
            Json(serde_json::json!({
                "accepted":    true,
                "token_id":    token_id,
                "instance_id": wait.instance_id,
            }))
            .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to enqueue HumanTaskComplete: {}", e),
        )
            .into_response(),
    }
}

/// Parse a token_id string to UUID, returning a 400 response on failure.
#[allow(dead_code)]
fn parse_token_id(s: &str) -> Result<Uuid, impl IntoResponse> {
    Uuid::parse_str(s).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            format!("token_id is not a valid UUID: {}", s),
        )
    })
}
