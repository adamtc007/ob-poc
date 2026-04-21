//! Integration coverage for the Phase 5e outbox drainer.
//!
//! Run with:
//! DATABASE_URL="postgresql:///data_designer" \
//!   cargo test --features database --test outbox_drainer_integration -- --ignored --nocapture

#[cfg(feature = "database")]
mod integration {
    use std::sync::Arc;
    use std::time::Duration;

    use anyhow::Result;
    use async_trait::async_trait;
    use ob_poc::outbox::{
        AsyncOutboxConsumer, OutboxDrainerConfig, OutboxDrainerImpl,
    };
    use ob_poc_types::{
        ClaimedOutboxRow, OutboxEffectKind, OutboxProcessOutcome,
    };
    use serde_json::json;
    use sqlx::PgPool;
    use tokio::sync::Mutex;
    use uuid::Uuid;

    /// In-test consumer that records every claim and returns a configured
    /// outcome. One instance per test so calls don't bleed across tests.
    struct RecordingConsumer {
        kind: OutboxEffectKind,
        label: &'static str,
        outcome: OutboxProcessOutcome,
        observed: Mutex<Vec<Uuid>>,
    }

    impl RecordingConsumer {
        fn new(
            kind: OutboxEffectKind,
            label: &'static str,
            outcome: OutboxProcessOutcome,
        ) -> Self {
            Self {
                kind,
                label,
                outcome,
                observed: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl AsyncOutboxConsumer for RecordingConsumer {
        fn effect_kind(&self) -> OutboxEffectKind {
            self.kind
        }
        fn label(&self) -> &str {
            self.label
        }
        async fn process(&self, row: ClaimedOutboxRow) -> OutboxProcessOutcome {
            self.observed.lock().await.push(row.id);
            self.outcome.clone()
        }
    }

    async fn pool() -> Result<PgPool> {
        let url = std::env::var("TEST_DATABASE_URL")
            .or_else(|_| std::env::var("DATABASE_URL"))
            .unwrap_or_else(|_| "postgresql:///data_designer".into());
        Ok(PgPool::connect(&url).await?)
    }

    /// Insert one outbox row with the given effect_kind. Returns the row id.
    async fn enqueue(
        pool: &PgPool,
        effect_kind: &str,
        idempotency_key: &str,
        payload: serde_json::Value,
    ) -> Result<Uuid> {
        let id = Uuid::new_v4();
        let trace_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO public.outbox
                (id, trace_id, envelope_version, effect_kind, payload,
                 idempotency_key, status)
            VALUES ($1, $2, 1, $3, $4, $5, 'pending')
            "#,
        )
        .bind(id)
        .bind(trace_id)
        .bind(effect_kind)
        .bind(&payload)
        .bind(idempotency_key)
        .execute(pool)
        .await?;
        Ok(id)
    }

    async fn fetch_status(pool: &PgPool, id: Uuid) -> Result<(String, i32, Option<String>)> {
        let row = sqlx::query_as::<_, (String, i32, Option<String>)>(
            r#"SELECT status, attempts, last_error
               FROM public.outbox WHERE id = $1"#,
        )
        .bind(id)
        .fetch_one(pool)
        .await?;
        Ok(row)
    }

    fn fast_config() -> OutboxDrainerConfig {
        OutboxDrainerConfig {
            poll_interval: Duration::from_millis(50),
            claim_batch_size: 8,
            claim_timeout: Duration::from_secs(60),
            max_attempts: 3,
            worker_id: format!("test-{}", Uuid::new_v4()),
        }
    }

    /// Wait up to `max` for `predicate` to return Ok with a true value.
    async fn wait_for<F, Fut>(max: Duration, mut predicate: F) -> bool
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = bool>,
    {
        let deadline = std::time::Instant::now() + max;
        while std::time::Instant::now() < deadline {
            if predicate().await {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        false
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore]
    async fn drainer_processes_pending_row_to_done() -> Result<()> {
        let pool = pool().await?;
        // Dedicated effect_kind per test so concurrent tests don't claim
        // each other's rows. Each drainer only registers its own kind.
        let id = enqueue(
            &pool,
            "ui_push",
            &format!("test-done:{}", Uuid::new_v4()),
            json!({"frame": "test"}),
        )
        .await?;

        let consumer = Arc::new(RecordingConsumer::new(
            OutboxEffectKind::UiPush,
            "test-done",
            OutboxProcessOutcome::Done,
        ));
        let mut drainer = OutboxDrainerImpl::new(pool.clone(), fast_config());
        drainer.register(consumer.clone())?;
        let handle = drainer.spawn();

        let pool_clone = pool.clone();
        let observed = wait_for(Duration::from_secs(5), || async {
            matches!(
                fetch_status(&pool_clone, id).await,
                Ok((s, _, _)) if s == "done"
            )
        })
        .await;
        assert!(observed, "row not marked done within 5s");

        let calls = consumer.observed.lock().await.clone();
        assert_eq!(calls, vec![id], "consumer saw exactly one call for the row");

        let (status, attempts, _) = fetch_status(&pool, id).await?;
        assert_eq!(status, "done");
        assert_eq!(attempts, 1, "exactly one claim attempt");

        // Cleanup
        sqlx::query("DELETE FROM public.outbox WHERE id = $1")
            .bind(id)
            .execute(&pool)
            .await?;
        handle.shutdown().await;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore]
    async fn drainer_retries_then_terminals_after_max_attempts() -> Result<()> {
        let pool = pool().await?;
        // Dedicated effect_kind so the parallel "done" test's drainer
        // doesn't claim this row.
        let id = enqueue(
            &pool,
            "constellation_broadcast",
            &format!("test-retry:{}", Uuid::new_v4()),
            json!({}),
        )
        .await?;

        let consumer = Arc::new(RecordingConsumer::new(
            OutboxEffectKind::ConstellationBroadcast,
            "test-retry",
            OutboxProcessOutcome::Retryable {
                reason: "always-fail".into(),
            },
        ));
        let mut drainer = OutboxDrainerImpl::new(pool.clone(), fast_config());
        drainer.register(consumer.clone())?;
        let handle = drainer.spawn();

        let pool_clone = pool.clone();
        let terminal = wait_for(Duration::from_secs(5), || async {
            matches!(
                fetch_status(&pool_clone, id).await,
                Ok((s, _, _)) if s == "failed_terminal"
            )
        })
        .await;
        assert!(terminal, "row not marked failed_terminal within 5s");

        let (status, attempts, last_error) = fetch_status(&pool, id).await?;
        assert_eq!(status, "failed_terminal");
        assert_eq!(attempts, 3, "expected exactly max_attempts=3 attempts");
        assert!(
            last_error
                .as_deref()
                .unwrap_or("")
                .contains("max_attempts"),
            "last_error should mention max_attempts: {:?}",
            last_error
        );

        sqlx::query("DELETE FROM public.outbox WHERE id = $1")
            .bind(id)
            .execute(&pool)
            .await?;
        handle.shutdown().await;
        Ok(())
    }

    /// Replay-safety gate (spec §10.4): "drainer replay-safe test
    /// passes — kill mid-stream, restart, all effects delivered
    /// exactly-semantically-once via idempotency key".
    ///
    /// Models a worker crash by inserting a row in `processing` with
    /// `claimed_at` already older than `claim_timeout`, then starts
    /// a fresh drainer instance. The drainer's stale-claim recycler
    /// MUST reset the row to `pending` and the new drainer MUST
    /// re-process it cleanly. Combined with the
    /// `(idempotency_key, effect_kind)` UNIQUE constraint, this
    /// gives at-least-once delivery + once-observable side effect.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore]
    async fn drainer_recovers_stale_claim_after_simulated_worker_crash() -> Result<()> {
        let pool = pool().await?;
        let idempotency_key = format!("test-replay:{}", Uuid::new_v4());

        // Step 1 — worker A claims a row, then crashes mid-process
        // (simulated by directly inserting in `processing` state with
        // a claimed_at timestamp older than the recycler's timeout).
        let id = Uuid::new_v4();
        let trace_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO public.outbox
                (id, trace_id, envelope_version, effect_kind, payload,
                 idempotency_key, status, claimed_by, claimed_at, attempts)
            VALUES
                ($1, $2, 1, 'narrate', '{}'::jsonb, $3,
                 'processing', 'crashed-worker-A',
                 NOW() - INTERVAL '10 minutes', 1)
            "#,
        )
        .bind(id)
        .bind(trace_id)
        .bind(&idempotency_key)
        .execute(&pool)
        .await?;

        // Step 2 — confirm a duplicate insert with the same
        // idempotency key is silently rejected (idempotency
        // contract).
        let dup_result = sqlx::query(
            r#"
            INSERT INTO public.outbox
                (id, trace_id, envelope_version, effect_kind, payload,
                 idempotency_key, status)
            VALUES
                ($1, $2, 1, 'narrate', '{"second":true}'::jsonb,
                 $3, 'pending')
            ON CONFLICT (idempotency_key, effect_kind) DO NOTHING
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(Uuid::new_v4())
        .bind(&idempotency_key)
        .execute(&pool)
        .await?;
        assert_eq!(
            dup_result.rows_affected(),
            0,
            "duplicate insert must dedupe at the (idempotency_key, effect_kind) UNIQUE"
        );

        // Step 3 — worker B starts. Its recycler resets the stale
        // claim → pending; its claim cycle re-claims; consumer marks
        // done.
        let consumer = Arc::new(RecordingConsumer::new(
            OutboxEffectKind::Narrate,
            "test-replay-worker-B",
            OutboxProcessOutcome::Done,
        ));
        let mut config = fast_config();
        // Keep claim_timeout below the inserted -10min stamp so the
        // recycler picks the row up immediately on the first cycle.
        config.claim_timeout = Duration::from_secs(60);
        let mut drainer = OutboxDrainerImpl::new(pool.clone(), config);
        drainer.register(consumer.clone())?;
        let handle = drainer.spawn();

        let pool_clone = pool.clone();
        let observed = wait_for(Duration::from_secs(5), || async {
            matches!(
                fetch_status(&pool_clone, id).await,
                Ok((s, _, _)) if s == "done"
            )
        })
        .await;
        assert!(observed, "row not reprocessed within 5s after stale-claim recovery");

        // Worker B's consumer must have seen the row exactly once;
        // the stale-claim recycle path adds no extra invocations.
        let calls = consumer.observed.lock().await.clone();
        assert_eq!(
            calls,
            vec![id],
            "consumer should observe exactly one re-process call"
        );

        // attempts column reflects at-least-once: worker A's pre-
        // crash claim left attempts=1; worker B's re-claim makes 2.
        let (status, attempts, _) = fetch_status(&pool, id).await?;
        assert_eq!(status, "done");
        assert_eq!(
            attempts, 2,
            "attempts records the at-least-once delivery surface; observable side effect remains once via idempotency_key"
        );

        sqlx::query("DELETE FROM public.outbox WHERE id = $1")
            .bind(id)
            .execute(&pool)
            .await?;
        handle.shutdown().await;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore]
    async fn drainer_skips_rows_without_a_registered_consumer() -> Result<()> {
        // Pre-filter behavior: the drainer's claim query restricts to
        // effect_kinds matching its registered consumers. A row whose
        // effect_kind has no consumer in this drainer is left untouched
        // (status='pending') for another drainer instance to handle.
        let pool = pool().await?;
        // Use MaintenanceSpawn so the row's effect_kind is unique to
        // this test (Narrate is used by the parallel replay-safety
        // test; ExternalNotify is the foreign-consumer kind below).
        let id = enqueue(
            &pool,
            "maintenance_spawn",
            &format!("test-no-consumer:{}", Uuid::new_v4()),
            json!({}),
        )
        .await?;

        // Drainer with a consumer for a DIFFERENT kind so the drainer
        // doesn't short-circuit on the empty-consumers fast path.
        let foreign_consumer = Arc::new(RecordingConsumer::new(
            OutboxEffectKind::ExternalNotify,
            "foreign-consumer",
            OutboxProcessOutcome::Done,
        ));
        let mut drainer = OutboxDrainerImpl::new(pool.clone(), fast_config());
        drainer.register(foreign_consumer)?;
        let handle = drainer.spawn();

        // Give the drainer a few cycles. The ui_push row should remain
        // pending the whole time.
        tokio::time::sleep(Duration::from_millis(500)).await;

        let (status, attempts, _) = fetch_status(&pool, id).await?;
        assert_eq!(
            status, "pending",
            "row should be untouched by a drainer that has no consumer for its effect_kind"
        );
        assert_eq!(attempts, 0, "row should not have been claimed");

        sqlx::query("DELETE FROM public.outbox WHERE id = $1")
            .bind(id)
            .execute(&pool)
            .await?;
        handle.shutdown().await;
        Ok(())
    }
}
