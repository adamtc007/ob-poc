//! Background drain task for event processing.
//!
//! The drain task runs in a separate tokio task and pulls events from the
//! channel, writing them to the event store. This keeps all I/O off the
//! executor hot path.
//!
//! Key properties:
//! - Runs forever in the background
//! - Automatically restarts on panic
//! - Batches writes for efficiency
//! - Never blocks the executor

use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use super::emitter::EventReceiver;
use super::store::EventStore;

/// Background task that drains events from the channel to storage.
///
/// The drain task runs on its own tokio task and never blocks the executor.
/// It periodically drains events from the channel and writes them to the
/// event store in batches.
pub struct EventDrain {
    receiver: EventReceiver,
    store: EventStore,
    batch_size: usize,
    flush_interval: Duration,
}

impl EventDrain {
    /// Create a new drain with default settings.
    ///
    /// - `batch_size`: 100 events per batch
    /// - `flush_interval`: 1 second
    pub fn new(receiver: EventReceiver, store: EventStore) -> Self {
        Self {
            receiver,
            store,
            batch_size: 100,
            flush_interval: Duration::from_secs(1),
        }
    }

    /// Create a drain with custom settings.
    pub fn with_config(
        receiver: EventReceiver,
        store: EventStore,
        batch_size: usize,
        flush_interval: Duration,
    ) -> Self {
        Self {
            receiver,
            store,
            batch_size,
            flush_interval,
        }
    }

    /// Run the drain loop forever.
    ///
    /// This should be called inside a `tokio::spawn`. It will run until
    /// the receiver is disconnected (which shouldn't happen normally).
    pub async fn run(self) {
        let mut flush_timer = interval(self.flush_interval);
        // Skip the first immediate tick
        flush_timer.tick().await;

        info!(
            batch_size = self.batch_size,
            flush_interval_ms = self.flush_interval.as_millis() as u64,
            "Event drain started"
        );

        loop {
            // Wait for next flush interval
            flush_timer.tick().await;

            // Drain all available events
            let drained = self.drain_batch().await;

            if drained > 0 {
                debug!(events = drained, "Drained events to store");
            }

            // Flush the store
            if let Err(e) = self.store.flush().await {
                warn!(error = %e, "Event store flush failed (non-fatal)");
            }
        }
    }

    /// Drain a batch of events from the channel.
    ///
    /// Returns the number of events drained.
    async fn drain_batch(&self) -> usize {
        let batch = self.receiver.try_recv_batch(self.batch_size);

        if batch.is_empty() {
            return 0;
        }

        let count = batch.len();

        // Write batch to store
        for event in batch {
            if let Err(e) = self.store.append(&event).await {
                warn!(error = %e, "Event store write failed (non-fatal)");
                // Continue - don't let one bad event stop the drain
            }
        }

        count
    }
}

/// Spawn the drain task with automatic restart on panic.
///
/// This spawns a supervisor task that will restart the drain if it panics.
/// Under normal operation, the drain runs forever.
///
/// # Returns
///
/// A `JoinHandle` for the supervisor task (not the drain task itself).
pub fn spawn_drain_task(receiver: EventReceiver, store: EventStore) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut restart_count = 0u32;

        loop {
            let drain = EventDrain::new(receiver.clone(), store.clone());

            // Spawn the actual drain task
            let result = tokio::spawn(drain.run()).await;

            match result {
                Ok(()) => {
                    // Normal exit (shouldn't happen unless receiver disconnected)
                    info!("Drain task exited normally");
                    break;
                }
                Err(e) => {
                    restart_count += 1;
                    error!(
                        error = ?e,
                        restart_count,
                        "Drain task panicked, restarting in 1 second"
                    );

                    // Exponential backoff with cap
                    let backoff = Duration::from_secs(1.min(restart_count as u64));
                    tokio::time::sleep(backoff).await;

                    // Cap restart count to prevent overflow
                    if restart_count > 100 {
                        error!("Drain task restarted too many times, giving up");
                        break;
                    }
                }
            }
        }
    })
}

/// Spawn drain task with custom configuration.
pub fn spawn_drain_task_with_config(
    receiver: EventReceiver,
    store: EventStore,
    batch_size: usize,
    flush_interval: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut restart_count = 0u32;

        loop {
            let drain = EventDrain::with_config(
                receiver.clone(),
                store.clone(),
                batch_size,
                flush_interval,
            );

            let result = tokio::spawn(drain.run()).await;

            match result {
                Ok(()) => {
                    info!("Drain task exited normally");
                    break;
                }
                Err(e) => {
                    restart_count += 1;
                    error!(
                        error = ?e,
                        restart_count,
                        "Drain task panicked, restarting"
                    );

                    let backoff = Duration::from_secs(1.min(restart_count as u64));
                    tokio::time::sleep(backoff).await;

                    if restart_count > 100 {
                        error!("Drain task restarted too many times, giving up");
                        break;
                    }
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{DslEvent, EventEmitter};
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_drain_batch() {
        let (emitter, receiver) = EventEmitter::new(100);
        let dir = tempdir().unwrap();
        let store = EventStore::new(dir.path().join("events.jsonl"));

        // Emit some events
        for i in 0..5 {
            emitter.emit(DslEvent::succeeded(
                None,
                format!("test.verb.{}", i),
                i as u64,
            ));
        }

        let drain = EventDrain::new(receiver, store.clone());

        // Drain should process all 5 events
        let count = drain.drain_batch().await;
        assert_eq!(count, 5);

        // Verify events were written
        store.flush().await.unwrap();

        // Read the file and count lines
        let content = tokio::fs::read_to_string(dir.path().join("events.jsonl"))
            .await
            .unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 5);
    }

    #[tokio::test]
    async fn test_drain_empty_batch() {
        let (_, receiver) = EventEmitter::new(100);
        let dir = tempdir().unwrap();
        let store = EventStore::new(dir.path().join("events.jsonl"));

        let drain = EventDrain::new(receiver, store);

        // Drain with no events should return 0
        let count = drain.drain_batch().await;
        assert_eq!(count, 0);
    }
}
