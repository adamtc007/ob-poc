# Constellation Map TODO Status

**Date:** 2026-03-14  
**Scope:** Current branch status against `TODO_constellation_map_implementation.md`  
**Purpose:** Precise handoff showing what is done, what is incomplete, and why.

---

## Overall Status

The constellation subsystem is **substantially implemented but not fully code-complete**.

- `Done`: map schema, loader, validation, builtin map loading, reducer integration hooks, normalization, action-surface, summary, plugin verbs, and focused test coverage.
- `Partial`: query-plan compilation and DB hydration exist and are useful, but they are still pragmatic rather than fully map-executed for every slot/overlay shape.
- `Incomplete`: full-fidelity graph payloads, edge-overlay hydration, and DB-backed end-to-end verb coverage.

This means the branch is **engineering-usable** and **test-green**, but a few TODO targets still depend on schema/product decisions that are not fully specified by the current docs.

---

## Phase-by-Phase Status

## Phase 1 — Map YAML Schema + Types + Loader

**Status:** `Done`

**Implemented**
- `rust/src/sem_reg/constellation/map_def.rs`
- `rust/src/sem_reg/constellation/map_loader.rs`
- `rust/config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml`
- `rust/src/sem_reg/constellation/builtin.rs`
- `rust/src/sem_reg/constellation/mod.rs`
- `rust/src/sem_reg/mod.rs`

**What is done**
- Map schema types exist and deserialize correctly.
- Builtin seed loading is in place.
- Stable map revision hashing exists.

**Why this is done**
- The builtin map is loadable, validated, and exercised by tests.

---

## Phase 2 — Map Validation

**Status:** `Done`

**Implemented**
- `rust/src/sem_reg/constellation/validate.rs`

**What is done**
- Root-slot validation.
- Recursive-slot validation.
- Join/dependency validation.
- Reducer state machine existence validation.
- Flattening, parent/path resolution, and topological ordering.

**Why this is done**
- The startup contract for the shipped builtin map is enforced.

---

## Phase 3 — Query Plan Compilation

**Status:** `Partial`

**Implemented**
- `rust/src/sem_reg/constellation/query_plan.rs`

**What is done**
- Query-plan types exist.
- Slots are compiled by depth.
- Recursive slots compile to `RecursiveCte`.
- Role-based entity slots compile through a coarse batch optimization path.
- Overlay work is now represented explicitly through `OverlayBatch` plan entries for slot overlays and edge overlays.

**What is incomplete**
- The compiled plan is not yet the runtime execution engine.
- SQL text is still coarse/planning-oriented rather than a full executable compiler output.

**Why incomplete**
- The hydrator still executes pragmatic per-slot queries directly.
- The TODO’s “plan compiler as source of truth” contract is larger than the current runtime path.

---

## Phase 4 — Hydration Walker

**Status:** `Partial`

**Implemented**
- `rust/src/sem_reg/constellation/hydration.rs`

**What is done**
- Root CBU hydration.
- Role-based entity slot hydration through `cbu_entity_roles` plus `roles`.
- Case hydration.
- Tollgate hydration from `tollgate_evaluations`.
- Mandate hydration from `cbu_trading_profiles`.
- Recursive entity-graph hydration from `entity_relationships_current`.
- Entity detail enrichment from `entities`.
- Reducer integration for state-machine-backed singular entity slots.
- Constellation-driven discovery of reducer slot contexts for `state.derive-all` / `state.check-consistency`.
- Overlay rows are materialized from reducer-fetched overlay sources.
- Deterministic representative row selection is now aligned between hydration and normalization.

**What is incomplete**
- Edge overlays declared in `edge_overlays` are not hydrated as first-class data yet.
- Graph-node reducer state is not emitted per node in the normalized payload.
- The walker does not yet execute the compiled `HydrationQueryPlan`.

**Why incomplete**
- The remaining work is mostly about payload fidelity and execution architecture, not missing basic plumbing.
- The TODO expects richer graph/edge semantics than the current repo docs fully pin down.

---

## Phase 5 — Slot Fill Normalization + Payload Types

**Status:** `Partial`

**Implemented**
- `rust/src/sem_reg/constellation/normalize.rs`
- `rust/src/sem_reg/constellation/hydrated.rs`

**What is done**
- Singular slot normalization exists.
- Recursive graph slot normalization exists.
- Deterministic multiplicity handling exists.
- Reducer-derived states, warnings, available verbs, and blocked verbs flow into normalized singular entity slots.
- Overlay rows are attached to normalized slots.

**What is incomplete**
- Graph payloads are still summary-oriented rather than node-by-node hydrated structures.
- Normalized payloads do not yet expose dedicated edge-overlay structures.
- Placeholder detection still uses the current repo-safe fallback, not a fully product-confirmed contract.

**Why incomplete**
- The remaining gap is mostly about richer return-shape design, not missing state derivation.

---

## Phase 6 — ActionSurface + Summary

**Status:** `Partial`

**Implemented**
- `rust/src/sem_reg/constellation/action_surface.rs`
- `rust/src/sem_reg/constellation/summary.rs`

**What is done**
- Progress computation exists.
- Mandatory-slot blocking exists.
- Dependency-aware action gating exists.
- Block reasons now name missing or insufficient dependencies directly.
- Reducer-provided verb availability is preserved.
- Palette-verb duplication is avoided.
- Reducer-machine state ordering is used when a slot has a bound state machine, with generic fallback for non-reducer slots.
- Summary counts exist.

**What is incomplete**
- Action surface is still slot-centric, not transition-aware in the same depth as a full workflow engine.
- Some non-reducer slot state semantics are still generic.

**Why incomplete**
- The TODO implies a richer UX-facing action contract than the current map + reducer surface fully defines.

---

## Phase 7 — Builtin Map Loading + Replace Reducer Stubs

**Status:** `Partial`

**Implemented**
- Builtin map loading exists.
- Reducer `derive-all` and `check-consistency` now attempt constellation-map-driven slot discovery first.
- Constellation hydration and reducer evaluation are connected.

**What is incomplete**
- Reducer slot discovery is not yet exclusively constellation-driven in every possible runtime path.
- Graph-node reducer derivation is not yet surfaced as a first-class constellation payload.

**Why incomplete**
- The current implementation replaced the pragmatic fallback where possible, but it still keeps the reducer-side fallback for resilience.

---

## Phase 8 — Tests

**Status:** `Partial`

**Implemented**
- `rust/tests/constellation_map_tests.rs`
- `rust/tests/constellation_hydration_tests.rs`
- `rust/tests/constellation_action_tests.rs`

**What is done**
- Loader and validation coverage exists.
- Revision stability is tested.
- Overlay-batch plan compilation is tested.
- Normalization coverage exists.
- Graph warning behavior is tested.
- Action-surface dependency/blocking behavior is tested.

**What is incomplete**
- No DB-backed integration tests for `constellation.hydrate` / `constellation.summary`.
- No end-to-end tests validating graph hydration against a real Postgres fixture.
- No tests for first-class edge-overlay payloads, because those payloads do not yet exist.

**Why incomplete**
- The remaining uncovered behavior is exactly the part that still depends on schema-confirmed runtime semantics.

---

## Phase 9 — DSL Verb Handlers + Module Wiring

**Status:** `Done`

**Implemented**
- `rust/src/sem_reg/constellation/verbs.rs`
- `rust/src/domain_ops/constellation_ops.rs`
- `rust/config/verbs/constellation.yaml`
- `rust/src/domain_ops/mod.rs`

**What is done**
- Plugin handlers exist for `constellation.hydrate` and `constellation.summary`.
- Verb config exists.
- Domain-op wiring exists.

**Why this is done**
- The plugin surface is present and executable.

---

## Concrete Incomplete Items

These are the remaining practical items that are not fully closed:

1. Execute compiled query plans as the hydrator’s primary source of truth instead of using direct per-slot query functions.
2. Add first-class edge-overlay hydration and payload modeling for recursive graph slots.
3. Expose graph-node reducer results in the normalized constellation payload instead of only graph-level summaries.
4. Replace the remaining reducer fallback slot-discovery path so constellation is the sole discovery engine for multi-slot reducer operations.
5. Add DB-backed integration tests for `constellation.hydrate`, `constellation.summary`, and recursive graph hydration.
6. Confirm and harden placeholder detection semantics against the intended business contract.

---

## Why These Items Remain Open

The remaining items are open because they need one of two things:

- a larger architectural step that is independent of correctness but not yet implemented, such as executing the compiled plan rather than direct queries; or
- schema/product confirmation that the current TODO does not completely specify, especially for graph payload semantics and placeholder detection.

The branch is not blocked on parser/evaluator uncertainty anymore. The open items are now mostly about runtime fidelity and exact data-contract choices.

---

## Questions / Unknowns For Follow-Up

These are the concrete unresolved knowledge gaps worth sending to a sandbox LLM or schema-review pass:

1. Should `HydratedSlot` for `entity_graph` expose node-level reducer states and per-edge overlays in the public payload, and if so what exact shape is expected?
2. Is `entity_relationships_current` the intended authoritative source for recursive `ownership_chain` hydration, or should the walker target a different canonical graph projection?
3. Should `edge_overlays: [ownership]` map to raw relationship fields from `entity_relationships_current`, or to a separate overlay source model?
4. Is placeholder detection supposed to remain name-based in this repo, or should it key off a dedicated placeholder flag/code on entity-linked records?
5. Is constellation intended to fully replace reducer-side slot discovery for every `state.*` bulk operation, or is the current fallback acceptable by design?
6. What DB fixture or seeded dataset should be treated as the canonical source for DB-backed constellation integration tests?

---

## Verification Status

Verified on current branch:

- `cargo check`
- `cargo test --test constellation_map_tests --test constellation_hydration_tests --test constellation_action_tests`
- `cargo test --test reducer_parser_tests --test reducer_eval_tests --test reducer_validation_tests --test reducer_state_phase2_tests --test reducer_integration_tests`

This verifies the implemented runtime and the current reducer/constellation integration surface. It does not yet prove the remaining schema-sensitive TODO items listed above.
