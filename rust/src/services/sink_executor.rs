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
        entity_id: Uuid,
    ) -> Result<(), String> {
        let attribute_id_str = attribute_id.to_string();

        // Persist to attribute_values_typed based on value type
        match value {
            Value::String(s) => {
                sqlx::query!(
                    r#"
                    INSERT INTO "ob-poc".attribute_values_typed (
                        entity_id, attribute_id, value_text, attribute_uuid
                    ) VALUES ($1, $2, $3, $4)
                    "#,
                    entity_id,
                    attribute_id_str,
                    s,
                    attribute_id.as_uuid()
                )
                .execute(&self.pool)
                .await
                .map_err(|e| format!("Failed to persist string value: {}", e))?;
            }
            Value::Number(n) => {
                if let Some(int_val) = n.as_i64() {
                    sqlx::query!(
                        r#"
                        INSERT INTO "ob-poc".attribute_values_typed (
                            entity_id, attribute_id, value_integer, attribute_uuid
                        ) VALUES ($1, $2, $3, $4)
                        "#,
                        entity_id,
                        attribute_id_str,
                        int_val,
                        attribute_id.as_uuid()
                    )
                    .execute(&self.pool)
                    .await
                    .map_err(|e| format!("Failed to persist integer value: {}", e))?;
                } else {
                    let decimal = bigdecimal::BigDecimal::from_str(&n.to_string())
                        .map_err(|e| format!("Failed to parse number: {}", e))?;
                    sqlx::query!(
                        r#"
                        INSERT INTO "ob-poc".attribute_values_typed (
                            entity_id, attribute_id, value_number, attribute_uuid
                        ) VALUES ($1, $2, $3, $4)
                        "#,
                        entity_id,
                        attribute_id_str,
                        decimal,
                        attribute_id.as_uuid()
                    )
                    .execute(&self.pool)
                    .await
                    .map_err(|e| format!("Failed to persist number value: {}", e))?;
                }
            }
            Value::Bool(b) => {
                sqlx::query!(
                    r#"
                    INSERT INTO "ob-poc".attribute_values_typed (
                        entity_id, attribute_id, value_boolean, attribute_uuid
                    ) VALUES ($1, $2, $3, $4)
                    "#,
                    entity_id,
                    attribute_id_str,
                    b,
                    attribute_id.as_uuid()
                )
                .execute(&self.pool)
                .await
                .map_err(|e| format!("Failed to persist boolean value: {}", e))?;
            }
            Value::Object(_) | Value::Array(_) => {
                sqlx::query!(
                    r#"
                    INSERT INTO "ob-poc".attribute_values_typed (
                        entity_id, attribute_id, value_json, attribute_uuid
                    ) VALUES ($1, $2, $3, $4)
                    "#,
                    entity_id,
                    attribute_id_str,
                    value,
                    attribute_id.as_uuid()
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
