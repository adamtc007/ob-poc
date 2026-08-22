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
use ob_poc_kyc_substrate::{
    check_preconditions, edges_invalidated_by_correction,
    entity_type_from_wire, find_subject_entity, fold_control_versioned, fold_obligations_versioned,
    fold_type_registry, natural_persons_from_events, assembly_lexicon, pipe_of,
    render_intent_event_to_sexpr, AuthorityRef, ControlProngStrategy, DeterminationStrategy,
    CooperativeMemberStrategy, EdgeId, EdgeKind, EntityId, FoldRegistry, FoundationCouncilStrategy,
    FundControlStrategy, NomineePierceStrategy, OwnershipProngStrategy, PersonId,
    ProngCandidate, SmoResult, StateOwnedStrategy, SubjectId, TargetBinding,
    TrustRoleStrategy, V1FoldImpl,
    EDGE_KIND_WIRE_VALUES, ENTITY_TYPE_WIRE_VALUES, STRUCTURE_CLASS_WIRE_VALUES,
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
    let lexicon = assembly_lexicon();
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

    append_in_scope(scope, &KYC_REGISTRY, &event, &source_text, |control, obligation, type_registry| {
        if let Some(e) = entry {
            check_preconditions(e, control, obligation, type_registry, &event)?;
        }
        Ok(())
    })
    .await
    .map_err(|e| anyhow!("{verb_fqn} stream append failed: {e}"))
}

/// K-23 "decision is final" — refuse if `subject` already has a
/// `kyc_ubo.decide.subject.approve`/`kyc_ubo.decide.subject.reject` record.
///
/// `Precondition::SubjectNotDecided` was live-enforced (T6.4) for
/// `kyc.obligation.{update-identity,update-screening,update-risk,satisfy,
/// waive}` — a subject could not have its obligations touched once decided.
/// Retired from the substrate TS.6 P2 (`kyc_ubo.decide.subject.approve`/`kyc_ubo.decide.subject.reject` no
/// longer append to the fact stream, so the fold can never see a decision to
/// check against) — re-homed here, onto `kyc_decision_records` directly,
/// preserving the same guarantee for these 5 fact-stream verbs exactly as
/// `ob-poc-kyc-decide`'s own finality check does for `kyc_ubo.decide.subject.approve`/
/// `kyc_ubo.decide.subject.reject` themselves.
async fn refuse_if_subject_already_decided(
    scope: &mut dyn TransactionScope,
    subject: SubjectId,
    verb_fqn: &str,
) -> Result<()> {
    let existing = sqlx::query_scalar::<_, String>(
        r#"SELECT verb_fqn FROM "ob-poc".kyc_decision_records
           WHERE subject_root = $1 AND verb_fqn IN ('kyc_ubo.decide.subject.approve', 'kyc_ubo.decide.subject.reject')
           LIMIT 1"#,
    )
    .bind(subject.0)
    .fetch_optional(scope.executor())
    .await
    .map_err(|e| anyhow!("{verb_fqn}: finality check failed: {e}"))?;

    if let Some(prior_fqn) = existing {
        return Err(anyhow!(
            "{verb_fqn} rejected: subject has already been decided ({prior_fqn}); \
             the decision is final (K-23)"
        ));
    }
    Ok(())
}

use super::helpers::{
    json_extract_string, json_extract_string_opt, json_extract_uuid, json_extract_uuid_opt,
};

/// The KYC fold registry (v1) — maps the phase-1 lexicon hash to its `FoldImpl`.
/// Module-level for now; becomes an injected platform service when fold
/// version-dispatch (D2) needs more than one registered version.
static KYC_REGISTRY: LazyLock<FoldRegistry> = LazyLock::new(|| {
    let mut registry = FoldRegistry::new();
    registry.register(assembly_lexicon().hash, Arc::new(V1FoldImpl));
    registry
});

// ── Edge lifecycle verbs ──────────────────────────────────────────────────────

/// `kyc_ubo.assert.edge.control` — claim a control edge (voting, board, GP statutory,
/// …). The first stream-backed determination verb; the pattern every other
/// `dsl.kyc` verb follows.
pub struct UboEdgeAssertControl;

#[async_trait]
impl SemOsVerbOp for UboEdgeAssertControl {
    fn fqn(&self) -> &str {
        "kyc_ubo.assert.edge.control"
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

        // TS.6 P2 (K-G7): `pierced-from` present means this call is the
        // first half of the `kyc_ubo.assert.edge.nominee-piercing` macro composition
        // (config/verb_schemas/macros/ubo.yaml), which replaced the retired
        // standalone verb. Op-layer fail-closed check (no precondition
        // primitive expresses "edge is of kind X" — the same discipline the
        // retired bespoke op used): the referenced edge must exist, be
        // `EdgeKind::Nominee`, and be active. Cheap no-op on the common
        // non-piercing path (no fold read at all unless the arg is present).
        if let Some(pierced_from) = json_extract_uuid_opt(args, ctx, "pierced-from").map(EdgeId) {
            let events = PgKycEventStore::load_events(scope.executor(), subject)
                .await
                .map_err(|e| anyhow!("kyc_ubo.assert.edge.control: load events failed: {e}"))?;
            let refs: Vec<&ob_poc_kyc_substrate::IntentEvent> = events.iter().collect();
            let control = fold_control_versioned(&refs, &KYC_REGISTRY)
                .map_err(|e| anyhow!("kyc_ubo.assert.edge.control: control fold failed: {e}"))?;
            match control.edges.get(&pierced_from) {
                None => {
                    return Err(anyhow!(
                        "kyc_ubo.assert.edge.control: pierced-from edge {} not found in the \
                         control graph (EdgeExists)",
                        pierced_from.0
                    ));
                }
                Some(e) if !matches!(e.kind, EdgeKind::Nominee) => {
                    return Err(anyhow!(
                        "kyc_ubo.assert.edge.control: pierced-from edge {} is not a nominee edge \
                         (kind {:?}) — only EdgeKind::Nominee arrangements can be pierced \
                         (K-8, fail-closed)",
                        pierced_from.0,
                        e.kind
                    ));
                }
                Some(e) if !e.is_active() => {
                    return Err(anyhow!(
                        "kyc_ubo.assert.edge.control: pierced-from edge {} is not active \
                         (EdgeActive)",
                        pierced_from.0
                    ));
                }
                Some(_) => {}
            }
        }

        let lexicon = assembly_lexicon();
        let entry = lexicon
            .get("kyc_ubo.assert.edge.control")
            .ok_or_else(|| anyhow!("kyc_ubo.assert.edge.control missing from lexicon"))?;

        // The verb args ARE the event payload (from_entity_id, to_entity_id,
        // edge_kind, percentage, …) — the fold reads them.
        let event = IntentEventDraft {
            verb_fqn: "kyc_ubo.assert.edge.control".into(),
            subject_root: subject,
            target: TargetBinding::for_edge(subject, edge),
            payload: normalize_assert_control_payload(args, edge)?,
            authority: AuthorityRef("analyst.assert-control".into()),
            lexicon_hash: lexicon.hash,
            as_of: ctx.as_of, // frozen at verb entry — never now() here
        }
        .into_event(&ctx.principal, ctx.correlation_id, ctx.execution_id);
        let source_text = render_intent_event_to_sexpr(&event, Some(entry));

        let outcome = append_in_scope(scope, &KYC_REGISTRY, &event, &source_text, |control, obligation, type_registry| {
            check_preconditions(entry, control, obligation, type_registry, &event)
        })
        .await
        .map_err(|e| anyhow!("kyc_ubo.assert.edge.control append failed: {e}"))?;

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
        "kyc_ubo.assert.edge.economic-interest"
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
            "kyc_ubo.assert.edge.economic-interest",
            subject,
            TargetBinding::for_edge(subject, edge),
            normalize_edge_id_payload(args, edge),
            "analyst.assert-economic-interest",
            Some("kyc_ubo.assert.edge.economic-interest"),
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
        "kyc_ubo.assert.edge.evidence"
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
            "kyc_ubo.assert.edge.evidence",
            subject,
            TargetBinding::for_edge(subject, edge),
            args.clone(),
            "analyst.attach-evidence",
            Some("kyc_ubo.assert.edge.evidence"),
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
        "kyc_ubo.assert.edge.verification"
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
            "kyc_ubo.assert.edge.verification",
            subject,
            TargetBinding::for_edge(subject, edge),
            args.clone(),
            "analyst.verify",
            Some("kyc_ubo.assert.edge.verification"),
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
        "kyc_ubo.assert.edge.supersession"
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
            "kyc_ubo.assert.edge.supersession",
            subject,
            TargetBinding::for_edge(subject, edge),
            args.clone(),
            "analyst.supersede",
            Some("kyc_ubo.assert.edge.supersession"),
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "seq": outcome.seq }),
        ))
    }
}

// `UboEdgePierceNominee` / `kyc_ubo.assert.edge.nominee-piercing` RETIRED (TS.6 P2, K-G7,
// 2026-08-22): the bespoke op's two governed effects — supersede the target
// nominee edge (K-13) + assert the disclosed nominator's real edge with
// `pierced_from` provenance (K-8) — are now two ordinary verb calls composed
// by the `kyc_ubo.assert.edge.nominee-piercing` MACRO (config/verb_schemas/macros/
// ubo.yaml): `kyc_ubo.assert.edge.control` (extended with an optional
// `pierced-from` arg — see its op-layer fail-closed check above and
// `normalize_assert_control_payload` below) followed by `kyc_ubo.assert.edge.supersession`
// (unchanged). Piercing records a real handed-over fact, so it wasn't
// deleted outright (unlike select-strategy/compute-fold) — it was
// redesigned because the old shape was a single-purpose verb doing two
// governed effects in one hand-rolled fold arm, with no reusable
// composition mechanism; the macro is executed atomically under the
// Sequencer's one-scope-per-runbook model, same as any other multi-step
// macro, so the two effects still commit or roll back together.

pub struct UboEdgeReconcileConflict;

#[async_trait]
impl SemOsVerbOp for UboEdgeReconcileConflict {
    fn fqn(&self) -> &str {
        "kyc_ubo.assert.edge.reconciliation"
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
            "kyc_ubo.assert.edge.reconciliation",
            subject,
            TargetBinding::for_subject(subject),
            args.clone(),
            "analyst.reconcile-conflict",
            Some("kyc_ubo.assert.edge.reconciliation"),
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

// `UboDeterminationSelectStrategy` / `ubo.determination.select-strategy`
// RETIRED (TS.6 P2, K-G7): the strategy is now DERIVED from
// `structure_class` (`strategy_for_structure_class`), never separately
// asserted. Reintroduction path: only if a structure class is ever ratified
// with more than one legitimate strategy to choose between — at that point
// the verb would need to record which one was picked, which today's
// total 1:1 class→strategy mapping makes unnecessary.

// `UboDeterminationComputeFold` / `ubo.determination.compute-fold` RETIRED
// (TS.6 P2, K-G7): a pure read (fold the stream, return a summary, append
// nothing) that carried freeze's own precondition pair
// (`ReconciledProjection`, `StructureClassSupported`) only to demonstrate
// it pre-freeze. `freeze` already declares the identical pair
// independently, so nothing needed building to "preserve the gate
// elsewhere" — see the lexicon.rs comment at this verb's former entry.

// TS.6 §5 (K-G7, RATIFIED): `UboDeterminationApplySmoFallback` RETIRED
// 2026-08-22 with its verb. SMO is PULLED on exhaustion by
// `determination.rs` (OfficerAppointment edges -> Prong::SmoFallback,
// straight into `candidates`, TS.3 §4a) — this op was a second, manual
// way to write an answer the traversal already computes, and could
// contradict it. The 54 historical events under this verb_fqn fall
// through to the fold's catch-all `_ => {}` (replay-faithful; none of
// those 54 subjects has a freeze, so no determination changes).

/// Normalize the edge-identity payload key for `kyc_ubo.assert.edge.control` /
/// `kyc_ubo.assert.edge.economic-interest` (T6.2, found while wiring the
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

/// Normalize `kyc_ubo.assert.edge.control`'s payload (TS.1, EOP-DD-KYCUBO-KIT-TS0
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
    let pierced = args.get("pierced-from").and_then(|v| v.as_str()).is_some();
    match args.get("kind").and_then(|v| v.as_str()) {
        Some("nominee") if pierced => {
            return Err(anyhow!(
                "kyc_ubo.assert.edge.control: a pierce cannot produce another nominee edge \
                 (K-8, fail-closed) — `kind` must be the UNDERLYING kind the nominator \
                 actually holds"
            ));
        }
        Some(kind) if EDGE_KIND_WIRE_VALUES.contains(&kind) => {}
        Some(unknown) => {
            return Err(anyhow!(
                "kyc_ubo.assert.edge.control: unrecognized kind '{unknown}' — rejected fail-closed \
                 (TS.1 §1b; an unknown kind previously collapsed silently to \
                 dominant_influence). Valid wire values: {}",
                EDGE_KIND_WIRE_VALUES.join(", ")
            ));
        }
        None => {
            return Err(anyhow!(
                "kyc_ubo.assert.edge.control: kind is required — rejected fail-closed (TS.1 §1b; \
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
        // TS.6 P2 (K-G7): stamps the fold-read key (`pierced_from`) from the
        // caller-facing kebab arg — the `kyc_ubo.assert.edge.nominee-piercing` macro's
        // provenance pointer at the nominee edge this assertion pierces.
        if let Some(v) = obj.remove("pierced-from") {
            obj.insert("pierced_from".to_string(), v);
        }
    }
    Ok(p)
}
// TS.6 §5: `normalize_smo_fallback_payload` deleted 2026-08-22 with
// `ubo.determination.apply-smo-fallback`. It fixed a real kebab/snake
// payload-key mismatch (`smo-person-id` vs `smo_person_id`) that had
// left `ControlState.smo_person_id` silently `None` — the same defect
// class as R3 (structure_class) and assert-control's kind/edge_kind.
// With the verb retired there is no longer any writer for that field,
// so the normalizer has nothing to normalize.

pub struct UboDeterminationFreeze;

#[async_trait]
impl SemOsVerbOp for UboDeterminationFreeze {
    fn fqn(&self) -> &str {
        "kyc_ubo.decide.determination.freeze"
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
        let type_registry = fold_type_registry(&refs);

        // 2. Run the actual determination strategy (EOP-DD-KYCUBO-003 R1/M1.2).
        //    TS.6 P2: `select-strategy` is retired — the strategy is DERIVED
        //    from `structure_class` (`strategy_for_structure_class`), never
        //    separately asserted. `StructureClassSupported` (gating
        //    compute-fold/freeze already) guarantees the class is a member
        //    of the pinned implemented-strategy set before we get here.
        let structure_class = control.structure_class.as_ref().ok_or_else(|| {
            anyhow!("freeze: no structure class set (StructureClassSupported precondition)")
        })?;
        let strategy_name = ob_poc_kyc_substrate::strategy_for_structure_class(structure_class);
        // TS.4 §3 Ruling B (widened from §2.6's original nominee_pierce_
        // strategy-only scope): `resolve()` returns `Vec<ProngCandidate>`
        // and cannot signal error, so the unpierced-nominee scan lives
        // here, BEFORE dispatch (so the dispatch arm below stays a plain
        // `"..." => &Strategy` expression the closure tooth's arm scanner
        // recognises). UNCONDITIONAL as of Ruling B: a nominee sitting
        // mid-chain inside a fund, trust, or corporate structure is
        // reachable under ANY strategy, not only when the subject itself
        // classified as `Nominee` — freezing while ANY active unpierced
        // nominee edge exists anywhere in the graph would either attribute
        // control to the nominee (the exact wrong answer K-8 exists to
        // prevent) or silently drop the arrangement — hard-error instead,
        // regardless of which strategy actually ran.
        let unpierced: Vec<String> = ob_poc_kyc_substrate::unpierced_nominee_edges(&control)
            .into_iter()
            .map(|id| id.0.to_string())
            .collect();
        if !unpierced.is_empty() {
            return Err(anyhow!(
                "freeze: unpierced nominee edge(s) remain active: [{}] — pierce each via \
                 kyc_ubo.assert.edge.nominee-piercing before freezing (TS.4 §3 Ruling B, K-8, fail-closed)",
                unpierced.join(", ")
            ));
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
                "freeze: no kyc_ubo.assert.subject.structure-class event found — subject entity unknown"
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

        // TS.3 §3: record any statutory-authority stop at the subject —
        // traversal already excludes it (control_admission ==
        // ControlAdmission::Stop), this makes the halt non-silent.
        let stops = ob_poc_kyc_substrate::detect_statutory_stops(&control, subject_entity_id);

        // TS.3 §4a: pull the officer/SMO population on exhaustion — fires
        // ONLY when ownership+control produced nothing
        // (`officers_contribute_only_on_exhaustion`); never pushed by edge
        // admission. Folds directly into `candidates`; the manual
        // `apply-smo-fallback` route (below) remains the K-5 escape hatch
        // when nothing is found to pull.
        let smo_pull = match ob_poc_kyc_substrate::pull_smo_on_exhaustion(
            &control,
            subject_entity_id,
            &natural_persons,
            &candidates,
        ) {
            Some((pulled, record)) => {
                candidates.extend(pulled);
                candidates.sort_by_key(|c| c.person_id.0);
                Some(record)
            }
            None => None,
        };

        // TS.4 §3 Ruling B: tag every candidate with the pierces (if any)
        // followed along ITS OWN chain — universal, regardless of which
        // strategy resolved it (`nominee_pierce_strategy` needs no special
        // case; the substitution already happened at edge-admission time,
        // this just makes it a visible, recorded part of the answer). Runs
        // AFTER the SMO pull above so pulled candidates are tagged too.
        for c in &mut candidates {
            c.pierces = ob_poc_kyc_substrate::detect_pierces_in_chain(&control, &c.ownership_chain);
        }

        // TS.4 §2 Ruling 2f / CTN-2e: a governing-mandate pivot lacking
        // contract evidence COMPUTES (candidates above already reflect it)
        // but may not FREEZE — "record freely, conclude carefully". Unlike
        // the unpierced-nominee guard above (a hard structural block before
        // dispatch), this is a post-resolve stud: it inspects whatever
        // pivots the strategy actually recorded, so it applies uniformly to
        // any strategy that produces a `pivot` (today: fund_control_strategy
        // only), not just the one dispatched this call.
        let unevidenced_pivots: Vec<String> = candidates
            .iter()
            .filter_map(|c| c.pivot.as_ref())
            .filter(|p| !p.mandate_evidenced)
            .map(|p| p.mandate_edge_id.0.to_string())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        if !unevidenced_pivots.is_empty() {
            return Err(anyhow!(
                "freeze: determination pivoted through governing-mandate edge(s) [{}] with no \
                 contract evidence attached — attach-evidence before freezing (TS.4 §2 Ruling 2f, \
                 CTN-2e: record freely, conclude carefully)",
                unevidenced_pivots.join(", ")
            ));
        }

        // SMO no longer arrives as a separate `smo_result`. `ControlState`'s
        // `smo_person_id`/`smo_event_id` were removed in TS.6 §5 along with
        // `ubo.determination.apply-smo-fallback`, their only writer. SMO now
        // enters through the traversal's pull-on-exhaustion as
        // `Prong::SmoFallback` candidates (TS.3 §4a), which the K-5 guard
        // below counts like any other candidate.
        let smo_result: Option<SmoResult> = None;

        // K-5: a determination must never be silent.
        //
        // TS.6 §5 (2026-08-22): this guard is now strictly STRONGER. It used
        // to be satisfiable by asserting a manual SMO
        // (`ubo.determination.apply-smo-fallback`, retired) — an operator
        // could write an answer to clear the gate. With that verb gone, the
        // only route to a non-empty result is the traversal itself, including
        // TS.3 §4a's automatic pull-on-exhaustion (OfficerAppointment edges
        // into the frontier → `Prong::SmoFallback`, folded into `candidates`).
        // So reaching here means: ownership+control resolved nothing AND the
        // SMO pull found no officer to pull. That is a real dead end, and
        // refusing is correct — there is no longer a way to paper over it.
        if candidates.is_empty() && smo_result.is_none() {
            return Err(anyhow!(
                "freeze: determination would be silent — ownership/control traversal \
                 produced no candidates and the SMO pull-on-exhaustion (TS.3 §4a) found \
                 no officer to pull (K-5). Assert the missing control/ownership facts, or \
                 record an officer appointment for the SMO pull to find; there is no \
                 manual SMO override (ubo.determination.apply-smo-fallback retired TS.6 §5)"
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

        // TS.3 §2a: the assurance surface — empty means fully proved; a
        // non-empty reason set never blocks the freeze (K-5/§2a: a
        // determination runs at any board state), it only labels it.
        let assurance =
            ob_poc_kyc_substrate::compute_assurance(&candidates, &stops, &control, &type_registry);

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
            obj.insert("stops".into(), serde_json::to_value(&stops)?);
            obj.insert("smo_pull".into(), serde_json::to_value(&smo_pull)?);
            obj.insert("assurance".into(), serde_json::to_value(&assurance)?);
        }
        //    `Some(fqn)` re-checks ReconciledProjection + StrategySelected against the
        //    freshly-locked state (K-14) — the declared lexicon preconditions were
        //    previously dead code here (freeze passed `None`, so they never ran).
        let outcome = stream_append(
            "kyc_ubo.decide.determination.freeze",
            subject,
            TargetBinding::for_subject(subject),
            payload,
            "senior-analyst.freeze",
            Some("kyc_ubo.decide.determination.freeze"),
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
            "stops": stops,
            "smo_pull": smo_pull,
            "assurance": assurance,
            "retracted_persons": prior_persons.len().saturating_sub(resolved_persons.len()),
        })))
    }
}

// ── Subject registration verbs ────────────────────────────────────────────────

pub struct KycSubjectRegister;

#[async_trait]
impl SemOsVerbOp for KycSubjectRegister {
    fn fqn(&self) -> &str {
        "kyc_ubo.assert.subject.register"
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
            "kyc_ubo.assert.subject.register",
            subject,
            TargetBinding::for_subject(subject),
            payload,
            "analyst.register",
            Some("kyc_ubo.assert.subject.register"),
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
        "kyc_ubo.assert.subject.structure-class"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let class = json_extract_string(args, "structure-class")?;
        // EOP-FUZZ-KYCUBO-001 §5 finding #2: `structure-class` previously had
        // no op-side fail-closed gate, unlike `kind` (`normalize_assert_control_payload`
        // below) — an unrecognized class silently folded to `structure_class: None`
        // instead of being rejected before append. Same discipline as TS.1 §1b.
        if !STRUCTURE_CLASS_WIRE_VALUES.contains(&class.as_str()) {
            return Err(anyhow!(
                "kyc_ubo.assert.subject.structure-class: unrecognized structure-class '{class}' — \
                 rejected fail-closed (same discipline as kyc_ubo.assert.edge.control's kind \
                 gate; an unknown class previously collapsed silently to \
                 structure_class: None). Valid wire values: {}",
                STRUCTURE_CLASS_WIRE_VALUES.join(", ")
            ));
        }
        let payload = normalize_classify_structure_payload(args, ctx, subject);
        // T6.3 row 10 finding: `validate_entry_fqn` was `None`, so the
        // newly-declared SubjectRegistered precondition would be dead at
        // the real write path without this wire.
        let outcome = stream_append(
            "kyc_ubo.assert.subject.structure-class",
            subject,
            TargetBinding::for_subject(subject),
            payload,
            "analyst.classify-structure",
            Some("kyc_ubo.assert.subject.structure-class"),
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "seq": outcome.seq }),
        ))
    }
}

// ── D1 (EOP-DD-KYCUBO-TS.1 §3) — the four new type-registry moves ──────────

/// `kyc_ubo.assert.subject.type` — TS.1 move 2. Always folds to `Alleged`
/// (CTN-2f); `Proved` is reachable only via a subsequent type-scoped
/// `kyc_ubo.assert.edge.evidence` (`fold/type_registry.rs`).
pub struct KycSubjectAssertType;

#[async_trait]
impl SemOsVerbOp for KycSubjectAssertType {
    fn fqn(&self) -> &str {
        "kyc_ubo.assert.subject.type"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let entity = json_extract_uuid(args, ctx, "entity-id")?;
        let entity_type = json_extract_string(args, "entity-type")?;
        if !ENTITY_TYPE_WIRE_VALUES.contains(&entity_type.as_str()) {
            return Err(anyhow!(
                "kyc_ubo.assert.subject.type: unrecognized entity-type '{entity_type}' — rejected \
                 fail-closed (same discipline as kyc_ubo.assert.edge.control's kind gate). Valid \
                 wire values: {}",
                ENTITY_TYPE_WIRE_VALUES.join(", ")
            ));
        }
        let payload = serde_json::json!({
            "entity_id": entity,
            "entity_type": entity_type,
        });
        let outcome = stream_append(
            "kyc_ubo.assert.subject.type",
            subject,
            TargetBinding { entity_id: Some(EntityId(entity)), ..TargetBinding::for_subject(subject) },
            payload,
            "analyst.assert-type",
            Some("kyc_ubo.assert.subject.type"),
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "seq": outcome.seq }),
        ))
    }
}

/// `kyc_ubo.assert.subject.type-correction` — TS.1 move 7 (§4 cascade). Op-layer duties,
/// no primitive exists for either (same "no Precondition, enforced here,
/// fail-closed" pattern as `kyc_ubo.assert.edge.nominee-piercing`'s nominee-kind check):
///
/// 1. The entity must already carry a prior type assertion — otherwise
///    there is nothing to correct (that is `assert-type`'s job).
/// 2. Compute which active edges touching the entity the geometry matrix no
///    longer permits under the corrected type (`edges_invalidated_by_correction`),
///    classifying each via `pipe_of` (TS.2, pulled forward). An edge whose
///    OTHER end has no recorded type is flagged conservatively — geometry
///    cannot be certified permitted without a definite type on both ends,
///    and TS.1 §4 forbids silently passing an uncertain edge through.
pub struct KycSubjectCorrectType;

#[async_trait]
impl SemOsVerbOp for KycSubjectCorrectType {
    fn fqn(&self) -> &str {
        "kyc_ubo.assert.subject.type-correction"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let entity = EntityId(json_extract_uuid(args, ctx, "entity-id")?);
        let entity_type_wire = json_extract_string(args, "entity-type")?;
        let corrected_type = entity_type_from_wire(&entity_type_wire).ok_or_else(|| {
            anyhow!(
                "kyc_ubo.assert.subject.type-correction: unrecognized entity-type '{entity_type_wire}' — \
                 rejected fail-closed. Valid wire values: {}",
                ENTITY_TYPE_WIRE_VALUES.join(", ")
            )
        })?;

        let events = PgKycEventStore::load_events(scope.executor(), subject)
            .await
            .map_err(|e| anyhow!("correct-type: load events failed: {e}"))?;
        let refs: Vec<&ob_poc_kyc_substrate::IntentEvent> = events.iter().collect();
        let control = fold_control_versioned(&refs, &KYC_REGISTRY)
            .map_err(|e| anyhow!("correct-type: control fold failed: {e}"))?;
        let type_registry = fold_type_registry(&refs);
        // Phase 2 of the tree-cleanup follow-up tranche (EOP-STATE-KYCUBO-D1
        // §4/§7): "prior type must exist" is no longer hand-checked here —
        // it is `Precondition::PriorTypeAsserted`, enforced by the single
        // `check_preconditions` checker inside `stream_append` below, under
        // the append lock (TOCTOU-safe, an improvement over this pre-fetch
        // fold, which could have raced a concurrent correction).

        let mut invalidated: std::collections::BTreeSet<EdgeId> = std::collections::BTreeSet::new();
        let mut known_tuples: Vec<(EdgeId, ob_poc_kyc_substrate::Pipe, ob_poc_kyc_substrate::EntityType, bool)> =
            Vec::new();
        for edge in control
            .edges
            .values()
            .filter(|e| e.is_active() && (e.from == entity || e.to == entity))
        {
            let this_is_source = edge.from == entity;
            let other = if this_is_source { edge.to } else { edge.from };
            match type_registry.type_of(other) {
                Some(other_type) => {
                    let target_type_for_pipe = if this_is_source { other_type } else { corrected_type };
                    match pipe_of(&edge.kind, Some(target_type_for_pipe)).pipe {
                        Some(pipe) => known_tuples.push((edge.id, pipe, other_type, this_is_source)),
                        None => {
                            // Classification unresolved (D1 corrective
                            // tranche Item 4: e.g. an EconomicInterest edge
                            // whose target type falls outside TS.2 §3's
                            // three ratified buckets) — cannot certify
                            // geometry permits it; flag conservatively,
                            // same discipline as the untyped-other-end case
                            // below (TS.1 §4 — never silently upgrade an
                            // unresolved classification to certainty).
                            invalidated.insert(edge.id);
                        }
                    }
                }
                None => {
                    // Other end untyped — cannot certify permitted; flag
                    // conservatively rather than silently pass (TS.1 §4).
                    invalidated.insert(edge.id);
                }
            }
        }
        invalidated.extend(edges_invalidated_by_correction(corrected_type, &known_tuples));
        let invalidated_edge_ids: Vec<Uuid> = invalidated.iter().map(|e| e.0).collect();

        let payload = serde_json::json!({
            "entity_id": entity.0,
            "entity_type": entity_type_wire,
            "invalidated_edge_ids": invalidated_edge_ids,
        });
        let outcome = stream_append(
            "kyc_ubo.assert.subject.type-correction",
            subject,
            TargetBinding { entity_id: Some(entity), ..TargetBinding::for_subject(subject) },
            payload,
            "senior-analyst.correct-type",
            Some("kyc_ubo.assert.subject.type-correction"),
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(json!({
            "seq": outcome.seq,
            "invalidated_edge_count": invalidated_edge_ids.len(),
        })))
    }
}

/// `kyc_ubo.assert.subject.member-withdrawal` — TS.1 move 6. Membership must exist
/// (`EntityRegistered`) AND be active (`MembershipActive`, Phase 2 of the
/// tree-cleanup follow-up tranche, EOP-STATE-KYCUBO-D1 §4/§7 — no longer
/// hand-checked here; enforced by `check_preconditions` inside
/// `stream_append` below, under the append lock, TOCTOU-safe).
pub struct KycSubjectWithdrawMember;

#[async_trait]
impl SemOsVerbOp for KycSubjectWithdrawMember {
    fn fqn(&self) -> &str {
        "kyc_ubo.assert.subject.member-withdrawal"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let entity = EntityId(json_extract_uuid(args, ctx, "entity-id")?);

        let payload = serde_json::json!({ "entity_id": entity.0 });
        let outcome = stream_append(
            "kyc_ubo.assert.subject.member-withdrawal",
            subject,
            TargetBinding { entity_id: Some(entity), ..TargetBinding::for_subject(subject) },
            payload,
            "analyst.withdraw-member",
            Some("kyc_ubo.assert.subject.member-withdrawal"),
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "seq": outcome.seq }),
        ))
    }
}

/// `kyc_ubo.assert.subject.enquiry` — TS.1 move 8. No preconditions (group
/// exists trivially, by construction — the subject stream this workbook is
/// open against IS the group).
pub struct KycSubjectRecordEnquiry;

#[async_trait]
impl SemOsVerbOp for KycSubjectRecordEnquiry {
    fn fqn(&self) -> &str {
        "kyc_ubo.assert.subject.enquiry"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let mut payload = serde_json::json!({});
        if let Some(obj) = payload.as_object_mut() {
            obj.insert(
                "sources_consulted".to_string(),
                args.get("sources-consulted").cloned().unwrap_or_else(|| json!([])),
            );
            obj.insert(
                "searches_run".to_string(),
                args.get("searches-run").cloned().unwrap_or_else(|| json!([])),
            );
        }
        let outcome = stream_append(
            "kyc_ubo.assert.subject.enquiry",
            subject,
            TargetBinding::for_subject(subject),
            payload,
            "analyst.record-enquiry",
            Some("kyc_ubo.assert.subject.enquiry"),
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "seq": outcome.seq }),
        ))
    }
}

/// Normalize `kyc_ubo.assert.subject.register` payload for the fold (EOP-DD-KYCUBO-003 R3/M1.1):
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

/// Normalize `kyc_ubo.assert.subject.structure-class` payload (EOP-DD-KYCUBO-003 R3/M1.1):
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

// `KycObligationCreate`/`kyc_ubo.assert.obligation.creation` DISSOLVED
// (EOP-DD-KYCUBO-D2.0 §5, K-G7, 2026-08-22): "nobody hands over an
// obligation; you run the checks and they fail. The finding is the
// record." See `ob-poc-kyc-substrate/src/fold/obligation.rs`'s retirement
// comment for the full rationale and reintroduction path.

pub struct KycObligationUpdateIdentity;

#[async_trait]
impl SemOsVerbOp for KycObligationUpdateIdentity {
    fn fqn(&self) -> &str {
        "kyc_ubo.assert.entity.identity"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        // T6.4 row 12 finding: `validate_entry_fqn` was `None` —
        // ObligationExists was dead at the real write path.
        // `SubjectNotDecided` re-homed onto kyc_decision_records TS.6 P2
        // (retired from the substrate — kyc_ubo.decide.subject.approve/kyc_ubo.decide.subject.reject no
        // longer append to the fact stream, so the fold can't see it).
        refuse_if_subject_already_decided(scope, subject, "kyc_ubo.assert.entity.identity").await?;
        let outcome = stream_append(
            "kyc_ubo.assert.entity.identity",
            subject,
            TargetBinding::for_subject(subject),
            normalize_obligation_payload(args),
            "analyst.obligation-update",
            Some("kyc_ubo.assert.entity.identity"),
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
        "kyc_ubo.assert.entity.screening"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        // T6.4 row 13 finding: `validate_entry_fqn` was `None` —
        // ObligationExists was dead at the real write path.
        // `SubjectNotDecided` re-homed onto kyc_decision_records TS.6 P2
        // (retired from the substrate — kyc_ubo.decide.subject.approve/kyc_ubo.decide.subject.reject no
        // longer append to the fact stream, so the fold can't see it).
        refuse_if_subject_already_decided(scope, subject, "kyc_ubo.assert.entity.screening").await?;
        let outcome = stream_append(
            "kyc_ubo.assert.entity.screening",
            subject,
            TargetBinding::for_subject(subject),
            normalize_obligation_payload(args),
            "analyst.obligation-update",
            Some("kyc_ubo.assert.entity.screening"),
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
        "kyc_ubo.assert.entity.risk"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        // T6.4 row 14 finding: `validate_entry_fqn` was `None` —
        // ObligationExists was dead at the real write path.
        // `SubjectNotDecided` re-homed onto kyc_decision_records TS.6 P2
        // (retired from the substrate — kyc_ubo.decide.subject.approve/kyc_ubo.decide.subject.reject no
        // longer append to the fact stream, so the fold can't see it).
        refuse_if_subject_already_decided(scope, subject, "kyc_ubo.assert.entity.risk").await?;
        let outcome = stream_append(
            "kyc_ubo.assert.entity.risk",
            subject,
            TargetBinding::for_subject(subject),
            normalize_obligation_payload(args),
            "analyst.obligation-update",
            Some("kyc_ubo.assert.entity.risk"),
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "seq": outcome.seq }),
        ))
    }
}

// `KycObligationSatisfy`/`kyc_ubo.assert.obligation.satisfaction` DISSOLVED
// (EOP-DD-KYCUBO-D2.0 §5, K-G7, 2026-08-22): "nothing is satisfied; you
// assert the missing fact and re-run. The new run supersedes the old."
//
// `KycObligationWaive`/`kyc_ubo.assert.obligation.waiver` MOVED (D2.0 §5) to
// `ob-poc-kyc-decide::DecideObligationWaive` as `kyc_ubo.decide.obligation.waiver`
// — a human ruling that a failing check does not apply, citing the run, is
// an Evaluation decision, never a fact-stream append.

// `KycPersonApprove`/`KycPersonReject` retired from this file TS.6 P2
// (K-G7) — renamed `kyc_ubo.decide.subject.approve`/`kyc_ubo.decide.subject.reject` and moved to
// `ob-poc-kyc-decide` (`crates/ob-poc-kyc-decide/src/lib.rs`), which has no
// dependency on `ob-poc-kyc-seam` and so writes only to
// `"ob-poc".kyc_decision_records`, never this file's `stream_append`/the
// fact stream. The K-23 gate and the finality check both moved with them
// (re-homed onto `kyc_decision_records` directly — see that crate's doc
// comment).

// ── W5 screening hook (EOP-DD-KYCUBO-004 Part 1) ───────────────────────────────

/// Map a `screenings.status` value to the `kyc_ubo.assert.entity.screening`
/// track `state` it represents. Fail-closed on any value outside the verb
/// YAML's declared `valid_values` — `valid_values` is descriptive metadata,
/// not a DSL-parse-time hard reject (see CLAUDE.md), so this op is the actual
/// enforcement point, matching the `STRUCTURE_CLASS_WIRE_VALUES` precedent.
fn screening_complete_status_to_track_state(status: &str) -> Result<&'static str> {
    match status {
        "CLEAR" => Ok("satisfied"),
        // Not yet resolved — awaits `screening.review-hit`.
        "HIT_PENDING_REVIEW" => Ok("in_progress"),
        // Transient provider/infra failure — still open, not a KYC outcome.
        "ERROR" => Ok("in_progress"),
        other => Err(anyhow!(
            "screening.complete: unrecognized status {other:?} (expected CLEAR | HIT_PENDING_REVIEW | ERROR)"
        )),
    }
}

/// Map a `screening.review-hit` resolution to the obligation track `state`.
fn review_hit_status_to_track_state(status: &str) -> Result<&'static str> {
    match status {
        // A confirmed sanctions/PEP/adverse-media match fails the screening
        // track outright — it does not go back to `in_progress`.
        "HIT_CONFIRMED" => Ok("rejected"),
        "HIT_DISMISSED" => Ok("satisfied"),
        other => Err(anyhow!(
            "screening.review-hit: unrecognized status {other:?} (expected HIT_CONFIRMED | HIT_DISMISSED)"
        )),
    }
}

/// Resolve `workstream_id` to the entity being screened, and fan out an
/// `kyc_ubo.assert.entity.screening` event to every obligation currently
/// registered for that entity's subject stream (`kyc.obligation.*` verbs key
/// `subject-id` to the natural person/entity's own UUID — see
/// `tests/kyc_w3_w5_w6.rs`).
///
/// If no obligation has been raised yet for this entity (screening ran ahead
/// of `kyc_ubo.assert.obligation.creation`), this is a no-op — the legacy `screenings` row
/// write already happened in the caller; there is nothing further to fold.
async fn apply_screening_outcome_to_obligations(
    workstream_id: Uuid,
    state: &'static str,
    ctx: &mut VerbExecutionContext,
    scope: &mut dyn TransactionScope,
) -> Result<()> {
    let entity: Option<(Uuid,)> = sqlx::query_as(
        r#"SELECT entity_id FROM "ob-poc".entity_workstreams WHERE workstream_id = $1"#,
    )
    .bind(workstream_id)
    .fetch_optional(scope.executor())
    .await?;
    let Some((entity_id,)) = entity else {
        return Ok(());
    };
    let subject = SubjectId(entity_id);

    let events = PgKycEventStore::load_events(scope.executor(), subject)
        .await
        .map_err(|e| anyhow!("apply_screening_outcome_to_obligations: load_events failed: {e}"))?;
    let refs: Vec<_> = events.iter().collect();
    let obligation_state = fold_obligations_versioned(&refs, &KYC_REGISTRY)
        .map_err(|e| anyhow!("apply_screening_outcome_to_obligations: fold failed: {e}"))?;
    let Some(rollup) = obligation_state.subjects.get(&subject) else {
        return Ok(());
    };
    let obligation_ids = rollup.obligations.clone();

    for oid in obligation_ids {
        let payload = serde_json::json!({
            "subject_id": subject.0,
            "obligation_id": oid.0,
            "state": state,
        });
        stream_append(
            "kyc_ubo.assert.entity.screening",
            subject,
            TargetBinding::for_subject(subject),
            payload,
            "system.screening-hook",
            Some("kyc_ubo.assert.entity.screening"),
            ctx,
            scope,
        )
        .await?;
    }
    Ok(())
}

/// EOP-PLAN-DAG-AWAITS-001 Phase 5: if every `screenings` row for this
/// workstream has now reached a real terminal status (the `screening` DAG
/// slot's own authoritative `terminal_states` —
/// `CLEAR`/`HIT_CONFIRMED`/`HIT_DISMISSED`, matching
/// `sem_os_postgres::ops::screening::TERMINAL_STATUSES`; `ERROR`/`EXPIRED`
/// are recoverable, not terminal, and correctly keep this a no-op), notify
/// any BPMN process instance waiting on `entity-workstream.SCREEN`'s await
/// (`config/bpmn/entity-workstream-screen.bpmn`'s `screening_settled`
/// message catch). Best-effort: `screening.run` not having been routed
/// through BPMN yet (no active correlation) is the common case today and
/// is silently a no-op inside `queue_bpmn_signal`, same as every other
/// caller of that helper.
async fn signal_if_workstream_screenings_settled(
    workstream_id: Uuid,
    scope: &mut dyn TransactionScope,
) {
    let statuses: Result<Vec<String>, sqlx::Error> = sqlx::query_scalar(
        r#"SELECT status FROM "ob-poc".screenings WHERE workstream_id = $1"#,
    )
    .bind(workstream_id)
    .fetch_all(scope.executor())
    .await;

    let statuses = match statuses {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(
                workstream_id = %workstream_id,
                error = %e,
                "Failed to check screening statuses for settled signal"
            );
            return;
        }
    };

    const TERMINAL: &[&str] = &["CLEAR", "HIT_CONFIRMED", "HIT_DISMISSED"];
    let all_settled = !statuses.is_empty() && statuses.iter().all(|s| TERMINAL.contains(&s.as_str()));
    if !all_settled {
        return;
    }

    crate::bpmn_integration::signal_outbox::queue_bpmn_signal(
        scope.pool(),
        "entity-workstream-screen",
        &workstream_id.to_string(),
        "screening_settled",
        &serde_json::json!({}),
    )
    .await;
}

/// `screening.complete` — was `behavior: crud`; now `plugin` so completion
/// also fans out to the dsl.kyc obligation stream (EOP-DD-KYCUBO-004 Part 1,
/// closing the previously-unwired W5 hook: `kyc_ubo.assert.entity.screening`
/// existed and folded correctly but nothing called it from a real screening
/// outcome). Preserves the original CRUD semantics exactly: only
/// `result-summary`/`match-count` when present, `completed_at = now()`
/// unconditionally.
pub struct ScreeningComplete;

#[async_trait]
impl SemOsVerbOp for ScreeningComplete {
    fn fqn(&self) -> &str {
        "screening.complete"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let screening_id = json_extract_uuid(args, ctx, "screening-id")?;
        let status = json_extract_string(args, "status")?;
        let state = screening_complete_status_to_track_state(&status)?;
        let result_summary = json_extract_string_opt(args, "result-summary");
        let match_count = args
            .get("match-count")
            .and_then(|v| v.as_i64())
            .map(|n| n as i32);

        let row: (Uuid,) = sqlx::query_as(
            r#"UPDATE "ob-poc".screenings
               SET completed_at = now(),
                   status = $1,
                   result_summary = COALESCE($2, result_summary),
                   match_count = COALESCE($3, match_count)
               WHERE screening_id = $4
               RETURNING workstream_id"#,
        )
        .bind(&status)
        .bind(&result_summary)
        .bind(match_count)
        .bind(screening_id)
        .fetch_one(scope.executor())
        .await
        .map_err(|e| anyhow!("screening.complete: update failed: {e}"))?;

        apply_screening_outcome_to_obligations(row.0, state, ctx, scope).await?;
        signal_if_workstream_screenings_settled(row.0, scope).await;

        Ok(VerbExecutionOutcome::Affected(1))
    }
}

/// `screening.review-hit` — same treatment as `ScreeningComplete` above.
pub struct ScreeningReviewHit;

#[async_trait]
impl SemOsVerbOp for ScreeningReviewHit {
    fn fqn(&self) -> &str {
        "screening.review-hit"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let screening_id = json_extract_uuid(args, ctx, "screening-id")?;
        let status = json_extract_string(args, "status")?;
        let state = review_hit_status_to_track_state(&status)?;
        let notes = json_extract_string(args, "notes")?;
        let red_flag_id = json_extract_uuid_opt(args, ctx, "red-flag-id");

        let row: (Uuid,) = sqlx::query_as(
            r#"UPDATE "ob-poc".screenings
               SET reviewed_at = now(),
                   status = $1,
                   review_notes = $2,
                   red_flag_id = COALESCE($3, red_flag_id)
               WHERE screening_id = $4
               RETURNING workstream_id"#,
        )
        .bind(&status)
        .bind(&notes)
        .bind(red_flag_id)
        .bind(screening_id)
        .fetch_one(scope.executor())
        .await
        .map_err(|e| anyhow!("screening.review-hit: update failed: {e}"))?;

        apply_screening_outcome_to_obligations(row.0, state, ctx, scope).await?;
        signal_if_workstream_screenings_settled(row.0, scope).await;

        Ok(VerbExecutionOutcome::Affected(1))
    }
}
