//! T6.2 gate tests — EOP-PLAN-KYCUBO-KIT-001 §T6.2 (edge family, closes the
//! ratified EOP-DD-KYCUBO-KIT-T6 matrix rows 1-5).
//!
//! Ten gates (5 rows x block/admit), each driven through the REAL governed
//! append path (live DB) — the real `SemOsVerbOp::execute()` for the verb
//! under test, not `check_preconditions` called in isolation. This mirrors
//! `kyc_t61_studs.rs::select_strategy_blocked_end_to_end`'s discipline, one
//! level stronger than kyc_t61's own two *_placement pure-checker tests
//! (T6.2's Part A closure tooth already covers the pure-checker layer for
//! the whole verb universe; these gates prove the specific new studs at the
//! layer that actually matters — the real op).
//!
//! Row coverage:
//! - rows 1/2 (`assert-control` / `assert-economic-interest`):
//!   `SubjectRegistered` + `NoDuplicateActiveEdge` — block is the duplicate
//!   case specifically (a *first* assertion on a registered subject must be
//!   admitted; a *second* identical (from,to,kind) assertion must not).
//! - rows 3/4 (`attach-evidence` / `supersede`): `EdgeExists` + `EdgeActive`
//!   — block targets a superseded edge; admit targets an active one.
//! - row 5 (`reconcile-conflict`): `SubjectRegistered` alone — block targets
//!   an unregistered subject; admit succeeds even with zero edges (the
//!   ratified no-amendment reading).
//! - row 9 (`kyc.subject.register`, T6.3 fix, 2026-08-17, corrects
//!   EOP-DD-KYCUBO-KIT-T6 §5 — see its §6 amendment): `NotAlreadyRegistered`,
//!   now keyed off the event's `entity_id` (`ControlState.registered_entity_ids`)
//!   rather than the bare per-subject `registered` bool — block targets a
//!   true duplicate (same subject_root + same entity_id); admit targets
//!   multi-person registration (same subject_root, distinct entity_id per
//!   candidate), the real production shape the original bare-boolean
//!   reading would have broken.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionContext};
use ob_poc::domain_ops::kyc_stream_ops::{
    KycSubjectRegister, UboEdgeAssertControl, UboEdgeAssertEconomicInterest,
    UboEdgeAttachEvidence, UboEdgeReconcileConflict, UboEdgeSupersede,
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
        .expect("first self-registration for a fresh subject must always be admitted");
}

// ── rows 1/2 — assert-control / assert-economic-interest ───────────────────

#[tokio::test]
async fn row1_assert_control_admits_first_assertion_on_registered_subject() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject).await;

    let result = UboEdgeAssertControl
        .execute(
            &serde_json::json!({
                "subject-id": subject.0,
                "from_entity_id": Uuid::new_v4(),
                "to_entity_id": Uuid::new_v4(),
                "kind": "voting_rights",
            }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_ok(),
        "a first assert-control on a freshly registered subject must be admitted: {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

#[tokio::test]
async fn row1_assert_control_blocks_duplicate_active_edge() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject).await;

    let from = Uuid::new_v4();
    let to = Uuid::new_v4();
    UboEdgeAssertControl
        .execute(
            &serde_json::json!({
                "subject-id": subject.0,
                "from_entity_id": from,
                "to_entity_id": to,
                "kind": "voting_rights",
            }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await
        .expect("first assert-control must succeed");

    let dup = UboEdgeAssertControl
        .execute(
            &serde_json::json!({
                "subject-id": subject.0,
                "from_entity_id": from,
                "to_entity_id": to,
                "kind": "voting_rights",
            }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        dup.is_err(),
        "a second identical (from,to,kind) assert-control must be blocked — use supersede, \
         never a contradicting assert (K-13)"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

#[tokio::test]
async fn row2_assert_economic_interest_admits_first_assertion_on_registered_subject() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject).await;

    let result = UboEdgeAssertEconomicInterest
        .execute(
            &serde_json::json!({
                "subject-id": subject.0,
                "from_entity_id": Uuid::new_v4(),
                "to_entity_id": Uuid::new_v4(),
                "percentage": 30.0,
            }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_ok(),
        "a first assert-economic-interest on a freshly registered subject must be admitted: \
         {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

#[tokio::test]
async fn row2_assert_economic_interest_blocks_duplicate_active_edge() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject).await;

    let from = Uuid::new_v4();
    let to = Uuid::new_v4();
    UboEdgeAssertEconomicInterest
        .execute(
            &serde_json::json!({
                "subject-id": subject.0,
                "from_entity_id": from,
                "to_entity_id": to,
                "percentage": 30.0,
            }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await
        .expect("first assert-economic-interest must succeed");

    let dup = UboEdgeAssertEconomicInterest
        .execute(
            &serde_json::json!({
                "subject-id": subject.0,
                "from_entity_id": from,
                "to_entity_id": to,
                "percentage": 45.0,
            }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        dup.is_err(),
        "a second active economic-interest edge for the same (from,to) must be blocked — use \
         supersede, never a contradicting assert (K-13)"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

// ── rows 3/4 — attach-evidence / supersede ──────────────────────────────────

async fn assert_control_edge(scope: &mut Scope, subject: SubjectId) -> Uuid {
    let edge_id = Uuid::new_v4();
    UboEdgeAssertControl
        .execute(
            &serde_json::json!({
                "subject-id": subject.0,
                "edge-id": edge_id,
                "from_entity_id": Uuid::new_v4(),
                "to_entity_id": Uuid::new_v4(),
                "kind": "voting_rights",
            }),
            &mut VerbExecutionContext::default(),
            scope,
        )
        .await
        .expect("assert-control must succeed");
    edge_id
}

#[tokio::test]
async fn row3_attach_evidence_admits_active_edge() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject).await;
    let edge_id = assert_control_edge(&mut scope, subject).await;

    let result = UboEdgeAttachEvidence
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "edge-id": edge_id }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_ok(),
        "attach-evidence must be admitted against an active (Asserted) edge: {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

#[tokio::test]
async fn row3_attach_evidence_blocks_superseded_edge() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject).await;
    let edge_id = assert_control_edge(&mut scope, subject).await;

    UboEdgeSupersede
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "edge-id": edge_id }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await
        .expect("supersede of a freshly asserted (active) edge must succeed");

    let result = UboEdgeAttachEvidence
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "edge-id": edge_id }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_err(),
        "attach-evidence must be blocked against a superseded edge: {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

#[tokio::test]
async fn row4_supersede_admits_active_edge() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject).await;
    let edge_id = assert_control_edge(&mut scope, subject).await;

    let result = UboEdgeSupersede
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "edge-id": edge_id }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_ok(),
        "supersede must be admitted against an active (Asserted) edge: {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

#[tokio::test]
async fn row4_supersede_blocks_already_superseded_edge() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject).await;
    let edge_id = assert_control_edge(&mut scope, subject).await;

    UboEdgeSupersede
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "edge-id": edge_id }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await
        .expect("first supersede must succeed");

    let dup = UboEdgeSupersede
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "edge-id": edge_id }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        dup.is_err(),
        "a double-supersede of the same edge must be blocked — it would silently no-op-pollute \
         the stream"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

// ── row 5 — reconcile-conflict ──────────────────────────────────────────────

#[tokio::test]
async fn row5_reconcile_conflict_blocks_unregistered_subject() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;
    // Deliberately NOT registered.

    let result = UboEdgeReconcileConflict
        .execute(
            &serde_json::json!({ "subject-id": subject.0 }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_err(),
        "reconcile-conflict must be blocked for an unregistered subject: {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

#[tokio::test]
async fn row5_reconcile_conflict_admits_registered_subject_with_zero_edges() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject).await;
    // Deliberately zero edges — the ratified matrix reading (row 5, no
    // amendment): reconcile stays callable early.

    let result = UboEdgeReconcileConflict
        .execute(
            &serde_json::json!({ "subject-id": subject.0 }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_ok(),
        "reconcile-conflict must be admitted for a registered subject even with zero edges \
         (ratified without the optional economic-edge amendment): {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

// ── row 9 — kyc.subject.register (T6.3 fix, 2026-08-17) ────────────────────

#[tokio::test]
async fn row9_register_blocks_true_duplicate_entity_id() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;
    let entity = Uuid::new_v4();
    KycSubjectRegister
        .execute(
            &serde_json::json!({
                "subject-id": subject.0, "entity-id": entity, "is_natural_person": true
            }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await
        .expect("first registration of a fresh (subject, entity) pair must be admitted");

    let result = KycSubjectRegister
        .execute(
            &serde_json::json!({
                "subject-id": subject.0, "entity-id": entity, "is_natural_person": true
            }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_err(),
        "re-registering the SAME (subject_root, entity_id) pair must be blocked: {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

#[tokio::test]
async fn row9_register_admits_multi_person_distinct_entity_ids() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;

    // The self-registration (no entity-id -> defaults to subject-id).
    register(&mut scope, subject).await;

    // Three DISTINCT natural-person candidates sharing the same subject_root
    // — the real production shape (kyc_m3_remediation.rs) that a bare
    // per-subject `registered` boolean would have wrongly blocked.
    for candidate in [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()] {
        let result = KycSubjectRegister
            .execute(
                &serde_json::json!({
                    "subject-id": subject.0, "entity-id": candidate, "is_natural_person": true
                }),
                &mut VerbExecutionContext::default(),
                &mut scope,
            )
            .await;
        assert!(
            result.is_ok(),
            "registering a distinct entity_id under an already-registered subject_root \
             must be admitted (multi-person registration): {result:?}"
        );
    }
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}
