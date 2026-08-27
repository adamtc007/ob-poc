//! TS.3 gate tests — EOP-DD-KYCUBO-KIT-TS0 §2.4 (`state_owned_strategy`) +
//! §2.5 (`cooperative_member_strategy`), ratified 2026-08-12 ACCEPT ALL.
//!
//! RED-first against the post-TS.2 tree:
//! - (a) state_owned end-to-end through the REAL ops: register → classify
//!   `state_owned` → assert a control edge from the rare genuine
//!   natural-person controller → select `state_owned_strategy` (proves the
//!   `StructureClassSupported` widening at the real append path — pre-TS.3
//!   StateOwned is fail-closed and select-strategy REJECTS) → resolve: the
//!   controller chain, `ControlByOtherMeans`, `effective_ownership_pct` None.
//! - (b) state_owned zero-candidate: no natural-person control chain crosses
//!   → empty Vec — the COMMON case for this class (§2.4: the controller is a
//!   state organ; the existing `apply-smo-fallback` path supplies the
//!   determination; the strategy legitimizes the SMO route, it does not
//!   build new SMO machinery).
//! - (c) cooperative end-to-end: register → classify `cooperative` → assert
//!   a `board_appointment` (office) edge → select
//!   `cooperative_member_strategy` → resolve: the office holder as
//!   `ControlByOtherMeans` (§2.5: control arises from OFFICE, never from
//!   membership per se — one-member-one-vote).
//! - (d) cooperative kind-filter: traverses `voting_rights` +
//!   `board_appointment` + `dominant_influence` ONLY — a `gp_statutory`
//!   edge does NOT produce a candidate; a `voting_rights` one does
//!   (anomalous concentrated voting arrangement, §2.5).
//! - (e) select-strategy ADMITS nominee (TS.4 fixture flip: Nominee joined
//!   the implemented set via NomineePierceStrategy — the old "still blocks"
//!   assertion inverted; the guard set is now total).
//! - (f) freeze's unknown-strategy arm still errors (TS.4 fixture fix: the
//!   exemplar moved to a never-will-exist string), message lists all 8
//!   implemented strategy names.
//!
//! NOTE: the render.rs:102 nested-null defect was fixed 2026-08-14
//! (`render_value` omits nulls at every depth), so the final determination
//! legs of (a)/(c)/(d) now drive `kyc_ubo.decide.determination.freeze` LIVE and read
//! the candidates + recorded strategy off the freeze outcome. Test (b)'s
//! zero-candidate case cannot freeze to SUCCESS by design (freeze refuses a
//! silent determination when no candidates and no SMO result exist), so it
//! pins the empty Vec at the strategy level over the real DB-loaded stream
//! AND drives the real freeze op to prove the silent-determination refusal.
//!
//! DB/fixture pattern mirrors `kyc_ts2_fund_foundation.rs`.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionContext};
use ob_poc::domain_ops::kyc_stream_ops::{
    KycSubjectClassifyStructure, KycSubjectPlace,
    UboDeterminationFreeze, UboEdgeAssertControl, UboEdgeAssertEconomicInterest,
    UboEdgeReconcileConflict,
};
use ob_poc_kyc_seam::append_in_scope;
use ob_poc_kyc_substrate::{
    fold_control_versioned, assembly_lexicon, AuthorityRef, DeterminationStrategy, EdgeId,
    FoldRegistry, IntentEvent, Principal, SubjectId, TargetBinding, V1FoldImpl,
};
use ob_poc_types::TransactionScopeId;
use sem_os_postgres::ops::SemOsVerbOp;

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string())
}

async fn pool() -> PgPool {
    PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url())
        .await
        .expect("DB")
}

struct Scope {
    tx: Transaction<'static, Postgres>,
    pool: PgPool,
    id: TransactionScopeId,
}
impl Scope {
    async fn begin(p: &PgPool) -> Self {
        Self {
            tx: p.begin().await.unwrap(),
            pool: p.clone(),
            id: TransactionScopeId::new(),
        }
    }
    async fn commit(self) {
        self.tx.commit().await.unwrap();
    }
}
impl TransactionScope for Scope {
    fn scope_id(&self) -> TransactionScopeId {
        self.id
    }
    fn transaction(&mut self) -> &mut Transaction<'static, Postgres> {
        &mut self.tx
    }
    fn pool(&self) -> &PgPool {
        &self.pool
    }
}

/// Dispatch a verb op and commit, returning the outcome as JSON.
/// Fresh `VerbExecutionContext` per call — `execution_id` seeds the
/// idempotency key (B3); reuse would dedupe follow-on calls.
async fn run(op: &dyn SemOsVerbOp, args: serde_json::Value, pool: &PgPool) -> serde_json::Value {
    let mut ctx = VerbExecutionContext::default();
    let mut scope = Scope::begin(pool).await;
    let out = op
        .execute(&args, &mut ctx, &mut scope)
        .await
        .unwrap_or_else(|error| panic!("{}: {error}", op.fqn()));
    scope.commit().await;
    match out {
        dsl_runtime::VerbExecutionOutcome::Record(v) => v,
        other => serde_json::to_value(format!("{other:?}")).unwrap(),
    }
}

/// Dispatch a verb op without unwrapping — for tests asserting rejection.
async fn run_fallible(
    op: &dyn SemOsVerbOp,
    args: serde_json::Value,
    pool: &PgPool,
) -> anyhow::Result<serde_json::Value> {
    let mut ctx = VerbExecutionContext::default();
    let mut scope = Scope::begin(pool).await;
    let out = op.execute(&args, &mut ctx, &mut scope).await?;
    scope.commit().await;
    Ok(match out {
        dsl_runtime::VerbExecutionOutcome::Record(v) => v,
        other => serde_json::to_value(format!("{other:?}")).unwrap(),
    })
}

/// Appends a `kyc_ubo.assert.edge.control` event directly, bypassing
/// `check_control_preconditions` (so `TypeGeometryPermits`, TS.5) — for
/// test (d) only, which deliberately puts an INCOMPATIBLE-geometry
/// `gp_statutory` edge onto a typed cooperative subject to prove
/// `cooperative_member_strategy`'s own kind filter excludes it. Pre-T2,
/// `register` never typed the subject, so this edge's geometry was always
/// `Unevaluable` (vacuous) on the live path; post-T2, `place` mandates a
/// type (§3.2), and no single `EntityType` permits both `VotingShares` and
/// `GpDesignation` (there is no real-world entity that is simultaneously
/// share-voted and GP-designated) — so this fixture's premise (both kinds
/// landing on the same typed subject, filtered by the STRATEGY layer, not
/// geometry) is only reachable as a historical-shaped append, same pattern
/// as `kyc_d21_engine.rs`'s `append_historical`.
async fn append_historical_control_edge(
    subject: SubjectId,
    from: Uuid,
    to: Uuid,
    kind: &str,
    pool: &PgPool,
) {
    let mut reg = FoldRegistry::new();
    reg.register(assembly_lexicon().hash, std::sync::Arc::new(V1FoldImpl));
    let edge = EdgeId(Uuid::new_v4());
    let payload = serde_json::json!({
        "edge_id": edge.0,
        "from_entity_id": from,
        "to_entity_id": to,
        "kind": kind,
    });
    let mut scope = Scope::begin(pool).await;
    let event = IntentEvent::new(
        subject,
        "kyc_ubo.assert.edge.control",
        Principal::test_analyst(),
        AuthorityRef("test.historical-shape".into()),
        TargetBinding::for_edge(subject, edge),
        payload,
        chrono::Utc::now(),
    )
    .with_lexicon_hash(assembly_lexicon().hash);
    append_in_scope(&mut scope, &reg, &event, "", |_, _, _| Ok(()))
        .await
        .unwrap();
    scope.commit().await;
}

async fn cleanup(pool: &PgPool, subjects: &[SubjectId]) {
    for s in subjects {
        for t in [
            "kyc_intent_events",
            "kyc_subject_streams",
            "kyc_control_edge_projection",
        ] {
            let _ = sqlx::query(&format!(
                r#"DELETE FROM "ob-poc".{t} WHERE subject_root = $1"#
            ))
            .bind(s.0)
            .execute(pool)
            .await;
        }
        let _ = sqlx::query(
            r#"DELETE FROM "public".outbox WHERE idempotency_key LIKE $1
               OR (payload->>'determination_subject')::text = $2
               OR (payload->>'subject_root')::text = $2"#,
        )
        .bind(format!("{}:%", s.0))
        .bind(s.0.to_string())
        .execute(pool)
        .await;
    }
}

/// Register subject + natural persons, then classify with `structure_class`.
async fn setup_subject(pool: &PgPool, subject: SubjectId, persons: &[Uuid], class: &str) {
    run(
        &KycSubjectPlace,
        serde_json::json!({ "subject-id": subject.0, "is_natural_person": false, "entity-type": "private_limited_company" }),
        pool,
    )
    .await;
    for p in persons {
        run(
            &KycSubjectPlace,
            serde_json::json!({
                "subject-id": subject.0, "entity-id": p, "is_natural_person": true, "entity-type": "natural_person",
            }),
            pool,
        )
        .await;
    }
    run(
        &KycSubjectClassifyStructure,
        serde_json::json!({ "subject-id": subject.0, "structure-class": class }),
        pool,
    )
    .await;
}

/// Drive the REAL freeze op, assert the recorded strategy name on the
/// outcome, and return the candidates array (render nested-null defect
/// fixed 2026-08-14 — freeze is drivable live for pct-less candidates).
async fn freeze_candidates(
    pool: &PgPool,
    subject: SubjectId,
    expected_strategy: &str,
) -> Vec<serde_json::Value> {
    let outcome = run(
        &UboDeterminationFreeze,
        serde_json::json!({ "subject-id": subject.0, "policy-version": "v1.0" }),
        pool,
    )
    .await;
    assert_eq!(
        outcome.get("strategy").and_then(|v| v.as_str()),
        Some(expected_strategy),
        "select-strategy (real op) must have recorded {expected_strategy} — freeze \
         dispatches to it and records it on the outcome; got {outcome:?}"
    );
    outcome
        .get("candidates")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default()
}

/// Load the subject's REAL stream from the DB, fold it, assert the recorded
/// strategy name, and resolve via the given strategy instance — the exact
/// composition the freeze op uses. Kept ONLY for test (b)'s zero-candidate
/// case, which freeze refuses to commit by design (silent determination).
async fn resolve_via_strategy(
    pool: &PgPool,
    subject: SubjectId,
    expected_strategy: &str,
    strategy: &dyn DeterminationStrategy,
) -> Vec<ob_poc_kyc_substrate::ProngCandidate> {
    let mut conn = pool.acquire().await.expect("acquire connection");
    let events = ob_poc_kyc_store::PgKycEventStore::load_events(&mut conn, subject)
        .await
        .expect("load events");
    let refs: Vec<&IntentEvent> = events.iter().collect();
    let mut reg = FoldRegistry::new();
    reg.register(assembly_lexicon().hash, std::sync::Arc::new(V1FoldImpl));
    let control = fold_control_versioned(&refs, &reg).expect("fold ok");
    assert_eq!(
        control
            .structure_class
            .as_ref()
            .map(|c| ob_poc_kyc_substrate::strategy_for_structure_class(c)),
        Some(expected_strategy),
        "structure_class must derive {expected_strategy} (TS.6 P2: strategy is derived, \
         not separately asserted)"
    );
    let subject_entity =
        ob_poc_kyc_substrate::find_subject_entity(&refs).expect("classify recorded entity");
    let persons = ob_poc_kyc_substrate::natural_persons_from_events(&refs);
    strategy.resolve(&control, subject_entity, &persons, 25.0)
}

// ── (a) state_owned_strategy end-to-end — the rare natural-person controller ─
//
// §2.4: the strategy runs the full control traversal (same edge-kind
// admission as control_prong_strategy) for the rare genuine natural-person
// controller. Here one exists (dominant_influence), so it resolves.

#[tokio::test]
async fn a_state_owned_strategy_resolves_rare_natural_person_controller() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let controller = Uuid::new_v4(); // the RARE genuine natural-person controller

    setup_subject(&pool, subject, &[controller], "state_owned").await;

    run(
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": controller, "to_entity_id": subject.0,
            "kind": "dominant_influence",
        }),
        &pool,
    )
    .await;
    run(
        &UboEdgeReconcileConflict,
        serde_json::json!({ "subject-id": subject.0 }),
        &pool,
    )
    .await;

    // The gate widening proven below at the REAL op: `freeze_candidates`
    // succeeding at all proves StructureClassSupported admits a
    // StateOwned-classified subject (pre-TS.3 this was fail-closed) —
    // `compute-fold`'s standalone probe of the identical precondition pair
    // was retired TS.6 P2 as redundant with this call.
    let candidates = freeze_candidates(&pool, subject, "state_owned_strategy").await;

    assert_eq!(
        candidates.len(),
        1,
        "expected exactly the rare natural-person controller; got {candidates:#?}"
    );
    assert_eq!(
        candidates[0].get("person_id").and_then(|v| v.as_str()),
        Some(controller.to_string().as_str()),
        "the dominant_influence controller must be the candidate (§2.4)"
    );
    assert_eq!(
        candidates[0].get("prong").and_then(|v| v.as_str()),
        Some("ControlByOtherMeans"),
        "K-1 basis: every state-owned candidate is ControlByOtherMeans"
    );
    assert!(
        candidates[0]
            .get("effective_ownership_pct")
            .map(|v| v.is_null())
            .unwrap_or(false),
        "state-owned control has no quantum — effective_ownership_pct must be null"
    );

    cleanup(&pool, &[subject]).await;
}

// ── (b) state_owned zero-candidate — the SMO route (the COMMON case) ────────
//
// §2.4: the controller is a state organ, not a natural person — no
// natural-person chain crosses, the strategy returns an empty Vec, and the
// EXISTING `apply-smo-fallback` path (stage-gated by ReconciledProjection +
// StrategySelected, matrix row 7) supplies the determination. The strategy
// legitimizes the SMO route for this class; no new SMO machinery.

#[tokio::test]
async fn b_state_owned_strategy_yields_empty_when_no_natural_person_crosses() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let ministry = Uuid::new_v4(); // a state organ — LEGAL entity, not registered as a person
    let economic_only = Uuid::new_v4(); // economic axis — never a control candidate

    setup_subject(&pool, subject, &[economic_only], "state_owned").await;

    // The state organ's control edge: `ministry` is NOT a registered natural
    // person and has no onward control edges, so the chain dead-ends.
    run(
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": ministry, "to_entity_id": subject.0,
            "kind": "voting_rights",
        }),
        &pool,
    )
    .await;
    run(
        &UboEdgeAssertEconomicInterest,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": economic_only, "to_entity_id": subject.0,
            "percentage": 15.0,
        }),
        &pool,
    )
    .await;
    run(
        &UboEdgeReconcileConflict,
        serde_json::json!({ "subject-id": subject.0 }),
        &pool,
    )
    .await;
    // The gate widening (StructureClassSupported admits StateOwned) is
    // already proven independently by test (a) above via a successful
    // `freeze_candidates` call on the same class — the precondition doesn't
    // read candidate count, so there is nothing distinct left for a
    // standalone probe to prove here. (`compute-fold`'s own such probe was
    // retired TS.6 P2; unlike test (a)/(c)/(d), swapping it for
    // `apply-smo-fallback` here would corrupt the fold state the freeze
    // K-5 refusal below depends on, since `apply-smo-fallback` itself
    // commits an SMO determination that `freeze` would then honour.)

    // Strategy-level: the empty Vec, pinned over the real DB-loaded stream.
    // (Freeze cannot commit this case — see below — so the empty candidate
    // set is only observable at the strategy layer.)
    let candidates = resolve_via_strategy(
        &pool,
        subject,
        "state_owned_strategy",
        &ob_poc_kyc_substrate::StateOwnedStrategy,
    )
    .await;
    assert!(
        candidates.is_empty(),
        "no natural-person control chain crosses — the strategy must return an \
         empty Vec (the SMO-fallback route supplies the determination, §2.4); \
         got {candidates:#?}"
    );

    // Real-op leg: freeze REFUSES a silent determination — zero candidates
    // and no SMO result must never commit (the SMO-fallback route is the
    // sanctioned path for this class, §2.4).
    let frozen = run_fallible(
        &UboDeterminationFreeze,
        serde_json::json!({ "subject-id": subject.0, "policy-version": "v1.0" }),
        &pool,
    )
    .await;
    assert!(
        frozen.is_err(),
        "freeze must refuse to commit a zero-candidate, no-SMO determination"
    );
    let msg = frozen.unwrap_err().to_string();
    assert!(
        msg.contains("determination would be silent"),
        "the refusal must be the silent-determination guard; got: {msg}"
    );

    cleanup(&pool, &[subject]).await;
}

// ── (c) cooperative_member_strategy end-to-end — control from OFFICE ────────
//
// §2.5: one-member-one-vote — membership per se never yields a UBO; control
// arises from office (board/management edges). A board_appointment edge to a
// natural person resolves as ControlByOtherMeans.

#[tokio::test]
async fn c_cooperative_member_strategy_resolves_office_holder() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let board_chair = Uuid::new_v4();

    setup_subject(&pool, subject, &[board_chair], "cooperative").await;

    run(
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": board_chair, "to_entity_id": subject.0,
            "kind": "board_appointment",
        }),
        &pool,
    )
    .await;
    run(
        &UboEdgeReconcileConflict,
        serde_json::json!({ "subject-id": subject.0 }),
        &pool,
    )
    .await;

    // `freeze_candidates` succeeding below proves StructureClassSupported
    // admits Cooperative (pre-TS.3 this was fail-closed) — `compute-fold`'s
    // standalone probe of the identical precondition pair was retired TS.6
    // P2 as redundant with this call (see test (a)'s comment).
    let candidates = freeze_candidates(&pool, subject, "cooperative_member_strategy").await;

    assert_eq!(
        candidates.len(),
        1,
        "expected exactly the board chair (office, §2.5); got {candidates:#?}"
    );
    assert_eq!(
        candidates[0].get("person_id").and_then(|v| v.as_str()),
        Some(board_chair.to_string().as_str()),
        "the board_appointment office holder must be the candidate (§2.5)"
    );
    assert_eq!(
        candidates[0].get("prong").and_then(|v| v.as_str()),
        Some("ControlByOtherMeans"),
        "K-1 basis: every cooperative candidate is ControlByOtherMeans"
    );
    assert!(
        candidates[0]
            .get("effective_ownership_pct")
            .map(|v| v.is_null())
            .unwrap_or(false),
        "cooperative control has no quantum — effective_ownership_pct must be null"
    );

    cleanup(&pool, &[subject]).await;
}

// ── (d) cooperative kind-filter — 3 admitted kinds ONLY ─────────────────────
//
// §2.5: traverses voting_rights + board_appointment + dominant_influence
// ONLY. A gp_statutory edge (a partnership-control kind foreign to the
// cooperative form) is IGNORED; an anomalous concentrated voting_rights
// arrangement resolves.

#[tokio::test]
async fn d_cooperative_member_strategy_filters_to_admitted_kinds() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let concentrated_voter = Uuid::new_v4(); // voting_rights → candidate
    let gp_holder = Uuid::new_v4(); // gp_statutory → NOT a candidate

    setup_subject(&pool, subject, &[concentrated_voter, gp_holder], "cooperative").await;

    run(
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": concentrated_voter,
            "to_entity_id": subject.0, "kind": "voting_rights",
        }),
        &pool,
    )
    .await;
    // gp_statutory geometrically cannot land on a typed cooperative subject
    // (no EntityType permits both VotingShares and GpDesignation) — bypass
    // geometry via a historical-shaped append; see the helper's doc comment.
    append_historical_control_edge(subject, gp_holder, subject.0, "gp_statutory", &pool).await;
    run(
        &UboEdgeReconcileConflict,
        serde_json::json!({ "subject-id": subject.0 }),
        &pool,
    )
    .await;
    // `freeze_candidates` succeeding proves StructureClassSupported admits
    // Cooperative (`compute-fold`'s standalone probe retired TS.6 P2 as
    // redundant with this call — see test (a)'s comment).
    let candidates = freeze_candidates(&pool, subject, "cooperative_member_strategy").await;
    let person_ids: std::collections::BTreeSet<Uuid> = candidates
        .iter()
        .filter_map(|c| {
            c.get("person_id")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<Uuid>().ok())
        })
        .collect();

    assert!(
        person_ids.contains(&concentrated_voter),
        "an anomalous concentrated voting_rights arrangement must resolve (§2.5); \
         got {candidates:#?}"
    );
    assert!(
        !person_ids.contains(&gp_holder),
        "a gp_statutory edge must be IGNORED by cooperative_member_strategy \
         (kind filter: voting_rights + board_appointment + dominant_influence \
         ONLY, §2.5); got {candidates:#?}"
    );
    assert_eq!(candidates.len(), 1, "exactly the voter; got {candidates:#?}");

    cleanup(&pool, &[subject]).await;
}

// ── (e) nominee ADMITTED post-TS.4 (the guard set went total) ───────────────

#[tokio::test]
async fn e_select_strategy_admits_nominee_after_ts4() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());

    // TS.4 fixture flip (was `e_select_strategy_still_blocks_nominee_after_
    // widening`): Nominee joined the implemented set (NomineePierceStrategy,
    // K-8 piercing via kyc_ubo.assert.edge.nominee-piercing), so the
    // `[ReconciledProjection, StructureClassSupported]` pair (formerly also
    // carried by `compute-fold`, retired TS.6 P2) on a Nominee-classified
    // subject is now ADMITTED — the old assertion inverted. Probed here via
    // `apply-smo-fallback`, which declares the identical precondition pair
    // and (unlike `freeze`) needs no candidate-producing edges — this
    // fixture has none. The fail-closed floor for unknown/garbage class
    // strings is pinned in kyc_t61_studs.rs and kyc_pack_closure.rs.
    setup_subject(&pool, subject, &[], "nominee").await;
    run(
        &UboEdgeReconcileConflict,
        serde_json::json!({ "subject-id": subject.0 }),
        &pool,
    )
    .await;

    // StructureClassSupported must ADMIT a Nominee-classified subject
    // (widened to the total 11-variant set at TS.4). Previously probed via
    // `apply-smo-fallback`, which declared the identical precondition pair —
    // retired TS.6 §5 (SMO is PULLED on exhaustion by the traversal, never
    // asserted). `freeze` is now the only verb carrying that pair, so it is
    // the probe: it will still error here (K-5 — no candidates and nothing
    // for the SMO pull to find), but that is a DOWNSTREAM refusal reached
    // only AFTER the preconditions admitted. The proof is the absence of the
    // StructureClassSupported refusal text, not success.
    let result = run_fallible(
        &UboDeterminationFreeze,
        serde_json::json!({ "subject-id": subject.0, "policy-version": "v1.0" }),
        &pool,
    )
    .await;
    let msg = match &result {
        Ok(_) => String::new(),
        Err(e) => e.to_string(),
    };
    assert!(
        !msg.contains("has no implemented determination strategy yet"),
        "StructureClassSupported must ADMIT a Nominee-classified subject \
         post-TS.4 (widened to the total set); got: {msg}"
    );

    cleanup(&pool, &[subject]).await;
}

// ── (f) freeze unknown-strategy arm — RETIRED (TS.6 P2) ─────────────────────
//
// `f_freeze_rejects_unknown_strategy_listing_all_eight` drove
// `select-strategy` with a `"no_such_strategy"` string to prove freeze's
// catch-all arm. That entry point is gone: the strategy is now DERIVED
// from `structure_class`, a TOTAL function over all 11 `StructureClass`
// variants (pinned by
// `kyc_pack_closure.rs::precondition_and_strategy_coverage_is_exactly_known`),
// so there is no governed way left to hand freeze an arbitrary strategy
// name — see the identical note on `kyc_ts1_trust.rs`'s deleted
// `f_freeze_still_rejects_unknown_strategy_after_trust_widening`.
