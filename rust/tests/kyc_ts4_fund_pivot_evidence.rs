//! TS.4 gate test — EOP-DD-KYCUBO-TS.4 §2 Ruling A, Ruling 2f / CTN-2e:
//! `mandate_pivot_without_evidence_computes_but_cannot_freeze`.
//!
//! Live-DB harness, same pattern as `kyc_ts4_nominee.rs`'s (d) test: drives
//! the REAL `kyc_ubo.decide.determination.freeze` op end-to-end. A governing-mandate
//! pivot with no contract evidence attached must COMPUTE (candidates
//! resolve, `fund_control_strategy` runs) but the freeze itself must
//! HARD-ERROR — "record freely, conclude carefully". Attaching evidence to
//! the mandate edge and retrying must then succeed.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionContext};
use ob_poc::domain_ops::kyc_stream_ops::{
    KycSubjectClassifyStructure, KycSubjectPlace, UboDeterminationFreeze, UboEdgeAssertControl,
    UboEdgeAttachEvidence, UboEdgeReconcileConflict,
};
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
        Self { tx: p.begin().await.unwrap(), pool: p.clone(), id: TransactionScopeId::new() }
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

async fn run_fallible(
    op: &dyn SemOsVerbOp,
    args: serde_json::Value,
    pool: &PgPool,
) -> anyhow::Result<serde_json::Value> {
    let mut ctx = VerbExecutionContext::default();
    let mut scope = Scope::begin(pool).await;
    let out = op.execute(&args, &mut ctx, &mut scope).await?;
    scope.commit().await;
    Ok(match out {
        dsl_runtime::VerbExecutionOutcome::Record(v) => v,
        other => serde_json::to_value(format!("{other:?}")).unwrap(),
    })
}

async fn cleanup(pool: &PgPool, subject: Uuid) {
    for t in [
        "kyc_intent_events",
        "kyc_subject_streams",
        "kyc_control_edge_projection",
    ] {
        let _ =
            sqlx::query(&format!(r#"DELETE FROM "ob-poc".{t} WHERE subject_root = $1"#))
                .bind(subject)
                .execute(pool)
                .await;
    }
    let _ = sqlx::query(
        r#"DELETE FROM "public".outbox WHERE idempotency_key LIKE $1
           OR (payload->>'determination_subject')::text = $2
           OR (payload->>'subject_root')::text = $2"#,
    )
    .bind(format!("{subject}:%"))
    .bind(subject.to_string())
    .execute(pool)
    .await;
}

#[tokio::test]
async fn mandate_pivot_without_evidence_computes_but_cannot_freeze() {
    let pool = pool().await;
    let subject = Uuid::new_v4(); // the fund
    let manco = Uuid::new_v4();
    let alice = Uuid::new_v4();
    let mandate_edge = Uuid::new_v4();

    run(
        &KycSubjectPlace,
        serde_json::json!({ "subject-id": subject, "is_natural_person": false, "entity-type": "private_limited_company" }),
        &pool,
    )
    .await;
    run(
        &KycSubjectPlace,
        serde_json::json!({ "subject-id": subject, "entity-id": alice, "is_natural_person": true, "entity-type": "natural_person" }),
        &pool,
    )
    .await;
    run(
        &KycSubjectClassifyStructure,
        serde_json::json!({ "subject-id": subject, "structure-class": "investment_fund" }),
        &pool,
    )
    .await;

    // Fund <-ManagementMandate- ManCo <-VotingRights- Alice. The mandate
    // edge is asserted but NEVER evidenced.
    run(
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject, "from_entity_id": manco, "to_entity_id": subject,
            "kind": "management_mandate", "edge-id": mandate_edge,
        }),
        &pool,
    )
    .await;
    run(
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject, "from_entity_id": alice, "to_entity_id": manco,
            "kind": "voting_rights",
        }),
        &pool,
    )
    .await;
    run(&UboEdgeReconcileConflict, serde_json::json!({ "subject-id": subject }), &pool).await;

    // Freeze must REFUSE — the pivot computed, but the mandate is unevidenced.
    let refused = run_fallible(
        &UboDeterminationFreeze,
        serde_json::json!({ "subject-id": subject, "policy-version": "v1.0" }),
        &pool,
    )
    .await;
    assert!(
        refused.is_err(),
        "freeze must refuse while the governing-mandate pivot is unevidenced (Ruling 2f)"
    );
    let msg = refused.unwrap_err().to_string();
    assert!(
        msg.contains("contract evidence") && msg.contains(&mandate_edge.to_string()),
        "the error must name the unevidenced mandate edge; got: {msg}"
    );

    // Attach evidence to the mandate edge — freeze must now succeed.
    run(
        &UboEdgeAttachEvidence,
        serde_json::json!({ "subject-id": subject, "edge-id": mandate_edge }),
        &pool,
    )
    .await;
    let outcome = run(
        &UboDeterminationFreeze,
        serde_json::json!({ "subject-id": subject, "policy-version": "v1.0" }),
        &pool,
    )
    .await;
    assert_eq!(outcome.get("strategy").and_then(|v| v.as_str()), Some("fund_control_strategy"));
    let candidates = outcome.get("candidates").and_then(|c| c.as_array()).cloned().unwrap_or_default();
    assert_eq!(candidates.len(), 1, "Alice resolves once the mandate is evidenced: {outcome:?}");

    cleanup(&pool, subject).await;
}
