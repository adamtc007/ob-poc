//! Document extraction service for extracting attributes from uploaded documents
//! Aligned with actual database schema

use crate::data_dictionary::{AttributeDefinition, AttributeId, DictionaryService};
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
        doc_id: Uuid,
        cbu_id: Uuid,
        dictionary_service: &dyn DictionaryService,
    ) -> Result<HashMap<AttributeId, Value>, String> {
        // Step 1: Get document details from actual schema
        let document = sqlx::query!(
            r#"
            SELECT
                doc_id,
                storage_key,
                mime_type,
                extracted_data
            FROM "ob-poc".document_catalog
            WHERE doc_id = $1 AND cbu_id = $2
            "#,
            doc_id,
            cbu_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Failed to fetch document: {}", e))?
        .ok_or_else(|| format!("Document {} not found", doc_id))?;

        // Step 2: Get applicable attributes (all document-extractable attributes)
        let applicable_attributes = self.get_applicable_attributes().await?;

        let mut extracted_values = HashMap::new();

        // Step 3: Extract each attribute
        for attribute_id in applicable_attributes {
            if let Some(definition) = dictionary_service.get_attribute(&attribute_id).await? {
                // For now, use mock extraction - in production would read file from storage_key
                if let Some(value) = self.mock_extract_single_attribute(&definition).await? {
                    // Step 4: Store in document_metadata (uses 'value' JSONB column)
                    self.store_document_metadata(doc_id, &attribute_id, &value)
                        .await?;

                    // Step 5: Store in attribute_values_typed (uses entity_id, not cbu_id)
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
                last_extracted_at = CURRENT_TIMESTAMP,
                updated_at = CURRENT_TIMESTAMP
            WHERE doc_id = $1
            "#,
            doc_id,
            bigdecimal::BigDecimal::from_str(&confidence.to_string()).unwrap()
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
        definition: &AttributeDefinition,
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
        doc_id: Uuid,
        attribute_id: &AttributeId,
        value: &Value,
    ) -> Result<(), String> {
        // document_metadata schema: doc_id, attribute_id, value (JSONB)
        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".document_metadata (
                doc_id, attribute_id, value
            ) VALUES ($1, $2, $3)
            ON CONFLICT (doc_id, attribute_id) DO UPDATE SET
                value = EXCLUDED.value
            "#,
            doc_id,
            attribute_id.as_uuid(),
            value
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
        entity_id: Uuid, // Using entity_id as per actual schema
    ) -> Result<(), String> {
        // attribute_values_typed has: entity_id, attribute_id (text), value_* columns
        let attribute_id_str = attribute_id.to_string();

        // Decompose value into typed columns
        match value {
            Value::String(s) => {
                sqlx::query!(
                    r#"
                    INSERT INTO "ob-poc".attribute_values_typed (
                        entity_id, attribute_id, value_text
                    ) VALUES ($1, $2, $3)
                    "#,
                    entity_id,
                    attribute_id_str,
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
                        entity_id, attribute_id, value_number
                    ) VALUES ($1, $2, $3)
                    "#,
                    entity_id,
                    attribute_id_str,
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
                        entity_id, attribute_id, value_boolean
                    ) VALUES ($1, $2, $3)
                    "#,
                    entity_id,
                    attribute_id_str,
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
                        entity_id, attribute_id, value_json
                    ) VALUES ($1, $2, $3)
                    "#,
                    entity_id,
                    attribute_id_str,
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
