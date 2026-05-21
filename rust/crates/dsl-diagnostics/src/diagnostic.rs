//! Source-attributed diagnostic types for the unified DSL v0.1.
//!
//! Diagnostics carry a severity, a human-readable message, an optional
//! source span, and an optional well-known code string. The `DiagnosticBag`
//! is the accumulator passed through parsing and lowering phases.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Well-known diagnostic codes
// ---------------------------------------------------------------------------

/// The atom's kind string was not found in the known kind catalogue.
pub const UNKNOWN_ATOM_KIND: &str = "E0001";
/// The atom's kind string looked declarative but was not in the declarative catalogue.
pub const UNKNOWN_DECLARATIVE_KIND: &str = "E0002";
/// A template parameter reference was not bound by the enclosing template.
pub const UNKNOWN_TEMPLATE_PARAMETER: &str = "E0003";
/// A name reference could not be resolved to a known atom.
pub const UNRESOLVED_NAME_REF: &str = "E0004";
/// A required slot was absent from an atom definition.
pub const MISSING_REQUIRED_SLOT: &str = "E0005";
/// An insertion marker (`$symbol`) was not resolved during template expansion.
pub const UNRESOLVED_INSERTION_MARKER: &str = "E0006";
/// A pack reference (`pack/atom`) pointed to an unknown pack.
pub const UNKNOWN_PACK_REFERENCE: &str = "E0007";
/// A pack dependency is present but at a deprecated version.
pub const DEPRECATED_PACK_VERSION: &str = "W0001";
/// A pack dependency is present but at a retired (no-longer-valid) version.
pub const RETIRED_PACK_VERSION: &str = "E0008";
/// Two atoms that were being merged carry conflicting slot values.
pub const MERGE_CONFLICT: &str = "E0009";
/// A merge was attempted but the source file did not declare merge intent.
pub const UNDECLARED_MERGE: &str = "E0010";
/// A parameter name contains a dot (`.`), which is reserved for the
/// `for-each` loop-variable accessor syntax (`,var.field`).
pub const INVALID_PARAMETER_NAME: &str = "E0011";
/// A `for-each` body uses `,var.field` but `var` is not the loop variable
/// declared by the enclosing `for-each` form.
pub const UNKNOWN_LOOP_VARIABLE: &str = "E0012";

// ---------------------------------------------------------------------------
// Span
// ---------------------------------------------------------------------------

/// A source location range in a single file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub file: String,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

// ---------------------------------------------------------------------------
// DiagnosticSeverity
// ---------------------------------------------------------------------------

/// How severe a diagnostic is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Note,
}

// ---------------------------------------------------------------------------
// Diagnostic
// ---------------------------------------------------------------------------

/// A single compiler / parser diagnostic with optional source attribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub span: Option<Span>,
    /// Well-known diagnostic code (e.g. `E0001`). `None` for ad-hoc messages.
    pub code: Option<String>,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            message: message.into(),
            span: None,
            code: None,
        }
    }

    pub fn error_with_code(message: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            message: message.into(),
            span: None,
            code: Some(code.into()),
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Warning,
            message: message.into(),
            span: None,
            code: None,
        }
    }

    pub fn note(message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Note,
            message: message.into(),
            span: None,
            code: None,
        }
    }

    /// Attach a source span to this diagnostic.
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    /// Attach a well-known code to this diagnostic.
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }
}

// ---------------------------------------------------------------------------
// DiagnosticBag
// ---------------------------------------------------------------------------

/// Accumulator for diagnostics emitted during a parse or lowering pass.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DiagnosticBag {
    pub diagnostics: Vec<Diagnostic>,
}

impl DiagnosticBag {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    /// Returns `true` if at least one `Error`-severity diagnostic is present.
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == DiagnosticSeverity::Error)
    }

    /// Iterator over error diagnostics.
    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Error)
    }

    /// Iterator over warning diagnostics.
    pub fn warnings(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Warning)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_bag_has_no_errors() {
        let bag = DiagnosticBag::new();
        assert!(!bag.has_errors());
    }

    #[test]
    fn bag_reports_errors() {
        let mut bag = DiagnosticBag::new();
        bag.push(Diagnostic::error("something went wrong"));
        assert!(bag.has_errors());
        assert_eq!(bag.errors().count(), 1);
        assert_eq!(bag.warnings().count(), 0);
    }

    #[test]
    fn bag_distinguishes_severity() {
        let mut bag = DiagnosticBag::new();
        bag.push(Diagnostic::warning("heads-up"));
        bag.push(Diagnostic::error("fatal"));
        assert!(bag.has_errors());
        assert_eq!(bag.errors().count(), 1);
        assert_eq!(bag.warnings().count(), 1);
    }
}
