//! Grammar validator for EBNF rule validation
//!
//! This module provides validation capabilities for grammar rules,
//! ensuring EBNF definitions are syntactically and semantically correct.

use crate::ast::types::{ErrorSeverity, ValidationError, ValidationState};
use crate::grammar::ebnf_parser::{EBNFError, EBNFRule};

/// Validator for grammar operations
pub struct GrammarValidator {
    validation_rules: Vec<GrammarValidationRule>,
}

#[derive(Debug, Clone)]
pub struct GrammarValidationRule {
    pub rule_name: String,
    pub rule_type: GrammarValidationRuleType,
    pub validator: fn(&str) -> Result<(), ValidationError>,
}

#[derive(Debug, Clone)]
pub enum GrammarValidationRuleType {
    Syntax,
    Semantic,
    Structure,
    Performance,
}

impl GrammarValidator {
    /// Create a new grammar validator
    pub fn new() -> Self {
        let mut validator = Self {
            validation_rules: Vec::new(),
        };

        validator.load_default_rules();
        validator
    }

    /// Load default validation rules
    fn load_default_rules(&mut self) {
        self.validation_rules.push(GrammarValidationRule {
            rule_name: "balanced_parentheses".to_string(),
            rule_type: GrammarValidationRuleType::Syntax,
            validator: Self::validate_balanced_parentheses,
        });

        self.validation_rules.push(GrammarValidationRule {
            rule_name: "valid_ebnf_syntax".to_string(),
            rule_type: GrammarValidationRuleType::Syntax,
            validator: Self::validate_ebnf_syntax,
        });

        self.validation_rules.push(GrammarValidationRule {
            rule_name: "no_empty_productions".to_string(),
            rule_type: GrammarValidationRuleType::Semantic,
            validator: Self::validate_no_empty_productions,
        });
    }

    /// Validate a grammar rule definition
    pub fn validate_rule_definition(
        &self,
        rule_definition: &str,
    ) -> Result<GrammarValidationResult, EBNFError> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Run all validation rules
        for rule in &self.validation_rules {
            match (rule.validator)(rule_definition) {
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

        Ok(GrammarValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
        })
    }

    /// Validate a parsed EBNF rule
    pub fn validate_parsed_rule(
        &self,
        _rule: &EBNFRule,
    ) -> Result<GrammarValidationResult, EBNFError> {
        // In a full implementation, this would validate the parsed AST
        Ok(GrammarValidationResult {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        })
    }

    /// Validate balanced parentheses in rule definition
    fn validate_balanced_parentheses(definition: &str) -> Result<(), ValidationError> {
        let mut balance = 0;
        let mut bracket_balance = 0;
        let mut brace_balance = 0;

        for (i, ch) in definition.chars().enumerate() {
            match ch {
                '(' => balance += 1,
                ')' => {
                    balance -= 1;
                    if balance < 0 {
                        return Err(ValidationError {
                            code: "UNBALANCED_PARENTHESES".to_string(),
                            message: format!("Unmatched closing parenthesis at position {}", i),
                            severity: ErrorSeverity::Error,
                            location: None,
                            suggestions: vec!["Check parentheses pairing".to_string()],
                        });
                    }
                }
                '[' => bracket_balance += 1,
                ']' => {
                    bracket_balance -= 1;
                    if bracket_balance < 0 {
                        return Err(ValidationError {
                            code: "UNBALANCED_BRACKETS".to_string(),
                            message: format!("Unmatched closing bracket at position {}", i),
                            severity: ErrorSeverity::Error,
                            location: None,
                            suggestions: vec!["Check bracket pairing".to_string()],
                        });
                    }
                }
                '{' => brace_balance += 1,
                '}' => {
                    brace_balance -= 1;
                    if brace_balance < 0 {
                        return Err(ValidationError {
                            code: "UNBALANCED_BRACES".to_string(),
                            message: format!("Unmatched closing brace at position {}", i),
                            severity: ErrorSeverity::Error,
                            location: None,
                            suggestions: vec!["Check brace pairing".to_string()],
                        });
                    }
                }
                _ => {}
            }
        }

        if balance != 0 {
            return Err(ValidationError {
                code: "UNBALANCED_PARENTHESES".to_string(),
                message: "Unbalanced parentheses in rule definition".to_string(),
                severity: ErrorSeverity::Error,
                location: None,
                suggestions: vec!["Ensure all parentheses are properly paired".to_string()],
            });
        }

        if bracket_balance != 0 {
            return Err(ValidationError {
                code: "UNBALANCED_BRACKETS".to_string(),
                message: "Unbalanced brackets in rule definition".to_string(),
                severity: ErrorSeverity::Error,
                location: None,
                suggestions: vec!["Ensure all brackets are properly paired".to_string()],
            });
        }

        if brace_balance != 0 {
            return Err(ValidationError {
                code: "UNBALANCED_BRACES".to_string(),
                message: "Unbalanced braces in rule definition".to_string(),
                severity: ErrorSeverity::Error,
                location: None,
                suggestions: vec!["Ensure all braces are properly paired".to_string()],
            });
        }

        Ok(())
    }

    /// Validate EBNF syntax
    fn validate_ebnf_syntax(definition: &str) -> Result<(), ValidationError> {
        // Check for required ::= operator
        if !definition.contains("::=") {
            return Err(ValidationError {
                code: "MISSING_PRODUCTION_OPERATOR".to_string(),
                message: "Rule definition must contain '::=' operator".to_string(),
                severity: ErrorSeverity::Error,
                location: None,
                suggestions: vec!["Add '::=' between rule name and definition".to_string()],
            });
        }

        // Check for empty rule name
        let parts: Vec<&str> = definition.splitn(2, "::=").collect();
        if parts.len() < 2 {
            return Err(ValidationError {
                code: "INVALID_RULE_STRUCTURE".to_string(),
                message: "Invalid rule structure".to_string(),
                severity: ErrorSeverity::Error,
                location: None,
                suggestions: vec!["Use format: rule_name ::= definition".to_string()],
            });
        }

        let rule_name = parts[0].trim();
        if rule_name.is_empty() {
            return Err(ValidationError {
                code: "EMPTY_RULE_NAME".to_string(),
                message: "Rule name cannot be empty".to_string(),
                severity: ErrorSeverity::Error,
                location: None,
                suggestions: vec!["Provide a valid rule name".to_string()],
            });
        }

        // Check for valid rule name format
        if !rule_name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(ValidationError {
                code: "INVALID_RULE_NAME".to_string(),
                message: "Rule name contains invalid characters".to_string(),
                severity: ErrorSeverity::Warning,
                location: None,
                suggestions: vec![
                    "Use only alphanumeric characters, underscores, and hyphens".to_string()
                ],
            });
        }

        Ok(())
    }

    /// Validate no empty productions
    fn validate_no_empty_productions(definition: &str) -> Result<(), ValidationError> {
        let parts: Vec<&str> = definition.splitn(2, "::=").collect();
        if parts.len() >= 2 {
            let production = parts[1].trim();
            if production.is_empty() {
                return Err(ValidationError {
                    code: "EMPTY_PRODUCTION".to_string(),
                    message: "Production cannot be empty".to_string(),
                    severity: ErrorSeverity::Error,
                    location: None,
                    suggestions: vec!["Provide a valid production definition".to_string()],
                });
            }
        }

        Ok(())
    }
}

/// Result of grammar validation
pub struct GrammarValidationResult {
    pub is_valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationError>,
}

impl GrammarValidationResult {
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

impl Default for GrammarValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_balanced_parentheses_validation() {
        // Valid cases
        assert!(GrammarValidator::validate_balanced_parentheses("(a | b)").is_ok());
        assert!(
            GrammarValidator::validate_balanced_parentheses("[optional] {zero_or_more}").is_ok()
        );

        // Invalid cases
        assert!(GrammarValidator::validate_balanced_parentheses("(unmatched").is_err());
        assert!(GrammarValidator::validate_balanced_parentheses("unmatched)").is_err());
        assert!(GrammarValidator::validate_balanced_parentheses("[unmatched").is_err());
        assert!(GrammarValidator::validate_balanced_parentheses("{unmatched").is_err());
    }

    #[test]
    fn test_ebnf_syntax_validation() {
        // Valid cases
        assert!(GrammarValidator::validate_ebnf_syntax("rule ::= \"terminal\"").is_ok());
        assert!(GrammarValidator::validate_ebnf_syntax("my-rule ::= other_rule").is_ok());

        // Invalid cases
        assert!(GrammarValidator::validate_ebnf_syntax("rule = \"terminal\"").is_err()); // Missing ::=
        assert!(GrammarValidator::validate_ebnf_syntax("::= \"terminal\"").is_err());
        // Empty rule name
    }

    #[test]
    fn test_empty_production_validation() {
        // Valid case
        assert!(GrammarValidator::validate_no_empty_productions("rule ::= \"terminal\"").is_ok());

        // Invalid case
        assert!(GrammarValidator::validate_no_empty_productions("rule ::=").is_err());
        assert!(GrammarValidator::validate_no_empty_productions("rule ::=   ").is_err());
    }

    #[test]
    fn test_validator_creation() {
        let validator = GrammarValidator::new();
        assert!(!validator.validation_rules.is_empty());
    }

    #[test]
    fn test_rule_definition_validation() {
        let validator = GrammarValidator::new();

        // Valid rule
        let result = validator.validate_rule_definition("identifier ::= [a-zA-Z][a-zA-Z0-9]*");
        assert!(result.is_ok());
        assert!(result.unwrap().is_valid());

        // Invalid rule
        let result = validator.validate_rule_definition("invalid rule");
        assert!(result.is_ok());
        assert!(!result.unwrap().is_valid());
    }
}
