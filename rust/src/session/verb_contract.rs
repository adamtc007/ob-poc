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

    // =========================================================================
    // MINIMAL tier rules (M0xx) - Required fields present
    // =========================================================================
    /// M001: Verb missing metadata block entirely
    pub const M001_MISSING_METADATA: &str = "M001";
    /// M002: Verb metadata missing tier field
    pub const M002_MISSING_TIER: &str = "M002";
    /// M003: Verb metadata missing source_of_truth field
    pub const M003_MISSING_SOURCE: &str = "M003";
    /// M004: Verb metadata missing scope field
    pub const M004_MISSING_SCOPE: &str = "M004";
    /// M005: Verb metadata missing noun field
    pub const M005_MISSING_NOUN: &str = "M005";
    /// M006: Deprecated verb missing replaced_by field
    pub const M006_DEPRECATED_NO_REPLACEMENT: &str = "M006";

    // =========================================================================
    // BASIC tier rules (B0xx) - Naming + semantics
    // =========================================================================
    /// B001: create-* verb should use insert operation
    pub const B001_CREATE_NOT_INSERT: &str = "B001";
    /// B002: ensure-* verb should use upsert operation
    pub const B002_ENSURE_NOT_UPSERT: &str = "B002";
    /// B003: delete-* on regulated noun missing dangerous: true
    pub const B003_DELETE_NOT_DANGEROUS: &str = "B003";
    /// B004: Deprecated verb has invalid replaced_by target
    pub const B004_INVALID_REPLACEMENT: &str = "B004";
    /// B005: list-* verb should be tier: diagnostics
    pub const B005_LIST_NOT_DIAGNOSTICS: &str = "B005";
    /// B006: get-* verb should be tier: diagnostics
    pub const B006_GET_NOT_DIAGNOSTICS: &str = "B006";

    // =========================================================================
    // STANDARD tier rules (S0xx) - Matrix-first enforcement
    // =========================================================================
    /// S001: Multiple intent verbs for same noun (single authoring surface)
    pub const S001_DUPLICATE_INTENT: &str = "S001";
    /// S002: writes_operational requires tier: projection or composite
    pub const S002_WRITES_OP_WRONG_TIER: &str = "S002";
    /// S003: projection + writes_operational requires internal: true
    pub const S003_PROJECTION_NOT_INTERNAL: &str = "S003";

    // =========================================================================
    // Legacy T0xx codes (kept for backward compatibility, mapped to new codes)
    // =========================================================================
    /// Verb writes to operational table but is not tier: projection or composite
    pub const TIER_WRITE_NOT_PROJECTION: &str = "T001";
    /// Verb is tier: projection but missing internal: true
    pub const TIER_PROJECTION_NOT_INTERNAL: &str = "T002";
    /// Verb marked deprecated but missing tier: projection
    pub const TIER_DEPRECATED_NOT_PROJECTION: &str = "T003";
    /// Verb has writes_operational: true but behavior is not CRUD insert/upsert/update/delete
    pub const TIER_WRITES_OP_MISMATCH: &str = "T004";
    /// Verb is tier: intent but writes to operational tables (should use materialize)
    pub const TIER_INTENT_WRITES_OPERATIONAL: &str = "T005";
    /// Verb is tier: diagnostics but has write behavior
    pub const TIER_DIAGNOSTICS_HAS_WRITE: &str = "T006";
    /// Verb missing metadata (no tiering info) - now M001
    pub const TIER_MISSING_METADATA: &str = "M001";
    /// Verb has inconsistent source_of_truth
    pub const TIER_INCONSISTENT_SOURCE: &str = "T008";
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
