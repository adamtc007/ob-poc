//! OutboxDispatcher — background task that claims outbox events and runs projections.
//!
//! Processes one event type (`snapshot_set_published`) and one projection job
//! (`write_active_snapshot_set`). Single-job model: one publish → one outbox event →
//! one projection job → one watermark advance.

use std::sync::Arc;
use std::time::Duration;

use sem_os_core::ports::{OutboxStore, ProjectionWriter};

/// Background outbox dispatcher that claims and processes outbox events.
pub struct OutboxDispatcher {
    outbox: Arc<dyn OutboxStore>,
    projector: Arc<dyn ProjectionWriter>,
    interval: Duration,
    max_fails: u32,
}

impl OutboxDispatcher {
    pub fn new(
        outbox: Arc<dyn OutboxStore>,
        projector: Arc<dyn ProjectionWriter>,
        interval: Duration,
        max_fails: u32,
    ) -> Self {
        Self {
            outbox,
            projector,
            interval,
            max_fails,
        }
    }

    /// Run the dispatcher loop. This never returns under normal operation.
    /// Spawn it as a background task via `tokio::spawn`.
    pub async fn run(&self) {
        tracing::info!(
            "OutboxDispatcher started (poll interval={:?}, max_fails={})",
            self.interval,
            self.max_fails
        );
        loop {
            match self.outbox.claim_next("dispatcher-1").await {
                Ok(Some(event)) => {
                    let seq = event.outbox_seq;
                    let event_id = event.event_id.0;
                    tracing::debug!("Processing outbox event seq={seq} id={event_id}");

                    match self.projector.write_active_snapshot_set(&event).await {
                        Ok(()) => {
                            if let Err(e) = self.outbox.mark_processed(&event.event_id).await {
                                tracing::error!(
                                    "Failed to mark outbox event {event_id} as processed: {e}"
                                );
                            } else {
                                tracing::debug!("Outbox event seq={seq} processed successfully");
                            }
                        }
                        Err(e) => {
                            tracing::error!("Projection failed for outbox event seq={seq}: {e}");
                            if event.attempt_count >= self.max_fails {
                                // Exceeded max attempts — permanently dead-letter.
                                tracing::error!(
                                    "DEAD LETTER: outbox_seq={seq} exceeded max_fails={} — event will not be retried",
                                    self.max_fails
                                );
                                if let Err(mark_err) = self
                                    .outbox
                                    .mark_dead_letter(&event.event_id, &e.to_string())
                                    .await
                                {
                                    tracing::error!(
                                        "Failed to dead-letter outbox event {event_id}: {mark_err}"
                                    );
                                }
                            } else {
                                // Retryable failure — release the claim for re-processing.
                                if let Err(mark_err) = self
                                    .outbox
                                    .record_failure(&event.event_id, &e.to_string())
                                    .await
                                {
                                    tracing::error!(
                                        "Failed to record failure for outbox event {event_id}: {mark_err}"
                                    );
                                }
                            }
                        }
                    }
                }
                Ok(None) => {
                    // No events to process — sleep and retry.
                    tokio::time::sleep(self.interval).await;
                }
                Err(e) => {
                    tracing::error!("Outbox claim failed: {e}");
                    tokio::time::sleep(self.interval).await;
                }
            }
        }
    }
}
