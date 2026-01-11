# 025: Implement Event Infrastructure (023a)

> **Status:** ✅ COMPLETE
> **Completed:** 2026-01-11
> **Design:** `ai-thoughts/023a-event-infrastructure.md`
> **Tests:** 589 passing (shared with 026)

---

## Objective

Implement always-on, zero-overhead event capture from DSL pipeline. This is the **only** component that touches the executor.

**Critical constraint:** Must not impact DSL performance or reliability.

---

## Implementation Tasks

### Phase 1: Core Types (2h)

- [ ] Create `rust/src/events/mod.rs`
- [ ] Create `rust/src/events/types.rs`:
  ```rust
  pub struct DslEvent {
      pub timestamp: DateTime<Utc>,
      pub session_id: Option<Uuid>,
      pub payload: EventPayload,
  }
  
  pub enum EventPayload {
      CommandSucceeded { verb: String, duration_ms: u64 },
      CommandFailed { verb: String, duration_ms: u64, error: ErrorSnapshot },
      SessionStarted { source: SessionSource },
      SessionEnded { command_count: u32, error_count: u32, duration_secs: u64 },
  }
  
  pub struct ErrorSnapshot {
      pub error_type: String,
      pub message: String,  // Truncated to 500 chars
      pub source_id: Option<String>,
      pub http_status: Option<u16>,
      pub verb: String,
  }
  
  pub enum SessionSource { Repl, Egui, Mcp, Api }
  ```
- [ ] Implement `DslEvent::succeeded()` and `DslEvent::failed()` constructors
- [ ] Ensure message truncation in `failed()` to avoid large allocations

### Phase 2: Lock-Free Emitter (3h)

- [ ] Add `crossbeam-channel = "0.5"` to Cargo.toml
- [ ] Create `rust/src/events/emitter.rs`:
  ```rust
  pub struct EventEmitter {
      sender: crossbeam_channel::Sender<DslEvent>,
      events_emitted: AtomicU64,
      events_dropped: AtomicU64,
  }
  
  impl EventEmitter {
      pub fn new(buffer_size: usize) -> (Self, EventReceiver);
      
      /// MUST: < 1μs, never block, never panic, never fail
      #[inline]
      pub fn emit(&self, event: DslEvent);
      
      pub fn stats(&self) -> EmitterStats;
  }
  ```
- [ ] Implement `try_send` with silent drop on full buffer
- [ ] Implement `EventReceiver` for drain task
- [ ] Add `try_recv_batch()` for efficient draining
- [ ] **Write benchmark test**: Verify emit < 1μs

### Phase 3: Drain Task (2h)

- [ ] Create `rust/src/events/drain.rs`:
  ```rust
  pub struct EventDrain {
      receiver: EventReceiver,
      store: EventStore,
      batch_size: usize,
      flush_interval: Duration,
  }
  
  impl EventDrain {
      pub async fn run(self);  // Runs forever in background
  }
  
  pub fn spawn_drain_task(receiver: EventReceiver, store: EventStore) -> JoinHandle<()>;
  ```
- [ ] Implement batch draining with periodic flush
- [ ] Implement automatic restart on panic
- [ ] Ensure drain task never blocks executor

### Phase 4: Event Store (2h)

- [ ] Create `rust/src/events/store.rs`
- [ ] Implement file-based store (JSONL):
  ```rust
  pub struct EventStore {
      path: PathBuf,
  }
  
  impl EventStore {
      pub async fn append(&self, event: &DslEvent) -> Result<()>;
      pub async fn flush(&self) -> Result<()>;
  }
  ```
- [ ] Implement DB-based store (optional, for later):
  ```rust
  pub struct DbEventStore {
      pool: PgPool,
  }
  ```
- [ ] Create migration `migrations/023a_events.sql`:
  ```sql
  CREATE TABLE events.log (
      id BIGSERIAL PRIMARY KEY,
      timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
      session_id UUID,
      event_type TEXT NOT NULL,
      payload JSONB NOT NULL
  );
  ```

### Phase 5: Session Logger (2h)

- [ ] Create `rust/src/events/session_log.rs`:
  ```rust
  pub struct SessionLogger {
      pool: PgPool,
      session_id: Uuid,
      source: String,
  }
  
  impl SessionLogger {
      pub async fn log_user_input(&self, content: &str) -> Result<i64>;
      pub async fn log_agent_thought(&self, content: &str) -> Result<i64>;
      pub async fn log_dsl_command(&self, content: &str, event_id: Option<i64>) -> Result<i64>;
      pub async fn log_response(&self, content: &str) -> Result<i64>;
      pub async fn log_error(&self, content: &str, event_id: Option<i64>) -> Result<i64>;
  }
  ```
- [ ] Add to migration:
  ```sql
  CREATE TABLE sessions.log (
      id BIGSERIAL PRIMARY KEY,
      session_id UUID NOT NULL,
      timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
      entry_type TEXT NOT NULL,
      content TEXT NOT NULL,
      event_id BIGINT,
      source TEXT NOT NULL,
      metadata JSONB DEFAULT '{}'
  );
  CREATE INDEX idx_session_log_session ON sessions.log (session_id, timestamp);
  ```

### Phase 6: Executor Integration (1h)

- [ ] Add to `DslExecutor`:
  ```rust
  pub struct DslExecutor {
      // ... existing fields ...
      events: Option<Arc<EventEmitter>>,
  }
  ```
- [ ] Add to `execute()` method - **ONLY THESE LINES**:
  ```rust
  let start = Instant::now();
  let result = self.execute_inner(cmd, session).await;
  let duration_ms = start.elapsed().as_millis() as u64;
  
  if let Some(ref events) = self.events {
      events.emit(match &result {
          Ok(_) => DslEvent::succeeded(session.id(), cmd.verb.full_name.clone(), duration_ms),
          Err(e) => DslEvent::failed(session.id(), cmd.verb.full_name.clone(), duration_ms, e),
      });
  }
  
  result
  ```
- [ ] Add `events` feature flag to Cargo.toml
- [ ] Make injection compile-out when feature disabled

### Phase 7: Initialization & Config (1h)

- [ ] Create `rust/src/events/config.rs`:
  ```rust
  pub struct EventConfig {
      pub enabled: bool,
      pub buffer_size: usize,  // default: 4096
      pub store: StoreConfig,
  }
  
  pub enum StoreConfig {
      File { path: PathBuf },
      Database { pool: PgPool },
  }
  ```
- [ ] Create `init_events()` function:
  ```rust
  pub fn init_events(config: &EventConfig) -> Option<Arc<EventEmitter>> {
      if !config.enabled { return None; }
      let (emitter, receiver) = EventEmitter::new(config.buffer_size);
      let store = EventStore::new(config.store.path.clone());
      spawn_drain_task(receiver, store);
      Some(Arc::new(emitter))
  }
  ```

### Phase 8: Health & Monitoring (1h)

- [ ] Add health check to `EventEmitter`:
  ```rust
  pub fn health(&self) -> EventHealth {
      let stats = self.stats();
      EventHealth {
          status: if drop_rate < 0.01 { "healthy" } else { "degraded" },
          emitted: stats.emitted,
          dropped: stats.dropped,
          drop_rate,
      }
  }
  ```
- [ ] Add `:events` REPL command to show health
- [ ] Add logging for dropped events (warn level, throttled)

### Phase 9: Tests (2h)

- [ ] Unit test: `EventEmitter::emit()` never panics
- [ ] Unit test: `EventEmitter::emit()` < 1μs (benchmark)
- [ ] Unit test: Buffer full → events dropped, counter incremented
- [ ] Unit test: Receiver disconnected → events dropped gracefully
- [ ] Integration test: Events flow to store
- [ ] Integration test: Session logger captures context

---

## Verification

After implementation:

```bash
# 1. Run benchmarks - emit must be < 1μs
cargo bench --bench event_emit

# 2. Run tests
cargo test events::

# 3. Manual test in REPL
:events  # Should show health stats

# 4. Verify DSL still works with events disabled
cargo run --no-default-features  # Should compile without events feature
```

---

## Files to Create

```
rust/src/events/
├── mod.rs           # pub use, init_events()
├── types.rs         # DslEvent, EventPayload, ErrorSnapshot
├── emitter.rs       # EventEmitter (lock-free)
├── drain.rs         # EventDrain, spawn_drain_task()
├── store.rs         # EventStore (file + DB)
├── session_log.rs   # SessionLogger
├── config.rs        # EventConfig
└── health.rs        # EventHealth

migrations/
└── 023a_events.sql

rust/benches/
└── event_emit.rs    # Performance benchmark
```

---

## Dependencies to Add

```toml
# Cargo.toml
[dependencies]
crossbeam-channel = "0.5"

[features]
default = ["events"]
events = []
```

---

## Critical Constraints

1. **`emit()` MUST be < 1μs** - Benchmark enforced
2. **`emit()` MUST never block** - Use `try_send`, not `send`
3. **`emit()` MUST never panic** - Wrap everything
4. **`emit()` MUST never return error** - Returns `()`
5. **Buffer full = drop event** - Never backpressure executor
6. **Drain task is separate** - Own tokio task, own error handling
7. **Feature flag** - Can compile out entirely

---

## Next

After this TODO: Implement 026 (Feedback Inspector - 023b)
