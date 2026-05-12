//! Lock-free, non-blocking event emitter.
//!
//! This module provides the core event emission mechanism that runs in the
//! DSL executor hot path. The critical constraint is that `emit()` must:
//!
//! - Never block (uses `try_send`, not `send`)
//! - Never fail (returns `()`, errors are silently counted)
//! - Never panic (all operations are wrapped)
//! - Be fast (< 1μs target)
//!
//! If the buffer is full, events are dropped and the drop counter is incremented.
//! This is intentional - we never slow down the DSL pipeline for observability.

use crossbeam_channel::{bounded, Receiver, Sender, TrySendError};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use super::types::DslEvent;

/// Lock-free, non-blocking event emitter.
///
/// The emitter uses a bounded crossbeam channel to send events to a background
/// drain task. If the channel is full, events are dropped (never blocks).
///
/// # Performance
///
/// - `emit()` is O(1), typically < 500ns
/// - Uses atomic counters for stats (no locks)
/// - Channel is lock-free (crossbeam)
///
/// # Example
///
/// ```ignore
/// let (emitter, receiver) = EventEmitter::new(4096);
/// let emitter = Arc::new(emitter);
///
/// // In executor hot path - never blocks, never fails
/// emitter.emit(DslEvent::succeeded(session_id, verb, duration));
///
/// // In background task
/// while let Some(event) = receiver.recv() {
///     store.append(&event).await?;
/// }
/// ```
pub struct EventEmitter {
    sender: Sender<DslEvent>,

    // Stats (atomic, no locks)
    events_emitted: AtomicU64,
    events_dropped: AtomicU64,
}

impl EventEmitter {
    /// Create a new emitter with the given buffer size.
    ///
    /// Buffer size tradeoff:
    /// - Too small: drops events under load
    /// - Too large: memory pressure
    /// - 4096 is ~1MB for typical events, handles bursts well
    ///
    /// Returns the emitter and a receiver for the drain task.
    pub fn new(buffer_size: usize) -> (Self, EventReceiver) {
        let (sender, receiver) = bounded(buffer_size);

        let emitter = Self {
            sender,
            events_emitted: AtomicU64::new(0),
            events_dropped: AtomicU64::new(0),
        };

        let receiver = EventReceiver { receiver };

        (emitter, receiver)
    }

    /// Emit an event - NEVER BLOCKS, NEVER FAILS.
    ///
    /// If the buffer is full, the event is dropped and the drop counter
    /// is incremented. This is intentional - we never slow down the DSL
    /// pipeline for observability.
    ///
    /// # Performance
    ///
    /// This method is designed to be called in the executor hot path.
    /// It should complete in < 1μs under normal conditions.
    #[inline]
    pub fn emit(&self, event: DslEvent) {
        match self.sender.try_send(event) {
            Ok(()) => {
                self.events_emitted.fetch_add(1, Ordering::Relaxed);
            }
            Err(TrySendError::Full(_)) => {
                // Buffer full - drop event, increment counter
                self.events_dropped.fetch_add(1, Ordering::Relaxed);
            }
            Err(TrySendError::Disconnected(_)) => {
                // Receiver gone - this shouldn't happen in normal operation
                // but we handle it gracefully
                self.events_dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Get current stats (for monitoring).
    ///
    /// Uses relaxed ordering since stats don't need to be perfectly accurate.
    pub fn stats(&self) -> EmitterStats {
        EmitterStats {
            emitted: self.events_emitted.load(Ordering::Relaxed),
            dropped: self.events_dropped.load(Ordering::Relaxed),
        }
    }

    /// Check if the channel capacity is available (receiver is processing).
    ///
    /// Note: crossbeam channels don't have a direct "is_disconnected" check,
    /// so we check if we can still send (will be caught by try_send in emit).
    pub fn has_capacity(&self) -> bool {
        !self.sender.is_full()
    }
}

/// Emitter statistics.
#[derive(Debug, Clone, Copy, Default)]
pub struct EmitterStats {
    /// Total events successfully emitted to the channel
    pub emitted: u64,
    /// Total events dropped (buffer full or receiver disconnected)
    pub dropped: u64,
}

impl EmitterStats {
    /// Calculate the drop rate as a fraction (0.0 to 1.0).
    pub fn drop_rate(&self) -> f64 {
        let total = self.emitted + self.dropped;
        if total == 0 {
            0.0
        } else {
            self.dropped as f64 / total as f64
        }
    }

    /// Check if the emitter is healthy (drop rate < 1%).
    pub fn is_healthy(&self) -> bool {
        self.drop_rate() < 0.01
    }
}

/// Receiver for the drain task.
///
/// This is held by the background drain task and used to receive events
/// from the emitter. The receiver is NOT used in the executor hot path.
pub struct EventReceiver {
    receiver: Receiver<DslEvent>,
}

impl EventReceiver {
    /// Blocking receive - only called by drain task, never by executor.
    ///
    /// Returns `None` if the sender is disconnected.
    pub fn recv(&self) -> Option<DslEvent> {
        self.receiver.recv().ok()
    }

    /// Try to receive without blocking.
    ///
    /// Returns `None` if the channel is empty.
    pub fn try_recv(&self) -> Option<DslEvent> {
        self.receiver.try_recv().ok()
    }

    /// Try to receive a batch of events efficiently.
    ///
    /// Returns up to `max` events without blocking. Useful for batched
    /// writes to the event store.
    pub fn try_recv_batch(&self, max: usize) -> Vec<DslEvent> {
        let mut batch = Vec::with_capacity(max.min(64)); // Don't over-allocate
        while batch.len() < max {
            match self.receiver.try_recv() {
                Ok(event) => batch.push(event),
                Err(_) => break,
            }
        }
        batch
    }

    /// Check how many events are pending in the channel.
    pub fn len(&self) -> usize {
        self.receiver.len()
    }

    /// Check if the channel is empty.
    pub fn is_empty(&self) -> bool {
        self.receiver.is_empty()
    }
}

// EventReceiver needs to be cloneable for the drain task restart logic
impl Clone for EventReceiver {
    fn clone(&self) -> Self {
        Self {
            receiver: self.receiver.clone(),
        }
    }
}

/// Shared emitter handle for use across threads.
pub type SharedEmitter = Arc<EventEmitter>;

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_emit_succeeds() {
        let (emitter, receiver) = EventEmitter::new(10);

        emitter.emit(DslEvent::succeeded(
            Some(Uuid::new_v4()),
            "test.verb".to_string(),
            100,
        ));

        let stats = emitter.stats();
        assert_eq!(stats.emitted, 1);
        assert_eq!(stats.dropped, 0);

        // Verify event was received
        let event = receiver.try_recv();
        assert!(event.is_some());
    }

    #[test]
    fn test_emit_drops_when_full() {
        let (emitter, _receiver) = EventEmitter::new(2);

        // Fill the buffer
        emitter.emit(DslEvent::succeeded(None, "v1".to_string(), 1));
        emitter.emit(DslEvent::succeeded(None, "v2".to_string(), 2));

        // This should be dropped
        emitter.emit(DslEvent::succeeded(None, "v3".to_string(), 3));

        let stats = emitter.stats();
        assert_eq!(stats.emitted, 2);
        assert_eq!(stats.dropped, 1);
    }

    #[test]
    fn test_emit_drops_when_disconnected() {
        let (emitter, receiver) = EventEmitter::new(10);

        // Drop the receiver
        drop(receiver);

        // This should be dropped (receiver gone)
        emitter.emit(DslEvent::succeeded(None, "v1".to_string(), 1));

        let stats = emitter.stats();
        assert_eq!(stats.emitted, 0);
        assert_eq!(stats.dropped, 1);
    }

    #[test]
    fn test_try_recv_batch() {
        let (emitter, receiver) = EventEmitter::new(100);

        // Emit 5 events
        for i in 0..5 {
            emitter.emit(DslEvent::succeeded(None, format!("verb.{}", i), i as u64));
        }

        // Receive batch of up to 10
        let batch = receiver.try_recv_batch(10);
        assert_eq!(batch.len(), 5);

        // Channel should be empty now
        assert!(receiver.is_empty());
    }

    #[test]
    fn test_stats_drop_rate() {
        let stats = EmitterStats {
            emitted: 90,
            dropped: 10,
        };
        assert!((stats.drop_rate() - 0.1).abs() < 0.001);
        assert!(!stats.is_healthy()); // 10% drop rate > 1%

        let healthy_stats = EmitterStats {
            emitted: 1000,
            dropped: 5,
        };
        assert!(healthy_stats.is_healthy()); // 0.5% drop rate < 1%
    }

    #[test]
    fn test_emit_performance() {
        // Verify emit is fast (this is more of a smoke test)
        let (emitter, _receiver) = EventEmitter::new(10000);

        let start = std::time::Instant::now();
        for _ in 0..1000 {
            emitter.emit(DslEvent::succeeded(None, "perf.test".to_string(), 1));
        }
        let elapsed = start.elapsed();

        // 1000 emits should complete in < 10ms (generous margin)
        assert!(
            elapsed.as_millis() < 10,
            "1000 emits took {:?}, expected < 10ms",
            elapsed
        );

        // Per-emit should be < 10μs (generous margin, actual target is < 1μs)
        let per_emit = elapsed / 1000;
        assert!(
            per_emit.as_micros() < 10,
            "Per emit: {:?}, expected < 10μs",
            per_emit
        );
    }

    #[test]
    fn test_emit_benchmark_1us_target() {
        // Benchmark: verify < 1μs emit target
        // Use a large buffer to avoid backpressure
        let (emitter, _receiver) = EventEmitter::new(100_000);

        // Warm up
        for _ in 0..100 {
            emitter.emit(DslEvent::succeeded(None, "warmup".to_string(), 1));
        }

        // Benchmark 10,000 emits
        let iterations = 10_000;
        let start = std::time::Instant::now();
        for i in 0..iterations {
            emitter.emit(DslEvent::succeeded(
                None,
                format!("bench.test.{}", i),
                i as u64,
            ));
        }
        let elapsed = start.elapsed();

        let per_emit_ns = elapsed.as_nanos() / iterations as u128;

        // Target: < 1μs (1000ns) per emit
        // In release mode this should be ~100-300ns
        // In debug mode, allow up to 5μs due to lack of optimizations
        #[cfg(debug_assertions)]
        let max_ns = 5000; // 5μs in debug
        #[cfg(not(debug_assertions))]
        let max_ns = 1000; // 1μs in release

        assert!(
            per_emit_ns < max_ns,
            "Emit took {}ns, target < {}ns ({} emits in {:?})",
            per_emit_ns,
            max_ns,
            iterations,
            elapsed
        );
    }
}
