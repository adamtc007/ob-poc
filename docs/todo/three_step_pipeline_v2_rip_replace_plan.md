# Three-Step Pipeline v2 — Rip and Replace Plan

## Classification
Explicit rip-and-replace of the current utterance-understanding pipeline.

This is not an incremental refactor of the existing SemTaxonomy composition path. It is a controlled replacement of the semantic pipeline that currently sits between:
- chat utterance ingress
- grounded business runbook proposal

## Replace vs Preserve

### Preserve
These are not being rewritten:
- DSL execution engine
- REPL/runbook execution
- SemOS registry and governance
- discovery verbs that query live business data
- `discovery.valid-transitions` computation if it remains useful as the Step 2 substrate
- confirmation/execution handling after a runbook has been selected

### Replace
These are the rip boundary:
- current `try_semtaxonomy_path()` orchestration body
- current SemTaxonomy composition chain in [semtaxonomy/mod.rs](/Users/adamtc007/Developer/ob-poc/rust/src/semtaxonomy/mod.rs)
- ad hoc domain inference as a separate semantic stage
- current scorer/fallback composition layering
- current partial-proposal logic where it exists outside the 3-step contract
- any residual semantic fallthrough into the old Sage/Coder path for normal utterances

## Objective
Build a new pipeline with exactly three semantic steps:
1. `step1_entity_scope`
2. `step2_entity_state`
3. `step3_select_verb`

All utterance understanding must go through those three steps only.

## Hard Gate Invariant
Step 1 is a hard gate.

- `Resolved` -> Step 2 may run
- `Ambiguous` -> Step 2 and Step 3 must not run
- `Unresolved` -> Step 2 and Step 3 must not run unless the utterance is explicitly generic and non-entity-scoped

This is non-negotiable. Ambiguity must never cascade into state derivation or transition selection.

## Why Rip and Replace
The current path has accumulated too many overlapping responsibilities:
- scope detection
- candidate search
- context hydration
- domain inference
- transition filtering
- scoring
- partial proposal generation
- compatibility fallbacks

That shape is hard to reason about, hard to measure, and too easy to re-break. Replanning the same path would still leave old semantics in the call stack. A clean module boundary is the lower-risk move now.

## New Module Boundary
Create a new dedicated pipeline module, for example:
- `rust/src/semtaxonomy_v2/mod.rs`

That module owns only:
- step contracts
- step execution
- final verb selection output

It must not import the old SemTaxonomy composition functions except temporarily through adapters during migration.

## New Typed Contracts
The replacement module should define these as the only semantic boundary types:

### Step 1 Output
- `EntityScope`
  - `entity_id`
  - `entity_type`
  - `name`
  - `confidence`
  - `source`
- `EntitySource`
  - `SessionCarry`
  - `SearchHit`
  - `UserConfirmed`
- `EntityScopeOutcome`
  - `Resolved`
  - `Ambiguous`
  - `Unresolved`
- `Step1Feedback`
  - user-facing Sage explanation of the scope decision
  - candidate list for confirmation when ambiguous

### Step 2 Output
- `EntityState`
  - `entity`
  - `lane_positions`
  - `valid_verbs`
  - `blocked_verbs`
- `LanePosition`
- `ValidVerb`
- `BlockedVerb`

### Step 3 Output
- `SelectedVerb`
  - `verb_id`
  - `args`
  - `explanation`
  - `requires_confirmation`
  - `missing_args`
  - `partial`
  - `resolution_mode`

Compatibility envelope:
- if the rest of the system still expects `ComposedRunbook`, derive it mechanically from `SelectedVerb`
- do not let `ComposedRunbook` remain a parallel semantic model

## Replacement Phases

### Phase R1 — Build the New Module Skeleton
Files:
- new: [semtaxonomy_v2/mod.rs](/Users/adamtc007/Developer/ob-poc/rust/src/semtaxonomy_v2/mod.rs)
- update: [lib.rs](/Users/adamtc007/Developer/ob-poc/rust/src/lib.rs)

Tasks:
- create the new module
- define the new three-step output structs
- add doc comments with examples for all new public functions
- do not wire it into the live path yet

Verification:
- `cargo check -p ob-poc`

### Phase R2 — Implement Step 1 in the New Module
Files:
- [semtaxonomy_v2/mod.rs](/Users/adamtc007/Developer/ob-poc/rust/src/semtaxonomy_v2/mod.rs)

Tasks:
- move the current entity-name extraction and candidate merge logic into `step1_entity_scope(...)`
- keep search/input behavior, but remove any action inference from this step
- Step 1 must only determine scope and ambiguity
- if multiple plausible candidates remain, return structured ambiguity, not legacy pending-decision state

Reuse allowed:
- discovery search ops
- extracted entity candidate utility
- candidate ranking helpers

Verification:
- `cargo check -p ob-poc`
- new unit tests for carry/search/ambiguity/bare-name cases

### Phase R3 — Implement Step 2 in the New Module
Files:
- [semtaxonomy_v2/mod.rs](/Users/adamtc007/Developer/ob-poc/rust/src/semtaxonomy_v2/mod.rs)

Tasks:
- implement `step2_entity_state(...)`
- use existing live state readers and `valid-transitions` as the substrate if appropriate
- normalize them into `EntityState`
- centralize:
  - lane derivation
  - phase derivation
  - valid verb list
  - blocked verb list
- Step 2 must not pick a final action

Reuse allowed:
- `discovery.entity-context`
- `discovery.valid-transitions`
- current lane/phase/precondition logic where cleanly portable

Verification:
- `cargo check -p ob-poc`
- unit tests for valid vs blocked outputs by lane

### Phase R4 — Implement Step 3 in the New Module
Files:
- [semtaxonomy_v2/mod.rs](/Users/adamtc007/Developer/ob-poc/rust/src/semtaxonomy_v2/mod.rs)

Tasks:
- implement `step3_select_verb(...)`
- deterministic trivial match first
- constrained LLM selection second if that remains enabled later
- partial proposal behavior belongs here only
- no discovery verbs as final output
- no separate scorer chain outside this function

Reuse allowed:
- current matching heuristics where still useful
- current partial-proposal rules where they map cleanly

Verification:
- `cargo check -p ob-poc`
- tests for list/create/update/partial/blocked cases

### Phase R5 — Compatibility Adapter
Files:
- [semtaxonomy_v2/mod.rs](/Users/adamtc007/Developer/ob-poc/rust/src/semtaxonomy_v2/mod.rs)
- [agent_service.rs](/Users/adamtc007/Developer/ob-poc/rust/src/api/agent_service.rs)

Tasks:
- add a tiny adapter that converts Step 1 feedback or `SelectedVerb` into whatever `AgentService` currently needs to return to chat
- keep this adapter thin and mechanical
- do not reintroduce semantic logic in the adapter

Verification:
- `cargo check -p ob-poc`

### Phase R6 — Hard Cutover in AgentService
Files:
- [agent_service.rs](/Users/adamtc007/Developer/ob-poc/rust/src/api/agent_service.rs)

Tasks:
- replace the current SemTaxonomy path body with:
  1. Step 1
  2. if and only if `Resolved`, Step 2
  3. if and only if `Resolved`, Step 3
- on `Ambiguous`, return Step 1 Sage clarification feedback immediately
- on `Unresolved`, return Step 1 no-scope feedback immediately unless the utterance is explicitly generic
- no normal utterance should go through the old SemTaxonomy composition code after this point
- explicit execution/confirmation handling remains outside the three semantic steps

Safety:
- keep a temporary env flag or dead-simple rollback branch only during cutover
- but the default normal path must be the new module only

Verification:
- `cargo check -p ob-poc`
- live smoke for read/write/partial/blocked cases

### Phase R7 — Harness Conversion
Files:
- [semtaxonomy_utterance_coverage.rs](/Users/adamtc007/Developer/ob-poc/rust/tests/semtaxonomy_utterance_coverage.rs)

Tasks:
- instrument by replacement steps:
  - Step 1 resolved
  - Step 1 ambiguous
  - Step 1 unresolved
  - Step 2 success
  - Step 3 proposal
- keep exact top-verb accuracy
- measure whether the new module actually removed the semantic loss points
- explicitly report how often ambiguity is stopped at Step 1 instead of leaking downstream

Verification:
- `cargo test -p ob-poc --test semtaxonomy_utterance_coverage --no-run`

### Phase R8 — Delete the Old Path
Files:
- [semtaxonomy/mod.rs](/Users/adamtc007/Developer/ob-poc/rust/src/semtaxonomy/mod.rs)
- [agent_service.rs](/Users/adamtc007/Developer/ob-poc/rust/src/api/agent_service.rs)
- any remaining tests that target old composition internals

Tasks:
- remove dead old composition functions
- remove duplicated semantic structs no longer used
- remove compatibility shims that are no longer needed
- keep only the new three-step module as the utterance-understanding path

Verification:
- `cargo check -p ob-poc`
- `cargo test -p ob-poc`
- live harness rerun

## Discipline Rules For This Replacement
- No copying old fallback branches into the new module
- No extra “temporary” semantic stages beyond the three defined steps
- No discovery verbs in final business output
- No hidden side channels for domain inference outside Step 2
- No semantic logic in chat adapter code
- No ambiguity may pass Step 1

## Success Criteria
1. The normal utterance path is implemented only in the new three-step module
2. Failures can be attributed directly to Step 1, Step 2, or Step 3
3. `no proposal` falls materially below current `68/176`
4. Business proposals rise materially above current `108/176`
5. Exact-verb accuracy improves materially above current `18.18%`
6. The old SemTaxonomy composition path is deleted

## Recommendation
Proceed as rip-and-replace. The current code shape has already proven that incremental semantic patching costs more than a clean boundary. The replacement should be small, typed, and ruthless about staying at three steps only.
