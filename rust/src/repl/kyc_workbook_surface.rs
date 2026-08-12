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
    check_preconditions, phase1_lexicon, AuthorityRef, FoldRegistry, KycError, SubjectId,
    V1FoldImpl,
};
use sem_os_core::principal::Principal as RuntimePrincipal;

use crate::domain_ops::kyc_workbook::{open_workbook, KycWorkbook, RecognitionError, WorkbookError};
use crate::sequencer_tx::PgTransactionScope;

use super::kyc_entity_resolver::{resolve_handles, ResolverError};

/// The KYC fold registry (v1) for the REPL surface — deliberately a
/// standalone construction (not reused from `domain_ops::kyc_stream_ops`,
/// whose `KYC_REGISTRY` is module-private) mirroring the same pattern
/// `tests/kyc_workbook.rs::v1_registry()` already uses.
static SURFACE_REGISTRY: LazyLock<FoldRegistry> = LazyLock::new(|| {
    let mut registry = FoldRegistry::new();
    registry.register(phase1_lexicon().hash, Arc::new(V1FoldImpl));
    registry
});

// ── Command parsing (tier-0 deterministic, no model involvement) ───────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum KycWorkbookCommand {
    Open { subject: SubjectId },
    Stage { text: String },
    Validate,
    Show,
    Commit,
    Run,
    Discard,
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
        "validate" => Ok(KycWorkbookCommand::Validate),
        "show" => Ok(KycWorkbookCommand::Show),
        "commit" => Ok(KycWorkbookCommand::Commit),
        "run" => Ok(KycWorkbookCommand::Run),
        "discard" => Ok(KycWorkbookCommand::Discard),
        other => Err(format!(
            "unknown kyc-workbook command {other:?} (expected one of: open, stage, validate, \
             show, commit, run, discard)"
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
pub(crate) async fn dispatch(
    cmd: KycWorkbookCommand,
    workbooks: &mut HashMap<Uuid, KycWorkbook>,
    session_id: Uuid,
    pool: &sqlx::PgPool,
    principal: &RuntimePrincipal,
    as_of: DateTime<Utc>,
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
                Ok(staged) => Ok(format!(
                    "staged move: {}\ncanonical: {}",
                    staged.legal_move.0, staged.source_text
                )),
                Err(WorkbookError::Recognition(RecognitionError::NotCurrentlyLegal {
                    verb_fqn,
                    legal,
                })) => Ok(format!(
                    "REJECTED — {verb_fqn} is not currently legal; currently-legal moves at the \
                     frontier: {legal:?}"
                )),
                Err(e) => Err(e.into()),
            }
        }
        KycWorkbookCommand::Validate => {
            let workbook = workbooks.get(&session_id).ok_or(SurfaceError::NoWorkbookOpen)?;
            let (control, obligation) = workbook.validate()?;
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
    let live_hash = phase1_lexicon().hash.to_hex();
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
            |control, obligation| {
                let entry = kit.get(staged.event.verb_fqn.as_str()).ok_or_else(|| {
                    ob_poc_kyc_substrate::KycError::UnknownVerb(staged.event.verb_fqn.clone())
                })?;
                check_preconditions(entry, control, obligation, &staged.event)
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
        let cmd = parse_kyc_workbook_command(r#"kyc-workbook.stage (kyc.subject.register)"#);
        assert_eq!(
            cmd,
            Some(Ok(KycWorkbookCommand::Stage { text: "(kyc.subject.register)".to_string() }))
        );
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
