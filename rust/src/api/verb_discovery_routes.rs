//! Verb Discovery API Routes
//!
//! Provides endpoints for RAG-style verb discovery based on:
//! - Natural language intent (full-text search)
//! - Graph context (cursor position, layer)
//! - Workflow phase (KYC lifecycle state)
//! - Recent verb history (category matching, typical next)
//!
//! Also provides:
//! - Macro taxonomy endpoint for verb picker UI
//! - Macro schema endpoint for form generation
//!
//! These endpoints are used by the agent to get contextually relevant
//! verb suggestions during DSL generation.

use crate::dsl_v2::verb_taxonomy::{verb_taxonomy, DomainSummary, VerbLocation};
use crate::macros::{MacroFilter, MacroTaxonomy, OperatorMacroRegistry};
use crate::session::{
    AgentVerbContext, CategoryInfo, DiscoveryQuery, VerbDiscoveryService, VerbSuggestion,
    WorkflowPhaseInfo,
};
use axum::{
    extract::{Path, Query, State},
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
    pub macro_registry: Arc<OperatorMacroRegistry>,
}

impl VerbDiscoveryState {
    pub fn new(pool: PgPool) -> Self {
        // Load macro registry from config directory
        let config_dir = std::env::var("DSL_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let macros_dir = std::path::Path::new(&config_dir).join("verb_schemas/macros");

        let macro_registry =
            OperatorMacroRegistry::load_from_dir(&macros_dir).unwrap_or_else(|e| {
                tracing::warn!("Failed to load operator macros: {}", e);
                OperatorMacroRegistry::new()
            });

        Self {
            service: Arc::new(VerbDiscoveryService::new(Arc::new(pool))),
            macro_registry: Arc::new(macro_registry),
        }
    }

    pub fn with_macro_registry(pool: PgPool, registry: OperatorMacroRegistry) -> Self {
        Self {
            service: Arc::new(VerbDiscoveryService::new(Arc::new(pool))),
            macro_registry: Arc::new(registry),
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
// Macro Taxonomy Types (P2/P3)
// ============================================================================

/// Query parameters for macro taxonomy
#[derive(Debug, Deserialize)]
pub struct MacroTaxonomyQuery {
    /// Filter by mode tag (e.g., "onboarding", "kyc", "trading")
    pub mode_tag: Option<String>,
    /// Filter by domain (e.g., "structure", "case", "mandate")
    pub domain: Option<String>,
    /// Search term (searches FQN, label, description)
    pub search: Option<String>,
}

/// Response containing macro taxonomy tree
#[derive(Debug, Serialize)]
pub struct MacroTaxonomyResponse {
    /// Taxonomy tree organized by domain
    pub taxonomy: MacroTaxonomy,
    /// Total count of macros
    pub total_macros: usize,
    /// Available domains
    pub domains: Vec<String>,
    /// Available mode tags
    pub mode_tags: Vec<String>,
}

/// Response containing a single macro schema
#[derive(Debug, Serialize)]
pub struct MacroSchemaResponse {
    /// Fully qualified name
    pub fqn: String,
    /// UI metadata
    pub ui: MacroSchemaUi,
    /// Routing info
    pub routing: MacroSchemaRouting,
    /// Arguments
    pub args: MacroSchemaArgs,
    /// Prerequisites
    pub prereqs: Vec<MacroSchemaPrereq>,
    /// Macros unlocked after this one
    pub unlocks: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct MacroSchemaUi {
    pub label: String,
    pub description: String,
    pub target_label: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MacroSchemaRouting {
    pub mode_tags: Vec<String>,
    pub operator_domain: String,
}

#[derive(Debug, Serialize)]
pub struct MacroSchemaArgs {
    pub style: String,
    pub required: Vec<MacroSchemaArg>,
    pub optional: Vec<MacroSchemaArg>,
}

#[derive(Debug, Serialize)]
pub struct MacroSchemaArg {
    pub name: String,
    pub arg_type: String,
    pub ui_label: String,
    pub required: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub valid_values: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub enum_values: Vec<MacroSchemaEnumValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autofill_from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picker: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MacroSchemaEnumValue {
    pub key: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub internal: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MacroSchemaPrereq {
    #[serde(rename = "type")]
    pub prereq_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verb: Option<String>,
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
        // Macro taxonomy (P2) - for verb picker UI
        .route("/api/verbs/taxonomy", get(get_macro_taxonomy))
        // Domain taxonomy - hierarchical verb organization
        .route("/api/verbs/domains", get(get_domain_list))
        .route("/api/verbs/domains/:domain_id", get(get_domain_detail))
        .route("/api/verbs/:fqn/location", get(get_verb_location))
        // Macro schema (P3) - for form generation
        .route("/api/verbs/:fqn/schema", get(get_macro_schema))
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

/// GET /api/verbs/taxonomy - Get macro taxonomy tree for verb picker UI
///
/// Query params:
/// - mode_tag: Filter by mode tag (e.g., "onboarding", "kyc", "trading")
/// - domain: Filter by domain (e.g., "structure", "case", "mandate")
/// - search: Search term (searches FQN, label, description)
///
/// Returns a tree structure organized by domain for rendering in the verb picker.
async fn get_macro_taxonomy(
    State(state): State<VerbDiscoveryState>,
    Query(params): Query<MacroTaxonomyQuery>,
) -> Result<Json<MacroTaxonomyResponse>, StatusCode> {
    let registry = &state.macro_registry;

    // Build filter from query params
    let filter = if params.mode_tag.is_some() || params.domain.is_some() || params.search.is_some()
    {
        Some(MacroFilter {
            mode_tag: params.mode_tag,
            domain: params.domain,
            search: params.search,
        })
    } else {
        None
    };

    // Get filtered macros
    let macros = registry.list(filter);
    let total_macros = macros.len();

    // Build taxonomy tree
    let taxonomy = registry.build_taxonomy();

    // Get available domains and mode tags
    let domains: Vec<String> = registry.domains().iter().map(|s| s.to_string()).collect();
    let mode_tags: Vec<String> = registry.mode_tags().iter().map(|s| s.to_string()).collect();

    Ok(Json(MacroTaxonomyResponse {
        taxonomy,
        total_macros,
        domains,
        mode_tags,
    }))
}

/// GET /api/verbs/:fqn/schema - Get schema for a specific macro
///
/// Path params:
/// - fqn: Fully qualified name (e.g., "structure.setup")
///
/// Returns the full schema including UI metadata, arguments, prerequisites,
/// suitable for generating a form in the verb picker.
async fn get_macro_schema(
    State(state): State<VerbDiscoveryState>,
    Path(fqn): Path<String>,
) -> Result<Json<MacroSchemaResponse>, StatusCode> {
    let registry = &state.macro_registry;

    let macro_def = registry.get(&fqn).ok_or_else(|| {
        tracing::warn!("Macro not found: {}", fqn);
        StatusCode::NOT_FOUND
    })?;

    // Convert to response format
    let response = MacroSchemaResponse {
        fqn: macro_def.fqn.clone(),
        ui: MacroSchemaUi {
            label: macro_def.ui.label.clone(),
            description: macro_def.ui.description.clone(),
            target_label: macro_def.ui.target_label.clone(),
        },
        routing: MacroSchemaRouting {
            mode_tags: macro_def.routing.mode_tags.clone(),
            operator_domain: macro_def.routing.operator_domain.clone(),
        },
        args: MacroSchemaArgs {
            style: macro_def.args.style.clone(),
            required: macro_def
                .args
                .required
                .iter()
                .map(|(name, arg)| MacroSchemaArg {
                    name: name.clone(),
                    arg_type: arg.arg_type.clone(),
                    ui_label: arg.ui_label.clone(),
                    required: true,
                    valid_values: arg.valid_values.clone(),
                    enum_values: arg
                        .values
                        .iter()
                        .map(|v| MacroSchemaEnumValue {
                            key: v.key.clone(),
                            label: v.label.clone(),
                            internal: v.internal.clone(),
                        })
                        .collect(),
                    default_value: arg.default_key.clone().or_else(|| arg.default.clone()),
                    autofill_from: arg.autofill_from.clone(),
                    picker: arg.picker.clone(),
                })
                .collect(),
            optional: macro_def
                .args
                .optional
                .iter()
                .map(|(name, arg)| MacroSchemaArg {
                    name: name.clone(),
                    arg_type: arg.arg_type.clone(),
                    ui_label: arg.ui_label.clone(),
                    required: false,
                    valid_values: arg.valid_values.clone(),
                    enum_values: arg
                        .values
                        .iter()
                        .map(|v| MacroSchemaEnumValue {
                            key: v.key.clone(),
                            label: v.label.clone(),
                            internal: v.internal.clone(),
                        })
                        .collect(),
                    default_value: arg.default_key.clone().or_else(|| arg.default.clone()),
                    autofill_from: arg.autofill_from.clone(),
                    picker: arg.picker.clone(),
                })
                .collect(),
        },
        prereqs: macro_def
            .prereqs
            .iter()
            .map(|p| match p {
                crate::macros::MacroPrereq::StateExists { key } => MacroSchemaPrereq {
                    prereq_type: "state_exists".to_string(),
                    key: Some(key.clone()),
                    verb: None,
                },
                crate::macros::MacroPrereq::VerbCompleted { verb } => MacroSchemaPrereq {
                    prereq_type: "verb_completed".to_string(),
                    key: None,
                    verb: Some(verb.clone()),
                },
                crate::macros::MacroPrereq::AnyOf { .. } => MacroSchemaPrereq {
                    prereq_type: "any_of".to_string(),
                    key: None,
                    verb: None,
                },
            })
            .collect(),
        unlocks: macro_def.unlocks.clone(),
    };

    Ok(Json(response))
}

// ============================================================================
// Domain Taxonomy Handlers
// ============================================================================

/// GET /api/verbs/domains - List all domains with summaries
///
/// Returns a list of domains sorted by priority, suitable for the verb picker
/// domain selector UI.
async fn get_domain_list(
    State(_state): State<VerbDiscoveryState>,
) -> Result<Json<DomainListResponse>, StatusCode> {
    let taxonomy = verb_taxonomy();
    let domains = taxonomy.domain_list();

    Ok(Json(DomainListResponse {
        domains,
        total: taxonomy.domains.len(),
    }))
}

/// Response for domain list
#[derive(Debug, Serialize)]
pub struct DomainListResponse {
    pub domains: Vec<DomainSummary>,
    pub total: usize,
}

/// GET /api/verbs/domains/:domain_id - Get domain detail with categories
///
/// Returns the full domain structure including categories and verbs,
/// for drill-down in the verb picker.
async fn get_domain_detail(
    State(_state): State<VerbDiscoveryState>,
    Path(domain_id): Path<String>,
) -> Result<Json<DomainDetailResponse>, StatusCode> {
    let taxonomy = verb_taxonomy();

    let domain = taxonomy.get_domain(&domain_id).ok_or_else(|| {
        tracing::warn!("Domain not found: {}", domain_id);
        StatusCode::NOT_FOUND
    })?;

    Ok(Json(DomainDetailResponse {
        id: domain.id.clone(),
        label: domain.label.clone(),
        description: domain.description.clone(),
        icon: domain.icon.clone(),
        categories: domain
            .categories
            .iter()
            .map(|c| CategoryDetail {
                id: c.id.clone(),
                label: c.label.clone(),
                description: c.description.clone(),
                verbs: c.verbs.clone(),
                verb_count: c.verbs.len(),
            })
            .collect(),
    }))
}

/// Response for domain detail
#[derive(Debug, Serialize)]
pub struct DomainDetailResponse {
    pub id: String,
    pub label: String,
    pub description: String,
    pub icon: Option<String>,
    pub categories: Vec<CategoryDetail>,
}

/// Category with verbs
#[derive(Debug, Serialize)]
pub struct CategoryDetail {
    pub id: String,
    pub label: String,
    pub description: String,
    pub verbs: Vec<String>,
    pub verb_count: usize,
}

/// GET /api/verbs/:fqn/location - Get location of a verb in taxonomy
///
/// Returns the domain and category containing this verb, for breadcrumb
/// display in the verb picker.
async fn get_verb_location(
    State(_state): State<VerbDiscoveryState>,
    Path(fqn): Path<String>,
) -> Result<Json<VerbLocationResponse>, StatusCode> {
    let taxonomy = verb_taxonomy();

    let location = taxonomy.location_for_verb(&fqn).ok_or_else(|| {
        tracing::debug!("Verb not in taxonomy: {}", fqn);
        StatusCode::NOT_FOUND
    })?;

    Ok(Json(VerbLocationResponse { fqn, location }))
}

/// Response for verb location
#[derive(Debug, Serialize)]
pub struct VerbLocationResponse {
    pub fqn: String,
    pub location: VerbLocation,
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
