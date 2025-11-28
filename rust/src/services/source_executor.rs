//! Source execution service for fetching attribute values from various sources
//!
//! Actual schemas:
//! - document_metadata: doc_id, attribute_id (uuid), value (jsonb)
//! - attribute_values_typed: entity_id, attribute_id (text), value_text/value_number/value_boolean/value_json

use crate::data_dictionary::{AttributeId, DbAttributeDefinition};
use async_trait::async_trait;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

#[async_trait]
pub trait SourceExecutor: Send + Sync {
    async fn fetch_value(
        &self,
        attribute_id: &AttributeId,
        definition: &DbAttributeDefinition,
        entity_id: Uuid,
    ) -> Result<Option<Value>, String>;
}

pub struct CompositeSourceExecutor {
    document_source: Box<dyn SourceExecutor>,
    database_source: Box<dyn SourceExecutor>,
}

impl CompositeSourceExecutor {
    pub fn new(pool: PgPool) -> Self {
        Self {
            document_source: Box::new(DocumentSource::new(pool.clone())),
            database_source: Box::new(DatabaseSource::new(pool)),
        }
    }

    pub async fn fetch_from_best_source(
        &self,
        attribute_id: &AttributeId,
        definition: &DbAttributeDefinition,
        entity_id: Uuid,
    ) -> Result<Option<Value>, String> {
        if let Some(source_config) = &definition.source_config {
            // Try sources in priority order
            let executor: &dyn SourceExecutor = match source_config.source_type.as_str() {
                "document" => self.document_source.as_ref(),
                _ => self.database_source.as_ref(),
            };

            executor
                .fetch_value(attribute_id, definition, entity_id)
                .await
        } else {
            // Default to database source
            self.database_source
                .fetch_value(attribute_id, definition, entity_id)
                .await
        }
    }
}

// Document source implementation
struct DocumentSource {
    pool: PgPool,
}

impl DocumentSource {
    fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SourceExecutor for DocumentSource {
    async fn fetch_value(
        &self,
        attribute_id: &AttributeId,
        _definition: &DbAttributeDefinition,
        _entity_id: Uuid,
    ) -> Result<Option<Value>, String> {
        // Fetch from document_metadata for most recent extraction
        // Schema: doc_id, attribute_id (uuid), value (jsonb)
        // Note: document_metadata links to documents, not directly to entities
        // This would need a join through document_entity_links for entity-specific queries

        let result = sqlx::query!(
            r#"
            SELECT dm.value
            FROM "ob-poc".document_metadata dm
            WHERE dm.attribute_id = $1
            ORDER BY dm.created_at DESC
            LIMIT 1
            "#,
            attribute_id.as_uuid()
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        Ok(result.map(|row| row.value))
    }
}

// Database source implementation
struct DatabaseSource {
    pool: PgPool,
}

impl DatabaseSource {
    fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SourceExecutor for DatabaseSource {
    async fn fetch_value(
        &self,
        attribute_id: &AttributeId,
        _definition: &DbAttributeDefinition,
        entity_id: Uuid,
    ) -> Result<Option<Value>, String> {
        // Fetch from attribute_values_typed
        // Schema: entity_id, attribute_id (text), value_text/value_number/value_boolean/value_json
        let attr_id_str = attribute_id.to_string();

        let result = sqlx::query!(
            r#"
            SELECT
                value_text,
                value_number,
                value_integer,
                value_boolean,
                value_date,
                value_json
            FROM "ob-poc".attribute_values_typed
            WHERE attribute_id = $1 AND entity_id = $2
            ORDER BY created_at DESC
            LIMIT 1
            "#,
            attr_id_str,
            entity_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        if let Some(row) = result {
            // Return the first non-null value
            if let Some(s) = row.value_text {
                return Ok(Some(Value::String(s)));
            }
            if let Some(n) = row.value_number {
                let f: f64 = n.to_string().parse().unwrap_or(0.0);
                if let Some(num) = serde_json::Number::from_f64(f) {
                    return Ok(Some(Value::Number(num)));
                }
            }
            if let Some(i) = row.value_integer {
                return Ok(Some(Value::Number(serde_json::Number::from(i))));
            }
            if let Some(b) = row.value_boolean {
                return Ok(Some(Value::Bool(b)));
            }
            if let Some(d) = row.value_date {
                return Ok(Some(Value::String(d.to_string())));
            }
            if let Some(j) = row.value_json {
                return Ok(Some(j));
            }
        }

        Ok(None)
    }
}
