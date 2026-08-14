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
//! legs of (a)/(c)/(d) now drive `ubo.determination.freeze` LIVE and read
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
    KycSubjectClassifyStructure, KycSubjectRegister, UboDeterminationFreeze,
    UboDeterminationSelectStrategy, UboEdgeAssertControl, UboEdgeAssertEconomicInterest,
    UboEdgeReconcileConflict,
};
use ob_poc_kyc_substrate::{
    fold_control_versioned, phase1_lexicon, DeterminationStrategy, FoldRegistry, IntentEvent,
    SubjectId, V1FoldImpl,
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

async fn cleanup(pool: &PgPool, subjects: &[SubjectId]) {
    for s in subjects {
        for t in [
            "kyc_intent_events",
            "kyc_subject_streams",
            "kyc_control_edge_projection",
            "kyc_obligation_projection",
            "kyc_subject_rollup_projection",
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
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject.0, "is_natural_person": false }),
        pool,
    )
    .await;
    for p in persons {
        run(
            &KycSubjectRegister,
            serde_json::json!({
                "subject-id": subject.0, "entity-id": p, "is_natural_person": true,
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
    reg.register(phase1_lexicon().hash, std::sync::Arc::new(V1FoldImpl));
    let control = fold_control_versioned(&refs, &reg).expect("fold ok");
    assert_eq!(
        control.selected_strategy.as_deref(),
        Some(expected_strategy),
        "select-strategy (real op) must have recorded {expected_strategy}"
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

    // The gate widening proven at the REAL op: select-strategy on a
    // StateOwned-classified subject is now ADMITTED
    // (StructureClassSupported) — pre-TS.3 this call fails-closed.
    let select = run_fallible(
        &UboDeterminationSelectStrategy,
        serde_json::json!({ "subject-id": subject.0, "strategy": "state_owned_strategy" }),
        &pool,
    )
    .await;
    assert!(
        select.is_ok(),
        "select-strategy on a StateOwned-classified subject must be admitted \
         post-TS.3 (StructureClassSupported widened): {select:?}"
    );

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
    let select = run_fallible(
        &UboDeterminationSelectStrategy,
        serde_json::json!({ "subject-id": subject.0, "strategy": "state_owned_strategy" }),
        &pool,
    )
    .await;
    assert!(
        select.is_ok(),
        "select-strategy on a StateOwned-classified subject must be admitted \
         post-TS.3: {select:?}"
    );

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

    // The gate widening proven at the REAL op: select-strategy on a
    // Cooperative-classified subject is now ADMITTED — pre-TS.3 this call
    // fails-closed.
    let select = run_fallible(
        &UboDeterminationSelectStrategy,
        serde_json::json!({ "subject-id": subject.0, "strategy": "cooperative_member_strategy" }),
        &pool,
    )
    .await;
    assert!(
        select.is_ok(),
        "select-strategy on a Cooperative-classified subject must be admitted \
         post-TS.3 (StructureClassSupported widened): {select:?}"
    );

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
    run(
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": gp_holder, "to_entity_id": subject.0,
            "kind": "gp_statutory",
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
    let select = run_fallible(
        &UboDeterminationSelectStrategy,
        serde_json::json!({ "subject-id": subject.0, "strategy": "cooperative_member_strategy" }),
        &pool,
    )
    .await;
    assert!(select.is_ok(), "select-strategy must be admitted post-TS.3: {select:?}");

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
    // K-8 piercing via ubo.edge.pierce-nominee), so select-strategy on a
    // Nominee-classified subject is now ADMITTED — the old assertion
    // inverted. The fail-closed floor for unknown/garbage class strings is
    // pinned in kyc_t61_studs.rs and kyc_pack_closure.rs.
    setup_subject(&pool, subject, &[], "nominee").await;

    let result = run_fallible(
        &UboDeterminationSelectStrategy,
        serde_json::json!({ "subject-id": subject.0, "strategy": "nominee_pierce_strategy" }),
        &pool,
    )
    .await;
    assert!(
        result.is_ok(),
        "select-strategy on a Nominee-classified subject must be ADMITTED \
         post-TS.4 (StructureClassSupported widened to the total set): {result:?}"
    );

    cleanup(&pool, &[subject]).await;
}

// ── (f) freeze unknown-strategy arm errors, listing all 8 ───────────────────

#[tokio::test]
async fn f_freeze_rejects_unknown_strategy_listing_all_eight() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let chair = Uuid::new_v4();

    setup_subject(&pool, subject, &[chair], "cooperative").await;
    run(
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": chair, "to_entity_id": subject.0,
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
    // A strategy name with no arm behind it — TS.4 fixture fix:
    // nominee_pierce_strategy gained a real dispatch arm, so the exemplar
    // moved to a never-will-exist string (as the TS.3 note predicted).
    run(
        &UboDeterminationSelectStrategy,
        serde_json::json!({ "subject-id": subject.0, "strategy": "no_such_strategy" }),
        &pool,
    )
    .await;

    let result = run_fallible(
        &UboDeterminationFreeze,
        serde_json::json!({ "subject-id": subject.0, "policy-version": "v1.0" }),
        &pool,
    )
    .await;
    assert!(
        result.is_err(),
        "freeze must refuse an unimplemented strategy rather than silently \
         substituting a registered one"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("no_such_strategy") && msg.contains("no DeterminationStrategy"),
        "error should name the missing strategy; got: {msg}"
    );
    for implemented in [
        "ownership_prong_strategy",
        "control_prong_strategy",
        "trust_role_strategy",
        "fund_control_strategy",
        "foundation_council_strategy",
        "state_owned_strategy",
        "cooperative_member_strategy",
        "nominee_pierce_strategy",
    ] {
        assert!(
            msg.contains(implemented),
            "the rejection must list all 8 implemented strategies (expected \
             {implemented} in: {msg})"
        );
    }

    cleanup(&pool, &[subject]).await;
}
