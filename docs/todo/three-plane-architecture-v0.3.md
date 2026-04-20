# Three-Plane Architecture — Vision & Scope (v0.3)

> **Status:** Revised proposal absorbing v0.2 peer review. Supersedes v0.1 and v0.2; preserves the destination topology correction in `docs/todo/sem_os_lift_out_plan.md`.
> **Date:** 2026-04-18
> **Prior versions:** v0.1 (architectural framing), v0.2 (capability disclosure + Sequencer + determinism).
> **v0.3 changes:** runbook/envelope ambiguity resolved; transaction ownership split (scope vs mechanics); durability model pinned (stage 9a in-txn, stage 9b via outbox); round-trip redefined as effect-equivalence; envelope carries versioning and replay anchors; `StateGateHash` concrete spec; dependency graph redrawn as two separate diagrams; entity resolution 2a/2b split to protect the control plane from NLP bleed-over.
> **Decision level:** architectural framing + migration plan. No code changes until this document is approved and Phase 0 artefacts are produced.

---

## 1. Purpose

Establish the target architectural frame for ob-poc and the migration path to reach it.

Three planes, three responsibilities:

- **SemOS** — the capability disclosure and governance plane.
- **DSL runtime** — the execution plane.
- **ob-poc** — the composition plane, including the Agentic Sequencer that connects the other two and owns the durability boundary.

This document is the destination specification. It is the input to an implementation-level TODO that a downstream Claude/Zed session will produce against the live codebase. It contains no file-level change lists; those belong in that TODO.

---

## 2. Vision

One sentence:

> **SemOS discloses which verbs are invocable on which entities at which DAG states, gates each invocation, and emits a gated envelope. The DSL runtime executes envelopes. The Agentic Sequencer in ob-poc sequences disclosure, gating, dispatch, and the durable feedback loop across a runbook of envelopes. These three concerns live in three crate families, with one-way dependencies and a single deterministic per-envelope handoff.**

Expanded:

- **SemOS is the capability disclosure engine.** Given utterance context and current DB state, SemOS resolves entities (from structured input, not free text), navigates the workspace DAG to each entity's current state node, derives the set of invocable verbs, gates the selected invocation, and applies declarative state advances inside a transaction the Sequencer opens. It never executes verbs and it never performs NLP.
- **The DSL runtime is the execution engine.** Given a gated envelope, it executes one verb inside a transactional scope the Sequencer supplies, and returns an outcome plus a declarative `PendingStateAdvance` for SemOS to apply in the same transaction. It never governs, discovers, or narrates.
- **The Agentic Sequencer is the composition engine.** It owns the nine-stage utterance-to-outcome loop, opens and commits transactions, iterates runbook steps with per-step re-gating, writes outbox rows for post-commit effects, and introduces no non-determinism.

---

## 3. Overarching principles

These principles govern every architectural decision in the refactor and every decision downstream of it. They are non-negotiable without explicit amendment.

### P1 — Separation of concerns along three planes

Each plane is authoritative within its domain and powerless outside it. SemOS cannot execute. The runtime cannot govern. The composition layer cannot redefine either contract. Overlap is a bug, not a feature.

### P2 — Capability disclosure is state-dependent

The verb surface at any given moment is a pure function of entity state in the workspace DAG. SemOS *derives* the surface; it does not publish a static menu. This is the mechanism that makes agent interaction bounded and deterministic: the agent never guesses what is invocable, because SemOS tells it.

### P3 — The control plane gates; the data plane executes; the composition layer sequences

No plane may take on another plane's role. If a test harness needs execution, it uses the runtime (or a mock of it) — not SemOS. If the runtime needs metadata, it is injected via registry — not pulled from SemOS at runtime. If language interpretation is needed, it happens in composition — never in SemOS.

### P4 — One-way dependencies

SemOS never imports the runtime. The runtime never imports SemOS. ob-poc imports both and is the only place they connect. `ob-poc-types` is the shared boundary-types crate and never accrues logic. Enforced at the `Cargo.toml` level by workspace deny-lints.

### P5 — The handoff is a value, not a callback

SemOS emits a `GatedVerbEnvelope` — a value — that the Sequencer carries to the runtime. Neither plane calls the other directly. The envelope is the entire contract between the planes. Anything not in the envelope does not cross the boundary.

### P6 — Determinism is a first-class invariant

Given a fixed DB snapshot and a fixed utterance, the verb surface, the verb selected, the gate decision, the runbook compiled, and the runtime outcome are byte-deterministic. Each plane has a local determinism obligation; the Sequencer has a "no non-determinism introduced here" obligation. See §9.

### P7 — Every verb is catalogued

Every runtime-executable verb has: a workspace DAG association, a state validity gate, an entity resolution shape, and a discovery signal. Uncatalogued verbs are not executable. Startup validation enforces this.

### P8 — Metadata is the source of truth for verb existence; code is the source of truth for execution mechanics

The two meet at the runtime boundary through a compiled registry. Dissolved-CRUD verbs are metadata + generic interpreter. Plugin verbs are metadata + registered `CustomOperation` impl. Both register through the same `VerbRegistrar` contract; the registry does not distinguish between them at dispatch time.

### P9 — Composition is an adapter pattern

Where the runtime depends on app-specific behaviour, ob-poc provides trait implementations of runtime-defined traits. The runtime defines no app types. `dsl-runtime` traits are expressed in terms of primitives and `ob-poc-types` only.

### P10 — Evolution is additive by default

New verbs add catalogue entries and (when needed) registry entries. Structural changes to the control-plane/data-plane contract — trait shape, envelope shape, registration contract — require explicit versioning and a deprecation cycle. After this refactor lands, the contract is frozen until a deliberate version bump.

### P11 — Durability is a first-class invariant (new in v0.3)

Runtime writes and SemOS state advances ride a single transaction. Post-commit external effects (narration, UI push, broadcast, cross-system notifications) ride an outbox with at-least-once delivery. No committed-writes-with-stale-control-plane failure mode is permitted. See §10.7.

---

## 4. Context — ground truth

### 4.1 Prior milestones

- `execute_json(&mut VerbExecutionContext) → VerbExecutionOutcome` migration complete for all 625 `CustomOperation` implementations.
- `VerbExecutionPort` trait defined in `sem_os_core::execution`.
- `PgCrudExecutor` in `sem_os_postgres` — 14/14 CRUD operations implemented.
- `ObPocVerbExecutor` adapter + `VerbExecutionPortStepExecutor` bridge wired into production startup.
- Pub API surface cleanup (Tier A) complete.
- 20 active crates (14 ob-poc-family + 6 sem_os-family); 1,194 passing unit tests.
- ESPER navigation, workspace DAG model, session stack machine, Motivated Sage architecture in place.

### 4.2 Ground-truth findings

1. Crate-dependency direction is already correct.
2. Ownership is inverted at the runtime level — 625 ops in `ob-poc/domain_ops`.
3. The `execute_json` shim still calls legacy `execute()` via `execute_json_via_legacy()`.
4. `VerbExecutionPort` (in `sem_os_core::execution`) and `PgCrudExecutor` (in `sem_os_postgres`) are architecturally misplaced.
5. The orchestrator is not a named component — highest-risk gap.
6. Discovery pipeline (1,100 → 5–60 narrowing) is not explicitly positioned; belongs in the Sequencer.
7. **Durability story was ambiguous in v0.1/v0.2** — stage 9 failures could commit business writes while leaving SemOS state stale. Resolved in this revision (§10.7).

---

## 5. Architectural principle

> **SemOS is a capability disclosure engine over a state-dependent verb surface. Given current entity state in a workspace DAG, SemOS resolves structured entity references, navigates the DAG, derives the set of invocable verbs, gates the selected invocation, and emits a gated envelope carrying resolved entities, DAG position, state snapshot, authorised args, discovery signals, version anchors, and a TOCTOU fingerprint. The DSL runtime consumes the envelope, executes one verb inside a Sequencer-supplied transaction, and returns a `PendingStateAdvance` the Sequencer applies to SemOS inside the same transaction. Post-commit external effects ride an outbox. The Agentic Sequencer in ob-poc sequences all of this. No plane may take on another plane's role.**

Consequences:

- Any code that executes a verb does not belong in `sem_os_*`.
- Any code that describes, governs, discovers, or catalogues a verb does not belong in `dsl-runtime`.
- Any code that sequences utterance-to-outcome, opens transactions, or writes outbox rows does not belong in either plane — it belongs in the Sequencer.
- Any code that interprets natural language does not belong in the control plane.
- The handoff between control and data is a single typed value, dispatched at a single call site per runbook step.

**Analogy.** Capability-based disclosure with policy gating, closer to capability-based kernels (seL4, Capsicum) than to Istio. The capability set at any moment is derived from state, disclosed on demand, and carried as a value. The transactional-outbox pattern on top handles the durable coupling between in-DB writes and external effects.

---

## 6. Why now

The `execute_json` migration created the contractual separation. Without the structural move now:

- The shim (`execute_json_via_legacy()`) has no endpoint to collapse into.
- New ops continue landing in `ob-poc/domain_ops` out of habit.
- The first reviewer who reads `sem_os_core::execution::VerbExecutionPort` concludes SemOS owns execution.
- Downstream consumers inherit the inversion.
- The Sequencer remains un-named and accretes coupling.
- The durability story stays ambiguous, and the committed-writes-with-stale-control-plane failure mode remains latent.

---

## 7. The three planes — capability split

### 7.1 Plane definitions

| Plane | Crates (target) | Owns | Does not own |
|---|---|---|---|
| **Control (SemOS)** | `sem_os_core`, `sem_os_postgres` (metadata only), `sem_os_server`, `sem_os_client`, `sem_os_harness`, `sem_os_obpoc_adapter` | verb catalogue, workspace DAG, taxonomy registries, deterministic entity resolution (from structured input), state-machine navigation, verb-surface derivation, gating (ABAC + state validity + session scope), narration hints, phrase bank, allowed-verb fingerprints, constellation rehydration metadata, change-set lifecycle, application of `PendingStateAdvance` within a Sequencer-supplied transaction | verb execution, pool management, NLP / embedding, transaction scope decisions, outbox drainage, UI push, utterance interpretation |
| **Execution (DSL runtime)** | `dsl-core` (unchanged — parser/AST/compiler, DB-free), **new `dsl-runtime`** | `VerbExecutionPort`, `CustomOperation`, `CustomOperationRegistry`, `VerbRegistrar`, `PgCrudExecutor`, **transaction mechanics within a scope supplied by the Sequencer** (statement execution, row accounting, deadlock retries inside the scope), the domain-neutral ops, macro-expansion runtime | verb governance, verb discovery, session scope, UI, narration, catalogue storage, **transaction scope decisions** (when to commit, how long to hold locks), outbox |
| **Composition (ob-poc)** | `ob-poc`, `ob-poc-web`, `ob-poc-ui-react`, `dsl-lsp`, app services, repositories, **Agentic Sequencer**, **outbox drainer**, app-coupled op adapters | HTTP/REST, session lifecycle, the Agentic Sequencer (§8), **transaction scope ownership** (opening, committing, rolling back), **outbox writing and drainage**, repositories, domain services, app-coupled op adapters, embedding / NLP / utterance interpretation, the unified REPL pipeline | verb contracts, execution mechanics, catalogue storage, state-advance semantics |

**Transaction ownership (reconciled from v0.2):** the Sequencer owns *scope* — when a transaction begins, when it commits, when it rolls back, what the boundary includes. The Runtime owns *mechanics* — statement execution, pool checkout, row accounting, deadlock retries — inside the scope the Sequencer supplies. Both are primary owners of non-overlapping concerns. Neither can decide the other's concern.

### 7.2 Agent-side capabilities (where they live)

"Agent side" is a view onto the three planes, not a fourth plane.

| Agent capability | Plane | Crate home |
|---|---|---|
| Utterance interpretation (free text → structured entity references + verb intent) | Composition | `ob-poc` — embeddings, BGE-small, NLP all live here; never cross into `sem_os_*` or `dsl-runtime` |
| Deterministic entity resolution (structured refs → canonical entity ids via taxonomy) | Control | `sem_os_core` — pure function, no NLP, no embeddings |
| DAG navigation (entity ids → current state nodes) | Control | `sem_os_core` |
| Verb surface disclosure (state nodes → candidate verb set) | Control | `sem_os_core` |
| NLP / embedding match against the surface | Composition | `ob-poc` |
| Runbook compilation (surface match + session intent → ordered envelope sequence) | Composition | `ob-poc` — Sequencer |
| Gate (ABAC + state validity + session scope) | Control | `sem_os_core` |
| Transaction scope management | Composition | `ob-poc` — Sequencer |
| Dispatch | Composition → Execution | Sequencer calls `dsl-runtime::VerbExecutionPort` |
| Execution (transaction mechanics) | Execution | `dsl-runtime` |
| State advance (declarative payload from outcome) | Control | `sem_os_core` — applied within Sequencer's transaction |
| Outbox write | Composition | `ob-poc` — Sequencer |
| Outbox drainage | Composition | `ob-poc` — drainer task |
| Narration synthesis | Control | `sem_os_core` — from outbox row |
| UI push / broadcast | Composition | `ob-poc-web` — from outbox row |

**Rule:** embedding and NLP machinery is Composition-only. SemOS APIs are structured-in, structured-out — they never accept free text. Entity resolution is the canonical boundary: the orchestrator does language-to-structured (stage 2a); SemOS does structured-to-canonical (stage 2b).

### 7.3 Dependency graphs (two diagrams — v0.3 split)

The v0.2 single diagram conflated Cargo dependency direction with runtime data flow. These are separated now.

**Diagram A — Cargo dependency graph** (arrow `A → B` means A's `Cargo.toml` imports B):

```
                   ob-poc-types (no deps)
                     ▲        ▲
                     │        │
                     │        │
            sem_os_core       dsl-runtime
                     ▲        ▲
                     │        │
           sem_os_postgres  dsl-core ◀──── dsl-runtime
          (metadata only)
                     ▲        ▲
                     │        │
                     └────────┴─────────────── ob-poc
                                          (imports both)
```

Key Cargo rules (enforced by workspace lint L1, §Appendix B):

- `sem_os_*` MUST NOT import `dsl-runtime`, `dsl-core` (beyond AST types if needed), `ob-poc`, or any ob-poc-adjacent crate.
- `dsl-runtime` MUST NOT import any `sem_os_*` crate or `ob-poc`.
- `dsl-core` MUST NOT import any DB crate (`sqlx`, `tokio-postgres`, `diesel`).
- `ob-poc-types` MUST NOT import anything except std and minimal ecosystem types (`uuid`, `chrono`, `serde`).

**Diagram B — Runtime data flow** (arrow = runtime data movement; NOT a Cargo edge):

```
             utterance
                 │
                 ▼
    ┌──────────────────────────┐
    │    Agentic Sequencer     │◀──── outbox drainer ◀── outbox table
    │   (ob-poc::sequencer)    │             │
    └──────────────────────────┘             ▼
        │     │     │     │          narration / UI push
        │     │     │     │
        ▼     ▼     ▼     ▼
     SemOS (structured queries, gate decision, state advance)
                   │
                   │   GatedVerbEnvelope (value)
                   ▼
        ┌──────────────────────┐
        │    dsl-runtime       │
        │   (executes within   │
        │  Sequencer-supplied  │
        │       txn scope)     │
        └──────────────────────┘
                   │
                   │   VerbExecutionOutcome + PendingStateAdvance
                   ▼
        Sequencer applies PendingStateAdvance via SemOS API
                   │
                   ▼
        Sequencer commits txn, writes outbox rows for 9b
```

---

## 8. The Agentic Sequencer

The Sequencer is a first-class architectural component. It is the only code that crosses the bi-plane boundary; it is the only code that opens transactions; it is the only code that writes outbox rows.

### 8.1 Position

Lives in `ob-poc` as a bounded module (`ob-poc::sequencer`). Not a crate. Not plural — one Sequencer contract; session-scoped state is injected.

### 8.2 Stages (revised from v0.2)

The Sequencer runs a fixed nine-stage pipeline per utterance. Stage 2 splits into 2a/2b to protect the control plane from NLP. Stage 8 is an inner loop over runbook steps. Stage 9 splits into 9a (in-txn) and 9b (async via outbox).

```
1.  Utterance receipt          (input: raw text, session id)
2a. Utterance interpretation   (Orchestrator, NLP: text → structured (type,name,scope) triples + verb intent)
2b. Entity resolution          (SemOS, deterministic: triples → canonical entity ids)
3.  DAG navigation             (SemOS: entity ids → current state nodes)
4.  Verb surface disclosure    (SemOS: state nodes → candidate verb set, 5–60 verbs)
5.  NLP match                  (Orchestrator: utterance + verb intent + surface → selected verb + arg binding)
6.  Gate decision              (SemOS: selected verb + args + session → GatedVerbEnvelope or rejection)
7.  Runbook compilation        (Orchestrator: envelope(s) → ordered runbook of pre-gated envelopes with conditional edges)
8.  Dispatch loop              (Orchestrator opens txn; for each runbook step: TOCTOU recheck → dispatch one envelope → capture outcome + PendingStateAdvance → apply PendingStateAdvance via SemOS in same txn)
9a. Commit                     (Orchestrator: commit txn. SemOS state + runtime writes commit atomically, or both roll back)
9b. Post-commit effects        (Outbox drainer consumes outbox rows written during stage 8: narration synthesis via SemOS, UI push, broadcast)
```

**Stage 7 produces a runbook; stage 8 iterates envelopes.** The runtime never sees a runbook — it sees one envelope per call. Runbook is orchestration metadata; it does not cross into `dsl-runtime`. This resolves the v0.2 ambiguity.

**Per-step TOCTOU recheck.** Each envelope carries a `StateGateHash` computed at stage 6. Inside stage 8's loop, before dispatch, the Sequencer (or the runtime at transaction boundary — see Open Q3) recomputes the hash against current locked state and fails closed on mismatch. Runbooks with multiple steps therefore tolerate state advancing between steps only in ways the DAG sanctions; any out-of-band change aborts the runbook.

### 8.3 Stage contracts

Each stage has:

- A typed input and output (no `serde_json::Value` pass-through).
- An explicit error shape (`UtteranceInterpretationError`, `EntityResolutionError`, `DagNavigationError`, `SurfaceEmpty`, `NlpNoMatch`, `Gated { reason }`, `RunbookCompilationError`, `Toctou`, `DispatchError`, `StateAdvanceError`, `CommitError`, `OutboxDrainError`).
- A determinism obligation (§9).
- A test fixture in the harness.

### 8.4 Orchestrator obligations (revised)

- **No non-determinism introduced.** No retries at Sequencer level (retries happen inside runtime mechanics, hidden from outcome). No speculative dispatch. No iteration over unsorted collections that affects output.
- **Single dispatch site.** `dsl-runtime::VerbExecutionPort::execute_json` has exactly one caller in the codebase: the Sequencer's stage-8 inner loop. Workspace lint enforces after Phase 5.
- **Transaction scope ownership.** Sequencer opens the transaction at start of stage 8, passes a scope handle into each dispatch, applies `PendingStateAdvance` via SemOS inside the scope, commits at stage 9a, rolls back on any stage-8/9a error.
- **Outbox ownership.** All post-commit effects are written to the outbox table inside the stage-8 transaction. The Sequencer never directly performs external side-effects.
- **Observability.** Each stage emits a structured event (stage id, input hash, output hash, trace id, duration). Stages 8 and 9b additionally emit per-envelope and per-outbox-row events.
- **Closed-loop invariant.** After stage 9a commits, the Sequencer confirms `writes_since_push` is advanced and the constellation is marked for rehydration. Stage 9b narration fires from the outbox.

### 8.5 Assumption A1 (flagged for review)

> **SemOS stage-9a state advance is DB-only.** All external effects (UI push, cross-system notifications, narration synthesis and delivery, constellation broadcast) route through the outbox in stage 9b. There is no SemOS state advance that requires external effects in the inner transactional path.

If any SemOS operation requires external effects inside the dispatch loop, it must either (a) be refactored to defer via outbox, or (b) force a re-design of 9a. The Phase 0 ownership matrix will test this assumption across every `PendingStateAdvance` shape; any violation is a blocker.

### 8.6 Mapping to conversational tollgates (2026-04-20 addition)

§8.2's nine-stage pipeline is described "per utterance," but in practice the REPL runs as an **outer conversational tollgate state machine** (ScopeGate → WorkspaceSelection → JourneySelection → InPack → Clarifying → SentencePlayback → RunbookEditing → Executing). Different tollgates activate different stage subsets; a single utterance does not necessarily flow through all nine stages. This is not a contradiction with §8.2 — it's the necessary reconciliation between the dispatch-loop model (stages 1–9 per *verb invocation*) and the conversational model (tollgates per *user turn*). The full mapping:

| V&S stage | Consumed by tollgate(s) | Current home in `rust/src/repl/orchestrator_v2.rs` |
|---|---|---|
| 1 Utterance receipt | all | `process()` top (lines 763–788) |
| 2a NLP interpretation | InPack | inside `handle_in_pack` via `IntentService` |
| 2b Entity resolution | ScopeGate, WorkspaceSelection, InPack | `handle_scope_gate` + `LookupService` + SemOS |
| 3 DAG navigation | (post-write rehydrate) | `rehydrate_tos` — called after writes, not inside a stage-numbered flow |
| 4 Verb surface disclosure | InPack, JourneySelection | inside `handle_in_pack`, reused from `handle_journey_selection` |
| 5 NLP match | InPack | `VerbSearchIntentMatcher` via `handle_in_pack` |
| 6 Gate decision | InPack → SentencePlayback transition | `ObPocVerbExecutor` adapter pre-flight |
| 7 Runbook compilation | SentencePlayback → RunbookEditing transition | `rust/src/runbook/compiler.rs` — already its own module |
| 8 Dispatch loop | Executing | `handle_executing` → `DslExecutorV2::execute_v2` |
| 9a Commit | Executing (but ownership inverted — see §8.4 G1) | **currently inside `DslExecutor`**, not the Sequencer. Phase 5c moves it. |
| 9b Post-commit effects | Executing (not implemented as outbox) | narration fires inline in `process()` after rehydrate; outbox drainer is Phase 5e |

**Implication for Phase 5b (Sequencer extraction).** The structural rename — `repl/orchestrator_v2.rs` → `ob-poc::sequencer` — preserves the tollgate state machine as the outer loop and names stages within tollgate handlers. It does NOT collapse tollgates into a single pipeline. The 9-stage contract §8.2 describes applies *within* tollgates and across the tollgate chain, not as a single linear flow per utterance.

**Implication for Phase 5c (txn ownership) and 5e (outbox).** Two gaps flagged in §8.4 — stage 9a commit currently lives inside `DslExecutor`, and stage 9b runs inline instead of via outbox — are both deferred. Phase 5b narrow extraction does not resolve these; their resolution lands in 5c and 5e respectively. The Sequencer's doc comments after 5b must disclaim 9a ownership and 9b outbox semantics, not claim them.

### 8.7 Session serialization (2026-04-20 addition)

The current `ReplOrchestratorV2::process()` holds `sessions.write().await` across the whole turn (single writer lock over the in-memory session map). **This is intentional.** A REPL is a conversational medium; concurrent writes against the same session map would race tollgate transitions and corrupt the state machine. Phase 5b preserves this serialization. The observable consequence — one process-wide write lock per turn — is an ergonomic ceiling the Sequencer is not trying to remove. If multi-session concurrency becomes a goal, the lock becomes per-session (hash-keyed) in a dedicated phase, not as a side effect of Phase 5b extraction.

---

## 9. Determinism as a first-class invariant

### 9.1 Statement

> Given a fixed DB snapshot and a fixed utterance, the verb surface disclosed in stage 4, the verb selected in stage 5, the gate decision and envelope produced in stage 6, the runbook compiled in stage 7, and the outcome emitted in stage 8 are byte-deterministic.

Testable: same `(snapshot, utterance)` → byte-identical stage outputs across runs and machines.

### 9.2 Per-plane obligations

| Plane | Obligation |
|---|---|
| SemOS | Surface derivation and gate decisions are pure functions of (DB snapshot, session context, structured input). No wall-clock, no random, no thread-local state. Phrase-bank lookup `BTreeMap`-ordered. `PendingStateAdvance` is a pure function of outcome + envelope. |
| Runtime | `CustomOperation` implementations are pure over (envelope, pool handle). Postgres non-determinism (row order, `NOW()`, sequence advancement) is stabilised in fixtures. Runtime internal retries on deadlock are allowed but must not affect outcome contents. |
| Sequencer | No retries at stage boundaries. No speculative dispatch. No iteration over unsorted collections affecting output. NLP match is deterministic given (utterance, surface) — embedding ties broken by lexicographic ordering. Outbox row writes are deterministic in both content and ordering within a transaction. |

### 9.3 Known non-determinism sources

- `HashMap`, `HashSet` iteration → `BTreeMap` / `BTreeSet` on output-affecting paths.
- `SystemTime::now()`, `chrono::Utc::now()` → injected `Clock` port, mockable.
- `rand::*` → injected `Rng` port.
- Postgres `NOW()`, `gen_random_uuid()`, default sequences → explicit values where outcome-visible.
- Threaded iteration order → deterministic collectors.
- `f64` accumulation order → deterministic reduce order for financial values.

### 9.4 Test strategy

- `determinism_harness` crate (new): runs the full pipeline twice per fixture and byte-compares stage outputs 4, 5, 6, 7, 8 plus stage-9a `PendingStateAdvance` plus outbox row set.
- Blocks every PR gate.

---

## 10. Target architecture — details

### 10.1 Type relocations

| Type | From | To | Rationale |
|---|---|---|---|
| `VerbExecutionPort` trait | `sem_os_core::execution` | `dsl-runtime` | Executors implement it; SemOS doesn't. |
| `CustomOperation` trait | `ob-poc` | `dsl-runtime` | Core runtime contract. |
| `CustomOperationRegistry` | `ob-poc` | `dsl-runtime` | Owned by the runtime. |
| `VerbRegistrar` trait (new / extracted) | — | `dsl-runtime` | Canonical registration path. |
| `PgCrudExecutor` | `sem_os_postgres` | `dsl-runtime` | Interprets metadata at runtime. |
| `VerbExecutionContext` | various | `dsl-runtime` | Runtime-owned context. |
| `VerbExecutionOutcome` (+ `PendingStateAdvance`) | various | `dsl-runtime` | Runtime output (§10.3). |
| `GatedVerbEnvelope` | — (new) | `ob-poc-types` | Bi-plane boundary type; preserves one-way deps. |
| `TransactionScopeId` | — (new) | `ob-poc-types` | Correlation ID only. Pure data, storage-backend-agnostic. |
| `TransactionScope` (trait) | — (new) | `dsl-runtime::tx` | Handle the Sequencer supplies; runtime executes mechanics inside it. **Lives in `dsl-runtime`, not `ob-poc-types`** — see correction note at §10.3 below. |
| `PendingStateAdvance` | — (new) | `ob-poc-types` | Declarative state mutation payload returned from runtime, applied by SemOS in same txn. |
| `OutboxRow` schema | — (new) | `ob-poc-types` + SQL migration | Outbox table. |
| `StateGateHash` | — (new, concrete) | `ob-poc-types` | Deterministic fingerprint (§10.5). |

`sem_os_postgres` retains all metadata persistence. It stops hosting execution infrastructure.

### 10.2 Crate naming (resolved)

**Decision:** `dsl-runtime`. Alternative `verb-runtime` rejected — speculating crate names against hypothetical non-DSL callers is a cost not a benefit. Revisit if that caller materialises.

### 10.3 The gated envelope and the outcome (expanded in v0.3)

Bi-plane contract is two values: `GatedVerbEnvelope` (SemOS → Runtime) and `VerbExecutionOutcome` (Runtime → Sequencer, carrying `PendingStateAdvance` for Sequencer → SemOS).

```rust
// in ob-poc-types::gated_envelope

/// The single value passed from SemOS (control plane) to dsl-runtime (data plane),
/// carried by the Agentic Sequencer. Fully deterministic given (snapshot, utterance).
#[derive(Debug, Clone)]
pub struct GatedVerbEnvelope {
    // --- versioning and replay (new in v0.3) ---
    pub envelope_version: EnvelopeVersion,           // u16 contract version, starts at 1
    pub catalogue_snapshot_id: CatalogueSnapshotId,  // SemOS catalogue revision used for gating
    pub trace_id: TraceId,                           // correlation across stages, outbox, replay

    // --- identity and position ---
    pub verb: VerbRef,                               // canonical verb id in the catalogue
    pub dag_position: DagNodeId,                     // current state node invocation originates from
    pub resolved_entities: ResolvedEntities,         // entity id → handle + state snapshot ref
    pub args: VerbArgs,                              // typed, validated args (never serde_json::Value)

    // --- authorisation and TOCTOU ---
    pub authorisation: AuthorisationProof,

    // --- discovery and closed-loop ---
    pub discovery_signals: DiscoverySignals,         // phrase bank entry used, narration hints
    pub closed_loop_marker: ClosedLoopMarker,        // writes_since_push at gate time
}

#[derive(Debug, Clone)]
pub struct AuthorisationProof {
    pub issued_at: LogicalClock,                     // NOT wall-clock; monotonic per session
    pub session_scope: SessionScopeRef,
    pub state_gate_hash: StateGateHash,              // concrete spec in 10.5
    pub recheck_required: bool,                      // true when hash must be re-verified pre-dispatch
}

/// Runtime output — consumed by Sequencer, which drives stage 9a and writes outbox.
#[derive(Debug, Clone)]
pub struct VerbExecutionOutcome {
    pub trace_id: TraceId,
    pub result: OutcomeResult,                       // success rows / returned values / error
    pub pending_state_advance: PendingStateAdvance,  // declarative payload for SemOS
    pub side_effect_summary: SideEffectSummary,      // sequence advances, trigger firings, audit writes
    pub outbox_drafts: Vec<OutboxDraft>,             // post-commit effects queued for 9b
}

/// Declarative state mutation applied by SemOS within the Sequencer's transaction.
/// Pure data — no logic. SemOS interprets.
#[derive(Debug, Clone)]
pub struct PendingStateAdvance {
    pub state_transitions: Vec<StateTransition>,     // DAG node movements per entity
    pub constellation_marks: Vec<ConstellationMark>, // rehydration markers
    pub writes_since_push_delta: u64,
    pub catalogue_effects: Vec<CatalogueEffect>,     // catalogue-level side effects (rare)
}

/// A post-commit effect queued in the outbox within the stage-8 transaction.
#[derive(Debug, Clone)]
pub struct OutboxDraft {
    pub effect_kind: OutboxEffectKind,               // Narrate, UiPush, Broadcast, Notify, etc.
    pub payload: serde_json::Value,                  // effect-specific data
    pub idempotency_key: IdempotencyKey,             // drainer uses for at-least-once semantics
}

// `TransactionScope` — the trait — lives in `dsl-runtime::tx`, NOT
// `ob-poc-types`. `ob-poc-types` carries only the correlation id:
//
//   // in ob-poc-types
//   pub struct TransactionScopeId(pub Uuid);
//
// The executor-access method lives with the trait in dsl-runtime:
//
//   // in dsl-runtime::tx
//   pub trait TransactionScope: Send + Sync {
//       fn scope_id(&self) -> TransactionScopeId;
//       fn executor(&mut self) -> &mut dyn sqlx::PgExecutor;   // Phase 5c
//   }
//
// ob-poc supplies a concrete `PgTransactionScope` wrapping
// `sqlx::Transaction`. This layering keeps `ob-poc-types` logic-free
// and sqlx-free; see the 2026-04-20 architectural correction below.
```

**2026-04-20 architectural correction.** The original v0.3 draft placed `TransactionScope` in `ob-poc-types` with a `fn executor(&mut self) -> &mut dyn PgExecutor` method. That was a latent contradiction: `ob-poc-types` is supposed to be logic-free and carry values only, but an executor-access method forces it to depend on `sqlx` (or whatever future backend). The contradiction was caught during Phase 0b (the trait was defined `scope_id()`-only and `executor()` deferred) and fully resolved at the start of Phase 5c: the trait moved to `dsl-runtime::tx`. The boundary crate now carries only `TransactionScopeId` (a pure `Uuid` newtype). This preserves one-way deps, keeps future backend-swap possible, and removes the habit risk of turning `ob-poc-types` into a shadow architecture crate. Nothing else in the bi-plane boundary type set references the `TransactionScope` trait directly (envelopes, outcomes, state-advance payloads all carry values only), so the move was a 30-LOC code change and a doc correction, not a structural refactor.

**Why `catalogue_snapshot_id`:** enables the runtime to detect "gated against catalogue v147, dispatched against catalogue v148" as its own failure class, distinct from TOCTOU on entity state. Mid-refactor catalogue reloads don't silently invalidate in-flight envelopes.

**Why `trace_id`:** single correlation key across all nine stages, outbox rows, drainer runs, and replay. One search term to follow an utterance from text to effect.

**Why `envelope_version`:** day-one versioning means the first additive contract change (v0.3 → v0.4) is a field addition with `#[serde(default)]`, not a migration. Starting at 1 now is free; introducing versioning later is not.

### 10.4 Dual-schema YAML for dissolved CRUD

Unchanged from v0.2. Each dissolved-CRUD YAML entry carries a **runtime schema** (table, columns, operation kind, conflict policy, returning shape) and a **catalogue schema** (DAG association, state gate expression, entity resolution shape, discovery signal). Both validated at startup; missing either fails loud.

### 10.5 `StateGateHash` concrete specification (new in v0.3)

```
StateGateHash = BLAKE3(canonical_encoding(
    envelope_version,                    // u16 LE
    entities_sorted_by_id[              // sorted by entity_id for determinism
        (entity_id: u128 LE, row_version: u64 LE)
    ],
    dag_node_id:          u128 LE,
    dag_node_version:     u64 LE,
    session_scope_id:     u128 LE,
    workspace_snapshot_id: u128 LE,
    catalogue_snapshot_id: u64 LE,
))
```

Canonical encoding is length-prefixed and fixed-order. Full specification lives in `ob-poc-types::state_gate_hash::encode`; test-vector set lives in the determinism harness.

**Row versioning.** Every entity table carries a monotonic `version: bigint` column (or uses Postgres `xmin` where acceptable). Row version increments on any row update visible to the gate surface. The Phase 0 ownership matrix audits this across the catalogue.

**Recheck site (Open Q3 carried from v0.2, now resolved).** The runtime re-checks `StateGateHash` inside the transaction, after acquiring row locks on the resolved entities, before issuing writes. This is closer to the transaction boundary than the Sequencer's pre-dispatch check would be and turns TOCTOU failure into a symmetric transactional abort. Failure shape: `DispatchError::ToctouMismatch { expected: StateGateHash, actual: StateGateHash }`; Sequencer rolls back the enclosing transaction.

### 10.6 The 625 ops — three destinations

Unchanged from v0.2. ~60% target for metadata dissolution (subject to round-trip); ~10–15% to `dsl-runtime` as domain-neutral plugins; ~25–30% stay in `ob-poc` behind runtime-defined service traits. Round-trip (§14) is the gate for dissolution.

### 10.7 Durability and the outbox (new in v0.3)

Solves the stage-8/stage-9 failure shape identified in peer review.

**In-transaction durability (stages 8 + 9a).** The Sequencer opens a transaction at the start of stage 8. Runtime writes execute inside it. `PendingStateAdvance` is applied by SemOS inside the same transaction. `OutboxDraft` rows are inserted into an `outbox` table inside the same transaction. Stage 9a commits. Either everything commits, or nothing commits. There is no window in which runtime writes land but SemOS state is stale, or vice versa.

**Post-commit durability (stage 9b).** A separate `outbox_drainer` task in `ob-poc` polls the outbox, claims rows, performs external effects (narration synthesis via SemOS API, UI push via WebSocket, broadcast to subscribers, cross-system notification), and marks rows done. Drainer is at-least-once:

- Each row: `status: pending | processing | done | failed_retryable | failed_terminal`.
- Claim: `UPDATE outbox SET status='processing', claimed_by=$worker, claimed_at=now() WHERE id=$id AND status='pending' RETURNING *` (atomic).
- Effect dispatch is idempotent via the `idempotency_key` — consumers dedupe.
- Retry policy with exponential backoff; terminal failure after N attempts with alerting.
- On worker crash: rows stuck in `processing` with stale `claimed_at` are recycled to `pending`.

**Outbox table schema (illustrative, finalised in Phase 0):**

```sql
CREATE TABLE outbox (
    id                uuid PRIMARY KEY,
    trace_id          uuid NOT NULL,
    envelope_version  smallint NOT NULL,
    effect_kind       text NOT NULL,
    payload           jsonb NOT NULL,
    idempotency_key   text NOT NULL,
    status            text NOT NULL DEFAULT 'pending',
    attempts          integer NOT NULL DEFAULT 0,
    claimed_by        text,
    claimed_at        timestamptz,
    created_at        timestamptz NOT NULL DEFAULT now(),
    last_error        text,
    UNIQUE (idempotency_key, effect_kind)
);
CREATE INDEX outbox_pending_idx ON outbox (status, created_at) WHERE status IN ('pending','failed_retryable');
```

**What goes in the outbox:** narration emission, UI push to the agent REPL, constellation rehydration broadcast to open sessions, cross-system notifications, anything with external effects.

**What does NOT go in the outbox:** SemOS state advances (those go in-txn via `PendingStateAdvance`), runtime writes (those are the transaction's primary content), catalogue reads, gate decisions.

**Assumption A1 (§8.5)** is what makes this split clean: if any SemOS state advance required external effects inside the inner transaction, the split breaks down. Phase 0 matrix validates.

---

## 11. Scope

### 11.1 In scope

- Create `dsl-runtime` crate.
- Relocate `VerbExecutionPort`, `CustomOperation`, `CustomOperationRegistry`, `VerbExecutionContext`, `VerbExecutionOutcome` (with `PendingStateAdvance`), `VerbRegistrar` to `dsl-runtime`.
- Define `GatedVerbEnvelope`, `PendingStateAdvance`, `TransactionScopeId`, `OutboxDraft`, `StateGateHash`, `OutboxRow` schema in `ob-poc-types`. (The `TransactionScope` trait itself lives in `dsl-runtime::tx` per the §10.3 correction.)
- Relocate `PgCrudExecutor` to `dsl-runtime`.
- Relocate domain-neutral plugin ops (~10–15% of `domain_ops`) to `dsl-runtime`.
- Define adapter traits in `dsl-runtime` for app-coupled ops; impls in `ob-poc`.
- Name and implement the Agentic Sequencer as a bounded module in `ob-poc`, including transaction scope ownership, outbox writing, and per-step TOCTOU recheck coordination.
- Implement `outbox_drainer` task in `ob-poc` with at-least-once semantics.
- Add `outbox` table via migration; add `row_version`/`xmin`-audit across entity tables used by the gate surface.
- Dual-schema YAML validation for dissolved-CRUD verbs.
- Round-trip harness as **effect-equivalence** (§14).
- Determinism harness (§9.4).
- Dissolve CRUD ops that pass round-trip; retain as plugins those that do not.
- Delete `execute_json_via_legacy()` shim.
- Workspace lints: `deny(unwanted_deps)`, `deny(multiple_dispatch_sites)`.
- Ownership matrix (§13 Phase 0) before any code move.

### 11.2 Out of scope

- Redesigning `CustomOperation` trait shape (locked — post-migration).
- Changing operation behaviour, SQL semantics, or schema beyond the outbox + row_version additions.
- Reorganising SemOS internal modules (only the two misplaced types move).
- Collapsing or renaming `sem_os_*` crates.
- Changing the intent-pipeline tier structure.
- Frontend changes beyond what the outbox drainer requires for UI push.
- Java 26 port (parked).

---

## 12. Non-goals

- No ten-plugin topology.
- No forcing of all app-coupled ops out of `ob-poc`.
- No move of `sem_os_postgres` schema ownership.
- No new execution trait.
- No change to the closed-loop invariant (ownership clarifies; shape preserved).
- No speculative crate fission.
- No NLP or embeddings in SemOS.
- No external side-effects in the inner transaction (A1).

---

## 13. Migration strategy

Phases ordered by structural dependency and blast radius. Each phase is independently reviewable. Rollback is safe through Phase 5; from Phase 6 rollback is restore-from-tag.

### Phase 0 — Matrix, envelope spec, harnesses, outbox schema (no production code)

Five deliverables, reviewed together:

**0a. Ownership matrix.** One row per `rust/src/domain_ops/*.rs` file:

| file | current callers | ob-poc internals used | behaviour type | destination | blocker traits | difficulty | round-trip candidate | `PendingStateAdvance` shape | A1 clean (no external effects) |

**0b. Concrete envelope + outcome types** in `ob-poc-types` (compile-only; not yet wired).

**0c. `StateGateHash` encoding spec** + test vectors.

**0d. Outbox schema migration** + drainer contract (compile-only; not wired to production paths yet).

**0e. Harnesses.**

- `round_trip_harness` — effect-equivalence comparison (§14).
- `determinism_harness` — byte-compares stage outputs 4–8 + `PendingStateAdvance` + outbox row set.

**Gate:** matrix reviewed and approved; types compile; harnesses run against representative fixtures (most ops may fail classification — that's the input to Phase 6).

### Phase 1 — Create `dsl-runtime` crate (skeleton)

- New crate `rust/crates/dsl-runtime/`.
- Two contents: `VerbExecutionPort` trait (moved from `sem_os_core`); compatibility re-export in `sem_os_core::execution` marked `#[deprecated]`.

**Gate:** workspace compiles clean; all tests pass; determinism harness byte-identical to Phase 0 baseline.

### Phase 2 — Move `CustomOperation` trait + registry + `VerbRegistrar` + expand `VerbExecutionOutcome`

- `CustomOperation`, `CustomOperationRegistry`, `VerbExecutionContext`, `VerbExecutionOutcome` (with `PendingStateAdvance` / `OutboxDraft`), `VerbRegistrar` move into `dsl-runtime`.
- `#[register_custom_op]` emits `VerbRegistrar` impls.
- Existing ops return empty `PendingStateAdvance` / `OutboxDraft` vectors initially — they are additive fields.

**Gate:** all 625 ops dispatch via `VerbExecutionPort`; determinism harness byte-identical with additive-field equality; `cargo test --workspace` green.

### Phase 3 — Move `PgCrudExecutor` to `dsl-runtime`

- `PgCrudExecutor` moves out of `sem_os_postgres`.
- `sem_os_postgres` retains only metadata-loading code.
- Registration manifest moves with it.
- Startup wiring in `ob-poc` adjusts import path.

**Gate:** `grep -r 'execute' sem_os_postgres/src/` returns only metadata-loading code; determinism harness byte-identical.

### Phase 4 — Move domain-neutral plugin ops to `dsl-runtime`

- Ops classified in Phase 0 as `destination: dsl-runtime` physically move.
- Registration via `VerbRegistrar`.
- `ob-poc` no longer defines these ops.

**Gate:** `dsl-runtime` + SemOS metadata is sufficient to dispatch moved ops against a `PgPool`; determinism harness byte-identical.

### Phase 5 — Adapter traits, Sequencer extraction, outbox wiring, transaction-scope model

The heaviest phase structurally. The original v0.3 draft bundled everything under a single Phase 5; practical execution split it into six sub-phases (5a–5f) as each sub-phase has its own risk surface. Sub-phase decomposition is authoritative as of 2026-04-20; each sub-phase has its own gate.

#### Phase 5a — service traits (ServiceRegistry + trait injection)

Define service-level traits in `dsl-runtime::service_traits` for ob-poc-retained capabilities. ob-poc wires concrete impls at startup via `ServiceRegistryBuilder`. Plugin ops that relocate to `dsl-runtime` consume services via `VerbExecutionContext::service::<dyn T>()`.

**Gate:** `ServiceRegistry` + `ServiceRegistryBuilder` compile with object-safe trait primitives; at least one trait pilot (recommended: `SemanticStateService`) proves the pattern end-to-end (ob-poc-side impl registered at startup; a relocated plugin op consumes it and passes all integration tests). Subsequent traits follow the same recipe.

**Status:** Pilot DONE — `SemanticStateService` shipped 55b0be16. Pattern established and in use by subsequent composite-blocker slices. Trait catalogue grown to 3 concrete services so far (`SemanticStateService`, `StewardshipDispatch`, `McpToolRegistry`). Additional traits (`TaxonomyAccess`, `AffinityGraph`, `ConstellationRuntime`, `AttributeIdentityService`, `ServiceResourcePipelineService`, `TemplateExpander+DslExecutor`, `TradingProfileDocument`, `TradingMatrixCorporateActions`) still to be defined as their consumer ops are worked.

**R-sweep relocation progress (tracked out-of-band in this file while 5a is in flight; authoritative until closed):**

- **R-batches (one per commit):** 9bc2fbb5 (8 files) · ec95f9e3 (4 files) · a7b68033 (11 files) · c90c66c0 (29 sem_os_* files) — ~52 files lifted into `dsl-runtime::domain_ops` via mechanical strip-and-move.
- **Composite-blockers (one trait/blocker per slice, smallest first):**
  1. `agent_ops` + `McpToolRegistry` trait — commit b441d132
  2. `cross_workspace/` module + `remediation_ops` (relocated together, cross_workspace is the consumer surface) — commit 27585e5d
  3. `shared_atom_ops` (clean lift — its blocker was cross_workspace, already resolved by #2) — commit 8f7ca9c3
  4. `research_workflow_ops` (self-contained; 4 ops, only json_* + sqlx) — commit 6480a0da

**Phase 5a progress: 54 / 89 op files relocated.** Remaining 35 split:
- **12 `ob-poc-adapter` destination** (legitimately stay in ob-poc): `billing_ops`, `booking_principal_ops`, `capital_ops`, `client_group_ops`, `deal_ops`, `gleif_ops`, `investor_ops`, `investor_role_ops`, `resource_ops`, `sem_os_maintenance_ops`, `sem_os_registry_ops`, `team_ops`.
- **Composite-blocker #5+ candidates** (`dsl-runtime`-destination, each needs ONE trait/dep resolved):
  - **No remaining blocker** (lift-and-strip candidates — verify first, then execute): `kyc_case_ops` (5 ops, touches `ontology`).
  - **Trait blocker** (needs new `service_traits` entry before move):
    - `ConstellationRuntime` → `constellation_ops` (handle_constellation_{hydrate,summary} from sem_os_runtime)
    - `TaxonomyAccess` → `view_ops` (already named in matrix §6)
    - `NavigationRuntime` / viewport state → `navigation_ops`
    - `SessionLifecycle` + `UnifiedSession` access → `session_ops`
    - `AttributeIdentityService` → `attribute_ops` + `observation_ops` (shared — compose as one slice)
    - `SemanticStageRegistry` → `onboarding` + `semantic_ops` (shared — compose)
    - `SkeletonBuildOrchestrator` → `skeleton_build_ops`
    - `DiscoveryExecutor` + insight builders → `discovery_ops`
    - `PhraseWatermarkScanner` + embedding similarity → `phrase_ops`
    - `ServiceResourcePipelineService` → `service_pipeline_ops`
    - `TemplateExpander` + `DslExecutor` → `template_ops`
    - `TradingProfileDocument` + `TradingMatrixCorporateActions` → `trading_profile` + `trading_profile_ca_ops` (shared — compose)
    - `MancoRoleBridge` SQL fn → `manco_ops`
    - `AffinityGraph` → `affinity_ops`
    - sem_reg audit_op! macros → `sem_os_audit_ops`
    - verb_contract affinity/diagram → `sem_os_schema_ops` (mixed metadata destination)
- **A1 Pattern B (deferred to Phase 5f ledger):** `bpmn_lite_ops` (5 ops, gRPC), `source_loader_ops` (16 ops, HTTP), `gleif_ops` (17 ops, HTTP — `ob-poc-adapter` dest regardless). See `docs/todo/pattern-b-a1-remediation-ledger.md`.

**Restart instructions for a fresh session:** Pick the next composite-blocker #5. The cheapest candidate is `kyc_case_ops` — verify its `ontology` import is the `crate::ontology` (ob-poc-owned) or something re-exportable cleanly. If the former, either expose a narrow trait for the ontology accessors it needs, or (more likely) lift the relevant ontology types to a shared crate first. Smaller lift-and-strip files gate on "does every import resolve via `dsl-runtime` or via a 1-method trait we can define in one sitting?"; if yes, move it; if no, define the trait + impl + registration in `ob-poc-web::main` first, then lift.

#### Phase 5b — Sequencer extraction

Relocate the orchestrator from `ob-poc::repl::orchestrator_v2` to `ob-poc::sequencer` per §8.1. This has two scopes:

- **5b-narrow:** path move only — `ReplOrchestratorV2` struct and tollgate state machine unchanged; module path aligns with §8.1. ~2-3 hours.
- **5b-deep:** typed per-stage functions with explicit I/O per §8.3; tollgate states re-modelled as persistent state between stage runs; per-stage test fixtures.

**Gate (5b-narrow):** `rust/src/sequencer.rs` exists and hosts the orchestrator; consumers reach it via `ob_poc::sequencer::`; Phase 0 harnesses + full test suite unchanged. §8.6 tollgate↔stage mapping pinned in the spec.

**Status:** 5b-narrow DONE — commit aec638de. 5b-deep deferred; not blocking.

#### Phase 5c — transaction-scope model

Hoist txn begin/commit out of `DslExecutor` into the Sequencer at stage 8; plugin ops consume a `&mut dyn TransactionScope` instead of `&PgPool`. Split into two sub-sub-slices:

- **5c-prep:** `TransactionScope` trait extended with `fn transaction(&mut self) -> &mut sqlx::Transaction<'static, Postgres>`; concrete `PgTransactionScope` added in `ob_poc::sequencer_tx` wrapping `sqlx::Transaction`. Plugin op signatures NOT changed — the primitives simply exist. ~1-2 hours.
- **5c-migrate:** mass-rewrite `CustomOperation::execute_json(pool: &PgPool)` → `execute_json(scope: &mut dyn TransactionScope)` across ~90 op files; adopt `scope.transaction()` inside each op body; hoist txn begin/commit from `DslExecutor::execute_plan_atomic*` into the Sequencer's stage-8 path; wire the stage-9a atomic-commit invariant (rollback if apply-PendingStateAdvance fails). 1-2 days.

**Gate (5c-prep):** trait extension + concrete impl compile; both dyn-compatible; unit + integration tests pass.

**Gate (5c-migrate):** `VerbExecutionPort::execute_json` has exactly one caller (Sequencer stage 8); transaction-abort-on-stage-9a-fail test passes (no committed writes).

**Status:** 5c-prep DONE this commit. 5c-migrate OPEN — depends on `PendingStateAdvance` production pipeline being real (no current plugin op produces non-empty values).

#### Phase 5d — TOCTOU recheck inside txn

Move `StateGateHash` re-computation inside the stage-8 transaction after acquiring row locks, per §10.5. Requires 5c-migrate (to have a real Sequencer-scoped txn) + row-versioning migrations per §0f.

**Gate:** every dispatched envelope re-checks `StateGateHash` inside the txn; mismatch aborts the runbook with `DispatchError::ToctouMismatch`.

**Status:** NOT STARTED. Blocked on 5c-migrate + row-versioning (§0f doc exists; migrations drafted but not executed).

#### Phase 5e — outbox wiring + drainer

`OutboxDraft` values from verb outcomes land in `public.outbox` rows written inside the stage-8 transaction (migration 131 already drafted at Phase 0d). Drainer task consumes outbox rows post-commit and routes stage-9b effects (narration synthesis, UI push, broadcast).

**Gate:** drainer replay-safe test passes (kill mid-stream, restart, all effects delivered exactly-semantically-once via idempotency key); narration no longer fires inline in `process()`.

**Status:** NOT STARTED. Scaffold migration + trait contracts exist in `ob-poc-types::OutboxDrainer`; drainer task absent.

#### Phase 5f — Pattern B A1 remediation

Refactor the 38 ops flagged as external-I/O in `pattern-b-a1-remediation-ledger.md` (bpmn_lite_ops, source_loader_ops, gleif_ops) into either (a) two-phase fetch-then-persist or (b) outbox deferral.

**Gate:** no plugin op performs external HTTP / gRPC / subprocess spawn inside `execute_json` body; workspace lint `L4 forbid-external-effects-in-verb` green; ledger §2 CLOSED.

**Status:** NOT STARTED. Pattern A (1 file) closed at Phase 0g.

#### Phase 5 integration gate

`dsl-runtime` has no transitive dependency on `ob-poc` for runtime ops; ob-poc can swap service impls without touching runtime; determinism harness byte-identical across all 5a-5f changes; single dispatch-site lint enforced post-5c-migrate.

### Phase 6 — Metadata-driven CRUD dissolution

Pre-phase: tag release `v-pre-dissolution`. Rollback becomes restore-from-tag here.

- For each `behaviour: crud` op that passes round-trip (§14): verify dual-schema YAML, delete Rust impl, manifest auto-populates from metadata.
- For each op that fails round-trip: reclassify as plugin, leave Rust impl, flag in matrix.
- Expected: 50–60% dissolve, 5–10% reclassify, remainder already plugin.

**Gate:** round-trip harness passes for every dissolved op; catalogue-completeness validator passes at startup; determinism harness byte-identical; intent hit rate within ±1% of pre-refactor baseline.

### Phase 7 — Remove shim, finalise, enforce lints

- `execute_json_via_legacy()` deleted.
- Legacy `execute(...)` deleted from `CustomOperation` if redundant.
- Deprecation re-export of `VerbExecutionPort` in `sem_os_core::execution` removed.
- Workspace lints `deny(unwanted_deps)` and `deny(multiple_dispatch_sites)` enforced.
- Update CLAUDE.md crate list, trigger-phrase table, annex references, architectural-principle statement.
- `sem_os_lift_out_plan.md` archived as superseded.

**Gate:** all acceptance criteria in §17 met; all lints green.

---

## 14. The round-trip test — effect-equivalence (redefined in v0.3)

A CRUD op is a candidate for Phase 6 dissolution only if it passes round-trip. **Round-trip is effect-equivalence, not SQL-equivalence.**

For each of N ≥ 50 `(args, pre-state fixture)` inputs per op:

1. Run the current Rust op impl against a fresh fixture DB.
2. Run `PgCrudExecutor` interpreting the proposed YAML against an equivalent fresh fixture DB.
3. Capture and compare:
   - **Post-state row diff** — byte-identical content across all tables the op could touch.
   - **Returned values** — byte-identical structural equality.
   - **Side-effect summary** — sequence advances, trigger firings, audit row content, all byte-identical.
   - **`PendingStateAdvance`** (once wired in Phase 5) — byte-identical.
   - **`OutboxDraft` vector** — set-equal on `idempotency_key`.

Pass = 100% identity across all fixtures on all comparison axes. SQL text is NOT compared — equivalent SQL produced with different ordering, formatting, parameter numbering, or aliasing passes as long as the *effects* are identical.

Failure modes commonly found:

- Default-value handling not surviving declarative model.
- Derived fields computed in Rust before the SQL.
- Coalescing / null handling.
- Audit columns beyond the obvious.
- Soft-delete conventions encoded in Rust flow control.

Reclassification budget: **25–30%** of the nominal CRUD bucket. Below 15% or above 40% means the matrix is wrong.

---

## 15. Risks and mitigations

### R1 — Naming confusion between `dsl-core` and `dsl-runtime`

*Mitigation:* top-of-crate comments; `Cargo.toml` DB-crate deny-list on `dsl-core`; CLAUDE.md trigger phrases.

### R2 — `VerbExecutionPort` trait grows to serve both planes

*Mitigation:* trait shape locked at Phase 2. Additions require written justification.

### R3 — Adapter traits drag app types into `dsl-runtime`

*Mitigation:* traits expressed in primitives + `ob-poc-types`. No app types in signatures.

### R4 — Metadata-driven CRUD loses fidelity

*Mitigation:* effect-equivalence round-trip (§14) is the gate. Budget 25–30%.

### R5 — Downstream tooling (LSP, `sem_os_server`) breaks

*Mitigation:* LSP reads catalogue from `sem_os_*`, not op code. `sem_os_server` serves governance, not execution.

### R6 — Closed-loop invariant silently breaks

*Mitigation:* determinism harness asserts closed-loop shape per-fixture. Dedicated test in `dsl-runtime` asserts `VerbExecutionOutcome` + `PendingStateAdvance` carries sufficient information.

### R7 — Sequencer becomes a god-module

*Mitigation:* typed stage inputs/outputs; stages independently testable; lint caps module size / fan-out after Phase 5.

### R8 — Performance regression from trait-object dispatch

*Mitigation:* vtable + `Arc::deref` overhead ~10–30 ns per call; invisible at session scale. Recorded here; generics-over-traits available if ever needed.

### R9 — Harness drift

*Mitigation:* harness fixtures versioned separately; fixture additions during refactor require sign-off; harnesses run on every PR.

### R10 — Outbox drainer bugs silently drop external effects (new in v0.3)

A non-idempotent consumer, a bug in the drainer state machine, or a race between claim and worker-crash detection could cause narration or UI push loss.

*Mitigation:* at-least-once drainer semantics; `idempotency_key` UNIQUE constraint; per-effect-kind consumer tests for idempotency; drainer-kill replay test in Phase 5 gate; monitoring on `outbox` backlog and `failed_terminal` counts; alert on either.

### R11 — Envelope version drift during refactor (new in v0.3)

Mid-refactor, a SemOS binary emits `envelope_version = 1` and a runtime binary compiled later expects `envelope_version = 2`, or vice versa.

*Mitigation:* start at version 1 day one; all changes through v0.3 refactor are additive under `#[serde(default)]`; any breaking change bumps the major and requires coordinated deploy; runtime checks `envelope_version` on receipt and fails loudly on unknown.

### R12 — A1 is wrong (new in v0.3)

Some SemOS state advance does require external effects inside the inner transaction.

*Mitigation:* Phase 0 matrix column explicitly tests A1 for every `PendingStateAdvance` shape. Any violation forces redesign of 9a before Phase 5 can proceed.

### R13 — Row-version backfill lag (new in v0.3)

Some entity tables may lack a `row_version` column (or reliable `xmin` semantics) needed for `StateGateHash`.

*Mitigation:* Phase 0 audit per entity table; tables without versioning get a migration before Phase 5; backfill under live traffic with zero-downtime policy.

---

## 16. Decision gates (must be agreed before Phase 0 begins)

1. ✅ / ❌ — three-plane framing is the right frame.
2. ✅ / ❌ — capability disclosure is the control plane's primary mode.
3. ✅ / ❌ — determinism is a first-class invariant, enforced by harness.
4. ✅ / ❌ — Agentic Sequencer is a named, bounded module implementing the nine-stage contract.
5. ✅ / ❌ — `VerbExecutionPort` and `PgCrudExecutor` move out of `sem_os_*`.
6. ✅ / ❌ — `dsl-runtime` is a new crate (not an expansion of `dsl-core`).
7. ✅ / ❌ — `GatedVerbEnvelope`, `PendingStateAdvance`, `TransactionScopeId`, `OutboxDraft` live in `ob-poc-types`. The `TransactionScope` *trait* lives in `dsl-runtime::tx` (§10.3 2026-04-20 correction).
8. ✅ / ❌ — three-destination model replaces the ten-plugin topology.
9. ✅ / ❌ — round-trip is effect-equivalence (§14), not SQL-equivalence.
10. ✅ / ❌ — SemOS does not call `VerbExecutionPort` directly; Sequencer stage 8 is the only dispatch site.
11. ✅ / ❌ — single canonical registration via `VerbRegistrar`.
12. ✅ / ❌ — Phase 0 produces matrix + envelope spec + `StateGateHash` spec + outbox schema + harnesses before any code moves.
13. ✅ / ❌ — rollback safe through Phase 5; restore-from-tag from Phase 6. **(v0.3 new)**
14. ✅ / ❌ — Sequencer owns transaction scope; runtime owns mechanics inside the scope.
15. ✅ / ❌ — durability model: stage 9a in-txn with runtime writes; stage 9b via outbox.
16. ✅ / ❌ — envelope carries `envelope_version`, `catalogue_snapshot_id`, `trace_id` from day one.
17. ✅ / ❌ — `StateGateHash` is BLAKE3 over the concrete input set in §10.5; runtime re-checks inside the transaction after acquiring row locks.
18. ✅ / ❌ — Assumption A1 holds (no SemOS state advance requires external effects in the inner txn); Phase 0 matrix validates.
19. ✅ / ❌ — entity resolution splits 2a/2b; NLP never crosses into the control plane.

---

## 17. Definition of done

The refactor is complete when:

1. No `sem_os_*` crate contains any `execute_*` function outside metadata loading.
2. `dsl-runtime` exists and owns `VerbExecutionPort`, `CustomOperation`, `CustomOperationRegistry`, `VerbRegistrar`, `PgCrudExecutor`, `VerbExecutionContext`, `VerbExecutionOutcome` (with `PendingStateAdvance`).
3. `GatedVerbEnvelope`, `PendingStateAdvance`, `TransactionScopeId`, `OutboxDraft`, `StateGateHash`, `OutboxRow` schema exist in `ob-poc-types`; the `TransactionScope` trait lives in `dsl-runtime::tx` (§10.3 correction); envelope carries `envelope_version`, `catalogue_snapshot_id`, `trace_id`.
4. `ob-poc/domain_ops` contains only app-coupled op implementations, each behind a `dsl-runtime`-defined service trait.
5. `ob-poc::sequencer` is the named module implementing the nine-stage contract. `VerbExecutionPort::execute_json` has exactly one caller.
6. Transaction model: Sequencer opens at stage 8, runtime executes mechanics inside, SemOS applies `PendingStateAdvance` in same txn, Sequencer commits at 9a.
7. Outbox table in place; `outbox_drainer` running; stage 9b effects (narration, UI push, broadcast) delivered via drainer with at-least-once semantics.
8. `StateGateHash` recheck inside the transaction after row locking; TOCTOU mismatch aborts transaction.
9. `execute_json_via_legacy()` deleted.
10. Every runtime-executable verb is catalogued; startup validation enforces it.
11. Dissolved-CRUD verbs carry dual-schema YAML; every dissolved op passed effect-equivalence round-trip.
12. Determinism harness green across the full fixture set.
13. `cargo test --workspace` green; intent hit rate within ±1% of baseline.
14. Workspace lints enforce one-way dependencies and single dispatch site.
15. Transaction-abort-on-stage-9a-fail test green. Drainer-kill-replay test green. TOCTOU recheck test green.
16. CLAUDE.md updated.
17. `sem_os_lift_out_plan.md` archived as superseded.
18. No `sem_os_*` symbol reachable from any `dsl-runtime` dependency graph; no `dsl-runtime` symbol reachable from any `sem_os_*` dependency graph.

---

## 18. Relationship to prior planning artefacts

| Document | Relationship |
|---|---|
| `three-plane-architecture-v0_2.md` | **Superseded.** v0.3 changes: runbook/envelope ambiguity resolved (§8.2); transaction scope-vs-mechanics split (§7.1, §8.4); durability model pinned (§10.7); round-trip redefined as effect-equivalence (§14); envelope carries `envelope_version` / `catalogue_snapshot_id` / `trace_id` (§10.3); `StateGateHash` concrete spec (§10.5); dependency graphs split (§7.3); entity resolution 2a/2b split (§8.2); Assumption A1 flagged (§8.5); R10–R13 added (§15); decision gates 13–19 added (§16). |
| `three-plane-architecture-v0_1.md` | Superseded via v0.2. |
| `docs/todo/sem_os_lift_out_plan.md` | Destination superseded; extraction discipline preserved. |
| `memory/project_semos_domain_ops_subsumption.md` | Refined — CRUD metadata in SemOS; CRUD execution in `dsl-runtime`. |
| `memory/project_semos_execution_port_progress.md` | Reframed — Phases 2+ redirect from "SemOS owns execution" to "dsl-runtime owns execution; SemOS owns catalogue." |
| `memory/project_semos_hub_invariant.md` | Preserved — SemOS is the hub for catalogue, state, association, discovery. |
| CLAUDE.md "SemOS is the hub for all things" | Clarified — SemOS is the hub for capability disclosure, governance, catalogue; execution is `dsl-runtime`; sequencing and durability are the Sequencer's. |

### v0.2 → v0.3 change summary

| Area | v0.2 state | v0.3 change |
|---|---|---|
| Stage model | 9 stages, stage 7 "runbook construction", stage 8 "dispatch" — boundary unclear | Stage 7 "Runbook compilation" produces pre-gated envelopes; stage 8 "Dispatch loop" iterates envelopes with per-step TOCTOU recheck; 2a/2b NLP split; 9a/9b durability split |
| Transaction ownership | Ambiguous between Runtime (§7.1) and Sequencer (§8.4) | Scope-vs-mechanics split: Sequencer owns scope, Runtime owns mechanics inside the scope |
| Durability | Stage-9 failures with stale control-plane flagged but not solved | `PendingStateAdvance` returned from outcome, applied by SemOS in same txn (9a); `OutboxDraft` for post-commit effects (9b) with idempotent drainer |
| Round-trip test | SQL byte-identity + row identity | Effect-equivalence: row diff + returned values + side-effect summary + `PendingStateAdvance` + `OutboxDraft` set — SQL text not compared |
| Envelope | 7 fields, no versioning | + `envelope_version`, `catalogue_snapshot_id`, `trace_id` |
| `StateGateHash` | Abstract placeholder | Concrete BLAKE3 spec over canonical length-prefixed encoding of sorted (entity_id, row_version), DAG node/version, session scope, workspace snapshot, catalogue snapshot; recheck inside txn after row lock |
| Dependency graph | Single diagram mixing Cargo deps and data flow | Two diagrams: Cargo graph (imports) + runtime data flow (logical) |
| Entity resolution | "SemOS: utterance → entity ids" | 2a Orchestrator (text → structured triples); 2b SemOS (structured → canonical ids) |
| Risks | R1–R9 | + R10 outbox drainer, R11 envelope versioning, R12 A1 violation, R13 row-version backfill |
| Decision gates | 13 items | 19 items |

---

## 19. Remaining open questions

Down from three in v0.2.

1. **Outbox effect catalogue.** `OutboxEffectKind` enum values and per-kind payload schemas. Delegated to Phase 0 deliverable 0d but agreed before Phase 5.
2. **Row-versioning strategy per table.** Explicit `version bigint` vs Postgres `xmin` vs hybrid. Delegated to Phase 0 audit.
3. **Drainer deployment model.** Single drainer task vs sharded vs one-per-effect-kind. Delegated to Phase 5 design.

Carried resolutions (from v0.2):
- Crate name: `dsl-runtime`.
- `PgCrudExecutor` lives in `dsl-runtime` (no `dsl-runtime-postgres` sub-crate).
- `VerbCall` envelope location: `ob-poc-types`.
- Macro location: with the runtime.
- Trait granularity: one per service, coarse-grained.
- Phase ordering: CRUD dissolution last.
- TOCTOU recheck site: runtime, inside transaction, after row lock.

---

## 20. Next artefact

The Phase 0 deliverables (matrix + concrete types + `StateGateHash` spec + outbox schema + harnesses), produced by a downstream Claude/Zed session reviewing this document against the live codebase. The implementation-level TODO produced by that session becomes the input to peer review cycle 3.

---

## Appendix A — Full envelope and outcome types (illustrative)

```rust
// ob-poc-types::gated_envelope

// --- envelope ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatedVerbEnvelope {
    #[serde(default = "default_envelope_version")]
    pub envelope_version: EnvelopeVersion,
    pub catalogue_snapshot_id: CatalogueSnapshotId,
    pub trace_id: TraceId,

    pub verb: VerbRef,
    pub dag_position: DagNodeId,
    pub resolved_entities: ResolvedEntities,
    pub args: VerbArgs,

    pub authorisation: AuthorisationProof,
    pub discovery_signals: DiscoverySignals,
    pub closed_loop_marker: ClosedLoopMarker,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnvelopeVersion(pub u16);

fn default_envelope_version() -> EnvelopeVersion { EnvelopeVersion(1) }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct CatalogueSnapshotId(pub u64);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TraceId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorisationProof {
    pub issued_at: LogicalClock,
    pub session_scope: SessionScopeRef,
    pub state_gate_hash: StateGateHash,
    pub recheck_required: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct StateGateHash(pub [u8; 32]);  // BLAKE3-256 output

// --- outcome ---

#[derive(Debug, Clone)]
pub struct VerbExecutionOutcome {
    pub trace_id: TraceId,
    pub result: OutcomeResult,
    pub pending_state_advance: PendingStateAdvance,
    pub side_effect_summary: SideEffectSummary,
    pub outbox_drafts: Vec<OutboxDraft>,
}

#[derive(Debug, Clone)]
pub struct PendingStateAdvance {
    pub state_transitions: Vec<StateTransition>,
    pub constellation_marks: Vec<ConstellationMark>,
    pub writes_since_push_delta: u64,
    pub catalogue_effects: Vec<CatalogueEffect>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxDraft {
    pub effect_kind: OutboxEffectKind,
    pub payload: serde_json::Value,
    pub idempotency_key: IdempotencyKey,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum OutboxEffectKind {
    Narrate,
    UiPush,
    ConstellationBroadcast,
    ExternalNotify,
    // extensible; consumers dedupe via idempotency_key
}

// --- transaction scope ---
// `ob-poc-types` carries only the correlation id. The `TransactionScope`
// trait (with executor access) lives in `dsl-runtime::tx`; see §10.3
// note dated 2026-04-20.

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TransactionScopeId(pub Uuid);      // in ob-poc-types

// in dsl-runtime::tx (NOT ob-poc-types):
//
//   pub trait TransactionScope: Send + Sync {
//       fn scope_id(&self) -> TransactionScopeId;
//       fn executor(&mut self) -> &mut dyn sqlx::PgExecutor;   // Phase 5c
//   }
```

Full definitions land in Phase 0 (`TransactionScopeId`) and Phase 5c (`TransactionScope` trait executor method).

---

## Appendix B — Workspace lint specifications (illustrative)

**L1: one-way dependencies.**

```toml
# workspace-level deny rules (pseudocode; exact mechanism TBD in Phase 7)
sem_os_core     = { deny-deps = ["dsl-runtime", "ob-poc", "ob-poc-web"] }
sem_os_postgres = { deny-deps = ["dsl-runtime", "ob-poc", "ob-poc-web"] }
dsl-runtime     = { deny-deps = ["sem_os_core", "sem_os_postgres", "sem_os_server", "ob-poc", "ob-poc-web"] }
dsl-core        = { deny-deps = ["sqlx", "tokio-postgres", "diesel"] }
ob-poc-types    = { allow-deps = ["std", "uuid", "chrono", "serde", "serde_json"] }
```

**L2: single dispatch site.**

A clippy-or-equivalent lint counting call sites of `VerbExecutionPort::execute_json` across the workspace. Fails if > 1 non-test site. Expected site: `ob-poc::sequencer::stage_8::dispatch_envelope`. Test mocks exempt via attribute.

**L3: no NLP in control plane (new in v0.3).**

Lint denies imports of `candle_*`, embedding crates, and tokenizer crates from any `sem_os_*` crate. Enforced in Phase 7.

---

**End of v0.3.**
