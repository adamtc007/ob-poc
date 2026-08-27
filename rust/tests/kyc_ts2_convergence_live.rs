//! D1 Part B — the live-DB half of TS.2 §6's `unparseable_wire_value_is_not_
//! dominant_influence` gate. The pure half (fold-level totality/mapping) is
//! `crates/ob-poc-kyc-substrate/tests/ts2_pipe_convergence.rs`. This proves
//! the actually-enforced boundary: the real `kyc_ubo.assert.edge.control` op
//! rejects a garbage or absent `kind` fail-closed, before append — a parse
//! failure never becomes a persisted `DominantInfluence` edge.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionContext};
use ob_poc::domain_ops::kyc_stream_ops::{KycSubjectPlace, UboEdgeAssertControl};
use ob_poc_kyc_store::PgKycEventStore;
use ob_poc_kyc_substrate::{fold_control, SubjectId};
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

async fn run(op: &dyn SemOsVerbOp, args: serde_json::Value, pool: &PgPool) {
    let mut ctx = VerbExecutionContext::default();
    let mut scope = Scope::begin(pool).await;
    op.execute(&args, &mut ctx, &mut scope).await.expect("op");
    scope.commit().await;
}

async fn run_fallible(
    op: &dyn SemOsVerbOp,
    args: serde_json::Value,
    pool: &PgPool,
) -> anyhow::Result<()> {
    let mut ctx = VerbExecutionContext::default();
    let mut scope = Scope::begin(pool).await;
    op.execute(&args, &mut ctx, &mut scope).await?;
    scope.commit().await;
    Ok(())
}

async fn cleanup(pool: &PgPool, subject: SubjectId) {
    for t in ["kyc_intent_events", "kyc_subject_streams"] {
        let _ = sqlx::query(&format!(r#"DELETE FROM "ob-poc".{t} WHERE subject_root = $1"#))
            .bind(subject.0)
            .execute(pool)
            .await;
    }
}

#[tokio::test]
async fn unparseable_wire_value_is_not_dominant_influence() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let from = Uuid::new_v4();
    let to = Uuid::new_v4();

    run(
        &KycSubjectPlace,
        serde_json::json!({ "subject-id": subject.0, "entity-type": "private_limited_company" }),
        &pool,
    )
    .await;

    // Garbage kind: rejected, nothing appended.
    let garbage = run_fallible(
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": from, "to_entity_id": to,
            "kind": "not_a_real_pipe_kind",
        }),
        &pool,
    )
    .await;
    assert!(garbage.is_err(), "a garbage kind must be rejected fail-closed, not folded to dominant_influence");

    // Absent kind: rejected, nothing appended.
    let absent = run_fallible(
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": from, "to_entity_id": to,
        }),
        &pool,
    )
    .await;
    assert!(absent.is_err(), "an absent kind must be rejected fail-closed, not folded to dominant_influence");

    // Confirm: zero control edges landed on this subject's stream at all —
    // a parse failure never becomes a persisted edge of ANY kind, let alone
    // DominantInfluence specifically.
    let mut conn = pool.acquire().await.expect("acquire connection");
    let events = PgKycEventStore::load_events(&mut conn, subject).await.unwrap();
    let refs: Vec<_> = events.iter().collect();
    let control = fold_control(&refs);
    assert!(
        control.edges.is_empty(),
        "no edge of any kind should have been appended by the two rejected calls"
    );

    cleanup(&pool, subject).await;
}
