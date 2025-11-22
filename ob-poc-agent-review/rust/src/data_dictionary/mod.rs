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
pub mod validation;

pub use attribute::*;

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
