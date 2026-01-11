# 023a: Event Infrastructure

> **Status:** ✅ IMPLEMENTED
> **Implemented:** 2026-01-11 (see 025)
> **Part of:** Adaptive Feedback System (023a + 023b)
> **Depends on:** DSL v2 execution pipeline

---

## Executive Summary

Lightweight, always-on event emission from DSL pipeline. Captures execution events and session context for later analysis.

**Critical constraint:** Zero impact on DSL pipeline performance and reliability.

---

## Part 1: Performance Contract

### 1.1 Guarantees

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│  GUARANTEE                          MECHANISM                                │
│  ═══════════════════════════════════════════════════════════════════════    │
│                                                                              │
│  Never block executor               Fire-and-forget, no await               │
│  Never fail DSL command             All event ops wrapped in catch-all      │
│  Never wait for subscribers         Bounded channel, drop on full           │
│  Never contend for locks            Lock-free channel (crossbeam)           │
│  Bounded memory                     Fixed-size ring buffer                  │
│  No I/O in hot path                 Async drain to separate task            │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Failure Modes

| Scenario | Behavior | DSL Impact |
|----------|----------|------------|
| Event buffer full | Oldest events dropped | None |
| Event store unavailable | Events queued in memory, then dropped | None |
| Event store write fails | Logged, continue | None |
| Drain task panics | Respawns, some events lost | None |
| OOM pressure | Events dropped first | None |

### 1.3 Performance Budget

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│  OPERATION                          BUDGET         ACTUAL (target)           │
│  ═══════════════════════════════════════════════════════════════════════    │
│                                                                              │
│  Event creation (struct init)       < 1 μs         ~200 ns                  │
│  Channel send (fire-and-forget)     < 1 μs         ~100 ns                  │
│  Total executor overhead            < 5 μs         ~500 ns                  │
│                                                                              │
│  For comparison:                                                            │
│  - Typical DSL command: 1-100 ms                                            │
│  - Network call: 50-500 ms                                                  │
│  - Event overhead: 0.0005-0.05% of command time                             │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Part 2: Architecture

### 2.1 Component Layout

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│  DSL EXECUTOR (hot path)                                                     │
│  ════════════════════════                                                   │
│                                                                              │
│  ┌─────────────────┐                                                        │
│  │ execute()       │                                                        │
│  │                 │      1. Create event struct (~200ns)                   │
│  │  result = ...   │      2. try_send to channel (~100ns)                   │
│  │                 │      3. Return result (event forgotten)                │
│  │  emit(event)  ──┼──►  Never awaits, never fails                         │
│  │                 │                                                        │
│  │  return result  │                                                        │
│  └─────────────────┘                                                        │
│           │                                                                 │
│           │ lock-free channel (bounded, drops on full)                      │
│           ▼                                                                 │
│  ┌─────────────────┐                                                        │
│  │ Drain Task      │  Separate tokio task, own budget                       │
│  │ (background)    │                                                        │
│  │                 │  - Reads from channel                                  │
│  │                 │  - Writes to event store                               │
│  │                 │  - If slow, channel fills, events drop                 │
│  │                 │  - Executor never waits                                │
│  └────────┬────────┘                                                        │
│           │                                                                 │
│           ▼                                                                 │
│  ┌─────────────────┐                                                        │
│  │ Event Store     │  Append-only (JSONL file or DB)                       │
│  │                 │  Batched writes for efficiency                         │
│  └─────────────────┘                                                        │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Why This Is Safe

```rust
// The executor does THIS (fast, infallible):
self.events.emit(event);  // Returns (), never fails, never blocks

// NOT this (slow, fallible):
self.events.send(event).await?;  // WRONG: awaits, can fail
```

---

## Part 3: Implementation

### 3.1 Event Types

```rust
// rust/src/events/types.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Lightweight event - cheap to create, cheap to clone
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslEvent {
    pub timestamp: DateTime<Utc>,
    pub session_id: Option<Uuid>,
    pub payload: EventPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EventPayload {
    CommandSucceeded {
        verb: String,
        duration_ms: u64,
    },
    
    CommandFailed {
        verb: String,
        duration_ms: u64,
        error: ErrorSnapshot,
    },
    
    SessionStarted {
        source: SessionSource,
    },
    
    SessionEnded {
        command_count: u32,
        error_count: u32,
        duration_secs: u64,
    },
}

/// Minimal error snapshot - no references, no large allocations in hot path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorSnapshot {
    pub error_type: String,
    pub message: String,           // Truncated to 500 chars
    pub source_id: Option<String>,
    pub http_status: Option<u16>,
    pub verb: String,
    // Heavy fields captured lazily by drain task if needed
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SessionSource {
    Repl,
    Egui,
    Mcp,
    Api,
}

impl DslEvent {
    /// Create success event - minimal allocation
    #[inline]
    pub fn succeeded(session_id: Option<Uuid>, verb: String, duration_ms: u64) -> Self {
        Self {
            timestamp: Utc::now(),
            session_id,
            payload: EventPayload::CommandSucceeded { verb, duration_ms },
        }
    }
    
    /// Create failure event - truncates message to avoid large allocs
    #[inline]
    pub fn failed(session_id: Option<Uuid>, verb: String, duration_ms: u64, error: &dyn std::error::Error, source_id: Option<&str>, http_status: Option<u16>) -> Self {
        let message = error.to_string();
        let message = if message.len() > 500 {
            format!("{}...", &message[..497])
        } else {
            message
        };
        
        Self {
            timestamp: Utc::now(),
            session_id,
            payload: EventPayload::CommandFailed {
                verb: verb.clone(),
                duration_ms,
                error: ErrorSnapshot {
                    error_type: std::any::type_name_of_val(error).to_string(),
                    message,
                    source_id: source_id.map(|s| s.to_string()),
                    http_status,
                    verb,
                },
            },
        }
    }
}
```

### 3.2 Event Emitter (Lock-Free)

```rust
// rust/src/events/emitter.rs

use crossbeam_channel::{bounded, Sender, TrySendError};
use std::sync::atomic::{AtomicU64, Ordering};

/// Lock-free, non-blocking event emitter
/// 
/// PERFORMANCE: emit() is O(1), never blocks, never fails
pub struct EventEmitter {
    sender: Sender<DslEvent>,
    
    // Stats (atomic, no locks)
    events_emitted: AtomicU64,
    events_dropped: AtomicU64,
}

impl EventEmitter {
    /// Create emitter with bounded buffer
    /// 
    /// Buffer size tradeoff:
    /// - Too small: drops events under load
    /// - Too large: memory pressure
    /// - 4096 is ~1MB for typical events, handles bursts well
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
    
    /// Emit event - NEVER BLOCKS, NEVER FAILS
    /// 
    /// If buffer is full, event is dropped and counter incremented.
    /// This is intentional - we never slow down the DSL pipeline.
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
                self.events_dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    
    /// Get stats (for monitoring)
    pub fn stats(&self) -> EmitterStats {
        EmitterStats {
            emitted: self.events_emitted.load(Ordering::Relaxed),
            dropped: self.events_dropped.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EmitterStats {
    pub emitted: u64,
    pub dropped: u64,
}

/// Receiver for drain task
pub struct EventReceiver {
    receiver: crossbeam_channel::Receiver<DslEvent>,
}

impl EventReceiver {
    /// Blocking receive - only called by drain task, never by executor
    pub fn recv(&self) -> Option<DslEvent> {
        self.receiver.recv().ok()
    }
    
    /// Try receive batch - for efficient draining
    pub fn try_recv_batch(&self, max: usize) -> Vec<DslEvent> {
        let mut batch = Vec::with_capacity(max);
        while batch.len() < max {
            match self.receiver.try_recv() {
                Ok(event) => batch.push(event),
                Err(_) => break,
            }
        }
        batch
    }
}
```

### 3.3 Drain Task (Background)

```rust
// rust/src/events/drain.rs

use std::time::Duration;
use tokio::time::interval;

/// Background task that drains events to storage
/// 
/// Runs on its own tokio task, never blocks executor
pub struct EventDrain {
    receiver: EventReceiver,
    store: EventStore,
    batch_size: usize,
    flush_interval: Duration,
}

impl EventDrain {
    pub fn new(receiver: EventReceiver, store: EventStore) -> Self {
        Self {
            receiver,
            store,
            batch_size: 100,
            flush_interval: Duration::from_secs(1),
        }
    }
    
    /// Run drain loop - call this in tokio::spawn
    pub async fn run(mut self) {
        let mut flush_timer = interval(self.flush_interval);
        
        loop {
            tokio::select! {
                // Periodic flush
                _ = flush_timer.tick() => {
                    self.drain_batch().await;
                    if let Err(e) = self.store.flush().await {
                        tracing::warn!("Event store flush failed (non-fatal): {}", e);
                    }
                }
            }
        }
    }
    
    async fn drain_batch(&mut self) {
        let batch = self.receiver.try_recv_batch(self.batch_size);
        
        if batch.is_empty() {
            return;
        }
        
        // Write batch to store
        for event in batch {
            if let Err(e) = self.store.append(&event).await {
                tracing::warn!("Event store write failed (non-fatal): {}", e);
                // Continue - don't let one bad event stop the drain
            }
        }
    }
}

/// Spawn drain task with automatic restart on panic
pub fn spawn_drain_task(receiver: EventReceiver, store: EventStore) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let drain = EventDrain::new(receiver.clone(), store.clone());
            
            if let Err(e) = tokio::spawn(drain.run()).await {
                tracing::error!("Drain task panicked, restarting: {:?}", e);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    })
}
```

### 3.4 Event Store (Append-Only)

```rust
// rust/src/events/store.rs

use std::path::PathBuf;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncWriteExt, BufWriter};

/// Append-only event store
/// 
/// Uses JSONL format for simplicity and grep-ability
#[derive(Clone)]
pub struct EventStore {
    path: PathBuf,
    // Writer is created lazily and managed per-task
}

impl EventStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
    
    /// Append event to store
    pub async fn append(&self, event: &DslEvent) -> Result<(), std::io::Error> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;
        
        let line = serde_json::to_string(event)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        
        file.write_all(line.as_bytes()).await?;
        file.write_all(b"\n").await?;
        
        Ok(())
    }
    
    /// Flush to disk
    pub async fn flush(&self) -> Result<(), std::io::Error> {
        // With append mode, each write is already flushed
        // This is a hook for future batching optimization
        Ok(())
    }
}

/// Alternative: Database-backed event store
pub struct DbEventStore {
    pool: sqlx::PgPool,
}

impl DbEventStore {
    pub async fn append(&self, event: &DslEvent) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            INSERT INTO events.log (timestamp, session_id, event_type, payload)
            VALUES ($1, $2, $3, $4)
            "#,
            event.timestamp,
            event.session_id,
            event.payload.event_type_str(),
            serde_json::to_value(&event.payload).unwrap_or_default(),
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
}
```

### 3.5 Executor Integration (Minimal)

```rust
// rust/src/dsl_v2/executor.rs

use crate::events::{DslEvent, EventEmitter};

pub struct DslExecutor {
    // ... existing fields ...
    
    /// Event emitter - if None, no events emitted
    /// This is the ONLY addition to executor state
    events: Option<Arc<EventEmitter>>,
}

impl DslExecutor {
    pub async fn execute(&self, cmd: &DslCommand, session: &mut Session) -> Result<Value> {
        let start = std::time::Instant::now();
        
        // Execute command (unchanged)
        let result = self.execute_inner(cmd, session).await;
        
        let duration_ms = start.elapsed().as_millis() as u64;
        
        // Emit event - ONE LINE, never blocks, never fails
        if let Some(ref events) = self.events {
            match &result {
                Ok(_) => events.emit(DslEvent::succeeded(
                    session.id(),
                    cmd.verb.full_name.clone(),
                    duration_ms,
                )),
                Err(e) => events.emit(DslEvent::failed(
                    session.id(),
                    cmd.verb.full_name.clone(),
                    duration_ms,
                    e.as_ref(),
                    e.source_id(),
                    e.http_status(),
                )),
            }
        }
        
        result  // Return unchanged
    }
}
```

---

## Part 4: Session Log

### 4.1 Schema

```sql
-- migrations/023a_events.sql

-- Event log (append-only, minimal indexes for write performance)
CREATE TABLE events.log (
    id BIGSERIAL PRIMARY KEY,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    session_id UUID,
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL
);

-- Only index we need for writes (partition key for future partitioning)
CREATE INDEX idx_events_timestamp ON events.log (timestamp);

-- Session log (conversation context)
CREATE TABLE sessions.log (
    id BIGSERIAL PRIMARY KEY,
    session_id UUID NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    entry_type TEXT NOT NULL,  -- 'user_input', 'dsl_command', 'response', 'error', 'agent_thought'
    content TEXT NOT NULL,
    
    -- Link to event (for DSL commands)
    event_id BIGINT,
    
    -- Source
    source TEXT NOT NULL,  -- 'repl', 'egui', 'mcp', 'api'
    
    -- Optional metadata
    metadata JSONB DEFAULT '{}'
);

CREATE INDEX idx_session_log_session ON sessions.log (session_id, timestamp);
```

### 4.2 Session Logger

```rust
// rust/src/events/session_log.rs

use sqlx::PgPool;
use uuid::Uuid;

/// Session logger - captures conversation context
/// 
/// This is called from REPL/egui/MCP, not from executor
pub struct SessionLogger {
    pool: PgPool,
    session_id: Uuid,
    source: String,
}

impl SessionLogger {
    pub fn new(pool: PgPool, session_id: Uuid, source: &str) -> Self {
        Self {
            pool,
            session_id,
            source: source.to_string(),
        }
    }
    
    /// Log user input
    pub async fn log_user_input(&self, content: &str) -> Result<i64> {
        self.log_entry("user_input", content, None).await
    }
    
    /// Log agent thought (for agent mode)
    pub async fn log_agent_thought(&self, content: &str) -> Result<i64> {
        self.log_entry("agent_thought", content, None).await
    }
    
    /// Log DSL command (links to event)
    pub async fn log_dsl_command(&self, content: &str, event_id: Option<i64>) -> Result<i64> {
        self.log_entry("dsl_command", content, event_id).await
    }
    
    /// Log response
    pub async fn log_response(&self, content: &str) -> Result<i64> {
        self.log_entry("response", content, None).await
    }
    
    /// Log error
    pub async fn log_error(&self, content: &str, event_id: Option<i64>) -> Result<i64> {
        self.log_entry("error", content, event_id).await
    }
    
    async fn log_entry(&self, entry_type: &str, content: &str, event_id: Option<i64>) -> Result<i64> {
        let id = sqlx::query_scalar!(
            r#"
            INSERT INTO sessions.log (session_id, entry_type, content, event_id, source)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id
            "#,
            self.session_id,
            entry_type,
            content,
            event_id,
            self.source,
        )
        .fetch_one(&self.pool)
        .await?;
        
        Ok(id)
    }
}
```

---

## Part 5: Startup & Configuration

### 5.1 Initialization

```rust
// rust/src/events/mod.rs

pub mod types;
pub mod emitter;
pub mod drain;
pub mod store;
pub mod session_log;

pub use types::*;
pub use emitter::*;
pub use session_log::*;

/// Initialize event infrastructure
/// 
/// Returns emitter for executor and spawns drain task
pub fn init_events(config: &EventConfig) -> Option<Arc<EventEmitter>> {
    if !config.enabled {
        tracing::info!("Event infrastructure disabled");
        return None;
    }
    
    let (emitter, receiver) = EventEmitter::new(config.buffer_size);
    let emitter = Arc::new(emitter);
    
    // Create store
    let store = match &config.store {
        StoreConfig::File { path } => EventStore::new(path.clone()),
        StoreConfig::Database { pool } => EventStore::new_db(pool.clone()),
    };
    
    // Spawn drain task (runs forever in background)
    drain::spawn_drain_task(receiver, store);
    
    tracing::info!(
        buffer_size = config.buffer_size,
        "Event infrastructure initialized"
    );
    
    Some(emitter)
}

#[derive(Debug, Clone)]
pub struct EventConfig {
    pub enabled: bool,
    pub buffer_size: usize,
    pub store: StoreConfig,
}

impl Default for EventConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            buffer_size: 4096,
            store: StoreConfig::File {
                path: PathBuf::from("data/events.jsonl"),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub enum StoreConfig {
    File { path: PathBuf },
    Database { pool: PgPool },
}
```

### 5.2 Feature Flag

```toml
# Cargo.toml

[features]
default = ["events"]
events = []  # Can disable for minimal builds
```

```rust
// In executor, compiled out if feature disabled
#[cfg(feature = "events")]
if let Some(ref events) = self.events {
    events.emit(event);
}
```

---

## Part 6: Monitoring

### 6.1 Health Check

```rust
// rust/src/events/health.rs

impl EventEmitter {
    /// Health check for monitoring
    pub fn health(&self) -> EventHealth {
        let stats = self.stats();
        let drop_rate = if stats.emitted > 0 {
            stats.dropped as f64 / (stats.emitted + stats.dropped) as f64
        } else {
            0.0
        };
        
        EventHealth {
            status: if drop_rate < 0.01 { "healthy" } else { "degraded" },
            emitted: stats.emitted,
            dropped: stats.dropped,
            drop_rate,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct EventHealth {
    pub status: &'static str,
    pub emitted: u64,
    pub dropped: u64,
    pub drop_rate: f64,
}
```

### 6.2 REPL Command

```rust
// In REPL handler

":events" => {
    if let Some(ref events) = executor.events {
        let health = events.health();
        println!("Event Infrastructure Status");
        println!("  Status:    {}", health.status);
        println!("  Emitted:   {}", health.emitted);
        println!("  Dropped:   {}", health.dropped);
        println!("  Drop Rate: {:.2}%", health.drop_rate * 100.0);
    } else {
        println!("Event infrastructure not enabled");
    }
}
```

---

## Part 7: Implementation Summary

### 7.1 Files to Create

```
rust/src/events/
├── mod.rs           # Public API, init_events()
├── types.rs         # DslEvent, EventPayload, ErrorSnapshot
├── emitter.rs       # EventEmitter (lock-free)
├── drain.rs         # EventDrain (background task)
├── store.rs         # EventStore (append-only)
├── session_log.rs   # SessionLogger
└── health.rs        # Health check

migrations/
└── 023a_events.sql  # events.log, sessions.log tables
```

### 7.2 Effort Estimate

| Task | Hours |
|------|-------|
| Event types | 1h |
| Lock-free emitter | 2h |
| Drain task | 2h |
| Event store (file) | 1h |
| Event store (DB) | 1h |
| Session logger | 2h |
| Executor integration | 1h |
| Health/monitoring | 1h |
| Tests | 2h |
| **Total** | **~13h** |

### 7.3 Dependencies

```toml
[dependencies]
crossbeam-channel = "0.5"  # Lock-free channels
```

---

## Part 8: Performance Validation

### 8.1 Benchmark

```rust
#[cfg(test)]
mod bench {
    use super::*;
    use std::time::Instant;
    
    #[test]
    fn bench_emit_overhead() {
        let (emitter, _receiver) = EventEmitter::new(4096);
        
        let iterations = 100_000;
        let start = Instant::now();
        
        for _ in 0..iterations {
            emitter.emit(DslEvent::succeeded(
                Some(Uuid::new_v4()),
                "test.verb".to_string(),
                100,
            ));
        }
        
        let elapsed = start.elapsed();
        let per_emit = elapsed / iterations;
        
        println!("Per emit: {:?}", per_emit);
        assert!(per_emit.as_nanos() < 1000, "Emit should be < 1μs");
    }
}
```

---

## Summary

**023a provides:**
- Always-on event emission (~500ns overhead per command)
- Session context capture for later analysis
- Zero impact on DSL pipeline reliability
- Graceful degradation (drops events, never blocks)

**023b will provide:**
- On-demand analysis of captured events
- Classification, fingerprinting, repro generation
- MCP interface for Claude
- Audit trail

The two are completely decoupled. 023a runs always. 023b starts when you want to inspect failures.
