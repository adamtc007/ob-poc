# BPMN-Lite — Detailed Annex

> This annex covers the BPMN-Lite fiber VM, process execution model, race semantics,
> gRPC interface, inclusive gateway, PostgresProcessStore, authoring pipeline,
> and ob-poc integration (12 modules).
> For the high-level overview see the root `CLAUDE.md`.

---

## Overview

BPMN-Lite is a lightweight durable workflow orchestration engine. It executes long-running workflows (KYC document solicitation, multi-step approvals) via a **fiber VM** backed by PostgreSQL event sourcing. It is a **standalone workspace** with a gRPC boundary between it and ob-poc.

**Design principles:**
- **Deterministic:** 18-opcode bytecode ISA, no external control flow
- **Durable:** Every state change appended to event log (full PITR)
- **Observable:** Audit trail of fibers, waits, signals, incidents
- **Decoupled:** gRPC boundary separates workflow engine from domain verbs

### Workspace Structure

```
bpmn-lite/                              # Standalone Rust workspace
├── bpmn-lite-core/
│   ├── src/
│   │   ├── vm.rs                       # Fiber execution engine (batched event emission)
│   │   ├── engine.rs                   # BpmnLiteEngine facade (tick, compile, start, signal)
│   │   ├── store.rs                    # ProcessStore trait (tenant-aware + batch append)
│   │   ├── store_memory.rs             # In-memory implementation (testing)
│   │   ├── store_postgres.rs           # PostgreSQL backend (production)
│   │   ├── types.rs                    # Fiber, ProcessInstance, 18 Instr variants
│   │   ├── events.rs                   # RuntimeEvent enum
│   │   └── authoring/                  # YAML → DTO → IR → Bytecode pipeline
│   │       ├── dto.rs                  # WorkflowGraphDto
│   │       ├── validate.rs             # DTO contract enforcement
│   │       ├── yaml.rs                 # YAML deserialization
│   │       ├── publish.rs              # Atomic compile + persist
│   │       ├── lints.rs                # Structural warnings
│   │       └── registry.rs             # TemplateStore trait
│   └── migrations/                     # 14 SQL migrations (001–014)
├── bpmn-lite-server/
│   ├── src/
│   │   ├── grpc.rs                     # BpmnLiteService impl
│   │   ├── load_harness.rs             # Reusable concurrent workflow stress harness
│   │   └── main.rs                     # Server bootstrap with configurable bind address
│   └── proto/bpmn_lite/v1/bpmn_lite.proto
├── xtask/
│   └── src/main.rs                     # smoke/stress orchestration, optional server spawn
└── Cargo.toml

rust/src/bpmn_integration/              # ob-poc ↔ bpmn-lite bridge (12 modules)
├── types.rs                            # ExecutionRoute, CorrelationRecord, JobFrame, ParkedToken
├── config.rs                           # WorkflowBinding registry
├── client.rs                           # BpmnLiteConnection gRPC client
├── dispatcher.rs                       # WorkflowDispatcher: DslExecutorV2 impl
├── worker.rs                           # JobWorker: long-poll + verb execution
├── event_bridge.rs                     # SubscribeEvents stream consumer
├── signal_relay.rs                     # Outcome → REPL orchestrator signaling
├── correlation.rs                      # CorrelationStore
├── parked_tokens.rs                    # ParkedTokenStore
├── job_frames.rs                       # JobFrameStore (dedupe + retry)
├── pending_dispatches.rs               # PendingDispatchStore (resilience queue)
└── pending_dispatch_worker.rs          # Background retry task (50 max attempts)
```

**Run standalone:**
```bash
cargo x bpmn-lite build
cargo x bpmn-lite test
cargo x bpmn-lite start --database-url postgresql:///data_designer
cd bpmn-lite && cargo run -p xtask -- smoke --spawn-server
cd bpmn-lite && cargo run -p xtask -- stress --spawn-server --instances 300 --workers 16
```

### Current hardening status

- Job completion now treats the worker-supplied completion hash as an expected/current snapshot hash only.
- The canonical persisted payload hash is recomputed from the returned payload before instance state and payload history are written.
- VM event emission is batched through `ProcessStore::batch_append_events()`.
- `ProcessInstance.domain_payload` is stored as `Arc<str>`.
- Store operations that scan instances or dequeue jobs are tenant-scoped.
- `Inspect` returns real `bytecode_version` and payload-hash metadata.

---

## Fiber VM

### Fiber Concept

A **fiber** is a stackless lightweight execution thread within a process instance. The VM schedules and multiplexes fibers manually — no OS threads. Same bytecode + same input = same execution trace.

### 18-Opcode ISA

| Category | Opcodes |
|----------|---------|
| Control flow | `Jump`, `BrIf`, `BrIfNot` |
| Stack ops | `PushBool`, `PushI64`, `Pop` |
| Flags | `LoadFlag`, `StoreFlag` |
| Work | `ExecNative` (job dispatch) |
| Concurrency | `Fork`, `Join` |
| Wait primitives | `WaitFor` (relative), `WaitUntil` (absolute), `WaitMsg` |
| Race semantics | `WaitAny`, `CancelWait` |
| Bounded loops | `IncCounter`, `BrCounterLt` |
| Inclusive gateway | `ForkInclusive`, `JoinDynamic` |
| Lifecycle | `End`, `EndTerminate`, `Fail` |

### Fiber State Machine

```
Running → (wait instr) → Parked { Timer | Msg | Job | Join | Race | Incident }
Parked  → (event fires) → Running
Running → (End) → Deleted
Running → (EndTerminate) → process terminated, all other fibers cancelled
```

### Core Types

```rust
pub struct Fiber {
    pub fiber_id: Uuid,
    pub pc: Addr,
    pub stack: Vec<Value>,
    pub regs: [Value; 8],
    pub wait: WaitState,
    pub loop_epoch: u32,           // Incremented by IncCounter (for job key uniqueness)
}

pub enum WaitState {
    Running,
    Timer { deadline_ms: u64 },
    Msg { wait_id: WaitId, name: u32, corr_key: Value },
    Job { job_key: String },
    Join { join_id: JoinId },
    Race { race_id: RaceId, timer_deadline_ms: Option<u64>, job_key: Option<String>,
           interrupting: bool, timer_arm_index: Option<usize>,
           cycle_remaining: Option<u32>, cycle_fired_count: u32 },
    Incident { incident_id: Uuid },
}

pub enum Value {
    Bool(bool),
    I64(i64),
    Str(u32),       // Interned string index
    Ref(u32),       // Opaque external store handle
}

pub enum ProcessState {
    Running,
    Completed { at: Timestamp },
    Cancelled { reason: String, at: Timestamp },
    Terminated { at: Timestamp },
    Failed { incident_id: Uuid },
}
```

### Completion hash invariant

After a successful `CompleteJob` call:

- the worker-supplied hash must match the instance payload snapshot that was activated
- `instance.domain_payload` is replaced with the returned payload
- `instance.domain_payload_hash` is recomputed from that returned payload
- payload history is written with the same recomputed hash

There is no valid path where a new payload is persisted with an old hash.

### Fork / Join (Token Passing)

**Example:** two parallel branches synchronized at a join.

```
Fork { targets: [10, 20] }
10: ExecNative                     # Branch A
12: Join { id: 1, expected: 2, next: 30 }
20: ExecNative                     # Branch B
22: Join { id: 1, expected: 2, next: 30 }
30: ExecNative                     # After join
31: End
```

Each branch executes independently. When both reach `Join { id: 1 }` and the barrier counter reaches `expected: 2`, the last-arriving fiber advances to `next: 30`. Others are consumed.

---

## Race Semantics

### Boundary Timer: Interrupting vs Non-Interrupting

**Interrupting (default):** When timer fires, cancel the job and jump to escalation path.
**Non-Interrupting:** When timer fires, fork a parallel child fiber — original task continues.

**Promotion pass (engine.rs::tick_instance):** For each `WaitState::Job` fiber, check `program.boundary_map[fiber.pc]`. If a race entry exists, promote fiber to `WaitState::Race` preserving `job_key`.

**Race check pass:** If `now >= timer_deadline_ms`:
- Interrupting → `vm.resolve_race()` — win arm advances, other arms emit `WaitCancelled`, job acknowledged
- Non-interrupting → fork child fiber at `resume_at`, original stays parked

### Ghost Signals (Correlation + Parked Tokens)

A ghost signal arrives after its target fiber is already deleted/resumed.

**Prevention:** Triple-part correlation key: `"{runbook_id}:{entry_id}:{correlation_id}"`.

**Dead-letter queue:** If a signal arrives and no fiber is waiting, store it. Next `WaitMsg` with matching `(name, corr_key)` claims it immediately.

---

## gRPC Interface (7 RPCs)

**Proto:** `bpmn-lite-server/proto/bpmn_lite/v1/bpmn_lite.proto`

```protobuf
service BpmnLite {
    rpc Compile(CompileRequest)          returns (CompileResponse);
    rpc StartProcess(StartRequest)       returns (StartResponse);
    rpc Signal(SignalRequest)            returns (SignalResponse);
    rpc Cancel(CancelRequest)            returns (CancelResponse);
    rpc Inspect(InspectRequest)          returns (InspectResponse);
    rpc ActivateJobs(ActivateJobsRequest) returns (stream JobActivationMsg);
    rpc CompleteJob(CompleteJobRequest)  returns (CompleteJobResponse);
    rpc FailJob(FailJobRequest)          returns (FailJobResponse);
    rpc SubscribeEvents(SubscribeRequest) returns (stream LifecycleEvent);
}
```

### Key Messages

**StartRequest / StartResponse:**
```protobuf
message StartRequest {
    string process_key = 1;
    bytes bytecode_version = 2;          // SHA-256 hash
    string domain_payload = 3;           // Canonical JSON (opaque to engine)
    bytes domain_payload_hash = 4;
    map<string, ProtoValue> orch_flags = 5;
    string correlation_id = 6;
}
message StartResponse {
    string process_instance_id = 1;
}
```

**JobActivationMsg (streaming):**
```protobuf
message JobActivationMsg {
    string job_key = 1;                  // Idempotency key
    string process_instance_id = 2;
    string task_type = 3;                // e.g. "create_case_record"
    string domain_payload = 5;           // Unchanged from StartProcess
    map<string, ProtoValue> orch_flags = 7;
    int32 retries_remaining = 8;
}
```

`domain_payload_hash` on activation is the hash of the currently persisted instance payload snapshot the worker is reading.

**CompleteJobRequest semantics:**
- request `domain_payload_hash` is the optimistic-concurrency guard hash for the worker snapshot
- it is not trusted as the canonical hash of the new returned payload
- the server recomputes the stored payload hash from `domain_payload`

**FailJobRequest:**
```protobuf
message FailJobRequest {
    string job_key = 1;
    string error_class = 2;              // "transient" | "contract_violation" | "business_rejection"
    string message = 3;
    int64 retry_hint_ms = 4;
}
```

---

## Inclusive Gateway: ForkInclusive & JoinDynamic

**Problem:** OR semantics — zero, one, or multiple branches taken at runtime.

```rust
Instr::ForkInclusive {
    branches: Box<[InclusiveBranch]>,    // (condition_flag?, target) pairs
    join_id: JoinId,
    default_target: Option<Addr>,
}

Instr::JoinDynamic {
    id: JoinId,
    next: Addr,
}
```

**Execution:**
1. Evaluate all branch conditions against current flags
2. Collect `taken_targets` (e.g., branches A, B, D → count = 3)
3. Record `instance.join_expected.insert(join_id, 3)`
4. Spawn 3 child fibers; delete parent
5. Each branch executes, hits `JoinDynamic { id }`
6. Increments barrier counter; if count ≥ expected → release to `next`

**No condition matched + no default** → incident (ContractViolation), process fails.

---

## PostgresProcessStore — Event Sourcing

### 13 Migrations

| Migration | Table | Purpose |
|-----------|-------|---------|
| 001 | `process_instances` | Instance snapshots (flags, counters, join_expected, state) |
| 002 | `fibers` | Fiber snapshots (PC, stack, regs, wait state) |
| 003 | `join_barriers` | Join barrier arrival counters |
| 004 | `dedupe_cache` | Job completion cache (prevent re-execution, TTL-prunable via `created_at`) |
| 005 | `job_queue` | External job queue (pending/claimed/completed, `claimed_at` for timeout reclaim) |
| 006 | `compiled_programs` | Bytecode cache (keyed by SHA-256) |
| 007 | `dead_letter_queue` | TTL-based message store |
| 008 | `event_sequences` | Per-instance sequence counter |
| 009 | `event_log` | Append-only audit trail |
| 010 | `payload_history` | Payload versioning (PITR) |
| 011 | `incidents` | Unrecoverable error states |
| 012 | `updated_at_trigger` | Automatic timestamp maintenance |
| 013 | `workflow_templates` | Published workflow templates |

### ProcessStore Trait (33 methods)

Key method groups: instance save/load/state-update, fiber CRUD, join barriers, dedupe cache, job queue, program store, dead-letter queue, event log, payload history, incidents.

**Compound atomic methods** (added for crash safety):
- `atomic_start(instance, root_fiber, event)` — atomically saves instance + root fiber + InstanceStarted event in one transaction
- `atomic_complete(instance, completion)` — atomically saves instance + dedupe entry + payload version in one transaction

**Housekeeping methods** (used by background tasks):
- `reclaim_stale_jobs(timeout_ms)` — reclaims jobs stuck in `claimed` state past timeout
- `prune_dedupe_cache(older_than_ms)` — prunes dedupe entries older than TTL
- `list_running_instances()` — lists all non-terminal instance IDs for timer tick orchestration

### PITR (Point-in-Time Recovery)

Rebuild instance state from event log at any historical sequence number. Payload versions stored separately (keyed by SHA-256 hash) for historical reconstruction.

---

## Authoring Pipeline: YAML → Bytecode

```
YAML → WorkflowGraphDto (parse) → validate → lint → IR → verify → bytecode → SHA-256 hash → publish
```

**Files:** `bpmn-lite-core/src/authoring/`

**WorkflowGraphDto elements:**
```rust
pub enum Element {
    StartEvent { id, outgoing },
    Task / ServiceTask { id, task_type, boundary? },
    ExclusiveGateway { id, conditions: Vec<SequenceFlow> },
    InclusiveGateway { id },
    ParallelGateway { id },
    BoundaryEvent { id, task_id, trigger: BoundaryTrigger },
    EndEvent { id },
}

pub enum BoundaryTrigger {
    Timer { duration_ms: u64 },
    Message { name: String, correlation_var: String },
    Signal { name: String },
}
```

**Bytecode verifier checks:**
- Bounded loops: every `BrCounterLt` must have a reachable `IncCounter` on the loop path
- No jumps past program end
- Stack balance at join points

**`publish_workflow()` is atomic:** writes program store idempotently (keyed by bytecode hash), writes Draft template first then transitions to Published. On failure, no orphaned draft rows.

---

## Bounded Loops: IncCounter & BrCounterLt

**Idempotency with loops:** Each loop iteration gets a unique job key via `fiber.loop_epoch`.

```
ExecNative { task_type: 0 }              # Task A
IncCounter { counter_id: 0 }
BrCounterLt { counter_id: 0, limit: 3, target: 0 }  # Loop back if < 3
ExecNative { task_type: 1 }              # Task B (exit)
End
```

**Job key per iteration:** `"{instance_id}:{service_task_id}:{pc}:{loop_epoch}"`
- Iteration 1: `...task_a:0:1`
- Iteration 2: `...task_a:0:2`
- Each independently deduplicatable.

---

## Error Handling

### Error Classes

```rust
pub enum ErrorClass {
    Transient,                                           // Retry immediately
    ContractViolation,                                   // Config error → incident
    BusinessRejection { rejection_code: String },        // Domain logic rejection
}
```

### Error Routes (Catch Boundaries)

```rust
pub struct ErrorRoute {
    pub error_code: Option<String>,  // None = catch-all
    pub resume_at: Addr,
    pub boundary_element_id: String,
}
// program.error_route_map: HashMap<Addr (task PC), Vec<ErrorRoute>>
```

**Matching logic:** `BusinessRejection { rejection_code }` matches routes by `rejection_code == error_code`. `None` error_code = catch-all.

**No match → Incident:** fiber parks on `WaitState::Incident`. Process state → `Failed`. Operator resolves manually (reset, flag override, escalate).

### EndTerminate

Immediately terminates all fibers in the process: emit `WaitCancelled` for all waiting fibers, cancel pending jobs, delete all fibers, set state to `Terminated`.

---

## ob-poc Integration (12 Modules)

### Architecture

```
REPL orchestrator
  → WorkflowDispatcher (dispatcher.rs)
       ├── Direct route    → RealDslExecutor (inner)
       └── Orchestrated    → StartProcess gRPC
                                 → CorrelationRecord stored
                                 → ParkedToken created
                                 → DslExecutionOutcome::Parked returned

                           ← JobWorker (worker.rs)
                                 ← ActivateJobs (stream)
                                 → execute verb via executor
                                 → CompleteJob / FailJob

                           ← EventBridge (event_bridge.rs)
                                 ← SubscribeEvents (stream)
                                 → resolve ParkedTokens
                                 → emit OutcomeEvent

                           ← SignalRelay (signal_relay.rs)
                                 ← OutcomeEvent
                                 → signal REPL orchestrator
                                 → REPL entry resumes
```

### Core Types

```rust
pub enum ExecutionRoute { Direct, Orchestrated }

pub struct CorrelationRecord {
    pub correlation_id: Uuid,
    pub process_instance_id: Uuid,
    pub session_id: Uuid,
    pub runbook_id: Uuid,
    pub entry_id: Uuid,
    pub process_key: String,
    pub domain_payload_hash: Vec<u8>,
    pub status: CorrelationStatus,           // Active | Completed | Failed | Cancelled
    pub domain_correlation_key: Option<String>,
}

pub struct ParkedToken {
    pub token_id: Uuid,
    pub correlation_key: String,             // "{runbook_id}:{entry_id}"
    pub session_id: Uuid,
    pub entry_id: Uuid,
    pub process_instance_id: Uuid,
    pub expected_signal: String,
    pub status: ParkedTokenStatus,           // Waiting | Resolved | TimedOut | Cancelled
}

pub struct PendingDispatch {
    pub dispatch_id: Uuid,
    pub payload_hash: Vec<u8>,               // Idempotency key
    pub verb_fqn: String,
    pub process_key: String,
    pub domain_payload: String,
    pub correlation_id: Uuid,                // Stable across retries
    pub status: PendingDispatchStatus,       // Pending | Dispatched | FailedPermanent
    pub attempts: i32,                       // Max 50 before FailedPermanent
}
```

### WorkflowDispatcher

**DslExecutorV2** implementation. Checks `WorkflowConfigIndex` to route each verb call:
1. Direct → delegate to inner executor
2. Orchestrated:
   - Canonicalize DSL args to JSON payload + SHA-256 hash
   - Call `StartProcess` gRPC
   - On success: save `CorrelationRecord`, create `ParkedToken`, return `Parked`
   - On failure: enqueue `PendingDispatch` (retried by background worker), return `Parked`

### JobWorker

Long-poll loop (`ActivateJobs` stream):
1. Receive job activation
2. Upsert `JobFrame` (dedupe: if already completed, return cached result)
3. Look up task_type → verb mapping
4. Build DSL from domain_payload
5. Execute verb via `executor.execute()`
6. Call `CompleteJob` or `FailJob`

### EventBridge

Subscribes to `SubscribeEvents` stream. Maps BPMN terminal events to ob-poc outcomes:

| BPMN Event | Action |
|------------|--------|
| `Completed` | Resolve parked tokens, update correlation status |
| `Cancelled` | Mark correlation failed, resolve tokens |
| `IncidentCreated` | Mark correlation failed |

Reconnection: exponential backoff 250ms → 30s, up to 10 attempts.

### SignalRelay

Sits between EventBridge and REPL orchestrator. On terminal `OutcomeEvent`:
1. Look up `CorrelationRecord` by `process_instance_id`
2. Reconstruct `correlation_key`
3. Call `orchestrator.signal_completion(correlation_key, status, error)`
4. REPL entry resumes from `Parked` state

### PendingDispatchWorker

Background task scanning `bpmn_pending_dispatches`:
- Batch size: 5 per cycle
- Backoff: skip rows updated < 10s ago
- On success: mark `Dispatched`, patch `process_instance_id` in correlation
- Max 50 attempts then `FailedPermanent`
- Sleep 10s between cycles

---

## CompiledProgram Structure

```rust
pub struct CompiledProgram {
    pub bytecode_version: [u8; 32],          // SHA-256 (version key)
    pub program: Vec<Instr>,
    pub debug_map: BTreeMap<Addr, String>,   // Addr → BPMN element ID
    pub join_plan: BTreeMap<JoinId, JoinPlanEntry>,
    pub wait_plan: BTreeMap<WaitId, WaitPlanEntry>,
    pub race_plan: BTreeMap<RaceId, RacePlanEntry>,
    pub boundary_map: BTreeMap<Addr, RaceId>, // Task PC → boundary race ID
    pub write_set: BTreeMap<String, HashSet<FlagKey>>,
    pub task_manifest: Vec<String>,
    pub error_route_map: BTreeMap<Addr, Vec<ErrorRoute>>,
}
```

---

## Durability & Crash Recovery (2026-04-02)

### Transaction Atomicity

Engine methods that perform multi-step state changes use compound atomic store methods:

- **`atomic_start()`** — instance creation: saves ProcessInstance + root Fiber + InstanceStarted event in a single Postgres transaction. Prevents orphaned instances on crash.
- **`atomic_complete()`** — job completion: saves updated ProcessInstance + dedupe cache entry + payload version in a single transaction. Prevents phantom jobs where ack completes but state isn't updated.

In `MemoryStore`, atomicity is guaranteed by the single `RwLock` write guard.

### Background Housekeeping Tasks (main.rs)

Three `tokio::spawn` background loops run alongside the gRPC server:

| Task | Interval | Purpose |
|------|----------|---------|
| **Job reclaim** | 60s | Reclaims jobs stuck in `claimed` state for >5 minutes (worker crash recovery) |
| **Tick all** | 500ms | Ticks all `Running` instances — ensures timers fire without requiring explicit per-instance tick calls |
| **Dedupe prune** | 1 hour | Prunes dedupe cache entries older than 24 hours (prevents unbounded growth) |

### Job Claim Timeout

The job queue uses a two-phase claim model: `pending` → `claimed` (with `claimed_at` timestamp) → deleted on `ack_job()`. If a worker crashes after claiming but before completing, `reclaim_stale_jobs()` moves the job back to `pending` after the configurable timeout (default 5 minutes). Uses `FOR UPDATE SKIP LOCKED` for concurrent worker safety.

### Timer Tick Orchestrator

`BpmnLiteEngine::tick_all()` queries all `Running` instances via `list_running_instances()` and ticks each. The background loop at 500ms ensures boundary timers, race conditions, and fiber promotions fire reliably at scale without requiring external orchestration.

---

## Key Invariants

| Invariant | Mechanism |
|-----------|-----------|
| Determinism | Bytecode-driven; same input = same trace |
| Durability | Every state change appended to `event_log` |
| Atomicity | `atomic_start()` + `atomic_complete()` — multi-step operations in single transactions |
| Race safety | `WaitState::Race` + boundary timer promotion pass |
| Bounded loops | `IncCounter` + `BrCounterLt`; verifier enforces presence |
| Deduplications | Job keys stable per-iteration via `loop_epoch`; dedupe cache with TTL pruning |
| Ghost signals | Dead-letter queue + `ParkedToken` registry |
| Crash recovery | Job claim timeout (5min) + `tick_all` background poller (500ms) |
| Incident recovery | Manual operator resolution; no auto-retry on ContractViolation |
| Job worker resilience | Long-poll + `PendingDispatchWorker` (50 retries) |
| Canonical payload | SHA-256 of canonical JSON = idempotency across retries |
