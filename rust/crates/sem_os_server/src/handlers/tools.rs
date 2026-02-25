//! Tool dispatch endpoints:
//!   POST /tools/call  — invoke a named MCP tool
//!   GET  /tools/list  — list available tool specifications

use std::sync::Arc;

use axum::{extract::Extension, Json};
use sem_os_core::{
    principal::Principal,
    proto::{ListToolSpecsResponse, ToolCallRequest, ToolCallResponse},
    service::CoreService,
};

use crate::error::AppError;

/// POST /tools/call — dispatch a tool invocation through CoreService.
pub async fn call_tool(
    Extension(principal): Extension<Principal>,
    Extension(service): Extension<Arc<dyn CoreService>>,
    Json(req): Json<ToolCallRequest>,
) -> Result<Json<ToolCallResponse>, AppError> {
    let resp = service.dispatch_tool(&principal, req).await?;
    Ok(Json(resp))
}

/// GET /tools/list — return the list of available tool specifications.
pub async fn list_tools(
    Extension(service): Extension<Arc<dyn CoreService>>,
) -> Result<Json<ListToolSpecsResponse>, AppError> {
    let resp = service.list_tool_specs().await?;
    Ok(Json(resp))
}
