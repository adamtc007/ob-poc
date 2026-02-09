//! PendingDispatchWorker — background retry task for queued BPMN dispatches.
//!
//! When the bpmn-lite gRPC service is temporarily unavailable, the
//! `WorkflowDispatcher` persists dispatch requests in `bpmn_pending_dispatches`.
//! This worker scans the queue periodically and retries `StartProcess` calls
//! until the service recovers or the maximum retry count is exceeded.
//!
//! ## Idempotency
//!
//! Each pending dispatch carries a pre-generated `correlation_id` that remains
//! stable across retries. The bpmn-lite service uses this for deduplication,
//! making retries safe even after partial success.

use std::sync::Arc;
use tokio::sync::watch;

use super::client::{BpmnLiteConnection, StartProcessRequest};
use super::config::WorkflowConfigIndex;
use super::correlation::CorrelationStore;
use super::pending_dispatches::PendingDispatchStore;

/// Retry interval between scan cycles.
const POLL_INTERVAL_SECS: u64 = 10;

/// Maximum dispatches to process per cycle.
const BATCH_SIZE: i32 = 5;

/// Maximum retry attempts before marking as permanently failed.
const MAX_ATTEMPTS: i32 = 50;

/// Backoff duration — only retry rows not attempted in the last N seconds.
const BACKOFF_SECS: u64 = 10;

// ---------------------------------------------------------------------------
// PendingDispatchWorker
// ---------------------------------------------------------------------------

/// Background worker that retries queued BPMN dispatch requests.
pub struct PendingDispatchWorker {
    /// gRPC connection to bpmn-lite (separate from dispatcher's client).
    bpmn_client: BpmnLiteConnection,
    /// Pending dispatch queue store.
    pending_dispatches: PendingDispatchStore,
    /// Correlation store — to patch process_instance_id after success.
    correlations: CorrelationStore,
    /// Workflow config — for bytecode version lookup.
    config: Arc<WorkflowConfigIndex>,
}

impl PendingDispatchWorker {
    pub fn new(
        bpmn_client: BpmnLiteConnection,
        pending_dispatches: PendingDispatchStore,
        correlations: CorrelationStore,
        config: Arc<WorkflowConfigIndex>,
    ) -> Self {
        Self {
            bpmn_client,
            pending_dispatches,
            correlations,
            config,
        }
    }

    /// Run the worker loop until the shutdown signal is received.
    pub async fn run(&self, mut shutdown_rx: watch::Receiver<bool>) {
        tracing::info!("PendingDispatchWorker started");

        loop {
            if *shutdown_rx.borrow() {
                tracing::info!("PendingDispatchWorker shutting down");
                break;
            }

            self.process_pending().await;

            tokio::select! {
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(POLL_INTERVAL_SECS)) => {}
                _ = shutdown_rx.changed() => {
                    tracing::info!("PendingDispatchWorker shutting down (during sleep)");
                    break;
                }
            }
        }

        tracing::info!("PendingDispatchWorker stopped");
    }

    /// Scan pending dispatches and retry each.
    async fn process_pending(&self) {
        let backoff = std::time::Duration::from_secs(BACKOFF_SECS);
        let dispatches = match self
            .pending_dispatches
            .claim_pending(BATCH_SIZE, backoff)
            .await
        {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "PendingDispatchWorker: failed to claim pending dispatches"
                );
                return;
            }
        };

        if !dispatches.is_empty() {
            tracing::debug!(
                count = dispatches.len(),
                "PendingDispatchWorker: processing pending dispatches"
            );
        }

        for dispatch in dispatches {
            self.try_dispatch(&dispatch).await;
        }
    }

    /// Attempt to dispatch a single queued request.
    async fn try_dispatch(&self, dispatch: &super::types::PendingDispatch) {
        // Use fresh bytecode version from config if available, otherwise
        // fall back to the version stored at queue time.
        let bytecode_version = self
            .config
            .bytecode_for_process(&dispatch.process_key)
            .map(|b| b.to_vec())
            .unwrap_or_else(|| dispatch.bytecode_version.clone());

        let result = self
            .bpmn_client
            .start_process(StartProcessRequest {
                process_key: dispatch.process_key.clone(),
                bytecode_version,
                domain_payload: dispatch.domain_payload.clone(),
                domain_payload_hash: dispatch.payload_hash.clone(),
                orch_flags: std::collections::HashMap::new(),
                correlation_id: dispatch.correlation_id,
            })
            .await;

        match result {
            Ok(process_instance_id) => {
                // Patch the correlation record with the real process_instance_id.
                if let Err(e) = self
                    .correlations
                    .update_process_instance_id(dispatch.correlation_id, process_instance_id)
                    .await
                {
                    tracing::error!(
                        correlation_id = %dispatch.correlation_id,
                        error = %e,
                        "PendingDispatchWorker: failed to update correlation after dispatch"
                    );
                }

                // Mark dispatch as complete.
                if let Err(e) = self
                    .pending_dispatches
                    .mark_dispatched(dispatch.dispatch_id)
                    .await
                {
                    tracing::error!(
                        dispatch_id = %dispatch.dispatch_id,
                        error = %e,
                        "PendingDispatchWorker: failed to mark dispatch as dispatched"
                    );
                }

                tracing::info!(
                    verb = %dispatch.verb_fqn,
                    process_key = %dispatch.process_key,
                    process_instance_id = %process_instance_id,
                    attempts = dispatch.attempts + 1,
                    "PendingDispatchWorker: successfully dispatched queued request"
                );
            }
            Err(e) => {
                if let Err(store_err) = self
                    .pending_dispatches
                    .record_failure(dispatch.dispatch_id, &e.to_string(), MAX_ATTEMPTS)
                    .await
                {
                    tracing::error!(
                        dispatch_id = %dispatch.dispatch_id,
                        error = %store_err,
                        "PendingDispatchWorker: failed to record failure"
                    );
                }

                if dispatch.attempts + 1 >= MAX_ATTEMPTS {
                    tracing::warn!(
                        verb = %dispatch.verb_fqn,
                        dispatch_id = %dispatch.dispatch_id,
                        attempts = dispatch.attempts + 1,
                        "PendingDispatchWorker: dispatch permanently failed after max retries"
                    );
                } else {
                    tracing::debug!(
                        verb = %dispatch.verb_fqn,
                        attempts = dispatch.attempts + 1,
                        error = %e,
                        "PendingDispatchWorker: retry failed, will try again"
                    );
                }
            }
        }
    }
}
