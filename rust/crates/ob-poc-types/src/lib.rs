//! Shared API Types for OB-POC
//!
//! This crate is the SINGLE SOURCE OF TRUTH for all types crossing HTTP boundaries.
//!
//! ## Boundaries
//!
//! ```text
//! ┌──────────────────┐         ┌──────────────────┐
//! │  Rust Server     │  JSON   │  TypeScript      │
//! │  (Axum)          │ ◄─────► │  (HTML panels)   │
//! └──────────────────┘         └──────────────────┘
//!          │
//!          │ JSON
//!          ▼
//! ┌──────────────────┐
//! │  Rust WASM       │
//! │  (Graph)         │
//! └──────────────────┘
//!
//! Plus: TS ◄──CustomEvent──► WASM (just entity IDs)
//! ```
//!
//! ## Rules
//!
//! 1. All API types live here - no inline struct definitions in handlers
//! 2. Use `#[derive(TS)]` for TypeScript generation
//! 3. Tagged enums only: `#[serde(tag = "type")]`
//! 4. CustomEvent payloads: just UUIDs as strings

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

// ============================================================================
// SESSION API
// ============================================================================

/// Request to create a new session
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CreateSessionRequest {
    #[serde(default)]
    pub domain_hint: Option<String>,
}

/// Response after creating a session
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CreateSessionResponse {
    pub session_id: String, // UUID as string for TS
    pub state: String,
    pub created_at: String, // ISO 8601
}

/// Bound entity info for session state
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BoundEntityInfo {
    pub id: String,          // UUID as string
    pub name: String,        // Display name
    pub entity_type: String, // e.g., "cbu", "entity"
}

/// Session state response
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SessionStateResponse {
    pub session_id: String,
    pub state: String,
    pub message_count: usize,
    pub can_execute: bool,
    #[serde(default)]
    #[ts(optional)]
    pub dsl_source: Option<String>,
    /// Active CBU for this session (if set via bind)
    #[serde(default)]
    #[ts(optional)]
    pub active_cbu: Option<BoundEntityInfo>,
    /// Named bindings available in the session (name -> entity info)
    #[serde(default)]
    pub bindings: std::collections::HashMap<String, BoundEntityInfo>,
}

// ============================================================================
// CHAT API
// ============================================================================

/// Chat request from user
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ChatRequest {
    pub message: String,
    #[serde(default)]
    pub cbu_id: Option<String>, // UUID as string
}

/// Chat response from agent
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ChatResponse {
    pub message: String,
    pub can_execute: bool,
    #[serde(default)]
    #[ts(optional)]
    pub dsl_source: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub ast: Option<Vec<AstStatement>>,
    pub session_state: String,
    /// UI commands to execute (show CBU, highlight entity, etc.)
    #[serde(default)]
    #[ts(optional)]
    pub commands: Option<Vec<AgentCommand>>,
}

/// SSE stream event - MUST use tagged enum for TypeScript discrimination
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
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
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "action", rename_all = "snake_case")]
#[ts(export)]
pub enum AgentCommand {
    /// Show a specific CBU in the graph
    ShowCbu { cbu_id: String },
    /// Highlight an entity in the graph
    HighlightEntity { entity_id: String },
    /// Navigate to a line in the DSL panel
    NavigateDsl { line: u32 },
    /// Focus an AST node
    FocusAst { node_id: String },
}

// ============================================================================
// EXECUTE API
// ============================================================================

/// Request to execute DSL
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ExecuteRequest {
    #[serde(default)]
    pub dsl: Option<String>,
}

/// Response from DSL execution
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ExecuteResponse {
    pub success: bool,
    pub results: Vec<ExecuteResult>,
    pub errors: Vec<String>,
    #[serde(default)]
    #[ts(optional)]
    pub bindings: Option<std::collections::HashMap<String, String>>, // name -> UUID
}

/// Individual statement execution result
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ExecuteResult {
    pub statement_index: usize,
    pub success: bool,
    pub message: String,
    #[serde(default)]
    #[ts(optional)]
    pub entity_id: Option<String>,
}

// ============================================================================
// CBU API
// ============================================================================

/// CBU summary for list views
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CbuSummary {
    pub cbu_id: String,
    pub name: String,
    #[serde(default)]
    #[ts(optional)]
    pub jurisdiction: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub client_type: Option<String>,
}

// ============================================================================
// GRAPH API
// ============================================================================

/// Full CBU graph for visualization
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CbuGraphResponse {
    pub cbu_id: String,
    pub label: String,
    #[serde(default)]
    #[ts(optional)]
    pub cbu_category: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub jurisdiction: Option<String>,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// Node in the CBU graph
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GraphNode {
    pub id: String,
    pub node_type: String,
    pub layer: String,
    pub label: String,
    #[serde(default)]
    #[ts(optional)]
    pub sublabel: Option<String>,
    pub status: String,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub role_categories: Vec<String>,
    #[serde(default)]
    #[ts(optional)]
    pub primary_role: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub jurisdiction: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub ownership_pct: Option<f64>,
    /// Role priority for layout ordering
    #[serde(default)]
    #[ts(optional)]
    pub role_priority: Option<i32>,
    /// Additional node data (JSON blob) - skipped in TS, use `any` in TypeScript
    #[serde(default)]
    #[ts(optional)]
    #[ts(type = "Record<string, unknown> | null")]
    pub data: Option<serde_json::Value>,
    /// Server-computed X position
    #[serde(default)]
    #[ts(optional)]
    pub x: Option<f64>,
    /// Server-computed Y position
    #[serde(default)]
    #[ts(optional)]
    pub y: Option<f64>,
}

/// Edge in the CBU graph
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub edge_type: String,
    #[serde(default)]
    #[ts(optional)]
    pub label: Option<String>,
}

// ============================================================================
// DSL API
// ============================================================================

/// DSL source response
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DslResponse {
    pub source: String,
    #[serde(default)]
    #[ts(optional)]
    pub session_id: Option<String>,
}

// ============================================================================
// AST API
// ============================================================================

/// AST response containing all statements
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AstResponse {
    pub statements: Vec<AstStatement>,
}

/// A single AST statement (VerbCall or Comment)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum AstStatement {
    VerbCall {
        domain: String,
        verb: String,
        arguments: Vec<AstArgument>,
        #[serde(default)]
        #[ts(optional)]
        binding: Option<String>,
        #[serde(default)]
        #[ts(optional)]
        span: Option<AstSpan>,
    },
    Comment {
        text: String,
        #[serde(default)]
        #[ts(optional)]
        span: Option<AstSpan>,
    },
}

/// AST argument (key-value pair)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AstArgument {
    pub key: String,
    pub value: AstValue,
    #[serde(default)]
    #[ts(optional)]
    pub span: Option<AstSpan>,
}

/// AST value types
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
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
        #[ts(optional)]
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
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AstMapEntry {
    pub key: String,
    pub value: AstValue,
}

/// Source location span
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AstSpan {
    pub start: usize,
    pub end: usize,
    #[serde(default)]
    #[ts(optional)]
    pub start_line: Option<u32>,
    #[serde(default)]
    #[ts(optional)]
    pub end_line: Option<u32>,
}

// ============================================================================
// CUSTOM EVENT PAYLOADS (TS ↔ WASM)
// Keep these dead simple - just IDs
// ============================================================================

/// Event from TypeScript to WASM: load a CBU
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LoadCbuEvent {
    pub cbu_id: String,
}

/// Event from TypeScript to WASM: focus an entity
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FocusEntityEvent {
    pub entity_id: String,
}

/// Event from TypeScript to WASM: change view mode
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SetViewModeEvent {
    pub view_mode: String, // "KYC_UBO", "SERVICE_DELIVERY", etc.
}

/// Event from WASM to TypeScript: entity selected
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct EntitySelectedEvent {
    pub entity_id: String,
}

/// Event from WASM to TypeScript: CBU changed
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CbuChangedEvent {
    pub cbu_id: String,
}

// ============================================================================
// DISAMBIGUATION API
// ============================================================================

/// Disambiguation request - sent when user input is ambiguous
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DisambiguationRequest {
    /// Unique ID for this disambiguation request
    pub request_id: String,
    /// The ambiguous items that need resolution
    pub items: Vec<DisambiguationItem>,
    /// Human-readable prompt for the user
    pub prompt: String,
}

/// A single ambiguous item needing resolution
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
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
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct EntityMatch {
    /// Entity UUID
    pub entity_id: String,
    /// Display name
    pub name: String,
    /// Entity type (e.g., "proper_person", "limited_company")
    pub entity_type: String,
    /// Jurisdiction code
    #[serde(default)]
    #[ts(optional)]
    pub jurisdiction: Option<String>,
    /// Additional context (roles, etc.)
    #[serde(default)]
    #[ts(optional)]
    pub context: Option<String>,
    /// Match score (0.0 - 1.0)
    #[serde(default)]
    #[ts(optional)]
    pub score: Option<f64>,
}

/// A possible interpretation of ambiguous text
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Interpretation {
    /// Interpretation ID
    pub id: String,
    /// Human-readable label
    pub label: String,
    /// What this interpretation means
    pub description: String,
    /// How this affects the generated DSL
    #[serde(default)]
    #[ts(optional)]
    pub effect: Option<String>,
}

/// User's disambiguation response
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DisambiguationResponse {
    /// The request ID being responded to
    pub request_id: String,
    /// Selected resolutions
    pub selections: Vec<DisambiguationSelection>,
}

/// A single disambiguation selection
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
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
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
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
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "status", rename_all = "snake_case")]
#[ts(export)]
pub enum ChatPayload {
    /// DSL is ready (no ambiguity or already resolved)
    Ready {
        dsl_source: String,
        #[serde(default)]
        #[ts(optional)]
        ast: Option<Vec<AstStatement>>,
        can_execute: bool,
        #[serde(default)]
        #[ts(optional)]
        commands: Option<Vec<AgentCommand>>,
    },
    /// Needs user disambiguation before generating DSL
    NeedsDisambiguation {
        disambiguation: DisambiguationRequest,
    },
    /// Just a message, no DSL
    Message {
        #[serde(default)]
        #[ts(optional)]
        commands: Option<Vec<AgentCommand>>,
    },
}

// ============================================================================
// CONVERSION HELPERS
// ============================================================================

impl CreateSessionResponse {
    pub fn new(session_id: Uuid, state: &str, created_at: DateTime<Utc>) -> Self {
        Self {
            session_id: session_id.to_string(),
            state: state.to_string(),
            created_at: created_at.to_rfc3339(),
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
            success: true,
            message: message.to_string(),
            entity_id: entity_id.map(|id| id.to_string()),
        }
    }

    pub fn failure(index: usize, message: &str) -> Self {
        Self {
            statement_index: index,
            success: false,
            message: message.to_string(),
            entity_id: None,
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
}
