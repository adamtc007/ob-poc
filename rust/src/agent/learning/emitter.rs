//! Agent Event Emitter
//!
//! Fire-and-forget event emission for the agent learning loop.
//! Same pattern as DslEvent emitter - lock-free channel, never blocks.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

use super::types::AgentEvent;

/// Channel capacity for agent events.
/// Smaller than DSL events since agent interactions are less frequent.
const DEFAULT_BUFFER_SIZE: usize = 256;

/// Receiver end of the agent event channel (re-exported for drain task).
pub type AgentEventReceiver = mpsc::Receiver<AgentEvent>;

/// Shared emitter for use across async boundaries.
pub type SharedAgentEmitter = Arc<AgentEventEmitter>;

/// Agent event emitter.
///
/// Fire-and-forget: `emit()` never blocks, never fails, drops on full buffer.
/// This ensures zero impact on the agent hot path.
pub struct AgentEventEmitter {
    sender: mpsc::Sender<AgentEvent>,
    stats: EmitterStats,
}

/// Statistics for monitoring emitter health.
#[derive(Debug, Default)]
pub struct EmitterStats {
    /// Total events emitted
    emitted: AtomicU64,
    /// Events dropped due to full buffer
    dropped: AtomicU64,
}

impl AgentEventEmitter {
    /// Create a new emitter with default buffer size.
    pub fn new() -> (Self, AgentEventReceiver) {
        Self::with_buffer_size(DEFAULT_BUFFER_SIZE)
    }

    /// Create a new emitter with custom buffer size.
    pub fn with_buffer_size(size: usize) -> (Self, AgentEventReceiver) {
        let (sender, receiver) = mpsc::channel(size);
        let emitter = Self {
            sender,
            stats: EmitterStats::default(),
        };
        (emitter, receiver)
    }

    /// Emit an event (fire-and-forget).
    ///
    /// This method:
    /// - Never blocks (uses try_send)
    /// - Never fails (drops on full buffer)
    /// - Takes < 1μs
    #[inline]
    pub fn emit(&self, event: AgentEvent) {
        self.stats.emitted.fetch_add(1, Ordering::Relaxed);

        if self.sender.try_send(event).is_err() {
            // Buffer full - drop event and record
            self.stats.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Get current stats.
    pub fn stats(&self) -> (u64, u64) {
        (
            self.stats.emitted.load(Ordering::Relaxed),
            self.stats.dropped.load(Ordering::Relaxed),
        )
    }
}

impl Default for AgentEventEmitter {
    fn default() -> Self {
        Self::new().0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_emit_and_receive() {
        let (emitter, mut receiver) = AgentEventEmitter::new();

        let event =
            AgentEvent::message_received(Some(Uuid::now_v7()), "test message".to_string(), None);

        emitter.emit(event);

        let received = receiver.recv().await;
        assert!(received.is_some());
    }

    #[tokio::test]
    async fn test_emit_never_blocks() {
        // Create tiny buffer
        let (emitter, _receiver) = AgentEventEmitter::with_buffer_size(2);

        // Emit more than buffer size
        for i in 0..10 {
            let event =
                AgentEvent::message_received(Some(Uuid::now_v7()), format!("message {}", i), None);
            emitter.emit(event);
        }

        let (emitted, dropped) = emitter.stats();
        assert_eq!(emitted, 10);
        assert!(dropped > 0); // Some should be dropped
    }

    #[test]
    fn test_emit_is_fast() {
        let (emitter, _receiver) = AgentEventEmitter::new();

        let start = std::time::Instant::now();
        for _ in 0..1000 {
            let event =
                AgentEvent::message_received(Some(Uuid::now_v7()), "test".to_string(), None);
            emitter.emit(event);
        }
        let elapsed = start.elapsed();

        // 1000 emits should take < 10ms (< 10μs each)
        assert!(elapsed.as_millis() < 10, "Emit too slow: {:?}", elapsed);
    }
}
