//! KYC/UBO Evaluation-pack verdicts — `decide.approve`, `decide.reject`
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
//! `kyc.obligation.waive` stays a fact-stream verb (`ob-poc-kyc-substrate`'s
//! `assembly_lexicon()`, unrenamed) for now — see that entry's own doc
//! comment. It still mutates live obligation-track fold state
//! (`TrackState::Waived`), so it cannot leave the fact stream until
//! obligation dissolution (D2.0, unbuilt) removes the track model it writes
//! to.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use sqlx::Row;
use uuid::Uuid;

use dsl_runtime::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};
use sem_os_postgres::ops::SemOsVerbOp;

use ob_poc_kyc_read::PgKycEventReader;
use ob_poc_kyc_substrate::{fold_obligations_versioned, FoldRegistry, SubjectId, SubjectOverallState, V1FoldImpl};

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

/// Fold the obligation graph for `subject` from the live fact stream
/// (a read, not a write — permitted under TS.6 §2's boundary: "Evaluation
/// reads... its determination").
async fn fold_obligations_for(
    scope: &mut dyn TransactionScope,
    subject: SubjectId,
) -> Result<ob_poc_kyc_substrate::ObligationState> {
    let events = PgKycEventReader::load_events(scope.executor(), subject)
        .await
        .map_err(|e| anyhow!("decide: load events failed: {e}"))?;
    let refs: Vec<&ob_poc_kyc_substrate::IntentEvent> = events.iter().collect();
    fold_obligations_versioned(&refs, &v1_registry())
        .map_err(|e| anyhow!("decide: obligation fold failed: {e}"))
}

/// Build the `basis` JSON recorded on every decision record
/// (`decide_verbs_cite_their_basis`, TS.6 §8) — the obligation-fold snapshot
/// that informed the decision. Never empty: always names the overall state
/// and every obligation folded, even when the subject has none yet.
fn basis_json(
    obligations: &ob_poc_kyc_substrate::ObligationState,
    subject: SubjectId,
) -> serde_json::Value {
    let overall = obligations.derive_subject_state(subject);
    let overall_name = match overall {
        SubjectOverallState::AllTerminal => "AllTerminal",
        SubjectOverallState::InProgress => "InProgress",
    };
    let obligation_ids: Vec<String> = obligations
        .subjects
        .get(&subject)
        .map(|rollup| rollup.obligations.iter().map(|oid| oid.0.to_string()).collect())
        .unwrap_or_default();
    serde_json::json!({
        "overall_state": overall_name,
        "obligation_ids": obligation_ids,
    })
}

/// K-23 "decision is final" — query `kyc_decision_records` directly rather
/// than the fact-stream fold (which can no longer see decisions at all,
/// TS.6 P2). Refuses if a prior `decide.approve`/`decide.reject` exists for
/// this subject.
async fn refuse_if_already_decided(
    scope: &mut dyn TransactionScope,
    subject: SubjectId,
) -> Result<()> {
    let existing = sqlx::query(
        r#"SELECT verb_fqn FROM "ob-poc".kyc_decision_records
           WHERE subject_root = $1 AND verb_fqn IN ('decide.approve', 'decide.reject')
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

// ── decide.approve ──────────────────────────────────────────────────────────

pub struct DecideApprove;

#[async_trait]
impl SemOsVerbOp for DecideApprove {
    fn fqn(&self) -> &str {
        "decide.approve"
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);

        let obligations = fold_obligations_for(scope, subject).await?;
        // K-23 gate: all required obligation tracks terminal.
        if obligations.derive_subject_state(subject) != SubjectOverallState::AllTerminal {
            return Err(anyhow!(
                "decide.approve rejected: subject {} obligations are not all terminal — \
                 K-23 gate (determination and approval are separate; approval requires \
                 every required obligation to reach a terminal state)",
                subject.0,
            ));
        }

        refuse_if_already_decided(scope, subject).await?;

        let basis = basis_json(&obligations, subject);
        let decided_by = ctx.principal.actor_id.clone();
        let decision_id = insert_decision_record(
            scope,
            subject,
            "decide.approve",
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

// ── decide.reject ───────────────────────────────────────────────────────────

pub struct DecideReject;

#[async_trait]
impl SemOsVerbOp for DecideReject {
    fn fqn(&self) -> &str {
        "decide.reject"
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);

        refuse_if_already_decided(scope, subject).await?;

        // Rejection is deliberately allowed at ANY stage of the obligation
        // graph (early rejection is a real compliance outcome) — the fold
        // here is for the basis snapshot only, never a gate.
        let obligations = fold_obligations_for(scope, subject).await?;
        let basis = basis_json(&obligations, subject);
        let reason = json_extract_string_opt(args, "reason");
        let decided_by = ctx.principal.actor_id.clone();
        let decision_id = insert_decision_record(
            scope,
            subject,
            "decide.reject",
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

/// Register this pack's ops into a `SemOsVerbOpRegistry`.
pub fn register(registry: &mut sem_os_postgres::ops::SemOsVerbOpRegistry) {
    registry.register(std::sync::Arc::new(DecideApprove));
    registry.register(std::sync::Arc::new(DecideReject));
}
