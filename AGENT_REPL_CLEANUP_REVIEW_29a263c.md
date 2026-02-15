# Agent REPL Cleanup Review — Patch `29a263c` (agent-loop-cleanup)

**Input:** `agent-loop-cleanup-29a263c.tar.gz` → `agent-loop-cleanup.patch`  
**Reviewed against:** your stated objective: *single executable truth = DSL runbook; delete call-stack debris; remove deadly-embrace harnesses.*

---

## 0) Executive summary

This patch is a **net-positive cleanup**: it removes an entire legacy “VerbIntent → DSL Builder” pipeline, eliminates `pending_intents` state, deletes the vestigial `MacroExpanded` outcome (and the SemReg governance branch that depended on it), and removes an unused LLM client initialization + a dead feedback-capture branch.

The one material “product” tradeoff is that **operator macro expansion is now *explicitly* unsupported** in the unified orchestrator path (it was previously present but effectively unreachable). If you still want macros as a deterministic preprocessor (your preferred model), they must be reintroduced intentionally as a compiler step (as per the Packs/Macros/Runbook paper), not by resurrecting the old `MacroExpanded` vestige.

---

## 1) What changed (high-signal)

### 1.1 Deleted legacy intent→DSL pipeline (big win)
**Removed:**
- `rust/src/api/intent.rs` (VerbIntent / ParamValue / IntentValidation etc)
- `rust/src/api/dsl_builder.rs` (deterministic statement/program builder)
- all API wiring that exposed these types (exports removed in `api/mod.rs`)
- session fields + message fields that carried these intents (`pending_intents`, message `intents` typed as `Vec<VerbIntent>`)

**Effect:** collapses “two ways” to represent intent into one: `mcp::intent_pipeline::PipelineResult`.

✅ Aligns with the invariant: *only DSL runbook matters at execution*.

---

### 1.2 Removed unused LLM client initialization in chat route (good)
In `rust/src/api/agent_routes.rs::chat_session()`:
- removed `create_llm_client()` creation + logging
- `AgentService::process_chat()` signature now no longer accepts an LLM client

✅ This eliminates a “double-init” (chat route previously created an LLM client that wasn’t actually used by the unified orchestrator path).

**Minor follow-up:** the doc comment in `agent_service.rs` still mentions `llm_client` in an example. Update it.

---

### 1.3 Deleted the dead “feedback capture” branch (good, but creates a new dead seam)
Removed the block that captured feedback only when `!response.intents.is_empty()`.
Given the service always returned `intents: []`, this capture was effectively dead.

✅ Removes noise and avoids misleading “learning loop” behavior.

⚠️ However: there is now **no replacement capture** using the *actual* signal (`IntentTrace` / chosen verb / candidates).  
So the downstream “record_outcome_with_dsl” logic remains present but is now starved of `pending_interaction_id`.

---

### 1.4 Removed `MacroExpanded` outcome and its SemReg governance branch (consistent)
Removed:
- `PipelineOutcome::MacroExpanded` variant (and labels in harness + telemetry)
- orchestrator branch that parsed expanded DSL (`parse_program`) to apply SemReg to expanded verbs
- the `scenarios/suites/macro_expansion.yaml` scenario suite

✅ Consistent with the reality that macro expansion was not actually reachable.
✅ Cleans out a high-complexity path that was “half there.”

⚠️ Tradeoff: if you reintroduce macro expansion later, you will need to restore a *proper* SemReg gating strategy for expanded verbs.

---

## 2) Contract / API compatibility notes

### 2.1 `DslState.intents` and `DslState.validation` are now always `None`
In `agent_routes.rs`, the `build_dsl_state()` function no longer builds intent or validation payloads and always returns:
- `validation: None`
- `intents: None`

This is consistent with the repo moving away from legacy `VerbIntent` typing, but it also means the UI loses:
- per-intent validation display
- “what verb did the agent extract” info

**Recommendation:** In a later patch, populate `DslState.intents` from `PipelineResult.intent` (StructuredIntent) and candidates, if you still want that UI affordance.

---

## 3) Remaining debris / new “dead seam” created by the cleanup

### 3.1 Learning loop outcome writers now have no producer
`agent_routes.rs::execute_session()` still does:
- read `pending_interaction_id` / `pending_feedback_id` from session context
- call `feedback_service.record_outcome*()` if present

But nothing sets `pending_interaction_id` anymore.

**Options:**
1) **Remove** the entire `pending_interaction_id`/`pending_feedback_id` path (cleanest if you’re not using learning loop yet).
2) **Rewire** capture to the real pipeline: capture match right after orchestration using:
   - `IntentTrace.chosen_verb` or `PipelineResult.verb_candidates[0]`
   - match method: `DirectDsl` vs `Semantic` (already computed previously)
   - alternatives: the remaining candidates

Given your long-term plan, (2) is worth doing: it turns the new single-pipeline into the data source for training/feedback without reintroducing legacy intent structs.

---

## 4) Macro / Pack semantics after this cleanup

**Important:** This patch does **not** imply “macro == broken” so much as:
- the old macro branch was a vestigial partial implementation, and now it is removed.

If you want operator macros as “bulk composite ops expanded into atomic DSL” (your stated philosophy), implement it as:

- **macro invocation → compile-step expansion → runbook DSL**  
not as a special pipeline outcome.

This is exactly what the Packs/Macros/Runbook paper recommends: keep Packs (scoping) separate from Macros (preprocessor).

---

## 5) Suggested next cleanup TODO (small, high impact)

### P0 — Close the learning-loop seam (choose one)
- If learning loop is not needed: delete `pending_interaction_id/pending_feedback_id` plumbing and the conditional `feedback_service.record_outcome*()` blocks.
- If learning loop is needed: add a *single* capture point in the unified pipeline that stores `pending_interaction_id` after intent selection, using `IntentTrace`/candidates.

### P1 — Remove stale docs / comments
- Update `agent_service.rs` example that still mentions `llm_client`.

### P2 — Add one integration harness test (non-LLM)
- A “slash command / run” path test that proves:
  - chat handler → service → runbook staging is still correct
  - no legacy intent structs are involved

---

## 6) Overall verdict

✅ **Pass**: This patch materially reduces call-path clutter and deletes meaningful dead code.  
⚠️ **Caveat**: It also makes the “operator macro expansion” gap explicit (which is good), but it means reintroducing macros must be done the “right way” (compile-step expansion), not by resurrecting `MacroExpanded`.

