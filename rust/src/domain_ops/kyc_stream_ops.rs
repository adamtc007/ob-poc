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
use ob_poc_kyc_substrate::fold::control::check_control_preconditions;
use ob_poc_kyc_substrate::{
    phase1_lexicon, AuthorityRef, EdgeId, FoldRegistry, SubjectId, TargetBinding, V1FoldImpl,
};

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
    async fn execute(&self, args: &serde_json::Value, ctx: &mut VerbExecutionContext, scope: &mut dyn TransactionScope) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let outcome = stream_append(
            "ubo.determination.freeze", subject,
            TargetBinding::for_subject(subject), args.clone(),
            "analyst.freeze", None,
            ctx, scope,
        ).await?;
        Ok(VerbExecutionOutcome::Record(serde_json::json!({ "seq": outcome.seq })))
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
