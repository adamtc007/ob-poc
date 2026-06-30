//! Control-edge projection tests (EOP-DD-KYCUBO-002 §5).
//!
//! Proves the load-bearing §5 properties of `rebuild_projection`:
//! - idempotent (re-running yields identical rows),
//! - convergent (the projection always equals the fold),
//! - tracks derived edge status (Asserted→Evidenced→Verified→Superseded),
//! - disposable (K-34): drop the rows, rebuild from the stream, no data loss.

use std::sync::Arc;

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use ob_poc_kyc_store::{PgKycEventStore, PgKycProjector};
use ob_poc_kyc_substrate::{
    phase1_lexicon, AuthorityRef, EdgeId, FoldRegistry, IdemKey, IntentEvent, Principal, SubjectId,
    TargetBinding, V1FoldImpl,
};

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

fn v1_registry() -> FoldRegistry {
    let mut r = FoldRegistry::new();
    r.register(phase1_lexicon().hash, Arc::new(V1FoldImpl));
    r
}

fn base(subject: SubjectId, verb: &str, target: TargetBinding, payload: serde_json::Value, idem: &str) -> IntentEvent {
    IntentEvent::new(
        subject,
        verb,
        Principal::test_analyst(),
        AuthorityRef("analyst".into()),
        target,
        payload,
        chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
    )
    .with_lexicon_hash(phase1_lexicon().hash)
    .with_idempotency_key(IdemKey::new(idem))
}

fn register(subject: SubjectId) -> IntentEvent {
    base(subject, "kyc.subject.register", TargetBinding::for_subject(subject),
         serde_json::json!({ "is_natural_person": false }), "reg")
}

fn assert_control(subject: SubjectId, edge: Uuid, idem: &str) -> IntentEvent {
    base(subject, "ubo.edge.assert-control", TargetBinding::for_edge(subject, EdgeId(edge)),
        serde_json::json!({
            "from_entity_id": Uuid::new_v4(),
            "to_entity_id": Uuid::new_v4(),
            "edge_id": edge.to_string(),
            "edge_kind": "voting_rights",
        }), idem)
}

fn edge_op(subject: SubjectId, verb: &str, edge: Uuid, idem: &str) -> IntentEvent {
    base(subject, verb, TargetBinding::for_edge(subject, EdgeId(edge)), serde_json::json!({}), idem)
}

async fn append(pool: &PgPool, registry: &FoldRegistry, ev: &IntentEvent) {
    let mut tx = pool.begin().await.unwrap();
    PgKycEventStore::append(&mut tx, registry, ev, |_| Ok(())).await.unwrap();
    tx.commit().await.unwrap();
}

async fn rebuild(pool: &PgPool, registry: &FoldRegistry, subject: SubjectId) -> usize {
    let mut tx = pool.begin().await.unwrap();
    let stats = PgKycProjector::rebuild_control_edges(&mut tx, registry, subject).await.unwrap();
    tx.commit().await.unwrap();
    stats.edges_written
}

async fn projection_rows(pool: &PgPool, subject: SubjectId) -> Vec<(Uuid, String)> {
    sqlx::query(
        r#"SELECT edge_id, status FROM "ob-poc".kyc_control_edge_projection
           WHERE subject_root = $1 ORDER BY edge_id"#,
    )
    .bind(subject.0)
    .fetch_all(pool)
    .await
    .unwrap()
    .iter()
    .map(|r| (r.get::<Uuid, _>("edge_id"), r.get::<String, _>("status")))
    .collect()
}

async fn status_of(pool: &PgPool, subject: SubjectId, edge: Uuid) -> String {
    sqlx::query_scalar(
        r#"SELECT status FROM "ob-poc".kyc_control_edge_projection
           WHERE subject_root = $1 AND edge_id = $2"#,
    )
    .bind(subject.0)
    .bind(edge)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn cleanup(pool: &PgPool, subject: SubjectId) {
    for t in ["kyc_intent_events", "kyc_subject_streams", "kyc_control_edge_projection"] {
        let _ = sqlx::query(&format!(r#"DELETE FROM "ob-poc".{t} WHERE subject_root = $1"#))
            .bind(subject.0)
            .execute(pool)
            .await;
    }
    let _ = sqlx::query(r#"DELETE FROM "public".outbox WHERE effect_kind = 'kyc.projection.control_edges' AND idempotency_key LIKE $1"#)
        .bind(format!("{}:%", subject.0))
        .execute(pool)
        .await;
}

#[tokio::test]
async fn rebuild_is_idempotent_and_matches_fold() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let registry = v1_registry();
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());

    append(&pool, &registry, &register(subject)).await;
    append(&pool, &registry, &assert_control(subject, a, "a")).await;
    append(&pool, &registry, &assert_control(subject, b, "b")).await;

    assert_eq!(rebuild(&pool, &registry, subject).await, 2, "two edges projected");
    let first = projection_rows(&pool, subject).await;
    assert_eq!(first.len(), 2);
    assert!(first.iter().all(|(_, s)| s == "Asserted"));

    // Idempotent: re-running yields identical rows (full replace, no dup PK).
    assert_eq!(rebuild(&pool, &registry, subject).await, 2);
    assert_eq!(projection_rows(&pool, subject).await, first, "rebuild is idempotent");

    cleanup(&pool, subject).await;
}

#[tokio::test]
async fn projection_tracks_derived_edge_status() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let registry = v1_registry();
    let edge = Uuid::new_v4();

    append(&pool, &registry, &register(subject)).await;
    append(&pool, &registry, &assert_control(subject, edge, "assert")).await;
    rebuild(&pool, &registry, subject).await;
    assert_eq!(status_of(&pool, subject, edge).await, "Asserted");

    append(&pool, &registry, &edge_op(subject, "ubo.edge.attach-evidence", edge, "ev")).await;
    rebuild(&pool, &registry, subject).await;
    assert_eq!(status_of(&pool, subject, edge).await, "Evidenced");

    append(&pool, &registry, &edge_op(subject, "ubo.edge.verify", edge, "vf")).await;
    rebuild(&pool, &registry, subject).await;
    assert_eq!(status_of(&pool, subject, edge).await, "Verified");

    // K-13: supersede-never-delete — the edge stays, status flips.
    append(&pool, &registry, &edge_op(subject, "ubo.edge.supersede", edge, "sup")).await;
    rebuild(&pool, &registry, subject).await;
    assert_eq!(status_of(&pool, subject, edge).await, "Superseded");

    cleanup(&pool, subject).await;
}

#[tokio::test]
async fn projection_is_disposable_and_rebuilds_from_stream() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let registry = v1_registry();
    let (a, b) = (Uuid::new_v4(), Uuid::new_v4());

    append(&pool, &registry, &register(subject)).await;
    append(&pool, &registry, &assert_control(subject, a, "a")).await;
    append(&pool, &registry, &edge_op(subject, "ubo.edge.attach-evidence", a, "ev")).await;
    append(&pool, &registry, &assert_control(subject, b, "b")).await;

    rebuild(&pool, &registry, subject).await;
    let before = projection_rows(&pool, subject).await;
    assert_eq!(before.len(), 2);

    // Drop the entire projection (K-34: disposable).
    sqlx::query(r#"DELETE FROM "ob-poc".kyc_control_edge_projection WHERE subject_root = $1"#)
        .bind(subject.0)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(projection_rows(&pool, subject).await.len(), 0, "projection dropped");

    // Rebuild purely from the stream → identical rows, no data loss.
    rebuild(&pool, &registry, subject).await;
    assert_eq!(projection_rows(&pool, subject).await, before, "rebuilt from stream, identical");

    cleanup(&pool, subject).await;
}
