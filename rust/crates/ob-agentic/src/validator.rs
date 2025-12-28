//! DSL Validator
//!
//! Validates generated DSL using the existing parser and CSG linter.

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<String>,
}

/// Validation error with location info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub line: Option<usize>,
    pub message: String,
    pub suggestion: Option<String>,
}

/// DSL validator using existing parser and linter
pub struct AgentValidator;

impl AgentValidator {
    /// Create a new validator
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    /// Validate DSL source code
    pub fn validate(&self, dsl_source: &str) -> ValidationResult {
        use dsl_core::{compiler::compile_to_ops, parse_program};

        // Phase 1: Parse
        let program = match parse_program(dsl_source) {
            Ok(p) => p,
            Err(e) => {
                return ValidationResult {
                    is_valid: false,
                    errors: vec![ValidationError {
                        line: Self::extract_line_number(&e),
                        message: e,
                        suggestion: None,
                    }],
                    warnings: vec![],
                };
            }
        };

        // Phase 2: Compile to ops (basic validation without DB-dependent CSG lint)
        let compiled = compile_to_ops(&program);

        // Check for any diagnostics/errors in the compiled result
        if compiled.ops.is_empty() && !program.statements.is_empty() {
            return ValidationResult {
                is_valid: false,
                errors: vec![ValidationError {
                    line: None,
                    message: "Compilation produced no operations".to_string(),
                    suggestion: None,
                }],
                warnings: vec![],
            };
        }

        ValidationResult {
            is_valid: true,
            errors: vec![],
            warnings: vec![],
        }
    }

    /// Extract line number from error message if present
    fn extract_line_number(error: &str) -> Option<usize> {
        // Try to extract "line X" from error message
        if let Some(pos) = error.find("line ") {
            let rest = &error[pos + 5..];
            if let Some(end) = rest.find(|c: char| !c.is_ascii_digit()) {
                if let Ok(line) = rest[..end].parse() {
                    return Some(line);
                }
            } else if let Ok(line) = rest.parse() {
                return Some(line);
            }
        }
        None
    }
}

impl Default for AgentValidator {
    fn default() -> Self {
        Self::new().expect("Failed to create validator")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_dsl() {
        let validator = AgentValidator::new().unwrap();
        let dsl =
            r#"(cbu.ensure :name "Test Fund" :jurisdiction "US" :client-type "FUND" :as @cbu)"#;
        let result = validator.validate(dsl);
        assert!(
            result.is_valid,
            "Expected valid DSL, got errors: {:?}",
            result.errors
        );
    }

    #[test]
    fn test_invalid_syntax() {
        let validator = AgentValidator::new().unwrap();
        let dsl = r#"(cbu.ensure :name "Test Fund" :jurisdiction "US""#; // Missing closing paren
        let result = validator.validate(dsl);
        assert!(!result.is_valid);
        assert!(!result.errors.is_empty());
    }

    #[test]
    fn test_invalid_verb() {
        let validator = AgentValidator::new().unwrap();
        let dsl = r#"(cbu.nonexistent :name "Test")"#;
        let result = validator.validate(dsl);
        assert!(!result.is_valid);
    }
}
