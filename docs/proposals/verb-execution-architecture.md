# Verb Execution Model: Sync & Durable Execution Architecture

## Enterprise Onboarding Platform — Core Architecture Enhancement

**Status:** Draft — For Review  
**Author:** Architecture Team  
**Date:** February 2026  
**Domain:** KYC/UBO Discovery, Compliance Orchestration

---

## 1. Motivation

The platform's DSL verb system currently handles two implicit execution patterns under a single synchronous model: state mutations (CRUD) and research/computation tasks (GLEIF traversal, Bloomberg lookups). Both execute as compiled Rust functions and return immediately.

A third pattern is emerging and cannot be deferred: **durable, long-running tasks** that require external actors — requesting documents from clients, awaiting regulatory confirmations, human review gates. These tasks share the same DSL verb surface but have fundamentally different execution semantics: they start, park, and resume on an external signal that may arrive minutes, days, or weeks later.

Today we have no first-class model for this. The risk is that we bolt on workflow orchestration as an afterthought, creating a seam between "agent-executable" and "human-involving" work that becomes a source of accidental complexity.

This document proposes a minimal, precise extension to the verb execution model that treats durable execution as a peer to synchronous execution — same verb interface, different runtime semantics, declared in YAML configuration.

### 1.1 Design Principles

- **Uniform verb surface.** An agent composing a runbook does not care whether a verb completes in 200ms or 9 days. The DSL grammar and verb invocation syntax are identical regardless of execution kind.
- **Execution semantics are configuration, not code.** Whether a verb is sync or durable is declared in the YAML verb definition. The same verb could theoretically change execution kind across environments (e.g., a mock-sync version in testing, durable in production).
- **Two kinds, not three.** CRUD is not a separate execution category — it is a well-known, generic, parameterised sync handler. Everything compiled and immediate is `sync`. Everything that parks and awaits an external signal is `durable`. There is no third kind.
- **Stack-based mental model for durability.** A durable verb invocation pushes an invocation record onto the runbook's pending set. A completion notification pops it. This is the entire lifecycle.
- **Deterministic resumption.** When a durable task completes, the runbook must resume from a well-defined point with well-defined state. No ambient context, no implicit assumptions about what was "in memory" when we parked.

---

## 2. Execution Model

### 2.1 The Two Kinds

Every verb in the system has exactly one execution kind:

| Kind | Semantics | Lifecycle | Examples |
|------|-----------|-----------|----------|
| **sync** | Execute a compiled Rust function. Bounded execution time. Returns a result value. | Invoke → Execute → Return | CRUD operations, GLEIF traversal, Bloomberg lookup, share registry query, risk scoring |
| **durable** | Spawn an external process. Park the current runbook branch. Resume when a correlation event arrives. | Invoke → Park → (wait) → Notification → Resume | Request client documents, await compliance approval, external due diligence review, regulatory filing confirmation |

There is no hybrid kind. A verb is one or the other. If a task involves a synchronous phase followed by a durable phase (e.g., "send document request email then wait for upload"), that is modelled as two verbs or as a durable verb whose process definition internally handles the send.

### 2.2 Sync Execution

Sync verbs are compiled Rust functions conforming to a common trait. The generic CRUD executor is one such function. A bespoke GLEIF hierarchy crawler is another. They differ in specificity, not in kind.

```rust
/// The universal result of any verb execution.
enum ExecutionResult {
    /// Verb completed. Value contains the result payload.
    Complete(Value),
    /// Verb has been parked. InvocationRecord contains the resumption ticket.
    Parked(InvocationRecord),
}

/// All verb handlers implement this trait.
#[async_trait]
trait VerbExecutor: Send + Sync {
    /// Execute the verb with the given context and parameters.
    /// 
    /// For sync verbs, this returns Complete(_).
    /// For durable verbs, this spawns the external process and returns Parked(_).
    async fn execute(
        &self,
        ctx: &VerbContext,
        params: &Value,
    ) -> Result<ExecutionResult>;
}
```

The CRUD executor is registered as a handler like any other:

```rust
struct CrudExecutor {
    db: PgPool,
    schema_registry: Arc<SchemaRegistry>,
}

#[async_trait]
impl VerbExecutor for CrudExecutor {
    async fn execute(&self, ctx: &VerbContext, params: &Value) -> Result<ExecutionResult> {
        let entity = params["entity"].as_str().unwrap();
        let operation = CrudOp::from_str(params["operation"].as_str().unwrap())?;
        let fields = &params["fields"];
        
        let result = match operation {
            CrudOp::Create => self.insert(entity, fields).await?,
            CrudOp::Read   => self.select(entity, fields).await?,
            CrudOp::Update => self.update(entity, fields).await?,
            CrudOp::Delete => self.soft_delete(entity, fields).await?,
        };
        
        Ok(ExecutionResult::Complete(result))
    }
}
```

The GLEIF researcher is equally just a handler:

```rust
struct GleifHierarchyExecutor { /* ... */ }

#[async_trait]
impl VerbExecutor for GleifHierarchyExecutor {
    async fn execute(&self, ctx: &VerbContext, params: &Value) -> Result<ExecutionResult> {
        let lei = params["lei"].as_str().unwrap();
        let depth = params.get("max_depth").and_then(|v| v.as_u64()).unwrap_or(5);
        let hierarchy = self.discover(lei, depth).await?;
        Ok(ExecutionResult::Complete(serde_json::to_value(hierarchy)?))
    }
}
```

**Key insight:** From the execution engine's perspective, these are identical. The difference between CRUD and GLEIF research is the handler registered, not the execution model.

### 2.3 Durable Execution

A durable verb does three things:

1. **Spawns** an external process (BPMN instance, message to a queue, row in a task table — the mechanism is pluggable).
2. **Creates** an `InvocationRecord` containing everything needed to resume.
3. **Returns** `Parked(record)`, signalling the orchestrator to suspend this branch of the runbook.

```rust
struct BpmnDurableExecutor {
    bpmn_client: BpmnEngineClient,
}

#[async_trait]
impl VerbExecutor for BpmnDurableExecutor {
    async fn execute(&self, ctx: &VerbContext, params: &Value) -> Result<ExecutionResult> {
        let process_ref = params["process_ref"].as_str().unwrap();
        let correlation_key = ctx.resolve_correlation(params)?;
        let timeout = parse_duration(params.get("timeout"))?;
        
        // Spawn the BPMN process instance
        let instance_id = self.bpmn_client.start_process(
            process_ref,
            &correlation_key,
            params,
        ).await?;
        
        let record = InvocationRecord {
            invocation_id: Uuid::new_v4(),
            verb: ctx.verb_name.clone(),
            process_instance_id: Some(instance_id),
            correlation_key,
            parked_at: Utc::now(),
            timeout,
            resume_point: ctx.dag_position.clone(),
            // Snapshot of any intermediate state the runbook needs on resume
            captured_context: ctx.snapshot_for_resume()?,
        };
        
        Ok(ExecutionResult::Parked(record))
    }
}
```

Resumption is handled by the orchestrator (Section 4), not by the verb handler itself.

### 2.4 YAML Verb Definition

The verb YAML gains an `execution` block. The existing verb metadata (domain, entity associations, parameter schemas) remains unchanged.

```yaml
# Sync verb — CRUD (generic handler, parameterised)
- name: "update_case_status"
  domain: "kyc"
  description: "Update the status of a KYC case"
  execution:
    kind: sync
    handler: "crud::execute"
    params:
      entity: "kyc_case"
      operation: update
  input_schema:
    required: [case_id, status]
    properties:
      case_id: { type: uuid }
      status: { type: string, enum: [open, pending_review, escalated, closed] }

# Sync verb — bespoke function
- name: "research_gleif_hierarchy"
  domain: "kyc"
  description: "Discover corporate hierarchy via GLEIF LEI relationships"
  execution:
    kind: sync
    handler: "gleif::discover_hierarchy"
  input_schema:
    required: [lei]
    properties:
      lei: { type: string, pattern: "^[A-Z0-9]{20}$" }
      max_depth: { type: integer, default: 5 }
  output_schema:
    type: object
    properties:
      hierarchy: { type: array, items: { "$ref": "#/definitions/LeiNode" } }

# Sync verb — Bloomberg research
- name: "research_bloomberg_subsidiaries"
  domain: "kyc"
  description: "Query Bloomberg for subsidiary and voting share structure"
  execution:
    kind: sync
    handler: "bloomberg::query_subsidiaries"
  input_schema:
    required: [entity_identifier]
    properties:
      entity_identifier: { type: string }
      include_voting_shares: { type: boolean, default: true }

# Durable verb — client document request
- name: "request_client_documents"
  domain: "kyc"
  description: "Request specific documents from the client and await upload"
  execution:
    kind: durable
    handler: "bpmn::start_process"
    params:
      process_ref: "client_doc_request_v1"
      correlation_field: "case_id"
      timeout: "P14D"                    # ISO 8601 duration — 14 days
      escalation: "compliance_escalation_v1"
  input_schema:
    required: [case_id, document_types, contact_email]
    properties:
      case_id: { type: uuid }
      document_types: { type: array, items: { type: string } }
      contact_email: { type: string, format: email }
      message: { type: string }

# Durable verb — compliance officer review
- name: "await_compliance_review"
  domain: "kyc"
  description: "Submit case for compliance officer review and await decision"
  execution:
    kind: durable
    handler: "bpmn::start_process"
    params:
      process_ref: "compliance_review_v1"
      correlation_field: "case_id"
      timeout: "P7D"
      escalation: "senior_compliance_escalation_v1"
  input_schema:
    required: [case_id, review_package]
    properties:
      case_id: { type: uuid }
      review_package: { "$ref": "#/definitions/ReviewPackage" }
```

### 2.5 Handler Registry

At startup, the runtime builds a handler registry from the verb definitions. This maps `(kind, handler)` pairs to trait objects:

```rust
struct HandlerRegistry {
    handlers: HashMap<String, Arc<dyn VerbExecutor>>,
}

impl HandlerRegistry {
    fn register(&mut self, handler_path: &str, executor: Arc<dyn VerbExecutor>) {
        self.handlers.insert(handler_path.to_string(), executor);
    }
    
    fn resolve(&self, verb_def: &VerbDefinition) -> Result<Arc<dyn VerbExecutor>> {
        self.handlers
            .get(&verb_def.execution.handler)
            .cloned()
            .ok_or_else(|| anyhow!("No handler registered for: {}", verb_def.execution.handler))
    }
}

// At startup:
fn build_registry(db: PgPool, bpmn: BpmnEngineClient /* ... */) -> HandlerRegistry {
    let mut reg = HandlerRegistry::new();
    
    // Sync handlers
    reg.register("crud::execute", Arc::new(CrudExecutor::new(db.clone())));
    reg.register("gleif::discover_hierarchy", Arc::new(GleifHierarchyExecutor::new()));
    reg.register("bloomberg::query_subsidiaries", Arc::new(BloombergExecutor::new()));
    
    // Durable handlers
    reg.register("bpmn::start_process", Arc::new(BpmnDurableExecutor::new(bpmn)));
    
    reg
}
```

---

## 3. Runbook Structure

### 3.1 What Is a Runbook

A runbook is a **directed acyclic graph (DAG)** of verb invocations generated by the agent to fulfil a KYC discovery objective. Each node in the DAG is a verb invocation. Edges represent data or sequencing dependencies.

The agent generates the runbook based on:
- The entity type being onboarded (fund, corporate, SPV, trust)
- Jurisdiction and regulatory requirements
- Results from prior discovery steps (the runbook may be extended mid-execution)

A runbook is **not** a BPMN process. It is the agent's plan. Individual nodes within the runbook may *spawn* BPMN processes (durable verbs), but the runbook itself is a lighter-weight structure optimised for agent reasoning and dynamic modification.

### 3.2 Runbook Data Model

```rust
/// A runbook is a DAG of steps with execution state.
struct Runbook {
    runbook_id: Uuid,
    case_id: Uuid,
    created_at: DateTime<Utc>,
    created_by: AgentId,
    status: RunbookStatus,
    steps: Vec<RunbookStep>,
    edges: Vec<RunbookEdge>,
}

enum RunbookStatus {
    /// Agent is still building the runbook (may add steps as results arrive)
    InProgress,
    /// All steps have been defined, execution is underway
    Executing,
    /// All steps complete
    Complete,
    /// A step failed and the runbook cannot proceed
    Failed { step_id: Uuid, error: String },
    /// A durable step has timed out or escalated
    Escalated { step_id: Uuid },
}

struct RunbookStep {
    step_id: Uuid,
    verb: String,                     // verb name from DSL
    params: Value,                    // resolved parameters
    depends_on: Vec<Uuid>,            // step_ids this step waits for
    status: StepStatus,
    result: Option<Value>,            // populated on completion
    invocation: Option<InvocationRecord>, // populated if durable and parked
}

enum StepStatus {
    /// Waiting for dependencies to complete
    Pending,
    /// Dependencies met, ready to execute
    Ready,
    /// Currently executing (sync) or spawning (durable)
    Running,
    /// Durable verb has been spawned, awaiting notification
    Parked,
    /// Completed successfully
    Complete,
    /// Failed
    Failed(String),
    /// Skipped (agent decided this step is unnecessary based on prior results)
    Skipped(String),
}

struct RunbookEdge {
    from: Uuid,  // step_id
    to: Uuid,    // step_id
    kind: EdgeKind,
}

enum EdgeKind {
    /// `to` needs the result of `from` as input
    DataDependency,
    /// `to` must execute after `from` regardless of data flow
    SequenceDependency,
}
```

### 3.3 Example: KYC Corporate Onboarding Runbook

```
                    ┌──────────────────────┐
                    │  research_gleif       │ sync
                    │  hierarchy            │
                    └──────────┬───────────┘
                               │
                    ┌──────────▼───────────┐
                    │  research_bloomberg   │ sync
                    │  subsidiaries         │
                    └──────────┬───────────┘
                               │
              ┌────────────────┼────────────────┐
              │                │                 │
   ┌──────────▼──────┐ ┌──────▼───────┐ ┌──────▼──────────┐
   │ research_voting  │ │ research_    │ │ request_client   │ durable
   │ share_registry   │ │ officers     │ │ documents        │
   │ (sync)           │ │ (sync)       │ │ (14 day timeout) │
   └──────────┬──────┘ └──────┬───────┘ └──────┬──────────┘
              │                │                 │
              └────────────────┼─────────────────┘
                               │
                    ┌──────────▼───────────┐
                    │  DECISION: Agent      │
                    │  evaluates UBO chain  │
                    │  completeness         │
                    └──────────┬───────────┘
                               │
                    ┌──────────▼───────────┐
                    │  compile_ubo_report   │ sync
                    └──────────┬───────────┘
                               │
                    ┌──────────▼───────────┐
                    │  await_compliance     │ durable
                    │  review               │
                    └──────────────────────┘
```

Note the parallelism: the three middle-tier steps can execute concurrently. Two complete in seconds (sync). One may take days (durable). The decision node cannot proceed until all three are done. This is the fundamental reason we need the parked/resume model — the runbook must survive across the durable gap.

---

## 4. Orchestration Engine

### 4.1 The Execution Loop

The orchestrator is a state machine that advances the runbook DAG:

```rust
struct RunbookOrchestrator {
    registry: Arc<HandlerRegistry>,
    store: Arc<RunbookStore>,       // persistence layer
    notifier: Arc<NotificationBus>, // receives durable completion events
}

impl RunbookOrchestrator {
    /// Main execution loop. Called on runbook creation and on every notification.
    async fn advance(&self, runbook_id: Uuid) -> Result<()> {
        let mut runbook = self.store.load(runbook_id).await?;
        
        loop {
            // Find all steps that are Ready (dependencies met, not yet started)
            let ready_steps = runbook.ready_steps();
            
            if ready_steps.is_empty() {
                // Either we're done, or everything remaining is Parked/Pending
                break;
            }
            
            // Execute all ready steps (potentially in parallel)
            let futures: Vec<_> = ready_steps.iter().map(|step| {
                self.execute_step(&mut runbook, step)
            }).collect();
            
            let results = join_all(futures).await;
            
            for result in results {
                match result? {
                    ExecutionResult::Complete(value) => {
                        // Step finished — mark complete, may unlock downstream steps
                        // Loop will pick up newly-ready steps on next iteration
                    }
                    ExecutionResult::Parked(record) => {
                        // Step is durable — mark parked, persist invocation record
                        // Do NOT advance downstream — wait for notification
                    }
                }
            }
            
            // Persist updated state after each batch
            self.store.save(&runbook).await?;
        }
        
        // Check terminal conditions
        if runbook.all_steps_complete() {
            runbook.status = RunbookStatus::Complete;
        }
        
        self.store.save(&runbook).await?;
        Ok(())
    }
    
    /// Called when a durable task sends a completion notification.
    async fn on_notification(&self, notification: CompletionNotification) -> Result<()> {
        // Find the runbook and step associated with this correlation key
        let (runbook_id, step_id) = self.store
            .find_parked_step(&notification.correlation_key)
            .await?;
        
        let mut runbook = self.store.load(runbook_id).await?;
        let step = runbook.step_mut(step_id)?;
        
        // Pop the invocation record
        step.status = StepStatus::Complete;
        step.result = Some(notification.result);
        step.invocation = None; // clear the parked record
        
        self.store.save(&runbook).await?;
        
        // Re-enter the advance loop — this may unlock downstream steps
        self.advance(runbook_id).await
    }
}
```

### 4.2 The Push/Pop Model

The invocation lifecycle for durable verbs follows a strict push/pop discipline:

```
PUSH (invoke durable verb):
  1. Execute handler → spawns external process
  2. Create InvocationRecord with:
     - correlation_key (how we match the notification back)
     - resume_point (which DAG node to continue from)
     - captured_context (any intermediate state needed for downstream steps)
     - timeout + escalation policy
  3. Persist InvocationRecord to runbook state
  4. Mark step as Parked
  5. Orchestrator stops advancing this branch

POP (notification arrives):
  1. Match notification.correlation_key → InvocationRecord
  2. Validate: is the record still active? (not timed out, not cancelled)
  3. Extract result payload from notification
  4. Mark step as Complete with result
  5. Remove InvocationRecord (pop)
  6. Re-enter orchestrator advance loop
  7. Newly-ready downstream steps begin executing
```

### 4.3 Parallel Durable Tasks

A runbook may have multiple branches parked simultaneously. This is not a stack in the strict LIFO sense — it's an **unordered pending set**. Multiple pops can occur independently:

```
Time 0:  PUSH(request_client_documents)     → pending: [A]
Time 0:  PUSH(await_external_due_diligence) → pending: [A, B]
Time 3d: POP(B) — due diligence complete    → pending: [A]
Time 8d: POP(A) — client uploads documents  → pending: []
         → All dependencies met, advance to next DAG layer
```

The data structure is a `HashMap<CorrelationKey, InvocationRecord>` rather than a `Vec`, enabling O(1) lookup on notification arrival.

### 4.4 Decision Nodes

Decision nodes are a special case of sync execution where the **agent itself** is the handler. When the orchestrator reaches a decision node, it:

1. Gathers all completed results from upstream steps.
2. Invokes the agent with a structured prompt: "Given these discovery results, what is the next action?"
3. The agent may: add new steps to the runbook (extending the DAG), mark certain pending steps as skipped, or determine the case is complete.

```rust
struct AgentDecisionExecutor {
    agent: Arc<AgentClient>,
}

#[async_trait]
impl VerbExecutor for AgentDecisionExecutor {
    async fn execute(&self, ctx: &VerbContext, params: &Value) -> Result<ExecutionResult> {
        let upstream_results = ctx.gather_upstream_results()?;
        
        let decision = self.agent.evaluate(AgentDecisionRequest {
            case_id: ctx.case_id,
            decision_point: ctx.verb_name.clone(),
            available_results: upstream_results,
            runbook_state: ctx.runbook_summary(),
        }).await?;
        
        Ok(ExecutionResult::Complete(serde_json::to_value(decision)?))
    }
}
```

Decision nodes are sync (the agent responds within a bounded time) but may have side effects on the runbook structure. The orchestrator must handle DAG mutations after a decision node completes.

---

## 5. Persistence & Durability

### 5.1 What Must Survive

The entire point of the durable execution model is surviving process restarts, deployments, and infrastructure failures. The following must be persisted transactionally:

| Data | Storage | Recovery Semantics |
|------|---------|-------------------|
| Runbook structure (DAG, steps, edges) | Postgres | Reload and re-enter advance loop |
| Step status and results | Postgres | Idempotent — re-executing a Complete step is a no-op |
| InvocationRecords (parked tasks) | Postgres | On restart, all Parked steps remain parked until notification |
| Completion notifications | Postgres (notification table) | At-least-once delivery; idempotent pop |

### 5.2 Schema

```sql
CREATE TABLE runbooks (
    runbook_id      UUID PRIMARY KEY,
    case_id         UUID NOT NULL REFERENCES kyc_cases(case_id),
    status          TEXT NOT NULL DEFAULT 'in_progress',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by      TEXT NOT NULL
);

CREATE TABLE runbook_steps (
    step_id         UUID PRIMARY KEY,
    runbook_id      UUID NOT NULL REFERENCES runbooks(runbook_id),
    verb            TEXT NOT NULL,
    params          JSONB NOT NULL,
    status          TEXT NOT NULL DEFAULT 'pending',
    result          JSONB,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at    TIMESTAMPTZ
);

CREATE TABLE runbook_edges (
    from_step_id    UUID NOT NULL REFERENCES runbook_steps(step_id),
    to_step_id      UUID NOT NULL REFERENCES runbook_steps(step_id),
    edge_kind       TEXT NOT NULL DEFAULT 'data',
    PRIMARY KEY (from_step_id, to_step_id)
);

CREATE TABLE invocation_records (
    invocation_id    UUID PRIMARY KEY,
    step_id          UUID NOT NULL REFERENCES runbook_steps(step_id),
    correlation_key  TEXT NOT NULL UNIQUE,
    process_instance TEXT,           -- external BPMN instance ID if applicable
    parked_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    timeout_at       TIMESTAMPTZ NOT NULL,
    escalation_ref   TEXT,
    captured_context JSONB,          -- snapshot for resumption
    status           TEXT NOT NULL DEFAULT 'active'  -- active | completed | timed_out | cancelled
);

CREATE INDEX idx_invocation_correlation ON invocation_records(correlation_key) WHERE status = 'active';

CREATE TABLE completion_notifications (
    notification_id  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    correlation_key  TEXT NOT NULL,
    result           JSONB NOT NULL,
    received_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    processed        BOOLEAN NOT NULL DEFAULT false
);

CREATE INDEX idx_notifications_unprocessed ON completion_notifications(received_at) WHERE NOT processed;
```

### 5.3 Notification Delivery

Two patterns, choose based on latency requirements:

**Pattern A: Postgres LISTEN/NOTIFY (recommended for v1)**

```rust
// Notification producer (BPMN engine callback, webhook handler, etc.)
async fn deliver_notification(db: &PgPool, key: &str, result: Value) -> Result<()> {
    let mut tx = db.begin().await?;
    
    sqlx::query("INSERT INTO completion_notifications (correlation_key, result) VALUES ($1, $2)")
        .bind(key)
        .bind(&result)
        .execute(&mut *tx)
        .await?;
    
    sqlx::query(&format!("NOTIFY runbook_completions, '{}'", key))
        .execute(&mut *tx)
        .await?;
    
    tx.commit().await?;
    Ok(())
}

// Orchestrator listener
async fn listen_for_completions(db: &PgPool, orchestrator: Arc<RunbookOrchestrator>) {
    let mut listener = PgListener::connect_with(&db).await.unwrap();
    listener.listen("runbook_completions").await.unwrap();
    
    loop {
        let notification = listener.recv().await.unwrap();
        let correlation_key = notification.payload();
        
        // Fetch and process
        if let Some(cn) = fetch_unprocessed(db, correlation_key).await {
            orchestrator.on_notification(cn).await.ok();
            mark_processed(db, cn.notification_id).await.ok();
        }
    }
}
```

**Pattern B: Polling (simpler, sufficient for many cases)**

A background task polls `completion_notifications WHERE NOT processed` every N seconds. Simpler to operate, slightly higher latency.

### 5.4 Timeout Handling

A separate background process scans for expired invocation records:

```rust
async fn check_timeouts(db: &PgPool, orchestrator: Arc<RunbookOrchestrator>) {
    loop {
        let expired = sqlx::query_as::<_, InvocationRecord>(
            "SELECT * FROM invocation_records WHERE status = 'active' AND timeout_at < now()"
        ).fetch_all(db).await.unwrap();
        
        for record in expired {
            // Mark timed out
            mark_timed_out(db, record.invocation_id).await.ok();
            
            // Trigger escalation if configured
            if let Some(escalation_ref) = &record.escalation_ref {
                spawn_escalation(db, escalation_ref, &record).await.ok();
            }
            
            // Notify orchestrator — the step has failed/escalated
            orchestrator.on_timeout(record).await.ok();
        }
        
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}
```

---

## 6. Agent Integration

### 6.1 Runbook Generation

The agent generates a runbook as a DSL program — a sequence of verb invocations with declared dependencies. The agent does not need to know execution kinds. It reasons about *what* needs to happen, not *how* each step executes:

```dsl
# Agent-generated runbook for corporate KYC discovery
LET gleif_result = EXEC research_gleif_hierarchy(lei: $entity_lei)
LET bloomberg_result = EXEC research_bloomberg_subsidiaries(entity_identifier: $entity_id)

# These can run in parallel — no data dependency between them
LET shares = EXEC research_voting_share_registry(entities: gleif_result.subsidiaries)
LET officers = EXEC research_company_officers(entities: gleif_result.subsidiaries)
LET docs = EXEC request_client_documents(
    case_id: $case_id,
    document_types: ["certificate_of_incorporation", "shareholder_register"],
    contact_email: $client_contact
)

# Decision gate — agent evaluates completeness
LET decision = EXEC evaluate_ubo_completeness(
    hierarchy: gleif_result,
    shares: shares,
    officers: officers,
    client_docs: docs
)

# Final steps based on decision
IF decision.complete THEN
    EXEC compile_ubo_report(case_id: $case_id, data: decision.compiled_data)
    EXEC await_compliance_review(case_id: $case_id, review_package: decision.review_package)
ELSE
    EXEC request_additional_information(case_id: $case_id, gaps: decision.gaps)
END
```

The DSL parser converts this into the DAG structure (Section 3.2). The execution engine handles sync vs. durable transparently.

### 6.2 Dynamic Runbook Extension

A critical capability: the agent can **extend the runbook mid-execution** based on intermediate results. If the GLEIF traversal reveals an unexpected subsidiary jurisdiction, the agent can add new steps to the DAG before those results are consumed by downstream nodes.

This means the runbook is not fully determined at creation time. The orchestrator must support:

1. Adding new steps and edges to an in-progress runbook.
2. Re-evaluating the `ready_steps()` set after mutation.
3. Persisting the extended runbook atomically.

### 6.3 Resumption Context

When a durable task completes and the orchestrator resumes, the agent may need to re-evaluate downstream steps in light of all accumulated results. The `captured_context` in `InvocationRecord` holds a snapshot of what was known at park time, but the agent should be given the full picture:

```rust
struct ResumptionContext {
    case_id: Uuid,
    runbook_id: Uuid,
    completed_step: StepSummary,           // the step that just finished
    all_completed_results: Vec<StepSummary>, // everything done so far
    pending_steps: Vec<StepSummary>,        // what's still waiting
    next_ready_steps: Vec<StepSummary>,     // what can run now
}
```

This allows the agent to make informed decisions about whether to proceed as planned, skip steps, or extend the runbook.

---

## 7. BPMN Integration

### 7.1 Role of BPMN

BPMN is **not** the orchestration layer for the runbook. It is a **runtime target** for durable verbs — a process engine that handles the human-facing, long-running workflows that individual verb invocations spawn.

The relationship is:

```
Agent → composes Runbook (DAG of verbs)
  Runbook → contains durable verbs
    Durable verb → spawns BPMN process instance
      BPMN process → manages human interaction (emails, reminders, uploads, approvals)
        BPMN completion → sends notification
          Notification → pops InvocationRecord
            Orchestrator → resumes runbook
```

### 7.2 BPMN Process Definitions

Each durable verb references a BPMN process definition. These are standalone, reusable process definitions:

**client_doc_request_v1:**
```
Start → Send Request Email → Wait for Upload (timer: 3 days)
  → [Upload received] → Validate Documents → End (success)
  → [Timer fired] → Send Reminder → Wait for Upload (timer: 5 days)
    → [Upload received] → Validate Documents → End (success)
    → [Timer fired] → Escalate to Compliance → End (escalated)
```

**compliance_review_v1:**
```
Start → Create Review Task → Assign to Compliance Officer → Wait for Decision
  → [Approved] → End (approved)
  → [Rejected] → End (rejected, with reasons)
  → [Timer: 7 days] → Escalate to Senior Compliance → Wait for Decision
    → [Decision] → End (decision)
    → [Timer: 3 days] → End (timeout, auto-escalate)
```

### 7.3 Correlation Contract

The BPMN engine and the runbook orchestrator communicate through a simple contract:

1. **Start:** Orchestrator calls BPMN engine API with `(process_ref, correlation_key, params)`.
2. **Progress (optional):** BPMN engine can post intermediate status updates (e.g., "reminder sent", "document received, validating"). These update the step's metadata but do not trigger resumption.
3. **Complete:** BPMN engine posts a completion notification with `(correlation_key, result)`. This triggers the pop.

The BPMN engine is a black box from the orchestrator's perspective. It could be Camunda, Flowable, a custom Rust implementation, or even a simple state machine backed by a database table for v1.

---

## 8. Error Handling & Resilience

### 8.1 Sync Verb Failures

A sync verb that fails (network error, invalid data, external service down) follows the retry policy declared in the verb definition:

```yaml
execution:
  kind: sync
  handler: "bloomberg::query_subsidiaries"
  retry:
    max_attempts: 3
    backoff: exponential
    base_delay: "PT2S"
    max_delay: "PT30S"
```

After exhausting retries, the step is marked `Failed` and the orchestrator evaluates whether the runbook can continue (are there alternative paths?) or must halt.

### 8.2 Durable Verb Failures

Durable verbs can fail in several ways:

| Failure Mode | Detection | Response |
|---|---|---|
| BPMN process fails to start | Immediate error from handler | Retry per policy, then fail step |
| Notification never arrives | Timeout expiry | Escalation policy, then fail/escalate step |
| BPMN process reports failure | Completion notification with error result | Fail step, agent evaluates alternatives |
| Correlation key mismatch | Notification arrives but no matching record | Log warning, dead-letter the notification |

### 8.3 Idempotency

All operations must be idempotent:

- Re-executing a `Complete` step returns the cached result.
- Re-delivering a completion notification for an already-popped invocation is a no-op.
- Re-starting the orchestrator advance loop on an unchanged runbook produces no new actions.

This is essential because the orchestrator may be restarted at any point, and the advance loop will re-evaluate the entire runbook state.

### 8.4 Compensation

For certain failures, the system may need to **undo** previously completed steps (e.g., if a compliance review is rejected after documents were already filed). This is out of scope for v1 but the architecture supports it through:

- A `compensate` handler declared on verb definitions.
- A reverse traversal of the DAG executing compensation handlers for completed steps.
- Saga pattern semantics: each step's compensation is independent.

---

## 9. Observability

### 9.1 Runbook Dashboard

Each runbook is fully observable:

- **DAG visualization:** Current state of all steps, color-coded by status.
- **Timeline view:** Wall-clock time per step, highlighting durable wait times.
- **Pending invocations:** Active parked tasks with time-to-timeout.
- **Audit trail:** Every state transition with timestamp and actor.

### 9.2 Metrics

| Metric | Purpose |
|--------|---------|
| `runbook_steps_total{kind, status}` | Volume and completion rates by execution kind |
| `runbook_step_duration_seconds{kind, verb}` | Execution time distribution |
| `durable_invocation_wait_seconds{verb}` | How long durable tasks actually take |
| `durable_timeout_total{verb}` | Timeout frequency — indicates process problems |
| `runbook_completion_seconds` | End-to-end runbook duration |
| `notification_delivery_lag_seconds` | Time from BPMN completion to orchestrator pop |

### 9.3 Tracing

Each runbook execution carries a trace context (OpenTelemetry) that spans across sync executions and survives durable gaps. The trace for a full KYC onboarding might span days but remains a single logical trace, enabling end-to-end observability of the discovery process.

---

## 10. Migration Path

### 10.1 Phase 1: Execution Kind Declaration (Low Risk)

- Add `execution` block to YAML verb definitions.
- All existing verbs get `kind: sync` with their current handler.
- No runtime changes. This is purely declarative metadata.
- Validates that the YAML schema extension works across all existing verbs.

### 10.2 Phase 2: Runbook Data Model (Medium Risk)

- Implement `Runbook`, `RunbookStep`, `RunbookEdge` tables and Rust types.
- Build the orchestrator advance loop for sync-only runbooks.
- Agent begins generating runbooks as DAGs instead of flat verb sequences.
- All execution is still synchronous — the DAG just enables parallelism.

### 10.3 Phase 3: Durable Execution (Higher Risk, Contained)

- Implement `InvocationRecord` table and push/pop lifecycle.
- Implement `BpmnDurableExecutor` (or a simpler "task table" executor for v1).
- Implement notification delivery (start with polling, upgrade to LISTEN/NOTIFY).
- Implement timeout checker background process.
- First durable verb: `request_client_documents` — well-understood, high-value.

### 10.4 Phase 4: BPMN Engine Integration

- Connect to chosen BPMN engine (or build minimal internal equivalent).
- Define BPMN process definitions for all durable verbs.
- Implement escalation policies.
- Production hardening: idempotency, dead-letter handling, compensation stubs.

### 10.5 Phase 5: Observability & Operational Maturity

- Runbook dashboard.
- Metrics and alerting.
- Distributed tracing across durable gaps.
- Operational runbooks for common failure scenarios.

---

## 11. Open Questions

1. **BPMN engine selection.** Build a minimal Rust-native process engine, or integrate Camunda/Flowable? The former keeps the stack uniform; the latter provides mature tooling for process design. A hybrid is possible: simple durable tasks (wait for event with timeout) use an internal Rust implementation, complex multi-step human workflows use an external engine.

2. **Agent re-evaluation on resume.** When a durable task completes after days, should the agent always re-evaluate the runbook plan? Or only at declared decision nodes? Always re-evaluating is safer but more expensive. Decision nodes are explicit but might miss cases where new information invalidates the plan.

3. **Runbook versioning.** If the agent extends a runbook mid-execution, how do we version and audit the changes? Git-style diffs of the DAG? An append-only log of mutations?

4. **Multi-case correlation.** Some durable tasks may affect multiple cases (e.g., a client uploads documents relevant to several onboarding cases simultaneously). How does the correlation model handle fan-out notifications?

5. **Cancellation semantics.** If a case is cancelled mid-runbook, how do we cancel parked durable tasks? The BPMN processes need a cancellation API, and the orchestrator needs to traverse all active invocation records for the runbook.

6. **Testing strategy.** Durable verbs are inherently difficult to test. Proposed approach: all durable verbs can be overridden with sync mocks in test configuration (`kind: sync, handler: "mock::instant_complete"`). This preserves the uniform verb surface while enabling fast, deterministic tests.

---

## 12. Summary

The core insight is simple: **two execution kinds, not three.** Everything the agent does is a verb. Every verb is either sync (execute and return) or durable (spawn, park, await, resume). CRUD is just a generic sync handler. BPMN is just a durable runtime target. The verb surface is uniform. The execution semantics are declared in configuration. The agent composes runbooks without caring about the difference.

The push/pop model for durable invocations provides a clean mental model that maps directly to implementation: push an invocation record when you park, pop it when the notification arrives. The runbook DAG advances whenever new steps become ready, whether that's milliseconds after a sync verb completes or days after a client uploads their documents.

This architecture gives us a foundation for the full spectrum of KYC discovery work — from sub-second database lookups to multi-week client interactions — within a single, coherent execution model.
