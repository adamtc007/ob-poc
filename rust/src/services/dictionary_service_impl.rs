//! DictionaryService implementation for attribute validation and management

use crate::data_dictionary::{AttributeDefinition, AttributeId, DictionaryService};
use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

pub struct DictionaryServiceImpl {
    pool: PgPool,
}

impl DictionaryServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DictionaryService for DictionaryServiceImpl {
    async fn validate_dsl_attributes(&self, dsl: &str) -> Result<Vec<AttributeId>, String> {
        // Parse DSL to find all @attr{uuid} references
        let attr_pattern = regex::Regex::new(r"@attr\{([a-f0-9-]+)\}").unwrap();
        let mut attribute_ids = Vec::new();

        for cap in attr_pattern.captures_iter(dsl) {
            if let Some(uuid_str) = cap.get(1) {
                let attr_id = AttributeId::from_str(uuid_str.as_str())
                    .map_err(|e| format!("Invalid attribute UUID: {}", e))?;

                // Verify it exists in dictionary
                let exists = sqlx::query!(
                    r#"SELECT COUNT(*) as count FROM "ob-poc".dictionary WHERE attribute_id = $1"#,
                    attr_id.as_uuid()
                )
                .fetch_one(&self.pool)
                .await
                .map_err(|e| format!("Database error: {}", e))?;

                if exists.count.unwrap_or(0) == 0 {
                    return Err(format!("Attribute {} not found in dictionary", attr_id));
                }

                attribute_ids.push(attr_id);
            }
        }

        Ok(attribute_ids)
    }

    async fn get_attribute(
        &self,
        attribute_id: &AttributeId,
    ) -> Result<Option<AttributeDefinition>, String> {
        let record = sqlx::query!(
            r#"
            SELECT
                attribute_id,
                name,
                long_description,
                mask,
                group_id,
                domain,
                source,
                sink
            FROM "ob-poc".dictionary
            WHERE attribute_id = $1
            "#,
            attribute_id.as_uuid()
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        // For now, return a simplified version since AttributeDefinition has many fields
        // In a full implementation, we'd map all the fields properly
        if record.is_some() {
            // TODO: Properly construct AttributeDefinition from database record
            return Err("AttributeDefinition mapping not yet implemented".to_string());
        }

        Ok(None)
    }

    async fn validate_attribute_value(
        &self,
        attribute_id: &AttributeId,
        value: &serde_json::Value,
    ) -> Result<(), String> {
        // Get attribute definition
        let record = sqlx::query!(
            r#"
            SELECT
                name,
                mask
            FROM "ob-poc".dictionary
            WHERE attribute_id = $1
            "#,
            attribute_id.as_uuid()
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        let record = record.ok_or_else(|| format!("Attribute {} not found", attribute_id))?;

        // Type validation based on mask field
        let mask = record.mask.as_deref().unwrap_or("string");
        match mask {
            "string" => {
                if !value.is_string() {
                    return Err(format!("Expected string for attribute {}", record.name));
                }
            }
            "number" | "numeric" => {
                if !value.is_number() {
                    return Err(format!("Expected number for attribute {}", record.name));
                }
            }
            "boolean" => {
                if !value.is_boolean() {
                    return Err(format!("Expected boolean for attribute {}", record.name));
                }
            }
            _ => {}
        }

        Ok(())
    }

    async fn extract_attributes_from_document(
        &self,
        doc_id: Uuid,
        cbu_id: Uuid,
    ) -> Result<Vec<AttributeId>, String> {
        // Get all extracted attributes from document_metadata
        // Note: document_metadata references doc_id, not document_id
        let attributes = sqlx::query!(
            r#"
            SELECT DISTINCT dm.attribute_id
            FROM "ob-poc".document_metadata dm
            JOIN "ob-poc".document_catalog dc ON dc.doc_id = dm.doc_id
            WHERE dm.doc_id = $1 AND dc.cbu_id = $2
            "#,
            doc_id,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        Ok(attributes
            .into_iter()
            .map(|r| AttributeId::from_uuid(r.attribute_id))
            .collect())
    }
}
