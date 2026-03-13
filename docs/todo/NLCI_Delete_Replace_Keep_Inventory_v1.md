# NLCI Delete / Replace / Keep Inventory

> **Date:** 2026-03-12
> **Status:** Phase 0 inventory
> **Purpose:** Define which current utterance-pipeline surfaces in `ob-poc` should be kept, replaced, or deleted during NLCI implementation

---

## 1. Rule

This inventory follows one rule:

- keep the NLCI architecture fixed
- replace internals that conflict with it
- delete superseded paths once the replacement path is proven

This is an implementation inventory, not an architecture redesign.

---

## 2. Canonical Replacement Boundary

The canonical home for the NLCI compiler path is:

- `rust/src/semtaxonomy_v2/`

The orchestrator remains the outer coordination boundary:

- `rust/src/agent/orchestrator.rs`

The existing `sage` module remains useful only where it can be narrowed to Layer 1 extraction concerns or temporary compatibility at the boundary.

---

## 3. Keep

These surfaces should be kept because they fit the target architecture or provide reusable boundary infrastructure.

### Keep as boundary/orchestration infrastructure

- `rust/src/agent/orchestrator.rs`
  - keep as the single top-level utterance entrypoint
  - replace its internal compiler path over time
- `rust/src/agent/harness/`
  - keep and extend as the canonical test harness
- `rust/src/api/agent_service.rs`
  - keep as API boundary
  - route through the new compiler path rather than preserving internal legacy types

### Keep as metadata/config surfaces

- `rust/config/lexicon/`
- `rust/config/verb_schemas/`
- `rust/config/noun_index.yaml`
- `rust/config/sem_os_seeds/domain_metadata.yaml`

These act as SemOS-like metadata inputs for discrimination and selection.

### Keep with narrowing

- `rust/src/dsl_v2/prompts/general_intent_extraction.md`
  - keep as Layer 1 prompt surface
  - rewrite to emit only canonical structured intent

---

## 4. Replace

These surfaces contain useful concepts but currently act as the wrong canonical contract.

### Replace as canonical compiler contracts

- `rust/src/sage/outcome.rs`
  - current role: implicit structured intent contract
  - problem: optimized for Sage/Coder handoff rather than the NLCI schema/IR contract
  - action: replace as canonical compiler contract with typed NLCI schema in `semtaxonomy_v2`

- `rust/src/sage/coder.rs`
  - current role: deterministic selection and DSL assembly
  - problem: mixes selection, thresholds, and legacy handoff semantics under the old `OutcomeIntent` model
  - action: replace with explicit phase interfaces in `semtaxonomy_v2`

- `rust/src/sage/arg_assembly.rs`
  - current role: `OutcomeStep` to DSL-ready structured intent
  - problem: assumes the legacy step model is the compiler boundary
  - action: replace with NLCI composition/binding phase logic

- `rust/src/sage/verb_resolve.rs`
  - current role: scoring and selection
  - problem: valuable internals may remain, but the current interface is not the target compiler contract
  - action: replace the public/compiler-facing role; salvage internals only if they fit the new phase modules cleanly

### Replace inside orchestrator

- ad hoc orchestration around:
  - `OutcomeIntent`
  - `CoderResult`
  - read fallbacks
  - implicit phase branching

Action:

- keep orchestrator
- replace its internal resolution path so it calls one typed NLCI compiler entrypoint

### Replace in `semtaxonomy_v2`

- current `SelectedVerb.args: serde_json::Value`
  - problem: stringly/untyped phase boundary
  - action: replace with typed compiler argument/binding structures

---

## 5. Delete After Cutover

These surfaces should be deleted once the new compiler path is live and proven by the harness.

### Delete likely legacy handoff surfaces

- `CoderHandoff` in `rust/src/sage/outcome.rs`
- legacy `OutcomeStep`-centric assembly path where no longer used
- legacy direct scorer-to-DSL shortcuts that bypass the canonical compiler phases

### Delete likely duplicate tests

- tests that validate retired `OutcomeIntent -> CoderResult` semantics as the main pipeline
- tests that assert old fallback behavior if that behavior conflicts with the NLCI architecture

### Delete legacy bypass logic

- any direct utterance-to-verb path that survives outside the compiler entrypoint
- any extraction path that produces DSL or quasi-DSL directly

---

## 6. Transitional Compatibility

Temporary compatibility is allowed only at these boundaries:

- API response shaping
- orchestrator-to-chat integration
- harness adapters while the cutover is in progress

Compatibility is not allowed in:

- compiler contracts
- phase boundaries
- failure taxonomy

---

## 7. Immediate Phase 1 Implications

The first code changes should therefore:

1. add canonical NLCI types under `rust/src/semtaxonomy_v2/`
2. avoid making `OutcomeIntent` or `SelectedVerb.args: Value` more central
3. prepare orchestrator to depend on the new compiler contracts instead of the legacy Sage/Coder handoff

---

## 8. Bottom Line

The implementation direction is:

- keep the outer pipeline shell
- replace the internal compiler contracts
- delete legacy handoff logic after harness-proven cutover

That is the cleanest way to avoid creating another layer of dead code.
