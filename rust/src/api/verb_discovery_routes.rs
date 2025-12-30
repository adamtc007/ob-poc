//! Verb Discovery API Routes
//!
//! Provides endpoints for RAG-style verb discovery based on:
//! - Natural language intent (full-text search)
//! - Graph context (cursor position, layer)
//! - Workflow phase (KYC lifecycle state)
//! - Recent verb history (category matching, typical next)
//!
//! These endpoints are used by the agent to get contextually relevant
//! verb suggestions during DSL generation.

use crate::session::{
    AgentVerbContext, CategoryInfo, DiscoveryQuery, VerbDiscoveryService, VerbSuggestion,
    WorkflowPhaseInfo,
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;

// ============================================================================
// State
// ============================================================================

#[derive(Clone)]
pub struct VerbDiscoveryState {
    pub service: Arc<VerbDiscoveryService>,
}

impl VerbDiscoveryState {
    pub fn new(pool: PgPool) -> Self {
        Self {
            service: Arc::new(VerbDiscoveryService::new(Arc::new(pool))),
        }
    }
}

// ============================================================================
// Request/Response Types
// ============================================================================

/// Query parameters for verb discovery
#[derive(Debug, Deserialize)]
pub struct DiscoverVerbsQuery {
    /// Natural language query (e.g., "create a person", "add director")
    #[serde(rename = "q")]
    pub query: Option<String>,
    /// Graph context (e.g., "cursor_on_cbu", "layer_ubo", "selected_entity")
    pub graph_context: Option<String>,
    /// Workflow phase (e.g., "intake", "entity_collection", "screening")
    pub workflow_phase: Option<String>,
    /// Comma-separated list of recently used verbs
    pub recent_verbs: Option<String>,
    /// Filter by category
    pub category: Option<String>,
    /// Filter by domain
    pub domain: Option<String>,
    /// Maximum results (default 10)
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    10
}

/// Response containing verb suggestions
#[derive(Debug, Serialize)]
pub struct DiscoverVerbsResponse {
    /// List of suggested verbs with scores and reasons
    pub suggestions: Vec<VerbSuggestion>,
    /// Total count of suggestions before limit
    pub total: usize,
    /// Query parameters used (for debugging)
    pub query_info: QueryInfo,
}

/// Info about the query that was executed
#[derive(Debug, Serialize)]
pub struct QueryInfo {
    pub query_text: Option<String>,
    pub graph_context: Option<String>,
    pub workflow_phase: Option<String>,
    pub recent_verb_count: usize,
    pub limit: usize,
}

/// Response with structured verb context for agent prompts
#[derive(Debug, Serialize)]
pub struct AgentContextResponse {
    /// Grouped verb suggestions
    pub context: AgentVerbContext,
    /// Prompt-ready text representation
    pub prompt_text: String,
    /// Top N verbs across all categories
    pub top_verbs: Vec<VerbSuggestion>,
}

/// Response listing categories
#[derive(Debug, Serialize)]
pub struct CategoriesResponse {
    pub categories: Vec<CategoryInfo>,
}

/// Response listing workflow phases
#[derive(Debug, Serialize)]
pub struct WorkflowPhasesResponse {
    pub phases: Vec<WorkflowPhaseInfo>,
}

/// Query params for category/phase verb lookup
#[derive(Debug, Deserialize)]
pub struct VerbLookupQuery {
    /// Category code or phase code
    pub code: String,
}

/// Response with verbs for a category/phase
#[derive(Debug, Serialize)]
pub struct VerbListResponse {
    pub verbs: Vec<String>,
    pub count: usize,
}

/// Query params for verb example lookup
#[derive(Debug, Deserialize)]
pub struct VerbExampleQuery {
    /// Full verb name (e.g., "cbu.assign-role")
    pub verb: String,
}

/// Response with verb example
#[derive(Debug, Serialize)]
pub struct VerbExampleResponse {
    pub verb: String,
    pub example: Option<String>,
}

// ============================================================================
// Router
// ============================================================================

pub fn create_verb_discovery_router(pool: PgPool) -> Router {
    let state = VerbDiscoveryState::new(pool);

    Router::new()
        // Main discovery endpoint
        .route("/api/verbs/discover", get(discover_verbs))
        // Agent-friendly grouped context
        .route("/api/verbs/agent-context", get(get_agent_context))
        // Reference data
        .route("/api/verbs/categories", get(list_categories))
        .route("/api/verbs/phases", get(list_workflow_phases))
        // Lookup by category/phase
        .route("/api/verbs/by-category", get(get_verbs_by_category))
        .route("/api/verbs/by-phase", get(get_verbs_by_phase))
        // Example lookup
        .route("/api/verbs/example", get(get_verb_example))
        .with_state(state)
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/verbs/discover - Discover relevant verbs based on context
///
/// Query params:
/// - q: Natural language query
/// - graph_context: Current graph context (cursor_on_cbu, layer_ubo, etc.)
/// - workflow_phase: Current KYC workflow phase
/// - recent_verbs: Comma-separated list of recently used verbs
/// - category: Filter by category
/// - domain: Filter by domain
/// - limit: Maximum results (default 10)
async fn discover_verbs(
    State(state): State<VerbDiscoveryState>,
    Query(params): Query<DiscoverVerbsQuery>,
) -> Result<Json<DiscoverVerbsResponse>, StatusCode> {
    // Parse recent verbs from comma-separated string
    let recent_verbs: Vec<String> = params
        .recent_verbs
        .as_ref()
        .map(|s| s.split(',').map(|v| v.trim().to_string()).collect())
        .unwrap_or_default();

    // Build discovery query
    let mut query = DiscoveryQuery::new().with_limit(params.limit);

    if let Some(ref text) = params.query {
        query = query.with_query(text);
    }
    if let Some(ref ctx) = params.graph_context {
        query = query.with_graph_context(ctx);
    }
    if let Some(ref phase) = params.workflow_phase {
        query = query.with_workflow_phase(phase);
    }
    if !recent_verbs.is_empty() {
        query = query.with_recent_verbs(recent_verbs.clone());
    }
    if let Some(ref cat) = params.category {
        query = query.with_category(cat);
    }
    if let Some(ref domain) = params.domain {
        query = query.with_domain(domain);
    }

    // Execute discovery
    let suggestions = state.service.discover(&query).await.map_err(|e| {
        tracing::error!("Verb discovery error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let total = suggestions.len();

    Ok(Json(DiscoverVerbsResponse {
        suggestions,
        total,
        query_info: QueryInfo {
            query_text: params.query,
            graph_context: params.graph_context,
            workflow_phase: params.workflow_phase,
            recent_verb_count: recent_verbs.len(),
            limit: params.limit,
        },
    }))
}

/// GET /api/verbs/agent-context - Get structured verb context for agent prompts
///
/// Returns verb suggestions grouped by:
/// - Intent match (from query text)
/// - Graph context
/// - Workflow phase
/// - Category/typical next
///
/// Also includes a prompt-ready text representation.
async fn get_agent_context(
    State(state): State<VerbDiscoveryState>,
    Query(params): Query<DiscoverVerbsQuery>,
) -> Result<Json<AgentContextResponse>, StatusCode> {
    // Parse recent verbs
    let recent_verbs: Vec<String> = params
        .recent_verbs
        .as_ref()
        .map(|s| s.split(',').map(|v| v.trim().to_string()).collect())
        .unwrap_or_default();

    // Build agent context
    let context = state
        .service
        .build_suggestions_for_agent(
            params.query.as_deref(),
            params.graph_context.as_deref(),
            params.workflow_phase.as_deref(),
            &recent_verbs,
        )
        .await
        .map_err(|e| {
            tracing::error!("Agent context error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let prompt_text = context.to_prompt_text();
    let top_verbs: Vec<VerbSuggestion> = context.top_verbs(5).into_iter().cloned().collect();

    Ok(Json(AgentContextResponse {
        context,
        prompt_text,
        top_verbs,
    }))
}

/// GET /api/verbs/categories - List all verb categories
async fn list_categories(
    State(state): State<VerbDiscoveryState>,
) -> Result<Json<CategoriesResponse>, StatusCode> {
    let categories = state.service.list_categories().await.map_err(|e| {
        tracing::error!("List categories error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(CategoriesResponse { categories }))
}

/// GET /api/verbs/phases - List all workflow phases
async fn list_workflow_phases(
    State(state): State<VerbDiscoveryState>,
) -> Result<Json<WorkflowPhasesResponse>, StatusCode> {
    let phases = state.service.list_workflow_phases().await.map_err(|e| {
        tracing::error!("List phases error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(WorkflowPhasesResponse { phases }))
}

/// GET /api/verbs/by-category - Get verbs for a category
async fn get_verbs_by_category(
    State(state): State<VerbDiscoveryState>,
    Query(params): Query<VerbLookupQuery>,
) -> Result<Json<VerbListResponse>, StatusCode> {
    let verbs = state
        .service
        .get_category_verbs(&params.code)
        .await
        .map_err(|e| {
            tracing::error!("Get category verbs error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let count = verbs.len();
    Ok(Json(VerbListResponse { verbs, count }))
}

/// GET /api/verbs/by-phase - Get verbs for a workflow phase
async fn get_verbs_by_phase(
    State(state): State<VerbDiscoveryState>,
    Query(params): Query<VerbLookupQuery>,
) -> Result<Json<VerbListResponse>, StatusCode> {
    let verbs = state
        .service
        .get_phase_verbs(&params.code)
        .await
        .map_err(|e| {
            tracing::error!("Get phase verbs error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let count = verbs.len();
    Ok(Json(VerbListResponse { verbs, count }))
}

/// GET /api/verbs/example - Get example DSL for a verb
async fn get_verb_example(
    State(state): State<VerbDiscoveryState>,
    Query(params): Query<VerbExampleQuery>,
) -> Result<Json<VerbExampleResponse>, StatusCode> {
    let example = state
        .service
        .get_verb_example(&params.verb)
        .await
        .map_err(|e| {
            tracing::error!("Get verb example error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(VerbExampleResponse {
        verb: params.verb,
        example,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_recent_verbs() {
        let params = DiscoverVerbsQuery {
            query: None,
            graph_context: None,
            workflow_phase: None,
            recent_verbs: Some("cbu.ensure, entity.create-proper-person, cbu.assign-role".into()),
            category: None,
            domain: None,
            limit: 10,
        };

        let recent_verbs: Vec<String> = params
            .recent_verbs
            .as_ref()
            .map(|s| s.split(',').map(|v| v.trim().to_string()).collect())
            .unwrap_or_default();

        assert_eq!(recent_verbs.len(), 3);
        assert_eq!(recent_verbs[0], "cbu.ensure");
        assert_eq!(recent_verbs[1], "entity.create-proper-person");
        assert_eq!(recent_verbs[2], "cbu.assign-role");
    }
}
