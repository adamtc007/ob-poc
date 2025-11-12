//! Dictionary models for attribute management and agentic CRUD operations
//!
//! This module contains data structures for managing the central attribute dictionary
//! that forms the foundation of our AttributeID-as-Type architecture.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::collections::HashMap;
use uuid::Uuid;

// Import types from dsl_types crate (Level 1 foundation)
use dsl_types::{AttributeAssetType, AttributeOperationType, DictionaryExecutionStatus};

/// Core dictionary attribute as stored in the database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DictionaryAttribute {
    pub attribute_id: Uuid,
    pub name: String,
    pub long_description: Option<String>,
    pub group_id: String,
    pub mask: String,
    pub domain: Option<String>,
    pub vector: Option<String>,
    pub source: Option<serde_json::Value>,
    pub sink: Option<serde_json::Value>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// New dictionary attribute for creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewDictionaryAttribute {
    pub name: String,
    pub long_description: Option<String>,
    pub group_id: Option<String>, // Defaults to 'default' in DB
    pub mask: Option<String>,     // Defaults to 'string' in DB
    pub domain: Option<String>,
    pub vector: Option<String>,
    pub source: Option<serde_json::Value>,
    pub sink: Option<serde_json::Value>,
}

/// Update dictionary attribute for modifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDictionaryAttribute {
    pub name: Option<String>,
    pub long_description: Option<String>,
    pub group_id: Option<String>,
    pub mask: Option<String>,
    pub domain: Option<String>,
    pub vector: Option<String>,
    pub source: Option<serde_json::Value>,
    pub sink: Option<serde_json::Value>,
}

/// Attribute search criteria
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeSearchCriteria {
    pub name_pattern: Option<String>,
    pub group_id: Option<String>,
    pub domain: Option<String>,
    pub mask: Option<String>,
    pub semantic_query: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

/// Attribute validation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeValidationRequest {
    pub attribute_id: Uuid,
    pub value: serde_json::Value,
    pub context: Option<HashMap<String, serde_json::Value>>,
}

/// Attribute validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeValidationResult {
    pub is_valid: bool,
    pub normalized_value: Option<serde_json::Value>,
    pub validation_errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Attribute discovery request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeDiscoveryRequest {
    pub semantic_query: String,
    pub domain_filter: Option<String>,
    pub group_filter: Option<String>,
    pub limit: Option<i32>,
}

/// Discovered attribute with relevance score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredAttribute {
    pub attribute: DictionaryAttribute,
    pub relevance_score: f64,
    pub match_reason: String,
}

/// Types of operations supported for attributes
// AttributeOperationType moved to dsl_types crate - import from there

impl std::str::FromStr for AttributeOperationType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "create" => Ok(AttributeOperationType::Create),
            "read" => Ok(AttributeOperationType::Read),
            "update" => Ok(AttributeOperationType::Update),
            "delete" => Ok(AttributeOperationType::Delete),
            "search" => Ok(AttributeOperationType::Search),
            "validate" => Ok(AttributeOperationType::Validate),
            "discover" => Ok(AttributeOperationType::Discover),
            _ => Err(format!("Unknown attribute operation type: {}", s)),
        }
    }
}

// AttributeAssetType moved to dsl_types crate - import from there
// All implementations moved with the type

/// Request for creating attributes via agentic CRUD
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticAttributeCreateRequest {
    pub instruction: String,
    pub asset_type: AttributeAssetType,
    pub context: HashMap<String, serde_json::Value>,
    pub constraints: Vec<String>,
    pub group_id: Option<String>,
    pub domain: Option<String>,
}

/// Request for reading attributes via agentic CRUD
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticAttributeReadRequest {
    pub instruction: String,
    pub asset_types: Vec<AttributeAssetType>,
    pub filters: HashMap<String, serde_json::Value>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

/// Request for updating attributes via agentic CRUD
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticAttributeUpdateRequest {
    pub instruction: String,
    pub asset_type: AttributeAssetType,
    pub identifier: HashMap<String, serde_json::Value>,
    pub updates: HashMap<String, serde_json::Value>,
}

/// Request for deleting attributes via agentic CRUD
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticAttributeDeleteRequest {
    pub instruction: String,
    pub asset_type: AttributeAssetType,
    pub identifier: HashMap<String, serde_json::Value>,
    pub cascade: bool,
}

/// Request for searching attributes via agentic CRUD
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticAttributeSearchRequest {
    pub instruction: String,
    pub search_criteria: AttributeSearchCriteria,
    pub semantic_search: bool,
}

/// Request for validating attribute values via agentic CRUD
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticAttributeValidateRequest {
    pub instruction: String,
    pub validation_request: AttributeValidationRequest,
}

/// Request for discovering attributes via agentic CRUD
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticAttributeDiscoverRequest {
    pub instruction: String,
    pub discovery_request: AttributeDiscoveryRequest,
}

// DictionaryExecutionStatus moved to dsl_types crate - import from there

/// Response from agentic attribute CRUD operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticAttributeCrudResponse {
    pub operation_id: Uuid,
    pub generated_dsl: String,
    pub execution_status: DictionaryExecutionStatus,
    pub affected_records: Vec<Uuid>,
    pub ai_explanation: String,
    pub ai_confidence: Option<f64>,
    pub execution_time_ms: Option<i32>,
    pub error_message: Option<String>,
    pub rag_context_used: Vec<String>,
    pub operation_type: AttributeOperationType,
    pub results: Option<serde_json::Value>,
}

/// Dictionary attribute with extended metadata for responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryAttributeWithMetadata {
    pub attribute: DictionaryAttribute,
    pub usage_count: Option<i64>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
    pub related_attributes: Option<Vec<String>>,
    pub validation_rules: Option<serde_json::Value>,
}

/// Batch operation request for attributes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AttributeBatchRequest {
    pub operation_type: AttributeOperationType,
    pub attributes: Vec<NewDictionaryAttribute>,
    pub transaction_id: Option<Uuid>,
}

/// Batch operation result for attributes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AttributeBatchResult {
    pub transaction_id: Uuid,
    pub total_requested: i32,
    pub successful: i32,
    pub failed: i32,
    pub results: Vec<AttributeBatchItemResult>,
    pub execution_time_ms: i64,
}

/// Individual batch item result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AttributeBatchItemResult {
    pub index: i32,
    pub success: bool,
    pub attribute_id: Option<Uuid>,
    pub error_message: Option<String>,
    pub generated_dsl: Option<String>,
}

/// Statistics for dictionary usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryStatistics {
    pub total_attributes: i64,
    pub attributes_by_domain: HashMap<String, i64>,
    pub attributes_by_group: HashMap<String, i64>,
    pub attributes_by_mask: HashMap<String, i64>,
    pub most_used_attributes: Vec<(String, i64)>,
    pub recently_created: Vec<DictionaryAttribute>,
    pub orphaned_attributes: i64, // Attributes not used in any operations
}

/// Dictionary health check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryHealthCheck {
    pub status: String,
    pub total_attributes: i64,
    pub attributes_with_descriptions: i64,
    pub attributes_with_validation: i64,
    pub duplicate_names: Vec<String>,
    pub missing_domains: i64,
    pub recommendations: Vec<String>,
    pub last_check_at: chrono::DateTime<chrono::Utc>,
}
