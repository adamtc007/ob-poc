//! T6.3 gate tests — EOP-PLAN-KYCUBO-KIT-001 §T6.3 (determination family,
//! closes the ratified EOP-DD-KYCUBO-KIT-T6 matrix rows 6 (remainder), 7,
//! 10; row 8 is the unchanged 8a freeze guard, already gated in
//! `kyc_t61_studs.rs`. Row 9 was HALTED during execution — see below.).
//!
//! Eight gates (block/admit per stud), each driven through the REAL governed
//! append path (live DB) — the real `SemOsVerbOp::execute()` for the verb
//! under test, mirroring `kyc_t62_studs.rs`'s discipline.
//!
//! Row coverage:
//! - row 6 (`select-strategy`): TWO new studs beyond the 6a exemplar —
//!   `SubjectRegistered` (block: unregistered subject) and
//!   `StructureClassified` (block: registered but not yet classified).
//! - row 7 (`apply-smo-fallback`): `ReconciledProjection` + `StrategySelected`
//!   reused from compute-fold/freeze — block before either has fired, admit
//!   once both have.
//! - row 9 (`kyc.subject.register`) HALTED, not shipped: the ratified
//!   `NotAlreadyRegistered` (a bare `!state.registered` boolean) is
//!   incompatible with `register`'s real production usage — one call
//!   registers the subject entity itself, one MORE call per natural-person
//!   candidate shares the same subject_root, differentiated only by
//!   payload `entity_id` (`natural_persons_from_events`,
//!   `kyc_stream_ops.rs::UboDeterminationFreeze`). Attaching the stud as
//!   ratified would block every multi-person determination; left
//!   geometry-free pending a matrix amendment to a KEYED
//!   (subject_root, entity_id) check. `phase1_lexicon()`'s entry for this
//!   verb is unchanged from pre-T6.3.
//! - row 10 (`kyc.subject.classify-structure`): `SubjectRegistered` — block
//!   classify-structure on an unregistered subject.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionContext};
use ob_poc::domain_ops::kyc_stream_ops::{
    KycSubjectClassifyStructure, KycSubjectRegister, UboDeterminationApplySmoFallback,
    UboDeterminationSelectStrategy, UboEdgeReconcileConflict,
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
            &serde_json::json!({ "subject-id": subject.0, "is_natural_person": false }),
            &mut VerbExecutionContext::default(),
            scope,
        )
        .await
        .expect("register has no NotAlreadyRegistered violation on a fresh subject");
}

async fn classify(scope: &mut Scope, subject: SubjectId, class: &str) {
    KycSubjectClassifyStructure
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "structure-class": class }),
            &mut VerbExecutionContext::default(),
            scope,
        )
        .await
        .expect("classify-structure must succeed on a registered subject");
}

// ── row 6 — select-strategy: SubjectRegistered + StructureClassified ───────

#[tokio::test]
async fn row6_select_strategy_blocks_unregistered_subject() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;
    // Deliberately NOT registered.

    let result = UboDeterminationSelectStrategy
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "strategy": "ownership_prong_strategy" }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_err(),
        "select-strategy must be blocked for an unregistered subject (SubjectRegistered): \
         {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

#[tokio::test]
async fn row6_select_strategy_blocks_unclassified_registered_subject() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject).await;
    // Deliberately NOT classified — SubjectRegistered passes, StructureClassified must not.

    let result = UboDeterminationSelectStrategy
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "strategy": "ownership_prong_strategy" }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_err(),
        "select-strategy must be blocked for a registered-but-unclassified subject \
         (StructureClassified): {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

#[tokio::test]
async fn row6_select_strategy_admits_registered_and_classified_subject() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject).await;
    classify(&mut scope, subject, "private_company").await;

    let result = UboDeterminationSelectStrategy
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "strategy": "ownership_prong_strategy" }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_ok(),
        "select-strategy must be admitted once registered + classified (supported class): \
         {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

// ── row 7 — apply-smo-fallback: ReconciledProjection + StrategySelected ────

#[tokio::test]
async fn row7_apply_smo_fallback_blocks_before_reconcile_and_strategy() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject).await;
    // Deliberately no reconcile-conflict / select-strategy before it.

    let result = UboDeterminationApplySmoFallback
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "smo-person-id": Uuid::new_v4() }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_err(),
        "apply-smo-fallback must be blocked before reconcile-conflict + select-strategy have \
         fired: {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

#[tokio::test]
async fn row7_apply_smo_fallback_admits_after_reconcile_and_strategy() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject).await;
    classify(&mut scope, subject, "private_company").await;
    UboEdgeReconcileConflict
        .execute(
            &serde_json::json!({ "subject-id": subject.0 }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await
        .expect("reconcile-conflict must succeed on a registered subject");
    UboDeterminationSelectStrategy
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "strategy": "ownership_prong_strategy" }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await
        .expect("select-strategy must succeed once registered + classified");

    let result = UboDeterminationApplySmoFallback
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "smo-person-id": Uuid::new_v4() }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_ok(),
        "apply-smo-fallback must be admitted once reconcile-conflict + select-strategy have \
         fired: {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

// ── row 9 — kyc.subject.register: NotAlreadyRegistered ─────────────────────

#[tokio::test]
async fn row9_register_admits_fresh_subject() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;

    let result = KycSubjectRegister
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "is_natural_person": false }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_ok(),
        "register must be admitted for a fresh (never-registered) subject: {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

// row9_register_blocks_double_registration intentionally NOT written: row 9
// (`NotAlreadyRegistered`) was HALTED during T6.3 execution, not shipped —
// see the lexicon entry's own comment in `lexicon.rs`. A second
// `kyc.subject.register` call on the same subject_root is a REAL production
// pattern (one call per natural-person candidate within a determination
// stream, see `kyc_m3_remediation.rs`'s
// `m3_1_freeze_differential_matches_ownership_prong_strategy` /
// `m4_control_prong_strategy_resolves_gp_statutory_control`), so it must
// stay legal; asserting it here would pin the wrong behaviour.

// ── row 10 — kyc.subject.classify-structure: SubjectRegistered ─────────────

#[tokio::test]
async fn row10_classify_structure_blocks_unregistered_subject() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;
    // Deliberately NOT registered.

    let result = KycSubjectClassifyStructure
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "structure-class": "private_company" }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_err(),
        "classify-structure must be blocked for an unregistered subject: {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

#[tokio::test]
async fn row10_classify_structure_admits_registered_subject() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject).await;

    let result = KycSubjectClassifyStructure
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "structure-class": "private_company" }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_ok(),
        "classify-structure must be admitted for a registered subject: {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}
