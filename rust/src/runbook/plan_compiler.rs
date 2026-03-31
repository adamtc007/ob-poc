//! Multi-workspace runbook plan compilation.
//!
//! Compiles a `RunbookPlan` from hydrated constellation states across workspaces.
//! Walks non-terminal constellation nodes, derives advancing verbs from the
//! scoped verb surface, orders by workspace grouping + dependency DAG, and
//! detects cross-workspace forward references.

use std::collections::{BTreeMap, HashMap};

use anyhow::Result;
use uuid::Uuid;

use super::plan_types::{
    BindingTable, EntityBinding, PlanStepStatus, RunbookPlan, RunbookPlanStep,
};
use crate::repl::types_v2::{SubjectKind, VerbRef, WorkspaceFrame, WorkspaceKind};
use sem_os_core::verb_contract::VerbOutput;

// ---------------------------------------------------------------------------
// Compiler input
// ---------------------------------------------------------------------------

/// A workspace context with its hydrated constellation state, used as compiler input.
#[derive(Debug, Clone)]
pub struct WorkspaceInput {
    pub workspace: WorkspaceKind,
    pub constellation_map: String,
    pub subject_kind: SubjectKind,
    pub subject_id: Option<Uuid>,
    pub advancing_verbs: Vec<VerbRef>,
    pub verb_outputs: BTreeMap<String, Vec<VerbOutput>>,
}

// ---------------------------------------------------------------------------
// Compiler
// ---------------------------------------------------------------------------

/// Compile a multi-workspace runbook plan from workspace inputs.
///
/// Steps:
/// 1. Iterate workspace inputs, gather non-terminal advancing verbs
/// 2. Order by workspace grouping + dependency edges
/// 3. Detect cross-workspace entity references → create ForwardRef bindings
/// 4. Compute content-addressed plan ID
pub fn compile_runbook_plan(
    session_id: Uuid,
    workspace_inputs: &[WorkspaceInput],
    source_research: Vec<u64>,
) -> Result<RunbookPlan> {
    let mut steps = Vec::new();
    let mut bindings = BindingTable::default();
    let mut seq = 0usize;

    // Phase 1: Gather steps from each workspace
    for input in workspace_inputs {
        for verb in &input.advancing_verbs {
            let subject_binding = if let Some(id) = input.subject_id {
                EntityBinding::Literal { id }
            } else {
                // Look for a forward ref from a prior step that produces this entity kind
                find_forward_ref(&steps, &input.subject_kind, &bindings)
                    .unwrap_or(EntityBinding::Literal { id: Uuid::nil() })
            };

            let step = RunbookPlanStep {
                seq,
                workspace: input.workspace.clone(),
                constellation_map: input.constellation_map.clone(),
                subject_kind: input.subject_kind.clone(),
                subject_binding,
                verb: verb.clone(),
                sentence: verb.display_name.clone(),
                args: BTreeMap::new(),
                preconditions: Vec::new(),
                expected_effect: format!("{} executed", verb.verb_fqn),
                depends_on: compute_dependencies(seq, &steps),
                status: PlanStepStatus::Pending,
            };
            steps.push(step);

            // Register outputs as forward-ref bindings
            if let Some(outputs) = input.verb_outputs.get(&verb.verb_fqn) {
                for output in outputs {
                    let binding_name = format!("${}", output.field_name);
                    bindings.entries.insert(
                        binding_name,
                        EntityBinding::ForwardRef {
                            source_step: seq,
                            output_field: output.field_name.clone(),
                        },
                    );
                }
            }

            seq += 1;
        }
    }

    Ok(RunbookPlan::new(
        session_id,
        steps,
        bindings,
        source_research,
    ))
}

/// Find a forward reference for a subject kind from prior steps.
fn find_forward_ref(
    prior_steps: &[RunbookPlanStep],
    _subject_kind: &SubjectKind,
    _bindings: &BindingTable,
) -> Option<EntityBinding> {
    // Look backwards for the most recent step that might produce this entity
    prior_steps.last().map(|last| EntityBinding::ForwardRef {
        source_step: last.seq,
        output_field: format!(
            "created_{}_id",
            last.verb.verb_fqn.split('.').next().unwrap_or("entity")
        ),
    })
}

/// Compute dependency edges for a step based on workspace transitions.
fn compute_dependencies(current_seq: usize, prior_steps: &[RunbookPlanStep]) -> Vec<usize> {
    if current_seq == 0 {
        return vec![];
    }
    // A step depends on the immediately preceding step if it's in a different workspace
    if let Some(prev) = prior_steps.last() {
        if prior_steps
            .first()
            .is_some_and(|first| first.workspace != prev.workspace)
        {
            return vec![prev.seq];
        }
        // Within same workspace, depend on prev for ordering
        vec![prev.seq]
    } else {
        vec![]
    }
}

/// Build workspace inputs from session workspace frames.
pub fn inputs_from_frames(
    frames: &[WorkspaceFrame],
    verb_surfaces: &HashMap<WorkspaceKind, Vec<VerbRef>>,
    verb_outputs: &BTreeMap<String, Vec<VerbOutput>>,
) -> Vec<WorkspaceInput> {
    frames
        .iter()
        .map(|f| WorkspaceInput {
            workspace: f.workspace.clone(),
            constellation_map: f.constellation_map.clone(),
            subject_kind: f.subject_kind.clone().unwrap_or(SubjectKind::Cbu),
            subject_id: f.subject_id,
            advancing_verbs: verb_surfaces.get(&f.workspace).cloned().unwrap_or_default(),
            verb_outputs: verb_outputs.clone(),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Constellation DAG Discovery
// ---------------------------------------------------------------------------

/// A non-terminal slot discovered during constellation DAG traversal.
#[derive(Debug, Clone)]
pub struct AdvancingSlot {
    /// Dot-delimited path within the constellation tree.
    pub slot_path: String,
    /// Slot name.
    pub slot_name: String,
    /// Current computed state of the slot.
    pub computed_state: String,
    /// Current effective state of the slot.
    pub effective_state: String,
    /// Completion progress (0-100).
    pub progress: u8,
    /// Whether this slot blocks overall progress.
    pub blocking: bool,
    /// Entity bound to this slot (if any).
    pub entity_id: Option<Uuid>,
    /// Verbs that can advance this slot.
    pub advancing_verbs: Vec<String>,
}

/// Walk a hydrated constellation tree and discover non-terminal slots
/// with available advancing verbs.
///
/// Returns slots ordered depth-first, blocking slots first within each level.
pub fn discover_advancing_slots(
    hydrated: &crate::sem_os_runtime::constellation_runtime::HydratedConstellation,
) -> Vec<AdvancingSlot> {
    let mut results = Vec::new();
    for slot in &hydrated.slots {
        collect_advancing_slots_recursive(slot, "", &mut results);
    }
    // Sort: blocking slots first, then by path for deterministic ordering
    results.sort_by(|a, b| {
        b.blocking
            .cmp(&a.blocking)
            .then_with(|| a.slot_path.cmp(&b.slot_path))
    });
    results
}

fn collect_advancing_slots_recursive(
    slot: &crate::sem_os_runtime::constellation_runtime::HydratedSlot,
    parent_path: &str,
    results: &mut Vec<AdvancingSlot>,
) {
    let path = if parent_path.is_empty() {
        slot.name.clone()
    } else {
        format!("{}.{}", parent_path, slot.name)
    };

    // Non-terminal: progress < 100 and has available verbs
    if slot.progress < 100 && !slot.available_verbs.is_empty() {
        results.push(AdvancingSlot {
            slot_path: path.clone(),
            slot_name: slot.name.clone(),
            computed_state: slot.computed_state.clone(),
            effective_state: slot.effective_state.clone(),
            progress: slot.progress,
            blocking: slot.blocking,
            entity_id: slot.entity_id,
            advancing_verbs: slot.available_verbs.clone(),
        });
    }

    // Recurse into children
    for child in &slot.children {
        collect_advancing_slots_recursive(child, &path, results);
    }
}

/// Build `WorkspaceInput` from a hydrated constellation and workspace context.
///
/// Uses DAG discovery to automatically derive advancing verbs from
/// non-terminal constellation slots.
pub fn input_from_hydrated_constellation(
    workspace: &WorkspaceKind,
    constellation_map: &str,
    subject_kind: SubjectKind,
    subject_id: Option<Uuid>,
    hydrated: &crate::sem_os_runtime::constellation_runtime::HydratedConstellation,
    verb_outputs: &BTreeMap<String, Vec<VerbOutput>>,
) -> WorkspaceInput {
    let slots = discover_advancing_slots(hydrated);
    let advancing_verbs: Vec<VerbRef> = slots
        .iter()
        .flat_map(|s| {
            s.advancing_verbs.iter().map(|v| VerbRef {
                verb_fqn: v.clone(),
                display_name: v.replace('.', " "),
            })
        })
        .collect();

    WorkspaceInput {
        workspace: workspace.clone(),
        constellation_map: constellation_map.to_string(),
        subject_kind,
        subject_id,
        advancing_verbs,
        verb_outputs: verb_outputs.clone(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_input(workspace: WorkspaceKind, verbs: Vec<(&str, &str)>) -> WorkspaceInput {
        WorkspaceInput {
            workspace,
            constellation_map: "test-map".into(),
            subject_kind: SubjectKind::Cbu,
            subject_id: Some(Uuid::nil()),
            advancing_verbs: verbs
                .into_iter()
                .map(|(fqn, name)| VerbRef {
                    verb_fqn: fqn.into(),
                    display_name: name.into(),
                })
                .collect(),
            verb_outputs: BTreeMap::new(),
        }
    }

    #[test]
    fn compile_three_workspace_plan() {
        let inputs = vec![
            make_input(WorkspaceKind::Cbu, vec![("cbu.create", "Create CBU")]),
            make_input(
                WorkspaceKind::Kyc,
                vec![("kyc-case.create", "Open KYC Case")],
            ),
            make_input(WorkspaceKind::Deal, vec![("deal.create", "Create Deal")]),
        ];
        let plan = compile_runbook_plan(Uuid::nil(), &inputs, vec![]).unwrap();
        assert_eq!(plan.steps.len(), 3);
        assert_eq!(plan.steps[0].workspace, WorkspaceKind::Cbu);
        assert_eq!(plan.steps[1].workspace, WorkspaceKind::Kyc);
        assert_eq!(plan.steps[2].workspace, WorkspaceKind::Deal);
    }

    #[test]
    fn dag_ordering_has_dependencies() {
        let inputs = vec![
            make_input(WorkspaceKind::Cbu, vec![("cbu.create", "Create CBU")]),
            make_input(
                WorkspaceKind::Kyc,
                vec![("kyc-case.create", "Open KYC Case")],
            ),
        ];
        let plan = compile_runbook_plan(Uuid::nil(), &inputs, vec![]).unwrap();
        // Step 1 depends on step 0
        assert!(plan.steps[1].depends_on.contains(&0));
    }

    #[test]
    fn forward_ref_detection() {
        let mut outputs = BTreeMap::new();
        outputs.insert(
            "cbu.create".to_string(),
            vec![VerbOutput {
                field_name: "created_cbu_id".into(),
                output_type: "uuid".into(),
                entity_kind: Some("cbu".into()),
                description: None,
            }],
        );
        let inputs = vec![WorkspaceInput {
            workspace: WorkspaceKind::Cbu,
            constellation_map: "cbu-onboarding".into(),
            subject_kind: SubjectKind::Cbu,
            subject_id: Some(Uuid::nil()),
            advancing_verbs: vec![VerbRef {
                verb_fqn: "cbu.create".into(),
                display_name: "Create CBU".into(),
            }],
            verb_outputs: outputs,
        }];
        let plan = compile_runbook_plan(Uuid::nil(), &inputs, vec![]).unwrap();
        assert!(plan.bindings.entries.contains_key("$created_cbu_id"));
    }

    #[test]
    fn discover_advancing_slots_filters_non_terminal() {
        use crate::sem_os_runtime::constellation_runtime::*;

        let hydrated = HydratedConstellation {
            constellation: "cbu-onboarding".into(),
            description: None,
            jurisdiction: "LU".into(),
            map_revision: "1".into(),
            cbu_id: Uuid::nil(),
            case_id: None,
            slots: vec![
                HydratedSlot {
                    name: "cbu".into(),
                    path: "cbu".into(),
                    slot_type: HydratedSlotType::Cbu,
                    cardinality: HydratedCardinality::Root,
                    entity_id: Some(Uuid::nil()),
                    record_id: None,
                    computed_state: "active".into(),
                    effective_state: "active".into(),
                    progress: 100,
                    blocking: false,
                    warnings: vec![],
                    overlays: vec![],
                    graph_node_count: None,
                    graph_edge_count: None,
                    graph_nodes: vec![],
                    graph_edges: vec![],
                    available_verbs: vec!["cbu.read".into()],
                    blocked_verbs: vec![],
                    children: vec![],
                },
                HydratedSlot {
                    name: "kyc".into(),
                    path: "kyc".into(),
                    slot_type: HydratedSlotType::Case,
                    cardinality: HydratedCardinality::Mandatory,
                    entity_id: None,
                    record_id: None,
                    computed_state: "empty".into(),
                    effective_state: "empty".into(),
                    progress: 0,
                    blocking: true,
                    warnings: vec![],
                    overlays: vec![],
                    graph_node_count: None,
                    graph_edge_count: None,
                    graph_nodes: vec![],
                    graph_edges: vec![],
                    available_verbs: vec!["kyc-case.create".into()],
                    blocked_verbs: vec![],
                    children: vec![],
                },
            ],
        };

        let slots = discover_advancing_slots(&hydrated);
        // Only the kyc slot should appear (cbu is at 100% progress)
        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].slot_name, "kyc");
        assert!(slots[0].blocking);
        assert_eq!(slots[0].advancing_verbs, vec!["kyc-case.create"]);
    }
}
