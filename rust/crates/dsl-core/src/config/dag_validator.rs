//! DAG-taxonomy validator (R-2b, v1.3 cross-DAG checks).
//!
//! Takes the set of loaded DAGs (`BTreeMap<workspace_name, LoadedDag>`)
//! and applies the v1.3 architectural checks from
//! `catalogue-platform-refinement-v1_3.md` §3.1:
//!
//! Errors:
//! - CrossWorkspaceConstraintUnresolved
//! - CrossWorkspaceConstraintSelfReference
//! - DerivedCrossWorkspaceStateUnresolved
//! - DerivedCrossWorkspaceStateCycle
//! - ParentSlotUnresolved
//! - StateDependencyInconsistent
//! - DualLifecycleJunctionMissing
//! - CategoryGatedUnresolvedColumn (structural — category_column must be present)
//! - CategoryGatedMutuallyExclusiveGates
//!
//! Warnings:
//! - LongLivedSlotMissingSuspended (V1.3-4 lint)
//! - PeriodicReviewCadenceInconsistent
//! - ValidityWindowWithoutExpiredState
//!
//! Pure function library — no DB, no IO. Takes in-memory DAG structs.

use crate::config::dag::*;
use std::collections::{BTreeMap, HashMap, HashSet};

// ---------------------------------------------------------------------------
// Error + warning taxonomy
// ---------------------------------------------------------------------------

/// Location of a DAG-level finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DagLocation {
    pub workspace: String,
    pub path: String,
}

impl std::fmt::Display for DagLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}::{}", self.workspace, self.path)
    }
}

/// Structural + well-formedness errors in DAG cross-references.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DagError {
    // V1.3-1 cross_workspace_constraints
    CrossWorkspaceConstraintUnresolved {
        location: DagLocation,
        constraint_id: String,
        unresolved_ref: String,
        reason: String,
    },
    CrossWorkspaceConstraintSelfReference {
        location: DagLocation,
        constraint_id: String,
    },
    // V1.3-2 derived_cross_workspace_state
    DerivedCrossWorkspaceStateUnresolved {
        location: DagLocation,
        derived_id: String,
        unresolved_ref: String,
    },
    DerivedCrossWorkspaceStateCycle {
        cycle_path: Vec<String>, // list of "workspace.slot.state" nodes
    },

    // V1.3-3 parent_slot + state_dependency
    ParentSlotUnresolved {
        location: DagLocation,
        slot_id: String,
        parent_workspace: String,
        parent_slot: String,
    },
    StateDependencyInconsistent {
        location: DagLocation,
        slot_id: String,
        parent_state: String,
        reason: String,
    },

    // V1.3-5 dual_lifecycle
    DualLifecycleJunctionMissing {
        location: DagLocation,
        slot_id: String,
        dual_id: String,
        junction_state: String,
    },

    // V1.3-8 category_gated
    CategoryGatedMutuallyExclusiveGates {
        location: DagLocation,
        slot_id: String,
    },
}

impl std::fmt::Display for DagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CrossWorkspaceConstraintUnresolved {
                location,
                constraint_id,
                unresolved_ref,
                reason,
            } => write!(
                f,
                "{location}: cross_workspace_constraint '{constraint_id}' references \
                 '{unresolved_ref}' — {reason}"
            ),
            Self::CrossWorkspaceConstraintSelfReference {
                location,
                constraint_id,
            } => write!(
                f,
                "{location}: cross_workspace_constraint '{constraint_id}' has \
                 source_workspace == target_workspace — use intra-DAG \
                 cross_slot_constraints instead"
            ),
            Self::DerivedCrossWorkspaceStateUnresolved {
                location,
                derived_id,
                unresolved_ref,
            } => write!(
                f,
                "{location}: derived_cross_workspace_state '{derived_id}' has \
                 unresolved derivation ref '{unresolved_ref}'"
            ),
            Self::DerivedCrossWorkspaceStateCycle { cycle_path } => write!(
                f,
                "derived_cross_workspace_state cycle detected: {}",
                cycle_path.join(" -> ")
            ),
            Self::ParentSlotUnresolved {
                location,
                slot_id,
                parent_workspace,
                parent_slot,
            } => write!(
                f,
                "{location}: slot '{slot_id}' parent_slot \
                 '{parent_workspace}.{parent_slot}' is not declared"
            ),
            Self::StateDependencyInconsistent {
                location,
                slot_id,
                parent_state,
                reason,
            } => write!(
                f,
                "{location}: slot '{slot_id}' state_dependency references \
                 parent_state '{parent_state}' — {reason}"
            ),
            Self::DualLifecycleJunctionMissing {
                location,
                slot_id,
                dual_id,
                junction_state,
            } => write!(
                f,
                "{location}: slot '{slot_id}' dual_lifecycle '{dual_id}' \
                 junction_state_from_primary '{junction_state}' is not a state \
                 in the slot's primary state_machine"
            ),
            Self::CategoryGatedMutuallyExclusiveGates { location, slot_id } => write!(
                f,
                "{location}: slot '{slot_id}' category_gated has both activated_by \
                 and deactivated_by — mutually exclusive"
            ),
        }
    }
}

/// Policy / lint warnings — advisory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DagWarning {
    // V1.3-4
    LongLivedSlotMissingSuspended {
        location: DagLocation,
        slot_id: String,
    },

    // V1.3-6
    PeriodicReviewCadenceWithoutRereviewTransition {
        location: DagLocation,
        slot_id: String,
    },
    ValidityWindowWithoutExpiredState {
        location: DagLocation,
        evidence_type_id: String,
    },
}

impl std::fmt::Display for DagWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LongLivedSlotMissingSuspended { location, slot_id } => write!(
                f,
                "{location}: slot '{slot_id}' is expected_lifetime=long_lived but \
                 state_machine has no SUSPENDED state. Declare SUSPENDED (bidirectional \
                 from preceding operational state) or mark suspended_state_exempt: true \
                 with a rationale comment. (V1.3-4)"
            ),
            Self::PeriodicReviewCadenceWithoutRereviewTransition { location, slot_id } => {
                write!(
                    f,
                    "{location}: slot '{slot_id}' declares periodic_review_cadence but \
                     the state machine has no transition that re-enters an earlier \
                     review state. Cadence will have no runtime effect. (V1.3-6)"
                )
            }
            Self::ValidityWindowWithoutExpiredState { location, evidence_type_id } => write!(
                f,
                "{location}: evidence_type '{evidence_type_id}' has a validity_window \
                 but the evidence state machine has no EXPIRED state. \
                 Validity-window expiration has no state to transition into. (V1.3-6)"
            ),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DagValidationReport {
    pub errors: Vec<DagError>,
    pub warnings: Vec<DagWarning>,
}

impl DagValidationReport {
    pub fn is_clean(&self) -> bool {
        self.errors.is_empty()
    }
    pub fn error_count(&self) -> usize {
        self.errors.len()
    }
}

// ---------------------------------------------------------------------------
// Validator entry point
// ---------------------------------------------------------------------------

/// Validate all loaded DAGs. Takes the full map so cross-DAG references
/// (cross_workspace_constraints, derived_cross_workspace_state, parent_slot
/// into other workspaces) can be resolved.
pub fn validate_dags(loaded: &BTreeMap<String, LoadedDag>) -> DagValidationReport {
    let mut report = DagValidationReport::default();

    // Index: workspace -> (slot_id -> Slot ref) and state_machine state_ids.
    let index = SlotIndex::build(loaded);

    for (ws, ld) in loaded {
        validate_cross_workspace_constraints(ws, &ld.dag, &index, &mut report);
        validate_derived_cross_workspace_state_refs(ws, &ld.dag, &index, &mut report);
        validate_parent_slots(ws, &ld.dag, &index, &mut report);
        validate_dual_lifecycles(ws, &ld.dag, &mut report);
        validate_category_gated(ws, &ld.dag, &mut report);

        // Warnings
        validate_long_lived_suspended_convention(ws, &ld.dag, &mut report);
        validate_periodic_review_cadence(ws, &ld.dag, &mut report);
        validate_validity_window_expired_state(ws, &ld.dag, &mut report);
    }

    // Cycle detection across all derivations (requires full index).
    detect_derivation_cycles(loaded, &mut report);

    report
}

// ---------------------------------------------------------------------------
// Index — workspace -> slot -> state set
// ---------------------------------------------------------------------------

struct SlotIndex {
    /// workspace -> slot_id -> (has_state_machine, states, transitions_count)
    slots: HashMap<String, HashMap<String, SlotSummary>>,
}

#[derive(Default)]
struct SlotSummary {
    has_state_machine: bool,
    states: HashSet<String>,
    transitions_count: usize,
    expected_lifetime: Option<ExpectedLifetime>,
    suspended_state_exempt: bool,
}

impl SlotIndex {
    fn build(loaded: &BTreeMap<String, LoadedDag>) -> Self {
        let mut slots = HashMap::new();
        for (ws, ld) in loaded {
            let mut ws_slots = HashMap::new();
            for slot in &ld.dag.slots {
                let mut summary = SlotSummary::default();
                if let Some(sm) = &slot.state_machine {
                    match sm {
                        SlotStateMachine::Structured(sm) => {
                            summary.has_state_machine = true;
                            for st in &sm.states {
                                summary.states.insert(st.id.clone());
                            }
                            summary.transitions_count = sm.transitions.len();
                            summary.expected_lifetime = sm.expected_lifetime.clone();
                        }
                        SlotStateMachine::Reference(_) => {
                            summary.has_state_machine = true;
                        }
                    }
                }
                // V1.3-5: dual_lifecycle states belong to the same slot
                // conceptually — index them too.
                for dual in &slot.dual_lifecycle {
                    for st in &dual.states {
                        summary.states.insert(st.id.clone());
                    }
                }
                summary.suspended_state_exempt = slot.suspended_state_exempt;
                ws_slots.insert(slot.id.clone(), summary);
            }
            slots.insert(ws.clone(), ws_slots);
        }
        SlotIndex { slots }
    }

    fn workspace_exists(&self, ws: &str) -> bool {
        self.slots.contains_key(ws)
    }

    fn slot_exists(&self, ws: &str, slot: &str) -> bool {
        self.slots.get(ws).is_some_and(|m| m.contains_key(slot))
    }

    fn state_exists(&self, ws: &str, slot: &str, state: &str) -> bool {
        self.slots
            .get(ws)
            .and_then(|m| m.get(slot))
            .is_some_and(|s| s.states.is_empty() || s.states.contains(state))
        // Empty states means "reference-only state machine" — skip strict check.
    }
}

// ---------------------------------------------------------------------------
// Individual checks
// ---------------------------------------------------------------------------

fn validate_cross_workspace_constraints(
    workspace: &str,
    dag: &Dag,
    index: &SlotIndex,
    report: &mut DagValidationReport,
) {
    for c in &dag.cross_workspace_constraints {
        let loc = DagLocation {
            workspace: workspace.to_string(),
            path: format!("cross_workspace_constraints[{}]", c.id),
        };

        // Self-reference check
        if c.source_workspace == c.target_workspace {
            report
                .errors
                .push(DagError::CrossWorkspaceConstraintSelfReference {
                    location: loc.clone(),
                    constraint_id: c.id.clone(),
                });
        }

        // state + predicate may be declared together — they compose
        // with AND semantics (state narrows WHICH state, predicate
        // narrows WHICH row).

        // Source workspace exists?
        if !index.workspace_exists(&c.source_workspace) {
            report
                .errors
                .push(DagError::CrossWorkspaceConstraintUnresolved {
                    location: loc.clone(),
                    constraint_id: c.id.clone(),
                    unresolved_ref: format!("workspace:{}", c.source_workspace),
                    reason: "workspace not loaded".to_string(),
                });
            continue;
        }

        // Source slot exists?
        if !index.slot_exists(&c.source_workspace, &c.source_slot) {
            report
                .errors
                .push(DagError::CrossWorkspaceConstraintUnresolved {
                    location: loc.clone(),
                    constraint_id: c.id.clone(),
                    unresolved_ref: format!("{}.{}", c.source_workspace, c.source_slot),
                    reason: "slot not declared in source workspace".to_string(),
                });
            continue;
        }

        // Source state exists in source slot?
        if let Some(state_sel) = &c.source_state {
            let states: Vec<String> = match state_sel {
                StateSelector::Single(s) => vec![s.clone()],
                StateSelector::Set(v) => v.clone(),
            };
            for s in states {
                if !index.state_exists(&c.source_workspace, &c.source_slot, &s) {
                    report
                        .errors
                        .push(DagError::CrossWorkspaceConstraintUnresolved {
                            location: loc.clone(),
                            constraint_id: c.id.clone(),
                            unresolved_ref: format!(
                                "{}.{}::{}",
                                c.source_workspace, c.source_slot, s
                            ),
                            reason: "state not in source slot's state machine".to_string(),
                        });
                }
            }
        }

        // Target workspace/slot exist?
        if !index.workspace_exists(&c.target_workspace) {
            report
                .errors
                .push(DagError::CrossWorkspaceConstraintUnresolved {
                    location: loc.clone(),
                    constraint_id: c.id.clone(),
                    unresolved_ref: format!("target workspace:{}", c.target_workspace),
                    reason: "target workspace not loaded".to_string(),
                });
        } else if !index.slot_exists(&c.target_workspace, &c.target_slot) {
            report
                .errors
                .push(DagError::CrossWorkspaceConstraintUnresolved {
                    location: loc.clone(),
                    constraint_id: c.id.clone(),
                    unresolved_ref: format!("{}.{}", c.target_workspace, c.target_slot),
                    reason: "target slot not declared".to_string(),
                });
        }
    }
}

fn validate_derived_cross_workspace_state_refs(
    workspace: &str,
    dag: &Dag,
    index: &SlotIndex,
    report: &mut DagValidationReport,
) {
    for d in &dag.derived_cross_workspace_state {
        let loc = DagLocation {
            workspace: workspace.to_string(),
            path: format!("derived_cross_workspace_state[{}]", d.id),
        };

        // Host exists?
        if !index.slot_exists(&d.host_workspace, &d.host_slot) {
            report
                .errors
                .push(DagError::DerivedCrossWorkspaceStateUnresolved {
                    location: loc.clone(),
                    derived_id: d.id.clone(),
                    unresolved_ref: format!(
                        "host {}.{}",
                        d.host_workspace, d.host_slot
                    ),
                });
        }

        // Each derivation condition resolvable
        for cond in d
            .derivation
            .all_of
            .iter()
            .chain(d.derivation.any_of.iter())
        {
            if let DerivationCondition::Structured(s) = cond {
                if !index.slot_exists(&s.workspace, &s.slot) {
                    report
                        .errors
                        .push(DagError::DerivedCrossWorkspaceStateUnresolved {
                            location: loc.clone(),
                            derived_id: d.id.clone(),
                            unresolved_ref: format!("{}.{}", s.workspace, s.slot),
                        });
                    continue;
                }
                if let Some(state_sel) = &s.state {
                    let states: Vec<String> = match state_sel {
                        StateSelector::Single(s) => vec![s.clone()],
                        StateSelector::Set(v) => v.clone(),
                    };
                    for st in states {
                        if !index.state_exists(&s.workspace, &s.slot, &st) {
                            report.errors.push(
                                DagError::DerivedCrossWorkspaceStateUnresolved {
                                    location: loc.clone(),
                                    derived_id: d.id.clone(),
                                    unresolved_ref: format!(
                                        "{}.{}::{}",
                                        s.workspace, s.slot, st
                                    ),
                                },
                            );
                        }
                    }
                }
            }
        }
    }
}

fn validate_parent_slots(
    workspace: &str,
    dag: &Dag,
    index: &SlotIndex,
    report: &mut DagValidationReport,
) {
    for slot in &dag.slots {
        if let Some(parent) = &slot.parent_slot {
            let parent_ws = parent.workspace.as_deref().unwrap_or(workspace);
            let loc = DagLocation {
                workspace: workspace.to_string(),
                path: format!("slots.{}.parent_slot", slot.id),
            };
            if !index.slot_exists(parent_ws, &parent.slot) {
                report.errors.push(DagError::ParentSlotUnresolved {
                    location: loc.clone(),
                    slot_id: slot.id.clone(),
                    parent_workspace: parent_ws.to_string(),
                    parent_slot: parent.slot.clone(),
                });
                continue;
            }

            // state_dependency consistency
            if let Some(dep) = &slot.state_dependency {
                for rule in &dep.cascade_rules {
                    if !index.state_exists(parent_ws, &parent.slot, &rule.parent_state) {
                        report.errors.push(DagError::StateDependencyInconsistent {
                            location: loc.clone(),
                            slot_id: slot.id.clone(),
                            parent_state: rule.parent_state.clone(),
                            reason: "parent_state is not in parent slot's state machine"
                                .to_string(),
                        });
                    }
                }
            }
        }
    }
}

fn validate_dual_lifecycles(workspace: &str, dag: &Dag, report: &mut DagValidationReport) {
    for slot in &dag.slots {
        let Some(sm) = slot.state_machine.as_ref() else {
            continue;
        };
        let SlotStateMachine::Structured(sm) = sm else {
            continue; // can't validate reference form
        };

        let primary_states: HashSet<&str> =
            sm.states.iter().map(|s| s.id.as_str()).collect();

        for dual in &slot.dual_lifecycle {
            if !primary_states.contains(dual.junction_state_from_primary.as_str()) {
                report.errors.push(DagError::DualLifecycleJunctionMissing {
                    location: DagLocation {
                        workspace: workspace.to_string(),
                        path: format!("slots.{}.dual_lifecycle", slot.id),
                    },
                    slot_id: slot.id.clone(),
                    dual_id: dual.id.clone(),
                    junction_state: dual.junction_state_from_primary.clone(),
                });
            }
        }
    }
}

fn validate_category_gated(workspace: &str, dag: &Dag, report: &mut DagValidationReport) {
    for slot in &dag.slots {
        let Some(cg) = &slot.category_gated else {
            continue;
        };
        if !cg.activated_by.is_empty() && !cg.deactivated_by.is_empty() {
            report
                .errors
                .push(DagError::CategoryGatedMutuallyExclusiveGates {
                    location: DagLocation {
                        workspace: workspace.to_string(),
                        path: format!("slots.{}.category_gated", slot.id),
                    },
                    slot_id: slot.id.clone(),
                });
        }
    }
}

fn validate_long_lived_suspended_convention(
    workspace: &str,
    dag: &Dag,
    report: &mut DagValidationReport,
) {
    for slot in &dag.slots {
        if slot.suspended_state_exempt {
            continue;
        }
        let Some(sm) = slot.state_machine.as_ref() else {
            continue;
        };
        let SlotStateMachine::Structured(sm) = sm else {
            continue; // reference-form SMs exempt
        };
        if sm.expected_lifetime != Some(ExpectedLifetime::LongLived) {
            continue;
        }
        // V1.3-4: SUSPENDED may live in the primary SM OR in any
        // dual_lifecycle chain — dual-lifecycle is the canonical home
        // for operational states like suspended.
        let has_suspended_primary = sm
            .states
            .iter()
            .any(|s| s.id.eq_ignore_ascii_case("SUSPENDED"));
        let has_suspended_dual = slot.dual_lifecycle.iter().any(|dl| {
            dl.states
                .iter()
                .any(|s| s.id.eq_ignore_ascii_case("SUSPENDED"))
        });
        if !has_suspended_primary && !has_suspended_dual {
            report.warnings.push(DagWarning::LongLivedSlotMissingSuspended {
                location: DagLocation {
                    workspace: workspace.to_string(),
                    path: format!("slots.{}.state_machine", slot.id),
                },
                slot_id: slot.id.clone(),
            });
        }
    }
}

fn validate_periodic_review_cadence(
    workspace: &str,
    dag: &Dag,
    report: &mut DagValidationReport,
) {
    for slot in &dag.slots {
        let Some(_cadence) = &slot.periodic_review_cadence else {
            continue;
        };
        let Some(SlotStateMachine::Structured(sm)) = &slot.state_machine else {
            continue;
        };

        // Build: set of states that are reached (appear as `to` in some
        // transition). A re-review transition is any transition whose
        // `to` state ALSO appears as an earlier `to` OR as an entry
        // state — i.e. we can revisit a "review phase" state.
        //
        // Equivalently: the state graph has at least one back-edge /
        // cycle. Simpler heuristic that correctly handles:
        //   APPROVED → EXPIRED → IN_PROGRESS → APPROVED (cycle through
        //   IN_PROGRESS which is re-reviewable).
        //
        // Detect: any state appears as `to` in 2+ transitions (reached
        // from multiple sources — implies revisit) OR the entry state
        // appears as a `to` (direct back-edge).
        let mut to_counts: std::collections::HashMap<&str, usize> =
            std::collections::HashMap::new();
        for t in &sm.transitions {
            to_counts.entry(t.to.as_str()).and_modify(|v| *v += 1).or_insert(1);
        }
        let has_back_edge = to_counts.values().any(|&c| c >= 2);

        let entry_state_id: Option<&str> = sm
            .states
            .iter()
            .find(|s| s.entry)
            .map(|s| s.id.as_str());
        let entry_reached = entry_state_id
            .map(|eid| sm.transitions.iter().any(|t| t.to == eid))
            .unwrap_or(false);

        if !has_back_edge && !entry_reached {
            report.warnings.push(
                DagWarning::PeriodicReviewCadenceWithoutRereviewTransition {
                    location: DagLocation {
                        workspace: workspace.to_string(),
                        path: format!("slots.{}.periodic_review_cadence", slot.id),
                    },
                    slot_id: slot.id.clone(),
                },
            );
        }
    }
}

fn validate_validity_window_expired_state(
    workspace: &str,
    dag: &Dag,
    report: &mut DagValidationReport,
) {
    for ev in &dag.evidence_types {
        if ev.validity_window == "once" {
            continue;
        }
        // Heuristic: somewhere in the DAG there should be a slot whose SM
        // has an EXPIRED state. If none, flag warning.
        let any_expired = dag.slots.iter().any(|s| {
            matches!(
                &s.state_machine,
                Some(SlotStateMachine::Structured(sm))
                    if sm.states.iter().any(|st| st.id.eq_ignore_ascii_case("EXPIRED"))
            )
        });
        if !any_expired {
            report
                .warnings
                .push(DagWarning::ValidityWindowWithoutExpiredState {
                    location: DagLocation {
                        workspace: workspace.to_string(),
                        path: format!("evidence_types[{}]", ev.id),
                    },
                    evidence_type_id: ev.id.clone(),
                });
        }
    }
}

// ---------------------------------------------------------------------------
// Cycle detection — DFS over derived_cross_workspace_state graph
// ---------------------------------------------------------------------------

fn detect_derivation_cycles(
    loaded: &BTreeMap<String, LoadedDag>,
    report: &mut DagValidationReport,
) {
    // Build graph: node = "workspace.slot.state" (for declared derived
    // states); edges = "this derived state depends on that (workspace,
    // slot, state)".
    type Node = String;
    let mut edges: HashMap<Node, Vec<Node>> = HashMap::new();
    let mut declared: HashSet<Node> = HashSet::new();

    for (ws, ld) in loaded {
        for d in &ld.dag.derived_cross_workspace_state {
            let node = format!("{}.{}.{}", d.host_workspace, d.host_slot, d.host_state);
            declared.insert(node.clone());
            let deps: Vec<Node> = d
                .derivation
                .all_of
                .iter()
                .chain(d.derivation.any_of.iter())
                .filter_map(|c| match c {
                    DerivationCondition::Structured(s) => {
                        let state_str = match &s.state {
                            Some(StateSelector::Single(s)) => s.clone(),
                            Some(StateSelector::Set(v)) => v.join("|"),
                            None => "<predicate>".to_string(),
                        };
                        Some(format!("{}.{}.{}", s.workspace, s.slot, state_str))
                    }
                    _ => None,
                })
                .collect();
            edges.entry(node).or_default().extend(deps);
            let _ = ws; // silence
        }
    }

    // DFS cycle detection restricted to declared derived states (edges to
    // non-declared nodes are leaves). Owned-node version to keep borrowing
    // simple; graphs are tiny so allocation cost is negligible.
    #[derive(Clone, Copy, PartialEq)]
    enum Color {
        White,
        Gray,
        Black,
    }
    let mut color: HashMap<Node, Color> = HashMap::new();
    for n in declared.iter() {
        color.insert(n.clone(), Color::White);
    }

    for start in declared.iter() {
        if color.get(start).copied() != Some(Color::White) {
            continue;
        }
        let mut stack: Vec<(Node, usize)> = vec![(start.clone(), 0)];
        let mut path: Vec<Node> = vec![start.clone()];
        color.insert(start.clone(), Color::Gray);
        while let Some((node, idx)) = stack.pop() {
            let neighbours: Vec<Node> =
                edges.get(&node).cloned().unwrap_or_default();
            if idx < neighbours.len() {
                stack.push((node.clone(), idx + 1));
                let next = neighbours[idx].clone();
                if !declared.contains(&next) {
                    continue; // leaf
                }
                match color.get(&next).copied().unwrap_or(Color::White) {
                    Color::Gray => {
                        let cycle_start = path
                            .iter()
                            .position(|n| n == &next)
                            .unwrap_or(path.len());
                        let mut cycle_path: Vec<String> =
                            path[cycle_start..].to_vec();
                        cycle_path.push(next);
                        report
                            .errors
                            .push(DagError::DerivedCrossWorkspaceStateCycle { cycle_path });
                    }
                    Color::White => {
                        color.insert(next.clone(), Color::Gray);
                        path.push(next.clone());
                        stack.push((next, 0));
                    }
                    Color::Black => {}
                }
            } else {
                color.insert(node, Color::Black);
                path.pop();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn ws_dag(yaml: &str) -> LoadedDag {
        let dag: Dag = serde_yaml::from_str(yaml).unwrap();
        LoadedDag {
            source_path: std::path::PathBuf::new(),
            dag,
        }
    }

    #[test]
    fn unresolved_cross_workspace_source_slot_errors() {
        let deal = ws_dag(r#"
workspace: deal
dag_id: deal_dag
cross_workspace_constraints:
  - id: bad_ref
    source_workspace: kyc
    source_slot: does_not_exist
    source_state: APPROVED
    target_workspace: deal
    target_slot: deal
    target_transition: "A -> B"
    severity: error
slots:
  - id: deal
    stateless: false
    state_machine:
      id: deal_lifecycle
      states: [{ id: A, entry: true }, { id: B }]
"#);
        let kyc = ws_dag(r#"
workspace: kyc
dag_id: kyc_dag
slots:
  - id: kyc_case
    stateless: false
    state_machine:
      id: kyc_case_lifecycle
      states: [{ id: APPROVED }]
"#);
        let mut map = BTreeMap::new();
        map.insert("deal".to_string(), deal);
        map.insert("kyc".to_string(), kyc);
        let report = validate_dags(&map);
        assert!(
            report
                .errors
                .iter()
                .any(|e| matches!(e, DagError::CrossWorkspaceConstraintUnresolved { .. }))
        );
    }

    #[test]
    fn self_referencing_cross_workspace_errors() {
        let deal = ws_dag(r#"
workspace: deal
dag_id: deal_dag
cross_workspace_constraints:
  - id: self_ref
    source_workspace: deal
    source_slot: deal
    source_state: A
    target_workspace: deal
    target_slot: deal
    target_transition: "A -> B"
slots:
  - id: deal
    stateless: false
    state_machine:
      id: deal_lifecycle
      states: [{ id: A, entry: true }, { id: B }]
"#);
        let mut map = BTreeMap::new();
        map.insert("deal".to_string(), deal);
        let report = validate_dags(&map);
        assert!(report.errors.iter().any(
            |e| matches!(e, DagError::CrossWorkspaceConstraintSelfReference { .. })
        ));
    }

    #[test]
    fn parent_slot_unresolved_errors() {
        let cbu = ws_dag(r#"
workspace: cbu
dag_id: cbu_dag
slots:
  - id: cbu
    stateless: false
    parent_slot:
      workspace: cbu
      slot: does_not_exist
    state_dependency:
      cascade_rules:
        - parent_state: SUSPENDED
          child_allowed_states: [SUSPENDED]
"#);
        let mut map = BTreeMap::new();
        map.insert("cbu".to_string(), cbu);
        let report = validate_dags(&map);
        assert!(
            report
                .errors
                .iter()
                .any(|e| matches!(e, DagError::ParentSlotUnresolved { .. }))
        );
    }

    #[test]
    fn long_lived_missing_suspended_warns() {
        let dag = ws_dag(r#"
workspace: demo
dag_id: demo
slots:
  - id: widget
    stateless: false
    state_machine:
      id: widget_lifecycle
      expected_lifetime: long_lived
      states: [{ id: DRAFT, entry: true }, { id: ACTIVE }, { id: CLOSED }]
"#);
        let mut map = BTreeMap::new();
        map.insert("demo".to_string(), dag);
        let report = validate_dags(&map);
        assert!(report.warnings.iter().any(
            |w| matches!(w, DagWarning::LongLivedSlotMissingSuspended { .. })
        ));
    }

    #[test]
    fn long_lived_exempt_suppresses_warning() {
        let dag = ws_dag(r#"
workspace: demo
dag_id: demo
slots:
  - id: widget
    stateless: false
    suspended_state_exempt: true
    state_machine:
      id: widget_lifecycle
      expected_lifetime: long_lived
      states: [{ id: DRAFT, entry: true }, { id: CLOSED }]
"#);
        let mut map = BTreeMap::new();
        map.insert("demo".to_string(), dag);
        let report = validate_dags(&map);
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn dual_lifecycle_missing_junction_errors() {
        let dag = ws_dag(r#"
workspace: demo
dag_id: demo
slots:
  - id: widget
    stateless: false
    state_machine:
      id: primary
      states: [{ id: A, entry: true }]
    dual_lifecycle:
      - id: secondary
        junction_state_from_primary: NOT_IN_PRIMARY
        states: [{ id: B }]
"#);
        let mut map = BTreeMap::new();
        map.insert("demo".to_string(), dag);
        let report = validate_dags(&map);
        assert!(
            report
                .errors
                .iter()
                .any(|e| matches!(e, DagError::DualLifecycleJunctionMissing { .. }))
        );
    }

    #[test]
    fn category_gated_both_gates_errors() {
        let dag = ws_dag(r#"
workspace: demo
dag_id: demo
slots:
  - id: widget
    stateless: false
    category_gated:
      category_column: cbu_category
      category_source: cbus
      activated_by: [FUND_MANDATE]
      deactivated_by: [RETAIL_CLIENT]
"#);
        let mut map = BTreeMap::new();
        map.insert("demo".to_string(), dag);
        let report = validate_dags(&map);
        assert!(report.errors.iter().any(
            |e| matches!(e, DagError::CategoryGatedMutuallyExclusiveGates { .. })
        ));
    }

    #[test]
    fn derivation_cycle_detected() {
        // A -> B -> A cycle in derived states
        let a = ws_dag(r#"
workspace: a
dag_id: a_dag
slots:
  - id: main
    stateless: false
    state_machine:
      id: a_sm
      states: [{ id: ready }]
derived_cross_workspace_state:
  - id: a_ready
    host_workspace: a
    host_slot: main
    host_state: ready
    derivation:
      all_of:
        - { workspace: b, slot: main, state: ready }
"#);
        let b = ws_dag(r#"
workspace: b
dag_id: b_dag
slots:
  - id: main
    stateless: false
    state_machine:
      id: b_sm
      states: [{ id: ready }]
derived_cross_workspace_state:
  - id: b_ready
    host_workspace: b
    host_slot: main
    host_state: ready
    derivation:
      all_of:
        - { workspace: a, slot: main, state: ready }
"#);
        let mut map = BTreeMap::new();
        map.insert("a".to_string(), a);
        map.insert("b".to_string(), b);
        let report = validate_dags(&map);
        assert!(
            report
                .errors
                .iter()
                .any(|e| matches!(e, DagError::DerivedCrossWorkspaceStateCycle { .. })),
            "expected cycle detection, got: {:#?}",
            report.errors
        );
    }

    #[test]
    fn resolved_tollgate_clean() {
        // Full happy path: KYC approved → CBU tollgate derived from KYC+deal
        let kyc = ws_dag(r#"
workspace: kyc
dag_id: kyc_dag
slots:
  - id: kyc_case
    stateless: false
    suspended_state_exempt: true
    state_machine:
      id: kyc_case_lifecycle
      states: [{ id: INTAKE, entry: true }, { id: APPROVED }]
"#);
        let deal = ws_dag(r#"
workspace: deal
dag_id: deal_dag
slots:
  - id: deal
    stateless: false
    state_machine:
      id: deal_lifecycle
      expected_lifetime: long_lived
      states: [{ id: PROSPECT, entry: true }, { id: CONTRACTED }, { id: ACTIVE }, { id: SUSPENDED }, { id: OFFBOARDED }]
"#);
        let cbu = ws_dag(r#"
workspace: cbu
dag_id: cbu_dag
slots:
  - id: cbu
    stateless: false
    state_machine:
      id: cbu_lifecycle
      expected_lifetime: long_lived
      states: [{ id: DISCOVERED, entry: true }, { id: VALIDATED }, { id: SUSPENDED }, { id: operationally_active }]
derived_cross_workspace_state:
  - id: cbu_operationally_active
    description: "Tollgate aggregate"
    host_workspace: cbu
    host_slot: cbu
    host_state: operationally_active
    derivation:
      all_of:
        - { workspace: kyc, slot: kyc_case, state: APPROVED }
        - { workspace: deal, slot: deal, state: [CONTRACTED, ACTIVE] }
"#);
        let mut map = BTreeMap::new();
        map.insert("kyc".to_string(), kyc);
        map.insert("deal".to_string(), deal);
        map.insert("cbu".to_string(), cbu);
        let report = validate_dags(&map);
        assert!(
            report.is_clean(),
            "expected clean, got: {:#?}",
            report.errors
        );
    }
}
