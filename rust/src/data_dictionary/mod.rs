//! Data Dictionary - Attribute metadata with RAG support
//!
//! This module defines the metadata structure for attributes used in KYC workflows.
//! Each attribute has:
//! - Semantic descriptions for RAG/LLM understanding
//! - UI layout hints for form generation
//! - Data lineage (sources and sinks)
//! - Validation rules

// Allow unused code - this is legacy data dictionary implementation
#![allow(dead_code)]
#![allow(unused_imports)]

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod attribute;
pub(crate) mod catalogue;
pub mod validation;

pub use attribute::*;

// Re-export key types for convenience
pub use attribute::AttributeId;

/// Service trait for dictionary validation and lookup
#[async_trait]
pub trait DictionaryService: Send + Sync {
    /// Validate DSL attributes against the data dictionary
    /// Returns the list of AttributeIds found in the DSL
    async fn validate_dsl_attributes(&self, dsl: &str) -> Result<Vec<AttributeId>, String>;

    /// Get attribute definition by ID - NOW USES AttributeId!
    async fn get_attribute(
        &self,
        attribute_id: &AttributeId,
    ) -> Result<Option<AttributeDefinition>, String>;

    /// Validate attribute value against its definition - NOW USES AttributeId!
    async fn validate_attribute_value(
        &self,
        attribute_id: &AttributeId,
        value: &serde_json::Value,
    ) -> Result<(), String>;

    /// Extract attributes from a document
    async fn extract_attributes_from_document(
        &self,
        doc_id: uuid::Uuid,
        cbu_id: uuid::Uuid,
    ) -> Result<Vec<AttributeId>, String>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DataDictionary {
    pub attributes: HashMap<String, AttributeDefinition>,
    pub categories: Vec<CategoryDefinition>,
    pub relationships: Vec<AttributeRelationship>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CategoryDefinition {
    pub category_id: String,
    pub display_name: String,
    pub description: String,
    pub display_order: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AttributeRelationship {
    pub from_attr: String,
    pub to_attr: String,
    pub relationship_type: RelationshipType,
    pub strength: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RelationshipType {
    ProximityPreference,
    CrossValidation,
    DependsOn,
    MutuallyExclusive,
}

impl DataDictionary {
    pub fn new() -> Self {
        DataDictionary {
            attributes: HashMap::new(),
            categories: Vec::new(),
            relationships: Vec::new(),
        }
    }

    pub(crate) fn get_attribute(&self, attr_id: &str) -> Option<&AttributeDefinition> {
        self.attributes.get(attr_id)
    }
}

impl Default for DataDictionary {
    fn default() -> Self {
        Self::new()
    }
}
