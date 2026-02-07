# DSL Verb ↔ BPMN Integration Architecture

**Document:** Architecture Vision  
**Version:** 3.0 — February 2026  
**Status:** Final Draft  
**Author:** Enterprise Onboarding Platform — ob-poc  
**Companion to:** REPL Re-Engineering v3 (Pack-Guided Runbook Architecture)  
**Lineage:** v1.0 (Anthropic draft) → vNext (ChatGPT revision) → v2.0 (merged with data philosophy) → v3.0 (consolidated with worker model + Camunda 8 guard rails)

---

## 0. Executive Summary

The DSL verb runtime is excellent for **short, deterministic** work — CRUD, lookups, calculations, entity creation. It is not designed for **durable, multi-day** work with human review gates, timeouts, escalation paths, external callbacks, and pause/resume semantics.

BPMN engines exist to solve durable orchestration. But they have a fundamental weakness: **they do not treat data as a first-class concern.** BPMN is a *flow* language, not a *data* language. Process variables are an afterthought — untyped, schema-free, stored as opaque blobs, with no concept of validation, transformation, or accumulation.

This architecture bridges the gap by keeping **verbs as the domain API** (where data is first-class) and using BPMN as a **durable orchestration substrate** (where flow is first-class). The verb runtime owns the data. The BPMN engine owns the wait. The two communicate through a strict payload contract that treats the engine as a carrier, never an interpreter.

From the REPL and runbook perspective, a verb invocation is uniform regardless of execution strategy. The runtime routes it to direct Rust execution or workflow execution based on the verb's definition.

---

## 1. The Problem

### 1.1 The Current Failure Mode

Today, every DSL verb executes as Rust control flow. That forces long-running processes into patterns like:

- Polling loops disguised as "durable execution"
- Human gates modelled as database flags rather than first-class wait states
- Retry and timeout logic hand-rolled per verb rather than declared as policy
- No visibility into where a long-running process is stuck
- No standard way to pause, resume, escalate, or cancel mid-flight

These are all symptoms of missing a first-class durable orchestration layer.

### 1.2 The BPMN Data Problem

BPMN engines solve the orchestration problem but introduce a data problem. In traditional BPMN integrations:

- **Process variables are untyped.** They are key-value pairs where values are serialised objects. There is no schema, no validation, no type safety. A process that expects `case_id` as a UUID will happily accept a string, an integer, or a null — and fail silently three steps later.

- **Data transformation happens in the wrong place.** BPMN's expression languages (JUEL, FEEL, Groovy scripts) are used to transform data inside the process model. This puts domain logic where it doesn't belong — in XML, maintained by a different team, versioned on a different cadence, and invisible to the domain type system.

- **Large payloads are an afterthought.** Engines persist process variables in their own storage. Documents, images, and complex entity graphs do not belong in process variable storage, but there is no standard mechanism for distinguishing "data the engine carries" from "data the engine references."

- **Accumulation is implicit.** As a process moves through steps, each step may add, modify, or remove variables. There is no formal contract for what a step receives, what it returns, and how its output merges with the running state. Variables drift, collide, and decay.

- **The payload boundary is invisible.** When data crosses from the domain runtime into the BPMN engine and back, there is no formal serialisation contract. Types are coerced silently. Nested structures may be flattened. Precision may be lost. What went in is not guaranteed to be what comes out.

**This is the integration gap.** BPMN solves flow. Verbs solve data. The architecture must define a precise boundary where flow and data meet, and must ensure that data integrity is never delegated to the flow layer.

### 1.3 What Must Be True

We need:

- **Durability:** park/resume across restarts for days/weeks
- **Wait-state primitives:** human task, timer, message signal
- **Policy:** retry/backoff/escalation as declarative config
- **Visibility:** "Step is waiting for X" without reading source code
- **Auditability:** uniform trace from runbook → verb → sub-verbs → outcomes
- **Data integrity:** the verb's typed payload survives the BPMN round-trip without loss, coercion, or schema drift
- **Zero "Java colonisation":** domain logic stays in Rust/DSL, not in BPMN expressions

---

## 2. Core Principles (Non-Negotiable)

### 2.1 One Verb Interface, Two Execution Strategies

From the perspective of the REPL, the runbook, and the user, every verb looks the same:

- Same invocation syntax: `(kyc.open-case :entity "Allianz GmbH" :case-type "periodic-review")`
- Same sentence playback: "Open a periodic review KYC case for Allianz GmbH"
- Same runbook entry: sentence + DSL + args + status + audit
- Same outcome event model: started → in-progress → completed / failed / parked

The difference is invisible to the caller. Behind the verb boundary, the runtime chooses one of two strategies:

| Strategy | Use when | Runtime does |
|---|---|---|
| **Direct** | Stateless or short-lived. CRUD, lookups, calculations, entity creation. | Call Rust handler, return immediately. |
| **Workflow** | Needs wait states, human gates, external callbacks, timeout/retry policies. | Start BPMN process, track until completed. |

The verb author declares the strategy. The runtime honours it. The caller never knows.

### 2.2 Hollow BPMN (BPMN Orchestrates, Verbs Do Domain)

- BPMN contains **sequencing**, **gateways**, **timers**, **human tasks**, **message waits**.
- BPMN contains **no embedded domain logic**, no data transforms, no database access, no expression-language evaluation of domain fields.
- Every service task is "call the verb runtime to execute a sub-verb."
- Gateway conditions are limited to **orchestration flags** — simple boolean or enum values that the verb runtime has explicitly set for routing purposes (e.g., `review_outcome == "approved"`). FEEL expressions that inspect, compute over, or transform domain entity fields are forbidden.

### 2.3 The Verb IS the Process Entry Point

In traditional BPMN, the process model is the primary artifact — you design a diagram, then wire service tasks to code. We invert this. **The verb is the primary artifact.** The BPMN process is an execution detail — a way of orchestrating the verb's internal steps. The verb's YAML definition declares the payload contract, the execution strategy, the process to dispatch to, and the wait-state semantics. The BPMN model is subordinate to the verb, not the other way around.

### 2.4 Payload Continuity (One Schema Everywhere)

The same typed payload governs every stage of a verb's lifecycle, regardless of execution strategy:

- **REPL slot filling** — the pack collects args that populate the payload
- **Runbook persistence** — the runbook entry stores the payload as the verb's args
- **Direct verb execution** — the Rust handler receives and returns the payload
- **Workflow dispatch** — the payload becomes the process instance's initial state
- **Sub-verb callbacks** — each service task receives and returns the payload
- **Final outcome** — the completed process returns the payload as the verb's result

There is no "BPMN payload" separate from the "verb payload." There is one schema, defined once in the verb's YAML, and every layer — REPL, runbook, Rust handler, BPMN engine, sub-verb callback — carries it faithfully. If a layer cannot carry the payload without loss, the architecture is broken at that layer.

### 2.5 Data Is a Verb Concern, Not an Engine Concern

**This is the central philosophical commitment of the architecture.**

The BPMN engine is a **flow machine**. It knows *when* to do things. It does not know *what* things mean. Data — entity schemas, validation rules, state transformations, business invariants, type safety — is exclusively the verb runtime's responsibility.

The engine's role with respect to data is strictly limited:

- **Carry** the payload between steps (as an opaque, typed envelope)
- **Branch** on orchestration flags (simple values set by the verb runtime for routing)
- **Never** validate, transform, inspect, compute over, or interpret the domain payload

If a gateway decision requires domain knowledge beyond a simple flag check, that decision must be expressed as a **sub-verb** that computes the flag and writes it to the orchestration namespace. The BPMN model then branches on the flag. The domain logic remains in Rust; the BPMN model remains hollow.

### 2.6 The Three-Layer Separation

The architecture enforces a strict separation of concerns:

**Layer 1 — Domain (DSL Verbs)**
Owns: Entity schemas, validation rules, state transformations, business invariants.
Knows nothing about: Process sequencing, retry policies, timer durations, human task assignment.

**Layer 2 — Orchestration (BPMN)**
Owns: When steps happen, parallel vs sequential execution, wait states, timeout policies, escalation paths, human task routing.
Knows nothing about: What the data means, how to validate it, what constitutes a correct transformation.

**Layer 3 — Interface (REPL / Runbook)**
Owns: User interaction, sentence playback, pack-guided Q/A, runbook editing.
Knows nothing about: Whether a verb executes directly or via workflow.

Each layer communicates through contracts, not shared implementation. A change to a KYC validation rule (Layer 1) does not require versioning a BPMN model (Layer 2). A change to an escalation timeout (Layer 2) does not require changing a verb definition (Layer 1). A change to how the user sees a KYC case (Layer 3) does not require changes to either.

### 2.7 No Nested Workflows

> A workflow verb may call **direct** verbs only. It may NOT call other workflow verbs.

This prevents recursive process spawning, ensures the BPMN layer remains a flat orchestration layer (not a nested process hierarchy), and keeps the execution model predictable. If a workflow needs to trigger another long-lived process, it does so via a message event (asynchronous handoff), not via a nested verb call.

### 2.8 "If It Isn't in the Runbook, It Didn't Happen"

All workflow progress must map to runbook entry status and events (Executing / Parked / Completed / Failed). No hidden state machines. The runbook is the single durable truth, for workflow verbs just as for direct verbs.

---

## 3. The Data Philosophy

This section exists because BPMN's data model is the source of almost every integration failure. It deserves dedicated attention.

### 3.1 The Problem: BPMN Variables Are Not Data

BPMN "process variables" are a persistence convenience, not a data model. They have:

- No schema (any key, any value, any time)
- No validation (write anything, discover the error later)
- No type continuity (what you write as an integer may come back as a long, a double, or a string depending on the engine's serialisation layer)
- No lifecycle (variables are created, mutated, and deleted ad hoc — there is no concept of "the payload at step N")
- No referential integrity (a variable that references an entity ID has no relationship to the entity it references, from the engine's perspective)

This is by design — BPMN is a flow specification, not a data specification. The variables mechanism exists to carry enough context for routing decisions, not to manage domain state.

**The mistake** in most BPMN integrations is treating process variables as if they *were* a data model — storing entity state in variables, transforming it with FEEL expressions, validating it with gateway conditions. This turns the process model into a de facto database and programming environment, which is exactly the "Java land colonisation" problem.

### 3.2 The Solution: Two-Namespace Payload

We solve this by defining a strict **two-namespace payload contract** for all workflow verb interactions:

**Namespace 1: Domain Payload (`domain.*`)**
- Typed, schema-governed data owned by the verb runtime
- Serialised as a **single opaque string** in one process variable (`domain_payload`) — canonical JSON string, or base64 of canonical JSON bytes
- The engine carries it but **never reads, writes, branches on, or transforms** any field within it
- Deserialised only by the verb runtime's callback handler when a sub-verb executes
- Validated at the boundary: the runtime checks the payload's integrity (schema version, hash) when it receives it back from the engine
- Accumulated by the verb runtime: each sub-verb receives the current domain payload, performs its transformation, and returns the updated domain payload

**Namespace 2: Orchestration Flags (`orch.*`)**
- Flat key-value pairs that the BPMN model is allowed to read for gateway conditions
- Set exclusively by the verb runtime (via sub-verb results)
- Limited to simple types: `string`, `boolean`, `integer`, `enum`
- Examples: `orch.review_outcome = "approved"`, `orch.docs_received = true`, `orch.escalation_required = false`
- The engine may branch on these. The engine may not compute, derive, or transform them.

**The hard rule:** BPMN gateway conditions may reference `orch.*` variables only. Any gateway condition that references `domain.*` (or any variable outside the `orch` namespace) is a violation of the architecture and must be caught by CI linting.

This separation means:
- The domain payload survives the BPMN round-trip as a typed, schema-governed, integrity-checked blob
- The engine has just enough information to make routing decisions (the flags) without interpreting domain semantics
- Domain logic stays in Rust. Orchestration logic stays in BPMN. They never cross.

### 3.3 Domain Payload Structure

The domain payload is a single JSON value with a defined envelope:

```
{
  "schema": "kyc.open-case/v1",
  "schema_hash": "sha256:a1b2c3...",
  "created_at": "2026-02-06T14:30:00Z",
  "last_modified_by": "kyc.request-documents",
  "data": {
    // The verb's typed entity state — whatever the verb schema defines
    "case_id": "uuid:...",
    "entity_ref": "Allianz GmbH",
    "case_type": "periodic-review",
    "documents_requested": [...],
    "review_decision": null
  },
  "sub_verb_trail": [
    // Accumulated audit of which sub-verbs have touched this payload
    { "verb": "kyc.create-case-record", "at": "...", "outcome": "ok" },
    { "verb": "kyc.request-documents", "at": "...", "outcome": "ok" }
  ]
}
```

The `data` field is the verb's typed entity state. The envelope (`schema`, `schema_hash`, `last_modified_by`, `sub_verb_trail`) provides integrity and audit. The entire structure is serialised into **one** process variable (`domain_payload`) as an **opaque string**. The engine never looks inside it.

#### Trail size policy

`sub_verb_trail` is useful for quick diagnosis, but it must not grow without bound (retries alone can multiply entries).

- Keep only a **minimal** trail in the payload: IDs + timestamps, optionally last *N* entries (recommended N=20).
- Store the full audit trail as **runbook events** in Postgres (authoritative, unbounded, queryable).
- Optionally include a `trail_ref` in the payload that points to the full audit stream (e.g., `"trail_ref": "runbook_entry:uuid/events"`).

### 3.4 Orchestration Flags Structure

Flat, top-level process variables that the BPMN model may reference:

```
orch_review_outcome = "pending"       // string enum
orch_docs_received = false            // boolean
orch_escalation_required = false      // boolean
orch_sub_verb_count = 2               // integer (informational)
```

These are written by the verb runtime's callback handler after each sub-verb completes. The BPMN model's gateway conditions reference these and only these.

### 3.5 Payload Lifecycle Through a Workflow

```
1. DISPATCH
   Verb runtime creates domain payload from verb args
   Verb runtime sets initial orchestration flags
   Verb runtime calls engine: start process with {domain_payload, domain_payload_hash, orch_*}

2. SERVICE TASK (sub-verb callback)
   Engine activates job, passes {domain_payload, domain_payload_hash, orch_*} to verb runtime
   Runtime validates domain_payload against domain_payload_hash
   Runtime deserialises domain_payload, validates schema
   Runtime executes sub-verb with domain_payload.data as input
   Sub-verb returns updated data + new orchestration flags
   Runtime re-serialises domain_payload (updated data, updated trail, new hash)
   Runtime returns {domain_payload, domain_payload_hash, orch_*} to engine

3. GATEWAY
   Engine reads orch_* flags (never domain_payload)
   Engine makes routing decision

4. REPEAT steps 2-3 for each subsequent service task / gateway

5. COMPLETION
   Engine reaches end event, passes final {domain_payload, domain_payload_hash, orch_*} to runtime
   Runtime validates domain_payload against hash
   Runtime deserialises domain_payload, validates integrity
   Runtime translates domain_payload.data into verb outcome
   Runtime updates runbook entry with outcome
```

At every boundary crossing (dispatch, callback, completion), the verb runtime validates the domain payload's integrity. If the engine has corrupted, truncated, or coerced the payload, the runtime detects it and fails the step with a clear diagnostic.

### 3.6 What About Large Data?

BPMN engines persist process variables in their own storage layer. Large data — documents, images, entity graphs with hundreds of nodes — should not be stored as process variable values.

**Rule of thumb:**

| Data Kind | In domain_payload.data? | How referenced? |
|---|---|---|
| Entity IDs, names, statuses | Yes | Direct value |
| Small structured metadata (timestamps, hashes, counts, enums) | Yes | Direct value |
| Document content, images, binary blobs | **No** | URI/ID reference in payload, actual content in platform storage |
| Large entity graphs (100+ nodes) | **No** | Root entity ID in payload, graph in platform database |
| Decision flags for BPMN routing | N/A (in orch_* flags) | Direct value |

The domain payload carries **references** to large data, not the data itself. The sub-verb that needs the actual content resolves the reference at execution time from platform storage. The BPMN engine never sees the large data — it only carries the reference.

### 3.7 Canonical Serialisation

To prevent subtle drift at the boundary:

- Domain payload is serialised as **canonical JSON** (deterministic key ordering at every nesting level).
  - Implement this explicitly (e.g., JCS / RFC 8785 style canonicalization) and test that identical payloads yield identical bytes across platforms/versions.
- The `schema_hash` is a SHA-256 of the canonical JSON of the `data` field
- The runtime validates the hash at every boundary crossing
- If the hash does not match, the sub-verb fails with `PayloadIntegrityError` — this is a hard failure, not a warning

JSON is chosen over CBOR or other binary formats because:
- The rest of the stack (verb args, runbook entries, MCP payloads) is JSON
- Debuggability: a human can read the payload in the engine's admin UI
- No additional serialisation dependency

### 3.8 Engine Encoding Rules (Make the Boundary Real)

Even if the BPMN model never "intentionally" reads `domain_payload`, engines can still **coerce and re-serialise** variables. To make the carrier boundary real:

- `domain_payload` MUST be stored as an **opaque string** — canonical JSON string, or base64(canonical JSON bytes)
- `domain_payload_hash` MUST be stored as a **separate process variable** (SHA-256 over the exact bytes of the chosen encoding) — this allows hash validation without deserialising the payload
- `orch_*` flags remain flat primitives for BPMN routing

**Hard rule:** BPMN expressions and gateways may reference `orch_*` only. They must never parse, branch on, or transform `domain_payload`.

This guarantees the payload you validate is the payload you produced.

---

## 4. What Changes in the Verb Registry

### 4.1 Direct Verb Definition (Unchanged Shape)

- Args schema
- Sentences (step / summary / clarify)
- Handler mapping (Rust function)
- Execution strategy: `direct` (implicit default)

### 4.2 Workflow Verb Definition (New)

In addition to the normal verb fields, workflow verbs declare:

- **Process key** — logical ID of the BPMN process definition. At dispatch, the adapter resolves this to a concrete deployed version/hash, and the runbook pins that resolved version for reproducibility.
- **Correlation scheme** — how to correlate: `runbook_entry_id` + `tenant_id` + optional business key
- **Payload mapping** — args → initial `domain_payload.data`, final `domain_payload.data` → verb outcome
- **Sub-verb allow-list** — which direct verbs the process is permitted to call back to (enforced at runtime)
- **Orchestration flags contract** — which `orch_*` flags this workflow produces (so the BPMN model can reference them)
- **Wait semantics** — fire-and-track (preferred) vs blocking (rare)
- **Policy surface** — retry policy declaration (max retries, backoff strategy, timeout duration) owned by the engine but declared by the verb author in the YAML. The verb runtime returns structured errors; the engine applies the declared policy to decide retry vs incident. This keeps retry logic out of Rust code while giving verb authors control over failure behaviour.
- **Compensation manifest** — which rollback sub-verbs exist for compensation handlers (if any)

### 4.3 Sub-Verb Composition

A workflow verb is a *composition* of direct verbs, orchestrated by BPMN:

> `kyc.open-case` (workflow) orchestrates:
> - `kyc.create-case-record` (direct) — create the case entity
> - `kyc.request-documents` (direct) — generate document requests
> - `kyc.assign-reviewer` (direct) — route to a human reviewer
> - *human task: reviewer examines documents*
> - `kyc.record-review-decision` (direct) — capture the outcome, set `orch_review_outcome`
> - `kyc.escalate-if-required` (direct) — conditional escalation, set `orch_escalation_required`

Each sub-verb is a normal DSL verb with its own YAML definition, its own sentence template, its own arg schema. The workflow verb's BPMN model sequences them. The sub-verbs know nothing about the process they participate in.

### 4.4 Nesting Constraint

> **A workflow verb may compose direct verbs. A workflow verb may NOT compose other workflow verbs.**

This single constraint prevents recursive process spawning, ensures the BPMN layer remains a flat orchestration layer, and keeps the execution model predictable. If a workflow needs to trigger another long-lived process, it does so via a message event (asynchronous handoff), not via a nested verb call.

---

## 5. Runtime Architecture (Components)

### 5.1 Strategy Router (Single Entry Point)

The runbook executor calls **one** router:

- If `Direct` → invoke Rust handler, return result
- If `Workflow` → package domain payload, set initial orchestration flags, start BPMN process, return tracking handle

No other code path is allowed to execute verbs. The router is the single point of dispatch. The runbook executor never knows which strategy was used.

### 5.2 BPMN Adapter (Engine Abstraction)

A thin interface that supports:

- Deploy/activate process definitions
- Start process instance with variables (`domain_payload` + `domain_payload_hash` + `orch_*` flags)
- Subscribe to lifecycle events (instance started/completed/failed, incidents)
- Implement "job worker" for service task callbacks
- Publish message signals (external callbacks, human task completions)
- Complete/fail jobs with updated variables

The adapter makes the engine swappable (Zeebe, Camunda 8, or equivalent) without infecting the domain runtime. The runtime depends on the adapter interface, not on a specific engine's API.

### 5.3 Job Worker (Service Task → Sub-Verb Callback)

#### 5.3.1 Idempotency and Deduplication (At-Least-Once Workers)

Most BPMN engines deliver service task jobs **at least once**. A job may be redelivered due to retries, timeouts, worker crashes, or incident recovery.

**Requirement:** every activated service task must carry a stable `job_key`, and the correlation store must persist:

- `job_key → sub_verb_invocation_id`
- the last known completion payload and orchestration flags (so the dedupe path can replay both)
- the failure classification (if the job failed)

Worker behaviour:
- If `job_key` is **new**: execute the sub-verb once, persist the result, then complete the job.
- If `job_key` is **already seen**: return the previously persisted completion payload and orchestration flags (or a safe noop), and complete the job without re-running the sub-verb.

Sub-verb behaviour:
- Accept an **idempotency key** (derived from `job_key`) and make side effects idempotent where possible (e.g., "create if absent", "upsert", "request docs if not already requested").

This is non-optional; without it, durable orchestration will randomly duplicate domain actions.

#### 5.3.2 Job Worker Execution Steps

When a BPMN service task activates, the adapter delivers a job. The job worker:

1. Identifies which **sub-verb** is requested (from the service task's type identifier)
2. Validates that the sub-verb is in the workflow verb's allow-list
3. Checks the dedupe store: if `job_key` already completed, short-circuit to stored result
4. Deserialises `domain_payload`, validates against `domain_payload_hash`
5. Executes the sub-verb as a direct Rust verb call
6. Merges the sub-verb's result into `domain_payload.data`
7. Appends to `domain_payload.sub_verb_trail` (respecting trail size policy)
8. Updates `orch_*` flags as specified by the sub-verb's result
9. Re-serialises `domain_payload` with updated hash
10. Persists the completion result, updated payload hash, and updated orch flags to the dedupe store
11. Returns `{domain_payload, domain_payload_hash, orch_*}` to the engine
12. On failure: returns a structured error (the engine decides retry vs incident based on the verb's policy surface)

### 5.4 Correlation Store (Durable Linking)

A durable mapping persisted in **the same Postgres database as the runbook** (not in-memory, not in a cache):

- `runbook_entry_id` ↔ `process_instance_id`
- `service_task_activation_id` ↔ `sub_verb_invocation_id`
- Plus tenant, pack, and runbook provenance

This is what lets the REPL say "Awaiting reviewer" without knowing anything about BPMN internals. It must survive agent restarts, engine restarts, and infrastructure failures. Postgres is the source of truth. Any in-memory caching (e.g., DashMap for in-flight job tracking) is a performance optimisation only — never the authoritative record.

### 5.5 Event Bridge (Unified Outcome Stream)

Translates BPMN lifecycle events into the standard `OutcomeEvent` model that the REPL/runbook already consumes:

| BPMN Event | OutcomeEvent |
|---|---|
| Process instance created | `DurableTaskStarted` |
| Service task completed (sub-verb) | `StepCompleted` (sub-verb level) |
| Human task assigned | `Parked(HumanGate, reason)` |
| Timer fired | Informational event |
| Message wait entered | `Parked(ExternalSignal, reason)` |
| Message correlation failed (dead-letter) | `CorrelationFailed(message_name, reason)` |
| Process completed | `ExecutionResult(ok)` with final domain payload |
| Process failed / incident | `ExecutionResult(err)` with incident metadata |

The REPL remains ignorant of BPMN specifics. It sees consistent events and sentences.

**Dead-letter visibility:** when an inbound message fails to correlate (zero or multiple matches), the event bridge MUST emit a `CorrelationFailed` runbook event with the message name, correlation key, and failure reason. The REPL can then surface "message delivery failed — manual resolution required" rather than silently losing the signal.

### 5.6 Completion Handler

When the BPMN process reaches its end event, the verb runtime:

- Receives the final `domain_payload`, `domain_payload_hash`, and `orch_*` flags
- Validates domain_payload against hash
- Deserialises and validates the domain payload (schema check)
- Translates `domain_payload.data` into the workflow verb's outcome type
- Updates the runbook entry: status → `Completed`, result populated
- Emits the `ExecutionResult` outcome event for the REPL
- Cleans up correlation state (marks as completed, retains for audit)

### 5.7 Progress Tracking

While a workflow verb is in-flight, the runtime can answer "where is this verb in its lifecycle?" by:

- Querying the correlation store for which sub-verbs have completed
- Querying the engine (via adapter) for active wait states
- Translating this into a progress model the runbook can display

The runbook's `EntryStatus` enum already includes `Executing` and `Parked`. Progress tracking enriches these with sub-step visibility when the verb is a workflow — without requiring the REPL to understand BPMN concepts.

---

## 6. Wait-State Semantics

The BPMN layer provides first-class support for states that the direct execution strategy cannot model cleanly.

### 6.1 Human Tasks

A step that waits for a person to act (review, approve, reject).

- The engine manages assignment, delegation, and escalation
- The verb runtime is notified of the outcome (via the adapter)
- The runbook entry shows `Parked` with reason "Awaiting reviewer"

**Where does the human act?** Two viable patterns:

**Pattern A — External Task UI (Fastest)**
Human tasks handled in the engine's own task-list UI. The REPL shows parked status plus a link/outcome. Good for early prototypes; less unified UX.

**Pattern B — REPL Surfaces Tasks (Preferred)**
REPL shows parked steps as actionable review tasks. User action posts to the verb runtime, which completes the task in the engine via the adapter. Keeps the user in one place and aligns with the "runbook is the product" philosophy.

This is a product decision. The architecture supports both by treating human task completion as an external signal delivered through the adapter.

### 6.2 Timer Events

A step that waits for a duration or deadline.

- The engine manages the clock
- The verb runtime is notified when the timer fires
- If the timer is a boundary event (attached to a human task), the engine takes the escalation path
- The runbook entry reflects the escalation status

### 6.3 Message Events

A step that waits for an external signal (document uploaded, external system callback, regulatory response).

- The engine manages message correlation (matching an incoming message to the correct process instance)
- The verb runtime publishes messages through the adapter when external events arrive
- The runbook entry shows `Parked` with reason "Awaiting document upload" (or equivalent)

**Correlation keys (required):** inbound messages MUST include `tenant_id` and `runbook_entry_id` (or a derived stable business key such as `case_id` / `document_request_id`). Correlation keys must be **stable and immutable** for the lifetime of the wait — do not use a variable that later changes.

**Dead-letter routing:** if an inbound message matches **zero** or **multiple** workflow instances, it must be routed to a dead-letter queue for manual resolution. Dead-lettered messages MUST generate a `CorrelationFailed` runbook event so the REPL can surface the failure.

### 6.4 Compensation

A step where partial completion must be rolled back.

- Process completes sub-verbs A, B, C successfully
- Sub-verb D fails with a non-recoverable error
- Engine triggers compensation handlers for C, B, A (in reverse order)
- Each compensation handler invokes a **rollback sub-verb** (e.g., `kyc.cancel-document-request`)
- Rollback sub-verbs are declared in the workflow verb's `compensation_manifest`
- Each rollback sub-verb receives the current domain payload and returns the rolled-back state
- The runbook entry status → `Failed` with a compensation audit trail in `domain_payload.sub_verb_trail`

Even if the MVP defers compensation, BPMN models should be designed with compensation boundaries from day one. Adding compensation after the fact requires re-designing the process model.

### 6.5 Cancel / Abort

Long-running workflow verbs must support explicit cancellation.

- User triggers cancel from the runbook step → `CancelRequested`
- Runtime calls adapter → engine cancels the process instance
- Runbook step transitions to `Cancelled` (or `Failed` with reason, if you prefer a smaller status set)

**Compensation note:** cancellation is not automatically compensation. Compensation is an explicit handler path, and must be designed with retries/at-least-once semantics in mind to avoid double-compensating. A cancelled process with no compensation handlers simply stops — any side effects from completed sub-verbs remain.

---

## 7. Lifecycle Patterns

### 7.1 Fire-and-Track (Default)

The most common pattern for workflow verbs. The runbook submits the verb, receives an acknowledgement, and tracks progress asynchronously.

- User confirms "Open KYC case for Allianz GmbH"
- Runbook entry status → `Executing`
- Runtime dispatches to BPMN, receives process instance ID
- REPL shows: "KYC case opened — in progress"
- As sub-verbs complete, progress events update the runbook entry
- When the process completes, runbook entry → `Completed` with full outcome

The user can close the REPL, come back days later, and see the current status of the parked step.

### 7.2 Human-in-the-Loop

A workflow verb that includes one or more human tasks.

- Process reaches a user task (e.g., "Review KYC documents")
- Engine assigns the task to a reviewer (via role, group, or specific user)
- Runbook entry status → `Parked` with reason "Awaiting reviewer"
- The REPL shows the parked status. The user cannot proceed past this step.
- Reviewer completes the task (via REPL or external task UI)
- Engine signals the verb runtime. Runtime updates the runbook entry.
- If approved: process continues (sub-verb sets `orch_review_outcome = "approved"`)
- If rejected: process takes the rejection path (which may invoke a compensation sub-verb)

### 7.3 Timeout and Escalation

A workflow verb with time-sensitive steps.

- Process reaches a step with a deadline (e.g., "Counterparty must respond within 5 business days")
- Engine starts a boundary timer event
- If the response arrives before timeout: normal continuation
- If the timer fires: engine takes the escalation path (which may invoke an escalation sub-verb, re-assign the task, or notify a supervisor)
- The runbook entry reflects the escalation status

---

## 8. How This Integrates with the REPL / Runbook Model

The REPL v3 architecture already models:

- **Execution modes:** `Sync` / `Durable` / `HumanGate`
- **Entry status:** `Executing` / `Parked` / `Completed` / `Failed`
- **OutcomeEvent stream** for UI updates

Workflow verbs map naturally onto this:

| Workflow Event | Runbook Mapping |
|---|---|
| Process started | Entry → `Executing`, mode → `Durable` |
| Human task assigned | Entry → `Parked`, reason → "Awaiting reviewer" |
| Timer/message wait | Entry → `Parked`, reason → context-specific |
| Sub-verb completed | Progress update (if sub-step visibility enabled) |
| Process completed | Entry → `Completed`, outcome from domain payload |
| Incident / failure | Entry → `Failed`, with error + compensation trail |
| Cancellation | Entry → `Cancelled`, with audit of completed sub-verbs |

The REPL remains ignorant of BPMN specifics. It sees consistent events and sentences. The only visible difference is that workflow verbs may take longer and may park for human interaction — but this is already modelled in the v3 runbook architecture.

---

## 9. Versioning and Evolution

### 9.1 Runbook Pins Verb and Process Versions

For reproducibility:

- Runbook entry pins `verb_version` (already in v3 via pack provenance)
- Workflow entries also pin `process_definition_version` (the deployed version at dispatch time)
- Correlation store ties process instances to pinned versions
- Domain payload carries `schema` version for integrity checking

### 9.2 In-Flight Instances

Default rule:

- In-flight process instances complete on their **original** process version
- New instances use the **latest deployed** version
- Migration of in-flight instances is a rare, explicit operational procedure — never automatic

### 9.3 Hollow BPMN Discipline Enforcement

Automated CI checks / linting for all BPMN models before deployment:

- **Forbid** script tasks (no inline code)
- **Forbid** expression-language evaluation of domain fields (no FEEL expressions referencing `domain_payload.*`)
- **Require** all gateway conditions reference `orch_*` variables only
- **Require** all service tasks reference sub-verbs in the workflow verb's declared allow-list
- **Forbid** direct database access, HTTP calls, or any external integration not mediated by the verb runtime
- **Warn** on process models with more than N service tasks (complexity signal)

These checks run on every BPMN model change and block deployment on violation.

---

## 10. What This Enables

### 10.1 Verb Authors Stay in Domain Land

A verb author writes domain logic — entity schemas, validation rules, state transformations. If the verb needs orchestration, the author declares it in YAML and provides a BPMN model that sequences sub-verbs. The author never writes retry logic, polling loops, timeout management, or human-task routing code.

### 10.2 Process Authors Stay in Orchestration Land

A process author designs BPMN models that sequence sub-verb calls, define wait states, and handle exceptional paths. The process author never writes domain logic, never validates data, never transforms entities. Service tasks are opaque calls to the verb runtime. Gateway conditions are limited to orchestration flags.

### 10.3 The REPL Stays Ignorant

The REPL, the runbook, and the user experience are completely unaware of whether a verb is direct or workflow. Same sentence, same runbook entry, same status progression, same outcome events.

### 10.4 Compliance Gets a Free Upgrade

Every sub-verb execution produces an audit record (it is a normal verb call through the standard runtime). The `domain_payload.sub_verb_trail` provides a data-level audit. The BPMN engine adds orchestration-level audit (when each step ran, who approved, whether timeouts fired). Combined, this produces a compliance trail that covers both *what happened to the data* (verb audit) and *how the process was followed* (orchestration audit) — without additional instrumentation.

### 10.5 Migration Path from "Everything Is Direct"

The architecture is backwards-compatible. Every existing direct verb continues to work unchanged. Workflow execution is opt-in per verb. An existing direct verb can be "promoted" to a workflow verb by:

1. Adding the execution strategy declaration to its YAML
2. Extracting its internal steps into sub-verbs (if they aren't already separate verbs)
3. Creating a BPMN model that sequences those sub-verbs
4. Deploying the model to the engine

No changes to the REPL, the runbook model, or any calling code.

---

## 11. Trait / Contract Sketch (Rust-Facing)

Intentionally minimal: the domain runtime defines the shape; the BPMN adapter is plug-replaceable.

```rust
enum ExecutionStrategy {
    Direct { handler: VerbHandlerId },
    Workflow {
        process_key: String,
        sub_verbs: Vec<VerbId>,
        compensation_verbs: Vec<VerbId>,
        orch_flags: Vec<OrchFlagContract>,
    },
}

/// The domain payload envelope carried through the BPMN process
struct DomainPayload {
    schema: String,                    // "kyc.open-case/v1"
    schema_hash: String,               // SHA-256 of canonical data JSON
    data: Value,                       // typed entity state
    sub_verb_trail: Vec<SubVerbRecord>,
}

trait VerbRuntime {
    fn execute_direct(&self, verb: VerbId, args: Value, ctx: ExecCtx) -> Result<Value>;
    fn start_workflow(&self, verb: VerbId, args: Value, ctx: ExecCtx) -> Result<WorkflowHandle>;
}

trait BpmnAdapter {
    fn start_process(&self, key: &str, domain_payload: DomainPayload, orch_flags: HashMap<String, Value>, corr: Correlation) -> Result<ProcessInstanceId>;
    fn complete_job(&self, job: JobId, domain_payload: DomainPayload, orch_flags: HashMap<String, Value>) -> Result<()>;
    fn fail_job(&self, job: JobId, err: StructuredError) -> Result<()>;
    fn publish_message(&self, key: &str, corr: Correlation, vars: HashMap<String, Value>) -> Result<()>;
}
```

Key property: the `BpmnAdapter` trait accepts a `DomainPayload` (typed envelope) rather than raw `Value`. The adapter is responsible for serialising it into the engine's variable format. The engine never sees the internal structure.

---

## 12. MVP Plan (Prove the Seam)

### MVP Target: `kyc.open-case` as a Workflow Verb

1. **Split into sub-verbs** (all direct):
   - `kyc.create-case-record` — create the case entity
   - `kyc.request-documents` — generate document requests
   - `kyc.assign-reviewer` — route to a human reviewer
   - `kyc.record-review-decision` — capture the outcome, set `orch_review_outcome`

2. **Build a BPMN model** that:
   - Sequences the sub-verbs
   - Includes a human task (review gate)
   - Has a timer escalation path (boundary event on the human task)
   - Uses only `orch_*` flags for gateway conditions
   - Contains zero domain logic

3. **Implement the runtime components:**
   - Strategy Router (dispatch based on verb definition)
   - BPMN Adapter (Zeebe/Camunda integration)
   - Job Worker (service task → sub-verb callback with payload validation and deduplication)
   - Correlation Store (Postgres-backed, durable)
   - Event Bridge (BPMN events → OutcomeEvent, including dead-letter visibility)

4. **Prove the data round-trip:**
   - Domain payload dispatched, carried through 4 service tasks + 1 human task, returned intact
   - Schema hash validates at every boundary crossing
   - Sub-verb trail accumulates correctly (respecting trail size policy)
   - Orchestration flags drive gateway decisions without touching domain payload
   - Job deduplication prevents duplicate sub-verb execution on retry

5. **Prove the REPL integration:**
   - User sees one runbook step "Open KYC case for Allianz GmbH"
   - Step parks for human review
   - Step resumes when reviewer acts
   - Step completes with full audit trail of sub-verbs
   - User never sees BPMN concepts

**Success criterion:** A user in the REPL experiences `kyc.open-case` identically to a direct verb, except that it parks for human review and later completes — with a full audit trail of sub-verb executions and a domain payload that has survived the BPMN round-trip with zero data loss.

---

## 13. What This Architecture Does NOT Do

- **Does not replace direct execution.** Most verbs are and should remain direct. Workflow is for verbs that genuinely need orchestration.
- **Does not put domain logic in BPMN.** The hollow BPMN principle is inviolable. Gateway conditions reference orchestration flags only.
- **Does not let the engine interpret domain data.** The engine carries `domain_payload` as an opaque value. It reads `orch_*` flags for routing. That is the full extent of its data interaction.
- **Does not create a dependency on a specific BPMN engine.** The verb runtime communicates through the `BpmnAdapter` trait. The engine behind it can be swapped.
- **Does not change the verb's external interface.** Direct and workflow verbs are invoked identically. Callers are unaware of the execution strategy.
- **Does not allow recursive process nesting.** Workflow verbs compose direct verbs only. Hard constraint.
- **Does not use FEEL expressions for domain decisions.** If a routing decision requires domain knowledge, a sub-verb computes the orchestration flag. The BPMN model branches on the flag.

---

## 14. Anti-Goals (Explicit)

- BPMN is not used as a scripting environment.
- Domain logic does not move into the process model.
- The REPL does not become a BPMN viewer; it remains runbook-centric.
- Workflow verbs do not spawn workflow verbs (nesting constraint).
- We do not require synchronous blocking calls for workflows (fire-and-track default).
- Process variables are not used as a domain data store; they carry an opaque payload and flat orchestration flags.

---

## 15. Open Questions (Carry Forward)

1. **Multi-tenancy.** If the same workflow verb serves multiple client groups with different approval chains, does each client group get a different BPMN model, or does one model handle variation via gateway conditions on orchestration flags? (Recommendation: parameterised single model where possible; per-tenant models only for fundamentally different workflows.)

2. **Human task UI.** External task-list (Pattern A) vs REPL task surface (Pattern B)? Architecture supports both. Product decision needed.

3. **Process governance.** Who authors BPMN models, who approves changes, how is the hollow discipline enforced beyond CI linting? (Recommendation: verb author owns the model; changes require code review like any other artifact.)

4. **Observability depth.** Pack-level "in progress / parked / done" vs detailed sub-step visibility ("3 of 5 steps complete, waiting for document upload")? (Recommendation: both, toggled by user preference or pack configuration.)

5. **Compensation MVP timing.** Should the first workflow verb include compensation handlers, or is it acceptable to defer compensation to a later phase? (Recommendation: design the BPMN model with compensation boundaries; implement the handlers in a later phase.)

6. **Payload size governance.** At what threshold does data move from inline values to URI references? (Recommendation: define a size budget per verb — e.g., 64KB for the serialised domain_payload — and enforce at the packaging boundary.)

---

## Appendix A — Why This Avoids "Java Land Colonisation"

Because BPMN cannot directly:

- Read or write databases
- Implement data transformations
- Compute domain decisions
- Evaluate FEEL expressions against domain entity fields
- Store or interpret domain state in process variables

It can only:

- Orchestrate calls to domain verbs (via service tasks → sub-verb callbacks)
- Wait for humans, timers, and messages
- Apply retry/escalation policy
- Branch on flat orchestration flags set by the verb runtime

That keeps the domain boundary intact while gaining the durable execution primitives the platform needs.

---

## Appendix B — Why the Two-Namespace Payload Matters

Consider the alternative: storing domain fields as individual process variables (the "natural" BPMN approach).

| Problem | What Goes Wrong |
|---|---|
| **Type coercion** | Engine serialises `case_id` (UUID) as string. On deserialisation, it comes back as a generic object. Downstream sub-verb expects UUID, gets ClassCastException (or Rust type error). |
| **Field collision** | Two sub-verbs both write a variable called `status`. The second silently overwrites the first. |
| **Schema drift** | Process model expects `entity_name`. Verb v2 renames it to `entity_ref`. Process variable still has `entity_name` from an in-flight instance. Null pointer at step 4. |
| **No integrity check** | A bug in the engine (or a manual intervention) modifies a process variable. There is no checksum, no schema validation, no way to detect the corruption until a sub-verb fails. |
| **Temptation to script** | With domain fields as top-level variables, process authors are one FEEL expression away from `if (case_type == "periodic-review") then ...` — and now domain logic is in the BPMN model. |

The two-namespace approach eliminates all five problems:

- Domain data lives in a single, hashed, schema-versioned blob. No type coercion, no field collision, no schema drift, integrity checked at every crossing.
- Orchestration flags are flat, simple, and explicitly contract-defined. No temptation to script — there is nothing complex enough to script *on*.

---

## Appendix C — Worker Execution Model: Durable Stack Frames (Forth Metaphor)

This appendix describes the **ob-poc-side execution model** used to integrate with a BPMN engine while keeping the domain runtime *deterministic, auditable, and "Forth-friendly."*

It is a metaphor and an implementation discipline: BPMN is not literally a stack machine, but the **job worker interface** can be treated as a **durable call/return protocol**.

### C.1 The Metaphor: BPMN Job Activations Are `CALL`, Job Completions Are `RET`

- **CALL (push):** the engine activates a service task and delivers a **job activation** to the worker.
- **RET (pop):** the worker completes (or fails) the job with updated variables.

This maps to a classic stack machine feel:
- job activation = "call subroutine with parameters"
- job completion = "return results to caller"

The key difference: the "stack" is not in memory — it is **persisted** (durable) so the system can resume after restarts and tolerate retries.

### C.2 The Three Durable Structures

#### 1) Runbook Entry (the user-visible macro frame)
The parent, user-facing unit of work:
- "Open KYC case …"
- Shows `Executing / Parked / Completed / Failed / Cancelled`
- Owns the audit trail (events) and correlation links

#### 2) Job Frame (the worker-visible stack frame)
A Job Frame represents an **activated service task** that must be executed exactly-once *logically* (even if delivered at-least-once physically).

Minimal fields:
- `job_key` (stable, engine-provided; **idempotency key**)
- `runbook_entry_id`
- `process_instance_id`
- `service_task_id` / `task_type`
- `sub_verb_id`
- `domain_payload_hash_in`
- `domain_payload_hash_out` (set on completion)
- `orch_flags_out` (the orchestration flags returned on completion — required for dedupe replay)
- `attempt_no`, `started_at`, `completed_at`
- `status` (Started / Completed / Failed / Incident)

#### 3) Parked Token (a wait-state marker, not a stack frame)
A Parked Token represents "the engine is waiting":
- Human task
- Timer
- Message wait (external callback / doc upload)

Parked Tokens are not frames because there is no executable "CALL" yet.
They become actionable only when a future event causes a new **job activation** (new frame).

### C.3 Parameter Stack: `domain_payload` Is the Carrier

The worker treats the payload envelope as the "parameter stack":

- Input parameters arrive via:
  - `domain_payload` (opaque string) + `domain_payload_hash`
  - `orch_*` routing flags
- The worker never relies on the engine to compute/transform parameters.
- The worker passes `domain_payload` into the sub-verb runtime and receives an updated `domain_payload` back.

**Invariant:** the worker validates `domain_payload_hash_in` before execution and produces `domain_payload_hash_out` on completion.
This gives you a concrete "stack discipline" for data integrity.

### C.4 Forth-Style Execution Rules (the Discipline)

#### Rule 1 — PUSH Is Persisted
On job activation:
1. Validate correlation (`tenant_id`, `runbook_entry_id`)
2. Check dedupe store: if `job_key` is already `Completed`, short-circuit to stored completion (including stored `orch_flags_out`)
3. Persist `JobFrame(status=Started)` with `domain_payload_hash_in`

#### Rule 2 — POP Is Idempotent
On job completion:
1. Persist completion outcome + `domain_payload_hash_out` + `orch_flags_out`
2. Complete the engine job
3. If the engine redelivers the same `job_key`, the worker returns the stored completion (payload + flags) without re-running the sub-verb

This is how you make "at-least-once delivery" behave like "exactly-once semantics" at the domain level.

#### Rule 3 — No Nested Workflows (No Dynamic CALLs)
A workflow verb may call **direct sub-verbs only**.
- No workflow verb may schedule another workflow verb.
- The worker enforces this using the sub-verb allow-list in the workflow verb definition.

This keeps the orchestration comprehensible and prevents runaway recursion.

#### Rule 4 — PARK Is Explicit, Not Hidden
When the engine reaches a human task/timer/message wait:
- Emit a runbook event: `Parked(reason, details)`
- Persist a Parked Token record (optional but recommended for querying wait states without engine calls)
- REPL can render: "Awaiting reviewer" / "Awaiting document upload" without BPMN knowledge

#### Rule 5 — RETURN Values Are Small and Typed
On completion, the worker returns:
- Updated `domain_payload` (opaque string) + `domain_payload_hash`
- Updated `orch_*` flags (flat primitives only)

Gateways branch only on `orch_*`, never on `domain_payload`.

### C.5 Why This Model Fits ob-poc (and the Esper/Forth Worldview)

- **Determinism:** the same job activation + payload yields the same sub-verb execution path.
- **Auditability:** each push/pop produces explicit runbook events and persisted frames.
- **Explainability:** the REPL can narrate progress at the runbook level while the worker keeps mechanical details.
- **Performance:** job execution is small, bounded work; large data stays referenced, not embedded.
- **Ergonomics:** engineers can reason in a "CALL/RET" mental model while BPMN remains the durable scheduler.

In short: BPMN provides durable control-flow; ob-poc preserves a Forth-like execution discipline at the boundary.

### C.6 Minimal Implementation Checklist

- [ ] Correlation store tables: `workflow_instance`, `job_frame`, `parked_token` (optional)
- [ ] Dedupe: `job_key` uniqueness constraint + stored completion payload + stored `orch_flags_out`
- [ ] Hashing: canonicalization + `domain_payload_hash_in/out`
- [ ] Worker contract: `CALL` (activate) → `PUSH` (persist) → execute sub-verb → `POP` (persist) → complete/fail job
- [ ] Event bridge: translate engine waits/incidents into runbook events (including dead-letter `CorrelationFailed`)
- [ ] Enforcement: reject any BPMN model that references `domain_payload` in expressions

---

## Appendix D — Camunda 8 Guard Rails (Zeebe-Specific Operational Constraints)

This appendix translates Camunda 8 / Zeebe semantics into **non-negotiable guard rails** for the ob-poc worker + runtime boundary.

The purpose is simple: make durable orchestration behave like your deterministic DSL runtime, even though the engine is distributed and delivers work at least once.

### D.1 Jobs Are At-Least-Once (Design for Duplicates)

Camunda 8 uses an **at-least-once strategy for job handlers**. A common failure mode is:
- Worker activates a job
- Worker crashes (or network partitions) before completing it
- After a configured timeout, the engine gives the job to another worker

**Guard rails:**
- Every job activation MUST be treated as **potentially duplicate**.
- Every service task MUST execute with **logical exactly-once semantics** implemented by:
  - `job_key` deduplication (persist `job_key → completion payload + orch_flags_out`)
  - Idempotency keys passed into sub-verbs (derived from `job_key`)
- Completing a job MUST be preceded by persisting the completion result (so a crash after completion can still be deduped).

### D.2 Retries, Incidents, and Operator-Facing Error Messages

In Camunda 8, a job failure decrements retries; when retries reach `0`, an **incident** is raised and requires human intervention (typically via Operate).

**Guard rails:**
- Worker failures MUST produce a **meaningful, operator-readable message** (it will be surfaced in Operate).
- "Retry vs incident" policy MUST be explicit:
  - Transient failures → fail job with retries remaining
  - Permanent schema/payload violations → fail job with retries set to `0` (force incident)
- The runtime should classify failures into:
  - `TransientExternal` — downstream service unavailable, network timeout
  - `TransientInfrastructure` — database connection lost, queue full
  - `PermanentContract` — payload integrity failure, schema mismatch, forbidden sub-verb
  - `PermanentBusiness` — invalid state transition, domain rule violation

### D.3 Backpressure and REST API Failure Modes

The orchestration REST API can respond with **backpressure** errors (e.g., `RESOURCE_EXHAUSTED`) and recommends retrying with backoff.

**Guard rails:**
- Any worker/adapter REST call that returns `RESOURCE_EXHAUSTED` MUST be retried with exponential backoff + jitter.
- `404 Not Found` for fail/complete job MUST be treated as "lost race":
  - Job already completed by another worker OR process canceled — record as benign and stop retrying.
- `409 Wrong State` MUST be treated as a strong signal the process is already in incident/canceled state; stop and surface as runbook event.

### D.4 Message Correlation Semantics (Name + CorrelationKey, Buffering, Cardinality)

Message correlation is based on **subscriptions** defined by:
- `message name`
- `correlationKey` (correlation value)

Key semantics:
- Subscriptions are opened when an instance awaits a message.
- The message name/correlationKey expressions are evaluated when the catch event activates.
- After a subscription is opened, it is **not updated** if the referenced variable later changes.
- Messages can be buffered with a TTL. A buffered message can be correlated later when a matching subscription opens.
- The **correlate message** endpoint is synchronous and **does not support buffering**.
- Message cardinality rules apply (e.g., only once per process across versions; if multiple subscriptions for the same process exist, the message correlates to only one; if subscriptions exist for different processes, the message can correlate to all of them).

**Guard rails:**
- Correlation keys MUST be stable and immutable for the lifetime of the wait (do not use a variable that later changes).
- Prefer correlation keys derived from durable IDs:
  - `runbook_entry_id`
  - `case_id`
  - `document_request_id`
- For buffered scenarios (docs may arrive "early"), publish messages with TTL > 0.
- For synchronous confirmation ("did it correlate now?"), use correlate-message; if not correlated, treat as a failure and emit a `CorrelationFailed` runbook event.

### D.5 Variable Mappings Must Not "Leak" the Domain Payload

Camunda supports input/output variable mappings on tasks/events. These are powerful — but can tempt model authors to transform data in the model.

**Guard rails:**
- Treat variable mappings as orchestration plumbing only.
- Do not map pieces of `domain_payload` into separate process variables for gateway branching.
- Gateways may reference `orch_*` flags only.
- Keep `domain_payload` as an opaque string; map it through unchanged.

### D.6 Process Version Pinning vs "Latest Version"

Operationally, Camunda lets you start the latest version by BPMN process ID. For auditability, ob-poc requires pinning.

**Guard rails:**
- Workflow verb definitions declare a logical `process_key`.
- The adapter MUST resolve it to a concrete deployed version/hash at start.
- Persist the resolved version/hash in:
  - the runbook entry
  - the correlation store
- In-flight instances complete on the pinned version; new instances use the latest resolved version.

### D.7 Observability: Runbook-Centric, Operate-Compatible

Operate is the operator lens; the REPL/runbook is the user lens.

**Guard rails:**
- Every engine-level state transition that matters MUST be bridged into runbook events:
  - Incident created/resolved
  - Job retries exhausted
  - Message correlated / timed out / dead-lettered
  - Cancellation requested/completed
- Include Operate identifiers as references (process instance key, incident key) but do not require users to navigate Operate for normal flow.

### D.8 Recommended Modelling Pattern for Long-Running Integrations

Camunda best practices distinguish between short synchronous work and longer async integrations.

**Guard rails:**
- For sub-verbs that are truly short (milliseconds), a single service task worker is fine.
- For any sub-verb that can block or depends on external systems, prefer:
  - A service task that triggers a request, then
  - A message catch (or receive task/event) that correlates the response later
- This keeps workers fast and reduces the chance of burning retries into incidents due to transient external slowness.

---

*This document defines the architecture for seamless DSL verb ↔ BPMN integration. It is a companion to the REPL Re-Engineering v3 spec. The next step is the MVP: implement `kyc.open-case` as a workflow verb, prove the domain payload round-trip, and demonstrate that the REPL experience is identical for direct and workflow verbs.*
