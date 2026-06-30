//! R1 proof — the stream-backed verb pattern, end to end.
//!
//! Calls the `ubo.edge.assert-control` SemOsVerbOp through a real
//! `VerbExecutionContext` + `TransactionScope`, and proves: the verb appends an
//! `IntentEvent` to the durable stream (committing/rolling back with the scope),
//! `as_of` flows from the context onto the event, and the outbox drainer
//! projects the edge. This is the template every other `dsl.kyc` verb follows.

use std::sync::Arc;

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionContext, VerbExecutionOutcome};
use ob_poc::domain_ops::kyc_stream_ops::UboEdgeAssertControl;
use ob_poc_kyc_store::{PgKycProjectionDrainer, CONTROL_EDGE_PROJECTION_EFFECT};
use ob_poc_kyc_substrate::{phase1_lexicon, FoldRegistry, SubjectId, V1FoldImpl};
use ob_poc_types::TransactionScopeId;
use sem_os_postgres::ops::SemOsVerbOp;

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string())
}

async fn connect() -> PgPool {
    PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url())
        .await
        .expect("connect to test DB")
}

fn v1_registry() -> FoldRegistry {
    let mut r = FoldRegistry::new();
    r.register(phase1_lexicon().hash, Arc::new(V1FoldImpl));
    r
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
    async fn rollback(self) {
        self.tx.rollback().await.unwrap();
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
    for t in ["kyc_intent_events", "kyc_subject_streams", "kyc_control_edge_projection"] {
        let _ = sqlx::query(&format!(r#"DELETE FROM "ob-poc".{t} WHERE subject_root = $1"#))
            .bind(subject.0)
            .execute(pool)
            .await;
    }
    let _ = sqlx::query(r#"DELETE FROM "public".outbox WHERE effect_kind = $1 AND idempotency_key LIKE $2"#)
        .bind(CONTROL_EDGE_PROJECTION_EFFECT)
        .bind(format!("{}:%", subject.0))
        .execute(pool)
        .await;
}

#[tokio::test]
async fn assert_control_verb_appends_to_stream_and_projects() {
    let pool = connect().await;
    let subject = SubjectId(Uuid::new_v4());

    // A context as the dispatcher builds it: as_of frozen at entry.
    let mut ctx = VerbExecutionContext::default();
    let fixed_as_of = chrono::DateTime::parse_from_rfc3339("2026-02-02T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    ctx.as_of = fixed_as_of;

    // The verb args (op extracts subject-id; the rest is the fold payload).
    let args = serde_json::json!({
        "subject-id": subject.0.to_string(),
        "from_entity_id": Uuid::new_v4().to_string(),
        "to_entity_id": Uuid::new_v4().to_string(),
        "edge_kind": "voting_rights",
    });

    // 1. Rollback path: the verb's append participates in the scope's txn.
    {
        let mut scope = TestScope::begin(&pool).await;
        UboEdgeAssertControl
            .execute(&args, &mut ctx, &mut scope)
            .await
            .expect("op executes");
        scope.rollback().await;
    }
    let rolled: i64 =
        sqlx::query_scalar(r#"SELECT count(*) FROM "ob-poc".kyc_intent_events WHERE subject_root = $1"#)
            .bind(subject.0)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(rolled, 0, "the verb's append rolled back with the scope");

    // 2. Commit path: event lands in the stream with the right verb + as_of.
    let outcome = {
        let mut scope = TestScope::begin(&pool).await;
        let outcome = UboEdgeAssertControl
            .execute(&args, &mut ctx, &mut scope)
            .await
            .expect("op executes");
        scope.commit().await;
        outcome
    };
    match outcome {
        VerbExecutionOutcome::Record(v) => assert_eq!(v["seq"].as_u64(), Some(0)),
        other => panic!("expected Record, got {other:?}"),
    }

    let (verb, as_of): (String, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
        r#"SELECT verb_fqn, as_of FROM "ob-poc".kyc_intent_events WHERE subject_root = $1"#,
    )
    .bind(subject.0)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(verb, "ubo.edge.assert-control");
    assert_eq!(as_of, fixed_as_of, "as_of flowed from the context, frozen at entry");

    // 3. The append enqueued a projection effect; the drainer projects the edge.
    PgKycProjectionDrainer::drain_all(&pool, &v1_registry(), 100).await.unwrap();
    let edges: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM "ob-poc".kyc_control_edge_projection WHERE subject_root = $1 AND status = 'Asserted'"#,
    )
    .bind(subject.0)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(edges, 1, "the asserted control edge is projected");

    cleanup(&pool, subject).await;
}
