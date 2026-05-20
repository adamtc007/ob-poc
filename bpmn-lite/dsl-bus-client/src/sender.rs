//! Sender task — drains the outbox and dispatches payloads to peers.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use dsl_bus_protocol::v1::invocation_service_client::InvocationServiceClient;
use dsl_bus_protocol::v1::result_service_client::ResultServiceClient;
use dsl_bus_protocol::v1::{InvocationRequest, InvocationResult};
use dsl_bus_storage::{
    mark_outbox_retry, mark_outbox_submitted, select_pending_outbox, BusEndpoint, OutboxEntry,
};
use prost::Message;
use sqlx::PgPool;
use tokio::sync::watch;
use tracing::{debug, warn};

use crate::client::PeerRegistry;
use crate::uuid_convert::from_proto_opt;

/// Shape of the §8.5 sender loop.
pub(crate) struct SenderConfig {
    pub pool: PgPool,
    pub peers: Arc<PeerRegistry>,
    pub interval: Duration,
    pub batch_size: i64,
    pub max_backoff_secs: i64,
    pub stats: Arc<SenderStats>,
    pub shutdown: watch::Receiver<bool>,
}

/// Atomic counters covering the sender's behaviour. Cheap to read; the
/// public surface is the `snapshot()` reflection.
#[derive(Default)]
pub struct SenderStats {
    submitted: AtomicU64,
    retried: AtomicU64,
    rows_seen: AtomicU64,
}

impl SenderStats {
    pub fn submitted(&self) -> u64 {
        self.submitted.load(Ordering::Relaxed)
    }
    pub fn retried(&self) -> u64 {
        self.retried.load(Ordering::Relaxed)
    }
    pub fn rows_seen(&self) -> u64 {
        self.rows_seen.load(Ordering::Relaxed)
    }

    /// Cloneable snapshot — useful for assertions that expect a frozen
    /// view of the counters.
    pub fn snapshot(&self) -> Self {
        Self {
            submitted: AtomicU64::new(self.submitted()),
            retried: AtomicU64::new(self.retried()),
            rows_seen: AtomicU64::new(self.rows_seen()),
        }
    }
}

pub(crate) async fn run(mut cfg: SenderConfig) {
    loop {
        if *cfg.shutdown.borrow() {
            break;
        }
        match drain_once(&cfg).await {
            Ok(n) => {
                cfg.stats.rows_seen.fetch_add(n as u64, Ordering::Relaxed);
            }
            Err(err) => {
                warn!(error = %err, "sender sweep failed; continuing");
            }
        }

        tokio::select! {
            _ = tokio::time::sleep(cfg.interval) => {}
            _ = cfg.shutdown.changed() => {
                if *cfg.shutdown.borrow() {
                    break;
                }
            }
        }
    }
    debug!("dsl-bus-client sender shutting down");
}

async fn drain_once(cfg: &SenderConfig) -> Result<usize, sqlx::Error> {
    let mut tx = cfg.pool.begin().await?;
    let entries = select_pending_outbox(&mut tx, cfg.batch_size)
        .await
        .map_err(|e| match e {
            dsl_bus_storage::BusStorageError::Sqlx(err) => err,
            other => sqlx::Error::Configuration(other.to_string().into()),
        })?;
    let claimed = entries.len();

    for entry in entries {
        dispatch_entry(cfg, &mut tx, entry).await?;
    }

    tx.commit().await?;
    Ok(claimed)
}

async fn dispatch_entry(
    cfg: &SenderConfig,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    entry: OutboxEntry,
) -> Result<(), sqlx::Error> {
    match entry.target_endpoint {
        BusEndpoint::Invocation => dispatch_invocation(cfg, tx, entry).await,
        BusEndpoint::Result => dispatch_result(cfg, tx, entry).await,
    }
}

async fn dispatch_invocation(
    cfg: &SenderConfig,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    entry: OutboxEntry,
) -> Result<(), sqlx::Error> {
    let endpoint = match cfg.peers.endpoint(&entry.target_domain) {
        Ok(e) => e.clone(),
        Err(err) => {
            return record_retry(cfg, tx, &entry, &err.to_string()).await;
        }
    };

    let channel = match endpoint.connect().await {
        Ok(c) => c,
        Err(err) => {
            return record_retry(cfg, tx, &entry, &format!("connect: {err}")).await;
        }
    };

    let req = match InvocationRequest::decode(&entry.payload[..]) {
        Ok(r) => r,
        Err(err) => {
            return record_retry(cfg, tx, &entry, &format!("decode: {err}")).await;
        }
    };

    let mut client = InvocationServiceClient::new(channel);
    match client.submit(req).await {
        Ok(resp) => {
            let ack = resp.into_inner();
            match from_proto_opt(&ack.execution_id) {
                Ok(Some(exec_id)) => {
                    mark_outbox_submitted(&mut **tx, entry.id, exec_id)
                        .await
                        .map_err(map_storage_err)?;
                    cfg.stats.submitted.fetch_add(1, Ordering::Relaxed);
                }
                Ok(None) => {
                    record_retry(cfg, tx, &entry, "ack missing execution_id").await?;
                }
                Err(err) => {
                    record_retry(cfg, tx, &entry, &err.to_string()).await?;
                }
            }
        }
        Err(status) => {
            record_retry(cfg, tx, &entry, &format!("status: {}", status.message())).await?;
        }
    }
    Ok(())
}

async fn dispatch_result(
    cfg: &SenderConfig,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    entry: OutboxEntry,
) -> Result<(), sqlx::Error> {
    let endpoint = match cfg.peers.endpoint(&entry.target_domain) {
        Ok(e) => e.clone(),
        Err(err) => {
            return record_retry(cfg, tx, &entry, &err.to_string()).await;
        }
    };

    let channel = match endpoint.connect().await {
        Ok(c) => c,
        Err(err) => {
            return record_retry(cfg, tx, &entry, &format!("connect: {err}")).await;
        }
    };

    let msg = match InvocationResult::decode(&entry.payload[..]) {
        Ok(r) => r,
        Err(err) => {
            return record_retry(cfg, tx, &entry, &format!("decode: {err}")).await;
        }
    };

    let mut client = ResultServiceClient::new(channel);
    let exec_id = from_proto_opt(&msg.execution_id)
        .ok()
        .flatten()
        .unwrap_or_else(uuid::Uuid::nil);

    match client.deliver_result(msg).await {
        Ok(_resp) => {
            // Result deliveries don't return a fresh execution_id — re-use
            // the one we sent so the outbox row carries something useful.
            mark_outbox_submitted(&mut **tx, entry.id, exec_id)
                .await
                .map_err(map_storage_err)?;
            cfg.stats.submitted.fetch_add(1, Ordering::Relaxed);
        }
        Err(status) => {
            record_retry(cfg, tx, &entry, &format!("status: {}", status.message())).await?;
        }
    }
    Ok(())
}

async fn record_retry(
    cfg: &SenderConfig,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    entry: &OutboxEntry,
    message: &str,
) -> Result<(), sqlx::Error> {
    let backoff = exp_backoff_secs(entry.attempt_count, cfg.max_backoff_secs);
    mark_outbox_retry(&mut **tx, entry.id, backoff, message)
        .await
        .map_err(map_storage_err)?;
    cfg.stats.retried.fetch_add(1, Ordering::Relaxed);
    Ok(())
}

fn map_storage_err(e: dsl_bus_storage::BusStorageError) -> sqlx::Error {
    match e {
        dsl_bus_storage::BusStorageError::Sqlx(err) => err,
        other => sqlx::Error::Configuration(other.to_string().into()),
    }
}

/// 1s, 2s, 4s, 8s, … capped at `max_secs` (v0.6 §6.4).
pub(crate) fn exp_backoff_secs(attempt_count: i32, max_secs: i64) -> i64 {
    let attempts = attempt_count.max(0) as u32;
    // 2^attempt — saturate before overflow.
    let raw: i64 = 1i64.checked_shl(attempts).unwrap_or(i64::MAX);
    raw.clamp(1, max_secs)
}
