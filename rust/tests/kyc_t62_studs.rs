//! T6.2 gate tests — EOP-PLAN-KYCUBO-KIT-001 §T6.2 (edge family, closes the
//! ratified EOP-DD-KYCUBO-KIT-T6 matrix rows 1-5).
//!
//! Eight gates (4 rows x block/admit — row 5 retired T3, see below), each driven through the REAL governed
//! append path (live DB) — the real `SemOsVerbOp::execute()` for the verb
//! under test, not `check_preconditions` called in isolation. This mirrors
//! `kyc_t61_studs.rs::select_strategy_blocked_end_to_end`'s discipline, one
//! level stronger than kyc_t61's own two *_placement pure-checker tests
//! (T6.2's Part A closure tooth already covers the pure-checker layer for
//! the whole verb universe; these gates prove the specific new studs at the
//! layer that actually matters — the real op).
//!
//! Row coverage:
//! - rows 1/2 (`connect` with `kind: voting_rights` / `kind:
//!   economic_interest` — EOP-VS-UBO-GAME-001 T3, §3.2, 2026-08-27 merged
//!   the former `assert-control`/`assert-economic-interest` into one verb):
//!   `SubjectRegistered` + `NoDuplicateActiveEdge` — block is the duplicate
//!   case specifically (a *first* assertion on a registered subject must be
//!   admitted; a *second* identical (from,to,kind) assertion must not).
//! - rows 3/4 (`attach-evidence` / `disconnect`, T3 renamed from
//!   `supersede`): `EdgeExists` + `EdgeActive` — block targets a disconnected
//!   edge; admit targets an active one.
//! - row 5 (`reconcile-conflict`) — RETIRED (EOP-VS-UBO-GAME-001 T3, §3.3,
//!   2026-08-27, K-G7): "No reconcile ... a third path to what two moves
//!   already do." No test survives it here.
//! - row 9 (`kyc_ubo.assert.subject.register`, T6.3 fix, 2026-08-17, corrects
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
    KycSubjectPlace, UboEdgeAttachEvidence, UboEdgeConnect, UboEdgeDisconnect,
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
    KycSubjectPlace
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "is_natural_person": false, "entity-type": "private_limited_company" }),
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

    let result = UboEdgeConnect
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
    UboEdgeConnect
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

    let dup = UboEdgeConnect
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

    let result = UboEdgeConnect
        .execute(
            &serde_json::json!({
                "subject-id": subject.0,
                "from_entity_id": Uuid::new_v4(),
                "to_entity_id": Uuid::new_v4(),
                "kind": "economic_interest",
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
    UboEdgeConnect
        .execute(
            &serde_json::json!({
                "subject-id": subject.0,
                "from_entity_id": from,
                "to_entity_id": to,
                "kind": "economic_interest",
                "percentage": 30.0,
            }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await
        .expect("first assert-economic-interest must succeed");

    let dup = UboEdgeConnect
        .execute(
            &serde_json::json!({
                "subject-id": subject.0,
                "from_entity_id": from,
                "to_entity_id": to,
                "kind": "economic_interest",
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

// ── rows 3/4 — attach-evidence / disconnect ─────────────────────────────────

async fn assert_control_edge(scope: &mut Scope, subject: SubjectId) -> Uuid {
    // §8 Q3: connect refuses a caller-supplied edge id — the system mints
    // it and returns it; capture the minted id from the response instead
    // of pre-choosing one.
    let outcome = UboEdgeConnect
        .execute(
            &serde_json::json!({
                "subject-id": subject.0,
                "from_entity_id": Uuid::new_v4(),
                "to_entity_id": Uuid::new_v4(),
                "kind": "voting_rights",
            }),
            &mut VerbExecutionContext::default(),
            scope,
        )
        .await
        .expect("connect must succeed");
    let dsl_runtime::VerbExecutionOutcome::Record(v) = outcome else {
        panic!("connect must return a Record outcome, got {outcome:?}");
    };
    Uuid::parse_str(v["edge_id"].as_str().expect("edge_id must be a string"))
        .expect("edge_id must be a valid UUID")
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

    UboEdgeDisconnect
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

    let result = UboEdgeDisconnect
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

    UboEdgeDisconnect
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "edge-id": edge_id }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await
        .expect("first supersede must succeed");

    let dup = UboEdgeDisconnect
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

// row 5 (reconcile-conflict) RETIRED (EOP-VS-UBO-GAME-001 T3, §3.3,
// 2026-08-27, K-G7): "No reconcile ... a third path to what two moves
// already do." Both gates (`row5_reconcile_conflict_blocks_unregistered_
// subject`, `row5_reconcile_conflict_admits_registered_subject_with_zero_
// edges`) are deleted with the verb — no successor to test.

// ── row 9 — kyc_ubo.assert.subject.register (T6.3 fix, 2026-08-17) ────────────────────

#[tokio::test]
async fn row9_register_blocks_true_duplicate_entity_id() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;
    let entity = Uuid::new_v4();
    KycSubjectPlace
        .execute(
            &serde_json::json!({
                "subject-id": subject.0, "entity-id": entity, "is_natural_person": true, "entity-type": "natural_person"
            }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await
        .expect("first registration of a fresh (subject, entity) pair must be admitted");

    let result = KycSubjectPlace
        .execute(
            &serde_json::json!({
                "subject-id": subject.0, "entity-id": entity, "is_natural_person": true, "entity-type": "natural_person"
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
        let result = KycSubjectPlace
            .execute(
                &serde_json::json!({
                    "subject-id": subject.0, "entity-id": candidate, "is_natural_person": true, "entity-type": "natural_person"
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
