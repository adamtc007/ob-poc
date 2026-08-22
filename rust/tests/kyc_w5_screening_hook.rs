//! W5 screening hook proof (EOP-DD-KYCUBO-004 Part 1): a real `screening.complete`
//! / `screening.review-hit` call now fans out to the dsl.kyc obligation stream —
//! closing the gap where `assert.screening` existed and folded
//! correctly but nothing in the real screening lifecycle ever called it.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionContext, VerbExecutionOutcome};
use ob_poc::domain_ops::kyc_stream_ops::{
    KycObligationCreate, KycSubjectRegister, ScreeningComplete, ScreeningReviewHit,
};
use ob_poc_kyc_store::PgKycEventStore;
use ob_poc_kyc_substrate::{fold_obligations, ObligationId, SubjectId, TrackState};
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

/// Legacy fixture: one throwaway CBU + entity + case + workstream, deleted in
/// `cleanup_fixture`. `entity_id` doubles as the dsl.kyc `subject-id` — see
/// `tests/kyc_w3_w5_w6.rs` and `apply_screening_outcome_to_obligations`'s doc
/// comment in `kyc_stream_ops.rs` for why that's the correct linkage.
struct Fixture {
    cbu_id: Uuid,
    entity_id: Uuid,
    case_id: Uuid,
    workstream_id: Uuid,
}

async fn seed_fixture(pool: &PgPool) -> Fixture {
    let cbu_id = Uuid::new_v4();
    sqlx::query(r#"INSERT INTO "ob-poc".cbus (cbu_id, name) VALUES ($1, 'W5 hook test CBU')"#)
        .bind(cbu_id)
        .execute(pool)
        .await
        .expect("insert cbu");

    let (entity_type_id,): (Uuid,) =
        sqlx::query_as(r#"SELECT entity_type_id FROM "ob-poc".entity_types LIMIT 1"#)
            .fetch_one(pool)
            .await
            .expect("at least one entity_type seed row");

    let entity_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name)
           VALUES ($1, $2, $3)"#,
    )
    .bind(entity_id)
    .bind(entity_type_id)
    .bind(format!("W5 hook test entity {entity_id}"))
    .execute(pool)
    .await
    .expect("insert entity");

    let case_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO "ob-poc".cases (case_id, cbu_id, case_ref) VALUES ($1, $2, $3)"#,
    )
    .bind(case_id)
    .bind(cbu_id)
    .bind(format!("W5-{}", &case_id.to_string()[..8]))
    .execute(pool)
    .await
    .expect("insert case");

    let workstream_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO "ob-poc".entity_workstreams (workstream_id, case_id, entity_id, status)
           VALUES ($1, $2, $3, 'SCREEN')"#,
    )
    .bind(workstream_id)
    .bind(case_id)
    .bind(entity_id)
    .execute(pool)
    .await
    .expect("insert workstream");

    Fixture {
        cbu_id,
        entity_id,
        case_id,
        workstream_id,
    }
}

async fn seed_screening(pool: &PgPool, workstream_id: Uuid, screening_type: &str) -> Uuid {
    let screening_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO "ob-poc".screenings (screening_id, workstream_id, screening_type, status)
           VALUES ($1, $2, $3, 'RUNNING')"#,
    )
    .bind(screening_id)
    .bind(workstream_id)
    .bind(screening_type)
    .execute(pool)
    .await
    .expect("insert screening");
    screening_id
}

async fn cleanup_fixture(pool: &PgPool, f: &Fixture) {
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".screenings WHERE workstream_id = $1"#)
        .bind(f.workstream_id)
        .execute(pool)
        .await;
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".entity_workstreams WHERE workstream_id = $1"#)
        .bind(f.workstream_id)
        .execute(pool)
        .await;
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".cases WHERE case_id = $1"#)
        .bind(f.case_id)
        .execute(pool)
        .await;
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".entities WHERE entity_id = $1"#)
        .bind(f.entity_id)
        .execute(pool)
        .await;
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".cbus WHERE cbu_id = $1"#)
        .bind(f.cbu_id)
        .execute(pool)
        .await;
}

async fn cleanup_stream(pool: &PgPool, subject: SubjectId) {
    for t in ["kyc_intent_events", "kyc_subject_streams"] {
        let _ = sqlx::query(&format!(
            r#"DELETE FROM "ob-poc".{t} WHERE subject_root = $1"#
        ))
        .bind(subject.0)
        .execute(pool)
        .await;
    }
}

async fn screening_track_for(pool: &PgPool, subject: SubjectId, obligation_id: Uuid) -> TrackState {
    let mut conn = pool.acquire().await.expect("conn");
    let events = PgKycEventStore::load_events(&mut conn, subject)
        .await
        .expect("load_events");
    let refs: Vec<_> = events.iter().collect();
    let state = fold_obligations(&refs);
    state
        .obligations
        .get(&ObligationId(obligation_id))
        .expect("obligation present in fold")
        .screening
        .clone()
}

#[tokio::test]
async fn screening_complete_clear_fans_out_satisfied_to_open_obligation() {
    let pool = pool().await;
    let fixture = seed_fixture(&pool).await;
    let subject = SubjectId(fixture.entity_id);

    dispatch(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject.0, "is_natural_person": true }),
        &pool,
    )
    .await;
    let obligation_id = Uuid::new_v4();
    dispatch(
        &KycObligationCreate,
        serde_json::json!({ "subject-id": subject.0, "obligation-id": obligation_id, "role": "beneficial_owner" }),
        &pool,
    )
    .await;

    let screening_id = seed_screening(&pool, fixture.workstream_id, "SANCTIONS").await;

    dispatch(
        &ScreeningComplete,
        serde_json::json!({ "screening-id": screening_id, "status": "CLEAR" }),
        &pool,
    )
    .await;

    let track = screening_track_for(&pool, subject, obligation_id).await;
    assert!(
        matches!(track, TrackState::Satisfied { .. }),
        "expected Satisfied, got {track:?}"
    );

    let (status,): (String,) =
        sqlx::query_as(r#"SELECT status FROM "ob-poc".screenings WHERE screening_id = $1"#)
            .bind(screening_id)
            .fetch_one(&pool)
            .await
            .expect("screening row");
    assert_eq!(status, "CLEAR", "legacy screenings row must still be written");

    cleanup_stream(&pool, subject).await;
    cleanup_fixture(&pool, &fixture).await;
}

#[tokio::test]
async fn screening_review_hit_confirmed_fans_out_rejected() {
    let pool = pool().await;
    let fixture = seed_fixture(&pool).await;
    let subject = SubjectId(fixture.entity_id);

    dispatch(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject.0, "is_natural_person": true }),
        &pool,
    )
    .await;
    let obligation_id = Uuid::new_v4();
    dispatch(
        &KycObligationCreate,
        serde_json::json!({ "subject-id": subject.0, "obligation-id": obligation_id, "role": "director" }),
        &pool,
    )
    .await;

    let screening_id = seed_screening(&pool, fixture.workstream_id, "PEP").await;

    dispatch(
        &ScreeningReviewHit,
        serde_json::json!({ "screening-id": screening_id, "status": "HIT_CONFIRMED", "notes": "confirmed PEP match" }),
        &pool,
    )
    .await;

    let track = screening_track_for(&pool, subject, obligation_id).await;
    assert!(
        matches!(track, TrackState::Rejected { .. }),
        "expected Rejected, got {track:?}"
    );

    cleanup_stream(&pool, subject).await;
    cleanup_fixture(&pool, &fixture).await;
}

#[tokio::test]
async fn screening_complete_unrecognized_status_fails_closed_without_writing() {
    let pool = pool().await;
    let fixture = seed_fixture(&pool).await;
    let screening_id = seed_screening(&pool, fixture.workstream_id, "SANCTIONS").await;

    let mut ctx = VerbExecutionContext::default();
    let mut scope = Scope::begin(&pool).await;
    let err = ScreeningComplete
        .execute(
            &serde_json::json!({ "screening-id": screening_id, "status": "NOT_A_REAL_STATUS" }),
            &mut ctx,
            &mut scope,
        )
        .await
        .expect_err("unrecognized status must be rejected");
    assert!(err.to_string().contains("unrecognized status"));
    drop(scope); // rolled back, not committed — nothing written

    let (status,): (String,) =
        sqlx::query_as(r#"SELECT status FROM "ob-poc".screenings WHERE screening_id = $1"#)
            .bind(screening_id)
            .fetch_one(&pool)
            .await
            .expect("screening row");
    assert_eq!(status, "RUNNING", "must fail before touching the row");

    cleanup_fixture(&pool, &fixture).await;
}
