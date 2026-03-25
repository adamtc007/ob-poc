//! Multi-workspace runbook plan executor.
//!
//! Cursor-driven execution loop with workspace transitions (Sage/REPL ping-pong).
//! Resolves forward references, tracks step results, and handles cancellation.

use anyhow::{anyhow, Result};
use chrono::Utc;
use uuid::Uuid;

use super::plan_types::{
    BindingTable, EntityBinding, PlanStepStatus, RunbookPlan, RunbookPlanStatus, StepResult,
};

// ---------------------------------------------------------------------------
// Executor types
// ---------------------------------------------------------------------------

/// Result of advancing the plan by one or more steps.
#[derive(Debug, Clone)]
pub struct PlanAdvanceResult {
    /// Steps that were executed in this advance.
    pub executed_steps: Vec<StepResult>,
    /// Whether the plan is complete.
    pub plan_complete: bool,
    /// Whether a workspace transition is needed before the next step.
    pub needs_workspace_transition: bool,
    /// The target workspace for the next step (if transition needed).
    pub next_workspace: Option<crate::repl::types_v2::WorkspaceKind>,
}

// ---------------------------------------------------------------------------
// Executor logic
// ---------------------------------------------------------------------------

/// Advance the plan by one step.
///
/// Returns the step result and whether more steps remain.
/// The caller is responsible for workspace transitions and actual verb execution.
pub fn advance_plan_step(
    plan: &mut RunbookPlan,
    cursor: usize,
    execution_result: Result<serde_json::Value>,
) -> Result<StepResult> {
    let step_count = plan.steps.len();
    let step = plan
        .steps
        .get_mut(cursor)
        .ok_or_else(|| anyhow!("Step {} out of range (plan has {} steps)", cursor, step_count))?;

    let (status, output, error) = match execution_result {
        Ok(value) => {
            step.status = PlanStepStatus::Succeeded;
            (PlanStepStatus::Succeeded, Some(value), None)
        }
        Err(e) => {
            step.status = PlanStepStatus::Failed;
            (PlanStepStatus::Failed, None, Some(e.to_string()))
        }
    };

    let result = StepResult {
        step_seq: cursor,
        verb_fqn: step.verb.verb_fqn.clone(),
        status,
        output,
        error,
        executed_at: Utc::now(),
    };

    Ok(result)
}

/// Resolve forward references for a step from the binding table.
///
/// Returns the resolved subject UUID if the step uses a forward ref.
pub fn resolve_step_bindings(
    plan: &RunbookPlan,
    step_seq: usize,
) -> Result<Option<Uuid>> {
    let step = plan
        .steps
        .get(step_seq)
        .ok_or_else(|| anyhow!("Step {} not found", step_seq))?;

    match &step.subject_binding {
        EntityBinding::Literal { id } => Ok(Some(*id)),
        EntityBinding::ForwardRef { output_field, .. } => {
            match plan.bindings.resolved.get(output_field) {
                Some(id) => Ok(Some(*id)),
                None => Ok(None), // Not yet resolved
            }
        }
    }
}

/// Record a resolved output into the binding table.
pub fn record_step_output(
    bindings: &mut BindingTable,
    _step_seq: usize,
    field_name: &str,
    value: Uuid,
) {
    bindings.resolved.insert(field_name.to_string(), value);
}

/// Skip all steps that depend on a failed step.
pub fn skip_dependent_steps(
    plan: &mut RunbookPlan,
    failed_step: usize,
) -> Vec<StepResult> {
    let mut skipped = Vec::new();
    for step in &mut plan.steps {
        if step.depends_on.contains(&failed_step)
            && step.status == PlanStepStatus::Pending
        {
            step.status = PlanStepStatus::Skipped;
            skipped.push(StepResult {
                step_seq: step.seq,
                verb_fqn: step.verb.verb_fqn.clone(),
                status: PlanStepStatus::Skipped,
                output: None,
                error: Some(format!("Dependency step {} failed", failed_step)),
                executed_at: Utc::now(),
            });
        }
    }
    skipped
}

/// Check if the next step requires a workspace transition.
pub fn needs_workspace_transition(
    plan: &RunbookPlan,
    current_cursor: usize,
) -> Option<crate::repl::types_v2::WorkspaceKind> {
    let current = plan.steps.get(current_cursor)?;
    let next = plan.steps.get(current_cursor + 1)?;
    if current.workspace != next.workspace {
        Some(next.workspace.clone())
    } else {
        None
    }
}

/// Cancel a plan mid-execution, marking remaining steps as Skipped.
pub fn cancel_plan(plan: &mut RunbookPlan) -> Vec<StepResult> {
    let mut cancelled = Vec::new();
    for step in &mut plan.steps {
        if matches!(step.status, PlanStepStatus::Pending | PlanStepStatus::Ready) {
            step.status = PlanStepStatus::Skipped;
            cancelled.push(StepResult {
                step_seq: step.seq,
                verb_fqn: step.verb.verb_fqn.clone(),
                status: PlanStepStatus::Skipped,
                output: None,
                error: Some("Plan cancelled".into()),
                executed_at: Utc::now(),
            });
        }
    }
    plan.status = RunbookPlanStatus::Cancelled;
    cancelled
}

/// Update plan status based on step results.
pub fn update_plan_status(plan: &mut RunbookPlan) {
    let all_done = plan
        .steps
        .iter()
        .all(|s| matches!(s.status, PlanStepStatus::Succeeded | PlanStepStatus::Skipped | PlanStepStatus::Failed));

    if !all_done {
        return;
    }

    let any_failed = plan.steps.iter().any(|s| s.status == PlanStepStatus::Failed);
    if any_failed {
        let failed_step = plan
            .steps
            .iter()
            .find(|s| s.status == PlanStepStatus::Failed)
            .map(|s| s.seq);
        plan.status = RunbookPlanStatus::Failed {
            error: "One or more steps failed".into(),
            failed_step,
        };
    } else {
        plan.status = RunbookPlanStatus::Completed {
            completed_at: Utc::now(),
        };
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runbook::plan_types::*;
    use crate::repl::types_v2::{SubjectKind, VerbRef, WorkspaceKind};
    use std::collections::BTreeMap;

    fn two_step_plan() -> RunbookPlan {
        RunbookPlan::new(
            Uuid::nil(),
            vec![
                RunbookPlanStep {
                    seq: 0,
                    workspace: WorkspaceKind::Cbu,
                    constellation_map: "cbu-onboarding".into(),
                    subject_kind: SubjectKind::Cbu,
                    subject_binding: EntityBinding::Literal { id: Uuid::nil() },
                    verb: VerbRef {
                        verb_fqn: "cbu.create".into(),
                        display_name: "Create CBU".into(),
                    },
                    sentence: "Create a new CBU".into(),
                    args: BTreeMap::new(),
                    preconditions: vec![],
                    expected_effect: "CBU created".into(),
                    depends_on: vec![],
                    status: PlanStepStatus::Pending,
                },
                RunbookPlanStep {
                    seq: 1,
                    workspace: WorkspaceKind::Kyc,
                    constellation_map: "kyc-lifecycle".into(),
                    subject_kind: SubjectKind::Case,
                    subject_binding: EntityBinding::ForwardRef {
                        source_step: 0,
                        output_field: "created_cbu_id".into(),
                    },
                    verb: VerbRef {
                        verb_fqn: "kyc.open-case".into(),
                        display_name: "Open KYC Case".into(),
                    },
                    sentence: "Open a KYC case".into(),
                    args: BTreeMap::new(),
                    preconditions: vec![],
                    expected_effect: "KYC case opened".into(),
                    depends_on: vec![0],
                    status: PlanStepStatus::Pending,
                },
            ],
            BindingTable::default(),
            vec![],
        )
    }

    #[test]
    fn advance_step_success() {
        let mut plan = two_step_plan();
        let result = advance_plan_step(
            &mut plan,
            0,
            Ok(serde_json::json!({"created_cbu_id": "00000000-0000-0000-0000-000000000001"})),
        )
        .unwrap();
        assert_eq!(result.status, PlanStepStatus::Succeeded);
        assert_eq!(plan.steps[0].status, PlanStepStatus::Succeeded);
    }

    #[test]
    fn advance_step_failure() {
        let mut plan = two_step_plan();
        let result =
            advance_plan_step(&mut plan, 0, Err(anyhow!("DB error"))).unwrap();
        assert_eq!(result.status, PlanStepStatus::Failed);
    }

    #[test]
    fn forward_ref_resolution() {
        let mut plan = two_step_plan();
        // Before resolution
        assert_eq!(resolve_step_bindings(&plan, 1).unwrap(), None);
        // After resolution
        let id = Uuid::new_v4();
        record_step_output(&mut plan.bindings, 0, "created_cbu_id", id);
        assert_eq!(resolve_step_bindings(&plan, 1).unwrap(), Some(id));
    }

    #[test]
    fn workspace_transition_detection() {
        let plan = two_step_plan();
        let next = needs_workspace_transition(&plan, 0);
        assert_eq!(next, Some(WorkspaceKind::Kyc));
    }

    #[test]
    fn cancel_marks_remaining_skipped() {
        let mut plan = two_step_plan();
        let cancelled = cancel_plan(&mut plan);
        assert_eq!(cancelled.len(), 2);
        assert!(matches!(plan.status, RunbookPlanStatus::Cancelled));
    }

    #[test]
    fn skip_dependent_steps_on_failure() {
        let mut plan = two_step_plan();
        plan.steps[0].status = PlanStepStatus::Failed;
        let skipped = skip_dependent_steps(&mut plan, 0);
        assert_eq!(skipped.len(), 1);
        assert_eq!(skipped[0].step_seq, 1);
    }
}
