# BPMN Runtime → Session Pipeline Integration v1

**Status:** Draft — for peer review  
**Date:** 2026-05-27  
**Author:** ob-poc engineering  
**Relates to:** `docs/design/camunda8-to-dsl-transpiler-v1.md` (v0.8), unified DSL v0.1

---

## 1. Context

The unified DSL v0.1 ships a complete BPMN execution kernel (`bpmn-runtime` crate: `RuntimeEngine`, `PostgresJourneyStore`, verb plugin system, park/resume machinery). It is fully tested in isolation — 89 tests, E2E park→submit→resume cycle proven.

It is **not connected to the session pipeline**. The `ReplOrchestratorV2` pipeline that handles every user turn has no awareness of running BPMN instances. Concretely:

- No mechanism to start a BPMN process from a session turn
- When a `dsl.form` verb parks a fiber, the `bpmn_form` field never appears in the `ChatMessage` the React UI receives
- `POST /api/forms/:token_id/submit` enqueues `HumanTaskComplete` into `dsl_event_queue` but nothing drains that queue and runs the engine forward

This document specifies the integration work to close those gaps.

---

## 2. Scope

**In scope:**
- Starting a BPMN process from a session turn
- Surfacing a parked `dsl.form` human task in the session response (`bpmn_form` field)
- Draining `dsl_event_queue` after form submission so the fiber advances
- A per-process-definition engine pool accessible from Axum app state

**Out of scope:**
- Replacing the existing bpmn-lite gRPC path (those verbs stay unchanged)
- Timer events, message correlation, error boundary handling (follow-on)
- Multi-tenant process isolation (single-user PoC, not a concern yet)
- UI for process monitoring or instance list

---

## 3. Current State

```
User turn
    │
    ▼
ReplOrchestratorV2.process()
    │
    ├─ SemOS verb pipeline (verb search, arg extract, execute)
    │
    └─ ReplResponseV2 → ChatResponse → ChatMessage
                                           │
                                           └─ bpmn_form: None  ← always None today
```

`RuntimeEngine` exists as a standalone unit tested with `InMemoryJourneyStore`. `PostgresJourneyStore` exists but is never wired at startup. The `dsl_event_queue` table is populated by form submit but has no consumer.

---

## 4. Target Architecture

```
Startup
    │
    ▼
ProcessRegistry::load_all()           ← compiles all known DSL process definitions
    │                                    stores Arc<RuntimeEngine> per process name
    ▼
AppState { ..., process_registry: Arc<ProcessRegistry> }


User turn — start a workflow
    │
    ▼
workflow.start-process <name> <initial_data>
    │
    ▼
ProcessRegistry::start_instance(name, initial_data)
    │  → RuntimeEngine::start_instance()
    │  → runs to quiescence
    │
    ▼
    ├─ If quiesced (no human task):  session response = normal
    │
    └─ If parked at dsl.form:
           query dsl_pending_wait WHERE instance_id = ? AND wait_kind = 'human_task'
           → inject BpmnFormPending into ReplResponseV2
           → ChatMessage.bpmn_form = { form_ref, token_id, mode, prefill_data }
           → React renders <FormioForm>


Form submit (React)
    │
    POST /api/forms/:token_id/submit  { data: {...} }
    │
    ▼
forms route:
    1. Look up dsl_pending_wait by correlation_key = token_id
    2. Insert HumanTaskComplete into dsl_event_queue
    3. ProcessRegistry::run_to_quiescence(instance_id)   ← NEW: drain inline
    4. Return 202
```

---

## 5. Component Design

### 5.1 `ProcessRegistry`

New type in `ob-poc-web` (or a shared crate if reuse emerges).

```rust
pub struct ProcessRegistry {
    engines: HashMap<String, Arc<RuntimeEngine>>,
    store:   Arc<PostgresJourneyStore>,
}

impl ProcessRegistry {
    /// Load all process definitions from `process_definitions` table (see §6),
    /// compile each via dsl_parser + dsl_bpmn_frontend + dsl_lowering,
    /// instantiate a RuntimeEngine per spec, register dsl.form builtin handler.
    pub async fn load_all(pool: PgPool) -> Result<Self>;

    /// Start a new instance of the named process.
    /// Returns (instance_id, Option<BpmnFormPending>) — the pending form if the
    /// process immediately parks at a human task.
    pub async fn start_instance(
        &self,
        process_name: &str,
        initial_data: serde_json::Value,
    ) -> Result<(Uuid, Option<BpmnFormPending>)>;

    /// Drain the event queue for a specific instance and run to quiescence.
    /// Returns the next pending human task if the process re-parks.
    pub async fn run_instance(
        &self,
        instance_id: Uuid,
    ) -> Result<Option<BpmnFormPending>>;

    /// Look up which engine owns a given instance_id (via dsl_workflow_instance).
    pub async fn engine_for_instance(&self, instance_id: Uuid) -> Result<Arc<RuntimeEngine>>;
}
```

**Key design decision:** `RuntimeEngine` is per-`JourneySpec` (one process definition), not per-instance. All instances of the same process definition share one engine. The store (`PostgresJourneyStore`) is shared across all engines — it is the durable state.

### 5.2 `process_definitions` table (new migration)

```sql
CREATE TABLE process_definitions (
    name        TEXT PRIMARY KEY,
    dsl_source  TEXT NOT NULL,
    version     INTEGER NOT NULL DEFAULT 1,
    enabled     BOOLEAN NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

Seeded from `rust/config/processes/*.dsl` files at migration time (analogous to `form_schemas`). The `name` matches what callers pass to `workflow.start-process`.

`ProcessRegistry::load_all` queries `WHERE enabled = TRUE`, compiles each row, and builds the engine map.

### 5.3 `workflow.start-process` verb

New plugin verb in `sem_os_postgres::ops`.

```yaml
- name: workflow.start-process
  behavior: plugin
  args:
    process_name: { type: string, required: true }
    initial_data: { type: object, required: false }
  returns:
    type: record
```

```rust
pub struct WorkflowStartProcess;

impl SemOsVerbOp for WorkflowStartProcess {
    fn fqn(&self) -> &str { "workflow.start-process" }

    async fn execute(&self, args: &Value, ctx: &mut VerbExecutionContext, _: &mut dyn TransactionScope)
        -> Result<VerbExecutionOutcome>
    {
        let name = args["process_name"].as_str().ok_or_else(|| anyhow!("process_name required"))?;
        let initial_data = args.get("initial_data").cloned().unwrap_or(json!({}));

        let registry = ctx.service::<dyn ProcessRegistryService>()?;
        let (instance_id, pending_form) = registry.start_instance(name, initial_data).await?;

        Ok(VerbExecutionOutcome::Record(json!({
            "instance_id": instance_id,
            "status": if pending_form.is_some() { "parked" } else { "running" },
            "bpmn_form": pending_form,   // passed up to response adapter
        })))
    }
}
```

The `bpmn_form` value in the outcome record is detected by the response adapter (§5.4) and promoted to `ReplResponseV2.bpmn_form`.

### 5.4 Response adapter — `bpmn_form` injection

`ReplResponseV2` gains a new optional field:

```rust
pub struct ReplResponseV2 {
    // ... existing fields ...
    pub bpmn_form: Option<BpmnFormPending>,
}
```

`response_adapter.rs::repl_to_chat_response` maps `resp.bpmn_form` → `ChatMessage.bpmn_form`.

The orchestrator sets `resp.bpmn_form` from the verb execution outcome if the record contains a `bpmn_form` key (same pattern as `narration` extraction today).

### 5.5 Forms submit — inline drain

`POST /api/forms/:token_id/submit` currently enqueues the event and returns. Add a drain call after:

```rust
// After enqueue:
let registry = state.process_registry.clone();
let pending = registry.run_instance(wait.instance_id).await?;
// pending = Some(BpmnFormPending) if the process immediately hits another human task
// Return the pending form ref in the response so React can chain to the next form
Json(json!({
    "accepted": true,
    "token_id": token_id,
    "next_form": pending,   // null if process advanced past all human tasks
}))
```

### 5.6 App state wiring

`AppState` gains `process_registry: Arc<ProcessRegistry>`. Initialised in `main.rs` before the router is built:

```rust
let process_registry = Arc::new(
    ProcessRegistry::load_all(pool.clone()).await?
);
let state = AppState::new(pool.clone())
    .with_process_registry(Arc::clone(&process_registry));
```

`ProcessRegistryService` trait registered in `ServiceRegistry` so verb ops can reach it via `ctx.service::<dyn ProcessRegistryService>()`.

---

## 6. Data Flow — Full Cycle

```
1. User: "start kyc review workflow"
   → verb search → workflow.start-process { process_name: "kyc-review" }

2. ProcessRegistry::start_instance("kyc-review", {})
   → RuntimeEngine::start_instance({})
   → fiber runs: start → service-task (kyc.verify) → user-task (dsl.form) → PARKS
   → query dsl_pending_wait WHERE instance_id=X AND wait_kind='human_task'
   → returns BpmnFormPending { form_ref: "kyc.review-summary", token_id: T, mode: "display", prefill_data: {...} }

3. ReplResponseV2.bpmn_form = BpmnFormPending { ... }
   → ChatMessage.bpmn_form set
   → React renders <FormioForm form_ref="kyc.review-summary" token_id=T mode="display" />

4. FormioForm:
   → GET /api/forms/kyc.review-summary       ← Postgres: form_schemas table
   → renders prefilled read-only form
   → user clicks Continue
   → POST /api/forms/T/submit { approved: true }

5. forms route:
   → dsl_pending_wait lookup by correlation_key=T → instance_id=X, node_name="review-task"
   → INSERT dsl_event_queue (X, HumanTaskComplete, { node_name, token_id, output_data })
   → ProcessRegistry::run_instance(X)
   → RuntimeEngine::run_to_quiescence(X)
   → fiber resumes: gateway → end-approved
   → returns { accepted: true, next_form: null }

6. React: form dismissed, session continues
```

---

## 7. Migration Plan

### Phase 1 — Foundation (no user-visible change)
- Migration: `process_definitions` table
- `ProcessRegistry` struct + `load_all` (empty map is valid if no definitions seeded)
- `PostgresJourneyStore` instantiated at startup, wired into `AppState`
- No verb, no route change — just infrastructure

### Phase 2 — Start verb + response injection
- `workflow.start-process` verb (YAML + op)
- `ReplResponseV2.bpmn_form` field + orchestrator extraction
- `response_adapter` mapping
- Integration test: call `workflow.start-process` from a session, assert `ChatMessage.bpmn_form` is populated

### Phase 3 — Submit drain + full cycle
- `ProcessRegistry::run_instance` called from forms submit route
- Inline drain: engine advances after form submission
- `next_form` in submit response (for chained human tasks)
- E2E session test: start → parks → submit → completes

### Phase 4 — Process seeding
- At least one real process DSL seeded into `process_definitions`
- Manual browser smoke: full cycle through UI

---

## 8. Open Questions

**Q1 — Process definition ownership**  
Who authors and governs process DSL? Currently: local DSL files seeded by migration. Should these be SemOS-governed objects (versioned, lifecycle FSM, changeset approval)? Same boundary question as form schemas (Q10). Recommend: start with local files, flag as SemOS candidate at board.

**Q2 — RuntimeEngine reload**  
If a process definition is updated in `process_definitions`, the in-memory `ProcessRegistry` is stale until restart. Options: (a) accept restart requirement for v1, (b) add `ProcessRegistry::reload(name)` triggered by a `process.reload-definition` verb. v1: accept restart.

**Q3 — Multi-step form chains**  
A process could park at multiple sequential human tasks. The submit response carries `next_form` if the engine immediately re-parks. The React side needs to handle this (render next form, dismiss current). Currently `FormioForm.onComplete` only dismisses — it needs to check `next_form` and render a new form. Scoped to Phase 3.

**Q4 — Verb execution context service injection**  
`WorkflowStartProcess` needs `ProcessRegistryService` via `ctx.service::<dyn ProcessRegistryService>()`. The `ServiceRegistry` pattern exists (see `dyn SemanticStateService`). `ProcessRegistryService` is a new trait — needs to be defined in `sem_os_core` or `ob-poc-types`, not `ob-poc-web`, to avoid a dep inversion. Alternatively the verb lives in `rust/src/domain_ops/` (Pattern B) where it can reach app state directly.

**Q5 — Concurrent instance access**  
`RuntimeEngine::run_to_quiescence` is not re-entrant per instance — if two requests race (unlikely in PoC but possible), the event queue provides the serialisation. The `FOR UPDATE SKIP LOCKED` in `dequeue_events` means only one caller drains at a time. The second caller returns immediately with no events — which is correct. No additional locking needed for v1.

**Q6 — Instance → engine lookup**  
`ProcessRegistry::engine_for_instance` needs to map `instance_id → process_name`. The `dsl_workflow_instance.journey_name` column holds this. Query: `SELECT journey_name FROM dsl_workflow_instance WHERE id = $1`. No schema change required.

---

## 9. Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| `RuntimeEngine` per-spec assumption breaks with many process types | Low (PoC) | Engine map scales fine for O(10) process types |
| DSL compilation fails at startup for a malformed process definition | Medium | `load_all` logs and skips failed definitions; server starts with partial registry |
| Form submission races with session turn reading instance state | Low | Event queue serialises; worst case: stale status until next turn |
| `bpmn_form` field in `ReplResponseV2` not rendered by older React build | Low | Build required after backend change; no backwards compat needed |

---

## 10. Estimated Effort

| Phase | Scope | Effort |
|-------|-------|--------|
| 1 — Foundation | Migration + ProcessRegistry + store wiring | 0.5 day |
| 2 — Start verb + response injection | Verb + adapter + integration test | 1 day |
| 3 — Submit drain + full cycle | Drain + next_form + E2E session test | 0.5 day |
| 4 — Process seeding + browser smoke | Seed DSL + manual test | 0.5 day |
| **Total** | | **~2.5 days** |
