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

pub mod chat;
pub mod commands;
pub mod control;
pub mod decision;
pub mod disambiguation;
pub mod galaxy;
pub mod investor_register;
pub mod manco_group;
pub mod resolution;
pub mod semantic_stage;
pub mod trading_matrix;
pub mod viewport;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Re-export sub-module types for convenience (backward compatible)
pub use chat::*;
pub use commands::*;
pub use decision::*;
pub use disambiguation::*;
pub use resolution::*;

// ============================================================================
// RESOLVED KEY - UUID vs Code distinction
// ============================================================================

/// Resolved key - either a database UUID or a code string
///
/// This enum prevents the anti-pattern of generating fake UUIDs for reference
/// data like role codes (`DIRECTOR`), jurisdiction codes (`US`), or product
/// codes (`FUND_ACCOUNTING`). These should remain as their natural string keys.
///
/// ## Examples
///
/// ```ignore
/// // Entity with UUID primary key
/// ResolvedKey::Uuid(Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap())
///
/// // Role code (string primary key)
/// ResolvedKey::Code("DIRECTOR".to_string())
///
/// // Jurisdiction code
/// ResolvedKey::Code("LU".to_string())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "key_type", content = "value", rename_all = "snake_case")]
pub enum ResolvedKey {
    /// UUID primary key (entities, persons, funds, CBUs)
    Uuid(Uuid),
    /// Code string (roles, jurisdictions, products, attributes, currencies)
    Code(String),
}

impl ResolvedKey {
    /// Check if this is a UUID key
    pub fn is_uuid(&self) -> bool {
        matches!(self, ResolvedKey::Uuid(_))
    }

    /// Check if this is a code key
    pub fn is_code(&self) -> bool {
        matches!(self, ResolvedKey::Code(_))
    }

    /// Get the UUID if this is a UUID key
    pub fn as_uuid(&self) -> Option<Uuid> {
        match self {
            ResolvedKey::Uuid(u) => Some(*u),
            ResolvedKey::Code(_) => None,
        }
    }

    /// Get the code if this is a code key
    pub fn as_code(&self) -> Option<&str> {
        match self {
            ResolvedKey::Uuid(_) => None,
            ResolvedKey::Code(c) => Some(c),
        }
    }

    /// Parse from string - tries UUID first, falls back to Code
    ///
    /// This is useful when receiving data from external sources where
    /// the key type isn't explicitly tagged.
    pub fn parse(s: &str) -> Self {
        match Uuid::parse_str(s) {
            Ok(u) => ResolvedKey::Uuid(u),
            Err(_) => ResolvedKey::Code(s.to_string()),
        }
    }

    /// Convert to string representation
    ///
    /// For UUIDs, returns the hyphenated string form.
    /// For codes, returns the code as-is.
    pub fn to_key_string(&self) -> String {
        match self {
            ResolvedKey::Uuid(u) => u.to_string(),
            ResolvedKey::Code(c) => c.clone(),
        }
    }
}

impl std::fmt::Display for ResolvedKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolvedKey::Uuid(u) => write!(f, "{}", u),
            ResolvedKey::Code(c) => write!(f, "{}", c),
        }
    }
}

impl From<Uuid> for ResolvedKey {
    fn from(u: Uuid) -> Self {
        ResolvedKey::Uuid(u)
    }
}

impl From<String> for ResolvedKey {
    fn from(s: String) -> Self {
        ResolvedKey::parse(&s)
    }
}

impl From<&str> for ResolvedKey {
    fn from(s: &str) -> Self {
        ResolvedKey::parse(s)
    }
}

// ============================================================================
// REF LOCATION - Location-based reference identification
// ============================================================================

/// Unique identifier for an unresolved reference location in the AST
///
/// This enables location-based resolution rather than text-based matching.
/// When two "John Smith" references appear in the same DSL (one as director,
/// one as UBO), they can be resolved to different people because they're
/// identified by their AST location, not their text content.
///
/// ## Example
///
/// ```ignore
/// // Statement 0: (cbu.assign-role :entity-id "John Smith" :role "DIRECTOR")
/// // Statement 1: (ownership.add-ubo :person "John Smith" :percentage 25)
///
/// // These create two distinct RefLocations:
/// RefLocation { stmt_index: 0, arg_name: "entity-id".to_string(), span: None }
/// RefLocation { stmt_index: 1, arg_name: "person".to_string(), span: None }
/// ```
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct RefLocation {
    /// Statement index in the AST (0-based)
    pub stmt_index: usize,
    /// Argument name within the statement (e.g., "entity-id", "cbu-id")
    pub arg_name: String,
    /// Optional byte span for sub-argument precision (start, end)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<(usize, usize)>,
}

impl RefLocation {
    /// Create a new RefLocation
    pub fn new(stmt_index: usize, arg_name: impl Into<String>) -> Self {
        Self {
            stmt_index,
            arg_name: arg_name.into(),
            span: None,
        }
    }

    /// Create a new RefLocation with span
    pub fn with_span(stmt_index: usize, arg_name: impl Into<String>, span: (usize, usize)) -> Self {
        Self {
            stmt_index,
            arg_name: arg_name.into(),
            span: Some(span),
        }
    }

    /// Generate a unique ref_id string for this location
    pub fn ref_id(&self) -> String {
        format!("{}:{}", self.stmt_index, self.arg_name)
    }
}

impl std::fmt::Display for RefLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "stmt[{}].{}", self.stmt_index, self.arg_name)
    }
}

// Re-export investor register types for convenience
pub use investor_register::{
    AggregateBreakdown, AggregateInvestorsNode, BreakdownDimension, ControlHolderNode, ControlTier,
    InvestorFilters, InvestorListItem, InvestorListResponse, InvestorRegisterView, IssuerSummary,
    PaginationInfo, ThresholdConfig,
};

// Re-export viewport types for convenience
pub use viewport::{
    CameraState, CbuRef, CbuViewMemory, CbuViewType, ConcreteEntityRef, ConcreteEntityType,
    ConfidenceZone, ConfigNodeRef, EnhanceArg, EnhanceLevelInfo, EnhanceOp, Enhanceable,
    FocusManager, FocusMode, InstrumentMatrixRef, InstrumentType, ProductServiceRef,
    ViewportFilters, ViewportFocusState, ViewportState,
};

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

/// Session state response - matches server's SessionStateResponse in session.rs
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

    /// Session state enum
    #[serde(default)]
    pub state: SessionStateEnum,

    /// Message count in conversation
    #[serde(default)]
    pub message_count: usize,

    /// Pending intents awaiting validation (empty vec if none, skipped in JSON if empty)
    #[serde(default)]
    pub pending_intents: Vec<serde_json::Value>,

    /// Assembled DSL statements (empty vec if none, skipped in JSON if empty)
    #[serde(default)]
    pub assembled_dsl: Vec<String>,

    /// Combined DSL (None if no DSL assembled, skipped in JSON if None)
    #[serde(default)]
    pub combined_dsl: Option<String>,

    /// Session context
    #[serde(default)]
    pub context: serde_json::Value,

    /// Conversation history (empty vec if none, skipped in JSON if empty)
    #[serde(default)]
    pub messages: Vec<ChatMessage>,

    /// Whether the session can execute
    #[serde(default)]
    pub can_execute: bool,

    /// Session version (ISO timestamp from server's updated_at)
    /// UI uses this to detect external changes (MCP/REPL modifying session)
    #[serde(default)]
    pub version: Option<String>,

    /// Run sheet - DSL statement ledger with per-statement status
    /// Used by REPL panel to show statement history and status
    #[serde(default)]
    pub run_sheet: Option<RunSheet>,

    /// Symbol bindings in this session (symbol name → bound entity)
    #[serde(default)]
    pub bindings: std::collections::HashMap<String, BoundEntityInfo>,
}

impl SessionStateResponse {
    /// Get combined DSL source (for UI compatibility)
    pub fn dsl_source(&self) -> Option<&str> {
        self.combined_dsl.as_deref()
    }

    /// Check if there's any DSL content
    pub fn has_dsl(&self) -> bool {
        self.combined_dsl
            .as_ref()
            .map(|s| !s.is_empty())
            .unwrap_or(false)
    }

    /// Get active CBU name from context (if set)
    pub fn active_cbu_name(&self) -> Option<String> {
        self.context
            .get("active_cbu")
            .and_then(|cbu| cbu.get("name"))
            .and_then(|n| n.as_str())
            .map(|s| s.to_string())
    }

    /// Get active CBU ID from context (if set)
    pub fn active_cbu_id(&self) -> Option<String> {
        self.context
            .get("active_cbu")
            .and_then(|cbu| cbu.get("id"))
            .and_then(|id| id.as_str())
            .map(|s| s.to_string())
    }

    /// Get bindings from context
    pub fn get_bindings(&self) -> std::collections::HashMap<String, BoundEntityInfo> {
        self.context
            .get("bindings")
            .and_then(|b| serde_json::from_value(b.clone()).ok())
            .unwrap_or_default()
    }
}

/// Session context information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionContextInfo {
    /// Stage focus for verb filtering
    #[serde(default)]
    pub stage_focus: Option<String>,

    /// Domain hint for RAG
    #[serde(default)]
    pub domain_hint: Option<String>,

    /// View mode (KYC_UBO, SERVICE_DELIVERY, etc.)
    #[serde(default)]
    pub view_mode: Option<String>,
}

// ============================================================================
// DSL STATE - Single source of truth for DSL across API boundary
// ============================================================================

/// Consolidated DSL state - the SINGLE source of truth for DSL content.
/// Replaces the previous scattered fields: dsl_source, combined_dsl, assembled_dsl
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DslState {
    /// The canonical DSL source text (always present if there's any DSL)
    #[serde(default)]
    pub source: Option<String>,

    /// Parsed AST statements (typed, not serde_json::Value)
    #[serde(default)]
    pub ast: Option<Vec<AstStatement>>,

    /// Whether this DSL is ready to execute (passed validation)
    #[serde(default)]
    pub can_execute: bool,

    /// Validation status
    #[serde(default)]
    pub validation: Option<DslValidation>,

    /// Intent information from agent (what verbs were extracted)
    #[serde(default)]
    pub intents: Option<Vec<VerbIntentInfo>>,

    /// Symbol bindings created by this DSL
    #[serde(default)]
    pub bindings: std::collections::HashMap<String, BoundEntityInfo>,
}

/// Validation result for DSL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslValidation {
    /// Whether validation passed
    pub valid: bool,
    /// Validation errors (if any)
    #[serde(default)]
    pub errors: Vec<ValidationError>,
    /// Validation warnings (if any)
    #[serde(default)]
    pub warnings: Vec<String>,
}

/// Information about an extracted verb intent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbIntentInfo {
    /// Full verb name (e.g., "cbu.assign-role")
    pub verb: String,
    /// Domain (e.g., "cbu")
    pub domain: String,
    /// Action (e.g., "assign-role")
    pub action: String,
    /// Parameter values (typed)
    #[serde(default)]
    pub params: std::collections::HashMap<String, ParamValue>,
    /// Binding name if `:as @name` specified
    #[serde(default)]
    pub bind_as: Option<String>,
    /// Validation status for this intent
    #[serde(default)]
    pub validation: Option<IntentValidationStatus>,
}

/// Parameter value in a verb intent
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ParamValue {
    /// Literal string value
    String { value: String },
    /// Literal number value
    Number { value: f64 },
    /// Literal boolean value
    Boolean { value: bool },
    /// Symbol reference (@name)
    SymbolRef { symbol: String },
    /// Resolved entity reference
    ResolvedEntity {
        /// Display name for UI
        display_name: String,
        /// Resolved UUID
        resolved_id: String,
        /// Entity type
        entity_type: String,
    },
    /// Unresolved entity lookup (needs resolution)
    UnresolvedLookup {
        /// Search text
        search_text: String,
        /// Expected entity type
        entity_type: String,
    },
}

/// Validation status for an intent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentValidationStatus {
    /// Whether this intent is valid
    pub valid: bool,
    /// Error message if invalid
    #[serde(default)]
    pub error: Option<String>,
    /// Missing required parameters
    #[serde(default)]
    pub missing_params: Vec<String>,
    /// Unresolved entity references
    #[serde(default)]
    pub unresolved_refs: Vec<String>,
}

// ============================================================================
// RUN SHEET - DSL Statement Ledger with per-statement status
// ============================================================================

/// Run sheet - DSL statement ledger for REPL panel display
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunSheet {
    /// Entries in the run sheet (ordered by creation)
    #[serde(default)]
    pub entries: Vec<RunSheetEntry>,
    /// Current cursor position (index of active entry)
    #[serde(default)]
    pub cursor: usize,
}

impl RunSheet {
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get entry count
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Get current entry at cursor
    pub fn current(&self) -> Option<&RunSheetEntry> {
        self.entries.get(self.cursor)
    }

    /// Count entries by status
    pub fn count_by_status(&self, status: RunSheetEntryStatus) -> usize {
        self.entries.iter().filter(|e| e.status == status).count()
    }

    /// Get executed count
    pub fn executed_count(&self) -> usize {
        self.count_by_status(RunSheetEntryStatus::Executed)
    }

    /// Get pending count (draft + ready + executing)
    pub fn pending_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.status.is_pending())
            .count()
    }
}

/// Single entry in the run sheet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSheetEntry {
    /// Unique entry ID
    pub id: String,
    /// DSL source text
    pub dsl_source: String,
    /// Display-friendly DSL (may have comments, formatting)
    #[serde(default)]
    pub display_dsl: Option<String>,
    /// Entry status
    #[serde(default)]
    pub status: RunSheetEntryStatus,
    /// Creation timestamp (ISO 8601)
    #[serde(default)]
    pub created_at: Option<String>,
    /// Execution timestamp (ISO 8601)
    #[serde(default)]
    pub executed_at: Option<String>,
    /// Entity IDs affected by execution
    #[serde(default)]
    pub affected_entities: Vec<String>,
    /// Symbol bindings created by this entry
    #[serde(default)]
    pub bindings: std::collections::HashMap<String, BoundEntityInfo>,
    /// Error message if failed
    #[serde(default)]
    pub error: Option<String>,
}

/// Run sheet entry status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunSheetEntryStatus {
    /// Parsed, awaiting user confirmation
    #[default]
    Draft,
    /// User confirmed, ready to execute
    Ready,
    /// Execution in progress
    Executing,
    /// Successfully executed
    Executed,
    /// User cancelled
    Cancelled,
    /// Execution failed
    Failed,
}

impl RunSheetEntryStatus {
    /// Check if this is a terminal status
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Executed | Self::Cancelled | Self::Failed)
    }

    /// Check if this is a pending status
    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Draft | Self::Ready | Self::Executing)
    }

    /// Get display icon for UI
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Draft => "📝",
            Self::Ready => "✓",
            Self::Executing => "⏳",
            Self::Executed => "✅",
            Self::Cancelled => "⊘",
            Self::Failed => "❌",
        }
    }

    /// Get display color (as RGB tuple)
    pub fn color_rgb(&self) -> (u8, u8, u8) {
        match self {
            Self::Draft => (148, 163, 184),     // slate-400
            Self::Ready => (34, 197, 94),       // green-500
            Self::Executing => (250, 204, 21),  // yellow-400
            Self::Executed => (34, 197, 94),    // green-500
            Self::Cancelled => (148, 163, 184), // slate-400
            Self::Failed => (239, 68, 68),      // red-500
        }
    }
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
    #[serde(default)]
    pub cbu_category: Option<String>,
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

/// Multi-CBU scope graph response
/// Contains combined graph for all CBUs in session scope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeGraphResponse {
    /// Combined graph containing all CBUs
    pub graph: Option<CbuGraphResponse>,
    /// All CBU IDs included in the graph
    #[serde(default)]
    pub cbu_ids: Vec<String>,
    /// Count of CBUs in scope
    #[serde(default)]
    pub cbu_count: usize,
    /// Entity IDs that were recently affected (for highlighting)
    #[serde(default)]
    pub affected_entity_ids: Vec<String>,
    /// Error message if graph couldn't be loaded
    pub error: Option<String>,
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

    /// Person state: GHOST, IDENTIFIED, or VERIFIED
    /// Ghost entities have minimal info (name only) and render with dashed/faded style
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub person_state: Option<String>,

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

    /// ID of the container node this node belongs to (for visual grouping)
    /// Entities inside a CBU have container_parent_id set to the CBU ID
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container_parent_id: Option<String>,
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
    /// Viewport state from DSL viewport.* verbs (focus, enhance, filter, camera)
    /// This drives the graph widget's view state
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewport_state: Option<ViewportState>,
    /// Agent state for research workflow automation
    /// Shows current agent mode, status, and any pending checkpoints
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_state: Option<AgentStateView>,
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
    /// CBU category (e.g., "SICAV", "SEGREGATED", "POOLED")
    #[serde(default)]
    pub cbu_category: Option<String>,
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
        cbu_category: Option<String>,
    ) -> Self {
        Self {
            cbu_id: cbu_id.to_string(),
            name,
            jurisdiction,
            client_type,
            cbu_category,
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
// AGENT STATE (for research workflow UI)
// ============================================================================

/// Agent mode - how the session operates
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentMode {
    /// User drives all operations manually
    #[default]
    Manual,
    /// Agent runs autonomously, auto-selects high-confidence matches
    Agent,
    /// Agent proposes, user confirms at checkpoints
    Hybrid,
}

/// Agent task type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTaskType {
    /// Fill ownership gaps in CBU structure
    ResolveGaps,
    /// Research full ownership chain to natural persons
    ChainResearch,
    /// Enrich a single entity with external data
    EnrichEntity,
    /// Enrich all entities in a CBU/group
    EnrichGroup,
    /// Screen entities for sanctions/PEP/adverse media
    ScreenEntities,
}

/// Agent execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    /// Not running
    #[default]
    Idle,
    /// Running and processing
    Running,
    /// Paused by user
    Paused,
    /// Waiting for user confirmation at checkpoint
    Checkpoint,
    /// Completed successfully
    Complete,
    /// Failed with error
    Failed,
    /// Cancelled by user
    Cancelled,
}

/// A candidate match for user selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCandidate {
    /// Candidate index
    pub index: i32,
    /// External key (LEI, company number, etc.)
    pub key: String,
    /// Key type (LEI, COMPANY_NUMBER, etc.)
    pub key_type: String,
    /// Entity name
    pub name: String,
    /// Jurisdiction
    #[serde(default)]
    pub jurisdiction: Option<String>,
    /// Match confidence (0.0 - 1.0)
    pub confidence: f32,
    /// Whether this was auto-selected
    #[serde(default)]
    pub auto_selected: bool,
    /// Additional metadata
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

/// A checkpoint requiring user confirmation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCheckpoint {
    /// Checkpoint ID
    pub checkpoint_id: String,
    /// Checkpoint type
    pub checkpoint_type: CheckpointType,
    /// Entity being researched
    pub target_entity_id: String,
    /// Search query that produced candidates
    pub search_query: String,
    /// Source provider (gleif, companies_house, etc.)
    pub source_provider: String,
    /// Candidate matches to choose from
    pub candidates: Vec<AgentCandidate>,
    /// Reason for checkpoint (why not auto-selected)
    pub reason: String,
    /// Created timestamp
    pub created_at: String,
}

/// Type of checkpoint
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointType {
    /// Need to select from ambiguous matches
    Disambiguation,
    /// Confirm before import
    ConfirmImport,
    /// Verify chain continuation
    ChainVerification,
    /// Confirm screening results
    ScreeningReview,
}

/// Agent state for UI display
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentStateView {
    /// Current mode
    pub mode: AgentMode,
    /// Current status
    pub status: AgentStatus,
    /// Agent session ID (if running)
    #[serde(default)]
    pub agent_session_id: Option<String>,
    /// Current task type
    #[serde(default)]
    pub task: Option<AgentTaskType>,
    /// Target entity being researched
    #[serde(default)]
    pub target_entity_id: Option<String>,
    /// Current loop iteration
    #[serde(default)]
    pub loop_iteration: u32,
    /// Maximum iterations allowed
    #[serde(default)]
    pub max_iterations: u32,
    /// Pending checkpoint (if status == Checkpoint)
    #[serde(default)]
    pub pending_checkpoint: Option<AgentCheckpoint>,
    /// Number of decisions made this session
    #[serde(default)]
    pub decisions_made: u32,
    /// Number of actions taken this session
    #[serde(default)]
    pub actions_taken: u32,
    /// Last error message (if status == Failed)
    #[serde(default)]
    pub error_message: Option<String>,
    /// Progress message for UI
    #[serde(default)]
    pub progress_message: Option<String>,
}

/// Agent event for SSE streaming to UI
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum AgentStreamEvent {
    /// Agent started
    Started {
        agent_session_id: String,
        task: AgentTaskType,
        target_entity_id: Option<String>,
    },
    /// Executing DSL
    Executing { dsl: String, iteration: u32 },
    /// Checkpoint created - needs user input
    Checkpoint { checkpoint: AgentCheckpoint },
    /// Progress update
    Progress {
        message: String,
        iteration: u32,
        decisions_made: u32,
        actions_taken: u32,
    },
    /// Agent completed
    Completed {
        decisions_made: u32,
        actions_taken: u32,
        entities_created: u32,
    },
    /// Agent paused
    Paused { iteration: u32 },
    /// Agent resumed
    Resumed { iteration: u32 },
    /// Agent failed
    Failed { error: String, iteration: u32 },
    /// Agent cancelled
    Cancelled { iteration: u32 },
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
    fn agent_command_esper_zoom() {
        // "enhance" -> ZoomIn
        let cmd = AgentCommand::ZoomIn { factor: Some(1.5) };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains(r#""action":"zoom_in""#));
        assert!(json.contains(r#""factor":1.5"#));
    }

    #[test]
    fn agent_command_esper_pan() {
        // "track 45 left" -> Pan
        let cmd = AgentCommand::Pan {
            direction: PanDirection::Left,
            amount: Some(45.0),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains(r#""action":"pan""#));
        assert!(json.contains(r#""direction":"left""#));
    }

    #[test]
    fn agent_command_esper_stop() {
        // "stop", "hold", "freeze"
        let cmd = AgentCommand::Stop;
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains(r#""action":"stop""#));
    }

    #[test]
    fn agent_command_hard_copy() {
        // "give me a hard copy"
        let cmd = AgentCommand::Export {
            format: Some("png".into()),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains(r#""action":"export""#));
        assert!(json.contains(r#""format":"png""#));
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
        let summary = CbuSummary::new(id, "Test Fund".into(), Some("LU".into()), None, None);

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

    // =========================================================================
    // ResolvedKey Tests
    // =========================================================================

    #[test]
    fn resolved_key_uuid_creation() {
        let uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let key = ResolvedKey::Uuid(uuid);

        assert!(key.is_uuid());
        assert!(!key.is_code());
        assert_eq!(key.as_uuid(), Some(uuid));
        assert_eq!(key.as_code(), None);
        assert_eq!(key.to_key_string(), "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn resolved_key_code_creation() {
        let key = ResolvedKey::Code("DIRECTOR".to_string());

        assert!(!key.is_uuid());
        assert!(key.is_code());
        assert_eq!(key.as_uuid(), None);
        assert_eq!(key.as_code(), Some("DIRECTOR"));
        assert_eq!(key.to_key_string(), "DIRECTOR");
    }

    #[test]
    fn resolved_key_parse_uuid() {
        let key = ResolvedKey::parse("550e8400-e29b-41d4-a716-446655440000");

        assert!(key.is_uuid());
        assert_eq!(
            key.as_uuid(),
            Some(Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap())
        );
    }

    #[test]
    fn resolved_key_parse_code() {
        let key = ResolvedKey::parse("DIRECTOR");

        assert!(key.is_code());
        assert_eq!(key.as_code(), Some("DIRECTOR"));
    }

    #[test]
    fn resolved_key_parse_jurisdiction_code() {
        // Jurisdiction codes like "LU", "US" should NOT be parsed as UUIDs
        let key = ResolvedKey::parse("LU");

        assert!(key.is_code());
        assert_eq!(key.as_code(), Some("LU"));
    }

    #[test]
    fn resolved_key_serializes_with_tag() {
        let uuid_key =
            ResolvedKey::Uuid(Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap());
        let json = serde_json::to_string(&uuid_key).unwrap();
        assert!(json.contains(r#""key_type":"uuid""#));
        assert!(json.contains(r#""value":"550e8400-e29b-41d4-a716-446655440000""#));

        let code_key = ResolvedKey::Code("DIRECTOR".to_string());
        let json = serde_json::to_string(&code_key).unwrap();
        assert!(json.contains(r#""key_type":"code""#));
        assert!(json.contains(r#""value":"DIRECTOR""#));
    }

    #[test]
    fn resolved_key_roundtrip() {
        let original = ResolvedKey::Code("FUND_ACCOUNTING".to_string());
        let json = serde_json::to_string(&original).unwrap();
        let parsed: ResolvedKey = serde_json::from_str(&json).unwrap();

        assert_eq!(original, parsed);
    }

    #[test]
    fn resolved_key_from_uuid() {
        let uuid = Uuid::new_v4();
        let key: ResolvedKey = uuid.into();
        assert!(key.is_uuid());
        assert_eq!(key.as_uuid(), Some(uuid));
    }

    #[test]
    fn resolved_key_from_string() {
        let key: ResolvedKey = "DIRECTOR".to_string().into();
        assert!(key.is_code());

        let uuid_str = "550e8400-e29b-41d4-a716-446655440000".to_string();
        let key: ResolvedKey = uuid_str.into();
        assert!(key.is_uuid());
    }

    #[test]
    fn resolved_key_display() {
        let uuid_key =
            ResolvedKey::Uuid(Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap());
        assert_eq!(
            format!("{}", uuid_key),
            "550e8400-e29b-41d4-a716-446655440000"
        );

        let code_key = ResolvedKey::Code("DIRECTOR".to_string());
        assert_eq!(format!("{}", code_key), "DIRECTOR");
    }

    // =========================================================================
    // RefLocation Tests
    // =========================================================================

    #[test]
    fn ref_location_creation() {
        let loc = RefLocation::new(0, "entity-id");
        assert_eq!(loc.stmt_index, 0);
        assert_eq!(loc.arg_name, "entity-id");
        assert_eq!(loc.span, None);
    }

    #[test]
    fn ref_location_with_span() {
        let loc = RefLocation::with_span(2, "cbu-id", (10, 25));
        assert_eq!(loc.stmt_index, 2);
        assert_eq!(loc.arg_name, "cbu-id");
        assert_eq!(loc.span, Some((10, 25)));
    }

    #[test]
    fn ref_location_ref_id() {
        let loc = RefLocation::new(3, "person");
        assert_eq!(loc.ref_id(), "3:person");
    }

    #[test]
    fn ref_location_display() {
        let loc = RefLocation::new(1, "entity-id");
        assert_eq!(format!("{}", loc), "stmt[1].entity-id");
    }

    #[test]
    fn ref_location_equality() {
        let loc1 = RefLocation::new(0, "entity-id");
        let loc2 = RefLocation::new(0, "entity-id");
        let loc3 = RefLocation::new(1, "entity-id");
        let loc4 = RefLocation::new(0, "cbu-id");

        assert_eq!(loc1, loc2);
        assert_ne!(loc1, loc3); // Different stmt_index
        assert_ne!(loc1, loc4); // Different arg_name
    }

    #[test]
    fn ref_location_serializes() {
        let loc = RefLocation::new(2, "entity-id");
        let json = serde_json::to_string(&loc).unwrap();
        assert!(json.contains(r#""stmt_index":2"#));
        assert!(json.contains(r#""arg_name":"entity-id""#));
        // span should be omitted when None
        assert!(!json.contains("span"));
    }

    #[test]
    fn ref_location_roundtrip() {
        let original = RefLocation::with_span(5, "owner-id", (100, 150));
        let json = serde_json::to_string(&original).unwrap();
        let parsed: RefLocation = serde_json::from_str(&json).unwrap();
        assert_eq!(original, parsed);
    }
}
