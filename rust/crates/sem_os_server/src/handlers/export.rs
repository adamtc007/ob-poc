//! GET /exports/snapshot_set/:id — export all snapshots in a set.

use std::sync::Arc;

use axum::{extract::Path, Extension, Json};
use sem_os_core::{proto::ExportSnapshotSetResponse, service::CoreService};

use crate::error::AppError;

pub async fn export_snapshot_set(
    Extension(service): Extension<Arc<dyn CoreService>>,
    Path(id): Path<String>,
) -> Result<Json<ExportSnapshotSetResponse>, AppError> {
    let resp = service.export_snapshot_set(&id).await?;
    Ok(Json(resp))
}
