//! Source execution service for fetching attribute values from various sources

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
        cbu_id: Uuid,
    ) -> Result<Option<Value>, String> {
        // Fetch from document_metadata for most recent extraction
        // DB schema: doc_id, attribute_id (uuid), value (jsonb)
        let result = sqlx::query!(
            r#"
            SELECT dm.value
            FROM "ob-poc".document_metadata dm
            JOIN "ob-poc".document_catalog dc ON dc.doc_id = dm.doc_id
            WHERE dm.attribute_id = $1 AND dc.cbu_id = $2
            ORDER BY dc.created_at DESC
            LIMIT 1
            "#,
            attribute_id.as_uuid(),
            cbu_id
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
        // DB schema: entity_id, attribute_id (text), value_text, value_number, value_boolean, value_date, value_json
        let attr_id_str = attribute_id.as_uuid().to_string();

        let result = sqlx::query!(
            r#"
            SELECT
                value_text,
                value_number,
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
            // Return value based on which column has data
            let value = if let Some(text) = row.value_text {
                Some(Value::String(text))
            } else if let Some(num) = row.value_number {
                Value::Number(
                    serde_json::Number::from_f64(num.to_string().parse().unwrap_or(0.0)).unwrap(),
                )
                .into()
            } else if let Some(b) = row.value_boolean {
                Some(Value::Bool(b))
            } else if let Some(date) = row.value_date {
                Some(Value::String(date.to_string()))
            } else {
                row.value_json
            };
            Ok(value)
        } else {
            Ok(None)
        }
    }
}
