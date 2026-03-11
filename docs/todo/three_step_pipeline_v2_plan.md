# Three-Step Pipeline v2 — Implementation Plan

## Classification
Refactor and replacement of the current SemTaxonomy composition path.

This plan deliberately collapses the utterance pipeline to exactly three semantic steps:
1. `EntityScope`
2. `EntityState`
3. `SelectedVerb`

The purpose is to stop the current step explosion and the associated lossy intermediate transforms.

## Goal
Replace the current mixture of:
- domain inference
- discovery search
- entity-context hydration
- valid-transitions lookup
- scorer composition
- partial proposal logic

with a single explicit three-step flow:
- `step1_entity_scope(...)`
- `step2_entity_state(...)`
- `step3_select_verb(...)`

The DSL execution engine, REPL, governance, and registry remain unchanged.

## Invariants
- No new semantic stages beyond the three steps above
- Discovery verbs remain research tools, not final business output
- SemOS remains the authority for verb contracts and governance
- Live business data/state remains the grounding source
- Step outputs must be typed and preserved end to end
- Any clarification / partial proposal behavior must emerge from Step 3 output, not from extra ad hoc sub-pipelines

## Current Code Reality
The current SemTaxonomy path already contains pieces of the new model, but they are spread across too many functions and caches:
- grounding in [agent_service.rs](/Users/adamtc007/Developer/ob-poc/rust/src/api/agent_service.rs)
- state hydration in [discovery_ops.rs](/Users/adamtc007/Developer/ob-poc/rust/src/domain_ops/discovery_ops.rs)
- transition narrowing in `discovery.valid-transitions`
- composition in [semtaxonomy/mod.rs](/Users/adamtc007/Developer/ob-poc/rust/src/semtaxonomy/mod.rs)

This plan consolidates those into three explicit boundaries instead of adding another layer.

## Step Model

### Step 1 — Entity Scope
Purpose:
- determine the active entity scope for the utterance
- resolve whether scope comes from session carry, search hit, or user confirmation

Output:
- `EntityScope`
  - `entity_id`
  - `entity_type`
  - `name`
  - `confidence`
  - `source`

Rules:
- use session carry only when the utterance does not clearly introduce a new entity
- use extracted entity-name candidates before raw utterance search
- preserve ambiguity when multiple hits are plausible; do not silently collapse low-confidence cases
- Step 1 must only answer: “what entity are we talking about?”
- Step 1 must not infer business action

### Step 2 — Entity State
Purpose:
- derive lane positions and valid verbs for the scoped entity using live state plus SemOS verb metadata

Output:
- `EntityState`
  - `entity`
  - `lane_positions`
  - `valid_verbs`

Rules:
- lane derivation is stateful and DAG-reconciled
- valid verbs are filtered by:
  - entity type
  - lane
  - phase
  - preconditions
- blocked verbs may still be retained for explain mode, but Step 2 output should clearly separate valid vs blocked
- Step 2 must answer: “what is the current state, and what transitions are currently available?”
- Step 2 must not choose the final verb

### Step 3 — Verb Selection
Purpose:
- choose one business verb from the already-valid transition set, attach args, and emit explanation

Output:
- `SelectedVerb`
  - `verb_id`
  - `args`
  - `explanation`
  - `requires_confirmation`
  - `missing_args`
  - `partial`

Rules:
- trivial deterministic matches first
- LLM selection only over the valid verb menu from Step 2
- no discovery verbs in final output
- if no executable selection is possible, return a partial or explain result rather than dropping out into another pipeline
- Step 3 must answer: “which action best matches the utterance, given the current valid options?”

## Implementation Phases

### Phase P1 — Introduce Typed Three-Step Contracts
Files:
- [semtaxonomy/mod.rs](/Users/adamtc007/Developer/ob-poc/rust/src/semtaxonomy/mod.rs)

Tasks:
- Add:
  - `EntityScope`
  - `EntitySource`
  - `EntityState`
  - `LanePosition`
  - `ValidVerb`
  - `SelectedVerb`
- Keep existing `ComposedRunbook` temporarily as a compatibility envelope, but derive it strictly from `SelectedVerb`
- Add doc comments with examples to all new public functions/types as required by project rules

Verification:
- `cargo check -p ob-poc`

### Phase P2 — Step 1 Extraction
Files:
- [agent_service.rs](/Users/adamtc007/Developer/ob-poc/rust/src/api/agent_service.rs)
- [semtaxonomy/mod.rs](/Users/adamtc007/Developer/ob-poc/rust/src/semtaxonomy/mod.rs)

Tasks:
- Implement `step1_entity_scope(...)`
- Move current entity-name extraction and candidate merging into this step
- Remove duplicated scope logic from `try_semtaxonomy_path()` and make it call Step 1 directly
- Ensure Step 1 returns structured ambiguity instead of leaking into legacy decision state

Verification:
- `cargo check -p ob-poc`
- unit tests for:
  - session carry
  - direct search hit
  - multi-candidate ambiguity
  - bare entity utterance

### Phase P3 — Step 2 State Engine
Files:
- [semtaxonomy/mod.rs](/Users/adamtc007/Developer/ob-poc/rust/src/semtaxonomy/mod.rs)
- [discovery_ops.rs](/Users/adamtc007/Developer/ob-poc/rust/src/domain_ops/discovery_ops.rs)
- config files only if annotations are actually needed

Tasks:
- Implement `step2_entity_state(...)`
- Centralize:
  - lane derivation
  - phase derivation
  - valid-verb filtering
  - blocked-verb reporting
- Prefer computing from existing SemOS contracts and current `valid-transitions` logic rather than inventing another transition path
- Treat `discovery.valid-transitions` as the implementation substrate if it remains useful, but its output should map into `EntityState`, not remain a separate semantic stage

Verification:
- `cargo check -p ob-poc`
- tests for:
  - lane derivation
  - valid verb filtering
  - blocked transition explanation

### Phase P4 — Invocation Phrase Enrichment
Files:
- relevant verb YAML files under `rust/config/verbs/`

Tasks:
- Enrich invocation phrases only for the verbs in the active lane surfaces that are currently ambiguous
- Do not do a repo-wide enrichment pass upfront
- Focus first on the highest-impact verbs already appearing in harness misses

Verification:
- `cargo check -p ob-poc`
- ensure config loads

### Phase P5 — Step 3 Selection
Files:
- [semtaxonomy/mod.rs](/Users/adamtc007/Developer/ob-poc/rust/src/semtaxonomy/mod.rs)

Tasks:
- Implement `step3_select_verb(...)`
- Deterministic trivial-match branch first
- LLM branch second, strictly over `EntityState.valid_verbs`
- Argument validation against the selected verb contract
- Partial proposal generation for missing required args
- No fallback to any other scorer chain outside Step 3

Verification:
- `cargo check -p ob-poc`
- tests for:
  - deterministic list/create cases
  - partial proposal on missing args
  - LLM selection parse/validation
  - rejection of `discovery.*` as final output

### Phase P6 — Wire the Three-Step Flow
Files:
- [agent_service.rs](/Users/adamtc007/Developer/ob-poc/rust/src/api/agent_service.rs)

Tasks:
- Replace the current SemTaxonomy path body with:
  1. `step1_entity_scope(...)`
  2. `step2_entity_state(...)`
  3. `step3_select_verb(...)`
- Remove any residual extra semantic branches from the normal utterance path
- Keep explicit execution/confirmation handling outside the three semantic steps, because execution is not part of utterance understanding

Verification:
- `cargo check -p ob-poc`
- live smoke for:
  - scope utterance
  - state/status utterance
  - read inventory utterance
  - write utterance with confirmation

### Phase P7 — Harness Alignment
Files:
- [semtaxonomy_utterance_coverage.rs](/Users/adamtc007/Developer/ob-poc/rust/tests/semtaxonomy_utterance_coverage.rs)

Tasks:
- Instrument rows by step success:
  - Step 1 success
  - Step 2 success
  - Step 3 business proposal
- Preserve top-verb exact match, but add per-step metrics so we can see where loss happens
- Confirm the pipeline is actually three stages in practice, not just in the design doc

Verification:
- `cargo test -p ob-poc --test semtaxonomy_utterance_coverage --no-run`

## What This Plan Explicitly Removes
- ad hoc domain inference as an extra semantic stage
- independent scorer chains outside Step 3
- discovery verbs as accidental end-user outputs
- hidden control-flow branches that re-enter old composition logic

## Success Criteria
1. The normal utterance path is visibly and measurably only:
   - Entity Scope
   - Entity State
   - Verb Selection
2. Harness reporting can attribute failures to one of those three stages
3. `no proposal` drops further from current `68/176`
4. Business proposals rise above current `108/176`
5. Exact-verb accuracy improves materially above current `18.18%`

## Review Decision
This spec is coherent and materially simpler than the exploding SemTaxonomy path. The main implementation discipline required is to avoid smuggling old intermediate semantics back in under new names.
