//! Vocabulary validation for DSL verb definitions
//!
//! This module provides validation capabilities for vocabulary operations,
//! ensuring verb definitions are correct and consistent.

use crate::ast::types::{ErrorSeverity, ValidationError, ValidationState};
use crate::vocabulary::{VocabularyCreateRequest, VocabularyError};

/// Validator for vocabulary operations
pub struct VocabularyValidator {
    validation_rules: Vec<ValidationRule>,
}

#[derive(Debug, Clone)]
pub struct ValidationRule {
    pub rule_name: String,
    pub rule_type: ValidationRuleType,
    pub validator: fn(&VocabularyCreateRequest) -> Result<(), ValidationError>,
}

#[derive(Debug, Clone)]
pub enum ValidationRuleType {
    Syntax,
    Semantic,
    Convention,
    Business,
}

impl VocabularyValidator {
    /// Create a new vocabulary validator
    pub fn new() -> Self {
        let mut validator = Self {
            validation_rules: Vec::new(),
        };

        validator.load_default_rules();
        validator
    }

    /// Load default validation rules
    fn load_default_rules(&mut self) {
        self.validation_rules.push(ValidationRule {
            rule_name: "verb_naming_convention".to_string(),
            rule_type: ValidationRuleType::Convention,
            validator: Self::validate_verb_naming,
        });

        self.validation_rules.push(ValidationRule {
            rule_name: "domain_exists".to_string(),
            rule_type: ValidationRuleType::Semantic,
            validator: Self::validate_domain_exists,
        });
    }

    /// Validate a vocabulary verb definition
    pub fn validate_verb_definition(
        &self,
        request: &VocabularyCreateRequest,
    ) -> Result<ValidationResult, VocabularyError> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Run all validation rules
        for rule in &self.validation_rules {
            match (rule.validator)(request) {
                Ok(()) => {
                    // Validation passed
                }
                Err(error) => match error.severity {
                    ErrorSeverity::Error => errors.push(error),
                    ErrorSeverity::Warning => warnings.push(error),
                    _ => warnings.push(error),
                },
            }
        }

        Ok(ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
        })
    }

    /// Validate verb naming convention
    fn validate_verb_naming(request: &VocabularyCreateRequest) -> Result<(), ValidationError> {
        // Check if verb follows domain.action pattern
        if !request.verb.contains('.') {
            return Err(ValidationError {
                code: "INVALID_VERB_FORMAT".to_string(),
                message: "Verb must follow 'domain.action' format".to_string(),
                severity: ErrorSeverity::Error,
                location: None,
                suggestions: vec![format!(
                    "Use format like '{}.{}'",
                    request.domain, request.verb
                )],
            });
        }

        // Check if verb starts with domain
        if !request.verb.starts_with(&format!("{}.", request.domain)) {
            return Err(ValidationError {
                code: "VERB_DOMAIN_MISMATCH".to_string(),
                message: "Verb must start with its domain".to_string(),
                severity: ErrorSeverity::Warning,
                location: None,
                suggestions: vec![format!(
                    "Consider using '{}.{}'",
                    request.domain,
                    request.verb.split('.').last().unwrap_or(&request.verb)
                )],
            });
        }

        Ok(())
    }

    /// Validate domain exists (stub implementation)
    fn validate_domain_exists(request: &VocabularyCreateRequest) -> Result<(), ValidationError> {
        // In a full implementation, this would check against available domains
        if request.domain.is_empty() {
            return Err(ValidationError {
                code: "EMPTY_DOMAIN".to_string(),
                message: "Domain cannot be empty".to_string(),
                severity: ErrorSeverity::Error,
                location: None,
                suggestions: vec!["Provide a valid domain name".to_string()],
            });
        }

        Ok(())
    }
}

/// Result of vocabulary validation
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationError>,
}

impl ValidationResult {
    pub fn is_valid(&self) -> bool {
        self.is_valid
    }

    pub fn get_errors(&self) -> Vec<ValidationError> {
        self.errors.clone()
    }

    pub fn get_warnings(&self) -> Vec<ValidationError> {
        self.warnings.clone()
    }
}

impl Default for VocabularyValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verb_naming_validation() {
        let valid_request = VocabularyCreateRequest {
            domain: "test".to_string(),
            verb: "test.action".to_string(),
            category: None,
            description: None,
            parameters: None,
            examples: None,
            phase: None,
            version: "1.0.0".to_string(),
        };

        let result = VocabularyValidator::validate_verb_naming(&valid_request);
        assert!(result.is_ok());

        let invalid_request = VocabularyCreateRequest {
            domain: "test".to_string(),
            verb: "action".to_string(), // Missing domain prefix
            category: None,
            description: None,
            parameters: None,
            examples: None,
            phase: None,
            version: "1.0.0".to_string(),
        };

        let result = VocabularyValidator::validate_verb_naming(&invalid_request);
        assert!(result.is_err());
    }

    #[test]
    fn test_validator_creation() {
        let validator = VocabularyValidator::new();
        assert!(!validator.validation_rules.is_empty());
    }
}
