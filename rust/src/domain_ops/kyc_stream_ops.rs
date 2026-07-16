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
use ob_poc_kyc_substrate::determination::DeterminationStrategy;
use ob_poc_kyc_substrate::fold::control::check_control_preconditions;
use ob_poc_kyc_substrate::{
    find_subject_entity, fold_control_versioned, fold_obligations_versioned,
    natural_persons_from_events, phase1_lexicon, AuthorityRef, ControlProngStrategy, EdgeId,
    FoldRegistry, OwnershipProngStrategy, PersonId, Prong, ProngCandidate, SmoResult, SubjectId,
    SubjectOverallState, TargetBinding, V1FoldImpl,
};
// fold_obligations_versioned is called for its error side-effect (precondition check)
#[allow(unused_imports)]
use ob_poc_kyc_substrate::fold::obligation::ObligationState as _ObligationStateCheck;

// ── Shared append helper ──────────────────────────────────────────────────────

/// Build a draft from verb args + frozen context identity, then append to stream.
/// `verb_fqn`, `target`, and `payload` are verb-specific; everything else is
/// threaded from `ctx`. If `validate_entry_fqn` is `Some`, the named lexicon
/// entry's preconditions are checked under the lock.
#[allow(clippy::too_many_arguments)] // each parameter is a distinct verb-call scalar/binding, not a bundle candidate shared across the 22 call sites below
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
        .map(|fqn| {
            lexicon
                .get(fqn)
                .ok_or_else(|| anyhow!("{fqn} missing from lexicon"))
        })
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
    fn fqn(&self) -> &str {
        "ubo.edge.assert-economic-interest"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let edge = EdgeId(json_extract_uuid_opt(args, ctx, "edge-id").unwrap_or_else(Uuid::new_v4));
        let outcome = stream_append(
            "ubo.edge.assert-economic-interest",
            subject,
            TargetBinding::for_edge(subject, edge),
            args.clone(),
            "analyst.assert-economic-interest",
            Some("ubo.edge.assert-economic-interest"),
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "edge_id": edge.0, "seq": outcome.seq }),
        ))
    }
}

pub struct UboEdgeAttachEvidence;

#[async_trait]
impl SemOsVerbOp for UboEdgeAttachEvidence {
    fn fqn(&self) -> &str {
        "ubo.edge.attach-evidence"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let edge = EdgeId(json_extract_uuid(args, ctx, "edge-id")?);
        let outcome = stream_append(
            "ubo.edge.attach-evidence",
            subject,
            TargetBinding::for_edge(subject, edge),
            args.clone(),
            "analyst.attach-evidence",
            Some("ubo.edge.attach-evidence"),
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "seq": outcome.seq }),
        ))
    }
}

pub struct UboEdgeVerify;

#[async_trait]
impl SemOsVerbOp for UboEdgeVerify {
    fn fqn(&self) -> &str {
        "ubo.edge.verify"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let edge = EdgeId(json_extract_uuid(args, ctx, "edge-id")?);
        let outcome = stream_append(
            "ubo.edge.verify",
            subject,
            TargetBinding::for_edge(subject, edge),
            args.clone(),
            "analyst.verify",
            Some("ubo.edge.verify"),
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "seq": outcome.seq }),
        ))
    }
}

pub struct UboEdgeSupersede;

#[async_trait]
impl SemOsVerbOp for UboEdgeSupersede {
    fn fqn(&self) -> &str {
        "ubo.edge.supersede"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let edge = EdgeId(json_extract_uuid(args, ctx, "edge-id")?);
        let outcome = stream_append(
            "ubo.edge.supersede",
            subject,
            TargetBinding::for_edge(subject, edge),
            args.clone(),
            "analyst.supersede",
            None,
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "seq": outcome.seq }),
        ))
    }
}

pub struct UboEdgeReconcileConflict;

#[async_trait]
impl SemOsVerbOp for UboEdgeReconcileConflict {
    fn fqn(&self) -> &str {
        "ubo.edge.reconcile-conflict"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let outcome = stream_append(
            "ubo.edge.reconcile-conflict",
            subject,
            TargetBinding::for_subject(subject),
            args.clone(),
            "analyst.reconcile-conflict",
            None,
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "seq": outcome.seq }),
        ))
    }
}

// ── Determination verbs ───────────────────────────────────────────────────────

pub struct UboDeterminationSelectStrategy;

#[async_trait]
impl SemOsVerbOp for UboDeterminationSelectStrategy {
    fn fqn(&self) -> &str {
        "ubo.determination.select-strategy"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let outcome = stream_append(
            "ubo.determination.select-strategy",
            subject,
            TargetBinding::for_subject(subject),
            args.clone(),
            "analyst.select-strategy",
            None,
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "seq": outcome.seq }),
        ))
    }
}

pub struct UboDeterminationComputeFold;

#[async_trait]
impl SemOsVerbOp for UboDeterminationComputeFold {
    fn fqn(&self) -> &str {
        "ubo.determination.compute-fold"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        // compute-fold is a projection read — fold the stream and return the state.
        let events = ob_poc_kyc_store::PgKycEventStore::load_events(scope.executor(), subject)
            .await
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
            "structure_class": state.structure_class,
        })))
    }
}

pub struct UboDeterminationApplySmoFallback;

#[async_trait]
impl SemOsVerbOp for UboDeterminationApplySmoFallback {
    fn fqn(&self) -> &str {
        "ubo.determination.apply-smo-fallback"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let payload = normalize_smo_fallback_payload(args);
        let outcome = stream_append(
            "ubo.determination.apply-smo-fallback",
            subject,
            TargetBinding::for_subject(subject),
            payload,
            "analyst.smo-fallback",
            None,
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "seq": outcome.seq }),
        ))
    }
}

/// Normalize `ubo.determination.apply-smo-fallback` payload: the YAML arg is
/// kebab-case `smo-person-id`, but the fold reads snake_case `smo_person_id`
/// (`fold::control::apply_one_control_event`, via `person_id(p, "smo_person_id")`)
/// — without this the fold silently left `ControlState.smo_person_id` at
/// `None`, meaning the entire SMO-fallback path (K-5's "never silent"
/// escape hatch, consumed at `freeze` time) never actually populated
/// anything even after a caller ran this verb. Same bug class as R3
/// (structure_class) and the `edge.assert-control` kind/edge_kind mismatch.
fn normalize_smo_fallback_payload(args: &serde_json::Value) -> serde_json::Value {
    let mut p = args.clone();
    if let Some(obj) = p.as_object_mut() {
        if let Some(v) = obj.remove("smo-person-id") {
            obj.insert("smo_person_id".to_string(), v);
        }
    }
    p
}

pub struct UboDeterminationFreeze;

#[async_trait]
impl SemOsVerbOp for UboDeterminationFreeze {
    fn fqn(&self) -> &str {
        "ubo.determination.freeze"
    }

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

        // 2. Run the actual determination strategy (EOP-DD-KYCUBO-003 R1/M1.2).
        //    `select-strategy` must have fired (ReconciledProjection/StrategySelected
        //    preconditions gate compute-fold/freeze already); dispatch on the
        //    recorded strategy name rather than assuming ownership_prong_strategy.
        let strategy_name = control
            .selected_strategy
            .as_deref()
            .ok_or_else(|| anyhow!("freeze: no strategy selected (K-4 precondition)"))?;
        let strategy: &dyn DeterminationStrategy = match strategy_name {
            "ownership_prong_strategy" => &OwnershipProngStrategy,
            // M4: control-by-other-means (voting rights, board appointment, GP
            // statutory control, LLP designated member, trust roles, dominant
            // influence). Scope note lives on ControlProngStrategy itself — v1
            // walks control-kind edges only, does not cross into the economic
            // axis for an intermediate controlling entity's own UBOs.
            "control_prong_strategy" => &ControlProngStrategy,
            other => {
                return Err(anyhow!(
                    "freeze: strategy '{other}' selected but no DeterminationStrategy is \
                     registered for it — only ownership_prong_strategy and \
                     control_prong_strategy exist today (role-based strategies for \
                     e.g. investor-role-profile-driven determination remain M2 \
                     follow-on work)"
                ));
            }
        };

        let subject_entity_id = find_subject_entity(&refs).ok_or_else(|| {
            anyhow!(
                "freeze: no kyc.subject.classify-structure event found — subject entity unknown"
            )
        })?;
        let natural_persons = natural_persons_from_events(&refs);
        // K-6: threshold should be reference-plane data (per jurisdiction/structure
        // class); until that table exists, a caller-suppliable default is the
        // documented interim (EOP-DD-KYCUBO-003 M1.4).
        let threshold_pct = args
            .get("threshold-pct")
            .and_then(|v| v.as_f64())
            .unwrap_or(25.0);

        let mut candidates: Vec<ProngCandidate> =
            strategy.resolve(&control, subject_entity_id, &natural_persons, threshold_pct);
        candidates.sort_by_key(|c| c.person_id.0);

        let smo_result = match (control.smo_person_id, control.smo_event_id) {
            (Some(pid), Some(orig)) => Some(SmoResult::Person(ProngCandidate {
                person_id: pid,
                prong: Prong::SmoFallback,
                effective_ownership_pct: None,
                ownership_chain: vec![],
                originating_event_id: orig,
            })),
            (None, _) => None,
            (Some(_), None) => {
                return Err(anyhow!(
                    "freeze: fold invariant violated — smo_person_id set without smo_event_id"
                ));
            }
        };

        // K-5: a determination must never be silent.
        if candidates.is_empty() && smo_result.is_none() {
            return Err(anyhow!(
                "freeze: determination would be silent — no ownership/control candidates and \
                 no SMO fallback applied (K-5); call ubo.determination.apply-smo-fallback first"
            ));
        }

        let resolved_persons: Vec<PersonId> = candidates
            .iter()
            .map(|c| c.person_id)
            .chain(smo_result.iter().filter_map(|s| match s {
                SmoResult::Person(c) => Some(c.person_id),
                SmoResult::AuthorisedWaiver { .. } => None,
            }))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();

        // 3. Get the prior freeze's emitted set for retraction diff (B2).
        let prior_persons = prior_freeze_persons(scope.executor(), subject)
            .await
            .map_err(|e| anyhow!("freeze: prior persons failed: {e}"))?;

        // 4. Append the freeze event to the stream (under the per-subject lock).
        //    The payload carries the resolved candidates + basis (K-1, K-35) —
        //    not just a bare person-id list — so the event itself is the audit record.
        let mut payload = args.clone();
        if let Some(obj) = payload.as_object_mut() {
            obj.insert(
                "strategy".into(),
                serde_json::Value::String(strategy_name.to_string()),
            );
            obj.insert("threshold_pct".into(), json!(threshold_pct));
            obj.insert("candidates".into(), serde_json::to_value(&candidates)?);
            obj.insert("smo_result".into(), serde_json::to_value(&smo_result)?);
        }
        //    `Some(fqn)` re-checks ReconciledProjection + StrategySelected against the
        //    freshly-locked state (K-14) — the declared lexicon preconditions were
        //    previously dead code here (freeze passed `None`, so they never ran).
        let outcome = stream_append(
            "ubo.determination.freeze",
            subject,
            TargetBinding::for_subject(subject),
            payload,
            "senior-analyst.freeze",
            Some("ubo.determination.freeze"),
            ctx,
            scope,
        )
        .await?;

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
            "strategy": strategy_name,
            "resolved_persons": resolved_persons.len(),
            "candidates": candidates,
            "smo_result": smo_result,
            "retracted_persons": prior_persons.len().saturating_sub(resolved_persons.len()),
        })))
    }
}

// ── Subject registration verbs ────────────────────────────────────────────────

pub struct KycSubjectRegister;

#[async_trait]
impl SemOsVerbOp for KycSubjectRegister {
    fn fqn(&self) -> &str {
        "kyc.subject.register"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let payload = normalize_register_payload(args, ctx, subject);
        let outcome = stream_append(
            "kyc.subject.register",
            subject,
            TargetBinding::for_subject(subject),
            payload,
            "analyst.register",
            None,
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "seq": outcome.seq }),
        ))
    }
}

pub struct KycSubjectClassifyStructure;

#[async_trait]
impl SemOsVerbOp for KycSubjectClassifyStructure {
    fn fqn(&self) -> &str {
        "kyc.subject.classify-structure"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let _class = json_extract_string(args, "structure-class")?;
        let payload = normalize_classify_structure_payload(args, ctx, subject);
        let outcome = stream_append(
            "kyc.subject.classify-structure",
            subject,
            TargetBinding::for_subject(subject),
            payload,
            "analyst.classify-structure",
            None,
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "seq": outcome.seq }),
        ))
    }
}

/// Normalize `kyc.subject.register` payload for the fold (EOP-DD-KYCUBO-003 R3/M1.1):
/// the fold reads `entity_id` (`natural_persons_from_events`, `find_subject_entity`);
/// default it to `subject-id` (self-registration) when the caller registers a
/// distinct entity/person within the same determination stream via `entity-id`.
fn normalize_register_payload(
    args: &serde_json::Value,
    ctx: &VerbExecutionContext,
    subject: SubjectId,
) -> serde_json::Value {
    let mut p = args.clone();
    if let Some(obj) = p.as_object_mut() {
        obj.remove("entity-id");
        let entity_id = json_extract_uuid_opt(args, ctx, "entity-id").unwrap_or(subject.0);
        obj.insert(
            "entity_id".to_string(),
            serde_json::Value::String(entity_id.to_string()),
        );
    }
    p
}

/// Normalize `kyc.subject.classify-structure` payload (EOP-DD-KYCUBO-003 R3/M1.1):
/// the YAML arg is kebab-case `structure-class`, but the fold reads snake_case
/// `structure_class` (`structure_class_from_payload`) — without this the fold
/// silently recorded `structure_class: None` in production. Also stamps
/// `entity_id` (defaulting to `subject-id`) so `find_subject_entity` resolves.
fn normalize_classify_structure_payload(
    args: &serde_json::Value,
    ctx: &VerbExecutionContext,
    subject: SubjectId,
) -> serde_json::Value {
    let mut p = args.clone();
    if let Some(obj) = p.as_object_mut() {
        if let Some(v) = obj.remove("structure-class") {
            obj.insert("structure_class".to_string(), v);
        }
        obj.remove("entity-id");
        let entity_id = json_extract_uuid_opt(args, ctx, "entity-id").unwrap_or(subject.0);
        obj.insert(
            "entity_id".to_string(),
            serde_json::Value::String(entity_id.to_string()),
        );
    }
    p
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
    fn fqn(&self) -> &str {
        "kyc.role.assign"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let _role = json_extract_string(args, "role")?;
        let outcome = stream_append(
            "kyc.role.assign",
            subject,
            TargetBinding::for_subject(subject),
            args.clone(),
            "analyst.role-assign",
            None,
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "seq": outcome.seq }),
        ))
    }
}

pub struct KycRoleWithdraw;

#[async_trait]
impl SemOsVerbOp for KycRoleWithdraw {
    fn fqn(&self) -> &str {
        "kyc.role.withdraw"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let outcome = stream_append(
            "kyc.role.withdraw",
            subject,
            TargetBinding::for_subject(subject),
            args.clone(),
            "analyst.role-withdraw",
            None,
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "seq": outcome.seq }),
        ))
    }
}

// ── W5: Obligation lifecycle ──────────────────────────────────────────────────

pub struct KycObligationCreate;

#[async_trait]
impl SemOsVerbOp for KycObligationCreate {
    fn fqn(&self) -> &str {
        "kyc.obligation.create"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let obligation_id =
            json_extract_uuid_opt(args, ctx, "obligation-id").unwrap_or_else(Uuid::new_v4);
        let _role = json_extract_string(args, "role")?;
        let mut payload = args.clone();
        payload["obligation_id"] = serde_json::Value::String(obligation_id.to_string());
        // The YAML arg is kebab-case `cbu-role`, but the fold reads snake_case
        // `cbu_role` (fold::obligation::str_field(p, "cbu_role"), K-24 exposure
        // linkage) — without this the fold silently left ObligationBasis.cbu_role
        // at None. Same bug class as R3 / the smo-person-id fix above.
        if let Some(obj) = payload.as_object_mut() {
            if let Some(v) = obj.remove("cbu-role") {
                obj.insert("cbu_role".to_string(), v);
            }
        }
        let outcome = stream_append(
            "kyc.obligation.create",
            subject,
            TargetBinding::for_subject(subject),
            payload,
            "analyst.obligation-create",
            None,
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "obligation_id": obligation_id, "seq": outcome.seq }),
        ))
    }
}

pub struct KycObligationUpdateIdentity;

#[async_trait]
impl SemOsVerbOp for KycObligationUpdateIdentity {
    fn fqn(&self) -> &str {
        "kyc.obligation.update-identity"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject =
            SubjectId(json_extract_uuid(args, ctx, "subject-id").unwrap_or_else(|_| Uuid::nil()));
        let outcome = stream_append(
            "kyc.obligation.update-identity",
            subject,
            TargetBinding::for_subject(subject),
            normalize_obligation_payload(args),
            "analyst.obligation-update",
            None,
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "seq": outcome.seq }),
        ))
    }
}

pub struct KycObligationUpdateScreening;

#[async_trait]
impl SemOsVerbOp for KycObligationUpdateScreening {
    fn fqn(&self) -> &str {
        "kyc.obligation.update-screening"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject =
            SubjectId(json_extract_uuid(args, ctx, "subject-id").unwrap_or_else(|_| Uuid::nil()));
        let outcome = stream_append(
            "kyc.obligation.update-screening",
            subject,
            TargetBinding::for_subject(subject),
            normalize_obligation_payload(args),
            "analyst.obligation-update",
            None,
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "seq": outcome.seq }),
        ))
    }
}

pub struct KycObligationUpdateRisk;

#[async_trait]
impl SemOsVerbOp for KycObligationUpdateRisk {
    fn fqn(&self) -> &str {
        "kyc.obligation.update-risk"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject =
            SubjectId(json_extract_uuid(args, ctx, "subject-id").unwrap_or_else(|_| Uuid::nil()));
        let outcome = stream_append(
            "kyc.obligation.update-risk",
            subject,
            TargetBinding::for_subject(subject),
            normalize_obligation_payload(args),
            "analyst.obligation-update",
            None,
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "seq": outcome.seq }),
        ))
    }
}

pub struct KycObligationSatisfy;

#[async_trait]
impl SemOsVerbOp for KycObligationSatisfy {
    fn fqn(&self) -> &str {
        "kyc.obligation.satisfy"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject =
            SubjectId(json_extract_uuid(args, ctx, "subject-id").unwrap_or_else(|_| Uuid::nil()));
        let outcome = stream_append(
            "kyc.obligation.satisfy",
            subject,
            TargetBinding::for_subject(subject),
            normalize_obligation_payload(args),
            "analyst.obligation-satisfy",
            None,
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "seq": outcome.seq }),
        ))
    }
}

pub struct KycObligationWaive;

#[async_trait]
impl SemOsVerbOp for KycObligationWaive {
    fn fqn(&self) -> &str {
        "kyc.obligation.waive"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject =
            SubjectId(json_extract_uuid(args, ctx, "subject-id").unwrap_or_else(|_| Uuid::nil()));
        let _reason = json_extract_string(args, "reason")?;
        let outcome = stream_append(
            "kyc.obligation.waive",
            subject,
            TargetBinding::for_subject(subject),
            normalize_obligation_payload(args),
            "analyst.obligation-waive",
            None,
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "seq": outcome.seq }),
        ))
    }
}

pub struct KycPersonApprove;

#[async_trait]
impl SemOsVerbOp for KycPersonApprove {
    fn fqn(&self) -> &str {
        "kyc.person.approve"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);

        // K-23 gate (EOP-DD-KYCUBO-003 R2/M1.3): a subject may be approved only
        // once all required obligations have reached an allowed terminal state.
        // Folded pre-append (same accepted small race window as freeze's
        // pre-fold, above) rather than inside the ControlState-only
        // `append_in_scope` validate closure, which has no obligation view.
        let events = PgKycEventStore::load_events(scope.executor(), subject)
            .await
            .map_err(|e| anyhow!("person.approve: load events failed: {e}"))?;
        let refs: Vec<&ob_poc_kyc_substrate::IntentEvent> = events.iter().collect();
        let obligations = fold_obligations_versioned(&refs, &KYC_REGISTRY)
            .map_err(|e| anyhow!("person.approve: obligation fold failed: {e}"))?;
        let overall = obligations.derive_subject_state(subject);
        if overall != SubjectOverallState::AllTerminal {
            return Err(anyhow!(
                "kyc.person.approve rejected: subject {} obligations are not all terminal \
                 (state={overall:?}) — K-23 gate (determination and approval are separate; \
                 approval requires every required obligation to reach a terminal state)",
                subject.0,
            ));
        }

        let outcome = stream_append(
            "kyc.person.approve",
            subject,
            TargetBinding::for_subject(subject),
            args.clone(),
            "senior-analyst.approve",
            None,
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "seq": outcome.seq }),
        ))
    }
}

pub struct KycPersonReject;

#[async_trait]
impl SemOsVerbOp for KycPersonReject {
    fn fqn(&self) -> &str {
        "kyc.person.reject"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let outcome = stream_append(
            "kyc.person.reject",
            subject,
            TargetBinding::for_subject(subject),
            args.clone(),
            "senior-analyst.reject",
            None,
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "seq": outcome.seq }),
        ))
    }
}
