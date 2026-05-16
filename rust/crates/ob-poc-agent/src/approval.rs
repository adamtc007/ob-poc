//! Approval policy + refused-draft tracking — Phase 3.6 (C-12 / C-13).
//!
//! C-12 (approval gates): the pack's `risk_policy` is read into a
//! typed [`ApprovalDecision`] every turn. Editors see whether a
//! draft requires explicit user confirmation before it can be
//! promoted to `Confirmed`. Phase 3.6 wires the policy read; the
//! enforcement gate is the lifecycle FSM (Phase 3.1d) — the editor
//! must call `obpoc/goal_frame/confirm` before the runtime takes
//! the frame to `Confirmed`.
//!
//! C-13 (refusal handling): the goal frame now carries a
//! `refused_drafts` list of verb FQNs the user explicitly rejected
//! within the current goal. A new ACP method
//! `obpoc/goal_frame/refuse_draft` appends to that list without
//! moving the frame to terminal. The motivation prompt (Phase 3.5)
//! and the deterministic fallback both honour the list so the next
//! draft picks something else.
//!
//! ## Spike scope
//!
//! - Approval evaluator is a pure read over the pack manifest +
//!   frame state — no policy plug-ins. Phase 4 widens with
//!   risk-tiered routing and HITL attestation gates per V&S §6.4.
//! - Refused-draft tracking is per-frame and goes away when a new
//!   frame is seeded (i.e. when the previous frame is refused or
//!   completed). Phase 4 may persist across frames if the editor
//!   asks for cross-session memory.

use serde::{Deserialize, Serialize};

use crate::blockers::BlockerReport;
use crate::frontier::Frontier;
use crate::goal_frame::GoalFrame;
use crate::index::SessionIndex;

/// Why approval was (or wasn't) required.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalReason {
    /// Pack `risk_policy.require_confirm_before_execute = true`.
    PackPolicy,
    /// Pack policy auto-confirms — proceed without HITL.
    NoConfirmationNeeded,
    /// (Phase 4) Guardrail blocked — outside the spike's surface
    /// but reserved so consumers can match without a future schema
    /// migration.
    DeniedByGuardrail,
}

/// Approval verdict for one draft.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalDecision {
    /// Whether the editor must call
    /// `obpoc/goal_frame/confirm` before the runtime promotes the
    /// frame to `Confirmed`.
    pub required: bool,
    /// Why approval was (or wasn't) required.
    pub reason: ApprovalReason,
    /// Free-form details for audit / editor display.
    pub detail: String,
}

/// Pure synchronous evaluator. No IO.
pub struct ApprovalEvaluator;

impl ApprovalEvaluator {
    /// Evaluate the approval decision for `frame` against `index`
    /// (pack manifest). `frontier` + `blockers` are passed for
    /// future risk-aware logic; today the evaluator reads only the
    /// pack `risk_policy.require_confirm_before_execute`.
    pub fn evaluate(
        index: &SessionIndex,
        _frame: &GoalFrame,
        _frontier: &Frontier,
        _blockers: &BlockerReport,
    ) -> ApprovalDecision {
        if index.pack.risk_policy.require_confirm_before_execute {
            ApprovalDecision {
                required: true,
                reason: ApprovalReason::PackPolicy,
                detail: format!(
                    "pack '{}' requires user confirmation before execution",
                    index.pack.id
                ),
            }
        } else {
            ApprovalDecision {
                required: false,
                reason: ApprovalReason::NoConfirmationNeeded,
                detail: format!(
                    "pack '{}' auto-confirms (risk_policy.require_confirm_before_execute = false)",
                    index.pack.id
                ),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockers::BlockerReport;
    use crate::constellation::ConstellationSnapshot;
    use crate::frontier::FrontierEngine;
    use chrono::Utc;
    use ob_poc_journey::pack::load_pack_from_bytes;
    use ob_poc_types::session::kinds::WorkspaceKind;

    fn pack_yaml(require: bool) -> String {
        format!(
            r#"
id: book-setup
name: Book Setup
version: "0.1"
description: approval test fixture
invocation_phrases: []
required_context: []
optional_context: []
workspaces:
  - cbu
allowed_verbs:
  - cbu.create
forbidden_verbs: []
risk_policy:
  require_confirm_before_execute: {require}
  max_steps_without_confirm: 1
required_questions: []
optional_questions: []
stop_rules: []
templates: []
section_layout: []
definition_of_done: []
progress_signals: []
"#
        )
    }

    fn make_index(require: bool) -> SessionIndex {
        let yaml = pack_yaml(require);
        let (pack, pack_hash) = load_pack_from_bytes(yaml.as_bytes()).unwrap();
        SessionIndex {
            pack,
            pack_hash,
            workspace: WorkspaceKind::Cbu,
            loaded_at: Utc::now(),
        }
    }

    #[test]
    fn require_confirm_yields_required_decision() {
        let index = make_index(true);
        let frame = GoalFrame::seed_for_spike("draft", &index);
        let snapshot = ConstellationSnapshot::empty();
        let frontier = FrontierEngine::compute(&index, &snapshot);
        let blockers = BlockerReport::default();
        let decision = ApprovalEvaluator::evaluate(&index, &frame, &frontier, &blockers);
        assert!(decision.required);
        assert_eq!(decision.reason, ApprovalReason::PackPolicy);
    }

    #[test]
    fn auto_confirm_pack_yields_not_required_decision() {
        let index = make_index(false);
        let frame = GoalFrame::seed_for_spike("draft", &index);
        let snapshot = ConstellationSnapshot::empty();
        let frontier = FrontierEngine::compute(&index, &snapshot);
        let blockers = BlockerReport::default();
        let decision = ApprovalEvaluator::evaluate(&index, &frame, &frontier, &blockers);
        assert!(!decision.required);
        assert_eq!(decision.reason, ApprovalReason::NoConfirmationNeeded);
    }
}
