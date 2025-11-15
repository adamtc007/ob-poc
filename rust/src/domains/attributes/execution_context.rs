//! Execution Context for Attribute Value Binding
//!
//! Phase 3: Value Binding During Execution
//!
//! This module provides the execution context that binds UUID-based
//! attribute references to their actual values during DSL execution.

use super::resolver::AttributeResolver;
use std::collections::HashMap;
use uuid::Uuid;

/// Execution context holds attribute values bound during DSL execution
pub struct ExecutionContext {
    /// Attribute resolver for UUID ↔ Semantic ID resolution
    pub resolver: AttributeResolver,

    /// Bound attribute values: UUID → JSON Value
    pub attribute_values: HashMap<Uuid, serde_json::Value>,

    /// Source tracking: UUID → Source information
    pub attribute_sources: HashMap<Uuid, Vec<ValueSource>>,

    /// CBU ID for this execution context
    pub cbu_id: Option<Uuid>,

    /// Entity ID for attribute storage
    pub entity_id: Option<Uuid>,

    /// Current document being processed (for document operations)
    pub current_document_id: Option<Uuid>,
}

/// Describes where an attribute value came from
#[derive(Clone, Debug)]
pub enum ValueSource {
    /// Extracted from a document
    DocumentExtraction {
        document_id: Uuid,
        page: Option<u32>,
        confidence: f64,
    },

    /// Provided by user input
    UserInput { form_id: String, field: String },

    /// Retrieved from third-party API
    ThirdPartyApi { service: String, endpoint: String },

    /// Calculated from other attributes
    Calculation {
        formula: String,
        dependencies: Vec<Uuid>,
    },

    /// Default value
    Default { reason: String },
}

impl ExecutionContext {
    /// Create a new execution context
    pub fn new() -> Self {
        Self {
            resolver: AttributeResolver::new(),
            attribute_values: HashMap::new(),
            attribute_sources: HashMap::new(),
            cbu_id: None,
            entity_id: None,
            current_document_id: None,
        }
    }

    /// Create a new execution context with CBU and entity IDs
    pub fn with_ids(cbu_id: Uuid, entity_id: Uuid) -> Self {
        Self {
            resolver: AttributeResolver::new(),
            attribute_values: HashMap::new(),
            attribute_sources: HashMap::new(),
            cbu_id: Some(cbu_id),
            entity_id: Some(entity_id),
            current_document_id: None,
        }
    }

    /// Set the current document ID
    pub fn set_document(&mut self, document_id: Uuid) {
        self.current_document_id = Some(document_id);
    }

    /// Get the CBU ID
    pub fn cbu_id(&self) -> Option<Uuid> {
        self.cbu_id
    }

    /// Get the entity ID
    pub fn entity_id(&self) -> Option<Uuid> {
        self.entity_id
    }

    /// Get the current document ID
    pub fn document_id(&self) -> Option<Uuid> {
        self.current_document_id
    }

    /// Bind an attribute value
    pub fn bind_value(&mut self, uuid: Uuid, value: serde_json::Value, source: ValueSource) {
        self.attribute_values.insert(uuid, value);
        self.attribute_sources.entry(uuid).or_default().push(source);
    }

    /// Get a bound attribute value
    pub fn get_value(&self, uuid: &Uuid) -> Option<&serde_json::Value> {
        self.attribute_values.get(uuid)
    }

    /// Get attribute value by semantic ID
    pub fn get_value_by_semantic(&self, semantic_id: &str) -> Option<&serde_json::Value> {
        let uuid = self.resolver.semantic_to_uuid(semantic_id).ok()?;
        self.get_value(&uuid)
    }

    /// Get all bound attribute UUIDs
    pub fn bound_attributes(&self) -> Vec<Uuid> {
        self.attribute_values.keys().copied().collect()
    }

    /// Get source information for an attribute
    pub fn get_sources(&self, uuid: &Uuid) -> Option<&Vec<ValueSource>> {
        self.attribute_sources.get(uuid)
    }

    /// Check if an attribute is bound
    pub fn is_bound(&self, uuid: &Uuid) -> bool {
        self.attribute_values.contains_key(uuid)
    }

    /// Clear all bindings
    pub fn clear(&mut self) {
        self.attribute_values.clear();
        self.attribute_sources.clear();
    }
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::attributes::kyc::FirstName;
    use crate::domains::attributes::types::AttributeType;

    #[test]
    fn test_context_creation() {
        let ctx = ExecutionContext::new();
        assert_eq!(ctx.bound_attributes().len(), 0);
    }

    #[test]
    fn test_bind_value() {
        let mut ctx = ExecutionContext::new();
        let uuid = FirstName::uuid();

        ctx.bind_value(
            uuid,
            serde_json::json!("John"),
            ValueSource::UserInput {
                form_id: "onboarding-form".to_string(),
                field: "first_name".to_string(),
            },
        );

        assert!(ctx.is_bound(&uuid));
        assert_eq!(ctx.get_value(&uuid), Some(&serde_json::json!("John")));
    }

    #[test]
    fn test_get_value_by_semantic() {
        let mut ctx = ExecutionContext::new();
        let uuid = FirstName::uuid();

        ctx.bind_value(
            uuid,
            serde_json::json!("Jane"),
            ValueSource::Default {
                reason: "Test value".to_string(),
            },
        );

        let value = ctx.get_value_by_semantic("attr.identity.first_name");
        assert_eq!(value, Some(&serde_json::json!("Jane")));
    }

    #[test]
    fn test_source_tracking() {
        let mut ctx = ExecutionContext::new();
        let uuid = FirstName::uuid();

        let source = ValueSource::DocumentExtraction {
            document_id: Uuid::new_v4(),
            page: Some(1),
            confidence: 0.95,
        };

        ctx.bind_value(uuid, serde_json::json!("Alice"), source.clone());

        let sources = ctx.get_sources(&uuid);
        assert!(sources.is_some());
        assert_eq!(sources.unwrap().len(), 1);
    }

    #[test]
    fn test_clear_bindings() {
        let mut ctx = ExecutionContext::new();
        let uuid = FirstName::uuid();

        ctx.bind_value(
            uuid,
            serde_json::json!("Bob"),
            ValueSource::Default {
                reason: "Test".to_string(),
            },
        );

        assert_eq!(ctx.bound_attributes().len(), 1);

        ctx.clear();

        assert_eq!(ctx.bound_attributes().len(), 0);
        assert!(!ctx.is_bound(&uuid));
    }
}
