//! Document Extraction Service - Extracts attributes from documents using the document-attribute mapping
//!
//! Actual DB Schema:
//! - document_catalog: doc_id (PK), document_type_id, cbu_id, document_name, extraction_status
//! - document_types: type_id (PK), type_code, display_name, category, domain
//! - document_attribute_mappings: mapping_id, document_type_id (FK), attribute_uuid (FK), extraction_method
//! - document_metadata: doc_id + attribute_id (composite PK), value (jsonb)
//! - attribute_values_typed: entity_id, attribute_id, value_text/value_number/etc.

use ob_poc_authoring::data_dictionary::{AttributeId, DbAttributeDefinition, DictionaryService};
use serde_json::Value;
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

/// Mapping from document_attribute_mappings table
#[derive(Debug, Clone)]
pub(crate) struct DocumentAttributeMapping {
    pub mapping_id: Uuid,
    pub attribute_uuid: Uuid,
    pub extraction_method: String,
    pub is_required: bool,
    pub field_name: Option<String>,
}

/// Document info from document_catalog
#[derive(Debug, Clone)]
pub(crate) struct DocumentInfo {
    pub doc_id: Uuid,
    pub document_type_id: Uuid,
    pub document_type_code: String,
    pub cbu_id: Option<Uuid>,
    pub document_name: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct DocumentExtractionService {
    pool: PgPool,
}

impl DocumentExtractionService {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // =========================================================================
    // DOCUMENT LOOKUP
    // =========================================================================

    /// Get document info including type code
    pub(crate) async fn get_document_info(&self, doc_id: Uuid) -> Result<Option<DocumentInfo>, String> {
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
    pub(crate) async fn get_attribute_mappings_for_doc_type(
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
        _doc_id: Uuid,
        _attribute_id: Uuid,
        _value: &Value,
    ) -> Result<(), String> {
        // document_metadata table was removed in schema cleanup
        // Document extraction results should be stored in document_catalog.extracted_data jsonb field
        // This is a placeholder - full implementation would update document_catalog
        Ok(())
    }

    // =========================================================================
    // QUERIES
    // =========================================================================


}
