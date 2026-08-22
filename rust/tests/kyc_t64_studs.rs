//! T6.4 gate tests — EOP-PLAN-KYCUBO-KIT-001 §T6.4 (obligation/person family,
//! closes the ratified EOP-DD-KYCUBO-KIT-T6 matrix rows 11-18).
//!
//! Every row in this family is cross-fold (⊗) — the stud reads
//! `ObligationState` (and, for row 11, `ControlState.registered`) via the
//! T6.1 unified checker. Sixteen gates (8 verbs x block/admit), each driven
//! through the REAL governed append path (live DB) — the real
//! `SemOsVerbOp::execute()` for the verb under test, mirroring
//! `kyc_t62_studs.rs`/`kyc_t63_studs.rs`'s discipline.
//!
//! Row coverage:
//! - row 11 (`kyc_ubo.assert.obligation.creation`): `SubjectRegistered` — block an
//!   unregistered subject (THE motivating cross-fold case for the T6.1
//!   unified checker).
//! - rows 12-14 (`update-identity`/`update-screening`/`update-risk`):
//!   `ObligationExists` — block a target that doesn't exist.
//! - row 15 (`satisfy`): `SubjectNotDecided` specifically — block satisfying
//!   an obligation that DOES exist once the subject has already been
//!   decided (approved), proving the second stud independently of
//!   `ObligationExists` (rows 12-14/16 already prove that one).
//! - row 16 (`waive`): `ObligationExists` — block a target that doesn't exist.
//! - row 17 (`kyc.person.approve`): `SubjectAllTerminal` — THE K-23 gate,
//!   block approval while an obligation track is still Pending.
//! - row 18 (`kyc.person.reject`): `SubjectNotDecided` — block a second
//!   decision once the subject has already been approved.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionContext};
use ob_poc::domain_ops::kyc_stream_ops::{
    KycObligationCreate, KycObligationSatisfy, KycObligationUpdateIdentity,
    KycObligationUpdateRisk, KycObligationUpdateScreening, KycObligationWaive,
    KycSubjectRegister,
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

async fn cleanup(pool: &PgPool, subject: SubjectId) {
    for t in ["kyc_intent_events", "kyc_subject_streams"] {
        let _ = sqlx::query(&format!(r#"DELETE FROM "ob-poc".{t} WHERE subject_root = $1"#))
            .bind(subject.0)
            .execute(pool)
            .await;
    }
    let _ = sqlx::query(r#"DELETE FROM "public".outbox WHERE idempotency_key LIKE $1"#)
        .bind(format!("{}:%", subject.0))
        .execute(pool)
        .await;
}

async fn register(scope: &mut Scope, subject: SubjectId) {
    KycSubjectRegister
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "is_natural_person": true }),
            &mut VerbExecutionContext::default(),
            scope,
        )
        .await
        .expect("register has no NotAlreadyRegistered violation on a fresh subject");
}

async fn create_obligation(scope: &mut Scope, subject: SubjectId, obligation_id: Uuid, role: &str) {
    KycObligationCreate
        .execute(
            &serde_json::json!({
                "subject-id": subject.0, "obligation-id": obligation_id, "role": role,
            }),
            &mut VerbExecutionContext::default(),
            scope,
        )
        .await
        .expect("obligation.create must succeed on a registered subject");
}

// ── row 11 — kyc_ubo.assert.obligation.creation: SubjectRegistered ───────────────────────

#[tokio::test]
async fn row11_obligation_create_blocks_unregistered_subject() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;
    // Deliberately NOT registered.

    let result = KycObligationCreate
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "role": "beneficial_owner" }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_err(),
        "obligation.create must be blocked for an unregistered subject (SubjectRegistered — \
         the motivating cross-fold case for the unified checker): {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

#[tokio::test]
async fn row11_obligation_create_admits_registered_subject() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject).await;

    let result = KycObligationCreate
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "role": "beneficial_owner" }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_ok(),
        "obligation.create must be admitted for a registered subject: {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

// ── row 12 — update-identity: ObligationExists ──────────────────────────────

#[tokio::test]
async fn row12_update_identity_blocks_missing_obligation() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject).await;
    // Deliberately no obligation.create — obligation-id references nothing.

    let result = KycObligationUpdateIdentity
        .execute(
            &serde_json::json!({
                "subject-id": subject.0, "obligation-id": Uuid::new_v4(), "state": "satisfied",
            }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_err(),
        "update-identity must be blocked against a non-existent obligation (ObligationExists): \
         {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

#[tokio::test]
async fn row12_update_identity_admits_existing_obligation() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let obligation_id = Uuid::new_v4();
    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject).await;
    create_obligation(&mut scope, subject, obligation_id, "beneficial_owner").await;

    let result = KycObligationUpdateIdentity
        .execute(
            &serde_json::json!({
                "subject-id": subject.0, "obligation-id": obligation_id, "state": "satisfied",
            }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_ok(),
        "update-identity must be admitted against an existing, undecided obligation: {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

// ── row 13 — update-screening: ObligationExists ─────────────────────────────

#[tokio::test]
async fn row13_update_screening_blocks_missing_obligation() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject).await;

    let result = KycObligationUpdateScreening
        .execute(
            &serde_json::json!({
                "subject-id": subject.0, "obligation-id": Uuid::new_v4(), "state": "satisfied",
            }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_err(),
        "update-screening must be blocked against a non-existent obligation (ObligationExists): \
         {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

#[tokio::test]
async fn row13_update_screening_admits_existing_obligation() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let obligation_id = Uuid::new_v4();
    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject).await;
    create_obligation(&mut scope, subject, obligation_id, "investor").await;

    let result = KycObligationUpdateScreening
        .execute(
            &serde_json::json!({
                "subject-id": subject.0, "obligation-id": obligation_id, "state": "satisfied",
            }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_ok(),
        "update-screening must be admitted against an existing, undecided obligation: {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

// ── row 14 — update-risk: ObligationExists ──────────────────────────────────

#[tokio::test]
async fn row14_update_risk_blocks_missing_obligation() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject).await;

    let result = KycObligationUpdateRisk
        .execute(
            &serde_json::json!({
                "subject-id": subject.0, "obligation-id": Uuid::new_v4(), "state": "in_progress",
            }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_err(),
        "update-risk must be blocked against a non-existent obligation (ObligationExists): \
         {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

#[tokio::test]
async fn row14_update_risk_admits_existing_obligation() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let obligation_id = Uuid::new_v4();
    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject).await;
    create_obligation(&mut scope, subject, obligation_id, "controller").await;

    let result = KycObligationUpdateRisk
        .execute(
            &serde_json::json!({
                "subject-id": subject.0, "obligation-id": obligation_id, "state": "in_progress",
            }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_ok(),
        "update-risk must be admitted against an existing, undecided obligation: {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

// ── row 15 — satisfy: SubjectNotDecided (proven independently of
//    ObligationExists — the target obligation genuinely exists here) ───────

#[tokio::test]
async fn row15_satisfy_blocks_once_subject_is_decided() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let first_obligation = Uuid::new_v4();
    let second_obligation = Uuid::new_v4();
    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject).await;
    create_obligation(&mut scope, subject, first_obligation, "beneficial_owner").await;
    KycObligationSatisfy
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "obligation-id": first_obligation }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await
        .expect("satisfy on an existing, undecided obligation must succeed");
    DecideApprove
        .execute(
            &serde_json::json!({ "subject-id": subject.0 }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await
        .expect("approve must succeed once the only obligation is all-terminal (satisfied)");
    // A second, genuinely-existing obligation created AFTER the decision
    // (obligation.create only gates on SubjectRegistered, not decision
    // state) — ObligationExists passes; SubjectNotDecided must be what
    // blocks satisfy here.
    create_obligation(&mut scope, subject, second_obligation, "director").await;

    let result = KycObligationSatisfy
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "obligation-id": second_obligation }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_err(),
        "satisfy must be blocked once the subject has already been decided (approved), even \
         against a genuinely-existing obligation (K-23 — decision is final): {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

#[tokio::test]
async fn row15_satisfy_admits_existing_obligation_before_decision() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let obligation_id = Uuid::new_v4();
    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject).await;
    create_obligation(&mut scope, subject, obligation_id, "beneficial_owner").await;

    let result = KycObligationSatisfy
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "obligation-id": obligation_id }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_ok(),
        "satisfy must be admitted against an existing, undecided obligation: {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

// ── row 16 — waive: ObligationExists ────────────────────────────────────────

#[tokio::test]
async fn row16_waive_blocks_missing_obligation() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject).await;

    let result = KycObligationWaive
        .execute(
            &serde_json::json!({
                "subject-id": subject.0, "obligation-id": Uuid::new_v4(), "reason": "test",
            }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_err(),
        "waive must be blocked against a non-existent obligation (ObligationExists): {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

#[tokio::test]
async fn row16_waive_admits_existing_obligation() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let obligation_id = Uuid::new_v4();
    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject).await;
    create_obligation(&mut scope, subject, obligation_id, "intermediate_entity").await;

    let result = KycObligationWaive
        .execute(
            &serde_json::json!({
                "subject-id": subject.0, "obligation-id": obligation_id,
                "reason": "simplified due diligence applies",
            }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_ok(),
        "waive must be admitted against an existing, undecided obligation: {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

// ── row 17 — kyc.person.approve: SubjectAllTerminal (the K-23 gate) ────────

#[tokio::test]
async fn row17_approve_blocks_non_terminal_obligation() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let obligation_id = Uuid::new_v4();
    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject).await;
    create_obligation(&mut scope, subject, obligation_id, "director").await;
    // Deliberately leave identity/screening/risk tracks Pending.

    let result = DecideApprove
        .execute(
            &serde_json::json!({ "subject-id": subject.0 }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_err(),
        "approve must be blocked while an obligation track is still Pending (SubjectAllTerminal \
         — the K-23 gate): {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

#[tokio::test]
async fn row17_approve_admits_all_terminal_obligation() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let obligation_id = Uuid::new_v4();
    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject).await;
    create_obligation(&mut scope, subject, obligation_id, "director").await;
    KycObligationSatisfy
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "obligation-id": obligation_id }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await
        .expect("satisfy must succeed on an existing, undecided obligation");

    let result = DecideApprove
        .execute(
            &serde_json::json!({ "subject-id": subject.0 }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_ok(),
        "approve must be admitted once the only obligation is all-terminal (satisfied): \
         {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

// ── row 18 — kyc.person.reject: SubjectNotDecided ───────────────────────────

#[tokio::test]
async fn row18_reject_blocks_once_already_decided() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let obligation_id = Uuid::new_v4();
    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject).await;
    create_obligation(&mut scope, subject, obligation_id, "director").await;
    KycObligationSatisfy
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "obligation-id": obligation_id }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await
        .expect("satisfy must succeed on an existing, undecided obligation");
    DecideApprove
        .execute(
            &serde_json::json!({ "subject-id": subject.0 }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await
        .expect("approve must succeed once the only obligation is all-terminal");

    let result = DecideReject
        .execute(
            &serde_json::json!({ "subject-id": subject.0 }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_err(),
        "reject must be blocked once the subject has already been decided (approved) — \
         SubjectNotDecided: {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

#[tokio::test]
async fn row18_reject_admits_first_decision_at_any_stage() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let obligation_id = Uuid::new_v4();
    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject).await;
    // Deliberately leave the obligation non-terminal — early rejection is a
    // real compliance outcome and must remain legal at ANY stage (the
    // ratified matrix is explicit reject carries no SubjectAllTerminal stud).
    create_obligation(&mut scope, subject, obligation_id, "director").await;

    let result = DecideReject
        .execute(
            &serde_json::json!({ "subject-id": subject.0 }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_ok(),
        "reject must be admitted as a first decision even with a non-terminal obligation still \
         open (early rejection is deliberately legal at any stage): {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}
