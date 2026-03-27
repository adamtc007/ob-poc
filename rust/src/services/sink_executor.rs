//! Sink execution service — DEPRECATED.
//!
//! Typed attribute values are now persisted through the canonical DSL verb
//! `typed-attribute.record` (CRUD insert into `attribute_values_typed`).
//!
//! This module is retained only for backward-compatible re-exports.
//! No raw INSERT statements remain here.

use crate::data_dictionary::{AttributeId, DbAttributeDefinition};
use async_trait::async_trait;
use serde_json::Value;
use sqlx::PgPool;
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

/// Composite sink executor — routes to the canonical DSL verb pipeline.
///
/// Direct database writes have been removed. Use `typed-attribute.record`
/// verb instead.
pub struct CompositeSinkExecutor {
    _pool: PgPool,
}

impl CompositeSinkExecutor {
    pub fn new(pool: PgPool) -> Self {
        Self { _pool: pool }
    }

    pub async fn persist_to_all_sinks(
        &self,
        _attribute_id: &AttributeId,
        _value: &Value,
        _definition: &DbAttributeDefinition,
        _entity_id: Uuid,
    ) -> Result<(), String> {
        Err(
            "Direct sink writes are deprecated. Use the typed-attribute.record DSL verb instead."
                .into(),
        )
    }
}
