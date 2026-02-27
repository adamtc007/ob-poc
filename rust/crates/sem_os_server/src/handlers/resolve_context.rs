//! POST /resolve_context — run the 12-step context resolution pipeline.

use std::sync::Arc;

use axum::{extract::Extension, Json};
use sem_os_core::{
    context_resolution::ContextResolutionResponse, principal::Principal,
    proto::ResolveContextRequest, service::CoreService,
};

use crate::error::AppError;

pub async fn resolve_context(
    Extension(principal): Extension<Principal>,
    Extension(service): Extension<Arc<dyn CoreService>>,
    Json(req): Json<ResolveContextRequest>,
) -> Result<Json<ContextResolutionResponse>, AppError> {
    let resp = service.resolve_context(&principal, req).await?;
    Ok(Json(resp))
}
