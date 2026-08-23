//! 100% live-DB integration coverage for the dsl.kyc verb surface
//! (kyc.role.assign/withdraw retired 2026-08-12, T0.3 K-G7 fold-blind write;
//! `kyc_ubo.assert.obligation.creation`/`.satisfaction` DISSOLVED and
//! `.waiver` MOVED to `kyc_ubo.decide.obligation.waiver`, D2.0 §5,
//! 2026-08-22).
//!
//! Each verb is exercised via its `SemOsVerbOp` through a real
//! `VerbExecutionContext` and `TransactionScope`. The test commits to the
//! durable stream and asserts that the event lands with the correct `verb_fqn`.
//! Verbs with preconditions run in natural dependency order
//! (assert → attach-evidence → verify, etc.).
//!
//! Verbs already proven in dedicated test files:
//!   kyc_ubo.assert.edge.control          → tests/kyc_stream_ops.rs
//!   kyc_ubo.assert.subject.register             → tests/kyc_stream_ops.rs + kyc_w3_w5_w6.rs
//!
//! `coverage_kyc_obligation_update_identity/screening/risk` below now assert
//! REFUSAL, not success — the disclosed D2.0 P0 consequence: with `creation`
//! dissolved (the only writer of a new `ObligationTracks` entry),
//! `Precondition::ObligationExists` can never again be satisfied for these
//! three verbs. Recorded, not silently left to bit-rot as a mystery failure.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::TransactionScope;
use ob_poc::domain_ops::kyc_stream_ops::{
    KycObligationUpdateIdentity, KycObligationUpdateRisk, KycObligationUpdateScreening,
    KycSubjectClassifyStructure, KycSubjectRegister, UboDeterminationFreeze, UboEdgeAssertControl,
    UboEdgeAssertEconomicInterest, UboEdgeAttachEvidence, UboEdgeReconcileConflict,
    UboEdgeSupersede, UboEdgeVerify,
};
// kyc.person.approve/.reject renamed kyc_ubo.decide.subject.approve/.reject TS.6 P2 — moved
// to ob-poc-kyc-decide. kyc_ubo.decide.obligation.waiver moved there too, D2.0 §5.
use ob_poc_kyc_decide::{DecideApprove, DecideObligationWaive, DecideReject};
use ob_poc_kyc_substrate::SubjectId;
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
        .expect("connect to test DB")
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

/// Dispatch a verb op and commit. Returns the raw VerbExecutionOutcome as
/// JSON. Runs under a real session identity (D2.1 §7 Q3) via the
/// `test-fixtures`-gated `test_verb_execution_context_with_session` — the
/// `decide.*` ops in this file refuse a context with no session_id.
async fn run(op: &dyn SemOsVerbOp, args: serde_json::Value, pool: &PgPool) -> serde_json::Value {
    let mut ctx = ob_poc_kyc_decide::test_verb_execution_context_with_session(Uuid::new_v4());
    let mut scope = Scope::begin(pool).await;
    let out = op
        .execute(&args, &mut ctx, &mut scope)
        .await
        .unwrap_or_else(|error| panic!("{}: {error}", op.fqn()));
    scope.commit().await;
    serde_json::to_value(format!("{:?}", out)).unwrap()
}

/// Dispatch a verb op expecting refusal; commits nothing (rolls back).
async fn run_expect_err(op: &dyn SemOsVerbOp, args: serde_json::Value, pool: &PgPool) -> String {
    let mut ctx = ob_poc_kyc_decide::test_verb_execution_context_with_session(Uuid::new_v4());
    let mut scope = Scope::begin(pool).await;
    let err = op
        .execute(&args, &mut ctx, &mut scope)
        .await
        .expect_err(&format!("{} was expected to be refused", op.fqn()));
    scope.tx.rollback().await.unwrap();
    err.to_string()
}

/// Assert `verb_fqn` appears in kyc_intent_events for subject.
async fn assert_event(pool: &PgPool, subject: SubjectId, verb_fqn: &str) {
    let count: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM "ob-poc".kyc_intent_events
           WHERE subject_root = $1 AND verb_fqn = $2"#,
    )
    .bind(subject.0)
    .bind(verb_fqn)
    .fetch_one(pool)
    .await
    .unwrap();
    assert!(
        count > 0,
        "expected event {verb_fqn} for subject {}",
        subject.0
    );
}

/// kyc_ubo.decide.subject.approve/kyc_ubo.decide.subject.reject write to `kyc_decision_records`, not the
/// fact stream (TS.6 P2) — this is `assert_event`'s counterpart for them.
async fn assert_decision_record(pool: &PgPool, subject: SubjectId, verb_fqn: &str) {
    let count: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM "ob-poc".kyc_decision_records
           WHERE subject_root = $1 AND verb_fqn = $2"#,
    )
    .bind(subject.0)
    .bind(verb_fqn)
    .fetch_one(pool)
    .await
    .unwrap();
    assert!(
        count > 0,
        "expected decision record {verb_fqn} for subject {}",
        subject.0
    );
}

async fn cleanup(pool: &PgPool, subjects: &[SubjectId]) {
    for s in subjects {
        for t in [
            "kyc_intent_events",
            "kyc_subject_streams",
            "kyc_control_edge_projection",
            // kyc_ubo.decide.subject.approve/kyc_ubo.decide.subject.reject write here now, not the fact
            // stream (TS.6 P2) — cleaned up alongside the other tables.
            "kyc_decision_records",
            // D2.0 §4 run book — same cleanup story.
            "kyc_evaluation_runs",
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

// ── Edge lifecycle (ubo.edge.*) ────────────────────────────────────────────────

#[tokio::test]
async fn coverage_ubo_edge_assert_economic_interest() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject.0, "is_natural_person": false }),
        &pool,
    )
    .await;
    run(
        &UboEdgeAssertEconomicInterest,
        serde_json::json!({
            "subject-id": subject.0,
            "from_entity_id": Uuid::new_v4(),
            "to_entity_id": Uuid::new_v4(),
            "percentage": 45.0,
        }),
        &pool,
    )
    .await;
    assert_event(&pool, subject, "kyc_ubo.assert.edge.economic-interest").await;
    cleanup(&pool, &[subject]).await;
}

#[tokio::test]
async fn coverage_ubo_edge_attach_evidence() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let edge = Uuid::new_v4();
    run(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject.0, "is_natural_person": false }),
        &pool,
    )
    .await;
    run(&UboEdgeAssertControl, serde_json::json!({
        "subject-id": subject.0, "edge-id": edge, "edge_id": edge.to_string(),
        "from_entity_id": Uuid::new_v4(), "to_entity_id": Uuid::new_v4(), "kind": "voting_rights",
    }), &pool).await;
    run(
        &UboEdgeAttachEvidence,
        serde_json::json!({ "subject-id": subject.0, "edge-id": edge }),
        &pool,
    )
    .await;
    assert_event(&pool, subject, "kyc_ubo.assert.edge.evidence").await;
    cleanup(&pool, &[subject]).await;
}

#[tokio::test]
async fn coverage_ubo_edge_verify() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let edge = Uuid::new_v4();
    run(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject.0, "is_natural_person": false }),
        &pool,
    )
    .await;
    // Pass edge_id (underscore) so the fold's edge_id_from_payload() finds it
    // and stores the edge under our explicit UUID (not a derived v5).
    run(&UboEdgeAssertControl, serde_json::json!({
        "subject-id": subject.0, "edge-id": edge, "edge_id": edge.to_string(),
        "from_entity_id": Uuid::new_v4(), "to_entity_id": Uuid::new_v4(), "kind": "voting_rights",
    }), &pool).await;
    // Precondition: must attach evidence before verify (EvidenceCited precondition, K-11)
    run(
        &UboEdgeAttachEvidence,
        serde_json::json!({ "subject-id": subject.0, "edge-id": edge }),
        &pool,
    )
    .await;
    run(
        &UboEdgeVerify,
        serde_json::json!({ "subject-id": subject.0, "edge-id": edge }),
        &pool,
    )
    .await;
    assert_event(&pool, subject, "kyc_ubo.assert.edge.verification").await;
    cleanup(&pool, &[subject]).await;
}

#[tokio::test]
async fn coverage_ubo_edge_supersede() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let edge = Uuid::new_v4();
    run(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject.0, "is_natural_person": false }),
        &pool,
    )
    .await;
    run(&UboEdgeAssertControl, serde_json::json!({
        "subject-id": subject.0, "edge-id": edge, "edge_id": edge.to_string(),
        "from_entity_id": Uuid::new_v4(), "to_entity_id": Uuid::new_v4(), "kind": "voting_rights",
    }), &pool).await;
    run(
        &UboEdgeSupersede,
        serde_json::json!({ "subject-id": subject.0, "edge-id": edge }),
        &pool,
    )
    .await;
    assert_event(&pool, subject, "kyc_ubo.assert.edge.supersession").await;
    // K-13: edge still in stream, not deleted
    let edge_count: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM "ob-poc".kyc_intent_events WHERE subject_root = $1 AND verb_fqn = 'kyc_ubo.assert.edge.control'"#,
    ).bind(subject.0).fetch_one(&pool).await.unwrap();
    assert_eq!(
        edge_count, 1,
        "K-13: assert-control event stays in stream after supersede"
    );
    cleanup(&pool, &[subject]).await;
}

#[tokio::test]
async fn coverage_ubo_edge_reconcile_conflict() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject.0, "is_natural_person": false }),
        &pool,
    )
    .await;
    run(&UboEdgeReconcileConflict, serde_json::json!({
        "subject-id": subject.0, "resolution": "dominant edge selected based on date precedence",
    }), &pool).await;
    assert_event(&pool, subject, "kyc_ubo.assert.edge.reconciliation").await;
    cleanup(&pool, &[subject]).await;
}

// ── Determination verbs (ubo.determination.*) ─────────────────────────────────
//
// `coverage_ubo_determination_select_strategy` RETIRED (TS.6 P2, K-G7):
// `ubo.determination.select-strategy` no longer exists — the strategy is
// derived from `structure_class` (`strategy_for_structure_class`), never
// separately asserted.
//
// `coverage_ubo_determination_compute_fold` RETIRED (TS.6 P2, K-G7):
// `ubo.determination.compute-fold` no longer exists — it was a derivation
// dressed as a verb, carrying the identical [ReconciledProjection,
// StructureClassSupported] precondition pair `freeze` already independently
// declares. Nothing needed building to preserve the gate elsewhere; see
// `coverage_ubo_determination_freeze` below and `coverage_ubo_determination_
// apply_smo_fallback`'s own precondition coverage for the surviving proof.

// `coverage_ubo_determination_apply_smo_fallback` RETIRED (TS.6 §5,
// 2026-08-22), mirroring the select-strategy/compute-fold retirements: the
// verb is gone, so there is no live op to cover. SMO now reaches a
// determination only via the traversal's pull-on-exhaustion (TS.3 §4a),
// which is covered by the determination tests, not by a verb-coverage row.

#[tokio::test]
async fn coverage_ubo_determination_freeze() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let owner = Uuid::new_v4();
    run(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject.0, "is_natural_person": false }),
        &pool,
    )
    .await;
    run(
        &KycSubjectClassifyStructure,
        serde_json::json!({
            "subject-id": subject.0, "structure-class": "private_company",
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
    // K-5: a determination must never be silent. This used to be satisfied by
    // asserting an SMO (`ubo.determination.apply-smo-fallback`, retired
    // TS.6 §5 — SMO is PULLED on exhaustion by the traversal, never written).
    // With no manual override left, freeze needs a REAL candidate: a natural
    // person holding ≥ the 25% default threshold directly in the subject.
    run(
        &KycSubjectRegister,
        serde_json::json!({
            "subject-id": subject.0, "entity-id": owner, "is_natural_person": true,
        }),
        &pool,
    )
    .await;
    run(
        &UboEdgeAssertEconomicInterest,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": owner, "to_entity_id": subject.0,
            "percentage": 60.0,
        }),
        &pool,
    )
    .await;
    run(
        &UboDeterminationFreeze,
        serde_json::json!({
            "subject-id": subject.0, "policy-version": "v1.0",
        }),
        &pool,
    )
    .await;
    assert_event(&pool, subject, "kyc_ubo.decide.determination.freeze").await;
    cleanup(&pool, &[subject]).await;
}

// ── Subject taxonomy (kyc.subject.*) ─────────────────────────────────────────

#[tokio::test]
async fn coverage_kyc_subject_classify_structure() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject.0, "is_natural_person": false }),
        &pool,
    )
    .await;
    run(
        &KycSubjectClassifyStructure,
        serde_json::json!({
            "subject-id": subject.0, "structure-class": "private_company",
        }),
        &pool,
    )
    .await;
    assert_event(&pool, subject, "kyc_ubo.assert.subject.structure-class").await;
    cleanup(&pool, &[subject]).await;
}

// ── Role-basis (kyc.role.*) ───────────────────────────────────────────────────

// coverage_kyc_role_withdraw removed 2026-08-12 — kyc.role.assign/withdraw
// retired (T0.3 K-G7 fold-blind write); see dsl-kyc-obligation.yaml's
// retirement comment.

// ── Obligation lifecycle (kyc.obligation.*) ───────────────────────────────────

/// D2.0 P0 disclosed consequence: `creation` (the only writer of a new
/// `ObligationTracks` entry) is dissolved, so `Precondition::ObligationExists`
/// can never again be satisfied — `update-identity` is now permanently
/// refused for any obligation-id, real or fabricated.
#[tokio::test]
async fn coverage_kyc_obligation_update_identity() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject.0, "is_natural_person": true }),
        &pool,
    )
    .await;
    let err = run_expect_err(
        &KycObligationUpdateIdentity,
        serde_json::json!({
            "subject-id": subject.0, "obligation-id": Uuid::new_v4(), "state": "satisfied",
        }),
        &pool,
    )
    .await;
    assert!(
        err.to_lowercase().contains("obligation") && err.to_lowercase().contains("not found"),
        "expected an ObligationExists refusal now that obligation.creation is dissolved: {err}"
    );
    cleanup(&pool, &[subject]).await;
}

/// Same disclosed consequence as `update-identity`, above.
#[tokio::test]
async fn coverage_kyc_obligation_update_screening() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject.0, "is_natural_person": true }),
        &pool,
    )
    .await;
    let err = run_expect_err(
        &KycObligationUpdateScreening,
        serde_json::json!({
            "subject-id": subject.0, "obligation-id": Uuid::new_v4(), "state": "satisfied",
        }),
        &pool,
    )
    .await;
    assert!(
        err.to_lowercase().contains("obligation") && err.to_lowercase().contains("not found"),
        "expected an ObligationExists refusal now that obligation.creation is dissolved: {err}"
    );
    cleanup(&pool, &[subject]).await;
}

/// Same disclosed consequence as `update-identity`, above.
#[tokio::test]
async fn coverage_kyc_obligation_update_risk() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject.0, "is_natural_person": true }),
        &pool,
    )
    .await;
    let err = run_expect_err(
        &KycObligationUpdateRisk,
        serde_json::json!({
            "subject-id": subject.0, "obligation-id": Uuid::new_v4(), "state": "in_progress",
        }),
        &pool,
    )
    .await;
    assert!(
        err.to_lowercase().contains("obligation") && err.to_lowercase().contains("not found"),
        "expected an ObligationExists refusal now that obligation.creation is dissolved: {err}"
    );
    cleanup(&pool, &[subject]).await;
}

/// `kyc_ubo.decide.obligation.waiver` (moved from `kyc_ubo.assert.obligation.waiver`,
/// D2.0 §5) — writes only to `kyc_decision_records`, citing the run, never
/// the fact stream.
#[tokio::test]
async fn coverage_kyc_decide_obligation_waiver() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject.0, "is_natural_person": false }),
        &pool,
    )
    .await;
    run(
        &DecideObligationWaive,
        serde_json::json!({
            "subject-id": subject.0,
            "check-id": "board.every-entity-has-a-proven-type",
            "reason": "entity is regulated financial institution — simplified due diligence applies",
        }),
        &pool,
    )
    .await;
    assert_decision_record(&pool, subject, "kyc_ubo.decide.obligation.waiver").await;
    cleanup(&pool, &[subject]).await;
}

// ── Person decision (kyc.person.*) ────────────────────────────────────────────

#[tokio::test]
async fn coverage_kyc_person_reject() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject.0, "is_natural_person": true }),
        &pool,
    )
    .await;
    run(
        &DecideReject,
        serde_json::json!({
            "subject-id": subject.0,
            "reason": "sanctions match confirmed — PEP designation upheld",
        }),
        &pool,
    )
    .await;
    assert_decision_record(&pool, subject, "kyc_ubo.decide.subject.reject").await;
    cleanup(&pool, &[subject]).await;
}

/// D2.0 §6 `decide_cites_a_run` — replaces TS.6 §8's
/// `decide_verbs_cite_their_basis` (which cited an obligation-fold snapshot,
/// meaningless once `creation` dissolved). Every verdict now cites the
/// `EvaluationRun` it relied on: `basis.run_id` names a row in
/// `kyc_evaluation_runs`, and the cited run's `board_state_hash` matches
/// what `run_id` row actually recorded — not just present, but genuinely
/// the run that ran. Exercises both landed verdicts: `kyc_ubo.decide.subject.approve`
/// (against a genuinely EMPTY board — no production Assembly verb can
/// reach `TypeProofStatus::Proved` today, since `UboEdgeAttachEvidence`
/// always edge-scopes its target and `fold_type_registry` only reads
/// `kyc_ubo.assert.edge.evidence` as a type-proof event when UNSCoped;
/// `ProvenTypeCheck` passes vacuously on an empty board, D2.1 §2, so K-23
/// does not refuse this run) and `kyc_ubo.decide.subject.reject` (citation
/// recorded even though rejection is allowed at any stage, board has one
/// registered-but-untyped entity so `ProvenTypeCheck` returns
/// `Unevaluable` — irrelevant to reject, which never gates on the work
/// list).
#[tokio::test]
async fn decide_cites_a_run() {
    let pool = pool().await;

    for (subject, op, fqn, extra_args, register_first) in [
        (
            SubjectId(Uuid::new_v4()),
            &DecideApprove as &dyn SemOsVerbOp,
            "kyc_ubo.decide.subject.approve",
            serde_json::json!({}),
            false,
        ),
        (
            SubjectId(Uuid::new_v4()),
            &DecideReject as &dyn SemOsVerbOp,
            "kyc_ubo.decide.subject.reject",
            serde_json::json!({ "reason": "sanctions match confirmed" }),
            true,
        ),
    ] {
        if register_first {
            run(
                &KycSubjectRegister,
                serde_json::json!({ "subject-id": subject.0, "is_natural_person": true }),
                &pool,
            )
            .await;
        }
        let mut args = serde_json::json!({ "subject-id": subject.0 });
        for (k, v) in extra_args.as_object().unwrap() {
            args[k] = v.clone();
        }
        run(op, args, &pool).await;

        let basis: serde_json::Value = sqlx::query_scalar(&format!(
            r#"SELECT basis FROM "ob-poc".kyc_decision_records
               WHERE subject_root = $1 AND verb_fqn = '{fqn}'"#
        ))
        .bind(subject.0)
        .fetch_one(&pool)
        .await
        .unwrap();
        let run_id: Uuid = basis
            .get("run_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .unwrap_or_else(|| panic!("{fqn}'s basis must cite a run_id: {basis:?}"));

        let (recorded_hash, recorded_subject): (String, Uuid) = sqlx::query_as(
            r#"SELECT board_state_hash, subject_root FROM "ob-poc".kyc_evaluation_runs
               WHERE run_id = $1"#,
        )
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .unwrap_or_else(|e| panic!("{fqn} cited run {run_id} but no such row exists in kyc_evaluation_runs: {e}"));
        assert_eq!(
            recorded_subject, subject.0,
            "{fqn}'s cited run must belong to the subject it decided about"
        );
        assert_eq!(
            basis.get("board_state_hash").and_then(|v| v.as_str()),
            Some(recorded_hash.as_str()),
            "{fqn}'s basis.board_state_hash must match the cited run's own recorded hash"
        );

        cleanup(&pool, &[subject]).await;
    }
}
