//! Runtime-side DAG registry — pre-indexed lookups for v1.3 enforcement.
//!
//! [`load_dags_from_dir`] / [`Dag`] (in `dag.rs`) give us the typed,
//! parsed shape. [`validate_dags`] (in `dag_validator.rs`) is the
//! build-time check.
//!
//! This module sits between the two: a runtime-loaded snapshot of all
//! DAGs that pre-computes the indices needed for hot-path checks:
//!
//!   * V1.3-1 cross_workspace_constraints — given (workspace, slot, from,
//!     to), find any constraints to enforce as a blocking gate.
//!   * V1.3-2 derived_cross_workspace_state — given (workspace, slot),
//!     find any derived states hosted on it (for evaluation at hydration).
//!   * V1.3-3 parent_slot — given (workspace, slot), find the parent slot
//!     reference (for cascade propagation).
//!
//! The registry itself is read-only after construction and meant to be
//! held in an `Arc<DagRegistry>` shared across the runtime. Reload is a
//! full re-construction (cheap; 9 DAGs at present).
//!
//! Pure data structure — no I/O at lookup time, no DB, no async. The
//! heavy lifting (running the source-state SQL queries that constraints
//! gate on, evaluating predicates) is done by callers using the registry
//! plus a SlotStateProvider (separate crate).

use crate::config::dag::*;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

// ---------------------------------------------------------------------------
// Index keys
// ---------------------------------------------------------------------------

/// (workspace, slot) — addresses a single slot within a single workspace.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SlotKey {
    pub workspace: String,
    pub slot: String,
}

impl SlotKey {
    pub fn new(workspace: impl Into<String>, slot: impl Into<String>) -> Self {
        Self {
            workspace: workspace.into(),
            slot: slot.into(),
        }
    }
}

/// Identifies a transition for constraint matching: target_workspace,
/// target_slot, and the (from, to) pair. `from` is `None` when the
/// constraint matches `* -> to` (any source state).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TransitionKey {
    pub workspace: String,
    pub slot: String,
    pub from_state: Option<String>,
    pub to_state: String,
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// Pre-indexed snapshot of all loaded DAGs.
///
/// Construction parses each DAG's `cross_workspace_constraints` /
/// `derived_cross_workspace_state` / per-slot `parent_slot` into hashmaps
/// keyed for the runtime's hot lookup paths.
#[derive(Debug, Clone, Default)]
pub struct DagRegistry {
    /// All DAGs by workspace name. Ownership lives here.
    dags: BTreeMap<String, Dag>,

    /// constraints_by_target[TransitionKey] → list of indices into
    /// the owning Dag's cross_workspace_constraints. Stores
    /// (workspace, index) for re-lookup against `dags`.
    constraints_by_target:
        HashMap<TransitionKey, Vec<ConstraintLocator>>,

    /// derived_states_by_host[SlotKey] → list of (workspace, index)
    /// into the owning Dag's derived_cross_workspace_state.
    derived_states_by_host: HashMap<SlotKey, Vec<DerivedStateLocator>>,

    /// parent_slot_by_child[SlotKey of child] → (parent SlotKey,
    /// the workspace+slot index where the child slot is declared).
    parent_slot_by_child: HashMap<SlotKey, ParentSlotLocator>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConstraintLocator {
    workspace: String,
    index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DerivedStateLocator {
    workspace: String,
    index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParentSlotLocator {
    parent: SlotKey,
    declaring_workspace: String,
    declaring_slot_index: usize,
}

impl DagRegistry {
    /// Build a registry from an already-loaded map (e.g. from
    /// [`load_dags_from_dir`]).
    pub fn from_loaded(loaded: BTreeMap<String, LoadedDag>) -> Self {
        let mut registry = DagRegistry::default();
        for (ws, ld) in loaded {
            registry.dags.insert(ws, ld.dag);
        }
        registry.rebuild_indices();
        registry
    }

    /// Convenience: load + index from disk in one call.
    pub fn from_dir(dir: &Path) -> anyhow::Result<Self> {
        let loaded = load_dags_from_dir(dir)?;
        Ok(DagRegistry::from_loaded(loaded))
    }

    /// Number of workspaces / DAGs in the registry.
    pub fn len(&self) -> usize {
        self.dags.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.dags.is_empty()
    }

    /// Borrow the DAG for a given workspace, if loaded.
    pub fn dag(&self, workspace: &str) -> Option<&Dag> {
        self.dags.get(workspace)
    }

    /// Iterate over all loaded DAGs.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Dag)> {
        self.dags.iter()
    }

    // -----------------------------------------------------------------------
    // V1.3-1 lookups
    // -----------------------------------------------------------------------

    /// Find all cross_workspace_constraints whose target_transition matches
    /// the given (workspace, slot, from_state → to_state). Both
    /// from-specific (`A -> B`) and wildcard (`* -> B`) constraints are
    /// returned.
    ///
    /// Result is borrow-anchored on the registry; callers can pull the
    /// constraint's source_workspace / source_slot / source_state /
    /// source_predicate / severity to drive enforcement.
    pub fn constraints_for_transition(
        &self,
        workspace: &str,
        slot: &str,
        from_state: &str,
        to_state: &str,
    ) -> Vec<&CrossWorkspaceConstraint> {
        let mut out = Vec::new();
        // Specific from-state lookup.
        let key = TransitionKey {
            workspace: workspace.to_string(),
            slot: slot.to_string(),
            from_state: Some(from_state.to_string()),
            to_state: to_state.to_string(),
        };
        if let Some(locators) = self.constraints_by_target.get(&key) {
            for loc in locators {
                if let Some(c) = self.lookup_constraint(loc) {
                    out.push(c);
                }
            }
        }
        // Wildcard `* -> to_state` lookup.
        let wildcard_key = TransitionKey {
            workspace: workspace.to_string(),
            slot: slot.to_string(),
            from_state: None,
            to_state: to_state.to_string(),
        };
        if let Some(locators) = self.constraints_by_target.get(&wildcard_key) {
            for loc in locators {
                if let Some(c) = self.lookup_constraint(loc) {
                    out.push(c);
                }
            }
        }
        out
    }

    fn lookup_constraint(
        &self,
        loc: &ConstraintLocator,
    ) -> Option<&CrossWorkspaceConstraint> {
        self.dags
            .get(&loc.workspace)
            .and_then(|dag| dag.cross_workspace_constraints.get(loc.index))
    }

    // -----------------------------------------------------------------------
    // V1.3-2 lookups
    // -----------------------------------------------------------------------

    /// Find all derived_cross_workspace_state entries hosted on (workspace,
    /// slot). Used at hydration / aggregate-evaluation time.
    pub fn derived_states_for_slot(
        &self,
        workspace: &str,
        slot: &str,
    ) -> Vec<&DerivedCrossWorkspaceState> {
        let key = SlotKey::new(workspace, slot);
        let mut out = Vec::new();
        if let Some(locators) = self.derived_states_by_host.get(&key) {
            for loc in locators {
                if let Some(ds) = self.lookup_derived_state(loc) {
                    out.push(ds);
                }
            }
        }
        out
    }

    fn lookup_derived_state(
        &self,
        loc: &DerivedStateLocator,
    ) -> Option<&DerivedCrossWorkspaceState> {
        self.dags
            .get(&loc.workspace)
            .and_then(|dag| dag.derived_cross_workspace_state.get(loc.index))
    }

    // -----------------------------------------------------------------------
    // V1.3-3 lookups
    // -----------------------------------------------------------------------

    /// Find the parent slot reference for a given (workspace, slot), if
    /// declared. Returns `(parent_workspace, parent_slot)`.
    pub fn parent_slot_for(
        &self,
        workspace: &str,
        slot: &str,
    ) -> Option<&ParentSlot> {
        let key = SlotKey::new(workspace, slot);
        let loc = self.parent_slot_by_child.get(&key)?;
        let dag = self.dags.get(&loc.declaring_workspace)?;
        dag.slots
            .get(loc.declaring_slot_index)
            .and_then(|s| s.parent_slot.as_ref())
    }

    /// Get the resolved parent SlotKey for a child slot, with the
    /// child's declaring workspace as the default if `parent_slot.workspace`
    /// is unset.
    pub fn parent_slot_key(
        &self,
        workspace: &str,
        slot: &str,
    ) -> Option<SlotKey> {
        self.parent_slot_by_child
            .get(&SlotKey::new(workspace, slot))
            .map(|loc| loc.parent.clone())
    }

    // -----------------------------------------------------------------------
    // Index construction
    // -----------------------------------------------------------------------

    fn rebuild_indices(&mut self) {
        self.constraints_by_target.clear();
        self.derived_states_by_host.clear();
        self.parent_slot_by_child.clear();

        for (ws, dag) in &self.dags {
            // V1.3-1 cross_workspace_constraints — index by target
            // transition.
            for (idx, c) in dag.cross_workspace_constraints.iter().enumerate() {
                let (from, to) = parse_transition(&c.target_transition);
                let key = TransitionKey {
                    workspace: c.target_workspace.clone(),
                    slot: c.target_slot.clone(),
                    from_state: from,
                    to_state: to,
                };
                self.constraints_by_target
                    .entry(key)
                    .or_default()
                    .push(ConstraintLocator {
                        workspace: ws.clone(),
                        index: idx,
                    });
            }

            // V1.3-2 derived_cross_workspace_state — index by host.
            for (idx, d) in dag.derived_cross_workspace_state.iter().enumerate() {
                let key = SlotKey::new(&d.host_workspace, &d.host_slot);
                self.derived_states_by_host
                    .entry(key)
                    .or_default()
                    .push(DerivedStateLocator {
                        workspace: ws.clone(),
                        index: idx,
                    });
            }

            // V1.3-3 parent_slot — index by child.
            for (slot_idx, slot) in dag.slots.iter().enumerate() {
                if let Some(parent) = &slot.parent_slot {
                    let parent_ws = parent
                        .workspace
                        .clone()
                        .unwrap_or_else(|| ws.clone());
                    let child_key = SlotKey::new(ws, &slot.id);
                    let parent_key = SlotKey::new(&parent_ws, &parent.slot);
                    self.parent_slot_by_child.insert(
                        child_key,
                        ParentSlotLocator {
                            parent: parent_key,
                            declaring_workspace: ws.clone(),
                            declaring_slot_index: slot_idx,
                        },
                    );
                }
            }
        }
    }
}

/// Parse a `"FROM -> TO"` or `"* -> TO"` string into (Option<from>, to).
/// Whitespace tolerant. Returns ("", "") for malformed input — validator
/// catches structural errors at build time, so runtime tolerates oddities
/// silently.
fn parse_transition(s: &str) -> (Option<String>, String) {
    let parts: Vec<&str> = s.split("->").map(|p| p.trim()).collect();
    if parts.len() != 2 {
        return (None, String::new());
    }
    let from = if parts[0] == "*" {
        None
    } else {
        Some(parts[0].to_string())
    };
    (from, parts[1].to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::dag::Dag;
    use std::path::PathBuf;

    fn ws_dag(yaml: &str) -> LoadedDag {
        let dag: Dag = serde_yaml::from_str(yaml).unwrap();
        LoadedDag {
            source_path: PathBuf::new(),
            dag,
        }
    }

    fn registry_from(workspaces: &[(&str, &str)]) -> DagRegistry {
        let mut map = BTreeMap::new();
        for (name, yaml) in workspaces {
            map.insert(name.to_string(), ws_dag(yaml));
        }
        DagRegistry::from_loaded(map)
    }

    #[test]
    fn empty_registry() {
        let r = DagRegistry::default();
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
        assert!(r.dag("foo").is_none());
    }

    #[test]
    fn basic_dag_loads() {
        let r = registry_from(&[(
            "demo",
            r#"
workspace: demo
dag_id: demo_dag
slots:
  - id: thing
    stateless: true
"#,
        )]);
        assert_eq!(r.len(), 1);
        assert_eq!(r.dag("demo").unwrap().slots.len(), 1);
    }

    #[test]
    fn cross_workspace_constraint_indexed_and_looked_up() {
        let r = registry_from(&[
            (
                "kyc",
                r#"
workspace: kyc
dag_id: kyc_dag
slots:
  - id: kyc_case
    stateless: false
    state_machine:
      id: kyc_case_lifecycle
      states: [{ id: APPROVED }]
"#,
            ),
            (
                "deal",
                r#"
workspace: deal
dag_id: deal_dag
cross_workspace_constraints:
  - id: deal_contracted_requires_kyc_approved
    source_workspace: kyc
    source_slot: kyc_case
    source_state: APPROVED
    target_workspace: deal
    target_slot: deal
    target_transition: "KYC_CLEARANCE -> CONTRACTED"
    severity: error
slots:
  - id: deal
    stateless: false
    state_machine:
      id: deal_lifecycle
      states:
        - { id: KYC_CLEARANCE, entry: true }
        - { id: CONTRACTED }
"#,
            ),
        ]);

        let hits = r.constraints_for_transition(
            "deal",
            "deal",
            "KYC_CLEARANCE",
            "CONTRACTED",
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].source_workspace, "kyc");
        assert_eq!(
            hits[0].source_state.as_ref(),
            Some(&StateSelector::Single("APPROVED".to_string())),
        );

        // No match for unrelated transition.
        let no_hits = r.constraints_for_transition(
            "deal",
            "deal",
            "PROSPECT",
            "QUALIFYING",
        );
        assert!(no_hits.is_empty());
    }

    #[test]
    fn wildcard_target_transition_matches_any_from() {
        let r = registry_from(&[(
            "deal",
            r#"
workspace: deal
dag_id: deal_dag
cross_workspace_constraints:
  - id: any_to_active_requires_billing
    source_workspace: deal
    source_slot: billing_profile
    source_state: ACTIVE
    target_workspace: deal
    target_slot: deal
    target_transition: "* -> ACTIVE"
    severity: error
slots:
  - id: deal
    stateless: false
    state_machine: { id: dl, states: [{ id: ACTIVE }] }
  - id: billing_profile
    stateless: false
    state_machine: { id: bpl, states: [{ id: ACTIVE }] }
"#,
        )]);

        // Should match transitions from any state into ACTIVE.
        let hits1 = r.constraints_for_transition("deal", "deal", "ONBOARDING", "ACTIVE");
        let hits2 = r.constraints_for_transition("deal", "deal", "SUSPENDED", "ACTIVE");
        assert_eq!(hits1.len(), 1);
        assert_eq!(hits2.len(), 1);

        // But not for transitions OUT of ACTIVE.
        let no = r.constraints_for_transition("deal", "deal", "ACTIVE", "SUSPENDED");
        assert!(no.is_empty());
    }

    #[test]
    fn derived_state_indexed_by_host() {
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

        let hits = r.derived_states_for_slot("cbu", "cbu");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].host_state, "operationally_active");

        let none = r.derived_states_for_slot("cbu", "evidence");
        assert!(none.is_empty());
    }

    #[test]
    fn parent_slot_indexed_by_child() {
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
    state_machine: { id: cl, states: [{ id: VALIDATED }] }
"#,
        )]);

        let parent = r.parent_slot_for("cbu", "cbu").unwrap();
        assert_eq!(parent.slot, "cbu");
        assert_eq!(parent.workspace.as_deref(), Some("cbu"));

        let key = r.parent_slot_key("cbu", "cbu").unwrap();
        assert_eq!(key.workspace, "cbu");
        assert_eq!(key.slot, "cbu");

        // Slot without parent_slot returns None.
        let none = r.parent_slot_for("cbu", "nonexistent");
        assert!(none.is_none());
    }

    #[test]
    fn parent_slot_defaults_to_owning_workspace_when_omitted() {
        let r = registry_from(&[(
            "demo",
            r#"
workspace: demo
dag_id: demo_dag
slots:
  - id: child
    stateless: true
    parent_slot:
      slot: parent
"#,
        )]);

        let key = r.parent_slot_key("demo", "child").unwrap();
        assert_eq!(key.workspace, "demo"); // defaulted
        assert_eq!(key.slot, "parent");
    }

    #[test]
    fn parse_transition_helper() {
        assert_eq!(
            parse_transition("KYC_CLEARANCE -> CONTRACTED"),
            (Some("KYC_CLEARANCE".to_string()), "CONTRACTED".to_string()),
        );
        assert_eq!(
            parse_transition("* -> ACTIVE"),
            (None, "ACTIVE".to_string()),
        );
        assert_eq!(
            parse_transition("malformed"),
            (None, String::new()),
        );
    }

    #[test]
    fn loads_real_dags_from_disk() {
        // Live integration: registry should pick up all 9 DAGs cleanly
        // from the repo's actual dag_taxonomies/ directory.
        let path = std::path::Path::new(
            "../../config/sem_os_seeds/dag_taxonomies",
        );
        if !path.exists() {
            eprintln!("real DAG dir not present (test running outside repo) — skipping");
            return;
        }
        let r = DagRegistry::from_dir(path).expect("load real DAGs");
        assert!(r.len() >= 4, "expected at least the Tranche-2 DAGs to load");

        // CBU should have a derived_cross_workspace_state for the tollgate.
        let cbu_aggregates = r.derived_states_for_slot("cbu", "cbu");
        assert!(
            cbu_aggregates
                .iter()
                .any(|d| d.host_state == "operationally_active"),
            "expected cbu_operationally_active tollgate to be indexed"
        );

        // Deal should have its KYC-clearance constraint indexed.
        let deal_constraints = r.constraints_for_transition(
            "deal",
            "deal",
            "KYC_CLEARANCE",
            "CONTRACTED",
        );
        assert!(
            deal_constraints
                .iter()
                .any(|c| c.id == "deal_contracted_requires_kyc_approved"),
            "expected deal contract gate to be indexed"
        );
    }
}
