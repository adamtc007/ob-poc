//! Document Models - EAV/Metadata-Driven Document Management
//!
//! This module provides data models for the new EAV-style document catalog system.
//! All document metadata is stored as attributes linking to the master dictionary,
//! following the AttributeID-as-Type pattern.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::collections::HashMap;
use uuid::Uuid;

// ============================================================================
// DOCUMENT CATALOG MODELS (Core Entity)
// ============================================================================

/// Central document catalog entry - stores file metadata and AI extraction results
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub(crate) struct DocumentCatalog {
    pub doc_id: Uuid,
    pub file_hash_sha256: String,
    pub storage_key: String,
    pub file_size_bytes: Option<i64>,
    pub mime_type: Option<String>,
    pub extracted_data: Option<serde_json::Value>,
    pub extraction_status: String,
    pub extraction_confidence: Option<f64>,
    pub last_extracted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// New document catalog entry for creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NewDocumentCatalog {
    pub file_hash_sha256: String,
    pub storage_key: String,
    pub file_size_bytes: Option<i64>,
    pub mime_type: Option<String>,
    pub extracted_data: Option<serde_json::Value>,
    pub extraction_status: Option<String>,
    pub extraction_confidence: Option<f64>,
}

/// Update document catalog entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UpdateDocumentCatalog {
    pub extracted_data: Option<serde_json::Value>,
    pub extraction_status: Option<String>,
    pub extraction_confidence: Option<f64>,
    pub last_extracted_at: Option<DateTime<Utc>>,
}

// ============================================================================
// DOCUMENT METADATA MODELS (EAV Attributes)
// ============================================================================

/// Document metadata entry - links documents to dictionary attributes
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub(crate) struct DocumentMetadata {
    pub doc_id: Uuid,
    pub attribute_id: Uuid,
    pub value: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// New document metadata entry for creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NewDocumentMetadata {
    pub doc_id: Uuid,
    pub attribute_id: Uuid,
    pub value: serde_json::Value,
}

/// Batch metadata update for a document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DocumentMetadataBatch {
    pub doc_id: Uuid,
    pub metadata: Vec<AttributeValue>,
}

/// Attribute-value pair for metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeValue {
    pub attribute_id: Uuid,
    pub value: serde_json::Value,
}

// ============================================================================
// DOCUMENT RELATIONSHIP MODELS
// ============================================================================

/// Document-to-document relationships
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub(crate) struct DocumentRelationship {
    pub relationship_id: Uuid,
    pub primary_doc_id: Uuid,
    pub related_doc_id: Uuid,
    pub relationship_type: String,
    pub created_at: DateTime<Utc>,
}

/// New document relationship for creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NewDocumentRelationship {
    pub primary_doc_id: Uuid,
    pub related_doc_id: Uuid,
    pub relationship_type: String,
}

/// Common relationship types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum DocumentRelationshipType {
    Amends,
    Supersedes,
    IsTranslationOf,
    IsVersionOf,
    References,
    Accompanies,
    IsAttachmentTo,
    IsSupplementTo,
}

impl DocumentRelationshipType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Amends => "AMENDS",
            Self::Supersedes => "SUPERSEDES",
            Self::IsTranslationOf => "IS_TRANSLATION_OF",
            Self::IsVersionOf => "IS_VERSION_OF",
            Self::References => "REFERENCES",
            Self::Accompanies => "ACCOMPANIES",
            Self::IsAttachmentTo => "IS_ATTACHMENT_TO",
            Self::IsSupplementTo => "IS_SUPPLEMENT_TO",
        }
    }
}

// ============================================================================
// DOCUMENT USAGE MODELS
// ============================================================================

/// Document usage tracking - links documents to CBUs and workflows
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DocumentUsage {
    pub usage_id: Uuid,
    pub doc_id: Uuid,
    pub cbu_id: Uuid,
    pub entity_id: Option<Uuid>,
    pub usage_context: Option<String>,
    pub used_at: DateTime<Utc>,
}

/// New document usage for creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NewDocumentUsage {
    pub doc_id: Uuid,
    pub cbu_id: Uuid,
    pub entity_id: Option<Uuid>,
    pub usage_context: Option<String>,
}

/// Common usage contexts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum DocumentUsageContext {
    EvidenceOfAddress,
    EvidenceOfIdentity,
    EvidenceOfIncome,
    ComplianceDocument,
    RegulatoryFiling,
    ContractualAgreement,
    KycVerification,
    UboEvidence,
    FinancialStatement,
    RiskAssessment,
}

impl DocumentUsageContext {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EvidenceOfAddress => "EVIDENCE_OF_ADDRESS",
            Self::EvidenceOfIdentity => "EVIDENCE_OF_IDENTITY",
            Self::EvidenceOfIncome => "EVIDENCE_OF_INCOME",
            Self::ComplianceDocument => "COMPLIANCE_DOCUMENT",
            Self::RegulatoryFiling => "REGULATORY_FILING",
            Self::ContractualAgreement => "CONTRACTUAL_AGREEMENT",
            Self::KycVerification => "KYC_VERIFICATION",
            Self::UboEvidence => "UBO_EVIDENCE",
            Self::FinancialStatement => "FINANCIAL_STATEMENT",
            Self::RiskAssessment => "RISK_ASSESSMENT",
        }
    }
}

// ============================================================================
// COMPOSITE MODELS & VIEWS
// ============================================================================

/// Document with aggregated metadata (matches the view)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub(crate) struct DocumentCatalogWithMetadata {
    pub doc_id: Uuid,
    pub file_hash_sha256: String,
    pub storage_key: String,
    pub file_size_bytes: Option<i64>,
    pub mime_type: Option<String>,
    pub extracted_data: Option<serde_json::Value>,
    pub extraction_status: String,
    pub extraction_confidence: Option<f64>,
    pub last_extracted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: serde_json::Value, // Aggregated metadata as JSONB
}

/// Full document details with relationships and usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DocumentDetails {
    pub catalog: DocumentCatalog,
    pub metadata: Vec<DocumentMetadata>,
    pub relationships: Vec<DocumentRelationship>,
    pub usage_history: Vec<DocumentUsage>,
}

/// Document summary for listings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DocumentSummary {
    pub doc_id: Uuid,
    pub storage_key: String,
    pub mime_type: Option<String>,
    pub extraction_status: String,
    pub extraction_confidence: Option<f64>,
    pub metadata_count: i64,
    pub usage_count: i64,
    pub relationship_count: i64,
    pub created_at: DateTime<Utc>,
}

// ============================================================================
// SEARCH & QUERY MODELS
// ============================================================================

/// Document search request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DocumentSearchRequest {
    /// Full-text search query
    pub query: Option<String>,
    /// Filter by specific attribute values
    pub attribute_filters: Option<HashMap<Uuid, serde_json::Value>>,
    /// Filter by extraction status
    pub extraction_status: Option<String>,
    /// Filter by mime type
    pub mime_type: Option<String>,
    /// Filter by minimum confidence
    pub min_confidence: Option<f64>,
    /// Filter by CBU usage
    pub used_by_cbu: Option<Uuid>,
    /// Filter by entity usage
    pub used_by_entity: Option<Uuid>,
    /// Filter by creation date range
    pub created_from: Option<DateTime<Utc>>,
    pub created_to: Option<DateTime<Utc>>,
    /// Pagination
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Document search response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DocumentSearchResponse {
    pub documents: Vec<DocumentCatalogWithMetadata>,
    pub total_count: i64,
    pub has_more: bool,
}

// ============================================================================
// AI EXTRACTION MODELS
// ============================================================================

/// AI extraction request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AiExtractionRequest {
    pub doc_id: Uuid,
    pub extraction_method: String,
    pub confidence_threshold: Option<f64>,
    pub ai_model: Option<String>,
    pub force_reextraction: bool,
}

/// AI extraction result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AiExtractionResult {
    pub doc_id: Uuid,
    pub extracted_data: serde_json::Value,
    pub extraction_confidence: f64,
    pub extraction_method: String,
    pub ai_model_used: String,
    pub processing_time_ms: u64,
    pub extracted_attributes: Vec<AttributeValue>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

/// Extraction status tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum ExtractionStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    RequiresReview,
}

impl ExtractionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::InProgress => "IN_PROGRESS",
            Self::Completed => "COMPLETED",
            Self::Failed => "FAILED",
            Self::RequiresReview => "REQUIRES_REVIEW",
        }
    }
}

// ============================================================================
// BULK OPERATIONS
// ============================================================================

/// Bulk document import request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BulkDocumentImport {
    pub documents: Vec<NewDocumentCatalog>,
    pub metadata_batches: Vec<DocumentMetadataBatch>,
    pub skip_duplicates: bool,
    pub auto_extract: bool,
}

/// Bulk import result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BulkImportResult {
    pub total_processed: usize,
    pub successful_imports: usize,
    pub failed_imports: usize,
    pub skipped_duplicates: usize,
    pub imported_doc_ids: Vec<Uuid>,
    pub errors: Vec<String>,
}

// ============================================================================
// STATISTICS & ANALYTICS
// ============================================================================

/// Document statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DocumentStatistics {
    pub total_documents: i64,
    pub documents_by_status: HashMap<String, i64>,
    pub documents_by_mime_type: HashMap<String, i64>,
    pub average_extraction_confidence: Option<f64>,
    pub total_metadata_entries: i64,
    pub total_relationships: i64,
    pub total_usage_entries: i64,
    pub most_used_attributes: Vec<AttributeUsageStats>,
}

/// Attribute usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AttributeUsageStats {
    pub attribute_id: Uuid,
    pub attribute_name: String,
    pub usage_count: i64,
    pub percentage: f64,
}

// ============================================================================
// DSL INTEGRATION MODELS
// ============================================================================

/// DSL verb execution context for document operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DocumentDslContext {
    pub operation_type: DocumentDslOperation,
    pub doc_id: Option<Uuid>,
    pub cbu_id: Option<Uuid>,
    pub entity_id: Option<Uuid>,
    pub attributes: Vec<AttributeValue>,
    pub relationships: Vec<NewDocumentRelationship>,
}

/// Document DSL operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum DocumentDslOperation {
    Catalog,
    Verify,
    Extract,
    Link,
    Use,
    Amend,
    Expire,
    Query,
}

impl DocumentDslOperation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Catalog => "document.catalog",
            Self::Verify => "document.verify",
            Self::Extract => "document.extract",
            Self::Link => "document.link",
            Self::Use => "document.use",
            Self::Amend => "document.amend",
            Self::Expire => "document.expire",
            Self::Query => "document.query",
        }
    }
}

// ============================================================================
// API RESPONSE WRAPPERS
// ============================================================================

/// Standard API response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: Option<String>,
    pub errors: Vec<String>,
    pub timestamp: DateTime<Utc>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: None,
            errors: vec![],
            timestamp: Utc::now(),
        }
    }

    pub fn error(message: String, errors: Vec<String>) -> Self {
        Self {
            success: false,
            data: None,
            message: Some(message),
            errors,
            timestamp: Utc::now(),
        }
    }
}

// ============================================================================
// VALIDATION MODELS
// ============================================================================

/// Document validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DocumentValidationResult {
    pub is_valid: bool,
    pub doc_id: Uuid,
    pub validation_errors: Vec<ValidationError>,
    pub missing_required_metadata: Vec<Uuid>,
    pub confidence_warnings: Vec<ConfidenceWarning>,
}

/// Validation error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub error_type: String,
    pub error_message: String,
    pub attribute_id: Option<Uuid>,
    pub suggested_fix: Option<String>,
}

/// Confidence warning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ConfidenceWarning {
    pub attribute_id: Uuid,
    pub current_confidence: f64,
    pub threshold: f64,
    pub recommendation: String,
}

// ============================================================================
// TRAIT IMPLEMENTATIONS
// ============================================================================

impl Default for NewDocumentCatalog {
    fn default() -> Self {
        Self {
            file_hash_sha256: String::new(),
            storage_key: String::new(),
            file_size_bytes: None,
            mime_type: None,
            extracted_data: None,
            extraction_status: Some("PENDING".to_string()),
            extraction_confidence: None,
        }
    }
}

impl Default for DocumentSearchRequest {
    fn default() -> Self {
        Self {
            query: None,
            attribute_filters: None,
            extraction_status: None,
            mime_type: None,
            min_confidence: None,
            used_by_cbu: None,
            used_by_entity: None,
            created_from: None,
            created_to: None,
            limit: Some(20),
            offset: Some(0),
        }
    }
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

impl DocumentCatalog {
    /// Check if document has been successfully extracted
    pub(crate) fn is_extracted(&self) -> bool {
        self.extraction_status == "COMPLETED" && self.extracted_data.is_some()
    }

    /// Get extraction confidence or 0.0 if not extracted
    pub fn confidence(&self) -> f64 {
        self.extraction_confidence.unwrap_or(0.0)
    }

    /// Check if extraction confidence meets threshold
    pub(crate) fn meets_confidence_threshold(&self, threshold: f64) -> bool {
        self.confidence() >= threshold
    }
}

impl DocumentMetadata {
    /// Convert value to string representation
    pub(crate) fn value_as_string(&self) -> String {
        match &self.value {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            _ => self.value.to_string(),
        }
    }

    /// Check if value matches a string pattern
    pub(crate) fn value_matches(&self, pattern: &str) -> bool {
        self.value_as_string()
            .to_lowercase()
            .contains(&pattern.to_lowercase())
    }
}
