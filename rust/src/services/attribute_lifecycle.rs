//! Attribute lifecycle service - orchestrates complete attribute processing

use crate::data_dictionary::{AttributeId, DictionaryService};
use crate::services::{CompositeSinkExecutor, CompositeSourceExecutor};
use serde_json::Value;
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

pub struct AttributeLifecycleService {
    #[allow(dead_code)]
    pool: PgPool,
    source_executor: CompositeSourceExecutor,
    sink_executor: CompositeSinkExecutor,
}

impl AttributeLifecycleService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool: pool.clone(),
            source_executor: CompositeSourceExecutor::new(pool.clone()),
            sink_executor: CompositeSinkExecutor::new(pool),
        }
    }

    /// Complete lifecycle: fetch from source → validate → persist to sinks
    pub async fn process_attribute(
        &self,
        attribute_id: &AttributeId,
        entity_id: Uuid,
        dictionary_service: &dyn DictionaryService,
    ) -> Result<Value, String> {
        // Step 1: Get attribute definition
        let definition = dictionary_service
            .get_attribute(attribute_id)
            .await?
            .ok_or_else(|| format!("Attribute {} not found", attribute_id))?;

        // Step 2: Fetch value from best source
        let value = self
            .source_executor
            .fetch_from_best_source(attribute_id, &definition, entity_id)
            .await?
            .ok_or_else(|| format!("No value found for attribute {}", attribute_id))?;

        // Step 3: Validate value
        dictionary_service
            .validate_attribute_value(attribute_id, &value)
            .await?;

        // Step 4: Persist to all configured sinks
        self.sink_executor
            .persist_to_all_sinks(attribute_id, &value, &definition, entity_id)
            .await?;

        Ok(value)
    }

    /// Process multiple attributes in batch
    pub async fn process_attributes_batch(
        &self,
        attribute_ids: Vec<AttributeId>,
        entity_id: Uuid,
        dictionary_service: &dyn DictionaryService,
    ) -> Result<HashMap<AttributeId, Value>, String> {
        let mut results = HashMap::new();

        for attribute_id in attribute_ids {
            match self
                .process_attribute(&attribute_id, entity_id, dictionary_service)
                .await
            {
                Ok(value) => {
                    results.insert(attribute_id, value);
                }
                Err(e) => {
                    eprintln!("Failed to process attribute {}: {}", attribute_id, e);
                    // Continue processing other attributes
                }
            }
        }

        Ok(results)
    }

    /// Compile-time DSL resolution
    pub async fn resolve_dsl_attributes(
        &self,
        dsl: &str,
        entity_id: Uuid,
        dictionary_service: &dyn DictionaryService,
    ) -> Result<String, String> {
        // Step 1: Extract all attribute references
        let attribute_ids = dictionary_service.validate_dsl_attributes(dsl).await?;

        // Step 2: Process all attributes
        let values = self
            .process_attributes_batch(attribute_ids, entity_id, dictionary_service)
            .await?;

        // Step 3: Replace attribute references with values in DSL
        let mut resolved_dsl = dsl.to_string();
        for (attribute_id, value) in values {
            let pattern = format!("@attr{{{}}}", attribute_id);
            let replacement = match value {
                Value::String(s) => format!("\"{}\"", s),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                _ => value.to_string(),
            };
            resolved_dsl = resolved_dsl.replace(&pattern, &replacement);
        }

        Ok(resolved_dsl)
    }
}
