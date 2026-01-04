//! Verb Contract Types
//!
//! Defines types for compiled verb contracts and compilation diagnostics.
//! These are used to store complete verb definitions in the database
//! for reproducibility and auditability.

use serde::{Deserialize, Serialize};

/// Diagnostics emitted during verb compilation/validation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VerbDiagnostics {
    /// Errors that prevent verb execution
    pub errors: Vec<VerbDiagnostic>,
    /// Warnings that should be addressed but don't block execution
    pub warnings: Vec<VerbDiagnostic>,
}

/// A single diagnostic message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbDiagnostic {
    /// Diagnostic code (e.g., "MISSING_LOOKUP_CONFIG")
    pub code: String,
    /// Human-readable message
    pub message: String,
    /// JSON path to the problematic field (e.g., "args[2].lookup")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Suggestion for how to fix the issue
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl VerbDiagnostics {
    /// Create empty diagnostics
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if there are any errors
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Check if there are any warnings
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    /// Check if there are any diagnostics at all
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty() && self.warnings.is_empty()
    }

    /// Total count of all diagnostics
    pub fn total_count(&self) -> usize {
        self.errors.len() + self.warnings.len()
    }

    /// Add an error diagnostic
    pub fn add_error(&mut self, code: &str, message: &str) {
        self.errors.push(VerbDiagnostic {
            code: code.to_string(),
            message: message.to_string(),
            path: None,
            hint: None,
        });
    }

    /// Add an error diagnostic with path
    pub fn add_error_at(&mut self, code: &str, message: &str, path: &str) {
        self.errors.push(VerbDiagnostic {
            code: code.to_string(),
            message: message.to_string(),
            path: Some(path.to_string()),
            hint: None,
        });
    }

    /// Add an error diagnostic with path and hint
    pub fn add_error_with_hint(&mut self, code: &str, message: &str, path: &str, hint: &str) {
        self.errors.push(VerbDiagnostic {
            code: code.to_string(),
            message: message.to_string(),
            path: Some(path.to_string()),
            hint: Some(hint.to_string()),
        });
    }

    /// Add an error diagnostic with optional path and hint
    pub fn add_error_with_path(
        &mut self,
        code: &str,
        message: &str,
        path: Option<&str>,
        hint: Option<&str>,
    ) {
        self.errors.push(VerbDiagnostic {
            code: code.to_string(),
            message: message.to_string(),
            path: path.map(|s| s.to_string()),
            hint: hint.map(|s| s.to_string()),
        });
    }

    /// Add a warning diagnostic
    pub fn add_warning(&mut self, code: &str, message: &str) {
        self.warnings.push(VerbDiagnostic {
            code: code.to_string(),
            message: message.to_string(),
            path: None,
            hint: None,
        });
    }

    /// Add a warning diagnostic with path
    pub fn add_warning_at(&mut self, code: &str, message: &str, path: &str) {
        self.warnings.push(VerbDiagnostic {
            code: code.to_string(),
            message: message.to_string(),
            path: Some(path.to_string()),
            hint: None,
        });
    }

    /// Add a warning diagnostic with path and hint
    pub fn add_warning_with_hint(&mut self, code: &str, message: &str, path: &str, hint: &str) {
        self.warnings.push(VerbDiagnostic {
            code: code.to_string(),
            message: message.to_string(),
            path: Some(path.to_string()),
            hint: Some(hint.to_string()),
        });
    }

    /// Add a warning diagnostic with optional path and hint
    pub fn add_warning_with_path(
        &mut self,
        code: &str,
        message: &str,
        path: Option<&str>,
        hint: Option<&str>,
    ) {
        self.warnings.push(VerbDiagnostic {
            code: code.to_string(),
            message: message.to_string(),
            path: path.map(|s| s.to_string()),
            hint: hint.map(|s| s.to_string()),
        });
    }

    /// Merge another diagnostics into this one
    pub fn merge(&mut self, other: VerbDiagnostics) {
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
    }
}

/// Known diagnostic codes for verb validation
pub mod codes {
    // Errors (block execution)
    pub const SERIALIZE_FAILED: &str = "SERIALIZE_FAILED";
    pub const LIFECYCLE_MISSING_ENTITY_ARG: &str = "LIFECYCLE_MISSING_ENTITY_ARG";
    pub const INVALID_CRUD_OPERATION: &str = "INVALID_CRUD_OPERATION";
    pub const MISSING_REQUIRED_FIELD: &str = "MISSING_REQUIRED_FIELD";
    pub const INVALID_TYPE: &str = "INVALID_TYPE";
    pub const CRUD_MISSING_TABLE: &str = "CRUD_MISSING_TABLE";

    // Warnings (should fix, but doesn't block)
    pub const LOOKUP_MISSING_ENTITY_TYPE: &str = "LOOKUP_MISSING_ENTITY_TYPE";
    pub const REQUIRED_WITH_DEFAULT: &str = "REQUIRED_WITH_DEFAULT";
    pub const UNUSED_PRODUCES: &str = "UNUSED_PRODUCES";
    pub const DEPRECATED_BEHAVIOR: &str = "DEPRECATED_BEHAVIOR";
    pub const MISSING_DESCRIPTION: &str = "MISSING_DESCRIPTION";
    pub const PRODUCES_EMPTY_TYPE: &str = "PRODUCES_EMPTY_TYPE";
    pub const PLUGIN_EMPTY_HANDLER: &str = "PLUGIN_EMPTY_HANDLER";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_diagnostics() {
        let diag = VerbDiagnostics::new();
        assert!(!diag.has_errors());
        assert!(!diag.has_warnings());
        assert!(diag.is_empty());
        assert_eq!(diag.total_count(), 0);
    }

    #[test]
    fn test_add_error() {
        let mut diag = VerbDiagnostics::new();
        diag.add_error("TEST_ERROR", "Something went wrong");

        assert!(diag.has_errors());
        assert!(!diag.has_warnings());
        assert!(!diag.is_empty());
        assert_eq!(diag.total_count(), 1);
    }

    #[test]
    fn test_add_warning() {
        let mut diag = VerbDiagnostics::new();
        diag.add_warning("TEST_WARN", "Consider fixing this");

        assert!(!diag.has_errors());
        assert!(diag.has_warnings());
        assert!(!diag.is_empty());
        assert_eq!(diag.total_count(), 1);
    }

    #[test]
    fn test_merge() {
        let mut diag1 = VerbDiagnostics::new();
        diag1.add_error("E1", "Error 1");

        let mut diag2 = VerbDiagnostics::new();
        diag2.add_warning("W1", "Warning 1");
        diag2.add_error("E2", "Error 2");

        diag1.merge(diag2);

        assert_eq!(diag1.errors.len(), 2);
        assert_eq!(diag1.warnings.len(), 1);
        assert_eq!(diag1.total_count(), 3);
    }

    #[test]
    fn test_serialization() {
        let mut diag = VerbDiagnostics::new();
        diag.add_error_with_hint("TEST", "Message", "args[0].type", "Try using 'string'");

        let json = serde_json::to_string(&diag).unwrap();
        let recovered: VerbDiagnostics = serde_json::from_str(&json).unwrap();

        assert_eq!(recovered.errors.len(), 1);
        assert_eq!(recovered.errors[0].code, "TEST");
        assert_eq!(recovered.errors[0].hint, Some("Try using 'string'".into()));
    }
}
