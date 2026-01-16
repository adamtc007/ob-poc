//! Agent Event Drain Task
//!
//! Background task that receives agent events from the channel
//! and persists them to the database for later analysis.

use sqlx::PgPool;
use std::time::Duration;
use tokio::time::interval;

use super::emitter::AgentEventReceiver;
use super::inspector::AgentLearningInspector;
use super::types::AgentEvent;

/// Spawn the agent event drain task.
///
/// Runs in background, batches events, persists to database.
/// Gracefully handles database unavailability (drops events).
pub fn spawn_agent_drain_task(
    mut receiver: AgentEventReceiver,
    pool: PgPool,
    batch_size: usize,
    flush_interval_ms: u64,
) {
    tokio::spawn(async move {
        let inspector = AgentLearningInspector::new(pool);
        let mut batch: Vec<AgentEvent> = Vec::with_capacity(batch_size);
        let mut flush_timer = interval(Duration::from_millis(flush_interval_ms));

        loop {
            tokio::select! {
                // Receive events from channel
                event = receiver.recv() => {
                    match event {
                        Some(e) => {
                            batch.push(e);
                            if batch.len() >= batch_size {
                                flush_batch(&inspector, &mut batch).await;
                            }
                        }
                        None => {
                            // Channel closed - flush remaining and exit
                            if !batch.is_empty() {
                                flush_batch(&inspector, &mut batch).await;
                            }
                            tracing::info!("Agent event drain task shutting down");
                            break;
                        }
                    }
                }
                // Periodic flush
                _ = flush_timer.tick() => {
                    if !batch.is_empty() {
                        flush_batch(&inspector, &mut batch).await;
                    }
                }
            }
        }
    });
}

/// Flush batch of events to database.
async fn flush_batch(inspector: &AgentLearningInspector, batch: &mut Vec<AgentEvent>) {
    let count = batch.len();
    let mut success = 0;
    let mut failed = 0;

    for event in batch.drain(..) {
        match inspector.store_event(&event).await {
            Ok(_) => success += 1,
            Err(e) => {
                failed += 1;
                tracing::debug!("Failed to store agent event: {}", e);
            }
        }
    }

    if failed > 0 {
        tracing::warn!(
            total = count,
            success = success,
            failed = failed,
            "Agent event batch flush completed with errors"
        );
    } else {
        tracing::debug!(count = count, "Agent event batch flushed");
    }
}

/// Configuration for drain task.
#[derive(Debug, Clone)]
pub struct DrainConfig {
    /// Number of events to batch before flushing
    pub batch_size: usize,
    /// Max time between flushes in milliseconds
    pub flush_interval_ms: u64,
}

impl Default for DrainConfig {
    fn default() -> Self {
        Self {
            batch_size: 50,
            flush_interval_ms: 5000, // 5 seconds
        }
    }
}
