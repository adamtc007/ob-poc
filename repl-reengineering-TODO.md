# REPL Re-Engineering — Implementation TODO

**Architecture spec:** `repl-reengineering-v3.md` (FROZEN — v3.0, February 2026)  
**Status:** Phase 0 — not started  
**Updated:** 2026-02-06

> Read the v3 spec before starting any task. Every task references a spec section.  
> Mark tasks `[x]` when done. Add commit hashes or notes inline.

---

## Phase 0 — Lay the Tracks (2–4 days)

**Goal:** Golden loop end-to-end with fake verbs and stub execution.  
**Spec:** §14 Phase 0

### 0.1 Sentence Generator
**File:** `rust/src/repl/sentence_generator.rs` (new)  
**Spec:** §9, §10.2

- [ ] Create `SentenceGenerator` struct with `Arc<RuntimeRegistry>`
- [ ] Implement `best_template()` — try `sentences.step[]` first, fall back to `search_phrases` (existing `invocation_phrases`), then `phrase_gen` output
- [ ] Implement `substitute()` — `{arg-name}` → value, unfilled → `"..."`
- [ ] Implement `format_list()` — Oxford comma ("IRS, CDS, and Equity")
- [ ] Handle map/nested args (e.g. `{markets}` as list)
- [ ] Unit tests: >20 verb/arg combinations covering all template sources
- [ ] Unit tests: edge cases — no templates, no args, all args missing, list of 1, list of 2, list of 5+
- [ ] Verify sub-1ms generation (template substitution only, no allocations in hot path)

### 0.2 Runbook Model
**File:** `rust/src/repl/runbook.rs` (new)  
**Spec:** §7

- [ ] Define `Runbook` struct with pack provenance fields (`pack_id`, `pack_version`, `pack_manifest_hash`)
- [ ] Define `RunbookEntry` with sentence, DSL, verb, args, labels
- [ ] Define `SlotProvenance` / `SlotSource` enum (§7.2)
- [ ] Define `ArgExtractionAudit` struct (§7.3)
- [ ] Define `RunbookStatus`, `EntryStatus`, `ExecutionMode`, `ConfirmPolicy` enums
- [ ] Define `RunbookEvent` for event-sourced audit trail
- [ ] Define `OutcomeEvent` enum (§7.7)
- [ ] Implement `Runbook::add_entry()`, `remove_entry()`, `reorder()`
- [ ] Implement `Runbook::entry_by_id()`, `entries_by_status()`
- [ ] Implement display: numbered sentence list (sentences only, no DSL — §7.4 visibility policy)
- [ ] Unit tests: add/remove/reorder entries, status transitions, provenance tracking

### 0.3 Pack Manifest Types
**File:** `rust/src/journey/pack.rs` (new)  
**Spec:** §4

- [ ] Define `PackManifest` struct matching YAML schema (§4.1)
- [ ] Define `PackQuestion` with `answer_kind` (list | boolean | string | entity_ref | enum), `options_source`, `prompt`
- [ ] Define `PackTemplate`, `TemplateStep`
- [ ] Define `PackCandidate` (for routing results)
- [ ] Define `RiskPolicy`, `SectionLayout`, `ProgressSignal`, `DefinitionOfDone`
- [ ] Implement YAML deserialiser (serde_yaml) for pack manifests
- [ ] Implement `PackManifest::manifest_hash()` — SHA-256 of serialised manifest
- [ ] Load from `config/packs/*.yaml` directory
- [ ] Unit test: deserialise example onboarding-request.yaml, verify all fields

### 0.4 Pack Handoff Types
**File:** `rust/src/journey/handoff.rs` (new)  
**Spec:** §7.6

- [ ] Define `PackHandoff` struct (source_runbook_id, target_pack_id, forwarded_context, forwarded_outcomes)
- [ ] Unit test: construct handoff from completed runbook

### 0.5 State Machine Types
**File:** `rust/src/repl/types.rs` (modify)  
**Spec:** §8.3

- [ ] Define new `ReplState` enum (7 variants: `ScopeGate`, `JourneySelection`, `InPack`, `Clarifying`, `SentencePlayback`, `RunbookEditing`, `Executing`)
- [ ] `InPack` carries `pack_id`, `required_slots_remaining`, `last_proposal_id`
- [ ] Define `ConfirmPolicy` enum (Always | QuickConfirm | PackConfigured)
- [ ] Simplify `UserInput` — collapse typed selection variants into `Message` + `Confirm` / `Reject` / `Edit`
- [ ] Keep old `ReplState` behind `#[cfg(not(feature = "vnext-repl"))]` for coexistence
- [ ] Unit tests: state construction, transition helpers

### 0.6 Orchestrator Skeleton
**File:** `rust/src/repl/orchestrator.rs` (modify)  
**Spec:** §8.2, §8.3

- [ ] New `OrchestratorV2` (or behind feature flag on existing) with new state machine
- [ ] Dispatch skeleton: `ScopeGate` → `JourneySelection` → `InPack` → `Clarifying` → `SentencePlayback` → `RunbookEditing` → `Executing`
- [ ] Wire `SentenceGenerator` into playback path
- [ ] Wire `Runbook` model — create on journey selection, append on confirm
- [ ] Stub executor: accepts runbook, marks all entries `Completed`, returns fake results
- [ ] Integration test: full golden loop — scope → pack select → Q/A → sentence → confirm → execute → done
- [ ] Integration test: direct DSL input → parsed → sentence generated → added to runbook

### 0.7 Starter Pack YAML (Stubs)
**Files:** `config/packs/onboarding-request.yaml`, `config/packs/book-setup.yaml`, `config/packs/kyc-case.yaml`  
**Spec:** §4.1, §4.4

- [ ] Create onboarding-request.yaml with full schema (use §4.1 example as base)
- [ ] Create book-setup.yaml with Lux SICAV + UK OEIC templates
- [ ] Create kyc-case.yaml with new case + renewal templates
- [ ] Verify all three deserialise correctly via pack loader

### Phase 0 — Done Checklist
- [ ] `cargo test` passes with all new modules
- [ ] Golden loop integration test passes end-to-end
- [ ] Sentence generator produces correct English for >20 verb/arg combos
- [ ] Runbook model tracks provenance and audit events
- [ ] Pack manifests load from YAML and hash correctly

---

## Phase 1 — Journey Packs + Starter Packs + Sentence Library (Week 1–2)

**Goal:** 3 real packs, real verb wiring, real playback.  
**Spec:** §14 Phase 1

### 1.1 Pack Router
**File:** `rust/src/journey/pack.rs` (extend) or `rust/src/repl/intent_service.rs` (new)  
**Spec:** §4.2

- [ ] Index pack `invocation_phrases` in `HybridVerbSearcher` with `pack:` prefix
- [ ] Implement `PackRouter::route()` — returns `PackRouteOutcome` (Matched | Ambiguous | NoMatch)
- [ ] Prefer pack matches over verb matches when in `JourneySelection` state
- [ ] Handle explicit user selection: "Use the onboarding journey"
- [ ] Fallback: generate question listing available packs
- [ ] Integration test: route "onboard Allianz" → Onboarding Request pack

### 1.2 Template Instantiation
**File:** `rust/src/journey/template.rs` (new)  
**Spec:** §4.1 templates section

- [ ] Parse template `steps[]` with arg substitution (`{context.*}`, `{answers.*}`)
- [ ] Handle `repeat_for` — expand single template step into N runbook entries
- [ ] Handle `when` conditions — include/exclude steps based on answers
- [ ] Handle `execution_mode` per step (Sync | Durable | HumanGate)
- [ ] Set `SlotProvenance` on each arg: `UserProvided` vs `TemplateDefault` vs `InferredFromContext`
- [ ] Record `template_id` and `template_hash` on the Runbook
- [ ] Unit tests: standard-onboarding template with 3 products → 5 entries
- [ ] Unit tests: minimal-onboarding template → 3 entries (no trading matrix)

### 1.3 Pack-Level Playback
**File:** `rust/src/journey/playback.rs` (new)  
**Spec:** §10.3

- [ ] Implement `PackPlayback::summarize()` using `pack_summary_template`
- [ ] Implement `PackPlayback::chapter_view()` using `section_layout`
- [ ] Handle template variables: `{context.*}`, `{answers.*}`, `| count`, `| join`
- [ ] Handle conditionals: `{if answers.trading_matrix}...{/if}`
- [ ] Unit tests: onboarding pack with 3 products → correct summary paragraph
- [ ] Unit tests: chapter grouping matches section_layout verbs

### 1.4 Verb Sentence Templates (Starter Pack Verbs)
**Spec:** §9

- [ ] Add `sentences.step[]`, `sentences.summary[]`, `sentences.clarify` to all verbs used by starter packs
- [ ] Verbs to cover: `cbu.assign-product`, `trading-profile.create-matrix`, `trading-profile.add-counterparty`, `trading-profile.add-instrument`, `entity.ensure-*` (all variants), `cbu.create-*`, `cbu.assign-manco`, `kyc.open-case`, `kyc.request-docs`, `kyc.review-gate`, `onboarding.create-request`
- [ ] Verify sentence generator produces correct English for each verb with sample args
- [ ] Verify `sentences.clarify` prompts read naturally for each missing arg

### 1.5 Arg Extraction Audit Wiring
**Spec:** §7.3

- [ ] When `IntentService.extract_args()` returns, capture `ArgExtractionAudit` (model_id, prompt_hash, user_input, extracted_args, confidence, timestamp)
- [ ] Store audit on the `RunbookEntry` created from that extraction
- [ ] Verify audit persists through runbook serialisation

### 1.6 Pack Question Policy in Orchestrator
**Spec:** §4.1 question policy, §8.5

- [ ] `InPack` state tracks `required_slots_remaining` from pack's `required_questions`
- [ ] Orchestrator asks required questions in order, skipping any already answered from user's initial message
- [ ] Stop rules: proceed to template when all required answered OR template fillable with defaults
- [ ] Handle `ask_when` conditions on optional questions
- [ ] Integration test: "Onboard Allianz Lux CBU book" → asks products + trading matrix → proposes template

### 1.7 Confirmation Policy Wiring
**Spec:** §8.4

- [ ] Read `ConfirmPolicy` from verb config (default `Always`)
- [ ] Override from pack template step if specified
- [ ] `QuickConfirm`: sentence played back with auto-proceed (shorter pause prompt)
- [ ] All policies still create a runbook entry — no bypass
- [ ] Integration test: navigation verb with QuickConfirm still appears in runbook

### Phase 1 — Definition of Done
**Spec:** §14 Phase 1 DoD

- [ ] User says "Onboard Allianz Lux CBU book" → routes to Onboarding pack
- [ ] System asks < 5 slot-filling questions
- [ ] Produces readable plan (pack summary + chapters + step sentences)
- [ ] Runbook editable (remove/reorder/edit args via orchestrator)
- [ ] Execution requires explicit "run"
- [ ] Durable/human-gate step shows parked status
- [ ] Pack version and template provenance recorded in runbook
- [ ] Arg extraction audit present on LLM-derived entries

---

## Phase 2 — Verb Registry vNext + Pipeline Consolidation (Week 3–4)

**Goal:** Separated sentence fields on all verbs. Three pipelines → one IntentService.  
**Spec:** §14 Phase 2

### 2.1 Verb Registry Schema Changes
**File:** `rust/crates/dsl-core/src/config/types.rs` (modify)  
**Spec:** §9.2

- [ ] Rename `invocation_phrases` → `search_phrases` (keep `invocation_phrases` as serde alias for backwards compat)
- [ ] Add `sentences` field: `VerbSentences { step: Vec<String>, summary: Vec<String>, clarify: HashMap<String, String> }`
- [ ] Add `confirm_policy` field per verb (default `Always`)
- [ ] Add `journey_tags` field (which packs can use this verb)
- [ ] Add `required` vs `optional` classification on verb args
- [ ] Update YAML deserialiser
- [ ] Migration: existing verb YAMLs continue to work (search_phrases auto-populated from invocation_phrases alias)

### 2.2 IntentService (Pipeline Consolidation)
**File:** `rust/src/repl/intent_service.rs` (new)  
**Spec:** §10.1

- [ ] Create `IntentService` struct with scope_resolver, pack_router, verb_searcher, arg_extractor, sentence_generator
- [ ] Implement Phase 0: `try_resolve_scope()` → `ScopeOutcome`
- [ ] Implement Phase 1: `route_pack()` → `PackRouteOutcome`
- [ ] Implement Phase 2: `match_verb()` (filters by pack `allowed_verbs`) → `VerbMatchOutcome`
- [ ] Implement Phase 3: `extract_args()` (LLM call) → `ArgExtractionOutcome`
- [ ] Implement Phase 4: `generate_sentence()` (template, no LLM) → `String`
- [ ] Implement Phase 5: `assemble_dsl()` → `Result<String>`
- [ ] Each phase: pure function, returns outcome enum, no side effects on session state
- [ ] Lexicon pipeline as optional fast path: if grammar matches, skip LLM arg extraction
- [ ] Integration test: full pipeline scope → pack → verb → args → sentence → DSL

### 2.3 Orchestrator Wiring to IntentService
- [ ] Replace `HybridIntentMatcher` calls with `IntentService` phase calls
- [ ] Orchestrator calls phases explicitly, inspects outcomes, drives state transitions
- [ ] Verify all existing golden loop tests still pass

### 2.4 Sentence Templates for All Exposed Verbs
- [ ] Populate `sentences.step[]` for every verb exposed via starter packs
- [ ] Populate `sentences.clarify` for every required arg on those verbs
- [ ] Verify sentence generator never falls back to search_phrases for pack verbs
- [ ] Verify playback quality: run all acceptance scenarios, review sentence output

### 2.5 Deprecate Old Pipeline Entry Points
- [ ] Mark `IntentPipeline::process()` / `process_with_scope()` as `#[deprecated]`
- [ ] Mark `HybridIntentMatcher::match_intent()` as `#[deprecated]`
- [ ] MCP handlers delegate to `IntentService` (behind feature flag initially)

### Phase 2 — Done Checklist
- [ ] Single `IntentService::process()` entry point handles all user input
- [ ] All pack verbs use `sentences.step[]` (no search_phrase fallback)
- [ ] Validation errors become clarification questions via `sentences.clarify`
- [ ] Old pipeline methods deprecated but still callable

---

## Phase 3 — Proposal Engine (Week 5)

**Goal:** Deterministic step proposals from runbook + message + pack.  
**Spec:** §14 Phase 3

- [ ] Create proposal engine module (in `intent_service.rs` or separate `rust/src/repl/proposal.rs`)
- [ ] Input: current runbook state + user message + pack manifest + journey context
- [ ] Output: ranked list of `StepProposal` with verb, args, evidence, "why this verb"
- [ ] Template fast path: match user message to pack template, fill skeleton
- [ ] Verb search fallback: semantic match within pack's `allowed_verbs`
- [ ] Reproducibility: same inputs → same output (no random, no non-deterministic LLM in ranking)
- [ ] Include evidence string per proposal ("matches template 'standard-onboarding' step 3")
- [ ] Integration test: "add IRS product" within onboarding pack → proposes `cbu.assign-product`
- [ ] Integration test: ambiguous input → returns multiple proposals ranked

---

## Phase 4 — Runbook Editing UX + Conversational Clarification (Week 6)

**Goal:** Co-authoring. Runbook = the thing, chat = the assistant.  
**Spec:** §14 Phase 4

### 4.1 Runbook Editing Operations
- [ ] Accept proposal → add to runbook, update status to Confirmed
- [ ] Reject proposal → discard, ask "what would you prefer?"
- [ ] Edit step args → modify entry, re-generate sentence, re-playback
- [ ] Disable step → mark Parked, excluded from execution
- [ ] Reorder steps → update sequence numbers, re-validate DAG
- [ ] Remove step → delete entry, re-number
- [ ] All edits logged as `RunbookEvent` in audit trail

### 4.2 Conversational Clarification
- [ ] Generate clarification questions from `sentences.clarify` templates
- [ ] Multi-turn loop within `Clarifying` state
- [ ] Entity resolution via conversation ("Did you mean Allianz GmbH or Allianz SE?")
- [ ] Scope resolution via conversation ("I see three Allianz groups — Luxembourg, Ireland, or Global?")
- [ ] Integration test: ambiguous entity → clarification → correct resolution

---

## Phase 5 — Durable Execution + Human Gates + Wiring (Week 7–8)

**Goal:** Real execution modes, pack handoff, full API surface.  
**Spec:** §14 Phase 5

### 5.1 Execution Modes
- [ ] `Sync`: execute immediately, return result
- [ ] `Durable`: start async task, park entry, resume on signal
- [ ] `HumanGate`: park entry, emit notification, await approval event
- [ ] Status reporting per step during execution
- [ ] DAG-aware phased execution (reuse `DagAnalyzer`)

### 5.2 Pack Handoff
- [ ] Implement `PackHandoff` creation from completed runbook
- [ ] Forward context (client_group, cbu_name, book_id) to new runbook
- [ ] Forward relevant outcomes (entity IDs created in previous pack)
- [ ] Mark forwarded slots as `SlotSource::CopiedFromPrevious`
- [ ] Integration test: Book Setup → Onboarding Request handoff

### 5.3 API Wiring
- [ ] Update `api/repl_routes.rs` to new orchestrator (behind feature flag)
- [ ] Update `mcp/handlers/runbook.rs` to new runbook model
- [ ] MCP tools (`process-intent`, `runbook-stage`, `runbook-run`) delegate to unified path
- [ ] WebSocket event stream for execution progress
- [ ] Integration test: API round-trip — create session → scope → pack → steps → execute

### Phase 5 — Done Checklist
- [ ] Runbook parks for durable steps and resumes on signal
- [ ] Human-gate steps show parked status and accept approval
- [ ] Pack handoff forwards context correctly
- [ ] API surface works end-to-end

---

## Phase 6 — Decommission Old Trapdoors (Week 9)

**Goal:** Parity matrix, delete old paths.  
**Spec:** §14 Phase 6

### 6.1 Parity Matrix
- [ ] Document every old flow → new journey mapping
- [ ] Verify all acceptance scenarios (§18 A–F) pass on new path
- [ ] Verify MCP tools work via new path

### 6.2 Deprecation
- [ ] Remove `session/dsl_sheet.rs` (or move to `_deprecated/`)
- [ ] Remove `repl/service.rs` (RunbookService)
- [ ] Remove old `ReplState` variants (`Idle`, `IntentMatching`, `DslReady`)
- [ ] Remove all six `ClarifyingState` variants
- [ ] Remove `can_auto_execute` trapdoor
- [ ] Remove `is_direct_dsl` bypass (direct DSL now flows through sentence path)
- [ ] Remove `#[cfg(not(feature = "vnext-repl"))]` guards — vnext is default
- [ ] Remove `#[deprecated]` markers on old pipeline methods, delete the methods

### 6.3 Cleanup
- [ ] `cargo test` — all tests pass
- [ ] `cargo clippy` — no warnings on new modules
- [ ] Dead code scan — no unused functions in repl/, journey/, mcp/
- [ ] Update README / developer docs to reference v3 architecture

---

## Cross-Cutting Concerns (Any Phase)

### Testing
- [ ] Unit tests for every new module (sentence_generator, runbook, pack, template, playback, handoff, intent_service)
- [ ] Integration tests for each golden loop scenario (§18 Scenarios A–F)
- [ ] Regression tests: ensure old API consumers aren't broken during coexistence

### Documentation
- [ ] Inline doc comments on all public types and methods
- [ ] Update `ARCHITECTURE.md` or equivalent to reference v3 spec
- [ ] Pack authoring guide: how to write a new pack YAML

### Performance
- [ ] Benchmark sentence generation (<1ms target)
- [ ] Benchmark end-to-end intent → sentence (<2s target)
- [ ] Profile pack loading at startup

---

## Components NOT to Touch

**Spec:** §15 — these are solid, wrap don't rewrite:

- `mcp/verb_search.rs` (HybridVerbSearcher) — wrap for pack routing
- `mcp/scope_resolution.rs` (ScopeResolver) — call from IntentService
- `repl/dag_analyzer.rs` (DagAnalyzer) — use for execution ordering
- `dsl-core/config/phrase_gen.rs` — sentence fallback only
- `ob-semantic-matcher/` crate — embedding infrastructure
- Entity resolution (`EntityArgResolver`) — wrap existing

---

## Quick Reference: File → Phase Map

| File | Phase | Action |
|---|---|---|
| `rust/src/repl/sentence_generator.rs` | 0.1 | Create |
| `rust/src/repl/runbook.rs` | 0.2 | Create |
| `rust/src/journey/pack.rs` | 0.3 | Create |
| `rust/src/journey/handoff.rs` | 0.4 | Create |
| `rust/src/repl/types.rs` | 0.5 | Modify |
| `rust/src/repl/orchestrator.rs` | 0.6 | Modify |
| `config/packs/*.yaml` | 0.7 | Create |
| `rust/src/journey/template.rs` | 1.2 | Create |
| `rust/src/journey/playback.rs` | 1.3 | Create |
| Verb YAML configs | 1.4 | Modify |
| `rust/crates/dsl-core/src/config/types.rs` | 2.1 | Modify |
| `rust/src/repl/intent_service.rs` | 2.2 | Create |
| `rust/src/api/repl_routes.rs` | 5.3 | Modify |
| `rust/src/mcp/handlers/runbook.rs` | 5.3 | Modify |
| `rust/src/session/dsl_sheet.rs` | 6.2 | Delete |
| `rust/src/repl/service.rs` | 6.2 | Delete |
| `rust/src/mcp/intent_pipeline.rs` | 6.2 | Delete |
| `rust/src/repl/intent_matcher.rs` | 6.2 | Delete |
