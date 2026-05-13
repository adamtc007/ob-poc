//! Transitional compatibility types for legacy dictionary call sites.

use ob_poc_macros::IdType;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for attributes in the compatibility dictionary layer.
#[derive(IdType)]
#[id(new_v4)]
pub struct AttributeId(Uuid);

/// Simplified attribute definition matching database schema
/// Used for DB queries in DictionaryService
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbAttributeDefinition {
    pub attribute_id: AttributeId,
    pub name: String,
    pub long_description: Option<String>,
    pub data_type: String,
    #[cfg(feature = "database")]
    pub source_config: Option<sqlx::types::Json<SourceConfig>>,
    #[cfg(not(feature = "database"))]
    pub source_config: Option<serde_json::Value>,
    #[cfg(feature = "database")]
    pub sink_config: Option<sqlx::types::Json<SinkConfig>>,
    #[cfg(not(feature = "database"))]
    pub sink_config: Option<serde_json::Value>,
    pub group_id: Option<String>,
    pub domain: Option<String>,
}

/// Source configuration for attribute data retrieval
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceConfig {
    pub source_type: String,
    pub extraction_rules: Vec<String>,
    pub priority: i32,
}

/// Sink configuration for attribute data persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SinkConfig {
    pub sink_type: String,
    pub destinations: Vec<String>,
}
