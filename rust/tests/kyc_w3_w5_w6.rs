//! W3+W5+W6 end-to-end proof.
//!
//! Proves the obligation lifecycle: subject registered → role assigned →
//! obligation created (with basis) → parallel tracks advanced → obligation
//! satisfied → subject approved → both projections correct after draining.

use std::sync::Arc;

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionContext, VerbExecutionOutcome};
use ob_poc::domain_ops::kyc_stream_ops::{
    KycObligationCreate, KycObligationSatisfy, KycSubjectRegister,
};
// kyc.person.approve renamed kyc_ubo.decide.subject.approve TS.6 P2 — moved to ob-poc-kyc-decide.
use ob_poc_kyc_decide::DecideApprove;
use ob_poc_kyc_store::{PgKycObligationProjector, PgKycProjector};
use ob_poc_kyc_substrate::{assembly_lexicon, FoldRegistry, SubjectId, V1FoldImpl};
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

fn v1_registry() -> FoldRegistry {
    let mut r = FoldRegistry::new();
    r.register(assembly_lexicon().hash, Arc::new(V1FoldImpl));
    r
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

async fn cleanup(pool: &PgPool, subject: SubjectId) {
    for t in [
        "kyc_decision_records",
        "kyc_intent_events",
        "kyc_subject_streams",
        "kyc_control_edge_projection",
        "kyc_obligation_projection",
        "kyc_subject_rollup_projection",
    ] {
        let _ = sqlx::query(&format!(
            r#"DELETE FROM "ob-poc".{t} WHERE subject_root = $1"#
        ))
        .bind(subject.0)
        .execute(pool)
        .await;
    }
}

async fn dispatch(
    op: &dyn SemOsVerbOp,
    args: serde_json::Value,
    pool: &PgPool,
) -> VerbExecutionOutcome {
    let mut ctx = VerbExecutionContext::default();
    let mut scope = Scope::begin(pool).await;
    let out = op.execute(&args, &mut ctx, &mut scope).await.expect("op");
    scope.commit().await;
    out
}

#[tokio::test]
async fn w3_w5_w6_obligation_lifecycle_end_to_end() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let registry = v1_registry();

    // W3: register subject. kyc.role.assign retired 2026-08-12 (T0.3 K-G7 —
    // it was fold-blind; the obligation basis below (K-21) is recorded
    // entirely by obligation.create's own required `role` arg, asserted at
    // `o_role` below, independent of role.assign ever having been called).
    dispatch(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject.0, "is_natural_person": true }),
        &pool,
    )
    .await;

    // W5: create obligation with the recorded basis
    let obligation_id = Uuid::new_v4();
    dispatch(&KycObligationCreate, serde_json::json!({ "subject-id": subject.0, "obligation-id": obligation_id, "role": "beneficial_owner", "jurisdiction": "LU" }), &pool).await;

    // W5: satisfy all tracks (simplest form)
    dispatch(
        &KycObligationSatisfy,
        serde_json::json!({ "subject-id": subject.0, "obligation-id": obligation_id }),
        &pool,
    )
    .await;

    // W5: approve the subject (kyc_ubo.decide.subject.approve, TS.6 P2 — writes to
    // kyc_decision_records, not the fact stream)
    dispatch(
        &DecideApprove,
        serde_json::json!({ "subject-id": subject.0 }),
        &pool,
    )
    .await;

    // W6: project THIS subject, on demand.
    //
    // There is no queue any more (2026-08-22 ruling — the outbox fan-out and
    // both drainers are gone). This used to call `drain_all`, which claimed
    // from the shared `public.outbox` GLOBALLY, so it folded whatever other
    // subjects were pending — in a long-lived dev database, a months-deep
    // backlog spanning every historical lexicon version, against a registry
    // holding only the current hash. That is the jam the ruling removed.
    let mut conn = pool.acquire().await.unwrap();
    PgKycProjector::rebuild_control_edges(&mut conn, &registry, subject)
        .await
        .unwrap();
    PgKycObligationProjector::rebuild_obligations(&mut conn, &registry, subject)
        .await
        .unwrap();
    drop(conn);

    // Verify: obligation projection has the satisfied obligation
    let (o_state, o_role): (String, String) = sqlx::query_as(
        r#"SELECT identity_state, basis_role FROM "ob-poc".kyc_obligation_projection
           WHERE subject_root = $1 AND obligation_id = $2"#,
    )
    .bind(subject.0)
    .bind(obligation_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(o_state, "Satisfied", "obligation track satisfied");
    assert_eq!(o_role, "beneficial_owner", "basis role recorded (K-21)");

    // Verify: the Assembly-side rollup shows the subject is eligible for the
    // approval gate — all obligation tracks terminal.
    //
    // TS.6 §1/§5 (2026-08-22): this used to assert `overall_state ==
    // "Approved"`. It cannot any more, and that is the point of the two-pack
    // split: `kyc_ubo.decide.subject.approve` is an Evaluation-pack verb that writes ONLY
    // `kyc_decision_records`, never the fact stream, so no fold over the
    // stream can ever observe a decision. `AllTerminal` is what Assembly can
    // truthfully say; the approval itself is asserted from Evaluation's own
    // record below.
    let (overall, all_term): (String, bool) = sqlx::query_as(
        r#"SELECT overall_state, all_terminal FROM "ob-poc".kyc_subject_rollup_projection
           WHERE subject_root = $1"#,
    )
    .bind(subject.0)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        overall, "AllTerminal",
        "subject eligible for the K-23 approval gate"
    );
    assert!(all_term, "all obligations terminal");

    // Verify: the decision itself, in the Evaluation pack's own record.
    // This is the half of K-23 the stream no longer carries — without it the
    // test would no longer prove the approval happened at all.
    let (verb_fqn, basis): (String, serde_json::Value) = sqlx::query_as(
        r#"SELECT verb_fqn, basis FROM "ob-poc".kyc_decision_records WHERE subject_root = $1"#,
    )
    .bind(subject.0)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(verb_fqn, "kyc_ubo.decide.subject.approve", "kyc_ubo.decide.subject.approve recorded (K-23)");
    assert_eq!(
        basis.get("overall_state").and_then(|v| v.as_str()),
        Some("AllTerminal"),
        "the decision records the gate state it was taken on (K-23, K-35)",
    );

    cleanup(&pool, subject).await;
}
