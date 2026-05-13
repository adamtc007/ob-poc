//! Transitional compatibility seam for legacy dictionary-facing services.

use async_trait::async_trait;
pub mod attribute;

// Re-export key types for convenience
pub use attribute::{AttributeId, DbAttributeDefinition, SinkConfig, SourceConfig};

/// Service trait for dictionary validation and lookup
#[async_trait]
pub trait DictionaryService: Send + Sync {
    /// Validate DSL attributes against the data dictionary
    /// Returns the list of validated attribute IDs
    async fn validate_dsl_attributes(&self, dsl: &str) -> Result<Vec<AttributeId>, String>;

    /// Get attribute definition by ID
    async fn get_attribute(
        &self,
        attribute_id: &AttributeId,
    ) -> Result<Option<DbAttributeDefinition>, String>;

    /// Validate attribute value against its definition
    async fn validate_attribute_value(
        &self,
        attribute_id: &AttributeId,
        value: &serde_json::Value,
    ) -> Result<(), String>;
}
