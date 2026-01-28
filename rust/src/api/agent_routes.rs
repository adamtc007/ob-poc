//! REST API routes for DSL v2 execution
//!
//! Session endpoints:
//! - POST   /api/session             - Create new session
//! - GET    /api/session/:id         - Get session state
//! - DELETE /api/session/:id         - Delete session
//! - POST   /api/session/:id/execute - Execute DSL
//! - POST   /api/session/:id/clear   - Clear session
//! - GET    /api/session/:id/context - Get session context (CBU, linked entities, symbols)
//!
//! Vocabulary endpoints:
//! - GET    /api/agent/domains      - List available DSL domains
//! - GET    /api/agent/vocabulary   - Get vocabulary for a domain
//! - GET    /api/agent/health       - Health check
//!
//! Onboarding endpoints:
//! - POST   /api/agent/onboard           - Generate onboarding DSL from natural language
//! - GET    /api/agent/onboard/templates - List available onboarding templates
//! - POST   /api/agent/onboard/render    - Render an onboarding template with parameters

use crate::api::agent_service::ChatRequest;
use crate::api::session::{
    CreateSessionRequest, CreateSessionResponse, ExecuteResponse, ExecutionResult,
    SessionStateResponse, SessionStore,
};
// Use unified session types - single source of truth
use crate::session::{
    MessageRole, ResearchSubSession, ResolutionSubSession, ReviewStatus, ReviewSubSession,
    SessionState, SubSessionType, UnifiedSession, UnresolvedRefInfo,
};

// API types - SINGLE SOURCE OF TRUTH for HTTP boundary
use crate::database::derive_semantic_state;
use crate::database::generation_log_repository::{
    CompileResult, ExecutionStatus, GenerationAttempt, GenerationLogRepository, LintResult,
    ParseResult,
};
use crate::dsl_v2::{
    compile, expand_templates_simple, parse_program, runtime_registry, verb_registry::registry,
    AtomicExecutionResult, BatchPolicy, BestEffortExecutionResult, DslExecutor, ExecutionContext,
    ExecutionResult as DslV2Result, SemanticValidator,
};
use crate::ontology::SemanticStageRegistry;
use ob_poc_types::{
    resolution::{
        DiscriminatorField as ApiDiscriminatorField, RefContext,
        ResolutionModeHint as ApiResolutionModeHint, ReviewRequirement,
        SearchKeyField as ApiSearchKeyField, SearchKeyFieldType,
        UnresolvedRefResponse as ApiUnresolvedRefResponse,
    },
    AstArgument, AstSpan, AstStatement, AstValue, BoundEntityInfo, ChatResponse, DslState,
    DslValidation, SessionStateEnum, ValidationError as ApiValidationError, VerbIntentInfo,
};

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

// ============================================================================
// Type Converters - Internal types to API types (ob_poc_types)
// ============================================================================

/// Convert internal SessionState to API SessionStateEnum (unified version)
fn to_session_state_enum(state: &SessionState) -> SessionStateEnum {
    match state {
        SessionState::New => SessionStateEnum::New,
        SessionState::Scoped => SessionStateEnum::Scoped,
        SessionState::PendingValidation => SessionStateEnum::PendingValidation,
        SessionState::ReadyToExecute => SessionStateEnum::ReadyToExecute,
        SessionState::Executing => SessionStateEnum::Executing,
        SessionState::Executed => SessionStateEnum::Executed,
        SessionState::Closed => SessionStateEnum::Executed, // Map closed to executed for API
    }
}

/// Convert api::session::SessionState to API SessionStateEnum (for backward compat)
fn api_session_state_to_enum(state: &crate::session::SessionState) -> SessionStateEnum {
    match state {
        crate::session::SessionState::New => SessionStateEnum::New,
        crate::session::SessionState::Scoped => SessionStateEnum::Scoped,
        crate::session::SessionState::PendingValidation => SessionStateEnum::PendingValidation,
        crate::session::SessionState::ReadyToExecute => SessionStateEnum::ReadyToExecute,
        crate::session::SessionState::Executing => SessionStateEnum::Executing,
        crate::session::SessionState::Executed => SessionStateEnum::Executed,
        crate::session::SessionState::Closed => SessionStateEnum::Executed,
    }
}

/// Convert dsl_core::Span to API AstSpan
fn to_api_span(span: &dsl_core::Span) -> AstSpan {
    AstSpan {
        start: span.start,
        end: span.end,
        start_line: None,
        end_line: None,
    }
}

/// Convert internal DisambiguationRequest to API DisambiguationRequest
/// Server uses Uuid, client uses String for JSON compatibility
fn to_api_disambiguation_request(
    req: &crate::api::session::DisambiguationRequest,
) -> ob_poc_types::DisambiguationRequest {
    ob_poc_types::DisambiguationRequest {
        request_id: req.request_id.to_string(),
        items: req.items.iter().map(to_api_disambiguation_item).collect(),
        prompt: req.prompt.clone(),
    }
}

/// Convert internal DisambiguationItem to API DisambiguationItem
fn to_api_disambiguation_item(
    item: &crate::api::session::DisambiguationItem,
) -> ob_poc_types::DisambiguationItem {
    match item {
        crate::api::session::DisambiguationItem::EntityMatch {
            param,
            search_text,
            matches,
            entity_type,
            search_column,
            ref_id,
        } => ob_poc_types::DisambiguationItem::EntityMatch {
            param: param.clone(),
            search_text: search_text.clone(),
            matches: matches.iter().map(to_api_entity_match).collect(),
            entity_type: entity_type.clone(),
            search_column: search_column.clone(),
            ref_id: ref_id.clone(),
        },
        crate::api::session::DisambiguationItem::InterpretationChoice { text, options } => {
            ob_poc_types::DisambiguationItem::InterpretationChoice {
                text: text.clone(),
                options: options
                    .iter()
                    .map(|opt| ob_poc_types::Interpretation {
                        id: opt.id.clone(),
                        label: opt.label.clone(),
                        description: opt.description.clone(),
                        effect: opt.effect.clone(),
                    })
                    .collect(),
            }
        }
        crate::api::session::DisambiguationItem::ClientGroupMatch {
            search_text,
            candidates,
        } => ob_poc_types::DisambiguationItem::ClientGroupMatch {
            search_text: search_text.clone(),
            candidates: candidates
                .iter()
                .map(|c| ob_poc_types::ClientGroupCandidate {
                    group_id: c.group_id.to_string(),
                    group_name: c.group_name.clone(),
                    matched_alias: c.matched_alias.clone(),
                    confidence: c.confidence,
                    entity_count: c.entity_count,
                })
                .collect(),
        },
    }
}

/// Convert internal EntityMatchOption to API EntityMatch
fn to_api_entity_match(opt: &crate::api::session::EntityMatchOption) -> ob_poc_types::EntityMatch {
    ob_poc_types::EntityMatch {
        entity_id: opt.entity_id.to_string(),
        name: opt.name.clone(),
        entity_type: opt.entity_type.clone(),
        jurisdiction: opt.jurisdiction.clone(),
        context: opt.context.clone(),
        score: opt.score.map(|s| s as f64),
    }
}

/// Convert internal dsl_core::Statement to API AstStatement
fn to_api_ast_statement(stmt: &dsl_core::Statement) -> AstStatement {
    match stmt {
        dsl_core::Statement::VerbCall(vc) => AstStatement::VerbCall {
            domain: vc.domain.clone(),
            verb: vc.verb.clone(),
            arguments: vc
                .arguments
                .iter()
                .map(|arg| AstArgument {
                    key: arg.key.clone(),
                    value: to_api_ast_value(&arg.value),
                    span: Some(to_api_span(&arg.span)),
                })
                .collect(),
            binding: vc.binding.clone(),
            span: Some(to_api_span(&vc.span)),
        },
        dsl_core::Statement::Comment(text) => AstStatement::Comment {
            text: text.clone(),
            span: None,
        },
    }
}

/// Convert internal dsl_core::AstNode to API AstValue
fn to_api_ast_value(node: &dsl_core::AstNode) -> AstValue {
    match node {
        dsl_core::AstNode::Literal(lit) => match lit {
            dsl_core::ast::Literal::String(s) => AstValue::String { value: s.clone() },
            dsl_core::ast::Literal::Integer(n) => AstValue::Number { value: *n as f64 },
            dsl_core::ast::Literal::Decimal(d) => {
                use rust_decimal::prelude::ToPrimitive;
                AstValue::Number {
                    value: d.to_f64().unwrap_or(0.0),
                }
            }
            dsl_core::ast::Literal::Boolean(b) => AstValue::Boolean { value: *b },
            dsl_core::ast::Literal::Null => AstValue::Null,
            dsl_core::ast::Literal::Uuid(u) => AstValue::String {
                value: u.to_string(),
            },
        },
        dsl_core::AstNode::SymbolRef { name, .. } => AstValue::SymbolRef { name: name.clone() },
        dsl_core::AstNode::EntityRef {
            entity_type,
            value,
            resolved_key,
            ..
        } => AstValue::EntityRef {
            entity_type: entity_type.clone(),
            search_key: value.clone(),
            resolved_key: resolved_key.clone(),
        },
        dsl_core::AstNode::List { items, .. } => AstValue::List {
            items: items.iter().map(to_api_ast_value).collect(),
        },
        dsl_core::AstNode::Map { entries, .. } => AstValue::Map {
            entries: entries
                .iter()
                .map(|(k, v)| ob_poc_types::AstMapEntry {
                    key: k.clone(),
                    value: to_api_ast_value(v),
                })
                .collect(),
        },
        dsl_core::AstNode::Nested(_vc) => {
            // Nested verb calls are complex - for now just represent as null
            // The UI doesn't typically need to display nested calls inline
            AstValue::Null
        }
    }
}

/// Convert internal VerbIntent to API VerbIntentInfo
fn to_api_verb_intent(intent: &crate::api::intent::VerbIntent) -> VerbIntentInfo {
    use ob_poc_types::ParamValue as ApiParamValue;

    // Split verb into domain.action
    let parts: Vec<&str> = intent.verb.splitn(2, '.').collect();
    let (domain, action) = if parts.len() == 2 {
        (parts[0].to_string(), parts[1].to_string())
    } else {
        ("unknown".to_string(), intent.verb.clone())
    };

    // Convert params
    let params: std::collections::HashMap<String, ApiParamValue> = intent
        .params
        .iter()
        .map(|(k, v)| {
            let api_val = match v {
                crate::api::intent::ParamValue::String(s) => {
                    ApiParamValue::String { value: s.clone() }
                }
                crate::api::intent::ParamValue::Number(n) => ApiParamValue::Number { value: *n },
                crate::api::intent::ParamValue::Integer(n) => {
                    ApiParamValue::Number { value: *n as f64 }
                }
                crate::api::intent::ParamValue::Boolean(b) => ApiParamValue::Boolean { value: *b },
                crate::api::intent::ParamValue::Uuid(u) => ApiParamValue::String {
                    value: u.to_string(),
                },
                crate::api::intent::ParamValue::ResolvedEntity {
                    display_name,
                    resolved_id,
                } => ApiParamValue::ResolvedEntity {
                    display_name: display_name.clone(),
                    resolved_id: resolved_id.to_string(),
                    entity_type: "entity".to_string(), // Default, could be enhanced
                },
                crate::api::intent::ParamValue::List(items) => {
                    // For lists, just stringify for now
                    ApiParamValue::String {
                        value: format!("{:?}", items),
                    }
                }
                crate::api::intent::ParamValue::Object(obj) => {
                    // For objects, just stringify for now
                    ApiParamValue::String {
                        value: format!("{:?}", obj),
                    }
                }
            };
            (k.clone(), api_val)
        })
        .collect();

    VerbIntentInfo {
        verb: intent.verb.clone(),
        domain,
        action,
        params,
        bind_as: None, // VerbIntent doesn't have bind_as field
        validation: None,
    }
}

/// Build DslState from AgentChatResponse fields
fn build_dsl_state(
    dsl_source: Option<&String>,
    ast: Option<&Vec<dsl_core::Statement>>,
    can_execute: bool,
    intents: &[crate::api::intent::VerbIntent],
    validation_results: &[crate::api::intent::IntentValidation],
    bindings: &std::collections::HashMap<String, crate::api::session::BoundEntity>,
) -> Option<DslState> {
    // Only create DslState if there's actual DSL content
    if dsl_source.is_none() && ast.is_none() && intents.is_empty() {
        return None;
    }

    // Convert AST
    let api_ast = ast.map(|stmts| stmts.iter().map(to_api_ast_statement).collect());

    // Convert intents
    let api_intents = if intents.is_empty() {
        None
    } else {
        Some(intents.iter().map(to_api_verb_intent).collect())
    };

    // Build validation from validation_results
    let validation = if validation_results.is_empty() {
        None
    } else {
        let errors: Vec<ApiValidationError> = validation_results
            .iter()
            .filter(|v| !v.valid)
            .flat_map(|v| {
                // Convert each IntentError to ApiValidationError
                v.errors.iter().map(|err| ApiValidationError {
                    message: format!("[{}] {}", v.intent.verb, err.message),
                    line: None,
                    column: None,
                    suggestion: err.param.clone(), // Use param as suggestion context
                })
            })
            .collect();

        Some(DslValidation {
            valid: errors.is_empty(),
            errors,
            warnings: vec![],
        })
    };

    // Convert bindings (BoundEntity.display_name -> BoundEntityInfo.name)
    let api_bindings: std::collections::HashMap<String, BoundEntityInfo> = bindings
        .iter()
        .map(|(k, v)| {
            (
                k.clone(),
                BoundEntityInfo {
                    id: v.id.to_string(),
                    name: v.display_name.clone(),
                    entity_type: v.entity_type.clone(),
                },
            )
        })
        .collect();

    Some(DslState {
        source: dsl_source.cloned(),
        ast: api_ast,
        can_execute,
        validation,
        intents: api_intents,
        bindings: api_bindings,
    })
}

/// Convert unified UnresolvedRefInfo to API UnresolvedRefResponse
#[allow(dead_code)]
fn to_api_unresolved_ref(info: &UnresolvedRefInfo) -> ApiUnresolvedRefResponse {
    // Create a simple search key from the entity_type and search_value
    let search_keys: Vec<ApiSearchKeyField> = vec![ApiSearchKeyField {
        name: "name".to_string(),
        label: "Name".to_string(),
        is_default: true,
        field_type: SearchKeyFieldType::Text,
        enum_values: None,
    }];

    // No discriminator fields in simplified unified type
    let discriminator_fields: Vec<ApiDiscriminatorField> = vec![];

    // Default to picker/modal for resolution
    let resolution_mode = ApiResolutionModeHint::SearchModal;

    // Parse ref_id to get statement index (format: "stmt_idx:arg_name")
    let (stmt_idx, arg_name) = if let Some(colon_pos) = info.ref_id.find(':') {
        let stmt_str = &info.ref_id[..colon_pos];
        let arg = info.ref_id[colon_pos + 1..].to_string();
        (stmt_str.parse::<usize>().unwrap_or(0), arg)
    } else {
        (0, info.ref_id.clone())
    };

    // Build context from ref_id
    let context = RefContext {
        statement_index: stmt_idx,
        verb: String::new(), // Not available in simplified type
        arg_name,
        dsl_snippet: Some(info.context_line.clone()),
    };

    // Convert initial matches from unified EntityMatchInfo
    let initial_matches: Vec<ob_poc_types::resolution::EntityMatchResponse> = info
        .initial_matches
        .iter()
        .map(|m| ob_poc_types::resolution::EntityMatchResponse {
            id: m.value.clone(),
            display: m.display.clone(),
            entity_type: info.entity_type.clone(),
            score: (m.score_pct as f32) / 100.0,
            discriminators: std::collections::HashMap::new(),
            status: ob_poc_types::resolution::EntityStatus::Unknown,
            context: m.detail.clone(),
        })
        .collect();

    ApiUnresolvedRefResponse {
        ref_id: info.ref_id.clone(),
        entity_type: info.entity_type.clone(),
        entity_subtype: None,
        search_value: info.search_value.clone(),
        context,
        initial_matches,
        agent_suggestion: None,
        suggestion_reason: None,
        review_requirement: ReviewRequirement::Optional,
        search_keys,
        discriminator_fields,
        resolution_mode,
        return_key_type: Some("uuid".to_string()),
    }
}

/// Convert a list of UnresolvedRefInfo (unified) to API types
#[allow(dead_code)]
fn to_api_unresolved_refs(refs: &[UnresolvedRefInfo]) -> Vec<ApiUnresolvedRefResponse> {
    refs.iter().map(to_api_unresolved_ref).collect()
}

/// Convert a list of UnresolvedRefInfo (unified) to API types
fn api_unresolved_refs_to_api(
    refs: &[crate::session::UnresolvedRefInfo],
) -> Vec<ApiUnresolvedRefResponse> {
    refs.iter().map(api_unresolved_ref_to_api).collect()
}

/// Convert unified::UnresolvedRefInfo to API response
fn api_unresolved_ref_to_api(info: &crate::session::UnresolvedRefInfo) -> ApiUnresolvedRefResponse {
    // Create a simple search key from the entity_type and search_value
    let search_keys: Vec<ApiSearchKeyField> = vec![ApiSearchKeyField {
        name: "name".to_string(),
        label: "Name".to_string(),
        is_default: true,
        field_type: SearchKeyFieldType::Text,
        enum_values: None,
    }];

    // No discriminator fields in simplified unified type
    let discriminator_fields: Vec<ApiDiscriminatorField> = vec![];

    // Default to picker/modal for resolution
    let resolution_mode = ApiResolutionModeHint::SearchModal;

    // Parse ref_id to get statement index (format: "stmt_idx:arg_name")
    let (stmt_idx, arg_name) = if let Some(colon_pos) = info.ref_id.find(':') {
        let stmt_str = &info.ref_id[..colon_pos];
        let arg = info.ref_id[colon_pos + 1..].to_string();
        (stmt_str.parse::<usize>().unwrap_or(0), arg)
    } else {
        (0, info.ref_id.clone())
    };

    // Build context from ref_id
    let context = RefContext {
        statement_index: stmt_idx,
        verb: String::new(), // Not available in simplified type
        arg_name,
        dsl_snippet: Some(info.context_line.clone()),
    };

    // Convert initial matches from unified EntityMatchInfo
    let initial_matches: Vec<ob_poc_types::resolution::EntityMatchResponse> = info
        .initial_matches
        .iter()
        .map(|m| ob_poc_types::resolution::EntityMatchResponse {
            id: m.value.clone(),
            display: m.display.clone(),
            entity_type: info.entity_type.clone(),
            score: (m.score_pct as f32) / 100.0,
            discriminators: std::collections::HashMap::new(),
            status: ob_poc_types::resolution::EntityStatus::Unknown,
            context: m.detail.clone(),
        })
        .collect();

    ApiUnresolvedRefResponse {
        ref_id: info.ref_id.clone(),
        entity_type: info.entity_type.clone(),
        entity_subtype: None,
        search_value: info.search_value.clone(),
        context,
        initial_matches,
        agent_suggestion: None,
        suggestion_reason: None,
        review_requirement: ReviewRequirement::Optional,
        search_keys,
        discriminator_fields,
        resolution_mode,
        return_key_type: Some("uuid".to_string()),
    }
}

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ValidateDslRequest {
    pub dsl: String,
}

#[derive(Debug, Deserialize)]
pub struct GenerateDslRequest {
    pub instruction: String,
    pub domain: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GenerateDslResponse {
    pub dsl: Option<String>,
    pub explanation: Option<String>,
    pub error: Option<String>,
}

// ============================================================================
// Batch Operations Request/Response Types
// ============================================================================

/// Request to add products to multiple CBUs (server-side DSL generation)
#[derive(Debug, Deserialize)]
pub struct BatchAddProductsRequest {
    /// CBU IDs to add products to
    pub cbu_ids: Vec<Uuid>,
    /// Product codes to add (e.g., ["CUSTODY", "FUND_ACCOUNTING"])
    pub products: Vec<String>,
}

/// Result of adding a product to a single CBU
#[derive(Debug, Serialize)]
pub struct BatchProductResult {
    pub cbu_id: Uuid,
    pub product: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub services_added: Option<i32>,
}

/// Response from batch add products
#[derive(Debug, Serialize)]
pub struct BatchAddProductsResponse {
    pub total_operations: usize,
    pub success_count: usize,
    pub failure_count: usize,
    pub duration_ms: u64,
    pub results: Vec<BatchProductResult>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidationError {
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub message: String,
    pub suggestion: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DomainsResponse {
    pub domains: Vec<DomainInfo>,
    pub total_verbs: usize,
}

#[derive(Debug, Serialize)]
pub struct DomainInfo {
    pub name: String,
    pub description: String,
    pub verb_count: usize,
}

#[derive(Debug, Deserialize)]
pub struct VocabQuery {
    pub domain: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct VocabResponse {
    pub verbs: Vec<VerbInfo>,
}

#[derive(Debug, Serialize)]
pub struct VerbInfo {
    pub domain: String,
    pub name: String,
    pub full_name: String,
    pub description: String,
    pub required_args: Vec<String>,
    pub optional_args: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub verb_count: usize,
    pub domain_count: usize,
}

// ============================================================================
// Completion Request/Response Types (LSP-style via EntityGateway)
// ============================================================================

/// Request for entity completion
#[derive(Debug, Deserialize)]
pub struct CompleteRequest {
    /// The type of entity to complete: "cbu", "entity", "product", "role", "jurisdiction", etc.
    pub entity_type: String,
    /// The search query (partial text to match)
    pub query: String,
    /// Maximum number of results (default 10)
    #[serde(default = "default_limit")]
    pub limit: i32,
}

fn default_limit() -> i32 {
    10
}

/// A single completion item
#[derive(Debug, Serialize)]
pub struct CompletionItem {
    /// The value to insert (UUID or code)
    pub value: String,
    /// Display label for the completion
    pub label: String,
    /// Additional detail (e.g., entity type, jurisdiction)
    pub detail: Option<String>,
    /// Relevance score (0.0-1.0)
    pub score: f32,
}

/// Response with completion items
#[derive(Debug, Serialize)]
pub struct CompleteResponse {
    pub items: Vec<CompletionItem>,
    pub total: usize,
}

// ============================================================================
// Session Watch Types (Long-Polling)
// ============================================================================

/// Query parameters for session watch endpoint
#[derive(Debug, Deserialize)]
pub struct WatchQuery {
    /// Timeout in milliseconds (default 30000, max 60000)
    #[serde(default = "default_watch_timeout")]
    pub timeout_ms: u64,
}

fn default_watch_timeout() -> u64 {
    30000
}

/// Response from session watch endpoint
#[derive(Debug, Serialize)]
pub struct WatchResponse {
    /// Session ID
    pub session_id: Uuid,
    /// Version number (incremented on each update)
    pub version: u64,
    /// Current scope path as string
    pub scope_path: String,
    /// Whether struct_mass has been computed
    pub has_mass: bool,
    /// Current effective view mode (if set)
    pub view_mode: Option<String>,
    /// Active CBU ID (if bound)
    pub active_cbu_id: Option<Uuid>,
    /// Timestamp of last update (RFC3339)
    pub updated_at: String,
    /// Whether this is the initial snapshot (no wait) or a change notification
    pub is_initial: bool,
    /// Session scope type (galaxy, book, cbu, jurisdiction, neighborhood, empty)
    pub scope_type: Option<String>,
    /// Whether scope data is fully loaded
    pub scope_loaded: bool,
}

impl WatchResponse {
    fn from_snapshot(
        snapshot: &crate::api::session_manager::SessionSnapshot,
        is_initial: bool,
    ) -> Self {
        // Extract scope type string from GraphScope
        let scope_type = snapshot.scope_definition.as_ref().map(|s| match s {
            crate::graph::GraphScope::Empty => "empty".to_string(),
            crate::graph::GraphScope::SingleCbu { .. } => "cbu".to_string(),
            crate::graph::GraphScope::Book { .. } => "book".to_string(),
            crate::graph::GraphScope::Jurisdiction { .. } => "jurisdiction".to_string(),
            crate::graph::GraphScope::EntityNeighborhood { .. } => "neighborhood".to_string(),
            crate::graph::GraphScope::Custom { .. } => "custom".to_string(),
        });

        Self {
            session_id: snapshot.session_id,
            version: snapshot.version,
            scope_path: snapshot.scope_path.clone(),
            has_mass: snapshot.has_mass,
            view_mode: snapshot.view_mode.clone(),
            active_cbu_id: snapshot.active_cbu_id,
            updated_at: snapshot.updated_at.to_rfc3339(),
            is_initial,
            scope_type,
            scope_loaded: snapshot.scope_loaded,
        }
    }
}

// ============================================================================
// Entity Reference Resolution Types
// ============================================================================

/// Identifies a specific EntityRef in the AST
#[derive(Debug, Deserialize)]
pub struct RefId {
    /// Index of statement in AST (0-based)
    pub statement_index: usize,
    /// Argument key containing the EntityRef (e.g., "entity-id")
    pub arg_key: String,
}

/// Request to resolve an EntityRef in the session AST
#[derive(Debug, Deserialize)]
pub struct ResolveRefRequest {
    /// Session containing the AST
    pub session_id: Uuid,
    /// Location of the EntityRef to resolve
    pub ref_id: RefId,
    /// Primary key from entity search (UUID or code)
    pub resolved_key: String,
}

/// Statistics about EntityRef resolution in the AST
#[derive(Debug, Serialize)]
pub struct ResolutionStats {
    /// Total EntityRef nodes in AST
    pub total_refs: i32,
    /// Remaining unresolved refs
    pub unresolved_count: i32,
}

// ============================================================================
// Span-Based Resolution Types (Issue K)
// ============================================================================

/// Request to resolve an EntityRef by span-based ref_id (Issue K)
///
/// Uses span-based ref_id format ("stmt_idx:start-end") for precise targeting
/// of refs in lists and maps. Includes dsl_hash to prevent stale commits.
#[derive(Debug, Deserialize)]
pub struct ResolveByRefIdRequest {
    /// Session containing the AST
    pub session_id: Uuid,
    /// Span-based ref_id (e.g., "0:15-30")
    pub ref_id: String,
    /// Primary key from entity search (UUID)
    pub resolved_key: String,
    /// Hash of DSL this resolution applies to (prevents race conditions)
    pub dsl_hash: String,
}

/// Response from resolving by ref_id (Issue K)
#[derive(Debug, Serialize)]
pub struct ResolveByRefIdResponse {
    /// Whether the update succeeded
    pub success: bool,
    /// Updated DSL with resolved ref
    pub dsl: String,
    /// New hash for the updated DSL
    pub dsl_hash: String,
    /// Remaining unresolved refs (so UI can continue without round-trip)
    pub remaining_unresolved: Vec<RemainingUnresolvedRef>,
    /// Whether all refs are now resolved
    pub fully_resolved: bool,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Info about an unresolved ref (for ResolveByRefIdResponse)
#[derive(Debug, Clone, Serialize)]
pub struct RemainingUnresolvedRef {
    /// Argument key (e.g., "entity-id")
    pub param_name: String,
    /// Search text (e.g., "John Smith")
    pub search_value: String,
    /// Entity type (e.g., "entity", "cbu")
    pub entity_type: String,
    /// Search column (e.g., "name")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_column: Option<String>,
    /// Span-based ref_id (e.g., "0:15-30")
    pub ref_id: String,
}

/// Response from resolving an EntityRef
#[derive(Debug, Serialize)]
pub struct ResolveRefResponse {
    /// Whether the update succeeded
    pub success: bool,
    /// DSL source re-rendered from updated AST (DSL + AST are a tuple pair)
    pub dsl_source: Option<String>,
    /// Full refreshed AST with updated triplet
    pub ast: Option<Vec<crate::dsl_v2::ast::Statement>>,
    /// Resolution statistics
    pub resolution_stats: ResolutionStats,
    /// True if all refs resolved (ready to execute)
    pub can_execute: bool,
    /// Error message if failed
    pub error: Option<String>,
    /// Error code for programmatic handling
    pub code: Option<String>,
}

// ============================================================================
// Onboarding Request/Response Types
// ============================================================================

/// Request to generate onboarding DSL from natural language
#[derive(Debug, Deserialize)]
pub struct OnboardingRequest {
    /// Natural language description of the onboarding request
    pub description: String,
    /// Whether to execute the DSL after generation
    #[serde(default)]
    pub execute: bool,
}

/// Response from onboarding DSL generation
#[derive(Debug, Serialize)]
pub struct OnboardingResponse {
    /// Generated DSL code
    pub dsl: Option<String>,
    /// Explanation of what was generated
    pub explanation: Option<String>,
    /// Validation result
    pub validation: Option<ValidationResult>,
    /// Execution result (if execute=true)
    pub execution: Option<OnboardingExecutionResult>,
    /// Error message if generation failed
    pub error: Option<String>,
}

/// Result of executing onboarding DSL
#[derive(Debug, Serialize)]
pub struct OnboardingExecutionResult {
    pub success: bool,
    pub cbu_id: Option<Uuid>,
    pub resource_count: usize,
    pub delivery_count: usize,
    pub errors: Vec<String>,
}

/// Outcome of DSL execution - either atomic (all-or-nothing) or best-effort (partial success)
///
/// This enum captures the execution strategy result, allowing the caller to handle
/// different outcomes appropriately (e.g., rollback vs partial success).
#[derive(Debug)]
enum ExecutionOutcome {
    /// Atomic execution result (all steps in single transaction)
    Atomic(AtomicExecutionResult),
    /// Best-effort execution result (continues on failure)
    BestEffort(BestEffortExecutionResult),
}

// ============================================================================
// State
// ============================================================================

#[derive(Clone)]
pub struct AgentState {
    pub pool: PgPool,
    pub dsl_v2_executor: Arc<DslExecutor>,
    pub sessions: SessionStore,
    pub session_manager: crate::api::session_manager::SessionManager,
    pub generation_log: Arc<GenerationLogRepository>,
    pub session_repo: Arc<crate::database::SessionRepository>,
    pub dsl_repo: Arc<crate::database::DslRepository>,
    pub agent_service: Arc<crate::api::agent_service::AgentService>,
    pub feedback_service: Arc<ob_semantic_matcher::FeedbackService>,
    pub expansion_audit: Arc<crate::database::ExpansionAuditRepository>,
}

impl AgentState {
    /// Create with semantic verb search (blocks on embedder init ~3-5s)
    ///
    /// This is the primary constructor. Initializes Candle embedder synchronously
    /// so semantic search is available immediately when server starts accepting requests.
    pub async fn with_semantic(pool: PgPool, sessions: SessionStore) -> Self {
        use crate::agent::learning::embedder::CandleEmbedder;

        let dsl_v2_executor = Arc::new(DslExecutor::new(pool.clone()));
        let generation_log = Arc::new(GenerationLogRepository::new(pool.clone()));
        let session_repo = Arc::new(crate::database::SessionRepository::new(pool.clone()));
        let dsl_repo = Arc::new(crate::database::DslRepository::new(pool.clone()));
        let feedback_service = Arc::new(ob_semantic_matcher::FeedbackService::new(pool.clone()));
        let session_manager = crate::api::session_manager::SessionManager::new(sessions.clone());

        // Initialize embedder synchronously (blocks ~3-5s, but only at startup)
        // This is REQUIRED - server cannot start without semantic search
        tracing::info!("Initializing Candle embedder...");
        let start = std::time::Instant::now();

        let embedder: Arc<CandleEmbedder> = match tokio::task::spawn_blocking(CandleEmbedder::new)
            .await
        {
            Ok(Ok(e)) => {
                tracing::info!("Candle embedder ready in {}ms", start.elapsed().as_millis());
                Arc::new(e)
            }
            Ok(Err(e)) => {
                panic!("FATAL: Failed to initialize Candle embedder: {}. Server cannot start without semantic search.", e);
            }
            Err(e) => {
                panic!("FATAL: Candle embedder task panicked: {}. Server cannot start without semantic search.", e);
            }
        };

        // Build agent service with embedder - REQUIRED, no fallback
        let agent_service = crate::api::agent_service::AgentService::new(pool.clone(), embedder);
        let expansion_audit =
            Arc::new(crate::database::ExpansionAuditRepository::new(pool.clone()));

        Self {
            pool,
            dsl_v2_executor,
            sessions,
            session_manager,
            generation_log,
            session_repo,
            dsl_repo,
            agent_service: Arc::new(agent_service),
            feedback_service,
            expansion_audit,
        }
    }
}

// ============================================================================
// Router
// ============================================================================

/// Create agent router with semantic verb search
///
/// This is the ONLY constructor. Initializes Candle embedder synchronously
/// so semantic search is available immediately when server starts accepting requests.
/// There is no non-semantic path - all chat goes through the IntentPipeline.
pub async fn create_agent_router_with_semantic(pool: PgPool, sessions: SessionStore) -> Router {
    let state = AgentState::with_semantic(pool, sessions).await;
    create_agent_router_with_state(state)
}

/// Internal: create router from pre-built state
fn create_agent_router_with_state(state: AgentState) -> Router {
    Router::new()
        // Session management
        .route("/api/session", post(create_session))
        .route("/api/session/:id", get(get_session))
        .route("/api/session/:id", delete(delete_session))
        .route("/api/session/:id/chat", post(chat_session))
        .route("/api/session/:id/execute", post(execute_session_dsl))
        .route("/api/session/:id/repl-edit", post(repl_edit_session))
        .route("/api/session/:id/clear", post(clear_session_dsl))
        .route("/api/session/:id/bind", post(set_session_binding))
        .route("/api/session/:id/context", get(get_session_context))
        .route("/api/session/:id/focus", post(set_session_focus))
        .route("/api/session/:id/view-mode", post(set_session_view_mode))
        .route("/api/session/:id/dsl/enrich", get(get_enriched_dsl))
        .route("/api/session/:id/watch", get(watch_session))
        // Sub-session management (create/get/complete/cancel only - chat goes through main pipeline)
        .route("/api/session/:id/subsession", post(create_subsession))
        .route("/api/session/:id/subsession/:child_id", get(get_subsession))
        .route(
            "/api/session/:id/subsession/:child_id/complete",
            post(complete_subsession),
        )
        .route(
            "/api/session/:id/subsession/:child_id/cancel",
            post(cancel_subsession),
        )
        // DSL parsing and entity reference resolution
        .route("/api/dsl/parse", post(parse_dsl))
        .route("/api/dsl/resolve-ref", post(resolve_entity_ref))
        .route("/api/dsl/resolve-by-ref-id", post(resolve_by_ref_id))
        // Discriminator parsing for resolution
        .route(
            "/api/resolution/parse-discriminators",
            post(parse_discriminators),
        )
        // Vocabulary and metadata
        .route("/api/agent/generate", post(generate_dsl))
        .route("/api/agent/validate", post(validate_dsl))
        .route("/api/agent/domains", get(list_domains))
        .route("/api/agent/vocabulary", get(get_vocabulary))
        .route("/api/agent/health", get(health_check))
        // Completions (LSP-style lookup via EntityGateway)
        .route("/api/agent/complete", post(complete_entity))
        // Onboarding
        .route("/api/agent/onboard", post(generate_onboarding_dsl))
        // Enhanced generation with tool use
        .route(
            "/api/agent/generate-with-tools",
            post(generate_dsl_with_tools),
        )
        // Batch operations (server-side DSL generation, no LLM)
        .route("/api/batch/add-products", post(batch_add_products))
        // Learning/feedback (captures user corrections for continuous improvement)
        .route("/api/agent/correction", post(report_correction))
        // Verb disambiguation selection (closes the learning loop)
        .route(
            "/api/session/:id/select-verb",
            post(select_verb_disambiguation),
        )
        // Verb disambiguation abandonment (user bailed - all options were wrong)
        .route(
            "/api/session/:id/abandon-disambiguation",
            post(abandon_disambiguation),
        )
        .with_state(state)
}

// ============================================================================
// Session Handlers
// ============================================================================

/// POST /api/session - Create new session
async fn create_session(
    State(state): State<AgentState>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<Json<CreateSessionResponse>, StatusCode> {
    use crate::session::constraint_cascade::update_dag_from_cascade;
    use crate::session::unified::StructureType;

    tracing::info!("=== CREATE SESSION ===");
    tracing::info!("Domain hint: {:?}", req.domain_hint);
    tracing::info!("Initial client: {:?}", req.initial_client);
    tracing::info!("Structure type: {:?}", req.structure_type);

    let mut session = UnifiedSession::new_for_entity(None, "cbu", None, req.domain_hint.clone());
    let session_id = session.id;
    let created_at = session.created_at;

    // Wire initial client constraint if provided
    let (final_state, welcome_message) = if let Some(ref client_ref) = req.initial_client {
        // Try to resolve client from client_id or client_name
        let resolved_client = resolve_initial_client(&state.pool, client_ref).await;

        match resolved_client {
            Ok(client) => {
                tracing::info!(
                    "Setting initial client constraint: {} ({})",
                    client.display_name,
                    client.client_id
                );

                // Set client context on session (constraint cascade level 1)
                session.client = Some(client.clone());

                // Set structure type if provided (constraint cascade level 2)
                if let Some(ref st) = req.structure_type {
                    if let Some(structure_type) = StructureType::from_internal(st) {
                        session.structure_type = Some(structure_type);
                        tracing::info!("Setting structure type: {:?}", structure_type);
                    }
                }

                // Update DAG state from cascade
                update_dag_from_cascade(&mut session);

                // Transition to Scoped state
                session.transition(crate::session::unified::SessionEvent::ScopeSet);

                (
                    SessionState::Scoped,
                    format!(
                        "Session scoped to {}. What would you like to do?",
                        client.display_name
                    ),
                )
            }
            Err(e) => {
                tracing::warn!("Failed to resolve initial client: {}", e);
                // Fall back to prompting for client
                (
                    SessionState::New,
                    crate::api::session::WELCOME_MESSAGE.to_string(),
                )
            }
        }
    } else {
        // No initial client - prompt for client selection
        (
            SessionState::New,
            crate::api::session::WELCOME_MESSAGE.to_string(),
        )
    };

    tracing::info!("Created session ID: {}", session_id);

    // Store in memory
    {
        let mut sessions = state.sessions.write().await;
        sessions.insert(session_id, session);
        tracing::info!(
            "Session stored in memory, total sessions: {}",
            sessions.len()
        );
    }

    // Persist to database asynchronously (simple insert, ~1-5ms)
    let session_repo = state.session_repo.clone();
    tokio::spawn(async move {
        if let Err(e) = session_repo
            .create_session_with_id(session_id, None, None)
            .await
        {
            tracing::error!("Failed to persist session {}: {}", session_id, e);
        }
    });

    let response = CreateSessionResponse {
        session_id,
        created_at,
        state: final_state.into(),
        welcome_message,
    };
    tracing::info!(
        "Returning CreateSessionResponse: session_id={}, state={:?}, welcome_message={}",
        response.session_id,
        response.state,
        response.welcome_message
    );
    Ok(Json(response))
}

/// Resolve initial client from client_id or client_name
async fn resolve_initial_client(
    pool: &sqlx::PgPool,
    client_ref: &crate::api::session::InitialClientRef,
) -> Result<crate::session::unified::ClientRef, String> {
    use crate::session::unified::ClientRef;

    // If client_id is provided, look it up directly
    if let Some(client_id) = client_ref.client_id {
        let row: Option<(Uuid, String)> = sqlx::query_as(
            r#"
            SELECT id, canonical_name
            FROM "ob-poc".client_group
            WHERE id = $1
            "#,
        )
        .bind(client_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        match row {
            Some((id, name)) => Ok(ClientRef {
                client_id: id,
                display_name: name,
            }),
            None => Err(format!("Client group not found: {}", client_id)),
        }
    }
    // If client_name is provided, search for it
    else if let Some(ref client_name) = client_ref.client_name {
        // Search by alias first (exact match)
        let row: Option<(Uuid, String)> = sqlx::query_as(
            r#"
            SELECT cg.id, cg.canonical_name
            FROM "ob-poc".client_group cg
            JOIN "ob-poc".client_group_alias cga ON cg.id = cga.client_group_id
            WHERE LOWER(cga.alias) = LOWER($1)
            LIMIT 1
            "#,
        )
        .bind(client_name)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        if let Some((id, name)) = row {
            return Ok(ClientRef {
                client_id: id,
                display_name: name,
            });
        }

        // Fallback: search by canonical name (case-insensitive)
        let row: Option<(Uuid, String)> = sqlx::query_as(
            r#"
            SELECT id, canonical_name
            FROM "ob-poc".client_group
            WHERE LOWER(canonical_name) LIKE LOWER($1)
            LIMIT 1
            "#,
        )
        .bind(format!("%{}%", client_name))
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        match row {
            Some((id, name)) => Ok(ClientRef {
                client_id: id,
                display_name: name,
            }),
            None => Err(format!("Client not found: {}", client_name)),
        }
    } else {
        Err("Either client_id or client_name must be provided".to_string())
    }
}

/// GET /api/session/:id - Get session state (creates if not found)
async fn get_session(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
) -> Json<SessionStateResponse> {
    // Try to get existing session, or create a new one with the requested ID
    let session = {
        let sessions = state.sessions.read().await;
        sessions.get(&session_id).cloned()
    };

    let session = match session {
        Some(s) => s,
        None => {
            // Create new session with the requested ID
            let mut new_session = UnifiedSession::new_for_entity(None, "cbu", None, None);
            new_session.id = session_id; // Use the requested ID
            let mut sessions = state.sessions.write().await;
            sessions.insert(session_id, new_session.clone());
            new_session
        }
    };

    Json(SessionStateResponse {
        session_id,
        entity_type: session.entity_type.clone(),
        entity_id: session.entity_id,
        state: session.state.clone().into(),
        message_count: session.messages.len(),
        pending_intents: vec![], // Legacy - now in run_sheet
        assembled_dsl: vec![],   // Legacy - now in run_sheet
        combined_dsl: session.run_sheet.combined_dsl(),
        context: session.context.clone(),
        messages: session.messages.iter().cloned().map(|m| m.into()).collect(),
        can_execute: session.run_sheet.has_runnable(),
        version: Some(session.updated_at.to_rfc3339()),
        run_sheet: Some(session.run_sheet.to_api()),
        bindings: session
            .context
            .bindings
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    ob_poc_types::BoundEntityInfo {
                        id: v.id.to_string(),
                        name: v.display_name.clone(),
                        entity_type: v.entity_type.clone(),
                    },
                )
            })
            .collect(),
    })
}

/// DELETE /api/session/:id - Delete session
async fn delete_session(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let mut sessions = state.sessions.write().await;
    sessions.remove(&session_id).ok_or(StatusCode::NOT_FOUND)?;
    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/session/:id/watch - Long-poll for session changes
///
/// This endpoint uses tokio::sync::watch channels to efficiently wait for
/// session updates. Clients can use this to react to changes made by
/// other consumers (MCP, REPL, other browser tabs).
///
/// ## Query Parameters
///
/// - `timeout_ms`: Maximum time to wait for changes (default 30000, max 60000)
///
/// ## Response
///
/// Returns a `WatchResponse` with:
/// - `is_initial: true` if this is the first call (returns current state immediately)
/// - `is_initial: false` if a change was detected within the timeout
///
/// If the timeout expires with no changes, returns the current state with `is_initial: false`.
///
/// ## Usage Pattern
///
/// ```javascript
/// async function watchSession(sessionId) {
///   while (true) {
///     const resp = await fetch(`/api/session/${sessionId}/watch?timeout_ms=30000`);
///     const data = await resp.json();
///     handleUpdate(data);
///   }
/// }
/// ```
async fn watch_session(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
    Query(query): Query<WatchQuery>,
) -> Result<Json<WatchResponse>, StatusCode> {
    // Cap timeout at 60 seconds
    let timeout_ms = query.timeout_ms.min(60000);
    let timeout = std::time::Duration::from_millis(timeout_ms);

    // Subscribe to session changes
    let mut watcher = state
        .session_manager
        .subscribe(session_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    // Get initial snapshot
    let initial_snapshot = watcher.borrow().clone();

    // Wait for a change or timeout
    let result = tokio::time::timeout(timeout, watcher.changed()).await;

    // Unsubscribe when done (cleanup)
    state.session_manager.unsubscribe(session_id).await;

    match result {
        Ok(Ok(())) => {
            // Change detected - return the new snapshot
            let snapshot = watcher.borrow();
            Ok(Json(WatchResponse::from_snapshot(&snapshot, false)))
        }
        Ok(Err(_)) => {
            // Watch channel closed (session was deleted)
            Err(StatusCode::GONE)
        }
        Err(_) => {
            // Timeout - return current state
            Ok(Json(WatchResponse::from_snapshot(&initial_snapshot, false)))
        }
    }
}

// ============================================================================
// Sub-Session Handlers
// ============================================================================

/// Request to create a sub-session
#[derive(Debug, Deserialize)]
pub struct CreateSubSessionRequest {
    /// Type of sub-session to create
    pub session_type: CreateSubSessionType,
}

/// Sub-session type for API (simplified for JSON)
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CreateSubSessionType {
    /// Resolution sub-session with unresolved refs
    Resolution {
        /// Unresolved refs to resolve
        unresolved_refs: Vec<UnresolvedRefInfo>,
        /// Parent DSL statement index
        parent_dsl_index: usize,
    },
    /// Research sub-session
    Research {
        /// Target entity ID (optional)
        target_entity_id: Option<Uuid>,
        /// Research type
        research_type: String,
    },
    /// Review sub-session
    Review {
        /// DSL to review
        pending_dsl: String,
    },
}

/// Response from creating a sub-session
#[derive(Debug, Serialize)]
pub struct CreateSubSessionResponse {
    /// New sub-session ID
    pub session_id: Uuid,
    /// Parent session ID
    pub parent_id: Uuid,
    /// Inherited symbol names (for display)
    pub inherited_symbols: Vec<String>,
    /// Sub-session type
    pub session_type: String,
}

/// POST /api/session/:id/subsession - Create a sub-session
async fn create_subsession(
    State(state): State<AgentState>,
    Path(parent_id): Path<Uuid>,
    Json(req): Json<CreateSubSessionRequest>,
) -> Result<Json<CreateSubSessionResponse>, (StatusCode, String)> {
    tracing::info!("Creating sub-session for parent: {}", parent_id);

    // Get parent session
    let parent = {
        let sessions = state.sessions.read().await;
        sessions.get(&parent_id).cloned()
    };

    let parent = parent.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("Parent session {} not found", parent_id),
        )
    })?;

    // Convert API type to internal type
    let sub_session_type = match req.session_type {
        CreateSubSessionType::Resolution {
            unresolved_refs,
            parent_dsl_index,
        } => SubSessionType::Resolution(ResolutionSubSession {
            unresolved_refs,
            parent_dsl_index,
            current_ref_index: 0,
            resolutions: std::collections::HashMap::new(),
        }),
        CreateSubSessionType::Research {
            target_entity_id,
            research_type,
        } => SubSessionType::Research(ResearchSubSession {
            target_entity_id,
            research_type,
            search_query: None,
        }),
        CreateSubSessionType::Review { pending_dsl } => SubSessionType::Review(ReviewSubSession {
            pending_dsl,
            review_status: ReviewStatus::Pending,
        }),
    };

    let session_type_name = match &sub_session_type {
        SubSessionType::Root => "root",
        SubSessionType::Resolution(_) => "resolution",
        SubSessionType::Research(_) => "research",
        SubSessionType::Review(_) => "review",
        SubSessionType::Correction(_) => "correction",
    }
    .to_string();

    // Create sub-session
    let child = UnifiedSession::new_subsession(&parent, sub_session_type);
    let child_id = child.id;
    let inherited_symbols: Vec<String> = child.inherited_symbols.keys().cloned().collect();

    // Store in memory
    {
        let mut sessions = state.sessions.write().await;
        sessions.insert(child_id, child);
    }

    tracing::info!(
        "Created sub-session {} (type: {})",
        child_id,
        session_type_name
    );

    Ok(Json(CreateSubSessionResponse {
        session_id: child_id,
        parent_id,
        inherited_symbols,
        session_type: session_type_name,
    }))
}

/// GET /api/session/:id/subsession/:child_id - Get sub-session state
async fn get_subsession(
    State(state): State<AgentState>,
    Path((parent_id, child_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<SubSessionStateResponse>, (StatusCode, String)> {
    let sessions = state.sessions.read().await;

    let child = sessions.get(&child_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("Sub-session {} not found", child_id),
        )
    })?;

    // Verify parent relationship
    if child.parent_session_id != Some(parent_id) {
        return Err((
            StatusCode::BAD_REQUEST,
            "Invalid parent-child relationship".to_string(),
        ));
    }

    Ok(Json(SubSessionStateResponse::from_session(child)))
}

/// Response for sub-session state
#[derive(Debug, Serialize)]
pub struct SubSessionStateResponse {
    pub session_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub session_type: String,
    pub state: String,
    pub messages: Vec<SubSessionMessage>,
    /// Resolution-specific state
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<ResolutionState>,
}

#[derive(Debug, Serialize)]
pub struct SubSessionMessage {
    pub role: String,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct ResolutionState {
    pub total_refs: usize,
    pub current_index: usize,
    pub resolved_count: usize,
    pub current_ref: Option<UnresolvedRefInfo>,
    pub pending_refs: Vec<UnresolvedRefInfo>,
}

impl SubSessionStateResponse {
    fn from_session(session: &UnifiedSession) -> Self {
        let session_type = match &session.sub_session_type {
            SubSessionType::Root => "root",
            SubSessionType::Resolution(_) => "resolution",
            SubSessionType::Research(_) => "research",
            SubSessionType::Review(_) => "review",
            SubSessionType::Correction(_) => "correction",
        }
        .to_string();

        let state = match session.state {
            SessionState::New => "new",
            SessionState::Scoped => "scoped",
            SessionState::PendingValidation => "pending_validation",
            SessionState::ReadyToExecute => "ready_to_execute",
            SessionState::Executing => "executing",
            SessionState::Executed => "executed",
            SessionState::Closed => "closed",
        }
        .to_string();

        let messages = session
            .messages
            .iter()
            .map(|m| SubSessionMessage {
                role: match m.role {
                    MessageRole::User => "user",
                    MessageRole::Agent => "agent",
                    MessageRole::System => "system",
                }
                .to_string(),
                content: m.content.clone(),
                timestamp: m.timestamp,
            })
            .collect();

        let resolution = if let SubSessionType::Resolution(r) = &session.sub_session_type {
            Some(ResolutionState {
                total_refs: r.unresolved_refs.len(),
                current_index: r.current_ref_index,
                resolved_count: r.resolutions.len(),
                current_ref: r.unresolved_refs.get(r.current_ref_index).cloned(),
                pending_refs: r
                    .unresolved_refs
                    .iter()
                    .skip(r.current_ref_index + 1)
                    .cloned()
                    .collect(),
            })
        } else {
            None
        };

        Self {
            session_id: session.id,
            parent_id: session.parent_session_id,
            session_type,
            state,
            messages,
            resolution,
        }
    }
}

/// Request for sub-session chat
#[derive(Debug, Deserialize)]
pub struct SubSessionChatRequest {
    pub message: String,
}

/// POST /api/session/:id/subsession/:child_id/chat - Chat in sub-session
/// Request to complete a resolution sub-session
#[derive(Debug, Deserialize)]
pub struct CompleteSubSessionRequest {
    /// Whether to apply resolutions to parent
    #[serde(default = "default_true")]
    pub apply: bool,
}

fn default_true() -> bool {
    true
}

/// Response from completing a sub-session
#[derive(Debug, Serialize)]
pub struct CompleteSubSessionResponse {
    pub success: bool,
    pub resolutions_applied: usize,
    pub message: String,
}

/// POST /api/session/:id/subsession/:child_id/complete - Complete sub-session
async fn complete_subsession(
    State(state): State<AgentState>,
    Path((parent_id, child_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<CompleteSubSessionRequest>,
) -> Result<Json<CompleteSubSessionResponse>, (StatusCode, String)> {
    tracing::info!("Completing sub-session: {} (apply={})", child_id, req.apply);

    // Get child session
    let child = {
        let mut sessions = state.sessions.write().await;
        sessions.remove(&child_id)
    }
    .ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("Sub-session {} not found", child_id),
        )
    })?;

    if child.parent_session_id != Some(parent_id) {
        return Err((
            StatusCode::BAD_REQUEST,
            "Invalid parent-child relationship".to_string(),
        ));
    }

    // Extract resolution data if this is a Resolution sub-session
    let (resolutions_count, bound_entities) =
        if let SubSessionType::Resolution(r) = &child.sub_session_type {
            // Build BoundEntity entries from resolutions
            let mut entities = Vec::new();
            for unresolved in &r.unresolved_refs {
                if let Some(resolved_value) = r.resolutions.get(&unresolved.ref_id) {
                    // Find the matching entity info from initial_matches
                    let match_info = unresolved
                        .initial_matches
                        .iter()
                        .find(|m| &m.value == resolved_value);

                    if let Some(info) = match_info {
                        // Parse UUID from resolved value
                        if let Ok(uuid) = Uuid::parse_str(resolved_value) {
                            entities.push((
                                unresolved.ref_id.clone(),
                                crate::api::session::BoundEntity {
                                    id: uuid,
                                    entity_type: unresolved.entity_type.clone(),
                                    display_name: info.display.clone(),
                                },
                            ));
                        }
                    }
                }
            }
            (r.resolutions.len(), entities)
        } else {
            (0, Vec::new())
        };

    if req.apply && resolutions_count > 0 {
        // Apply resolutions to parent session's bindings
        let mut sessions = state.sessions.write().await;
        if let Some(parent) = sessions.get_mut(&parent_id) {
            for (ref_id, bound_entity) in &bound_entities {
                // Add to parent's bindings using the ref_id as the binding name
                parent
                    .context
                    .bindings
                    .insert(ref_id.clone(), bound_entity.clone());
                tracing::info!(
                    "Applied resolution: {} -> {} ({})",
                    ref_id,
                    bound_entity.id,
                    bound_entity.display_name
                );
            }
        }
    }

    Ok(Json(CompleteSubSessionResponse {
        success: true,
        resolutions_applied: if req.apply { resolutions_count } else { 0 },
        message: format!(
            "Sub-session completed. {} resolutions {}.",
            resolutions_count,
            if req.apply { "applied" } else { "discarded" }
        ),
    }))
}

/// POST /api/session/:id/subsession/:child_id/cancel - Cancel sub-session
async fn cancel_subsession(
    State(state): State<AgentState>,
    Path((parent_id, child_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<CompleteSubSessionResponse>, (StatusCode, String)> {
    tracing::info!("Cancelling sub-session: {}", child_id);

    // Remove child session
    let child = {
        let mut sessions = state.sessions.write().await;
        sessions.remove(&child_id)
    }
    .ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("Sub-session {} not found", child_id),
        )
    })?;

    if child.parent_session_id != Some(parent_id) {
        return Err((
            StatusCode::BAD_REQUEST,
            "Invalid parent-child relationship".to_string(),
        ));
    }

    Ok(Json(CompleteSubSessionResponse {
        success: true,
        resolutions_applied: 0,
        message: "Sub-session cancelled. No changes applied.".to_string(),
    }))
}

/// Generate help text showing all available MCP tools/commands
fn generate_commands_help() -> String {
    r#"# Available Commands

## DSL Operations

**Commands:**
| Command | Description |
|---------|-------------|
| `dsl_validate` | Parse and validate DSL syntax/semantics |
| `dsl_execute` | Execute DSL against database |
| `dsl_plan` | Show execution plan without running |
| `dsl_generate` | Generate DSL from natural language |
| `dsl_lookup` | Look up database IDs (prevents hallucination) |
| `dsl_complete` | Get completions for verbs, domains, products |
| `dsl_signature` | Get verb signature with parameters |

**Natural Language:**
| Say | Effect |
|-----|--------|
| "create a CBU for Acme Corp" | → `dsl_generate` → `(cbu.create :name "Acme Corp")` |
| "add John Smith as director" | → `dsl_generate` → `(cbu.assign-role ...)` |
| "what verbs are there for kyc?" | → `dsl_complete` or `/verbs kyc` |

**DSL Syntax:**
```clojure
(cbu.create :name "Acme Fund" :jurisdiction "LU" :as @cbu)
(entity.create :name "John Smith" :type PERSON :as @person)
(cbu.assign-role :cbu-id @cbu :entity-id @person :role DIRECTOR)
```

**Response:** `{"success": true, "bindings": {"@cbu": "uuid-..."}, "steps_executed": 1}`

---

## Session & Navigation

**Commands:**
| Command | Description |
|---------|-------------|
| `session_load_cbu` | Load single CBU into session |
| `session_load_jurisdiction` | Load all CBUs in jurisdiction |
| `session_load_galaxy` | Load all CBUs under apex entity |
| `session_unload_cbu` | Remove CBU from session |
| `session_clear` | Clear all CBUs |
| `session_undo` / `session_redo` | History navigation |
| `session_info` / `session_list` | Query session state |

**Natural Language:**
| Say | DSL |
|-----|-----|
| "load the Allianz book" | `(session.load-galaxy :apex-name "Allianz")` |
| "show me Luxembourg CBUs" | `(session.load-jurisdiction :jurisdiction "LU")` |
| "focus on Acme Fund" | `(session.load-cbu :cbu-name "Acme Fund")` |
| "clear the session" | `(session.clear)` |
| "undo" / "go back" | `(session.undo)` |

**Response:** `{"loaded": true, "cbu_count": 15, "cbu_ids": [...], "scope_size": 15}`

---

## CBU & Entity Operations

**Commands:**
| Command | Description |
|---------|-------------|
| `cbu_get` | Get CBU with entities, roles, documents |
| `cbu_list` | List/search CBUs |
| `entity_get` | Get entity details |
| `entity_search` | Smart search with disambiguation |

**Natural Language:**
| Say | DSL |
|-----|-----|
| "create a fund called Lux Alpha" | `(cbu.create :name "Lux Alpha" :type FUND)` |
| "add custody product" | `(cbu.add-product :product "Custody")` |
| "who are the directors?" | `(cbu.list-roles :role DIRECTOR)` |
| "create person John Smith" | `(entity.create :name "John Smith" :type PERSON)` |
| "find BlackRock" | `(entity.search :query "BlackRock")` |

**Response:** `{"cbu": {"cbu_id": "...", "name": "Lux Alpha"}, "entities": [...], "roles": [...]}`

---

## View & Zoom (ESPER Navigation)

**Natural Language:**
| Say | Effect |
|-----|--------|
| "enhance" / "zoom in" / "closer" | Zoom in on current view |
| "zoom out" / "pull back" | Zoom out to wider view |
| "universe" / "show everything" | View all CBUs |
| "galaxy view" / "cluster" | View CBU groups |
| "land on" / "system view" | Focus single CBU |
| "drill through" / "go deeper" | Drill into entity |
| "surface" / "come back up" | Return from drill |
| "x-ray" / "show hidden" | Show hidden layers |
| "follow the money" | Trace money flow |
| "who controls this?" | Show control chain |

**DSL Syntax:**
```clojure
(view.universe)
(view.book :apex-name "Allianz")
(view.cbu :cbu-id @cbu :mode trading)
(view.drill :entity-id @entity)
(view.surface)
```

---

## KYC & UBO

**Natural Language:**
| Say | DSL |
|-----|-----|
| "start KYC for this CBU" | `(kyc.start-case :cbu-id @cbu)` |
| "who owns this company?" | `(ubo.discover :entity-id @entity)` |
| "show ownership chain" | `(ubo.trace :entity-id @entity)` |
| "screen this person" | `(screening.run :entity-id @person)` |

**DSL Syntax:**
```clojure
(kyc.start-case :cbu-id @cbu :case-type ONBOARDING :as @case)
(kyc.add-document :case-id @case :doc-type PASSPORT :entity-id @person)
(ubo.discover :entity-id @entity :threshold 0.25)
(screening.run :entity-id @person :type PEP_SANCTIONS)
```

**Response:** `{"case_id": "...", "status": "IN_PROGRESS", "documents_required": [...]}`

---

## Trading Profile & Custody

**Natural Language:**
| Say | DSL |
|-----|-----|
| "set up trading for equities" | `(trading-profile.add-instrument-class :class EQUITY)` |
| "add XLON market" | `(trading-profile.add-market :mic "XLON")` |
| "create SSI for USD" | `(custody.create-ssi :currency "USD" ...)` |
| "show trading matrix" | `trading_matrix_get` |

**DSL Syntax:**
```clojure
(trading-profile.create :cbu-id @cbu :as @profile)
(trading-profile.add-instrument-class :profile-id @profile :class EQUITY)
(trading-profile.add-market :profile-id @profile :mic "XLON")
(custody.create-ssi :cbu-id @cbu :currency "USD" :account "..." :as @ssi)
```

---

## Research Macros

**Commands:**
| Command | Description |
|---------|-------------|
| `research_list` | List available research macros |
| `research_execute` | Execute research (LLM + web) |
| `research_approve` / `research_reject` | Review results |

**Natural Language:**
| Say | Effect |
|-----|--------|
| "research Allianz group structure" | Execute GLEIF hierarchy lookup |
| "find beneficial owners of Acme" | UBO research macro |
| "approve the research" | Accept results, get DSL |

**Response:** `{"results": {...}, "suggested_verbs": ["(entity.create ...)", "(ownership.link ...)"]}`

---

## Learning & Feedback

**Commands:**
| Command | Description |
|---------|-------------|
| `verb_search` | Search verbs by natural language |
| `intent_feedback` | Record correction for learning |
| `learning_analyze` | Find patterns to learn |
| `learning_apply` | Apply pattern→verb mapping |
| `embeddings_status` | Check semantic coverage |

**Natural Language:**
| Say | Effect |
|-----|--------|
| "that should have been cbu.create" | Records correction → learning candidate |
| "block this verb for this phrase" | Adds to blocklist |

**Response:** `{"recorded": true, "occurrence_count": 3, "will_auto_apply_at": 5}`

---

## Promotion Pipeline (Quality-Gated Learning)

**Commands:**
| Command | Description |
|---------|-------------|
| `promotion_run_cycle` | Run full promotion pipeline |
| `promotion_candidates` | List auto-promotable candidates |
| `promotion_review_queue` | List manual review queue |
| `promotion_approve` | Approve candidate |
| `promotion_reject` | Reject and blocklist |
| `promotion_health` | Weekly health metrics |

**Thresholds:** 5+ occurrences, 80%+ success rate, 24h+ age, collision-safe

**Response:** `{"promoted": ["spin up a fund → cbu.create"], "skipped": 2, "collisions": 0}`

---

## Teaching (Direct Pattern Learning)

**Commands:**
| Command | Description |
|---------|-------------|
| `teach_phrase` | Add phrase→verb mapping |
| `unteach_phrase` | Remove mapping (with audit) |
| `teaching_status` | View taught patterns |

**Natural Language:**
| Say | Maps To |
|-----|---------|
| "teach: 'spin up a fund' = cbu.create" | `agent.teach` |
| "learn this phrase" / "remember this" | `agent.teach` |
| "forget this phrase" / "unteach" | `agent.unteach` |
| "what have I taught?" | `agent.teaching-status` |

**DSL Syntax:**
```clojure
(agent.teach :phrase "spin up a fund" :verb "cbu.create")
(agent.unteach :phrase "spin up a fund" :reason "too_generic")
(agent.teaching-status :limit 20)
```

**Response:**
```json
{"taught": true, "phrase": "spin up a fund", "verb": "cbu.create",
 "message": "Taught: 'spin up a fund' → cbu.create", "needs_reembed": true}
```

---

## Contracts & Onboarding

**Commands:**
| Command | Description |
|---------|-------------|
| `contract_create` | Create legal contract for client |
| `contract_get` | Get contract details |
| `contract_list` | List contracts (filter by client) |
| `contract_terminate` | Terminate contract |
| `contract_add_product` | Add product with rate card |
| `contract_subscribe` | Subscribe CBU to contract+product |
| `contract_can_onboard` | Check onboarding eligibility |

**Natural Language:**
| Say | DSL |
|-----|-----|
| "create contract for allianz" | `(contract.create :client "allianz" :effective-date "2024-01-01")` |
| "add custody to contract" | `(contract.add-product :contract-id @contract :product "CUSTODY")` |
| "subscribe CBU to custody" | `(contract.subscribe :cbu-id @cbu :contract-id @contract :product "CUSTODY")` |
| "can this CBU onboard to custody?" | `(contract.can-onboard :cbu-id @cbu :product "CUSTODY")` |
| "show allianz contract" | `(contract.for-client :client "allianz")` |
| "list subscriptions for client" | `(contract.list-subscriptions :client "allianz")` |

**Key Concept:** CBU onboarding requires contract+product subscription. No contract = no onboarding.

```
legal_contracts (client_label: "allianz")
    └── contract_products (product_code, rate_card_id)
         └── cbu_subscriptions (cbu_id) ← Onboarding gate
```

**DSL Syntax:**
```clojure
;; Create contract with products
(contract.create :client "allianz" :reference "MSA-2024-001" :effective-date "2024-01-01" :as @contract)
(contract.add-product :contract-id @contract :product "CUSTODY" :rate-card-id @rate)
(contract.add-product :contract-id @contract :product "FUND_ADMIN")

;; Subscribe CBU (onboarding)
(contract.subscribe :cbu-id @cbu :contract-id @contract :product "CUSTODY")

;; Check eligibility
(contract.can-onboard :cbu-id @cbu :product "CUSTODY")
```

**Response:** `{"contract_id": "...", "client_label": "allianz", "products": ["CUSTODY", "FUND_ADMIN"]}`

---

## Workflow & Templates

**Commands:**
| Command | Description |
|---------|-------------|
| `workflow_start` | Start workflow instance |
| `workflow_status` | Get status with blockers |
| `workflow_advance` | Advance (evaluates guards) |
| `template_list` | List templates |
| `template_expand` | Expand to DSL |

**Natural Language:**
| Say | Effect |
|-----|--------|
| "start onboarding workflow" | Creates workflow instance |
| "what's blocking?" | Shows blockers |
| "advance the workflow" | Evaluates guards, moves state |

---

## Batch Operations

**Commands:** `batch_start`, `batch_add_entities`, `batch_confirm_keyset`,
`batch_set_scalar`, `batch_get_state`, `batch_expand_current`,
`batch_record_result`, `batch_skip_current`, `batch_cancel`

**Use case:** Apply same template to multiple entities (e.g., add role to 50 people)

---

*Type `/commands` or `/help` to see this list.*
*Type `/verbs` to see all DSL verbs, `/verbs <domain>` for specific domain.*
*Type `/verbs agent` for agent verbs, `/verbs session` for session verbs.*"#
        .to_string()
}

/// Generate help text showing all available DSL verbs
///
/// This shows the same information the agent "sees" when generating DSL:
/// - Verb name and description
/// - Required and optional arguments with types
/// - Lookup info for entity resolution
fn generate_verbs_help(domain_filter: Option<&str>) -> String {
    let reg = registry();
    let mut output = String::new();

    // Group verbs by domain
    let mut domains: std::collections::BTreeMap<
        &str,
        Vec<&crate::dsl_v2::verb_registry::UnifiedVerbDef>,
    > = std::collections::BTreeMap::new();

    for verb in reg.all_verbs() {
        if let Some(filter) = domain_filter {
            if verb.domain != filter {
                continue;
            }
        }
        domains.entry(&verb.domain).or_default().push(verb);
    }

    if domains.is_empty() {
        if let Some(filter) = domain_filter {
            let available = reg.domains().join(", ");
            return format!(
                "Unknown domain: '{}'\n\n\
                 **Usage:** `/commands <domain>` or `/verbs <domain>`\n\n\
                 **Available domains:** {}\n\n\
                 **Examples:**\n\
                 - `/commands session` - session scope management\n\
                 - `/commands kyc` - KYC case verbs\n\
                 - `/commands entity` - entity management\n\
                 - `/verbs` - show all verbs",
                filter, available
            );
        }
        return "No verbs loaded in registry.".to_string();
    }

    // Header
    if let Some(filter) = domain_filter {
        output.push_str(&format!("# DSL Verbs: {} domain\n\n", filter));
    } else {
        output.push_str(&format!(
            "# DSL Verbs ({} verbs across {} domains)\n\n",
            reg.len(),
            reg.domains().len()
        ));
        output.push_str("*Use `/verbs <domain>` for detailed view of a specific domain.*\n\n");
        output.push_str("## Available Domains\n\n");
        for domain in reg.domains() {
            let count = domains.get(domain.as_str()).map(|v| v.len()).unwrap_or(0);
            output.push_str(&format!("- **{}** ({} verbs)\n", domain, count));
        }
        output.push_str("\n---\n\n");
    }

    // Verb details
    for (domain, verbs) in &domains {
        if domain_filter.is_none() {
            // Summary view - just list verbs with descriptions
            output.push_str(&format!("## {}\n\n", domain));
            for verb in verbs {
                output.push_str(&format!(
                    "- `{}.{}` - {}\n",
                    verb.domain, verb.verb, verb.description
                ));
            }
            output.push('\n');
        } else {
            // Detailed view - show full prompt helper info
            for verb in verbs {
                output.push_str(&format!("## `{}.{}`\n\n", verb.domain, verb.verb));
                output.push_str(&format!("{}\n\n", verb.description));

                // Arguments table
                if !verb.args.is_empty() {
                    output.push_str("### Arguments\n\n");
                    output.push_str("| Name | Type | Required | Description |\n");
                    output.push_str("|------|------|----------|-------------|\n");

                    for arg in &verb.args {
                        let req = if arg.required { "✓" } else { "" };
                        let desc = if arg.description.is_empty() {
                            "-".to_string()
                        } else {
                            arg.description.replace('\n', " ")
                        };
                        let type_info = if let Some(ref lookup) = arg.lookup {
                            if let Some(ref entity_type) = lookup.entity_type {
                                format!("{} (→{})", arg.arg_type, entity_type)
                            } else {
                                arg.arg_type.clone()
                            }
                        } else {
                            arg.arg_type.clone()
                        };
                        output.push_str(&format!(
                            "| `{}` | {} | {} | {} |\n",
                            arg.name, type_info, req, desc
                        ));
                    }
                    output.push('\n');
                }

                // Example DSL
                let required_args: Vec<_> = verb
                    .args
                    .iter()
                    .filter(|a| a.required)
                    .map(|a| format!(":{} ...", a.name))
                    .collect();
                let optional_args: Vec<_> = verb
                    .args
                    .iter()
                    .filter(|a| !a.required)
                    .take(2) // Show first 2 optional args
                    .map(|a| format!(":{} ...", a.name))
                    .collect();

                output.push_str("### Example\n\n");
                output.push_str("```clojure\n");
                if optional_args.is_empty() {
                    output.push_str(&format!(
                        "({}.{} {})\n",
                        verb.domain,
                        verb.verb,
                        required_args.join(" ")
                    ));
                } else {
                    output.push_str(&format!(
                        "({}.{} {} {})\n",
                        verb.domain,
                        verb.verb,
                        required_args.join(" "),
                        optional_args.join(" ")
                    ));
                }
                output.push_str("```\n\n");

                output.push_str("---\n\n");
            }
        }
    }

    output
}

/// POST /api/session/:id/chat - Process chat message and generate DSL via LLM
///
/// Pipeline: User message → Intent extraction (tool call) → DSL builder → Linter → Feedback loop
///
/// This mirrors the Claude Code workflow:
/// 1. Extract structured intents from natural language (tool call)
/// 2. Build DSL from intents (deterministic Rust code)
/// 3. Validate with CSG linter (same as Zed/LSP)
/// 4. If errors, feed back to agent and retry
/// 5. Write valid DSL to session file
///
/// Supports both Anthropic and OpenAI backends via AGENT_BACKEND env var.
///
/// This handler delegates to AgentService for the core chat logic:
/// - Intent extraction via LLM
/// - Entity resolution via EntityGateway
/// - DSL generation with validation/retry loop
async fn chat_session(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, StatusCode> {
    tracing::info!("=== CHAT SESSION START ===");
    tracing::info!("Session ID: {}", session_id);
    tracing::info!("Message: {:?}", req.message);
    tracing::info!("CBU ID: {:?}", req.cbu_id);

    // Get session (create if needed - matches get_session behavior)
    let session = {
        let sessions = state.sessions.read().await;
        tracing::info!("Looking up session in store...");
        match sessions.get(&session_id) {
            Some(s) => {
                tracing::info!("Session found, state: {:?}", s.state);
                s.clone()
            }
            None => {
                tracing::info!("Session not found, creating new session: {}", session_id);
                drop(sessions); // Release read lock before acquiring write lock
                let mut new_session = UnifiedSession::new_for_entity(None, "cbu", None, None);
                new_session.id = session_id; // Use the requested ID
                let mut sessions = state.sessions.write().await;
                sessions.insert(session_id, new_session.clone());
                tracing::info!("Session created: {}", session_id);
                new_session
            }
        }
    };

    // Handle slash commands: /commands, /verbs, /help with optional domain filter
    let trimmed_msg = req.message.trim().to_lowercase();
    let parts: Vec<&str> = trimmed_msg.split_whitespace().collect();

    match parts.as_slice() {
        // /help or /commands (no args) - show MCP tools overview
        ["/help"] | ["/commands"] | ["show", "commands"] => {
            return Ok(Json(ChatResponse {
                message: generate_commands_help(),
                dsl: None,
                session_state: to_session_state_enum(&session.state),
                commands: None,
                disambiguation_request: None,
                verb_disambiguation: None,
                unresolved_refs: None,
                current_ref_index: None,
                dsl_hash: None,
            }));
        }
        // /commands <domain> or /verbs <domain> - show verbs for domain
        ["/commands" | "/verbs", domain] => {
            return Ok(Json(ChatResponse {
                message: generate_verbs_help(Some(domain)),
                dsl: None,
                session_state: to_session_state_enum(&session.state),
                commands: None,
                disambiguation_request: None,
                verb_disambiguation: None,
                unresolved_refs: None,
                current_ref_index: None,
                dsl_hash: None,
            }));
        }
        // /verbs (no args) - show all verbs
        ["/verbs"] => {
            return Ok(Json(ChatResponse {
                message: generate_verbs_help(None),
                dsl: None,
                session_state: to_session_state_enum(&session.state),
                commands: None,
                disambiguation_request: None,
                verb_disambiguation: None,
                unresolved_refs: None,
                current_ref_index: None,
                dsl_hash: None,
            }));
        }
        _ => {} // Not a slash command, continue to LLM
    }

    // Make session mutable for the rest of the handler
    let mut session = session;

    // Create LLM client (uses AGENT_BACKEND env var to select provider)
    let llm_client = match crate::agentic::create_llm_client() {
        Ok(client) => client,
        Err(e) => {
            let error_msg = format!("Error: LLM client initialization failed: {}", e);
            return Ok(Json(ChatResponse {
                message: error_msg,
                dsl: None,
                session_state: to_session_state_enum(&session.state),
                commands: None,
                disambiguation_request: None,
                verb_disambiguation: None,
                unresolved_refs: None,
                current_ref_index: None,
                dsl_hash: None,
            }));
        }
    };

    tracing::info!(
        "Chat session using {} ({})",
        llm_client.provider_name(),
        llm_client.model_name()
    );

    // Delegate to centralized AgentService (single pipeline)
    let response = match state
        .agent_service
        .process_chat(&mut session, &req, llm_client)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("AgentService error: {}", e);
            // Return error as JSON response instead of opaque 500
            return Ok(Json(ChatResponse {
                message: format!("Agent error: {}", e),
                dsl: None,
                session_state: to_session_state_enum(&session.state),
                commands: None,
                disambiguation_request: None,
                verb_disambiguation: None,
                unresolved_refs: None,
                current_ref_index: None,
                dsl_hash: None,
            }));
        }
    };

    // =========================================================================
    // CAPTURE FEEDBACK FOR LEARNING LOOP
    // Record the verb match so we can correlate with execution outcome later
    // =========================================================================
    if !response.intents.is_empty() {
        let first_intent = &response.intents[0];

        // Determine match method: DirectDsl if user typed DSL, Semantic if LLM generated
        let is_direct_dsl = req.message.trim().starts_with('(');
        let match_method = if is_direct_dsl {
            ob_semantic_matcher::MatchMethod::DirectDsl
        } else {
            ob_semantic_matcher::MatchMethod::Semantic
        };

        // Build a MatchResult from the intent for feedback capture
        let match_result = ob_semantic_matcher::MatchResult {
            verb_name: first_intent.verb.clone(),
            pattern_phrase: req.message.clone(), // The user's input
            similarity: 1.0,                     // Direct DSL or LLM-selected verb
            match_method,
            category: "chat".to_string(),
            is_agent_bound: true,
        };

        match state
            .feedback_service
            .capture_match(
                session_id,
                &req.message,
                ob_semantic_matcher::feedback::InputSource::Chat,
                Some(&match_result),
                &[], // No alternatives from LLM path
                session.context.domain_hint.as_deref(),
                session.context.stage_focus.as_deref(),
            )
            .await
        {
            Ok(interaction_id) => {
                // Store both interaction_id (for record_outcome) and feedback_id (for FK)
                session.context.pending_interaction_id = Some(interaction_id);

                // intent_feedback uses BIGSERIAL id, need to look it up by interaction_id
                if let Ok(Some(feedback_id)) = sqlx::query_scalar::<_, i64>(
                    r#"SELECT id FROM "ob-poc".intent_feedback WHERE interaction_id = $1"#,
                )
                .bind(interaction_id)
                .fetch_optional(&state.pool)
                .await
                {
                    session.context.pending_feedback_id = Some(feedback_id);
                    tracing::debug!(
                        "Captured feedback: interaction_id={}, feedback_id={}, method={:?}",
                        interaction_id,
                        feedback_id,
                        match_method
                    );
                }
            }
            Err(e) => {
                tracing::warn!("Failed to capture feedback: {}", e);
            }
        }
    }

    // =========================================================================
    // TRACK PROPOSED DSL FOR DIFF (learning from user edits)
    // =========================================================================
    if let Some(ref dsl_source) = response.dsl_source {
        // Only set proposed_dsl if agent generated it (not DirectDsl)
        let is_direct_dsl = req.message.trim().starts_with('(');
        if !is_direct_dsl {
            state
                .session_manager
                .set_proposed_dsl(session_id, dsl_source)
                .await;
        }
    }

    // Persist session changes back
    {
        let mut sessions = state.sessions.write().await;
        sessions.insert(session_id, session.clone());
    }

    // Notify watchers that session changed
    state.session_manager.notify(session_id).await;

    // Build DslState from response fields
    let dsl_state = build_dsl_state(
        response.dsl_source.as_ref(),
        response.ast.as_ref(),
        response.can_execute,
        &response.intents,
        &response.validation_results,
        &session.context.bindings,
    );

    // Return response using API types (single source of truth)
    Ok(Json(ChatResponse {
        message: response.message,
        dsl: dsl_state,
        session_state: api_session_state_to_enum(&response.session_state),
        commands: response.commands,
        disambiguation_request: response
            .disambiguation
            .as_ref()
            .map(to_api_disambiguation_request),
        verb_disambiguation: response.verb_disambiguation,
        unresolved_refs: response
            .unresolved_refs
            .as_ref()
            .map(|refs| api_unresolved_refs_to_api(refs)),
        current_ref_index: response.current_ref_index,
        dsl_hash: response.dsl_hash,
    }))
}

/// POST /api/session/:id/execute - Execute DSL
async fn execute_session_dsl(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<Option<ExecuteDslRequest>>,
) -> Result<Json<ExecuteResponse>, StatusCode> {
    tracing::debug!("[EXEC] Session {} - START execute_session_dsl", session_id);

    // Get or create execution context
    let (mut context, current_state, user_intent) = {
        let sessions = state.sessions.read().await;
        let session = sessions.get(&session_id).ok_or(StatusCode::NOT_FOUND)?;

        tracing::debug!(
            "[EXEC] Session {} - Loading context, named_refs: {:?}",
            session_id,
            session.context.named_refs
        );

        // Extract user intent from last user message
        let user_intent = session
            .messages
            .iter()
            .rev()
            .find(|m| m.role == MessageRole::User)
            .map(|m| m.content.clone())
            .unwrap_or_else(|| "Direct DSL execution".to_string());

        (session.context.clone(), session.state.clone(), user_intent)
    };

    // Get DSL and check for pre-compiled plan in session
    // If DSL matches session.pending.source, use the pre-compiled plan
    // Otherwise run full pipeline (DSL was edited externally)
    let (dsl, precompiled_plan, cached_ast) = {
        let sessions = state.sessions.read().await;
        let session = sessions.get(&session_id).ok_or(StatusCode::NOT_FOUND)?;

        // Determine DSL source - prefer explicit request, then run_sheet current entry
        let dsl_source = if let Some(ref req) = req {
            if let Some(ref dsl_str) = req.dsl {
                dsl_str.clone()
            } else {
                session.run_sheet.combined_dsl().unwrap_or_default()
            }
        } else {
            session.run_sheet.combined_dsl().unwrap_or_default()
        };

        // Note: UnifiedSession.RunSheetEntry doesn't store pre-compiled plan/ast
        // Always run full pipeline. In the future, we could add optional caching.
        let plan: Option<crate::dsl_v2::ExecutionPlan> = None;
        let ast: Option<Vec<dsl_core::ast::Statement>> = None;
        tracing::debug!("[EXEC] Running full pipeline (UnifiedSession doesn't cache plans)");

        (dsl_source, plan, ast)
    };

    if dsl.is_empty() {
        return Ok(Json(ExecuteResponse {
            success: false,
            results: Vec::new(),
            errors: vec!["No DSL to execute".to_string()],
            new_state: current_state.into(),
            bindings: None,
        }));
    }

    // =========================================================================
    // START GENERATION LOG
    // Get feedback IDs from session context if available (set by chat handler)
    // =========================================================================
    let (intent_feedback_id, pending_interaction_id) = {
        let sessions = state.sessions.read().await;
        sessions
            .get(&session_id)
            .map(|s| {
                (
                    s.context.pending_feedback_id,
                    s.context.pending_interaction_id,
                )
            })
            .unwrap_or((None, None))
    };

    let log_id = state
        .generation_log
        .start_log(
            &user_intent,
            "session",
            Some(session_id),
            context.last_cbu_id,
            None,
            intent_feedback_id,
        )
        .await
        .ok();

    let start_time = std::time::Instant::now();

    // Create execution context
    let mut exec_ctx = ExecutionContext::new().with_audit_user(&format!("session-{}", session_id));

    // Pre-bind symbols from session context
    if let Some(id) = context.last_cbu_id {
        exec_ctx.bind("last_cbu", id);
        tracing::debug!("[EXEC] Pre-bound last_cbu = {}", id);
    }
    if let Some(id) = context.last_entity_id {
        exec_ctx.bind("last_entity", id);
        tracing::debug!("[EXEC] Pre-bound last_entity = {}", id);
    }
    // Pre-bind all named references from previous executions
    tracing::debug!(
        "[EXEC] Pre-binding {} named_refs: {:?}",
        context.named_refs.len(),
        context.named_refs
    );
    for (name, id) in &context.named_refs {
        exec_ctx.bind(name, *id);
        tracing::debug!("[EXEC] Pre-bound @{} = {}", name, id);
    }

    // Pre-populate session's CBU scope for bulk operations
    // Verbs can access this via ctx.session_cbu_ids() or @session_cbus symbol
    if !context.cbu_ids.is_empty() {
        tracing::debug!(
            "[EXEC] Pre-populating session CBU scope: {} CBUs",
            context.cbu_ids.len()
        );
        exec_ctx.set_session_cbu_ids(context.cbu_ids.clone());
    }

    // =========================================================================
    // GET OR BUILD EXECUTION PLAN
    // If we have a pre-compiled plan (DSL unchanged), use it directly.
    // Otherwise run full pipeline: Parse → Validate → Compile
    // Returns (plan, ast_for_persistence) - AST needed for dsl_instances table
    // =========================================================================
    let (plan, ast_for_persistence): (
        crate::dsl_v2::execution_plan::ExecutionPlan,
        Option<Vec<crate::dsl_v2::Statement>>,
    ) = if let Some(cached_plan) = precompiled_plan {
        tracing::info!(
            "[EXEC] Using pre-compiled execution plan ({} steps)",
            cached_plan.len()
        );
        (cached_plan, cached_ast)
    } else {
        tracing::info!("[EXEC] Running full pipeline: parse → validate → compile");

        // PARSE DSL
        let program = match parse_program(&dsl) {
            Ok(p) => p,
            Err(e) => {
                let parse_error = format!("Parse error: {}", e);

                // Log failed attempt
                if let Some(lid) = log_id {
                    let attempt = GenerationAttempt {
                        attempt: 1,
                        timestamp: chrono::Utc::now(),
                        prompt_template: None,
                        prompt_text: String::new(),
                        raw_response: String::new(),
                        extracted_dsl: Some(dsl.clone()),
                        parse_result: ParseResult {
                            success: false,
                            error: Some(parse_error.clone()),
                        },
                        lint_result: LintResult {
                            valid: false,
                            errors: vec![],
                            warnings: vec![],
                        },
                        compile_result: CompileResult {
                            success: false,
                            error: None,
                            step_count: 0,
                        },
                        latency_ms: Some(start_time.elapsed().as_millis() as i32),
                        input_tokens: None,
                        output_tokens: None,
                    };
                    let _ = state.generation_log.add_attempt(lid, &attempt).await;
                    let _ = state.generation_log.mark_failed(lid).await;
                }

                return Ok(Json(ExecuteResponse {
                    success: false,
                    results: Vec::new(),
                    errors: vec![parse_error],
                    new_state: current_state.into(),
                    bindings: None,
                }));
            }
        };

        // CSG VALIDATION (includes dataflow)
        tracing::debug!("[EXEC] Starting CSG validation");
        tracing::debug!(
            "[EXEC] Validation known_symbols: {:?}",
            context.named_refs.keys().collect::<Vec<_>>()
        );
        let validator_result = async {
            let v = SemanticValidator::new(state.pool.clone()).await?;
            v.with_csg_linter().await
        }
        .await;

        if let Ok(mut validator) = validator_result {
            use crate::dsl_v2::validation::{Severity, ValidationContext, ValidationRequest};
            let request = ValidationRequest {
                source: dsl.clone(),
                context: ValidationContext::default()
                    .with_known_symbols(context.named_refs.clone()),
            };
            tracing::debug!(
                "[EXEC] ValidationRequest.context.known_symbols: {:?}",
                request.context.known_symbols
            );
            if let crate::dsl_v2::validation::ValidationResult::Err(diagnostics) =
                validator.validate(&request).await
            {
                tracing::debug!(
                    "[EXEC] Validation returned {} diagnostics",
                    diagnostics.len()
                );
                for diag in &diagnostics {
                    tracing::debug!("[EXEC] Diagnostic: [{:?}] {}", diag.severity, diag.message);
                }
                let csg_errors: Vec<String> = diagnostics
                    .iter()
                    .filter(|d| d.severity == Severity::Error)
                    .map(|d| format!("[{}] {}", d.code.as_str(), d.message))
                    .collect();
                if !csg_errors.is_empty() {
                    // Log failed attempt
                    if let Some(lid) = log_id {
                        let attempt = GenerationAttempt {
                            attempt: 1,
                            timestamp: chrono::Utc::now(),
                            prompt_template: None,
                            prompt_text: String::new(),
                            raw_response: String::new(),
                            extracted_dsl: Some(dsl.clone()),
                            parse_result: ParseResult {
                                success: true,
                                error: None,
                            },
                            lint_result: LintResult {
                                valid: false,
                                errors: csg_errors.clone(),
                                warnings: vec![],
                            },
                            compile_result: CompileResult {
                                success: false,
                                error: None,
                                step_count: 0,
                            },
                            latency_ms: Some(start_time.elapsed().as_millis() as i32),
                            input_tokens: None,
                            output_tokens: None,
                        };
                        let _ = state.generation_log.add_attempt(lid, &attempt).await;
                        let _ = state.generation_log.mark_failed(lid).await;
                    }

                    return Ok(Json(ExecuteResponse {
                        success: false,
                        results: Vec::new(),
                        errors: csg_errors,
                        new_state: current_state.into(),
                        bindings: None,
                    }));
                }
            }
        }

        // COMPILE (includes DAG toposort)
        // Capture statements for AST persistence before consuming program
        let statements = program.statements.clone();
        match compile(&program) {
            Ok(p) => (p, Some(statements)),
            Err(e) => {
                let compile_error = format!("Compile error: {}", e);

                // Log failed attempt
                if let Some(lid) = log_id {
                    let attempt = GenerationAttempt {
                        attempt: 1,
                        timestamp: chrono::Utc::now(),
                        prompt_template: None,
                        prompt_text: String::new(),
                        raw_response: String::new(),
                        extracted_dsl: Some(dsl.clone()),
                        parse_result: ParseResult {
                            success: true,
                            error: None,
                        },
                        lint_result: LintResult {
                            valid: false,
                            errors: vec![compile_error.clone()],
                            warnings: vec![],
                        },
                        compile_result: CompileResult {
                            success: false,
                            error: Some(compile_error.clone()),
                            step_count: 0,
                        },
                        latency_ms: Some(start_time.elapsed().as_millis() as i32),
                        input_tokens: None,
                        output_tokens: None,
                    };
                    let _ = state.generation_log.add_attempt(lid, &attempt).await;
                    let _ = state.generation_log.mark_failed(lid).await;
                }

                return Ok(Json(ExecuteResponse {
                    success: false,
                    results: Vec::new(),
                    errors: vec![compile_error],
                    new_state: current_state.into(),
                    bindings: None,
                }));
            }
        }
    };

    // =========================================================================
    // EXPANSION STAGE - Determine batch policy and derive locks
    // =========================================================================
    let templates = runtime_registry().templates();
    let expansion_result = expand_templates_simple(&dsl, templates);

    let expansion_report = match expansion_result {
        Ok(output) => {
            tracing::debug!(
                "[EXEC] Expansion complete: batch_policy={:?}, locks={}, statements={}",
                output.report.batch_policy,
                output.report.derived_lock_set.len(),
                output.report.expanded_statement_count
            );
            Some(output.report)
        }
        Err(e) => {
            tracing::warn!(
                "[EXEC] Expansion failed (continuing with best-effort): {}",
                e
            );
            None
        }
    };

    // Persist expansion report for audit trail (async, non-blocking)
    if let Some(ref report) = expansion_report {
        let expansion_audit = state.expansion_audit.clone();
        let report_clone = report.clone();
        tokio::spawn(async move {
            if let Err(e) = expansion_audit.save(session_id, &report_clone).await {
                tracing::error!(
                    session_id = %session_id,
                    expansion_id = %report_clone.expansion_id,
                    "Failed to persist expansion report: {}",
                    e
                );
            }
        });
    }

    // Determine batch policy from expansion report (default: BestEffort)
    let batch_policy = expansion_report
        .as_ref()
        .map(|r| r.batch_policy)
        .unwrap_or(BatchPolicy::BestEffort);

    // =========================================================================
    // EXECUTE - Route based on batch policy
    // =========================================================================
    let mut results = Vec::new();
    let mut all_success = true;
    let mut errors = Vec::new();

    // Execute based on batch policy
    let execution_outcome = match batch_policy {
        BatchPolicy::Atomic => {
            tracing::info!(
                "[EXEC] Using atomic execution with locks (policy=atomic, locks={})",
                expansion_report
                    .as_ref()
                    .map(|r| r.derived_lock_set.len())
                    .unwrap_or(0)
            );
            state
                .dsl_v2_executor
                .execute_plan_atomic_with_locks(&plan, &mut exec_ctx, expansion_report.as_ref())
                .await
                .map(ExecutionOutcome::Atomic)
        }
        BatchPolicy::BestEffort => {
            tracing::info!("[EXEC] Using best-effort execution (policy=best_effort)");
            state
                .dsl_v2_executor
                .execute_plan_best_effort(&plan, &mut exec_ctx)
                .await
                .map(ExecutionOutcome::BestEffort)
        }
    };

    match execution_outcome {
        Ok(outcome) => {
            // Extract results based on outcome type
            let exec_results: Vec<DslV2Result> = match &outcome {
                ExecutionOutcome::Atomic(atomic) => match atomic {
                    AtomicExecutionResult::Committed { step_results, .. } => step_results.clone(),
                    AtomicExecutionResult::RolledBack {
                        failed_at_step,
                        error,
                        ..
                    } => {
                        all_success = false;
                        errors.push(format!(
                            "Atomic execution rolled back at step {}: {}",
                            failed_at_step, error
                        ));
                        Vec::new()
                    }
                    AtomicExecutionResult::LockContention {
                        entity_type,
                        entity_id,
                        ..
                    } => {
                        all_success = false;
                        errors.push(format!(
                            "Lock contention on {}:{} - another session is modifying this entity",
                            entity_type, entity_id
                        ));
                        Vec::new()
                    }
                },
                ExecutionOutcome::BestEffort(best_effort) => {
                    // Check for partial failures
                    if !best_effort.errors.is_empty() {
                        all_success = false;
                        errors.push(best_effort.errors.summary());
                    }
                    // Convert Option<ExecutionResult> to ExecutionResult, filtering None
                    best_effort
                        .verb_results
                        .iter()
                        .filter_map(|r| r.clone())
                        .collect()
                }
            };

            for (idx, exec_result) in exec_results.iter().enumerate() {
                let mut entity_id: Option<Uuid> = None;
                let mut result_data: Option<serde_json::Value> = None;

                match exec_result {
                    DslV2Result::Uuid(uuid) => {
                        entity_id = Some(*uuid);

                        // Only set last_cbu_id if this was a cbu.* verb
                        if let Some(step) = plan.steps.get(idx) {
                            if step.verb_call.domain == "cbu" {
                                context.last_cbu_id = Some(*uuid);
                                context.cbu_ids.push(*uuid);
                                // Also add cbu_id alias so LLM can use @cbu_id
                                context.named_refs.insert("cbu_id".to_string(), *uuid);

                                // AUTO-PROMOTE: Set newly created CBU as active_cbu for session context
                                // This ensures subsequent operations use this CBU without explicit bind
                                let cbu_display_name = step
                                    .verb_call
                                    .arguments
                                    .iter()
                                    .find(|arg| arg.key == "name")
                                    .and_then(|arg| {
                                        if let crate::dsl_v2::ast::AstNode::Literal(
                                            crate::dsl_v2::ast::Literal::String(s),
                                        ) = &arg.value
                                        {
                                            Some(s.clone())
                                        } else {
                                            None
                                        }
                                    })
                                    .unwrap_or_else(|| format!("CBU-{}", &uuid.to_string()[..8]));

                                tracing::info!(
                                    "[EXEC] Auto-promoting new CBU to active_cbu: {} ({})",
                                    cbu_display_name,
                                    uuid
                                );
                                context.set_active_cbu(*uuid, &cbu_display_name);
                            }
                            // Add entity_id alias for entity.* verbs
                            if step.verb_call.domain == "entity" {
                                context.named_refs.insert("entity_id".to_string(), *uuid);
                            }
                        }
                    }
                    DslV2Result::Record(json) => {
                        result_data = Some(json.clone());
                    }
                    DslV2Result::RecordSet(records) => {
                        result_data = Some(serde_json::Value::Array(records.clone()));
                    }
                    DslV2Result::Affected(_) | DslV2Result::Void => {
                        // No special handling needed
                    }
                    DslV2Result::EntityQuery(query_result) => {
                        // Entity query result for batch operations
                        result_data = Some(serde_json::json!({
                            "items": query_result.items.iter().map(|(id, name)| {
                                serde_json::json!({"id": id.to_string(), "name": name})
                            }).collect::<Vec<serde_json::Value>>(),
                            "entity_type": query_result.entity_type,
                            "total_count": query_result.total_count,
                        }));
                    }
                    DslV2Result::TemplateInvoked(invoke_result) => {
                        // Template invocation result
                        entity_id = invoke_result.primary_entity_id;
                        result_data = Some(serde_json::json!({
                            "template_id": invoke_result.template_id,
                            "statements_executed": invoke_result.statements_executed,
                            "outputs": invoke_result.outputs.iter().map(|(k, v)| {
                                (k.clone(), v.to_string())
                            }).collect::<std::collections::HashMap<String, String>>(),
                            "primary_entity_id": invoke_result.primary_entity_id.map(|id| id.to_string()),
                        }));
                    }
                    DslV2Result::TemplateBatch(batch_result) => {
                        // Template batch execution result
                        entity_id = batch_result.primary_entity_ids.first().copied();
                        result_data = Some(serde_json::json!({
                            "template_id": batch_result.template_id,
                            "total_items": batch_result.total_items,
                            "success_count": batch_result.success_count,
                            "failure_count": batch_result.failure_count,
                            "primary_entity_ids": batch_result.primary_entity_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
                            "primary_entity_type": batch_result.primary_entity_type,
                            "aborted": batch_result.aborted,
                        }));
                    }
                    DslV2Result::BatchControl(control_result) => {
                        // Batch control operation result
                        result_data = Some(serde_json::json!({
                            "operation": control_result.operation,
                            "success": control_result.success,
                            "status": control_result.status,
                            "message": control_result.message,
                        }));
                    }
                }

                results.push(ExecutionResult {
                    statement_index: idx,
                    dsl: dsl.clone(),
                    success: true,
                    message: "Executed successfully".to_string(),
                    entity_id,
                    entity_type: None,
                    result: result_data,
                });
            }

            // =========================================================================
            // PERSIST SYMBOLS TO SESSION CONTEXT WITH TYPE INFO
            // =========================================================================
            tracing::debug!("[EXEC] Persisting symbols to session context");
            // Extract binding metadata from plan steps and store typed bindings
            for (idx, exec_result) in exec_results.iter().enumerate() {
                if let DslV2Result::Uuid(uuid) = exec_result {
                    if let Some(step) = plan.steps.get(idx) {
                        if let Some(ref binding_name) = step.bind_as {
                            // Get entity type from domain
                            let entity_type = step.verb_call.domain.clone();

                            // Extract display name from arguments (look for :name param)
                            let display_name = step
                                .verb_call
                                .arguments
                                .iter()
                                .find(|arg| {
                                    arg.key == "name"
                                        || arg.key == "cbu-name"
                                        || arg.key == "first-name"
                                })
                                .and_then(|arg| {
                                    if let crate::dsl_v2::ast::AstNode::Literal(
                                        crate::dsl_v2::ast::Literal::String(s),
                                    ) = &arg.value
                                    {
                                        Some(s.clone())
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or_else(|| binding_name.clone());

                            // Store typed binding
                            tracing::debug!(
                                "[EXEC] set_binding: @{} = {} (type: {}, display: {})",
                                binding_name,
                                uuid,
                                entity_type,
                                display_name
                            );
                            context.set_binding(binding_name, *uuid, &entity_type, &display_name);

                            // Update primary domain keys
                            context.update_primary_key(&entity_type, binding_name, *uuid);
                        }
                    }
                }
            }

            // Also copy raw symbols for backward compatibility
            tracing::debug!(
                "[EXEC] Copying {} exec_ctx.symbols to named_refs: {:?}",
                exec_ctx.symbols.len(),
                exec_ctx.symbols
            );
            for (name, id) in &exec_ctx.symbols {
                context.named_refs.insert(name.clone(), *id);
                tracing::debug!("[EXEC] named_refs.insert(@{} = {})", name, id);
            }
            tracing::debug!(
                "[EXEC] After symbol persist, context.named_refs: {:?}",
                context.named_refs
            );

            // =========================================================================
            // PROPAGATE VIEW STATE FROM EXECUTION CONTEXT
            // =========================================================================
            // View operations (view.universe, view.book, etc.) store ViewState in
            // ExecutionContext.pending_view_state. Propagate it to SessionContext
            // so the UI can access it. This fixes the "session state side door" where
            // ViewState was previously discarded because CustomOperation only receives
            // ExecutionContext, not the full session.
            if let Some(view_state) = exec_ctx.take_pending_view_state() {
                tracing::info!(
                    "[EXEC] Propagating ViewState to session context (node_type: {:?}, label: {})",
                    view_state.taxonomy.node_type,
                    view_state.taxonomy.label
                );
                context.set_view_state(view_state);
            }

            // =========================================================================
            // PROPAGATE VIEWPORT STATE FROM EXECUTION CONTEXT
            // =========================================================================
            // Viewport operations (viewport.focus, viewport.enhance, etc.) store ViewportState
            // in ExecutionContext.pending_viewport_state. Propagate it to SessionContext
            // so the UI can access it for CBU-focused navigation.
            if let Some(viewport_state) = exec_ctx.take_pending_viewport_state() {
                tracing::info!(
                    "[EXEC] Propagating ViewportState to session context (view_type: {:?}, focus_mode: {:?})",
                    viewport_state.view_type,
                    viewport_state.focus.focus_mode
                );
                context.set_viewport_state(viewport_state);
            }

            // =========================================================================
            // PROPAGATE SCOPE CHANGE FROM EXECUTION CONTEXT
            // =========================================================================
            // Session scope operations (session.set-galaxy, session.set-cbu, etc.) store
            // GraphScope in ExecutionContext.pending_scope_change. Propagate it to
            // SessionContext so the UI can rebuild the viewport with new scope.
            if let Some(scope_change) = exec_ctx.take_pending_scope_change() {
                tracing::info!(
                    "[EXEC] Propagating scope change to session context (scope: {:?})",
                    scope_change
                );
                context.set_scope(crate::session::SessionScope::from_graph_scope(scope_change));
            }

            // =========================================================================
            // PROPAGATE UNIFIED SESSION FROM EXECUTION CONTEXT
            // =========================================================================
            // Session load operations (session.load-galaxy, session.load-cbu, etc.) store
            // loaded CBU IDs in ExecutionContext.pending_session. Sync these to
            // SessionContext.cbu_ids so the scope-graph endpoint can build the multi-CBU view.
            if let Some(unified_session) = exec_ctx.take_pending_session() {
                let loaded_cbu_ids = unified_session.cbu_ids_vec();
                let cbu_count = loaded_cbu_ids.len();
                tracing::info!(
                    "[EXEC] Propagating {} CBU IDs from UnifiedSession to context.cbu_ids",
                    cbu_count
                );
                // Merge loaded CBUs into context (avoid duplicates)
                for cbu_id in &loaded_cbu_ids {
                    if !context.cbu_ids.contains(cbu_id) {
                        context.cbu_ids.push(*cbu_id);
                    }
                }

                // Set scope definition so UI knows to trigger scope_graph refetch
                // Use Custom scope for client-scoped loads, Book for single CBU
                let scope_def = if cbu_count == 1 {
                    crate::graph::GraphScope::SingleCbu {
                        cbu_id: loaded_cbu_ids[0],
                        cbu_name: unified_session.name.clone().unwrap_or_default(),
                    }
                } else {
                    // Multi-CBU scope - use Custom with session name or description
                    crate::graph::GraphScope::Custom {
                        description: unified_session
                            .name
                            .clone()
                            .unwrap_or_else(|| format!("{} CBUs", cbu_count)),
                    }
                };

                context.scope = Some(crate::session::SessionScope {
                    definition: scope_def,
                    stats: crate::session::ScopeSummary {
                        total_cbus: cbu_count,
                        ..Default::default()
                    },
                    load_status: crate::session::LoadStatus::Full,
                });
                tracing::info!(
                    "[EXEC] Set context.scope with {} CBUs, scope_type={:?}",
                    cbu_count,
                    context.scope.as_ref().map(|s| &s.definition)
                );
            }

            // =========================================================================
            // CAPTURE DSL DIFF FOR LEARNING (proposed vs final)
            // =========================================================================
            let dsl_diff = state
                .session_manager
                .capture_dsl_diff(session_id, &dsl)
                .await;
            if let Some(ref diff) = dsl_diff {
                if diff.was_edited {
                    tracing::info!(
                        "[EXEC] DSL was edited by user: {} edit(s) detected",
                        diff.edits.len()
                    );
                    for edit in &diff.edits {
                        tracing::debug!(
                            "[EXEC] Edit: {} changed from '{}' to '{}'",
                            edit.field,
                            edit.from,
                            edit.to
                        );
                    }
                }
            }

            // =========================================================================
            // LOG SUCCESS
            // =========================================================================
            if let Some(lid) = log_id {
                let attempt = GenerationAttempt {
                    attempt: 1,
                    timestamp: chrono::Utc::now(),
                    prompt_template: None,
                    prompt_text: String::new(),
                    raw_response: String::new(),
                    extracted_dsl: Some(dsl.clone()),
                    parse_result: ParseResult {
                        success: true,
                        error: None,
                    },
                    lint_result: LintResult {
                        valid: true,
                        errors: vec![],
                        warnings: vec![],
                    },
                    compile_result: CompileResult {
                        success: true,
                        error: None,
                        step_count: plan.len() as i32,
                    },
                    latency_ms: Some(start_time.elapsed().as_millis() as i32),
                    input_tokens: None,
                    output_tokens: None,
                };
                let _ = state.generation_log.add_attempt(lid, &attempt).await;
                let _ = state.generation_log.mark_success(lid, &dsl, None).await;
                // Record execution outcome for learning loop
                let _ = state
                    .generation_log
                    .record_execution_outcome(lid, ExecutionStatus::Executed, None, None)
                    .await;
            }

            // Update intent_feedback outcome with DSL diff
            if let Some(interaction_id) = pending_interaction_id {
                let elapsed_ms = start_time.elapsed().as_millis() as i32;

                // Convert DSL diff to feedback format
                let (generated_dsl, final_dsl_str, user_edits_json) =
                    if let Some(ref diff) = dsl_diff {
                        let edits_json = if diff.edits.is_empty() {
                            None
                        } else {
                            Some(serde_json::to_value(&diff.edits).unwrap_or_default())
                        };
                        (
                            Some(diff.proposed.clone()),
                            Some(diff.final_dsl.clone()),
                            edits_json,
                        )
                    } else {
                        (None, Some(dsl.clone()), None)
                    };

                let _ = state
                    .feedback_service
                    .record_outcome_with_dsl(
                        interaction_id,
                        ob_semantic_matcher::feedback::Outcome::Executed,
                        None, // outcome_verb same as matched
                        None, // no correction
                        Some(elapsed_ms),
                        generated_dsl,
                        final_dsl_str,
                        user_edits_json,
                    )
                    .await;
            }
        }
        Err(e) => {
            all_success = false;
            let error_msg = format!("Execution error: {}", e);
            errors.push(error_msg.clone());

            // Log execution failure
            if let Some(lid) = log_id {
                let attempt = GenerationAttempt {
                    attempt: 1,
                    timestamp: chrono::Utc::now(),
                    prompt_template: None,
                    prompt_text: String::new(),
                    raw_response: String::new(),
                    extracted_dsl: Some(dsl.clone()),
                    parse_result: ParseResult {
                        success: true,
                        error: None,
                    },
                    lint_result: LintResult {
                        valid: true,
                        errors: vec![],
                        warnings: vec![],
                    },
                    compile_result: CompileResult {
                        success: true,
                        error: None,
                        step_count: plan.len() as i32,
                    },
                    latency_ms: Some(start_time.elapsed().as_millis() as i32),
                    input_tokens: None,
                    output_tokens: None,
                };
                let _ = state.generation_log.add_attempt(lid, &attempt).await;
                let _ = state.generation_log.mark_failed(lid).await;
                // Record execution outcome for learning loop
                let _ = state
                    .generation_log
                    .record_execution_outcome(lid, ExecutionStatus::Failed, Some(&error_msg), None)
                    .await;
            }

            // Update intent_feedback outcome (execution failed)
            if let Some(interaction_id) = pending_interaction_id {
                let elapsed_ms = start_time.elapsed().as_millis() as i32;
                let _ = state
                    .feedback_service
                    .record_outcome(
                        interaction_id,
                        ob_semantic_matcher::feedback::Outcome::Executed, // Still "executed" from feedback perspective
                        None,
                        None,
                        Some(elapsed_ms),
                    )
                    .await;
            }

            results.push(ExecutionResult {
                statement_index: 0,
                dsl: dsl.clone(),
                success: false,
                message: error_msg,
                entity_id: None,
                entity_type: None,
                result: None,
            });
        }
    }

    // Collect bindings to return to the client BEFORE moving context
    let bindings_map: std::collections::HashMap<String, uuid::Uuid> = context.named_refs.clone();

    // =========================================================================
    // PERSIST TO DATABASE (on success only)
    // =========================================================================
    if all_success {
        let session_repo = state.session_repo.clone();
        let dsl_repo = state.dsl_repo.clone();
        let dsl_clone = dsl.clone();
        let dsl_for_instance = dsl.clone();
        let bindings_clone = bindings_map.clone();
        let cbu_id = context.last_cbu_id;
        let domains = crate::database::extract_domains(&dsl_clone);
        let primary_domain = crate::database::detect_domain(&dsl_clone);
        let primary_domain_for_instance = primary_domain.clone();
        let execution_ms = start_time.elapsed().as_millis() as i32;

        // Serialize AST to JSON for dsl_instances persistence
        let ast_json = ast_for_persistence
            .as_ref()
            .and_then(|ast| serde_json::to_value(ast).ok());

        // Persist snapshot asynchronously (simple insert, ~1-5ms)
        tokio::spawn(async move {
            // Insert snapshot
            if let Err(e) = session_repo
                .save_snapshot(
                    session_id,
                    &dsl_clone,
                    &bindings_clone,
                    &[],
                    &domains,
                    Some(execution_ms),
                )
                .await
            {
                tracing::error!("Failed to save snapshot for session {}: {}", session_id, e);
            }

            // Update session bindings
            if let Err(e) = session_repo
                .update_bindings(
                    session_id,
                    &bindings_clone,
                    cbu_id,
                    None,
                    primary_domain.as_deref(),
                )
                .await
            {
                tracing::error!(
                    "Failed to update bindings for session {}: {}",
                    session_id,
                    e
                );
            }
        });

        // Persist DSL instance (confirmed DSL/AST pair) asynchronously
        let business_ref = format!("session-{}-{}", session_id, chrono::Utc::now().timestamp());
        let domain_name = primary_domain_for_instance.unwrap_or_else(|| "unknown".to_string());
        tokio::spawn(async move {
            match dsl_repo
                .save_execution(
                    &dsl_for_instance,
                    &domain_name,
                    &business_ref,
                    cbu_id,
                    &ast_json.unwrap_or(serde_json::Value::Null),
                )
                .await
            {
                Ok(result) => {
                    tracing::info!(
                        "Persisted DSL instance: id={}, version={}, ref={}",
                        result.instance_id,
                        result.version,
                        result.business_reference
                    );
                }
                Err(e) => {
                    tracing::error!("Failed to persist DSL instance: {}", e);
                }
            }
        });
    } else {
        // Log error to session asynchronously
        let session_repo = state.session_repo.clone();
        let error_msg = errors.join("; ");
        let dsl_clone = dsl.clone();
        tokio::spawn(async move {
            let _ = session_repo.record_error(session_id, &error_msg).await;
            let _ = session_repo
                .log_event(
                    session_id,
                    crate::database::SessionEventType::ExecuteFailed,
                    Some(&dsl_clone),
                    Some(&error_msg),
                    None,
                )
                .await;
        });
    }

    // Update session
    let new_state = {
        let mut sessions = state.sessions.write().await;
        if let Some(session) = sessions.get_mut(&session_id) {
            // Only persist DSL to AST and file on successful execution
            if all_success {
                // Add executed statements to session AST
                if let Ok(program) = crate::dsl_v2::parse_program(&dsl) {
                    session.context.add_statements(program.statements);
                    tracing::info!(
                        "Added {} statements to session AST after successful execution",
                        session.context.statement_count()
                    );
                }

                // Write DSL to session file (only after successful execution)
                let file_manager = crate::api::dsl_session_file::DslSessionFileManager::new();
                let dsl_clone = dsl.clone();
                let description = format!("Executed: {} statement(s)", plan.len());
                tokio::spawn(async move {
                    if let Err(e) = file_manager
                        .append_dsl(session_id, &dsl_clone, &description)
                        .await
                    {
                        tracing::error!("Failed to write DSL to session file: {}", e);
                    }
                });
            }

            tracing::debug!(
                "[EXEC] Saving context to session, named_refs: {:?}",
                context.named_refs
            );

            // AUTO-UPDATE SESSION ENTITY_ID: If this is a "cbu" session and we just created
            // the first CBU, update the session's primary entity_id to match
            if session.entity_type == "cbu"
                && session.entity_id.is_none()
                && context.active_cbu.is_some()
            {
                let cbu_id = context.active_cbu.as_ref().unwrap().id;
                tracing::info!(
                    "[EXEC] Auto-setting session.entity_id to newly created CBU: {}",
                    cbu_id
                );
                session.set_entity_id(cbu_id);
            }

            session.context = context;
            tracing::debug!(
                "[EXEC] Session context after save, named_refs: {:?}",
                session.context.named_refs
            );

            // Update DAG state for executed verbs (Phase 5: context flows down)
            // This enables prereq-based verb readiness checking
            for step in &plan.steps {
                let verb_fqn = format!("{}.{}", step.verb_call.domain, step.verb_call.verb);
                crate::mcp::update_dag_after_execution(session, &verb_fqn);
            }
            tracing::debug!(
                "[EXEC] Updated DAG state with {} executed verbs, completed={:?}",
                plan.steps.len(),
                session.dag_state.completed
            );

            let session_results: Vec<crate::api::session::ExecutionResult> = results
                .iter()
                .map(|r| crate::api::session::ExecutionResult {
                    statement_index: r.statement_index,
                    dsl: r.dsl.clone(),
                    success: r.success,
                    message: r.message.clone(),
                    entity_id: r.entity_id,
                    entity_type: r.entity_type.clone(),
                    result: r.result.clone(),
                })
                .collect();
            session.record_execution(session_results);

            // Mark current run_sheet entry as executed with affected entities
            if let Some(entry) = session.run_sheet.current_mut() {
                entry.status = crate::session::EntryStatus::Executed;
                entry.executed_at = Some(chrono::Utc::now());
                // Collect all entity IDs affected by this execution
                entry.affected_entities = results.iter().filter_map(|r| r.entity_id).collect();
                // Note: UnifiedSession.RunSheetEntry doesn't have bindings field
                // Bindings are stored in session.bindings and session.context.bindings
            }

            session.state.clone()
        } else {
            SessionState::New
        }
    };

    // Notify watchers that session changed after execution
    state.session_manager.notify(session_id).await;

    Ok(Json(ExecuteResponse {
        success: all_success,
        results,
        errors,
        new_state: new_state.into(),
        bindings: if bindings_map.is_empty() {
            None
        } else {
            Some(bindings_map)
        },
    }))
}

/// POST /api/session/:id/repl-edit - Record REPL edit event
///
/// Called by the UI when the user edits DSL in the REPL editor.
/// This tracks the current_dsl so we can compute diff on execute.
async fn repl_edit_session(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<ReplEditRequest>,
) -> Result<Json<ReplEditResponse>, StatusCode> {
    use crate::dsl_v2::parse_program;

    tracing::debug!(
        "[REPL_EDIT] Session {} - DSL length: {}",
        session_id,
        req.current_dsl.len()
    );

    // Update current_dsl in session via SessionManager
    state
        .session_manager
        .update_current_dsl(session_id, &req.current_dsl)
        .await;

    // Check if there are edits (proposed != current)
    let has_edits = state.session_manager.has_dsl_edits(session_id).await;

    // Validate the current DSL
    let (valid, errors) = match parse_program(&req.current_dsl) {
        Ok(ast) => {
            // Parse succeeded, try compile
            match crate::dsl_v2::compile(&ast) {
                Ok(_) => (true, None),
                Err(e) => (false, Some(vec![format!("Compile error: {:?}", e)])),
            }
        }
        Err(e) => (false, Some(vec![format!("Parse error: {:?}", e)])),
    };

    Ok(Json(ReplEditResponse {
        recorded: true,
        has_edits,
        valid,
        errors,
    }))
}

/// POST /api/session/:id/clear - Clear/cancel pending DSL
async fn clear_session_dsl(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<SessionStateResponse>, StatusCode> {
    let mut sessions = state.sessions.write().await;
    let session = sessions.get_mut(&session_id).ok_or(StatusCode::NOT_FOUND)?;

    // Cancel any pending/draft entries in run_sheet
    for entry in session.run_sheet.entries.iter_mut() {
        if entry.status == crate::session::EntryStatus::Draft {
            entry.status = crate::session::EntryStatus::Cancelled;
        }
    }

    Ok(Json(SessionStateResponse {
        session_id,
        entity_type: session.entity_type.clone(),
        entity_id: session.entity_id,
        state: session.state.clone().into(),
        message_count: session.messages.len(),
        pending_intents: vec![],
        assembled_dsl: vec![],
        combined_dsl: None,
        context: session.context.clone(),
        messages: session.messages.iter().cloned().map(|m| m.into()).collect(),
        can_execute: false,
        version: Some(session.updated_at.to_rfc3339()),
        run_sheet: Some(session.run_sheet.to_api()),
        bindings: session
            .context
            .bindings
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    ob_poc_types::BoundEntityInfo {
                        id: v.id.to_string(),
                        name: v.display_name.clone(),
                        entity_type: v.entity_type.clone(),
                    },
                )
            })
            .collect(),
    }))
}

/// Request to set a binding in a session
#[derive(Debug, Deserialize)]
pub struct SetBindingRequest {
    /// The binding name (without @)
    pub name: String,
    /// The UUID to bind (accepts string for TypeScript compat)
    #[serde(deserialize_with = "deserialize_uuid_string")]
    pub id: Uuid,
    /// Entity type (e.g., "cbu", "entity", "case")
    pub entity_type: String,
    /// Human-readable display name
    pub display_name: String,
}

/// Deserialize UUID from string
fn deserialize_uuid_string<'de, D>(deserializer: D) -> Result<Uuid, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = serde::Deserialize::deserialize(deserializer)?;
    Uuid::parse_str(&s).map_err(serde::de::Error::custom)
}

/// Response from setting a binding
#[derive(Debug, Serialize)]
pub struct SetBindingResponse {
    pub success: bool,
    pub binding_name: String,
    pub bindings: std::collections::HashMap<String, Uuid>,
}

/// Request to set stage focus in a session
#[derive(Debug, Deserialize)]
pub struct SetFocusRequest {
    /// The stage code to focus on (e.g., "KYC_REVIEW")
    /// Pass None or empty string to clear focus
    #[serde(default)]
    pub stage_code: Option<String>,
}

/// Response from setting stage focus
#[derive(Debug, Serialize)]
pub struct SetFocusResponse {
    pub success: bool,
    /// The stage that is now focused (None if cleared)
    pub stage_code: Option<String>,
    /// Stage name for display
    pub stage_name: Option<String>,
    /// Verbs relevant to this stage (for agent filtering)
    pub relevant_verbs: Vec<String>,
}

/// Request to set view mode on session (sync from egui client)
#[derive(Debug, Deserialize)]
pub struct SetViewModeRequest {
    /// The view mode to set (e.g., "KYC_UBO", "SERVICE_DELIVERY", "TRADING")
    pub view_mode: String,
    /// Optional view level (e.g., "Universe", "Cluster", "System")
    #[serde(default)]
    pub view_level: Option<String>,
}

/// Response from setting view mode
#[derive(Debug, Serialize)]
pub struct SetViewModeResponse {
    pub success: bool,
    /// The view mode that was set
    pub view_mode: String,
    /// The view level that was set (if any)
    pub view_level: Option<String>,
}

/// POST /api/session/:id/bind - Set a binding in the session context
///
/// This allows the UI to register external entities (like a selected CBU)
/// as available symbols for DSL generation and execution.
///
/// For CBU bindings, this also loads the current DSL version for optimistic locking.
/// The version is stored in `session.context.loaded_dsl_version` and used when
/// saving DSL to detect concurrent modifications.
async fn set_session_binding(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<SetBindingRequest>,
) -> Result<Json<SetBindingResponse>, StatusCode> {
    // For CBU bindings, load the current DSL version for optimistic locking
    let (loaded_version, business_ref) = if req.entity_type == "cbu" {
        // Use display_name as the business_reference (CBU name is the canonical key)
        let business_ref = req.display_name.clone();
        match state
            .dsl_repo
            .get_instance_by_reference(&business_ref)
            .await
        {
            Ok(Some(instance)) => {
                tracing::debug!(
                    "Loaded DSL version {} for CBU '{}' (optimistic locking)",
                    instance.current_version,
                    business_ref
                );
                (Some(instance.current_version), Some(business_ref))
            }
            Ok(None) => {
                tracing::debug!(
                    "No existing DSL instance for CBU '{}' (will create new)",
                    business_ref
                );
                (None, Some(business_ref))
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to load DSL version for CBU '{}': {} (continuing without lock)",
                    business_ref,
                    e
                );
                (None, Some(business_ref))
            }
        }
    } else {
        (None, None)
    };

    let mut sessions = state.sessions.write().await;
    let session = sessions.get_mut(&session_id).ok_or(StatusCode::NOT_FOUND)?;

    // Set the typed binding (includes display name for LLM context)
    let actual_name =
        session
            .context
            .set_binding(&req.name, req.id, &req.entity_type, &req.display_name);

    // Also set common aliases for CBU
    if req.entity_type == "cbu" {
        session.context.named_refs.insert("cbu".to_string(), req.id);
        session
            .context
            .named_refs
            .insert("cbu_id".to_string(), req.id);
        session.context.set_active_cbu(req.id, &req.display_name);

        // Store version info for optimistic locking
        session.context.loaded_dsl_version = loaded_version;
        session.context.business_reference = business_ref;
    }

    tracing::debug!(
        "Bind success: session={} @{}={} type={} dsl_version={:?}",
        session_id,
        actual_name,
        req.id,
        req.entity_type,
        session.context.loaded_dsl_version
    );

    // Clone before dropping the lock to avoid holding it during response serialization
    // This fixes a deadlock that caused ERR_EMPTY_RESPONSE
    let bindings_clone = session.context.named_refs.clone();
    let actual_name_clone = actual_name.clone();
    drop(sessions);

    // Notify watchers that session changed
    state.session_manager.notify(session_id).await;

    Ok(Json(SetBindingResponse {
        success: true,
        binding_name: actual_name_clone,
        bindings: bindings_clone,
    }))
}

/// POST /api/session/:id/view-mode - Set view mode on session (sync from egui client)
///
/// This allows the UI to sync its current view mode (e.g., KYC_UBO, SERVICE_DELIVERY, TRADING)
/// and view level (e.g., Universe, Cluster, System) to the server session.
/// This is important for ensuring the server knows the visualization context.
async fn set_session_view_mode(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<SetViewModeRequest>,
) -> Result<Json<SetViewModeResponse>, StatusCode> {
    // Update session
    {
        let mut sessions = state.sessions.write().await;
        let session = sessions.get_mut(&session_id).ok_or(StatusCode::NOT_FOUND)?;
        session.context.view_mode = Some(req.view_mode.clone());
        // Note: view_level could be stored in session context if needed in future
    }

    tracing::debug!(
        "View mode set: session={} view_mode={} view_level={:?}",
        session_id,
        req.view_mode,
        req.view_level
    );

    // Notify watchers that session changed
    state.session_manager.notify(session_id).await;

    Ok(Json(SetViewModeResponse {
        success: true,
        view_mode: req.view_mode,
        view_level: req.view_level,
    }))
}

/// POST /api/session/:id/focus - Set stage focus for verb filtering
///
/// When a stage is focused, the agent will prioritize verbs relevant to that stage.
/// Pass an empty or null stage_code to clear the focus.
async fn set_session_focus(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<SetFocusRequest>,
) -> Result<Json<SetFocusResponse>, StatusCode> {
    // Normalize empty string to None
    let stage_code = req.stage_code.filter(|s| !s.is_empty());

    // Update session
    {
        let mut sessions = state.sessions.write().await;
        let session = sessions.get_mut(&session_id).ok_or(StatusCode::NOT_FOUND)?;
        session.context.stage_focus = stage_code.clone();
    }

    // Get stage info and relevant verbs if focused
    let (stage_name, relevant_verbs) = if let Some(ref code) = stage_code {
        // Load registry to get stage info
        // Use .ok() to avoid holding Box<dyn Error> across await
        let registry_opt = SemanticStageRegistry::load_default().ok();
        if let Some(registry) = registry_opt {
            if let Some(stage) = registry.get_stage(code) {
                let name = stage.name.clone();
                // Get relevant verbs from stage config (if defined)
                let verbs = stage.relevant_verbs.clone().unwrap_or_default();
                (Some(name), verbs)
            } else {
                (None, vec![])
            }
        } else {
            (None, vec![])
        }
    } else {
        (None, vec![])
    };

    tracing::debug!(
        "Focus set: session={} stage={:?} verbs={}",
        session_id,
        stage_code,
        relevant_verbs.len()
    );

    // Notify watchers that session changed
    state.session_manager.notify(session_id).await;

    Ok(Json(SetFocusResponse {
        success: true,
        stage_code,
        stage_name,
        relevant_verbs,
    }))
}

/// GET /api/session/:id/context - Get session context for agent and UI
///
/// Returns the session's context including:
/// - Active CBU with entity/role counts
/// - Linked KYC cases, ISDA agreements, products
/// - Trading profile if available
/// - Symbol table from DSL execution
///
/// This enables both the agent (for prompt context) and UI (for context panel)
/// to understand "where we are" in the workflow.
async fn get_session_context(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<ob_poc_types::GetContextResponse>, StatusCode> {
    // Get session to find active CBU
    let active_cbu_id = {
        let sessions = state.sessions.read().await;
        let session = sessions.get(&session_id).ok_or(StatusCode::NOT_FOUND)?;
        session.context.active_cbu.as_ref().map(|cbu| cbu.id)
    };

    // If we have an active CBU, discover linked contexts from DB
    let context = if let Some(cbu_id) = active_cbu_id {
        let discovery_service = crate::database::ContextDiscoveryService::new(state.pool.clone());

        match discovery_service.discover_for_cbu(cbu_id).await {
            Ok(discovered) => {
                let mut ctx: ob_poc_types::SessionContext = discovered.into();

                // Merge in symbols and stage_focus from session
                let sessions = state.sessions.read().await;
                if let Some(session) = sessions.get(&session_id) {
                    // Convert session bindings to SymbolValue
                    for (name, binding) in &session.context.bindings {
                        ctx.symbols.insert(
                            name.clone(),
                            ob_poc_types::SymbolValue {
                                id: binding.id.to_string(),
                                entity_type: binding.entity_type.clone(),
                                display_name: binding.display_name.clone(),
                                source: Some("execution".to_string()),
                            },
                        );
                    }

                    // Set active scope if we have an active CBU
                    if let Some(cbu) = &ctx.cbu {
                        ctx.active_scope = Some(ob_poc_types::ActiveScope::Cbu {
                            cbu_id: cbu.id.clone(),
                            cbu_name: cbu.name.clone(),
                        });
                    }

                    // Copy stage focus from session
                    ctx.stage_focus = session.context.stage_focus.clone();

                    // Copy viewport state from session (set by viewport.* DSL verbs)
                    ctx.viewport_state = session.context.viewport_state.clone();
                }

                // Derive semantic state for the CBU (onboarding journey progress)
                // Note: We load the registry and immediately extract it to avoid
                // holding Box<dyn Error> across await points (not Send)
                let registry_opt = SemanticStageRegistry::load_default().ok();
                if let Some(registry) = registry_opt {
                    match derive_semantic_state(&state.pool, &registry, cbu_id).await {
                        Ok(semantic_state) => {
                            ctx.semantic_state = Some(semantic_state);
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to derive semantic state for CBU {}: {}",
                                cbu_id,
                                e
                            );
                        }
                    }
                }

                ctx
            }
            Err(e) => {
                tracing::warn!("Context discovery failed for CBU {}: {}", cbu_id, e);
                ob_poc_types::SessionContext::default()
            }
        }
    } else {
        // No active CBU, return empty context with just symbols
        let sessions = state.sessions.read().await;
        let mut ctx = ob_poc_types::SessionContext::default();

        if let Some(session) = sessions.get(&session_id) {
            for (name, binding) in &session.context.bindings {
                ctx.symbols.insert(
                    name.clone(),
                    ob_poc_types::SymbolValue {
                        id: binding.id.to_string(),
                        entity_type: binding.entity_type.clone(),
                        display_name: binding.display_name.clone(),
                        source: Some("execution".to_string()),
                    },
                );
            }

            // Copy viewport state from session (set by viewport.* DSL verbs)
            ctx.viewport_state = session.context.viewport_state.clone();
        }

        ctx
    };

    Ok(Json(ob_poc_types::GetContextResponse { context }))
}

/// GET /api/session/:id/dsl/enrich - Get enriched DSL with binding info for display
///
/// Returns the session's DSL source with inline binding information:
/// - @symbols resolved to display names
/// - EntityRefs marked as resolved or needing resolution
/// - Binding summary for context panel
///
/// This is optimized for fast round-trips - no database or EntityGateway calls.
async fn get_enriched_dsl(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<ob_poc_types::EnrichedDsl>, StatusCode> {
    let sessions = state.sessions.read().await;
    let session = sessions.get(&session_id).ok_or(StatusCode::NOT_FOUND)?;

    // Get DSL source from session run sheet
    let dsl_source = session
        .run_sheet
        .combined_dsl()
        .unwrap_or_else(|| session.context.to_dsl_source());

    if dsl_source.trim().is_empty() {
        // Return empty enriched DSL
        return Ok(Json(ob_poc_types::EnrichedDsl {
            source: String::new(),
            segments: vec![],
            binding_summary: vec![],
            fully_resolved: true,
        }));
    }

    // Convert session bindings to enrichment format
    let bindings = crate::services::bindings_from_session_context(&session.context.bindings);

    // Get active CBU for summary
    let active_cbu = session
        .context
        .bindings
        .get("cbu")
        .map(|b| crate::services::BindingInfo {
            id: b.id,
            display_name: b.display_name.clone(),
            entity_type: b.entity_type.clone(),
        });

    // Enrich DSL
    match crate::services::enrich_dsl(&dsl_source, &bindings, active_cbu.as_ref()) {
        Ok(enriched) => Ok(Json(enriched)),
        Err(e) => {
            tracing::warn!("DSL enrichment failed: {}", e);
            // Return raw DSL on parse failure
            Ok(Json(ob_poc_types::EnrichedDsl {
                source: dsl_source,
                segments: vec![ob_poc_types::DslDisplaySegment::Text {
                    content: format!("Parse error: {}", e),
                }],
                binding_summary: vec![],
                fully_resolved: false,
            }))
        }
    }
}

// ============================================================================
// Auto-Resolution Helper
// ============================================================================

/// Auto-resolve EntityRefs with exact matches (100% confidence) via EntityGateway.
///
/// For each unresolved EntityRef, searches the gateway. If an exact match is found
/// (value matches token exactly, case-insensitive for lookup tables), the resolved_key
/// is set automatically without requiring user interaction.
async fn auto_resolve_entity_refs(
    program: crate::dsl_v2::ast::Program,
) -> crate::dsl_v2::ast::Program {
    use crate::dsl_v2::ast::{
        find_unresolved_ref_locations, Argument, AstNode, Program, Statement, VerbCall,
    };
    use entity_gateway::proto::ob::gateway::v1::{SearchMode, SearchRequest};

    // Find all unresolved refs
    let unresolved = find_unresolved_ref_locations(&program);
    if unresolved.is_empty() {
        return program;
    }

    // Connect to EntityGateway
    let addr = crate::dsl_v2::gateway_resolver::gateway_addr();
    let mut client = match entity_gateway::proto::ob::gateway::v1::entity_gateway_client::EntityGatewayClient::connect(addr.clone()).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Auto-resolve: Failed to connect to EntityGateway: {}", e);
            return program;
        }
    };

    // Build resolution map: (stmt_idx, arg_key) -> resolved_key
    let mut resolutions: std::collections::HashMap<(usize, String), String> =
        std::collections::HashMap::new();

    for loc in &unresolved {
        // Map entity_type to gateway nickname (uppercase)
        let nickname = loc.entity_type.to_uppercase();

        let search_request = SearchRequest {
            nickname: nickname.clone(),
            values: vec![loc.search_text.clone()],
            search_key: None,
            mode: SearchMode::Exact as i32, // Exact mode for precise matching
            limit: Some(1),
            discriminators: std::collections::HashMap::new(),
            tenant_id: None,
            cbu_id: None,
        };

        if let Ok(response) = client.search(search_request).await {
            let inner = response.into_inner();
            if let Some(m) = inner.matches.first() {
                // Check for exact match: search_text matches token (case-insensitive for lookup tables)
                let is_exact = m.token.eq_ignore_ascii_case(&loc.search_text)
                    || m.display
                        .to_uppercase()
                        .contains(&loc.search_text.to_uppercase());

                if is_exact {
                    tracing::debug!(
                        "Auto-resolved: {} '{}' -> '{}'",
                        loc.entity_type,
                        loc.search_text,
                        m.token
                    );
                    resolutions.insert((loc.statement_index, loc.arg_key.clone()), m.token.clone());
                }
            }
        }
    }

    if resolutions.is_empty() {
        return program;
    }

    // Apply resolutions to AST
    let resolved_statements: Vec<Statement> = program
        .statements
        .into_iter()
        .enumerate()
        .map(|(stmt_idx, stmt)| {
            if let Statement::VerbCall(vc) = stmt {
                let resolved_args: Vec<Argument> = vc
                    .arguments
                    .into_iter()
                    .map(|arg| {
                        if let Some(resolved_key) = resolutions.get(&(stmt_idx, arg.key.clone())) {
                            // Update the EntityRef with resolved_key
                            if let AstNode::EntityRef {
                                entity_type,
                                search_column,
                                value,
                                span,
                                ref_id,
                                ..
                            } = arg.value
                            {
                                return Argument {
                                    key: arg.key,
                                    value: AstNode::EntityRef {
                                        entity_type,
                                        search_column,
                                        value,
                                        resolved_key: Some(resolved_key.clone()),
                                        span,
                                        ref_id,
                                        explain: None, // Resolution explain not captured in batch commit
                                    },
                                    span: arg.span,
                                };
                            }
                        }
                        arg
                    })
                    .collect();

                Statement::VerbCall(VerbCall {
                    domain: vc.domain,
                    verb: vc.verb,
                    arguments: resolved_args,
                    binding: vc.binding,
                    span: vc.span,
                })
            } else {
                stmt
            }
        })
        .collect();

    Program {
        statements: resolved_statements,
    }
}

// ============================================================================
// DSL Parse Handler
// ============================================================================

/// Request to parse DSL source into AST
#[derive(Debug, Deserialize)]
pub struct ParseDslRequest {
    /// DSL source text to parse
    pub dsl: String,
    /// Optional session ID to store the parsed AST
    pub session_id: Option<Uuid>,
}

/// A missing required argument
#[derive(Debug, Clone, Serialize)]
pub struct MissingArg {
    /// Statement index (0-based)
    pub statement_index: usize,
    /// Argument name (e.g., "name", "jurisdiction")
    pub arg_name: String,
    /// Verb that requires this arg
    pub verb: String,
}

#[derive(Debug, Serialize)]
pub struct ParseDslResponse {
    /// Whether parsing succeeded
    pub success: bool,
    /// Pipeline stage reached
    pub stage: PipelineStage,
    /// DSL source (echoed back)
    pub dsl_source: String,
    /// Parsed AST (if parse succeeded)
    pub ast: Option<Vec<crate::dsl_v2::ast::Statement>>,
    /// Unresolved EntityRefs requiring resolution
    pub unresolved_refs: Vec<UnresolvedRef>,
    /// Missing required arguments (from CSG validation)
    pub missing_args: Vec<MissingArg>,
    /// Validation errors (non-missing-arg errors)
    pub validation_errors: Vec<String>,
    /// Parse error (if failed)
    pub error: Option<String>,
}

/// POST /api/dsl/parse - Parse DSL source into AST
///
/// Parses DSL source text and returns the AST with any unresolved EntityRefs.
/// DSL + AST are persisted as a tuple pair. If there are unresolved refs,
/// the UI should prompt the user to resolve them before execution.
///
/// ## Request
/// ```json
/// {
///   "dsl": "(cbu.assign-role :cbu-id \"Apex\" :entity-id \"John Smith\" :role \"DIRECTOR\")",
///   "session_id": "optional-uuid"
/// }
/// ```
///
/// ## Response
/// Returns AST with unresolved_refs array for UI to show resolution popups.
async fn parse_dsl(
    State(state): State<AgentState>,
    Json(req): Json<ParseDslRequest>,
) -> Json<ParseDslResponse> {
    use crate::dsl_v2::ast::find_unresolved_ref_locations;
    use crate::dsl_v2::validation::{Severity, ValidationContext, ValidationRequest};
    use crate::dsl_v2::{enrich_program, runtime_registry};

    // Parse the DSL (raw AST with literals only)
    let raw_program = match parse_program(&req.dsl) {
        Ok(p) => p,
        Err(e) => {
            return Json(ParseDslResponse {
                success: false,
                stage: PipelineStage::Draft,
                dsl_source: req.dsl,
                ast: None,
                unresolved_refs: vec![],
                missing_args: vec![],
                validation_errors: vec![],
                error: Some(format!("Parse error: {}", e)),
            });
        }
    };

    // Enrich: convert string literals to EntityRefs based on YAML verb config
    let registry = runtime_registry();
    let enrichment_result = enrich_program(raw_program, registry);
    let mut program = enrichment_result.program;

    // Auto-resolve EntityRefs with exact matches via EntityGateway
    // This handles cases like :product "CUSTODY" where the value matches exactly
    program = auto_resolve_entity_refs(program).await;

    // Run semantic validation with CSG linter to find missing required args
    let mut missing_args: Vec<MissingArg> = vec![];
    let mut validation_errors: Vec<String> = vec![];

    // Try to initialize validator and run CSG validation
    let validator_result = async {
        let v = SemanticValidator::new(state.pool.clone()).await?;
        v.with_csg_linter().await
    }
    .await;

    if let Ok(mut validator) = validator_result {
        let request = ValidationRequest {
            source: req.dsl.clone(),
            context: ValidationContext::default(),
        };
        if let crate::dsl_v2::validation::ValidationResult::Err(diagnostics) =
            validator.validate(&request).await
        {
            for (idx, diag) in diagnostics.iter().enumerate() {
                if diag.severity != Severity::Error {
                    continue;
                }
                let msg = format!("[{}] {}", diag.code.as_str(), diag.message);
                // Pattern: "[E020] missing required argument 'name' for verb 'cbu.create'"
                if diag.code.as_str() == "E020" {
                    // Parse: "missing required argument 'name' for verb 'cbu.create'"
                    // Find first quoted string (arg name) and last quoted string (verb)
                    let parts: Vec<&str> = diag.message.split('\'').collect();
                    if parts.len() >= 4 {
                        // parts: ["missing required argument ", "name", " for verb ", "cbu.create", ""]
                        let arg_name = parts[1].to_string();
                        let verb = parts[3].to_string();
                        missing_args.push(MissingArg {
                            statement_index: idx,
                            arg_name,
                            verb,
                        });
                        continue;
                    }
                }
                // Other validation errors
                validation_errors.push(msg);
            }
        }
    }

    // Find unresolved EntityRefs (those with resolved_key: None)
    let unresolved_locations = find_unresolved_ref_locations(&program);
    let unresolved_refs: Vec<UnresolvedRef> = unresolved_locations
        .into_iter()
        .map(|loc| UnresolvedRef {
            statement_index: loc.statement_index,
            arg_key: loc.arg_key,
            entity_type: loc.entity_type,
            search_text: loc.search_text,
        })
        .collect();

    // Determine stage based on what's needed
    let stage = if !missing_args.is_empty() {
        PipelineStage::Draft // Still need required args
    } else if !unresolved_refs.is_empty() {
        PipelineStage::Resolving // Have all args, need to resolve refs
    } else if !validation_errors.is_empty() {
        PipelineStage::Draft // Other validation errors
    } else {
        PipelineStage::Resolved // Ready to execute
    };

    // If session_id provided, store AST in in-memory session only.
    // Database persistence happens at execution time when we have a CBU context.
    if let Some(session_id) = req.session_id {
        let mut sessions = state.sessions.write().await;
        if let Some(session) = sessions.get_mut(&session_id) {
            session.context.ast = program.statements.clone();
            if stage == PipelineStage::Resolved {
                session.state = SessionState::ReadyToExecute;
            } else {
                session.state = SessionState::PendingValidation;
            }
        }
    }

    Json(ParseDslResponse {
        success: true,
        stage,
        dsl_source: req.dsl,
        ast: Some(program.statements),
        unresolved_refs,
        missing_args,
        validation_errors,
        error: None,
    })
}

// ============================================================================
// Entity Reference Resolution Handler
// ============================================================================

/// POST /api/dsl/resolve-ref - Update AST triplet with resolved primary key
///
/// Updates an EntityRef's resolved_key in the session AST without changing
/// the DSL source text. The AST is the source of truth; source is rendered from it.
///
/// ## Request
///
/// ```json
/// {
///   "session_id": "uuid",
///   "ref_id": { "statement_index": 2, "arg_key": "entity-id" },
///   "resolved_key": "550e8400-..."
/// }
/// ```
///
/// ## Response
///
/// Returns the refreshed AST with resolution stats and can_execute flag.
async fn resolve_entity_ref(
    State(state): State<AgentState>,
    Json(req): Json<ResolveRefRequest>,
) -> Result<Json<ResolveRefResponse>, StatusCode> {
    use crate::dsl_v2::ast::{count_entity_refs, AstNode, Statement};

    let mut sessions = state.sessions.write().await;
    let session = sessions
        .get_mut(&req.session_id)
        .ok_or(StatusCode::NOT_FOUND)?;

    // Validate statement index
    if req.ref_id.statement_index >= session.context.ast.len() {
        return Ok(Json(ResolveRefResponse {
            success: false,
            dsl_source: None,
            ast: None,
            resolution_stats: ResolutionStats {
                total_refs: 0,
                unresolved_count: 0,
            },
            can_execute: false,
            error: Some(format!(
                "Statement index {} out of range (AST has {} statements)",
                req.ref_id.statement_index,
                session.context.ast.len()
            )),
            code: Some("INVALID_REF_ID".to_string()),
        }));
    }

    // Get the statement and find the argument
    let stmt = &mut session.context.ast[req.ref_id.statement_index];

    let update_result = match stmt {
        Statement::VerbCall(vc) => {
            // Find the argument by key
            let arg = vc
                .arguments
                .iter_mut()
                .find(|a| a.key == req.ref_id.arg_key);

            match arg {
                Some(arg) => {
                    // Check if it's an EntityRef
                    match &arg.value {
                        AstNode::EntityRef {
                            entity_type,
                            search_column,
                            value,
                            resolved_key,
                            span,
                            ref_id,
                            explain,
                        } => {
                            if resolved_key.is_some() {
                                Err((
                                    "EntityRef already has a resolved_key".to_string(),
                                    "ALREADY_RESOLVED".to_string(),
                                ))
                            } else {
                                // Update with resolved key
                                arg.value = AstNode::EntityRef {
                                    entity_type: entity_type.clone(),
                                    search_column: search_column.clone(),
                                    value: value.clone(),
                                    resolved_key: Some(req.resolved_key.clone()),
                                    span: *span,
                                    ref_id: ref_id.clone(),
                                    explain: explain.clone(),
                                };
                                Ok(())
                            }
                        }
                        _ => Err((
                            format!("Argument '{}' is not an EntityRef", req.ref_id.arg_key),
                            "NOT_ENTITY_REF".to_string(),
                        )),
                    }
                }
                None => Err((
                    format!("Argument '{}' not found in statement", req.ref_id.arg_key),
                    "INVALID_REF_ID".to_string(),
                )),
            }
        }
        Statement::Comment(_) => Err((
            "Cannot resolve ref in a comment statement".to_string(),
            "INVALID_REF_ID".to_string(),
        )),
    };

    // Handle update result
    match update_result {
        Ok(()) => {
            // Calculate resolution stats
            let program = session.context.as_program();
            let stats = count_entity_refs(&program);

            let can_execute = stats.is_fully_resolved();

            // Update session state if ready to execute
            if can_execute && !session.context.ast.is_empty() {
                session.state = SessionState::ReadyToExecute;
            }

            // Re-render DSL from updated AST (DSL + AST are a tuple pair)
            let dsl_source = session.context.to_dsl_source();

            Ok(Json(ResolveRefResponse {
                success: true,
                dsl_source: Some(dsl_source),
                ast: Some(session.context.ast.clone()),
                resolution_stats: ResolutionStats {
                    total_refs: stats.total_refs,
                    unresolved_count: stats.unresolved_count,
                },
                can_execute,
                error: None,
                code: None,
            }))
        }
        Err((message, code)) => Ok(Json(ResolveRefResponse {
            success: false,
            dsl_source: None,
            ast: None,
            resolution_stats: ResolutionStats {
                total_refs: 0,
                unresolved_count: 0,
            },
            can_execute: false,
            error: Some(message),
            code: Some(code),
        })),
    }
}

// ============================================================================
// Span-Based Resolution Handler (Issue K)
// ============================================================================

/// POST /api/dsl/resolve-by-ref-id
///
/// Resolve an EntityRef by span-based ref_id with dsl_hash verification.
/// This enables precise targeting of refs in lists and maps.
///
/// ## Request
/// ```json
/// {
///   "session_id": "...",
///   "ref_id": "0:15-30",
///   "resolved_key": "550e8400-e29b-41d4-a716-446655440000",
///   "dsl_hash": "a1b2c3d4e5f67890"
/// }
/// ```
async fn resolve_by_ref_id(
    State(state): State<AgentState>,
    Json(req): Json<ResolveByRefIdRequest>,
) -> Result<Json<ResolveByRefIdResponse>, StatusCode> {
    use crate::dsl_v2::ast::{find_unresolved_ref_locations, Statement};

    let mut sessions = state.sessions.write().await;
    let session = sessions
        .get_mut(&req.session_id)
        .ok_or(StatusCode::NOT_FOUND)?;

    // Step 1: Verify dsl_hash matches current session DSL
    let current_dsl = session.context.to_dsl_source();
    let current_hash = compute_dsl_hash_internal(&current_dsl);

    if current_hash != req.dsl_hash {
        return Ok(Json(ResolveByRefIdResponse {
            success: false,
            dsl: current_dsl,
            dsl_hash: current_hash,
            remaining_unresolved: vec![],
            fully_resolved: false,
            error: Some(
                "DSL has changed since disambiguation was generated. Please refresh.".to_string(),
            ),
        }));
    }

    // Step 2: Parse ref_id format "stmt_idx:start-end"
    let parts: Vec<&str> = req.ref_id.split(':').collect();
    if parts.len() != 2 {
        return Ok(Json(ResolveByRefIdResponse {
            success: false,
            dsl: current_dsl,
            dsl_hash: current_hash,
            remaining_unresolved: vec![],
            fully_resolved: false,
            error: Some(format!(
                "Invalid ref_id format: '{}'. Expected 'stmt_idx:start-end'",
                req.ref_id
            )),
        }));
    }

    let stmt_idx: usize = parts[0].parse().map_err(|_| StatusCode::BAD_REQUEST)?;
    let span_parts: Vec<&str> = parts[1].split('-').collect();
    if span_parts.len() != 2 {
        return Ok(Json(ResolveByRefIdResponse {
            success: false,
            dsl: current_dsl,
            dsl_hash: current_hash,
            remaining_unresolved: vec![],
            fully_resolved: false,
            error: Some(format!("Invalid span format in ref_id: '{}'", req.ref_id)),
        }));
    }

    let span_start: usize = span_parts[0].parse().map_err(|_| StatusCode::BAD_REQUEST)?;
    let span_end: usize = span_parts[1].parse().map_err(|_| StatusCode::BAD_REQUEST)?;

    // Step 3: Find and update the EntityRef by span
    if stmt_idx >= session.context.ast.len() {
        return Ok(Json(ResolveByRefIdResponse {
            success: false,
            dsl: current_dsl,
            dsl_hash: current_hash,
            remaining_unresolved: vec![],
            fully_resolved: false,
            error: Some(format!("Statement index {} out of range", stmt_idx)),
        }));
    }

    let stmt = &mut session.context.ast[stmt_idx];
    let update_result = match stmt {
        Statement::VerbCall(vc) => {
            update_entity_ref_by_span(&mut vc.arguments, span_start, span_end, &req.resolved_key)
        }
        Statement::Comment(_) => Err("Cannot resolve ref in a comment statement".to_string()),
    };

    match update_result {
        Ok(()) => {
            // Step 4: Re-render DSL and compute new hash
            let updated_dsl = session.context.to_dsl_source();
            let new_hash = compute_dsl_hash_internal(&updated_dsl);

            // Step 5: Get remaining unresolved refs
            let program = session.context.as_program();
            let locations = find_unresolved_ref_locations(&program);

            let remaining: Vec<RemainingUnresolvedRef> = locations
                .into_iter()
                .map(|loc| RemainingUnresolvedRef {
                    param_name: loc.arg_key,
                    search_value: loc.search_text,
                    entity_type: loc.entity_type,
                    search_column: loc.search_column,
                    ref_id: loc.ref_id.unwrap_or_default(),
                })
                .collect();

            let fully_resolved = remaining.is_empty();

            // Update session state and run_sheet if ready to execute
            if fully_resolved && !session.context.ast.is_empty() {
                session.state = SessionState::ReadyToExecute;
                // Add resolved DSL to run_sheet so /execute can find it
                session.set_pending_dsl(
                    updated_dsl.clone(),
                    session.context.ast.clone(),
                    None,
                    false,
                );
            }

            Ok(Json(ResolveByRefIdResponse {
                success: true,
                dsl: updated_dsl,
                dsl_hash: new_hash,
                remaining_unresolved: remaining,
                fully_resolved,
                error: None,
            }))
        }
        Err(message) => Ok(Json(ResolveByRefIdResponse {
            success: false,
            dsl: current_dsl,
            dsl_hash: current_hash,
            remaining_unresolved: vec![],
            fully_resolved: false,
            error: Some(message),
        })),
    }
}

/// Recursively update an EntityRef by matching span coordinates
fn update_entity_ref_by_span(
    args: &mut [crate::dsl_v2::ast::Argument],
    span_start: usize,
    span_end: usize,
    resolved_key: &str,
) -> Result<(), String> {
    for arg in args.iter_mut() {
        if update_node_by_span(&mut arg.value, span_start, span_end, resolved_key)? {
            return Ok(());
        }
    }
    Err(format!(
        "No EntityRef found with span {}-{}",
        span_start, span_end
    ))
}

/// Recursively search and update a node by span (handles lists/maps)
fn update_node_by_span(
    node: &mut crate::dsl_v2::ast::AstNode,
    span_start: usize,
    span_end: usize,
    resolved_key: &str,
) -> Result<bool, String> {
    use crate::dsl_v2::ast::AstNode;

    match node {
        AstNode::EntityRef {
            span,
            resolved_key: ref mut existing_key,
            ..
        } => {
            if span.start == span_start && span.end == span_end {
                if existing_key.is_some() {
                    return Err("EntityRef already resolved".to_string());
                }
                *existing_key = Some(resolved_key.to_string());
                return Ok(true);
            }
            Ok(false)
        }
        AstNode::List { items, .. } => {
            for item in items.iter_mut() {
                if update_node_by_span(item, span_start, span_end, resolved_key)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        AstNode::Map { entries, .. } => {
            for (_, value) in entries.iter_mut() {
                if update_node_by_span(value, span_start, span_end, resolved_key)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        _ => Ok(false),
    }
}

/// Compute SHA-256 hash of DSL string (first 16 hex chars)
fn compute_dsl_hash_internal(dsl: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(dsl.as_bytes());
    let result = hasher.finalize();
    format!("{:x}", result)[..16].to_string()
}

// ============================================================================
// Discriminator Parsing Handler
// ============================================================================

/// Request to parse natural language into discriminators
#[derive(Debug, Deserialize)]
pub struct ParseDiscriminatorsRequest {
    /// The natural language input to parse
    pub input: String,
    /// Optional entity type context
    pub entity_type: Option<String>,
}

/// Parsed discriminators for entity resolution
#[derive(Debug, Serialize, Default)]
pub struct ParsedDiscriminators {
    /// Nationality code (e.g., "GB", "US")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nationality: Option<String>,
    /// Year of birth
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dob_year: Option<i32>,
    /// Full date of birth
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dob: Option<String>,
    /// Role (e.g., "DIRECTOR", "UBO")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Associated entity name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub associated_entity: Option<String>,
    /// Jurisdiction code
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jurisdiction: Option<String>,
    /// Selection index (e.g., "first", "second", "1", "2")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection_index: Option<usize>,
}

/// Response from discriminator parsing
#[derive(Debug, Serialize)]
pub struct ParseDiscriminatorsResponse {
    pub success: bool,
    pub discriminators: ParsedDiscriminators,
    /// Whether input appears to be a selection (number or ordinal)
    pub is_selection: bool,
    /// The original input
    pub input: String,
    /// Unrecognized parts of the input
    pub unrecognized: Vec<String>,
}

/// POST /api/resolution/parse-discriminators - Parse natural language into discriminators
///
/// Takes user input like "the British one" or "born 1965" and extracts
/// structured discriminators for entity resolution filtering.
///
/// ## Examples
///
/// - "the British one" → nationality: "GB"
/// - "born 1965" → dob_year: 1965
/// - "the director" → role: "DIRECTOR"
/// - "first" or "1" → selection_index: 0
/// - "UK citizen, director at Acme" → nationality: "GB", role: "DIRECTOR", associated_entity: "Acme"
async fn parse_discriminators(
    Json(req): Json<ParseDiscriminatorsRequest>,
) -> Json<ParseDiscriminatorsResponse> {
    let input = req.input.trim().to_lowercase();
    let mut discriminators = ParsedDiscriminators::default();
    let mut unrecognized = Vec::new();
    let mut is_selection = false;

    // Check for selection patterns first
    if let Some(idx) = parse_selection_pattern(&input) {
        discriminators.selection_index = Some(idx);
        is_selection = true;
    }

    // Parse nationality patterns
    if let Some(nat) = parse_nationality(&input) {
        discriminators.nationality = Some(nat);
    }

    // Parse date of birth patterns
    if let Some(year) = parse_dob_year(&input) {
        discriminators.dob_year = Some(year);
    }

    // Parse role patterns
    if let Some(role) = parse_role(&input) {
        discriminators.role = Some(role);
    }

    // Parse association patterns ("at X", "works for X", "from X company")
    if let Some(entity) = parse_association(&input) {
        discriminators.associated_entity = Some(entity);
    }

    // Parse jurisdiction patterns
    if let Some(juris) = parse_jurisdiction(&input) {
        discriminators.jurisdiction = Some(juris);
    }

    // Track what wasn't recognized
    // (simplified - in production would do proper tokenization)
    if discriminators.nationality.is_none()
        && discriminators.dob_year.is_none()
        && discriminators.role.is_none()
        && discriminators.associated_entity.is_none()
        && discriminators.jurisdiction.is_none()
        && discriminators.selection_index.is_none()
    {
        unrecognized.push(req.input.clone());
    }

    Json(ParseDiscriminatorsResponse {
        success: true,
        discriminators,
        is_selection,
        input: req.input,
        unrecognized,
    })
}

/// Parse selection patterns: "1", "first", "the first one", "select 2"
fn parse_selection_pattern(input: &str) -> Option<usize> {
    // Direct number
    if let Ok(n) = input.parse::<usize>() {
        if n >= 1 {
            return Some(n - 1);
        }
    }

    // "select N"
    if let Some(rest) = input.strip_prefix("select ") {
        if let Ok(n) = rest.trim().parse::<usize>() {
            if n >= 1 {
                return Some(n - 1);
            }
        }
    }

    // Ordinals
    let ordinals = [
        ("first", 0),
        ("1st", 0),
        ("second", 1),
        ("2nd", 1),
        ("third", 2),
        ("3rd", 2),
        ("fourth", 3),
        ("4th", 3),
        ("fifth", 4),
        ("5th", 4),
    ];

    for (word, idx) in ordinals {
        if input.contains(word) {
            return Some(idx);
        }
    }

    None
}

/// Parse nationality from natural language
fn parse_nationality(input: &str) -> Option<String> {
    // Map of patterns to ISO codes
    let patterns = [
        // Demonyms
        ("british", "GB"),
        ("uk citizen", "GB"),
        ("english", "GB"),
        ("scottish", "GB"),
        ("welsh", "GB"),
        ("american", "US"),
        ("us citizen", "US"),
        ("german", "DE"),
        ("french", "FR"),
        ("italian", "IT"),
        ("spanish", "ES"),
        ("dutch", "NL"),
        ("belgian", "BE"),
        ("swiss", "CH"),
        ("austrian", "AT"),
        ("irish", "IE"),
        ("luxembourgish", "LU"),
        ("luxembourg", "LU"),
        ("canadian", "CA"),
        ("australian", "AU"),
        ("japanese", "JP"),
        ("chinese", "CN"),
        ("indian", "IN"),
        ("brazilian", "BR"),
        ("mexican", "MX"),
        ("swedish", "SE"),
        ("norwegian", "NO"),
        ("danish", "DK"),
        ("finnish", "FI"),
        ("polish", "PL"),
        ("portuguese", "PT"),
        ("greek", "GR"),
        ("russian", "RU"),
        // Direct codes
        ("from uk", "GB"),
        ("from us", "US"),
        ("from usa", "US"),
        ("from gb", "GB"),
    ];

    for (pattern, code) in patterns {
        if input.contains(pattern) {
            return Some(code.to_string());
        }
    }

    None
}

/// Parse year of birth
fn parse_dob_year(input: &str) -> Option<i32> {
    // Pattern: "born YYYY" or "dob YYYY" or "birth year YYYY"
    let year_patterns = ["born ", "dob ", "birth year ", "year of birth ", "born in "];

    for prefix in year_patterns {
        if let Some(rest) = input.find(prefix).map(|i| &input[i + prefix.len()..]) {
            // Extract first 4 digits
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if digits.len() == 4 {
                if let Ok(year) = digits.parse::<i32>() {
                    if (1900..=2010).contains(&year) {
                        return Some(year);
                    }
                }
            }
        }
    }

    None
}

/// Parse role from natural language
fn parse_role(input: &str) -> Option<String> {
    let role_patterns = [
        ("director", "DIRECTOR"),
        ("ubo", "UBO"),
        ("beneficial owner", "UBO"),
        ("shareholder", "SHAREHOLDER"),
        ("officer", "OFFICER"),
        ("secretary", "SECRETARY"),
        ("chairman", "CHAIRMAN"),
        ("ceo", "CEO"),
        ("cfo", "CFO"),
        ("manager", "MANAGER"),
        ("partner", "PARTNER"),
        ("trustee", "TRUSTEE"),
        ("signatory", "SIGNATORY"),
        ("authorized", "AUTHORISED_SIGNATORY"),
    ];

    for (pattern, role) in role_patterns {
        if input.contains(pattern) {
            return Some(role.to_string());
        }
    }

    None
}

/// Parse entity association
fn parse_association(input: &str) -> Option<String> {
    // Patterns: "at X", "works for X", "from X company", "employed by X"
    let patterns = [
        " at ",
        " works for ",
        " employed by ",
        " works at ",
        " from ",
    ];

    for pattern in patterns {
        if let Some(idx) = input.find(pattern) {
            let rest = &input[idx + pattern.len()..];
            // Take words until punctuation or end
            let entity: String = rest
                .split([',', '.', ';'])
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if !entity.is_empty() && entity.len() > 2 {
                return Some(entity);
            }
        }
    }

    None
}

/// Parse jurisdiction
fn parse_jurisdiction(input: &str) -> Option<String> {
    let patterns = [
        ("luxembourg", "LU"),
        ("lux ", "LU"),
        ("cayman", "KY"),
        ("jersey", "JE"),
        ("guernsey", "GG"),
        ("ireland", "IE"),
        ("delaware", "US-DE"),
        ("singapore", "SG"),
        ("hong kong", "HK"),
        ("switzerland", "CH"),
        ("liechtenstein", "LI"),
    ];

    for (pattern, code) in patterns {
        if input.contains(pattern) {
            return Some(code.to_string());
        }
    }

    None
}

// ============================================================================
// Vocabulary and Metadata Handlers
// ============================================================================

/// POST /api/agent/validate - Validate DSL
/// POST /api/agent/generate - Generate DSL from natural language
async fn generate_dsl(Json(req): Json<GenerateDslRequest>) -> Json<GenerateDslResponse> {
    // Get vocabulary for the prompt
    let vocab = build_vocab_prompt(req.domain.as_deref());

    // Build the system prompt with onboarding context
    let system_prompt = format!(
        r#"You are a DSL generator for a KYC/AML onboarding system.
Generate valid DSL S-expressions from natural language instructions.

AVAILABLE VERBS:
{}

DSL SYNTAX:
- Format: (domain.verb :key "value" :key2 value2)
- Strings must be quoted: "text"
- Numbers are unquoted: 42, 25.5
- References start with @: @symbol_name (use underscores, not hyphens)
- Use :as @name to capture results

## CRITICAL: EXISTING vs NEW CBUs

**EXISTING CBU** - When user references an existing CBU by name (e.g., "onboard Aviva", "add custody to Apex"):
- Use `cbu.add-product` to add a product to the existing CBU
- The CBU name is matched case-insensitively in the database
- DO NOT use `cbu.ensure` - that would create a duplicate!

**NEW CBU** - Only when explicitly creating a new client:
- Use `cbu.ensure` to create the CBU first
- Then use `cbu.add-product` to add products

### EXAMPLE: Adding product to EXISTING CBU
User: "Onboard Aviva to Custody product"
```
(cbu.add-product :cbu-id "Aviva" :product "CUSTODY")
```

User: "Add Fund Accounting to Apex Capital"
```
(cbu.add-product :cbu-id "Apex Capital" :product "FUND_ACCOUNTING")
```

### EXAMPLE: Creating NEW CBU and adding product
User: "Create a new fund called Pacific Growth in Luxembourg and add Custody"
```
(cbu.ensure :name "Pacific Growth" :jurisdiction "LU" :client-type "fund" :as @fund)
(cbu.add-product :cbu-id @fund :product "CUSTODY")
```

## Available Products (use product CODE, not display name)

| Product Code | Description |
|--------------|-------------|
| `CUSTODY` | Asset safekeeping, settlement, corporate actions |
| `FUND_ACCOUNTING` | NAV calculation, investor accounting, reporting |
| `TRANSFER_AGENCY` | Investor registry, subscriptions, redemptions |
| `MIDDLE_OFFICE` | Position management, trade capture, P&L |
| `COLLATERAL_MGMT` | Collateral optimization and margin |
| `MARKETS_FX` | Foreign exchange services |
| `ALTS` | Alternative investment administration |

## Client Types
- `fund` - Investment fund (hedge fund, mutual fund, etc.)
- `corporate` - Corporate client
- `individual` - Individual client
- `trust` - Trust structure

## Common Jurisdictions
- `US` - United States
- `GB` - United Kingdom
- `LU` - Luxembourg
- `IE` - Ireland
- `KY` - Cayman Islands
- `JE` - Jersey

## Other DSL Examples

Create entities:
(entity.create-proper-person :first-name "John" :last-name "Smith" :date-of-birth "1980-01-15" :as @john)
(entity.create-limited-company :name "Holdings Ltd" :jurisdiction "GB" :as @company)

Assign roles (note: @fund must be defined first with cbu.ensure):
(cbu.ensure :name "Acme Fund" :jurisdiction "LU" :client-type "fund" :as @fund)
(cbu.assign-role :cbu-id @fund :entity-id @john :role "DIRECTOR")
(cbu.assign-role :cbu-id @fund :entity-id @company :role "PRINCIPAL")

List CBUs:
(cbu.list)

Respond with ONLY the DSL, no explanation. If you cannot generate valid DSL, respond with: ERROR: <reason>"#,
        vocab
    );

    // Create LLM client (uses AGENT_BACKEND env var to select provider)
    let llm_client = match crate::agentic::create_llm_client() {
        Ok(client) => client,
        Err(e) => {
            return Json(GenerateDslResponse {
                dsl: None,
                explanation: None,
                error: Some(format!("LLM client error: {}", e)),
            });
        }
    };

    // Call LLM API with JSON output format
    let json_system_prompt = format!(
        "{}\n\nIMPORTANT: Always respond with valid JSON in this exact format:\n{{\n  \"dsl\": \"(verb.name :arg value ...)\",\n  \"explanation\": \"Brief explanation of what the DSL does\"\n}}\n\nIf you cannot generate DSL, respond with:\n{{\n  \"dsl\": null,\n  \"explanation\": null,\n  \"error\": \"Error message explaining why\"\n}}",
        system_prompt
    );

    let response = llm_client
        .chat_json(&json_system_prompt, &req.instruction)
        .await;

    match response {
        Ok(content) => {
            match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(structured) => {
                    let dsl = structured["dsl"].as_str().map(|s| s.to_string());
                    let explanation = structured["explanation"].as_str().map(|s| s.to_string());
                    let error = structured["error"].as_str().map(|s| s.to_string());

                    if let Some(err) = error {
                        Json(GenerateDslResponse {
                            dsl: None,
                            explanation,
                            error: Some(err),
                        })
                    } else if let Some(ref dsl_str) = dsl {
                        // Validate the generated DSL
                        match parse_program(dsl_str) {
                            Ok(_) => Json(GenerateDslResponse {
                                dsl,
                                explanation,
                                error: None,
                            }),
                            Err(e) => Json(GenerateDslResponse {
                                dsl,
                                explanation,
                                error: Some(format!("Syntax error: {}", e)),
                            }),
                        }
                    } else {
                        Json(GenerateDslResponse {
                            dsl: None,
                            explanation,
                            error: Some("No DSL in response".to_string()),
                        })
                    }
                }
                Err(e) => Json(GenerateDslResponse {
                    dsl: None,
                    explanation: None,
                    error: Some(format!("Failed to parse structured response: {}", e)),
                }),
            }
        }
        Err(e) => Json(GenerateDslResponse {
            dsl: None,
            explanation: None,
            error: Some(format!("LLM API error: {}", e)),
        }),
    }
}

/// POST /api/agent/generate-with-tools - Generate DSL using Claude tool_use
///
/// This endpoint uses Claude's tool calling feature to look up real database IDs
/// before generating DSL, preventing UUID hallucination.
async fn generate_dsl_with_tools(
    State(state): State<AgentState>,
    Json(req): Json<GenerateDslRequest>,
) -> Json<GenerateDslResponse> {
    let api_key = match std::env::var("ANTHROPIC_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            return Json(GenerateDslResponse {
                dsl: None,
                explanation: None,
                error: Some("ANTHROPIC_API_KEY not configured".to_string()),
            });
        }
    };

    let vocab = build_vocab_prompt(req.domain.as_deref());
    let system_prompt = build_tool_use_system_prompt(&vocab);

    // Define tools for Claude
    let tools = serde_json::json!([
        {
            "name": "lookup_cbu",
            "description": "Look up an existing CBU (Client Business Unit) by name. ALWAYS use this before referencing a CBU to get the real ID.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "CBU name to search for (case-insensitive)"
                    }
                },
                "required": ["name"]
            }
        },
        {
            "name": "lookup_entity",
            "description": "Look up an existing entity by name. Use this to find persons or companies.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Entity name to search for"
                    },
                    "entity_type": {
                        "type": "string",
                        "description": "Optional: filter by type (proper_person, limited_company, etc.)"
                    }
                },
                "required": ["name"]
            }
        },
        {
            "name": "lookup_product",
            "description": "Look up available products by name.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Product name to search for"
                    }
                },
                "required": ["name"]
            }
        },
        {
            "name": "list_cbus",
            "description": "List all CBUs in the system. Use this to see what clients exist.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "description": "Max results to return (default 10)"
                    }
                }
            }
        }
    ]);

    let client = reqwest::Client::new();

    // First call - may include tool use
    let mut messages = vec![serde_json::json!({"role": "user", "content": req.instruction})];

    let mut tool_results: Vec<String> = Vec::new();
    let max_iterations = 5;

    for iteration in 0..max_iterations {
        let response = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&serde_json::json!({
                "model": "claude-sonnet-4-20250514",
                "max_tokens": 2048,
                "system": system_prompt,
                "tools": tools,
                "messages": messages
            }))
            .send()
            .await;

        let resp = match response {
            Ok(r) => r,
            Err(e) => {
                return Json(GenerateDslResponse {
                    dsl: None,
                    explanation: None,
                    error: Some(format!("Request failed: {}", e)),
                });
            }
        };

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Json(GenerateDslResponse {
                dsl: None,
                explanation: None,
                error: Some(format!("API error {}: {}", status, body)),
            });
        }

        let json: serde_json::Value = match resp.json().await {
            Ok(j) => j,
            Err(e) => {
                return Json(GenerateDslResponse {
                    dsl: None,
                    explanation: None,
                    error: Some(format!("Failed to parse response: {}", e)),
                });
            }
        };

        let stop_reason = json["stop_reason"].as_str().unwrap_or("");

        // Check if Claude wants to use tools
        if stop_reason == "tool_use" {
            let empty_vec = vec![];
            let content = json["content"].as_array().unwrap_or(&empty_vec);
            let mut tool_use_results = Vec::new();

            for block in content {
                if block["type"] == "tool_use" {
                    let tool_name = block["name"].as_str().unwrap_or("");
                    let tool_id = block["id"].as_str().unwrap_or("");
                    let input = &block["input"];

                    // Execute the tool
                    let result = execute_tool(&state.pool, tool_name, input).await;
                    tool_results.push(format!("{}: {}", tool_name, result));

                    tool_use_results.push(serde_json::json!({
                        "type": "tool_result",
                        "tool_use_id": tool_id,
                        "content": result
                    }));
                }
            }

            // Add assistant message with tool use
            messages.push(serde_json::json!({
                "role": "assistant",
                "content": content
            }));

            // Add tool results
            messages.push(serde_json::json!({
                "role": "user",
                "content": tool_use_results
            }));

            tracing::debug!("Tool use iteration {}: {:?}", iteration, tool_results);
            continue;
        }

        // Claude finished - extract the DSL
        // Note: tool_use mode doesn't support structured outputs, so we extract DSL from text
        let empty_vec2 = vec![];
        let content = json["content"].as_array().unwrap_or(&empty_vec2);
        for block in content {
            if block["type"] == "text" {
                let text = block["text"].as_str().unwrap_or("").trim();

                if text.starts_with("ERROR:") {
                    return Json(GenerateDslResponse {
                        dsl: None,
                        explanation: None,
                        error: Some(text.to_string()),
                    });
                }

                // Try to extract DSL from the response (handle markdown fencing, etc.)
                let dsl_text = extract_dsl_from_text(text);

                // Validate the generated DSL
                match parse_program(&dsl_text) {
                    Ok(_) => {
                        let explanation = if tool_results.is_empty() {
                            "DSL generated successfully".to_string()
                        } else {
                            format!("DSL generated with lookups: {}", tool_results.join(", "))
                        };
                        return Json(GenerateDslResponse {
                            dsl: Some(dsl_text),
                            explanation: Some(explanation),
                            error: None,
                        });
                    }
                    Err(e) => {
                        return Json(GenerateDslResponse {
                            dsl: Some(dsl_text),
                            explanation: None,
                            error: Some(format!("Generated DSL has syntax error: {}", e)),
                        });
                    }
                }
            }
        }

        break;
    }

    Json(GenerateDslResponse {
        dsl: None,
        explanation: None,
        error: Some("Failed to generate DSL after max iterations".to_string()),
    })
}

/// Execute a tool call via EntityGateway and return the result as a string
///
/// All lookups go through the central EntityGateway service for consistent
/// fuzzy matching behavior across LSP, validation, MCP tools, and Claude tool_use.
async fn execute_tool(_pool: &PgPool, tool_name: &str, input: &serde_json::Value) -> String {
    // Connect to EntityGateway
    let addr = crate::dsl_v2::gateway_resolver::gateway_addr();
    let mut client = match entity_gateway::proto::ob::gateway::v1::entity_gateway_client::EntityGatewayClient::connect(addr.clone()).await {
        Ok(c) => c,
        Err(e) => return format!("Failed to connect to EntityGateway at {}: {}", addr, e),
    };

    // Helper to search via gateway
    async fn gateway_search(
        client: &mut entity_gateway::proto::ob::gateway::v1::entity_gateway_client::EntityGatewayClient<tonic::transport::Channel>,
        nickname: &str,
        search: &str,
        limit: i32,
    ) -> Result<Vec<(String, String, f32)>, String> {
        use entity_gateway::proto::ob::gateway::v1::{SearchMode, SearchRequest};

        let request = SearchRequest {
            nickname: nickname.to_string(),
            values: vec![search.to_string()],
            search_key: None,
            mode: SearchMode::Fuzzy as i32,
            limit: Some(limit),
            discriminators: std::collections::HashMap::new(),
            tenant_id: None,
            cbu_id: None,
        };

        let response = client
            .search(request)
            .await
            .map_err(|e| format!("EntityGateway search failed: {}", e))?;

        Ok(response
            .into_inner()
            .matches
            .into_iter()
            .map(|m| (m.token, m.display, m.score))
            .collect())
    }

    match tool_name {
        "lookup_cbu" => {
            let name = input["name"].as_str().unwrap_or("");
            match gateway_search(&mut client, "CBU", name, 5).await {
                Ok(matches) if !matches.is_empty() => {
                    let results: Vec<String> = matches
                        .iter()
                        .map(|(id, display, _)| format!("- {} (id: {})", display, id))
                        .collect();
                    format!("Found {} CBU(s):\n{}", matches.len(), results.join("\n"))
                }
                Ok(_) => format!("No CBU found matching '{}'", name),
                Err(e) => e,
            }
        }
        "lookup_entity" => {
            let name = input["name"].as_str().unwrap_or("");
            // Use ENTITY nickname which searches across all entity types
            match gateway_search(&mut client, "ENTITY", name, 5).await {
                Ok(matches) if !matches.is_empty() => {
                    let results: Vec<String> = matches
                        .iter()
                        .map(|(id, display, _)| format!("- {} (id: {})", display, id))
                        .collect();
                    format!("Found {} entity(s):\n{}", matches.len(), results.join("\n"))
                }
                Ok(_) => format!("No entity found matching '{}'", name),
                Err(e) => e,
            }
        }
        "lookup_product" => {
            let name = input["name"].as_str().unwrap_or("");
            match gateway_search(&mut client, "PRODUCT", name, 5).await {
                Ok(matches) if !matches.is_empty() => {
                    let results: Vec<String> = matches
                        .iter()
                        .map(|(id, display, _)| format!("- {} (code: {})", display, id))
                        .collect();
                    format!(
                        "Found {} product(s):\n{}",
                        matches.len(),
                        results.join("\n")
                    )
                }
                Ok(_) => format!("No product found matching '{}'", name),
                Err(e) => e,
            }
        }
        "list_cbus" => {
            let limit = input["limit"].as_i64().unwrap_or(10) as i32;
            // Empty search with high limit to list all
            match gateway_search(&mut client, "CBU", "", limit).await {
                Ok(matches) => {
                    let results: Vec<String> = matches
                        .iter()
                        .map(|(_, display, _)| format!("- {}", display))
                        .collect();
                    format!("CBUs in system:\n{}", results.join("\n"))
                }
                Err(e) => e,
            }
        }
        _ => format!("Unknown tool: {}", tool_name),
    }
}

/// Build system prompt for tool-use generation
fn build_tool_use_system_prompt(vocab: &str) -> String {
    format!(
        r#"You are a DSL generator for a KYC/AML onboarding system.
Generate valid DSL S-expressions from natural language instructions.

## CRITICAL WORKFLOW

1. **ALWAYS look up existing data first** before generating DSL that references existing entities:
   - Use `lookup_cbu` when user mentions a client/CBU name
   - Use `lookup_entity` when user mentions a person or company
   - Use `lookup_product` when adding products
   - Use `list_cbus` if unsure what clients exist

2. **Use names, not UUIDs** - The DSL system accepts names for CBUs:
   - `(cbu.add-product :cbu-id "Apex Capital" :product "CUSTODY")` ✓
   - The name is matched case-insensitively in the database

3. **Create new entities with @references**:
   - `(cbu.ensure :name "New Fund" :jurisdiction "LU" :as @fund)`
   - Then reference: `(cbu.add-product :cbu-id @fund :product "CUSTODY")`

## AVAILABLE VERBS
{}

## DSL SYNTAX
- Format: (domain.verb :key "value" :key2 value2)
- Strings must be quoted: "text"
- Numbers are unquoted: 42, 25.5
- References start with @: @symbol_name
- Use :as @name to capture results

## EXAMPLES

Adding product to EXISTING CBU (after lookup confirms it exists):
```
(cbu.add-product :cbu-id "Apex Capital" :product "CUSTODY")
```

Creating NEW CBU and adding product:
```
(cbu.ensure :name "Pacific Growth" :jurisdiction "LU" :client-type "fund" :as @fund)
(cbu.add-product :cbu-id @fund :product "CUSTODY")
```

## PRODUCTS (use exact CODES)
- CUSTODY
- FUND_ACCOUNTING
- TRANSFER_AGENCY
- MIDDLE_OFFICE
- COLLATERAL_MGMT
- MARKETS_FX
- ALTS

Respond with ONLY the DSL, no explanation. If you cannot generate valid DSL, respond with: ERROR: <reason>"#,
        vocab
    )
}

/// Extract DSL from agent response text
///
/// Handles common agent output formats:
/// 1. Raw DSL text (ideal)
/// 2. Markdown code fences (```dsl ... ``` or ``` ... ```)
/// 3. Text with DSL embedded (extracts S-expressions)
fn extract_dsl_from_text(text: &str) -> String {
    let trimmed = text.trim();

    // If it already parses as DSL, return as-is
    if trimmed.starts_with('(') && parse_program(trimmed).is_ok() {
        return trimmed.to_string();
    }

    // Try to extract from markdown code fence
    // Matches ```dsl, ```clojure, ```lisp, or just ```
    let fence_patterns = ["```dsl", "```clojure", "```lisp", "```"];
    for pattern in fence_patterns {
        if let Some(start_idx) = trimmed.find(pattern) {
            let after_fence = &trimmed[start_idx + pattern.len()..];
            // Skip to newline after opening fence
            let content_start = after_fence.find('\n').map(|i| i + 1).unwrap_or(0);
            let content = &after_fence[content_start..];
            // Find closing fence
            if let Some(end_idx) = content.find("```") {
                let extracted = content[..end_idx].trim();
                if parse_program(extracted).is_ok() {
                    return extracted.to_string();
                }
            }
        }
    }

    // Try to find S-expression block in text
    // Look for opening paren and find matching close
    if let Some(start) = trimmed.find('(') {
        let mut depth = 0;
        let mut end = start;
        for (i, c) in trimmed[start..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = start + i + 1;
                        // Check if there are more statements
                        let remaining = trimmed[end..].trim();
                        if remaining.starts_with('(') {
                            // Multiple statements - find the last closing paren
                            continue;
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
        // Find last balanced paren for multi-statement DSL
        let mut last_end = end;
        let mut search_start = end;
        while let Some(next_start) = trimmed[search_start..].find('(') {
            let abs_start = search_start + next_start;
            let mut d = 0;
            for (i, c) in trimmed[abs_start..].char_indices() {
                match c {
                    '(' => d += 1,
                    ')' => {
                        d -= 1;
                        if d == 0 {
                            last_end = abs_start + i + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            search_start = last_end;
        }
        let extracted = trimmed[start..last_end].trim();
        if parse_program(extracted).is_ok() {
            return extracted.to_string();
        }
    }

    // Fall back to original text
    trimmed.to_string()
}

/// Build vocabulary prompt for a domain
fn build_vocab_prompt(domain: Option<&str>) -> String {
    let mut lines = Vec::new();
    let reg = registry();

    let domain_list: Vec<String> = if let Some(d) = domain {
        vec![d.to_string()]
    } else {
        reg.domains().to_vec()
    };

    for domain_name in domain_list {
        for verb in reg.verbs_for_domain(&domain_name) {
            let required = verb.required_arg_names().join(", ");
            let optional = verb.optional_arg_names().join(", ");
            lines.push(format!(
                "{}.{}: {} [required: {}] [optional: {}]",
                verb.domain, verb.verb, verb.description, required, optional
            ));
        }
    }

    lines.join("\n")
}

/// POST /api/agent/validate - Validate DSL syntax and semantics (including dataflow)
async fn validate_dsl(
    State(state): State<AgentState>,
    Json(req): Json<ValidateDslRequest>,
) -> Result<Json<ValidationResult>, StatusCode> {
    use crate::dsl_v2::validation::{Severity, ValidationContext, ValidationRequest};

    // First parse
    if let Err(e) = parse_program(&req.dsl) {
        return Ok(Json(ValidationResult {
            valid: false,
            errors: vec![ValidationError {
                line: None,
                column: None,
                message: e,
                suggestion: None,
            }],
            warnings: vec![],
        }));
    }

    // Then run full semantic validation with CSG linter (includes dataflow)
    let validator_result = async {
        let v = SemanticValidator::new(state.pool.clone()).await?;
        v.with_csg_linter().await
    }
    .await;

    match validator_result {
        Ok(mut validator) => {
            let request = ValidationRequest {
                source: req.dsl.clone(),
                context: ValidationContext::default(),
            };
            match validator.validate(&request).await {
                crate::dsl_v2::validation::ValidationResult::Ok(_) => Ok(Json(ValidationResult {
                    valid: true,
                    errors: vec![],
                    warnings: vec![],
                })),
                crate::dsl_v2::validation::ValidationResult::Err(diagnostics) => {
                    let errors: Vec<ValidationError> = diagnostics
                        .iter()
                        .filter(|d| d.severity == Severity::Error)
                        .map(|d| ValidationError {
                            line: Some(d.span.line as usize),
                            column: Some(d.span.column as usize),
                            message: format!("[{}] {}", d.code.as_str(), d.message),
                            suggestion: d.suggestions.first().map(|s| s.message.clone()),
                        })
                        .collect();
                    let warnings: Vec<String> = diagnostics
                        .iter()
                        .filter(|d| d.severity == Severity::Warning)
                        .map(|d| format!("[{}] {}", d.code.as_str(), d.message))
                        .collect();
                    Ok(Json(ValidationResult {
                        valid: errors.is_empty(),
                        errors,
                        warnings,
                    }))
                }
            }
        }
        Err(_) => {
            // If validator fails to initialize, fall back to parse-only validation
            Ok(Json(ValidationResult {
                valid: true,
                errors: vec![],
                warnings: vec![],
            }))
        }
    }
}

/// GET /api/agent/domains - List available domains
async fn list_domains() -> Json<DomainsResponse> {
    let reg = registry();
    let domain_list = reg.domains();
    let domains: Vec<DomainInfo> = domain_list
        .iter()
        .map(|name| {
            let verbs = reg.verbs_for_domain(name);
            DomainInfo {
                name: name.to_string(),
                description: get_domain_description(name),
                verb_count: verbs.len(),
            }
        })
        .collect();

    Json(DomainsResponse {
        total_verbs: reg.len(),
        domains,
    })
}

/// GET /api/agent/vocabulary - Get vocabulary
async fn get_vocabulary(Query(query): Query<VocabQuery>) -> Json<VocabResponse> {
    let reg = registry();
    let verbs: Vec<VerbInfo> = if let Some(domain) = &query.domain {
        reg.verbs_for_domain(domain)
            .iter()
            .map(|v| VerbInfo {
                domain: v.domain.to_string(),
                name: v.verb.to_string(),
                full_name: format!("{}.{}", v.domain, v.verb),
                description: v.description.to_string(),
                required_args: v
                    .required_arg_names()
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                optional_args: v
                    .optional_arg_names()
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            })
            .collect()
    } else {
        // Return all verbs
        reg.domains()
            .iter()
            .flat_map(|d| {
                reg.verbs_for_domain(d)
                    .iter()
                    .map(|v| VerbInfo {
                        domain: v.domain.to_string(),
                        name: v.verb.to_string(),
                        full_name: format!("{}.{}", v.domain, v.verb),
                        description: v.description.to_string(),
                        required_args: v
                            .required_arg_names()
                            .iter()
                            .map(|s| s.to_string())
                            .collect(),
                        optional_args: v
                            .optional_arg_names()
                            .iter()
                            .map(|s| s.to_string())
                            .collect(),
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    };

    Json(VocabResponse { verbs })
}

/// GET /api/agent/health - Health check
async fn health_check() -> Json<HealthResponse> {
    let reg = registry();
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        verb_count: reg.len(),
        domain_count: reg.domains().len(),
    })
}

// ============================================================================
// Completion Handler (LSP-style via EntityGateway)
// ============================================================================

/// POST /api/agent/complete - Get completions for entities
///
/// Provides LSP-style autocomplete for CBUs, entities, products, roles,
/// jurisdictions, and other reference data via EntityGateway.
async fn complete_entity(
    Json(req): Json<CompleteRequest>,
) -> Result<Json<CompleteResponse>, StatusCode> {
    // Map entity_type to EntityGateway nickname
    let nickname = match req.entity_type.to_lowercase().as_str() {
        "cbu" => "CBU",
        "entity" | "person" | "company" => "ENTITY",
        "product" => "PRODUCT",
        "role" => "ROLE",
        "jurisdiction" => "JURISDICTION",
        "currency" => "CURRENCY",
        "client_type" | "clienttype" => "CLIENT_TYPE",
        "instrument_class" | "instrumentclass" => "INSTRUMENT_CLASS",
        "market" => "MARKET",
        _ => {
            // Unknown type - return empty
            return Ok(Json(CompleteResponse {
                items: vec![],
                total: 0,
            }));
        }
    };

    // Connect to EntityGateway
    let addr = crate::dsl_v2::gateway_resolver::gateway_addr();
    let mut client = match entity_gateway::proto::ob::gateway::v1::entity_gateway_client::EntityGatewayClient::connect(addr.clone()).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to connect to EntityGateway at {}: {}", addr, e);
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
    };

    // Search via EntityGateway
    use entity_gateway::proto::ob::gateway::v1::{SearchMode, SearchRequest};

    let search_request = SearchRequest {
        nickname: nickname.to_string(),
        values: vec![req.query.clone()],
        search_key: None,
        mode: SearchMode::Fuzzy as i32,
        limit: Some(req.limit),
        discriminators: std::collections::HashMap::new(),
        tenant_id: None,
        cbu_id: None,
    };

    let response = match client.search(search_request).await {
        Ok(r) => r.into_inner(),
        Err(e) => {
            tracing::error!("EntityGateway search failed: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Convert to completion items
    let items: Vec<CompletionItem> = response
        .matches
        .into_iter()
        .map(|m| CompletionItem {
            value: m.token,
            label: m.display.clone(),
            detail: if m.display.contains('(') {
                // Extract detail from display if present, e.g., "Apex Fund (LU)"
                None
            } else {
                Some(nickname.to_string())
            },
            score: m.score,
        })
        .collect();

    let total = items.len();
    Ok(Json(CompleteResponse { items, total }))
}

// ============================================================================
// Onboarding Handlers
// ============================================================================

/// POST /api/agent/onboard - Generate onboarding DSL from natural language
///
/// Uses the enhanced system prompt with onboarding context to generate
/// complete onboarding workflows from natural language descriptions.
async fn generate_onboarding_dsl(
    State(state): State<AgentState>,
    Json(req): Json<OnboardingRequest>,
) -> Json<OnboardingResponse> {
    // Use the existing generate_dsl logic with onboarding-focused instruction
    let generate_req = GenerateDslRequest {
        instruction: req.description.clone(),
        domain: None, // Let it use all domains including resource and delivery
    };

    let gen_response = generate_dsl(Json(generate_req)).await;

    match (gen_response.dsl.clone(), gen_response.error.clone()) {
        (Some(dsl), None) => {
            // Validate the generated DSL
            let validation = match parse_program(&dsl) {
                Ok(_) => ValidationResult {
                    valid: true,
                    errors: vec![],
                    warnings: vec![],
                },
                Err(e) => ValidationResult {
                    valid: false,
                    errors: vec![ValidationError {
                        line: None,
                        column: None,
                        message: e,
                        suggestion: None,
                    }],
                    warnings: vec![],
                },
            };

            // Execute if requested and valid
            let execution = if req.execute && validation.valid {
                match execute_onboarding_dsl(&state, &dsl).await {
                    Ok(result) => Some(result),
                    Err(e) => Some(OnboardingExecutionResult {
                        success: false,
                        cbu_id: None,
                        resource_count: 0,
                        delivery_count: 0,
                        errors: vec![e],
                    }),
                }
            } else {
                None
            };

            Json(OnboardingResponse {
                dsl: Some(dsl),
                explanation: Some("Onboarding DSL generated successfully".to_string()),
                validation: Some(validation),
                execution,
                error: None,
            })
        }
        (_, Some(error)) => Json(OnboardingResponse {
            dsl: None,
            explanation: None,
            validation: None,
            execution: None,
            error: Some(error),
        }),
        _ => Json(OnboardingResponse {
            dsl: None,
            explanation: None,
            validation: None,
            execution: None,
            error: Some("Unknown error generating DSL".to_string()),
        }),
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Helper: Execute onboarding DSL and count results
async fn execute_onboarding_dsl(
    state: &AgentState,
    dsl: &str,
) -> Result<OnboardingExecutionResult, String> {
    use crate::dsl_v2::validation::{Severity, ValidationContext, ValidationRequest};

    let program = parse_program(dsl).map_err(|e| format!("Parse error: {}", e))?;

    // CSG validation (includes dataflow)
    let validator_result = async {
        let v = SemanticValidator::new(state.pool.clone()).await?;
        v.with_csg_linter().await
    }
    .await;

    if let Ok(mut validator) = validator_result {
        let request = ValidationRequest {
            source: dsl.to_string(),
            context: ValidationContext::default(),
        };
        if let crate::dsl_v2::validation::ValidationResult::Err(diagnostics) =
            validator.validate(&request).await
        {
            let errors: Vec<String> = diagnostics
                .iter()
                .filter(|d| d.severity == Severity::Error)
                .map(|d| format!("[{}] {}", d.code.as_str(), d.message))
                .collect();
            if !errors.is_empty() {
                return Err(format!("Validation errors: {}", errors.join("; ")));
            }
        }
    }

    let plan = compile(&program).map_err(|e| format!("Compile error: {}", e))?;

    let mut ctx = ExecutionContext::new();
    state
        .dsl_v2_executor
        .execute_plan(&plan, &mut ctx)
        .await
        .map_err(|e| format!("Execution error: {}", e))?;

    // Count resources and deliveries from bindings
    let cbu_id = ctx
        .symbols
        .get("cbu_id")
        .or_else(|| ctx.symbols.get("client"))
        .copied();

    let resource_count = ctx
        .symbols
        .keys()
        .filter(|k| {
            k.contains("custody")
                || k.contains("settle")
                || k.contains("swift")
                || k.contains("nav")
                || k.contains("ibor")
                || k.contains("pnl")
                || k.contains("ledger")
        })
        .count();

    let delivery_count = ctx
        .symbols
        .keys()
        .filter(|k| k.contains("delivery"))
        .count();

    Ok(OnboardingExecutionResult {
        success: true,
        cbu_id,
        resource_count,
        delivery_count,
        errors: vec![],
    })
}

fn get_domain_description(domain: &str) -> String {
    match domain {
        "cbu" => "Client Business Unit lifecycle management".to_string(),
        "entity" => "Legal entity creation and management".to_string(),
        "document" => "Document management and verification".to_string(),
        "kyc" => "KYC investigation and risk assessment".to_string(),
        "screening" => "PEP, sanctions, and adverse media screening".to_string(),
        "decision" => "Approval workflow and decision management".to_string(),
        "monitoring" => "Ongoing monitoring and periodic reviews".to_string(),
        "attribute" => "Attribute value management".to_string(),
        "resource" => "Resource instance management for onboarding".to_string(),
        "delivery" => "Service delivery tracking for onboarding".to_string(),
        _ => format!("{} domain operations", domain),
    }
}

// ============================================================================
// Additional Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ExecuteDslRequest {
    /// DSL source to execute. If None/missing, uses session's assembled_dsl.
    #[serde(default)]
    pub dsl: Option<String>,
}

/// Request body for REPL edit events
#[derive(Debug, Clone, Deserialize)]
pub struct ReplEditRequest {
    /// Current DSL content in the REPL editor
    pub current_dsl: String,
}

/// Response for REPL edit events
#[derive(Debug, Clone, Serialize)]
pub struct ReplEditResponse {
    /// Whether the edit was recorded
    pub recorded: bool,
    /// Whether the DSL differs from the proposed DSL
    pub has_edits: bool,
    /// Validation status of the current DSL
    pub valid: bool,
    /// Validation errors if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<String>>,
}

// NOTE: Direct /execute endpoint removed - use /api/session/:id/execute instead
// All DSL execution now requires a session for proper binding persistence and audit trail

/// Pipeline stage indicating where processing stopped
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PipelineStage {
    /// DSL source received but parse failed
    Draft,
    /// Parse succeeded - AST exists, tokens valid
    /// May have unresolved EntityRefs
    Parsed,
    /// AST has unresolved EntityRefs requiring user/agent resolution
    /// UI should show search popup for each unresolved ref
    Resolving,
    /// All EntityRefs resolved - ready for lint
    Resolved,
    /// CSG linter passed - dataflow valid
    Linted,
    /// Compile succeeded - execution plan ready
    Compiled,
    /// Execute succeeded - DB mutations committed
    Executed,
}

/// An unresolved EntityRef that needs user/agent resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnresolvedRef {
    /// Statement index in AST (0-based)
    pub statement_index: usize,
    /// Argument key containing the EntityRef
    pub arg_key: String,
    /// Entity type for search (e.g., "cbu", "entity", "product")
    pub entity_type: String,
    /// The search text entered by user
    pub search_text: String,
}

// ============================================================================
// Batch Operations Handlers
// ============================================================================

/// POST /api/batch/add-products - Add products to multiple CBUs
/// Server-side DSL generation and execution (no LLM needed)
async fn batch_add_products(
    State(state): State<AgentState>,
    Json(req): Json<BatchAddProductsRequest>,
) -> Json<BatchAddProductsResponse> {
    use std::time::Instant;

    let start = Instant::now();
    let mut results = Vec::new();
    let mut success_count = 0;
    let mut failure_count = 0;

    // Process each CBU × product combination
    for cbu_id in &req.cbu_ids {
        for product in &req.products {
            // Generate DSL server-side (deterministic, no LLM)
            let dsl = format!(
                r#"(cbu.add-product :cbu-id "{}" :product "{}")"#,
                cbu_id, product
            );

            // Parse and execute using shared executor (singleton batch)
            match parse_program(&dsl) {
                Ok(program) => {
                    let plan = match compile(&program) {
                        Ok(p) => p,
                        Err(e) => {
                            failure_count += 1;
                            results.push(BatchProductResult {
                                cbu_id: *cbu_id,
                                product: product.clone(),
                                success: false,
                                error: Some(format!("Compile error: {}", e)),
                                services_added: None,
                            });
                            continue;
                        }
                    };

                    let mut ctx = ExecutionContext::new().with_audit_user("batch_add_products");
                    match state.dsl_v2_executor.execute_plan(&plan, &mut ctx).await {
                        Ok(exec_results) => {
                            // Count services added from the result
                            let services_added = exec_results
                                .iter()
                                .filter_map(|r| {
                                    if let DslV2Result::Affected(n) = r {
                                        Some(*n as i32)
                                    } else {
                                        None
                                    }
                                })
                                .sum();

                            success_count += 1;
                            results.push(BatchProductResult {
                                cbu_id: *cbu_id,
                                product: product.clone(),
                                success: true,
                                error: None,
                                services_added: Some(services_added),
                            });
                        }
                        Err(e) => {
                            failure_count += 1;
                            results.push(BatchProductResult {
                                cbu_id: *cbu_id,
                                product: product.clone(),
                                success: false,
                                error: Some(format!("Execution error: {}", e)),
                                services_added: None,
                            });
                        }
                    }
                }
                Err(e) => {
                    failure_count += 1;
                    results.push(BatchProductResult {
                        cbu_id: *cbu_id,
                        product: product.clone(),
                        success: false,
                        error: Some(format!("Parse error: {:?}", e)),
                        services_added: None,
                    });
                }
            }
        }
    }

    let duration_ms = start.elapsed().as_millis() as u64;

    Json(BatchAddProductsResponse {
        total_operations: results.len(),
        success_count,
        failure_count,
        duration_ms,
        results,
    })
}

// ============================================================================
// Learning/Feedback Handlers
// ============================================================================

/// Request to report a user correction (for learning loop)
#[derive(Debug, Deserialize)]
pub struct ReportCorrectionRequest {
    /// Session ID where correction occurred
    pub session_id: Uuid,
    /// Original user message that triggered DSL generation
    #[serde(default)]
    pub original_message: Option<String>,
    /// DSL generated by the agent
    pub generated_dsl: String,
    /// DSL after user correction (what was actually executed)
    pub corrected_dsl: String,
}

/// Response from reporting a correction
#[derive(Debug, Serialize)]
pub struct ReportCorrectionResponse {
    /// Whether the correction was recorded
    pub recorded: bool,
    /// Event ID for tracking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<i64>,
}

/// POST /api/agent/correction - Report a user correction for learning
///
/// Called by the UI when a user edits agent-generated DSL before executing.
/// This feeds into the continuous improvement loop.
async fn report_correction(
    State(state): State<AgentState>,
    Json(req): Json<ReportCorrectionRequest>,
) -> Json<ReportCorrectionResponse> {
    use crate::agent::learning::{AgentEvent, AgentEventPayload};

    tracing::info!(
        "Recording user correction for session {}: {} chars -> {} chars",
        req.session_id,
        req.generated_dsl.len(),
        req.corrected_dsl.len()
    );

    // Classify the correction type by analyzing the diff
    let correction_type = classify_correction(&req.generated_dsl, &req.corrected_dsl);

    // Build the event
    let event = AgentEvent {
        timestamp: chrono::Utc::now(),
        session_id: Some(req.session_id),
        payload: AgentEventPayload::UserCorrection {
            original_message: req.original_message.unwrap_or_default(),
            generated_dsl: req.generated_dsl,
            corrected_dsl: req.corrected_dsl,
            correction_type,
        },
    };

    // Store directly to database (fire-and-forget style, but we wait for event_id)
    let event_id = match store_correction_event(&state.pool, &event).await {
        Ok(id) => Some(id),
        Err(e) => {
            tracing::error!("Failed to store correction event: {}", e);
            None
        }
    };

    Json(ReportCorrectionResponse {
        recorded: event_id.is_some(),
        event_id,
    })
}

/// Classify the type of correction by analyzing the diff
fn classify_correction(generated: &str, corrected: &str) -> crate::agent::learning::CorrectionType {
    use crate::agent::learning::CorrectionType;

    // Simple heuristics - can be made more sophisticated
    let gen_lines: Vec<&str> = generated.lines().collect();
    let cor_lines: Vec<&str> = corrected.lines().collect();

    // Check for full rewrite (very different)
    let similarity = compute_line_similarity(&gen_lines, &cor_lines);
    if similarity < 0.3 {
        return CorrectionType::FullRewrite;
    }

    // Check for additions (corrected has more content)
    if cor_lines.len() > gen_lines.len() && corrected.contains(generated.trim()) {
        let added = corrected.replace(generated.trim(), "").trim().to_string();
        if !added.is_empty() {
            return CorrectionType::Addition { added };
        }
    }

    // Check for removals (generated has more content)
    if gen_lines.len() > cor_lines.len() && generated.contains(corrected.trim()) {
        let removed = generated.replace(corrected.trim(), "").trim().to_string();
        if !removed.is_empty() {
            return CorrectionType::Removal { removed };
        }
    }

    // Check for verb changes (look for domain.verb pattern changes)
    let gen_verbs: Vec<&str> = gen_lines.iter().filter_map(|l| extract_verb(l)).collect();
    let cor_verbs: Vec<&str> = cor_lines.iter().filter_map(|l| extract_verb(l)).collect();

    if gen_verbs.len() == 1 && cor_verbs.len() == 1 && gen_verbs[0] != cor_verbs[0] {
        return CorrectionType::VerbChange {
            from_verb: gen_verbs[0].to_string(),
            to_verb: cor_verbs[0].to_string(),
        };
    }

    // Default to full rewrite if we can't classify more specifically
    CorrectionType::FullRewrite
}

/// Extract verb from a DSL line like "(domain.verb ...)"
fn extract_verb(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if !trimmed.starts_with('(') {
        return None;
    }
    // Find the verb: between '(' and first space or ')'
    let start = 1;
    let end = trimmed[start..]
        .find(|c: char| c.is_whitespace() || c == ')')
        .map(|i| i + start)?;
    Some(&trimmed[start..end])
}

/// Compute simple line-based similarity (0.0 to 1.0)
fn compute_line_similarity(a: &[&str], b: &[&str]) -> f32 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let a_set: std::collections::HashSet<&str> = a.iter().map(|s| s.trim()).collect();
    let b_set: std::collections::HashSet<&str> = b.iter().map(|s| s.trim()).collect();

    let intersection = a_set.intersection(&b_set).count();
    let union = a_set.union(&b_set).count();

    if union == 0 {
        return 1.0;
    }

    intersection as f32 / union as f32
}

/// Store correction event directly to database
async fn store_correction_event(
    pool: &PgPool,
    event: &crate::agent::learning::AgentEvent,
) -> Result<i64, sqlx::Error> {
    use crate::agent::learning::AgentEventPayload;

    let event_type = event.payload.event_type_str();

    // Extract fields from the UserCorrection payload
    let (user_message, generated_dsl, corrected_dsl, correction_type) =
        if let AgentEventPayload::UserCorrection {
            ref original_message,
            ref generated_dsl,
            ref corrected_dsl,
            ref correction_type,
        } = event.payload
        {
            (
                Some(original_message.clone()),
                Some(generated_dsl.clone()),
                Some(corrected_dsl.clone()),
                Some(format!("{:?}", correction_type)),
            )
        } else {
            (None, None, None, None)
        };

    let event_id = sqlx::query_scalar!(
        r#"
        INSERT INTO agent.events (
            session_id, event_type, user_message, generated_dsl,
            corrected_dsl, correction_type, was_corrected
        )
        VALUES ($1, $2, $3, $4, $5, $6, true)
        RETURNING id
        "#,
        event.session_id,
        event_type,
        user_message,
        generated_dsl,
        corrected_dsl,
        correction_type,
    )
    .fetch_one(pool)
    .await?;

    tracing::debug!("Stored correction event with ID {}", event_id);
    Ok(event_id)
}

// ============================================================================
// Verb Disambiguation Selection (closes the learning loop)
// ============================================================================

/// POST /api/session/:id/select-verb
///
/// Called when user clicks a verb option in disambiguation UI.
/// This is GOLD-STANDARD training data - user explicitly chose from alternatives.
///
/// Flow:
/// 1. Record learning signal (input → selected_verb, confidence=0.95)
/// 2. Record negative signals for rejected alternatives
/// 3. Re-run intent pipeline with selected verb to generate DSL
/// 4. Return DSL ready for execution
async fn select_verb_disambiguation(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<ob_poc_types::VerbSelectionRequest>,
) -> Result<Json<ob_poc_types::VerbSelectionResponse>, StatusCode> {
    tracing::info!(
        session_id = %session_id,
        selected_verb = %req.selected_verb,
        original_input = %req.original_input,
        num_candidates = req.all_candidates.len(),
        "Recording verb disambiguation selection"
    );

    // 1. Record learning signal to database (gold-standard, confidence=0.95)
    let learning_recorded = match record_verb_selection_signal(
        &state.pool,
        &req.original_input,
        &req.selected_verb,
        &req.all_candidates,
    )
    .await
    {
        Ok(_) => {
            tracing::info!(
                "Recorded gold-standard learning signal: '{}' → '{}'",
                req.original_input,
                req.selected_verb
            );
            true
        }
        Err(e) => {
            tracing::error!("Failed to record learning signal: {}", e);
            false
        }
    };

    // 2. Get session and re-run pipeline with selected verb
    let mut sessions = state.sessions.write().await;
    let session = match sessions.get_mut(&session_id) {
        Some(s) => s,
        None => {
            return Ok(Json(ob_poc_types::VerbSelectionResponse {
                recorded: learning_recorded,
                execution_result: None,
                message: "Session not found".to_string(),
            }));
        }
    };

    // 3. Generate DSL for the selected verb using the original input
    // This goes through the normal pipeline but we force the verb selection
    let dsl_result = generate_dsl_for_selected_verb(
        &state.pool,
        &req.original_input,
        &req.selected_verb,
        session,
    )
    .await;

    match dsl_result {
        Ok(dsl) => {
            // Stage the DSL in session
            let ast = parse_program(&dsl)
                .map(|p| p.statements)
                .unwrap_or_default();
            session.set_pending_dsl(dsl.clone(), ast, None, false);

            let msg = format!(
                "Selected '{}'. Staged: {}\n\nSay 'run' to execute.",
                req.selected_verb, dsl
            );
            session.add_agent_message(msg.clone(), None, Some(dsl));

            Ok(Json(ob_poc_types::VerbSelectionResponse {
                recorded: learning_recorded,
                execution_result: None,
                message: msg,
            }))
        }
        Err(e) => Ok(Json(ob_poc_types::VerbSelectionResponse {
            recorded: learning_recorded,
            execution_result: None,
            message: format!("Failed to generate DSL: {}", e),
        })),
    }
}

/// Record verb selection as gold-standard learning signal
///
/// This is HIGH CONFIDENCE data (confidence=0.95) because:
/// - User was shown multiple options
/// - User explicitly clicked one
/// - This is an active correction, not passive acceptance
///
/// Uses agent.user_learned_phrases table for immediate effect on verb search.
/// Uses a "global" user_id (all zeros) since this is system-wide learning.
///
/// Also generates and stores phrase variants (confidence=0.85) to make learning
/// more robust to phrasings like "show me the cbus" vs "list all cbus".
async fn record_verb_selection_signal(
    pool: &PgPool,
    original_input: &str,
    selected_verb: &str,
    all_candidates: &[String],
) -> Result<(), sqlx::Error> {
    // Use a "global" user_id for system-wide disambiguation learning
    // This allows the learning to benefit all users immediately
    let global_user_id = Uuid::nil(); // 00000000-0000-0000-0000-000000000000

    // Insert primary phrase with gold-standard confidence (0.95)
    sqlx::query!(
        r#"
        INSERT INTO agent.user_learned_phrases (
            user_id,
            phrase,
            verb,
            occurrence_count,
            confidence,
            source,
            created_at,
            updated_at
        )
        VALUES ($1, $2, $3, 1, 0.95, 'user_disambiguation', NOW(), NOW())
        ON CONFLICT (user_id, phrase)
        DO UPDATE SET
            occurrence_count = agent.user_learned_phrases.occurrence_count + 1,
            confidence = GREATEST(agent.user_learned_phrases.confidence, 0.95),
            verb = EXCLUDED.verb,
            updated_at = NOW()
        "#,
        global_user_id,
        original_input,
        selected_verb,
    )
    .execute(pool)
    .await?;

    // Generate and store phrase variants with slightly lower confidence (0.85)
    // This addresses the "too literal" learning failure case
    let variants = generate_phrase_variants(original_input);
    let mut variants_stored = 0;
    for variant in &variants {
        if variant != original_input {
            sqlx::query!(
                r#"
                INSERT INTO agent.user_learned_phrases (
                    user_id,
                    phrase,
                    verb,
                    occurrence_count,
                    confidence,
                    source,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, 1, 0.85, 'generated_variant', NOW(), NOW())
                ON CONFLICT (user_id, phrase)
                DO UPDATE SET
                    occurrence_count = agent.user_learned_phrases.occurrence_count + 1,
                    confidence = GREATEST(agent.user_learned_phrases.confidence, 0.85),
                    updated_at = NOW()
                "#,
                global_user_id,
                variant,
                selected_verb,
            )
            .execute(pool)
            .await?;
            variants_stored += 1;
        }
    }

    // Record to phrase_blocklist for rejected alternatives
    // This prevents the same phrase from matching wrong verbs in future
    for candidate in all_candidates {
        if candidate != selected_verb {
            // Add to blocklist with reason
            // Schema: phrase, blocked_verb, user_id, reason, embedding, embedding_model, expires_at, created_at
            sqlx::query!(
                r#"
                INSERT INTO agent.phrase_blocklist (
                    phrase,
                    blocked_verb,
                    reason,
                    created_at
                )
                VALUES ($1, $2, 'user_disambiguation_rejected', NOW())
                ON CONFLICT (phrase, blocked_verb, COALESCE(user_id, '00000000-0000-0000-0000-000000000000'::uuid)) DO NOTHING
                "#,
                original_input,
                candidate,
            )
            .execute(pool)
            .await?;
        }
    }

    tracing::info!(
        "Recorded disambiguation learning: '{}' → '{}' ({} variants, blocked {} alternatives)",
        original_input,
        selected_verb,
        variants_stored,
        all_candidates.len() - 1
    );

    Ok(())
}

/// Generate phrase variants for more robust learning
///
/// Addresses the failure case where "list all cbus" was learned
/// but "show me the cbus" wasn't recognized.
///
/// One disambiguation teaches multiple phrasings:
/// - "list all cbus" → cbu.list (0.95 confidence)
/// - "list cbu"      → cbu.list (0.85 confidence)  // generated
/// - "show all cbus" → cbu.list (0.85 confidence)  // generated
/// - "show cbus"     → cbu.list (0.85 confidence)  // generated
fn generate_phrase_variants(phrase: &str) -> Vec<String> {
    // MAX 5 VARIANTS (prevent pollution per TODO spec)
    const MAX_VARIANTS: usize = 5;
    // MIN 2 tokens (quality filter per TODO spec)
    const MIN_TOKENS: usize = 2;

    let mut variants = vec![phrase.to_string()];
    let lower = phrase.to_lowercase();

    // Plural normalization (cbus -> cbu, entities -> entity)
    if lower.contains("cbus") {
        variants.push(lower.replace("cbus", "cbu"));
    }
    if lower.contains("entities") {
        variants.push(lower.replace("entities", "entity"));
    }

    // Common verb swaps
    let verb_swaps = [
        ("list", "show"),
        ("show", "list"),
        ("display", "show"),
        ("get", "list"),
        ("view", "show"),
        ("find", "search"),
        ("search", "find"),
    ];
    for (from, to) in verb_swaps {
        if lower.starts_with(from) || lower.contains(&format!(" {}", from)) {
            let swapped = lower.replace(from, to);
            if !variants.contains(&swapped) {
                variants.push(swapped);
            }
        }
    }

    // Article/quantifier removal
    let stripped = lower
        .replace(" the ", " ")
        .replace(" all ", " ")
        .replace(" my ", " ")
        .replace("  ", " ")
        .trim()
        .to_string();
    if stripped != lower && !variants.contains(&stripped) {
        variants.push(stripped);
    }

    // Also try with articles removed at start
    let prefixes_to_strip = ["show me ", "list all ", "get all ", "display all "];
    for prefix in prefixes_to_strip {
        if lower.starts_with(prefix) {
            let without_prefix = lower.strip_prefix(prefix).unwrap_or(&lower).to_string();
            if !without_prefix.is_empty() && !variants.contains(&without_prefix) {
                variants.push(without_prefix);
            }
        }
    }

    // Dedupe and sort
    variants.sort();
    variants.dedup();

    // Quality filter: Min 2 tokens, not generic alone
    let filtered: Vec<String> = variants
        .into_iter()
        .filter(|v| {
            let tokens: Vec<&str> = v.split_whitespace().collect();
            // Must have at least MIN_TOKENS words
            if tokens.len() < MIN_TOKENS {
                return false;
            }
            // Not just generic stopwords
            let generic_only = tokens.iter().all(|t| {
                matches!(
                    *t,
                    "the"
                        | "a"
                        | "an"
                        | "all"
                        | "my"
                        | "this"
                        | "that"
                        | "please"
                        | "can"
                        | "you"
                        | "i"
                        | "me"
                        | "show"
                        | "list"
                        | "get"
                )
            });
            !generic_only
        })
        .collect();

    // Apply MAX_VARIANTS limit - always include original if it passed filter
    let result: Vec<String> = filtered.into_iter().take(MAX_VARIANTS).collect();

    // If original passed filter, ensure it's first
    if result.contains(&phrase.to_string()) {
        let mut final_result = vec![phrase.to_string()];
        for v in result {
            if v != phrase && final_result.len() < MAX_VARIANTS {
                final_result.push(v);
            }
        }
        final_result
    } else if result.is_empty() {
        // Fallback: return original even if short
        vec![phrase.to_string()]
    } else {
        result
    }
}

// ============================================================================
// Disambiguation Abandonment (user bailed - negative signal for ALL candidates)
// ============================================================================

/// POST /api/session/:id/abandon-disambiguation
///
/// Called when user abandons disambiguation without selecting any option.
/// This is a NEGATIVE signal for ALL candidates - they were all wrong.
///
/// Triggers:
/// - User types new input instead of clicking an option
/// - User closes session or navigates away
/// - Timeout (>30s with no interaction)
async fn abandon_disambiguation(
    State(state): State<AgentState>,
    Path(session_id): Path<Uuid>,
    Json(req): Json<ob_poc_types::AbandonDisambiguationRequest>,
) -> Result<Json<ob_poc_types::AbandonDisambiguationResponse>, StatusCode> {
    tracing::info!(
        session_id = %session_id,
        original_input = %req.original_input,
        num_candidates = req.candidates.len(),
        reason = ?req.abandon_reason,
        "Recording disambiguation abandonment (all candidates rejected)"
    );

    let mut signals_recorded = 0;

    // Record negative signals for ALL candidates - they were all wrong
    for candidate in &req.candidates {
        match record_abandon_negative_signal(
            &state.pool,
            &req.original_input,
            candidate,
            &req.abandon_reason,
        )
        .await
        {
            Ok(_) => signals_recorded += 1,
            Err(e) => {
                tracing::warn!(
                    "Failed to record abandon signal for '{}' → '{}': {}",
                    req.original_input,
                    candidate,
                    e
                );
            }
        }
    }

    // Also record to feedback log for analysis
    if let Err(e) = record_abandon_event(&state.pool, &req).await {
        tracing::warn!("Failed to record abandon event: {}", e);
    }

    tracing::info!(
        "Recorded {} negative signals for abandoned disambiguation",
        signals_recorded
    );

    Ok(Json(ob_poc_types::AbandonDisambiguationResponse {
        recorded: signals_recorded > 0,
        signals_recorded,
    }))
}

/// Record a negative signal when user abandons disambiguation
///
/// Uses lower confidence (0.3) than explicit rejection (0.7) because:
/// - User didn't explicitly say "this is wrong"
/// - They just gave up - could be for other reasons
/// - But still valuable signal that these options weren't helpful
async fn record_abandon_negative_signal(
    pool: &PgPool,
    original_input: &str,
    rejected_verb: &str,
    reason: &Option<ob_poc_types::AbandonReason>,
) -> Result<(), sqlx::Error> {
    let reason_str = match reason {
        Some(ob_poc_types::AbandonReason::TypedNewInput) => "abandon_typed_new",
        Some(ob_poc_types::AbandonReason::ClosedSession) => "abandon_closed",
        Some(ob_poc_types::AbandonReason::Timeout) => "abandon_timeout",
        Some(ob_poc_types::AbandonReason::Cancelled) => "abandon_cancelled",
        Some(ob_poc_types::AbandonReason::Other) | None => "abandon_other",
    };

    // Add to phrase_blocklist with lower-weight reason
    // ON CONFLICT: accumulate evidence (increment a counter or update timestamp)
    // Schema: phrase, blocked_verb, user_id, reason, embedding, embedding_model, expires_at, created_at
    sqlx::query!(
        r#"
        INSERT INTO agent.phrase_blocklist (
            phrase,
            blocked_verb,
            reason,
            created_at
        )
        VALUES ($1, $2, $3, NOW())
        ON CONFLICT (phrase, blocked_verb, COALESCE(user_id, '00000000-0000-0000-0000-000000000000'::uuid))
        DO UPDATE SET
            reason = EXCLUDED.reason,
            created_at = NOW()
        "#,
        original_input,
        rejected_verb,
        reason_str,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Record abandon event to feedback log for analysis
async fn record_abandon_event(
    pool: &PgPool,
    req: &ob_poc_types::AbandonDisambiguationRequest,
) -> Result<(), sqlx::Error> {
    let reason_str = req
        .abandon_reason
        .as_ref()
        .map(|r| format!("{:?}", r))
        .unwrap_or_else(|| "unknown".to_string());

    let candidates_json = serde_json::to_value(&req.candidates).unwrap_or_default();

    // Record to intent_feedback for analysis
    // Schema: session_id, user_input, user_input_hash, matched_verb, outcome, alternatives, created_at
    sqlx::query!(
        r#"
        INSERT INTO "ob-poc".intent_feedback (
            session_id,
            user_input,
            user_input_hash,
            matched_verb,
            outcome,
            alternatives,
            created_at
        )
        VALUES (
            '00000000-0000-0000-0000-000000000000'::uuid,
            $1,
            md5($1),
            NULL,
            'abandoned',
            $2,
            NOW()
        )
        "#,
        req.original_input,
        serde_json::json!({
            "request_id": req.request_id,
            "candidates": candidates_json,
            "abandon_reason": reason_str,
        }),
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Generate DSL for a user-selected verb
///
/// This bypasses verb search (we already know the verb) and goes straight
/// to argument extraction and DSL building.
async fn generate_dsl_for_selected_verb(
    _pool: &PgPool,
    original_input: &str,
    selected_verb: &str,
    _session: &mut UnifiedSession,
) -> Result<String, String> {
    use crate::dsl_v2::verb_registry::registry;

    // Parse verb into domain.name format
    let parts: Vec<&str> = selected_verb.splitn(2, '.').collect();
    if parts.len() != 2 {
        return Err(format!("Invalid verb format: {}", selected_verb));
    }
    let (domain, verb_name) = (parts[0], parts[1]);

    // Get verb definition from registry
    let reg = registry();
    let verb_def = reg
        .get_runtime_verb(domain, verb_name)
        .ok_or_else(|| format!("Unknown verb: {}", selected_verb))?;

    // For verbs with no required args, just generate the basic call
    let required_args: Vec<_> = verb_def.args.iter().filter(|a| a.required).collect();

    if required_args.is_empty() {
        // No required args - generate simple verb call
        return Ok(format!("({})", selected_verb));
    }

    // For verbs with required args, we need to extract them from the input
    // This is a simplified version - full implementation would use LLM
    // For now, generate a template with placeholders
    let arg_placeholders: Vec<String> = required_args
        .iter()
        .map(|a| format!(":{} <{}>", a.name, a.name))
        .collect();

    // Try to extract simple values from input
    // This handles cases like "list all cbus" → (cbu.list) with no args needed
    // Or "create fund Alpha" → (cbu.create :name "Alpha")
    let dsl = if arg_placeholders.is_empty() {
        format!("({})", selected_verb)
    } else {
        // Check if we can extract any values from the input
        let extracted_args = extract_simple_args(original_input, &required_args);
        if extracted_args.is_empty() {
            // Return template with placeholders for user to fill
            format!("({} {})", selected_verb, arg_placeholders.join(" "))
        } else {
            format!("({} {})", selected_verb, extracted_args.join(" "))
        }
    };

    Ok(dsl)
}

/// Extract simple argument values from user input
fn extract_simple_args(
    input: &str,
    required_args: &[&crate::dsl_v2::runtime_registry::RuntimeArg],
) -> Vec<String> {
    use dsl_core::ArgType;

    let mut args = Vec::new();
    let words: Vec<&str> = input.split_whitespace().collect();

    for arg in required_args {
        match arg.arg_type {
            ArgType::String => {
                // Look for quoted strings or significant words
                // Skip common words like "create", "add", "list", "show", etc.
                let skip_words = [
                    "create", "add", "list", "show", "get", "find", "search", "all", "the", "a",
                    "an", "for", "to", "from", "with", "in", "on", "by",
                ];
                for word in &words {
                    let lower = word.to_lowercase();
                    if !skip_words.contains(&lower.as_str()) && word.len() > 2 {
                        // Found a potential value
                        args.push(format!(":{} \"{}\"", arg.name, word));
                        break;
                    }
                }
            }
            _ => {
                // For other types, leave as placeholder
            }
        }
    }

    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_dsl() {
        let valid_dsl = r#"(cbu.create :name "Test Fund" :jurisdiction "LU")"#;
        let result = parse_program(valid_dsl);
        assert!(result.is_ok());
    }

    #[test]
    fn test_domain_list() {
        let reg = registry();
        let domains = reg.domains();
        assert!(!domains.is_empty());
        assert!(domains.iter().any(|d| d == "cbu"));
        assert!(domains.iter().any(|d| d == "entity"));
    }
}
