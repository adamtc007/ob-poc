# Unified Session Pipeline Merge

## Context

Two separate session models exist: `UnifiedSession` (agent pipeline, 3,551 lines) and `ReplSessionV2` (REPL V2 pipeline). The REPL V2 has the correct tollgate architecture (ScopeGate → WorkspaceSelection → JourneySelection → InPack → Execution) but the frontend only uses the agent pipeline, which has no workspace gates. The user's non-negotiable requirement: every session must start with client group selection, then workspace selection, before any free-form work.

**Decision:** `ReplSessionV2` becomes the canonical session. `UnifiedSession` is retired. Agent capabilities (Sage narration, entity resolution, SemOS governance) are injected into the REPL V2 orchestrator. Dead code is removed at each phase — not deferred.

**Net effect:** ~14,800 lines removed, ~580 lines added (24:1 ratio).

## Tollgate Sequence (Non-Negotiable)

```
1. ScopeGate        → "Which client group?" (mandatory, no bypass)
2. WorkspaceSelection → KYC | OnBoard | CBU | Deal | Product Maint | Instrument Matrix
3. JourneySelection  → Pack selection within workspace
4. InPack            → Verb matching + arg extraction + sentence generation
5. SentencePlayback  → Confirm/reject proposed DSL
6. RunbookEditing    → Review runsheet, execute
7. Executing         → Step-by-step execution with narration
```

---

## Phase 0: Response Adapter Layer

**Goal:** Build `ReplResponseV2 → ChatResponse` adapter so the frontend works unchanged.

**Add:**
- `rust/src/api/response_adapter.rs` (~120 lines) — mapping layer
- `session_feedback: Option<serde_json::Value>` field on `ChatResponse` in `ob-poc-types/src/chat.rs`

**Mapping:**
| ReplResponseKindV2 | ChatResponse field |
|---|---|
| `ScopeRequired` | `decision: DecisionPacket { kind: "group_clarification" }` |
| `WorkspaceOptions` | `decision: DecisionPacket { kind: "workspace_selection" }` |
| `JourneyOptions` | `decision: DecisionPacket { kind: "journey_selection" }` |
| `SentencePlayback` | `coder_proposal: { requires_confirmation: true }` |
| `Clarification` | `verb_disambiguation` |
| `Executed` | `dsl` with results |
| All | `session_feedback` from resp |

**Remove:** Nothing yet — this phase is purely additive.

**Verify:** Unit tests mapping each variant.

---

## Phase 1: Unified Session Creation + First Dead Code Cut (~390 lines removed)

**Goal:** `POST /api/session` creates `ReplSessionV2` in ScopeGate. All utterances route through `ReplOrchestratorV2.process()`. Remove superseded agent session creation code.

**Rewire:**
- `create_session()` in `agent_routes.rs` — call `ReplOrchestratorV2.create_session()` instead of `UnifiedSession::new_for_entity()`
- `session_input(kind=utterance)` — route to `ReplOrchestratorV2.process()`, convert via adapter
- `session_input(kind=decision_reply)` — check REPL session state, convert to `UserInputV2::SelectScope/SelectWorkspace/SelectPack`

**Remove (in this phase):**
| Target | Location | Lines | Reason |
|--------|----------|-------|--------|
| `build_semos_workflow_decision()` | agent_routes.rs:1012-1093 | ~80 | Replaced by ScopeGate (no workflow bypass) |
| `build_client_group_decision()` | agent_routes.rs:1096-1173 | ~80 | Replaced by ScopeGate bootstrap |
| `resolve_initial_client()` | agent_routes.rs:~1180-1230 | ~50 | Replaced by REPL bootstrap resolution |
| Semantic OS workflow branch | agent_routes.rs:768-810 | ~40 | No more `workflow_focus` bypass |
| `chat_session()` delegation to agent | agent_routes.rs:2739-3107 | ~370 | Replaced by REPL orchestrator routing |
| `SessionInputRequest::ReplV2` variant | ob-poc-types/session_input.rs | ~10 | All input is utterance or decision_reply |
| `/api/repl/v2/*` standalone routes | repl_routes_v2.rs:835-844 | ~10 | Session creation goes through `/api/session` |
| REPL V2 nesting in agent_state.rs | agent_state.rs:444-448 | ~5 | Router consolidated |

**Verify:** `cargo x deploy` → new session → type "Allianz" → see workspace options (not free-form chat).

---

## Phase 2: Agent Service Decomposition + Second Cut (~1,800 lines removed)

**Goal:** Extract reusable agent capabilities from `process_chat()` into standalone functions, then remove `process_chat()` itself.

**Extract (keep as standalone functions):**
| Capability | Extract from | Extract to | Callers after extraction |
|------------|-------------|-----------|------------------------|
| Entity resolution | agent_service.rs:2114-2180 | `agent_enrichment.rs::resolve_entities()` | REPL orchestrator (Phase 3) |
| Onboarding state view | agent_routes.rs:2990-3010 | `agent_enrichment.rs::compute_onboarding_state()` | Response adapter |
| Verb surface / available_verbs | agent_routes.rs:3036-3078 | `agent_enrichment.rs::compute_verb_surface()` | Response adapter |
| TOCTOU recheck | agent_service.rs:1655-1700 | Already exists in REPL: orchestrator_v2.rs:2594-2642 | No extraction needed |

**Remove:**
| Target | Location | Lines | Reason |
|--------|----------|-------|--------|
| `process_chat()` | agent_service.rs:1634-4700+ | ~1,500 | Replaced by REPL orchestrator + enrichment |
| `run_sage_stage()` / `run_coder_stage()` | agent_service.rs | ~200 | Sage inlined into enrichment; Coder = REPL sentence gen |
| `build_mutation_confirmation()` | agent_service.rs | ~100 | Replaced by REPL SentencePlayback |
| `check_session_context()` | agent_service.rs:2842-2900 | ~60 | Replaced by REPL ScopeGate (mandatory) |
| `handle_decision_selection()` | agent_service.rs:3138-3200 | ~60 | Replaced by REPL state machine |

**Remove (route files superseded by unified input):**
| Target | Location | Lines | Reason |
|--------|----------|-------|--------|
| `agent_learning_routes.rs` | rust/src/api/ | ~950 | Decision reply handling moved to session_input |
| `agent_dsl_routes.rs` | rust/src/api/ | ~2,000 | DSL execution via REPL runbook only |
| `cbu_session_routes.rs` | rust/src/api/ | ~950 | Standalone CBU session model retired |

**Verify:** Full flow: scope → workspace → journey → utterance → verb match → sentence → confirm → execute. No `process_chat()` in call stack.

---

## Phase 3: UnifiedSession Retirement + Final Cut (~8,000+ lines removed)

**Goal:** Delete `UnifiedSession` and all code that exclusively depends on it.

**Migrate (small additions to ReplSessionV2):**
| Field | From UnifiedSession | Add to ReplSessionV2 | Lines |
|-------|-------------------|---------------------|-------|
| `bindings` | `HashMap<String, BoundEntity>` | Same type, default empty | +5 |
| `recent_sage_intents` | `Vec<SageIntent>` | `Vec<String>` (simplified) | +3 |
| Entity scope metadata | `entity_scope`, `dominant_entity_id` | Via workspace frame subject | Already exists |

**Remove (entire modules):**
| Target | Location | Lines | Reason |
|--------|----------|-------|--------|
| `session/unified.rs` | rust/src/session/ | ~3,551 | Replaced by ReplSessionV2 |
| `session/mod.rs` + submodules | rust/src/session/ | ~2,000 | Session directory retired |
| `api/agent_service.rs` | rust/src/api/ | ~4,700 | Decomposed in Phase 2, remainder dead |
| `SessionState` enum | ob-poc-types | ~50 | Replaced by ReplStateV2 |
| `vnext-repl` feature flag | 11 files | ~100 | REPL V2 always on |

**Remove (feature flags):**
- `vnext-repl` in `Cargo.toml` (workspace + crate level)
- All `#[cfg(feature = "vnext-repl")]` gates (11 files)
- `SessionInputRequest::ReplV2` variant (already removed in Phase 1)

**Remove (frontend dead code):**
| Target | Location | Lines | Reason |
|--------|----------|-------|--------|
| `replV2.ts` API client | ob-poc-ui-react/src/api/ | ~200 | All input through chat.ts |
| REPL V2 types duplication | ob-poc-ui-react/src/types/ | ~100 | Consolidated into chat types |

**Verify:** `cargo x pre-commit` — zero dead code warnings, zero unused imports. `npm run build` — no unused TS imports. Full E2E: scope → workspace → journey → pack → utterance → execute.

---

## Risk Mitigation

| Risk | Mitigation |
|------|-----------|
| Phase 2 misses edge cases from `process_chat()` | Shadow-mode: run both pipelines, log divergences, return REPL result only |
| Phase 3 breaks tests referencing `UnifiedSession` | Audit all 19 files importing `UnifiedSession` before deletion |
| Frontend rendering breaks from adapter mapping | Phase 0 adapter has exhaustive unit tests for every `ReplResponseKindV2` variant |
| Session persistence breaks | `SessionRepositoryV2` already handles `ReplSessionV2`; verify round-trip after field additions |

## What Survives (Not Touched)

- `ReplSessionV2` + `ReplOrchestratorV2` (the replacements)
- `SemOsContextEnvelope` + `SessionVerbSurface` (governance)
- Session trace + replay (Phase R1-R9 work)
- Runbook plan compilation + execution
- All domain operations (verb implementations)
- BPMN-Lite integration
- SemReg stewardship + governed authoring
- Entity Gateway + Candle embeddings
- React frontend (no component changes, adapter handles format)
