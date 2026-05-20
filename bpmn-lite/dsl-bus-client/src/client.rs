//! `BusClient` builder + public send-side API.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use dsl_bus_protocol::v1::{InvocationRequest, InvocationResult};
use dsl_bus_storage::{insert_outbox, BusEndpoint, InsertOutcome, OutboxEntry};
use prost::Message;
use sqlx::PgPool;
use thiserror::Error;
use tokio::task::JoinHandle;
use tonic::transport::Endpoint;
use uuid::Uuid;

use crate::sender::{self, SenderConfig, SenderStats};
use crate::uuid_convert::from_proto_opt;

/// Default sender sweep cadence — 100 ms matches the pseudocode in
/// v0.6 §8.5.
const DEFAULT_SENDER_INTERVAL_MS: u64 = 100;
/// Maximum number of outbox rows claimed per sweep.
const DEFAULT_SENDER_BATCH: i64 = 10;
/// Backoff ceiling for retries (caps exponential growth).
const DEFAULT_MAX_BACKOFF_SECS: i64 = 60;

#[derive(Debug, Error)]
pub enum BusClientError {
    #[error("bus storage error: {0}")]
    Storage(#[from] dsl_bus_storage::BusStorageError),

    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("invalid uuid payload (expected 16 bytes, got {actual_len})")]
    MalformedUuid { actual_len: usize },

    #[error("unknown target domain '{0}' — no peer endpoint registered")]
    UnknownPeer(String),

    #[error("invocation request must carry an idempotency_key")]
    MissingIdempotencyKey,

    #[error("invocation result must carry an idempotency_key")]
    MissingResultIdempotencyKey,

    #[error("transport error: {0}")]
    Transport(#[from] tonic::transport::Error),
}

/// Tunable knobs for the bus client. Built via [`BusClientBuilder`].
#[derive(Debug, Clone)]
pub struct BusClientConfig {
    pub sender_interval: Duration,
    pub sender_batch_size: i64,
    pub max_backoff_secs: i64,
}

impl Default for BusClientConfig {
    fn default() -> Self {
        Self {
            sender_interval: Duration::from_millis(DEFAULT_SENDER_INTERVAL_MS),
            sender_batch_size: DEFAULT_SENDER_BATCH,
            max_backoff_secs: DEFAULT_MAX_BACKOFF_SECS,
        }
    }
}

/// Registry of peer endpoints, keyed by `target_domain`.
#[derive(Debug, Clone, Default)]
pub(crate) struct PeerRegistry {
    endpoints: HashMap<String, Endpoint>,
}

impl PeerRegistry {
    pub(crate) fn endpoint(&self, domain: &str) -> Result<&Endpoint, BusClientError> {
        self.endpoints
            .get(domain)
            .ok_or_else(|| BusClientError::UnknownPeer(domain.to_owned()))
    }
}

/// Send-side bus client. Cloneable handle backed by `Arc` state.
#[derive(Clone)]
pub struct BusClient {
    pub(crate) pool: PgPool,
    pub(crate) peers: Arc<PeerRegistry>,
    pub(crate) config: Arc<BusClientConfig>,
    /// Domain id of the local participant (carried into outbound
    /// `InvocationRequest.source_domain` when callers do not set it).
    pub(crate) local_domain: Arc<String>,
}

/// Handle returned by [`BusClient::start_sender`] — call
/// [`shutdown`](SenderHandle::shutdown) to stop the loop and await the
/// background task.
pub struct SenderHandle {
    pub(crate) shutdown: tokio::sync::watch::Sender<bool>,
    pub(crate) join: JoinHandle<()>,
    pub(crate) stats: Arc<SenderStats>,
}

impl SenderHandle {
    /// Snapshot of in-flight sender metrics.
    pub fn stats(&self) -> SenderStats {
        (*self.stats).snapshot()
    }

    /// Signal the sender to stop and await its termination.
    pub async fn shutdown(self) -> Result<(), tokio::task::JoinError> {
        let _ = self.shutdown.send(true);
        self.join.await
    }
}

impl BusClient {
    /// Start a builder; defaults are mostly OK but you must provide a
    /// `pool` + `local_domain` before `build()`.
    pub fn builder() -> BusClientBuilder {
        BusClientBuilder::default()
    }

    /// Write an invocation to the outbox. Returns the idempotency key
    /// (also present in `req`) and an [`InsertOutcome`] so callers can
    /// distinguish a fresh enqueue from an idempotent replay.
    pub async fn submit_invocation(
        &self,
        target_domain: &str,
        mut req: InvocationRequest,
    ) -> Result<(Uuid, InsertOutcome), BusClientError> {
        // Confirm the target is registered so we fail fast at submit
        // time rather than from inside the sender loop.
        self.peers.endpoint(target_domain)?;

        let key = from_proto_opt(&req.idempotency_key)?
            .ok_or(BusClientError::MissingIdempotencyKey)?;

        if req.source_domain.is_empty() {
            req.source_domain = self.local_domain.as_str().to_owned();
        }

        let payload = req.encode_to_vec();
        let entry = OutboxEntry::new_pending(
            Uuid::now_v7(),
            target_domain.to_owned(),
            BusEndpoint::Invocation,
            payload,
            key,
        );
        let outcome = insert_outbox(&self.pool, &entry).await?;
        Ok((key, outcome))
    }

    /// Receiver-side: enqueue a `DeliverResult` payload bound for the
    /// originating domain. Mirrors [`submit_invocation`] but with the
    /// `result` endpoint.
    pub async fn send_result(
        &self,
        target_domain: &str,
        result: InvocationResult,
    ) -> Result<(Uuid, InsertOutcome), BusClientError> {
        self.peers.endpoint(target_domain)?;

        let key = from_proto_opt(&result.idempotency_key)?
            .ok_or(BusClientError::MissingResultIdempotencyKey)?;

        let payload = result.encode_to_vec();
        let entry = OutboxEntry::new_pending(
            Uuid::now_v7(),
            target_domain.to_owned(),
            BusEndpoint::Result,
            payload,
            key,
        );
        let outcome = insert_outbox(&self.pool, &entry).await?;
        Ok((key, outcome))
    }

    /// Spawn the §8.5 sender task.
    pub fn start_sender(&self) -> SenderHandle {
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let stats = Arc::new(SenderStats::default());
        let config = SenderConfig {
            pool: self.pool.clone(),
            peers: self.peers.clone(),
            interval: self.config.sender_interval,
            batch_size: self.config.sender_batch_size,
            max_backoff_secs: self.config.max_backoff_secs,
            stats: stats.clone(),
            shutdown: shutdown_rx,
        };
        let join = tokio::spawn(sender::run(config));
        SenderHandle {
            shutdown: shutdown_tx,
            join,
            stats,
        }
    }

}

/// Builder for [`BusClient`].
#[derive(Default)]
pub struct BusClientBuilder {
    pool: Option<PgPool>,
    peers: HashMap<String, String>,
    config: BusClientConfig,
    local_domain: Option<String>,
}

impl BusClientBuilder {
    pub fn pool(mut self, pool: PgPool) -> Self {
        self.pool = Some(pool);
        self
    }

    /// Identify the local participant. Used to fill
    /// `InvocationRequest.source_domain` when the caller leaves it
    /// blank.
    pub fn local_domain(mut self, domain: impl Into<String>) -> Self {
        self.local_domain = Some(domain.into());
        self
    }

    pub fn add_peer(mut self, domain: impl Into<String>, endpoint: impl Into<String>) -> Self {
        self.peers.insert(domain.into(), endpoint.into());
        self
    }

    pub fn sender_interval(mut self, interval: Duration) -> Self {
        self.config.sender_interval = interval;
        self
    }

    pub fn sender_batch_size(mut self, batch_size: i64) -> Self {
        self.config.sender_batch_size = batch_size;
        self
    }

    pub fn max_backoff_secs(mut self, secs: i64) -> Self {
        self.config.max_backoff_secs = secs;
        self
    }

    pub async fn build(self) -> Result<BusClient, BusClientError> {
        let pool = self.pool.expect("BusClientBuilder.pool is required");
        let local_domain = self
            .local_domain
            .expect("BusClientBuilder.local_domain is required");
        let mut endpoints = HashMap::with_capacity(self.peers.len());
        for (domain, uri) in self.peers {
            let endpoint = Endpoint::from_shared(uri)?;
            endpoints.insert(domain, endpoint);
        }
        Ok(BusClient {
            pool,
            peers: Arc::new(PeerRegistry { endpoints }),
            config: Arc::new(self.config),
            local_domain: Arc::new(local_domain),
        })
    }
}
