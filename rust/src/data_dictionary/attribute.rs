//! Attribute definition with complete metadata

use super::*;
use uuid::Uuid;

/// Unique identifier for attributes in the data dictionary
/// This MUST be used everywhere instead of String or raw UUID
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AttributeId(pub Uuid);

impl Default for AttributeId {
    fn default() -> Self {
        Self::new()
    }
}

impl AttributeId {
    /// Create a new attribute ID with a fresh UUID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create an AttributeId from a UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the inner UUID reference
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// Parse AttributeId from string
    pub fn from_str(s: &str) -> Result<Self, uuid::Error> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl std::fmt::Display for AttributeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for AttributeId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<AttributeId> for Uuid {
    fn from(attr_id: AttributeId) -> Self {
        attr_id.0
    }
}

// CRITICAL: sqlx Type trait implementations for database operations
#[cfg(feature = "database")]
impl sqlx::Type<sqlx::Postgres> for AttributeId {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <Uuid as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

#[cfg(feature = "database")]
impl<'r> sqlx::Decode<'r, sqlx::Postgres> for AttributeId {
    fn decode(
        value: <sqlx::Postgres as sqlx::Database>::ValueRef<'r>,
    ) -> Result<Self, sqlx::error::BoxDynError> {
        let uuid = <Uuid as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        Ok(AttributeId(uuid))
    }
}

#[cfg(feature = "database")]
impl<'q> sqlx::Encode<'q, sqlx::Postgres> for AttributeId {
    fn encode_by_ref(
        &self,
        buf: &mut <sqlx::Postgres as sqlx::Database>::ArgumentBuffer<'q>,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        self.0.encode_by_ref(buf)
    }
}

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
pub(crate) enum Importance {
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
pub(crate) struct SourceDefinition {
    pub source_type: SourceType,
    pub details: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum SourceType {
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
pub(crate) struct SinkDefinition {
    pub sink_type: SinkType,
    pub details: HashMap<String, serde_json::Value>,
    pub retention: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum SinkType {
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
