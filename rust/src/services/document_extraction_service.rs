//! Aligned with actual database schema
//! Live DB: document_catalog.document_id, document_metadata.extracted_value,
//!          attribute_values_typed uses cbu_id + string_value/numeric_value/etc.

use crate::data_dictionary::{DbAttributeDefinition, AttributeId, DictionaryService};
use serde_json::Value;
use sqlx::PgPool;
use std::collections::HashMap;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct DocumentExtractionService {
    pool: PgPool,
}

impl DocumentExtractionService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Extract attributes from an uploaded document
    pub async fn extract_attributes_from_document(
        &self,
        document_id: Uuid,
        cbu_id: Uuid,
        dictionary_service: &dyn DictionaryService,
    ) -> Result<HashMap<AttributeId, Value>, String> {
        // Step 1: Get document details - live DB uses document_id, not doc_id
        let _document = sqlx::query!(
            r#"
            SELECT
                document_id,
                file_path,
                mime_type,
                extracted_attributes
            FROM "ob-poc".document_catalog
            WHERE document_id = $1 AND cbu_id = $2
            "#,
            document_id,
            cbu_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Failed to fetch document: {}", e))?
        .ok_or_else(|| format!("Document {} not found", document_id))?;

        // Step 2: Get applicable attributes (all document-extractable attributes)
        let applicable_attributes = self.get_applicable_attributes().await?;

        let mut extracted_values = HashMap::new();

        // Step 3: Extract each attribute
        for attribute_id in applicable_attributes {
            if let Some(definition) = dictionary_service.get_attribute(&attribute_id).await? {
                // For now, use mock extraction - in production would read file from file_path
                if let Some(value) = self.mock_extract_single_attribute(&definition).await? {
                    // Step 4: Store in document_metadata (uses extracted_value TEXT column)
                    self.store_document_metadata(document_id, &attribute_id, &value)
                        .await?;

                    // Step 5: Store in attribute_values_typed (uses cbu_id)
                    self.store_attribute_value(&attribute_id, &value, cbu_id)
                        .await?;

                    extracted_values.insert(attribute_id, value);
                }
            }
        }

        // Step 6: Update document extraction status
        let confidence: f64 = 0.85;
        sqlx::query!(
            r#"
            UPDATE "ob-poc".document_catalog
            SET extraction_status = 'completed',
                extraction_confidence = $2,
                updated_at = CURRENT_TIMESTAMP
            WHERE document_id = $1
            "#,
            document_id,
            confidence
        )
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to update document status: {}", e))?;

        Ok(extracted_values)
    }

    async fn get_applicable_attributes(&self) -> Result<Vec<AttributeId>, String> {
        // Get attributes that can be extracted from documents
        let attributes = sqlx::query!(
            r#"
            SELECT DISTINCT d.attribute_id
            FROM "ob-poc".dictionary d
            WHERE d.source IS NOT NULL
                AND d.source::jsonb @> jsonb_build_object('source_type', 'document')
            LIMIT 10
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Failed to get applicable attributes: {}", e))?;

        Ok(attributes
            .into_iter()
            .map(|r| AttributeId::from_uuid(r.attribute_id))
            .collect())
    }

    // Mock extraction - in production would analyze actual document content
    async fn mock_extract_single_attribute(
        &self,
        definition: &DbAttributeDefinition,
    ) -> Result<Option<Value>, String> {
        // For now, return a mock value based on data type
        let mock_value = match definition.data_type.as_str() {
            "string" => Value::String(format!("Extracted {}", definition.name)),
            "number" | "numeric" => Value::Number(serde_json::Number::from_f64(42.0).unwrap()),
            "boolean" => Value::Bool(true),
            _ => Value::String("mock".to_string()),
        };

        Ok(Some(mock_value))
    }

    async fn store_document_metadata(
        &self,
        document_id: Uuid,
        attribute_id: &AttributeId,
        value: &Value,
    ) -> Result<(), String> {
        // Live DB schema: document_id, attribute_id (UUID), extracted_value (TEXT)
        // No ON CONFLICT - live table doesn't have unique constraint on (document_id, attribute_id)
        let extracted_value = match value {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };

        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".document_metadata (
                document_id, attribute_id, extracted_value
            ) VALUES ($1, $2, $3)
            "#,
            document_id,
            attribute_id.as_uuid(),
            extracted_value
        )
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to store document metadata: {}", e))?;

        Ok(())
    }

    async fn store_attribute_value(
        &self,
        attribute_id: &AttributeId,
        value: &Value,
        cbu_id: Uuid,
    ) -> Result<(), String> {
        // Live DB schema: cbu_id, attribute_id (UUID), value_type, string_value/numeric_value/etc.
        match value {
            Value::String(s) => {
                sqlx::query!(
                    r#"
                    INSERT INTO "ob-poc".attribute_values_typed (
                        cbu_id, attribute_id, value_type, string_value
                    ) VALUES ($1, $2, 'string', $3)
                    "#,
                    cbu_id,
                    attribute_id.as_uuid(),
                    s
                )
                .execute(&self.pool)
                .await
                .map_err(|e| format!("Failed to store string value: {}", e))?;
            }
            Value::Number(n) => {
                let num_val = bigdecimal::BigDecimal::from_str(&n.to_string()).unwrap();
                sqlx::query!(
                    r#"
                    INSERT INTO "ob-poc".attribute_values_typed (
                        cbu_id, attribute_id, value_type, numeric_value
                    ) VALUES ($1, $2, 'numeric', $3)
                    "#,
                    cbu_id,
                    attribute_id.as_uuid(),
                    num_val
                )
                .execute(&self.pool)
                .await
                .map_err(|e| format!("Failed to store number value: {}", e))?;
            }
            Value::Bool(b) => {
                sqlx::query!(
                    r#"
                    INSERT INTO "ob-poc".attribute_values_typed (
                        cbu_id, attribute_id, value_type, boolean_value
                    ) VALUES ($1, $2, 'boolean', $3)
                    "#,
                    cbu_id,
                    attribute_id.as_uuid(),
                    b
                )
                .execute(&self.pool)
                .await
                .map_err(|e| format!("Failed to store boolean value: {}", e))?;
            }
            Value::Object(_) | Value::Array(_) => {
                sqlx::query!(
                    r#"
                    INSERT INTO "ob-poc".attribute_values_typed (
                        cbu_id, attribute_id, value_type, json_value
                    ) VALUES ($1, $2, 'json', $3)
                    "#,
                    cbu_id,
                    attribute_id.as_uuid(),
                    value
                )
                .execute(&self.pool)
                .await
                .map_err(|e| format!("Failed to store JSON value: {}", e))?;
            }
            Value::Null => {
                // Skip null values
            }
        }

        Ok(())
    }
}
