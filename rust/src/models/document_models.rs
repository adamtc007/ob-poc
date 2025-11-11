//! Document-Attribute Models for DSL-as-State + AttributeID-as-Type Architecture
//!
//! This module provides comprehensive models for the document library system with full
//! AttributeID referential integrity, ISO asset type integration, and AI extraction support.

use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;
use std::collections::HashMap;
use uuid::Uuid;

// ============================================================================
// ISO ASSET TYPES MODELS
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct IsoAssetType {
    pub asset_type_id: Uuid,
    pub iso_code: String,
    pub asset_name: String,
    pub asset_category: String,
    pub asset_subcategory: Option<String>,
    pub description: Option<String>,
    pub regulatory_classification: Option<String>,
    pub liquidity_profile: Option<String>,

    // Investment mandate compatibility
    pub suitable_for_conservative: bool,
    pub suitable_for_moderate: bool,
    pub suitable_for_aggressive: bool,
    pub suitable_for_balanced: bool,

    // Risk characteristics
    pub credit_risk_level: Option<String>,
    pub market_risk_level: Option<String>,
    pub liquidity_risk_level: Option<String>,

    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewIsoAssetType {
    pub iso_code: String,
    pub asset_name: String,
    pub asset_category: String,
    pub asset_subcategory: Option<String>,
    pub description: Option<String>,
    pub regulatory_classification: Option<String>,
    pub liquidity_profile: Option<String>,
    pub suitable_for_conservative: bool,
    pub suitable_for_moderate: bool,
    pub suitable_for_aggressive: bool,
    pub suitable_for_balanced: bool,
    pub credit_risk_level: Option<String>,
    pub market_risk_level: Option<String>,
    pub liquidity_risk_level: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetSuitabilityCheck {
    pub iso_code: String,
    pub asset_name: String,
    pub is_suitable: bool,
    pub reason: String,
}

// ============================================================================
// DOCUMENT TYPES MODELS
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DocumentType {
    pub type_id: Uuid,
    pub type_code: String,
    pub display_name: String,
    pub category: String,
    pub domain: Option<String>,
    pub primary_attribute_id: Option<Uuid>,
    pub description: Option<String>,
    pub typical_issuers: Vec<String>,
    pub validity_period_days: Option<i32>,
    pub renewal_required: bool,
    pub expected_attribute_ids: Vec<Uuid>,
    pub validation_attribute_ids: Option<Vec<Uuid>>,
    pub extraction_template: Option<serde_json::Value>,
    pub required_for_products: Option<Vec<String>>,
    pub compliance_frameworks: Option<Vec<String>>,
    pub risk_classification: Option<String>,
    pub ai_description: Option<String>,
    pub common_contents: Option<String>,
    pub key_data_point_attributes: Option<Vec<Uuid>>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewDocumentType {
    pub type_code: String,
    pub display_name: String,
    pub category: String,
    pub domain: Option<String>,
    pub primary_attribute_id: Option<Uuid>,
    pub description: Option<String>,
    pub typical_issuers: Vec<String>,
    pub validity_period_days: Option<i32>,
    pub expected_attribute_ids: Vec<Uuid>,
    pub key_data_point_attributes: Option<Vec<Uuid>>,
    pub ai_description: Option<String>,
    pub common_contents: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDocumentType {
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub expected_attribute_ids: Option<Vec<Uuid>>,
    pub key_data_point_attributes: Option<Vec<Uuid>>,
    pub ai_description: Option<String>,
    pub active: Option<bool>,
}

// ============================================================================
// DOCUMENT ISSUERS MODELS
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DocumentIssuer {
    pub issuer_id: Uuid,
    pub issuer_code: String,
    pub legal_name: String,
    pub jurisdiction: Option<String>,
    pub regulatory_type: Option<String>,
    pub contact_information: Option<serde_json::Value>,
    pub document_types_issued: Option<Vec<String>>,
    pub authority_level: Option<String>,
    pub verification_endpoint: Option<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewDocumentIssuer {
    pub issuer_code: String,
    pub legal_name: String,
    pub jurisdiction: Option<String>,
    pub regulatory_type: Option<String>,
    pub document_types_issued: Option<Vec<String>>,
    pub authority_level: Option<String>,
}

// ============================================================================
// DOCUMENT CATALOG MODELS
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DocumentCatalog {
    pub document_id: Uuid,
    pub document_code: String,
    pub document_type_id: Uuid,
    pub issuer_id: Option<Uuid>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub issue_date: Option<NaiveDateTime>,
    pub expiry_date: Option<NaiveDateTime>,
    pub language: Option<String>,
    pub file_path: Option<String>,
    pub file_size: Option<i64>,
    pub mime_type: Option<String>,
    pub checksum: Option<String>,
    pub related_entities: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub confidentiality_level: Option<String>,
    pub verification_status: Option<String>,
    pub verification_date: Option<DateTime<Utc>>,
    pub extracted_attributes: Option<serde_json::Value>,
    pub extraction_confidence: Option<f64>,
    pub extraction_method: Option<String>,
    pub extraction_date: Option<DateTime<Utc>>,
    pub metadata: Option<serde_json::Value>,
    pub version: String,
    pub parent_document_id: Option<Uuid>,
    pub is_current_version: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewDocumentCatalog {
    pub document_code: String,
    pub document_type_id: Uuid,
    pub issuer_id: Option<Uuid>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub issue_date: Option<NaiveDateTime>,
    pub expiry_date: Option<NaiveDateTime>,
    pub language: Option<String>,
    pub related_entities: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub confidentiality_level: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDocumentCatalog {
    pub title: Option<String>,
    pub description: Option<String>,
    pub verification_status: Option<String>,
    pub extracted_attributes: Option<serde_json::Value>,
    pub extraction_confidence: Option<f64>,
    pub extraction_method: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DocumentCatalogWithAttributes {
    #[sqlx(flatten)]
    pub document: DocumentCatalog,
    pub type_code: Option<String>,
    pub document_type_name: Option<String>,
    pub document_category: Option<String>,
    pub document_domain: Option<String>,
    pub expected_attribute_ids: Option<Vec<Uuid>>,
    pub key_data_point_attributes: Option<Vec<Uuid>>,
    pub issuer_code: Option<String>,
    pub issuer_name: Option<String>,
}

// ============================================================================
// DOCUMENT USAGE MODELS
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DocumentUsage {
    pub usage_id: Uuid,
    pub document_id: Uuid,
    pub dsl_version_id: Option<Uuid>,
    pub cbu_id: Option<String>,
    pub workflow_stage: Option<String>,
    pub usage_type: String,
    pub usage_context: Option<serde_json::Value>,
    pub business_purpose: Option<String>,
    pub risk_assessment: Option<String>,
    pub compliance_notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewDocumentUsage {
    pub document_id: Uuid,
    pub dsl_version_id: Option<Uuid>,
    pub cbu_id: Option<String>,
    pub workflow_stage: Option<String>,
    pub usage_type: String,
    pub usage_context: Option<serde_json::Value>,
    pub business_purpose: Option<String>,
}

// ============================================================================
// DOCUMENT RELATIONSHIPS MODELS
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DocumentRelationship {
    pub relationship_id: Uuid,
    pub source_document_id: Uuid,
    pub target_document_id: Uuid,
    pub relationship_type: String,
    pub relationship_strength: String,
    pub description: Option<String>,
    pub business_rationale: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewDocumentRelationship {
    pub source_document_id: Uuid,
    pub target_document_id: Uuid,
    pub relationship_type: String,
    pub relationship_strength: Option<String>,
    pub description: Option<String>,
    pub business_rationale: Option<String>,
}

// ============================================================================
// INVESTMENT MANDATE SPECIFIC MODELS
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvestmentMandateExtraction {
    pub fund_name: Option<String>,
    pub investment_objective: Option<String>,
    pub asset_allocation: Option<String>,
    pub permitted_assets: Option<Vec<String>>,  // ISO codes
    pub prohibited_assets: Option<Vec<String>>, // ISO codes
    pub risk_profile: Option<String>,
    pub benchmark_index: Option<String>,
    pub geographic_focus: Option<String>,
    pub leverage_limit: Option<f64>,
    pub liquidity_terms: Option<String>,
    pub concentration_limits: Option<String>,
    pub duration_target: Option<f64>,
    pub credit_quality_floor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvestmentMandateValidation {
    pub is_valid: bool,
    pub validation_errors: Vec<String>,
    pub asset_suitability_issues: Vec<AssetSuitabilityCheck>,
    pub permitted_assets_validated: Vec<String>,
    pub prohibited_assets_validated: Vec<String>,
}

// ============================================================================
// SEARCH AND QUERY MODELS
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSearchRequest {
    pub query: Option<String>,
    pub document_type: Option<String>,
    pub category: Option<String>,
    pub domain: Option<String>,
    pub issuer: Option<String>,
    pub tags: Option<Vec<String>>,
    pub confidentiality_level: Option<String>,
    pub verification_status: Option<String>,
    pub issue_date_from: Option<NaiveDateTime>,
    pub issue_date_to: Option<NaiveDateTime>,
    pub extracted_attributes: Option<serde_json::Value>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSearchResponse {
    pub documents: Vec<DocumentCatalogWithAttributes>,
    pub total_count: i64,
    pub has_more: bool,
}

// ============================================================================
// STATISTICS AND ANALYTICS MODELS
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DocumentAttributeStatistics {
    pub total_documents: i64,
    pub documents_with_extractions: i64,
    pub average_extraction_confidence: Option<f64>,
    pub most_common_document_type: Option<String>,
    pub extraction_methods_used: Vec<String>,
    pub attribute_coverage_percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DocumentTypeStatistics {
    pub type_code: String,
    pub display_name: String,
    pub category: String,
    pub total_documents: i64,
    pub extracted_documents: i64,
    pub average_confidence: Option<f64>,
    pub most_recent_document: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMappingSummary {
    pub total_document_types: i64,
    pub mapped_document_types: i64,
    pub coverage_percentage: f64,
    pub total_iso_asset_types: i64,
    pub total_document_attributes: i64,
    pub investment_mandate_ready: bool,
}

// ============================================================================
// ERROR AND VALIDATION MODELS
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeValidationError {
    pub attribute_id: Uuid,
    pub attribute_name: Option<String>,
    pub error_type: String,
    pub error_message: String,
    pub suggested_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentValidationResult {
    pub is_valid: bool,
    pub document_id: Uuid,
    pub validation_errors: Vec<AttributeValidationError>,
    pub missing_required_attributes: Vec<Uuid>,
    pub unexpected_attributes: Vec<String>,
    pub confidence_issues: Vec<String>,
}

// ============================================================================
// API RESPONSE MODELS
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: Option<String>,
    pub errors: Option<Vec<String>>,
    pub timestamp: DateTime<Utc>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: None,
            errors: None,
            timestamp: Utc::now(),
        }
    }

    pub fn error(message: String, errors: Vec<String>) -> Self {
        Self {
            success: false,
            data: None,
            message: Some(message),
            errors: Some(errors),
            timestamp: Utc::now(),
        }
    }
}

// ============================================================================
// BULK OPERATIONS MODELS
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkDocumentImport {
    pub documents: Vec<NewDocumentCatalog>,
    pub validate_attributes: bool,
    pub skip_duplicates: bool,
    pub extraction_method: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkImportResult {
    pub total_processed: usize,
    pub successful_imports: usize,
    pub failed_imports: usize,
    pub skipped_duplicates: usize,
    pub errors: Vec<String>,
    pub imported_document_ids: Vec<Uuid>,
}

// ============================================================================
// AI EXTRACTION MODELS
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiExtractionRequest {
    pub document_id: Uuid,
    pub extraction_method: String, // "ai", "ocr", "manual", "api"
    pub template: Option<String>,
    pub confidence_threshold: Option<f64>,
    pub ai_model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiExtractionResult {
    pub document_id: Uuid,
    pub extracted_attributes: HashMap<String, serde_json::Value>, // AttributeID -> Value
    pub confidence_scores: HashMap<String, f64>,                  // AttributeID -> Confidence
    pub overall_confidence: f64,
    pub extraction_method: String,
    pub processing_time_ms: u64,
    pub ai_model_used: Option<String>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

// ============================================================================
// INTEGRATION MODELS FOR DSL-AS-STATE
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentDslIntegration {
    pub document_id: Uuid,
    pub dsl_version_id: Uuid,
    pub cbu_id: String,
    pub integration_type: String, // "evidence", "reference", "compliance"
    pub dsl_verb_context: Option<String>,
    pub attribute_mappings: HashMap<String, String>, // DSL attr -> Document attr
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentStateSnapshot {
    pub document_id: Uuid,
    pub snapshot_date: DateTime<Utc>,
    pub document_state: serde_json::Value,
    pub extracted_attributes: serde_json::Value,
    pub verification_status: String,
    pub compliance_status: Option<String>,
    pub related_dsl_versions: Vec<Uuid>,
}
