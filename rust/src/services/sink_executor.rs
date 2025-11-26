//! Sink execution service for persisting attribute values to various destinations

use crate::data_dictionary::{AttributeId, DbAttributeDefinition};
use async_trait::async_trait;
use serde_json::Value;
use sqlx::PgPool;
use std::str::FromStr;
use uuid::Uuid;

#[async_trait]
pub trait SinkExecutor: Send + Sync {
    async fn persist_value(
        &self,
        attribute_id: &AttributeId,
        value: &Value,
        definition: &DbAttributeDefinition,
        entity_id: Uuid,
    ) -> Result<(), String>;
}

pub struct CompositeSinkExecutor {
    database_sink: Box<dyn SinkExecutor>,
}

impl CompositeSinkExecutor {
    pub fn new(pool: PgPool) -> Self {
        Self {
            database_sink: Box::new(DatabaseSink::new(pool)),
        }
    }

    pub async fn persist_to_all_sinks(
        &self,
        attribute_id: &AttributeId,
        value: &Value,
        definition: &DbAttributeDefinition,
        entity_id: Uuid,
    ) -> Result<(), String> {
        // Always persist to database
        self.database_sink
            .persist_value(attribute_id, value, definition, entity_id)
            .await?;

        // Additional sinks can be added here based on sink_config
        if let Some(sink_config) = &definition.sink_config {
            for _destination in &sink_config.destinations {
                // Future: implement webhook, cache, API sinks
                // For now, just database persistence
            }
        }

        Ok(())
    }
}

// Database sink implementation
struct DatabaseSink {
    pool: PgPool,
}

impl DatabaseSink {
    fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SinkExecutor for DatabaseSink {
    async fn persist_value(
        &self,
        attribute_id: &AttributeId,
        value: &Value,
        _definition: &DbAttributeDefinition,
        cbu_id: Uuid,
    ) -> Result<(), String> {
        // Persist to attribute_values_typed based on value type
        // Live DB schema uses: cbu_id, string_value, numeric_value, boolean_value, json_value, value_type
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
                .map_err(|e| format!("Failed to persist string value: {}", e))?;
            }
            Value::Number(n) => {
                let decimal = bigdecimal::BigDecimal::from_str(&n.to_string())
                    .map_err(|e| format!("Failed to parse number: {}", e))?;
                sqlx::query!(
                    r#"
                    INSERT INTO "ob-poc".attribute_values_typed (
                        cbu_id, attribute_id, value_type, numeric_value
                    ) VALUES ($1, $2, 'numeric', $3)
                    "#,
                    cbu_id,
                    attribute_id.as_uuid(),
                    decimal
                )
                .execute(&self.pool)
                .await
                .map_err(|e| format!("Failed to persist number value: {}", e))?;
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
                .map_err(|e| format!("Failed to persist boolean value: {}", e))?;
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
                .map_err(|e| format!("Failed to persist JSON value: {}", e))?;
            }
            Value::Null => {
                // Skip null values
            }
        }

        Ok(())
    }
}
