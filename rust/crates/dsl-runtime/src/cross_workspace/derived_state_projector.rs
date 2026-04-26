//! DerivedStateProjector — composes a [`DagRegistry`] with a
//! [`DerivedStateEvaluator`] to project V1.3-2 cross-workspace
//! aggregates over a set of hydrated slot entities.
//!
//! Consumers (constellation projection pipeline, UI, agent narration)
//! call [`DerivedStateProjector::project_for`] with `(workspace, slot,
//! entity_id)` triples and receive back the fully-evaluated aggregate
//! states. The projector handles:
//!
//!   * Looking up which derived states apply to each host slot.
//!   * Running each evaluation against `SlotStateProvider`.
//!   * Bundling results with diagnostic info for surfacing.
//!
//! Stateless / thread-safe. Cheap to clone (Arc-only). Caching is the
//! caller's concern (per OQ-2: session-scope cache invalidated on
//! verb-touched slots).

use anyhow::Result;
use dsl_core::config::DagRegistry;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use super::derived_state::{DerivedStateEvaluator, DerivedStateValue};

/// One projected derived state for a specific host entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedStateProjection {
    /// Host slot identification.
    pub host_workspace: String,
    pub host_slot: String,
    pub host_entity_id: Uuid,
    /// The derived-state id (from the DAG declaration).
    pub derived_id: String,
    /// The host_state name (e.g. "operationally_active").
    pub host_state: String,
    /// Evaluated value with per-condition diagnostics.
    pub value: DerivedStateValue,
}

/// Projector — combines the registry (lookup) + evaluator (compute).
#[derive(Clone)]
pub struct DerivedStateProjector {
    registry: Arc<DagRegistry>,
    evaluator: Arc<DerivedStateEvaluator>,
}

impl DerivedStateProjector {
    pub fn new(registry: Arc<DagRegistry>, evaluator: Arc<DerivedStateEvaluator>) -> Self {
        Self {
            registry,
            evaluator,
        }
    }

    /// Project all derived states for a single host slot's entity.
    pub async fn project_for(
        &self,
        host_workspace: &str,
        host_slot: &str,
        host_entity_id: Uuid,
        pool: &PgPool,
    ) -> Result<Vec<DerivedStateProjection>> {
        let derived_specs = self
            .registry
            .derived_states_for_slot(host_workspace, host_slot);
        let mut out = Vec::with_capacity(derived_specs.len());
        for d in derived_specs {
            let value = self.evaluator.evaluate(d, host_entity_id, pool).await?;
            out.push(DerivedStateProjection {
                host_workspace: host_workspace.to_string(),
                host_slot: host_slot.to_string(),
                host_entity_id,
                derived_id: d.id.clone(),
                host_state: d.host_state.clone(),
                value,
            });
        }
        Ok(out)
    }

    /// Project derived states for a batch of host triples in one
    /// async pass. Caller pre-batches the (workspace, slot, entity_id)
    /// triples; results are aggregated.
    pub async fn project_batch(
        &self,
        targets: &[(String, String, Uuid)],
        pool: &PgPool,
    ) -> Result<Vec<DerivedStateProjection>> {
        let mut out = Vec::new();
        for (ws, slot, id) in targets {
            let projections = self.project_for(ws, slot, *id, pool).await?;
            out.extend(projections);
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cross_workspace::SameEntityResolver;
    use crate::cross_workspace::SlotStateProvider;
    use async_trait::async_trait;
    use dsl_core::config::dag::{Dag, LoadedDag};
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::sync::Mutex;

    fn ws_dag(yaml: &str) -> LoadedDag {
        let dag: Dag = serde_yaml::from_str(yaml).unwrap();
        LoadedDag {
            source_path: PathBuf::new(),
            dag,
        }
    }

    fn registry_from(workspaces: &[(&str, &str)]) -> Arc<DagRegistry> {
        let mut map = BTreeMap::new();
        for (name, yaml) in workspaces {
            map.insert(name.to_string(), ws_dag(yaml));
        }
        Arc::new(DagRegistry::from_loaded(map))
    }

    #[derive(Default)]
    struct MockSlotStateProvider {
        states: Mutex<std::collections::HashMap<(String, String, Uuid), Option<String>>>,
    }

    #[async_trait]
    impl SlotStateProvider for MockSlotStateProvider {
        async fn read_slot_state(
            &self,
            ws: &str,
            slot: &str,
            id: Uuid,
            _pool: &PgPool,
        ) -> Result<Option<String>> {
            let map = self.states.lock().unwrap();
            Ok(map
                .get(&(ws.to_string(), slot.to_string(), id))
                .cloned()
                .unwrap_or(None))
        }
    }

    #[test]
    fn projector_construction() {
        // Construct the projector against a registry that declares one
        // derived state. Full evaluation requires PgPool — exercised in
        // integration tests.
        let r = registry_from(&[(
            "cbu",
            r#"
workspace: cbu
dag_id: cbu_dag
slots:
  - id: cbu
    stateless: false
    state_machine: { id: cl, states: [{ id: VALIDATED }] }
derived_cross_workspace_state:
  - id: cbu_operationally_active
    host_workspace: cbu
    host_slot: cbu
    host_state: operationally_active
    derivation:
      all_of:
        - { workspace: kyc, slot: kyc_case, state: APPROVED }
"#,
        )]);
        let provider = Arc::new(MockSlotStateProvider::default());
        let resolver = Arc::new(SameEntityResolver);
        let evaluator = Arc::new(DerivedStateEvaluator::new(provider, resolver));
        let _projector = DerivedStateProjector::new(r, evaluator);
    }

    #[test]
    fn projector_no_derived_states_returns_empty() {
        // Registry with no derived states → project_for returns empty.
        let r = registry_from(&[(
            "demo",
            r#"
workspace: demo
dag_id: demo
slots:
  - id: thing
    stateless: true
"#,
        )]);
        // We only test the lookup path — registry.derived_states_for_slot
        // returns empty, so the projector's loop yields no output.
        let hits = r.derived_states_for_slot("demo", "thing");
        assert!(hits.is_empty());
    }
}
