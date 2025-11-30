//! DSL Viewer API Routes
//!
//! Endpoints for visualizing persisted agent-generated DSL
//!
//! Routes:
//! - GET /api/dsl/list              - List all DSL instances
//! - GET /api/dsl/show/:ref         - Get latest DSL for business_reference
//! - GET /api/dsl/show/:ref/:ver    - Get specific version
//! - GET /api/dsl/history/:ref      - Get all versions for business_reference

use crate::database::dsl_repository::{DslInstanceSummary, DslRepository};
use crate::dsl_v2::{compile, parse_program};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

// ============================================================================
// Response Types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct DslListResponse {
    pub instances: Vec<DslInstanceSummary>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct DslShowResponse {
    pub business_reference: String,
    pub domain_name: String,
    pub version: i32,
    pub dsl_source: String,
    pub ast_json: Option<serde_json::Value>,
    pub execution_plan: Vec<ExecutionStepInfo>,
    pub compilation_status: String,
    pub created_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ExecutionStepInfo {
    pub step: usize,
    pub verb: String,
    pub bind_as: Option<String>,
    pub injections: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct DslHistoryResponse {
    pub business_reference: String,
    pub versions: Vec<DslVersionSummary>,
}

#[derive(Debug, Serialize)]
pub struct DslVersionSummary {
    pub version: i32,
    pub operation_type: String,
    pub compilation_status: String,
    pub created_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

// ============================================================================
// Query Params
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub limit: Option<i32>,
    pub domain: Option<String>,
}

// ============================================================================
// State
// ============================================================================

#[derive(Clone)]
pub struct DslViewerState {
    pub pool: PgPool,
}

// ============================================================================
// Router
// ============================================================================

pub fn create_dsl_viewer_router(pool: PgPool) -> Router {
    let state = DslViewerState { pool };

    Router::new()
        .route("/api/dsl/list", get(list_instances))
        .route("/api/dsl/show/{business_ref}", get(show_dsl))
        .route(
            "/api/dsl/show/{business_ref}/{version}",
            get(show_dsl_version),
        )
        .route("/api/dsl/history/{business_ref}", get(dsl_history))
        .with_state(state)
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/dsl/list - List all DSL instances
async fn list_instances(
    State(state): State<DslViewerState>,
    Query(params): Query<ListQuery>,
) -> Result<Json<DslListResponse>, StatusCode> {
    let repo = DslRepository::new(state.pool);

    let instances = repo
        .list_instances_for_display(params.limit, params.domain.as_deref())
        .await
        .map_err(|e| {
            tracing::error!("Failed to list DSL instances: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let total = instances.len();

    Ok(Json(DslListResponse { instances, total }))
}

/// GET /api/dsl/show/:business_ref - Get latest DSL for business_reference
async fn show_dsl(
    State(state): State<DslViewerState>,
    Path(business_ref): Path<String>,
) -> Result<Json<DslShowResponse>, StatusCode> {
    show_dsl_internal(state, &business_ref, None).await
}

/// GET /api/dsl/show/:business_ref/:version - Get specific version
async fn show_dsl_version(
    State(state): State<DslViewerState>,
    Path((business_ref, version)): Path<(String, i32)>,
) -> Result<Json<DslShowResponse>, StatusCode> {
    show_dsl_internal(state, &business_ref, Some(version)).await
}

/// Internal implementation for showing DSL
async fn show_dsl_internal(
    state: DslViewerState,
    business_ref: &str,
    version: Option<i32>,
) -> Result<Json<DslShowResponse>, StatusCode> {
    let repo = DslRepository::new(state.pool);

    let display_data = repo
        .get_dsl_for_display(business_ref, version)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get DSL for display: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Compile DSL to get execution plan
    let execution_plan = compute_execution_plan(&display_data.dsl_content);

    Ok(Json(DslShowResponse {
        business_reference: display_data.business_reference,
        domain_name: display_data.domain_name,
        version: display_data.version_number,
        dsl_source: display_data.dsl_content,
        ast_json: display_data.ast_json,
        execution_plan,
        compilation_status: display_data.compilation_status,
        created_at: display_data.created_at.map(|dt| dt.to_rfc3339()),
    }))
}

/// GET /api/dsl/history/:business_ref - Get all versions for business_reference
async fn dsl_history(
    State(state): State<DslViewerState>,
    Path(business_ref): Path<String>,
) -> Result<Json<DslHistoryResponse>, StatusCode> {
    let repo = DslRepository::new(state.pool);

    let versions = repo.get_all_versions(&business_ref).await.map_err(|e| {
        tracing::error!("Failed to get DSL history: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if versions.is_empty() {
        return Err(StatusCode::NOT_FOUND);
    }

    let version_summaries: Vec<DslVersionSummary> = versions
        .into_iter()
        .map(|v| DslVersionSummary {
            version: v.version_number,
            operation_type: v.operation_type,
            compilation_status: v.compilation_status,
            created_at: v.created_at.map(|dt| dt.to_rfc3339()),
        })
        .collect();

    Ok(Json(DslHistoryResponse {
        business_reference: business_ref,
        versions: version_summaries,
    }))
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Compute execution plan from DSL source
/// Returns empty vec if parsing/compilation fails
fn compute_execution_plan(dsl_content: &str) -> Vec<ExecutionStepInfo> {
    // Parse DSL
    let program = match parse_program(dsl_content) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Failed to parse DSL for execution plan: {}", e);
            return vec![];
        }
    };

    // Compile to execution plan
    let plan = match compile(&program) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Failed to compile DSL for execution plan: {}", e);
            return vec![];
        }
    };

    // Map steps to display format
    plan.steps
        .iter()
        .enumerate()
        .map(|(idx, step)| {
            let verb = format!("{}.{}", step.verb_call.domain, step.verb_call.verb);
            let injections: Vec<String> = step
                .injections
                .iter()
                .map(|inj| format!("{} ← ${}", inj.into_arg, inj.from_step))
                .collect();

            ExecutionStepInfo {
                step: idx,
                verb,
                bind_as: step.bind_as.clone(),
                injections,
            }
        })
        .collect()
}
