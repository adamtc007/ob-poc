//! Data Dictionary - Attribute metadata with RAG support
//!
//! This module defines the metadata structure for attributes used in KYC workflows.
//! Each attribute has:
//! - Semantic descriptions for RAG/LLM understanding
//! - UI layout hints for form generation
//! - Data lineage (sources and sinks)
//! - Validation rules

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod attribute;
pub mod catalogue;
pub mod validation;

pub use attribute::*;
pub use catalogue::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataDictionary {
    pub attributes: HashMap<String, AttributeDefinition>,
    pub categories: Vec<CategoryDefinition>,
    pub relationships: Vec<AttributeRelationship>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryDefinition {
    pub category_id: String,
    pub display_name: String,
    pub description: String,
    pub display_order: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeRelationship {
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

    pub fn add_attribute(&mut self, attr: AttributeDefinition) {
        self.attributes.insert(attr.attr_id.clone(), attr);
    }

    pub fn get_attribute(&self, attr_id: &str) -> Option<&AttributeDefinition> {
        self.attributes.get(attr_id)
    }

    pub fn find_by_category(&self, category: &str) -> Vec<&AttributeDefinition> {
        self.attributes
            .values()
            .filter(|a| a.ui_metadata.category == category)
            .collect()
    }
}

impl Default for DataDictionary {
    fn default() -> Self {
        Self::new()
    }
}
