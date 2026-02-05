# Verb Execution Model: Sync & Durable Execution Architecture

## Enterprise Onboarding Platform — Core Architecture Enhancement

**Status:** Draft — Redraft incorporating hardening & durability semantics  
**Author:** Architecture Team  
**Date:** February 2026  
**Domain:** KYC/UBO Discovery, Compliance Orchestration

---

## 1. Motivation

The platform’s DSL verb system currently executes everything under a single **synchronous** model: verbs run as compiled Rust handlers and return immediately.

A third operational reality is now unavoidable: **durable, long-running work** that involves external actors and asynchronous signals — requesting documents, waiting for uploads, compliance approvals, regulatory confirmations. These tasks must **start, park, and resume** on an external event that may arrive minutes, days, or weeks later.

We need a first-class model for durable verbs that remains consistent with the existing “agent-runbook + DAG” approach, avoids bolting on workflow seams, and preserves deterministic resumption.

This document proposes a minimal extension:

- **Uniform verb surface** for agents/runbooks (same DSL syntax).
- **Two execution kinds** declared in YAML: `sync` and `durable`.
- A small, explicit **idempotency + concurrency contract** that makes restarts and duplicates safe.

### 1.1 Design Principles

- **Uniform verb surface.** The agent composes runbooks without caring whether a verb completes in 200ms or 9 days.
- **Execution semantics are configuration, not code.** Kind/handler/retry/idempotency are declared in YAML.
- **Two kinds, not three.** CRUD is a generic parameterised `sync` handler; research tasks are also `sync`. Everything that parks is `durable`.
- **Deterministic resumption.** Durable completion resumes from a well-defined point with a well-defined state snapshot (no ambient in-memory context).
- **Exactly-once illusion layer.** Runtime must tolerate duplicates, retries, and restarts while preserving deterministic outcomes.

---

## 2. Execution Model

### 2.1 The Two Kinds

Every verb has exactly one execution kind:

| Kind | Semantics | Lifecycle | Examples |
|------|-----------|-----------|----------|
| **sync** | Execute a Rust handler. Bounded runtime. Returns a result payload. | Invoke → Execute → Return | CRUD, GLEIF traversal, Bloomberg lookup, registry queries, scoring |
| **durable** | Spawn an external process. Park the runbook branch. Resume on correlation event. | Invoke → Park → (wait) → Notification → Resume | doc requests, human review, EDD, regulatory confirmations |

If a task has a sync phase followed by a durable phase, model it as two verbs (preferred) or a durable process whose internal definition includes the sync sub-step.

### 2.2 Universal Execution Result

```rust
/// The universal result of any verb execution.
enum ExecutionResult {
    /// Verb completed; contains a result payload.
    Complete(Value),
    /// Verb has been parked; contains a resumption ticket.
    Parked(InvocationRecord),
}
```

### 2.3 Universal Handler Trait (Sync and Durable)

```rust
#[async_trait]
trait VerbExecutor: Send + Sync {
    async fn execute(
        &self,
        ctx: &VerbContext,
        params: &Value,
    ) -> anyhow::Result<ExecutionResult>;
}
```

### 2.4 Sync Execution

Sync verbs are compiled Rust handlers. CRUD is a generic parameterised sync handler.

```rust
struct CrudExecutor { /* db + registry */ }

#[async_trait]
impl VerbExecutor for CrudExecutor {
    async fn execute(&self, ctx: &VerbContext, params: &Value) -> anyhow::Result<ExecutionResult> {
        // ... execute insert/select/update/delete ...
        Ok(ExecutionResult::Complete(result))
    }
}
```

### 2.5 Durable Execution (Normative Ordering)

Durable verbs must avoid “split-brain” failure (spawned externally but not persisted, or vice versa). Use this ordering:

1. Derive **deterministic** `invocation_id` and `correlation_key`
2. **Persist** `InvocationRecord` (status=`active`) transactionally
3. Spawn external process **passing idempotency token** (invocation_id/correlation_key)
4. Update record with external `process_instance_id` (best-effort)

```rust
struct BpmnDurableExecutor { bpmn_client: BpmnEngineClient }

#[async_trait]
impl VerbExecutor for BpmnDurableExecutor {
    async fn execute(&self, ctx: &VerbContext, params: &Value) -> anyhow::Result<ExecutionResult> {
        let process_ref = params["process_ref"].as_str().unwrap();

        // Deterministic correlation (example): UUIDv5(runbook_id + step_id)
        let invocation_id = ctx.deterministic_invocation_id()?;
        let correlation_key = ctx.deterministic_correlation_key()?;

        let timeout_at = ctx.compute_timeout_at(params)?;

        // (1) Persist invocation record first (authoritative)
        let record = ctx.invocations.persist_active(InvocationRecord {
            invocation_id,
            step_id: ctx.step_id,
            runbook_id: ctx.runbook_id,
            verb: ctx.verb_name.clone(),
            correlation_key: correlation_key.clone(),
            process_instance_id: None,
            parked_at: Utc::now(),
            timeout_at,
            escalation_ref: params.get("escalation").and_then(|v| v.as_str()).map(|s| s.to_string()),
            captured_context: ctx.snapshot_for_resume()?,
            status: InvocationStatus::Active,
        }).await?;

        // (2) Spawn external process with idempotency token
        // External engine must treat (process_ref, correlation_key) or invocation_id as idempotent.
        let instance_id = self.bpmn_client.start_process(
            process_ref,
            &correlation_key,
            invocation_id,
            params,
        ).await?;

        // (3) Best-effort update with external instance id
        ctx.invocations.set_process_instance(record.invocation_id, instance_id.clone()).await.ok();

        Ok(ExecutionResult::Parked(record.with_process_instance(instance_id)))
    }
}
```

**Key contract:** Durable completion is handled by the orchestrator, not by the verb handler.

---

## 2.6 Exactly-Once Illusion: Leases + Idempotency (Required)

The orchestrator must assume:
- duplicate notifications
- duplicate durable starts (retries/timeouts)
- restart mid-step
- concurrent `advance()` calls

To keep outcomes deterministic, we introduce two persistence primitives:

1. **Step Execution Lease** — prevents double execution and enables safe restarts.
2. **Notification Inbox** — records completion events and allows at-least-once delivery.

These are *small* but foundational.

---

## 3. YAML Verb Definition (Extended)

Verbs gain an `execution` block that declares kind, handler, and operational semantics.

New fields:
- `idempotency`: how to dedupe starts/completions
- `timeouts`: park timeout + optional heartbeat extension
- `side_effects`: informs retry policy and safety (internal vs external)

```yaml
- name: "request_client_documents"
  domain: "kyc"
  description: "Request documents and await upload"
  execution:
    kind: durable
    handler: "bpmn::start_process"
    params:
      process_ref: "client_doc_request_v1"
      correlation_field: "case_id"
      escalation: "compliance_escalation_v1"
    idempotency:
      scope: "runbook_step"            # runbook_step | case | global
      key_fields: ["case_id", "document_types"]
    timeouts:
      park_timeout: "P14D"
    side_effects: "human_process"      # none | internal_db | external_call | human_process
    retry:
      max_attempts: 3
      backoff: exponential
      base_delay: "PT2S"
      max_delay: "PT30S"
  input_schema:
    required: [case_id, document_types, contact_email]
    properties:
      case_id: { type: uuid }
      document_types: { type: array, items: { type: string } }
      contact_email: { type: string, format: email }
      message: { type: string }
```

### 3.1 Freeze Execution Semantics at Compile Time (Important)

Even though YAML can differ across environments, once a runbook is created the step must not “change kind” mid-flight.

**Rule:** when compiling a runbook, persist `execution_kind`, `handler`, and `side_effects` into each step record. YAML changes affect *new* runbooks only.

---

## 4. Runbook Structure

### 4.1 What Is a Runbook

A runbook is a **DAG of verb invocations** generated by the agent to fulfil an onboarding objective. It is not BPMN.

- Nodes are steps (verb invocations).
- Edges encode data dependencies and sequencing requirements.
- The agent may extend the DAG mid-execution.

### 4.2 Runbook Data Model (Canonical Edges)

**Canonical representation:** edges table is the source of truth. Do not duplicate dependency lists inside steps.

```rust
struct Runbook {
    runbook_id: Uuid,
    case_id: Uuid,
    created_at: DateTime<Utc>,
    created_by: AgentId,
    status: RunbookStatus,
}

enum RunbookStatus {
    InProgress,   // agent is still building
    Executing,
    Complete,
    Failed { step_id: Uuid, error: String },
    Cancelled,
    Escalated { step_id: Uuid },
}

struct RunbookStep {
    step_id: Uuid,
    runbook_id: Uuid,
    verb: String,
    params: Value,                         // resolved params
    execution_kind: ExecutionKind,          // frozen at compile time
    handler: String,                       // frozen at compile time
    side_effects: SideEffects,             // frozen at compile time
    status: StepStatus,
    result: Option<Value>,
    invocation_id: Option<Uuid>,           // if durable parked
}
```

### 4.3 Step Status

```rust
enum StepStatus {
    Pending,     // waiting for dependencies
    Ready,       // deps complete, ready to run
    Running,     // lease acquired
    Parked,      // durable invocation active
    Complete,
    Failed(String),
    Skipped(String),
    Cancelled,
}
```

### 4.4 Edges

```rust
struct RunbookEdge {
    runbook_id: Uuid,
    from_step_id: Uuid,
    to_step_id: Uuid,
    kind: EdgeKind,
}

enum EdgeKind { DataDependency, SequenceDependency }
```

---

## 5. Orchestration Engine

### 5.1 Concurrency Control (Required)

To avoid lost updates when multiple notifications arrive, `advance()` must be serialized per runbook.

Pick one strategy:

- **Advisory lock** per runbook: `pg_advisory_lock(hash(runbook_id))` (recommended v1)
- Optimistic concurrency via `runbooks.version`
- `SELECT ... FOR UPDATE` on runbook row

This document assumes advisory locks.

### 5.2 Step Execution Lease (Required)

The orchestrator must acquire a lease before executing a step. This prevents double-run if:
- `advance()` runs twice concurrently
- worker restarts mid-execution
- notification triggers `advance()` while a batch is still running

Lease semantics:
- Lease is keyed by `(step_id)` (unique)
- Only one RUNNING lease can exist
- Completed steps are no-ops (cached results)

### 5.3 The Advance Loop (Lease-aware)

```rust
impl RunbookOrchestrator {
    async fn advance(&self, runbook_id: Uuid) -> anyhow::Result<()> {
        let _lock = self.store.advisory_lock_runbook(runbook_id).await?;

        // Reload under lock
        let runbook = self.store.load_runbook(runbook_id).await?;

        if matches!(runbook.status, RunbookStatus::Cancelled | RunbookStatus::Complete) {
            return Ok(());
        }

        loop {
            let ready = self.store.ready_steps(runbook_id).await?;
            if ready.is_empty() { break; }

            // Execute ready steps concurrently; each step must acquire its lease
            let futs: Vec<_> = ready.into_iter().map(|step| {
                let me = self.clone();
                async move { me.execute_step(runbook_id, step.step_id).await }
            }).collect();

            for r in futures::future::join_all(futs).await {
                r?; // step executor updates store as it goes
            }
        }

        // Terminal check
        self.store.update_runbook_terminal_status(runbook_id).await?;
        Ok(())
    }

    async fn execute_step(&self, runbook_id: Uuid, step_id: Uuid) -> anyhow::Result<()> {
        // Acquire lease (idempotent). If already completed/leased, no-op.
        if !self.store.try_acquire_step_lease(step_id, self.worker_id()).await? {
            return Ok(());
        }

        let step = self.store.load_step(step_id).await?;
        if step.status == StepStatus::Complete { return Ok(()); }

        self.store.set_step_status(step_id, StepStatus::Running).await?;

        let verb_def = self.registry.resolve_frozen(&step)?; // uses frozen handler/kind
        let exec = self.registry.resolve(&verb_def)?;

        let ctx = self.store.build_ctx(runbook_id, step_id).await?;
        let res = exec.execute(&ctx, &step.params).await;

        match res {
            Ok(ExecutionResult::Complete(v)) => {
                self.store.complete_step(step_id, v).await?;
                self.store.complete_lease(step_id).await?;
            }
            Ok(ExecutionResult::Parked(inv)) => {
                self.store.park_step(step_id, inv.invocation_id).await?;
                self.store.complete_lease(step_id).await?;
            }
            Err(e) => {
                self.store.fail_step(step_id, e.to_string()).await?;
                self.store.fail_lease(step_id, e.to_string()).await?;
            }
        }

        Ok(())
    }
}
```

**Note:** durable steps release the lease after persisting the invocation record and parking. The “in-flight” durable wait is represented by `invocation_records.status=active`, not by a held lease.

---

## 6. Durable Push/Pop Lifecycle

### 6.1 PUSH (park)

1. Acquire step lease  
2. Persist invocation record (active)  
3. Spawn external process (idempotent token)  
4. Mark step `Parked` with `invocation_id`  
5. Release lease  

### 6.2 POP (resume on notification)

1. Insert notification into inbox (idempotent insert)  
2. Under runbook lock:
   - find active invocation for correlation_key
   - mark invocation completed
   - mark step `Complete` with result  
3. Mark notification processed (idempotent update)  
4. Re-enter `advance(runbook_id)`  

---

## 7. Persistence & Durability

### 7.1 What Must Survive

| Data | Storage | Recovery Semantics |
|------|---------|-------------------|
| Runbook rows | Postgres | reload and resume under runbook lock |
| Steps + results | Postgres | completed steps are cached (no-op on re-exec) |
| Edges | Postgres | canonical graph definition |
| Step leases | Postgres | prevents double-run |
| Invocation records | Postgres | parked tasks remain parked across restarts |
| Notification inbox | Postgres | at-least-once delivery, idempotent pop |
| Runbook mutations log | Postgres | audit + reconstruct agent changes |

---

## 8. Schema (Proposed)

> Names are illustrative; align with existing OB-POC naming conventions.

### 8.1 Status enums / CHECK constraints (Recommended)

Use Postgres enums or CHECK constraints to prevent typos:
- runbook_status in (...)
- step_status in (...)
- invocation_status in (...)

### 8.2 Core Tables

```sql
CREATE TABLE runbooks (
  runbook_id  UUID PRIMARY KEY,
  case_id     UUID NOT NULL REFERENCES kyc.cases(case_id),
  status      TEXT NOT NULL CHECK (status IN ('in_progress','executing','complete','failed','cancelled','escalated')),
  created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
  created_by  TEXT NOT NULL,
  version     BIGINT NOT NULL DEFAULT 0
);

CREATE TABLE runbook_steps (
  step_id         UUID PRIMARY KEY,
  runbook_id      UUID NOT NULL REFERENCES runbooks(runbook_id) ON DELETE CASCADE,
  verb            TEXT NOT NULL,
  params          JSONB NOT NULL,
  execution_kind  TEXT NOT NULL CHECK (execution_kind IN ('sync','durable')),
  handler         TEXT NOT NULL,
  side_effects    TEXT NOT NULL CHECK (side_effects IN ('none','internal_db','external_call','human_process')),
  status          TEXT NOT NULL CHECK (status IN ('pending','ready','running','parked','complete','failed','skipped','cancelled')),
  result          JSONB,
  invocation_id   UUID,
  created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
  completed_at    TIMESTAMPTZ
);

CREATE TABLE runbook_edges (
  runbook_id     UUID NOT NULL REFERENCES runbooks(runbook_id) ON DELETE CASCADE,
  from_step_id   UUID NOT NULL REFERENCES runbook_steps(step_id) ON DELETE CASCADE,
  to_step_id     UUID NOT NULL REFERENCES runbook_steps(step_id) ON DELETE CASCADE,
  edge_kind      TEXT NOT NULL CHECK (edge_kind IN ('data','sequence')),
  PRIMARY KEY (from_step_id, to_step_id)
);
```

### 8.3 Step Execution Lease

```sql
CREATE TABLE step_execution_leases (
  step_id       UUID PRIMARY KEY REFERENCES runbook_steps(step_id) ON DELETE CASCADE,
  status        TEXT NOT NULL CHECK (status IN ('running','completed','failed')),
  attempt       INT NOT NULL DEFAULT 1,
  worker_id     TEXT NOT NULL,
  started_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  completed_at  TIMESTAMPTZ,
  error         TEXT,
  result_hash   TEXT
);
```

### 8.4 Invocation Records (Durable)

```sql
CREATE TABLE invocation_records (
  invocation_id    UUID PRIMARY KEY,
  step_id          UUID NOT NULL REFERENCES runbook_steps(step_id) ON DELETE CASCADE,
  runbook_id       UUID NOT NULL REFERENCES runbooks(runbook_id) ON DELETE CASCADE,
  correlation_key  TEXT NOT NULL UNIQUE,
  process_instance TEXT,
  parked_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
  timeout_at       TIMESTAMPTZ NOT NULL,
  escalation_ref   TEXT,
  captured_context JSONB NOT NULL DEFAULT '{}'::jsonb,
  status           TEXT NOT NULL CHECK (status IN ('active','completed','timed_out','cancelled'))
);

CREATE INDEX idx_invocation_active_correlation
  ON invocation_records(correlation_key)
  WHERE status = 'active';
```

### 8.5 Notification Inbox

```sql
CREATE TABLE completion_notifications (
  notification_id  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  correlation_key  TEXT NOT NULL,
  result           JSONB NOT NULL,
  received_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
  processed        BOOLEAN NOT NULL DEFAULT false
);

CREATE UNIQUE INDEX uq_completion_notifications_key
  ON completion_notifications(correlation_key);

CREATE INDEX idx_notifications_unprocessed
  ON completion_notifications(received_at)
  WHERE NOT processed;
```

### 8.6 Runbook Mutations (Agent Auditing)

```sql
CREATE TABLE runbook_mutations (
  mutation_id   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  runbook_id    UUID NOT NULL REFERENCES runbooks(runbook_id) ON DELETE CASCADE,
  actor         TEXT NOT NULL,
  kind          TEXT NOT NULL,
  payload       JSONB NOT NULL,
  created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

---

## 9. Notification Delivery

### 9.1 LISTEN/NOTIFY (v1 recommended)

```rust
async fn deliver_notification(db: &PgPool, key: &str, result: Value) -> anyhow::Result<()> {
    let mut tx = db.begin().await?;

    sqlx::query(
        r#"
        INSERT INTO completion_notifications (correlation_key, result)
        VALUES ($1, $2)
        ON CONFLICT (correlation_key) DO NOTHING
        "#
    )
    .bind(key)
    .bind(&result)
    .execute(&mut *tx)
    .await?;

    sqlx::query("SELECT pg_notify('runbook_completions', $1)")
        .bind(key)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}
```

---

## 10. Timeout & Escalation

A background job scans for active invocations whose `timeout_at < now()`:
- set invocation status `timed_out`
- mark corresponding step as `Failed` or `Escalated`
- spawn escalation (optional)

---

## 11. Cancellation Semantics (Normative)

On cancel:
1. `runbooks.status='cancelled'`
2. active invocations → `cancelled`
3. non-terminal steps → `cancelled`
4. best-effort `bpmn.cancel(process_instance_id)`
5. later completion notifications become no-ops (but remain recorded)

---

## 12. Summary

Two kinds of verbs (`sync`, `durable`) + uniform DSL surface.

To make it operationally correct:
- runbook-level locking
- step execution leases
- inbox notifications
- deterministic correlation/idempotency tokens
- append-only mutation log

This yields deterministic execution across sub-second research and multi-week human workflows without splitting the platform into separate orchestration universes.
