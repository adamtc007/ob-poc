# AUDIT-CURRENT-STRUCTURES.md

> **Phase 0 Artifact** — Pre-implementation codebase audit for Journey Packs as Semantic Context
> **Date:** 2026-02-06
> **Scope:** V2 REPL (`rust/src/repl/`), Journey (`rust/src/journey/`), related test files
> **Total lines audited:** ~15,973 across 27 source files + 10 test files

---

## 1. Structure Inventory — Fate Map

### 1.1 Session Layer

| Struct | File | Lines | Fate | Rationale |
|--------|------|-------|------|-----------|
| `ReplSessionV2` | `session_v2.rs` | ~438 | **REFACTOR** | Remove `client_context`, `journey_context` fields. Keep `id`, `state`, `runbook`, `messages`, `pending_arg_audit`, `last_proposal_set`. |
| `ClientContext` | `session_v2.rs` | ~30 | **DELETE** | All 4 fields (`client_group_id`, `client_group_name`, `default_cbu`, `default_book`) are already on `Runbook` or derivable from runbook fold. |
| `JourneyContext` | `session_v2.rs` | ~50 | **DELETE** | `pack`, `pack_manifest_hash` duplicated on `Runbook`. `answers` needs new representation as runbook entry args. `template_id`, `progress`, `handoff_source` derivable from runbook fold. |

**Mutation sites for `client_context`:**

| Location | Operation | Classification |
|----------|-----------|----------------|
| `session_v2.rs:set_client_context()` | Write client_group_id/name | REPLACE-WITH-RUNBOOK-FOLD |
| `session_v2.rs:set_client_context()` | Write runbook.client_group_id (duplication) | REMOVE (already on runbook) |
| `orchestrator_v2.rs:complete_scope_gate()` | Calls `set_client_context()` | REPLACE — fold from `session.load-cluster` entry |
| `orchestrator_v2.rs` (~8 reads) | Reads `client_context.client_group_id` for scope checks | REPLACE — derive from `ContextStack.derived_scope()` |
| `orchestrator_v2.rs` (~2 reads) | Reads `client_context.client_group_name` for messages | REPLACE — derive from runbook fold |

**Mutation sites for `journey_context`:**

| Location | Operation | Classification |
|----------|-----------|----------------|
| `session_v2.rs:activate_pack()` | Write pack, hash, also runbook.pack_id/version/hash | REPLACE-WITH-RUNBOOK-FOLD |
| `session_v2.rs:set_journey_answers()` | Write `answers` HashMap | REPLACE — store as runbook entry args |
| `session_v2.rs:set_journey_progress()` | Write `progress` | REPLACE — derive from completed entries |
| `session_v2.rs:set_handoff_source()` | Write `handoff_source` | REPLACE — derive from `pack.select` entries |
| `session_v2.rs:clear_journey()` | Clears all journey fields | REPLACE — pack transition = new `pack.select` entry |
| `orchestrator_v2.rs` (~19 reads) | Reads pack, answers, template_id, progress | REPLACE — all derivable from `ContextStack` |
| `orchestrator_v2.rs:handle_handoff()` (line ~2862) | **Clears runbook entirely** — DANGEROUS | REPLACE — handoff should NOT clear runbook |

**Critical finding:** Pack answers (`journey_context.answers`) have **no current runbook representation**. Phase A must define how Q&A answers are stored as runbook entries (e.g., `pack.answer :question-id "q1" :value "yes"`).

### 1.2 State Machine

| Struct/Enum | File | Lines | Fate | Rationale |
|-------------|------|-------|------|-----------|
| `ReplStateV2` | `types_v2.rs` | ~50 | **KEEP** | 7-state machine is correct: ScopeGate, JourneySelection, InPack, Clarifying, SentencePlayback, RunbookEditing, Executing. |
| `UserInputV2` | `types_v2.rs` | ~80 | **KEEP** | Input variants cover all interaction types. May extend with pack-specific variants. |
| `ReplCommandV2` | `types_v2.rs` | ~40 | **KEEP + EXTEND** | Add power user commands: `why`, `options`, `switch journey`, `run step N`, `drop step N`. |
| `ExecutionProgress` | `types_v2.rs` | ~20 | **KEEP** | Tracks runbook execution progress. |
| `ClarifyingPayload` | `types_v2.rs` | ~50 | **KEEP** | Verb, entity, scope, client group disambiguation. |

### 1.3 Runbook (Authority)

| Struct/Enum | File | Lines | Fate | Rationale |
|-------------|------|-------|------|-----------|
| `Runbook` | `runbook.rs` | ~200 | **KEEP** | Single source of truth. Already has client_group_id, pack_id/version/hash, template_id/hash. |
| `RunbookEntry` | `runbook.rs` | ~100 | **KEEP + EXTEND** | Add `slot_provenance` tracking, pack answer entries. |
| `EntryStatus` | `runbook.rs` | ~20 | **KEEP** | 8 states: Proposed→Confirmed→Resolved→Executing→Completed/Failed/Parked/Disabled. |
| `ExecutionMode` | `runbook.rs` | ~10 | **KEEP** | Sync, Durable, HumanGate. |
| `ConfirmPolicy` | `runbook.rs` | ~10 | **KEEP** | Always, QuickConfirm, PackConfigured. |
| `SlotSource` | `runbook.rs` | ~10 | **KEEP** | UserProvided, TemplateDefault, InferredFromContext, CopiedFromPrevious. |
| `InvocationRecord` | `runbook.rs` | ~30 | **KEEP** | Durable execution correlation. |
| `RunbookEvent` | `runbook.rs` | ~50 | **KEEP** | Append-only audit log. |

### 1.4 Intent Pipeline

| Struct/Trait | File | Lines | Fate | Rationale |
|-------------|------|-------|------|-----------|
| `IntentMatcher` (trait) | `intent_matcher.rs` | ~39 | **REFACTOR** | Add `match_with_context(input, ContextStack)` method accepting ContextStack for pack-scoped search. |
| `IntentService` | `intent_service.rs` | ~597 | **REFACTOR** | Accept ContextStack. Route through `search_with_context()` instead of flat search. |
| `VerbMatchOutcome` | `intent_service.rs` | ~30 | **KEEP** | 8 variants cover all outcomes. |
| `MatchContext` | `types.rs` | ~40 | **REFACTOR** | Replace with `ContextStack` fields. Currently has session_id, cbu_ids, client_group_id — all derivable. |
| `MatchOutcome` | `types.rs` | ~30 | **KEEP** | Maps to ambiguity policy. |
| `VerbCandidate` | `types.rs` | ~20 | **KEEP** | Score, verb, description. |

### 1.5 Orchestrator

| Struct | File | Lines | Fate | Rationale |
|--------|------|-------|------|-----------|
| `ReplOrchestratorV2` | `orchestrator_v2.rs` | ~3612 | **REFACTOR** | Major refactor: build ContextStack from runbook on each turn, pass to intent pipeline. Remove all client_context/journey_context reads. ~40 `set_state()` calls to audit. |

**Key orchestrator methods and their fate:**

| Method | ~Line | Fate | Notes |
|--------|-------|------|-------|
| `process()` | 150 | KEEP | Main dispatch, add ContextStack construction at top |
| `complete_scope_gate()` | 300 | REFACTOR | Remove client_context write, fold from runbook |
| `handle_bootstrap_outcome()` | 450 | KEEP | Works correctly, produces runbook entry |
| `handle_journey_selection()` | 600 | REFACTOR | `pack.select` → runbook entry, not session metadata |
| `handle_in_pack()` | 800 | REFACTOR | Build ContextStack, pass to intent matching |
| `propose_for_input()` | 1200 | REFACTOR | ProposalEngine needs ContextStack |
| `match_verb_for_input()` | 1400 | REFACTOR | IntentService needs ContextStack |
| `handle_sentence_playback()` | 1800 | KEEP | Presentation layer |
| `handle_runbook_editing()` | 2000 | KEEP | User editing flow |
| `handle_executing()` | 2200 | KEEP | Execution coordination |
| `handle_handoff()` | 2862 | **CRITICAL REFACTOR** | Currently clears runbook — must preserve history |

### 1.6 Proposal Engine

| Struct | File | Lines | Fate | Rationale |
|--------|------|-------|------|-----------|
| `ProposalEngine` | `proposal_engine.rs` | ~832 | **REFACTOR** | Accept ContextStack. Template scoring uses pack context. |
| `StepProposal` | `proposal_engine.rs` | ~30 | **KEEP** | Proposal output type. |
| `ProposalSet` | `proposal_engine.rs` | ~20 | **KEEP** | Collection with hash for dedup. |

### 1.7 Journey / Pack Layer

| Struct | File | Lines | Fate | Rationale |
|--------|------|-------|------|-----------|
| `PackManifest` | `journey/pack.rs` | ~604 | **KEEP + EXTEND** | Add `forbidden_verbs`, `domain_affinity` if missing. Validate against §5.2 requirements. |
| `PackRouter` | `journey/router.rs` | ~540 | **KEEP** | Route user input to packs. Ensure produces `pack.select` DSL entry. |
| `PackSemanticScorer` | `journey/router.rs` | ~200 | **KEEP** | Scoring for pack selection. |
| `Template` | `journey/template.rs` | ~724 | **KEEP** | Template instantiation. |
| `ChapterView` | `journey/playback.rs` | ~290 | **KEEP** | Presentation of progress. |
| `HandoffContext` | `journey/handoff.rs` | ~50 | **REFACTOR** | Context forwarding via runbook `@N` refs, not mutable state. |

### 1.8 Support Modules

| Module | File | Lines | Fate | Rationale |
|--------|------|-------|------|-----------|
| `verb_config_index.rs` | repl/ | ~755 | **KEEP** | VerbConfigIndex, sentence templates. |
| `sentence_gen.rs` | repl/ | ~514 | **KEEP** | SentenceGenerator — converts verb+args to NL. |
| `executor_bridge.rs` | repl/ | ~148 | **KEEP** | Bridge to execution engine. |
| `session_repository.rs` | repl/ | ~258 | **KEEP** | DB persistence for sessions. |
| `response_v2.rs` | repl/ | ~263 | **KEEP** | Response types for API. |
| `bootstrap.rs` | repl/ | ~432 | **REFACTOR** | Bootstrap resolution stays, but becomes the resolver for `session-bootstrap` pack. |

### 1.9 New Modules (Phase A+)

| Module | Phase | Purpose |
|--------|-------|---------|
| `context_stack.rs` | A | ContextStack, DerivedScope, FocusContext, ExclusionSet, canonicalization |
| `scoring.rs` | C | Pack scoring constants and `apply_pack_scoring()` |
| `deterministic_extraction.rs` | F | Skip LLM when ContextStack provides all args |
| `decision_log.rs` | G | Structured logging for tuning |

---

## 2. Session Mutation Sites — Complete Inventory

### 2.1 Fields to REPLACE with Runbook Fold

| Field | Write Sites | Read Sites | Replacement |
|-------|-------------|------------|-------------|
| `client_context.client_group_id` | `set_client_context()` | ~8 in orchestrator | `ContextStack.derived_scope().client_group_id` |
| `client_context.client_group_name` | `set_client_context()` | ~2 in orchestrator | `ContextStack.derived_scope().client_group_name` |
| `client_context.default_cbu` | `set_client_context()` | ~1 in orchestrator | `ContextStack.derived_scope().default_cbu` |
| `client_context.default_book` | `set_client_context()` | ~1 in orchestrator | `ContextStack.derived_scope().default_book` |
| `journey_context.pack` | `activate_pack()` | ~10 in orchestrator | `ContextStack.pack_context()` |
| `journey_context.pack_manifest_hash` | `activate_pack()` | ~2 in orchestrator | Derived from `pack.select` entry |
| `journey_context.answers` | `set_journey_answers()` | ~5 in orchestrator | Runbook entry args for `pack.answer` verb |
| `journey_context.template_id` | `activate_pack()` | ~3 in orchestrator | Derived from `pack.select` entry |
| `journey_context.progress` | `set_journey_progress()` | ~4 in orchestrator | Count completed entries matching template |
| `journey_context.handoff_source` | `set_handoff_source()` | ~2 in orchestrator | Derived from previous `pack.select` entries |

### 2.2 Fields to KEEP as Transient (Not Persisted)

| Field | Purpose | Justification |
|-------|---------|---------------|
| `pending_arg_audit` | In-flight argument collection | Conversation-scoped, not durable |
| `last_proposal_set` | Most recent proposals shown | Conversation-scoped, dedup only |
| `state` (ReplStateV2) | Current machine state | Could be derived but simpler as cache |
| `messages` | Chat history | Presentation layer, not authority |

### 2.3 Runbook Fields Already Duplicating Session State

| Runbook Field | Session Field | Notes |
|---------------|---------------|-------|
| `runbook.client_group_id` | `client_context.client_group_id` | Written simultaneously in `set_client_context()` |
| `runbook.pack_id` | `journey_context.pack.id` | Written simultaneously in `activate_pack()` |
| `runbook.pack_version` | `journey_context.pack.version` | Written simultaneously in `activate_pack()` |
| `runbook.pack_manifest_hash` | `journey_context.pack_manifest_hash` | Written simultaneously in `activate_pack()` |
| `runbook.template_id` | `journey_context.template_id` | Written simultaneously in `activate_pack()` |

This duplication confirms the runbook already contains the data needed to derive session state.

---

## 3. Test Coverage Inventory

### 3.1 Test Files

| Test File | Focus | Will Break in Phase |
|-----------|-------|---------------------|
| `repl_v2_integration.rs` | End-to-end V2 REPL flows | A (ContextStack), C (verb search) |
| `repl_v2_phase2.rs` | Pack selection, journey context | A (JourneyContext deletion) |
| `repl_v2_phase3.rs` | Proposal engine, template scoring | C (pack-scoped scoring) |
| `repl_v2_phase4.rs` | Durable execution, human gates | — (unlikely to break) |
| `repl_v2_phase5.rs` | Runbook editing, undo/redo | — (unlikely to break) |
| `repl_v2_phase6.rs` | Parity matrix, pack handoff | A (client_context), H (handoff refactor) |
| `repl_v2_golden_loop.rs` | Golden path: scope → pack → execute | A (ClientContext), C (verb search) |
| `verb_search_integration.rs` | Verb semantic search accuracy | C (search_with_context) |
| `staged_runbook_integration.rs` | Legacy staged runbook | — (V1, not affected) |
| `incremental_session.rs` | Session persistence | A (session shape change) |

### 3.2 Test Impact by Phase

| Phase | Tests Affected | Risk |
|-------|----------------|------|
| A | `repl_v2_integration`, `repl_v2_phase2`, `repl_v2_phase6`, `repl_v2_golden_loop`, `incremental_session` | HIGH — ClientContext/JourneyContext deleted |
| B | `repl_v2_phase2`, `repl_v2_golden_loop` | MEDIUM — pack selection flow changes |
| C | `repl_v2_phase3`, `verb_search_integration`, `repl_v2_golden_loop` | HIGH — verb search API changes |
| D | `repl_v2_integration` | MEDIUM — entity resolution changes |
| E-G | Targeted tests | LOW — additive features |
| H | `repl_v2_phase6`, `repl_v2_golden_loop` | HIGH — ClientContext/JourneyContext deletion completed |

---

## 4. Architectural Invariants (from Research Document)

| ID | Invariant | Current Status | Gap |
|----|-----------|----------------|-----|
| I-1 | Runbook is sole durable artifact | ✅ Runbook exists | Client/Journey context duplicates data |
| I-2 | Session state = left fold over runbook | ❌ Not implemented | ClientContext/JourneyContext are mutable stores |
| I-3 | Pack context narrows verb search | ❌ Not implemented | Flat verb search, no pack scoring |
| I-4 | Bounded candidate universe | ❌ Not implemented | No `build_candidate_universe()` |
| I-5 | Uniform ambiguity policy | Partial | Ambiguity detection exists but no pack-aware thresholds |
| I-6 | Focus/pronoun resolution | ❌ Not implemented | No FocusContext |
| I-7 | Bootstrap is a pack | ❌ Not implemented | ScopeGate is special state, not a pack |
| I-8 | Deterministic extraction before LLM | ❌ Not implemented | Always falls to LLM for arg extraction |
| I-9 | No ESPER/viewport coupling | ✅ Already true | ESPER crates deprecated |
| I-10 | Decision log for tuning | ❌ Not implemented | No structured logging |

---

## 5. Summary Statistics

| Metric | Value |
|--------|-------|
| Source files in `repl/` | 21 |
| Source files in `journey/` | 6 |
| Test files | 10 |
| Total lines (source) | ~15,973 |
| Structs to DELETE | 2 (`ClientContext`, `JourneyContext`) |
| Structs to REFACTOR | 5 (`ReplSessionV2`, `IntentMatcher`, `IntentService`, `MatchContext`, `ReplOrchestratorV2`) |
| Structs to KEEP | 20+ |
| New modules to CREATE | 4 (`context_stack`, `scoring`, `deterministic_extraction`, `decision_log`) |
| Mutation sites to replace | ~35 (client_context + journey_context combined) |
| `set_state()` calls in orchestrator | ~40 |
