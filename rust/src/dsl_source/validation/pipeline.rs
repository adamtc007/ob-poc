//! Multi-stage validation pipeline for generated DSL
//!
//! Validates DSL against Runtime vocabulary (in-memory) rather than
//! the deprecated vocabulary_registry DB table.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::forth_engine::ast::DslParser;
use crate::forth_engine::parser_nom::NomDslParser;
use crate::forth_engine::runtime::Runtime;
use crate::forth_engine::vocab_registry::create_standard_runtime;

pub struct ValidationPipeline {
    runtime: Runtime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<String>,
    pub stage_reached: ValidationStage,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ValidationStage {
    Syntax,
    Semantic,
    BusinessRules,
    Complete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationError {
    SyntaxError {
        position: usize,
        message: String,
    },
    UnknownVerb {
        verb: String,
        suggestions: Vec<String>,
    },
    InvalidAttribute {
        attr: String,
        reason: String,
    },
    BusinessRuleViolation {
        rule: String,
        message: String,
    },
}

impl ValidationPipeline {
    /// Create a new ValidationPipeline with default runtime
    /// Note: pool parameter kept for API compatibility but unused (validation is in-memory)
    pub fn new(_pool: PgPool) -> Self {
        Self {
            runtime: create_standard_runtime(),
        }
    }

    /// Create with explicit Runtime
    pub fn with_runtime(_pool: PgPool, runtime: Runtime) -> Self {
        Self { runtime }
    }

    pub async fn validate(&self, dsl_text: &str) -> Result<ValidationResult> {
        let mut result = ValidationResult {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            stage_reached: ValidationStage::Syntax,
        };

        // Stage 1: Syntax validation
        match self.validate_syntax(dsl_text) {
            Ok(_) => result.stage_reached = ValidationStage::Semantic,
            Err(e) => {
                result.is_valid = false;
                result.errors.push(ValidationError::SyntaxError {
                    position: 0,
                    message: e.to_string(),
                });
                return Ok(result);
            }
        }

        // Stage 2: Semantic validation
        match self.validate_semantics(dsl_text).await {
            Ok(warnings) => {
                result.warnings.extend(warnings);
                result.stage_reached = ValidationStage::BusinessRules;
            }
            Err(e) => {
                result.is_valid = false;
                result.errors.push(ValidationError::UnknownVerb {
                    verb: "unknown".to_string(),
                    suggestions: vec![],
                });
                result.warnings.push(e.to_string());
                return Ok(result);
            }
        }

        // Stage 3: Business rules validation
        match self.validate_business_rules(dsl_text).await {
            Ok(warnings) => {
                result.warnings.extend(warnings);
                result.stage_reached = ValidationStage::Complete;
            }
            Err(e) => {
                result.is_valid = false;
                result.errors.push(ValidationError::BusinessRuleViolation {
                    rule: "unknown".to_string(),
                    message: e.to_string(),
                });
                return Ok(result);
            }
        }

        Ok(result)
    }

    fn validate_syntax(&self, dsl_text: &str) -> Result<()> {
        let parser = NomDslParser::new();
        let _parsed = parser.parse(dsl_text).context("Syntax validation failed")?;
        Ok(())
    }

    async fn validate_semantics(&self, dsl_text: &str) -> Result<Vec<String>> {
        let mut warnings = Vec::new();

        // Extract verbs from DSL
        let verbs = self.extract_verbs(dsl_text);

        // Check each verb against Runtime vocabulary (not DB)
        for verb in verbs {
            if self.runtime.get_word(&verb).is_none() {
                // Suggest similar verbs
                let suggestions = self.find_similar_verbs(&verb);
                if suggestions.is_empty() {
                    warnings.push(format!("Unknown verb '{}'", verb));
                } else {
                    warnings.push(format!(
                        "Unknown verb '{}'. Did you mean: {}?",
                        verb,
                        suggestions.join(", ")
                    ));
                }
            }
        }

        Ok(warnings)
    }

    /// Find similar verbs for suggestions
    fn find_similar_verbs(&self, verb: &str) -> Vec<String> {
        let all_verbs = self.runtime.get_all_word_names();

        // Simple prefix matching
        let prefix = verb.split('.').next().unwrap_or(verb);
        all_verbs
            .iter()
            .filter(|v| v.starts_with(prefix))
            .take(3)
            .map(|v| v.to_string())
            .collect()
    }

    async fn validate_business_rules(&self, dsl_text: &str) -> Result<Vec<String>> {
        let mut warnings = Vec::new();

        // Check for common business rule patterns

        // Rule 1: CBU operations should have cbu-id or cbu-name
        if dsl_text.contains("cbu.")
            && !dsl_text.contains(":cbu-id")
            && !dsl_text.contains(":cbu-name")
        {
            warnings.push("CBU operations typically require :cbu-id or :cbu-name".to_string());
        }

        // Rule 2: Document operations should have doc-id
        if dsl_text.contains("document.") && !dsl_text.contains(":doc-id") {
            warnings.push("Document operations typically require :doc-id".to_string());
        }

        // Rule 3: Entity declarations should have entity-type
        if dsl_text.contains("declare-entity") && !dsl_text.contains(":entity-type") {
            warnings.push("Entity declarations should specify :entity-type".to_string());
        }

        Ok(warnings)
    }

    /// Extract verb names from DSL text
    pub fn extract_verbs(&self, dsl_text: &str) -> Vec<String> {
        Self::extract_verbs_from_text(dsl_text)
    }

    /// Static helper to extract verbs without requiring a ValidationPipeline instance
    pub fn extract_verbs_from_text(dsl_text: &str) -> Vec<String> {
        let mut verbs = Vec::new();
        let mut chars = dsl_text.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '(' {
                // Skip whitespace
                while chars.peek().map(|c| c.is_whitespace()).unwrap_or(false) {
                    chars.next();
                }

                // Collect verb name
                let mut verb = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_whitespace() || c == ')' || c == '(' {
                        break;
                    }
                    verb.push(c);
                    chars.next();
                }

                if !verb.is_empty() && verb.contains('.') {
                    verbs.push(verb);
                }
            }
        }

        verbs
    }

    pub fn format_error_for_llm(&self, error: &ValidationError) -> String {
        Self::format_error(error)
    }

    /// Static helper to format errors without requiring a ValidationPipeline instance
    pub fn format_error(error: &ValidationError) -> String {
        match error {
            ValidationError::SyntaxError { position, message } => {
                format!("Syntax error at position {}: {}", position, message)
            }
            ValidationError::UnknownVerb { verb, suggestions } => {
                if suggestions.is_empty() {
                    format!("Unknown verb '{}'", verb)
                } else {
                    format!(
                        "Unknown verb '{}'. Did you mean: {}?",
                        verb,
                        suggestions.join(", ")
                    )
                }
            }
            ValidationError::InvalidAttribute { attr, reason } => {
                format!("Invalid attribute '{}': {}", attr, reason)
            }
            ValidationError::BusinessRuleViolation { rule, message } => {
                format!("Business rule '{}' violated: {}", rule, message)
            }
        }
    }

    pub fn format_errors_for_llm(&self, result: &ValidationResult) -> String {
        result
            .errors
            .iter()
            .map(|e| self.format_error_for_llm(e))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_verbs() {
        let dsl = "(cbu.create :cbu-name \"Test\") (document.catalog :doc-id \"123\")";
        let verbs = ValidationPipeline::extract_verbs_from_text(dsl);

        assert_eq!(verbs.len(), 2);
        assert!(verbs.contains(&"cbu.create".to_string()));
        assert!(verbs.contains(&"document.catalog".to_string()));
    }

    #[test]
    fn test_extract_nested_verbs() {
        let dsl = "(kyc.declare-entity :entity-type \"PERSON\" :name \"John\")";
        let verbs = ValidationPipeline::extract_verbs_from_text(dsl);

        assert_eq!(verbs.len(), 1);
        assert_eq!(verbs[0], "kyc.declare-entity");
    }

    #[test]
    fn test_format_errors() {
        let error = ValidationError::SyntaxError {
            position: 10,
            message: "Unexpected token".to_string(),
        };

        let formatted = ValidationPipeline::format_error(&error);
        assert!(formatted.contains("position 10"));
        assert!(formatted.contains("Unexpected token"));
    }

    #[tokio::test]
    #[ignore] // Requires database
    async fn test_validate_syntax() {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://localhost/data_designer".to_string());
        let pool = sqlx::PgPool::connect(&database_url).await.unwrap();

        let pipeline = ValidationPipeline::new(pool);

        // Valid DSL
        let result = pipeline
            .validate("(cbu.create :cbu-name \"Test\")")
            .await
            .unwrap();
        assert!(result.is_valid);
        assert_eq!(result.stage_reached, ValidationStage::Complete);

        // Invalid DSL - missing closing paren
        let result = pipeline
            .validate("(cbu.create :cbu-name \"Test\"")
            .await
            .unwrap();
        assert!(!result.is_valid);
        assert_eq!(result.stage_reached, ValidationStage::Syntax);
    }
}
