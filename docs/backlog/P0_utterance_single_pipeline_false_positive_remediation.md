# Utterance Single-Pipeline False-Positive Remediation

## Goal

Harden the utterance -> intent -> DSL pipeline so that:

- no alternative path can surface, score, or stage verbs outside Sem OS governance
- no path can degrade the original utterance into a premature local heuristic hit
- no path can present a hallucinated or weakly grounded DSL as a credible match
- Sem OS metadata coverage improves enough to raise governed utterance -> intent -> REPL DSL hit rate

## Concern Being Addressed

The main risk is not only policy leakage. The bigger operational failure mode is:

1. an alternative pipeline bypasses the unified orchestrator
2. it rewrites or locally interprets the utterance too early
3. it scores against a local action surface with incomplete governance metadata
4. it produces a false positive that looks grounded
5. the user is shown or allowed to confirm a hallucinated hit

This has already happened before in the product shape described by the user: presumptive early scoring, false positives, and hallucinated hits.

## Current Risk Summary

### P0. Active alternate utterance pipeline

`AgentService::try_semtaxonomy_path` runs before the orchestrator and is enabled by default unless `SEMTAXONOMY_ENABLED` is explicitly disabled.

Impact:

- utterances can be interpreted outside `handle_utterance()`
- Sem OS discovery state is not the first-class gatekeeper on this path
- telemetry written only by the orchestrator will not fully describe this path

### P0. Premature local scoring

The SemTaxonomy path builds a local `action_surface` from `discovery.graph-walk` and `discovery.available-actions`, then uses `step3_select_verb()` to choose a verb before the unified intent pipeline runs.

Impact:

- early scoring happens against a local surface
- the surface is not derived from `SessionVerbSurface`
- false positives can be presented as grounded discovery

### P0. Shadow metadata and policy drift

`discovery.available-actions` reads verb YAML directly and filters with local metadata rules.
`discovery.graph-walk` reads authored stategraph YAML directly and returns valid verbs outside the Sem OS DI boundary.

Impact:

- Sem OS and discovery metadata can drift
- DSL and action surfaces can expose verbs Sem OS would not select
- metadata coverage improvements in Sem OS will not fully benefit this path

### P1. Audit gap

Only orchestrator paths persist `intent_events`.

Impact:

- missing `allowed_verbs_fingerprint` does not currently prove a violation
- alternate-path false positives can evade the main audit stream

## Remediation Plan

## Phase 0: Stop False Positives First

### 0.1 Delete alternate utterance resolution code

- Rip out the pre-orchestrator SemTaxonomy utterance-selection path completely.
- Do not leave it behind as dormant code, a fallback branch, or a production-toggle path.
- Remove the code that can:
  - select verbs from free-form utterances
  - build executable DSL proposals from free-form utterances
  - stage pending mutations from free-form utterances
- If any SemTaxonomy logic remains, it must be refactored into pure context enrichment called from the orchestrator path only.

- Make the unified orchestrator the only code path allowed to select a verb from a free-form utterance.

Acceptance criteria:

- no free-form utterance can reach `step3_select_verb()` before `handle_utterance()`
- SemTaxonomy may enrich context, but may not choose a verb or produce executable DSL
- no production code path can re-enable alternate utterance scoring with an env var
- deleted path is absent from the build, not merely unreachable by configuration

### 0.2 Remove presumptive verb proposals from SemTaxonomy

- Stop returning `coder_proposal.dsl`, `verb_fqn`, or `ready_to_execute` from the pre-orchestrator path.
- Replace with one of:
  - clarification request
  - discovery bootstrap update
  - grounded context summary only

Acceptance criteria:

- SemTaxonomy responses never mark a change as ready to execute
- SemTaxonomy responses never stage a mutation directly
- the only place that stages executable DSL from utterance input is the orchestrator path

### 0.3 Add a hard invariant test

- Add a test that fails if any utterance path outside the orchestrator can:
  - select a verb
  - emit DSL
  - mark a response executable
  - populate pending mutation state

Acceptance criteria:

- CI fails if a second utterance-to-verb path is introduced

### 0.4 Remove dead flags and dead entrypoints

- Remove env flags, helper methods, and branches that existed only to support the deleted path.
- Remove dead comments and docs that imply SemTaxonomy still owns utterance scoring.
- Add a static guard test that fails if deleted symbols or deleted branches reappear.

Acceptance criteria:

- no dead feature flag remains that can revive the old path
- no dead helper remains that can be called to restore the old path
- CI fails if deleted path symbols are reintroduced

## Phase 1: Unify Discovery Governance

### 1.1 Replace local action surfaces with governed surfaces

- Refactor SemTaxonomy discovery to consume `SemOsContextEnvelope` and `SessionVerbSurface`.
- Treat `graph-walk` and `available-actions` as explanatory overlays, not authoritative allow-lists.
- Intersect any discovery-derived verbs with the current governed verb surface before display.

Acceptance criteria:

- every displayed action has a corresponding governed FQN in the current `SessionVerbSurface`
- discovery UIs cannot show a verb absent from the governed surface

### 1.2 Move discovery metadata authority into Sem OS

- Stop using YAML-loaded verb metadata as the source of truth for utterance-facing discovery.
- Expose subject kinds, phase tags, constellation links, DSL affordances, and argument prompts through Sem OS or a governed projection derived from it.

Acceptance criteria:

- a single governed metadata source drives:
  - discovery bootstrap
  - allowed verb surfaces
  - utterance clarification prompts
  - constellation and family suggestions

### 1.3 Add post-discovery intersection guards

- If `graph-walk` or stategraphs return verbs, intersect them with:
  - current `allowed_verbs_fingerprint`
  - current `surface_fingerprint`
- Log and drop all mismatches.

Acceptance criteria:

- mismatched discovery verbs are never shown as valid
- mismatches emit structured warnings and counters

## Phase 2: Reduce Utterance Degradation

### 2.1 Preserve original utterance through scoring

- Treat utterance rewrites, noun extraction, and heuristic domain inference as annotations, not replacements.
- Store:
  - raw utterance
  - rewritten utterance
  - domain hints
  - scoring path

Acceptance criteria:

- every scored outcome can be traced back to the raw utterance
- no rewrite can silently replace the user utterance in audit records

### 2.2 Introduce a "premature certainty" guard

- Do not allow a verb proposal when the score comes from:
  - local surface heuristics only
  - unresolved entity grounding
  - inferred domain scope without Sem OS confirmation
- Require clarification instead.

Acceptance criteria:

- unresolved scope plus heuristic-only ranking cannot yield a staged DSL
- low-grounding paths degrade to "needs clarification", never "ready"

### 2.3 Add negative tests for historical false positives

- Build fixtures from past hallucinated hits.
- Assert that they now result in:
  - no verb
  - clarification
  - or governed low-confidence handling

Acceptance criteria:

- regression suite covers known false-positive utterances

## Phase 3: Improve Sem OS Metadata Coverage

### 3.1 Fill DB and DSL metadata gaps required for governed matching

- Extend governed metadata to cover:
  - constellation membership
  - family and domain disambiguators
  - subject-kind constraints
  - phase and lifecycle affordances
  - argument prompts and missing-input semantics
  - canonical invocation phrases

Acceptance criteria:

- Sem OS can answer discovery/bootstrap and verb-surface questions without falling back to local YAML heuristics

### 3.2 Reconcile constellations, DSL, and runtime verb registry

- Produce a reconciliation report:
  - verbs in runtime registry but not in Sem OS metadata
  - verbs in Sem OS metadata but missing DSL/runtime bindings
  - constellations that cannot lead to governed verb narrowing

Acceptance criteria:

- every constellation used for utterance steering narrows to governed verbs
- no dead-end constellation remains in production metadata

### 3.3 Improve governed invocation coverage

- Add or normalize invocation phrases for verbs with poor governed recall.
- Ensure phrase coverage is tied to Sem OS metadata, not only cold-start embeddings.

Acceptance criteria:

- common utterances can hit governed verbs without relying on speculative local scoring

## Phase 4: Telemetry and Audit

### 4.1 Make missing fingerprints a first-class alert

- Emit telemetry for every utterance path, including rejected and alternate paths.
- Alert when any utterance-derived response includes:
  - a verb proposal
  - a DSL proposal
  - executable readiness
  - pending mutation
  and lacks `allowed_verbs_fingerprint` and `surface_fingerprint`

Acceptance criteria:

- dashboards clearly distinguish governed, blocked, and non-governed attempts

### 4.2 Record scoring provenance

- For every candidate or final verb, record:
  - scoring path
  - source tiers consulted
  - whether Sem OS governed the candidate set first
  - whether the outcome came from local heuristic discovery only

Acceptance criteria:

- false positives can be diagnosed from telemetry without code archaeology

## Implementation Order

1. Delete `try_semtaxonomy_path` as an utterance-to-verb path and remove revival flags/helpers.
2. Prevent SemTaxonomy from returning executable or stageable DSL.
3. Add invariant tests and static guards that enforce single utterance-to-verb selection.
4. Intersect all discovery surfaces with governed `SessionVerbSurface`.
5. Expand Sem OS metadata coverage for constellations, DSL prompts, and invocation coverage.
6. Add full-path telemetry for alternate-path attempts and blocked proposals.

## Definition of Done

- there is exactly one production utterance-to-verb selection path
- all user-visible verb proposals come from governed `SessionVerbSurface`
- discovery can enrich grounding but cannot hallucinate a hit
- alternate paths can no longer degrade the utterance into premature certainty
- telemetry can prove whether a proposed hit was governed or blocked
- deleted alternate-path code cannot be revived by configuration drift
