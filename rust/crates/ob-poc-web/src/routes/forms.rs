//! Form.io verb integration routes.
//!
//! Two endpoints bridge the JS/React side of the dsl.form verb to the
//! bpmn-runtime's HumanTaskComplete event:
//!
//!   GET  /api/forms/:ref          — serve form schema JSON by ref key
//!   POST /api/forms/:token_id/submit — accept submission, deliver HumanTaskComplete
//!
//! The Rust verb (DslFormHandler) is form.io-agnostic: it emits {form_ref, mode,
//! prefill_data} and parks the fiber. These endpoints complete the JS→Rust return.
//!
//! Schema storage for v1: local JSON files at `config/forms/{ref}.json`.
//! Q10 (schema authority) is open for board decision — see design doc §10.6.

use axum::{
    extract::Path,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde_json::Value;
use std::path::PathBuf;

/// Build the forms router.
pub(crate) fn create_forms_router() -> Router {
    Router::new()
        .route("/api/forms/:form_ref", get(get_form_schema))
        .route("/api/forms/:token_id/submit", post(submit_form))
}

/// `GET /api/forms/:form_ref`
///
/// Serves a form schema JSON file from `config/forms/{form_ref}.json`.
/// The React side calls this to resolve the schema after receiving {form_ref}
/// from the session response.
async fn get_form_schema(Path(form_ref): Path<String>) -> impl IntoResponse {
    let schema_path = forms_dir().join(format!("{}.json", form_ref));
    match std::fs::read_to_string(&schema_path) {
        Ok(json) => match serde_json::from_str::<Value>(&json) {
            Ok(schema) => Json(schema).into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("form schema parse error: {}", e),
            )
                .into_response(),
        },
        Err(_) => (
            StatusCode::NOT_FOUND,
            format!("form schema not found: {}", form_ref),
        )
            .into_response(),
    }
}

/// `POST /api/forms/:token_id/submit`
///
/// Accepts a form submission from the React FormioForm component.
/// The token_id in the URL identifies the parked bpmn-runtime fiber.
///
/// In v1 this endpoint logs the submission and returns 200. Full wiring
/// (dispatching HumanTaskComplete into the running RuntimeEngine) requires
/// access to the RuntimeEngine instance from the session — that wire is
/// a follow-up once the in-process runtime is integrated into the session
/// execution path (currently the ob-poc session uses the bpmn-lite gRPC path).
async fn submit_form(
    Path(token_id): Path<String>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    tracing::info!(
        token_id = %token_id,
        submission_keys = ?payload.as_object().map(|o| o.keys().collect::<Vec<_>>()),
        "form submission received"
    );

    // TODO(T5): dispatch EventKind::HumanTaskComplete into the running
    // RuntimeEngine for this token_id once in-process runtime is session-wired.
    // For now: accept and log; return the echo so the React side can confirm.
    Json(serde_json::json!({
        "accepted": true,
        "token_id": token_id,
        "submission": payload,
    }))
    .into_response()
}

fn forms_dir() -> PathBuf {
    // Resolve config/forms/ relative to the workspace config root.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let candidates = [
        format!("{}/../../config/forms", manifest_dir),
        format!("{}/../../../config/forms", manifest_dir),
        "config/forms".to_owned(),
    ];
    for c in &candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return p;
        }
    }
    PathBuf::from("config/forms")
}
