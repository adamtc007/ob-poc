//! TS.4 gate tests — EOP-DD-KYCUBO-KIT-TS0 §2.6 (`ubo.edge.pierce-nominee` +
//! `nominee_pierce_strategy`), ratified 2026-08-12 ACCEPT ALL. TS.4 = K-8:
//! the LAST TS tranche — the only class needing a NEW verb, exercised with
//! full kit citizenship (the K-G7 reintroduction discipline).
//!
//! RED-first against the post-TS.3 tree:
//! - (a) pierce end-to-end through the REAL ops: register → classify
//!   `nominee` → assert a nominee edge → pierce with the disclosed
//!   nominator → the fold shows BOTH effects of the ONE governed event: the
//!   target nominee edge Superseded with `superseded_by` = the pierce
//!   event, and a NEW edge from the nominator with the underlying kind and
//!   `pierced_from` provenance (K-13 supersede-never-contradict; K-35
//!   traceability).
//! - (b) pierce REFUSES a non-nominee target edge (no precondition
//!   primitive expresses "edge is of kind X" — the check is op-layer,
//!   fail-closed, per the §2.6 checklist note).
//! - (b2) the pierce normalizer REFUSES `kind: nominee` (a pierce cannot
//!   produce another nominee edge — fail-closed) and any unknown kind
//!   (the TS.1 §1b wire-normalizer discipline extended to the new verb).
//! - (c) pierce REFUSES an inactive (superseded) target (EdgeActive stud,
//!   matrix row 3/4 vocabulary, enforced at the real append path) and a
//!   non-existent target edge.
//! - (d) freeze with `nominee_pierce_strategy` HARD-ERRORS while an
//!   unpierced nominee edge remains active — `resolve()` cannot signal
//!   error (trait returns `Vec<ProngCandidate>`), so the fail-closed scan
//!   lives at the freeze dispatch site (kyc_stream_ops.rs), per §2.6
//!   "errors if unpierced nominee edges remain active".
//! - (e) post-pierce the strategy delegates to the control prong and
//!   resolves the NOMINATOR chain (the underlying structure), never the
//!   nominee (K-8's whole point).
//! - (f) freeze's unknown-strategy arm still errors on a never-will-exist
//!   name, listing all 8 implemented strategies (nominee_pierce_strategy
//!   included — the TS.3 exemplar `nominee_pierce_strategy` moved to
//!   `no_such_strategy` in this tranche because the arm is now real).
//!
//! FENCED-DEFECT NOTE (same as `kyc_ts1_trust.rs` (d)/(e),
//! `kyc_ts2_fund_foundation.rs`, and `kyc_ts3_state_owned_cooperative.rs`):
//! test (e) cannot drive `ubo.determination.freeze` to SUCCESS — freeze's
//! event payload embeds the candidates, and every ControlByOtherMeans
//! candidate carries `effective_ownership_pct: null`, which trips the
//! PRE-EXISTING render.rs:102 nested-null defect (the exact reason
//! `m4_control_prong_strategy_resolves_gp_statutory_control` and
//! `coverage_ubo_determination_freeze` are fenced pre-existing reds). That
//! defect stays fenced in TS.4, so candidates are resolved via the exact
//! strategy instance freeze dispatches to (arm proven by test (f) +
//! `implemented_class_split_matches_strategy_arms`), over the REAL
//! DB-loaded stream + the same `fold_control_versioned` /
//! `natural_persons_from_events` / `find_subject_entity` composition the
//! freeze op uses. Tests (d)/(f) DO drive the real freeze op — both error
//! paths fire before the payload render. Upgrade (e) to drive freeze
//! directly once the render defect is fixed.
//!
//! DB/fixture pattern mirrors `kyc_ts3_state_owned_cooperative.rs`.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionContext};
use ob_poc::domain_ops::kyc_stream_ops::{
    KycSubjectClassifyStructure, KycSubjectRegister, UboDeterminationFreeze,
    UboDeterminationSelectStrategy, UboEdgeAssertControl, UboEdgePierceNominee,
    UboEdgeReconcileConflict, UboEdgeSupersede,
};
use ob_poc_kyc_substrate::{
    fold_control_versioned, phase1_lexicon, DeterminationStrategy, EdgeKind, EdgeStatus,
    FoldRegistry, IntentEvent, SubjectId, V1FoldImpl,
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

/// Fold the subject's REAL DB stream (the same composition the ops use).
async fn fold_subject(pool: &PgPool, subject: SubjectId) -> ob_poc_kyc_substrate::ControlState {
    let mut conn = pool.acquire().await.expect("acquire connection");
    let events = ob_poc_kyc_store::PgKycEventStore::load_events(&mut conn, subject)
        .await
        .expect("load events");
    let refs: Vec<&IntentEvent> = events.iter().collect();
    let mut reg = FoldRegistry::new();
    reg.register(phase1_lexicon().hash, std::sync::Arc::new(V1FoldImpl));
    fold_control_versioned(&refs, &reg).expect("fold ok")
}

/// Load the subject's REAL stream from the DB, fold it, assert the recorded
/// strategy name, and resolve via the given strategy instance — the exact
/// composition the freeze op uses (see the FENCED-DEFECT NOTE above).
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

// ── (a) pierce end-to-end — supersede + reassert in ONE governed event ──────
//
// §2.6: (a) the target nominee edge is superseded (`superseded_by` = the
// pierce event); (b) a new control edge from the nominator is asserted with
// the underlying kind and `pierced_from` provenance.

#[tokio::test]
async fn a_pierce_supersedes_nominee_and_asserts_nominator_edge() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let nominee = Uuid::new_v4(); // the on-paper holder
    let nominator = Uuid::new_v4(); // the disclosed true holder
    let nominee_edge = Uuid::new_v4();

    setup_subject(&pool, subject, &[nominator], "nominee").await;

    run(
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": nominee, "to_entity_id": subject.0,
            "kind": "nominee", "edge-id": nominee_edge,
        }),
        &pool,
    )
    .await;

    let out = run(
        &UboEdgePierceNominee,
        serde_json::json!({
            "subject-id": subject.0, "edge-id": nominee_edge,
            "nominator-id": nominator, "kind": "voting_rights",
        }),
        &pool,
    )
    .await;
    assert!(out.get("seq").is_some(), "pierce must append: {out:?}");

    let state = fold_subject(&pool, subject).await;
    let old = state
        .edges
        .get(&ob_poc_kyc_substrate::EdgeId(nominee_edge))
        .expect("nominee edge stays in the fold (K-13 supersede-never-delete)");
    assert_eq!(
        old.status,
        EdgeStatus::Superseded,
        "effect (a): the nominee edge must be Superseded"
    );
    assert!(
        old.superseded_by.is_some(),
        "effect (a): superseded_by must point at the pierce event (K-35)"
    );

    let new_edge = state
        .edges
        .values()
        .find(|e| e.from.0 == nominator && e.is_active())
        .expect("effect (b): a new active edge from the nominator must exist");
    assert_eq!(
        new_edge.kind,
        EdgeKind::VotingRights,
        "the new edge carries the UNDERLYING kind, never nominee"
    );
    assert_eq!(
        new_edge.to.0, subject.0,
        "the new edge targets the same controlled entity"
    );
    assert_eq!(
        new_edge.pierced_from,
        Some(ob_poc_kyc_substrate::EdgeId(nominee_edge)),
        "the new edge carries pierced_from provenance (§2.6)"
    );
    assert_eq!(
        new_edge.originating_event_id,
        old.superseded_by.unwrap(),
        "both effects originate from the SAME governed pierce event"
    );

    cleanup(&pool, &[subject]).await;
}

// ── (b) pierce refuses a non-nominee target edge ────────────────────────────

#[tokio::test]
async fn b_pierce_refuses_non_nominee_target_edge() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let holder = Uuid::new_v4();
    let nominator = Uuid::new_v4();
    let edge = Uuid::new_v4();

    setup_subject(&pool, subject, &[nominator], "nominee").await;
    run(
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": holder, "to_entity_id": subject.0,
            "kind": "voting_rights", "edge-id": edge,
        }),
        &pool,
    )
    .await;

    let result = run_fallible(
        &UboEdgePierceNominee,
        serde_json::json!({
            "subject-id": subject.0, "edge-id": edge,
            "nominator-id": nominator, "kind": "voting_rights",
        }),
        &pool,
    )
    .await;
    assert!(
        result.is_err(),
        "pierce must refuse a target edge that is not EdgeKind::Nominee"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("not a nominee edge"),
        "rejection must be attributable to the kind check; got: {msg}"
    );

    cleanup(&pool, &[subject]).await;
}

// ── (b2) normalizer refuses kind=nominee and unknown kinds ──────────────────

#[tokio::test]
async fn b2_pierce_normalizer_rejects_nominee_and_unknown_kinds() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let nominee = Uuid::new_v4();
    let nominator = Uuid::new_v4();
    let edge = Uuid::new_v4();

    setup_subject(&pool, subject, &[nominator], "nominee").await;
    run(
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": nominee, "to_entity_id": subject.0,
            "kind": "nominee", "edge-id": edge,
        }),
        &pool,
    )
    .await;

    // A pierce cannot produce another nominee edge (fail-closed, §2.6).
    let nominee_kind = run_fallible(
        &UboEdgePierceNominee,
        serde_json::json!({
            "subject-id": subject.0, "edge-id": edge,
            "nominator-id": nominator, "kind": "nominee",
        }),
        &pool,
    )
    .await;
    assert!(nominee_kind.is_err(), "kind=nominee must be rejected");
    let msg = nominee_kind.unwrap_err().to_string();
    assert!(
        msg.contains("cannot produce another nominee edge"),
        "rejection must name the nominee-kind rule; got: {msg}"
    );

    // Unknown kind — the TS.1 §1b wire-normalizer discipline, fail-closed.
    let unknown = run_fallible(
        &UboEdgePierceNominee,
        serde_json::json!({
            "subject-id": subject.0, "edge-id": edge,
            "nominator-id": nominator, "kind": "votng_rights",
        }),
        &pool,
    )
    .await;
    assert!(
        unknown.is_err(),
        "an unknown kind must be rejected fail-closed"
    );
    let msg = unknown.unwrap_err().to_string();
    assert!(
        msg.contains("unrecognized kind"),
        "rejection must be the wire-normalizer's; got: {msg}"
    );

    cleanup(&pool, &[subject]).await;
}

// ── (c) pierce refuses inactive/superseded and non-existent targets ─────────

#[tokio::test]
async fn c_pierce_refuses_superseded_and_missing_targets() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let nominee = Uuid::new_v4();
    let nominator = Uuid::new_v4();
    let edge = Uuid::new_v4();

    setup_subject(&pool, subject, &[nominator], "nominee").await;
    run(
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": nominee, "to_entity_id": subject.0,
            "kind": "nominee", "edge-id": edge,
        }),
        &pool,
    )
    .await;
    run(
        &UboEdgeSupersede,
        serde_json::json!({ "subject-id": subject.0, "edge-id": edge }),
        &pool,
    )
    .await;

    // Superseded target — the EdgeActive stud (matrix rows 3/4) refuses.
    let inactive = run_fallible(
        &UboEdgePierceNominee,
        serde_json::json!({
            "subject-id": subject.0, "edge-id": edge,
            "nominator-id": nominator, "kind": "voting_rights",
        }),
        &pool,
    )
    .await;
    assert!(
        inactive.is_err(),
        "pierce must refuse a superseded (inactive) target edge"
    );

    // Non-existent target — refused before anything appends.
    let missing = run_fallible(
        &UboEdgePierceNominee,
        serde_json::json!({
            "subject-id": subject.0, "edge-id": Uuid::new_v4(),
            "nominator-id": nominator, "kind": "voting_rights",
        }),
        &pool,
    )
    .await;
    assert!(
        missing.is_err(),
        "pierce must refuse a non-existent target edge"
    );

    cleanup(&pool, &[subject]).await;
}

// ── (d) freeze fail-closes while unpierced nominee edges remain ─────────────
//
// §2.6: `resolve()` cannot error (trait signature), so the fail-closed scan
// lives at the freeze dispatch site.

#[tokio::test]
async fn d_freeze_hard_errors_while_unpierced_nominee_edge_active() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let nominee = Uuid::new_v4();
    let nominator = Uuid::new_v4();
    let nominee_edge = Uuid::new_v4();

    setup_subject(&pool, subject, &[nominator], "nominee").await;
    run(
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": nominee, "to_entity_id": subject.0,
            "kind": "nominee", "edge-id": nominee_edge,
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
    // Nominee-classified subject is now ADMITTED (StructureClassSupported
    // widened at TS.4) — pre-TS.4 this call fails-closed.
    let select = run_fallible(
        &UboDeterminationSelectStrategy,
        serde_json::json!({ "subject-id": subject.0, "strategy": "nominee_pierce_strategy" }),
        &pool,
    )
    .await;
    assert!(
        select.is_ok(),
        "select-strategy on a Nominee-classified subject must be admitted \
         post-TS.4 (StructureClassSupported widened): {select:?}"
    );

    let result = run_fallible(
        &UboDeterminationFreeze,
        serde_json::json!({ "subject-id": subject.0, "policy-version": "v1.0" }),
        &pool,
    )
    .await;
    assert!(
        result.is_err(),
        "freeze under nominee_pierce_strategy must hard-error while an \
         unpierced nominee edge remains active (K-8 fail-closed)"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("unpierced") && msg.contains(&nominee_edge.to_string()),
        "the error must name the unpierced edge(s); got: {msg}"
    );

    cleanup(&pool, &[subject]).await;
}

// ── (e) post-pierce the strategy delegates and resolves the nominator ───────

#[tokio::test]
async fn e_post_pierce_strategy_resolves_nominator_chain() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let nominee = Uuid::new_v4();
    let nominator = Uuid::new_v4();
    let nominee_edge = Uuid::new_v4();

    setup_subject(&pool, subject, &[nominator], "nominee").await;
    run(
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": nominee, "to_entity_id": subject.0,
            "kind": "nominee", "edge-id": nominee_edge,
        }),
        &pool,
    )
    .await;
    run(
        &UboEdgePierceNominee,
        serde_json::json!({
            "subject-id": subject.0, "edge-id": nominee_edge,
            "nominator-id": nominator, "kind": "voting_rights",
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
        serde_json::json!({ "subject-id": subject.0, "strategy": "nominee_pierce_strategy" }),
        &pool,
    )
    .await;
    assert!(
        select.is_ok(),
        "select-strategy must be admitted post-TS.4: {select:?}"
    );

    let candidates = resolve_via_strategy(
        &pool,
        subject,
        "nominee_pierce_strategy",
        &ob_poc_kyc_substrate::NomineePierceStrategy,
    )
    .await;

    assert_eq!(
        candidates.len(),
        1,
        "expected exactly the disclosed nominator; got {candidates:#?}"
    );
    assert_eq!(
        candidates[0].person_id.0, nominator,
        "post-pierce, the NOMINATOR (never the nominee) is the candidate (K-8)"
    );
    assert_eq!(
        candidates[0].prong,
        ob_poc_kyc_substrate::Prong::ControlByOtherMeans,
        "K-1 basis: the delegated control walk yields ControlByOtherMeans"
    );
    assert!(
        candidates[0].effective_ownership_pct.is_none(),
        "control carries no quantum — effective_ownership_pct must be None"
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
    // TS.4 fixture fix: nominee_pierce_strategy gained a real dispatch arm
    // (NomineePierceStrategy) — the unknown-arm exemplar is now a
    // never-will-exist string.
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
