# Packs / Macros / Runbook — Implementation TODO

**Companion to:** `PACKS_MACROS_RUNBOOK_ARCH_PAPER_v0.5.md` (the spec)  
**Consumer:** Claude Code  
**Date:** 2026‑02‑15

---

## How to read this document

This is an **ordered implementation plan** in six phases (P0–P5). Each phase references specific sections of the architecture paper (the spec) using `→ SPEC §X.Y` notation. The spec is normative — if this TODO and the spec disagree, the spec wins.

**Phases are dependency‑ordered.** Do not start a phase until its predecessor's acceptance criteria are met. Within a phase, steps are ordered top‑to‑bottom.

**Continuation rule:** After completing each phase, emit a progress summary (`Phase N DONE — N/M tests passing`) and → IMMEDIATELY proceed to the next phase. Do not stop between phases.

---

## Phase 0 — Core types and execution gate (foundation)

**Goal:** Establish the runbook‑only execution gate (→ SPEC §11.1, INV‑3) and the type system that enforces it. Nothing else executes until this gate exists.

**Why first:** Every subsequent phase depends on compiled runbooks being the only execution path. If shadow execution paths survive, macros, packs, and locking all have bypass vectors.

### P0.1 — Define core runbook types

Create a module (suggest `rust/src/runbook/types.rs` or appropriate location in current crate structure):

```rust
// → SPEC §11.4, §11.4.1
pub struct CompiledRunbookId(Uuid);  // UUIDv7

pub struct CompiledRunbook {
    pub id: CompiledRunbookId,
    pub session_id: SessionId,
    pub version: u64,               // monotonic per session → SPEC §11.4
    pub steps: Vec<RunbookStep>,     // expanded DSL, post-macro
    pub envelope: ReplayEnvelope,    // → SPEC §9.1
    pub status: RunbookStatus,
    pub created_at: OffsetDateTime,
}

// → SPEC §11.4.1 — no Draft status; compilation succeeds or fails
pub enum RunbookStatus {
    Compiled,
    Executing,
    Parked { reason: ParkReason, cursor: StepCursor },
    Completed,
    Failed { error: RunbookError },
}

pub enum ParkReason {
    AwaitingCallback { callback_id: String },
    UserPaused,
    ResourceUnavailable { detail: String },
}
```

**xref:** Status definitions are normative in → SPEC §11.4.1. No `Draft` — compilation either succeeds or returns `CompilationError`.

### P0.2 — Define OrchestratorResponse

```rust
// → SPEC §11.3
pub enum OrchestratorResponse {
    Clarification(ClarificationRequest),
    ConstraintViolation(ConstraintViolationDetail),
    Compiled(CompiledRunbookSummary),
}

pub struct ClarificationRequest {
    pub question: String,
    pub missing_fields: Vec<String>,
    pub context: serde_json::Value,
}

// → SPEC §11.3, §7.3
pub struct ConstraintViolationDetail {
    pub explanation: String,
    pub expanded_plan: Option<Vec<String>>,     // verb names from expansion
    pub active_constraints: Vec<PackConstraint>, // what blocked it
    pub remediation_options: Vec<Remediation>,
}

pub enum Remediation {
    WidenScope { pack_id: PackId, suggested_verbs: Vec<String> },
    SuspendPack { pack_id: PackId },
    AlternativeVerbs { alternatives: Vec<String> },
}

pub struct CompiledRunbookSummary {
    pub compiled_runbook_id: CompiledRunbookId,
    pub runbook_version: u64,
    pub envelope_refs: EnvelopeRefs,
    pub preview: Vec<String>,  // human-readable step summaries
}
```

### P0.3 — Define replay envelope type

```rust
// → SPEC §9.1
pub struct ReplayEnvelope {
    pub session_cursor: u64,                          // event log position at compile time
    pub entity_bindings: HashMap<String, EntityId>,    // resolved entity refs
    pub external_lookups: HashMap<String, serde_json::Value>,  // pinned external results
    pub macro_audit: Option<MacroExpansionAudit>,      // if macro was involved
}

// → SPEC §9.1
pub struct MacroExpansionAudit {
    pub macro_name: String,
    pub params: serde_json::Value,
    pub resolved_autofill: HashMap<String, serde_json::Value>,
    pub expansion_digest: String,   // hash of expanded DSL for integrity
}
```

### P0.4 — Define the two public surfaces

```rust
// → SPEC §11.2
// Compile surface
pub async fn process_utterance(
    session_id: &SessionId,
    utterance: &str,
) -> Result<OrchestratorResponse, PipelineError>;

// Execution surface — the gate (→ SPEC §11.1, INV-3)
pub async fn execute_runbook(
    session_id: &SessionId,
    compiled_runbook_id: &CompiledRunbookId,
    cursor: Option<StepCursor>,
) -> Result<ExecutionResult, ExecutionError>;
```

`execute_runbook` **must not** accept:
- raw verb names
- macro invocations
- in-memory DSL strings
- anything other than a persisted `CompiledRunbookId`

### P0.5 — Audit and remove shadow execution paths

Search the codebase for any function that executes verbs, macros, or DSL outside of `execute_runbook`:

```bash
# indicative grep patterns — adapt to actual codebase
grep -rn "execute_verb\|run_dsl\|exec_statement\|dispatch_verb" rust/src/
grep -rn "try_expand_macro.*execute\|macro.*run\|expand.*exec" rust/src/
```

For each hit:
- If it's a test: refactor to compile → persist → call `execute_runbook` (defer to P4 if extensive).
- If it's production code: remove or route through `execute_runbook`.
- If removal would break current functionality: document as tech debt with `// TODO(P4): route through execute_runbook gate` and continue.

### P0.6 — Stub the execution gate with locking placeholder

Implement `execute_runbook` with:
- runbook lookup by `CompiledRunbookId`
- status validation (must be `Compiled` or `Parked`)
- status transition: `Compiled → Executing → Completed|Failed|Parked`
- **placeholder** for entity UUID locking (filled in P0.7)
- step iteration with per-step result recording as session events

### P0.7 — Entity UUID locking (pre‑lock set)

→ SPEC §10.1, §10.2

Inside `execute_runbook`, before step iteration:

```rust
// Compute write-set: all entity UUIDs that any step in the execution range mutates
let write_set: BTreeSet<EntityUuid> = compute_write_set(&runbook.steps, cursor_range);

// Acquire locks in canonical order (BTreeSet is sorted)
let lock_guard = entity_lock_manager.acquire_set(&write_set).await?;

// Execute steps under lock
let result = execute_steps_under_lock(&runbook, cursor_range, &lock_guard).await;

// Release on completion/park/failure
drop(lock_guard);
```

Key requirements (→ SPEC §10.1):
- Lock acquisition happens ONLY inside `execute_runbook`. Verify no other call site acquires entity locks.
- Use `BTreeSet<EntityUuid>` for deterministic ordering (avoids deadlocks across concurrent sessions).
- Lock granularity: per entity UUID, not per table or per session.

### P0 Acceptance

- [ ] `CompiledRunbookId` is the only accepted execution handle.
- [ ] `execute_runbook` rejects anything that isn't a persisted compiled runbook.
- [ ] Entity UUID locks are acquired as pre-lock set inside `execute_runbook` only.
- [ ] Status transitions are recorded as session events.
- [ ] No shadow execution paths remain in production code (tests may have `TODO(P4)` markers).

**→ IMMEDIATELY proceed to Phase 1. Progress: ~15% complete.**

---

## Phase 1 — Macro compiler semantics

**Goal:** Macros are discoverable, expandable, and produce compiled runbooks identical to hand‑written DSL. → SPEC §8, §4.2

**Depends on:** P0 (execution gate exists; expanded DSL can be compiled and executed through it).

### P1.1 — Unify macro registry

→ SPEC §8.3

Locate the two registries:
- `rust/src/dsl_v2/macros/` — `MacroRegistry` (canonical, keep)
- `rust/src/macros/` — `OperatorMacroRegistry` (deprecate)

Action:
1. Make `MacroRegistry` the single source of truth for macro schemas.
2. If `OperatorMacroRegistry` is used in verb discovery (`verb_search.rs`, `verb_discovery_routes.rs`), replace those call sites with `MacroRegistry`.
3. If `OperatorMacroRegistry` has unique functionality, migrate it into `MacroRegistry`.
4. Delete `OperatorMacroRegistry` or reduce to a thin re‑export.

### P1.2 — VerbClassifier: macros as first‑class candidates

→ SPEC §7.1

In the intent pipeline (or wherever verb selection occurs post-REPL-cleanup):

```rust
pub enum VerbClassification {
    Primitive(RuntimeVerbId),
    Macro(MacroSchemaRef),
    Unknown(String),
}

pub fn classify_verb(verb_name: &str, registry: &UnifiedRegistry) -> VerbClassification {
    if let Some(rv) = registry.get_runtime_verb(verb_name) {
        VerbClassification::Primitive(rv.id)
    } else if let Some(ms) = registry.get_macro_schema(verb_name) {
        VerbClassification::Macro(ms)
    } else {
        VerbClassification::Unknown(verb_name.to_string())
    }
}
```

The pipeline must NOT call `get_runtime_verb(...)` and error on miss. It must classify first, then branch.

### P1.3 — Argument extraction for macros

When `VerbClassification::Macro` is selected:
- Use `MacroSchema.parameters` for argument extraction (required/optional, types, enum keys).
- Support `autofill-from` by pre‑seeding values from session state where available (→ SPEC §9.2 — resolved values are baked into expanded DSL).
- Return `ClarificationRequest` (→ SPEC §11.3) if required params are missing.

### P1.4 — Macro expansion as compiler pass

→ SPEC §8.1

After argument extraction, before runbook acceptance:

```rust
pub fn expand_macro(
    schema: &MacroSchema,
    params: &MacroParams,
    session_snapshot: &SessionSnapshot,
) -> Result<MacroExpansionResult, MacroExpansionError> {
    // Pure function: same inputs → same outputs (→ SPEC §4.2)
    // Returns expanded DSL steps + audit metadata
}

pub struct MacroExpansionResult {
    pub steps: Vec<DslStatement>,
    pub audit: MacroExpansionAudit,  // → SPEC §9.1 / P0.3
}
```

Requirements:
- Expansion is **pure** — no side effects, no DB writes, no lock acquisition.
- Autofill values resolved from session snapshot are recorded in `MacroExpansionAudit`.
- Expanded steps have explicit subject entity bindings (→ SPEC §10.3 — lock sets must be computable).

### P1.5 — Wire expansion into compilation pipeline

The full flow for a macro‑selected utterance:

```
utterance
  → verb discovery (may return macro candidate)
  → VerbClassifier → Macro
  → extract args from MacroSchema
  → expand_macro(schema, params, session_snapshot)
  → PackConstraintGate(expanded_steps, active_constraints)  // → P2, stub as pass-through for now
  → validate / lint / semreg
  → DAG / toposort
  → persist as CompiledRunbook with ReplayEnvelope
  → return OrchestratorResponse::Compiled
```

For P1, stub the PackConstraintGate as always-pass. It gets real enforcement in P2.

### P1 Acceptance

- [ ] One macro registry (`MacroRegistry`) — `OperatorMacroRegistry` gone or thin wrapper.
- [ ] `VerbClassifier` distinguishes primitive / macro / unknown.
- [ ] Utterance matching `structure.setup` produces a `CompiledRunbook` containing expanded atomic DSL steps.
- [ ] The compiled runbook executes via `execute_runbook` identically to a hand-written DSL runbook.
- [ ] `MacroExpansionAudit` is persisted in the `ReplayEnvelope`.
- [ ] Macro expansion is a pure function (no side effects).

**→ IMMEDIATELY proceed to Phase 2. Progress: ~40% complete.**

---

## Phase 2 — Pack Manager lifecycle and constraint enforcement

**Goal:** Packs constrain and guide (control plane only). Pack constraints are enforced post-expansion via the PackConstraintGate. → SPEC §6, §7.3, §5.1

**Depends on:** P1 (macros expand into candidate DSL that the gate can inspect).

### P2.1 — Pack state machine

→ SPEC §6.1

```rust
pub enum PackState {
    Dormant,
    Active { progress: PackProgress },
    Suspended { preserved_progress: PackProgress },
    Completed { completed_at: OffsetDateTime },
}

pub struct PackDefinition {
    pub id: PackId,
    pub allowed_verbs: HashSet<String>,
    pub allowed_entity_kinds: HashSet<String>,
    pub gates: Vec<PackGate>,           // conditions for progress/completion
    pub prompts: Vec<PackPrompt>,       // clarification guidance
}

pub struct PackProgress {
    pub gates_satisfied: HashSet<GateId>,
    pub gates_remaining: HashSet<GateId>,
}
```

State transitions:
- `Dormant → Active`: explicit activation (by orchestrator or another pack's recommendation).
- `Active → Suspended`: explicit suspension (higher-priority pack, user command).
- `Active → Completed`: all gates satisfied (→ SPEC §6.2, event-driven).
- `Suspended → Active`: reactivation.
- `Completed` is terminal.

### P2.2 — Pack Manager as event projection

→ SPEC §6.2

```rust
pub struct PackManager {
    packs: HashMap<PackId, (PackDefinition, PackState)>,
}

impl PackManager {
    /// Called when session events are emitted (step executed, entity created, etc.)
    /// Advances pack gates. Never called by verb discovery or arg extraction.
    pub fn process_event(&mut self, event: &SessionEvent) {
        for (def, state) in self.packs.values_mut() {
            if let PackState::Active { progress } = state {
                for gate in &def.gates {
                    if gate.is_satisfied_by(event) {
                        progress.gates_satisfied.insert(gate.id);
                        progress.gates_remaining.remove(&gate.id);
                    }
                }
                if progress.gates_remaining.is_empty() {
                    *state = PackState::Completed {
                        completed_at: OffsetDateTime::now_utc(),
                    };
                }
            }
        }
    }

    /// Returns the effective constraint set (intersection of all active packs)
    /// → SPEC §6.3
    pub fn effective_constraints(&self) -> EffectiveConstraints { ... }
}
```

### P2.3 — Composition by intersection and conflict detection

→ SPEC §6.3

```rust
pub struct EffectiveConstraints {
    pub allowed_verbs: Option<HashSet<String>>,       // None = unconstrained
    pub allowed_entity_kinds: Option<HashSet<String>>, // None = unconstrained
}
```

- If no packs are active: `allowed_verbs = None` (unconstrained).
- If one pack is active: its verb/entity sets directly.
- If multiple active: intersection of their sets.
- If intersection is empty: return a `PackConflict` that surfaces as `ConstraintViolation` (→ SPEC §11.3).

### P2.4 — PackConstraintGate (real enforcement)

→ SPEC §7.3, §8.2

Replace the P1 stub:

```rust
pub fn check_pack_constraints(
    candidate_verbs: &[String],        // verb names from expanded DSL steps
    constraints: &EffectiveConstraints,
) -> Result<(), ConstraintViolationDetail> {
    if let Some(allowed) = &constraints.allowed_verbs {
        let violations: Vec<_> = candidate_verbs.iter()
            .filter(|v| !allowed.contains(*v))
            .collect();
        if !violations.is_empty() {
            return Err(ConstraintViolationDetail {
                explanation: format!(
                    "Expanded plan contains verbs outside active pack scope: {:?}",
                    violations
                ),
                expanded_plan: Some(candidate_verbs.to_vec()),
                active_constraints: /* serialize active pack constraints */,
                remediation_options: /* compute options */,
            });
        }
    }
    Ok(())
}
```

This gate is positioned in the pipeline **after** macro expansion, **before** validate/lint/semreg (→ SPEC §5.2).

### P2.5 — Completion widening and mid‑execution rule

→ SPEC §6.3

- When `PackManager::process_event` transitions a pack to `Completed`, the pack's constraints are removed from the effective set immediately.
- **Mid‑execution rule:** this widening applies to the **next** `process_utterance` call, not to any currently executing runbook (INV‑1a — runbook is immutable once executing).

### P2.6 — Wire Pack Manager into pipeline

- `process_utterance` calls `pack_manager.effective_constraints()` before verb discovery.
- Verb discovery filters candidates by pack constraints.
- After macro expansion (P1.4), call `check_pack_constraints(expanded_verbs, constraints)`.
- On rejection: return `OrchestratorResponse::ConstraintViolation(...)`.
- After `execute_runbook` emits session events, call `pack_manager.process_event(event)` for each.

### P2 Acceptance

- [ ] Pack state machine with Dormant/Active/Suspended/Completed transitions.
- [ ] Pack state is session-owned, mutated only by PackManager via session events.
- [ ] Multiple active packs compose by intersection; empty intersection surfaces as `ConstraintViolation`.
- [ ] PackConstraintGate rejects expanded plans with out-of-scope verbs and returns actionable remediation.
- [ ] Pack completion during execution widens constraints for the next compilation only.

**→ IMMEDIATELY proceed to Phase 3. Progress: ~65% complete.**

---

## Phase 3 — Plan Builder decomposition and error typing

**Goal:** Separate VerbClassifier / PlanAssembler / PackConstraintGate into distinct components with explicit failure types. → SPEC §7

**Depends on:** P1 (VerbClassifier exists), P2 (PackConstraintGate exists). This phase formalises what P1 and P2 created into clean module boundaries.

### P3.1 — Module structure

```
rust/src/plan_builder/
  mod.rs              // PlanBuilder orchestrator
  verb_classifier.rs  // §7.1 — VerbClassification enum + classify_verb()
  plan_assembler.rs   // §7.2 — step ordering, dependency detection
  constraint_gate.rs  // §7.3 — PackConstraintGate (moved from wherever P2 put it)
  errors.rs           // typed errors for each component
```

### P3.2 — Error typing

→ SPEC §5.3

```rust
/// Errors from verb classification
pub enum ClassificationError {
    UnknownVerb { verb: String, suggestions: Vec<String> },
    AmbiguousVerb { verb: String, candidates: Vec<VerbClassification> },
}

/// Errors from plan assembly
pub enum AssemblyError {
    CyclicDependency { cycle: Vec<String> },
    MissingDependency { step: String, requires: String },
    InvalidStepOrder { detail: String },
}

/// Constraint gate results are already typed in P2.4
```

Each error type maps to an `OrchestratorResponse` variant:
- `ClassificationError` → `ClarificationRequest` (user should clarify verb)
- `AssemblyError` → `CompilationError` (system error, surfaced to user as diagnostic)
- `ConstraintViolationDetail` → `ConstraintViolation` (→ SPEC §11.3)

### P3.3 — PlanAssembler

→ SPEC §7.2

Takes the output of VerbClassifier (one or more classified verbs) and macro expansion, produces an ordered step list:

```rust
pub struct AssembledPlan {
    pub steps: Vec<RunbookStep>,
    pub dependencies: Vec<(StepIndex, StepIndex)>,  // (step, depends_on)
}

pub fn assemble_plan(
    classified_verbs: &[VerbClassification],
    expanded_macros: &[MacroExpansionResult],
    entity_bindings: &EntityBindings,
) -> Result<AssembledPlan, AssemblyError>;
```

### P3 Acceptance

- [ ] Plan Builder is three distinct components with clear module boundaries.
- [ ] Each component has typed errors that map cleanly to `OrchestratorResponse` variants.
- [ ] No silent failures — every error path produces a user-surfaceable diagnostic.

**→ IMMEDIATELY proceed to Phase 4. Progress: ~80% complete.**

---

## Phase 4 — Contract‑first harness consolidation

**Goal:** All tests consume the public pipeline contract (§11.2). No test pins internal types. → SPEC §3.3, §11.1

**Depends on:** P0–P3 (the public contract exists and works end-to-end).

### P4.1 — Canonical integration harness

Create one test harness that exercises the full pipeline:

```rust
/// The canonical test path. All integration tests use this.
pub async fn test_utterance_to_execution(
    session: &TestSession,
    utterance: &str,
) -> TestResult {
    // 1. Compile
    let response = process_utterance(&session.id, utterance).await?;

    match response {
        OrchestratorResponse::Compiled(summary) => {
            // 2. Execute via gate (→ SPEC §11.1)
            let result = execute_runbook(
                &session.id,
                &summary.compiled_runbook_id,
                None,
            ).await?;
            TestResult::Executed(result)
        }
        OrchestratorResponse::Clarification(c) => TestResult::NeedsClarification(c),
        OrchestratorResponse::ConstraintViolation(v) => TestResult::Rejected(v),
    }
}
```

### P4.2 — Required integration tests

| Test name | What it proves | Spec ref |
|---|---|---|
| `macro_end_to_end` | Utterance → macro selection → expansion → compile → execute | §8.1, §11.1 |
| `pack_scoping` | Active pack constrains verb candidates; out-of-scope plan returns `ConstraintViolation` | §6, §7.3, §8.2 |
| `pack_completion_widening` | Pack completion during execution widens constraints for next compilation | §6.3 |
| `replay_determinism` | Same compiled runbook + envelope produces identical execution | §9, INV-2 |
| `locking_no_deadlock` | Concurrent `execute_runbook` calls with overlapping entity sets don't deadlock | §10.2 |
| `execution_gate_rejects_raw` | `execute_runbook` rejects non-persisted DSL, direct verb, direct macro | §11.1, INV-3 |
| `constraint_violation_remediation` | `ConstraintViolation` includes actionable remediation options | §11.3 |
| `runbook_immutability` | Executing runbook is not mutated; outcomes are session events | §1, INV-1a |

### P4.3 — Harness cleanup

1. Find all existing test harnesses:
   ```bash
   find rust/ -name "*harness*" -o -name "*test_runner*" | head -30
   grep -rn "fn.*harness\|fn.*test_scenario" rust/src/ rust/xtask/
   ```

2. For each:
   - If it calls internal types that bypass `process_utterance` / `execute_runbook`: refactor to use the canonical harness, or delete.
   - If it tests a genuinely internal unit (parser, expander in isolation): keep, but it must not call execution functions.
   - Remove `TODO(P4)` markers added in P0.5.

3. Shell harnesses that call deprecated endpoints: delete.

### P4 Acceptance

- [ ] One canonical integration harness used by all integration tests.
- [ ] All integration tests go through `process_utterance` → `execute_runbook` (no shortcuts).
- [ ] No test constructs a `CompiledRunbook` manually and feeds it to the executor.
- [ ] `grep -rn "TODO(P4)" rust/` returns zero results.
- [ ] Legacy harnesses (`xtask/*harness.rs`, `bin/batch_test_harness.rs`) deleted or refactored.

**→ IMMEDIATELY proceed to Phase 5. Progress: ~92% complete.**

---

## Phase 5 — Learning/feedback seam (choose direction)

**Goal:** No dead seams. Feedback either works correctly through the single pipeline or is cleanly removed. → SPEC (implied by §11 — one contract, no hidden state).

**Depends on:** P4 (pipeline is consolidated; we can see what's live and what's dead).

### P5.1 — Audit the feedback/learning surface

```bash
grep -rn "learning\|feedback\|outcome_writer\|intent_trace.*store\|session.*learn" rust/src/
```

Classify each hit:
- **Live and correct:** writes to session event log using data from `process_utterance` output (IntentTrace, chosen candidate). Keep.
- **Live but wrong:** captures from legacy intent structs or internal pipeline types. Rewire to the single pipeline output.
- **Dead:** session fields / endpoints / writers that nothing reads. Delete.

### P5.2 — If learning is deferred (recommended for initial delivery)

- Delete unused session fields related to learning/feedback.
- Delete outcome writer endpoints/functions that have no consumers.
- Add a `// FUTURE: learning/feedback capture point` comment at the single pipeline output point.

### P5.3 — If learning is active

- Rewire feedback capture to consume `IntentTrace` + chosen candidate from `OrchestratorResponse`.
- Store as session events (not hidden state).
- Ensure feedback writes go through the session event log (same as execution events), maintaining one event stream.

### P5 Acceptance

- [ ] `grep -rn "learning\|feedback\|outcome_writer" rust/src/` returns only live, correctly-wired code or clean removal.
- [ ] No session fields exist that are written but never read.
- [ ] If deferred: zero learning/feedback code paths remain. If active: all feedback flows through session event log.

**→ Phase 5 DONE. Progress: 100% complete.**

---

## Final verification checklist

Map each item to a spec invariant or section:

| Check | Spec ref | How to verify |
|---|---|---|
| `execute_runbook` is the only mutation path | INV-3, §11.1 | `grep` for direct verb/macro execution outside the gate |
| Compiled runbooks are immutable | INV-1a, §1 | No code path writes to a runbook post-`Compiled` status |
| Replay with envelope is deterministic | INV-2, §9 | `replay_determinism` test passes |
| Entity locks acquired only in `execute_runbook` | §10.1 | `grep` for lock acquisition outside the gate |
| Pack constraints enforced post-expansion | §8.2, §7.3 | `pack_scoping` test: macro expansion rejected when out-of-scope |
| One macro registry | §8.3 | `OperatorMacroRegistry` gone or thin wrapper |
| One public pipeline contract | §11.2 | `process_utterance` and `execute_runbook` are the only exported surfaces |
| No test pins internal types | §3.3 | All integration tests use canonical harness |
| Three OrchestratorResponse variants | §11.3 | Type system enforces it |
| Runbook versioning is monotonic | §11.4 | Version counter test; no version reuse |

---

## Notes for Claude Code execution

1. **Read the spec first.** Before writing any code for a phase, read the referenced spec sections. The spec is normative.

2. **Run tests after each sub-step.** Don't accumulate changes across an entire phase before testing.

3. **If a grep reveals more shadow paths than expected in P0.5,** don't try to fix them all immediately. Mark with `TODO(P4)` and move on. P4 is specifically for harness cleanup.

4. **The Rust type signatures in this TODO are illustrative, not prescriptive.** Adapt field names and module locations to the existing codebase conventions. The *contracts* (what each type must contain, what each function must accept/reject) are normative per the spec.

5. **Continuation gates matter.** After each phase, emit progress and immediately continue. Do not wait for confirmation between phases unless blocked by a compilation error or test failure that requires human input.
