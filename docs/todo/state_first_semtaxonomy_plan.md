# State-First SemTaxonomy Plan

## Objective

Replace the current `utterance -> compose verb` bias with:

1. ground the real object in scope
2. load its current operational state
3. derive valid next actions from state + Sem OS
4. decide whether to:
   - explain current state
   - explain options to move forward
   - compose an executable runbook

This is a refactor of the new SemTaxonomy path, not a new execution architecture.

## Core Principle

Users usually mean one of three things:

1. `state_now`
   - "what is this?"
   - "what documents are missing?"
   - "who owns this?"
   - "what deals does Allianz have?"

2. `options_forward`
   - "what can I do next?"
   - "how do I progress this onboarding?"
   - "what is blocking this?"

3. `requested_action`
   - "create a new CBU"
   - "run sanctions screening"
   - "add this entity to the CBU"

The system should determine which of these applies only after grounding real state.

## Why This Change

The current low hit rate is dominated by `no proposal`, not wrong proposals.

That means:

- grounding is too thin
- state research is too shallow
- composition happens before the system knows enough about the live object

Sem OS tells us what is allowed.
Live business state tells us what is relevant now.
Both are required.

## Target Flow

1. `utterance`
2. `candidate grounding`
   - fuzzy search across:
     - entities
     - client groups
     - client group aliases
     - CBUs
     - deals
     - document titles
3. `operational anchor selection`
   - choose whether the turn is anchored on:
     - `cbu`
     - `entity`
     - `deal`
     - `case`
     - `document`
   - prefer `cbu` for onboarding-state questions when a linked CBU exists
4. `state hydration`
   - load:
     - onboarding state
     - deal state
     - KYC/case state
     - screening state
     - document gaps
     - parties / relationships
     - linked entities
5. `intent mode detection`
   - `state_now`
   - `options_forward`
   - `requested_action`
6. `next action derivation`
   - derive action surface from:
     - current state
     - Sem OS allowed verbs
     - preconditions / phase tags / subject kinds
7. `response selection`
   - state explanation only
   - options explanation
   - composed runbook + confirmation

## Decision Rules

### R1. Anchor Selection

- If a matched entity is linked to a `CBU`, and the utterance is about onboarding, documents, screening, parties, progress, or blockers:
  - anchor on the `CBU`
- If a matched entity is not linked to a `CBU`:
  - do not pretend onboarding state exists
  - mark it as `candidate_for_cbu = true`
- If the utterance is pure entity research:
  - anchor on the entity directly

### R2. CBU Rule

`CBU` is the onboarding unit.

That means:

- onboarding state lives on or through `CBU`
- linked entities provide structural backing
- user-facing workflow status should be read from `CBU` context first

### R3. No Discovery Verbs In User Runbooks

`discovery.*` verbs are research tools only.

They may be used internally during grounding and state research, but they must not appear in composed user-facing business runbooks except for explicit discovery questions.

### R4. Missing State Is A Valid Outcome

If no linked `CBU` exists:

- read onboarding question:
  - explain that no onboarding unit exists yet
- write onboarding question:
  - propose the create/add-to-CBU path

Never return `None` silently.

## Execution Plan

### S1. Grounding Model Upgrade

Files:

- `rust/src/domain_ops/discovery_ops.rs`
- `rust/src/semtaxonomy/mod.rs`
- `rust/src/api/agent_service.rs`

Work:

- carry full candidate set forward, not just first hit
- annotate each hit with:
  - `source_kind`
  - `linked_cbu_ids`
  - `is_onboarding_member`
  - `candidate_for_cbu`
- choose dominant anchor with explicit rules, not first result

Gate:

- `cargo check -p ob-poc`
- seeded grounding cases still pass

### S2. State Hydration Upgrade

Files:

- `rust/src/domain_ops/discovery_ops.rs`
- `rust/src/semtaxonomy/mod.rs`

Work:

- deepen `entity-context` / state summaries for:
  - `cbu`
  - `deal`
  - `case`
  - `document`
  - `entity`
- add explicit fields:
  - `onboarding_present`
  - `blocked_by`
  - `missing_documents`
  - `screening_pending`
  - `active_deals`
  - `next_action_candidates`

Gate:

- `cargo check -p ob-poc`
- seeded state assertions pass

### S3. Intent Mode Detection

Files:

- `rust/src/semtaxonomy/mod.rs`
- `rust/src/api/agent_service.rs`

Work:

- replace direct verb-first heuristics with mode detection:
  - `state_now`
  - `options_forward`
  - `requested_action`
- use utterance cues only after state is known

Gate:

- `cargo check -p ob-poc`
- focused mode-detection tests pass

### S4. Next-Action Derivation

Files:

- `rust/src/semtaxonomy/mod.rs`
- `rust/src/domain_ops/discovery_ops.rs`

Work:

- derive available next actions from:
  - state
  - Sem OS action surface
  - subject kind
  - phase tags
  - harm class / action class
- return ranked action options before composition

Gate:

- `cargo check -p ob-poc`
- state -> options tests pass

### S5. Composition Refactor

Files:

- `rust/src/semtaxonomy/mod.rs`
- `rust/src/api/agent_service.rs`

Work:

- compose business verbs only from ranked next actions
- for `state_now`:
  - prefer explanation or business read runbook
- for `options_forward`:
  - explain top action options
- for `requested_action`:
  - compose runbook if valid
  - otherwise explain blocker / missing prerequisite

Gate:

- `cargo check -p ob-poc`
- seeded composition coverage improves

### S6. Seeded Capability Expansion

Files:

- `rust/tests/support/semtaxonomy_seed.rs`
- `rust/tests/semtaxonomy_seeded_capability.rs`

Work:

- expand fixtures for:
  - ownership / UBO
  - documents
  - case / KYC
  - composite CBU with linked entities
- add assertions for:
  - state questions
  - options questions
  - action questions

Gate:

- `cargo check -p ob-poc`
- seeded capability harness passes

### S7. Harness Modernization

Files:

- `rust/tests/semtaxonomy_utterance_coverage.rs`

Work:

- break the score into:
  - grounding success
  - anchor selection success
  - state hydration success
  - action option success
  - final runbook success
- stop using only exact top-verb as the single signal

Gate:

- `cargo check -p ob-poc`
- harness emits stage metrics

## Immediate Batch Order

1. `S1` grounding model upgrade
2. `S2` state hydration upgrade
3. `S6` seeded capability expansion
4. `S3` intent mode detection
5. `S4` next-action derivation
6. `S5` composition refactor
7. `S7` harness modernization

## Success Criteria

- no silent `no proposal` for grounded turns
- no `discovery.*` in user-facing business runbooks
- onboarding questions anchored on `CBU` when linked
- unlinked entities explained as candidates for CBU creation/addition
- seeded harness passes across:
  - cbu
  - deal
  - document
  - kyc/case
  - ubo/ownership
- live harness shows material reduction in `no proposal`

## Non-Goals

- replacing DSL execution
- replacing REPL/runbook execution
- changing Sem OS governance ownership
- deleting discovery verbs

