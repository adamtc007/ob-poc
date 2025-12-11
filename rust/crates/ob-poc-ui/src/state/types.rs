//! API Response Types
//!
//! Types matching the Rust backend API responses.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::session::BoundEntity;

// =============================================================================
// SESSION API TYPES
// =============================================================================

/// Response from POST /api/session
#[derive(Debug, Clone, Deserialize)]
pub struct SessionResponse {
    pub session_id: Uuid,
    pub state: String,
    #[serde(default)]
    pub domain_hint: Option<String>,
}

/// Response from POST /api/session/:id/chat
#[derive(Debug, Clone, Deserialize)]
pub struct ChatResponse {
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub dsl_source: Option<String>,
    #[serde(default)]
    pub ast: Option<Vec<AstStatement>>,
    #[serde(default)]
    pub bindings: Option<HashMap<String, BoundEntity>>,
    #[serde(default)]
    pub can_execute: bool,
    #[serde(default)]
    pub session_state: Option<String>,
    #[serde(default)]
    pub validation_errors: Vec<String>,
}

/// Response from POST /api/session/:id/execute
#[derive(Debug, Clone, Deserialize)]
pub struct ExecuteResponse {
    pub success: bool,
    #[serde(default)]
    pub results: Vec<ExecutionResult>,
    #[serde(default)]
    pub bindings: Option<HashMap<String, String>>,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub new_state: Option<String>,
    #[serde(default)]
    pub dsl_source: Option<String>,
    #[serde(default)]
    pub ast: Option<Vec<AstStatement>>,
}

/// Single execution result
#[derive(Debug, Clone, Deserialize)]
pub struct ExecutionResult {
    pub statement_index: usize,
    pub dsl: String,
    pub success: bool,
    pub message: String,
    #[serde(default)]
    pub entity_id: Option<String>,
    #[serde(default)]
    pub entity_type: Option<String>,
}

/// Binding request for POST /api/session/:id/bind
#[derive(Debug, Clone, Serialize)]
pub struct BindRequest {
    pub name: String,
    pub id: String,
    pub entity_type: String,
    pub display_name: String,
}

// =============================================================================
// AST TYPES (matches Rust backend AST)
// =============================================================================

/// AST Statement - either a VerbCall or Comment
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum AstStatement {
    VerbCall(AstVerbCall),
    #[allow(non_snake_case)]
    Comment {
        Comment: String,
    },
}

/// AST VerbCall wrapper
#[derive(Debug, Clone, Deserialize)]
pub struct AstVerbCall {
    #[serde(rename = "VerbCall")]
    pub verb_call: VerbCallData,
}

/// VerbCall data
#[derive(Debug, Clone, Deserialize)]
pub struct VerbCallData {
    pub domain: String,
    pub verb: String,
    pub arguments: Vec<AstArgument>,
    #[serde(default)]
    pub binding: Option<String>,
    pub span: Span,
}

/// AST Argument
#[derive(Debug, Clone, Deserialize)]
pub struct AstArgument {
    pub key: String,
    pub value: AstValue,
    pub span: Span,
}

/// AST Value variants
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum AstValue {
    EntityRef(AstEntityRef),
    SymbolRef(AstSymbolRef),
    Literal(AstLiteral),
    List(Vec<AstValue>),
    Map(HashMap<String, AstValue>),
}

/// EntityRef - reference to an external entity
#[derive(Debug, Clone, Deserialize)]
pub struct AstEntityRef {
    #[serde(rename = "EntityRef")]
    pub entity_ref: EntityRefData,
}

/// EntityRef data
#[derive(Debug, Clone, Deserialize)]
pub struct EntityRefData {
    pub entity_type: String,
    #[serde(default)]
    pub search_column: Option<String>,
    pub value: String,
    #[serde(default)]
    pub resolved_key: Option<String>,
    pub span: Span,
}

impl EntityRefData {
    /// Check if this EntityRef is unresolved (needs resolution)
    pub fn is_unresolved(&self) -> bool {
        self.resolved_key.is_none()
    }
}

/// SymbolRef - reference to a bound symbol (@name)
#[derive(Debug, Clone, Deserialize)]
pub struct AstSymbolRef {
    #[serde(rename = "SymbolRef")]
    pub symbol_ref: SymbolRefData,
}

/// SymbolRef data
#[derive(Debug, Clone, Deserialize)]
pub struct SymbolRefData {
    pub name: String,
    pub span: Span,
}

/// Literal value
#[derive(Debug, Clone, Deserialize)]
pub struct AstLiteral {
    #[serde(rename = "Literal")]
    pub literal: LiteralData,
}

/// Literal data
#[derive(Debug, Clone, Deserialize)]
pub struct LiteralData {
    pub value: serde_json::Value,
    pub span: Span,
}

/// Source span
#[derive(Debug, Clone, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

// =============================================================================
// SIMPLIFIED AST TYPES FOR UI RENDERING
// =============================================================================

/// Simplified AST statement for UI rendering
#[derive(Debug, Clone)]
pub enum SimpleAstStatement {
    VerbCall {
        verb: String,
        args: Vec<SimpleAstArg>,
        bind_as: Option<String>,
    },
    Comment(String),
}

/// Simplified AST argument for UI rendering
#[derive(Debug, Clone)]
pub struct SimpleAstArg {
    pub key: String,
    pub value: AstNode,
}

/// Simplified AST node for UI rendering
#[derive(Debug, Clone)]
pub enum AstNode {
    String(String),
    Number(f64),
    Boolean(bool),
    Null,
    SymbolRef(String),
    EntityRef {
        entity_type: String,
        value: String,
        resolved_key: Option<String>,
    },
    List(Vec<AstNode>),
    Map(Vec<(String, AstNode)>),
}

impl SimpleAstStatement {
    /// Convert from API AstStatement to simplified form
    pub fn from_api(stmt: &AstStatement) -> Self {
        match stmt {
            AstStatement::VerbCall(vc) => {
                let data = &vc.verb_call;
                SimpleAstStatement::VerbCall {
                    verb: format!("{}.{}", data.domain, data.verb),
                    args: data
                        .arguments
                        .iter()
                        .map(|a| SimpleAstArg {
                            key: a.key.clone(),
                            value: AstNode::from_api_value(&a.value),
                        })
                        .collect(),
                    bind_as: data.binding.clone(),
                }
            }
            AstStatement::Comment { Comment } => SimpleAstStatement::Comment(Comment.clone()),
        }
    }
}

impl AstNode {
    /// Convert from API AstValue to simplified form
    pub fn from_api_value(value: &AstValue) -> Self {
        match value {
            AstValue::EntityRef(er) => AstNode::EntityRef {
                entity_type: er.entity_ref.entity_type.clone(),
                value: er.entity_ref.value.clone(),
                resolved_key: er.entity_ref.resolved_key.clone(),
            },
            AstValue::SymbolRef(sr) => AstNode::SymbolRef(sr.symbol_ref.name.clone()),
            AstValue::Literal(lit) => AstNode::from_json(&lit.literal.value),
            AstValue::List(items) => {
                AstNode::List(items.iter().map(AstNode::from_api_value).collect())
            }
            AstValue::Map(map) => AstNode::Map(
                map.iter()
                    .map(|(k, v)| (k.clone(), AstNode::from_api_value(v)))
                    .collect(),
            ),
        }
    }

    /// Convert from serde_json::Value
    fn from_json(value: &serde_json::Value) -> Self {
        match value {
            serde_json::Value::Null => AstNode::Null,
            serde_json::Value::Bool(b) => AstNode::Boolean(*b),
            serde_json::Value::Number(n) => AstNode::Number(n.as_f64().unwrap_or(0.0)),
            serde_json::Value::String(s) => AstNode::String(s.clone()),
            serde_json::Value::Array(arr) => {
                AstNode::List(arr.iter().map(AstNode::from_json).collect())
            }
            serde_json::Value::Object(obj) => AstNode::Map(
                obj.iter()
                    .map(|(k, v)| (k.clone(), AstNode::from_json(v)))
                    .collect(),
            ),
        }
    }
}

// =============================================================================
// ENTITY SEARCH TYPES
// =============================================================================

/// Entity search response
#[derive(Debug, Clone, Deserialize)]
pub struct EntitySearchResponse {
    pub results: Vec<EntityMatch>,
    #[serde(default)]
    pub total: usize,
}

/// Single entity match
#[derive(Debug, Clone, Deserialize)]
pub struct EntityMatch {
    #[serde(alias = "token")]
    pub value: String,
    #[serde(alias = "display")]
    pub name: String,
    #[serde(default)]
    pub entity_type: Option<String>,
    #[serde(default)]
    pub jurisdiction: Option<String>,
    #[serde(default)]
    pub score: f32,
}

// =============================================================================
// DSL OPERATIONS TYPES
// =============================================================================

/// Response from POST /api/dsl/parse
#[derive(Debug, Clone, Deserialize)]
pub struct ParseResponse {
    pub success: bool,
    #[serde(default)]
    pub ast: Option<Vec<AstStatement>>,
    #[serde(default)]
    pub error: Option<String>,
}

/// Request for POST /api/dsl/resolve-ref
#[derive(Debug, Clone, Serialize)]
pub struct ResolveRefRequest {
    pub dsl: String,
    pub ref_id: RefId,
    pub resolved_key: String,
}

/// Reference ID for EntityRef resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefId {
    pub statement_index: usize,
    pub arg_key: String,
}

/// Response from POST /api/dsl/resolve-ref
#[derive(Debug, Clone, Deserialize)]
pub struct ResolveRefResponse {
    pub success: bool,
    #[serde(default)]
    pub dsl: Option<String>,
    #[serde(default)]
    pub ast: Option<Vec<AstStatement>>,
    #[serde(default)]
    pub error: Option<String>,
}

// =============================================================================
// CBU TYPES
// =============================================================================

/// CBU summary for picker
#[derive(Debug, Clone, Deserialize)]
pub struct CbuSummary {
    pub cbu_id: Uuid,
    pub name: String,
    #[serde(default)]
    pub jurisdiction: Option<String>,
    #[serde(default)]
    pub client_type: Option<String>,
}

/// Completion request for /api/agent/complete
#[derive(Debug, Clone, Serialize)]
pub struct CompleteRequest {
    pub entity_type: String,
    pub query: String,
    #[serde(default)]
    pub limit: Option<usize>,
}

/// Completion response
#[derive(Debug, Clone, Deserialize)]
pub struct CompleteResponse {
    pub items: Vec<CompletionItem>,
}

/// Single completion item
#[derive(Debug, Clone, Deserialize)]
pub struct CompletionItem {
    pub token: String,
    pub display: String,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub score: f32,
}

// =============================================================================
// GRAPH TYPES (re-export from graph module)
// =============================================================================

/// View mode for graph visualization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub enum ViewMode {
    #[default]
    #[serde(rename = "KYC_UBO")]
    KycUbo,
    #[serde(rename = "SERVICE_DELIVERY")]
    ServiceDelivery,
    #[serde(rename = "CUSTODY")]
    Custody,
    #[serde(rename = "PRODUCTS_ONLY")]
    ProductsOnly,
}

impl ViewMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ViewMode::KycUbo => "KYC_UBO",
            ViewMode::ServiceDelivery => "SERVICE_DELIVERY",
            ViewMode::Custody => "CUSTODY",
            ViewMode::ProductsOnly => "PRODUCTS_ONLY",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ViewMode::KycUbo => "KYC / UBO",
            ViewMode::ServiceDelivery => "Service Delivery",
            ViewMode::Custody => "Custody",
            ViewMode::ProductsOnly => "Products",
        }
    }

    pub fn all() -> &'static [ViewMode] {
        &[
            ViewMode::KycUbo,
            ViewMode::ServiceDelivery,
            ViewMode::Custody,
            ViewMode::ProductsOnly,
        ]
    }
}

/// Graph orientation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub enum Orientation {
    #[default]
    #[serde(rename = "VERTICAL")]
    Vertical,
    #[serde(rename = "HORIZONTAL")]
    Horizontal,
}

impl Orientation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Orientation::Vertical => "VERTICAL",
            Orientation::Horizontal => "HORIZONTAL",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Orientation::Vertical => "Top-Down",
            Orientation::Horizontal => "Left-Right",
        }
    }

    pub fn all() -> &'static [Orientation] {
        &[Orientation::Vertical, Orientation::Horizontal]
    }
}
