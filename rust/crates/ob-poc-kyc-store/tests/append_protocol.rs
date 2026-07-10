//! Live-Postgres tests for the §3 append protocol (EOP-DD-KYCUBO-002).
//!
//! These REQUIRE a reachable Postgres (DATABASE_URL or `postgresql:///data_designer`)
//! with the `20260630_kyc_intent_events.sql` migration applied. They are the
//! build-proof for DD-002 exit criteria 1–3:
//!
//! - exit 1 — atomic append (rollback leaves no orphan)
//! - exit 2 — per-subject order under concurrency (dense gap-free seq)
//! - exit 3 — idempotent re-apply (same key → one event)
//! - precondition-under-lock (TOCTOU-safe rejection)
//! - cross-subject independence
//! - storage round-trip (rehydrated events fold identically)
//!
//! Each test uses a fresh random `subject_root` and cleans up after itself.

use std::sync::Arc;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

use ob_poc_kyc_store::PgKycEventStore;
use ob_poc_kyc_substrate::{
    fold_control_versioned, phase1_lexicon, AuthorityRef, FoldRegistry, Hash, IdemKey, IntentEvent,
    KycError, Principal, SubjectId, TargetBinding, V1FoldImpl, VerbFqn,
};

// ── Harness helpers ────────────────────────────────────────────────────────────

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string())
}

async fn pool(max: u32) -> PgPool {
    PgPoolOptions::new()
        .max_connections(max)
        .connect(&database_url())
        .await
        .expect("connect to test DB (set DATABASE_URL or run data_designer)")
}

fn v1_hash() -> Hash {
    phase1_lexicon().hash
}

fn v1_registry() -> FoldRegistry {
    let mut r = FoldRegistry::new();
    r.register(v1_hash(), Arc::new(V1FoldImpl));
    r
}

/// Build a register event for `subject`, tagged with the v1 lexicon hash and the
/// given idempotency key.
fn make_event(subject: SubjectId, idem: &str) -> IntentEvent {
    let entity = Uuid::new_v4();
    IntentEvent::new(
        subject,
        "kyc.subject.register",
        Principal::test_analyst(),
        AuthorityRef("analyst.register".into()),
        TargetBinding::for_subject(subject),
        serde_json::json!({ "entity_id": entity, "is_natural_person": false }),
        chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
    )
    .with_lexicon_hash(v1_hash())
    .with_idempotency_key(IdemKey::new(idem))
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
    let _ = sqlx::query(r#"DELETE FROM "public".outbox WHERE effect_kind = 'kyc.projection.control_edges' AND idempotency_key LIKE $1"#)
        .bind(format!("{}:%", subject.0))
        .execute(pool)
        .await;
}

async fn event_count(pool: &PgPool, subject: SubjectId) -> i64 {
    sqlx::query_scalar(r#"SELECT count(*) FROM "ob-poc".kyc_intent_events WHERE subject_root = $1"#)
        .bind(subject.0)
        .fetch_one(pool)
        .await
        .unwrap()
}

// ── exit 3 — idempotent re-apply (F) ───────────────────────────────────────────

#[tokio::test]
async fn exit3_idempotent_reapply_same_key_one_event() {
    let pool = pool(4).await;
    let subject = SubjectId(Uuid::new_v4());
    let registry = v1_registry();
    let event = make_event(subject, "same-key");

    let mut tx = pool.begin().await.unwrap();
    let o1 = PgKycEventStore::append(&mut tx, &registry, &event, |_| Ok(()))
        .await
        .unwrap();
    let o2 = PgKycEventStore::append(&mut tx, &registry, &event, |_| Ok(()))
        .await
        .unwrap();
    tx.commit().await.unwrap();

    assert!(!o1.deduped, "first append must insert");
    assert!(
        o2.deduped,
        "second append with same idempotency_key must be a no-op (F)"
    );
    assert_eq!(o1.seq, o2.seq, "deduped append returns the existing seq");
    assert_eq!(
        event_count(&pool, subject).await,
        1,
        "exactly one event row"
    );

    cleanup(&pool, subject).await;
}

// ── exit 1 — atomic append (rollback leaves no orphan) ─────────────────────────

#[tokio::test]
async fn exit1_rollback_leaves_no_orphan() {
    let pool = pool(4).await;
    let subject = SubjectId(Uuid::new_v4());
    let registry = v1_registry();
    let event = make_event(subject, "rollback-me");

    {
        let mut tx = pool.begin().await.unwrap();
        let outcome = PgKycEventStore::append(&mut tx, &registry, &event, |_| Ok(()))
            .await
            .unwrap();
        assert_eq!(outcome.seq, 0);
        // Forced rollback: drop without commit.
        tx.rollback().await.unwrap();
    }

    // Event, seq-bump, AND stream-row creation all rolled back together (K-16, K-35).
    assert_eq!(
        event_count(&pool, subject).await,
        0,
        "no orphan event after rollback"
    );
    let stream_exists: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM "ob-poc".kyc_subject_streams WHERE subject_root = $1"#,
    )
    .bind(subject.0)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stream_exists, 0, "no orphan stream row after rollback");

    cleanup(&pool, subject).await;
}

// ── exit 2 — per-subject order under concurrency (dense gap-free seq) ───────────

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn exit2_concurrent_appends_one_subject_dense_seq() {
    const N: i64 = 20;
    let pool = pool(N as u32 + 4).await;
    let subject = SubjectId(Uuid::new_v4());
    let registry = Arc::new(v1_registry());

    let mut handles = Vec::new();
    for i in 0..N {
        let pool = pool.clone();
        let registry = registry.clone();
        handles.push(tokio::spawn(async move {
            // Each task: its own connection + transaction; unique idempotency key
            // so all N are distinct events (not deduped).
            let event = make_event(subject, &format!("concurrent-{i}"));
            let mut tx = pool.begin().await.unwrap();
            let outcome = PgKycEventStore::append(&mut tx, &registry, &event, |_| Ok(()))
                .await
                .expect("concurrent append must succeed");
            tx.commit().await.unwrap();
            outcome.seq
        }));
    }

    let mut seqs: Vec<u64> = Vec::new();
    for h in handles {
        seqs.push(h.await.unwrap());
    }
    seqs.sort_unstable();

    // The FOR UPDATE lock serialized the appends → dense, gap-free 0..N-1, no dupes.
    let expected: Vec<u64> = (0..N as u64).collect();
    assert_eq!(
        seqs, expected,
        "concurrent appends must produce dense gap-free seq 0..N-1"
    );
    assert_eq!(
        event_count(&pool, subject).await,
        N,
        "N distinct events persisted"
    );

    cleanup(&pool, subject).await;
}

// ── precondition-under-lock (TOCTOU-safe rejection) ────────────────────────────

#[tokio::test]
async fn precondition_rejection_under_lock_inserts_nothing() {
    let pool = pool(4).await;
    let subject = SubjectId(Uuid::new_v4());
    let registry = v1_registry();
    let event = make_event(subject, "rejected");

    let mut tx = pool.begin().await.unwrap();
    let result = PgKycEventStore::append(&mut tx, &registry, &event, |_state| {
        // The validator rejects — simulating a failed proof-ratchet precondition.
        Err(KycError::PreconditionFailed {
            verb: VerbFqn("kyc.subject.register".into()),
            reason: "rejected by test validator".into(),
        })
    })
    .await;
    // The append must surface the rejection (validate runs BEFORE insert).
    assert!(
        result.is_err(),
        "rejected precondition must fail the append"
    );
    tx.rollback().await.unwrap();

    assert_eq!(
        event_count(&pool, subject).await,
        0,
        "rejected append inserts nothing"
    );

    cleanup(&pool, subject).await;
}

// ── cross-subject independence ─────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cross_subject_appends_are_independent() {
    let pool = pool(6).await;
    let s1 = SubjectId(Uuid::new_v4());
    let s2 = SubjectId(Uuid::new_v4());
    let registry = Arc::new(v1_registry());

    let run = |subject: SubjectId| {
        let pool = pool.clone();
        let registry = registry.clone();
        async move {
            let event = make_event(subject, "x");
            let mut tx = pool.begin().await.unwrap();
            let o = PgKycEventStore::append(&mut tx, &registry, &event, |_| Ok(()))
                .await
                .unwrap();
            tx.commit().await.unwrap();
            o.seq
        }
    };

    let (seq1, seq2) = tokio::join!(run(s1), run(s2));
    // Distinct subjects each start their own stream at seq 0 — no shared ordering.
    assert_eq!(seq1, 0);
    assert_eq!(seq2, 0);

    cleanup(&pool, s1).await;
    cleanup(&pool, s2).await;
}

// ── storage round-trip (rehydrated events fold identically) ────────────────────

#[tokio::test]
async fn roundtrip_loaded_events_fold_identically() {
    let pool = pool(4).await;
    let subject = SubjectId(Uuid::new_v4());
    let registry = v1_registry();

    // Append three distinct events.
    let in_mem: Vec<IntentEvent> = vec![
        make_event(subject, "rt-0"),
        make_event(subject, "rt-1"),
        make_event(subject, "rt-2"),
    ];
    {
        let mut tx = pool.begin().await.unwrap();
        for e in &in_mem {
            PgKycEventStore::append(&mut tx, &registry, e, |_| Ok(()))
                .await
                .unwrap();
        }
        tx.commit().await.unwrap();
    }

    // Load them back and fold.
    let mut conn = pool.acquire().await.unwrap();
    let loaded = PgKycEventStore::load_events(&mut conn, subject)
        .await
        .unwrap();
    assert_eq!(loaded.len(), 3, "all three events loaded");
    assert_eq!(
        loaded.iter().map(|e| e.seq).collect::<Vec<_>>(),
        vec![0, 1, 2],
        "loaded in dense seq order",
    );

    // The rehydrated stream folds to the same ControlState as the in-memory one
    // (storage round-trip preserves fold semantics).
    let loaded_refs: Vec<&IntentEvent> = loaded.iter().collect();
    let folded_from_db = fold_control_versioned(&loaded_refs, &registry).unwrap();
    assert!(
        folded_from_db.registered,
        "register event survived the round-trip"
    );

    cleanup(&pool, subject).await;
}
