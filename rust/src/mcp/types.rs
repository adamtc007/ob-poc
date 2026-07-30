//! Agent-friendly response types for MCP tools
//!
//! These types transform internal DSL structures into JSON-serializable
//! formats optimized for agent consumption.

use serde::Serialize;

// ============================================================================
// dsl_validate enhanced types
// ============================================================================

/// Enhanced validation output with resolution options
#[derive(Debug, Serialize)]
pub(crate) struct ValidationOutput {
    pub valid: bool,
    pub diagnostics: Vec<AgentDiagnostic>,
    pub plan_summary: Option<String>,
    pub suggested_fixes: Vec<SuggestedFix>,
    /// True if statements would be reordered during execution
    pub needs_reorder: bool,
}

/// Agent-friendly diagnostic with resolution options
#[derive(Debug, Serialize)]
pub(crate) struct AgentDiagnostic {
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
pub(crate) struct Location {
    pub line: u32,
    pub column: u32,
    pub length: u32,
}

/// A single resolution option for a diagnostic
#[derive(Debug, Serialize)]
pub(crate) struct ResolutionOption {
    pub description: String,
    /// "replace", "insert_before", "delete", "search"
    pub action: String,
    pub replacement: Option<String>,
}

/// A suggested DSL fix (e.g., for implicit creates)
#[derive(Debug, Serialize)]
pub(crate) struct SuggestedFix {
    pub description: String,
    /// The DSL code to insert/replace
    pub dsl: String,
    /// Line number to insert at (if applicable)
    pub insert_at: Option<u32>,
}

// ============================================================================
// Conversion helpers
// ============================================================================

/// Convert Severity to string representation (helper to avoid orphan rule)
pub(crate) fn severity_to_string(s: crate::dsl_v2::diagnostics::Severity) -> String {
    match s {
        crate::dsl_v2::diagnostics::Severity::Error => "error".to_string(),
        crate::dsl_v2::diagnostics::Severity::Warning => "warning".to_string(),
        crate::dsl_v2::diagnostics::Severity::Hint => "hint".to_string(),
        crate::dsl_v2::diagnostics::Severity::Info => "info".to_string(),
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
