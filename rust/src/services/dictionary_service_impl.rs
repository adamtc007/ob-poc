//! DictionaryService implementation for attribute validation and management

use crate::data_dictionary::{AttributeId, DbAttributeDefinition, DictionaryService, SinkConfig, SourceConfig};
use async_trait::async_trait;
use sqlx::PgPool;

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
    ) -> Result<Option<DbAttributeDefinition>, String> {
        let record = sqlx::query!(
            r#"
            SELECT
                attribute_id,
                name,
                long_description,
                mask,
                source,
                sink,
                group_id,
                domain
            FROM "ob-poc".dictionary
            WHERE attribute_id = $1
            "#,
            attribute_id.as_uuid()
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        if let Some(rec) = record {
            // Parse source configuration from JSONB
            let source_config = if let Some(source_json) = rec.source {
                match serde_json::from_value::<SourceConfig>(source_json) {
                    Ok(config) => Some(sqlx::types::Json(config)),
                    Err(_) => {
                        // Provide default if parsing fails
                        Some(sqlx::types::Json(SourceConfig {
                            source_type: "document".to_string(),
                            extraction_rules: vec![],
                            priority: 0,
                        }))
                    }
                }
            } else {
                None
            };

            // Parse sink configuration from JSONB
            let sink_config = if let Some(sink_json) = rec.sink {
                match serde_json::from_value::<SinkConfig>(sink_json) {
                    Ok(config) => Some(sqlx::types::Json(config)),
                    Err(_) => {
                        // Provide default if parsing fails
                        Some(sqlx::types::Json(SinkConfig {
                            sink_type: "database".to_string(),
                            destinations: vec![],
                        }))
                    }
                }
            } else {
                None
            };

            Ok(Some(DbAttributeDefinition {
                attribute_id: AttributeId::from_uuid(rec.attribute_id),
                name: rec.name,
                long_description: rec.long_description,
                data_type: rec.mask.unwrap_or_else(|| "string".to_string()),
                source_config,
                sink_config,
                group_id: Some(rec.group_id),
                domain: rec.domain,
            }))
        } else {
            Ok(None)
        }
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
}
