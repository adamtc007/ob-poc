# StateGraph Pipeline — Implementation Plan Against Current Codebase

## Classification
Refactor and phased build. Preserve the current three-step pipeline boundary and replace the heuristic Step 2 transition substrate with a graph-backed StateGraph engine.

## Objective
Implement the consolidated TODO in three strict phases without re-expanding the semantic pipeline. The target runtime contract remains:

1. `step1_entity_scope`
2. `step2_entity_state`
3. `step3_select_verb`

The change is in what Step 2 uses as its source of truth:
- today: signal-heavy heuristics plus `discovery.valid-transitions`
- target: graph-backed lane/node/edge evaluation with explicit valid and blocked transitions

## Current Baseline

### Already implemented
- Three-step utterance pipeline exists in [rust/src/semtaxonomy_v2/mod.rs](/Users/adamtc007/Developer/ob-poc/rust/src/semtaxonomy_v2/mod.rs).
- `discovery.valid-transitions` exists in [rust/config/verbs/discovery.yaml](/Users/adamtc007/Developer/ob-poc/rust/config/verbs/discovery.yaml) and [rust/src/domain_ops/discovery_ops.rs](/Users/adamtc007/Developer/ob-poc/rust/src/domain_ops/discovery_ops.rs).
- `entity-context` already exposes core signals:
  - `has_active_onboarding`
  - `has_active_deal`
  - `has_active_kyc`
  - `has_incomplete_ubo`
  - `has_pending_documentation`
- Lifecycle-aware candidate ranking is already present in the SemTaxonomy path.
- Harness exists at [rust/tests/semtaxonomy_utterance_coverage.rs](/Users/adamtc007/Developer/ob-poc/rust/tests/semtaxonomy_utterance_coverage.rs).

### Missing
- No StateGraph data model or graph loader.
- No graph-walk engine.
- No graph-backed `discovery.graph-walk` verb.
- `discovery.valid-transitions` is still derived from verb metadata and signals, not from lane graphs.
- Invocation-phrase enrichment is incomplete and inconsistent by domain.
- Several TODO verb IDs are stale relative to the current registry.

## Normalization Required Before Implementation
The TODO uses some verb names that do not match the current canonical surface. The plan must normalize to current verb IDs before touching code or tests.

### Normalize these verb IDs
- `deal.get` -> `deal.read-record`
- `deal.update-status` -> verify exact current registry entry before Phase 1.2 edits
- `cbu.role.assign` -> current canonical already present
- `document.missing-for-entity` -> current canonical already present
- `document.for-entity` -> current canonical already present
- `fund.create` -> current canonical already present
- `entity.list-placeholders` -> current canonical already present
- `screening.sanctions`, `screening.pep`, `screening.adverse-media` -> current canonical already present
- `ubo.registry.advance` -> verify registry presence before editing invocation phrases

### Rule
Do not introduce stale aliases into graph files or phrase enrichment. Phase 0 of implementation is a canonical-name audit against the live registry.

## Phase 0 — Canonical Surface Audit

### Goal
Freeze the exact verb IDs, subject kinds, and lifecycle/precondition fields that StateGraph will reference.

### Tasks
1. Export the current canonical verb list for the target domains:
   - `cbu`
   - `deal`
   - `screening`
   - `ubo`
   - `document`
   - `fund`
   - `entity`
2. Verify each TODO-referenced verb exists under its current canonical ID.
3. Produce a normalization map for any stale TODO names.
4. Validate all graph edge verbs against the registry before authoring graph files.

### Deliverables
- Normalization note in the TODO or a companion mapping file.
- Verified target list of graph-referenced verbs.

## Phase 1 — SemOS Cleanup

### Goal
Raise the quality of grounding/state signals and invocation phrases before building graph logic on top.

### Status against existing code
- `1.1 Entity-Context Signal Enrichment`: partially implemented
- `1.2 Invocation Phrase Enrichment`: partially implemented
- `1.3 Verify All Graph Edge Verbs Exist`: not implemented as a dedicated gate

### 1.1 Entity-context signal enrichment

#### What already exists
Current signal fields in `entity-context` are too coarse for graph walking, but the substrate is present.

#### Required additions
Add or split these signals in [rust/src/domain_ops/discovery_ops.rs](/Users/adamtc007/Developer/ob-poc/rust/src/domain_ops/discovery_ops.rs):
- Client-group:
  - active deal count
  - active onboarding count
  - active KYC case count
  - pending document count
  - screening review count by type where possible
  - linked CBU count
- Deal:
  - current deal phase/status normalized for lane derivation
  - rate-card state if available
  - linked document count / pending docs
- KYC:
  - case count split by open / blocked / pending review where possible
- Screening:
  - split counts by sanctions / pep / adverse-media instead of one generic screening signal
- Document:
  - split missing / pending verification / rejected / catalogued counts
- Fund / CBU:
  - expose signals needed for fund/cbu lane derivation without conflating the two models
- UBO:
  - fix verification completeness to distinguish:
    - no UBOs recorded
    - UBOs recorded but not verified
    - verified complete

#### Gate
Add focused unit/integration tests around each signal family.

### 1.2 Invocation phrase enrichment

#### Scope
Only enrich the domains listed in the TODO, but normalize against the live canonical names first.

#### Rule
Invocation phrases must be action-specific and lane-specific. No vague phrases that collapse read/write or status/create semantics.

#### Implementation targets
Edit the verb YAML files for:
- `cbu`
- `screening`
- `ubo`
- `document`
- `deal`
- `fund`
- `entity`

#### Gate
Add regression tests proving the enriched phrases separate the intended same-domain tie pairs.

### 1.3 Verify graph edge verbs exist

#### Implementation
Create a validation helper that loads graph files and verifies every edge verb ID exists in the registry.

#### Gate
This validation must run at startup and in tests before Phase 2 graph walking is considered complete.

## Phase 2 — StateGraph Engine

### Goal
Replace heuristic Step 2 transition derivation with a graph-backed engine.

### 2.1 StateGraph data model

#### New module
Create a dedicated module, preferably:
- `rust/src/stategraph/mod.rs`

#### Types to add
Implement the TODO data model directly:
- `StateGraph`
- `GraphNode`
- `NodeType`
- `GraphEdge`
- `EdgeType`
- `GraphGate`
- `SignalCondition`
- `GraphWalkResult`
- `ValidVerb`
- `BlockedVerb`
- `GateStatus`
- `GateRequirement`

#### Design constraint
Keep this module purely about graph evaluation. Do not put utterance parsing or response shaping in it.

### 2.2 YAML loader + startup validation

#### New config surface
Add graph YAML files under a dedicated directory, for example:
- `rust/config/stategraphs/`

#### Loader requirements
- load all graph files at startup
- validate:
  - duplicate graph IDs
  - missing nodes
  - missing edges
  - unknown verb IDs
  - unknown lane names
  - invalid gate references
  - invalid signal names

### 2.3 Signal evaluation

#### Implementation
Add `evaluate_signal(...)` that maps graph signal names to live `entity-context` data.

#### Rule
Signal evaluation must be explicit and typed. No fallback string matching.

#### Dependency
This phase depends on Phase 1.1 signal enrichment being complete enough to support the graph conditions.

### 2.4 Graph walker

#### Implementation
Add:
- `walk_graph(...)`
- `is_node_satisfied(...)`

#### Behavior
For a grounded entity and a selected graph:
- determine satisfied nodes
- determine frontier nodes
- emit:
  - valid verbs on frontier edges
  - blocked verbs with unmet conditions
  - gate status

#### Constraint
Do not derive valid verbs from raw registry surface once graph walk is active. The graph result becomes the Step 2 source of truth.

### 2.5 `discovery.graph-walk`

#### New discovery verb
Add `graph-walk` to [rust/config/verbs/discovery.yaml](/Users/adamtc007/Developer/ob-poc/rust/config/verbs/discovery.yaml) and implement `DiscoveryGraphWalkOp` in [rust/src/domain_ops/discovery_ops.rs](/Users/adamtc007/Developer/ob-poc/rust/src/domain_ops/discovery_ops.rs).

#### Behavior
- input: grounded `entity-id`
- output: `GraphWalkResult`
- optional `include-blocked`

#### Positioning
`graph-walk` becomes the StateGraph-backed replacement for the current heuristic `valid-transitions` path. Keep `valid-transitions` during migration, but make it a compatibility layer once `graph-walk` is stable.

## Phase 3 — DAG Population + Three-Step Pipeline + Harness

### Goal
Populate the graphs, switch Step 2 over to graph walking, and update the harness to measure the new contract.

### 3.1 Place corrected graph files

#### Required graphs
Author graph files for the lanes in the TODO:
- `entity`
- `cbu`
- `deal`
- `onboarding`
- `kyc`
- `ubo`
- `document`
- `screening`
- `fund`

#### Rule
Use current canonical verb IDs only.

### 3.2 Three-step pipeline switch

#### Current state
- Step 1 exists and is the hard gate.
- Step 2 currently normalizes `valid-transitions` output.
- Step 3 scores over `ValidVerb` candidates.

#### Required change
Switch `step2_entity_state(...)` in [rust/src/semtaxonomy_v2/mod.rs](/Users/adamtc007/Developer/ob-poc/rust/src/semtaxonomy_v2/mod.rs) to consume `discovery.graph-walk` output instead of the heuristic `valid-transitions` payload.

#### Resolution tiers
Keep only the simple final outcome tiers needed by the three-step contract:
- `SessionCommand`
- `TrivialMatch`
- `LlmSelection`
- `NoProposal`

### 3.3 Harness update + run

#### Required harness changes
Update [rust/tests/semtaxonomy_utterance_coverage.rs](/Users/adamtc007/Developer/ob-poc/rust/tests/semtaxonomy_utterance_coverage.rs) to report:
- Step 1 resolved / ambiguous / unresolved
- Step 2 graph-walk success
- Step 2 valid-verb count
- Step 3 proposal rate
- exact top-verb rate
- blocked explain rate

#### Measurement order
1. Seeded capability harness first
2. Live 176-utterance harness second

## Recommended Execution Order
1. Phase 0 canonical audit
2. Phase 1.1 signal enrichment
3. Phase 1.2 invocation phrase enrichment
4. Phase 1.3 graph-edge verb validation
5. Phase 2.1 data model
6. Phase 2.2 loader + validation
7. Phase 2.3 signal evaluation
8. Phase 2.4 graph walker
9. Phase 2.5 `discovery.graph-walk`
10. Phase 3.1 graph population
11. Phase 3.2 Step 2 switch in `semtaxonomy_v2`
12. Phase 3.3 harness reruns

## Success Criteria
- Step count stays fixed at three semantic steps.
- Step 1 remains a hard gate.
- Step 2 is graph-backed, not heuristic.
- `discovery.*` verbs remain research tools, not final business output.
- All graph edge verbs validate against the live registry at startup.
- Harness reports show improved Step 2 valid-verb quality and a materially lower no-proposal count.

## Key Risks
- TODO verb IDs that no longer match the registry will poison graph authoring if not normalized first.
- Overloading `fund` and `cbu` signals without clarifying the operational boundary will create graph ambiguity.
- If Phase 1 signal enrichment is weak, the graph walker will only make the current ambiguity more explicit, not improve it.

## Recommendation
Implement this as a true substrate replacement for Step 2, not as another parallel transition source. The current `valid-transitions` logic is useful as a migration bridge and as a fallback oracle during development, but the end state should have one authoritative Step 2 source: graph walk over enriched live state.
