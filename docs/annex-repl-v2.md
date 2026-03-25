# V2 REPL — Detailed Annex

> This annex covers the V2 REPL architecture, pack system, scoring, preconditions,
> context stack, golden corpus, session trace/replay, and runbook compilation.
> For the high-level overview see the root `CLAUDE.md`.

---

## Architecture Overview

**Directory:** `rust/src/repl/` (24 files) + `rust/src/runbook/` (10 files)

**Feature gate:** `vnext-repl` — all V2 REPL code is behind this Cargo feature.

| File | Purpose |
|------|---------|
| `repl/orchestrator_v2.rs` | Core state machine dispatcher (~700 lines) |
| `repl/types_v2.rs` | `ReplStateV2` enum + input/output types |
| `repl/session_v2.rs` | Session container (runbook + state + trace) |
| `repl/response_v2.rs` | Response kinds for different UI states |
| `repl/context_stack.rs` | Unified context derived from runbook fold |
| `repl/intent_service.rs` | Unified 5-phase intent matching pipeline |
| `repl/intent_matcher.rs` | Pure function trait for verb matching |
| `repl/proposal_engine.rs` | Deterministic step proposal generation |
| `repl/scoring.rs` | Pack-scoped verb scoring + ambiguity policy |
| `repl/preconditions.rs` | DAG-aware eligibility gating |
| `repl/decision_log.rs` | Structured logging for replay + tuning |
| `repl/session_trace.rs` | Append-only mutation log |
| `repl/session_replay.rs` | Trace replay engine (Strict/Relaxed/DryRun) |
| `repl/sentence_gen.rs` | Deterministic sentence generation from templates |
| `repl/verb_config_index.rs` | In-memory verb metadata index |
| `repl/executor_bridge.rs` | Bridge to DSL pipeline execution |
| `repl/session_repository.rs` | Session persistence (database feature) |
| `repl/trace_repository.rs` | Trace log persistence (database feature) |
| `runbook/types.rs` | Core runbook types |
| `runbook/plan_compiler.rs` | Compilation pipeline |
| `runbook/plan_executor.rs` | Execution gate |
| `runbook/plan_types.rs` | Plan type definitions |
| `runbook/narration.rs` | Narration generation |

---

## 7-State Machine (ReplStateV2)

```rust
pub enum ReplStateV2 {
    /// 1. Waiting for client/scope selection before pack can start
    ScopeGate {
        pending_input: Option<String>,
        candidates: Option<Vec<BootstrapCandidate>>,
    },
    /// 2. Scope selected; must now choose a workspace
    WorkspaceSelection { workspaces: Vec<WorkspaceOption> },
    /// 3. Scope + workspace; now choosing a journey pack
    JourneySelection { candidates: Option<Vec<PackCandidate>> },
    /// 4. Inside an active pack — Q&A, verb matching, building runbook
    InPack {
        pack_id: String,
        required_slots_remaining: Vec<String>,
        last_proposal_id: Option<Uuid>,
    },
    /// 5. Waiting for user to disambiguate a verb or entity
    Clarifying {
        question: String,
        candidates: Vec<VerbCandidate>,
        original_input: String,
    },
    /// 6. Showing a sentence for user to confirm or reject
    SentencePlayback {
        sentence: String,
        verb: String,
        dsl: String,
        args: HashMap<String, String>,
    },
    /// 7. Runbook exists; user reviewing/editing
    RunbookEditing,
    // (+ Executing phase during active runbook execution)
    Executing { runbook_id: Uuid, progress: ExecutionProgress },
}
```

---

## Orchestrator V2 — Dispatch Table

**Entry:** `pub async fn process(&self, session_id: Uuid, input: UserInputV2) -> Result<ReplResponseV2>`

| From State | Input | Handler | To State |
|-----------|-------|---------|---------|
| ScopeGate | Message | `try_resolve_scope()` | WorkspaceSelection or ScopeGate |
| ScopeGate | SelectScope | `set_scope()` | WorkspaceSelection |
| WorkspaceSelection | SelectWorkspace | `set_workspace()` | JourneySelection |
| JourneySelection | Message | `route_pack()` | InPack or JourneySelection |
| JourneySelection | SelectPack | `activate_pack()` | InPack |
| InPack | Message | `handle_in_pack_msg()` | SentencePlayback or InPack |
| InPack | Command(Run) | `validate_and_execute()` | Executing |
| Clarifying | Message/Select | `resolve_clarification()` | SentencePlayback or Clarifying |
| SentencePlayback | Confirm | `add_to_runbook()` | RunbookEditing or InPack |
| SentencePlayback | Reject | `discard_proposal()` | InPack |
| RunbookEditing | Command(Run) | `execute_runbook()` | Executing |
| RunbookEditing | Message | `handle_in_pack_msg()` | SentencePlayback |
| Executing | completion | `execute_runbook_from()` | RunbookEditing |

**Execution outcomes:**
```rust
pub enum DslExecutionOutcome {
    Completed(serde_json::Value),
    Parked { task_id: Uuid, correlation_key: String, timeout: Option<Duration> },
    Failed(String),
}
```

---

## Context Stack

**File:** `rust/src/repl/context_stack.rs`

**Design principle:** Session state is a **left fold over executed runbook entries**. No mutable scope object. No session table.

**ONLY constructor:** `ContextStack::from_runbook(runbook, staged_pack, turn)`

```rust
pub struct ContextStack {
    pub derived_scope: DerivedScope,              // CBU, client group, book
    pub pack_staged: Option<PackContext>,         // Active pack (preferred)
    pub pack_executed: Option<PackContext>,       // Executed pack (fallback)
    pub template_hint: Option<TemplateStepHint>, // Next expected step from template
    pub focus: FocusContext,                      // Pronoun resolution
    pub recent: RecentContext,                    // Last N entity mentions
    pub exclusions: ExclusionSet,                 // Rejected candidates (3-turn decay)
    pub outcomes: OutcomeRegistry,                // Execution results by entry ID
    pub accumulated_answers: HashMap<String, serde_json::Value>,
    pub executed_verbs: HashSet<String>,
    pub staged_verbs: HashSet<String>,
    pub turn: u32,                                // For exclusion decay
}
```

**Invariant:** No mutator methods. To change context, produce a different runbook and fold again.

---

## Pack System

**Config:** `rust/config/packs/` (5 YAML files)

| Pack | Purpose |
|------|---------|
| `onboarding-request.yaml` | CBU onboarding workflow |
| `book-setup.yaml` | Book/workspace setup |
| `kyc-case.yaml` | KYC case management |
| `session-bootstrap.yaml` | Session scope initialization |
| `product-service-taxonomy.yaml` | Product/service classification |

**Pack YAML structure (key fields):**
```yaml
id: kyc-case
invocation_phrases:            # Semantic scoring for pack selection
  - "open a KYC case"
  - "start KYC review"
required_context: [client_group_id]
optional_context: [default_cbu, target_entity]
workspaces: [kyc, on_boarding]
allowed_verbs:
  - kyc.create-case
  - kyc.assign-reviewer
forbidden_verbs:
  - kyc.delete-case
required_questions:
  - field: entity_name
    prompt: "Which entity is this KYC case for?"
    answer_kind: entity_ref
stop_rules:
  - "KYC case created"
  - "UBO discovery complete"
```

**Loading mechanism:**
1. `PackRouter::new(packs)` loads all packs
2. `PackSemanticScorer` scores packs against utterance via invocation phrases
3. Top-scored pack recommended; user can force-select via `pack.select` verb
4. Template steps boost verbs matching the next expected step

---

## Scoring System

**File:** `rust/src/repl/scoring.rs`

**Constants:**
```rust
pub const PACK_VERB_BOOST: f32 = 0.10;       // In-pack verbs
pub const PACK_VERB_PENALTY: f32 = 0.05;     // Out-of-pack verbs
pub const TEMPLATE_STEP_BOOST: f32 = 0.15;  // Next expected template step
pub const DOMAIN_AFFINITY_BOOST: f32 = 0.03;
pub const ABSOLUTE_FLOOR: f32 = 0.55;        // Drop candidates below this
pub const THRESHOLD: f32 = 0.55;             // Top candidate must exceed this
pub const MARGIN: f32 = 0.05;                // Ambiguity margin
pub const STRONG_THRESHOLD: f32 = 0.70;      // No disambiguation needed above this
```

**Algorithm (per candidate):**
1. If forbidden → remove entirely
2. If in-pack → boost +0.10
3. Else if out-of-pack → penalize -0.05
4. If template_step → boost +0.15
5. If domain affinity → boost +0.03
6. If excluded (ExclusionSet) → remove
7. If score < ABSOLUTE_FLOOR → remove
8. Re-sort descending

**Ambiguity policy (Invariant I-5):**
- top ≥ STRONG_THRESHOLD → no clarification, use top
- top - runner_up ≥ MARGIN → "Confident"
- top ≥ THRESHOLD → "Ambiguous" (ask user)
- else → "NoMatch"

---

## Preconditions

**File:** `rust/src/repl/preconditions.rs`

```rust
pub struct Preconditions {
    pub requires_scope: Vec<String>,   // e.g. ["cbu"]
    pub requires_prior: Vec<String>,   // e.g. ["cbu.create"]
    pub requires_entities: Vec<String>,
    pub forbids_prior: Vec<String>,    // e.g. ["cbu.delete"]
}
```

**Eligibility modes:**
- `Executable` — only completed entries count as facts
- `Plan` — executed + staged (confirmed but not yet executed)

Returns `Vec<PreconditionResult>` with `met: bool` and `unmet_reasons: Vec<UnmetReason>`.

---

## Intent Matching — 3-Pronged

**File:** `rust/src/repl/intent_matcher.rs`

```rust
#[async_trait]
pub trait IntentMatcher: Send + Sync {
    async fn match_intent(&self, utterance: &str, context: &MatchContext)
        -> Result<IntentMatchResult>;
    async fn search_with_context(&self, utterance: &str, context: &MatchContext, stack: &ContextStack)
        -> Result<IntentMatchResult>;
    fn is_direct_dsl(&self, input: &str) -> bool;
}
```

**Three prongs:**
1. **Template Fast-Path** — word-overlap scoring against pack template steps
2. **Verb Matching Fallback** — `IntentMatcher.match_intent()` returns semantic candidates
3. **Ambiguity Policy** — `apply_ambiguity_policy()` → Confident / Ambiguous / NoMatch

Direct DSL detection (`input.starts_with('(')`) is **logged but does not bypass SemReg filtering** — all utterances flow through semantic search.

**FocusMode / ExclusionSet:**
- `FocusMode` — derived from recent executed verbs, drives domain-scoped scoring; enables pronoun/shorthand resolution
- `ExclusionSet` — 3-turn decay on user rejections; prevents immediate re-proposal of rejected options

---

## Runbook & CompiledRunbook

**Files:** `rust/src/runbook/`

**Entry lifecycle:**
```
Proposed → Confirmed → Resolved → Executing → Completed / Failed / Parked / Disabled
```

**EntryStatus confirm policies:**
- `QuickConfirm` — skip confirmation for navigation verbs
- `RequireConfirm` — user must confirm
- `RequireApproval` — human approver gate (durable execution)

**Execution modes:**
- `Sync` — normal synchronous execution
- `Durable` — parking-aware (BPMN integration)
- `HumanGate` — pause before execution, require approval

**RunbookStatus:**
```
Draft → Building → Ready → Executing → Completed / Parked / Aborted
```

**CompiledRunbook:**
```rust
pub struct CompiledRunbook {
    pub id: CompiledRunbookId,           // UUID-wrapped opaque handle
    pub session_id: Uuid,
    pub version: u64,                    // Monotonic within session
    pub steps: Vec<CompiledStep>,        // Frozen at creation
    pub envelope: ReplayEnvelope,        // Determinism boundary
    pub status: CompiledRunbookStatus,
    pub created_at: DateTime<Utc>,
}

pub struct CompiledStep {
    pub step_id: Uuid,
    pub sentence: String,                // Human-readable
    pub verb: String,                    // FQN e.g. "cbu.create"
    pub dsl: String,                     // S-expression
    pub args: BTreeMap<String, String>,  // BTreeMap for deterministic serialization
    pub arg_sources: HashMap<String, SlotSource>,
    pub gate_type: GateType,
}
```

**Invariants:**
- **INV-1a:** `CompiledRunbook` is immutable once created; status transitions never mutate steps
- **INV-2:** Same input → same ID (content-addressed via SHA-256)
- **INV-3:** `execute_runbook()` is the **ONLY** path to the executor; no raw DSL execution

**Compilation surface:**
```rust
pub fn compile_verb(
    session_id, classification, args, session, macro_registry,
    runbook_version, constraints, sem_reg_allowed_verbs, verb_snapshot_pins
) -> OrchestratorResponse
```

---

## Session Trace & Replay

### SessionTrace

**File:** `rust/src/repl/session_trace.rs`

```rust
pub enum TraceOp {
    StackPush { workspace: WorkspaceKind },
    StackPop { workspace: WorkspaceKind },
    StackCommit,
    VerbExecuted { verb_fqn: String, step_id: Uuid },
    RunbookCompiled { runbook_id: String },
    RunbookApproved { runbook_id: String },
    StateTransition { from: String, to: String },
    Input { utterance_hash: String },
}

pub struct TraceEntry {
    pub session_id: Uuid,
    pub sequence: u64,                 // Monotonic
    pub timestamp: DateTime<Utc>,
    pub agent_mode: AgentMode,
    pub op: TraceOp,
    pub stack_snapshot: Vec<FrameRef>,
    pub snapshot: Option<serde_json::Value>, // Gated by SnapshotPolicy
}
```

**SnapshotPolicy:**
- `Never` — no snapshots
- `EveryN(u32)` — every N operations
- `OnStackOp` — on push/pop/commit
- `OnExecution` — on each verb execution

### SessionReplay

**File:** `rust/src/repl/session_replay.rs`

**Replay modes:**
- `Strict` — verify intermediate state matches snapshots exactly; fail on divergence
- `Relaxed` — log divergences, continue
- `DryRun` — skip verb execution, compare decisions only

```rust
pub struct ReplayResult {
    pub mode: ReplayMode,
    pub entries_replayed: usize,
    pub divergences: Vec<ReplayDivergence>,
    pub final_state: Option<serde_json::Value>,
}
```

**Use cases:** compliance auditing, regression testing, production diagnostics.

---

## DecisionLog

**File:** `rust/src/repl/decision_log.rs`

```rust
pub struct DecisionLog {
    pub id: Uuid,
    pub session_id: Uuid,
    pub turn: u32,
    pub timestamp: DateTime<Utc>,
    pub input_hash: String,             // SHA-256 (always present)
    pub raw_input: Option<String>,      // Redacted in operational mode (Invariant I-10)
    pub verb_candidates_pre: Vec<VerbCandidateSnapshot>,
    pub verb_candidates_post: Vec<VerbCandidateSnapshot>,
    pub arg_extraction_method: ExtractionMethod,
    pub arg_extraction_audit: Option<ArgExtractionAudit>,
    pub scoring_config: ScoringConfig,  // Snapshot for replay tuner
    pub dsl_proposed: String,
}
```

`SessionDecisionLog` accumulates entries across turns for offline replay and scoring parameter sweeps.

---

## Golden Corpus & Test Coverage

**Test files:**

| File | Lines | Purpose |
|------|-------|---------|
| `tests/repl_v2_golden_loop.rs` | 947 | Phase 0: pack select → Q/A → sentence → confirm → execute |
| `tests/repl_v2_integration.rs` | 1,300+ | Phase 1: real packs, verb filtering, arg audit, confirm policy |
| `tests/repl_v2_phase3.rs` | 948 | Phase 3: proposal engine, ranked proposals, template scoring |
| `tests/repl_v2_phase4.rs` | 1,720 | Phase 4: scoring calibration, ambiguity policy, clarification UX |
| `tests/repl_v2_phase5.rs` | 1,964 | Phase 5: durable execution, parking, human gates (28 cases) |
| `tests/repl_v2_phase6.rs` | 1,573 | Phase 6: session repository, persistence, replay |

**Test categories:** state machine transitions, scoring (boost/penalty/floor), precondition eligibility, proposal engine, sentence generation, durable execution, golden corpus replay, persistence, ambiguity policy, pack routing.

**Replay tuner CLI:** sweeps `ScoringConfig` constants offline to find optimal thresholds without rerunning live tests.

---

## DslStatus (Agent API — separate from REPL)

**Note:** This is for the Agent API (`/api/session/*`), not V2 REPL internals.

```rust
pub enum DslStatus {
    Draft,      // AST valid, awaiting confirmation
    Ready,      // User confirmed, ready to execute
    Executed,   // Successfully executed
    Cancelled,  // User declined
    Failed,     // Execution failed
}
```

---

## Session V2 Shape

```rust
pub struct ReplSessionV2 {
    pub id: Uuid,
    pub state: ReplStateV2,
    pub staged_pack: Option<Arc<PackManifest>>,
    pub runbook: Runbook,
    pub messages: Vec<ChatMessage>,
    pub decision_log: SessionDecisionLog,
    pub trace: Vec<TraceEntry>,
    pub agent_mode: AgentMode,
    pub workspace_stack: Vec<WorkspaceFrame>,
    pub conversation_mode: ConversationMode,
    // + context, pending lookups, etc.
}
```

---

## Integration Points

| System | Integration |
|--------|-------------|
| DSL Pipeline | `executor_bridge.rs` — `RealDslExecutor` wraps dsl_v2 pipeline |
| BPMN-Lite | `DslExecutorV2` trait with parking signals; `ExecutionMode::Durable` |
| SemOS | `SemOsContextEnvelope` validates verbs before execution; TOCTOU recheck |
| Persistence | `session_repository.rs` + `trace_repository.rs` (behind `database` feature) |
