//! HierarchyCascade — runtime planner for V1.3-3 parent_slot cascade
//! propagation.
//!
//! When a parent slot transitions, child slots (those declaring
//! `parent_slot:` pointing back at the parent) may need to react. The
//! reaction is governed by the child's `state_dependency.cascade_rules`:
//!
//! ```yaml
//! state_dependency:
//!   cascade_rules:
//!     - parent_state: SUSPENDED
//!       child_allowed_states: [SUSPENDED]
//!       cascade_on_parent_transition: true
//!       default_child_state_on_cascade: SUSPENDED
//! ```
//!
//! The planner answers: given a parent that just transitioned to state
//! X, what list of (child_workspace, child_slot, target_state) cascade
//! actions should fire? It does NOT execute them — that's the
//! orchestrator's job (each cascade target may itself be a verb call,
//! gate-checked, and may chain into further cascades).
//!
//! The planner returns one entry PER CHILD ENTITY that needs to react.
//! Looking up the actual child rows (which entities are children of
//! this parent?) is delegated to the caller via a [`ChildEntityResolver`]
//! trait — the planner doesn't know about cbu_entity_relationships
//! tables, FK joins, etc.

use anyhow::Result;
use async_trait::async_trait;
use dsl_core::config::dag::CascadeRule;
use dsl_core::config::DagRegistry;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// ChildEntityResolver — caller-supplied "which entities are children
// of this parent?" lookup.
// ---------------------------------------------------------------------------

/// Resolves the entity_ids of all children belonging to a specific
/// parent entity, for a given (parent_slot, child_slot) relationship.
///
/// Implementations consult the `parent_slot.join` clause (via, parent_fk,
/// child_fk) to construct the right SQL.
#[async_trait]
pub trait ChildEntityResolver: Send + Sync {
    /// List entity_ids of children belonging to `parent_entity_id`.
    ///
    /// `parent_workspace` / `parent_slot` and `child_workspace` /
    /// `child_slot` together identify the relationship; the
    /// implementation knows how to walk the join.
    async fn list_children(
        &self,
        parent_workspace: &str,
        parent_slot: &str,
        parent_entity_id: Uuid,
        child_workspace: &str,
        child_slot: &str,
        pool: &PgPool,
    ) -> Result<Vec<Uuid>>;
}

/// No-op resolver — returns an empty list. Useful for tests where no
/// real child rows exist yet.
#[derive(Debug, Default, Clone)]
pub struct NoChildrenResolver;

#[async_trait]
impl ChildEntityResolver for NoChildrenResolver {
    async fn list_children(
        &self,
        _: &str,
        _: &str,
        _: Uuid,
        _: &str,
        _: &str,
        _: &PgPool,
    ) -> Result<Vec<Uuid>> {
        Ok(Vec::new())
    }
}

// ---------------------------------------------------------------------------
// CascadeAction — one planned cascade transition.
// ---------------------------------------------------------------------------

/// One planned cascade action — "transition this child entity to this
/// state because of a parent transition".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CascadeAction {
    pub child_workspace: String,
    pub child_slot: String,
    pub child_entity_id: Uuid,
    /// The target state to apply to the child.
    pub target_state: String,
    /// The declared `child_allowed_states` for this rule. The
    /// orchestrator validates the child's pre-cascade state is in this
    /// set before applying (lest cascade conflict with child's own
    /// invariants).
    pub allowed_states: Vec<String>,
    /// Severity from the child slot's state_dependency declaration.
    pub severity: String,
    /// The rule that produced this action — for diagnostics.
    pub rule_parent_state: String,
}

// ---------------------------------------------------------------------------
// CascadePlanner — the planning entry point.
// ---------------------------------------------------------------------------

/// V1.3-3 cascade planner. Stateless; cheap to clone.
#[derive(Clone)]
pub struct CascadePlanner {
    registry: Arc<DagRegistry>,
    child_resolver: Arc<dyn ChildEntityResolver>,
}

impl CascadePlanner {
    pub fn new(
        registry: Arc<DagRegistry>,
        child_resolver: Arc<dyn ChildEntityResolver>,
    ) -> Self {
        Self {
            registry,
            child_resolver,
        }
    }

    /// Plan all cascade actions that should fire as a result of a
    /// parent slot's transition into `parent_new_state`.
    ///
    /// Returns one [`CascadeAction`] per (child slot type) × (child
    /// entity), filtered to those rules where
    /// `cascade_on_parent_transition: true` and a
    /// `default_child_state_on_cascade` is declared.
    pub async fn plan_cascade(
        &self,
        parent_workspace: &str,
        parent_slot: &str,
        parent_entity_id: Uuid,
        parent_new_state: &str,
        pool: &PgPool,
    ) -> Result<Vec<CascadeAction>> {
        let mut actions = Vec::new();

        // Walk all child slot types declared with this parent.
        let child_keys: Vec<_> = self
            .registry
            .children_of(parent_workspace, parent_slot)
            .to_vec();
        for child_key in child_keys {
            let dep = match self
                .registry
                .state_dependency_for(&child_key.workspace, &child_key.slot)
            {
                Some(d) => d,
                None => continue, // child has parent_slot but no cascade rules
            };

            // Find the rule whose parent_state matches the new parent state.
            let rule = match dep
                .cascade_rules
                .iter()
                .find(|r| r.parent_state == parent_new_state)
            {
                Some(r) => r,
                None => continue, // no rule for this parent state
            };

            if !rule.cascade_on_parent_transition {
                continue;
            }
            let target_state = match &rule.default_child_state_on_cascade {
                Some(s) => s.clone(),
                None => continue,
            };

            // Resolve which child entities belong to this parent.
            let child_ids = self
                .child_resolver
                .list_children(
                    parent_workspace,
                    parent_slot,
                    parent_entity_id,
                    &child_key.workspace,
                    &child_key.slot,
                    pool,
                )
                .await?;

            for child_id in child_ids {
                actions.push(CascadeAction {
                    child_workspace: child_key.workspace.clone(),
                    child_slot: child_key.slot.clone(),
                    child_entity_id: child_id,
                    target_state: target_state.clone(),
                    allowed_states: rule.child_allowed_states.clone(),
                    severity: severity_str(&dep.severity),
                    rule_parent_state: rule.parent_state.clone(),
                });
            }
        }

        Ok(actions)
    }
}

fn severity_str(s: &dsl_core::config::dag::Severity) -> String {
    use dsl_core::config::dag::Severity::*;
    match s {
        Error => "error",
        Warning => "warning",
        Informational => "informational",
    }
    .to_string()
}

// Silence "unused" — kept for re-export consistency with other types
// in the crate.
#[allow(dead_code)]
fn _silence_unused(_: &CascadeRule) {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
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

    /// Test resolver returning a fixed list of child entity_ids.
    struct StaticChildResolver {
        children: Mutex<Vec<Uuid>>,
    }

    impl StaticChildResolver {
        fn new(children: Vec<Uuid>) -> Self {
            Self {
                children: Mutex::new(children),
            }
        }
    }

    #[async_trait]
    impl ChildEntityResolver for StaticChildResolver {
        async fn list_children(
            &self,
            _: &str,
            _: &str,
            _: Uuid,
            _: &str,
            _: &str,
            _: &PgPool,
        ) -> Result<Vec<Uuid>> {
            Ok(self.children.lock().unwrap().clone())
        }
    }

    #[test]
    fn registry_indexes_parent_and_children() {
        let r = registry_from(&[(
            "cbu",
            r#"
workspace: cbu
dag_id: cbu_dag
slots:
  - id: cbu
    stateless: false
    parent_slot:
      workspace: cbu
      slot: cbu
      join:
        via: cbu_entity_relationships
        parent_fk: parent_cbu_id
        child_fk: child_cbu_id
    state_dependency:
      cascade_rules:
        - parent_state: suspended
          child_allowed_states: [suspended]
          cascade_on_parent_transition: true
          default_child_state_on_cascade: suspended
        - parent_state: offboarded
          child_allowed_states: [offboarded, archived]
          cascade_on_parent_transition: true
          default_child_state_on_cascade: offboarded
      severity: error
    state_machine:
      id: cl
      states: [{ id: VALIDATED }]
"#,
        )]);

        // self-referencing parent → child appears as a child of itself.
        let kids = r.children_of("cbu", "cbu");
        assert_eq!(kids.len(), 1);
        assert_eq!(kids[0].workspace, "cbu");
        assert_eq!(kids[0].slot, "cbu");

        // state_dependency lookup
        let dep = r.state_dependency_for("cbu", "cbu").unwrap();
        assert_eq!(dep.cascade_rules.len(), 2);
    }

    #[test]
    fn plan_cascade_construction_only() {
        // Construction without a real PgPool. Full async planning is
        // exercised in integration tests with DB fixtures.
        let r = registry_from(&[(
            "cbu",
            r#"
workspace: cbu
dag_id: cbu_dag
slots:
  - id: cbu
    stateless: false
    parent_slot:
      workspace: cbu
      slot: cbu
    state_dependency:
      cascade_rules:
        - parent_state: suspended
          child_allowed_states: [suspended]
          cascade_on_parent_transition: true
          default_child_state_on_cascade: suspended
      severity: error
    state_machine: { id: cl, states: [{ id: VALIDATED }] }
"#,
        )]);
        let resolver = Arc::new(StaticChildResolver::new(vec![Uuid::new_v4()]));
        let _planner = CascadePlanner::new(r, resolver);
    }
}
