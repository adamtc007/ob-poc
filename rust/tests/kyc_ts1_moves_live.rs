//! D1 Part A — RED-first live-DB gate for `kyc_ubo.assert.subject.enquiry`
//! (TS.1 §3 move 8), the one of the four original TS.1 §3 moves this file
//! covered that EOP-VS-UBO-GAME-001 T2 leaves untouched.
//!
//! **T2 retirement note (2026-08-27):** this file previously also covered
//! `kyc_ubo.assert.subject.type` (move 2), `.type-correction` (move 7), and
//! `.member-withdrawal` (move 6) — all three retired or dissolved by T2.
//! `type`'s admission-pair/persistence properties are superseded by
//! `tests/kyc_t2_place_remove.rs::place_records_entity_and_type_in_one_move`
//! (register+type merged into `place`, which has no separate
//! "must-already-be-registered" precondition to test — placement and typing
//! are now one atomic act). `type-correction`'s admission-pair/cascade
//! properties have no replacement: the verb is dissolved (§3.2 — a wrong
//! type is `remove` then `place`), not merged, so there is nothing left to
//! test at that shape. `member-withdrawal`'s admission-pair properties are
//! superseded by `remove`'s equivalent gates, added to
//! `tests/kyc_t2_place_remove.rs` in the same T2 tranche.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionContext};
use ob_poc::domain_ops::kyc_stream_ops::KycSubjectRecordEnquiry;
use ob_poc_kyc_store::PgKycEventStore;
use ob_poc_kyc_substrate::{fold_type_registry, SubjectId};
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

async fn run(op: &dyn SemOsVerbOp, args: serde_json::Value, pool: &PgPool) -> serde_json::Value {
    let mut ctx = VerbExecutionContext::default();
    let mut scope = Scope::begin(pool).await;
    let out = op
        .execute(&args, &mut ctx, &mut scope)
        .await
        .unwrap_or_else(|error| panic!("{}: {error}", op.fqn()));
    scope.commit().await;
    match out {
        dsl_runtime::VerbExecutionOutcome::Record(v) => v,
        other => serde_json::to_value(format!("{other:?}")).unwrap(),
    }
}

async fn cleanup(pool: &PgPool, subjects: &[SubjectId]) {
    for s in subjects {
        for t in [
            "kyc_intent_events",
            "kyc_subject_streams",
            "kyc_control_edge_projection",
        ] {
            let _ = sqlx::query(&format!(
                r#"DELETE FROM "ob-poc".{t} WHERE subject_root = $1"#
            ))
            .bind(s.0)
            .execute(pool)
            .await;
        }
    }
}

#[tokio::test]
async fn record_enquiry_admitted_without_registration_and_persists() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());

    // Legal even with nothing else on the stream — "group exists" is true
    // by construction (TS.1 §3 row 8).
    run(
        &KycSubjectRecordEnquiry,
        serde_json::json!({
            "subject-id": subject.0,
            "sources-consulted": ["GLEIF", "companies-house"],
            "searches-run": ["group structure search"],
        }),
        &pool,
    )
    .await;

    let mut conn = pool.acquire().await.expect("acquire connection");
    let events = PgKycEventStore::load_events(&mut conn, subject).await.unwrap();
    let refs: Vec<_> = events.iter().collect();
    let type_registry = fold_type_registry(&refs);
    assert_eq!(type_registry.enquiries.len(), 1);
    assert_eq!(
        type_registry.enquiries[0].sources_consulted,
        vec!["GLEIF".to_string(), "companies-house".to_string()]
    );
    assert_eq!(
        type_registry.enquiries[0].searches_run,
        vec!["group structure search".to_string()]
    );

    cleanup(&pool, &[subject]).await;
}
