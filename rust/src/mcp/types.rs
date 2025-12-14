//! Agent-friendly response types for MCP tools
//!
//! These types transform internal DSL structures into JSON-serializable
//! formats optimized for agent consumption.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// dsl_validate enhanced types
// ============================================================================

/// Enhanced validation output with resolution options
#[derive(Debug, Serialize)]
pub struct ValidationOutput {
    pub valid: bool,
    pub diagnostics: Vec<AgentDiagnostic>,
    pub plan_summary: Option<String>,
    pub suggested_fixes: Vec<SuggestedFix>,
    /// True if statements would be reordered during execution
    pub needs_reorder: bool,
}

/// Agent-friendly diagnostic with resolution options
#[derive(Debug, Serialize)]
pub struct AgentDiagnostic {
    /// "error", "warning", "hint", "info"
    pub severity: String,
    pub message: String,
    pub location: Option<Location>,
    /// Diagnostic code (e.g., "UndefinedSymbol", "CycleDetected")
    pub code: String,
    /// Actionable resolution options
    pub resolution_options: Vec<ResolutionOption>,
}

/// Source location
#[derive(Debug, Clone, Serialize)]
pub struct Location {
    pub line: u32,
    pub column: u32,
    pub length: u32,
}

/// A single resolution option for a diagnostic
#[derive(Debug, Serialize)]
pub struct ResolutionOption {
    pub description: String,
    /// "replace", "insert_before", "delete", "search"
    pub action: String,
    pub replacement: Option<String>,
}

/// A suggested DSL fix (e.g., for implicit creates)
#[derive(Debug, Serialize)]
pub struct SuggestedFix {
    pub description: String,
    /// The DSL code to insert/replace
    pub dsl: String,
    /// Line number to insert at (if applicable)
    pub insert_at: Option<u32>,
}

// ============================================================================
// dsl_execute enhanced types
// ============================================================================

/// Enhanced execution output
#[derive(Debug, Serialize)]
pub struct ExecutionOutput {
    pub success: bool,
    pub results: Vec<StepResultSummary>,
    /// Bindings created: name → uuid
    pub bindings: HashMap<String, String>,
    pub error: Option<String>,
    pub summary: String,
}

/// Summary of a single execution step
#[derive(Debug, Serialize)]
pub struct StepResultSummary {
    pub verb: String,
    /// "created", "updated", "linked", "deleted", "skipped"
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<String>,
    pub entity_display: String,
    pub binding: Option<String>,
}

// ============================================================================
// entity_search types
// ============================================================================

/// Search output with disambiguation support
#[derive(Debug, Serialize)]
pub struct SearchOutput {
    pub matches: Vec<EntityMatch>,
    pub exact_match: Option<EntityMatch>,
    pub ambiguous: bool,
    /// Prompt to show user when disambiguation needed
    pub disambiguation_prompt: Option<String>,
}

/// A single entity match from search
#[derive(Debug, Clone, Serialize)]
pub struct EntityMatch {
    pub id: String,
    pub display: String,
    pub entity_type: String,
    pub score: f32,
    /// Additional context for disambiguation
    pub context: HashMap<String, String>,
}

// ============================================================================
// session_context types
// ============================================================================

/// Session action for managing conversation state
#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum SessionAction {
    /// Create a new session
    Create,
    /// Get existing session state
    Get { session_id: String },
    /// Update session with new bindings
    Update {
        session_id: String,
        bindings: HashMap<String, String>,
    },
    /// Undo last execution block
    Undo { session_id: String },
    /// Clear all bindings
    Clear { session_id: String },
}

/// Current session state
#[derive(Debug, Serialize)]
pub struct SessionState {
    pub session_id: String,
    pub bindings: HashMap<String, BindingInfo>,
    pub history_count: usize,
    pub can_undo: bool,
}

/// Information about a single binding
#[derive(Debug, Clone, Serialize)]
pub struct BindingInfo {
    pub name: String,
    pub uuid: String,
    pub entity_type: String,
}

// ============================================================================
// Conversion helpers
// ============================================================================

impl From<crate::dsl_v2::diagnostics::Severity> for String {
    fn from(s: crate::dsl_v2::diagnostics::Severity) -> Self {
        match s {
            crate::dsl_v2::diagnostics::Severity::Error => "error".to_string(),
            crate::dsl_v2::diagnostics::Severity::Warning => "warning".to_string(),
            crate::dsl_v2::diagnostics::Severity::Hint => "hint".to_string(),
            crate::dsl_v2::diagnostics::Severity::Info => "info".to_string(),
        }
    }
}

impl From<crate::dsl_v2::diagnostics::SourceSpan> for Location {
    fn from(span: crate::dsl_v2::diagnostics::SourceSpan) -> Self {
        Location {
            line: span.start_line,
            column: span.start_col,
            length: span.end_col.saturating_sub(span.start_col),
        }
    }
}
