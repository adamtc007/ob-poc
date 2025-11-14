//! DSL Builder with Type-Safe Attributes
//!
//! This module provides a fluent builder API for constructing DSL expressions
//! with compile-time type safety for attributes.

use crate::domains::attributes::types::AttributeType;
use std::collections::HashMap;

/// Builder for creating DSL verb forms with type-safe attributes
#[derive(Debug, Clone)]
pub struct DslBuilder {
    verb: String,
    properties: HashMap<String, DslValue>,
}

/// Type-safe DSL value that can include attribute references
#[derive(Debug, Clone, PartialEq)]
pub enum DslValue {
    String(String),
    Number(f64),
    Integer(i64),
    Boolean(bool),
    AttributeRef(String),
    List(Vec<DslValue>),
}

impl DslBuilder {
    /// Create a new DSL builder for a specific verb
    pub fn new(verb: impl Into<String>) -> Self {
        Self {
            verb: verb.into(),
            properties: HashMap::new(),
        }
    }

    /// Add a string property
    pub fn with_string(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties
            .insert(key.into(), DslValue::String(value.into()));
        self
    }

    /// Add a number property
    pub fn with_number(mut self, key: impl Into<String>, value: f64) -> Self {
        self.properties.insert(key.into(), DslValue::Number(value));
        self
    }

    /// Add an integer property
    pub fn with_integer(mut self, key: impl Into<String>, value: i64) -> Self {
        self.properties.insert(key.into(), DslValue::Integer(value));
        self
    }

    /// Add a boolean property
    pub fn with_boolean(mut self, key: impl Into<String>, value: bool) -> Self {
        self.properties.insert(key.into(), DslValue::Boolean(value));
        self
    }

    /// Add a type-safe attribute reference
    pub fn with_attribute<T: AttributeType>(mut self, key: impl Into<String>) -> Self {
        self.properties
            .insert(key.into(), DslValue::AttributeRef(T::ID.to_string()));
        self
    }

    /// Add a list of values
    pub fn with_list(mut self, key: impl Into<String>, values: Vec<DslValue>) -> Self {
        self.properties.insert(key.into(), DslValue::List(values));
        self
    }

    /// Build the DSL string representation
    pub fn build(self) -> String {
        let mut result = format!("({}", self.verb);

        for (key, value) in self.properties {
            result.push_str(&format!(" :{} {}", key, value.to_dsl_string()));
        }

        result.push(')');
        result
    }

    /// Get the verb name
    pub fn verb(&self) -> &str {
        &self.verb
    }

    /// Get all properties
    pub fn properties(&self) -> &HashMap<String, DslValue> {
        &self.properties
    }
}

impl DslValue {
    /// Convert the value to its DSL string representation
    pub fn to_dsl_string(&self) -> String {
        match self {
            DslValue::String(s) => format!("\"{}\"", s.replace('\"', "\\\"")),
            DslValue::Number(n) => n.to_string(),
            DslValue::Integer(i) => i.to_string(),
            DslValue::Boolean(b) => b.to_string(),
            DslValue::AttributeRef(attr_id) => format!("@{}", attr_id),
            DslValue::List(values) => {
                let items: Vec<String> = values.iter().map(|v| v.to_dsl_string()).collect();
                format!("[{}]", items.join(" "))
            }
        }
    }
}

/// Helper function to create a DSL builder for entity operations
pub fn entity_set_attribute<T: AttributeType>(
    entity_id: impl Into<String>,
    value: T::Value,
) -> DslBuilder
where
    T::Value: std::fmt::Display,
{
    DslBuilder::new("entity.set-attribute")
        .with_string("entity-id", entity_id)
        .with_attribute::<T>("attribute")
        .with_string("value", value.to_string())
}

/// Helper function to create a DSL builder for validation operations
pub fn validate_attribute<T: AttributeType>(
    entity_id: impl Into<String>,
    value: T::Value,
) -> DslBuilder
where
    T::Value: std::fmt::Display,
{
    DslBuilder::new("validation.validate-attribute")
        .with_string("entity-id", entity_id)
        .with_attribute::<T>("attribute")
        .with_string("value", value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::attributes::kyc::*;
    use crate::domains::attributes::types::AttributeType;

    #[test]
    fn test_builder_basic() {
        let dsl = DslBuilder::new("test.verb")
            .with_string("name", "John")
            .with_number("age", 30.5)
            .build();

        assert!(dsl.contains("(test.verb"));
        assert!(dsl.contains(":name \"John\""));
        assert!(dsl.contains(":age 30.5"));
    }

    #[test]
    fn test_builder_with_attribute() {
        let dsl = DslBuilder::new("entity.set-attribute")
            .with_string("entity-id", "test-123")
            .with_attribute::<FirstName>("attribute")
            .with_string("value", "John")
            .build();

        // Check that all components are present (order doesn't matter due to HashMap)
        assert!(dsl.starts_with("(entity.set-attribute"));
        assert!(dsl.contains(":entity-id \"test-123\""));
        assert!(dsl.contains(":attribute @attr.identity.first_name"));
        assert!(dsl.contains(":value \"John\""));
        assert!(dsl.ends_with(")"));
    }

    #[test]
    fn test_builder_multiple_attributes() {
        let dsl = DslBuilder::new("validation.check")
            .with_attribute::<FirstName>("attr1")
            .with_attribute::<LastName>("attr2")
            .build();

        assert!(dsl.contains("@attr.identity.first_name"));
        assert!(dsl.contains("@attr.identity.last_name"));
    }

    #[test]
    fn test_entity_set_attribute_helper() {
        let dsl = entity_set_attribute::<FirstName>("entity-123", "John".to_string()).build();

        assert!(dsl.starts_with("(entity.set-attribute"));
        assert!(dsl.contains(":entity-id \"entity-123\""));
        assert!(dsl.contains(":attribute @attr.identity.first_name"));
        assert!(dsl.contains(":value \"John\""));
        assert!(dsl.ends_with(")"));
    }

    #[test]
    fn test_validate_attribute_helper() {
        let dsl = validate_attribute::<Email>("entity-123", "test@example.com".to_string()).build();

        assert!(dsl.starts_with("(validation.validate-attribute"));
        assert!(dsl.contains(":entity-id \"entity-123\""));
        assert!(dsl.contains(":attribute @attr.contact.email"));
        assert!(dsl.contains(":value \"test@example.com\""));
        assert!(dsl.ends_with(")"));
    }

    #[test]
    fn test_builder_with_list() {
        let dsl = DslBuilder::new("test.verb")
            .with_list(
                "values",
                vec![
                    DslValue::String("a".to_string()),
                    DslValue::String("b".to_string()),
                ],
            )
            .build();

        assert!(dsl.contains(":values [\"a\" \"b\"]"));
    }

    #[test]
    fn test_dsl_value_string_escaping() {
        let value = DslValue::String("Hello \"World\"".to_string());
        assert_eq!(value.to_dsl_string(), "\"Hello \\\"World\\\"\"");
    }

    #[test]
    fn test_builder_with_boolean() {
        let dsl = DslBuilder::new("test.verb")
            .with_boolean("active", true)
            .with_boolean("verified", false)
            .build();

        assert!(dsl.contains(":active true"));
        assert!(dsl.contains(":verified false"));
    }

    #[test]
    fn test_builder_with_ubo_attributes() {
        let dsl = DslBuilder::new("ubo.register")
            .with_attribute::<UboFullName>("name")
            .with_attribute::<UboOwnershipPercentage>("ownership")
            .with_attribute::<UboControlType>("control-type")
            .build();

        assert!(dsl.contains("@attr.ubo.full_name"));
        assert!(dsl.contains("@attr.ubo.ownership_percentage"));
        assert!(dsl.contains("@attr.ubo.control_type"));
    }

    #[test]
    fn test_builder_properties_access() {
        let builder = DslBuilder::new("test.verb")
            .with_string("name", "John")
            .with_attribute::<FirstName>("attr");

        assert_eq!(builder.verb(), "test.verb");
        assert_eq!(builder.properties().len(), 2);
        assert!(builder.properties().contains_key("name"));
        assert!(builder.properties().contains_key("attr"));
    }
}
