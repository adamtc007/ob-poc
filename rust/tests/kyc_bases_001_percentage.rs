//! EOP-DD-UBO-BASES-001 §6/§7 `percentage_is_bounded_at_connect` — finding
//! #3, re-characterised: a single `connect` asserting `percentage: 972.0`
//! (or negative) on an `economic_interest` edge is accepted verbatim by
//! both live call paths today — same two-surface discipline as
//! `tests/kyc_connect_withdrawn_endpoint.rs`'s `ConnectEndpointsNotWithdrawn`
//! gate (op AND workbook). P1 promotes a `Precondition` bounding
//! `percentage` to `[0, 100]` on `connect`'s lexicon entry and flips these
//! assertions to expect refusal.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionContext};
use ob_poc::domain_ops::kyc_stream_ops::{KycSubjectPlace, UboEdgeConnect};
use ob_poc::domain_ops::kyc_workbook::open_workbook;
use ob_poc_kyc_substrate::{assembly_lexicon, FoldRegistry, SubjectId, V1FoldImpl};
use ob_poc_types::TransactionScopeId;
use sem_os_core::principal::Principal as RuntimePrincipal;
use sem_os_postgres::ops::SemOsVerbOp;

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string())
}

async fn connect_pool() -> PgPool {
    PgPoolOptions::new()
        .max_connections(8)
        .connect(&database_url())
        .await
        .expect("connect to test DB")
}

fn v1_registry() -> FoldRegistry {
    let mut r = FoldRegistry::new();
    r.register(assembly_lexicon().hash, std::sync::Arc::new(V1FoldImpl));
    r
}

fn runtime_principal() -> RuntimePrincipal {
    RuntimePrincipal {
        actor_id: "bases-001-percentage-gate".to_string(),
        roles: vec!["analyst".to_string()],
        claims: std::collections::HashMap::new(),
        tenancy: None,
    }
}

fn fixed_ts() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc)
}

struct TestScope {
    tx: Transaction<'static, Postgres>,
    pool: PgPool,
    id: TransactionScopeId,
}
impl TestScope {
    async fn begin(pool: &PgPool) -> Self {
        Self { tx: pool.begin().await.unwrap(), pool: pool.clone(), id: TransactionScopeId::new() }
    }
    async fn commit(self) {
        self.tx.commit().await.unwrap();
    }
}
impl TransactionScope for TestScope {
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
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".kyc_intent_events WHERE subject_root = $1"#)
        .bind(subject.0)
        .execute(pool)
        .await;
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".kyc_subject_streams WHERE subject_root = $1"#)
        .bind(subject.0)
        .execute(pool)
        .await;
}

/// Places subject + voter (natural_person) + company (private_limited_company),
/// both actively placed (no withdrawal — this gate is orthogonal to
/// finding #2's geometry-closure fix). Returns (subject, voter, company).
async fn placed_fixture(pool: &PgPool) -> (SubjectId, Uuid, Uuid) {
    let subject = SubjectId(Uuid::new_v4());
    let voter = Uuid::new_v4();
    let company = Uuid::new_v4();

    let mut scope = TestScope::begin(pool).await;
    KycSubjectPlace
        .execute(
            &serde_json::json!({ "subject-id": subject.0.to_string(), "entity-type": "private_limited_company" }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await
        .expect("place subject");
    KycSubjectPlace
        .execute(
            &serde_json::json!({ "subject-id": subject.0.to_string(), "entity-id": voter.to_string(), "entity-type": "natural_person" }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await
        .expect("place voter");
    KycSubjectPlace
        .execute(
            &serde_json::json!({ "subject-id": subject.0.to_string(), "entity-id": company.to_string(), "entity-type": "private_limited_company" }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await
        .expect("place company");
    scope.commit().await;

    (subject, voter, company)
}

/// Through the op. `percentage: 972.3529411764705` — the exact fuzzer-found
/// value (finding #3's minimised reproduction).
#[tokio::test]
async fn op_refuses_percentage_above_100() {
    let pool = connect_pool().await;
    let (subject, voter, company) = placed_fixture(&pool).await;

    let result = {
        let mut scope = TestScope::begin(&pool).await;
        UboEdgeConnect
            .execute(
                &serde_json::json!({
                    "subject-id": subject.0.to_string(),
                    "from_entity_id": voter.to_string(),
                    "to_entity_id": company.to_string(),
                    "kind": "economic_interest",
                    "percentage": 972.3529411764705_f64,
                }),
                &mut VerbExecutionContext::default(),
                &mut scope,
            )
            .await
    };
    cleanup(&pool, subject).await;
    assert!(
        result.is_err(),
        "EOP-DD-UBO-BASES-001 §6: the op must refuse a percentage above 100 — got: {result:?}"
    );
}

/// Through the op. Negative percentage — the other half of the fuzzer's
/// documented adversarial range (`Tape::percentage()`: `[-50.0, 1050.0]`).
#[tokio::test]
async fn op_refuses_negative_percentage() {
    let pool = connect_pool().await;
    let (subject, voter, company) = placed_fixture(&pool).await;

    let result = {
        let mut scope = TestScope::begin(&pool).await;
        UboEdgeConnect
            .execute(
                &serde_json::json!({
                    "subject-id": subject.0.to_string(),
                    "from_entity_id": voter.to_string(),
                    "to_entity_id": company.to_string(),
                    "kind": "economic_interest",
                    "percentage": -12.5_f64,
                }),
                &mut VerbExecutionContext::default(),
                &mut scope,
            )
            .await
    };
    cleanup(&pool, subject).await;
    assert!(
        result.is_err(),
        "EOP-DD-UBO-BASES-001 §6: the op must refuse a negative percentage — got: {result:?}"
    );
}

/// Through the workbook.
#[tokio::test]
async fn workbook_refuses_percentage_above_100() {
    let pool = connect_pool().await;
    let registry = v1_registry();
    let (subject, voter, company) = placed_fixture(&pool).await;

    let commit_result = {
        let mut scope = TestScope::begin(&pool).await;
        let mut workbook = open_workbook(scope.transaction(), subject).await.unwrap();
        workbook
            .stage(
                &format!(
                    r#"(kyc_ubo.assert.edge.connect :subject-id "{}" :from_entity_id "{voter}" :to_entity_id "{company}" :kind "economic_interest" :percentage 972.3529411764705)"#,
                    subject.0
                ),
                &runtime_principal(),
                ob_poc_kyc_substrate::AuthorityRef("gate".into()),
                fixed_ts(),
            )
            .expect("staging recognises (frontier admits any connect)");
        workbook.commit(&mut scope, &registry).await
    };
    cleanup(&pool, subject).await;
    assert!(
        commit_result.is_err(),
        "EOP-DD-UBO-BASES-001 §6: the workbook must refuse a percentage above 100 — got: {commit_result:?}"
    );
}

/// Sanity: an ordinary in-range percentage (30%) must still be admitted on
/// both surfaces — the bound must not become an over-refusal.
#[tokio::test]
async fn op_admits_percentage_in_range() {
    let pool = connect_pool().await;
    let (subject, voter, company) = placed_fixture(&pool).await;

    let result = {
        let mut scope = TestScope::begin(&pool).await;
        let r = UboEdgeConnect
            .execute(
                &serde_json::json!({
                    "subject-id": subject.0.to_string(),
                    "from_entity_id": voter.to_string(),
                    "to_entity_id": company.to_string(),
                    "kind": "economic_interest",
                    "percentage": 30.0_f64,
                }),
                &mut VerbExecutionContext::default(),
                &mut scope,
            )
            .await;
        if r.is_ok() {
            scope.commit().await;
        }
        r
    };
    cleanup(&pool, subject).await;
    assert!(result.is_ok(), "a 30% economic_interest connect must still be admitted — got: {result:?}");
}
