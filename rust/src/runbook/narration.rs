//! Effect narration for runbook plan execution.
//!
//! Generates human-readable narration per step and aggregate summaries
//! using the three-part fixed response: what changed, what state is true, what next.

use serde::{Deserialize, Serialize};

use super::plan_types::{PlanStepStatus, RunbookPlan, StepResult};
use crate::repl::types_v2::WorkspaceKind;

// ---------------------------------------------------------------------------
// Narration types
// ---------------------------------------------------------------------------

/// Outcome of narrating a single step.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum NarrationOutcome {
    Success {
        what_changed: String,
        state_now: String,
        what_next: Vec<String>,
    },
    Failed {
        error: String,
        recovery_hint: Option<String>,
    },
    Skipped {
        reason: String,
    },
}

/// Narration for a single plan step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepNarration {
    pub step_index: usize,
    pub verb_fqn: String,
    pub sentence: String,
    pub outcome: NarrationOutcome,
    pub workspace: WorkspaceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale_warning: Option<String>,
}

/// Aggregate narration for an entire plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanNarration {
    pub plan_id: String,
    pub total_steps: usize,
    pub completed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub step_narrations: Vec<StepNarration>,
    pub aggregate_summary: String,
}

// ---------------------------------------------------------------------------
// Narration generation
// ---------------------------------------------------------------------------

/// Generate narration for a single step result.
pub fn narrate_step(
    plan: &RunbookPlan,
    result: &StepResult,
) -> StepNarration {
    let step = plan.steps.get(result.step_seq);
    let sentence = step
        .map(|s| s.sentence.clone())
        .unwrap_or_else(|| format!("Step {}", result.step_seq));
    let workspace = step
        .map(|s| s.workspace.clone())
        .unwrap_or(WorkspaceKind::Cbu);

    let outcome = match result.status {
        PlanStepStatus::Succeeded => NarrationOutcome::Success {
            what_changed: format!("{} completed successfully", result.verb_fqn),
            state_now: "Step completed".into(),
            what_next: vec![],
        },
        PlanStepStatus::Failed => NarrationOutcome::Failed {
            error: result
                .error
                .clone()
                .unwrap_or_else(|| "Unknown error".into()),
            recovery_hint: Some("Review the error and retry or skip this step".into()),
        },
        PlanStepStatus::Skipped => NarrationOutcome::Skipped {
            reason: result
                .error
                .clone()
                .unwrap_or_else(|| "Dependency failed".into()),
        },
        _ => NarrationOutcome::Skipped {
            reason: format!("Step in {:?} state", result.status),
        },
    };

    // Check for stale workspace warning
    let stale_warning = step.and_then(|s| {
        if result.step_seq > 0 {
            if let Some(pw) = plan.steps.get(result.step_seq - 1).map(|p| &p.workspace) {
                if pw != &s.workspace {
                    return Some(format!(
                        "Workspace transition from {:?} — frame may have stale state",
                        pw
                    ));
                }
            }
        }
        None
    });

    StepNarration {
        step_index: result.step_seq,
        verb_fqn: result.verb_fqn.clone(),
        sentence,
        outcome,
        workspace,
        stale_warning,
    }
}

/// Generate aggregate narration from all step results.
pub fn narrate_plan(plan: &RunbookPlan, results: &[StepResult]) -> PlanNarration {
    let step_narrations: Vec<StepNarration> =
        results.iter().map(|r| narrate_step(plan, r)).collect();

    let completed = results
        .iter()
        .filter(|r| r.status == PlanStepStatus::Succeeded)
        .count();
    let failed = results
        .iter()
        .filter(|r| r.status == PlanStepStatus::Failed)
        .count();
    let skipped = results
        .iter()
        .filter(|r| r.status == PlanStepStatus::Skipped)
        .count();

    let aggregate_summary = if failed == 0 && skipped == 0 {
        format!(
            "All {} steps completed successfully",
            plan.steps.len()
        )
    } else {
        format!(
            "{} of {} steps completed, {} failed, {} skipped",
            completed,
            plan.steps.len(),
            failed,
            skipped
        )
    };

    PlanNarration {
        plan_id: plan.id.0.clone(),
        total_steps: plan.steps.len(),
        completed,
        failed,
        skipped,
        step_narrations,
        aggregate_summary,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runbook::plan_types::*;
    use crate::repl::types_v2::{SubjectKind, VerbRef};
    use chrono::Utc;
    use std::collections::BTreeMap;
    use uuid::Uuid;

    fn test_plan() -> RunbookPlan {
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
    fn narrate_success_step() {
        let plan = test_plan();
        let result = StepResult {
            step_seq: 0,
            verb_fqn: "cbu.create".into(),
            status: PlanStepStatus::Succeeded,
            output: None,
            error: None,
            executed_at: Utc::now(),
        };
        let narration = narrate_step(&plan, &result);
        assert!(matches!(narration.outcome, NarrationOutcome::Success { .. }));
        assert_eq!(narration.workspace, WorkspaceKind::Cbu);
    }

    #[test]
    fn narrate_failed_step() {
        let plan = test_plan();
        let result = StepResult {
            step_seq: 0,
            verb_fqn: "cbu.create".into(),
            status: PlanStepStatus::Failed,
            output: None,
            error: Some("DB connection failed".into()),
            executed_at: Utc::now(),
        };
        let narration = narrate_step(&plan, &result);
        match &narration.outcome {
            NarrationOutcome::Failed { error, .. } => {
                assert_eq!(error, "DB connection failed");
            }
            _ => panic!("Expected Failed outcome"),
        }
    }

    #[test]
    fn narrate_plan_aggregate() {
        let plan = test_plan();
        let results = vec![
            StepResult {
                step_seq: 0,
                verb_fqn: "cbu.create".into(),
                status: PlanStepStatus::Succeeded,
                output: None,
                error: None,
                executed_at: Utc::now(),
            },
            StepResult {
                step_seq: 1,
                verb_fqn: "kyc.open-case".into(),
                status: PlanStepStatus::Failed,
                output: None,
                error: Some("precondition failed".into()),
                executed_at: Utc::now(),
            },
        ];
        let narration = narrate_plan(&plan, &results);
        assert_eq!(narration.total_steps, 2);
        assert_eq!(narration.completed, 1);
        assert_eq!(narration.failed, 1);
        assert!(narration.aggregate_summary.contains("1 failed"));
    }

    #[test]
    fn stale_warning_on_workspace_transition() {
        let plan = test_plan();
        let result = StepResult {
            step_seq: 1,
            verb_fqn: "kyc.open-case".into(),
            status: PlanStepStatus::Succeeded,
            output: None,
            error: None,
            executed_at: Utc::now(),
        };
        let narration = narrate_step(&plan, &result);
        assert!(narration.stale_warning.is_some());
    }
}
