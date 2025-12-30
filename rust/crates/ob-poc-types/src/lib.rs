//! Shared API Types for OB-POC
//!
//! This crate is the SINGLE SOURCE OF TRUTH for all types crossing HTTP boundaries.
//!
//! ## Boundaries
//!
//! ```text
//! ┌──────────────────┐         ┌──────────────────┐
//! │  Rust Server     │  JSON   │  WASM UI         │
//! │  (Axum)          │ ◄─────► │  (egui)          │
//! └──────────────────┘         └──────────────────┘
//! ```
//!
//! ## Rules
//!
//! 1. All API types live here - no inline struct definitions in handlers
//! 2. Tagged enums only: `#[serde(tag = "type")]`
//! 3. UUIDs as strings for JSON compatibility

pub mod resolution;
pub mod semantic_stage;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Re-export resolution types for convenience
pub use resolution::*;

// ============================================================================
// SESSION API
// ============================================================================

/// Request to create a new session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionRequest {
    #[serde(default)]
    pub domain_hint: Option<String>,
}

/// Response after creating a session
/// NOTE: Accepts flexible types to handle server's native types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionResponse {
    /// Session ID - server sends UUID, we accept any string-serializable value
    #[serde(deserialize_with = "deserialize_uuid_or_string")]
    pub session_id: String,
    /// State - server sends enum, we accept anything
    #[serde(default)]
    pub state: serde_json::Value,
    /// Created at - server sends DateTime, we accept any
    #[serde(default)]
    pub created_at: serde_json::Value,
}

/// Helper to deserialize UUID or String into String
fn deserialize_uuid_or_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;

    struct UuidOrStringVisitor;

    impl<'de> de::Visitor<'de> for UuidOrStringVisitor {
        type Value = String;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a UUID or string")
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            Ok(v.to_string())
        }

        fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
            Ok(v)
        }
    }

    deserializer.deserialize_any(UuidOrStringVisitor)
}

/// Bound entity info for session state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundEntityInfo {
    pub id: String,          // UUID as string
    pub name: String,        // Display name
    pub entity_type: String, // e.g., "cbu", "entity"
}

/// Session state response
/// NOTE: Accepts flexible types to handle server's native types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStateResponse {
    #[serde(deserialize_with = "deserialize_uuid_or_string")]
    pub session_id: String,
    /// Entity type this session operates on ("cbu", "kyc_case", "onboarding", "bulk", etc.)
    #[serde(default)]
    pub entity_type: String,
    /// Entity ID this session operates on (None if creating new or bulk mode)
    #[serde(default)]
    pub entity_id: Option<String>,
    #[serde(default)]
    pub state: serde_json::Value,
    #[serde(default)]
    pub message_count: usize,
    #[serde(default)]
    pub can_execute: bool,
    #[serde(default)]
    pub dsl_source: Option<String>,
    /// Active CBU for this session (if set via bind)
    #[serde(default)]
    pub active_cbu: Option<BoundEntityInfo>,
    /// Named bindings available in the session (name -> entity info)
    #[serde(default)]
    pub bindings: serde_json::Value,
    // Extra fields from server
    #[serde(default)]
    pub pending_intents: Option<serde_json::Value>,
    #[serde(default)]
    pub assembled_dsl: Option<serde_json::Value>,
    #[serde(default)]
    pub combined_dsl: Option<serde_json::Value>,
    #[serde(default)]
    pub context: Option<serde_json::Value>,
    #[serde(default)]
    pub messages: Option<serde_json::Value>,
}

// ============================================================================
// CHAT API
// ============================================================================

/// Chat request from user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    #[serde(default)]
    pub cbu_id: Option<String>, // UUID as string
}

/// Chat response from agent
/// NOTE: Fields use #[serde(default)] to be flexible with server response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub message: String,
    #[serde(default)]
    pub can_execute: bool,
    #[serde(default)]
    pub dsl_source: Option<String>,
    /// AST - accepts any JSON since server sends different format
    #[serde(default)]
    pub ast: Option<serde_json::Value>,
    /// Session state - accepts any JSON (server sends enum, we accept anything)
    #[serde(default)]
    pub session_state: serde_json::Value,
    /// UI commands to execute (show CBU, highlight entity, etc.)
    #[serde(default)]
    pub commands: Option<Vec<AgentCommand>>,
    // Extra fields from server that we ignore but must accept
    #[serde(default)]
    pub intents: Option<serde_json::Value>,
    #[serde(default)]
    pub validation_results: Option<serde_json::Value>,
    #[serde(default)]
    pub assembled_dsl: Option<serde_json::Value>,
    #[serde(default)]
    pub bindings: Option<serde_json::Value>,
}

/// SSE stream event - tagged enum for discrimination
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatStreamEvent {
    /// Text chunk from agent
    Chunk { content: String },
    /// DSL source generated
    Dsl { source: String },
    /// AST parsed from DSL
    Ast { statements: Vec<AstStatement> },
    /// Agent command (show CBU, highlight entity, etc.)
    Command { action: AgentCommand },
    /// Stream complete
    Done { session_id: String },
    /// Error occurred
    Error { message: String },
}

/// Commands the agent can issue to the UI
/// This is the canonical vocabulary for agent → UI communication.
/// The LLM maps natural language ("run it", "undo that") to these commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AgentCommand {
    // =========================================================================
    // REPL Commands
    // =========================================================================
    /// Execute accumulated DSL ("execute", "run it", "do it", "go")
    Execute,
    /// Undo last DSL block ("undo", "take that back", "never mind")
    Undo,
    /// Clear all accumulated DSL ("clear", "start over", "reset")
    Clear,
    /// Delete specific statement by index ("delete the second one", "remove that")
    Delete { index: u32 },
    /// Delete the last statement ("delete that", "remove the last one")
    DeleteLast,

    // =========================================================================
    // Navigation Commands
    // =========================================================================
    /// Show a specific CBU in the graph ("show me X fund")
    ShowCbu { cbu_id: String },
    /// Highlight an entity in the graph
    HighlightEntity { entity_id: String },
    /// Navigate to a line in the DSL panel
    NavigateDsl { line: u32 },
    /// Focus an AST node
    FocusAst { node_id: String },

    // =========================================================================
    // Graph Filtering Commands
    // =========================================================================
    /// Filter graph to show only specific entity types ("show only shells", "filter to people")
    FilterByType {
        /// Entity type codes to show (e.g., ["SHELL", "PERSON"])
        type_codes: Vec<String>,
    },
    /// Highlight entities of a specific type without filtering others
    HighlightType {
        /// Entity type code to highlight (e.g., "SHELL")
        type_code: String,
    },
    /// Clear all graph filters and highlights
    ClearFilter,
    /// Set the view mode ("show kyc view", "switch to custody")
    SetViewMode {
        /// View mode: "KYC_UBO", "SERVICE_DELIVERY", "CUSTODY"
        view_mode: String,
    },
}

// ============================================================================
// EXECUTE API
// ============================================================================

/// Request to execute DSL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteRequest {
    #[serde(default)]
    pub dsl: Option<String>,
}

/// Response from DSL execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteResponse {
    pub success: bool,
    pub results: Vec<ExecuteResult>,
    pub errors: Vec<String>,
    /// New session state after execution (accept any JSON)
    #[serde(default)]
    pub new_state: serde_json::Value,
    #[serde(default)]
    pub bindings: Option<std::collections::HashMap<String, String>>, // name -> UUID (as string)
}

/// Individual statement execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteResult {
    pub statement_index: usize,
    #[serde(default)]
    pub dsl: Option<String>,
    pub success: bool,
    pub message: String,
    #[serde(default)]
    pub entity_id: Option<String>,
    #[serde(default)]
    pub entity_type: Option<String>,
    /// Query result data (for cbu.show, cbu.list, etc.)
    #[serde(default)]
    pub result: Option<serde_json::Value>,
}

// ============================================================================
// CBU API
// ============================================================================

/// CBU summary for list views
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuSummary {
    pub cbu_id: String,
    pub name: String,
    #[serde(default)]
    pub jurisdiction: Option<String>,
    #[serde(default)]
    pub client_type: Option<String>,
}

// ============================================================================
// GRAPH API
// ============================================================================

/// Full CBU graph for visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuGraphResponse {
    pub cbu_id: String,
    pub label: String,
    #[serde(default)]
    pub cbu_category: Option<String>,
    #[serde(default)]
    pub jurisdiction: Option<String>,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// Node in the CBU graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub node_type: String,
    pub layer: String,
    pub label: String,
    #[serde(default)]
    pub sublabel: Option<String>,
    pub status: String,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub role_categories: Vec<String>,
    #[serde(default)]
    pub primary_role: Option<String>,
    #[serde(default)]
    pub jurisdiction: Option<String>,
    #[serde(default)]
    pub ownership_pct: Option<f64>,
    /// Role priority for layout ordering
    #[serde(default)]
    pub role_priority: Option<i32>,
    /// Additional node data (JSON blob)
    #[serde(default)]
    pub data: Option<serde_json::Value>,
    /// Server-computed X position
    #[serde(default)]
    pub x: Option<f64>,
    /// Server-computed Y position
    #[serde(default)]
    pub y: Option<f64>,

    // =========================================================================
    // VISUAL HINTS - computed by server, used by renderer
    // =========================================================================
    /// Node importance score (0.0 - 1.0) - affects rendered size
    /// CBU = 1.0, direct children = 0.8, deeper = decreasing
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub importance: Option<f32>,

    /// Depth in ownership hierarchy (0 = root CBU, 1 = direct, 2+ = chain)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hierarchy_depth: Option<i32>,

    /// KYC completion percentage (0-100) - affects fill pattern
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kyc_completion: Option<i32>,

    /// Verification status summary for this entity's relationships
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_summary: Option<VerificationSummary>,

    /// Whether this node needs attention (has issues/gaps)
    #[serde(default)]
    pub needs_attention: bool,

    /// Entity category: PERSON or SHELL
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_category: Option<String>,

    // =========================================================================
    // CONTAINER FIELDS - for nodes that contain browseable children
    // =========================================================================
    /// Whether this node is a container (can be double-clicked to browse)
    #[serde(default)]
    pub is_container: bool,

    /// Type of items this container holds (e.g., "investor_holding", "resource_instance")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contains_type: Option<String>,

    /// Number of child items (for badge display)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_count: Option<i64>,

    /// EntityGateway nickname for searching children
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browse_nickname: Option<String>,

    /// Parent key for scoped queries (e.g., cbu_id)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_key: Option<String>,
}

/// Verification status summary for entity relationships
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VerificationSummary {
    pub total_edges: i32,
    pub proven_edges: i32,
    pub alleged_edges: i32,
    pub disputed_edges: i32,
}

/// Edge in the CBU graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub edge_type: String,
    #[serde(default)]
    pub label: Option<String>,

    // =========================================================================
    // VISUAL HINTS
    // =========================================================================
    /// Ownership percentage (0-100) - affects edge thickness
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<f32>,

    /// Verification status - affects line style
    /// Values: "proven", "alleged", "disputed", "pending"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_status: Option<String>,
}

// ============================================================================
// DSL API
// ============================================================================

/// DSL source response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslResponse {
    pub source: String,
    #[serde(default)]
    pub session_id: Option<String>,
}

// ============================================================================
// AST API
// ============================================================================

/// AST response containing all statements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AstResponse {
    pub statements: Vec<AstStatement>,
}

/// A single AST statement (VerbCall or Comment)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AstStatement {
    VerbCall {
        domain: String,
        verb: String,
        arguments: Vec<AstArgument>,
        #[serde(default)]
        binding: Option<String>,
        #[serde(default)]
        span: Option<AstSpan>,
    },
    Comment {
        text: String,
        #[serde(default)]
        span: Option<AstSpan>,
    },
}

/// AST argument (key-value pair)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AstArgument {
    pub key: String,
    pub value: AstValue,
    #[serde(default)]
    pub span: Option<AstSpan>,
}

/// AST value types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AstValue {
    /// String literal
    String { value: String },
    /// Number literal
    Number { value: f64 },
    /// Boolean literal
    Boolean { value: bool },
    /// Symbol reference (@name)
    SymbolRef { name: String },
    /// Entity reference (type, search_key, resolved_key)
    EntityRef {
        entity_type: String,
        search_key: String,
        #[serde(default)]
        resolved_key: Option<String>,
    },
    /// List of values
    List { items: Vec<AstValue> },
    /// Map of key-value pairs
    Map { entries: Vec<AstMapEntry> },
    /// Null value
    Null,
}

/// Map entry for AST Map values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AstMapEntry {
    pub key: String,
    pub value: AstValue,
}

/// Source location span
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AstSpan {
    pub start: usize,
    pub end: usize,
    #[serde(default)]
    pub start_line: Option<u32>,
    #[serde(default)]
    pub end_line: Option<u32>,
}

// ============================================================================
// EVENT PAYLOADS
// Keep these dead simple - just IDs
// ============================================================================

/// Event: load a CBU
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadCbuEvent {
    pub cbu_id: String,
}

/// Event: focus an entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusEntityEvent {
    pub entity_id: String,
}

/// Event: change view mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetViewModeEvent {
    pub view_mode: String, // "KYC_UBO", "SERVICE_DELIVERY", etc.
}

/// Event: entity selected
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySelectedEvent {
    pub entity_id: String,
}

/// Event: CBU changed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuChangedEvent {
    pub cbu_id: String,
}

// ============================================================================
// DISAMBIGUATION API
// ============================================================================

/// Disambiguation request - sent when user input is ambiguous
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisambiguationRequest {
    /// Unique ID for this disambiguation request
    pub request_id: String,
    /// The ambiguous items that need resolution
    pub items: Vec<DisambiguationItem>,
    /// Human-readable prompt for the user
    pub prompt: String,
}

/// A single ambiguous item needing resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DisambiguationItem {
    /// Multiple entities match a search term
    EntityMatch {
        /// Parameter name (e.g., "entity-id")
        param: String,
        /// Original search text (e.g., "John Smith")
        search_text: String,
        /// Matching entities to choose from
        matches: Vec<EntityMatch>,
    },
    /// Ambiguous interpretation (e.g., "UK" = name part or jurisdiction?)
    InterpretationChoice {
        /// The ambiguous text
        text: String,
        /// Possible interpretations
        options: Vec<Interpretation>,
    },
}

/// A matching entity for disambiguation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityMatch {
    /// Entity UUID
    pub entity_id: String,
    /// Display name
    pub name: String,
    /// Entity type (e.g., "proper_person", "limited_company")
    pub entity_type: String,
    /// Jurisdiction code
    #[serde(default)]
    pub jurisdiction: Option<String>,
    /// Additional context (roles, etc.)
    #[serde(default)]
    pub context: Option<String>,
    /// Match score (0.0 - 1.0)
    #[serde(default)]
    pub score: Option<f64>,
}

/// A possible interpretation of ambiguous text
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interpretation {
    /// Interpretation ID
    pub id: String,
    /// Human-readable label
    pub label: String,
    /// What this interpretation means
    pub description: String,
    /// How this affects the generated DSL
    #[serde(default)]
    pub effect: Option<String>,
}

/// User's disambiguation response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisambiguationResponse {
    /// The request ID being responded to
    pub request_id: String,
    /// Selected resolutions
    pub selections: Vec<DisambiguationSelection>,
}

/// A single disambiguation selection
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DisambiguationSelection {
    /// Selected entity for an EntityMatch
    Entity { param: String, entity_id: String },
    /// Selected interpretation for an InterpretationChoice
    Interpretation {
        text: String,
        interpretation_id: String,
    },
}

/// Extended chat response that can include disambiguation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponseV2 {
    /// Agent message
    pub message: String,
    /// Response type
    #[serde(flatten)]
    pub payload: ChatPayload,
    /// Session state
    pub session_state: String,
}

/// Chat response payload - either ready DSL or needs disambiguation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ChatPayload {
    /// DSL is ready (no ambiguity or already resolved)
    Ready {
        dsl_source: String,
        #[serde(default)]
        ast: Option<Vec<AstStatement>>,
        can_execute: bool,
        #[serde(default)]
        commands: Option<Vec<AgentCommand>>,
    },
    /// Needs user disambiguation before generating DSL
    NeedsDisambiguation {
        disambiguation: DisambiguationRequest,
    },
    /// Just a message, no DSL
    Message {
        #[serde(default)]
        commands: Option<Vec<AgentCommand>>,
    },
}

// ============================================================================
// ENTITY SEARCH API (for resolution popups in egui)
// ============================================================================

/// A single match from entity search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySearchMatch {
    /// Primary key (UUID or code)
    pub value: String,
    /// Human-readable label
    pub display: String,
    /// Additional context
    #[serde(default)]
    pub detail: Option<String>,
    /// Relevance score
    pub score: f32,
}

/// Response from entity search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySearchResponse {
    pub matches: Vec<EntitySearchMatch>,
    pub total: usize,
    pub truncated: bool,
}

// ============================================================================
// BIND ENTITY API (for binding CBU to session)
// ============================================================================

/// Request to set a binding in a session (matches agent_routes.rs SetBindingRequest)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetBindingRequest {
    /// Binding name (without @)
    pub name: String,
    /// UUID to bind (as string)
    pub id: String,
    /// Entity type (e.g., "cbu", "entity", "case")
    pub entity_type: String,
    /// Human-readable display name
    pub display_name: String,
}

/// Response from setting a binding (matches agent_routes.rs SetBindingResponse)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetBindingResponse {
    pub success: bool,
    pub binding_name: String,
    pub bindings: std::collections::HashMap<String, String>,
}

// ============================================================================
// VALIDATION API (for DSL validation in egui)
// ============================================================================

/// Request to validate DSL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateDslRequest {
    pub dsl: String,
}

/// Validation error with location info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    #[serde(default)]
    pub line: Option<usize>,
    #[serde(default)]
    pub column: Option<usize>,
    pub message: String,
    #[serde(default)]
    pub suggestion: Option<String>,
}

/// Response from /api/agent/validate (matches server's ValidationResult)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateDslResponse {
    pub valid: bool,
    #[serde(default)]
    pub errors: Vec<ValidationError>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

// ============================================================================
// DSL DISPLAY SEGMENTS (for rich rendering with inline binding info)
// ============================================================================

/// A segment of DSL text for rich rendering
/// The UI receives pre-segmented DSL and renders each segment appropriately
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DslDisplaySegment {
    /// Plain text (keywords, punctuation, literals)
    Text { content: String },

    /// A binding reference (@symbol) with resolved info
    Binding {
        /// The symbol name (without @)
        symbol: String,
        /// Resolved display name (e.g., "Apex Capital")
        display_name: Option<String>,
        /// Entity type (e.g., "cbu", "proper_person")
        entity_type: Option<String>,
        /// Resolved UUID (if resolved)
        entity_id: Option<String>,
        /// Whether this binding is editable/clickable
        editable: bool,
        /// Byte offset in source for click handling
        source_offset: usize,
    },

    /// An unresolved entity reference that needs resolution
    UnresolvedRef {
        /// The search/display value from DSL
        search_value: String,
        /// Expected entity type from verb schema
        entity_type: String,
        /// Argument name (e.g., ":cbu-id")
        arg_name: String,
        /// Reference ID for resolution API
        ref_id: String,
        /// Byte offset in source
        source_offset: usize,
    },

    /// A comment
    Comment { content: String },

    /// Newline/whitespace (preserved for layout)
    Whitespace { content: String },
}

/// Enriched DSL for display - raw source plus segmented view
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichedDsl {
    /// Raw DSL source (for editing mode)
    pub source: String,
    /// Segmented view for rich rendering
    pub segments: Vec<DslDisplaySegment>,
    /// Summary of bindings used
    pub binding_summary: Vec<BindingSummary>,
    /// Whether all references are resolved
    pub fully_resolved: bool,
}

/// Summary of a binding for the context panel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindingSummary {
    /// Symbol name (without @)
    pub symbol: String,
    /// Display name
    pub display_name: String,
    /// Entity type
    pub entity_type: String,
    /// UUID
    pub entity_id: String,
    /// Is this the active/primary binding (e.g., active_cbu)
    pub is_primary: bool,
}

// ============================================================================
// SESSION CONTEXT API (for agent prompt and UI context panel)
// ============================================================================

/// Context surfaced to agent and UI - what the session knows about
/// This is the UI-facing context, distinct from server-side SessionContext
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionContext {
    /// Active CBU context (if a CBU is selected)
    #[serde(default)]
    pub cbu: Option<CbuContext>,
    /// Linked onboarding request (if in onboarding flow)
    #[serde(default)]
    pub onboarding_request: Option<LinkedContext>,
    /// Linked KYC cases
    #[serde(default)]
    pub kyc_cases: Vec<LinkedContext>,
    /// Trading matrix context (if available)
    #[serde(default)]
    pub trading_matrix: Option<LinkedContext>,
    /// ISDA agreements
    #[serde(default)]
    pub isda_agreements: Vec<LinkedContext>,
    /// Product subscriptions
    #[serde(default)]
    pub product_subscriptions: Vec<LinkedContext>,
    /// Current active scope (what the user is "working on")
    #[serde(default)]
    pub active_scope: Option<ActiveScope>,
    /// Symbol table - accumulated bindings from DSL execution
    #[serde(default)]
    pub symbols: std::collections::HashMap<String, SymbolValue>,
    /// Semantic stage state - onboarding journey progress
    /// Derived on-demand from entity tables, NOT stored
    #[serde(default)]
    pub semantic_state: Option<crate::semantic_stage::SemanticState>,
    /// Currently focused stage (for verb filtering)
    /// Set by user clicking on a stage in the UI
    #[serde(default)]
    pub stage_focus: Option<String>,
}

/// CBU-specific context with summary info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuContext {
    /// CBU UUID
    pub id: String,
    /// CBU name
    pub name: String,
    /// Jurisdiction code (e.g., "LU", "US")
    #[serde(default)]
    pub jurisdiction: Option<String>,
    /// Client type (e.g., "FUND", "CORPORATE")
    #[serde(default)]
    pub client_type: Option<String>,
    /// Number of linked entities
    #[serde(default)]
    pub entity_count: i32,
    /// Number of assigned roles
    #[serde(default)]
    pub role_count: i32,
    /// KYC status summary
    #[serde(default)]
    pub kyc_status: Option<String>,
    /// Risk rating
    #[serde(default)]
    pub risk_rating: Option<String>,
}

/// Generic linked context for related entities (cases, agreements, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkedContext {
    /// Entity UUID
    pub id: String,
    /// Context type (e.g., "kyc_case", "isda_agreement", "onboarding_request")
    pub context_type: String,
    /// Display name/label
    pub label: String,
    /// Status (e.g., "ACTIVE", "PENDING", "CLOSED")
    #[serde(default)]
    pub status: Option<String>,
    /// Created date (ISO 8601)
    #[serde(default)]
    pub created_at: Option<String>,
    /// Additional metadata
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

/// Current active scope - what the user is focused on
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ActiveScope {
    /// Working on a CBU
    Cbu { cbu_id: String, cbu_name: String },
    /// Working on a KYC case
    KycCase {
        case_id: String,
        case_type: String,
        cbu_id: String,
    },
    /// Working on an entity within a CBU
    Entity {
        entity_id: String,
        entity_name: String,
        cbu_id: String,
    },
    /// Working on onboarding
    Onboarding {
        request_id: String,
        cbu_id: Option<String>,
    },
    /// Bulk/batch mode
    Bulk { template_id: Option<String> },
}

/// Symbol binding value - what a @symbol resolves to
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolValue {
    /// Resolved UUID
    pub id: String,
    /// Entity type (e.g., "cbu", "proper_person", "limited_company")
    pub entity_type: String,
    /// Display name for UI
    pub display_name: String,
    /// Source of the binding (e.g., "execution", "user_selection", "default")
    #[serde(default)]
    pub source: Option<String>,
}

/// Request to set the active scope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetScopeRequest {
    pub scope: ActiveScope,
}

/// Response after setting scope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetScopeResponse {
    pub success: bool,
    pub context: SessionContext,
}

/// Request to get session context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetContextRequest {
    /// Optional CBU ID to get context for (if not using session's active CBU)
    #[serde(default)]
    pub cbu_id: Option<String>,
}

/// Response with session context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetContextResponse {
    pub context: SessionContext,
}

// ============================================================================
// CONVERSION HELPERS
// ============================================================================

impl CreateSessionResponse {
    pub fn new(session_id: Uuid, state: &str, created_at: DateTime<Utc>) -> Self {
        Self {
            session_id: session_id.to_string(),
            state: serde_json::Value::String(state.to_string()),
            created_at: serde_json::Value::String(created_at.to_rfc3339()),
        }
    }
}

impl CbuSummary {
    pub fn new(
        cbu_id: Uuid,
        name: String,
        jurisdiction: Option<String>,
        client_type: Option<String>,
    ) -> Self {
        Self {
            cbu_id: cbu_id.to_string(),
            name,
            jurisdiction,
            client_type,
        }
    }
}

impl ExecuteResult {
    pub fn success(index: usize, message: &str, entity_id: Option<Uuid>) -> Self {
        Self {
            statement_index: index,
            dsl: None,
            success: true,
            message: message.to_string(),
            entity_id: entity_id.map(|id| id.to_string()),
            entity_type: None,
            result: None,
        }
    }

    pub fn success_with_result(index: usize, message: &str, result: serde_json::Value) -> Self {
        Self {
            statement_index: index,
            dsl: None,
            success: true,
            message: message.to_string(),
            entity_id: None,
            entity_type: None,
            result: Some(result),
        }
    }

    pub fn failure(index: usize, message: &str) -> Self {
        Self {
            statement_index: index,
            dsl: None,
            success: false,
            message: message.to_string(),
            entity_id: None,
            entity_type: None,
            result: None,
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_stream_event_tagged_correctly() {
        let event = ChatStreamEvent::Chunk {
            content: "hello".into(),
        };
        let json = serde_json::to_string(&event).unwrap();

        // Must have "type" field for TS discrimination
        assert!(json.contains(r#""type":"chunk""#));
    }

    #[test]
    fn agent_command_tagged_correctly() {
        let cmd = AgentCommand::ShowCbu {
            cbu_id: "abc-123".into(),
        };
        let json = serde_json::to_string(&cmd).unwrap();

        assert!(json.contains(r#""action":"show_cbu""#));
    }

    #[test]
    fn ast_value_tagged_correctly() {
        let val = AstValue::String {
            value: "test".into(),
        };
        let json = serde_json::to_string(&val).unwrap();

        assert!(json.contains(r#""type":"string""#));
    }

    #[test]
    fn ast_statement_tagged_correctly() {
        let stmt = AstStatement::VerbCall {
            domain: "cbu".into(),
            verb: "ensure".into(),
            arguments: vec![],
            binding: Some("fund".into()),
            span: None,
        };
        let json = serde_json::to_string(&stmt).unwrap();

        assert!(json.contains(r#""type":"verb_call""#));
    }

    #[test]
    fn roundtrip_execute_response() {
        let response = ExecuteResponse {
            success: true,
            results: vec![ExecuteResult::success(0, "OK", None)],
            errors: vec![],
            new_state: serde_json::Value::String("executed".to_string()),
            bindings: None,
        };

        let json = serde_json::to_string(&response).unwrap();
        let parsed: ExecuteResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(response.success, parsed.success);
        assert_eq!(response.results.len(), parsed.results.len());
    }

    #[test]
    fn uuid_as_string() {
        let id = Uuid::new_v4();
        let summary = CbuSummary::new(id, "Test Fund".into(), Some("LU".into()), None);

        let json = serde_json::to_string(&summary).unwrap();

        // UUID should be serialized as string, not object
        assert!(json.contains(&id.to_string()));
        assert!(!json.contains("Uuid"));
    }

    #[test]
    fn agent_command_execute_serializes() {
        let cmd = AgentCommand::Execute;
        let json = serde_json::to_string(&cmd).unwrap();
        println!("AgentCommand::Execute JSON: {}", json);
        assert!(json.contains(r#""action":"execute""#));
    }

    #[test]
    fn optional_commands_with_execute() {
        // Simulate what server sends
        #[derive(serde::Serialize)]
        struct TestResponse {
            message: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            commands: Option<Vec<AgentCommand>>,
        }

        let resp = TestResponse {
            message: "Executing...".to_string(),
            commands: Some(vec![AgentCommand::Execute]),
        };
        let json = serde_json::to_string(&resp).unwrap();
        println!("TestResponse JSON: {}", json);

        // Verify commands field is present and correctly serialized
        assert!(json.contains(r#""commands""#), "commands field missing");
        assert!(
            json.contains(r#""action":"execute""#),
            "execute action missing"
        );
        assert!(
            !json.contains(r#""commands":null"#),
            "commands should not be null"
        );
    }
}
