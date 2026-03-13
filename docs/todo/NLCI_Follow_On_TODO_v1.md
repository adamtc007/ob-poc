# NLCI Follow-On TODO

> **Date:** 2026-03-12
> **Status:** Pause-point handoff
> **Purpose:** Capture the exact follow-on execution plan for the next Codex session without redesigning the NLCI architecture

---

## 1. Current Pause Point

This is a good pause point.

The following CBU slices are now compiler-backed through `rust/src/semtaxonomy_v2/cbu_compiler.rs` and routed through orchestrator preference:

- `cbu.create`
- `cbu.read`
- `cbu.rename`
- `cbu.set-jurisdiction`
- `cbu.set-client-type`
- `cbu.set-commercial-client`
- `cbu.set-category`
- `cbu.submit-for-validation`
- `cbu.request-proof-update`
- `cbu.reopen-validation`

The following cleanup has already been done:

- narrow CBU property verbs removed from old phrase/discovery fixture surfaces
- narrow CBU lifecycle verbs removed from old phrase/discovery fixture surfaces
- narrow `cbu.create` removed from the main legacy test/discovery fixture surfaces
- generic `sage`, `orchestrator`, and `mcp` unit tests that used `cbu.*` as generic examples were converted to neutral `deal.*` cases where appropriate
- bridge now preserves legacy lifecycle signal as typed qualifiers:
  - `legacy-summary`
  - `legacy-notes`

Verification state at pause:

- `cargo check` passes

---

## 2. What Must Not Be Reopened

Do **not** reopen these already-decided implementation rules:

- keep the NLCI architecture fixed
- favor rip-and-replace internally over compatibility layering
- keep compatibility only at the API/orchestrator boundary where needed
- delete superseded test/discovery surfaces once the compiler-backed path is proven
- do not redesign the architecture unless a gate review exposes a structural defect

---

## 3. Active Canonical Path

The current canonical implementation path is:

1. Layer 1 structured extraction
2. canonical `StructuredIntentPlan`
3. canonical `SemanticIr`
4. `semtaxonomy_v2` compiler phases
5. orchestrator compiler preference for supported CBU slice
6. existing runtime/confirmation shell

Primary active files:

- `rust/src/semtaxonomy_v2/intent_schema.rs`
- `rust/src/semtaxonomy_v2/semantic_ir.rs`
- `rust/src/semtaxonomy_v2/binding.rs`
- `rust/src/semtaxonomy_v2/failure.rs`
- `rust/src/semtaxonomy_v2/compiler.rs`
- `rust/src/semtaxonomy_v2/bridge.rs`
- `rust/src/semtaxonomy_v2/extraction.rs`
- `rust/src/semtaxonomy_v2/cbu_compiler.rs`
- `rust/src/agent/orchestrator.rs`
- `rust/src/mcp/intent_pipeline.rs`

---

## 4. Immediate Next Work

### P0. Review remaining active lexical/runtime guidance for `cbu.create`

Goal:

- decide whether active create guidance should still point through lexical search surfaces, or whether it should now be deliberately aligned to the compiler-backed route

Review first:

- `rust/src/mcp/noun_index.rs`
- `rust/src/mcp/tools.rs`

Decision required:

- keep these as intentional lexical guidance
- or scrub/retune them to stop teaching `cbu.create` as a primary old-path route

Rule:

- do not edit these blindly during a test scrub
- treat them as active runtime guidance, not stale fixtures

### P1. Expand `cbu.create` carefully or stop

If create expansion is desired, do it incrementally:

1. add one more create arg shape at a time to `cbu_compiler.rs`
2. smoke-test in `orchestrator.rs`
3. add/adjust harness scenario
4. delete overlapping legacy expectation immediately after

Likely next create args:

- `commercial-client-entity-id`
- `fund-entity-id`
- `manco-entity-id`

Do **not** jump straight to full create coverage in one pass.

### P2. Decide whether `cbu.list` should stay legacy or move next

Recommendation:

- only move `cbu.list` after the team confirms the create cutover boundary is clean enough

Reason:

- `cbu.list` is lower risk
- create/mutation path cleanup yields more architectural value first

If moved:

- treat it as a separate read-family cutover
- do not mix it with broader create expansion in the same patch set

---

## 5. Legacy Surfaces Still Worth Reviewing

These may still contain active old-route guidance or duplicate expectations:

### Review for active lexical guidance

- `rust/src/mcp/noun_index.rs`
- `rust/src/mcp/tools.rs`
- `rust/tests/verb_search_scenarios.txt`

### Review for create-path search assumptions

- `rust/src/mcp/verb_search.rs`
- `rust/src/mcp/verb_search_intent_matcher.rs`
- `rust/tests/verb_search_integration.rs`

### Review for remaining CBU-specific test bias

- `rust/src/sage/coder.rs`
- `rust/src/sage/verb_resolve.rs`
- `rust/src/agent/orchestrator.rs`

Only remove coverage that is now false because the compiler path owns that intent family.

---

## 6. Do Not Delete These

These are still valid and should remain:

- actual DSL/runtime tests for `cbu.create`
- runbook tests using `cbu.create` as a real DSL verb
- semantic validator / DB integration tests using `cbu.create`
- harness scenarios for the compiler-backed CBU slice
- CBU read/list tests that still reflect genuinely legacy-owned behavior

---

## 7. Suggested Next Session Order

Resume in this order:

1. inspect `rust/src/mcp/noun_index.rs`
2. inspect `rust/src/mcp/tools.rs`
3. decide whether active lexical guidance for `cbu.create` should be scrubbed or retained
4. if retained:
   - stop cleanup
   - begin narrow create-shape expansion in `cbu_compiler.rs`
5. if scrubbed:
   - update runtime guidance
   - run `cargo check`
   - then begin narrow create-shape expansion
6. only after create is reviewed, decide whether to move `cbu.list`

---

## 8. Resume Prompt

Use this in the next session:

> Continue NLCI rip-and-replace execution without redesigning the architecture.
> Current pause point: narrow CBU property verbs, narrow CBU lifecycle verbs, and narrow `cbu.create` are compiler-backed.
> First review active lexical/runtime guidance in `rust/src/mcp/noun_index.rs` and `rust/src/mcp/tools.rs` before expanding further.
> Do not reintroduce dual-route test/discovery surfaces for compiler-owned CBU intents.
> Keep deleting stale legacy expectations immediately after each proven cutover.

---

## 9. Definition of a Good Resume

The next session is on track if it does all of the following:

- starts from this file rather than re-analyzing from scratch
- preserves the current cutover boundary
- avoids adding new mixed legacy/compiler routes
- runs `cargo check` after edits
- keeps deletion paired with each successful cutover
