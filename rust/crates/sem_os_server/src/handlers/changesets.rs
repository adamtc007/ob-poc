//! Changeset / Workbench handlers.
//!
//! GET  /changesets                    — list changesets (optional query filters)
//! GET  /changesets/:id/diff           — diff entries vs current active
//! GET  /changesets/:id/impact         — downstream impact analysis
//! POST /changesets/:id/gate_preview   — run publish gates (dry-run)
//! POST /changesets/:id/publish        — promote approved changeset

use std::sync::Arc;

use axum::{
    extract::{Path, Query},
    Extension, Json,
};
use sem_os_core::{principal::Principal, proto::ListChangesetsQuery, service::CoreService};

use crate::error::AppError;

pub async fn list_changesets(
    Extension(service): Extension<Arc<dyn CoreService>>,
    Query(query): Query<ListChangesetsQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let resp = service.list_changesets(query).await?;
    let json = serde_json::to_value(&resp)
        .map_err(|e| sem_os_core::error::SemOsError::Internal(e.into()))?;
    Ok(Json(json))
}

pub async fn changeset_diff(
    Extension(service): Extension<Arc<dyn CoreService>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let resp = service.changeset_diff(&id).await?;
    let json = serde_json::to_value(&resp)
        .map_err(|e| sem_os_core::error::SemOsError::Internal(e.into()))?;
    Ok(Json(json))
}

pub async fn changeset_impact(
    Extension(service): Extension<Arc<dyn CoreService>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let resp = service.changeset_impact(&id).await?;
    let json = serde_json::to_value(&resp)
        .map_err(|e| sem_os_core::error::SemOsError::Internal(e.into()))?;
    Ok(Json(json))
}

pub async fn changeset_gate_preview(
    Extension(service): Extension<Arc<dyn CoreService>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let resp = service.changeset_gate_preview(&id).await?;
    let json = serde_json::to_value(&resp)
        .map_err(|e| sem_os_core::error::SemOsError::Internal(e.into()))?;
    Ok(Json(json))
}

pub async fn publish_changeset(
    Extension(principal): Extension<Principal>,
    Extension(service): Extension<Arc<dyn CoreService>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    principal.require_admin()?;
    let count = service.promote_changeset(&principal, &id).await?;
    let resp = sem_os_core::proto::ChangesetPublishResponse {
        changeset_id: id,
        snapshots_created: count,
        snapshot_set_id: String::new(),
    };
    let json = serde_json::to_value(&resp)
        .map_err(|e| sem_os_core::error::SemOsError::Internal(e.into()))?;
    Ok(Json(json))
}
