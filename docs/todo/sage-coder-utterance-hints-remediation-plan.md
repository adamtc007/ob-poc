# Sage/Coder Utterance Hint Preservation — Remediation Plan

## Goal

Preserve more of the user's utterance and multi-turn semantic hints through the
`utterance -> Sage -> Coder -> DSL` pipeline, without breaking the Sem OS
boundary:

- Sage still receives raw utterance + pre-resolution session context
- Coder still resolves from structured Sage output, not from direct NL parsing
- Sem OS remains the authority for semantic context and allowed verb surface
- ob-poc remains the DSL/runbook executor

Also make the Sage layer visible in the user experience:

- Sage must explain its understanding back to the user
- Sage must emit a structured handoff payload to the Coder/REPL side
- the UI must render both the Sage explanation and the proposed runbook/DSL state

## Current Gap

The raw utterance is preserved at session ingress and passed into Sage, but two
mechanical losses remain:

1. `SageContext` is underfed in `run_sage_stage(...)`
   - `entity_kind` is always `None`
   - `dominant_entity_name` is always `None`
   - `last_intents` is always empty
2. `OutcomeIntent` is too compressed for the Sage -> Coder boundary
   - Coder only sees `summary`, `subject`, `steps`, and a few top-level fields
   - utterance cues that do not fit those fields are lost

This weakens deterministic resolution, especially for:
- inventory/list reads
- scoped follow-up utterances
- explicit subject carry-forward
- write intents where the target name is present but not captured strongly

There is also a presentation gap:

3. Sage understanding is not represented as first-class response structure
   - backend chat responses are mostly plain `message`
   - pending mutation prompts are chat text, not a typed Sage explanation block
   - React chat renders message history, but not a distinct Sage understanding /
     Coder proposal boundary

## Non-Negotiable Invariants

1. Sage must continue to run before entity linking.
2. Sage must never see verb FQNs or scored verb candidates.
3. Coder must remain structured-input driven; no direct NL parsing from raw chat text.
4. Sem OS context and verb-surface filtering remain authoritative.
5. `cargo check -p ob-poc` must pass after every edit batch.
6. Sage must not execute DSL; it only explains intent and proposes the target runbook.
7. The Coder/REPL boundary must be explicit and typed, not inferred from chat prose.

## Phase H1 — Fill `SageContext` Properly

### H1.1 Populate `entity_kind`

Source from current session/scope state before Sage classification.

Candidate sources:
- current focused entity kind in session context
- scope entity type if already known from prior turns
- workflow-specific active object type where stable

Do not source from entity linking for the current utterance.

### H1.2 Populate `dominant_entity_name`

Provide Sage the best pre-resolution text mention already available from the
current session turn.

Allowed inputs:
- dominant text mention extracted earlier in the request path
- current scope display name if the user is clearly operating on scoped state

Do not inject UUID-resolved canonical entities from current-turn entity linking.

### H1.3 Populate `last_intents`

Carry forward a short rolling ledger of prior Sage outcomes.

Minimum shape:
- last 3-5 turns
- `(plane, domain_concept)` pairs

Preferred extension:
- include action category as a third field in a new typed struct rather than an
  unstructured tuple if the code change stays local and mechanical

### H1 Verification

- Unit tests for `run_sage_stage(...)` input construction
- Unit tests for fresh session vs scoped session behavior
- `cargo check -p ob-poc`

## Phase H2 — Add an Explicit Hint Ledger to Sage Output

### H2.1 Add `UtteranceHints` type

Create a new structured type owned by the Sage module.

Suggested fields:
- `raw_preview: String`
- `subject_phrase: Option<String>`
- `explicit_domain_terms: Vec<String>`
- `explicit_action_terms: Vec<String>`
- `scope_carry_forward_used: bool`
- `inventory_read: bool`
- `structure_read: bool`
- `create_name_candidate: Option<String>`

This is not a free-form dump. It must stay compact and typed.

### H2.2 Extend `OutcomeIntent`

Add:
- `hints: UtteranceHints`

Keep existing fields intact. Do not remove `summary`.

### H2.3 Emit hints from `DeterministicSage`

Populate hints from existing deterministic signals already being computed:
- clue words
- domain hints
- subject extraction
- structure/read rewrite signals
- inventory/list heuristics

### H2.4 Emit hints from `LlmSage`

Map LLM output into the same `UtteranceHints` shape.

If the LLM does not provide a field, default it safely rather than inventing it.

### H2 Verification

- Serialization/deserialization tests for new types
- DeterministicSage unit tests proving hints are populated
- LlmSage parse tests for safe defaulting
- `cargo check -p ob-poc`

## Phase H2.5 — Add Explicit Sage Explain + Handoff Types

### H2.5.1 Add `SageExplain`

Create a typed user-facing explanation payload owned by the Sage module.

Suggested fields:
- `understanding: String`
- `mode: String`
- `scope_summary: Option<String>`
- `confidence: String`
- `clarifications: Vec<String>`

Example semantics:
- `"So you want to list the deals for Allianz."`
- `"I am treating this as a read-only request."`

### H2.5.2 Add `CoderHandoff`

Create a typed Sage -> Coder instruction payload.

Suggested fields:
- `goal: String`
- `intent_summary: String`
- `required_outcome: String`
- `constraints: Vec<String>`
- `hint_terms: Vec<String>`
- `serve_safe: bool`
- `requires_confirmation: bool`

This is not user prose. It is the boundary contract for the deterministic
Coder/REPL side.

### H2.5.3 Extend `OutcomeIntent`

Add:
- `explain: SageExplain`
- `coder_handoff: CoderHandoff`

Keep `summary` as the compact internal summary. The new fields are for:
- user-facing replay
- explicit boundary crossing into Coder

### H2.5 Verification

- serde tests for `SageExplain` and `CoderHandoff`
- DeterministicSage tests proving:
  - read intents emit read-mode explanation
  - write intents emit confirmation-required handoff
- `cargo check -p ob-poc`

## Phase H3 — Make Coder Use the Hint Ledger

### H3.1 Extend structured scoring inputs

Update verb scoring to consume:
- top-level `OutcomeIntent`
- `OutcomeIntent.hints`

### H3.2 Apply hints only as bounded bias, not replacement logic

Examples:
- `inventory_read = true` biases `.list` over `.search`
- `structure_read = true` biases schema/describe verbs
- `create_name_candidate` helps required name-param completion
- `explicit_domain_terms` can lift domain matches where summary text is too compressed

The hint ledger must not bypass Sem OS surface filtering or polarity gating.

### H3.3 Improve required-arg assembly from hints

Use `create_name_candidate` and `subject_phrase` only for parameters that are:
- semantically aligned
- required by the chosen verb

Do not spray hints into optional params.

### H3.4 Consume `CoderHandoff`, not chat prose

Coder should consume the structured handoff contract from Sage, not parse:
- `message`
- `explain.understanding`
- any UI-formatted string

The handoff should be the authoritative “what outcome to achieve” bundle for
the deterministic DSL/runbook proposal.

### H3 Verification

- scoring tests showing bounded preference shifts
- arg assembly tests showing hint-assisted required param completion
- no regressions in polarity/plane gating
- `cargo check -p ob-poc`

## Phase H4 — Preserve Intent Memory Across Turns

### H4.1 Persist recent Sage outcomes in session state

Store only the minimal carry-forward record needed for the next turn:
- plane
- domain_concept
- action
- confidence

Do not persist raw verb candidates or raw scorer output.

### H4.2 Rehydrate into `SageContext.last_intents`

On each new utterance, pass the recent ledger into Sage.

### H4.3 Clear or downgrade carry-forward on explicit pivots

Examples:
- switching workflow
- new client scope
- clear domain change in utterance
- explicit structure vs instance pivot

### H4 Verification

- multi-turn tests:
  - read follow-up retains domain correctly
  - explicit pivot clears stale carry-forward
  - write confirmation flow does not poison next read turn
- `cargo check -p ob-poc`

## Phase H5 — Instrument the Loss Points

### H5.1 Add trace fields

Add explicit trace fields for:
- whether `dominant_entity_name` was supplied to Sage
- whether `last_intents` was supplied
- whether `OutcomeIntent.hints` was populated
- whether `SageExplain` was emitted
- whether `CoderHandoff` was emitted
- whether Coder used hint-assisted arg assembly

### H5.2 Add smoke coverage

Exercise at least these utterance families:
- scoped inventory read
- unscoped inventory read
- structure read in data management
- create with embedded subject name
- follow-up after a prior read
- follow-up after cancelled mutation

Store artifact output under `rust/target/` as with prior smoke passes.

### H5 Verification

- smoke report confirms hint fields are present in trace/log output
- `cargo check -p ob-poc`

## Phase H6 — Expose Sage Replay in Backend Chat Responses

### H6.1 Extend `AgentChatResponse`

Add typed fields for UI consumption.

Suggested fields:
- `sage_explain: Option<SageExplainDto>`
- `coder_proposal: Option<CoderProposalDto>`

Where:
- `SageExplainDto` mirrors the user-facing Sage replay
- `CoderProposalDto` carries the proposed DSL/runbook state without execution

Suggested `CoderProposalDto` fields:
- `verb_fqn: Option<String>`
- `dsl: Option<String>`
- `change_summary: Vec<String>`
- `requires_confirmation: bool`
- `ready_to_execute: bool`

### H6.2 Populate these fields in `AgentService`

Serve path:
- emit `sage_explain`
- emit `coder_proposal` only when there is a staged deterministic proposal worth showing

Delegate path:
- emit `sage_explain`
- emit `coder_proposal`
- keep plain `message` as fallback text, not the primary contract

### H6.3 Preserve backward compatibility

Do not remove:
- `message`
- `dsl_source`
- `can_execute`

The UI migration should be additive first.

### H6 Verification

- unit tests for JSON response shape
- integration test for serve response carrying Sage explanation
- integration test for delegate response carrying both explanation and proposal
- `cargo check -p ob-poc`

## Phase H7 — React UI Review and Render Contract

### H7.1 Review current UI consumption

Current state to account for:
- chat pages render message history from backend messages
- they do not render a distinct Sage explanation block
- they do not render a typed Coder proposal card

Affected areas likely include:
- `ob-poc-ui-react/src/api/chat.ts`
- `ob-poc-ui-react/src/features/chat/ChatPage.tsx`
- `ob-poc-ui-react/src/features/semantic-os/SemOsPage.tsx`
- related chat message types/stores

### H7.2 Add UI types

Extend React-side chat response/message types to support:
- `sageExplain`
- `coderProposal`

### H7.3 Add explicit visual separation

The UI should show:
1. user utterance
2. Sage understanding
3. proposed action / runbook / DSL state
4. confirmation state if mutation
5. execution result after confirmation

This boundary must be visible so the user can tell:
- what Sage understood
- what the system is proposing to do
- whether the system is still in read-only mode

### H7.4 Do not hide safe-read reasoning

For read-only/serve paths, show a compact explanation such as:
- `Sage understanding`
- `Read-only mode`
- `Serving current state`

### H7 Verification

- frontend typecheck/build
- UI smoke confirming:
  - serve response renders Sage explanation
  - delegate response renders proposal + confirmation state
  - post-confirmation response renders execution result distinctly

## Recommended Execution Order

1. H1 — feed `SageContext` properly
2. H2 — add `UtteranceHints` to `OutcomeIntent`
3. H2.5 — add Sage explain + Coder handoff types
4. H3 — use hints and handoff in Coder scoring and arg assembly
5. H4 — persist and rehydrate recent Sage outcomes
6. H5 — add traceability and smoke coverage
7. H6 — expose typed Sage/Coder payloads in backend chat responses
8. H7 — update React UI to render the boundary explicitly

## Files Expected to Change

Core Sage/Coder:
- `rust/src/sage/context.rs`
- `rust/src/sage/outcome.rs`
- `rust/src/sage/deterministic.rs`
- `rust/src/sage/llm_sage.rs`
- `rust/src/sage/coder.rs`
- `rust/src/sage/verb_resolve.rs`
- `rust/src/sage/arg_assembly.rs`
- `rust/src/sage/mod.rs`

Orchestration/session wiring:
- `rust/src/agent/orchestrator.rs`
- `rust/src/api/agent_service.rs`
- `rust/src/session/unified.rs`
- `rust/src/api/agent_routes.rs`

Frontend/UI:
- `ob-poc-ui-react/src/api/chat.ts`
- `ob-poc-ui-react/src/types/chat.ts`
- `ob-poc-ui-react/src/features/chat/ChatPage.tsx`
- `ob-poc-ui-react/src/features/semantic-os/SemOsPage.tsx`
- related chat stores/components as needed

Tests:
- Sage unit tests
- orchestrator/session carry-forward tests
- smoke harness scripts or ad hoc validation commands

## Success Criteria

1. Sage receives real session hints instead of placeholder `None`/empty values.
2. Coder can exploit structured hint data without directly parsing raw NL.
3. Follow-up utterances improve without widening fallback reliance.
4. The raw utterance remains preserved in session history and traceable through Sage.
5. Sem OS remains the context/verb-surface authority; execution still occurs through the deterministic DSL/runbook path.
6. The user can see Sage’s understanding before or alongside the proposed action.
7. The Coder/REPL side receives a typed handoff from Sage rather than inferring from prose.
8. The React UI visibly separates:
   - Sage understanding
   - proposed DSL/runbook
   - confirmation state
   - execution result
