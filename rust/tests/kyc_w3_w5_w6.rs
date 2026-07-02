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
    KycObligationCreate, KycObligationSatisfy, KycPersonApprove, KycRoleAssign, KycSubjectRegister,
};
use ob_poc_kyc_store::{
    PgKycObligationDrainer, PgKycProjectionDrainer, CONTROL_EDGE_PROJECTION_EFFECT,
    OBLIGATION_PROJECTION_EFFECT,
};
use ob_poc_kyc_substrate::{phase1_lexicon, FoldRegistry, SubjectId, V1FoldImpl};
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
    r.register(phase1_lexicon().hash, Arc::new(V1FoldImpl));
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
    for ek in [CONTROL_EDGE_PROJECTION_EFFECT, OBLIGATION_PROJECTION_EFFECT] {
        let _ = sqlx::query(
            r#"DELETE FROM "public".outbox WHERE effect_kind=$1 AND idempotency_key LIKE $2"#,
        )
        .bind(ek)
        .bind(format!("{}:%", subject.0))
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

    // W3: register subject + assign role (obligation basis)
    dispatch(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject.0, "is_natural_person": true }),
        &pool,
    )
    .await;
    dispatch(&KycRoleAssign, serde_json::json!({ "subject-id": subject.0, "role": "beneficial_owner", "jurisdiction": "LU" }), &pool).await;

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

    // W5: approve the subject
    dispatch(
        &KycPersonApprove,
        serde_json::json!({ "subject-id": subject.0 }),
        &pool,
    )
    .await;

    // W6: drain both projections
    PgKycProjectionDrainer::drain_all(&pool, &registry, 100)
        .await
        .unwrap();
    PgKycObligationDrainer::drain_all(&pool, &registry, 100)
        .await
        .unwrap();

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

    // Verify: subject rollup shows Approved (K-23 approval gate)
    let (overall, all_term): (String, bool) = sqlx::query_as(
        r#"SELECT overall_state, all_terminal FROM "ob-poc".kyc_subject_rollup_projection
           WHERE subject_root = $1"#,
    )
    .bind(subject.0)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(overall, "Approved", "subject approved (K-23)");
    assert!(all_term, "all obligations terminal");

    cleanup(&pool, subject).await;
}
