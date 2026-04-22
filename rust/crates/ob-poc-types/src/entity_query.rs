//! `EntityQueryResult` — structured result from `entity.query` verb.
//!
//! Relocated from `dsl-runtime::domain_ops::entity_query` in Phase
//! 5c-migrate Phase B slice #70 so the type lives upstream of both
//! `dsl-runtime` (where the legacy verb impl reads it) and
//! `sem_os_postgres` (where the YAML-first port writes it). Consumed
//! as a variant of `dsl_v2::executor::ExecutionResult::EntityQuery`
//! plus the idempotency cache projection.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Entity query result — a list of `(entity_id, name)` tuples for
/// batch iteration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EntityQueryResult {
    /// Entity items: `(entity_id, display_name)`
    pub items: Vec<(Uuid, String)>,
    /// Entity type queried (if the caller scoped to one).
    pub entity_type: Option<String>,
    /// Total count (may differ from `items.len()` if limited).
    pub total_count: usize,
}

impl EntityQueryResult {
    /// Get entity IDs only.
    pub fn entity_ids(&self) -> Vec<Uuid> {
        self.items.iter().map(|(id, _)| *id).collect()
    }
}
