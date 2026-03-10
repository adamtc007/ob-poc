# Value + State Grounded SemTaxonomy — Implementation Plan

## Classification
Enhancement and refactor of the SemTaxonomy replacement path.

This does not change:
- the DSL execution engine
- the REPL/runbook executor
- the Sem OS governance layer
- the verb registry as the source of allowed action contracts

This does change:
- how utterances are grounded before composition
- how real data/state is gathered and fed into composition
- how candidate entities and current operational state influence runbook selection

## Why This Plan Exists
Current SemTaxonomy coverage is poor because the system is still too structure-first and too shallowly grounded.

Observed failure shape:
- most failures are `no proposal`, not bad final ranking
- discovery verbs are being used as a safe default instead of as a grounding layer feeding business composition
- one inferred domain plus one active entity is not enough context to compose robustly
- dictionary / noun-index / verb-surface matching is too weak by itself for natural business utterances

Working hypothesis:
- the lack of rich actual-data search and actual-state inspection is a major contributor to the low hit rate
- Sem OS structure remains necessary, but it is not sufficient
- the resolver must use both:
  - Sem OS structural context
  - live data values and live workflow state

## Core Principle
Sem OS defines what is allowed.
Live data and workflow state define what is relevant now.
Composition must use both.

Target path:
1. utterance
2. fuzzy candidate discovery over real values
3. state research over plausible candidates
4. Sem OS action-surface narrowing
5. runbook composition over the combined grounded context

## Current Failure Summary
From the latest SemTaxonomy coverage run:
- total: 176
- passed: 4
- accuracy: 2.27%

Dominant failure classes:
- no proposal: 147
- discovery-search overuse: 21
- wrong-domain selection: rare
- wrong-action selection: rare

Interpretation:
- the system is mostly failing before confident business composition
- this is a grounding/state deficit more than a final-scoring deficit

## Objectives
1. Replace first-hit grounding with candidate-set grounding
2. Expand discovery from name search to stateful research
3. Feed candidate state into composition
4. Prefer business verbs over discovery verbs once grounding is adequate
5. Return partial, explainable proposals instead of `None`

## Phase G1 — Candidate-Set Grounding
### Goal
Stop collapsing discovery to the first entity hit.

### Implementation
- Extend `SageSession` in `rust/src/semtaxonomy/mod.rs` to carry:
  - `entity_candidates: Vec<EntityCandidate>`
  - `grounding_strategy: Option<String>`
  - `grounding_confidence: Option<String>`
- Introduce `EntityCandidate` struct:
  - `entity_id`
  - `entity_type`
  - `name`
  - `match_score`
  - `match_field`
  - `summary`
- In `try_semtaxonomy_path()` in `rust/src/api/agent_service.rs`:
  - store the full `discovery.search-entities` result set
  - do not immediately reduce it to `active_entity`
  - derive `active_entity` only when one candidate is dominant
- Add candidate ranking rules based on:
  - name score
  - entity-type affinity from utterance
  - current session scope
  - stage focus
  - prior selected entity

### Gate
- candidate list persists in session and reloads
- grounding response can report multiple candidates
- `cargo check -p ob-poc`

## Phase G2 — Expand Fuzzy Search Over Real Values
### Goal
Use actual business values, not only current gateway/entity tables.

### Implementation
- Expand `discovery.search-entities` in `rust/src/domain_ops/discovery_ops.rs` to search across additional high-value tables/views where names/labels matter:
  - entity / legal entity names
  - client groups and aliases
  - CBU names / display names
  - funds / subfunds / share classes where applicable
  - documents by display label where applicable
  - deals by human labels / reference strings where applicable
- Preserve origin/source on each hit:
  - `source_table`
  - `source_kind`
- Add lightweight candidate type normalization so business object names can still map onto subject kinds used by Sem OS.

### Gate
- multi-table search returns heterogeneous candidates for ambiguous business names
- grounding artifacts show candidate provenance
- `cargo check -p ob-poc`

## Phase G3 — Stateful Research Verbs in the Live Path
### Goal
Move from simple entity lookup to operational state lookup.

### Implementation
- Expand `discovery.entity-context` to report real operational state for:
  - active onboarding
  - active deals
  - KYC status
  - open case state
  - screening state
  - document-gap state
  - linked structures / mandates where relevant
- Start using `discovery.cascade-research` in `try_semtaxonomy_path()` when:
  - there is no dominant candidate
  - the utterance is exploratory
  - multiple domain lanes are plausible
- Make `discovery.entity-relationships` part of normal grounding for ownership/control phrasing, not just fallback
- Use `discovery.inspect-data` and `discovery.search-data` only after candidate grounding and domain-state selection, not as generic read defaults

### Gate
- `entity-context` exposes actual deal/onboarding/KYC/document signals
- `cascade-research` is used in ambiguous grounding flows
- `cargo check -p ob-poc`

## Phase G4 — Composition Context Upgrade
### Goal
Feed the composer a real grounded working set.

### Implementation
- Extend `CompositionContext` in `rust/src/semtaxonomy/mod.rs` with:
  - `entity_candidates`
  - `active_states`
  - `domain_state_summaries`
  - `grounding_notes`
- `build_composition_request()` should include:
  - the top candidate set
  - the selected active entity if one is dominant
  - a compact summary of relevant workflow state
  - the Sem OS action surface
- Composition should reason over:
  - candidate ambiguity
  - state blockers
  - available business actions
  - only then choose runbook steps

### Gate
- composer input is visibly richer than `domain_scope + entity + verb_surface`
- trace artifacts show stateful grounding context
- `cargo check -p ob-poc`

## Phase G5 — Business-Verb-First Composition
### Goal
Use discovery verbs to research, not to finish the user task by default.

### Implementation
- In `compose_runbook()`:
  - prefer business verbs whenever grounding and state are sufficient
  - only emit discovery verbs when the user is explicitly asking to research/discover/inspect
- Add explicit composition branches for:
  - inventory/list reads
  - record/status reads
  - relationship/ownership reads
  - create/update/write with partial args
- If a write is plausible but incomplete:
  - emit a partial proposal with missing args/blockers
  - do not return `None`

### Gate
- `discovery.search-entities` and `discovery.inspect-data` are no longer dominant default outputs
- more rows convert from `no proposal` to partial or full business proposals
- `cargo check -p ob-poc`

## Phase G6 — Clarification and Grounded Alternatives
### Goal
When ambiguous, explain grounded options instead of silently failing.

### Implementation
- If multiple candidates remain plausible:
  - return a typed clarification payload with top grounded entities
  - include the relevant state summary per candidate
- If state blocks an intended action:
  - explain the blocker using actual state
  - suggest the nearest valid next action
- Make this visible in both backend response payloads and React UI

### Gate
- ambiguous utterances produce grounded alternatives instead of `<none>`
- blocked writes explain why
- `cargo check -p ob-poc`

## Phase G7 — Harness Modernization
### Goal
Measure the right thing.

### Implementation
- Extend `rust/tests/semtaxonomy_utterance_coverage.rs` to record:
  - grounded candidate count
  - whether active entity was dominant or ambiguous
  - whether state research was used
  - whether result was:
    - business verb
    - discovery verb
    - no proposal
    - partial proposal
- Add separate metrics:
  - grounding success
  - state-research success
  - business-verb composition success
  - final exact-verb success

### Gate
- harness explains where loss occurs
- not just final top-verb miss/no-miss

## Phase G8 — Domain Batches
### Goal
Implement the highest-yield state research first.

### Batch order
1. `cbu` / onboarding
2. `deal`
3. `screening`
4. `document`
5. `ubo` / ownership
6. `case` / `kyc`
7. `fund` / `struct`

Reason:
- these are the biggest miss clusters in the current coverage output

## Exit Criteria
This enhancement is successful when:
- `no proposal` is no longer the dominant failure class
- discovery verbs are used mainly for explicit research turns
- business verbs dominate normal operational utterances
- coverage rises materially above the current 2.27%
- harness output shows grounding/state usage as a positive contributor, not dead weight

## What This Plan Assumes
- yes, the lack of actual value/state grounding is a major contributor to the poor hit rate
- no, Sem OS alone is not enough for composition quality
- yes, dictionary/noun-index/verb-xref alone is too weak for the user’s natural business utterances
- no, this does not require changing the execution engine

## Recommended Implementation Order
1. G1
2. G2
3. G3
4. G4
5. G5
6. G6
7. G7
8. G8
