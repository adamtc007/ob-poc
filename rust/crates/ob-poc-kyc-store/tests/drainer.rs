//! Outbox-incremental projection drainer tests (EOP-DD-KYCUBO-002 §5).
//!
//! Proves: `append` enqueues a projection effect; the drainer claims it and
//! rebuilds the projection; a deduped append enqueues nothing; concurrent
//! drainers process each row once (FOR UPDATE SKIP LOCKED).
//!
//! ONE test function, sequential phases: the drainer claims GLOBALLY by
//! effect-kind (its real behaviour), so two parallel drainer tests would drain
//! each other's rows. A single function keeps them serial; all assertions are
//! subject-scoped, so other test binaries' rows cannot perturb them.

use std::sync::Arc;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

use ob_poc_kyc_store::{PgKycEventStore, PgKycProjectionDrainer, CONTROL_EDGE_PROJECTION_EFFECT};
use ob_poc_kyc_substrate::{
    phase1_lexicon, AuthorityRef, EdgeId, FoldRegistry, IdemKey, IntentEvent, Principal, SubjectId,
    TargetBinding, V1FoldImpl,
};

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string())
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

fn register(subject: SubjectId, idem: &str) -> IntentEvent {
    base(subject, "kyc.subject.register", TargetBinding::for_subject(subject),
         serde_json::json!({ "is_natural_person": false }), idem)
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

async fn append(pool: &PgPool, registry: &FoldRegistry, ev: &IntentEvent) {
    let mut tx = pool.begin().await.unwrap();
    PgKycEventStore::append(&mut tx, registry, ev, |_| Ok(())).await.unwrap();
    tx.commit().await.unwrap();
}

async fn outbox_count(pool: &PgPool, subject: SubjectId, status: &str) -> i64 {
    sqlx::query_scalar(
        r#"SELECT count(*) FROM "public".outbox
           WHERE effect_kind = $1 AND idempotency_key LIKE $2 AND status = $3"#,
    )
    .bind(CONTROL_EDGE_PROJECTION_EFFECT)
    .bind(format!("{}:%", subject.0))
    .bind(status)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn projection_count(pool: &PgPool, subject: SubjectId) -> i64 {
    sqlx::query_scalar(r#"SELECT count(*) FROM "ob-poc".kyc_control_edge_projection WHERE subject_root = $1"#)
        .bind(subject.0)
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
    let _ = sqlx::query(r#"DELETE FROM "public".outbox WHERE effect_kind = $1 AND idempotency_key LIKE $2"#)
        .bind(CONTROL_EDGE_PROJECTION_EFFECT)
        .bind(format!("{}:%", subject.0))
        .execute(pool)
        .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn drainer_enqueue_dedupe_and_concurrency() {
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .connect(&database_url())
        .await
        .expect("connect to test DB");
    let registry = Arc::new(v1_registry());

    // ── Phase 1: append enqueues an effect; the drainer projects it. ──
    {
        let subject = SubjectId(Uuid::new_v4());
        let edge = Uuid::new_v4();
        append(&pool, &registry, &register(subject, "reg")).await;
        append(&pool, &registry, &assert_control(subject, edge, "edge")).await;

        assert_eq!(outbox_count(&pool, subject, "pending").await, 2, "two pending effects enqueued");
        assert_eq!(projection_count(&pool, subject).await, 0, "drainer has not run yet");

        PgKycProjectionDrainer::drain_all(&pool, &registry, 1000).await.unwrap();

        assert_eq!(projection_count(&pool, subject).await, 1, "edge projected by the drainer");
        assert_eq!(outbox_count(&pool, subject, "done").await, 2, "both effects marked done");
        assert_eq!(outbox_count(&pool, subject, "pending").await, 0);
        cleanup(&pool, subject).await;
    }

    // ── Phase 2: a deduped append enqueues nothing. ──
    {
        let subject = SubjectId(Uuid::new_v4());
        let ev = register(subject, "same");
        append(&pool, &registry, &ev).await;
        append(&pool, &registry, &ev).await; // same idempotency_key → deduped at the store
        assert_eq!(outbox_count(&pool, subject, "pending").await, 1, "deduped append enqueues nothing");
        cleanup(&pool, subject).await;
    }

    // ── Phase 3: concurrent drainers, disjoint subjects, each effect done once. ──
    {
        let subjects: Vec<SubjectId> = (0..6).map(|_| SubjectId(Uuid::new_v4())).collect();
        for s in &subjects {
            append(&pool, &registry, &assert_control(*s, Uuid::new_v4(), "edge")).await;
        }

        let drain = || {
            let pool = pool.clone();
            let registry = registry.clone();
            async move { PgKycProjectionDrainer::drain_all(&pool, &registry, 1000).await.unwrap() }
        };
        let _ = tokio::join!(drain(), drain());

        // SKIP LOCKED + terminal 'done' → each subject's effect processed exactly once.
        for s in &subjects {
            assert_eq!(outbox_count(&pool, *s, "done").await, 1, "effect done exactly once");
            assert_eq!(outbox_count(&pool, *s, "pending").await, 0);
            assert_eq!(projection_count(&pool, *s).await, 1, "subject's edge projected");
        }
        for s in &subjects {
            cleanup(&pool, *s).await;
        }
    }
}
