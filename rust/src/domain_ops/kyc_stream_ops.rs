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

use ob_poc_kyc_seam::{append_in_scope, canonical_event_shape, IntentEventDraft};
use ob_poc_kyc_store::{enqueue_cross_stream_obligations, prior_freeze_persons, PgKycEventStore};
use ob_poc_kyc_substrate::{
    check_preconditions,
    find_subject_entity, fold_control_versioned, fold_obligations_versioned,
    fold_type_registry, natural_persons_from_events, assembly_lexicon,
    render_intent_event_to_sexpr, AuthorityRef, ControlProngStrategy, DeterminationStrategy,
    CooperativeMemberStrategy, FoldRegistry, FoundationCouncilStrategy,
    FundControlStrategy, NomineePierceStrategy, OwnershipProngStrategy, PersonId,
    ProngCandidate, SmoResult, StateOwnedStrategy, SubjectId, TargetBinding,
    TrustRoleStrategy, V1FoldImpl,
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

/// `kyc_ubo.assert.edge.connect` — a link exists, from this block to that
/// one, of this kind (EOP-VS-UBO-GAME-001 §3.1). Merges the former
/// `UboEdgeAssertControl` + `UboEdgeAssertEconomicInterest` (§3.2: they
/// differ by kind, geometry already validates the classified pipe, their
/// preconditions are identical). The first stream-backed determination
/// verb; the pattern every other `dsl.kyc` verb follows.
pub struct UboEdgeConnect;

#[async_trait]
impl SemOsVerbOp for UboEdgeConnect {
    fn fqn(&self) -> &str {
        "kyc_ubo.assert.edge.connect"
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        // The determination root this edge belongs to (the subject stream).
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);

        // 2026-09-07 (audit item 2, P1): the hand-rolled `pierced-from`
        // existence/kind/active check that used to live here is GONE — it is
        // now `Precondition::PiercedFromIsActiveNominee`, declared on this
        // verb's lexicon entry and evaluated by `check_preconditions` inside
        // `stream_append` below, under the same append lock every other
        // stud runs under (K-14). One copy, both surfaces (op + workbook)
        // enforce it identically — `EOP-VS-UBO-GAME-001` §3.4 R7.

        let (target, payload, edge) =
            canonical_event_shape("kyc_ubo.assert.edge.connect", subject, args)?;
        let edge = edge.expect("connect always mints an edge id");

        let outcome = stream_append(
            "kyc_ubo.assert.edge.connect",
            subject,
            target,
            payload,
            "analyst.connect",
            Some("kyc_ubo.assert.edge.connect"),
            ctx,
            scope,
        )
        .await
        .map_err(|e| anyhow!("kyc_ubo.assert.edge.connect append failed: {e}"))?;

        Ok(VerbExecutionOutcome::Record(json!({
            "edge_id": edge.0,
            "seq": outcome.seq,
            "deduped": outcome.deduped,
        })))
    }
}

pub struct UboEdgeAttachEvidence;

#[async_trait]
impl SemOsVerbOp for UboEdgeAttachEvidence {
    fn fqn(&self) -> &str {
        "kyc_ubo.assert.edge.evidence"
    }
    /// Dual-target (TS.1 §3 row 4, wired 2026-08-24 corrective tranche
    /// Item 3a): `edge-id` logs a proof against a control/economic edge;
    /// `entity-id` logs a proof against an entity's asserted type
    /// (`fold::type_registry`'s citation-set arm). Exactly one of the two
    /// must be present — the board's own placement set
    /// (`type_registry_candidates` vs. the edge-scoped main loop) never
    /// proposes both for the same move, and neither shape's lexicon
    /// preconditions apply to the other (`EdgeExists`/`EdgeActive` vs.
    /// `PriorTypeAsserted`/`MembershipActive`, each vacuous when their own
    /// target field is absent). EOP-DD-UBO-PROOF-001 §1/§4 (T5): no
    /// ratchet — `evidence` logs a proof (kind, source, date, all in
    /// `canonical_event_shape`'s payload) unconditionally; the returned
    /// `event_id` is the proof's own citation id, the fact `retract` later
    /// targets.
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let (target, payload, _edge) =
            canonical_event_shape("kyc_ubo.assert.edge.evidence", subject, args)?;
        let outcome = stream_append(
            "kyc_ubo.assert.edge.evidence",
            subject,
            target,
            payload,
            "analyst.attach-evidence",
            Some("kyc_ubo.assert.edge.evidence"),
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(serde_json::json!({
            "seq": outcome.seq,
            "citation_id": outcome.event_id.0,
        })))
    }
}

// `UboEdgeVerify` RETIRED (EOP-DD-UBO-PROOF-001 §3/§4, T5, 2026-08-28)
// alongside `kyc_ubo.assert.edge.verification` — "the board collects
// facts; the policy rules on adequacy," so there is no ratchet left to
// move an edge into (K-G7: 0 real committed events under this FQN).

/// Move 5 (§3.1's move table: "that proof no longer stands — group,
/// citation", EOP-DD-UBO-PROOF-001 §4, T5): withdraw one previously logged
/// proof by its citation id. No target — the citation alone identifies
/// the proof, wherever it lives (`fold::control`/`fold::type_registry`'s
/// mirrored `retract` arms each scan their own axis).
pub struct UboEdgeRetract;

#[async_trait]
impl SemOsVerbOp for UboEdgeRetract {
    fn fqn(&self) -> &str {
        "kyc_ubo.assert.edge.retract"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let (target, payload, _edge) =
            canonical_event_shape("kyc_ubo.assert.edge.retract", subject, args)?;
        let outcome = stream_append(
            "kyc_ubo.assert.edge.retract",
            subject,
            target,
            payload,
            "analyst.retract",
            Some("kyc_ubo.assert.edge.retract"),
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "seq": outcome.seq }),
        ))
    }
}

/// `kyc_ubo.assert.edge.disconnect` — that link is not on the board
/// (EOP-VS-UBO-GAME-001 §3.1). Renamed from `UboEdgeSupersede` (§3.2: pure
/// rename, K-13 supersede-never-delete unchanged).
pub struct UboEdgeDisconnect;

#[async_trait]
impl SemOsVerbOp for UboEdgeDisconnect {
    fn fqn(&self) -> &str {
        "kyc_ubo.assert.edge.disconnect"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let (target, payload, _edge) =
            canonical_event_shape("kyc_ubo.assert.edge.disconnect", subject, args)?;
        // T6.2 row 4: EdgeExists + EdgeActive now attached — `Some(fqn)`
        // wires it to the real checker (previously `None` was harmless
        // because the entry declared no preconditions; leaving it `None`
        // now would silently never enforce the new stud — the exact defect
        // class the Part A closure tooth exists to catch).
        let outcome = stream_append(
            "kyc_ubo.assert.edge.disconnect",
            subject,
            target,
            payload,
            "analyst.disconnect",
            Some("kyc_ubo.assert.edge.disconnect"),
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
// ubo.yaml): `kyc_ubo.assert.edge.connect` (extended with an optional
// `pierced-from` arg — see its op-layer fail-closed check above and
// `normalize_assert_control_payload` below) followed by `kyc_ubo.assert.edge.disconnect`
// (unchanged). Piercing records a real handed-over fact, so it wasn't
// deleted outright (unlike select-strategy/compute-fold) — it was
// redesigned because the old shape was a single-purpose verb doing two
// governed effects in one hand-rolled fold arm, with no reusable
// composition mechanism; the macro is executed atomically under the
// Sequencer's one-scope-per-runbook model, same as any other multi-step
// macro, so the two effects still commit or roll back together.

// `UboEdgeReconcileConflict` / `kyc_ubo.assert.edge.reconciliation` RETIRED
// (EOP-VS-UBO-GAME-001 T3, §3.3, 2026-08-27, K-G7 full deletion): "No
// reconcile ... a third path to what two moves [connect/disconnect]
// already do." No op survives it here — see fold/control.rs's former
// reconciliation fold arm (deleted) for the full reasoning.

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

// `normalize_edge_id_payload` and `normalize_assert_control_payload` —
// T1 (EOP-VS-UBO-GAME-001 §3.4 R6) DELETED both: the edge-id kebab→snake
// stamping, the `kind` wire-value fail-closed gate, the nominee+pierced-from
// mutual exclusion, and the `trust-revocable`/`pierced-from` renames they
// used to duplicate now live once, inside `ob_poc_kyc_seam::canonical_event_shape`
// — called by `UboEdgeConnect` (T3: merged from `UboEdgeAssertControl`/
// `UboEdgeAssertEconomicInterest`) above instead of each building its own
// payload.
//
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
        //    EOP-DD-UBO-DISPATCH-001 T4 (2026-08-28): the strategy is DERIVED
        //    from the subject's `EntityType` (`dispatch_for_entity_type`),
        //    replacing `structure_class`/`strategy_for_structure_class`.
        //    `EntityTypeSupportsStrategy` (gating freeze already) guarantees
        //    the type dispatches to a real strategy, not `NotADeterminationSubject`
        //    and not unknown, before we get here. `find_subject_entity` no
        //    longer scans for a `structure-class` event — the subject's own
        //    entity is `EntityId(subject.0)` by construction.
        let subject_entity_id = find_subject_entity(&refs).ok_or_else(|| {
            anyhow!("freeze: no events for this subject — subject entity unknown")
        })?;
        let entity_type = type_registry.type_of(subject_entity_id).ok_or_else(|| {
            anyhow!("freeze: no entity type known for the subject (EntityTypeSupportsStrategy precondition)")
        })?;
        let strategy_name = match ob_poc_kyc_substrate::dispatch_for_entity_type(&entity_type) {
            ob_poc_kyc_substrate::DeterminationDispatch::Strategy(name) => name,
            ob_poc_kyc_substrate::DeterminationDispatch::NotADeterminationSubject => {
                return Err(anyhow!(
                    "freeze: {entity_type:?} is not a determination subject \
                     (EOP-DD-UBO-DISPATCH-001 §4 D2, EntityTypeSupportsStrategy precondition)"
                ));
            }
        };
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
        // 2026-09-07 (audit item 2, P2): the RULE is now declared once —
        // `Precondition::NoUnpiercedNomineeEdges`, on this verb's lexicon
        // entry, evaluated by `check_preconditions` inside `stream_append`
        // below (both the op AND, were freeze ever staged through a second
        // surface, that surface would enforce it identically — R7). This
        // early call is a SECOND invocation of that same shared
        // `unpierced_nominee_edges` scan, not a second implementation of
        // it — same discipline as `TypeGeometryPermits`, whose scan
        // (`evaluate_type_geometry`) is likewise called both early (board
        // preview, `placement.rs`) and late (the real checker). Kept here,
        // not deleted, because a first attempt at deleting it changed
        // observable behaviour: without the early call, K-5's "would be
        // silent" refusal fires first whenever the unpierced nominee also
        // makes the traversal produce zero candidates, masking the real
        // cause behind a downstream symptom
        // (`kyc_ts4_nominee.rs::d_freeze_hard_errors_while_unpierced_nominee_edge_active`
        // caught this on the first attempt).
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
            // this dispatch, backed by `Precondition::NoUnpiercedNomineeEdges`
            // (declared, re-checked at append too — see the comment above).
            // Scope note lives on NomineePierceStrategy.
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

// ── Subject placement verbs ───────────────────────────────────────────────────

/// `kyc_ubo.assert.subject.place` — EOP-VS-UBO-GAME-001 T2, §3.2. Absorbs
/// `register` + `assert-type` (both retired — see their fold arms' own
/// comments). One move, one event, both membership and type; refuses a
/// currently-placed entity (`Precondition::NotCurrentlyPlaced`, §8 Q1 —
/// place never carries update semantics, correction is `remove` then `place`).
pub struct KycSubjectPlace;

#[async_trait]
impl SemOsVerbOp for KycSubjectPlace {
    fn fqn(&self) -> &str {
        "kyc_ubo.assert.subject.place"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let (target, payload, _edge) =
            canonical_event_shape("kyc_ubo.assert.subject.place", subject, args)?;
        let outcome = stream_append(
            "kyc_ubo.assert.subject.place",
            subject,
            target,
            payload,
            "analyst.place",
            Some("kyc_ubo.assert.subject.place"),
            ctx,
            scope,
        )
        .await?;
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "seq": outcome.seq }),
        ))
    }
}

// `KycSubjectClassifyStructure` (`kyc_ubo.assert.subject.structure-class`)
// DELETED — EOP-DD-UBO-DISPATCH-001 T4 (2026-08-28). The strategy is now
// derived directly from the subject's EntityType (`dispatch_for_entity_type`,
// §2's ratified 21-type/7-strategy mapping), never separately classified.
// 10 real committed events existed for this FQN before deletion (P0
// census) — the fold-side parser
// (`ob-poc-kyc-substrate::fold::control::structure_class_from_payload`)
// stays R5-historical for replay; this op struct is deleted, not left
// unregistered (nothing dispatches to it any longer).

// ── D1 (EOP-DD-KYCUBO-TS.1 §3) — the remaining type-registry moves ─────────
// `kyc_ubo.assert.subject.type` RETIRED (T2, §3.2) — absorbed into
// `KycSubjectPlace` above; its op struct is deleted, not left unregistered
// (nothing dispatches to it — dead code, not a fold-target the way T1's
// dispatching-fold pattern keeps specialist structs alive).

// `KycSubjectCorrectType` (`kyc_ubo.assert.subject.type-correction`) DELETED
// — EOP-VS-UBO-GAME-001 T2 (2026-08-27, §8 Q1) DISSOLVED this verb. §8 Q1
// RULED: correcting a type is `remove` then `place`, two ordinary moves,
// never a superseding placement or a computed cascade. 0 real committed
// events existed for this FQN (confirmed by DB query before deletion), so
// this is a full K-G7 deletion — no fold-arm-kept historical retirement, no
// unregistered dispatch target. Its op-layer cascade computation
// (`edges_invalidated_by_correction`) is deleted from
// `ob-poc-kyc-substrate::fold::type_registry` in the same diff.

/// `kyc_ubo.assert.subject.remove` — EOP-VS-UBO-GAME-001 T2, §3.2. Absorbs
/// `member-withdrawal` (retired — see its fold arm's own comment).
/// Membership must exist (`EntityRegistered`) AND be active
/// (`MembershipActive`), enforced by `check_preconditions` inside
/// `stream_append` below, under the append lock, TOCTOU-safe. §2: withdraws
/// the placement, never the entity — it remains a group member and can be
/// placed again.
pub struct KycSubjectRemove;

#[async_trait]
impl SemOsVerbOp for KycSubjectRemove {
    fn fqn(&self) -> &str {
        "kyc_ubo.assert.subject.remove"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        let (target, payload, _edge) =
            canonical_event_shape("kyc_ubo.assert.subject.remove", subject, args)?;
        let outcome = stream_append(
            "kyc_ubo.assert.subject.remove",
            subject,
            target,
            payload,
            "analyst.remove",
            Some("kyc_ubo.assert.subject.remove"),
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
        let (target, payload, _edge) =
            canonical_event_shape("kyc_ubo.assert.subject.enquiry", subject, args)?;
        let outcome = stream_append(
            "kyc_ubo.assert.subject.enquiry",
            subject,
            target,
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

// `normalize_register_payload`, `normalize_classify_structure_payload`, and
// `normalize_obligation_payload` — the last three of the five T1 (§3.4 R6)
// normalizers named in the tranche's own recon — DELETED (this tranche):
// their kebab→snake + entity_id-defaulting duties now live once, inside
// `ob_poc_kyc_seam::canonical_event_shape`, called by every op above instead
// of each op calling its own copy. `normalize_edge_id_payload` and
// `normalize_assert_control_payload` (the first two) were deleted the same
// way, just above `UboDeterminationFreeze`.

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
        let (target, payload, _edge) = canonical_event_shape("kyc_ubo.assert.entity.identity", subject, args)?;
        let outcome = stream_append(
            "kyc_ubo.assert.entity.identity",
            subject,
            target,
            payload,
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
        let (target, payload, _edge) = canonical_event_shape("kyc_ubo.assert.entity.screening", subject, args)?;
        let outcome = stream_append(
            "kyc_ubo.assert.entity.screening",
            subject,
            target,
            payload,
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
        let (target, payload, _edge) = canonical_event_shape("kyc_ubo.assert.entity.risk", subject, args)?;
        let outcome = stream_append(
            "kyc_ubo.assert.entity.risk",
            subject,
            target,
            payload,
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
