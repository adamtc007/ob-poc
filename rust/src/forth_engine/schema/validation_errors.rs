//! Error types for schema validation.

use crate::forth_engine::schema::ast::span::{Span, span_to_line_col, get_source_line};
use crate::forth_engine::schema::ast::symbols::SymbolError;

/// Validation report containing all errors.
#[derive(Debug, Clone, Default)]
pub struct ValidationReport {
    pub errors: Vec<ValidationError>,
}

/// A single validation error.
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub span: Span,
    pub kind: ErrorKind,
}

/// Kind of validation error.
#[derive(Debug, Clone)]
pub enum ErrorKind {
    /// Unknown verb name
    UnknownVerb {
        name: String,
        suggestions: Vec<&'static str>,
    },
    /// Unknown argument for verb
    UnknownArg {
        arg: String,
        verb: &'static str,
        suggestions: Vec<String>,
    },
    /// Missing required argument
    MissingRequired {
        arg: &'static str,
        verb: &'static str,
        required_because: String,
    },
    /// Type mismatch
    TypeMismatch {
        arg: &'static str,
        expected: String,
        got: String,
    },
    /// Validation rule failed
    ValidationFailed {
        arg: &'static str,
        rule: String,
        message: String,
    },
    /// Cross-constraint violation
    ConstraintViolation {
        constraint: String,
    },
    /// Undefined symbol reference
    UndefinedSymbol {
        name: String,
        defined_symbols: Vec<String>,
    },
    /// Symbol definition error
    SymbolError(SymbolError),
    /// Invalid reference (not in lookup table)
    InvalidRef {
        ref_type: String,
        code: String,
        suggestions: Vec<String>,
    },
}

impl ErrorKind {
    /// Get error code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnknownVerb { .. } => "E001",
            Self::UnknownArg { .. } => "E002",
            Self::MissingRequired { .. } => "E003",
            Self::TypeMismatch { .. } => "E004",
            Self::ValidationFailed { .. } => "E005",
            Self::ConstraintViolation { .. } => "E006",
            Self::UndefinedSymbol { .. } => "E007",
            Self::SymbolError(_) => "E008",
            Self::InvalidRef { .. } => "E009",
        }
    }

    /// Get error message.
    pub fn message(&self) -> String {
        match self {
            Self::UnknownVerb { name, .. } =>
                format!("unknown verb '{}'", name),
            Self::UnknownArg { arg, verb, .. } =>
                format!("unknown argument '{}' for verb '{}'", arg, verb),
            Self::MissingRequired { arg, verb, required_because } =>
                format!("missing required argument '{}' for '{}' ({})", arg, verb, required_because),
            Self::TypeMismatch { arg, expected, got } =>
                format!("'{}': expected {}, got {}", arg, expected, got),
            Self::ValidationFailed { arg, message, .. } =>
                format!("'{}': {}", arg, message),
            Self::ConstraintViolation { constraint } =>
                format!("constraint violated: {}", constraint),
            Self::UndefinedSymbol { name, .. } =>
                format!("undefined symbol '@{}'", name),
            Self::SymbolError(e) => e.to_string(),
            Self::InvalidRef { ref_type, code, .. } =>
                format!("unknown {}: '{}'", ref_type, code),
        }
    }

    /// Get hint for fixing the error.
    pub fn hint(&self) -> Option<String> {
        match self {
            Self::UnknownVerb { suggestions, .. } if !suggestions.is_empty() =>
                Some(format!("did you mean: {}?", suggestions.join(", "))),
            Self::UnknownArg { suggestions, .. } if !suggestions.is_empty() =>
                Some(format!("did you mean: {}?", suggestions.join(", "))),
            Self::UndefinedSymbol { defined_symbols, .. } if !defined_symbols.is_empty() =>
                Some(format!("defined symbols: {}", defined_symbols.join(", "))),
            Self::InvalidRef { suggestions, .. } if !suggestions.is_empty() =>
                Some(format!("did you mean: {}?", suggestions.join(", "))),
            _ => None,
        }
    }
}

impl ValidationReport {
    /// Create a new empty report.
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    /// Add an error.
    pub fn push(&mut self, error: ValidationError) {
        self.errors.push(error);
    }

    /// Check if there are any errors.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Get error count.
    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    /// Format errors for display.
    pub fn format(&self, source: &str, filename: &str) -> String {
        let mut out = String::new();

        for err in &self.errors {
            let (line, col) = span_to_line_col(source, &err.span);
            let line_text = get_source_line(source, line);

            // Error header
            out += &format!(
                "\x1b[1;31merror[{}]\x1b[0m: {}\n",
                err.kind.code(),
                err.kind.message()
            );

            // Location
            out += &format!(
                "  \x1b[1;34m-->\x1b[0m {}:{}:{}\n",
                filename,
                line,
                col
            );

            // Source context
            out += &format!("   \x1b[1;34m|\x1b[0m\n");
            out += &format!(
                "\x1b[1;34m{:3}|\x1b[0m {}\n",
                line,
                line_text
            );
            
            // Underline
            let underline_start = col.saturating_sub(1) as usize;
            let underline_len = err.span.len().max(1);
            out += &format!(
                "   \x1b[1;34m|\x1b[0m {}\x1b[1;31m{}\x1b[0m\n",
                " ".repeat(underline_start),
                "^".repeat(underline_len.min(line_text.len().saturating_sub(underline_start)))
            );

            // Hint
            if let Some(hint) = err.kind.hint() {
                out += &format!(
                    "   \x1b[1;34m= \x1b[0m\x1b[1mhint\x1b[0m: {}\n",
                    hint
                );
            }

            out += "\n";
        }

        // Summary
        if !self.errors.is_empty() {
            out += &format!(
                "\x1b[1;31merror\x1b[0m: aborting due to {} previous error{}\n",
                self.errors.len(),
                if self.errors.len() == 1 { "" } else { "s" }
            );
        }

        out
    }

    /// Format errors without ANSI colors (for logs/tests).
    pub fn format_plain(&self, source: &str, filename: &str) -> String {
        let mut out = String::new();

        for err in &self.errors {
            let (line, col) = span_to_line_col(source, &err.span);

            out += &format!(
                "error[{}]: {} at {}:{}:{}\n",
                err.kind.code(),
                err.kind.message(),
                filename,
                line,
                col
            );

            if let Some(hint) = err.kind.hint() {
                out += &format!("  hint: {}\n", hint);
            }
        }

        out
    }
}

impl std::fmt::Display for ValidationReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for err in &self.errors {
            writeln!(f, "error[{}]: {}", err.kind.code(), err.kind.message())?;
            if let Some(hint) = err.kind.hint() {
                writeln!(f, "  hint: {}", hint)?;
            }
        }
        Ok(())
    }
}

impl std::error::Error for ValidationReport {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes() {
        assert_eq!(ErrorKind::UnknownVerb { name: "x".into(), suggestions: vec![] }.code(), "E001");
        assert_eq!(ErrorKind::MissingRequired { arg: "x", verb: "y", required_because: "z".into() }.code(), "E003");
    }

    #[test]
    fn test_format_plain() {
        let mut report = ValidationReport::new();
        report.push(ValidationError {
            span: Span::new(0, 5, 1, 1),
            kind: ErrorKind::UnknownVerb {
                name: "foo.bar".into(),
                suggestions: vec!["foo.baz"],
            },
        });

        let output = report.format_plain("(foo.bar)", "test.dsl");
        assert!(output.contains("E001"));
        assert!(output.contains("foo.bar"));
        assert!(output.contains("foo.baz"));
    }
}
