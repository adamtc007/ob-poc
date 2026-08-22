//! 100% live-DB integration coverage for all 20 dsl.kyc verbs
//! (kyc.role.assign/withdraw retired 2026-08-12, T0.3 K-G7 fold-blind write).
//!
//! Each verb is exercised via its `SemOsVerbOp` through a real
//! `VerbExecutionContext` and `TransactionScope`. The test commits to the
//! durable stream and asserts that the event lands with the correct `verb_fqn`.
//! Verbs with preconditions run in natural dependency order
//! (assert → attach-evidence → verify, etc.).
//!
//! Five verbs are already proven in dedicated test files:
//!   kyc_ubo.assert.edge.control          → tests/kyc_stream_ops.rs
//!   kyc_ubo.assert.subject.register             → tests/kyc_stream_ops.rs + kyc_w3_w5_w6.rs
//!   kyc_ubo.assert.obligation.creation            → tests/kyc_w3_w5_w6.rs
//!   kyc_ubo.assert.obligation.satisfaction           → tests/kyc_w3_w5_w6.rs
//!   kyc.person.approve               → tests/kyc_w3_w5_w6.rs
//!
//! This file covers the remaining 15.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionContext};
use ob_poc::domain_ops::kyc_stream_ops::{
    KycObligationCreate, KycObligationSatisfy, KycObligationUpdateIdentity, KycObligationUpdateRisk,
    KycObligationUpdateScreening, KycObligationWaive,
    KycSubjectClassifyStructure, KycSubjectRegister,
    UboDeterminationFreeze, UboEdgeAssertControl,
    UboEdgeAssertEconomicInterest, UboEdgeAttachEvidence, UboEdgeReconcileConflict,
    UboEdgeSupersede, UboEdgeVerify,
};
// kyc.person.approve/.reject renamed kyc_ubo.decide.subject.approve/.reject TS.6 P2 — moved
// to ob-poc-kyc-decide.
use ob_poc_kyc_decide::{DecideApprove, DecideReject};
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

/// Dispatch a verb op and commit. Returns the raw VerbExecutionOutcome as JSON.
async fn run(op: &dyn SemOsVerbOp, args: serde_json::Value, pool: &PgPool) -> serde_json::Value {
    let mut ctx = VerbExecutionContext::default();
    let mut scope = Scope::begin(pool).await;
    let out = op
        .execute(&args, &mut ctx, &mut scope)
        .await
        .unwrap_or_else(|error| panic!("{}: {error}", op.fqn()));
    scope.commit().await;
    serde_json::to_value(format!("{:?}", out)).unwrap()
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
            "kyc_obligation_projection",
            "kyc_subject_rollup_projection",
            // kyc_ubo.decide.subject.approve/kyc_ubo.decide.subject.reject write here now, not the fact
            // stream (TS.6 P2) — cleaned up alongside the other tables.
            "kyc_decision_records",
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

#[tokio::test]
async fn coverage_kyc_obligation_update_identity() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let obligation_id = Uuid::new_v4();
    run(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject.0, "is_natural_person": true }),
        &pool,
    )
    .await;
    run(
        &KycObligationCreate,
        serde_json::json!({
            "subject-id": subject.0, "obligation-id": obligation_id, "role": "beneficial_owner",
        }),
        &pool,
    )
    .await;
    run(
        &KycObligationUpdateIdentity,
        serde_json::json!({
            "subject-id": subject.0, "obligation-id": obligation_id, "state": "satisfied",
        }),
        &pool,
    )
    .await;
    assert_event(&pool, subject, "kyc_ubo.assert.entity.identity").await;
    cleanup(&pool, &[subject]).await;
}

#[tokio::test]
async fn coverage_kyc_obligation_update_screening() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let obligation_id = Uuid::new_v4();
    run(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject.0, "is_natural_person": true }),
        &pool,
    )
    .await;
    run(
        &KycObligationCreate,
        serde_json::json!({
            "subject-id": subject.0, "obligation-id": obligation_id, "role": "investor",
        }),
        &pool,
    )
    .await;
    run(
        &KycObligationUpdateScreening,
        serde_json::json!({
            "subject-id": subject.0, "obligation-id": obligation_id, "state": "satisfied",
        }),
        &pool,
    )
    .await;
    assert_event(&pool, subject, "kyc_ubo.assert.entity.screening").await;
    cleanup(&pool, &[subject]).await;
}

#[tokio::test]
async fn coverage_kyc_obligation_update_risk() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let obligation_id = Uuid::new_v4();
    run(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject.0, "is_natural_person": true }),
        &pool,
    )
    .await;
    run(
        &KycObligationCreate,
        serde_json::json!({
            "subject-id": subject.0, "obligation-id": obligation_id, "role": "controller",
        }),
        &pool,
    )
    .await;
    run(
        &KycObligationUpdateRisk,
        serde_json::json!({
            "subject-id": subject.0, "obligation-id": obligation_id, "state": "in_progress",
        }),
        &pool,
    )
    .await;
    assert_event(&pool, subject, "kyc_ubo.assert.entity.risk").await;
    cleanup(&pool, &[subject]).await;
}

#[tokio::test]
async fn coverage_kyc_obligation_waive() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let obligation_id = Uuid::new_v4();
    run(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject.0, "is_natural_person": false }),
        &pool,
    )
    .await;
    run(
        &KycObligationCreate,
        serde_json::json!({
            "subject-id": subject.0, "obligation-id": obligation_id, "role": "intermediate_entity",
        }),
        &pool,
    )
    .await;
    run(&KycObligationWaive, serde_json::json!({
        "subject-id": subject.0, "obligation-id": obligation_id,
        "reason": "entity is regulated financial institution — simplified due diligence applies",
    }), &pool).await;
    assert_event(&pool, subject, "kyc_ubo.assert.obligation.waiver").await;
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

/// TS.6 §8 `decide_verbs_cite_their_basis`: every verdict records what it
/// relied on — a non-empty obligation-fold snapshot, never a bare stamp.
/// Exercises both landed verdicts: `kyc_ubo.decide.subject.approve` (basis reflects an
/// AllTerminal obligation) and `kyc_ubo.decide.subject.reject` (basis reflects a subject
/// with no obligations yet — rejection is allowed at any stage, but the
/// basis must still be recorded, not omitted because there was "nothing to
/// cite").
#[tokio::test]
async fn decide_verbs_cite_their_basis() {
    let pool = pool().await;

    // kyc_ubo.decide.subject.approve — basis must reflect the AllTerminal obligation.
    let approve_subject = SubjectId(Uuid::new_v4());
    run(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": approve_subject.0, "is_natural_person": true }),
        &pool,
    )
    .await;
    let obligation_id = Uuid::new_v4();
    run(
        &KycObligationCreate,
        serde_json::json!({
            "subject-id": approve_subject.0,
            "obligation-id": obligation_id,
            "role": "beneficial_owner",
            "jurisdiction": "LU",
        }),
        &pool,
    )
    .await;
    run(
        &KycObligationSatisfy,
        serde_json::json!({ "subject-id": approve_subject.0, "obligation-id": obligation_id }),
        &pool,
    )
    .await;
    run(
        &DecideApprove,
        serde_json::json!({ "subject-id": approve_subject.0 }),
        &pool,
    )
    .await;

    let approve_basis: serde_json::Value = sqlx::query_scalar(
        r#"SELECT basis FROM "ob-poc".kyc_decision_records
           WHERE subject_root = $1 AND verb_fqn = 'kyc_ubo.decide.subject.approve'"#,
    )
    .bind(approve_subject.0)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        approve_basis.get("overall_state").and_then(|v| v.as_str()),
        Some("AllTerminal"),
        "kyc_ubo.decide.subject.approve's basis must cite the AllTerminal obligation state it relied on: {approve_basis:?}"
    );
    let approve_obligation_ids = approve_basis
        .get("obligation_ids")
        .and_then(|v| v.as_array())
        .expect("basis.obligation_ids must be an array");
    assert_eq!(
        approve_obligation_ids.len(),
        1,
        "kyc_ubo.decide.subject.approve's basis must name the obligation it relied on: {approve_basis:?}"
    );

    // kyc_ubo.decide.subject.reject — no obligations exist yet, but the basis must still be
    // a real (non-empty) snapshot, not an omitted/null citation.
    let reject_subject = SubjectId(Uuid::new_v4());
    run(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": reject_subject.0, "is_natural_person": true }),
        &pool,
    )
    .await;
    run(
        &DecideReject,
        serde_json::json!({
            "subject-id": reject_subject.0,
            "reason": "sanctions match confirmed",
        }),
        &pool,
    )
    .await;

    let reject_basis: serde_json::Value = sqlx::query_scalar(
        r#"SELECT basis FROM "ob-poc".kyc_decision_records
           WHERE subject_root = $1 AND verb_fqn = 'kyc_ubo.decide.subject.reject'"#,
    )
    .bind(reject_subject.0)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        reject_basis.get("overall_state").and_then(|v| v.as_str()),
        Some("InProgress"),
        "kyc_ubo.decide.subject.reject's basis must still cite the (empty) obligation state, not omit citation: {reject_basis:?}"
    );
    assert!(
        reject_basis.get("obligation_ids").is_some(),
        "kyc_ubo.decide.subject.reject's basis must name the obligation_ids key even when empty: {reject_basis:?}"
    );

    cleanup(&pool, &[approve_subject, reject_subject]).await;
}
