//! Unified Diagnostics Module
//!
//! Single diagnostic type used across parse, validation, planning, and execution.
//! Designed to integrate with LSP for rich error reporting.

use serde::{Deserialize, Serialize};

/// Diagnostic severity level
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
    Hint,
    Info,
}

/// Diagnostic codes for categorizing issues
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticCode {
    // =========================================================================
    // Parse errors
    // =========================================================================
    SyntaxError,
    UnexpectedToken,

    // =========================================================================
    // Validation errors
    // =========================================================================
    UnknownVerb,
    UnknownArg,
    MissingRequiredArg,
    InvalidValue,
    UndefinedSymbol,
    DuplicateBinding,
    TypeMismatch,

    // =========================================================================
    // Planning errors
    // =========================================================================
    CyclicDependency,
    MissingProducer,
    UnresolvedBinding,

    // =========================================================================
    // Planning hints
    // =========================================================================
    ImplicitCreateSuggested,
    ReorderingSuggested,

    // =========================================================================
    // Execution errors
    // =========================================================================
    DatabaseError,
    ConstraintViolation,
    CustomOpFailed,
    OpExecutionFailed,
}

/// Source location span
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceSpan {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

impl SourceSpan {
    pub fn new(start_line: u32, start_col: u32, end_line: u32, end_col: u32) -> Self {
        Self {
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }

    /// Create a span from byte offsets (requires source text for line/col calculation)
    pub fn from_byte_offset(source: &str, start: usize, end: usize) -> Self {
        let (start_line, start_col) = byte_to_line_col(source, start);
        let (end_line, end_col) = byte_to_line_col(source, end);
        Self::new(start_line, start_col, end_line, end_col)
    }
}

/// Convert byte offset to line and column
fn byte_to_line_col(source: &str, offset: usize) -> (u32, u32) {
    let mut line = 1u32;
    let mut col = 1u32;

    for (i, c) in source.char_indices() {
        if i >= offset {
            break;
        }
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }

    (line, col)
}

/// Related information for multi-location diagnostics
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RelatedInfo {
    pub message: String,
    pub span: SourceSpan,
}

/// Suggested fix for code actions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SuggestedFix {
    pub description: String,
    pub replacement: String,
    pub span: SourceSpan,
}

/// A diagnostic message with location, severity, and optional fix
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: DiagnosticCode,
    pub message: String,
    pub span: Option<SourceSpan>,
    pub related: Vec<RelatedInfo>,
    pub suggested_fix: Option<SuggestedFix>,
}

impl Diagnostic {
    /// Create an error diagnostic
    pub fn error(code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            code,
            message: message.into(),
            span: None,
            related: vec![],
            suggested_fix: None,
        }
    }

    /// Create a warning diagnostic
    pub fn warning(code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            code,
            message: message.into(),
            span: None,
            related: vec![],
            suggested_fix: None,
        }
    }

    /// Create a hint diagnostic
    pub fn hint(code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Hint,
            code,
            message: message.into(),
            span: None,
            related: vec![],
            suggested_fix: None,
        }
    }

    /// Create an info diagnostic
    pub fn info(code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Info,
            code,
            message: message.into(),
            span: None,
            related: vec![],
            suggested_fix: None,
        }
    }

    /// Add source span
    pub fn with_span(mut self, span: SourceSpan) -> Self {
        self.span = Some(span);
        self
    }

    /// Add suggested fix
    pub fn with_fix(mut self, fix: SuggestedFix) -> Self {
        self.suggested_fix = Some(fix);
        self
    }

    /// Add related information
    pub fn with_related(mut self, related: RelatedInfo) -> Self {
        self.related.push(related);
        self
    }

    /// Check if this is an error
    pub fn is_error(&self) -> bool {
        matches!(self.severity, Severity::Error)
    }

    /// Check if this is a warning
    pub fn is_warning(&self) -> bool {
        matches!(self.severity, Severity::Warning)
    }

    /// Check if this is a hard error (blocks execution)
    pub fn is_hard_error(&self) -> bool {
        self.is_error()
            && matches!(
                self.code,
                DiagnosticCode::SyntaxError
                    | DiagnosticCode::CyclicDependency
                    | DiagnosticCode::UndefinedSymbol
            )
    }
}

// =============================================================================
// Convenience Builders
// =============================================================================

/// Create a hint for implicit entity creation
pub fn implicit_create_hint(binding: &str, entity_type: &str, verb: &str) -> Diagnostic {
    Diagnostic::hint(
        DiagnosticCode::ImplicitCreateSuggested,
        format!(
            "Will inject '{}' to create '{}' (type: {})",
            verb, binding, entity_type
        ),
    )
}

/// Create an error for undefined symbol
pub fn undefined_symbol_error(symbol: &str, span: Option<SourceSpan>) -> Diagnostic {
    let mut diag = Diagnostic::error(
        DiagnosticCode::UndefinedSymbol,
        format!("undefined symbol '@{}'", symbol),
    );
    if let Some(s) = span {
        diag = diag.with_span(s);
    }
    diag
}

/// Create an error for cyclic dependency
pub fn cycle_error(involved_ops: &[String]) -> Diagnostic {
    Diagnostic::error(
        DiagnosticCode::CyclicDependency,
        format!(
            "Circular dependency detected involving: {}",
            involved_ops.join(", ")
        ),
    )
}

/// Create an error for missing required argument
pub fn missing_arg_error(arg_name: &str, verb: &str) -> Diagnostic {
    Diagnostic::error(
        DiagnosticCode::MissingRequiredArg,
        format!(
            "missing required argument '{}' for verb '{}'",
            arg_name, verb
        ),
    )
}

/// Create an error for unknown verb
pub fn unknown_verb_error(verb: &str) -> Diagnostic {
    Diagnostic::error(
        DiagnosticCode::UnknownVerb,
        format!("unknown verb '{}'", verb),
    )
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let diag = Diagnostic::error(DiagnosticCode::SyntaxError, "unexpected token");
        assert!(diag.is_error());
        assert!(diag.is_hard_error());
        assert_eq!(diag.message, "unexpected token");
    }

    #[test]
    fn test_warning_not_error() {
        let diag = Diagnostic::warning(DiagnosticCode::ReorderingSuggested, "consider reordering");
        assert!(!diag.is_error());
        assert!(diag.is_warning());
    }

    #[test]
    fn test_with_span() {
        let span = SourceSpan::new(1, 5, 1, 15);
        let diag = Diagnostic::error(DiagnosticCode::InvalidValue, "bad value").with_span(span);
        assert!(diag.span.is_some());
        assert_eq!(diag.span.as_ref().unwrap().start_col, 5);
    }

    #[test]
    fn test_byte_to_line_col() {
        let source = "line1\nline2\nline3";
        assert_eq!(byte_to_line_col(source, 0), (1, 1));
        assert_eq!(byte_to_line_col(source, 5), (1, 6));
        assert_eq!(byte_to_line_col(source, 6), (2, 1));
        assert_eq!(byte_to_line_col(source, 12), (3, 1));
    }

    #[test]
    fn test_implicit_create_hint() {
        let hint = implicit_create_hint("fund", "cbu", "cbu.ensure");
        assert!(!hint.is_error());
        assert!(hint.message.contains("cbu.ensure"));
        assert!(hint.message.contains("fund"));
    }
}
