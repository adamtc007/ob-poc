//! Event Infrastructure for DSL Observability
//!
//! This module provides always-on, zero-overhead event capture from the DSL
//! pipeline. It's designed to have minimal impact on the executor hot path.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────┐
//! │ DSL Executor    │
//! │                 │     1. Create event struct (~200ns)
//! │  result = ...   │     2. try_send to channel (~100ns)
//! │                 │     3. Return result (event forgotten)
//! │  emit(event)  ──┼──►  Never awaits, never fails
//! │                 │
//! │  return result  │
//! └────────┬────────┘
//!          │
//!          │ lock-free channel (bounded, drops on full)
//!          ▼
//! ┌─────────────────┐
//! │ Drain Task      │  Separate tokio task
//! │ (background)    │  - Reads from channel
//! │                 │  - Writes to event store
//! │                 │  - If slow, events drop
//! └────────┬────────┘
//!          │
//!          ▼
//! ┌─────────────────┐
//! │ Event Store     │  Append-only (JSONL or DB)
//! └─────────────────┘
//! ```
//!
//! # Performance Guarantees
//!
//! - `emit()` is < 1μs (benchmarked)
//! - `emit()` never blocks
//! - `emit()` never fails (drops on full buffer)
//! - `emit()` never panics
//!
//! # Usage
//!
//! ```ignore
//! use ob_poc::events::{init_events, EventConfig, DslEvent};
//!
//! // Initialize at startup
//! let emitter = init_events(&EventConfig::default());
//!
//! // In executor (if enabled)
//! if let Some(ref events) = emitter {
//!     events.emit(DslEvent::succeeded(session_id, verb, duration));
//! }
//! ```

pub mod config;
pub mod drain;
pub mod emitter;
pub mod health;
pub mod store;
pub mod types;

#[cfg(feature = "database")]
pub mod session_log;

// Re-exports for convenience
pub use config::{DrainConfig, EventConfig, StoreConfig};
pub use drain::spawn_drain_task;
pub use emitter::{EmitterStats, EventEmitter, EventReceiver, SharedEmitter};
pub use health::EventHealth;
pub use store::EventStore;
pub use types::{DslEvent, ErrorSnapshot, EventPayload, SessionSource};

#[cfg(feature = "database")]
pub use session_log::{EntryType, SessionLogEntry, SessionLogQuery, SessionLogger};

use std::sync::Arc;
use tracing::info;

/// Initialize the event infrastructure.
///
/// Returns an optional shared emitter. If events are disabled in config,
/// returns `None` and the executor won't emit any events.
///
/// This function:
/// 1. Creates the event channel (bounded, lock-free)
/// 2. Creates the event store (file or DB based on config)
/// 3. Spawns the drain task (background, auto-restart on panic)
/// 4. Returns the emitter for use in the executor
///
/// # Example
///
/// ```ignore
/// let config = EventConfig::default();
/// let emitter = init_events(&config);
///
/// // Pass to executor
/// let executor = DslExecutor::new(pool, registry).with_events(emitter);
/// ```
pub fn init_events(config: &EventConfig) -> Option<SharedEmitter> {
    if !config.enabled {
        info!("Event infrastructure disabled");
        return None;
    }

    // Create the channel
    let (emitter, receiver) = EventEmitter::new(config.buffer_size);
    let emitter = Arc::new(emitter);

    // Create the store
    let store = match &config.store {
        StoreConfig::File { path } => {
            // Ensure parent directory exists
            if let Some(parent) = path.parent() {
                if !parent.exists() {
                    if let Err(e) = std::fs::create_dir_all(parent) {
                        tracing::warn!(
                            error = %e,
                            path = ?parent,
                            "Failed to create event store directory"
                        );
                    }
                }
            }
            EventStore::new(path.clone())
        }
        #[cfg(feature = "database")]
        StoreConfig::Database { pool: _ } => {
            // For now, fall back to file store
            // DB store will be used when we implement DbEventStore properly
            tracing::warn!("Database event store not yet implemented, using file store");
            EventStore::new(std::path::PathBuf::from("data/events.jsonl"))
        }
    };

    // Spawn the drain task
    let flush_interval = config.drain.flush_interval();
    drain::spawn_drain_task_with_config(receiver, store, config.drain.batch_size, flush_interval);

    info!(
        buffer_size = config.buffer_size,
        batch_size = config.drain.batch_size,
        flush_interval_ms = config.drain.flush_interval_ms,
        "Event infrastructure initialized"
    );

    Some(emitter)
}

/// Create an event emitter for testing (no drain task).
///
/// Useful for unit tests that want to capture events without persistence.
#[cfg(test)]
pub fn test_emitter(buffer_size: usize) -> (SharedEmitter, EventReceiver) {
    let (emitter, receiver) = EventEmitter::new(buffer_size);
    (Arc::new(emitter), receiver)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use uuid::Uuid;

    #[test]
    fn test_init_disabled() {
        let config = EventConfig::disabled();
        let emitter = init_events(&config);
        assert!(emitter.is_none());
    }

    #[tokio::test]
    async fn test_init_with_file_store() {
        let dir = tempdir().unwrap();
        let config = EventConfig::with_file_store(dir.path().join("events.jsonl"));

        let emitter = init_events(&config);
        assert!(emitter.is_some());

        // Emit some events
        let emitter = emitter.unwrap();
        emitter.emit(DslEvent::succeeded(
            Some(Uuid::new_v4()),
            "test.verb".to_string(),
            100,
        ));

        // Give drain task time to process
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let stats = emitter.stats();
        assert_eq!(stats.emitted, 1);
        assert_eq!(stats.dropped, 0);
    }

    #[test]
    fn test_test_emitter() {
        let (emitter, receiver) = test_emitter(10);

        emitter.emit(DslEvent::succeeded(None, "test".to_string(), 1));

        let event = receiver.try_recv();
        assert!(event.is_some());
    }
}
