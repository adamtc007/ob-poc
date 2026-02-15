# OB-POC — Agent Loop Call-Stack Trace & Cleanup TODO (for Claude Code)

**Repo snapshot:** `ob-poc-main (7).zip` (extracted 2026-02-15)

Goal: **reduce call-stack debris** in the agent REPL loop by:
- documenting the *actual* utterance → intent → DSL → run-sheet → execution path
- enumerating **all intent→DSL pipelines currently in the codebase**
- deleting / feature-gating legacy paths, structs, and harness/tests that keep them “alive”

---

## 1) Current “single path” call stack (what’s actually running)

### 1.1 Chat (HTTP) — utterance → intent discovery → DSL staging → execution

**Entry:** `POST /api/session/:id/chat`

**Call chain (high-confidence):**

1. **HTTP handler**
   - `rust/src/api/agent_routes.rs` — chat route handler

2. **Conversation loop + orchestration**
   - `rust/src/api/agent_service.rs::process_chat()`
     - RUN keyword shortcut → `execute_runbook()`
     - pending verb disambiguation / decision flows
     - **main path:** builds `OrchestratorContext` → calls unified orchestrator

3. **Unified Orchestrator (the real “intent router”)**
   - `rust/src/agent/orchestrator.rs::handle_utterance()`
     - (optional) **entity linking** via `LookupService.analyze()`
     - **SemReg context resolution** → allowed verb set
     - **IntentPipeline discovery** → candidates + DSL
     - apply SemReg filtering (strict/fail-open)
     - emits `IntentTrace` + best-effort telemetry

4. **IntentPipeline (semantic match → arg extraction → DSL assembly)**
   - `rust/src/mcp/intent_pipeline.rs::process_with_scope()`
     - Stage 0: scope phrase hard-gate (`ScopeResolver`)
     - `HybridVerbSearcher.search()`
     - ambiguity policy (`check_ambiguity`)
     - **LLM arg extraction** (`extract_arguments()`)
     - deterministic DSL assembly + enrich + unresolved-ref extraction

5. **Run-sheet staging / execution**
   - `rust/src/session/unified.rs::UnifiedSession::set_pending_dsl()`
   - `rust/src/api/agent_service.rs::execute_runbook()`
     - parse DSL → enrich → resolve EntityRefs → `DslExecutor.execute_dsl()`

6. **Back to caller**
   - `AgentChatResponse` → `ChatResponse` mapping in `agent_routes.rs`

### 1.2 MCP — dsl_generate uses the same orchestrator

- `rust/src/mcp/handlers/core.rs::dsl_generate()`
  - builds `OrchestratorContext { source: Mcp }`
  - calls `agent/orchestrator.rs::handle_utterance()`

### 1.3 REPL v2 (pack-guided) — separate pipeline

- `rust/src/api/repl_routes_v2.rs` → `rust/src/repl/orchestrator_v2.rs`
  - uses **IntentService**: `rust/src/repl/intent_service.rs`

This is a **second, parallel** intent→dsl→runbook architecture.

---

## 2) Intent→DSL “pipelines” currently present (multiple paths)

| ID | Path | Status | Where |
|---:|------|--------|-------|
| A | **Orchestrator → IntentPipeline** (semantic search + LLM arg extraction) | **ACTIVE** | `agent/orchestrator.rs` → `mcp/intent_pipeline.rs` |
| B | **Legacy LLM writes DSL directly** (`/api/agent/generate*`) | **LEGACY / gated** | `api/agent_dsl_routes.rs::generate_dsl*` |
| C | **Legacy “LLM outputs VerbIntent → dsl_builder builds DSL”** | **DEAD / vestigial** | `api/intent.rs`, `api/dsl_builder.rs`, plus unused session fields |
| D | **REPL v2 pack-guided pipeline** (IntentService, deterministic sentences) | **PARALLEL** | `repl/*` + `api/repl_routes_v2.rs` |

Your “single path” aspiration is **A**. B/C/D are the main sources of call-stack debris.

---

## 3) Concrete debris / dead-path findings (actionable)

### 3.1 Chat handler still builds an LLM client that is unused

- `api/agent_routes.rs` creates `llm_client = crate::agentic::create_llm_client()` and logs provider/model.
- `AgentService::process_chat()` receives `_llm_client` but **never uses it**.

This is pure noise and (worse) implies the chat loop still depends on external LLM setup.

### 3.2 Feedback capture block is effectively dead

- In `api/agent_routes.rs`, feedback capture is guarded by:
  - `if !response.intents.is_empty()`
- In `api/agent_service.rs`, all responses set `intents: vec![]`.

So:
- `to_api_verb_intent()` and intent/validation conversion logic is dead
- session fields like `pending_interaction_id/pending_feedback_id` look legacy

### 3.3 MacroExpanded is (currently) unreachable under the orchestrator

`IntentPipeline` can return `PipelineOutcome::MacroExpanded`, but expansion requires `self.session`:
- `mcp/intent_pipeline.rs` has `session: Option<RwLock<UnifiedSession>>`
- there is **no setter** and orchestrator never injects it
- macro expansion falls through (“no session available”) to normal processing

Yet:
- `agent/orchestrator.rs` contains a large MacroExpanded SemReg governance block
- `api/agent_service.rs` contains MacroExpanded staging/auto-run logic
- harness/telemetry code includes MacroExpanded handling

This entire branch looks like **historic scaffolding** that no longer executes.

### 3.4 Legacy VerbIntent pipeline structs are mostly unused in runtime

- `api/intent.rs` + `api/dsl_builder.rs` exist (with unit tests)
- `session/unified.rs` has `pending_intents` and `add_intents()`
  - `pending_intents` is **only ever extended/cleared**, never read
  - corresponding tests in `api/session.rs` keep it alive

### 3.5 Scenario harness assertions are misaligned with what they test

- xtask harness (`rust/xtask/src/harness.rs`) runs orchestrator in **stub mode**
- stub searcher returns NoMatch for everything
- scenarios assert `run_sheet_delta` / `runnable_count`, but orchestrator does **not** mutate the session

So those suites currently validate “nothing staged” (always) rather than the agent loop.

---

## 4) Cleanup TODO (prioritized)

### P0 — Make the “single pipeline” obvious in code

1) **Remove unused LLM plumbing from chat HTTP path**
- Edit `rust/src/api/agent_routes.rs`:
  - remove `create_llm_client()` creation + logging
  - update comment “continue to LLM” → “continue to orchestrator pipeline”
- Edit `rust/src/api/agent_service.rs`:
  - remove `_llm_client: Arc<dyn LlmClient>` param from `process_chat()`
  - remove related imports/re-exports if unused

**Acceptance:** chat endpoint compiles with zero references to `create_llm_client()` in the request path.

2) **Delete dead feedback capture branch based on `response.intents`**
- `rust/src/api/agent_routes.rs`:
  - remove `if !response.intents.is_empty()` block entirely
  - (optional replacement) link feedback capture to orchestrator telemetry instead of legacy intents

**Acceptance:** `to_api_verb_intent()` no longer needed (remove it too).

---

### P1 — Remove MacroExpanded scaffolding (if you agree macro expansion is runtime-only now)

3) **Delete the MacroExpanded outcome (or fully re-enable it, but pick one)**

**Option A (recommended for cleanup): remove MacroExpanded**
- `rust/src/mcp/intent_pipeline.rs`:
  - remove `PipelineOutcome::MacroExpanded` variant
  - remove the macro expansion branch that returns it
  - remove `session` field from `IntentPipeline` (no setter, unused)
- `rust/src/agent/orchestrator.rs`:
  - remove the MacroExpanded SemReg governance block
  - remove `IntentTrace` fields that only exist for macro governance:
    - `macro_semreg_checked`, `macro_denied_verbs` (and related trace expectations)
- `rust/src/api/agent_service.rs`:
  - remove MacroExpanded handling (the runtime `combined_dsl` staging already covers macro workflows)
- `rust/src/agent/telemetry/mod.rs` and `src/agent/harness/assertions.rs`:
  - remove MacroExpanded label mapping
- `scenarios/suites/macro_expansion.yaml`:
  - either delete suite or rewrite it to validate **runtime macro** behavior (via execute_runbook), not pipeline expansion

**Acceptance:** no `MacroExpanded` symbol exists in codebase; macro workflows still work via runtime `combined_dsl` results.

**Option B: re-enable MacroExpanded properly (higher effort)**
- add a `with_session()` setter on `IntentPipeline`
- inject session into pipeline from orchestrator/agent_service
- keep governance block

Pick one; do not keep half-wired scaffolding.

---

### P2 — Remove legacy VerbIntent pipeline artifacts (C)

4) **Remove pending_intents and intent builder types (after verifying UI doesn’t need them)**
- `rust/src/session/unified.rs`:
  - remove `pending_intents` field + `add_intents()` + `clear_pending_intents()`
- `rust/src/api/session.rs`:
  - remove any pending_intents fields/methods/tests
- `rust/src/api/intent.rs` and `rust/src/api/dsl_builder.rs`:
  - delete modules entirely if nothing else references them
- `rust/src/api/mod.rs`:
  - remove re-exports of intent/builder types
- `rust/src/api/agent_routes.rs`:
  - remove conversion helpers (`to_api_verb_intent`) and any response fields that are always empty

**Acceptance:** chat responses no longer carry dead `intents`/`validation_results` fields (or they are removed from API types), and all old intent modules are gone.

---

### P3 — Remove / feature-gate legacy “LLM writes DSL” endpoints (B)

5) **Delete legacy /api/agent/generate* endpoints (recommended)**
- `rust/src/api/agent_dsl_routes.rs`:
  - remove `generate_dsl`, `generate_dsl_with_tools`, and helpers only used by them
- `rust/src/api/agent_routes.rs` router:
  - remove routes pointing at those handlers
- `rust/src/policy/*`:
  - remove `allow_legacy_generate` config + checks (or keep but unused)

**Acceptance:** only the orchestrator pipeline can produce DSL from NL; no endpoint exists where an LLM emits raw DSL.

If you *must* keep them, put them behind a feature flag like `legacy-llm-generate` and disable by default.

---

### P4 — Harness & tests: break the “deadly embrace”

6) **Fix scenario harness so assertions match reality**

Current harness runs orchestrator only (no session mutation). Choose one:

**Option A (quick):**
- remove `run_sheet_delta` + `runnable_count` assertions from:
  - `agent/harness/assertions.rs`
  - suites that assert them (e.g., `scenarios/suites/runsheet_lifecycle.yaml`)

**Option B (better): create an AgentService harness**
- add a new harness runner that drives:
  - `AgentService.process_chat()`
  - with a **test double** for argument extraction (no external LLM)
- move run-sheet assertions to the AgentService harness suites

Given today’s `IntentPipeline` always calls an LLM for arg extraction, Option B likely requires introducing an injectable `ArgExtractor` trait.

7) **Cull unit tests that only protect legacy structures**
- delete tests in:
  - `api/intent.rs`
  - `api/dsl_builder.rs`
  - `api/session.rs` that validate `pending_intents` retention

**Acceptance:** tests validate the single pipeline, not legacy intermediates.

---

## 5) Suggested “end state” invariants (useful for future PR reviews)

- **INV-PIPELINE-1:** All NL → DSL goes through `agent/orchestrator.rs::handle_utterance()`
- **INV-PIPELINE-2:** No HTTP endpoint accepts NL and returns LLM-authored DSL
- **INV-PIPELINE-3:** Direct DSL bypass requires `dsl:` prefix AND PolicyGate allow flag
- **INV-TRACE-1:** Every utterance produces an `IntentTrace` persisted best-effort
- **INV-HARNESS-1:** Harness assertions match the layer they test:
  - orchestrator harness asserts trace/outcome only
  - agent harness asserts run-sheet/session side effects

---

## 6) Quick “search checklist” for Claude Code while deleting

These strings should go to **zero occurrences** after cleanup (if you adopt the recommendations):

- `create_llm_client()` in the chat route path
- `response.intents.is_empty()` gating feedback capture
- `PipelineOutcome::MacroExpanded`
- `pending_intents` (both in `api/session.rs` and `session/unified.rs`)
- `/api/agent/generate` route wiring

