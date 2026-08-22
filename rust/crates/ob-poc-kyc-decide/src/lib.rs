//! KYC/UBO Evaluation-pack verdicts — `kyc_ubo.decide.subject.approve`, `kyc_ubo.decide.subject.reject`
//! (EOP-DD-KYCUBO-TS.6 §1/§4, landed TS.6 P1/P2).
//!
//! **This crate must never depend on `ob-poc-kyc-seam`**, directly or
//! transitively. `ob-poc-kyc-seam::append_in_scope` is the sole chokepoint
//! for writes to the dsl.kyc fact stream (`"ob-poc".kyc_intent_events`); by
//! having no dependency on it, this crate structurally cannot write a fact
//! — `evaluation_pack_cannot_write_facts` (TS.6 §8) is proven by the
//! crate-dependency graph, not by inspecting what each op happens to call.
//! `scripts/check_kyc_decide_deps.sh` is the CI-enforced proof, mirroring
//! `ob-poc-kyc-substrate`'s existing `check_kyc_substrate_deps.sh` pattern.
//!
//! Both ops write only to `"ob-poc".kyc_decision_records` (migration
//! `20260822_kyc_decision_records.sql`) — never to the fact stream. They
//! read the fact stream (via `ob-poc-kyc-store` + `ob-poc-kyc-substrate`'s
//! pure fold) to compute the K-23 gate and the decision's basis, which is a
//! permitted read, not a write.
//!
//! Renamed from `kyc.person.approve`/`kyc.person.reject` (TS.6 P2). The
//! K-23 "decision is final" finality guard moved here too: the substrate's
//! `Precondition::SubjectNotDecided` and `SubjectOverallState::Approved`/
//! `Rejected` were retired (the pure fold can no longer see a decision that
//! never reaches the fact stream) — these ops query
//! `"ob-poc".kyc_decision_records` directly for a prior terminal decision on
//! the subject before writing a new one.
//!
//! `kyc_ubo.decide.obligation.waiver` (D2.0 §5, moved here 2026-08-22 from
//! `kyc_ubo.assert.obligation.waiver`, which dissolved alongside `creation`/
//! `satisfaction`) lives in this crate too: a human rules that a failing
//! check does not apply, with reason and authority, citing the run — never
//! a fact-stream append.
//!
//! **D2.0 §5 basis correction:** `basis_json` used to cite an obligation-fold
//! snapshot — no longer meaningful once `creation` (the only writer of a new
//! `ObligationTracks` entry) is dissolved. A verdict now cites the
//! `EvaluationRun` it relied on (§4/§6 `decide_cites_a_run`) — computed
//! against `evaluation_lexicon`'s check catalogue (empty today; the
//! catalogue's contents are D2.0 §7 Q2, explicitly out of scope here) and
//! persisted to `"ob-poc".kyc_evaluation_runs` before the decision record
//! that cites it is written.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use sqlx::Row;
use uuid::Uuid;

use dsl_runtime::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};
use sem_os_postgres::ops::SemOsVerbOp;

use ob_poc_kyc_read::PgKycEventReader;
use ob_poc_kyc_substrate::{
    board_state_hash, fold_control, fold_obligations_versioned, fold_type_registry,
    in_scope_check_ids, ApplicabilityCondition, BoardSnapshot, Check, EvaluationRun, FoldRegistry,
    Hash, RunPins, SubjectId, V1FoldImpl,
};

// ── JSON arg extraction (local — `super::helpers` in `ob-poc` is
// crate-private and this crate must not depend on the `ob-poc` binary
// crate, only on the KYC substrate/store tier) ──────────────────────────────

fn json_extract_string_opt(args: &serde_json::Value, arg_name: &str) -> Option<String> {
    args.get(arg_name)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn json_extract_uuid(
    args: &serde_json::Value,
    ctx: &VerbExecutionContext,
    arg_name: &str,
) -> Result<Uuid> {
    args.get(arg_name)
        .and_then(|v| v.as_str())
        .and_then(|s| match s.strip_prefix('@') {
            Some(sym) => ctx.resolve(sym),
            None => Uuid::parse_str(s).ok(),
        })
        .ok_or_else(|| anyhow!("Missing {arg_name} argument"))
}

// ── Fold registry (this crate's own — read-only use, never appends) ────────

/// The v1 fold registry, keyed by `assembly_lexicon()`'s hash — same
/// registration `kyc_stream_ops.rs` uses, duplicated here rather than
/// shared, since sharing would require depending on `ob-poc-kyc-seam` or
/// the `ob-poc` binary crate, either of which reopens the dependency this
/// crate exists to close off.
fn v1_registry() -> FoldRegistry {
    let mut registry = FoldRegistry::new();
    registry.register(
        ob_poc_kyc_substrate::assembly_lexicon().hash,
        std::sync::Arc::new(V1FoldImpl),
    );
    registry
}

/// Load the board (control + type registry + obligation state) for
/// `subject` from the live fact stream — a read, not a write (permitted
/// under TS.6 §2's boundary: "Evaluation reads... its determination").
async fn load_board_state(
    scope: &mut dyn TransactionScope,
    subject: SubjectId,
) -> Result<(
    ob_poc_kyc_substrate::ControlState,
    ob_poc_kyc_substrate::TypeRegistryState,
    ob_poc_kyc_substrate::ObligationState,
)> {
    let events = PgKycEventReader::load_events(scope.executor(), subject)
        .await
        .map_err(|e| anyhow!("decide: load events failed: {e}"))?;
    let refs: Vec<&ob_poc_kyc_substrate::IntentEvent> = events.iter().collect();
    let control = fold_control(&refs);
    let type_registry = fold_type_registry(&refs);
    let obligations = fold_obligations_versioned(&refs, &v1_registry())
        .map_err(|e| anyhow!("decide: obligation fold failed: {e}"))?;
    Ok((control, type_registry, obligations))
}

/// No real checks exist yet — the check catalogue's contents are D2.0 §7
/// Q2, explicitly out of scope for this tranche. This marker fixes the
/// generic `Check` type parameter so the (real) in-scope-computation and
/// run-construction machinery runs today, against an empty catalogue,
/// rather than being left unwired until a catalogue exists.
struct NoChecksYet;
impl Check for NoChecksYet {
    fn check_id(&self) -> &str {
        unreachable!("NoChecksYet is never constructed — it only fixes the Check type parameter")
    }
    fn applicability(&self) -> &[ApplicabilityCondition] {
        &[]
    }
}

/// D2.0 §6 `checks_run_at_any_board_state`: compute and persist a fresh
/// `EvaluationRun` for `subject`, against today's (empty) check catalogue.
/// The founding property holds regardless of catalogue size — a run over
/// ANY board state produces a run, with verdicts (here, trivially none),
/// and no error.
async fn compute_and_persist_run(
    scope: &mut dyn TransactionScope,
    subject: SubjectId,
    control: &ob_poc_kyc_substrate::ControlState,
    type_registry: &ob_poc_kyc_substrate::TypeRegistryState,
    trigger: &str,
) -> Result<EvaluationRun> {
    let board = BoardSnapshot { control, type_registry, determination: None };
    let in_scope = in_scope_check_ids::<NoChecksYet>(&[], &board);
    let now = chrono::Utc::now();
    let pins = RunPins {
        subject_root: subject,
        board_state_hash: board_state_hash(&board),
        evaluation_pack_version_hash: Hash::of_json(
            &serde_json::json!({ "evaluation_lexicon_hash": ob_poc_kyc_substrate::evaluation_lexicon().hash.to_hex() }),
        ),
        valid_time: now,
        knowledge_time: now,
        trigger: trigger.to_string(),
        in_scope_check_ids: in_scope,
    };
    let run = EvaluationRun::new(Uuid::new_v4(), pins, vec![])
        .map_err(|e| anyhow!("decide: run construction refused: {e}"))?;
    persist_run(scope, &run).await?;
    Ok(run)
}

async fn persist_run(scope: &mut dyn TransactionScope, run: &EvaluationRun) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO "ob-poc".kyc_evaluation_runs
           (run_id, subject_root, board_state_hash, evaluation_pack_version_hash,
            valid_time, knowledge_time, trigger, in_scope_check_ids, findings)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#,
    )
    .bind(run.run_id)
    .bind(run.subject_root.0)
    .bind(run.board_state_hash.to_hex())
    .bind(run.evaluation_pack_version_hash.to_hex())
    .bind(run.valid_time)
    .bind(run.knowledge_time)
    .bind(&run.trigger)
    .bind(serde_json::to_value(&run.in_scope_check_ids).unwrap_or_default())
    // `Finding` carries no `Serialize` today — every run's catalogue is
    // empty (D2.0 §7 Q2 out of scope), so findings are always `[]`. Wiring
    // real findings here is future work gated on a real check catalogue.
    .bind(serde_json::json!([]))
    .execute(scope.executor())
    .await
    .map_err(|e| anyhow!("decide: persist run failed: {e}"))?;
    Ok(())
}

/// Build the `basis` JSON recorded on every decision record — cites the
/// `EvaluationRun` the decision relied on (D2.0 §5 correction; replaces the
/// obligation-fold snapshot, which stopped being meaningful once
/// `kyc_ubo.assert.obligation.creation` — the only writer of a new
/// `ObligationTracks` entry — dissolved, D2.0 §5).
fn basis_json(run: &EvaluationRun) -> serde_json::Value {
    serde_json::json!({
        "run_id": run.run_id,
        "board_state_hash": run.board_state_hash.to_hex(),
        "in_scope_check_ids": run.in_scope_check_ids,
    })
}

/// D2.0 §6 `decide_cites_a_run` — FALSIFIABLE: refuses to insert a decision
/// record whose basis carries no `run_id`, rather than relying on a NOT
/// NULL column no code path can ever violate (the defect the prior
/// `decide_verbs_cite_their_basis` gate had — it tested an obligation-fold
/// snapshot that was, in practice, always non-empty).
fn assert_basis_cites_run(basis: &serde_json::Value) -> Result<()> {
    match basis.get("run_id") {
        Some(v) if !v.is_null() => Ok(()),
        _ => Err(anyhow!(
            "decide: basis does not cite a run — refusing to record an uncited verdict \
             (decide_cites_a_run)"
        )),
    }
}

/// K-23 "decision is final" — query `kyc_decision_records` directly rather
/// than the fact-stream fold (which can no longer see decisions at all,
/// TS.6 P2). Refuses if a prior `kyc_ubo.decide.subject.approve`/`kyc_ubo.decide.subject.reject` exists for
/// this subject.
async fn refuse_if_already_decided(
    scope: &mut dyn TransactionScope,
    subject: SubjectId,
) -> Result<()> {
    let existing = sqlx::query(
        r#"SELECT verb_fqn FROM "ob-poc".kyc_decision_records
           WHERE subject_root = $1 AND verb_fqn IN ('kyc_ubo.decide.subject.approve', 'kyc_ubo.decide.subject.reject')
           LIMIT 1"#,
    )
    .bind(subject.0)
    .fetch_optional(scope.executor())
    .await
    .map_err(|e| anyhow!("decide: finality check failed: {e}"))?;

    if let Some(row) = existing {
        let prior_fqn: String = row.get("verb_fqn");
        return Err(anyhow!(
            "subject has already been decided ({prior_fqn}); the decision is final (K-23)"
        ));
    }
    Ok(())
}

async fn insert_decision_record(
    scope: &mut dyn TransactionScope,
    subject: SubjectId,
    verb_fqn: &str,
    decided_by: &str,
    basis: serde_json::Value,
    reason: Option<&str>,
    raw_args: &serde_json::Value,
) -> Result<Uuid> {
    assert_basis_cites_run(&basis)?;
    let row = sqlx::query(
        r#"INSERT INTO "ob-poc".kyc_decision_records
           (subject_root, verb_fqn, decided_by, basis, reason, raw_args)
           VALUES ($1, $2, $3, $4, $5, $6)
           RETURNING id"#,
    )
    .bind(subject.0)
    .bind(verb_fqn)
    .bind(decided_by)
    .bind(&basis)
    .bind(reason)
    .bind(raw_args)
    .fetch_one(scope.executor())
    .await
    .map_err(|e| anyhow!("decide: insert decision record failed: {e}"))?;
    Ok(row.get::<Uuid, _>("id"))
}

// ── kyc_ubo.decide.subject.approve ──────────────────────────────────────────────────────────

pub struct DecideApprove;

#[async_trait]
impl SemOsVerbOp for DecideApprove {
    fn fqn(&self) -> &str {
        "kyc_ubo.decide.subject.approve"
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);

        let (control, type_registry, _obligations) = load_board_state(scope, subject).await?;
        let run = compute_and_persist_run(
            scope,
            subject,
            &control,
            &type_registry,
            "kyc_ubo.decide.subject.approve",
        )
        .await?;

        // K-23 gate: nothing in the run's work list (D2.0 §5 correction —
        // replaces the dissolved obligation-fold `AllTerminal` check).
        // Trivially satisfied today (the check catalogue is empty, D2.0 §7
        // Q2 out of scope) — wired for real the moment a check exists.
        if !run.work_list().is_empty() {
            return Err(anyhow!(
                "kyc_ubo.decide.subject.approve rejected: subject {} has {} failing/unevaluable \
                 finding(s) in run {} — K-23 gate (determination and approval are separate; \
                 approval requires the latest run's work list to be empty)",
                subject.0,
                run.work_list().len(),
                run.run_id,
            ));
        }

        refuse_if_already_decided(scope, subject).await?;

        let basis = basis_json(&run);
        let decided_by = ctx.principal.actor_id.clone();
        let decision_id = insert_decision_record(
            scope,
            subject,
            "kyc_ubo.decide.subject.approve",
            &decided_by,
            basis,
            None,
            args,
        )
        .await?;

        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "decision_id": decision_id }),
        ))
    }
}

// ── kyc_ubo.decide.subject.reject ───────────────────────────────────────────────────────────

pub struct DecideReject;

#[async_trait]
impl SemOsVerbOp for DecideReject {
    fn fqn(&self) -> &str {
        "kyc_ubo.decide.subject.reject"
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);

        refuse_if_already_decided(scope, subject).await?;

        // Rejection is deliberately allowed at ANY stage (early rejection is
        // a real compliance outcome) — the run here is for the basis
        // citation only, never a gate.
        let (control, type_registry, _obligations) = load_board_state(scope, subject).await?;
        let run = compute_and_persist_run(
            scope,
            subject,
            &control,
            &type_registry,
            "kyc_ubo.decide.subject.reject",
        )
        .await?;
        let basis = basis_json(&run);
        let reason = json_extract_string_opt(args, "reason");
        let decided_by = ctx.principal.actor_id.clone();
        let decision_id = insert_decision_record(
            scope,
            subject,
            "kyc_ubo.decide.subject.reject",
            &decided_by,
            basis,
            reason.as_deref(),
            args,
        )
        .await?;

        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "decision_id": decision_id }),
        ))
    }
}

// ── kyc_ubo.decide.obligation.waiver ────────────────────────────────────────

/// D2.0 §5: moved here from `kyc_ubo.assert.obligation.waiver` (dissolved
/// alongside `creation`/`satisfaction`). The one genuine act among the six
/// former obligation verbs: a human rules that a failing check does not
/// apply, with reason and authority, citing the run — never a fact-stream
/// append.
pub struct DecideObligationWaive;

#[async_trait]
impl SemOsVerbOp for DecideObligationWaive {
    fn fqn(&self) -> &str {
        "kyc_ubo.decide.obligation.waiver"
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let check_id = args
            .get("check-id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing check-id argument"))?
            .to_string();
        let reason = args
            .get("reason")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing reason argument"))?;

        let (control, type_registry, _obligations) = load_board_state(scope, subject).await?;
        let run = compute_and_persist_run(
            scope,
            subject,
            &control,
            &type_registry,
            "kyc_ubo.decide.obligation.waiver",
        )
        .await?;

        let basis = serde_json::json!({
            "run_id": run.run_id,
            "board_state_hash": run.board_state_hash.to_hex(),
            "waived_check_id": check_id,
        });
        let decided_by = ctx.principal.actor_id.clone();
        let decision_id = insert_decision_record(
            scope,
            subject,
            "kyc_ubo.decide.obligation.waiver",
            &decided_by,
            basis,
            Some(reason),
            args,
        )
        .await?;

        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "decision_id": decision_id }),
        ))
    }
}

/// Register this pack's ops into a `SemOsVerbOpRegistry`.
pub fn register(registry: &mut sem_os_postgres::ops::SemOsVerbOpRegistry) {
    registry.register(std::sync::Arc::new(DecideApprove));
    registry.register(std::sync::Arc::new(DecideReject));
    registry.register(std::sync::Arc::new(DecideObligationWaive));
}
