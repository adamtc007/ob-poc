//! Document-Attribute Repository
//!
//! This module provides comprehensive database access for the foundational document-attribute
//! bridge, implementing CRUD operations for consolidated attributes, document types, and
//! document-attribute mappings. Enables the complete DSL-as-State architecture with
//! document-driven workflows and AI-powered data extraction.

use crate::error::DatabaseError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use uuid::Uuid;

/// Repository for document-attribute bridge operations
#[derive(Clone)]
pub struct DocumentAttributeRepository {
    pool: PgPool,
}

/// Consolidated attribute from the universal AttributeID dictionary
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ConsolidatedAttribute {
    pub attribute_id: Uuid,
    pub attribute_code: String,
    pub attribute_name: String,
    pub data_type: String,
    pub category: String,
    pub subcategory: Option<String>,
    pub description: String,
    pub privacy_classification: String,
    pub validation_rules: Option<serde_json::Value>,
    pub extraction_patterns: Option<serde_json::Value>,
    pub ai_extraction_guidance: String,
    pub business_context: String,
    pub regulatory_significance: Option<String>,
    pub cross_document_validation: Option<String>,
    pub source_documents: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Document type definition with AI extraction metadata
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DocumentType {
    pub document_type_id: Uuid,
    pub document_code: String,
    pub document_name: String,
    pub category: String,
    pub subcategory: Option<String>,
    pub description: Option<String>,
    pub issuing_authority: Option<String>,
    pub typical_jurisdictions: Vec<String>,
    pub regulatory_framework: Option<String>,
    pub validity_period_months: Option<i32>,
    pub renewal_required: bool,
    pub digital_format_accepted: bool,
    pub standardized_format: bool,
    pub multilingual_variants: Vec<String>,
    pub ai_extraction_complexity: String,
    pub ai_narrative: String,
    pub business_purpose: String,
    pub compliance_implications: Vec<String>,
    pub verification_methods: Vec<String>,
    pub related_document_types: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Document-attribute mapping with extraction guidance
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DocumentAttributeMapping {
    pub mapping_id: Uuid,
    pub document_type_code: String,
    pub attribute_id: Uuid,
    pub extraction_priority: i32,
    pub is_required: bool,
    pub field_location_hints: Vec<String>,
    pub validation_cross_refs: Option<Vec<String>>,
    pub ai_extraction_notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Complete AI extraction template for a document type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentExtractionTemplate {
    pub document_code: String,
    pub document_name: String,
    pub category: String,
    pub ai_narrative: String,
    pub attributes: Vec<AttributeExtractionSpec>,
}

/// Individual attribute extraction specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeExtractionSpec {
    pub attribute_code: String,
    pub attribute_name: String,
    pub data_type: String,
    pub required: bool,
    pub priority: i32,
    pub field_hints: Vec<String>,
    pub ai_guidance: String,
    pub validation_rules: Option<serde_json::Value>,
    pub privacy_class: String,
}

/// Cross-document validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossDocumentValidation {
    pub entity_identifier: String,
    pub attribute_code: String,
    pub unique_values: Vec<String>,
    pub consistency_score: f64,
    pub is_consistent: bool,
    pub requires_review: bool,
    pub extracted_from: HashMap<String, String>,
}

/// Mapping coverage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappingCoverageStats {
    pub document_code: String,
    pub document_name: String,
    pub category: String,
    pub mapped_attributes: i64,
    pub required_attributes: i64,
    pub high_priority_attributes: i64,
    pub attribute_codes: Vec<String>,
}

/// Attribute usage analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeUsageAnalysis {
    pub attribute_code: String,
    pub attribute_name: String,
    pub category: String,
    pub privacy_classification: String,
    pub appears_in_document_count: i64,
    pub document_types: Vec<String>,
    pub required_in_count: i64,
    pub avg_priority: Option<f64>,
}

/// Data bridge metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataBridgeMetrics {
    pub total_attributes: i64,
    pub total_document_types: i64,
    pub mapped_documents: i64,
    pub total_mappings: i64,
    pub universal_attributes: i64,
    pub protected_attributes: i64,
}

impl DocumentAttributeRepository {
    /// Create a new document-attribute repository
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ============================================================================
    // CONSOLIDATED ATTRIBUTES OPERATIONS
    // ============================================================================

    /// Get consolidated attribute by code
    pub async fn get_attribute_by_code(
        &self,
        attribute_code: &str,
    ) -> Result<Option<ConsolidatedAttribute>, DatabaseError> {
        let result = sqlx::query_as!(
            ConsolidatedAttribute,
            r#"
            SELECT
                attribute_id,
                attribute_code,
                attribute_name,
                data_type,
                category,
                subcategory,
                description,
                privacy_classification,
                validation_rules,
                extraction_patterns,
                ai_extraction_guidance,
                business_context,
                regulatory_significance,
                cross_document_validation,
                source_documents,
                created_at,
                updated_at
            FROM "ob-poc".consolidated_attributes
            WHERE attribute_code = $1
            "#,
            attribute_code
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(DatabaseError::SqlxError)?;

        Ok(result)
    }

    /// Get all attributes in a category
    pub async fn get_attributes_by_category(
        &self,
        category: &str,
    ) -> Result<Vec<ConsolidatedAttribute>, DatabaseError> {
        let results = sqlx::query_as!(
            ConsolidatedAttribute,
            r#"
            SELECT
                attribute_id,
                attribute_code,
                attribute_name,
                data_type,
                category,
                subcategory,
                description,
                privacy_classification,
                validation_rules,
                extraction_patterns,
                ai_extraction_guidance,
                business_context,
                regulatory_significance,
                cross_document_validation,
                source_documents,
                created_at,
                updated_at
            FROM "ob-poc".consolidated_attributes
            WHERE category = $1
            ORDER BY attribute_code
            "#,
            category
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::SqlxError)?;

        Ok(results)
    }

    /// Get all attributes with privacy classification
    pub async fn get_attributes_by_privacy_class(
        &self,
        privacy_class: &str,
    ) -> Result<Vec<ConsolidatedAttribute>, DatabaseError> {
        let results = sqlx::query_as!(
            ConsolidatedAttribute,
            r#"
            SELECT
                attribute_id,
                attribute_code,
                attribute_name,
                data_type,
                category,
                subcategory,
                description,
                privacy_classification,
                validation_rules,
                extraction_patterns,
                ai_extraction_guidance,
                business_context,
                regulatory_significance,
                cross_document_validation,
                source_documents,
                created_at,
                updated_at
            FROM "ob-poc".consolidated_attributes
            WHERE privacy_classification = $1
            ORDER BY category, attribute_code
            "#,
            privacy_class
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::SqlxError)?;

        Ok(results)
    }

    /// Create new consolidated attribute
    pub async fn create_attribute(
        &self,
        attribute: &ConsolidatedAttribute,
    ) -> Result<ConsolidatedAttribute, DatabaseError> {
        let result = sqlx::query_as!(
            ConsolidatedAttribute,
            r#"
            INSERT INTO "ob-poc".consolidated_attributes (
                attribute_id,
                attribute_code,
                attribute_name,
                data_type,
                category,
                subcategory,
                description,
                privacy_classification,
                validation_rules,
                extraction_patterns,
                ai_extraction_guidance,
                business_context,
                regulatory_significance,
                cross_document_validation,
                source_documents
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            RETURNING
                attribute_id,
                attribute_code,
                attribute_name,
                data_type,
                category,
                subcategory,
                description,
                privacy_classification,
                validation_rules,
                extraction_patterns,
                ai_extraction_guidance,
                business_context,
                regulatory_significance,
                cross_document_validation,
                source_documents,
                created_at,
                updated_at
            "#,
            attribute.attribute_id,
            attribute.attribute_code,
            attribute.attribute_name,
            attribute.data_type,
            attribute.category,
            attribute.subcategory,
            attribute.description,
            attribute.privacy_classification,
            attribute.validation_rules,
            attribute.extraction_patterns,
            attribute.ai_extraction_guidance,
            attribute.business_context,
            attribute.regulatory_significance,
            attribute.cross_document_validation,
            &attribute.source_documents
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::SqlxError)?;

        Ok(result)
    }

    // ============================================================================
    // DOCUMENT TYPES OPERATIONS
    // ============================================================================

    /// Get document type by code
    pub async fn get_document_type(
        &self,
        document_code: &str,
    ) -> Result<Option<DocumentType>, DatabaseError> {
        let result = sqlx::query_as!(
            DocumentType,
            r#"
            SELECT
                document_type_id,
                document_code,
                document_name,
                category,
                subcategory,
                description,
                issuing_authority,
                typical_jurisdictions,
                regulatory_framework,
                validity_period_months,
                renewal_required,
                digital_format_accepted,
                standardized_format,
                multilingual_variants,
                ai_extraction_complexity,
                ai_narrative,
                business_purpose,
                compliance_implications,
                verification_methods,
                related_document_types,
                created_at,
                updated_at
            FROM "ob-poc".document_types
            WHERE document_code = $1
            "#,
            document_code
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(DatabaseError::SqlxError)?;

        Ok(result)
    }

    /// Get all document types in category
    pub async fn get_document_types_by_category(
        &self,
        category: &str,
    ) -> Result<Vec<DocumentType>, DatabaseError> {
        let results = sqlx::query_as!(
            DocumentType,
            r#"
            SELECT
                document_type_id,
                document_code,
                document_name,
                category,
                subcategory,
                description,
                issuing_authority,
                typical_jurisdictions,
                regulatory_framework,
                validity_period_months,
                renewal_required,
                digital_format_accepted,
                standardized_format,
                multilingual_variants,
                ai_extraction_complexity,
                ai_narrative,
                business_purpose,
                compliance_implications,
                verification_methods,
                related_document_types,
                created_at,
                updated_at
            FROM "ob-poc".document_types
            WHERE category = $1
            ORDER BY document_name
            "#,
            category
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::SqlxError)?;

        Ok(results)
    }

    /// Get all document types
    pub async fn get_all_document_types(&self) -> Result<Vec<DocumentType>, DatabaseError> {
        let results = sqlx::query_as!(
            DocumentType,
            r#"
            SELECT
                document_type_id,
                document_code,
                document_name,
                category,
                subcategory,
                description,
                issuing_authority,
                typical_jurisdictions,
                regulatory_framework,
                validity_period_months,
                renewal_required,
                digital_format_accepted,
                standardized_format,
                multilingual_variants,
                ai_extraction_complexity,
                ai_narrative,
                business_purpose,
                compliance_implications,
                verification_methods,
                related_document_types,
                created_at,
                updated_at
            FROM "ob-poc".document_types
            ORDER BY category, document_name
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::SqlxError)?;

        Ok(results)
    }

    // ============================================================================
    // DOCUMENT-ATTRIBUTE MAPPINGS OPERATIONS
    // ============================================================================

    /// Get all attribute mappings for a document type
    pub async fn get_document_mappings(
        &self,
        document_code: &str,
    ) -> Result<Vec<DocumentAttributeMapping>, DatabaseError> {
        let results = sqlx::query_as!(
            DocumentAttributeMapping,
            r#"
            SELECT
                mapping_id,
                document_type_code,
                attribute_id,
                extraction_priority,
                is_required,
                field_location_hints,
                validation_cross_refs,
                ai_extraction_notes,
                created_at
            FROM "ob-poc".document_attribute_mappings
            WHERE document_type_code = $1
            ORDER BY extraction_priority, attribute_id
            "#,
            document_code
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::SqlxError)?;

        Ok(results)
    }

    /// Get all documents that use a specific attribute
    pub async fn get_documents_using_attribute(
        &self,
        attribute_code: &str,
    ) -> Result<Vec<DocumentAttributeMapping>, DatabaseError> {
        let results = sqlx::query_as!(
            DocumentAttributeMapping,
            r#"
            SELECT
                dam.mapping_id,
                dam.document_type_code,
                dam.attribute_id,
                dam.extraction_priority,
                dam.is_required,
                dam.field_location_hints,
                dam.validation_cross_refs,
                dam.ai_extraction_notes,
                dam.created_at
            FROM "ob-poc".document_attribute_mappings dam
            JOIN "ob-poc".consolidated_attributes ca ON dam.attribute_id = ca.attribute_id
            WHERE ca.attribute_code = $1
            ORDER BY dam.extraction_priority, dam.document_type_code
            "#,
            attribute_code
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::SqlxError)?;

        Ok(results)
    }

    // ============================================================================
    // AI EXTRACTION TEMPLATES
    // ============================================================================

    /// Get complete extraction template for a document type
    pub async fn get_document_extraction_template(
        &self,
        document_code: &str,
    ) -> Result<Option<DocumentExtractionTemplate>, DatabaseError> {
        // First get document metadata
        let doc_type = match self.get_document_type(document_code).await? {
            Some(doc) => doc,
            None => return Ok(None),
        };

        // Get all attribute specifications for this document
        let rows = sqlx::query!(
            r#"
            SELECT
                ca.attribute_code,
                ca.attribute_name,
                ca.data_type,
                ca.ai_extraction_guidance,
                ca.validation_rules,
                ca.privacy_classification,
                dam.extraction_priority,
                dam.is_required,
                dam.field_location_hints,
                dam.ai_extraction_notes
            FROM "ob-poc".consolidated_attributes ca
            JOIN "ob-poc".document_attribute_mappings dam ON ca.attribute_id = dam.attribute_id
            WHERE dam.document_type_code = $1
            ORDER BY dam.extraction_priority, ca.attribute_code
            "#,
            document_code
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::SqlxError)?;

        let attributes: Vec<AttributeExtractionSpec> = rows
            .into_iter()
            .map(|row| AttributeExtractionSpec {
                attribute_code: row.attribute_code,
                attribute_name: row.attribute_name,
                data_type: row.data_type,
                required: row.is_required,
                priority: row.extraction_priority,
                field_hints: row.field_location_hints,
                ai_guidance: row.ai_extraction_guidance,
                validation_rules: row.validation_rules,
                privacy_class: row.privacy_classification,
            })
            .collect();

        let template = DocumentExtractionTemplate {
            document_code: doc_type.document_code,
            document_name: doc_type.document_name,
            category: doc_type.category,
            ai_narrative: doc_type.ai_narrative,
            attributes,
        };

        Ok(Some(template))
    }

    /// Get extraction templates for all documents in a category
    pub async fn get_category_extraction_templates(
        &self,
        category: &str,
    ) -> Result<Vec<DocumentExtractionTemplate>, DatabaseError> {
        let doc_types = self.get_document_types_by_category(category).await?;
        let mut templates = Vec::new();

        for doc_type in doc_types {
            if let Some(template) = self
                .get_document_extraction_template(&doc_type.document_code)
                .await?
            {
                templates.push(template);
            }
        }

        Ok(templates)
    }

    // ============================================================================
    // CROSS-DOCUMENT VALIDATION
    // ============================================================================

    /// Validate cross-document consistency for an attribute
    pub async fn validate_cross_document_consistency(
        &self,
        entity_identifier: &str,
        attribute_code: &str,
        extracted_values: &HashMap<String, String>,
    ) -> Result<CrossDocumentValidation, DatabaseError> {
        // Convert HashMap to JSON for database function
        let values_json = serde_json::to_value(extracted_values)
            .map_err(|e| DatabaseError::SerializationError(e.to_string()))?;

        let result = sqlx::query!(
            r#"
            SELECT "ob-poc".validate_cross_document_data($1, $2, $3) as validation_result
            "#,
            entity_identifier,
            attribute_code,
            values_json
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::SqlxError)?;

        let validation: CrossDocumentValidation =
            serde_json::from_value(result.validation_result.unwrap_or_default())
                .map_err(|e| DatabaseError::SerializationError(e.to_string()))?;

        Ok(validation)
    }

    /// Get attributes requiring cross-document validation
    pub async fn get_cross_validation_attributes(
        &self,
    ) -> Result<Vec<ConsolidatedAttribute>, DatabaseError> {
        let results = sqlx::query_as!(
            ConsolidatedAttribute,
            r#"
            SELECT
                attribute_id,
                attribute_code,
                attribute_name,
                data_type,
                category,
                subcategory,
                description,
                privacy_classification,
                validation_rules,
                extraction_patterns,
                ai_extraction_guidance,
                business_context,
                regulatory_significance,
                cross_document_validation,
                source_documents,
                created_at,
                updated_at
            FROM "ob-poc".consolidated_attributes
            WHERE cross_document_validation IS NOT NULL
            ORDER BY attribute_code
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::SqlxError)?;

        Ok(results)
    }

    // ============================================================================
    // ANALYTICS AND REPORTING
    // ============================================================================

    /// Get mapping coverage statistics
    pub async fn get_mapping_coverage_stats(
        &self,
    ) -> Result<Vec<MappingCoverageStats>, DatabaseError> {
        let rows = sqlx::query!(
            r#"
            SELECT
                dt.document_code,
                dt.document_name,
                dt.category,
                COUNT(dam.attribute_id) as mapped_attributes,
                COUNT(CASE WHEN dam.is_required THEN 1 END) as required_attributes,
                COUNT(CASE WHEN dam.extraction_priority <= 2 THEN 1 END) as high_priority_attributes,
                array_agg(ca.attribute_code ORDER BY dam.extraction_priority) as attribute_codes
            FROM "ob-poc".document_types dt
            LEFT JOIN "ob-poc".document_attribute_mappings dam ON dt.document_code = dam.document_type_code
            LEFT JOIN "ob-poc".consolidated_attributes ca ON dam.attribute_id = ca.attribute_id
            GROUP BY dt.document_code, dt.document_name, dt.category
            ORDER BY mapped_attributes DESC, dt.document_code
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::SqlxError)?;

        let stats: Vec<MappingCoverageStats> = rows
            .into_iter()
            .map(|row| MappingCoverageStats {
                document_code: row.document_code,
                document_name: row.document_name,
                category: row.category,
                mapped_attributes: row.mapped_attributes.unwrap_or(0),
                required_attributes: row.required_attributes.unwrap_or(0),
                high_priority_attributes: row.high_priority_attributes.unwrap_or(0),
                attribute_codes: row.attribute_codes.unwrap_or_default(),
            })
            .collect();

        Ok(stats)
    }

    /// Get attribute usage analysis
    pub async fn get_attribute_usage_analysis(
        &self,
    ) -> Result<Vec<AttributeUsageAnalysis>, DatabaseError> {
        let rows = sqlx::query!(
            r#"
            SELECT
                ca.attribute_code,
                ca.attribute_name,
                ca.category,
                ca.privacy_classification,
                COUNT(dam.document_type_code) as appears_in_document_count,
                array_agg(dam.document_type_code ORDER BY dam.extraction_priority) as document_types,
                COUNT(CASE WHEN dam.is_required THEN 1 END) as required_in_count,
                AVG(dam.extraction_priority::float) as avg_priority
            FROM "ob-poc".consolidated_attributes ca
            LEFT JOIN "ob-poc".document_attribute_mappings dam ON ca.attribute_id = dam.attribute_id
            GROUP BY ca.attribute_code, ca.attribute_name, ca.category, ca.privacy_classification
            ORDER BY appears_in_document_count DESC, ca.attribute_code
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::SqlxError)?;

        let analysis: Vec<AttributeUsageAnalysis> = rows
            .into_iter()
            .map(|row| AttributeUsageAnalysis {
                attribute_code: row.attribute_code,
                attribute_name: row.attribute_name,
                category: row.category,
                privacy_classification: row.privacy_classification,
                appears_in_document_count: row.appears_in_document_count.unwrap_or(0),
                document_types: row.document_types.unwrap_or_default(),
                required_in_count: row.required_in_count.unwrap_or(0),
                avg_priority: row.avg_priority,
            })
            .collect();

        Ok(analysis)
    }

    /// Get data bridge metrics
    pub async fn get_data_bridge_metrics(&self) -> Result<DataBridgeMetrics, DatabaseError> {
        let total_attributes =
            sqlx::query_scalar!("SELECT COUNT(*) FROM \"ob-poc\".consolidated_attributes")
                .fetch_one(&self.pool)
                .await
                .map_err(DatabaseError::SqlxError)?
                .unwrap_or(0);

        let total_document_types =
            sqlx::query_scalar!("SELECT COUNT(*) FROM \"ob-poc\".document_types")
                .fetch_one(&self.pool)
                .await
                .map_err(DatabaseError::SqlxError)?
                .unwrap_or(0);

        let mapped_documents = sqlx::query_scalar!(
            "SELECT COUNT(DISTINCT document_type_code) FROM \"ob-poc\".document_attribute_mappings"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::SqlxError)?
        .unwrap_or(0);

        let total_mappings =
            sqlx::query_scalar!("SELECT COUNT(*) FROM \"ob-poc\".document_attribute_mappings")
                .fetch_one(&self.pool)
                .await
                .map_err(DatabaseError::SqlxError)?
                .unwrap_or(0);

        let universal_attributes = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) FROM (
                SELECT ca.attribute_code
                FROM "ob-poc".consolidated_attributes ca
                JOIN "ob-poc".document_attribute_mappings dam ON ca.attribute_id = dam.attribute_id
                GROUP BY ca.attribute_code
                HAVING COUNT(dam.document_type_code) >= 5
            ) universal_attrs
            "#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::SqlxError)?
        .unwrap_or(0);

        let protected_attributes = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) FROM "ob-poc".consolidated_attributes
            WHERE privacy_classification IN ('PII', 'confidential')
            "#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::SqlxError)?
        .unwrap_or(0);

        Ok(DataBridgeMetrics {
            total_attributes,
            total_document_types,
            mapped_documents,
            total_mappings,
            universal_attributes,
            protected_attributes,
        })
    }

    /// Test database connectivity for document-attribute tables
    pub async fn test_connection(&self) -> Result<(), DatabaseError> {
        sqlx::query!("SELECT 1 FROM \"ob-poc\".consolidated_attributes LIMIT 1")
            .fetch_optional(&self.pool)
            .await
            .map_err(DatabaseError::SqlxError)?;

        sqlx::query!("SELECT 1 FROM \"ob-poc\".document_types LIMIT 1")
            .fetch_optional(&self.pool)
            .await
            .map_err(DatabaseError::SqlxError)?;

        sqlx::query!("SELECT 1 FROM \"ob-poc\".document_attribute_mappings LIMIT 1")
            .fetch_optional(&self.pool)
            .await
            .map_err(DatabaseError::SqlxError)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;

    async fn create_test_pool() -> PgPool {
        let database_url = std::env::var("TEST_DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://localhost:5432/ob-poc-test".to_string());

        PgPoolOptions::new()
            .max_connections(2)
            .connect(&database_url)
            .await
            .expect("Failed to create test database pool")
    }

    #[tokio::test]
    async fn test_repository_creation() {
        let pool = create_test_pool().await;
        let repo = DocumentAttributeRepository::new(pool);

        // Test connection
        assert!(repo.test_connection().await.is_ok());
    }

    #[tokio::test]
    async fn test_get_attribute_by_code() {
        let pool = create_test_pool().await;
        let repo = DocumentAttributeRepository::new(pool);

        // Test getting a known attribute
        let result = repo.get_attribute_by_code("entity.legal_name").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_document_type() {
        let pool = create_test_pool().await;
        let repo = DocumentAttributeRepository::new(pool);

        // Test getting a known document type
        let result = repo.get_document_type("CERT_INCORPORATION").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_extraction_template() {
        let pool = create_test_pool().await;
        let repo = DocumentAttributeRepository::new(pool);

        // Test getting extraction template
        let result = repo.get_document_extraction_template("PASSPORT").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_data_bridge_metrics() {
        let pool = create_test_pool().await;
        let repo = DocumentAttributeRepository::new(pool);

        // Test getting metrics
        let result = repo.get_data_bridge_metrics().await;
        assert!(result.is_ok());

        let metrics = result.unwrap();
        assert!(metrics.total_attributes > 0);
        assert!(metrics.total_document_types > 0);
    }
}
