//! 100% live-DB integration coverage for all 23 dsl.kyc verbs.
//!
//! Each verb is exercised via its SemOsVerbOp through a real VerbExecutionContext
//! + TransactionScope, committing to the durable stream and asserting the event
//! lands with the correct verb_fqn. Verbs that have preconditions are tested in
//! natural dependency order (assert → attach-evidence → verify, etc.).
//!
//! Six verbs are already proven in dedicated test files:
//!   ubo.edge.assert-control          → tests/kyc_stream_ops.rs
//!   kyc.subject.register             → tests/kyc_stream_ops.rs + kyc_w3_w5_w6.rs
//!   kyc.role.assign                  → tests/kyc_w3_w5_w6.rs
//!   kyc.obligation.create            → tests/kyc_w3_w5_w6.rs
//!   kyc.obligation.satisfy           → tests/kyc_w3_w5_w6.rs
//!   kyc.person.approve               → tests/kyc_w3_w5_w6.rs
//!
//! This file covers the remaining 17.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionContext};
use ob_poc::domain_ops::kyc_stream_ops::{
    KycObligationUpdateIdentity, KycObligationUpdateRisk, KycObligationUpdateScreening,
    KycObligationWaive, KycPersonReject, KycRoleWithdraw, KycSubjectClassifyStructure,
    KycSubjectRegister, KycObligationCreate, KycRoleAssign,
    UboBoardControllerOverride, UboDeterminationApplySmoFallback, UboDeterminationComputeFold,
    UboDeterminationFreeze, UboDeterminationSelectStrategy, UboEdgeAssertControl,
    UboEdgeAssertEconomicInterest, UboEdgeAttachEvidence, UboEdgeReconcileConflict,
    UboEdgeSupersede, UboEdgeVerify,
};
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
        Self { tx: p.begin().await.unwrap(), pool: p.clone(), id: TransactionScopeId::new() }
    }
    async fn commit(self) { self.tx.commit().await.unwrap(); }
}
impl TransactionScope for Scope {
    fn scope_id(&self) -> TransactionScopeId { self.id }
    fn transaction(&mut self) -> &mut Transaction<'static, Postgres> { &mut self.tx }
    fn pool(&self) -> &PgPool { &self.pool }
}

/// Dispatch a verb op and commit. Returns the raw VerbExecutionOutcome as JSON.
async fn run(op: &dyn SemOsVerbOp, args: serde_json::Value, pool: &PgPool) -> serde_json::Value {
    let mut ctx = VerbExecutionContext::default();
    let mut scope = Scope::begin(pool).await;
    let out = op.execute(&args, &mut ctx, &mut scope).await.expect(op.fqn());
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
    assert!(count > 0, "expected event {verb_fqn} for subject {}", subject.0);
}

async fn cleanup(pool: &PgPool, subjects: &[SubjectId]) {
    for s in subjects {
        for t in ["kyc_intent_events","kyc_subject_streams","kyc_control_edge_projection",
                  "kyc_obligation_projection","kyc_subject_rollup_projection"] {
            let _ = sqlx::query(&format!(r#"DELETE FROM "ob-poc".{t} WHERE subject_root = $1"#))
                .bind(s.0).execute(pool).await;
        }
        let _ = sqlx::query(
            r#"DELETE FROM "public".outbox WHERE idempotency_key LIKE $1
               OR (payload->>'determination_subject')::text = $2
               OR (payload->>'subject_root')::text = $2"#
        )
        .bind(format!("{}:%", s.0))
        .bind(s.0.to_string())
        .execute(pool).await;
    }
}

// ── Edge lifecycle (ubo.edge.*) ────────────────────────────────────────────────

#[tokio::test]
async fn coverage_ubo_edge_assert_economic_interest() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(&KycSubjectRegister, serde_json::json!({ "subject-id": subject.0, "is_natural_person": false }), &pool).await;
    run(&UboEdgeAssertEconomicInterest, serde_json::json!({
        "subject-id": subject.0,
        "from_entity_id": Uuid::new_v4(),
        "to_entity_id": Uuid::new_v4(),
        "percentage": 45.0,
    }), &pool).await;
    assert_event(&pool, subject, "ubo.edge.assert-economic-interest").await;
    cleanup(&pool, &[subject]).await;
}

#[tokio::test]
async fn coverage_ubo_edge_attach_evidence() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let edge = Uuid::new_v4();
    run(&KycSubjectRegister, serde_json::json!({ "subject-id": subject.0, "is_natural_person": false }), &pool).await;
    run(&UboEdgeAssertControl, serde_json::json!({
        "subject-id": subject.0, "edge-id": edge, "edge_id": edge.to_string(),
        "from_entity_id": Uuid::new_v4(), "to_entity_id": Uuid::new_v4(), "edge_kind": "voting_rights",
    }), &pool).await;
    run(&UboEdgeAttachEvidence, serde_json::json!({ "subject-id": subject.0, "edge-id": edge }), &pool).await;
    assert_event(&pool, subject, "ubo.edge.attach-evidence").await;
    cleanup(&pool, &[subject]).await;
}

#[tokio::test]
async fn coverage_ubo_edge_verify() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let edge = Uuid::new_v4();
    run(&KycSubjectRegister, serde_json::json!({ "subject-id": subject.0, "is_natural_person": false }), &pool).await;
    // Pass edge_id (underscore) so the fold's edge_id_from_payload() finds it
    // and stores the edge under our explicit UUID (not a derived v5).
    run(&UboEdgeAssertControl, serde_json::json!({
        "subject-id": subject.0, "edge-id": edge, "edge_id": edge.to_string(),
        "from_entity_id": Uuid::new_v4(), "to_entity_id": Uuid::new_v4(), "edge_kind": "voting_rights",
    }), &pool).await;
    // Precondition: must attach evidence before verify (EvidenceCited precondition, K-11)
    run(&UboEdgeAttachEvidence, serde_json::json!({ "subject-id": subject.0, "edge-id": edge }), &pool).await;
    run(&UboEdgeVerify, serde_json::json!({ "subject-id": subject.0, "edge-id": edge }), &pool).await;
    assert_event(&pool, subject, "ubo.edge.verify").await;
    cleanup(&pool, &[subject]).await;
}

#[tokio::test]
async fn coverage_ubo_edge_supersede() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let edge = Uuid::new_v4();
    run(&KycSubjectRegister, serde_json::json!({ "subject-id": subject.0, "is_natural_person": false }), &pool).await;
    run(&UboEdgeAssertControl, serde_json::json!({
        "subject-id": subject.0, "edge-id": edge, "edge_id": edge.to_string(),
        "from_entity_id": Uuid::new_v4(), "to_entity_id": Uuid::new_v4(), "edge_kind": "voting_rights",
    }), &pool).await;
    run(&UboEdgeSupersede, serde_json::json!({ "subject-id": subject.0, "edge-id": edge }), &pool).await;
    assert_event(&pool, subject, "ubo.edge.supersede").await;
    // K-13: edge still in stream, not deleted
    let edge_count: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM "ob-poc".kyc_intent_events WHERE subject_root = $1 AND verb_fqn = 'ubo.edge.assert-control'"#,
    ).bind(subject.0).fetch_one(&pool).await.unwrap();
    assert_eq!(edge_count, 1, "K-13: assert-control event stays in stream after supersede");
    cleanup(&pool, &[subject]).await;
}

#[tokio::test]
async fn coverage_ubo_edge_reconcile_conflict() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(&KycSubjectRegister, serde_json::json!({ "subject-id": subject.0, "is_natural_person": false }), &pool).await;
    run(&UboEdgeReconcileConflict, serde_json::json!({
        "subject-id": subject.0, "resolution": "dominant edge selected based on date precedence",
    }), &pool).await;
    assert_event(&pool, subject, "ubo.edge.reconcile-conflict").await;
    cleanup(&pool, &[subject]).await;
}

// ── Determination verbs (ubo.determination.*) ─────────────────────────────────

#[tokio::test]
async fn coverage_ubo_determination_select_strategy() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(&KycSubjectRegister, serde_json::json!({ "subject-id": subject.0, "is_natural_person": false }), &pool).await;
    run(&UboDeterminationSelectStrategy, serde_json::json!({
        "subject-id": subject.0, "strategy": "ownership_prong",
    }), &pool).await;
    assert_event(&pool, subject, "ubo.determination.select-strategy").await;
    cleanup(&pool, &[subject]).await;
}

#[tokio::test]
async fn coverage_ubo_determination_compute_fold() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(&KycSubjectRegister, serde_json::json!({ "subject-id": subject.0, "is_natural_person": false }), &pool).await;
    // compute-fold is a pure read — returns a summary, no new event appended
    run(&UboDeterminationComputeFold, serde_json::json!({ "subject-id": subject.0 }), &pool).await;
    // Only the register event is in the stream (compute-fold is a read)
    let count: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM "ob-poc".kyc_intent_events WHERE subject_root = $1"#,
    ).bind(subject.0).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1, "compute-fold is a pure read — no event appended");
    cleanup(&pool, &[subject]).await;
}

#[tokio::test]
async fn coverage_ubo_determination_apply_smo_fallback() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let smo_person = Uuid::new_v4();
    run(&KycSubjectRegister, serde_json::json!({ "subject-id": subject.0, "is_natural_person": false }), &pool).await;
    run(&UboDeterminationApplySmoFallback, serde_json::json!({
        "subject-id": subject.0, "smo-person-id": smo_person,
    }), &pool).await;
    assert_event(&pool, subject, "ubo.determination.apply-smo-fallback").await;
    cleanup(&pool, &[subject]).await;
}

#[tokio::test]
async fn coverage_ubo_determination_freeze() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(&KycSubjectRegister, serde_json::json!({ "subject-id": subject.0, "is_natural_person": false }), &pool).await;
    run(&KycSubjectClassifyStructure, serde_json::json!({
        "subject-id": subject.0, "structure-class": "private_company",
    }), &pool).await;
    run(&UboEdgeReconcileConflict, serde_json::json!({ "subject-id": subject.0 }), &pool).await;
    run(&UboDeterminationSelectStrategy, serde_json::json!({
        "subject-id": subject.0, "strategy": "ownership_prong_strategy",
    }), &pool).await;
    // No qualifying economic edges asserted — K-5 requires an SMO fallback so the
    // determination is never silent.
    run(&UboDeterminationApplySmoFallback, serde_json::json!({
        "subject-id": subject.0, "smo_person_id": Uuid::new_v4(),
    }), &pool).await;
    run(&UboDeterminationFreeze, serde_json::json!({
        "subject-id": subject.0, "policy-version": "v1.0",
    }), &pool).await;
    assert_event(&pool, subject, "ubo.determination.freeze").await;
    cleanup(&pool, &[subject]).await;
}

// ── D3: Board-controller override ─────────────────────────────────────────────

#[tokio::test]
async fn coverage_ubo_board_controller_override() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let controller = Uuid::new_v4();
    run(&KycSubjectRegister, serde_json::json!({ "subject-id": subject.0, "is_natural_person": false }), &pool).await;
    run(&UboBoardControllerOverride, serde_json::json!({
        "subject-id": subject.0,
        "controller-entity-id": controller,
        "basis": "senior managing official designation by board resolution 2026-01-15",
    }), &pool).await;
    assert_event(&pool, subject, "ubo.board-controller.override").await;
    cleanup(&pool, &[subject]).await;
}

// ── Subject taxonomy (kyc.subject.*) ─────────────────────────────────────────

#[tokio::test]
async fn coverage_kyc_subject_classify_structure() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(&KycSubjectRegister, serde_json::json!({ "subject-id": subject.0, "is_natural_person": false }), &pool).await;
    run(&KycSubjectClassifyStructure, serde_json::json!({
        "subject-id": subject.0, "structure-class": "private_company",
    }), &pool).await;
    assert_event(&pool, subject, "kyc.subject.classify-structure").await;
    cleanup(&pool, &[subject]).await;
}

// ── Role-basis (kyc.role.*) ───────────────────────────────────────────────────

#[tokio::test]
async fn coverage_kyc_role_withdraw() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(&KycSubjectRegister, serde_json::json!({ "subject-id": subject.0, "is_natural_person": true }), &pool).await;
    run(&KycRoleAssign, serde_json::json!({ "subject-id": subject.0, "role": "signatory" }), &pool).await;
    run(&KycRoleWithdraw, serde_json::json!({
        "subject-id": subject.0, "role": "signatory", "reason": "no longer authorised signatory",
    }), &pool).await;
    assert_event(&pool, subject, "kyc.role.withdraw").await;
    // K-13 analogue: assign event stays in stream
    let assigns: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM "ob-poc".kyc_intent_events WHERE subject_root=$1 AND verb_fqn='kyc.role.assign'"#,
    ).bind(subject.0).fetch_one(&pool).await.unwrap();
    assert_eq!(assigns, 1, "role.assign event stays in stream after withdraw");
    cleanup(&pool, &[subject]).await;
}

// ── Obligation lifecycle (kyc.obligation.*) ───────────────────────────────────

#[tokio::test]
async fn coverage_kyc_obligation_update_identity() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let obligation_id = Uuid::new_v4();
    run(&KycSubjectRegister, serde_json::json!({ "subject-id": subject.0, "is_natural_person": true }), &pool).await;
    run(&KycObligationCreate, serde_json::json!({
        "subject-id": subject.0, "obligation-id": obligation_id, "role": "beneficial_owner",
    }), &pool).await;
    run(&KycObligationUpdateIdentity, serde_json::json!({
        "subject-id": subject.0, "obligation-id": obligation_id, "state": "satisfied",
    }), &pool).await;
    assert_event(&pool, subject, "kyc.obligation.update-identity").await;
    cleanup(&pool, &[subject]).await;
}

#[tokio::test]
async fn coverage_kyc_obligation_update_screening() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let obligation_id = Uuid::new_v4();
    run(&KycSubjectRegister, serde_json::json!({ "subject-id": subject.0, "is_natural_person": true }), &pool).await;
    run(&KycObligationCreate, serde_json::json!({
        "subject-id": subject.0, "obligation-id": obligation_id, "role": "investor",
    }), &pool).await;
    run(&KycObligationUpdateScreening, serde_json::json!({
        "subject-id": subject.0, "obligation-id": obligation_id, "state": "satisfied",
    }), &pool).await;
    assert_event(&pool, subject, "kyc.obligation.update-screening").await;
    cleanup(&pool, &[subject]).await;
}

#[tokio::test]
async fn coverage_kyc_obligation_update_risk() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let obligation_id = Uuid::new_v4();
    run(&KycSubjectRegister, serde_json::json!({ "subject-id": subject.0, "is_natural_person": true }), &pool).await;
    run(&KycObligationCreate, serde_json::json!({
        "subject-id": subject.0, "obligation-id": obligation_id, "role": "controller",
    }), &pool).await;
    run(&KycObligationUpdateRisk, serde_json::json!({
        "subject-id": subject.0, "obligation-id": obligation_id, "state": "in_progress",
    }), &pool).await;
    assert_event(&pool, subject, "kyc.obligation.update-risk").await;
    cleanup(&pool, &[subject]).await;
}

#[tokio::test]
async fn coverage_kyc_obligation_waive() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let obligation_id = Uuid::new_v4();
    run(&KycSubjectRegister, serde_json::json!({ "subject-id": subject.0, "is_natural_person": false }), &pool).await;
    run(&KycObligationCreate, serde_json::json!({
        "subject-id": subject.0, "obligation-id": obligation_id, "role": "intermediate_entity",
    }), &pool).await;
    run(&KycObligationWaive, serde_json::json!({
        "subject-id": subject.0, "obligation-id": obligation_id,
        "reason": "entity is regulated financial institution — simplified due diligence applies",
    }), &pool).await;
    assert_event(&pool, subject, "kyc.obligation.waive").await;
    cleanup(&pool, &[subject]).await;
}

// ── Person decision (kyc.person.*) ────────────────────────────────────────────

#[tokio::test]
async fn coverage_kyc_person_reject() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(&KycSubjectRegister, serde_json::json!({ "subject-id": subject.0, "is_natural_person": true }), &pool).await;
    run(&KycPersonReject, serde_json::json!({
        "subject-id": subject.0,
        "reason": "sanctions match confirmed — PEP designation upheld",
    }), &pool).await;
    assert_event(&pool, subject, "kyc.person.reject").await;
    cleanup(&pool, &[subject]).await;
}
