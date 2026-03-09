# Sage/Coder Pipeline Mechanics Remediation Plan

## Scope
This plan targets only pipeline mechanics: short-circuits, fallback paths, control-flow leaks, and state-machine interference between Sage, Coder, and legacy intent routing.

## Current Mechanical Failure Pattern
Observed from code paths in `rust/src/agent/orchestrator.rs`, `rust/src/api/agent_service.rs`, and harness artifacts:

- `handle_utterance()` is Sage-primary in name, but still executes `legacy_handle_utterance()` first for both Serve and Delegate paths, then conditionally overrides.
- Serve override is narrow (`coder_complete && !pipeline.valid` or specific fast-path checks), so legacy ambiguity/disambiguation can dominate even when Sage intent is high quality.
- Delegate path still depends on legacy outcome for fallback mutation confirmation (`coder_result_from_pipeline_result(&outcome)`), coupling write UX to legacy resolution quality.
- Chat state machine has multiple early handlers (`pending_mutation`, `pending_decision`, `pending_intent_tier`, `pending_verb_disambiguation`) that can preempt Sage/Coder intent handling.
- Safe read fallback is currently implemented in chat (`AgentService`) rather than as a first-class orchestrator decision policy.

## Root Causes (Mechanics Only)
1. Legacy-first execution ordering
- Sage/Coder should decide first, but legacy pipeline is still authoritative in many branches.

2. Mixed policy layers across Orchestrator and Chat service
- Read safety and ambiguity policy is split between `orchestrator.rs` and `agent_service.rs`.

3. Fallback chain is too permissive and non-deterministic
- Multiple branches can return `NeedsClarification`/`NeedsUserInput` before Sage/Coder policies are fully applied.

4. Forced-verb and normal utterance paths diverge
- `handle_utterance_with_forced_verb()` semantics are not mechanically aligned with Serve policy, creating different behavior for the same intent.

5. Pending state precedence can eclipse intent routing
- Pending decision/disambiguation handling can consume turns before the latest utterance is classified by Sage.

## Remediation Strategy

### Phase M1: Make Orchestrator the Single Policy Authority
Goal: remove chat-layer policy fallbacks from critical routing decisions.

Changes:
- Move safe read ambiguity resolution ("if in doubt -> list/read") from `AgentService` into orchestrator Serve policy.
- Add a deterministic `ServePolicyDecision` output (e.g., `AutoExecute`, `Stage`, `Clarify`) in orchestrator.
- Restrict `AgentService` to rendering/orchestration state transitions only.

Files:
- `rust/src/agent/orchestrator.rs`
- `rust/src/api/agent_service.rs`

Gate:
- No `build_verb_disambiguation_response(...)` call for inventory-style read utterances when top candidates are read-safe variants of same domain intent.

### Phase M2: Eliminate Legacy-First Execution in Serve Path
Goal: Sage/Coder resolution must run before legacy pipeline for Serve dispositions.

Changes:
- In `handle_utterance()`, replace `legacy_handle_utterance()`-first Serve logic with:
  - Sage classify -> Coder resolve -> Serve policy outcome
  - Legacy pipeline only as explicit fallback branch when Coder fails AND policy allows fallback.
- Emit trace fields that indicate explicit fallback reason (`serve_fallback_reason`) whenever legacy is used.

Files:
- `rust/src/agent/orchestrator.rs`

Gate:
- For read utterances with Coder `Confident|Proposed` and no missing args, legacy path must not run.

### Phase M3: De-couple Delegate from Legacy Outcome
Goal: write confirmation mechanics must depend on Coder-only contract, not legacy-generated DSL.

Changes:
- Remove `coder_result_from_pipeline_result(&outcome)` fallback from Delegate path.
- Require Coder to produce explicit mutation candidate and unresolved requirements.
- If Coder cannot produce candidate, return deterministic `NeedsInput` with structured missing slots (no legacy mutation fallback).

Files:
- `rust/src/agent/orchestrator.rs`
- `rust/src/sage/coder.rs`

Gate:
- Delegate path confirmation text and pending mutation can only be sourced from `CoderResult`.

### Phase M4: Normalize Pending State Precedence
Goal: ensure new utterance intent routing is not accidentally shadowed by stale pending state.

Changes:
- Introduce a strict pending-state precedence contract:
  - confirmation tokens -> pending mutation
  - explicit decision reply payload -> pending decision
  - otherwise reclassify utterance first, then decide whether pending state should persist.
- Add a helper that validates pending packet freshness against current utterance intent class.

Files:
- `rust/src/api/agent_service.rs`
- `rust/src/session/unified.rs`

Gate:
- New read utterance while mutation pending must always produce explicit cancel + read execution path.

### Phase M5: Align Forced-Verb and Normal Serve Semantics
Goal: same intent should produce same policy behavior regardless of forced-verb path.

Changes:
- Refactor `handle_utterance_with_forced_verb()` to reuse the same Serve policy evaluator used by normal `handle_utterance()`.
- Ensure auto-execute/stage decision for read verbs is shared logic.

Files:
- `rust/src/agent/orchestrator.rs`
- `rust/src/api/agent_service.rs`

Gate:
- For the same utterance and selected verb, forced and non-forced paths must emit identical `PipelineOutcome` class and `auto_execute`.

## Test Plan

### Unit
- `orchestrator_serve_does_not_call_legacy_when_coder_resolves_read`
- `orchestrator_delegate_requires_coder_result_for_pending_mutation`
- `agent_service_pending_state_reclassifies_non_confirmation_utterance`
- `forced_verb_and_normal_serve_policy_are_equivalent`

### Integration (chat)
- read inventory utterance does not surface verb disambiguation when candidates are read-safe alternatives.
- write -> pending mutation -> read pivot cancels pending and executes read path.
- explicit confirmation token executes pending mutation; non-confirmation does not.

### Harness Gates
Re-run:
- `cargo test -p ob-poc --test utterance_api_coverage -- --ignored --nocapture`
- targeted smoke flow for:
  - `what deals does Allianz have?`
  - write confirmation lifecycle
  - semos data-management structure reads

Acceptance metrics (mechanics-focused):
- `Both wrong` decreases via reduced ambiguity loops.
- Read utterances no longer bounce through clarify/search-vs-list when both are safe-read variants.
- No pending-state loop regressions in smoke scripts.

## Implementation Order
1. M1 (policy authority)
2. M2 (Serve ordering)
3. M3 (Delegate de-coupling)
4. M4 (pending-state precedence)
5. M5 (forced-path parity)

## Non-Goals
- Vocabulary rationalization changes
- LLM prompt/model tuning
- Registry/domain taxonomy redesign

