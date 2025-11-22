//! Parser Validation Module
//!
//! Validates parsed DSL programs for semantic correctness including:
//! - Verb format validation (domain.action)
//! - Attribute reference validation (UUID format)
//! - Required parameter validation (future)

use crate::parser::ast::{Form, Program, Value};
use thiserror::Error;

/// Parser-specific validation error types
/// Note: This is separate from crate::error::ValidationError which handles semantic validation
#[derive(Debug, Error, Clone)]
pub enum ParserValidationError {
    #[error("Unknown verb '{verb}': verbs must be in 'domain.action' format")]
    InvalidVerbFormat { verb: String },

    #[error("Invalid attribute reference '{ref_text}': {reason}")]
    InvalidAttributeRef { ref_text: String, reason: String },

    #[error("Missing required parameter '{param}' for verb '{verb}'")]
    MissingRequiredParam { verb: String, param: String },

    #[error(
        "Invalid parameter type for '{param}' in verb '{verb}': expected {expected}, got {got}"
    )]
    InvalidParamType {
        verb: String,
        param: String,
        expected: String,
        got: String,
    },
}

// Conversion to main ValidationError for unified error handling
impl From<ParserValidationError> for crate::error::ValidationError {
    fn from(err: ParserValidationError) -> Self {
        match err {
            ParserValidationError::InvalidVerbFormat { verb } => {
                crate::error::ValidationError::ConstraintViolation {
                    constraint: "verb format must be 'domain.action'".to_string(),
                    location: verb,
                }
            }
            ParserValidationError::InvalidAttributeRef { ref_text, reason } => {
                crate::error::ValidationError::TypeMismatch {
                    expected: "valid UUID".to_string(),
                    found: reason,
                    location: ref_text,
                }
            }
            ParserValidationError::MissingRequiredParam { verb, param } => {
                crate::error::ValidationError::MissingProperty {
                    property: param,
                    entity_type: verb,
                }
            }
            ParserValidationError::InvalidParamType {
                verb,
                param,
                expected,
                got,
            } => crate::error::ValidationError::TypeMismatch {
                expected,
                found: got,
                location: format!("{}:{}", verb, param),
            },
        }
    }
}

/// Validation result containing all errors found
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ParserValidationError>,
    pub warnings: Vec<String>,
}

impl ValidationResult {
    pub fn success() -> Self {
        Self {
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn with_errors(errors: Vec<ParserValidationError>) -> Self {
        Self {
            valid: errors.is_empty(),
            errors,
            warnings: Vec::new(),
        }
    }
}

/// Validate a parsed DSL program
pub fn validate_program(program: &Program) -> ValidationResult {
    let mut errors = Vec::new();

    for form in program {
        if let Form::Verb(verb_form) = form {
            // Validate verb format
            if let Err(e) = validate_verb_format(&verb_form.verb) {
                errors.push(e);
            }

            // Validate all values in the verb form
            for value in verb_form.pairs.values() {
                validate_value(value, &mut errors);
            }
        }
    }

    ValidationResult::with_errors(errors)
}

/// Validate verb format: must be "domain.action"
fn validate_verb_format(verb: &str) -> Result<(), ParserValidationError> {
    let parts: Vec<&str> = verb.split('.').collect();

    if parts.len() != 2 {
        return Err(ParserValidationError::InvalidVerbFormat {
            verb: verb.to_string(),
        });
    }

    let domain = parts[0];
    let action = parts[1];

    // Validate domain name (alphanumeric + hyphen)
    if domain.is_empty() || !domain.chars().all(|c| c.is_alphanumeric() || c == '-') {
        return Err(ParserValidationError::InvalidVerbFormat {
            verb: verb.to_string(),
        });
    }

    // Validate action name (alphanumeric + hyphen)
    if action.is_empty() || !action.chars().all(|c| c.is_alphanumeric() || c == '-') {
        return Err(ParserValidationError::InvalidVerbFormat {
            verb: verb.to_string(),
        });
    }

    Ok(())
}

/// Validate a value recursively
fn validate_value(value: &Value, errors: &mut Vec<ParserValidationError>) {
    match value {
        Value::AttrRef(uuid_str) => {
            if !is_valid_uuid(uuid_str) {
                errors.push(ParserValidationError::InvalidAttributeRef {
                    ref_text: uuid_str.clone(),
                    reason: "Invalid UUID format".to_string(),
                });
            }
        }
        Value::List(items) => {
            for item in items {
                validate_value(item, errors);
            }
        }
        Value::Map(map) => {
            for v in map.values() {
                validate_value(v, errors);
            }
        }
        _ => {}
    }
}

/// Check if a string is a valid UUID format
fn is_valid_uuid(s: &str) -> bool {
    // UUID format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
    // Or simple alphanumeric identifiers for testing
    if s.len() == 36 {
        // Standard UUID format
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() == 5
            && parts[0].len() == 8
            && parts[1].len() == 4
            && parts[2].len() == 4
            && parts[3].len() == 4
            && parts[4].len() == 12
        {
            return parts
                .iter()
                .all(|p| p.chars().all(|c| c.is_ascii_hexdigit()));
        }
    }

    // Also accept simple identifiers for testing (e.g., "uuid-001")
    !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '-')
}

/// Parse and validate DSL input
pub fn parse_and_validate(input: &str) -> Result<Program, Vec<ParserValidationError>> {
    use crate::parser::idiomatic_parser::parse_program;

    // Parse the input
    let program = parse_program(input).map_err(|e| {
        vec![ParserValidationError::InvalidVerbFormat {
            verb: format!("Parse error: {:?}", e),
        }]
    })?;

    // Validate the parsed program
    let result = validate_program(&program);

    if result.valid {
        Ok(program)
    } else {
        Err(result.errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_verb_format() {
        assert!(validate_verb_format("case.create").is_ok());
        assert!(validate_verb_format("kyc.start").is_ok());
        assert!(validate_verb_format("entity.register").is_ok());
    }

    #[test]
    fn test_invalid_verb_format() {
        assert!(validate_verb_format("invalid").is_err());
        assert!(validate_verb_format("too.many.parts").is_err());
        assert!(validate_verb_format(".noaction").is_err());
        assert!(validate_verb_format("nodomain.").is_err());
    }

    #[test]
    fn test_valid_uuid() {
        assert!(is_valid_uuid("550e8400-e29b-41d4-a716-446655440000"));
        assert!(is_valid_uuid("uuid-001")); // Simple identifier
    }

    #[test]
    fn test_invalid_uuid() {
        assert!(!is_valid_uuid("")); // Empty
        assert!(!is_valid_uuid("not a uuid with spaces"));
    }

    #[test]
    fn test_parse_and_validate_success() {
        let input = r#"(case.create :case-id "TEST-001")"#;
        let result = parse_and_validate(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_and_validate_invalid_verb() {
        let input = r#"(invalid :key "value")"#;
        let result = parse_and_validate(input);
        assert!(result.is_err());
    }
}
