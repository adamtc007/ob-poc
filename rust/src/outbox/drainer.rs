//! Concrete outbox drainer implementation.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use ob_poc_types::{
    ClaimedOutboxRow, EnvelopeVersion, IdempotencyKey, OutboxEffectKind, OutboxProcessOutcome,
    TraceId,
};
use serde_json::Value;
use sqlx::PgPool;
use tokio::sync::Notify;
use tokio::task::JoinHandle;
use uuid::Uuid;

use super::consumer::AsyncOutboxConsumer;

/// Tunables for the drainer's polling loop.
#[derive(Debug, Clone)]
pub struct OutboxDrainerConfig {
    /// Sleep duration between poll cycles when there is nothing to do.
    pub poll_interval: Duration,
    /// Maximum rows to claim per cycle. Larger batches amortise the
    /// claim query cost; smaller batches reduce per-row latency.
    pub claim_batch_size: u32,
    /// Worker timeout. Rows in `processing` with `claimed_at` older
    /// than this are considered abandoned and recycled to `pending`.
    pub claim_timeout: Duration,
    /// After this many `Retryable` outcomes the row is auto-promoted
    /// to `failed_terminal` and removed from the retry queue. Alerting
    /// is the operator's responsibility.
    pub max_attempts: u32,
    /// Identifier put in `claimed_by` so operators can attribute
    /// row claims to a specific worker process.
    pub worker_id: String,
}

impl Default for OutboxDrainerConfig {
    fn default() -> Self {
        let pid = std::process::id();
        let host = std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".into());
        Self {
            poll_interval: Duration::from_millis(500),
            claim_batch_size: 16,
            claim_timeout: Duration::from_secs(120),
            max_attempts: 5,
            worker_id: format!("ob-poc:{host}:{pid}"),
        }
    }
}

/// Builder + runtime handle for the outbox drainer task.
///
/// Construct with `new`, register consumers via `register`, then
/// `spawn` to start the polling loop. The returned
/// [`OutboxDrainerHandle`] cancels the loop on `shutdown`.
pub struct OutboxDrainerImpl {
    pool: PgPool,
    config: OutboxDrainerConfig,
    consumers: HashMap<OutboxEffectKind, Arc<dyn AsyncOutboxConsumer>>,
}

impl OutboxDrainerImpl {
    pub fn new(pool: PgPool, config: OutboxDrainerConfig) -> Self {
        Self {
            pool,
            config,
            consumers: HashMap::new(),
        }
    }

    /// Register one consumer. At most one per effect kind — second
    /// registration of the same kind returns an error.
    pub fn register(&mut self, consumer: Arc<dyn AsyncOutboxConsumer>) -> Result<()> {
        let kind = consumer.effect_kind();
        if self.consumers.contains_key(&kind) {
            return Err(anyhow!(
                "outbox drainer: consumer for {:?} already registered",
                kind
            ));
        }
        tracing::info!(
            effect_kind = ?kind,
            label = consumer.label(),
            "outbox drainer: registered consumer"
        );
        self.consumers.insert(kind, consumer);
        Ok(())
    }

    /// Spawn the drainer as a background tokio task. Returns a
    /// handle the caller stores for graceful shutdown at process
    /// exit.
    pub fn spawn(self) -> OutboxDrainerHandle {
        let cancel = Arc::new(Notify::new());
        let cancel_signal = cancel.clone();
        let task = tokio::spawn(async move {
            let drainer = self;
            drainer.run(cancel_signal).await;
        });
        OutboxDrainerHandle { cancel, task }
    }

    /// Polling loop. Runs until `cancel` is notified.
    async fn run(self, cancel: Arc<Notify>) {
        tracing::info!(
            worker_id = %self.config.worker_id,
            poll_interval_ms = self.config.poll_interval.as_millis() as u64,
            claim_batch_size = self.config.claim_batch_size,
            "outbox drainer: starting"
        );
        loop {
            // Recycle stale claims first so any in-flight work that
            // crashed becomes visible again before we try to claim
            // new rows.
            if let Err(e) = self.recycle_stale_claims().await {
                tracing::warn!(error = %e, "outbox drainer: recycle pass failed");
            }

            let claimed = match self.claim_batch().await {
                Ok(rows) => rows,
                Err(e) => {
                    tracing::warn!(error = %e, "outbox drainer: claim failed");
                    Vec::new()
                }
            };

            let cycle_count = claimed.len();
            let mut cycle_done = 0usize;
            let mut cycle_retryable = 0usize;
            let mut cycle_terminal = 0usize;
            for row in claimed {
                let id = row.id;
                self.process_row(row).await;
                // Re-read the row's terminal status to bucket. The
                // drainer's status-update path is already idempotent;
                // this read just classifies for the per-cycle Stage
                // 9b rollup below.
                if let Ok(row) = sqlx::query_scalar::<_, String>(
                    "SELECT status FROM public.outbox WHERE id = $1",
                )
                .bind(id)
                .fetch_one(&self.pool)
                .await
                {
                    match row.as_str() {
                        "done" => cycle_done += 1,
                        "failed_retryable" => cycle_retryable += 1,
                        "failed_terminal" => cycle_terminal += 1,
                        _ => {}
                    }
                }
            }

            // Stage 9b — post-commit drain (Phase 5b-deep typed
            // contract). Per-cycle rollup; per-trace aggregation
            // arrives once dispatch-side Stage 9a tags rows with the
            // trace anchor.
            if cycle_count > 0 {
                let stage_9b = crate::sequencer_stages::PostCommitDrainOutput::from_counters(
                    cycle_done,
                    cycle_retryable,
                    cycle_terminal,
                );
                tracing::debug!(
                    rows_done = stage_9b.rows_done,
                    rows_retryable = stage_9b.rows_retryable,
                    rows_terminal = stage_9b.rows_terminal,
                    fully_drained = stage_9b.fully_drained(),
                    "Stage 9b — drainer cycle summary"
                );
            }

            // Wait poll_interval OR until shutdown is requested.
            tokio::select! {
                _ = tokio::time::sleep(self.config.poll_interval) => {}
                _ = cancel.notified() => {
                    tracing::info!("outbox drainer: shutdown requested, exiting loop");
                    break;
                }
            }
        }
    }

    async fn recycle_stale_claims(&self) -> Result<()> {
        let timeout_secs = self.config.claim_timeout.as_secs() as i64;
        let recycled = sqlx::query!(
            r#"
            UPDATE public.outbox
            SET status = 'pending',
                claimed_by = NULL,
                claimed_at = NULL
            WHERE status = 'processing'
              AND claimed_at < NOW() - make_interval(secs => $1::double precision)
            RETURNING id
            "#,
            timeout_secs as f64,
        )
        .fetch_all(&self.pool)
        .await?;
        if !recycled.is_empty() {
            tracing::warn!(
                count = recycled.len(),
                "outbox drainer: recycled stale processing rows"
            );
        }
        Ok(())
    }

    async fn claim_batch(&self) -> Result<Vec<ClaimedOutboxRow>> {
        if self.consumers.is_empty() {
            // No consumers registered → nothing this drainer can process.
            // Skip the claim cycle entirely.
            return Ok(Vec::new());
        }
        let batch_size = self.config.claim_batch_size as i64;
        // Only claim rows whose effect_kind we have a consumer for. This
        // prevents a drainer from claiming a row it would just have to
        // mark terminal, and lets multiple drainers safely coexist (e.g.
        // sharded by effect-kind in a future Phase 5e+ extension).
        let registered: Vec<String> = self
            .consumers
            .keys()
            .map(|k| {
                serde_json::to_value(k)
                    .ok()
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_default()
            })
            .collect();
        let rows = sqlx::query!(
            r#"
            UPDATE public.outbox
            SET status = 'processing',
                claimed_by = $1,
                claimed_at = NOW(),
                attempts = attempts + 1
            WHERE id IN (
                SELECT id FROM public.outbox
                WHERE status IN ('pending', 'failed_retryable')
                  AND effect_kind = ANY($3::text[])
                ORDER BY created_at
                FOR UPDATE SKIP LOCKED
                LIMIT $2
            )
            RETURNING id, trace_id, envelope_version, effect_kind, payload, idempotency_key, attempts
            "#,
            self.config.worker_id,
            batch_size,
            &registered as &[String],
        )
        .fetch_all(&self.pool)
        .await?;

        let mut claimed = Vec::with_capacity(rows.len());
        for row in rows {
            let kind = match parse_effect_kind(&row.effect_kind) {
                Ok(k) => k,
                Err(e) => {
                    tracing::error!(
                        id = %row.id,
                        effect_kind = %row.effect_kind,
                        error = %e,
                        "outbox drainer: unknown effect_kind, marking terminal"
                    );
                    self.mark_terminal(row.id, &format!("unknown effect_kind: {e}"))
                        .await
                        .ok();
                    continue;
                }
            };
            let attempts: u32 = row.attempts.try_into().unwrap_or(u32::MAX);
            claimed.push(ClaimedOutboxRow {
                id: row.id,
                trace_id: TraceId(row.trace_id),
                envelope_version: EnvelopeVersion(row.envelope_version as u16),
                effect_kind: kind,
                payload: row.payload,
                idempotency_key: IdempotencyKey(row.idempotency_key),
                attempts,
            });
        }
        Ok(claimed)
    }

    async fn process_row(&self, row: ClaimedOutboxRow) {
        let id = row.id;
        let kind = row.effect_kind;
        let attempts = row.attempts;
        let consumer = match self.consumers.get(&kind) {
            Some(c) => c.clone(),
            None => {
                tracing::error!(
                    id = %id,
                    effect_kind = ?kind,
                    "outbox drainer: no consumer registered, marking terminal"
                );
                self.mark_terminal(id, "no consumer registered for effect_kind")
                    .await
                    .ok();
                return;
            }
        };

        let label = consumer.label().to_string();
        tracing::debug!(
            id = %id,
            effect_kind = ?kind,
            consumer = %label,
            attempts,
            "outbox drainer: dispatching to consumer"
        );

        // Catch panics so a buggy consumer can't bring down the loop.
        let outcome = tokio::task::spawn(async move { consumer.process(row).await })
            .await
            .unwrap_or_else(|join_err| {
                tracing::error!(
                    id = %id,
                    error = %join_err,
                    "outbox drainer: consumer panicked"
                );
                OutboxProcessOutcome::Terminal {
                    reason: format!("consumer panicked: {join_err}"),
                }
            });

        let result = match outcome {
            OutboxProcessOutcome::Done | OutboxProcessOutcome::Deduped => {
                self.mark_done(id).await
            }
            OutboxProcessOutcome::Retryable { reason } => {
                if attempts >= self.config.max_attempts {
                    tracing::error!(
                        id = %id,
                        attempts,
                        max = self.config.max_attempts,
                        reason = %reason,
                        "outbox drainer: max attempts exceeded, promoting to terminal"
                    );
                    self.mark_terminal(
                        id,
                        &format!("max_attempts ({}) exceeded: {reason}", self.config.max_attempts),
                    )
                    .await
                } else {
                    tracing::info!(
                        id = %id,
                        attempts,
                        reason = %reason,
                        "outbox drainer: retryable, will retry"
                    );
                    self.mark_retryable(id, &reason).await
                }
            }
            OutboxProcessOutcome::Terminal { reason } => {
                tracing::error!(id = %id, reason = %reason, "outbox drainer: terminal failure");
                self.mark_terminal(id, &reason).await
            }
        };
        if let Err(e) = result {
            tracing::error!(id = %id, error = %e, "outbox drainer: status update failed");
        }
    }

    async fn mark_done(&self, id: Uuid) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE public.outbox
            SET status = 'done',
                processed_at = NOW(),
                last_error = NULL
            WHERE id = $1
            "#,
            id,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn mark_retryable(&self, id: Uuid, reason: &str) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE public.outbox
            SET status = 'failed_retryable',
                claimed_by = NULL,
                claimed_at = NULL,
                last_error = $2
            WHERE id = $1
            "#,
            id,
            reason,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn mark_terminal(&self, id: Uuid, reason: &str) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE public.outbox
            SET status = 'failed_terminal',
                processed_at = NOW(),
                last_error = $2
            WHERE id = $1
            "#,
            id,
            reason,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

fn parse_effect_kind(raw: &str) -> Result<OutboxEffectKind> {
    let value = Value::String(raw.to_string());
    serde_json::from_value::<OutboxEffectKind>(value)
        .map_err(|e| anyhow!("invalid effect_kind '{raw}': {e}"))
}

/// Handle returned by [`OutboxDrainerImpl::spawn`] for graceful
/// shutdown at process exit.
pub struct OutboxDrainerHandle {
    cancel: Arc<Notify>,
    task: JoinHandle<()>,
}

impl OutboxDrainerHandle {
    /// Signal the drainer loop to exit and wait for it to finish.
    pub async fn shutdown(self) {
        self.cancel.notify_waiters();
        if let Err(e) = self.task.await {
            tracing::warn!(error = %e, "outbox drainer task join failed");
        }
    }
}
