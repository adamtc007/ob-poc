//! Data Dictionary - Attribute metadata with RAG support
//!
//! This module defines the metadata structure for attributes used in KYC workflows.
//! Each attribute has:
//! - Semantic descriptions for RAG/LLM understanding
//! - UI layout hints for form generation
//! - Data lineage (sources and sinks)
//! - Validation rules

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod attribute;
pub(crate) mod catalogue;
pub mod validation;

pub use attribute::*;
pub(crate) use catalogue::*;

// Re-export key types for convenience
pub use attribute::AttributeId;

/// Service trait for dictionary validation and lookup
#[async_trait]
pub trait DictionaryService: Send + Sync {
    /// Validate DSL attributes against the data dictionary
    async fn validate_dsl_attributes(&self, dsl: &str) -> Result<(), String>;

    /// Get attribute definition by ID
    async fn get_attribute(
        &self,
        attribute_id: &str,
    ) -> Result<Option<AttributeDefinition>, String>;

    /// Validate attribute value against its definition
    async fn validate_attribute_value(
        &self,
        attribute_id: &str,
        value: &serde_json::Value,
    ) -> Result<(), String>;
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
