//! POST /resolve_context — run the 12-step context resolution pipeline.

use std::sync::Arc;

use axum::{extract::Extension, Json};
use sem_os_core::principal::Principal;
use sem_os_policy::{
    context_resolution::{ContextResolutionRequest, ContextResolutionResponse},
    service::CoreService,
};

use crate::error::AppError;

pub(crate) async fn resolve_context(
    Extension(principal): Extension<Principal>,
    Extension(service): Extension<Arc<dyn CoreService>>,
    Json(req): Json<ContextResolutionRequest>,
) -> Result<Json<ContextResolutionResponse>, AppError> {
    let resp = service.resolve_context(&principal, req).await?;
    Ok(Json(resp))
}
