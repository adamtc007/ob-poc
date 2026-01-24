//! Attribute definition with complete metadata

use super::*;
use ob_poc_macros::IdType;
use uuid::Uuid;

/// Unique identifier for attributes in the data dictionary
#[derive(IdType)]
#[id(new_v4)]
pub struct AttributeId(Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeDefinition {
    pub attr_id: String,
    pub display_name: String,
    pub data_type: DataType,
    pub constraints: Option<Constraints>,

    // RAG-optimized semantic content
    pub semantic: SemanticMetadata,

    // Vector embedding (populated by background job)
    pub embedding: Option<EmbeddingInfo>,

    // UI/form layout hints
    pub ui_metadata: UiMetadata,

    // Data lineage - sources (where to GET data)
    pub sources: DataSources,

    // Data persistence - sinks (where to PUT data)
    pub sinks: DataSinks,

    // Verification requirements
    pub verification: VerificationRules,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataType {
    String,
    Numeric,
    Integer,
    Boolean,
    Date,
    Address,
    Currency,
    Percentage,
    Email,
    Phone,
    TaxId,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraints {
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub precision: Option<u32>,
    pub pattern: Option<String>,
    pub allowed_values: Option<Vec<String>>,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticMetadata {
    /// Deep English description for RAG
    pub description: String,

    /// Business context and use cases
    pub context: String,

    /// Related concepts/attributes for semantic search
    pub related_concepts: Vec<String>,

    /// Concrete usage examples
    pub usage_examples: Vec<String>,

    /// Regulatory citations if applicable
    pub regulatory_citations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingInfo {
    pub vector: Option<Vec<f32>>, // Actual vector (3072-dim for text-embedding-3-large)
    pub model: String,
    pub dimension: usize,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiMetadata {
    pub category: String,
    pub subcategory: String,
    pub display_order: u32,
    pub form_section: String,
    pub layout_weight: f64,
    pub visual_importance: Importance,
    pub proximity_preferences: Vec<String>,
    pub break_after: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Importance {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSources {
    pub primary: Option<SourceDefinition>,
    pub secondary: Option<SourceDefinition>,
    pub tertiary: Option<SourceDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceDefinition {
    pub source_type: SourceType,
    pub details: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SourceType {
    DocumentExtraction,
    Solicitation,
    ThirdPartyService,
    InternalSystem,
    ManualEntry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSinks {
    pub operational: Option<SinkDefinition>,
    pub master: Option<SinkDefinition>,
    pub archive: Option<SinkDefinition>,
    pub audit: Option<SinkDefinition>,
    pub analytics: Option<SinkDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SinkDefinition {
    pub sink_type: SinkType,
    pub details: HashMap<String, serde_json::Value>,
    pub retention: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SinkType {
    PostgreSQL,
    S3,
    DataLake,
    VectorDb,
    AuditLog,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationRules {
    pub required_confidence: f64,
    pub requires_human_review: bool,
    pub review_trigger: Option<String>,
    pub cross_validation: Vec<String>,
}

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

impl DataType {
    pub fn as_str(&self) -> &str {
        match self {
            DataType::String => "string",
            DataType::Numeric => "numeric",
            DataType::Integer => "integer",
            DataType::Boolean => "boolean",
            DataType::Date => "date",
            DataType::Address => "address",
            DataType::Currency => "currency",
            DataType::Percentage => "percentage",
            DataType::Email => "email",
            DataType::Phone => "phone",
            DataType::TaxId => "tax_id",
            DataType::Custom(s) => s,
        }
    }
}
