//! Attribute Executor
//!
//! Orchestrates attribute resolution by coordinating multiple sources
//! and sinks, implementing fallback logic and validation.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::domains::attributes::execution_context::{ExecutionContext, ValueSource};
// Note: ObPocError removed - not available in current error module
use super::document_catalog_source::{AttributeSource, SourceError};

/// Result type for executor operations
pub type ExecutorResult<T> = Result<T, ExecutorError>;

/// Executor-specific errors
#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    #[error("No value found for attribute {0}")]
    NoValueFound(Uuid),

    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    #[error("Source error: {0}")]
    SourceError(#[from] SourceError),

    #[error("Sink error: {0}")]
    SinkError(String),

    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
}

/// Trait for attribute sinks - persist attribute values
#[async_trait]
pub trait AttributeSink: Send + Sync {
    /// Write attribute value to this sink
    async fn write_value(
        &self,
        attribute_id: &Uuid,
        value: &serde_json::Value,
        context: &ExecutionContext,
    ) -> ExecutorResult<()>;

    /// Name of this sink for logging
    fn sink_name(&self) -> &'static str;
}

/// Dictionary for attribute definitions and validation
pub struct AttributeDictionary {
    pool: PgPool,
}

impl AttributeDictionary {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get attribute definition
    pub async fn get_attribute(&self, attribute_id: &Uuid) -> ExecutorResult<AttributeDef> {
        let attr = sqlx::query_as::<_, AttributeDef>(
            r#"
            SELECT attribute_id, name, mask as data_type, long_description as description
            FROM "ob-poc".dictionary
            WHERE attribute_id = $1
            "#,
        )
        .bind(attribute_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| ExecutorError::NoValueFound(*attribute_id))?;

        Ok(attr)
    }

    /// Validate attribute value against its definition
    pub async fn validate_attribute_value(
        &self,
        attribute_id: &Uuid,
        value: &serde_json::Value,
    ) -> ExecutorResult<()> {
        let attr = self.get_attribute(attribute_id).await?;

        // Basic type validation
        let valid = match attr.data_type.as_str() {
            "string" | "text" => value.is_string(),
            "integer" | "number" => value.is_number(),
            "boolean" => value.is_boolean(),
            "date" => value.is_string(), // TODO: Validate date format
            "json" | "jsonb" => true,    // Already JSON
            _ => true,                   // Unknown type, skip validation
        };

        if !valid {
            return Err(ExecutorError::ValidationFailed(format!(
                "Value type mismatch for attribute {}: expected {}, got {}",
                attr.name, attr.data_type, value
            )));
        }

        Ok(())
    }
}

/// Attribute definition from dictionary
#[derive(Debug, sqlx::FromRow)]
pub struct AttributeDef {
    pub attribute_id: Uuid,
    pub name: String,
    pub data_type: String,
    pub description: Option<String>,
}

/// Database sink - writes to attribute_values table
pub struct DatabaseSink {
    pool: PgPool,
}

impl DatabaseSink {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AttributeSink for DatabaseSink {
    async fn write_value(
        &self,
        attribute_id: &Uuid,
        value: &serde_json::Value,
        _context: &ExecutionContext,
    ) -> ExecutorResult<()> {
        // Note: In real implementation, we'd get CBU ID from context
        let cbu_id = Uuid::nil(); // TODO: Get from context

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".attribute_values
            (cbu_id, attribute_id, value, state, observed_at)
            VALUES ($1, $2, $3, 'resolved', NOW())
            ON CONFLICT (cbu_id, attribute_id, dsl_version)
            DO UPDATE SET
                value = EXCLUDED.value,
                observed_at = NOW()
            "#,
        )
        .bind(cbu_id)
        .bind(attribute_id)
        .bind(value)
        .execute(&self.pool)
        .await
        .map_err(|e| ExecutorError::SinkError(e.to_string()))?;

        Ok(())
    }

    fn sink_name(&self) -> &'static str {
        "database"
    }
}

/// Attribute executor - coordinates sources and sinks
pub struct AttributeExecutor {
    sources: Vec<Arc<dyn AttributeSource>>,
    sinks: Vec<Arc<dyn AttributeSink>>,
    dictionary: AttributeDictionary,
}

impl AttributeExecutor {
    /// Create new executor with sources and sinks
    pub fn new(
        sources: Vec<Arc<dyn AttributeSource>>,
        sinks: Vec<Arc<dyn AttributeSink>>,
        dictionary: AttributeDictionary,
    ) -> Self {
        // Sort sources by priority (highest first)
        let mut sources = sources;
        sources.sort_by(|a, b| b.priority().cmp(&a.priority()));

        Self {
            sources,
            sinks,
            dictionary,
        }
    }

    /// Resolve attribute with fallback chain
    pub async fn resolve_attribute(
        &self,
        attribute_id: &Uuid,
        context: &ExecutionContext,
    ) -> ExecutorResult<serde_json::Value> {
        // Get attribute definition
        let _attr_def = self.dictionary.get_attribute(attribute_id).await?;

        // Try sources in priority order
        let mut value = None;
        for source in &self.sources {
            tracing::debug!(
                "Trying source '{}' for attribute {}",
                source.source_name(),
                attribute_id
            );

            match source.get_value(attribute_id, context).await {
                Ok(Some(v)) => {
                    tracing::info!(
                        "Found value from source '{}' for attribute {}",
                        source.source_name(),
                        attribute_id
                    );
                    value = Some(v);
                    break;
                }
                Ok(None) => {
                    tracing::debug!(
                        "Source '{}' returned no value for attribute {}",
                        source.source_name(),
                        attribute_id
                    );
                    continue;
                }
                Err(e) => {
                    tracing::warn!(
                        "Source '{}' failed for attribute {}: {}",
                        source.source_name(),
                        attribute_id,
                        e
                    );
                    continue;
                }
            }
        }

        let value = value.ok_or_else(|| ExecutorError::NoValueFound(*attribute_id))?;

        // Validate
        self.dictionary
            .validate_attribute_value(attribute_id, &value)
            .await?;

        // Persist to sinks
        for sink in &self.sinks {
            if let Err(e) = sink.write_value(attribute_id, &value, context).await {
                tracing::error!(
                    "Failed to write to sink '{}' for attribute {}: {}",
                    sink.sink_name(),
                    attribute_id,
                    e
                );
                // Continue even if sink fails - we still have the value
            } else {
                tracing::debug!(
                    "Wrote value to sink '{}' for attribute {}",
                    sink.sink_name(),
                    attribute_id
                );
            }
        }

        Ok(value)
    }

    /// Batch resolve multiple attributes
    pub async fn batch_resolve(
        &self,
        attribute_ids: &[Uuid],
        context: &ExecutionContext,
    ) -> Vec<ExecutorResult<serde_json::Value>> {
        let mut results = Vec::new();
        for attr_id in attribute_ids {
            results.push(self.resolve_attribute(attr_id, context).await);
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::document_catalog_source::DocumentCatalogSource;
    use crate::services::extraction_service::MockExtractionService;

    #[tokio::test]
    async fn test_attribute_executor_fallback() {
        // This test would require a test database connection
        // For now, it's a placeholder showing the structure
    }

    #[test]
    fn test_executor_source_ordering() {
        // Verify sources are sorted by priority
        // Placeholder test
    }
}
