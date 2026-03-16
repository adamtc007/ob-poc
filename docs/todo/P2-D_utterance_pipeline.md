# P2-D: Utterance-to-DSL Pipeline Architecture Review

**Review Session:** P2-D
**Date:** 2026-03-16
**Reviewer:** Claude Opus 4.6
**Scope:** `HybridVerbSearcher` 10-tier verb discovery, `VerbSearchOutcome` disambiguation, `SemTaxonomy v2` three-step compiler (`EntityScope -> EntityState -> SelectedVerb`), CBU compiler slice, bridge layer, `ContextEnvelope` / `SessionVerbSurface` governance, Sage/Coder confirmation boundary.

---

## 1. Executive Summary

The utterance pipeline has two parallel paths: the production `HybridVerbSearcher` (10-tier, embedding-backed) handling all chat/MCP traffic, and the `SemTaxonomy v2` compiler (three-step, deterministic) handling only a narrow CBU slice (0.87% of registry verbs). The `HybridVerbSearcher` is well-built with clean tier ordering, correct ambiguity detection, and sound governance pre-constraints. The SemTaxonomy v2 compiler is architecturally sound but its coverage is extremely narrow. The confirmation boundary for write mutations uses plain-text phrase matching against a hardcoded list rather than the existing cryptographic `ConfirmToken` infrastructure, which is a correctness gap. The 7.95% three-step exact-hit rate is bounded by empty verb contract metadata, not pipeline logic.

**Overall Verdict:** MINOR (no critical defects, several actionable improvements)

---

## 2. Focus Area Verdicts

### Focus Area 1: Entity Scope Identification
**Verdict: PASS (MINOR findings)**

The entity scope stage uses two separate resolution paths:

**Deterministic path (CBU slice):** `supports_cbu_compiler_slice()` gates on `entity == "cbu"` and `action in {create, read, update}`. Within this gate, `CbuSurfaceResolver` maps the entity name to a canonical surface object. `BindingMode` is derived from context: UUID present -> `Identifier`, session entity -> `SessionReference`, neither -> `Unbound`.

**Sage-backed path (everything else):** Entity resolution delegates to the Sage LLM extraction, then the bridge layer (`compiler_input_from_outcome_intent()`) converts `OutcomeIntent` into `CompilerInputEnvelope`. The bridge infers `BindingMode` heuristically: if the subject has a UUID field -> `Identifier`, if session_entity_id is present -> `SessionReference`, else `Unbound`.

**Findings:**

| ID | Severity | Finding |
|----|----------|---------|
| ES-1 | MINOR | Bridge `BindingMode` heuristic cannot distinguish between a genuine UUID identifier and a UUID that happens to appear as a subject string in Sage output. This produces incorrect `BindingMode::Identifier` when the UUID is actually session-derived. Mitigated by the fact that most non-CBU paths still fall through to legacy `IntentPipeline` which does not consume `BindingMode`. |
| ES-2 | MINOR | Entity scope gate (`supports_cbu_compiler_slice`) does not include `"delete"` or `"list"` actions, even though several CBU verb families map to these. Coverage is unnecessarily narrow. |
| ES-3 | CLEAN | `EntitySource` enum (`SessionCarry`, `SearchHit`, `UserConfirmed`) correctly tracks provenance. |
| ES-4 | CLEAN | `EntityScopeOutcome` enum (`Resolved`, `Ambiguous`, `Unresolved`) handles all resolution states without silent failure. |

---

### Focus Area 2: Graph Walk Pruning (Verb Candidate Narrowing)
**Verdict: PASS (MINOR findings)**

Two pruning mechanisms operate in sequence:

**a) SessionVerbSurface (8-step governance filter):** Reduces the ~642 base verb registry through AgentMode, workflow phase, SemReg CCIR, lifecycle state, and actor gating. The result is a pre-constrained `HashSet<String>` of allowed verb FQNs threaded into the `IntentPipeline`.

**b) HybridVerbSearcher (10-tier priority search):** Searches within the pre-constrained allowed set. Tiers -2A (ScenarioIndex) and -2B (MacroIndex) handle compound/journey utterances; Tier -1 (ECIR NounIndex) provides deterministic noun-to-verb mapping; Tiers 0-7 provide exact match, semantic search, and phonetic fallback.

**Tier Ordering:**

| Tier | Name | Score | Short-Circuit |
|------|------|-------|---------------|
| -2A | ScenarioIndex | 0.97 | Compound signals + scenario match |
| -2B | MacroIndex | 0.96 | Single-macro utterance match |
| -1 | ECIR NounIndex | 0.95 | 1 candidate -> short-circuit; 2+ -> post-boost |
| 0 | Operator macros | 1.0/0.95 | Exact match |
| 1 | User learned exact | 1.0 | Exact match |
| 2 | Global learned exact | 1.0 | Exact match |
| 3 | User semantic (pgvector) | 0.5-0.95 | None |
| 5 | Blocklist filter | -- | Negative gate |
| 6 | Global semantic | 0.5-0.95 | None |
| 7 | Phonetic fallback | 0.80 | None |

**Findings:**

| ID | Severity | Finding |
|----|----------|---------|
| GW-1 | CLEAN | CompoundSignals gate correctly suppresses ECIR single-candidate short-circuit when Tier -2A/-2B signals are present, preventing journey-level scenarios from being intercepted by a single-noun match. |
| GW-2 | CLEAN | `normalize_candidates()` deduplicates by verb keeping highest score, sorts descending, truncates. Correct behavior. |
| GW-3 | MINOR | ECIR post-boost (+0.05) applies to embedding results (Tiers 3/6) but not to Tier 0 macro results. When both ECIR and macro paths match, macro score is unaffected by the ECIR signal. Low impact since macro scores are already 0.95-1.0. |
| GW-4 | CLEAN | SessionVerbSurface FailClosed safe-harbor correctly reduces to ~30 always-safe verbs (session.*, view.*, agent.*) when SemReg is unavailable. No ungoverned expansion possible. |
| GW-5 | CLEAN | TOCTOU recheck (`toctou_recheck()`) compares fingerprints between original envelope and fresh resolution. Three outcomes: `StillAllowed`, `AllowedButDrifted`, `Denied`. |

---

### Focus Area 3: LLM Select Prompt Robustness
**Verdict: PASS (MINOR findings)**

The LLM is used in two places within the pipeline:

**a) Sage extraction:** Converts raw utterance into `OutcomeIntent` with polarity (Read/Write/Ambiguous), action, domain_concept, subject, steps, and confidence. This is the only LLM call in the Sage-primary path.

**b) Coder resolution:** Given the Sage `OutcomeIntent`, resolves specific verb FQN and arguments. Output is `CoderResult { verb_fqn, dsl, missing_args, unresolved_refs }`.

The deterministic CBU compiler slice bypasses both LLM calls entirely.

**Findings:**

| ID | Severity | Finding |
|----|----------|---------|
| LP-1 | CLEAN | Sage confidence enum (`High`, `Medium`, `Low`) is used as a hard gate: `Low` confidence + pending clarifications -> "I need a clearer instruction" (no execution). |
| LP-2 | MINOR | Coder completeness check (`missing_args.is_empty() && unresolved_refs.is_empty()`) is a necessary condition for write-path confirmation. However, the check does not validate that the resolved verb FQN is in the SessionVerbSurface allowed set before building the PendingMutation. The SemReg gate is applied earlier at the surface level, so this is not exploitable, but the delegate path does not re-verify against the envelope after Coder resolves a potentially different verb. |
| LP-3 | CLEAN | `read_only_list_fallback()` correctly handles the case where Sage produces a Read intent but Coder fails to resolve, by falling back to a simple list operation. |
| LP-4 | CLEAN | The `can_use_coder_for_serve()` guard prevents auto-execution of Coder results that are incomplete or blocked by surface policy. |

---

### Focus Area 4: Fallback Chain Degradation
**Verdict: PASS (CLEAN)**

The fallback chain is structured as a clean waterfall:

```
handle_utterance()
  |
  +-> Sage stage (run_sage_stage)
  |     |
  |     +-> Sage fails entirely -> legacy_handle_utterance()
  |
  +-> Sage succeeds -> route(intent)
        |
        +-> Serve path:
        |     +-> Coder resolves -> can_use_coder_for_serve() -> auto-execute
        |     +-> Coder fails/incomplete -> legacy_handle_utterance()
        |
        +-> Delegate path:
              +-> Low confidence -> "need clearer instruction" (no exec)
              +-> Coder complete -> PendingMutation (confirmation required)
              +-> Coder incomplete -> "need a few details"
              +-> Coder fails -> error message
```

**Findings:**

| ID | Severity | Finding |
|----|----------|---------|
| FC-1 | CLEAN | Sage failure is a hard gate: `sage_stage.intent == None` immediately falls back to `legacy_handle_utterance()`. No partial Sage output is consumed. |
| FC-2 | CLEAN | The legacy pipeline (`legacy_handle_utterance`) is a complete, independent pipeline: entity linking -> SemReg -> SessionVerbSurface -> IntentPipeline -> HybridVerbSearcher -> post-filter. It does not share mutable state with the Sage path. |
| FC-3 | CLEAN | Serve path fallback to legacy pipeline correctly preserves `serve_fallback_reason` in the IntentTrace for observability. |
| FC-4 | CLEAN | NLCI compiler shadow compilation (`compiler.compile(compiler_input)`) for non-CBU utterances is wrapped in `if let Err(error)` with a `warn!` log. Shadow failures do not affect the main pipeline. |

---

### Focus Area 5: Sage/Coder Confirmation Boundary Enforcement
**Verdict: PASS (FLAG findings)**

The Sage/Coder boundary enforces an asymmetric confirmation protocol:

- **Read (Serve):** Sage polarity `Read` or `Ambiguous` -> auto-execute if Coder resolves successfully
- **Write (Delegate):** Sage polarity `Write` -> requires explicit user confirmation via `PendingMutation`

The confirmation mechanism uses `is_confirmation()` -- a hardcoded list of 14 plain-text phrases matched against session-stored `PendingMutation`, rather than the cryptographic `ConfirmToken` infrastructure in `rust/src/clarify/confirm.rs`.

**Findings:**

| ID | Severity | Finding |
|----|----------|---------|
| CB-1 | FLAG | **Confirmation uses plain-text phrase matching, not cryptographic tokens.** The `is_confirmation()` function matches against 14 hardcoded phrases ("yes", "y", "go ahead", "do it", etc.). The `ConfirmToken` infrastructure (base64-encoded random bytes + timestamp, 30-second TTL, `validate_confirm_token()`) exists in `rust/src/clarify/confirm.rs` but is not wired into the Sage/Coder confirmation path. The session-stored `PendingMutation` is cleared on any non-matching input (line 2599), so a stale "yes" after mutation cancellation correctly returns "There is no pending change to confirm." However, the plain-text approach means: (a) there is no replay protection beyond session state, (b) a user saying "yes" to an unrelated question while a mutation is pending would trigger execution, and (c) there is no TTL on the pending mutation. |
| CB-2 | MINOR | `is_confirmation()` does not include "run" as a confirmation phrase, but the RUN command check (line 2625) matches "run", "execute", "do it", "go", "run it", "execute it" and calls `execute_runbook()` directly. "do it" appears in BOTH the confirmation list AND the RUN command list, but the confirmation check (line 2572) runs before the RUN check (line 2625), so "do it" with a pending mutation triggers mutation confirmation, not runbook execution. This is correct but fragile -- the ordering dependency is implicit rather than documented. |
| CB-3 | CLEAN | Read-pivot cancellation is correct: `is_read_only_pivot_request()` detects when a user switches from write-intent to read-intent, cancels the pending mutation, and emits an explicit cancellation notice ("Cancelled the pending change and switched back to read-only mode."). |
| CB-4 | CLEAN | The delegate path correctly handles all four Coder outcomes: low confidence (reject), complete (confirm), incomplete (request details), failure (error message). No silent execution of incomplete mutations. |
| CB-5 | CLEAN | `Ambiguous` polarity routes to `Serve` (not `Delegate`), which is the safe default -- ambiguous intent is treated as a read operation, never auto-mutated. |

---

## 3. Failure Mode Catalogue

| ID | Failure Mode | Trigger Condition | Severity | Mitigation |
|----|-------------|-------------------|----------|------------|
| FM-1 | Write mutation executes without explicit confirmation | User says "yes" while PendingMutation exists but intended to answer a different question | FLAG | Wire `ConfirmToken` with 30-second TTL into PendingMutation flow; include token in confirmation prompt |
| FM-2 | Stale PendingMutation with no TTL | User receives mutation confirmation, walks away, returns hours later and says "yes" | FLAG | Add timestamp to `PendingMutation` and enforce TTL check in `is_confirmation()` path |
| FM-3 | Verb FQN not re-verified after Coder resolution in delegate path | Coder resolves a verb that was allowed at surface-compute time but was pruned by a concurrent SemReg update | MINOR | TOCTOU recheck exists for the serve path but the delegate path builds PendingMutation without re-verifying Coder's verb against the envelope |
| FM-4 | CBU compiler slice misses valid CBU verbs | User says "delete this cbu" -- action "delete" not in gate condition | MINOR | Extend `supports_cbu_compiler_slice()` to include "delete" and "list" |
| FM-5 | Bridge BindingMode misclassifies UUID subjects | Sage produces a subject with UUID field that is session-derived, bridge classifies as `Identifier` instead of `SessionReference` | MINOR | Add entity-kind-aware override to binding heuristic |
| FM-6 | Three-step pipeline returns wrong verb due to empty metadata | All 1,004 verb contracts have empty preconditions/postconditions/writes_to/reads_from, so Step 2 filtering is a no-op | MINOR | Extend adapter scanner to populate verb contract metadata from YAML |
| FM-7 | Legacy qualifier names opaque to v2 compiler | "legacy-summary" / "legacy-notes" qualifiers cannot be interpreted by v2 phases | MINOR | Define structured qualifier schema to replace legacy naming |

---

## 4. Stage-by-Stage Review

### Stage A: Pre-Processing (Entity Linking + SemReg Resolution)
**Verdict: CLEAN**

`prepare_turn_context()` runs entity linking (LookupService), SemReg context resolution (ContextEnvelope), and SessionVerbSurface computation. All three are independent and composable. The envelope's `AllowedVerbSetFingerprint` (SHA-256, format `v1:<hex>`) is distinct from the surface's `SurfaceFingerprint` (format `vs1:<hex>`). No cross-contamination.

### Stage B: Sage Extraction
**Verdict: CLEAN**

`run_sage_stage()` produces `OutcomeIntent` with discrete polarity, confidence, action, and domain. Failure -> `None` -> immediate fallback to legacy pipeline. No partial state leaks.

### Stage C: Route Decision (Read vs Write)
**Verdict: CLEAN**

Binary `route()` function: `Read | Ambiguous` -> Serve, `Write` -> Delegate. No third path. No ambiguity in the routing itself.

### Stage D: Serve Path (Auto-Execute)
**Verdict: CLEAN**

Coder resolution -> `can_use_coder_for_serve()` guard -> execution or fallback. The guard checks completeness, surface policy, and scope. Fallback preserves `serve_fallback_reason` in trace.

### Stage E: Delegate Path (Confirmation Required)
**Verdict: FLAG**

Three findings: (1) CB-1: plain-text confirmation without crypto tokens, (2) CB-2: "do it" overlap between confirmation and RUN command lists, (3) LP-2: Coder verb FQN not re-verified against envelope. See Focus Area 5 for details.

### Stage F: Legacy Fallback Pipeline
**Verdict: CLEAN**

Full independent pipeline: entity linking -> SemReg -> SessionVerbSurface -> IntentPipeline -> HybridVerbSearcher -> post-filter -> TOCTOU recheck. Well-instrumented with IntentTrace. No shared mutable state with Sage path.

### Stage G: CBU Compiler Slice
**Verdict: MINOR**

Architecturally sound 6-phase deterministic pipeline. 14 test cases cover all supported verb families. Finding ES-2 (gate condition too narrow) is the only issue. Coverage is 0.87% of registry -- by design, but the gate could be wider.

### Stage H: Bridge Layer (Sage -> NLCI Input)
**Verdict: MINOR**

`compiler_input_from_outcome_intent()` correctly converts OutcomeIntent to CompilerInputEnvelope. Findings ES-1 (BindingMode heuristic) and FM-7 (opaque legacy qualifiers) apply.

---

## 5. Summary of Findings by Severity

| Severity | Count | IDs |
|----------|-------|-----|
| CRITICAL | 0 | -- |
| FLAG | 2 | CB-1, FM-2 |
| MINOR | 8 | ES-1, ES-2, GW-3, LP-2, CB-2, FM-3, FM-4, FM-6 |
| CLEAN | 14 | ES-3, ES-4, GW-1, GW-2, GW-4, GW-5, LP-1, LP-3, LP-4, FC-1, FC-2, FC-3, FC-4, CB-3, CB-4, CB-5 |

**Highest-priority recommendation:** Wire the existing `ConfirmToken` (30-second TTL, crypto-random, base64) into the PendingMutation confirmation flow to replace the plain-text `is_confirmation()` matching. This closes FM-1 and FM-2 with infrastructure that already exists.

---

## 6. Recommendations (Priority-Ordered)

1. **Wire ConfirmToken into PendingMutation** (closes CB-1, FM-1, FM-2). The `ConfirmToken` infrastructure exists at `rust/src/clarify/confirm.rs`. Add token generation when building `PendingMutation`, include the token in the confirmation prompt, and validate it in the confirmation handler. This adds TTL enforcement and replay protection with minimal code.

2. **Re-verify Coder verb FQN against envelope in delegate path** (closes LP-2, FM-3). After `run_coder_stage()` resolves the verb, check `envelope.is_allowed(&coder_result.verb_fqn)` before building `PendingMutation`. This closes the TOCTOU gap specific to the delegate path.

3. **Extend CBU compiler slice gate** to include `"delete"` and `"list"` actions (closes ES-2, FM-4). Several CBU verb families already map to these actions. This expands deterministic coverage without a new domain implementation.

4. **Populate verb contract metadata via scanner extension** (closes FM-6). Extract `lifecycle.preconditions` and `metadata.subject_kinds` from verb YAML into registry `VerbContractBody`. This unblocks Step 2 EntityState filtering.

5. **Implement domain-specific compiler slices** for top-5 non-CBU domains (kyc, entity, trading-profile, deal, ubo) following the CBU compiler pattern. Each replaces one Sage extraction call with a deterministic resolver.

6. **Define structured qualifier schema** (closes FM-7). Replace "legacy-summary" / "legacy-notes" with typed qualifier pairs that v2 phases can interpret.

7. **Document the "do it" ordering dependency** (closes CB-2). The confirmation check must run before the RUN command check for correct behavior. Add a comment and/or test asserting this invariant.

---

## Appendix A: Key File Locations

```
rust/src/agent/orchestrator.rs          -- handle_utterance(), route(), is_confirmation(), legacy_handle_utterance()
rust/src/agent/context_envelope.rs      -- ContextEnvelope, PruneReason, AllowedVerbSetFingerprint
rust/src/agent/verb_surface.rs          -- SessionVerbSurface, compute_session_verb_surface()
rust/src/api/agent_service.rs           -- process_chat(), confirmation handling, PendingMutation wiring
rust/src/clarify/confirm.rs             -- ConfirmToken generation/validation (unused by Sage path)
rust/src/semtaxonomy_v2/mod.rs          -- Pipeline orchestrator, EntityScope, EntityState, SelectedVerb
rust/src/semtaxonomy_v2/cbu_compiler.rs -- CBU-only deterministic compiler slice
rust/src/semtaxonomy_v2/bridge.rs       -- Sage OutcomeIntent -> CompilerInputEnvelope conversion
rust/src/semtaxonomy_v2/compiler.rs     -- 6-phase IntentCompiler pipeline
rust/src/mcp/verb_search.rs             -- HybridVerbSearcher, VerbSearchOutcome, ambiguity detection
rust/src/mcp/noun_index.rs              -- NounIndex + VerbContractIndex (Tier -1 ECIR)
rust/src/mcp/scenario_index.rs          -- ScenarioIndex (Tier -2A)
rust/src/mcp/macro_index.rs             -- MacroIndex (Tier -2B)
rust/src/mcp/intent_pipeline.rs         -- IntentPipeline with allowed_verbs pre-constraint
rust/src/sem_reg/reducer/mod.rs         -- State reducer for constellation slot state
```

## Appendix B: Threshold Reference

| Constant | Value | Location | Purpose |
|----------|-------|----------|---------|
| `DEFAULT_FALLBACK_THRESHOLD` | 0.55 | verb_search.rs | Retrieval cutoff for DB queries |
| `DEFAULT_SEMANTIC_THRESHOLD` | 0.65 | verb_search.rs | Decision gate for accepting match |
| `BLOCKLIST_THRESHOLD` | 0.80 | verb_search.rs | Collision detection |
| `AMBIGUITY_MARGIN` | 0.05 | verb_search.rs | Min gap between top two candidates |
| `DEFAULT_TOKEN_TTL_SECS` | 30 | confirm.rs | Confirm token validity window |
| `RANDOM_BYTES_LEN` | 16 | confirm.rs | Crypto token random component size |
