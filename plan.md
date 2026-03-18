# Sem OS Universe and Leaky-Pipe Remediation Plan

Status: Draft for peer review

Objective:
- Remove the legacy utterance-to-DSL leak where Sage or downstream agent paths can discover verbs from the raw DSL/runtime registry without going through Sem OS grounding.
- Introduce an explicit empty-session discovery universe above constellation selection.
- Keep the Sem OS architecture intact by adding a top-level navigation layer that maps down to constellation families and concrete constellations.

Core target flow:
- `session start -> Sage bootstrap/discovery -> Sem OS universe resolution -> constellation family narrowing -> constellation selection -> Sem OS grounded graph/action surface -> REPL edit/execute`

Non-goals:
- Replace constellations, state machines, or the grounded action surface model.
- Introduce a second planner parallel to Sem OS.
- Preserve graceful degradation to unconstrained runtime-verb search for agent utterances.

## 1. Problem Statement

Current issue:
- Sem OS is not yet the sole source of action discovery.
- Chat, MCP, and REPL still use legacy `HybridVerbSearcher` and `IntentPipeline` paths over the broad DSL/runtime verb inventory.
- Sem OS currently acts mostly as an `allowed_verbs` pre-filter, not as the authoritative source of executable grounded actions.

Leak sources already identified:
- Legacy direct SemReg fallback when `SemOsClient` is absent.
- MCP `verb_search` falls back to unconstrained search when Sem OS is unavailable.
- REPL v2 falls back to unconstrained matching when Sem OS is unavailable.
- Agent/MCP informational surfaces still expose full runtime verb inventories.
- Sidecar discovery utilities still use `discover_dsl()` / `match_intent()` over raw verb contracts.

Architectural gap:
- The new `grounded_action_surface` exists in Sem OS core, but the agent stack does not consume it as the primary action surface.
- The empty session state is not modeled explicitly; `TaskId + entity_kind=None` is too broad and can produce underconstrained candidate/action surfaces.

## 2. Desired Model

### Session phases

Target session phases:
- `empty_session`
- `universe_navigation`
- `family_narrowing`
- `constellation_selection`
- `grounded_instance`
- `action_selection`
- `repl_edit_execute`
- `graph_refresh`

### Responsibility split

Sage owns:
- objective capture
- target/client discovery
- entity resolution/disambiguation
- new-vs-existing classification
- domain/universe navigation
- constellation-family narrowing

Sem OS owns:
- universe interpretation
- constellation selection readiness
- grounded instance graph hydration
- node/slot state evaluation
- valid/blocked action computation
- deterministic DSL candidate production

REPL owns:
- downstream edit/execute only after Sem OS grounding

## 3. Empty Session Universe Model

The empty session should not mean:
- all constellations
- all actions
- all verbs

It should mean:
- a top-level universe of work domains that cluster constellation families

Target conceptual hierarchy:
- `Universe -> Domain Cluster -> Constellation Family -> Constellation -> Instance Graph`

Examples of domain clusters:
- `kyc_onboarding`
- `cbu_management`
- `deal_management`
- `document_governance`
- `service_configuration`
- `screening_investigation`
- `ownership_structure_work`
- `entity_maintenance`

Examples of constellation families:
- `fund_onboarding`
- `operating_cbu`
- `deal_lifecycle`
- `document_compliance`
- `ownership_graph`
- `service_readiness`

Rule:
- Empty-session Sem OS responses may return ranked domains/families/questions.
- They must not return broad executable action surfaces.

## 4. Proposed Sem OS Data Model

### New authored object types

Add first-class Sem OS defs:
- `UniverseDef`
- `UniverseDomainDef`
- `ConstellationFamilyDef`

These are conceptually taxonomy/navigation objects, but should be authored as explicit defs rather than overloading plain taxonomy nodes.

### `UniverseDef`

Fields:
- `universe_id`
- `name`
- `description`
- `version`
- `domains`
- `transitions`
- `default_entry_domain`

### `UniverseDomainDef`

Fields:
- `domain_id`
- `label`
- `description`
- `objective_tags`
- `utterance_signals`
- `candidate_entity_kinds`
- `candidate_constellation_families`
- `required_grounding_inputs`
- `entry_questions`
- `allowed_discovery_actions`

### `ConstellationFamilyDef`

Fields:
- `family_id`
- `label`
- `description`
- `domain_id`
- `selection_rules`
- `constellation_refs`
- `candidate_jurisdictions`
- `candidate_entity_kinds`
- `grounding_threshold`

### `ConstellationRef`

Fields:
- `constellation_id`
- `label`
- `description`
- `jurisdiction`
- `entity_kind`
- `triggers`

### `UtteranceSignal`

Fields:
- `signal_type`
- `pattern`
- `weight`

### `GroundingInput`

Fields:
- `key`
- `label`
- `required`
- `input_type`

### `EntryQuestion`

Fields:
- `question_id`
- `prompt`
- `maps_to`
- `priority`

### `GroundingThreshold`

Fields:
- `min_required_inputs`
- `requires_entity_instance`
- `allows_draft_instance`

## 5. New Sem OS Resolution Stages

Add an explicit stage to context resolution or a pre-resolution pass.

Recommended shape:
- `resolve_universe()`
- `resolve_context()`

### `UniverseResolutionRequest`

Fields:
- `session_id`
- `utterance`
- `sage_summary`
- `objective`
- `actor`
- `known_inputs`

### `UniverseResolutionResponse`

Fields:
- `universe_id`
- `matched_domains`
- `matched_families`
- `matched_constellations`
- `missing_inputs`
- `entry_questions`
- `grounding_readiness`
- `next_stage`

### Grounding readiness states
- `not_ready`
- `family_ready`
- `constellation_ready`
- `grounded`

Rule:
- `grounded_action_surface` is only valid once `grounding_readiness == grounded`.

## 6. Required Runtime Behavior Changes

### 6.1 Empty session behavior

Sem OS must treat the neutral `TaskId` session as discovery-only unless enough grounding inputs exist.

Required fixes:
- do not compute grounded actions for empty/unidentified sessions
- do not accept score `0` as sufficient to choose a constellation slot in empty-state discovery
- return clarifications and candidate families instead

### 6.2 Sage behavior

Sage must stop selecting raw verbs.

Sage outputs should be limited to:
- objective summary
- target/client hints
- entity kind hints
- new-vs-existing
- jurisdiction hints
- candidate domain/family hints
- missing clarifications

Forbidden in Sage bootstrap mode:
- raw DSL verb selection
- full registry search as authority
- executable DSL generation

### 6.3 Agent orchestration behavior

After grounding:
- consume Sem OS `grounded_action_surface.valid_actions`
- consume Sem OS `grounded_action_surface.dsl_candidates`
- stop using `HybridVerbSearcher` / `IntentPipeline` as the primary action selector

### 6.4 REPL behavior

REPL must become downstream-only:
- no unconstrained verb search when Sem OS is unavailable
- no “all verbs available” posture for grounded work
- only allow edits against Sem OS-approved actions/candidates

## 7. Concrete Code Remediation Steps

### Phase 1: Safety gates

1. Make `SemOsClient` mandatory for agent utterance flows.
2. Remove direct legacy SemReg fallback in `rust/src/agent/orchestrator.rs`.
3. Remove runtime mode where `SEM_OS_MODE` unset means “legacy direct sem_reg”.
4. Change agent/repl behavior from graceful degradation to fail-closed for action discovery.

### Phase 2: Universe layer

1. Add new Sem OS defs:
   - `universe_def.rs`
   - `universe_domain_def.rs`
   - `constellation_family_def.rs`
2. Add seed types and registry support.
3. Add seed scanner support under `rust/config/sem_os_seeds/universes`.
4. Introduce `resolve_universe()` in Sem OS core.

### Phase 3: Empty-session correctness

1. Add explicit discovery stage / universe stage enums.
2. Make empty `TaskId` sessions discovery-only by default.
3. Prevent grounded action surface construction without sufficient grounding inputs.
4. Add tests proving empty-session responses never return broad executable actions.

### Phase 4: Sage cutover

1. Change Sage output contract from “best verb/coder result” to “bootstrap resolution request”.
2. Thread Sage bootstrap output into universe resolution.
3. Use Sem OS universe results to drive next questions and family narrowing.
4. Only call full `resolve_context()` after family/constellation readiness is met.

### Phase 5: Grounded action cutover

1. Replace agent post-Sage `IntentPipeline` action selection with Sem OS `grounded_action_surface`.
2. Prefer Sem OS `dsl_candidates` over runtime-registry-driven DSL construction.
3. Make UI session action selection operate over Sem OS graph/action payloads.

### Phase 6: Delete or quarantine legacy bypasses

Target removals or restrictions:
- `rust/src/mcp/handlers/core.rs` `verb_search`
- `rust/src/mcp/handlers/core.rs` `verbs_list`
- `rust/src/api/agent_dsl_routes.rs` vocabulary routes returning all verbs
- `rust/src/domain_ops/affinity_ops.rs` `discover_dsl()` tool path
- `rust/src/domain_ops/sem_reg_schema_ops.rs` `match_intent()` sidecar path

Rule:
- if kept, these must be explicitly admin/debug-only and not agent-facing.

## 8. Suggested File / Module Worklist

Sem OS core:
- `rust/crates/sem_os_core/src/context_resolution.rs`
- `rust/crates/sem_os_core/src/service.rs`
- `rust/crates/sem_os_core/src/seeds.rs`
- new universe/family def modules

Sem OS adapter:
- `rust/crates/sem_os_obpoc_adapter/src/lib.rs`
- new `pipeline_seeds` or dedicated universe seed scanners

Agent orchestration:
- `rust/src/agent/orchestrator.rs`
- `rust/src/agent/context_envelope.rs`
- `rust/src/api/agent_service.rs`

REPL:
- `rust/src/repl/orchestrator_v2.rs`
- `rust/src/repl/intent_service.rs`
- `rust/src/repl/intent_matcher.rs`

MCP / agent-facing tools:
- `rust/src/mcp/handlers/core.rs`
- `rust/src/mcp/tools.rs`
- `rust/src/api/agent_dsl_routes.rs`

Server wiring:
- `rust/crates/ob-poc-web/src/main.rs`

## 9. Acceptance Criteria

### Functional
- Empty session returns discovery universe/domain/family candidates, not broad executable actions.
- Sage bootstrap can navigate the universe without selecting raw verbs.
- Once target + family/constellation are identified, Sem OS returns grounded graph/action data.
- UI session receives populated constellation graph and grounded valid/blocked actions.
- REPL only executes or edits Sem OS-approved DSL/action candidates.

### Safety
- No utterance-driven chat/MCP/REPL path can search the full runtime verb inventory unless explicitly marked debug/admin.
- No direct legacy SemReg fallback remains in production utterance handling.
- Sem OS unavailability blocks action discovery rather than widening it.

### Verification
- Unit tests for empty-session universe resolution.
- Unit tests for family-to-constellation narrowing.
- Unit tests proving `grounded_action_surface` is absent in discovery mode.
- Integration tests for:
  - `empty session -> universe questions`
  - `objective + target -> family narrowing`
  - `new entity -> draft constellation grounding`
  - `grounded instance -> valid Sem OS actions only`

## 10. Open Review Questions

1. Should discovery be modeled as:
   - a separate `resolve_universe()` call, or
   - a new stage within `resolve_context()`?

2. Should `UniverseDef` / `ConstellationFamilyDef` be:
   - first-class authored object types, or
   - encoded via higher-order taxonomy/view metadata?

3. For new entities, should Sem OS ground against:
   - a draft subject type, or
   - a draft instance flag on a normal subject reference?

4. Which legacy tools must be deleted immediately vs retained as explicit admin-only diagnostics?

5. Should the UI display:
   - domain clusters first,
   - constellation families first, or
   - direct Sage clarification prompts with hidden family inference?

## 11. Recommended Initial Decision Set

Recommended defaults for implementation:
- Introduce first-class `UniverseDef` and `ConstellationFamilyDef`.
- Implement `resolve_universe()` separately from `resolve_context()`.
- Treat empty session as `discovery` and never emit grounded actions there.
- Make `SemOsClient` mandatory for utterance discovery.
- Fail closed when Sem OS is unavailable.
- Cut Sage over from raw verb choice to universe navigation and grounding requests.

