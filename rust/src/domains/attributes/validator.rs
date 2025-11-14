//! Attribute DSL Validator
//!
//! This module provides validation for attribute references in DSL,
//! ensuring that all referenced attributes exist and are used correctly.

use crate::domains::attributes::types::{AttributeCategory, AttributeMetadata, DataType};
use crate::parser_ast::{Form, Program, Value, VerbForm};
use std::collections::HashMap;

/// Validator for attribute references in DSL
#[derive(Debug, Clone)]
pub struct AttributeValidator {
    /// Registry of all known attributes
    registry: HashMap<String, AttributeMetadata>,
}

impl AttributeValidator {
    /// Create a new validator with an empty registry
    pub fn new() -> Self {
        Self {
            registry: HashMap::new(),
        }
    }

    /// Register an attribute in the validator
    pub fn register(&mut self, metadata: AttributeMetadata) {
        self.registry.insert(metadata.id.clone(), metadata);
    }

    /// Register multiple attributes at once
    pub fn register_all(&mut self, attributes: Vec<AttributeMetadata>) {
        for attr in attributes {
            self.register(attr);
        }
    }

    /// Check if an attribute ID is registered
    pub fn is_registered(&self, attribute_id: &str) -> bool {
        self.registry.contains_key(attribute_id)
    }

    /// Get metadata for an attribute
    pub fn get_metadata(&self, attribute_id: &str) -> Option<&AttributeMetadata> {
        self.registry.get(attribute_id)
    }

    /// Validate a single attribute reference
    pub fn validate_attr_ref(&self, attribute_id: &str) -> Result<(), ValidationError> {
        if !self.is_registered(attribute_id) {
            return Err(ValidationError::UnknownAttribute {
                attribute_id: attribute_id.to_string(),
            });
        }
        Ok(())
    }

    /// Validate all attribute references in a DSL program
    pub fn validate_program(&self, program: &Program) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        for form in program {
            if let Form::Verb(verb_form) = form {
                if let Err(form_errors) = self.validate_verb_form(verb_form) {
                    errors.extend(form_errors);
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Validate attribute references in a verb form
    pub fn validate_verb_form(&self, verb_form: &VerbForm) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        for (_key, value) in &verb_form.pairs {
            self.collect_attr_ref_errors(value, &mut errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Recursively collect attribute reference errors from a value
    fn collect_attr_ref_errors(&self, value: &Value, errors: &mut Vec<ValidationError>) {
        match value {
            Value::AttrRef(attr_id) => {
                if let Err(err) = self.validate_attr_ref(attr_id) {
                    errors.push(err);
                }
            }
            Value::List(values) => {
                for v in values {
                    self.collect_attr_ref_errors(v, errors);
                }
            }
            Value::Map(map) => {
                for v in map.values() {
                    self.collect_attr_ref_errors(v, errors);
                }
            }
            Value::Array(values) => {
                for v in values {
                    self.collect_attr_ref_errors(v, errors);
                }
            }
            _ => {} // Other value types don't contain attribute references
        }
    }

    /// Get all registered attribute IDs
    pub fn get_all_attribute_ids(&self) -> Vec<&str> {
        self.registry.keys().map(|s| s.as_str()).collect()
    }

    /// Get attributes by category
    pub fn get_by_category(&self, category: AttributeCategory) -> Vec<&AttributeMetadata> {
        self.registry
            .values()
            .filter(|meta| meta.category == category)
            .collect()
    }

    /// Get attributes by data type
    pub fn get_by_data_type(&self, data_type: DataType) -> Vec<&AttributeMetadata> {
        self.registry
            .values()
            .filter(|meta| meta.data_type == data_type)
            .collect()
    }

    /// Count of registered attributes
    pub fn count(&self) -> usize {
        self.registry.len()
    }
}

impl Default for AttributeValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Validation error types
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    /// Referenced attribute is not in the registry
    UnknownAttribute { attribute_id: String },
    /// Attribute is used with wrong type
    TypeMismatch {
        attribute_id: String,
        expected_type: DataType,
        actual_type: String,
    },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::UnknownAttribute { attribute_id } => {
                write!(f, "Unknown attribute: {}", attribute_id)
            }
            ValidationError::TypeMismatch {
                attribute_id,
                expected_type,
                actual_type,
            } => {
                write!(
                    f,
                    "Type mismatch for attribute {}: expected {:?}, got {}",
                    attribute_id, expected_type, actual_type
                )
            }
        }
    }
}

impl std::error::Error for ValidationError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::attributes::types::ValidationRules;
    use crate::parser::parse_program;

    #[test]
    fn test_validator_registration() {
        let mut validator = AttributeValidator::new();

        let metadata = AttributeMetadata {
            id: "attr.identity.first_name".to_string(),
            uuid: uuid::Uuid::parse_str("3020d46f-472c-5437-9647-1b0682c35935").unwrap(),
            display_name: "First Name".to_string(),
            category: AttributeCategory::Identity,
            data_type: DataType::String,
            validation: ValidationRules::new(),
        };

        validator.register(metadata);
        assert!(validator.is_registered("attr.identity.first_name"));
        assert!(!validator.is_registered("attr.identity.unknown"));
    }

    #[test]
    fn test_validate_known_attribute() {
        let mut validator = AttributeValidator::new();

        let metadata = AttributeMetadata {
            id: "attr.identity.first_name".to_string(),
            uuid: uuid::Uuid::parse_str("3020d46f-472c-5437-9647-1b0682c35935").unwrap(),
            display_name: "First Name".to_string(),
            category: AttributeCategory::Identity,
            data_type: DataType::String,
            validation: ValidationRules::new(),
        };

        validator.register(metadata);
        assert!(validator
            .validate_attr_ref("attr.identity.first_name")
            .is_ok());
    }

    #[test]
    fn test_validate_unknown_attribute() {
        let validator = AttributeValidator::new();
        let result = validator.validate_attr_ref("attr.identity.unknown");
        assert!(result.is_err());

        if let Err(ValidationError::UnknownAttribute { attribute_id }) = result {
            assert_eq!(attribute_id, "attr.identity.unknown");
        } else {
            panic!("Expected UnknownAttribute error");
        }
    }

    #[test]
    fn test_validate_program_with_known_attributes() {
        let mut validator = AttributeValidator::new();

        // Register attributes
        validator.register(AttributeMetadata {
            id: "attr.identity.first_name".to_string(),
            uuid: uuid::Uuid::parse_str("3020d46f-472c-5437-9647-1b0682c35935").unwrap(),
            display_name: "First Name".to_string(),
            category: AttributeCategory::Identity,
            data_type: DataType::String,
            validation: ValidationRules::new(),
        });

        validator.register(AttributeMetadata {
            id: "attr.identity.last_name".to_string(),
            uuid: uuid::Uuid::parse_str("0af112fd-ec04-5938-84e8-6e5949db0b52").unwrap(),
            display_name: "Last Name".to_string(),
            category: AttributeCategory::Identity,
            data_type: DataType::String,
            validation: ValidationRules::new(),
        });

        // Parse DSL with attribute references
        let dsl = r#"(entity.set-attributes :entity-id "test-123" :first @attr.identity.first_name :last @attr.identity.last_name)"#;
        let program = parse_program(dsl).unwrap();

        // Validate
        let result = validator.validate_program(&program);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_program_with_unknown_attributes() {
        let validator = AttributeValidator::new();

        // Parse DSL with unknown attribute reference
        let dsl = r#"(entity.set-attribute :entity-id "test-123" :attr @attr.identity.unknown)"#;
        let program = parse_program(dsl).unwrap();

        // Validate
        let result = validator.validate_program(&program);
        assert!(result.is_err());

        if let Err(errors) = result {
            assert_eq!(errors.len(), 1);
            assert!(matches!(
                errors[0],
                ValidationError::UnknownAttribute { .. }
            ));
        }
    }

    #[test]
    fn test_get_by_category() {
        let mut validator = AttributeValidator::new();

        validator.register(AttributeMetadata {
            id: "attr.identity.first_name".to_string(),
            uuid: uuid::Uuid::parse_str("3020d46f-472c-5437-9647-1b0682c35935").unwrap(),
            display_name: "First Name".to_string(),
            category: AttributeCategory::Identity,
            data_type: DataType::String,
            validation: ValidationRules::new(),
        });

        validator.register(AttributeMetadata {
            id: "attr.compliance.fatca_status".to_string(),
            uuid: uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000099").unwrap(),
            display_name: "FATCA Status".to_string(),
            category: AttributeCategory::Compliance,
            data_type: DataType::String,
            validation: ValidationRules::new(),
        });

        let identity_attrs = validator.get_by_category(AttributeCategory::Identity);
        assert_eq!(identity_attrs.len(), 1);
        assert_eq!(identity_attrs[0].id, "attr.identity.first_name");

        let compliance_attrs = validator.get_by_category(AttributeCategory::Compliance);
        assert_eq!(compliance_attrs.len(), 1);
        assert_eq!(compliance_attrs[0].id, "attr.compliance.fatca_status");
    }

    #[test]
    fn test_validator_count() {
        let mut validator = AttributeValidator::new();
        assert_eq!(validator.count(), 0);

        validator.register(AttributeMetadata {
            id: "attr.identity.first_name".to_string(),
            uuid: uuid::Uuid::parse_str("3020d46f-472c-5437-9647-1b0682c35935").unwrap(),
            display_name: "First Name".to_string(),
            category: AttributeCategory::Identity,
            data_type: DataType::String,
            validation: ValidationRules::new(),
        });

        assert_eq!(validator.count(), 1);
    }
}
