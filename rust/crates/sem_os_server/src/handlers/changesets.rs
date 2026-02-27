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
use sem_os_core::{
    principal::Principal,
    proto::{
        ChangesetDiffResponse, ChangesetImpactResponse, ChangesetPublishResponse,
        GatePreviewResponse, ListChangesetsQuery, ListChangesetsResponse,
    },
    service::CoreService,
};

use crate::error::AppError;

pub async fn list_changesets(
    Extension(service): Extension<Arc<dyn CoreService>>,
    Query(query): Query<ListChangesetsQuery>,
) -> Result<Json<ListChangesetsResponse>, AppError> {
    let resp = service.list_changesets(query).await?;
    Ok(Json(resp))
}

pub async fn changeset_diff(
    Extension(service): Extension<Arc<dyn CoreService>>,
    Path(id): Path<String>,
) -> Result<Json<ChangesetDiffResponse>, AppError> {
    let resp = service.changeset_diff(&id).await?;
    Ok(Json(resp))
}

pub async fn changeset_impact(
    Extension(service): Extension<Arc<dyn CoreService>>,
    Path(id): Path<String>,
) -> Result<Json<ChangesetImpactResponse>, AppError> {
    let resp = service.changeset_impact(&id).await?;
    Ok(Json(resp))
}

pub async fn changeset_gate_preview(
    Extension(service): Extension<Arc<dyn CoreService>>,
    Path(id): Path<String>,
) -> Result<Json<GatePreviewResponse>, AppError> {
    let resp = service.changeset_gate_preview(&id).await?;
    Ok(Json(resp))
}

pub async fn publish_changeset(
    Extension(principal): Extension<Principal>,
    Extension(service): Extension<Arc<dyn CoreService>>,
    Path(id): Path<String>,
) -> Result<Json<ChangesetPublishResponse>, AppError> {
    principal.require_admin()?;
    let count = service.promote_changeset(&principal, &id).await?;
    let resp = ChangesetPublishResponse {
        changeset_id: id,
        snapshots_created: count,
        snapshot_set_id: String::new(),
    };
    Ok(Json(resp))
}
