//! Diagnostic types for lint results
//!
//! This module defines the types used to report lint errors, warnings, and info.

use serde::{Deserialize, Serialize};

/// Severity level for diagnostics
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Informational message (does not fail build)
    Info,
    /// Warning (does not fail build, but should be addressed)
    Warn,
    /// Error (fails build)
    Error,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Info => write!(f, "info"),
            Severity::Warn => write!(f, "warn"),
            Severity::Error => write!(f, "error"),
        }
    }
}

/// A diagnostic message from lint
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Rule code (e.g., "MACRO012")
    pub code: String,
    /// Severity level
    pub severity: Severity,
    /// Path within YAML (e.g., "structure.setup.ui.label")
    pub path: String,
    /// Human-readable message
    pub message: String,
    /// Optional hint for fixing the issue
    pub hint: Option<String>,
}

impl Diagnostic {
    /// Create an error diagnostic
    pub fn error(
        code: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            severity: Severity::Error,
            path: path.into(),
            message: message.into(),
            hint: None,
        }
    }

    /// Create a warning diagnostic
    pub fn warn(
        code: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            severity: Severity::Warn,
            path: path.into(),
            message: message.into(),
            hint: None,
        }
    }

    /// Create an info diagnostic
    pub fn info(
        code: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            severity: Severity::Info,
            path: path.into(),
            message: message.into(),
            hint: None,
        }
    }

    /// Add a hint to this diagnostic
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Check if this is an error
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }

    /// Check if this is a warning
    pub fn is_warning(&self) -> bool {
        self.severity == Severity::Warn
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {} at {}: {}",
            self.code, self.severity, self.path, self.message
        )?;
        if let Some(hint) = &self.hint {
            write!(f, " (hint: {})", hint)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_creation() {
        let d = Diagnostic::error("MACRO001", "$.verbs", "Missing required field");
        assert_eq!(d.code, "MACRO001");
        assert_eq!(d.severity, Severity::Error);
        assert_eq!(d.path, "$.verbs");
        assert!(d.is_error());
        assert!(!d.is_warning());
    }

    #[test]
    fn test_diagnostic_with_hint() {
        let d = Diagnostic::warn("MACRO080a", "$.args.entity", "Missing autofill_from")
            .with_hint("Add autofill_from: [session.current_structure]");
        assert!(d.hint.is_some());
        assert_eq!(
            d.hint.unwrap(),
            "Add autofill_from: [session.current_structure]"
        );
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Error > Severity::Warn);
        assert!(Severity::Warn > Severity::Info);
    }
}
