//! Integration coverage for Phase 5e-narration-cutover.
//!
//! Verifies that:
//! 1. `emit_narration_outbox` writes a row with effect_kind=narrate.
//! 2. The drainer (registered with `NarrateConsumer`) claims the row
//!    and marks it `done`.
//! 3. Re-emission with the same trace_id is deduped by the
//!    `(idempotency_key, effect_kind)` UNIQUE constraint.
//!
//! Run with:
//! DATABASE_URL="postgresql:///data_designer" \
//!   cargo test --features database --test narration_outbox_integration -- --ignored --nocapture

#[cfg(feature = "database")]
mod integration {
    use std::sync::Arc;
    use std::time::Duration;

    use anyhow::Result;
    use ob_poc::outbox::{
        narration_emit, NarrateConsumer, OutboxDrainerConfig, OutboxDrainerImpl,
    };
    use ob_poc_types::TraceId;
    use serde_json::json;
    use sqlx::PgPool;
    use uuid::Uuid;

    async fn pool() -> Result<PgPool> {
        let url = std::env::var("TEST_DATABASE_URL")
            .or_else(|_| std::env::var("DATABASE_URL"))
            .unwrap_or_else(|_| "postgresql:///data_designer".into());
        Ok(PgPool::connect(&url).await?)
    }

    fn fast_config() -> OutboxDrainerConfig {
        OutboxDrainerConfig {
            poll_interval: Duration::from_millis(50),
            claim_batch_size: 8,
            claim_timeout: Duration::from_secs(60),
            max_attempts: 3,
            worker_id: format!("test-narrate-{}", Uuid::new_v4()),
        }
    }

    async fn fetch_status_by_idempotency_key(
        pool: &PgPool,
        idempotency_key: &str,
    ) -> Result<Option<(Uuid, String, i32)>> {
        let row = sqlx::query_as::<_, (Uuid, String, i32)>(
            r#"SELECT id, status, attempts
               FROM public.outbox
               WHERE idempotency_key = $1 AND effect_kind = 'narrate'"#,
        )
        .bind(idempotency_key)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

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
    async fn emit_writes_narrate_row_drainer_marks_done() -> Result<()> {
        let pool = pool().await?;
        let session_id = Uuid::new_v4();
        let trace_id = TraceId::new();
        let narration = json!({
            "progress": "1 of 5 slots filled",
            "verbosity": "summary",
        });

        let outbox_id = narration_emit::emit_narration_outbox(
            &pool,
            session_id,
            trace_id,
            Some("cbu"),
            &narration,
        )
        .await?;

        let idempotency_key = format!("narrate:{}:session:{}", trace_id.0, session_id);

        let mut drainer = OutboxDrainerImpl::new(pool.clone(), fast_config());
        drainer.register(Arc::new(NarrateConsumer::new()))?;
        let handle = drainer.spawn();

        let pool_clone = pool.clone();
        let key_clone = idempotency_key.clone();
        let observed = wait_for(Duration::from_secs(5), || async {
            matches!(
                fetch_status_by_idempotency_key(&pool_clone, &key_clone).await,
                Ok(Some((_, s, _))) if s == "done"
            )
        })
        .await;
        assert!(observed, "narrate row not marked done within 5s");

        let row = fetch_status_by_idempotency_key(&pool, &idempotency_key)
            .await?
            .expect("row exists");
        assert_eq!(row.0, outbox_id, "idempotency key resolves to the inserted row");
        assert_eq!(row.1, "done");
        assert_eq!(row.2, 1, "exactly one claim attempt");

        sqlx::query("DELETE FROM public.outbox WHERE id = $1")
            .bind(outbox_id)
            .execute(&pool)
            .await?;
        handle.shutdown().await;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore]
    async fn emit_is_idempotent_on_duplicate_trace_session_pair() -> Result<()> {
        let pool = pool().await?;
        let session_id = Uuid::new_v4();
        let trace_id = TraceId::new();
        let narration = json!({"progress": "test"});

        let id_first = narration_emit::emit_narration_outbox(
            &pool,
            session_id,
            trace_id,
            Some("cbu"),
            &narration,
        )
        .await?;

        // Second emission with the same (trace_id, session_id) should
        // be deduped by ON CONFLICT DO NOTHING.
        let id_second = narration_emit::emit_narration_outbox(
            &pool,
            session_id,
            trace_id,
            Some("cbu"),
            &json!({"progress": "different content"}),
        )
        .await?;

        // Both calls return their own freshly-generated outbox_id —
        // the dedupe happens at INSERT time, so id_second was never
        // actually persisted. Verify by counting rows.
        let count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM public.outbox
               WHERE effect_kind = 'narrate'
                 AND idempotency_key = $1"#,
        )
        .bind(format!("narrate:{}:session:{}", trace_id.0, session_id))
        .fetch_one(&pool)
        .await?;
        assert_eq!(count, 1, "second emission must be deduped");

        // The persisted row must be the FIRST emission's id.
        let actual_id: Uuid = sqlx::query_scalar(
            r#"SELECT id FROM public.outbox
               WHERE effect_kind = 'narrate'
                 AND idempotency_key = $1"#,
        )
        .bind(format!("narrate:{}:session:{}", trace_id.0, session_id))
        .fetch_one(&pool)
        .await?;
        assert_eq!(actual_id, id_first, "first emission wins");
        assert_ne!(actual_id, id_second, "second emission was dropped");

        sqlx::query("DELETE FROM public.outbox WHERE id = $1")
            .bind(id_first)
            .execute(&pool)
            .await?;
        Ok(())
    }
}
