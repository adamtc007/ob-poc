//! T4.5 (`EOP-PLAN-KYCUBO-KIT-001` §T4.5) — wires the T4 `KycWorkbook`
//! (`crate::domain_ops::kyc_workbook`) into the REPL session surface.
//!
//! Command family: `kyc-workbook.{open,stage,validate,show,commit,run,discard}`.
//! Deliberately a SEPARATE, textually-distinct command family from
//! `ReplCommandV2` (which already has its own, unrelated `Run` variant —
//! "execute the runbook", see `types_v2.rs`) — the plan's hard fence
//! ("commit vs run never conflated in code or docs") is enforced by
//! construction here: `KycWorkbookCommand` shares no type, no variant name
//! collision path, and no dispatch table with `ReplCommandV2`.
//!
//! Dual execution semantics (plan v0.6 Q4 ruling):
//! - `Commit` => `KycWorkbook::commit()` verbatim, atomic all-or-nothing.
//!   This module never re-implements or duplicates that method's body — it
//!   calls it exactly once, unmodified.
//! - `Run` => NEW: a loop of single-move commits in staged order, each going
//!   through the SAME real governed append path (`append_in_scope`) that
//!   `KycWorkbook::commit()` itself uses internally for one move at a time.
//!   Stops at the first failure; the already-landed prefix stands (no
//!   rollback of prior moves); remaining (unlanded) moves stay staged.
//!
//! Resolver integration: `Stage` resolves `@handle`/quoted-entity-name
//! occurrences (`kyc_entity_resolver::resolve_handles`) BEFORE calling
//! `KycWorkbook::stage`, which only ever sees UUID-literal text — see that
//! module's doc comment for why this keeps `kyc_workbook.rs` itself
//! completely unmodified.
//!
//! **Zero-inference.** No model/LLM/agent import anywhere in this file —
//! see the `zero_inference_assertion_surface` test below (mirrors
//! `tests/kyc_workbook.rs`'s own allowlist gate, extended to this module and
//! `kyc_entity_resolver.rs` per the tranche's mandate to cover every new
//! module, not just the pre-existing one).

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use chrono::{DateTime, Utc};
use uuid::Uuid;

use ob_poc_kyc_seam::append_in_scope;
use ob_poc_kyc_store::{PgKycEventStore, StoreError};
use ob_poc_kyc_substrate::{
    check_preconditions, enumerate_placement_set, assembly_lexicon, AuthorityRef, FoldRegistry,
    KycError, MoveId, PlacementSet, SubjectId, V1FoldImpl,
};
use sem_os_core::principal::Principal as RuntimePrincipal;

use semantic_decision_contracts::MoveAttemptOutcome;

use crate::domain_ops::kyc_ramp_capture::{record_capture, CaptureRecord, Disposition};
use crate::domain_ops::kyc_workbook::{open_workbook, KycWorkbook, RecognitionError, WorkbookError};
use crate::sequencer_tx::PgTransactionScope;

use super::kyc_entity_resolver::{resolve_handles, ResolverError};

/// The KYC fold registry (v1) for the REPL surface — deliberately a
/// standalone construction (not reused from `domain_ops::kyc_stream_ops`,
/// whose `KYC_REGISTRY` is module-private) mirroring the same pattern
/// `tests/kyc_workbook.rs::v1_registry()` already uses.
static SURFACE_REGISTRY: LazyLock<FoldRegistry> = LazyLock::new(|| {
    let mut registry = FoldRegistry::new();
    registry.register(assembly_lexicon().hash, Arc::new(V1FoldImpl));
    registry
});

// ── Command parsing (tier-0 deterministic, no model involvement) ───────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum KycWorkbookCommand {
    Open { subject: SubjectId },
    Stage { text: String },
    /// T7.2 (`EOP-PLAN-KYCUBO-KIT-T7` §3 Q2a): plain-English proposal —
    /// tier-0 board-restricted retrieval (`super::kyc_ramp`) over the
    /// current frontier, never a call into `stage()`/`commit()`/`run()`
    /// (I-1). Purely advisory: the operator still issues an ordinary
    /// `kyc-workbook.stage <dsl-text>` themselves to actually recognise and
    /// stage a move — this command only reads back what the board looks
    /// like and which candidate the deterministic disposition policy
    /// selected/clarified/abstained on (I-3). Argument values are never
    /// synthesized here (I-4): the proposal names a verb, not filled-in
    /// DSL text.
    Propose { utterance: String },
    Validate,
    Show,
    Commit,
    Run,
    Discard,
}

/// T7 capture-correlation slice 1
/// (`EOP-DD-KYCUBO-KIT-T7_Capture-Correlation-Design_v0.1.md`, RATIFIED
/// 2026-08-17): what `Propose` showed the operator, remembered until their
/// next `Stage` resolves it into a `CaptureRecord`. Session-scoped,
/// in-memory, process-lifetime — same lifecycle rationale as `KycWorkbook`
/// itself (design doc §3.1): a proposal's pending status is exactly as
/// ephemeral as an unstaged workbook edit. `pub(crate)` because
/// `sequencer.rs` names this type in a field declaration; fields stay
/// module-private.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PendingProposal {
    utterance_text: String,
    placement_set_hash: String,
    proposal: String,
    disposition: Disposition,
    /// 1 entry for a `Select` disposition, 2+ for `Clarify`. Never built for
    /// `Abstain` — there is no candidate to later accept or reject.
    candidate_verb_fqns: Vec<String>,
    created_at: DateTime<Utc>,
}

/// `None` when `content` isn't a `kyc-workbook.*` command at all (falls
/// through to ordinary REPL dispatch). `Some(Err(_))` for a recognised verb
/// with malformed/missing arguments — a parse-shaped failure, not a guess.
pub(crate) fn parse_kyc_workbook_command(content: &str) -> Option<Result<KycWorkbookCommand, String>> {
    let rest = content.trim().strip_prefix("kyc-workbook.")?;
    let (verb, arg) = match rest.split_once(char::is_whitespace) {
        Some((v, a)) => (v, a.trim()),
        None => (rest, ""),
    };
    Some(match verb {
        "open" => Uuid::parse_str(arg)
            .map(|u| KycWorkbookCommand::Open { subject: SubjectId(u) })
            .map_err(|e| format!("kyc-workbook.open requires a subject UUID: {e}")),
        "stage" => {
            if arg.is_empty() {
                Err("kyc-workbook.stage requires DSL text after the command".to_string())
            } else {
                Ok(KycWorkbookCommand::Stage { text: arg.to_string() })
            }
        }
        "propose" => {
            if arg.is_empty() {
                Err("kyc-workbook.propose requires plain-English text after the command".to_string())
            } else {
                Ok(KycWorkbookCommand::Propose { utterance: arg.to_string() })
            }
        }
        "validate" => Ok(KycWorkbookCommand::Validate),
        "show" => Ok(KycWorkbookCommand::Show),
        "commit" => Ok(KycWorkbookCommand::Commit),
        "run" => Ok(KycWorkbookCommand::Run),
        "discard" => Ok(KycWorkbookCommand::Discard),
        other => Err(format!(
            "unknown kyc-workbook command {other:?} (expected one of: open, stage, propose, \
             validate, show, commit, run, discard)"
        )),
    })
}

// ── Errors ───────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub(crate) enum SurfaceError {
    #[error("no workbook is open for this session — run kyc-workbook.open <subject-uuid> first")]
    NoWorkbookOpen,
    #[error(transparent)]
    Resolver(#[from] ResolverError),
    #[error(transparent)]
    Workbook(#[from] WorkbookError),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Kyc(#[from] KycError),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
}

/// Outcome of one `run` step.
#[derive(Debug, Clone)]
pub(crate) struct RunStepOutcome {
    pub verb_fqn: String,
    pub landed: bool,
    pub error: Option<String>,
}

/// Full report of a `run` invocation (design: report exactly which moves
/// landed/failed; remaining moves stay staged).
#[derive(Debug, Clone)]
pub(crate) struct RunReport {
    pub steps: Vec<RunStepOutcome>,
    pub remaining_staged: usize,
}

// ── Dispatch ─────────────────────────────────────────────────────────────

/// Human-readable outcome text for each command — the REPL orchestrator
/// wraps this in `ReplResponseKindV2::Info`.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn dispatch(
    cmd: KycWorkbookCommand,
    workbooks: &mut HashMap<Uuid, KycWorkbook>,
    // T7 capture-correlation slice 1: bridges a `Propose` call to whatever
    // `Stage` follows it, so `record_capture` can be called with a real
    // (never guessed) `user_action`. See `PendingProposal`'s doc comment.
    pending_proposals: &mut HashMap<Uuid, PendingProposal>,
    session_id: Uuid,
    pool: &sqlx::PgPool,
    principal: &RuntimePrincipal,
    as_of: DateTime<Utc>,
    // T7.2: precomputed utterance embedding for `Propose`, or `None` when
    // the caller has no embedder configured (falls back to unscored board
    // enumeration — every candidate ties at 0.0, so disposition abstains).
    // Computed by the caller, not this module — see the module doc: no
    // model/embedder import belongs in this zero-inference-import-gated
    // file (I-5); `super::kyc_ramp` is the only thing here that knows how
    // to turn a `&[f32]` into a ranked board.
    utterance_embedding: Option<&[f32]>,
) -> Result<String, SurfaceError> {
    match cmd {
        KycWorkbookCommand::Open { subject } => {
            let mut conn = pool.acquire().await?;
            let workbook = open_workbook(&mut conn, subject).await?;
            let committed_count = workbook.committed.len();
            workbooks.insert(session_id, workbook);
            Ok(format!(
                "opened workbook for subject {} — {committed_count} committed event(s) loaded",
                subject.0
            ))
        }
        KycWorkbookCommand::Stage { text } => {
            let workbook = workbooks.get_mut(&session_id).ok_or(SurfaceError::NoWorkbookOpen)?;
            let mut conn = pool.acquire().await?;
            let resolved_text = resolve_handles(&mut conn, &text).await?;
            drop(conn);

            let authority = AuthorityRef(format!("repl.kyc-workbook.stage/{}", principal.actor_id));
            match workbook.stage(&resolved_text, principal, authority, as_of) {
                Ok(staged) => {
                    let response = format!(
                        "staged move: {}\ncanonical: {}",
                        staged.legal_move.0, staged.source_text
                    );
                    if let Some(pending) = pending_proposals.remove(&session_id) {
                        let staged_verb_fqn = staged.event.verb_fqn.as_str();
                        // Applied: the operator staged exactly what was
                        // proposed. Corrected: they staged a different,
                        // still-legal move instead — a real signal, distinct
                        // from an outright rejection (T7 §7 Amendment
                        // 2026-08-17).
                        let user_action = if pending
                            .candidate_verb_fqns
                            .iter()
                            .any(|c| c == staged_verb_fqn)
                        {
                            MoveAttemptOutcome::Applied
                        } else {
                            MoveAttemptOutcome::Corrected
                        };
                        record_pending_capture(
                            pool,
                            pending,
                            user_action,
                            Some(staged.event.id.0),
                        )
                        .await;
                    }
                    Ok(response)
                }
                Err(WorkbookError::Recognition(RecognitionError::NotCurrentlyLegal {
                    verb_fqn,
                    legal,
                })) => {
                    let response = format!(
                        "REJECTED — {verb_fqn} is not currently legal; currently-legal moves at \
                         the frontier: {legal:?}"
                    );
                    if let Some(pending) = pending_proposals.remove(&session_id) {
                        // The board itself refused this as no-longer-legal —
                        // distinct from the operator declining an accepted
                        // proposal (`RejectedByUser`, not yet a produced
                        // outcome from any call site here).
                        record_pending_capture(
                            pool,
                            pending,
                            MoveAttemptOutcome::CompilerRefused,
                            None,
                        )
                        .await;
                    }
                    Ok(response)
                }
                // Any other error (parse failure, kit drift, DB error): a
                // pending proposal, if any, is left untouched — nothing
                // conclusively happened yet (T7 capture-correlation design
                // §3.4 / Q2: "record nothing on silent abandonment", applied
                // the same way to a failed-but-retriable attempt).
                Err(e) => Err(e.into()),
            }
        }
        KycWorkbookCommand::Propose { utterance } => {
            let workbook = workbooks.get(&session_id).ok_or(SurfaceError::NoWorkbookOpen)?;
            let (control, obligation, type_registry) = workbook.validate()?;
            let board = enumerate_placement_set(
                workbook.subject,
                &control,
                &obligation,
                &type_registry,
                &workbook.kit,
            );

            let ranked = match utterance_embedding {
                Some(embedding) => {
                    super::kyc_ramp::rank_placement_set(pool, embedding, &board).await?
                }
                None => super::kyc_ramp::rank_board_with_scores(&board, &HashMap::new()),
            };
            let outcome = super::kyc_ramp::decide_disposition(
                &ranked,
                super::kyc_ramp::DEFAULT_SELECT_THRESHOLD,
                super::kyc_ramp::DEFAULT_CLARIFY_MARGIN,
            );
            let proposal_text = render_proposal(&utterance, &board, &outcome);

            // Q1(a) binary v0: only Select/Clarify are ever reachable
            // dispositions for capture — Abstain has no candidate to later
            // accept or reject, so no PendingProposal is stored for it.
            // Overwriting a prior unresolved entry without recording it as
            // Rejected is the explicit, ratified slice-1 scope boundary
            // (Q3) — supersede handling is a fast-follow.
            let candidate_verb_fqns: Vec<String> = match &outcome {
                super::kyc_ramp::DispositionOutcome::Select(move_id) => {
                    vec![verb_fqn_for_move(&board, move_id).to_string()]
                }
                super::kyc_ramp::DispositionOutcome::Clarify(candidates) => candidates
                    .iter()
                    .map(|m| verb_fqn_for_move(&board, m).to_string())
                    .collect(),
                super::kyc_ramp::DispositionOutcome::Abstain => Vec::new(),
            };
            let disposition = match &outcome {
                super::kyc_ramp::DispositionOutcome::Select(_) => Some(Disposition::Select),
                super::kyc_ramp::DispositionOutcome::Clarify(_) => Some(Disposition::Clarify),
                super::kyc_ramp::DispositionOutcome::Abstain => None,
            };
            if let Some(disposition) = disposition {
                pending_proposals.insert(
                    session_id,
                    PendingProposal {
                        utterance_text: utterance.clone(),
                        placement_set_hash: board.board_hash.to_hex(),
                        proposal: proposal_text.clone(),
                        disposition,
                        candidate_verb_fqns,
                        created_at: as_of,
                    },
                );
            }
            Ok(proposal_text)
        }
        KycWorkbookCommand::Validate => {
            let workbook = workbooks.get(&session_id).ok_or(SurfaceError::NoWorkbookOpen)?;
            let (control, obligation, _type_registry) = workbook.validate()?;
            Ok(render_preview(&control, &obligation, workbook))
        }
        KycWorkbookCommand::Show => {
            let workbook = workbooks.get(&session_id).ok_or(SurfaceError::NoWorkbookOpen)?;
            let mut msg = format!(
                "subject {} — kit {} — {} committed, {} staged\n",
                workbook.subject.0,
                workbook.kit_hash,
                workbook.committed.len(),
                workbook.staged.len()
            );
            for (i, staged) in workbook.staged.iter().enumerate() {
                msg.push_str(&format!("  [{i}] {}\n", staged.source_text));
            }
            Ok(msg)
        }
        KycWorkbookCommand::Commit => {
            let workbook = workbooks.remove(&session_id).ok_or(SurfaceError::NoWorkbookOpen)?;
            let mut scope = PgTransactionScope::begin(pool).await?;
            match workbook.commit(&mut scope, &SURFACE_REGISTRY).await {
                Ok(outcomes) => {
                    scope.commit().await?;
                    Ok(format!("committed {} move(s) atomically", outcomes.len()))
                }
                Err(e) => {
                    scope.rollback().await?;
                    Err(SurfaceError::Workbook(e))
                }
            }
        }
        KycWorkbookCommand::Run => {
            let workbook = workbooks.get_mut(&session_id).ok_or(SurfaceError::NoWorkbookOpen)?;
            subject_kit_hash_check(workbook)?;
            let report = run_sequential(workbook, pool, &SURFACE_REGISTRY).await?;
            Ok(render_run_report(&report))
        }
        KycWorkbookCommand::Discard => {
            let removed = workbooks.remove(&session_id).is_some();
            if removed {
                Ok("workbook discarded".to_string())
            } else {
                Err(SurfaceError::NoWorkbookOpen)
            }
        }
    }
}

/// KIT-10 drift check, mirrored from `KycWorkbook::commit()` — `run()` is a
/// loop of single-move commits, each of which must uphold the same
/// never-silently-substitute guarantee `commit()` gives for the whole chain.
fn subject_kit_hash_check(workbook: &KycWorkbook) -> Result<(), SurfaceError> {
    let live_hash = assembly_lexicon().hash.to_hex();
    if live_hash != workbook.kit_hash {
        return Err(SurfaceError::Workbook(WorkbookError::KitDrift {
            pinned: workbook.kit_hash.clone(),
            live: live_hash,
        }));
    }
    Ok(())
}

/// `run`: sequential per-verb execution of `workbook.staged`, in order, each
/// through `append_in_scope` (the SAME governed chokepoint `commit()` uses)
/// in its OWN transaction — never `commit()`'s single whole-chain
/// transaction. Stops at the first failure; the landed prefix is left
/// committed (no rollback of prior moves); the failed move and everything
/// after it stay staged on `workbook` for the caller to inspect/retry.
async fn run_sequential(
    workbook: &mut KycWorkbook,
    pool: &sqlx::PgPool,
    registry: &FoldRegistry,
) -> Result<RunReport, SurfaceError> {
    let kit = workbook.kit.clone();
    let mut steps = Vec::new();
    let mut landed_count = 0usize;

    for staged in workbook.staged.iter() {
        let mut scope = PgTransactionScope::begin(pool).await?;
        let verb_fqn = staged.event.verb_fqn.as_str().to_string();

        let result = append_in_scope(
            &mut scope,
            registry,
            &staged.event,
            &staged.source_text,
            |control, obligation, type_registry| {
                let entry = kit.get(staged.event.verb_fqn.as_str()).ok_or_else(|| {
                    ob_poc_kyc_substrate::KycError::UnknownVerb(staged.event.verb_fqn.clone())
                })?;
                check_preconditions(entry, control, obligation, type_registry, &staged.event)
            },
        )
        .await;

        match result {
            Ok(_outcome) => {
                scope.commit().await?;
                steps.push(RunStepOutcome { verb_fqn, landed: true, error: None });
                landed_count += 1;
            }
            Err(e) => {
                scope.rollback().await?;
                steps.push(RunStepOutcome { verb_fqn, landed: false, error: Some(e.to_string()) });
                break;
            }
        }
    }

    // The landed prefix is now committed to the database — remove it from
    // `staged` so the workbook reflects reality; the failed move (if any)
    // and every move after it remain staged.
    workbook.staged.drain(0..landed_count);

    // Re-sync `committed` so subsequent `validate()`/`show` on this
    // in-memory workbook reflect the moves `run` actually landed.
    if landed_count > 0 {
        let mut conn = pool.acquire().await?;
        workbook.committed = PgKycEventStore::load_events(&mut conn, workbook.subject).await?;
    }

    let remaining_staged = workbook.staged.len();
    Ok(RunReport { steps, remaining_staged })
}

fn render_preview(
    control: &ob_poc_kyc_substrate::ControlState,
    obligation: &ob_poc_kyc_substrate::ObligationState,
    workbook: &KycWorkbook,
) -> String {
    let subject_state = obligation.derive_subject_state(workbook.subject);
    format!(
        "consequence preview — {} edge(s), structure_class={:?}, registered={}, \
         {} obligation subject(s), subject overall state={:?}",
        control.edges.len(),
        control.structure_class,
        control.registered,
        obligation.subjects.len(),
        subject_state
    )
}

/// Looks up a `MoveId`'s verb fqn on `board`. Shared by `render_proposal`
/// (rendering text for the operator) and the `Propose` dispatch arm
/// (building a `PendingProposal`'s `candidate_verb_fqns`) so both read the
/// same board lookup rather than duplicating it.
fn verb_fqn_for_move<'a>(board: &'a PlacementSet, move_id: &MoveId) -> &'a str {
    board
        .moves
        .iter()
        .find(|m| &m.move_id == move_id)
        .map(|m| m.verb_fqn.as_str())
        .unwrap_or("?")
}

/// Renders a `Propose` disposition into plain text. Never fills in argument
/// values (I-4) — only names the verb(s) the disposition landed on; the
/// operator composes and issues the actual `kyc-workbook.stage <dsl-text>`.
fn render_proposal(
    utterance: &str,
    board: &PlacementSet,
    outcome: &super::kyc_ramp::DispositionOutcome,
) -> String {
    match outcome {
        super::kyc_ramp::DispositionOutcome::Select(move_id) => format!(
            "proposal for {utterance:?}: {} — review, then accept with \
             kyc-workbook.stage (<verb-with-args>), or type a different stage yourself",
            verb_fqn_for_move(board, move_id)
        ),
        super::kyc_ramp::DispositionOutcome::Clarify(candidates) => {
            let options: Vec<&str> =
                candidates.iter().map(|m| verb_fqn_for_move(board, m)).collect();
            format!(
                "ambiguous — {utterance:?} could mean any of: {options:?}; stage one of these \
                 directly to disambiguate"
            )
        }
        super::kyc_ramp::DispositionOutcome::Abstain => {
            let legal: Vec<&str> = board.moves.iter().map(|m| m.verb_fqn.as_str()).collect();
            format!(
                "no confident match for {utterance:?} — currently legal moves at the frontier: \
                 {legal:?}"
            )
        }
    }
}

/// Best-effort capture write for a resolved `PendingProposal`: by the time
/// this runs, the real KYC stage attempt has already succeeded or already
/// fully resolved to a well-formed rejection — a telemetry write failure
/// must never surface to the operator as if their stage failed, so this
/// never propagates an error, only logs one.
async fn record_pending_capture(
    pool: &sqlx::PgPool,
    pending: PendingProposal,
    user_action: MoveAttemptOutcome,
    staged_move_id: Option<Uuid>,
) {
    let record = CaptureRecord {
        utterance_text: pending.utterance_text,
        placement_set_hash: pending.placement_set_hash,
        proposal: pending.proposal,
        disposition: pending.disposition,
        user_action,
        staged_move_id,
    };
    let mut conn = match pool.acquire().await {
        Ok(conn) => conn,
        Err(e) => {
            tracing::warn!("kyc-workbook: capture write failed to acquire a connection: {e}");
            return;
        }
    };
    if let Err(e) = record_capture(&mut conn, &record).await {
        tracing::warn!("kyc-workbook: capture write failed: {e}");
    }
}

fn render_run_report(report: &RunReport) -> String {
    let mut msg = String::new();
    for (i, step) in report.steps.iter().enumerate() {
        if step.landed {
            msg.push_str(&format!("  [{i}] LANDED {}\n", step.verb_fqn));
        } else {
            msg.push_str(&format!(
                "  [{i}] FAILED {} — {}\n",
                step.verb_fqn,
                step.error.as_deref().unwrap_or("unknown error")
            ));
        }
    }
    msg.push_str(&format!("{} move(s) remain staged", report.remaining_staged));
    msg
}

// ── Workbook-never-bypassed structural guarantee ────────────────────────
//
// The plan requires: "only write routes are commit/run" — a real,
// enforceable check, not a comment. `append_in_scope` (the sole governed
// append chokepoint — see `ob_poc_kyc_seam`'s module doc) must appear in
// this file ONLY inside `run_sequential`, and `KycWorkbook::commit` must be
// called ONLY inside `dispatch`'s `Commit` arm. See
// `structural_only_commit_and_run_are_write_routes` below, which greps this
// file's own source for both call sites and asserts their positions.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_open_with_uuid() {
        let id = Uuid::new_v4();
        let cmd = parse_kyc_workbook_command(&format!("kyc-workbook.open {id}"));
        assert_eq!(cmd, Some(Ok(KycWorkbookCommand::Open { subject: SubjectId(id) })));
    }

    #[test]
    fn rejects_open_without_uuid() {
        let cmd = parse_kyc_workbook_command("kyc-workbook.open not-a-uuid");
        assert!(matches!(cmd, Some(Err(_))));
    }

    #[test]
    fn non_workbook_input_falls_through() {
        assert_eq!(parse_kyc_workbook_command("view.universe"), None);
    }

    #[test]
    fn parses_stage_with_dsl_text() {
        let cmd = parse_kyc_workbook_command(r#"kyc-workbook.stage (kyc_ubo.assert.subject.register)"#);
        assert_eq!(
            cmd,
            Some(Ok(KycWorkbookCommand::Stage { text: "(kyc_ubo.assert.subject.register)".to_string() }))
        );
    }

    #[test]
    fn parses_propose_with_utterance() {
        let cmd = parse_kyc_workbook_command("kyc-workbook.propose register this subject");
        assert_eq!(
            cmd,
            Some(Ok(KycWorkbookCommand::Propose {
                utterance: "register this subject".to_string()
            }))
        );
    }

    #[test]
    fn rejects_propose_without_utterance() {
        let cmd = parse_kyc_workbook_command("kyc-workbook.propose");
        assert!(matches!(cmd, Some(Err(_))));
    }

    #[test]
    fn parses_bare_verbs() {
        for (input, expected) in [
            ("kyc-workbook.validate", KycWorkbookCommand::Validate),
            ("kyc-workbook.show", KycWorkbookCommand::Show),
            ("kyc-workbook.commit", KycWorkbookCommand::Commit),
            ("kyc-workbook.run", KycWorkbookCommand::Run),
            ("kyc-workbook.discard", KycWorkbookCommand::Discard),
        ] {
            assert_eq!(parse_kyc_workbook_command(input), Some(Ok(expected)));
        }
    }

    /// Structural: greps this module's own source for `append_in_scope(` and
    /// `.commit(&mut scope` call sites and asserts they are confined to
    /// `run_sequential` and `dispatch`'s `Commit` arm respectively — the
    /// plan's "never a bespoke append mechanism" / "only write routes are
    /// commit/run" requirement, enforced mechanically rather than by
    /// comment.
    #[test]
    fn structural_only_commit_and_run_are_write_routes() {
        // Scan only the non-test portion of this file's own source — the
        // test module below (this very function) necessarily mentions both
        // strings in prose/assertions, which would otherwise self-trigger.
        let full_source = include_str!("kyc_workbook_surface.rs");
        let source = full_source
            .split_once("#[cfg(test)]")
            .map(|(before, _)| before)
            .unwrap_or(full_source);

        let mut in_run_sequential = false;
        let mut in_commit_arm = false;
        let mut append_in_scope_sites = 0usize;
        let mut workbook_commit_sites = 0usize;

        for line in source.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue; // doc/line comments never count as call sites
            }
            if line.contains("async fn run_sequential") {
                in_run_sequential = true;
            }
            if line.contains("KycWorkbookCommand::Commit => {") {
                in_commit_arm = true;
            }
            if line.contains("append_in_scope(") {
                assert!(
                    in_run_sequential,
                    "append_in_scope( call site found outside run_sequential: {line:?}"
                );
                append_in_scope_sites += 1;
            }
            if line.contains("workbook.commit(&mut scope") {
                assert!(
                    in_commit_arm,
                    "KycWorkbook::commit( call site found outside the Commit dispatch arm: {line:?}"
                );
                workbook_commit_sites += 1;
            }
        }

        assert_eq!(append_in_scope_sites, 1, "expected exactly one append_in_scope( call site (in run_sequential)");
        assert_eq!(workbook_commit_sites, 1, "expected exactly one workbook.commit( call site (in the Commit arm)");
    }

    /// Import allowlist over this module (mirrors `tests/kyc_workbook.rs`'s
    /// `zero_inference_assertion`, extended here per the tranche mandate to
    /// cover every new module).
    #[test]
    fn zero_inference_assertion_surface() {
        const ALLOWLIST: &[&str] = &[
            "std",
            "chrono",
            "sqlx",
            "uuid",
            "thiserror",
            "dsl_runtime",
            "ob_poc_kyc_seam",
            "ob_poc_kyc_store",
            "ob_poc_kyc_substrate",
            "sem_os_core",
            // Pure, inference-free vocabulary crate (hex/serde/sha2/thiserror
            // only, no model/embedder) — T7 §8 gameboard-vocabulary adoption.
            "semantic_decision_contracts",
            "crate",
            "super",
        ];
        assert_no_inference_imports(include_str!("kyc_workbook_surface.rs"), ALLOWLIST);
    }

    #[test]
    fn zero_inference_assertion_resolver() {
        const ALLOWLIST: &[&str] = &["regex", "sqlx", "uuid", "thiserror", "std", "super"];
        assert_no_inference_imports(include_str!("kyc_entity_resolver.rs"), ALLOWLIST);
    }

    /// T7.2 gate test (plan §2): scripted utterance → proposal → user
    /// accepts by staging the proposed verb → ordinary `stage()` → the
    /// resulting preview matches real T4 semantics. The "utterance" here is
    /// simulated by feeding `Propose` a real stored embedding for
    /// `kyc_ubo.assert.subject.place` (fetched from the live
    /// `verb_pattern_embeddings` table) rather than tokenizing English
    /// text — this module has no embedder of its own (I-5); production
    /// wiring (`sequencer.rs::handle_kyc_workbook_command`) is what turns
    /// real utterance text into this same shape of vector. What this test
    /// proves is everything downstream of that: `Propose` names the right
    /// verb, and staging the operator's own resulting DSL text lands
    /// through the SAME `KycWorkbook::stage()`/`validate()` path T4 always
    /// used — the ramp never bypasses or duplicates it (I-1).
    ///
    /// Rewritten 2026-09-07 (P3, journey-pack/frontier tranche) from the
    /// original `register`-verb fixture: `register` retired
    /// (EOP-VS-UBO-GAME-001 T2, merged into `place`) and has zero embedding
    /// rows left to fetch — this test's own `RowNotFound` was one of the
    /// tranche's 5 carried reds. `place` still flips `control.registered`
    /// (`fold/control.rs`'s `place` arm — §3.2, absorbed `register`'s
    /// axis), so the preview assertion below is unchanged in substance.
    ///
    /// Disposition changed from `Select` to `Clarify` in the same rewrite
    /// (P2 of this tranche — audit item 3): `place` candidates are now
    /// type-level (21 on an empty board, `EOP-VS-UBO-GAME-001` §3.4 R9),
    /// all sharing one verb_fqn and therefore one embedding score
    /// (`kyc_ramp::rank_board_with_scores` scores by verb_fqn) — the ramp
    /// can no longer call a bare "place"-shaped utterance unambiguous, and
    /// correctly should not: the word "place" alone never says WHICH type.
    /// This is the honest post-fix behavior, not a workaround. The operator
    /// still resolves it in one `Stage` with a fully-specified DSL text
    /// (I-4), same as before.
    #[cfg(feature = "database")]
    #[tokio::test]
    async fn propose_then_stage_matches_frontier_semantics() {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql:///data_designer".to_string());
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&database_url)
            .await
            .expect("connect to test DB");

        // A real stored embedding for the verb we expect the proposal to
        // land on — stands in for "an utterance that clearly means this".
        let (embedding,): (pgvector::Vector,) = sqlx::query_as(
            r#"SELECT embedding FROM "ob-poc".verb_pattern_embeddings
               WHERE verb_name = 'kyc_ubo.assert.subject.place' AND embedding IS NOT NULL LIMIT 1"#,
        )
        .fetch_one(&pool)
        .await
        .expect("kyc_ubo.assert.subject.place must have at least one populated embedding");

        // Guard against a stale row from a prior interrupted/failed run of
        // this test (this test's own capture-row assertion below does a
        // `fetch_one`, which is ambiguous if more than one row matches).
        sqlx::query(
            r#"DELETE FROM "ob-poc".kyc_ramp_capture WHERE utterance_text = 'place this entity on the board'"#,
        )
        .execute(&pool)
        .await
        .expect("pre-test cleanup of any stale capture row");

        let subject = SubjectId(Uuid::new_v4());
        let session_id = Uuid::new_v4();
        let mut workbooks: HashMap<Uuid, KycWorkbook> = HashMap::new();
        let mut pending_proposals: HashMap<Uuid, PendingProposal> = HashMap::new();
        let principal = RuntimePrincipal {
            actor_id: "test-actor".to_string(),
            roles: vec!["viewer".to_string()],
            claims: std::collections::HashMap::new(),
            tenancy: None,
        };
        let as_of = Utc::now();

        dispatch(
            KycWorkbookCommand::Open { subject },
            &mut workbooks,
            &mut pending_proposals,
            session_id,
            &pool,
            &principal,
            as_of,
            None,
        )
        .await
        .expect("open");

        let proposal = dispatch(
            KycWorkbookCommand::Propose { utterance: "place this entity on the board".to_string() },
            &mut workbooks,
            &mut pending_proposals,
            session_id,
            &pool,
            &principal,
            as_of,
            Some(embedding.as_slice()),
        )
        .await
        .expect("propose");
        assert!(
            proposal.starts_with("ambiguous —") && proposal.contains("kyc_ubo.assert.subject.place"),
            "a bare place-shaped utterance must CLARIFY on kyc_ubo.assert.subject.place — the \
             type-level candidates (21 on an empty board) all share this verb_fqn and score \
             identically, so the ramp cannot call it unambiguous; the word \"place\" alone never \
             says WHICH type (2026-09-07, audit item 3): {proposal:?}"
        );
        assert!(pending_proposals.contains_key(&session_id), "Propose(Clarify) must stash a pending proposal too");

        // The operator reads the proposal and stages the real DSL text
        // themselves (I-4: args are never synthesized by the ramp) —
        // ordinary `Stage`, wholly unmodified by T7.
        dispatch(
            KycWorkbookCommand::Stage {
                text: r#"(kyc_ubo.assert.subject.place :entity-type "private_limited_company")"#
                    .to_string(),
            },
            &mut workbooks,
            &mut pending_proposals,
            session_id,
            &pool,
            &principal,
            as_of,
            None,
        )
        .await
        .expect("stage the proposed verb");
        assert!(
            !pending_proposals.contains_key(&session_id),
            "Stage must resolve and clear the pending proposal"
        );

        let preview = dispatch(
            KycWorkbookCommand::Validate,
            &mut workbooks,
            &mut pending_proposals,
            session_id,
            &pool,
            &principal,
            as_of,
            None,
        )
        .await
        .expect("validate");
        assert!(
            preview.contains("registered=true"),
            "T4 semantics: staging kyc_ubo.assert.subject.place must flip registered=true in the preview: {preview:?}"
        );

        // T7 capture-correlation slice 1: Stage after a Clarify proposal
        // whose candidates all name the staged verb must still write one
        // "applied" (not "corrected") resolved capture row — the operator
        // supplied the missing type argument, they did not choose a
        // DIFFERENT verb than what was proposed.
        let (disposition, user_action, staged_move_id): (String, String, Option<Uuid>) =
            sqlx::query_as(
                r#"SELECT disposition, user_action, staged_move_id FROM "ob-poc".kyc_ramp_capture
                   WHERE utterance_text = 'place this entity on the board'"#,
            )
            .fetch_one(&pool)
            .await
            .expect("capture row written for this test's utterance");
        assert_eq!(disposition, "clarify");
        assert_eq!(user_action, "applied");
        assert!(staged_move_id.is_some());

        sqlx::query(
            r#"DELETE FROM "ob-poc".kyc_ramp_capture WHERE utterance_text = 'place this entity on the board'"#,
        )
        .execute(&pool)
        .await
        .expect("cleanup capture row");
    }

    /// Shared setup for the correlation tests below: a connected pool and a
    /// freshly opened, empty workbook. Not used by
    /// `propose_then_stage_matches_frontier_semantics` above (predates this
    /// helper; left as-is to avoid an unrelated diff).
    #[cfg(feature = "database")]
    async fn open_test_workbook() -> (
        sqlx::PgPool,
        SubjectId,
        Uuid,
        HashMap<Uuid, KycWorkbook>,
        HashMap<Uuid, PendingProposal>,
        RuntimePrincipal,
        DateTime<Utc>,
    ) {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql:///data_designer".to_string());
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&database_url)
            .await
            .expect("connect to test DB");

        let subject = SubjectId(Uuid::new_v4());
        let session_id = Uuid::new_v4();
        let mut workbooks: HashMap<Uuid, KycWorkbook> = HashMap::new();
        let pending_proposals: HashMap<Uuid, PendingProposal> = HashMap::new();
        let principal = RuntimePrincipal {
            actor_id: "test-actor".to_string(),
            roles: vec!["viewer".to_string()],
            claims: std::collections::HashMap::new(),
            tenancy: None,
        };
        let as_of = Utc::now();

        let mut opening_pending = HashMap::new();
        dispatch(
            KycWorkbookCommand::Open { subject },
            &mut workbooks,
            &mut opening_pending,
            session_id,
            &pool,
            &principal,
            as_of,
            None,
        )
        .await
        .expect("open");

        (pool, subject, session_id, workbooks, pending_proposals, principal, as_of)
    }

    async fn cleanup_capture_rows(pool: &sqlx::PgPool, utterance_text: &str) {
        sqlx::query(r#"DELETE FROM "ob-poc".kyc_ramp_capture WHERE utterance_text = $1"#)
            .bind(utterance_text)
            .execute(pool)
            .await
            .expect("cleanup capture rows");
    }

    /// `Stage` after a `PendingProposal` whose `candidate_verb_fqns` do NOT
    /// include the staged verb must record `user_action = Corrected` — the
    /// staged move itself lands successfully, but it wasn't what was
    /// proposed (T7 §7 Amendment 2026-08-17: distinct from an outright
    /// `RejectedByUser`/`CompilerRefused`).
    #[cfg(feature = "database")]
    #[tokio::test]
    async fn stage_mismatched_verb_records_corrected_capture() {
        let (pool, _subject, session_id, mut workbooks, mut pending_proposals, principal, as_of) =
            open_test_workbook().await;
        cleanup_capture_rows(&pool, "mismatch-utterance").await;

        pending_proposals.insert(
            session_id,
            PendingProposal {
                utterance_text: "mismatch-utterance".to_string(),
                placement_set_hash: "deadbeef".to_string(),
                proposal: "proposal for \"mismatch-utterance\": some.other.verb".to_string(),
                disposition: Disposition::Select,
                candidate_verb_fqns: vec!["some.other.verb".to_string()],
                created_at: as_of,
            },
        );

        dispatch(
            KycWorkbookCommand::Stage {
                text: r#"(kyc_ubo.assert.subject.place :entity-type "private_limited_company")"#
                    .to_string(),
            },
            &mut workbooks,
            &mut pending_proposals,
            session_id,
            &pool,
            &principal,
            as_of,
            None,
        )
        .await
        .expect("stage a legal but non-matching verb");
        assert!(!pending_proposals.contains_key(&session_id), "resolved either way — cleared");

        let (disposition, user_action, staged_move_id): (String, String, Option<Uuid>) =
            sqlx::query_as(
                r#"SELECT disposition, user_action, staged_move_id FROM "ob-poc".kyc_ramp_capture
                   WHERE utterance_text = 'mismatch-utterance'"#,
            )
            .fetch_one(&pool)
            .await
            .expect("capture row written");
        assert_eq!(disposition, "select");
        assert_eq!(user_action, "corrected");
        assert!(staged_move_id.is_some(), "the stage itself succeeded — a real move landed");

        cleanup_capture_rows(&pool, "mismatch-utterance").await;
    }

    /// `Stage` that resolves to `NotCurrentlyLegal` (well-formed DSL, not
    /// admitted at this position) with a pending proposal present must
    /// record `CompilerRefused` with `staged_move_id = None` — a completed,
    /// well-formed attempt the board itself refused, not silence, and not
    /// the operator declining an accepted proposal (T7 §7 Amendment
    /// 2026-08-17).
    #[cfg(feature = "database")]
    #[tokio::test]
    async fn stage_not_currently_legal_records_compiler_refused_with_no_staged_move_id() {
        let (pool, _subject, session_id, mut workbooks, mut pending_proposals, principal, as_of) =
            open_test_workbook().await;
        cleanup_capture_rows(&pool, "illegal-utterance").await;

        // `kyc_ubo.assert.subject.remove` requires EntityRegistered +
        // MembershipActive (lexicon.rs) — on a freshly opened,
        // never-registered subject with no entity ever placed, it is NOT in
        // the frontier placement set. (EOP-DD-UBO-DISPATCH-001 T4,
        // 2026-08-28: swapped from `structure-class`, retired — same
        // NotCurrentlyLegal refusal shape, any verb with an unmet
        // precondition on a fresh subject demonstrates it equally.)
        pending_proposals.insert(
            session_id,
            PendingProposal {
                utterance_text: "illegal-utterance".to_string(),
                placement_set_hash: "deadbeef".to_string(),
                proposal: "proposal for \"illegal-utterance\": kyc_ubo.assert.subject.remove"
                    .to_string(),
                disposition: Disposition::Select,
                candidate_verb_fqns: vec!["kyc_ubo.assert.subject.remove".to_string()],
                created_at: as_of,
            },
        );

        let response = dispatch(
            KycWorkbookCommand::Stage {
                text: r#"(kyc_ubo.assert.subject.remove :entity-id "00000000-0000-0000-0000-000000000001")"#
                    .to_string(),
            },
            &mut workbooks,
            &mut pending_proposals,
            session_id,
            &pool,
            &principal,
            as_of,
            None,
        )
        .await
        .expect("dispatch returns Ok with a REJECTED message, not an Err");
        assert!(response.starts_with("REJECTED"), "unexpected response: {response:?}");
        assert!(!pending_proposals.contains_key(&session_id));

        let (disposition, user_action, staged_move_id): (String, String, Option<Uuid>) =
            sqlx::query_as(
                r#"SELECT disposition, user_action, staged_move_id FROM "ob-poc".kyc_ramp_capture
                   WHERE utterance_text = 'illegal-utterance'"#,
            )
            .fetch_one(&pool)
            .await
            .expect("capture row written");
        assert_eq!(disposition, "select");
        assert_eq!(user_action, "compiler_refused");
        assert!(staged_move_id.is_none(), "no IntentEvent exists for a NotCurrentlyLegal refusal");

        cleanup_capture_rows(&pool, "illegal-utterance").await;
    }

    /// A `Stage` call that errors outright (malformed DSL text) must leave a
    /// pending proposal completely untouched and record nothing — nothing
    /// conclusively happened yet (Q2). A well-formed follow-up `Stage` then
    /// resolves it normally, proving the earlier failure only deferred
    /// resolution, never dropped it.
    #[cfg(feature = "database")]
    #[tokio::test]
    async fn stage_error_leaves_pending_proposal_untouched_and_records_nothing() {
        let (pool, _subject, session_id, mut workbooks, mut pending_proposals, principal, as_of) =
            open_test_workbook().await;
        cleanup_capture_rows(&pool, "error-utterance").await;

        let original = PendingProposal {
            utterance_text: "error-utterance".to_string(),
            placement_set_hash: "deadbeef".to_string(),
            proposal: "proposal for \"error-utterance\": kyc_ubo.assert.subject.place".to_string(),
            disposition: Disposition::Select,
            candidate_verb_fqns: vec!["kyc_ubo.assert.subject.place".to_string()],
            created_at: as_of,
        };
        pending_proposals.insert(session_id, original.clone());

        let err = dispatch(
            KycWorkbookCommand::Stage { text: "not valid dsl text at all".to_string() },
            &mut workbooks,
            &mut pending_proposals,
            session_id,
            &pool,
            &principal,
            as_of,
            None,
        )
        .await;
        assert!(err.is_err(), "malformed DSL text must be a dispatch Err, not an Ok/REJECTED");
        assert_eq!(
            pending_proposals.get(&session_id),
            Some(&original),
            "an Err stage must leave the pending proposal byte-for-byte untouched"
        );

        let row_exists: Option<(Uuid,)> = sqlx::query_as(
            r#"SELECT id FROM "ob-poc".kyc_ramp_capture WHERE utterance_text = 'error-utterance'"#,
        )
        .fetch_optional(&pool)
        .await
        .expect("query capture rows");
        assert!(row_exists.is_none(), "an Err stage must record nothing");

        // The earlier malformed attempt only deferred resolution — a
        // well-formed follow-up now resolves the SAME pending proposal.
        dispatch(
            KycWorkbookCommand::Stage {
                text: r#"(kyc_ubo.assert.subject.place :entity-type "private_limited_company")"#
                    .to_string(),
            },
            &mut workbooks,
            &mut pending_proposals,
            session_id,
            &pool,
            &principal,
            as_of,
            None,
        )
        .await
        .expect("well-formed follow-up stage");
        assert!(!pending_proposals.contains_key(&session_id));

        let (user_action,): (String,) = sqlx::query_as(
            r#"SELECT user_action FROM "ob-poc".kyc_ramp_capture WHERE utterance_text = 'error-utterance'"#,
        )
        .fetch_one(&pool)
        .await
        .expect("capture row written by the follow-up stage");
        assert_eq!(user_action, "applied");

        cleanup_capture_rows(&pool, "error-utterance").await;
    }

    /// Regression guard (I-1 "ramp is optional"): `Stage` with no prior
    /// `Propose` for this session must behave exactly as it did before
    /// slice 1 — no map entry, no capture row, unchanged response shape.
    #[cfg(feature = "database")]
    #[tokio::test]
    async fn stage_without_pending_proposal_matches_pre_slice1_behavior() {
        let (pool, _subject, session_id, mut workbooks, mut pending_proposals, principal, as_of) =
            open_test_workbook().await;
        assert!(pending_proposals.is_empty());

        let response = dispatch(
            KycWorkbookCommand::Stage {
                text: r#"(kyc_ubo.assert.subject.place :entity-type "private_limited_company")"#
                    .to_string(),
            },
            &mut workbooks,
            &mut pending_proposals,
            session_id,
            &pool,
            &principal,
            as_of,
            None,
        )
        .await
        .expect("stage with no pending proposal");
        assert!(response.starts_with("staged move:"));
        assert!(pending_proposals.is_empty(), "no pending proposal existed to insert or clear");

        let count: (i64,) = sqlx::query_as(
            r#"SELECT count(*) FROM "ob-poc".kyc_ramp_capture WHERE proposal = $1"#,
        )
        .bind(&response)
        .fetch_one(&pool)
        .await
        .expect("query capture rows");
        assert_eq!(count.0, 0, "no capture row should be written when there was nothing to resolve");
    }

    /// `Commit` never resolves a pending proposal — only `Stage` (and,
    /// post-fast-follow, `Discard`) are resolution points. Proposing then
    /// committing without staging must leave the pending-proposal entry
    /// present afterward. Accepted bounded side effect for slice 1: the
    /// entry is left dangling in memory until process restart or a future
    /// `Propose`/`Discard` for the same session — no persistence, no
    /// cross-session leak.
    #[cfg(feature = "database")]
    #[tokio::test]
    async fn commit_never_resolves_pending_proposal() {
        let (pool, _subject, session_id, mut workbooks, mut pending_proposals, principal, as_of) =
            open_test_workbook().await;

        let (embedding,): (pgvector::Vector,) = sqlx::query_as(
            r#"SELECT embedding FROM "ob-poc".verb_pattern_embeddings
               WHERE verb_name = 'kyc_ubo.assert.subject.place' AND embedding IS NOT NULL LIMIT 1"#,
        )
        .fetch_one(&pool)
        .await
        .expect("kyc_ubo.assert.subject.place must have at least one populated embedding");

        dispatch(
            KycWorkbookCommand::Propose { utterance: "commit-guard-utterance".to_string() },
            &mut workbooks,
            &mut pending_proposals,
            session_id,
            &pool,
            &principal,
            as_of,
            Some(embedding.as_slice()),
        )
        .await
        .expect("propose");
        assert!(pending_proposals.contains_key(&session_id));

        dispatch(
            KycWorkbookCommand::Commit,
            &mut workbooks,
            &mut pending_proposals,
            session_id,
            &pool,
            &principal,
            as_of,
            None,
        )
        .await
        .expect("commit with nothing staged");

        assert!(
            pending_proposals.contains_key(&session_id),
            "Commit must never silently resolve a pending proposal — only Stage/Discard may"
        );

        pending_proposals.remove(&session_id);
    }

    fn assert_no_inference_imports(source: &str, allowlist: &[&str]) {
        let mut violations = Vec::new();
        for line in source.lines() {
            let trimmed = line.trim();
            let Some(rest) = trimmed.strip_prefix("use ") else { continue };
            let root = rest
                .split(|c: char| c == ':' || c == ';' || c == '{' || c.is_whitespace())
                .next()
                .unwrap_or("");
            if root.is_empty() {
                continue;
            }
            if !allowlist.contains(&root) {
                violations.push(line.to_string());
            }
        }
        assert!(
            violations.is_empty(),
            "import outside the zero-inference allowlist {allowlist:?}: {violations:#?}"
        );
    }
}
