//! Authoring pipeline handlers — governance verb HTTP endpoints.
//!
//! POST /authoring/propose              — propose a new ChangeSet from a bundle
//! POST /authoring/:id/validate         — run Stage 1 validation
//! POST /authoring/:id/dry-run          — run Stage 2 dry-run
//! GET  /authoring/:id/plan             — generate publish plan (read-only)
//! POST /authoring/:id/publish          — publish a ChangeSet
//! POST /authoring/publish-batch        — batch publish in topological order
//! POST /authoring/diff                 — diff two ChangeSets
//! GET  /authoring                      — list ChangeSets
//! GET  /authoring/:id                  — get a single ChangeSet

use std::sync::Arc;

use axum::{
    extract::{Path, Query},
    Extension, Json,
};
use sem_os_core::{
    authoring::{
        bundle::{build_bundle_from_map, parse_manifest},
        types::ChangeSetStatus as AuthoringChangeSetStatus,
    },
    principal::Principal,
    service::CoreService,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::AppError;

// ── Request types ────────────────────────────────────────────

/// Request body for `POST /authoring/propose`.
#[derive(Debug, Deserialize)]
pub struct ProposeRequest {
    /// Raw YAML manifest content (changeset.yaml).
    pub manifest_yaml: String,
    /// Map of artifact path → content.
    pub artifacts: std::collections::HashMap<String, String>,
}

/// Request body for `POST /authoring/publish-batch`.
#[derive(Debug, Deserialize)]
pub struct PublishBatchRequest {
    pub change_set_ids: Vec<Uuid>,
    pub publisher: String,
}

/// Request body for `POST /authoring/:id/publish`.
#[derive(Debug, Deserialize)]
pub struct PublishRequest {
    pub publisher: String,
}

/// Request body for `POST /authoring/diff`.
#[derive(Debug, Deserialize)]
pub struct DiffRequest {
    pub base_id: Uuid,
    pub target_id: Uuid,
}

/// Query parameters for `GET /authoring`.
#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub status: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    100
}

// ── Handlers ─────────────────────────────────────────────────

/// Propose a new ChangeSet from a bundle manifest + artifacts.
pub async fn propose(
    Extension(principal): Extension<Principal>,
    Extension(service): Extension<Arc<dyn CoreService>>,
    Json(body): Json<ProposeRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let raw = parse_manifest(&body.manifest_yaml)
        .map_err(|e| sem_os_core::error::SemOsError::InvalidInput(e.to_string()))?;
    let bundle = build_bundle_from_map(&raw, &body.artifacts)
        .map_err(|e| sem_os_core::error::SemOsError::InvalidInput(e.to_string()))?;

    let cs = service.authoring_propose(&principal, &bundle).await?;
    let json = serde_json::to_value(&cs)
        .map_err(|e| sem_os_core::error::SemOsError::Internal(e.into()))?;
    Ok(Json(json))
}

/// Run Stage 1 (pure) validation on a ChangeSet.
pub async fn validate(
    Extension(service): Extension<Arc<dyn CoreService>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let cs_id = parse_uuid(&id)?;
    let report = service.authoring_validate(cs_id).await?;
    let json = serde_json::to_value(&report)
        .map_err(|e| sem_os_core::error::SemOsError::Internal(e.into()))?;
    Ok(Json(json))
}

/// Run Stage 2 (DB-backed) dry-run on a ChangeSet.
pub async fn dry_run(
    Extension(service): Extension<Arc<dyn CoreService>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let cs_id = parse_uuid(&id)?;
    let report = service.authoring_dry_run(cs_id).await?;
    let json = serde_json::to_value(&report)
        .map_err(|e| sem_os_core::error::SemOsError::Internal(e.into()))?;
    Ok(Json(json))
}

/// Generate a publish plan with blast-radius analysis. Read-only.
pub async fn plan_publish(
    Extension(service): Extension<Arc<dyn CoreService>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let cs_id = parse_uuid(&id)?;
    let summary = service.authoring_plan_publish(cs_id).await?;
    let json = serde_json::to_value(&summary)
        .map_err(|e| sem_os_core::error::SemOsError::Internal(e.into()))?;
    Ok(Json(json))
}

/// Publish a ChangeSet. Transitions DryRunPassed → Published.
/// Requires Governed mode + admin role. Publisher is always the authenticated principal.
pub async fn publish(
    Extension(principal): Extension<Principal>,
    Extension(service): Extension<Arc<dyn CoreService>>,
    Path(id): Path<String>,
    Json(_body): Json<PublishRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_publish_permission(&principal)?;
    let cs_id = parse_uuid(&id)?;
    let batch = service
        .authoring_publish(cs_id, &principal.actor_id)
        .await?;
    let json = serde_json::to_value(&batch)
        .map_err(|e| sem_os_core::error::SemOsError::Internal(e.into()))?;
    Ok(Json(json))
}

/// Publish multiple ChangeSets atomically in topological order.
/// Requires Governed mode + admin role. Publisher is always the authenticated principal.
pub async fn publish_batch(
    Extension(principal): Extension<Principal>,
    Extension(service): Extension<Arc<dyn CoreService>>,
    Json(body): Json<PublishBatchRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_publish_permission(&principal)?;
    let batch = service
        .authoring_publish_batch(&body.change_set_ids, &principal.actor_id)
        .await?;
    let json = serde_json::to_value(&batch)
        .map_err(|e| sem_os_core::error::SemOsError::Internal(e.into()))?;
    Ok(Json(json))
}

/// Compute structural diff between two ChangeSets.
pub async fn diff(
    Extension(service): Extension<Arc<dyn CoreService>>,
    Json(body): Json<DiffRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let summary = service.authoring_diff(body.base_id, body.target_id).await?;
    let json = serde_json::to_value(&summary)
        .map_err(|e| sem_os_core::error::SemOsError::Internal(e.into()))?;
    Ok(Json(json))
}

/// List ChangeSets with optional status filter.
pub async fn list(
    Extension(service): Extension<Arc<dyn CoreService>>,
    Query(query): Query<ListQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let status = query
        .status
        .as_deref()
        .and_then(AuthoringChangeSetStatus::parse);

    let changesets = service.authoring_list(status, query.limit).await?;
    let json = serde_json::to_value(&changesets)
        .map_err(|e| sem_os_core::error::SemOsError::Internal(e.into()))?;
    Ok(Json(json))
}

/// Get a single ChangeSet by ID.
pub async fn get(
    Extension(service): Extension<Arc<dyn CoreService>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let cs_id = parse_uuid(&id)?;
    let cs = service.authoring_get(cs_id).await?;
    let json = serde_json::to_value(&cs)
        .map_err(|e| sem_os_core::error::SemOsError::Internal(e.into()))?;
    Ok(Json(json))
}

// ── Helpers ──────────────────────────────────────────────────

fn parse_uuid(s: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(s)
        .map_err(|_| sem_os_core::error::SemOsError::InvalidInput(format!("invalid UUID: {s}")))
        .map_err(AppError::from)
}

/// Require Governed mode + admin role for publish operations.
/// Research mode agents cannot publish. Non-admin users cannot publish.
fn require_publish_permission(principal: &Principal) -> Result<(), AppError> {
    let mode = principal.agent_mode();
    if !matches!(
        mode,
        sem_os_core::authoring::agent_mode::AgentMode::Governed
    ) {
        return Err(AppError::from(
            sem_os_core::error::SemOsError::Unauthorized(format!(
                "blocked by AgentMode: {} cannot publish",
                mode.as_str()
            )),
        ));
    }
    principal.require_admin().map_err(AppError::from)
}
