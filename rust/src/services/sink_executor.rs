//! Sink execution service — DEPRECATED.
//!
//! Typed attribute values are now persisted through the canonical DSL verb
//! `typed-attribute.record` (CRUD insert into `attribute_values_typed`).
//!
//! This module is retained only for backward-compatible re-exports.
//! No raw INSERT statements remain here.

use async_trait::async_trait;
use ob_poc_authoring::data_dictionary::{AttributeId, DbAttributeDefinition};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

#[async_trait]
pub(crate) trait SinkExecutor: Send + Sync {
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
pub(crate) struct CompositeSinkExecutor {
    _pool: PgPool,
}

impl CompositeSinkExecutor {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { _pool: pool }
    }

}
