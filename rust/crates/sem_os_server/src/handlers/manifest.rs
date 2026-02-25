//! GET /snapshot_sets/:id/manifest — get the manifest of a snapshot set.

use std::sync::Arc;

use axum::{extract::Path, Extension, Json};
use sem_os_core::service::CoreService;

use crate::error::AppError;

pub async fn get_manifest(
    Extension(service): Extension<Arc<dyn CoreService>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let resp = service.get_manifest(&id).await?;
    let json = serde_json::to_value(&resp)
        .map_err(|e| sem_os_core::error::SemOsError::Internal(e.into()))?;
    Ok(Json(json))
}
