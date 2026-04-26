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
    constraints_by_target: HashMap<TransitionKey, Vec<ConstraintLocator>>,

    /// derived_states_by_host[SlotKey] → list of (workspace, index)
    /// into the owning Dag's derived_cross_workspace_state.
    derived_states_by_host: HashMap<SlotKey, Vec<DerivedStateLocator>>,

    /// parent_slot_by_child[SlotKey of child] → (parent SlotKey,
    /// the workspace+slot index where the child slot is declared).
    parent_slot_by_child: HashMap<SlotKey, ParentSlotLocator>,

    /// transitions_by_verb_fqn[verb_fqn] → list of transitions that
    /// declare this verb in their `via:` field. Used by runtime to
    /// answer "what transitions could this verb cause?" — the
    /// foundation for hooking GateChecker into verb dispatch.
    transitions_by_verb_fqn: HashMap<String, Vec<TransitionRef>>,

    /// children_by_parent[parent SlotKey] → list of child SlotKeys
    /// (slots whose parent_slot points back to the parent). Reverse
    /// of parent_slot_by_child; used by V1.3-3 cascade planning to
    /// answer "given a parent transition, which child slots need to
    /// react?".
    children_by_parent: HashMap<SlotKey, Vec<SlotKey>>,
}

/// A reference to a single declared transition, materialised for
/// verb-fqn lookup. `from` may be a comma-list / parenthesised group
/// in the source YAML (e.g. `(PROSPECT, QUALIFYING) -> CANCELLED`);
/// the parser flattens this into one TransitionRef per source state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionRef {
    pub workspace: String,
    pub slot: String,
    pub from_state: String,
    pub to_state: String,
    /// Whether this transition lives in a `dual_lifecycle:` chain
    /// (rather than the slot's primary `state_machine`).
    pub from_dual_lifecycle: bool,
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

    fn lookup_constraint(&self, loc: &ConstraintLocator) -> Option<&CrossWorkspaceConstraint> {
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
    pub fn parent_slot_for(&self, workspace: &str, slot: &str) -> Option<&ParentSlot> {
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
    pub fn parent_slot_key(&self, workspace: &str, slot: &str) -> Option<SlotKey> {
        self.parent_slot_by_child
            .get(&SlotKey::new(workspace, slot))
            .map(|loc| loc.parent.clone())
    }

    // -----------------------------------------------------------------------
    // Verb → transitions lookup (foundation for verb-dispatch gate hook)
    // -----------------------------------------------------------------------

    /// Find all transitions a verb participates in (across all DAGs,
    /// all slots, both primary state machines and dual_lifecycle chains).
    ///
    /// Returns a slice of `TransitionRef` — borrowed from the registry's
    /// internal index, valid for the lifetime of the registry.
    ///
    /// Used by the runtime to answer "what transitions could this verb
    /// cause?" — the input to deciding which gate checks apply.
    pub fn transitions_for_verb(&self, verb_fqn: &str) -> &[TransitionRef] {
        self.transitions_by_verb_fqn
            .get(verb_fqn)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Find all child slots whose parent_slot points to (workspace,
    /// slot). Used by V1.3-3 cascade planning to answer "given a
    /// parent transitioned to state X, which children need to react?"
    pub fn children_of(&self, parent_workspace: &str, parent_slot: &str) -> &[SlotKey] {
        self.children_by_parent
            .get(&SlotKey::new(parent_workspace, parent_slot))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Look up a child slot's `state_dependency` block, if declared.
    /// Returns the cascade rules that govern how the child reacts to
    /// parent state changes.
    pub fn state_dependency_for(&self, workspace: &str, slot: &str) -> Option<&StateDependency> {
        let dag = self.dags.get(workspace)?;
        dag.slots
            .iter()
            .find(|s| s.id == slot)
            .and_then(|s| s.state_dependency.as_ref())
    }

    // -----------------------------------------------------------------------
    // Index construction
    // -----------------------------------------------------------------------

    fn rebuild_indices(&mut self) {
        self.constraints_by_target.clear();
        self.derived_states_by_host.clear();
        self.parent_slot_by_child.clear();
        self.transitions_by_verb_fqn.clear();
        self.children_by_parent.clear();

        for (ws, dag) in &self.dags {
            // Verb → transition index: walk every slot's primary +
            // dual lifecycle transitions, extract verb FQNs from `via:`,
            // and record one TransitionRef per (from_state, to_state)
            // pair the verb is declared on.
            for slot in &dag.slots {
                if let Some(SlotStateMachine::Structured(sm)) = &slot.state_machine {
                    for t in &sm.transitions {
                        index_transition(ws, &slot.id, t, false, &mut self.transitions_by_verb_fqn);
                    }
                }
                for dl in &slot.dual_lifecycle {
                    for t in &dl.transitions {
                        index_transition(ws, &slot.id, t, true, &mut self.transitions_by_verb_fqn);
                    }
                }
            }

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

            // V1.3-3 parent_slot — index by child + reverse index by parent.
            for (slot_idx, slot) in dag.slots.iter().enumerate() {
                if let Some(parent) = &slot.parent_slot {
                    let parent_ws = parent.workspace.clone().unwrap_or_else(|| ws.clone());
                    let child_key = SlotKey::new(ws, &slot.id);
                    let parent_key = SlotKey::new(&parent_ws, &parent.slot);
                    self.parent_slot_by_child.insert(
                        child_key.clone(),
                        ParentSlotLocator {
                            parent: parent_key.clone(),
                            declaring_workspace: ws.clone(),
                            declaring_slot_index: slot_idx,
                        },
                    );
                    self.children_by_parent
                        .entry(parent_key)
                        .or_default()
                        .push(child_key);
                }
            }
        }
    }
}

/// Index a single TransitionDef into the verb→transition map.
fn index_transition(
    workspace: &str,
    slot: &str,
    t: &TransitionDef,
    from_dual: bool,
    out: &mut HashMap<String, Vec<TransitionRef>>,
) {
    let verbs = extract_verbs_from_via(&t.via);
    if verbs.is_empty() {
        return;
    }
    let froms = extract_from_states(&t.from);
    for verb_fqn in verbs {
        for from_state in &froms {
            // Skip transitions whose `from` we couldn't parse as a
            // state id (e.g. `"(any non-terminal)"` free-text escape) —
            // those are documentation, not enforceable transitions.
            if !is_valid_state_id(from_state) {
                continue;
            }
            out.entry(verb_fqn.clone())
                .or_default()
                .push(TransitionRef {
                    workspace: workspace.to_string(),
                    slot: slot.to_string(),
                    from_state: from_state.clone(),
                    to_state: t.to.clone(),
                    from_dual_lifecycle: from_dual,
                });
        }
    }
}

/// Pull verb FQNs from a `via:` field that may be a string, a list,
/// or a backend-marker string like `"(backend: ...)"` (in which case
/// no verbs are returned — backend transitions aren't verb-driven).
fn extract_verbs_from_via(via: &Option<serde_yaml::Value>) -> Vec<String> {
    let Some(v) = via else { return Vec::new() };
    match v {
        serde_yaml::Value::String(s) => {
            // "(backend: ...)" / "(time-decay)" / "(implicit: ...)" etc
            // are documentation strings, not verb FQNs.
            if s.trim().starts_with('(') {
                return Vec::new();
            }
            vec![s.clone()]
        }
        serde_yaml::Value::Sequence(seq) => seq
            .iter()
            .filter_map(|item| match item {
                serde_yaml::Value::String(s) if !s.trim().starts_with('(') => Some(s.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Pull from-state ids from a `from:` field.
///
/// The `from:` field can be:
///   - A bare state id: `"PROSPECT"` or `PROSPECT` (string)
///   - A list (YAML sequence): `[PROSPECT, QUALIFYING]`
///   - A quoted parenthesised group: `"(PROSPECT, QUALIFYING)"` —
///     parsed by splitting on commas inside the parens.
///   - A free-text descriptor: `"(any non-terminal)"` — unparseable;
///     returned as-is for caller filtering.
fn extract_from_states(from: &serde_yaml::Value) -> Vec<String> {
    match from {
        serde_yaml::Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.starts_with('(') && trimmed.ends_with(')') {
                let inner = &trimmed[1..trimmed.len() - 1];
                inner
                    .split(',')
                    .map(|p| p.trim().to_string())
                    .filter(|p| !p.is_empty())
                    .collect()
            } else {
                vec![trimmed.to_string()]
            }
        }
        serde_yaml::Value::Sequence(seq) => seq
            .iter()
            .filter_map(|item| match item {
                serde_yaml::Value::String(s) => Some(s.trim().to_string()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Heuristic: a state id is a single token of letters / digits /
/// underscores. Excludes free-text escapes like "any non-terminal".
fn is_valid_state_id(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
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

        let hits = r.constraints_for_transition("deal", "deal", "KYC_CLEARANCE", "CONTRACTED");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].source_workspace, "kyc");
        assert_eq!(
            hits[0].source_state.as_ref(),
            Some(&StateSelector::Single("APPROVED".to_string())),
        );

        // No match for unrelated transition.
        let no_hits = r.constraints_for_transition("deal", "deal", "PROSPECT", "QUALIFYING");
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
        assert_eq!(parse_transition("malformed"), (None, String::new()),);
    }

    #[test]
    fn verb_to_transition_index_single_via() {
        let r = registry_from(&[(
            "deal",
            r#"
workspace: deal
dag_id: deal_dag
slots:
  - id: deal
    stateless: false
    state_machine:
      id: dl
      states: [{ id: PROSPECT, entry: true }, { id: QUALIFYING }]
      transitions:
        - from: PROSPECT
          to: QUALIFYING
          via: deal.update-status
"#,
        )]);
        let hits = r.transitions_for_verb("deal.update-status");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].workspace, "deal");
        assert_eq!(hits[0].slot, "deal");
        assert_eq!(hits[0].from_state, "PROSPECT");
        assert_eq!(hits[0].to_state, "QUALIFYING");
        assert!(!hits[0].from_dual_lifecycle);
    }

    #[test]
    fn verb_to_transition_index_via_list() {
        let r = registry_from(&[(
            "deal",
            r#"
workspace: deal
dag_id: deal_dag
slots:
  - id: deal
    stateless: false
    state_machine:
      id: dl
      states: [{ id: PROSPECT, entry: true }, { id: QUALIFYING }]
      transitions:
        - from: PROSPECT
          to: QUALIFYING
          via: [deal.create, deal.update-status]
"#,
        )]);
        // Both verbs should index the same transition.
        let create_hits = r.transitions_for_verb("deal.create");
        let update_hits = r.transitions_for_verb("deal.update-status");
        assert_eq!(create_hits.len(), 1);
        assert_eq!(update_hits.len(), 1);
        assert_eq!(create_hits[0].to_state, "QUALIFYING");
        assert_eq!(update_hits[0].to_state, "QUALIFYING");
    }

    #[test]
    fn verb_to_transition_index_parenthesised_from() {
        let r = registry_from(&[(
            "deal",
            r#"
workspace: deal
dag_id: deal_dag
slots:
  - id: deal
    stateless: false
    state_machine:
      id: dl
      states:
        - { id: PROSPECT, entry: true }
        - { id: QUALIFYING }
        - { id: NEGOTIATING }
        - { id: CANCELLED }
      transitions:
        - from: "(PROSPECT, QUALIFYING, NEGOTIATING)"
          to: CANCELLED
          via: deal.cancel
"#,
        )]);
        let hits = r.transitions_for_verb("deal.cancel");
        assert_eq!(hits.len(), 3);
        let froms: Vec<&str> = hits.iter().map(|h| h.from_state.as_str()).collect();
        assert!(froms.contains(&"PROSPECT"));
        assert!(froms.contains(&"QUALIFYING"));
        assert!(froms.contains(&"NEGOTIATING"));
        assert!(hits.iter().all(|h| h.to_state == "CANCELLED"));
    }

    #[test]
    fn verb_to_transition_index_dual_lifecycle_marked() {
        let r = registry_from(&[(
            "deal",
            r#"
workspace: deal
dag_id: deal_dag
slots:
  - id: deal
    stateless: false
    state_machine:
      id: primary
      states: [{ id: CONTRACTED, entry: true }]
      transitions: []
    dual_lifecycle:
      - id: ops
        junction_state_from_primary: CONTRACTED
        states: [{ id: ONBOARDING }, { id: ACTIVE }]
        transitions:
          - from: ONBOARDING
            to: ACTIVE
            via: deal.update-status
"#,
        )]);
        let hits = r.transitions_for_verb("deal.update-status");
        assert_eq!(hits.len(), 1);
        assert!(hits[0].from_dual_lifecycle);
        assert_eq!(hits[0].from_state, "ONBOARDING");
        assert_eq!(hits[0].to_state, "ACTIVE");
    }

    #[test]
    fn verb_to_transition_index_skips_backend_via() {
        let r = registry_from(&[(
            "im",
            r#"
workspace: im
dag_id: im_dag
slots:
  - id: trading_activity
    stateless: false
    state_machine:
      id: ta
      states: [{ id: never_traded, entry: true }, { id: trading }]
      transitions:
        - from: never_traded
          to: trading
          via: "(backend: first trade posted)"
"#,
        )]);
        // No verbs declared; backend-marker via doesn't get indexed.
        // Only check that no real verb FQN got accidentally indexed.
        assert!(r.transitions_for_verb("backend").is_empty());
        assert!(r
            .transitions_for_verb("(backend: first trade posted)")
            .is_empty());
    }

    #[test]
    fn verb_to_transition_index_skips_unparseable_from() {
        let r = registry_from(&[(
            "kyc",
            r#"
workspace: kyc
dag_id: kyc_dag
slots:
  - id: kyc_case
    stateless: false
    state_machine:
      id: kc
      states: [{ id: BLOCKED }]
      transitions:
        - from: "(any non-terminal)"
          to: BLOCKED
          via: kyc-case.escalate
"#,
        )]);
        // The free-text from is not a state id; index drops it.
        let hits = r.transitions_for_verb("kyc-case.escalate");
        assert!(hits.is_empty());
    }

    #[test]
    fn loads_real_dags_from_disk() {
        // Live integration: registry should pick up all 9 DAGs cleanly
        // from the repo's actual dag_taxonomies/ directory.
        let path = std::path::Path::new("../../config/sem_os_seeds/dag_taxonomies");
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
        let deal_constraints =
            r.constraints_for_transition("deal", "deal", "KYC_CLEARANCE", "CONTRACTED");
        assert!(
            deal_constraints
                .iter()
                .any(|c| c.id == "deal_contracted_requires_kyc_approved"),
            "expected deal contract gate to be indexed"
        );

        // deal.cancel should be indexed across many from-states.
        let cancel_hits = r.transitions_for_verb("deal.cancel");
        assert!(
            cancel_hits.len() >= 4,
            "expected deal.cancel to participate in multiple transitions; got {}",
            cancel_hits.len()
        );
        assert!(
            cancel_hits
                .iter()
                .all(|h| h.workspace == "deal" && h.slot == "deal"),
            "all deal.cancel transitions should target the deal slot"
        );
        assert!(
            cancel_hits.iter().any(|h| h.to_state == "CANCELLED"),
            "deal.cancel should have a transition into CANCELLED"
        );

        // deal.bac-approve (added in R-5) should index a single
        // BAC_APPROVAL → KYC_CLEARANCE transition.
        let bac_hits = r.transitions_for_verb("deal.bac-approve");
        assert_eq!(
            bac_hits.len(),
            1,
            "expected deal.bac-approve once: {bac_hits:?}"
        );
        assert_eq!(bac_hits[0].from_state, "BAC_APPROVAL");
        assert_eq!(bac_hits[0].to_state, "KYC_CLEARANCE");
    }
}
