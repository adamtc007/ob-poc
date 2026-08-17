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

use ob_poc_kyc_seam::{append_in_scope, map_principal, IntentEventDraft};
use ob_poc_kyc_store::{enqueue_cross_stream_obligations, prior_freeze_persons, PgKycEventStore};
use ob_poc_kyc_substrate::{
    check_control_preconditions, check_preconditions, find_subject_entity, fold_control_versioned,
    fold_obligations_versioned, natural_persons_from_events, phase1_lexicon,
    render_intent_event_to_sexpr, AuthorityRef, ControlProngStrategy, DeterminationStrategy,
    CooperativeMemberStrategy, EdgeId, EdgeKind, FoldRegistry, FoundationCouncilStrategy,
    FundControlStrategy, NomineePierceStrategy, OwnershipProngStrategy, PersonId, Prong,
    ProngCandidate, SmoResult, StateOwnedStrategy, SubjectId, SubjectOverallState, TargetBinding,
    TrustRoleStrategy, V1FoldImpl,
    EDGE_KIND_WIRE_VALUES,
};
// fold_obligations_versioned is called for its error side-effect (precondition check)
#[allow(unused_imports)]
use ob_poc_kyc_substrate::ObligationState as _ObligationStateCheck;

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
    // Rendering looks the entry up independently of `validate_entry_fqn` — 8 of
    // the 20 dsl.kyc verbs have no lexicon entry at all yet (T0.3 gap K-G6;
    // T6.0 closes this), so `render_intent_event_to_sexpr` takes `Option` and
    // degrades gracefully.
    let render_entry = entry.or_else(|| lexicon.get(verb_fqn));

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
    let source_text = render_intent_event_to_sexpr(&event, render_entry);

    append_in_scope(scope, &KYC_REGISTRY, &event, &source_text, |control, obligation| {
        if let Some(e) = entry {
            check_preconditions(e, control, obligation, &event)?;
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
            payload: normalize_assert_control_payload(args, edge)?,
            authority: AuthorityRef("analyst.assert-control".into()),
            lexicon_hash: lexicon.hash,
            as_of: ctx.as_of, // frozen at verb entry — never now() here
        }
        .into_event(&ctx.principal, ctx.correlation_id, ctx.execution_id);
        let source_text = render_intent_event_to_sexpr(&event, Some(entry));

        let outcome = append_in_scope(scope, &KYC_REGISTRY, &event, &source_text, |control, obligation| {
            check_preconditions(entry, control, obligation, &event)
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
            normalize_edge_id_payload(args, edge),
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
        // T6.2 row 4: EdgeExists + EdgeActive now attached — `Some(fqn)`
        // wires it to the real checker (previously `None` was harmless
        // because the entry declared no preconditions; leaving it `None`
        // now would silently never enforce the new stud — the exact defect
        // class the Part A closure tooth exists to catch).
        let outcome = stream_append(
            "ubo.edge.supersede",
            subject,
            TargetBinding::for_edge(subject, edge),
            args.clone(),
            "analyst.supersede",
            Some("ubo.edge.supersede"),
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "seq": outcome.seq }),
        ))
    }
}

/// `ubo.edge.pierce-nominee` — TS.4 = K-8 (EOP-DD-KYCUBO-KIT-TS0 §2.6,
/// ratified 2026-08-12): pierce a nominee arrangement. ONE governed event,
/// TWO fold effects: (a) the target nominee edge is superseded (K-13,
/// `superseded_by` = the pierce event); (b) a new control edge from the
/// disclosed nominator is asserted with the UNDERLYING kind and
/// `pierced_from` provenance. The "target is actually a nominee edge" check
/// has no precondition primitive — enforced here, fail-closed, via a
/// pre-append fold (same pre-fold pattern as freeze/person.approve; the
/// EdgeExists/EdgeActive/SubjectRegistered studs re-check under the lock).
pub struct UboEdgePierceNominee;

#[async_trait]
impl SemOsVerbOp for UboEdgePierceNominee {
    fn fqn(&self) -> &str {
        "ubo.edge.pierce-nominee"
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let edge = EdgeId(json_extract_uuid(args, ctx, "edge-id")?);
        let nominator = json_extract_uuid(args, ctx, "nominator-id")?;
        let payload = normalize_pierce_nominee_payload(args, edge, nominator)?;

        // Op-layer fail-closed kind check (§2.6 checklist note): no
        // precondition primitive expresses "edge is of kind X", so the
        // nominee-kind verification lives here. Pre-append fold — same
        // accepted small race window as freeze's pre-fold; existence/
        // activeness are ALSO re-checked under the lock by the declared
        // EdgeExists/EdgeActive studs.
        let events = PgKycEventStore::load_events(scope.executor(), subject)
            .await
            .map_err(|e| anyhow!("pierce-nominee: load events failed: {e}"))?;
        let refs: Vec<&ob_poc_kyc_substrate::IntentEvent> = events.iter().collect();
        let control = fold_control_versioned(&refs, &KYC_REGISTRY)
            .map_err(|e| anyhow!("pierce-nominee: control fold failed: {e}"))?;
        match control.edges.get(&edge) {
            None => {
                return Err(anyhow!(
                    "ubo.edge.pierce-nominee: target edge {} not found in the control graph \
                     (EdgeExists)",
                    edge.0
                ));
            }
            Some(e) if !matches!(e.kind, EdgeKind::Nominee) => {
                return Err(anyhow!(
                    "ubo.edge.pierce-nominee: target edge {} is not a nominee edge \
                     (kind {:?}) — only EdgeKind::Nominee arrangements can be pierced \
                     (K-8, fail-closed)",
                    edge.0,
                    e.kind
                ));
            }
            Some(_) => {}
        }

        let outcome = stream_append(
            "ubo.edge.pierce-nominee",
            subject,
            TargetBinding::for_edge(subject, edge),
            payload,
            "senior-analyst.pierce-nominee",
            Some("ubo.edge.pierce-nominee"),
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(json!({
            "pierced_edge_id": edge.0,
            "seq": outcome.seq,
        })))
    }
}

/// Normalize `ubo.edge.pierce-nominee`'s payload (TS.4, §2.6) — fail-closed
/// duties, mirroring `normalize_assert_control_payload`'s §1b discipline:
///
/// 1. **`kind` must be a member of the canonical wire set AND not
///    `nominee`:** the arg is the UNDERLYING kind the nominator actually
///    holds; a pierce can never produce another nominee edge (fail-closed —
///    that would just relocate the K-8 problem). Unknown/absent kinds are
///    rejected before append, exactly as assert-control's normalizer does.
/// 2. **Kebab→snake:** `edge-id`→`edge_id` (the TARGET nominee edge — the
///    fold reads `target.edge_id`, but the payload copy keeps the event
///    self-describing), `nominator-id`→`nominator_entity_id` (read by the
///    fold arm), `trust-revocable`→`trust_revocable` (meaningful if the
///    underlying kind is `trust_settlor`).
/// 3. **Provenance stamp:** `pierced_from` = the target nominee edge id
///    (§2.6 — carried on the event payload; the fold also records it on the
///    new `EdgeState`).
fn normalize_pierce_nominee_payload(
    args: &serde_json::Value,
    edge: EdgeId,
    nominator: Uuid,
) -> Result<serde_json::Value> {
    let underlying_values = || {
        EDGE_KIND_WIRE_VALUES
            .iter()
            .filter(|k| **k != "nominee")
            .copied()
            .collect::<Vec<_>>()
            .join(", ")
    };
    match args.get("kind").and_then(|v| v.as_str()) {
        Some("nominee") => {
            return Err(anyhow!(
                "ubo.edge.pierce-nominee: kind 'nominee' rejected — a pierce cannot produce \
                 another nominee edge (K-8, fail-closed); pass the UNDERLYING kind the \
                 nominator actually holds. Valid wire values: {}",
                underlying_values()
            ));
        }
        Some(kind) if EDGE_KIND_WIRE_VALUES.contains(&kind) => {}
        Some(unknown) => {
            return Err(anyhow!(
                "ubo.edge.pierce-nominee: unrecognized kind '{unknown}' — rejected fail-closed \
                 (TS.1 §1b wire-normalizer discipline). Valid wire values: {}",
                underlying_values()
            ));
        }
        None => {
            return Err(anyhow!(
                "ubo.edge.pierce-nominee: kind is required (the UNDERLYING kind the nominator \
                 actually holds) — rejected fail-closed. Valid wire values: {}",
                underlying_values()
            ));
        }
    }
    let mut p = args.clone();
    if let Some(obj) = p.as_object_mut() {
        obj.remove("edge-id");
        obj.insert(
            "edge_id".to_string(),
            serde_json::Value::String(edge.0.to_string()),
        );
        obj.remove("nominator-id");
        obj.insert(
            "nominator_entity_id".to_string(),
            serde_json::Value::String(nominator.to_string()),
        );
        if let Some(v) = obj.remove("trust-revocable") {
            obj.insert("trust_revocable".to_string(), v);
        }
        obj.insert(
            "pierced_from".to_string(),
            serde_json::Value::String(edge.0.to_string()),
        );
    }
    Ok(p)
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
        // T6.2 row 5: SubjectRegistered now attached — see the supersede
        // comment above for why `Some(fqn)` (not `None`) is required.
        let outcome = stream_append(
            "ubo.edge.reconcile-conflict",
            subject,
            TargetBinding::for_subject(subject),
            args.clone(),
            "analyst.reconcile-conflict",
            Some("ubo.edge.reconcile-conflict"),
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
        //    `Some(fqn)` — T6.1(c): select-strategy now carries a real
        //    precondition (`StructureClassSupported`, the 6a exemplar). Before
        //    this it had none, so `None` here was harmless; leaving it `None`
        //    now would silently never enforce the fail-closed guard at the
        //    real write path (same defect class as freeze's dead-precondition
        //    bug fixed in EOP-DD-KYCUBO-003 — a declared-but-unwired check).
        let outcome = stream_append(
            "ubo.determination.select-strategy",
            subject,
            TargetBinding::for_subject(subject),
            args.clone(),
            "analyst.select-strategy",
            Some("ubo.determination.select-strategy"),
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

        // T6-tooth fix (2026-08-12, EOP-PLAN-KYCUBO-KIT-001 Part A): compute-fold's
        // lexicon entry declares [ReconciledProjection, StrategySelected] (K-14),
        // but this op is a pure read/projection with NO append call — it never
        // routed through `stream_append`'s `validate_entry_fqn`, so the declared
        // precondition was dead: the placement/preview board (which calls
        // `check_preconditions` directly over every lexicon entry) already
        // enforced it, but the real invocation path did not. Same defect family
        // as freeze's pre-DD-003 dead precondition and the pre-T6.1(c)
        // select-strategy gap — found by the new `every_precondition_carrying_
        // verb_is_reached_by_the_checker` closure tooth (A2). Both declared
        // preconditions read only `ControlState`, so `check_control_preconditions`
        // (no obligation fold needed) is sufficient; the probe event mirrors the
        // pattern `placement.rs`/`preview.rs` already use for precondition-only
        // (no-append) evaluation.
        let lexicon = phase1_lexicon();
        let entry = lexicon
            .get("ubo.determination.compute-fold")
            .ok_or_else(|| anyhow!("ubo.determination.compute-fold missing from lexicon"))?;
        let probe = ob_poc_kyc_substrate::IntentEvent::new(
            subject,
            "ubo.determination.compute-fold",
            map_principal(&ctx.principal),
            AuthorityRef("compute-fold.precondition-probe".into()),
            TargetBinding::for_subject(subject),
            serde_json::Value::Null,
            ctx.as_of,
        );
        check_control_preconditions(entry, &state, &probe)
            .map_err(|e| anyhow!("ubo.determination.compute-fold precondition failed: {e}"))?;

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
        // T6.3 row 7 finding: `validate_entry_fqn` was `None`, so the
        // lexicon's declared ReconciledProjection/StrategySelected
        // preconditions were dead at the real write path — same defect
        // class as freeze's pre-DD-003 dead precondition.
        let outcome = stream_append(
            "ubo.determination.apply-smo-fallback",
            subject,
            TargetBinding::for_subject(subject),
            payload,
            "analyst.smo-fallback",
            Some("ubo.determination.apply-smo-fallback"),
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "seq": outcome.seq }),
        ))
    }
}

/// Normalize the edge-identity payload key for `ubo.edge.assert-control` /
/// `ubo.edge.assert-economic-interest` (T6.2, found while wiring the
/// `EdgeExists`/`EdgeActive` studs onto `attach-evidence`/`supersede`): the
/// caller-facing arg is kebab-case `edge-id`, but the fold's
/// `edge_id_from_payload` only recognises snake_case `edge_id`
/// (`fold::control::edge_id_from_payload`) — without this, a caller-supplied
/// `edge-id` was silently ignored by the fold, which fell back to its own
/// deterministic `Uuid::new_v5` hash of `(from, to, kind)` as the edge's real
/// key. `attach-evidence`/`verify`/`supersede` all require an explicit
/// `edge-id` arg (`json_extract_uuid`, not `_opt`) and address the edge via
/// `TargetBinding::for_edge(subject, edge)` using exactly that caller-supplied
/// id — so the two ends of the addressing scheme never actually agreed on
/// the edge's identity. Harmless while no precondition read `state.edges` by
/// id; live-breaking now that `EdgeExists`/`EdgeActive` do (T6.2 rows 3/4) —
/// same bug class as R3 (`structure_class`)/`cbu_role`/`smo_person_id` above.
/// Stamps the OP's resolved `edge` (caller-supplied-or-fresh) as `edge_id` in
/// the payload, so the fold's key and the op's own `target`/`Record` output
/// are always the same id, by construction.
fn normalize_edge_id_payload(args: &serde_json::Value, edge: EdgeId) -> serde_json::Value {
    let mut p = args.clone();
    if let Some(obj) = p.as_object_mut() {
        obj.remove("edge-id");
        obj.insert(
            "edge_id".to_string(),
            serde_json::Value::String(edge.0.to_string()),
        );
    }
    p
}

/// Normalize `ubo.edge.assert-control`'s payload (TS.1, EOP-DD-KYCUBO-KIT-TS0
/// §1b + §2.1) — three duties, fail-closed:
///
/// 1. **Kill the silent catch-all:** the `kind` string must be present AND a
///    member of the canonical wire set (`EDGE_KIND_WIRE_VALUES`, the single
///    source of truth kept in lockstep with the fold's
///    `edge_kind_from_payload` arms). Anything else — a typo, the old
///    un-mapped `trust_role`, or an omitted key — previously folded silently
///    to `DominantInfluence` (the exact "silently-wrong" defect class of
///    R3/M4). The fold's own catch-all REMAINS (total dispatch for
///    historical events); only the append path rejects.
/// 2. **Kebab→snake for `trust-revocable`:** the YAML arg is kebab-case, the
///    fold reads snake_case `trust_revocable` (the edge-id kebab/snake
///    defect class, T6.2) — meaningful on `trust_settlor` edges only; see
///    `EdgeState::trust_revocable` for the fail-closed polarity.
/// 3. The existing `edge_id` stamping (`normalize_edge_id_payload`).
fn normalize_assert_control_payload(
    args: &serde_json::Value,
    edge: EdgeId,
) -> Result<serde_json::Value> {
    match args.get("kind").and_then(|v| v.as_str()) {
        Some(kind) if EDGE_KIND_WIRE_VALUES.contains(&kind) => {}
        Some(unknown) => {
            return Err(anyhow!(
                "ubo.edge.assert-control: unrecognized kind '{unknown}' — rejected fail-closed \
                 (TS.1 §1b; an unknown kind previously collapsed silently to \
                 dominant_influence). Valid wire values: {}",
                EDGE_KIND_WIRE_VALUES.join(", ")
            ));
        }
        None => {
            return Err(anyhow!(
                "ubo.edge.assert-control: kind is required — rejected fail-closed (TS.1 §1b; \
                 an absent kind previously collapsed silently to dominant_influence). \
                 Valid wire values: {}",
                EDGE_KIND_WIRE_VALUES.join(", ")
            ));
        }
    }
    let mut p = normalize_edge_id_payload(args, edge);
    if let Some(obj) = p.as_object_mut() {
        if let Some(v) = obj.remove("trust-revocable") {
            obj.insert("trust_revocable".to_string(), v);
        }
    }
    Ok(p)
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
        // TS.4 (§2.6) fail-closed guard, BEFORE dispatch (so the dispatch arm
        // below stays a plain `"..." => &Strategy` expression the closure
        // tooth's arm scanner recognises): `resolve()` returns
        // `Vec<ProngCandidate>` and cannot signal error, so the
        // unpierced-nominee scan lives here. Freezing while an unpierced
        // nominee edge is active would either attribute control to the
        // nominee (the exact wrong answer K-8 exists to prevent) or silently
        // drop the arrangement — hard-error instead.
        if strategy_name == "nominee_pierce_strategy" {
            let unpierced: Vec<String> = control
                .edges
                .values()
                .filter(|e| e.is_active() && matches!(e.kind, EdgeKind::Nominee))
                .map(|e| e.id.0.to_string())
                .collect();
            if !unpierced.is_empty() {
                return Err(anyhow!(
                    "freeze: nominee_pierce_strategy selected but unpierced nominee edge(s) \
                     remain active: [{}] — pierce each via ubo.edge.pierce-nominee before \
                     freezing (K-8, fail-closed)",
                    unpierced.join(", ")
                ));
            }
        }
        let strategy: &dyn DeterminationStrategy = match strategy_name {
            "ownership_prong_strategy" => &OwnershipProngStrategy,
            // M4: control-by-other-means (voting rights, board appointment, GP
            // statutory control, LLP designated member, trust roles, dominant
            // influence). Scope note lives on ControlProngStrategy itself — v1
            // walks control-kind edges only, does not cross into the economic
            // axis for an intermediate controlling entity's own UBOs.
            "control_prong_strategy" => &ControlProngStrategy,
            // TS.1: trust control follows role (trustee/protector always;
            // settlor unless proven irrevocable; beneficiary never). Scope
            // note lives on TrustRoleStrategy itself.
            "trust_role_strategy" => &TrustRoleStrategy,
            // TS.2: fund control sits with the manager (ManCo/AIFM/GP-analog),
            // asserted as dominant_influence/board_appointment — a NAMED thin
            // delegate to the control-prong traversal so the freeze pin
            // records WHICH model ran. Scope note lives on FundControlStrategy.
            "fund_control_strategy" => &FundControlStrategy,
            // TS.2: a foundation has no owners by construction — control sits
            // with the council/board. Traverses board_appointment +
            // dominant_influence ONLY. Scope note lives on
            // FoundationCouncilStrategy.
            "foundation_council_strategy" => &FoundationCouncilStrategy,
            // TS.3: the controller of a state-owned entity is a state organ
            // — the strategy runs the full control walk for the rare genuine
            // natural-person controller; zero candidates legitimizes the
            // existing apply-smo-fallback route. Scope note lives on
            // StateOwnedStrategy.
            "state_owned_strategy" => &StateOwnedStrategy,
            // TS.3: one-member-one-vote — membership never yields a UBO;
            // control arises from office. Traverses voting_rights +
            // board_appointment + dominant_influence ONLY. Scope note lives
            // on CooperativeMemberStrategy.
            "cooperative_member_strategy" => &CooperativeMemberStrategy,
            // TS.4: post-piercing the subject resolves by the UNDERLYING
            // structure — a thin delegate to the control-prong traversal;
            // the unpierced-nominee fail-closed guard runs above, before
            // this dispatch. Scope note lives on NomineePierceStrategy.
            "nominee_pierce_strategy" => &NomineePierceStrategy,
            other => {
                return Err(anyhow!(
                    "freeze: strategy '{other}' selected but no DeterminationStrategy is \
                     registered for it — only ownership_prong_strategy, \
                     control_prong_strategy, trust_role_strategy, \
                     fund_control_strategy, foundation_council_strategy, \
                     state_owned_strategy, cooperative_member_strategy, and \
                     nominee_pierce_strategy exist today"
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
        // T6 row 9 CLOSED (2026-08-17, corrects EOP-DD-KYCUBO-KIT-T6 §5 —
        // see the lexicon entry's own comment): `NotAlreadyRegistered` is
        // now keyed off `entity_id`, not the bare per-subject `registered`
        // bool, so it no longer conflicts with this verb's real
        // multi-call-per-stream usage (one call per entity under a shared
        // subject_root).
        let outcome = stream_append(
            "kyc.subject.register",
            subject,
            TargetBinding::for_subject(subject),
            payload,
            "analyst.register",
            Some("kyc.subject.register"),
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
        // T6.3 row 10 finding: `validate_entry_fqn` was `None`, so the
        // newly-declared SubjectRegistered precondition would be dead at
        // the real write path without this wire.
        let outcome = stream_append(
            "kyc.subject.classify-structure",
            subject,
            TargetBinding::for_subject(subject),
            payload,
            "analyst.classify-structure",
            Some("kyc.subject.classify-structure"),
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

// KycRoleAssign / KycRoleWithdraw retired 2026-08-12 (T0.3 K-G7 fold-blind
// write — see dsl-kyc-obligation.yaml's retirement comment for the full
// rationale; reintroduction only via T6, precondition attached from birth).

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
        // T6.4 row 11 finding: `validate_entry_fqn` was `None` — the
        // SubjectRegistered precondition (the motivating cross-fold case
        // for the T6.1 unified checker) was dead at the real write path.
        let outcome = stream_append(
            "kyc.obligation.create",
            subject,
            TargetBinding::for_subject(subject),
            payload,
            "analyst.obligation-create",
            Some("kyc.obligation.create"),
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
        // T6.4 row 12 finding: `validate_entry_fqn` was `None` —
        // ObligationExists / SubjectNotDecided were dead at the real write
        // path.
        let outcome = stream_append(
            "kyc.obligation.update-identity",
            subject,
            TargetBinding::for_subject(subject),
            normalize_obligation_payload(args),
            "analyst.obligation-update",
            Some("kyc.obligation.update-identity"),
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
        // T6.4 row 13 finding: `validate_entry_fqn` was `None` —
        // ObligationExists / SubjectNotDecided were dead at the real write
        // path.
        let outcome = stream_append(
            "kyc.obligation.update-screening",
            subject,
            TargetBinding::for_subject(subject),
            normalize_obligation_payload(args),
            "analyst.obligation-update",
            Some("kyc.obligation.update-screening"),
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
        // T6.4 row 14 finding: `validate_entry_fqn` was `None` —
        // ObligationExists / SubjectNotDecided were dead at the real write
        // path.
        let outcome = stream_append(
            "kyc.obligation.update-risk",
            subject,
            TargetBinding::for_subject(subject),
            normalize_obligation_payload(args),
            "analyst.obligation-update",
            Some("kyc.obligation.update-risk"),
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
        // T6.4 row 15 finding: `validate_entry_fqn` was `None` —
        // ObligationExists / SubjectNotDecided were dead at the real write
        // path.
        let outcome = stream_append(
            "kyc.obligation.satisfy",
            subject,
            TargetBinding::for_subject(subject),
            normalize_obligation_payload(args),
            "analyst.obligation-satisfy",
            Some("kyc.obligation.satisfy"),
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
        // T6.4 row 16 finding: `validate_entry_fqn` was `None` —
        // ObligationExists / SubjectNotDecided were dead at the real write
        // path.
        let outcome = stream_append(
            "kyc.obligation.waive",
            subject,
            TargetBinding::for_subject(subject),
            normalize_obligation_payload(args),
            "analyst.obligation-waive",
            Some("kyc.obligation.waive"),
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

        // T6.4 row 17 finding: `validate_entry_fqn` was `None` — the K-23
        // gate (SubjectAllTerminal / SubjectNotDecided) was declared in the
        // lexicon but never reached by the checker at the real op call
        // site; the hand-rolled fold above already enforces the same gate
        // (kept as-is), this wires the declared precondition too so the
        // checker is the single enforced source of truth, not just this
        // op's bespoke pre-check.
        let outcome = stream_append(
            "kyc.person.approve",
            subject,
            TargetBinding::for_subject(subject),
            args.clone(),
            "senior-analyst.approve",
            Some("kyc.person.approve"),
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
        // T6.4 row 18 finding: `validate_entry_fqn` was `None` —
        // SubjectNotDecided was dead at the real write path.
        let outcome = stream_append(
            "kyc.person.reject",
            subject,
            TargetBinding::for_subject(subject),
            args.clone(),
            "senior-analyst.reject",
            Some("kyc.person.reject"),
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "seq": outcome.seq }),
        ))
    }
}
