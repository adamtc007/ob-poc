//! Health check endpoints.
//!
//! - `GET /health` — basic liveness
//! - `GET /health/semreg/pending-changesets` — pending ChangeSet counts
//! - `GET /health/semreg/stale-dryruns` — stale dry-run detection

use std::sync::Arc;

use axum::{Extension, Json};
use sem_os_core::service::CoreService;
use serde_json::{json, Value};

use crate::error::AppError;

/// Basic liveness check.
pub async fn health() -> Json<Value> {
    Json(json!({"status": "ok"}))
}

/// Pending ChangeSets grouped by status.
pub async fn semreg_pending_changesets(
    Extension(service): Extension<Arc<dyn CoreService>>,
) -> Result<Json<Value>, AppError> {
    let health = service.authoring_health_pending().await?;
    Ok(Json(serde_json::to_value(health).unwrap_or_default()))
}

/// ChangeSets with stale dry-run evaluations.
pub async fn semreg_stale_dryruns(
    Extension(service): Extension<Arc<dyn CoreService>>,
) -> Result<Json<Value>, AppError> {
    let health = service.authoring_health_stale_dryruns().await?;
    Ok(Json(serde_json::to_value(health).unwrap_or_default()))
}
