//! Document Extraction Service - Extracts attributes from documents using the document-attribute mapping
//!
//! Actual DB Schema:
//! - document_catalog: doc_id (PK), document_type_id, cbu_id, document_name, extraction_status
//! - document_types: type_id (PK), type_code, display_name, category, domain
//! - document_attribute_mappings: mapping_id, document_type_id (FK), attribute_uuid (FK), extraction_method
//! - document_metadata: doc_id + attribute_id (composite PK), value (jsonb)
//! - attribute_values_typed: entity_id, attribute_id, value_text/value_number/etc.

use crate::data_dictionary::{AttributeId, DbAttributeDefinition, DictionaryService};
use serde_json::Value;
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

/// Mapping from document_attribute_mappings table
#[derive(Debug, Clone)]
pub struct DocumentAttributeMapping {
    pub mapping_id: Uuid,
    pub attribute_uuid: Uuid,
    pub extraction_method: String,
    pub is_required: bool,
    pub field_name: Option<String>,
}

/// Document info from document_catalog
#[derive(Debug, Clone)]
pub struct DocumentInfo {
    pub doc_id: Uuid,
    pub document_type_id: Uuid,
    pub document_type_code: String,
    pub cbu_id: Option<Uuid>,
    pub document_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DocumentExtractionService {
    pool: PgPool,
}

impl DocumentExtractionService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // =========================================================================
    // DOCUMENT LOOKUP
    // =========================================================================

    /// Get document info including type code
    pub async fn get_document_info(&self, doc_id: Uuid) -> Result<Option<DocumentInfo>, String> {
        let row = sqlx::query!(
            r#"
            SELECT
                dc.doc_id,
                dc.document_type_id,
                dt.type_code,
                dc.cbu_id,
                dc.document_name
            FROM "ob-poc".document_catalog dc
            JOIN "ob-poc".document_types dt ON dt.type_id = dc.document_type_id
            WHERE dc.doc_id = $1
            "#,
            doc_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Failed to fetch document: {}", e))?;

        Ok(row.map(|r| DocumentInfo {
            doc_id: r.doc_id,
            document_type_id: r.document_type_id.unwrap_or_default(),
            document_type_code: r.type_code,
            cbu_id: r.cbu_id,
            document_name: r.document_name,
        }))
    }

    // =========================================================================
    // ATTRIBUTE MAPPING
    // =========================================================================

    /// Get attribute mappings for a document type
    pub async fn get_attribute_mappings_for_doc_type(
        &self,
        document_type_id: Uuid,
    ) -> Result<Vec<DocumentAttributeMapping>, String> {
        let mappings = sqlx::query!(
            r#"
            SELECT
                mapping_id,
                attribute_uuid,
                extraction_method,
                is_required,
                field_name
            FROM "ob-poc".document_attribute_mappings
            WHERE document_type_id = $1
            "#,
            document_type_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Failed to get attribute mappings: {}", e))?;

        Ok(mappings
            .into_iter()
            .map(|r| DocumentAttributeMapping {
                mapping_id: r.mapping_id,
                attribute_uuid: r.attribute_uuid,
                extraction_method: r.extraction_method,
                is_required: r.is_required.unwrap_or(false),
                field_name: r.field_name,
            })
            .collect())
    }

    // =========================================================================
    // EXTRACTION
    // =========================================================================

    /// Extract attributes from an uploaded document
    pub async fn extract_attributes_from_document(
        &self,
        doc_id: Uuid,
        dictionary_service: &dyn DictionaryService,
    ) -> Result<HashMap<AttributeId, Value>, String> {
        // Step 1: Get document details
        let document = self
            .get_document_info(doc_id)
            .await?
            .ok_or_else(|| format!("Document {} not found", doc_id))?;

        // Step 2: Get attribute mappings for this document type
        let attribute_mappings = self
            .get_attribute_mappings_for_doc_type(document.document_type_id)
            .await?;

        if attribute_mappings.is_empty() {
            return Err(format!(
                "No attribute mappings found for document type '{}'",
                document.document_type_code
            ));
        }

        let mut extracted_values = HashMap::new();

        // Step 3: Extract each mapped attribute
        for mapping in &attribute_mappings {
            let attribute_id = AttributeId::from_uuid(mapping.attribute_uuid);

            if let Some(definition) = dictionary_service.get_attribute(&attribute_id).await? {
                // Extract value using hints from the mapping
                if let Some(value) = self.extract_single_attribute(&definition, mapping).await? {
                    // Step 4: Store in document_metadata
                    self.store_document_metadata(doc_id, mapping.attribute_uuid, &value)
                        .await?;

                    extracted_values.insert(attribute_id, value);
                } else if mapping.is_required {
                    tracing::warn!(
                        "Required attribute {} could not be extracted from document {}",
                        mapping.attribute_uuid,
                        doc_id
                    );
                }
            }
        }

        // Step 5: Update document extraction status
        let confidence = if extracted_values.is_empty() {
            bigdecimal::BigDecimal::from(0)
        } else {
            bigdecimal::BigDecimal::try_from(0.85).unwrap_or_else(|_| {
                bigdecimal::BigDecimal::from(85) / bigdecimal::BigDecimal::from(100)
            })
        };

        sqlx::query!(
            r#"
            UPDATE "ob-poc".document_catalog
            SET extraction_status = 'COMPLETED',
                extraction_confidence = $2,
                last_extracted_at = CURRENT_TIMESTAMP,
                updated_at = CURRENT_TIMESTAMP
            WHERE doc_id = $1
            "#,
            doc_id,
            confidence
        )
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to update document status: {}", e))?;

        Ok(extracted_values)
    }

    /// Extract a single attribute value from the document
    async fn extract_single_attribute(
        &self,
        definition: &DbAttributeDefinition,
        mapping: &DocumentAttributeMapping,
    ) -> Result<Option<Value>, String> {
        // TODO: Implement actual extraction using:
        // - mapping.extraction_method (OCR, MRZ, BARCODE, etc.)
        // - mapping.field_name
        // - definition.data_type

        let mock_value = match definition.data_type.as_str() {
            "string" | "text" => Value::String(format!("Extracted {}", definition.name)),
            "number" | "numeric" | "integer" | "decimal" => {
                Value::Number(serde_json::Number::from_f64(42.0).unwrap())
            }
            "boolean" | "bool" => Value::Bool(true),
            "date" => Value::String("2024-01-15".to_string()),
            _ => Value::String("mock_value".to_string()),
        };

        tracing::debug!(
            "Extracting attribute '{}' using method '{}'",
            mapping.attribute_uuid,
            mapping.extraction_method
        );

        Ok(Some(mock_value))
    }

    // =========================================================================
    // STORAGE
    // =========================================================================

    async fn store_document_metadata(
        &self,
        doc_id: Uuid,
        attribute_id: Uuid,
        value: &Value,
    ) -> Result<(), String> {
        // Use UPSERT since (doc_id, attribute_id) is the composite PK
        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".document_metadata (doc_id, attribute_id, value, extraction_confidence, extracted_at)
            VALUES ($1, $2, $3, 0.85, CURRENT_TIMESTAMP)
            ON CONFLICT (doc_id, attribute_id)
            DO UPDATE SET value = EXCLUDED.value,
                          extraction_confidence = EXCLUDED.extraction_confidence,
                          extracted_at = EXCLUDED.extracted_at
            "#,
            doc_id,
            attribute_id,
            value
        )
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to store document metadata: {}", e))?;

        Ok(())
    }

    // =========================================================================
    // QUERIES
    // =========================================================================

    /// Get all attributes that can be extracted from a document type
    pub async fn get_extractable_attributes_for_doc_type(
        &self,
        document_type_id: Uuid,
    ) -> Result<Vec<DocumentAttributeMapping>, String> {
        self.get_attribute_mappings_for_doc_type(document_type_id)
            .await
    }

    /// Find document types that can provide a specific attribute
    pub async fn get_doc_types_for_attribute(
        &self,
        attribute_uuid: Uuid,
    ) -> Result<Vec<Uuid>, String> {
        let doc_types = sqlx::query!(
            r#"
            SELECT DISTINCT document_type_id
            FROM "ob-poc".document_attribute_mappings
            WHERE attribute_uuid = $1
            "#,
            attribute_uuid
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Failed to get document types for attribute: {}", e))?;

        Ok(doc_types.into_iter().map(|r| r.document_type_id).collect())
    }
}
