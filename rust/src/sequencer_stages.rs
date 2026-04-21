//! Phase 5b-deep — typed stage I/O + error contracts (per spec §8.3).
//!
//! Each of the nine V&S stages gets a typed input, typed output, and
//! an explicit error variant. No `serde_json::Value` pass-through.
//!
//! # Status
//!
//! This is the **scaffold** half of Phase 5b-deep. The types defined
//! here are the contracts every stage extraction will consume; the
//! actual extraction of `sequencer.rs`'s tollgate handlers into typed
//! per-stage functions is incremental work that lands one stage at a
//! time as `5b-deep-stage-N` slices. See §8.6 for the tollgate ↔
//! stage mapping that drives the per-stage extraction.
//!
//! # Why a separate file
//!
//! `sequencer.rs` is already 7800 LOC. Carving the stage contracts
//! into a sibling module gives us:
//!
//! 1. A single place to read every stage's contract without having to
//!    page through the orchestrator's tollgate handler bodies.
//! 2. A natural extension point — when a stage gets extracted, its
//!    function lives next to its contract.
//! 3. A determinism harness target — the future
//!    `determinism_harness` crate (per §9.4) imports this module to
//!    pin the stage-output shapes it byte-compares.
//!
//! # Error shape rationale
//!
//! [`StageError`] is the union of every stage's failure mode named in
//! §8.3. The variants are wide because the harness wants to assert
//! "no error" or "specific error variant" without unwrapping a generic
//! `String`. Stage handlers convert their internal errors into the
//! appropriate variant at the stage boundary.

use std::time::Duration;

use ob_poc_types::{EnvelopeVersion, TraceId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::repl::types_v2::UserInputV2;

// ─── Stage 1 — Utterance receipt ──────────────────────────────────────────────

/// Stage 1 input: raw user input + session id.
///
/// Maps to `ReplOrchestratorV2::process()` lines 763-788.
#[derive(Debug, Clone)]
pub struct UtteranceReceiptInput {
    pub session_id: Uuid,
    pub input: UserInputV2,
}

impl UtteranceReceiptInput {
    /// Pure stage-1 transform. Produces the trace anchor + envelope-
    /// version stamp + content hash that downstream stages key off.
    /// `now` is injected so the determinism harness can supply a
    /// fixed clock per fixture.
    ///
    /// This helper is the typed contract for §8.3 Stage 1; the
    /// orchestrator's `process()` head still owns the side-effecting
    /// trace-store write (`persist_trace_scaffold`) so that the
    /// shadow extraction can land without behavior change. Phase
    /// 5b-deep-stage-1 will plumb this output into the orchestrator;
    /// for now it is consumable directly by harnesses.
    pub fn run(
        self,
        now: chrono::DateTime<chrono::Utc>,
        envelope_version: EnvelopeVersion,
    ) -> UtteranceReceiptOutput {
        let utterance_hash = match &self.input {
            UserInputV2::Message { content } => Some(hash_utterance(content)),
            _ => None,
        };
        UtteranceReceiptOutput {
            session_id: self.session_id,
            trace_id: TraceId::new(),
            envelope_version,
            utterance_hash,
            received_at: now,
        }
    }
}

fn hash_utterance(content: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("sha256:{:x}", Sha256::digest(content.as_bytes()))
}

/// Stage 1 output: trace anchor + envelope version stamp + receipt
/// timestamp. The receipt step itself is non-fallible past argument
/// validation; the only meaningful product is the trace anchor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtteranceReceiptOutput {
    pub session_id: Uuid,
    pub trace_id: TraceId,
    pub envelope_version: EnvelopeVersion,
    pub utterance_hash: Option<String>,
    /// Wall-clock timestamp at receipt. Determinism harness fixtures
    /// must mock the clock.
    pub received_at: chrono::DateTime<chrono::Utc>,
}

// ─── Stage 2a — Utterance interpretation (NLP) ────────────────────────────────

/// Stage 2a input: raw text + session context (workspace, scope).
///
/// Maps to `handle_in_pack` body via `IntentService`.
#[derive(Debug, Clone)]
pub struct UtteranceInterpretationInput {
    pub trace_id: TraceId,
    pub utterance: String,
    pub workspace: Option<String>,
    pub scope_summary: Option<String>,
}

/// Stage 2a output: structured triples + verb intent.
///
/// `triples` is the (type, name, scope) projection that stage 2b
/// will resolve. `verb_intent` is the candidate verb FQN(s) — `None`
/// when the utterance is contextual ("what's next?") or
/// non-actionable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtteranceInterpretationOutput {
    pub triples: Vec<EntityTriple>,
    pub verb_intent: Option<Vec<String>>,
    /// Confidence in [0, 1] from the NLP layer. The harness pins this
    /// against the intent fixture.
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityTriple {
    pub kind: String,
    pub name: String,
    pub scope: Option<String>,
}

// ─── Stage 2b — Entity resolution ─────────────────────────────────────────────

/// Stage 2b input: triples from 2a + session-scoped lookup context.
#[derive(Debug, Clone)]
pub struct EntityResolutionInput {
    pub trace_id: TraceId,
    pub triples: Vec<EntityTriple>,
    pub session_id: Uuid,
}

/// Stage 2b output: triples mapped to canonical entity ids.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityResolutionOutput {
    pub resolved: Vec<ResolvedEntity>,
    pub unresolved: Vec<EntityTriple>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedEntity {
    pub triple: EntityTriple,
    pub entity_id: Uuid,
    pub entity_kind: String,
}

// ─── Stage 3 — DAG navigation ─────────────────────────────────────────────────

/// Stage 3 input: resolved entity ids → session DAG cursor.
#[derive(Debug, Clone)]
pub struct DagNavigationInput {
    pub trace_id: TraceId,
    pub session_id: Uuid,
    pub anchor_entity_ids: Vec<Uuid>,
}

/// Stage 3 output: current state nodes for the session's DAG cursor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagNavigationOutput {
    pub current_state_nodes: Vec<StateNodeRef>,
    /// True if rehydration was triggered (writes_since_push > 0).
    pub rehydrated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateNodeRef {
    pub node_id: Uuid,
    pub state_kind: String,
}

// ─── Stage 4 — Verb surface disclosure ────────────────────────────────────────

/// Stage 4 input: state nodes from stage 3 + session actor.
#[derive(Debug, Clone)]
pub struct VerbSurfaceInput {
    pub trace_id: TraceId,
    pub session_id: Uuid,
    pub state_nodes: Vec<StateNodeRef>,
}

/// Stage 4 output: candidate verb set (5–60 verbs typical) plus
/// pruning attribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbSurfaceOutput {
    pub allowed_verbs: Vec<String>,
    pub fingerprint: String,
    /// Number of verbs pruned from the universe by tier/policy. Used
    /// by the determinism harness to spot surface drift.
    pub pruned_count: usize,
}

// ─── Stage 5 — NLP match ──────────────────────────────────────────────────────

/// Stage 5 input: utterance interpretation + verb surface.
#[derive(Debug, Clone)]
pub struct NlpMatchInput {
    pub trace_id: TraceId,
    pub utterance: String,
    pub verb_intent: Option<Vec<String>>,
    pub allowed_verbs: Vec<String>,
}

/// Stage 5 output: selected verb + arg binding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NlpMatchOutput {
    pub selected_verb: String,
    pub arg_bindings: serde_json::Value,
    pub match_score: f64,
}

// ─── Stage 6 — Gate decision ──────────────────────────────────────────────────

/// Stage 6 input: selected verb + bound args + session.
///
/// The gate produces a `GatedVerbEnvelope` (defined in `ob-poc-types`)
/// which already has its own typed contract; we re-export it here as
/// the stage output for symmetry.
#[derive(Debug, Clone)]
pub struct GateDecisionInput {
    pub trace_id: TraceId,
    pub session_id: Uuid,
    pub selected_verb: String,
    pub arg_bindings: serde_json::Value,
}

/// Stage 6 output: a sealed envelope ready for stage-7 runbook
/// compilation. Boxed because `GatedVerbEnvelope` is a wide struct
/// and the harness fixtures pass it around by reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateDecisionOutput {
    pub envelope: ob_poc_types::GatedVerbEnvelope,
}

// ─── Stage 7 — Runbook compilation ────────────────────────────────────────────

/// Stage 7 input: one or more envelopes from stage 6.
#[derive(Debug, Clone)]
pub struct RunbookCompilationInput {
    pub trace_id: TraceId,
    pub envelopes: Vec<ob_poc_types::GatedVerbEnvelope>,
}

/// Stage 7 output: an ordered runbook the dispatch loop can iterate.
///
/// Currently the runbook lives at `crate::runbook::compiler::CompiledRunbook`;
/// we project to its typed shape here so the contract surface is
/// uniform with the other stages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunbookCompilationOutput {
    pub runbook_id: Uuid,
    pub step_count: usize,
}

// ─── Stage 8 — Dispatch loop ──────────────────────────────────────────────────

/// Stage 8 input: a compiled runbook + a transaction scope handle the
/// dispatch loop will pass into each step.
///
/// `scope_id` is the correlation id; the actual `&mut dyn TransactionScope`
/// lives in the dispatch fn's closure scope, not in this input shape
/// (it would require lifetime parameters that defeat the typed-fixture
/// determinism harness).
#[derive(Debug, Clone)]
pub struct DispatchLoopInput {
    pub trace_id: TraceId,
    pub session_id: Uuid,
    pub runbook_id: Uuid,
    pub scope_id: ob_poc_types::TransactionScopeId,
}

/// Stage 8 output: per-step outcomes + cumulative
/// `PendingStateAdvance` totals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchLoopOutput {
    pub steps_executed: usize,
    pub steps_succeeded: usize,
    pub steps_failed: usize,
    pub outbox_drafts_emitted: usize,
}

// ─── Stage 9a — Commit ────────────────────────────────────────────────────────

/// Stage 9a input: the scope handle the dispatch loop owned during
/// stage 8.
#[derive(Debug, Clone)]
pub struct CommitInput {
    pub trace_id: TraceId,
    pub scope_id: ob_poc_types::TransactionScopeId,
    pub dispatch_summary: DispatchLoopOutput,
}

/// Stage 9a output: commit confirmation + the outbox row count that
/// will be drained in stage 9b.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitOutput {
    pub committed_at: chrono::DateTime<chrono::Utc>,
    pub outbox_rows_committed: usize,
    pub commit_duration: Duration,
}

// ─── Stage 9b — Post-commit (drainer) ─────────────────────────────────────────

/// Stage 9b input: the trace anchor and the count of rows the commit
/// emitted. The drainer is a long-running task; this "input" shape is
/// the per-commit handoff signal it processes.
#[derive(Debug, Clone)]
pub struct PostCommitDrainInput {
    pub trace_id: TraceId,
    pub committed_outbox_count: usize,
}

/// Stage 9b output: roll-up of drainer outcomes for this trace.
///
/// Phase 5e foundation drains rows one at a time and per-row outcomes
/// are recorded against `public.outbox`; this stage output is a
/// per-trace summary the harness can pin against fixtures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostCommitDrainOutput {
    pub rows_done: usize,
    pub rows_retryable: usize,
    pub rows_terminal: usize,
}

// ─── Errors ──────────────────────────────────────────────────────────────────

/// Union of every stage's failure mode (§8.3). Each variant is named
/// after the stage it can fire from; the wrapped data is the
/// minimum context the harness or the operator needs to attribute the
/// failure.
#[derive(Debug, thiserror::Error)]
pub enum StageError {
    #[error("stage 2a — utterance interpretation failed: {0}")]
    UtteranceInterpretationError(String),
    #[error("stage 2b — entity resolution failed: {0}")]
    EntityResolutionError(String),
    #[error("stage 3 — DAG navigation failed: {0}")]
    DagNavigationError(String),
    #[error("stage 4 — verb surface is empty for the current state ({reason})")]
    SurfaceEmpty { reason: String },
    #[error("stage 5 — NLP found no matching verb: {0}")]
    NlpNoMatch(String),
    #[error("stage 6 — gate rejected: {reason}")]
    Gated { reason: String },
    #[error("stage 7 — runbook compilation failed: {0}")]
    RunbookCompilationError(String),
    #[error("stage 8 — TOCTOU mismatch on envelope {envelope_id}")]
    Toctou { envelope_id: Uuid },
    #[error("stage 8 — dispatch failed: {0}")]
    DispatchError(String),
    #[error("stage 8 — PendingStateAdvance application failed: {0}")]
    StateAdvanceError(String),
    #[error("stage 9a — commit failed: {0}")]
    CommitError(String),
    #[error("stage 9b — outbox drain failed: {0}")]
    OutboxDrainError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pin the stage-output JSON shapes — the determinism harness
    /// will eventually byte-compare these across runs.
    #[test]
    fn stage_outputs_round_trip_through_serde() {
        let receipt = UtteranceReceiptOutput {
            session_id: Uuid::nil(),
            trace_id: TraceId(Uuid::nil()),
            envelope_version: EnvelopeVersion::CURRENT,
            utterance_hash: Some("sha256:abc".into()),
            received_at: chrono::DateTime::from_timestamp(0, 0).unwrap(),
        };
        let json = serde_json::to_string(&receipt).unwrap();
        let parsed: UtteranceReceiptOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.session_id, receipt.session_id);
        assert_eq!(parsed.utterance_hash, receipt.utterance_hash);
    }

    #[test]
    fn stage_error_variants_format_with_attribution() {
        let err = StageError::SurfaceEmpty {
            reason: "no governance tier matches actor".into(),
        };
        assert!(err.to_string().contains("stage 4"));
        assert!(err.to_string().contains("no governance tier"));
    }

    #[test]
    fn dispatch_loop_output_is_purely_data() {
        let out = DispatchLoopOutput {
            steps_executed: 5,
            steps_succeeded: 4,
            steps_failed: 1,
            outbox_drafts_emitted: 3,
        };
        let json = serde_json::to_value(&out).unwrap();
        assert_eq!(json["steps_executed"], 5);
        assert_eq!(json["outbox_drafts_emitted"], 3);
    }

    /// Demonstrates the typed Stage 1 helper. The determinism harness
    /// will mirror this shape: pin the inputs (clock, envelope
    /// version, session id, utterance), run, byte-compare the output.
    #[test]
    fn stage_1_utterance_receipt_is_deterministic_under_fixed_clock() {
        let session_id = Uuid::nil();
        let now = chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        let input = UtteranceReceiptInput {
            session_id,
            input: UserInputV2::Message {
                content: "show me Allianz".into(),
            },
        };

        let output = input.run(now, EnvelopeVersion::CURRENT);

        // Determinism: pin everything that's not the trace_id (which
        // is generated fresh per receipt by design).
        assert_eq!(output.session_id, session_id);
        assert_eq!(output.received_at, now);
        assert_eq!(output.envelope_version, EnvelopeVersion::CURRENT);
        assert_eq!(
            output.utterance_hash.as_deref(),
            Some("sha256:813d9199a7324b64496adeab7aab45d6df454a7cb9b5d9185bc6760dde92dce1"),
            "utterance hash is a pure function of content — must not drift"
        );
    }

    #[test]
    fn stage_1_non_message_input_has_no_utterance_hash() {
        let input = UtteranceReceiptInput {
            session_id: Uuid::nil(),
            input: UserInputV2::Confirm,
        };
        let output = input.run(
            chrono::DateTime::from_timestamp(0, 0).unwrap(),
            EnvelopeVersion::CURRENT,
        );
        assert!(output.utterance_hash.is_none());
    }
}
