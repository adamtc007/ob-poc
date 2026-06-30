//! Transaction-time recovery tests (EOP-DD-KYCUBO-002 §5 / B1 / D1 / K-33).
//!
//! Proves `recover_control_at` folds the `committed_at <= T` **prefix**, that
//! `committed_at` is monotonic with `seq` (the `clock_timestamp()` fix), and
//! that recovery at "now" equals folding the whole stream.
//!
//! Events are appended in SEPARATE transactions so each gets a distinct
//! `committed_at`. Fresh random subject + cleanup.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

use ob_poc_kyc_store::PgKycEventStore;
use ob_poc_kyc_substrate::{
    phase1_lexicon, AuthorityRef, FoldRegistry, IdemKey, IntentEvent, Principal, SubjectId,
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

fn event(subject: SubjectId, verb: &str, payload: serde_json::Value, idem: &str) -> IntentEvent {
    IntentEvent::new(
        subject,
        verb,
        Principal::test_analyst(),
        AuthorityRef("analyst".into()),
        TargetBinding::for_subject(subject),
        payload,
        DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
    )
    .with_lexicon_hash(phase1_lexicon().hash)
    .with_idempotency_key(IdemKey::new(idem))
}

fn register(subject: SubjectId) -> IntentEvent {
    event(subject, "kyc.subject.register", serde_json::json!({ "is_natural_person": false }), "reg")
}

fn assert_control(subject: SubjectId, to: Uuid, idem: &str) -> IntentEvent {
    event(
        subject,
        "ubo.edge.assert-control",
        serde_json::json!({
            "from_entity_id": Uuid::new_v4(),
            "to_entity_id": to,
            "edge_kind": "control",
        }),
        idem,
    )
}

async fn append_committed(pool: &PgPool, registry: &FoldRegistry, ev: &IntentEvent) {
    let mut tx = pool.begin().await.unwrap();
    PgKycEventStore::append(&mut tx, registry, ev, |_| Ok(()))
        .await
        .unwrap();
    tx.commit().await.unwrap();
}

async fn committed_at_of(pool: &PgPool, subject: SubjectId, seq: i64) -> DateTime<Utc> {
    sqlx::query_scalar(
        r#"SELECT committed_at FROM "ob-poc".kyc_intent_events
           WHERE subject_root = $1 AND seq = $2"#,
    )
    .bind(subject.0)
    .bind(seq)
    .fetch_one(pool)
    .await
    .unwrap()
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

#[tokio::test]
async fn recover_control_at_folds_the_transaction_time_prefix() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let registry = v1_registry();

    // Three events, each committed in its own transaction → distinct committed_at.
    append_committed(&pool, &registry, &register(subject)).await;
    append_committed(&pool, &registry, &assert_control(subject, Uuid::new_v4(), "edge-a")).await;
    append_committed(&pool, &registry, &assert_control(subject, Uuid::new_v4(), "edge-b")).await;

    let t0 = committed_at_of(&pool, subject, 0).await;
    let t1 = committed_at_of(&pool, subject, 1).await;
    let t2 = committed_at_of(&pool, subject, 2).await;

    // B1 fix: committed_at is monotonic with seq (clock_timestamp under the lock).
    assert!(t0 < t1 && t1 < t2, "committed_at strictly increases with seq: {t0} < {t1} < {t2}");

    let mut conn = pool.acquire().await.unwrap();

    // As-of t0 → prefix {register}: registered, zero edges.
    let s0 = PgKycEventStore::recover_control_at(&mut conn, &registry, subject, t0).await.unwrap();
    assert!(s0.registered, "register is in the t0 prefix");
    assert_eq!(s0.edges.len(), 0, "no edges asserted yet at t0");

    // As-of t1 → {register, edge-a}: one edge.
    let s1 = PgKycEventStore::recover_control_at(&mut conn, &registry, subject, t1).await.unwrap();
    assert_eq!(s1.edges.len(), 1, "one edge at t1");

    // As-of t2 (and as-of now) → full stream: two edges.
    let s2 = PgKycEventStore::recover_control_at(&mut conn, &registry, subject, t2).await.unwrap();
    assert_eq!(s2.edges.len(), 2, "two edges at t2");
    let now = PgKycEventStore::recover_control_at(&mut conn, &registry, subject, Utc::now()).await.unwrap();
    assert_eq!(now.edges.len(), 2, "recover at now == fold whole stream");

    // Before the first event → empty fold (not registered, no edges).
    let before = t0 - chrono::Duration::seconds(1);
    let sb = PgKycEventStore::recover_control_at(&mut conn, &registry, subject, before).await.unwrap();
    assert!(!sb.registered, "nothing committed before t0");
    assert_eq!(sb.edges.len(), 0);

    // Prefix loader counts line up with the recovery folds.
    let n_before = PgKycEventStore::load_events_up_to_committed(&mut conn, subject, before).await.unwrap();
    let n1 = PgKycEventStore::load_events_up_to_committed(&mut conn, subject, t1).await.unwrap();
    assert_eq!(n_before.len(), 0);
    assert_eq!(n1.len(), 2, "prefix at t1 is exactly [seq0, seq1]");
    assert_eq!(n1.iter().map(|e| e.seq).collect::<Vec<_>>(), vec![0, 1]);

    cleanup(&pool, subject).await;
}

/// The adversarial begin-order that `now()` gets wrong: a transaction begins
/// EARLY (early transaction-start clock) but is assigned a LATE seq because it
/// touches the subject last. With `DEFAULT now()` the high-seq event would stamp
/// an *earlier* committed_at than the low-seq event → non-monotonic → holey
/// recovery. `clock_timestamp()` (real wall-clock at the INSERT, under the lock)
/// keeps committed_at monotonic with seq. This test fails under `now()`.
#[tokio::test]
async fn committed_at_is_monotonic_with_seq_under_adversarial_begin_order() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let registry = v1_registry();

    // tx_b begins FIRST (early start clock) but will touch the subject LAST.
    let mut tx_b = pool.begin().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;

    // tx_a begins later, inserts seq0, commits.
    {
        let mut tx_a = pool.begin().await.unwrap();
        PgKycEventStore::append(&mut tx_a, &registry, &register(subject), |_| Ok(()))
            .await
            .unwrap();
        tx_a.commit().await.unwrap();
    }

    // tx_b (early begin) now inserts seq1 and commits.
    PgKycEventStore::append(
        &mut tx_b,
        &registry,
        &assert_control(subject, Uuid::new_v4(), "adversarial"),
        |_| Ok(()),
    )
    .await
    .unwrap();
    tx_b.commit().await.unwrap();

    let t0 = committed_at_of(&pool, subject, 0).await; // tx_a — LATE begin, LOW seq
    let t1 = committed_at_of(&pool, subject, 1).await; // tx_b — EARLY begin, HIGH seq

    assert!(
        t0 < t1,
        "committed_at must be monotonic with seq even when the higher-seq event's \
         transaction began earlier (requires clock_timestamp(), not now()): seq0={t0} seq1={t1}"
    );

    cleanup(&pool, subject).await;
}
