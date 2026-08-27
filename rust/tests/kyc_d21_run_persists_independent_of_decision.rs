//! EOP-DD-KYCUBO-D2.1 reconciliation, 2026-08-24 corrective tranche Item 4.
//!
//! Reconciliation finding: `DecideApprove::execute` computes and persists an
//! `EvaluationRun` (via `compute_and_persist_run`), then — if the run's work
//! list is non-empty — refuses with a K-23 error before ever inserting a
//! decision record. Before this tranche, the run insert used
//! `scope.executor()`: the SAME ambient transaction the whole op ran under.
//! A K-23 refusal therefore rolled back the run right along with the
//! (never-attempted) decision record — the run that justified the refusal
//! did not survive the refusal it justified. That is exactly backwards for
//! an audit trail: a refused approval is precisely the case where the
//! evidence needs to persist.
//!
//! Ruling (Adam, 2026-08-24, asked via the corrective tranche's Item 4 STOP):
//! persist the run in its own committed unit, independent of the decision.
//! `crates/ob-poc-kyc-decide/src/lib.rs::persist_run` now commits via a
//! fresh connection off `scope.pool()`, never joining the ambient
//! transaction — see that function's doc comment for the accepted tradeoff
//! (run/decision atomicity is no longer guaranteed; a crash between the two
//! commits can leave an orphan run with no citing decision, never the
//! reverse).
//!
//! This gate proves the fix through the REAL `SemOsVerbOp` dispatch path:
//! build a board with an alleged (non-Proved) type, dispatch `DecideApprove`
//! directly (not through a helper that panics on error — refusal is the
//! expected outcome here), confirm the op returns Err (K-23 fired), roll
//! the ambient transaction back exactly as a real caller would on error, and
//! then — from a SEPARATE connection — confirm the run row is there anyway.
//!
//! RED-proofed: `cp` a backup of `lib.rs`, temporarily revert `persist_run`
//! to use `scope.executor()` again, re-run this test — confirms it fails
//! (zero rows), restore, re-confirm green.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionOutcome};
use ob_poc::domain_ops::kyc_stream_ops::KycSubjectPlace;
use ob_poc_kyc_decide::{test_verb_execution_context_with_session, DecideApprove};
use ob_poc_kyc_substrate::SubjectId;
use ob_poc_types::TransactionScopeId;
use sem_os_postgres::ops::SemOsVerbOp;

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string())
}

async fn pool() -> PgPool {
    PgPoolOptions::new().max_connections(4).connect(&database_url()).await.expect("connect to test DB")
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

/// Dispatch a verb op and COMMIT, unlike the K-23 dispatch below — used only
/// for board setup, where every call is expected to succeed.
async fn run_ok(op: &dyn SemOsVerbOp, args: serde_json::Value, pool: &PgPool) {
    let mut ctx = test_verb_execution_context_with_session(Uuid::new_v4());
    let mut scope = Scope::begin(pool).await;
    op.execute(&args, &mut ctx, &mut scope).await.unwrap_or_else(|e| panic!("{}: {e}", op.fqn()));
    scope.tx.commit().await.unwrap();
}

async fn cleanup(pool: &PgPool, subject: SubjectId) {
    for t in [
        "kyc_intent_events",
        "kyc_subject_streams",
        "kyc_control_edge_projection",
        "kyc_decision_records",
        "kyc_evaluation_runs",
    ] {
        let _ = sqlx::query(&format!(r#"DELETE FROM "ob-poc".{t} WHERE subject_root = $1"#))
            .bind(subject.0)
            .execute(pool)
            .await;
    }
}

#[tokio::test]
async fn refused_approval_leaves_its_justifying_run_persisted() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());

    // A board with exactly one entity, alleged (not Proved) type — the
    // `ProvenTypeCheck` comes back Unevaluable for it, so the run's work
    // list is non-empty and `DecideApprove` must refuse (K-23).
    run_ok(
        &KycSubjectPlace,
        serde_json::json!({ "subject-id": subject.0, "is_natural_person": false, "entity-type": "private_limited_company" }),
        &pool,
    )
    .await;
    let entity = Uuid::new_v4();
    run_ok(
        &KycSubjectPlace,
        serde_json::json!({ "subject-id": subject.0, "entity-id": entity, "is_natural_person": false, "entity-type": "private_limited_company" }),
        &pool,
    )
    .await;

    // Sanity: nothing decided yet, no run yet.
    let pre: i64 = sqlx::query_scalar(r#"SELECT count(*) FROM "ob-poc".kyc_evaluation_runs WHERE subject_root=$1"#)
        .bind(subject.0)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(pre, 0, "no run should exist before the approve attempt");

    // Dispatch DecideApprove DIRECTLY — expect Err, then roll back exactly
    // as a real caller does on error (the transaction is simply dropped
    // without a commit; sqlx rolls back on drop).
    {
        let mut ctx = test_verb_execution_context_with_session(Uuid::new_v4());
        let mut scope = Scope::begin(&pool).await;
        let result = DecideApprove.execute(&serde_json::json!({ "subject-id": subject.0 }), &mut ctx, &mut scope).await;
        match result {
            Err(e) => assert!(
                e.to_string().contains("K-23"),
                "expected a K-23 refusal, got a different error: {e}"
            ),
            Ok(VerbExecutionOutcome::Record(v)) => {
                panic!("expected DecideApprove to refuse via K-23 on an alleged-only board, but it succeeded: {v}")
            }
            Ok(other) => panic!("expected DecideApprove to refuse via K-23, got unexpected Ok: {other:?}"),
        }
        // No commit — matches production: the Sequencer rolls back the
        // ambient transaction when a `SemOsVerbOp::execute` call errors.
        drop(scope);
    }

    // No decision record was ever written — the refusal held.
    let decisions: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM "ob-poc".kyc_decision_records WHERE subject_root=$1"#,
    )
    .bind(subject.0)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(decisions, 0, "a K-23 refusal must never leave a decision record behind");

    // But the run that justified the refusal survived the rollback — the
    // whole point of Item 4's fix, and the property this gate exists to
    // hold. Read from a fresh connection, not the (rolled-back, dropped)
    // scope above.
    let post: i64 = sqlx::query_scalar(r#"SELECT count(*) FROM "ob-poc".kyc_evaluation_runs WHERE subject_root=$1"#)
        .bind(subject.0)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        post, 1,
        "the EvaluationRun that justified the K-23 refusal must survive the refusal's rollback \
         (Item 4: persist_run commits independently via scope.pool(), not scope.executor())"
    );

    let trigger: String = sqlx::query_scalar(
        r#"SELECT trigger FROM "ob-poc".kyc_evaluation_runs WHERE subject_root=$1"#,
    )
    .bind(subject.0)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(trigger, "kyc_ubo.decide.subject.approve", "the surviving run must be the one the approve attempt computed");

    cleanup(&pool, subject).await;
}
