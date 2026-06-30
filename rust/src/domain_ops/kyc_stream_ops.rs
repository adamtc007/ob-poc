//! Stream-backed KYC/UBO determination verbs (EOP-DD-KYCUBO-002, rip-and-replace R1+).
//!
//! These are the `dsl.kyc` lexicon verbs — the V&S determination vocabulary that
//! REPLACES the legacy `ubo.*` / `control.*` / `ownership.*` / `board.*` write
//! verbs. Each op holds **no** determination logic: it builds an `IntentEvent`
//! from the verb args + the frozen execution identity (via the seam) and appends
//! it to the durable verb stream. Current state is a fold/projection of that
//! stream — never a direct table write.

use std::sync::{Arc, LazyLock};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::json;
use uuid::Uuid;

use dsl_runtime::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};
use sem_os_postgres::ops::SemOsVerbOp;

use ob_poc_kyc_seam::{append_in_scope, IntentEventDraft};
use ob_poc_kyc_store::{enqueue_cross_stream_obligations, prior_freeze_persons, PgKycEventStore};
use ob_poc_kyc_substrate::fold::control::check_control_preconditions;
use ob_poc_kyc_substrate::{
    fold_control_versioned, fold_obligations_versioned,
    phase1_lexicon, AuthorityRef, EdgeId, FoldRegistry, PersonId, SubjectId, TargetBinding,
    V1FoldImpl,
};
// fold_obligations_versioned is called for its error side-effect (precondition check)
#[allow(unused_imports)]
use ob_poc_kyc_substrate::fold::obligation::ObligationState as _ObligationStateCheck;

// ── Shared append helper ──────────────────────────────────────────────────────

/// Build a draft from verb args + frozen context identity, then append to stream.
/// `verb_fqn`, `target`, and `payload` are verb-specific; everything else is
/// threaded from `ctx`. If `validate_entry_fqn` is `Some`, the named lexicon
/// entry's preconditions are checked under the lock.
async fn stream_append(
    verb_fqn: &str,
    subject: SubjectId,
    target: TargetBinding,
    payload: serde_json::Value,
    authority: &str,
    validate_entry_fqn: Option<&str>,
    ctx: &mut VerbExecutionContext,
    scope: &mut dyn TransactionScope,
) -> Result<ob_poc_kyc_store::AppendOutcome> {
    let lexicon = phase1_lexicon();
    let entry = validate_entry_fqn
        .map(|fqn| lexicon.get(fqn).ok_or_else(|| anyhow!("{fqn} missing from lexicon")))
        .transpose()?;

    let event = IntentEventDraft {
        verb_fqn: verb_fqn.into(),
        subject_root: subject,
        target,
        payload,
        authority: AuthorityRef(authority.into()),
        lexicon_hash: lexicon.hash,
        as_of: ctx.as_of,
    }
    .into_event(&ctx.principal, ctx.correlation_id, ctx.execution_id);

    append_in_scope(scope, &KYC_REGISTRY, &event, |state| {
        if let Some(e) = entry {
            check_control_preconditions(e, state, &event)?;
        }
        Ok(())
    })
    .await
    .map_err(|e| anyhow!("{verb_fqn} stream append failed: {e}"))
}

use super::helpers::{json_extract_string, json_extract_uuid, json_extract_uuid_opt};

/// The KYC fold registry (v1) — maps the phase-1 lexicon hash to its `FoldImpl`.
/// Module-level for now; becomes an injected platform service when fold
/// version-dispatch (D2) needs more than one registered version.
static KYC_REGISTRY: LazyLock<FoldRegistry> = LazyLock::new(|| {
    let mut registry = FoldRegistry::new();
    registry.register(phase1_lexicon().hash, Arc::new(V1FoldImpl));
    registry
});

// ── Edge lifecycle verbs ──────────────────────────────────────────────────────

/// `ubo.edge.assert-control` — claim a control edge (voting, board, GP statutory,
/// …). The first stream-backed determination verb; the pattern every other
/// `dsl.kyc` verb follows.
pub struct UboEdgeAssertControl;

#[async_trait]
impl SemOsVerbOp for UboEdgeAssertControl {
    fn fqn(&self) -> &str {
        "ubo.edge.assert-control"
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        // The determination root this edge belongs to (the subject stream).
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        // Stable edge id (caller-supplied or fresh); the edge's identity in the fold.
        let edge = EdgeId(json_extract_uuid_opt(args, ctx, "edge-id").unwrap_or_else(Uuid::new_v4));

        let lexicon = phase1_lexicon();
        let entry = lexicon
            .get("ubo.edge.assert-control")
            .ok_or_else(|| anyhow!("ubo.edge.assert-control missing from lexicon"))?;

        // The verb args ARE the event payload (from_entity_id, to_entity_id,
        // edge_kind, percentage, …) — the fold reads them.
        let event = IntentEventDraft {
            verb_fqn: "ubo.edge.assert-control".into(),
            subject_root: subject,
            target: TargetBinding::for_edge(subject, edge),
            payload: args.clone(),
            authority: AuthorityRef("analyst.assert-control".into()),
            lexicon_hash: lexicon.hash,
            as_of: ctx.as_of, // frozen at verb entry — never now() here
        }
        .into_event(&ctx.principal, ctx.correlation_id, ctx.execution_id);

        let outcome = append_in_scope(scope, &KYC_REGISTRY, &event, |state| {
            check_control_preconditions(entry, state, &event)
        })
        .await
        .map_err(|e| anyhow!("ubo.edge.assert-control append failed: {e}"))?;

        Ok(VerbExecutionOutcome::Record(json!({
            "edge_id": edge.0,
            "seq": outcome.seq,
            "deduped": outcome.deduped,
        })))
    }
}

pub struct UboEdgeAssertEconomicInterest;

#[async_trait]
impl SemOsVerbOp for UboEdgeAssertEconomicInterest {
    fn fqn(&self) -> &str { "ubo.edge.assert-economic-interest" }
    async fn execute(&self, args: &serde_json::Value, ctx: &mut VerbExecutionContext, scope: &mut dyn TransactionScope) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let edge = EdgeId(json_extract_uuid_opt(args, ctx, "edge-id").unwrap_or_else(Uuid::new_v4));
        let outcome = stream_append(
            "ubo.edge.assert-economic-interest", subject,
            TargetBinding::for_edge(subject, edge), args.clone(),
            "analyst.assert-economic-interest", Some("ubo.edge.assert-economic-interest"),
            ctx, scope,
        ).await?;
        Ok(VerbExecutionOutcome::Record(serde_json::json!({ "edge_id": edge.0, "seq": outcome.seq })))
    }
}

pub struct UboEdgeAttachEvidence;

#[async_trait]
impl SemOsVerbOp for UboEdgeAttachEvidence {
    fn fqn(&self) -> &str { "ubo.edge.attach-evidence" }
    async fn execute(&self, args: &serde_json::Value, ctx: &mut VerbExecutionContext, scope: &mut dyn TransactionScope) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let edge = EdgeId(json_extract_uuid(args, ctx, "edge-id")?);
        let outcome = stream_append(
            "ubo.edge.attach-evidence", subject,
            TargetBinding::for_edge(subject, edge), args.clone(),
            "analyst.attach-evidence", Some("ubo.edge.attach-evidence"),
            ctx, scope,
        ).await?;
        Ok(VerbExecutionOutcome::Record(serde_json::json!({ "seq": outcome.seq })))
    }
}

pub struct UboEdgeVerify;

#[async_trait]
impl SemOsVerbOp for UboEdgeVerify {
    fn fqn(&self) -> &str { "ubo.edge.verify" }
    async fn execute(&self, args: &serde_json::Value, ctx: &mut VerbExecutionContext, scope: &mut dyn TransactionScope) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let edge = EdgeId(json_extract_uuid(args, ctx, "edge-id")?);
        let outcome = stream_append(
            "ubo.edge.verify", subject,
            TargetBinding::for_edge(subject, edge), args.clone(),
            "analyst.verify", Some("ubo.edge.verify"),
            ctx, scope,
        ).await?;
        Ok(VerbExecutionOutcome::Record(serde_json::json!({ "seq": outcome.seq })))
    }
}

pub struct UboEdgeSupersede;

#[async_trait]
impl SemOsVerbOp for UboEdgeSupersede {
    fn fqn(&self) -> &str { "ubo.edge.supersede" }
    async fn execute(&self, args: &serde_json::Value, ctx: &mut VerbExecutionContext, scope: &mut dyn TransactionScope) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let edge = EdgeId(json_extract_uuid(args, ctx, "edge-id")?);
        let outcome = stream_append(
            "ubo.edge.supersede", subject,
            TargetBinding::for_edge(subject, edge), args.clone(),
            "analyst.supersede", None,
            ctx, scope,
        ).await?;
        Ok(VerbExecutionOutcome::Record(serde_json::json!({ "seq": outcome.seq })))
    }
}

pub struct UboEdgeReconcileConflict;

#[async_trait]
impl SemOsVerbOp for UboEdgeReconcileConflict {
    fn fqn(&self) -> &str { "ubo.edge.reconcile-conflict" }
    async fn execute(&self, args: &serde_json::Value, ctx: &mut VerbExecutionContext, scope: &mut dyn TransactionScope) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let outcome = stream_append(
            "ubo.edge.reconcile-conflict", subject,
            TargetBinding::for_subject(subject), args.clone(),
            "analyst.reconcile-conflict", None,
            ctx, scope,
        ).await?;
        Ok(VerbExecutionOutcome::Record(serde_json::json!({ "seq": outcome.seq })))
    }
}

// ── Determination verbs ───────────────────────────────────────────────────────

pub struct UboDeterminationSelectStrategy;

#[async_trait]
impl SemOsVerbOp for UboDeterminationSelectStrategy {
    fn fqn(&self) -> &str { "ubo.determination.select-strategy" }
    async fn execute(&self, args: &serde_json::Value, ctx: &mut VerbExecutionContext, scope: &mut dyn TransactionScope) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let outcome = stream_append(
            "ubo.determination.select-strategy", subject,
            TargetBinding::for_subject(subject), args.clone(),
            "analyst.select-strategy", None,
            ctx, scope,
        ).await?;
        Ok(VerbExecutionOutcome::Record(serde_json::json!({ "seq": outcome.seq })))
    }
}

pub struct UboDeterminationComputeFold;

#[async_trait]
impl SemOsVerbOp for UboDeterminationComputeFold {
    fn fqn(&self) -> &str { "ubo.determination.compute-fold" }
    async fn execute(&self, args: &serde_json::Value, ctx: &mut VerbExecutionContext, scope: &mut dyn TransactionScope) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        // compute-fold is a projection read — fold the stream and return the state.
        let events = ob_poc_kyc_store::PgKycEventStore::load_events(scope.executor(), subject).await
            .map_err(|e| anyhow!("compute-fold load failed: {e}"))?;
        let refs: Vec<&ob_poc_kyc_substrate::IntentEvent> = events.iter().collect();
        let state = ob_poc_kyc_substrate::fold_control_versioned(&refs, &KYC_REGISTRY)
            .map_err(|e| anyhow!("compute-fold failed: {e}"))?;
        Ok(VerbExecutionOutcome::Record(serde_json::json!({
            "registered": state.registered,
            "edge_count": state.edges.len(),
            "active_edges": state.edges.values().filter(|e| e.is_active()).count(),
            "verified_edges": state.edges.values().filter(|e| e.is_verified()).count(),
            "is_reconciled": state.is_reconciled(),
            "has_strategy": state.has_strategy(),
        })))
    }
}

pub struct UboDeterminationApplySmoFallback;

#[async_trait]
impl SemOsVerbOp for UboDeterminationApplySmoFallback {
    fn fqn(&self) -> &str { "ubo.determination.apply-smo-fallback" }
    async fn execute(&self, args: &serde_json::Value, ctx: &mut VerbExecutionContext, scope: &mut dyn TransactionScope) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let outcome = stream_append(
            "ubo.determination.apply-smo-fallback", subject,
            TargetBinding::for_subject(subject), args.clone(),
            "analyst.smo-fallback", None,
            ctx, scope,
        ).await?;
        Ok(VerbExecutionOutcome::Record(serde_json::json!({ "seq": outcome.seq })))
    }
}

pub struct UboDeterminationFreeze;

#[async_trait]
impl SemOsVerbOp for UboDeterminationFreeze {
    fn fqn(&self) -> &str { "ubo.determination.freeze" }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);

        // 1. Fold current control+obligation state under the lock (precondition check).
        //    The fold runs INSIDE append_in_scope (under the FOR UPDATE lock), but we
        //    also need the resolved persons BEFORE the append so we can diff. We fold
        //    here — the append folds again under the lock; deterministic, same result.
        let events = PgKycEventStore::load_events(scope.executor(), subject)
            .await
            .map_err(|e| anyhow!("freeze: load events failed: {e}"))?;
        let refs: Vec<&ob_poc_kyc_substrate::IntentEvent> = events.iter().collect();
        let control = fold_control_versioned(&refs, &KYC_REGISTRY)
            .map_err(|e| anyhow!("freeze: control fold failed: {e}"))?;
        let _ = fold_obligations_versioned(&refs, &KYC_REGISTRY)
            .map_err(|e| anyhow!("freeze: obligation fold failed: {e}"))?;

        // 2. Extract resolved persons from the control fold's determination candidates.
        //    (A full strategy run is out of scope here; we use the fold's natural-person
        //    edges as a proxy for the resolved set. The full OwnershipProngStrategy
        //    is in the substrate and would need entity→person resolution data.)
        let resolved_persons: Vec<PersonId> = control
            .edges
            .values()
            .filter(|e| e.is_active())
            .map(|e| PersonId(e.from.0))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();

        // 3. Get the prior freeze's emitted set for retraction diff (B2).
        let prior_persons = prior_freeze_persons(scope.executor(), subject)
            .await
            .map_err(|e| anyhow!("freeze: prior persons failed: {e}"))?;

        // 4. Append the freeze event to the stream (under the per-subject lock).
        let outcome = stream_append(
            "ubo.determination.freeze", subject,
            TargetBinding::for_subject(subject), args.clone(),
            "senior-analyst.freeze", None,
            ctx, scope,
        ).await?;

        // 5. Enqueue cross-stream obligation effects (B2 retraction + B3 idem keys).
        //    These commit atomically with the freeze event in the same scope transaction.
        enqueue_cross_stream_obligations(
            scope.executor(),
            outcome.event_id,
            subject,
            &resolved_persons,
            &prior_persons,
            "ubo_candidate",
            ctx.correlation_id,
        )
        .await
        .map_err(|e| anyhow!("freeze: cross-stream enqueue failed: {e}"))?;

        Ok(VerbExecutionOutcome::Record(json!({
            "seq": outcome.seq,
            "resolved_persons": resolved_persons.len(),
            "retracted_persons": prior_persons.len().saturating_sub(resolved_persons.len()),
        })))
    }
}

// ── Subject registration verbs ────────────────────────────────────────────────

pub struct KycSubjectRegister;

#[async_trait]
impl SemOsVerbOp for KycSubjectRegister {
    fn fqn(&self) -> &str { "kyc.subject.register" }
    async fn execute(&self, args: &serde_json::Value, ctx: &mut VerbExecutionContext, scope: &mut dyn TransactionScope) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let outcome = stream_append(
            "kyc.subject.register", subject,
            TargetBinding::for_subject(subject), args.clone(),
            "analyst.register", None,
            ctx, scope,
        ).await?;
        Ok(VerbExecutionOutcome::Record(serde_json::json!({ "seq": outcome.seq })))
    }
}

pub struct KycSubjectClassifyStructure;

#[async_trait]
impl SemOsVerbOp for KycSubjectClassifyStructure {
    fn fqn(&self) -> &str { "kyc.subject.classify-structure" }
    async fn execute(&self, args: &serde_json::Value, ctx: &mut VerbExecutionContext, scope: &mut dyn TransactionScope) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let _class = json_extract_string(args, "structure-class")?;
        let outcome = stream_append(
            "kyc.subject.classify-structure", subject,
            TargetBinding::for_subject(subject), args.clone(),
            "analyst.classify-structure", None,
            ctx, scope,
        ).await?;
        Ok(VerbExecutionOutcome::Record(serde_json::json!({ "seq": outcome.seq })))
    }
}


/// Normalize YAML-style arg names (kebab-case) to the fold's expected payload keys (snake_case).
/// The obligation fold reads "obligation_id" (underscore), not "obligation-id" (hyphen).
fn normalize_obligation_payload(args: &serde_json::Value) -> serde_json::Value {
    let mut p = args.clone();
    if let Some(obj) = p.as_object_mut() {
        if let Some(v) = obj.remove("obligation-id") {
            obj.insert("obligation_id".to_string(), v);
        }
        if let Some(v) = obj.remove("subject-id") {
            obj.insert("subject_id".to_string(), v);
        }
    }
    p
}

// ── W3: Role-basis recording ──────────────────────────────────────────────────

pub struct KycRoleAssign;

#[async_trait]
impl SemOsVerbOp for KycRoleAssign {
    fn fqn(&self) -> &str { "kyc.role.assign" }
    async fn execute(&self, args: &serde_json::Value, ctx: &mut VerbExecutionContext, scope: &mut dyn TransactionScope) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let _role = json_extract_string(args, "role")?;
        let outcome = stream_append("kyc.role.assign", subject, TargetBinding::for_subject(subject), args.clone(), "analyst.role-assign", None, ctx, scope).await?;
        Ok(VerbExecutionOutcome::Record(serde_json::json!({ "seq": outcome.seq })))
    }
}

pub struct KycRoleWithdraw;

#[async_trait]
impl SemOsVerbOp for KycRoleWithdraw {
    fn fqn(&self) -> &str { "kyc.role.withdraw" }
    async fn execute(&self, args: &serde_json::Value, ctx: &mut VerbExecutionContext, scope: &mut dyn TransactionScope) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let outcome = stream_append("kyc.role.withdraw", subject, TargetBinding::for_subject(subject), args.clone(), "analyst.role-withdraw", None, ctx, scope).await?;
        Ok(VerbExecutionOutcome::Record(serde_json::json!({ "seq": outcome.seq })))
    }
}

// ── W5: Obligation lifecycle ──────────────────────────────────────────────────

pub struct KycObligationCreate;

#[async_trait]
impl SemOsVerbOp for KycObligationCreate {
    fn fqn(&self) -> &str { "kyc.obligation.create" }
    async fn execute(&self, args: &serde_json::Value, ctx: &mut VerbExecutionContext, scope: &mut dyn TransactionScope) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let obligation_id = json_extract_uuid_opt(args, ctx, "obligation-id").unwrap_or_else(Uuid::new_v4);
        let _role = json_extract_string(args, "role")?;
        let mut payload = args.clone();
        payload["obligation_id"] = serde_json::Value::String(obligation_id.to_string());
        let outcome = stream_append("kyc.obligation.create", subject, TargetBinding::for_subject(subject), payload, "analyst.obligation-create", None, ctx, scope).await?;
        Ok(VerbExecutionOutcome::Record(serde_json::json!({ "obligation_id": obligation_id, "seq": outcome.seq })))
    }
}

pub struct KycObligationUpdateIdentity;

#[async_trait]
impl SemOsVerbOp for KycObligationUpdateIdentity {
    fn fqn(&self) -> &str { "kyc.obligation.update-identity" }
    async fn execute(&self, args: &serde_json::Value, ctx: &mut VerbExecutionContext, scope: &mut dyn TransactionScope) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id").unwrap_or_else(|_| Uuid::nil()));
        let outcome = stream_append("kyc.obligation.update-identity", subject, TargetBinding::for_subject(subject), normalize_obligation_payload(args), "analyst.obligation-update", None, ctx, scope).await?;
        Ok(VerbExecutionOutcome::Record(serde_json::json!({ "seq": outcome.seq })))
    }
}

pub struct KycObligationUpdateScreening;

#[async_trait]
impl SemOsVerbOp for KycObligationUpdateScreening {
    fn fqn(&self) -> &str { "kyc.obligation.update-screening" }
    async fn execute(&self, args: &serde_json::Value, ctx: &mut VerbExecutionContext, scope: &mut dyn TransactionScope) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id").unwrap_or_else(|_| Uuid::nil()));
        let outcome = stream_append("kyc.obligation.update-screening", subject, TargetBinding::for_subject(subject), normalize_obligation_payload(args), "analyst.obligation-update", None, ctx, scope).await?;
        Ok(VerbExecutionOutcome::Record(serde_json::json!({ "seq": outcome.seq })))
    }
}

pub struct KycObligationUpdateRisk;

#[async_trait]
impl SemOsVerbOp for KycObligationUpdateRisk {
    fn fqn(&self) -> &str { "kyc.obligation.update-risk" }
    async fn execute(&self, args: &serde_json::Value, ctx: &mut VerbExecutionContext, scope: &mut dyn TransactionScope) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id").unwrap_or_else(|_| Uuid::nil()));
        let outcome = stream_append("kyc.obligation.update-risk", subject, TargetBinding::for_subject(subject), normalize_obligation_payload(args), "analyst.obligation-update", None, ctx, scope).await?;
        Ok(VerbExecutionOutcome::Record(serde_json::json!({ "seq": outcome.seq })))
    }
}

pub struct KycObligationSatisfy;

#[async_trait]
impl SemOsVerbOp for KycObligationSatisfy {
    fn fqn(&self) -> &str { "kyc.obligation.satisfy" }
    async fn execute(&self, args: &serde_json::Value, ctx: &mut VerbExecutionContext, scope: &mut dyn TransactionScope) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id").unwrap_or_else(|_| Uuid::nil()));
        let outcome = stream_append("kyc.obligation.satisfy", subject, TargetBinding::for_subject(subject), normalize_obligation_payload(args), "analyst.obligation-satisfy", None, ctx, scope).await?;
        Ok(VerbExecutionOutcome::Record(serde_json::json!({ "seq": outcome.seq })))
    }
}

pub struct KycObligationWaive;

#[async_trait]
impl SemOsVerbOp for KycObligationWaive {
    fn fqn(&self) -> &str { "kyc.obligation.waive" }
    async fn execute(&self, args: &serde_json::Value, ctx: &mut VerbExecutionContext, scope: &mut dyn TransactionScope) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id").unwrap_or_else(|_| Uuid::nil()));
        let _reason = json_extract_string(args, "reason")?;
        let outcome = stream_append("kyc.obligation.waive", subject, TargetBinding::for_subject(subject), normalize_obligation_payload(args), "analyst.obligation-waive", None, ctx, scope).await?;
        Ok(VerbExecutionOutcome::Record(serde_json::json!({ "seq": outcome.seq })))
    }
}

pub struct KycPersonApprove;

#[async_trait]
impl SemOsVerbOp for KycPersonApprove {
    fn fqn(&self) -> &str { "kyc.person.approve" }
    async fn execute(&self, args: &serde_json::Value, ctx: &mut VerbExecutionContext, scope: &mut dyn TransactionScope) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let outcome = stream_append("kyc.person.approve", subject, TargetBinding::for_subject(subject), args.clone(), "senior-analyst.approve", None, ctx, scope).await?;
        Ok(VerbExecutionOutcome::Record(serde_json::json!({ "seq": outcome.seq })))
    }
}

pub struct KycPersonReject;

#[async_trait]
impl SemOsVerbOp for KycPersonReject {
    fn fqn(&self) -> &str { "kyc.person.reject" }
    async fn execute(&self, args: &serde_json::Value, ctx: &mut VerbExecutionContext, scope: &mut dyn TransactionScope) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let outcome = stream_append("kyc.person.reject", subject, TargetBinding::for_subject(subject), args.clone(), "senior-analyst.reject", None, ctx, scope).await?;
        Ok(VerbExecutionOutcome::Record(serde_json::json!({ "seq": outcome.seq })))
    }
}

// ── D3: Board-controller override as verb (K-13/K-15/K-32/K-35) ──────────────

/// `ubo.board-controller.override` — the D3 ratified decision: a board-control
/// override is a first-class verb event on the stream, not a side-effecting table
/// write. The human-authored `board_controller_overrides` table (which never
/// existed in this DB) is replaced by this verb + fold. Supersede-never-delete.
pub struct UboBoardControllerOverride;

#[async_trait]
impl SemOsVerbOp for UboBoardControllerOverride {
    fn fqn(&self) -> &str { "ubo.board-controller.override" }
    async fn execute(&self, args: &serde_json::Value, ctx: &mut VerbExecutionContext, scope: &mut dyn TransactionScope) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let _controller = json_extract_uuid(args, ctx, "controller-entity-id")?;
        let _basis = json_extract_string(args, "basis")?;
        let outcome = stream_append(
            "ubo.board-controller.override", subject,
            TargetBinding::for_subject(subject), args.clone(),
            "senior-analyst.board-controller-override", None,
            ctx, scope,
        ).await?;
        Ok(VerbExecutionOutcome::Record(serde_json::json!({ "seq": outcome.seq })))
    }
}
