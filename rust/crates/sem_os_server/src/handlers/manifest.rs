//! GET /snapshot_sets/:id/manifest — get the manifest of a snapshot set.

use std::sync::Arc;

use axum::{extract::Path, Extension, Json};
use sem_os_core::{proto::GetManifestResponse, service::CoreService};

use crate::error::AppError;

pub async fn get_manifest(
    Extension(service): Extension<Arc<dyn CoreService>>,
    Path(id): Path<String>,
) -> Result<Json<GetManifestResponse>, AppError> {
    let resp = service.get_manifest(&id).await?;
    Ok(Json(resp))
}
