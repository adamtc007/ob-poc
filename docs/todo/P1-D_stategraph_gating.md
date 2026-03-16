# P1-D: StateGraph & Lane-Phase Gating Audit

**Review Session:** P1-D
**Date:** 2026-03-16
**Scope:** Verify that the StateGraph implementation correctly reduces the verb surface per entity state. Covers the StateGraph engine, Reducer state machine, SemTaxonomy V2 pipeline, and the integration paths between them.

---

## 1. Executive Summary

The codebase contains **three independent verb-gating systems** that operate without formal cross-system integration:

1. **StateGraph Engine** (`rust/src/stategraph/mod.rs`) — graph-based, signal-driven, produces `valid_verbs` / `blocked_verbs` per entity context.
2. **Reducer State Machine** (`rust/src/sem_reg/reducer/`) — YAML-driven, condition/rule/overlay-based, produces `SlotReduceResult` with state and available verbs.
3. **SemTaxonomy V2 Pipeline** (`rust/src/semtaxonomy_v2/`) — three-step utterance compiler (EntityScope -> EntityState -> SelectedVerb), with its own `ValidVerb` / `BlockedVerb` types.

**Central finding:** The StateGraph engine is structurally sound — deterministic, well-tested, and correctly computes frontiers — but it is **not wired into the main utterance-to-DSL pipeline**. Its output reaches the chat path only through a fragile `research_cache["graph-walk"]` indirection in `agent_service.rs` that requires explicit prior invocation of `discovery.graph-walk`. The orchestrator (`orchestrator.rs`), SessionVerbSurface (`verb_surface.rs`), and SemTaxonomy V2 compiler have zero StateGraph awareness.

The Reducer state machine is better integrated — it powers the constellation panel UI and has REST routes — but also does not feed back into the verb surface or utterance pipeline. Both systems produce verb eligibility information that is never consumed by the components responsible for presenting available verbs to the user.

**Bottom line:** StateGraph does correctly reduce the verb surface per entity state *within its own evaluation boundary*, but that reduction does not propagate to where users interact with the system. A user will see ineligible verbs in the verb browser, attempt them, and only then receive a blocked response.

---

## 2. Coverage Table

### 2.1 StateGraph Definitions (7 files)

| Graph File | entity_types | Nodes | Edges | Gates | Lanes | Verb Coverage | Terminal Nodes |
|---|---|---|---|---|---|---|---|
| `cbu.yaml` | `["client-business-unit"]` | 4 | 4 | 1 | 3 (cbu, document, ubo) | 8 verb_ids | 0 |
| `deal.yaml` | `["deal"]` | 2 | 2 | 0 | 1 (deal) | 5 verb_ids | 0 |
| `entity.yaml` | `["entity"]` | 2 | 3 | 0 | 1 (entity) | 5 verb_ids | 0 |
| `fund.yaml` | `["fund"]` | 3 | 3 | 0 | 1 (fund) | 5 verb_ids | 0 |
| `screening.yaml` | `["screening"]` | 2 | 2 | 0 | 1 (screening) | 3 verb_ids | 0 |
| `ubo.yaml` | `["ubo"]` | 2 | 2 | 0 | 1 (ubo) | 3 verb_ids | 0 |
| `document.yaml` | `["document"]` | 2 | 2 | 0 | 1 (document) | 3 verb_ids | 0 |
| **Totals** | **7 entity types** | **17** | **18** | **1** | **9** | **32 verb_ids** | **0** |

### 2.2 Reducer State Machine Definitions (2 builtins)

| State Machine | States | Transitions | Conditions | Rules | Overlay Sources | Consistency Checks |
|---|---|---|---|---|---|---|
| `entity_kyc_lifecycle` | 8 | 9 | 10 | 8 | 6 | 0 |
| `ubo_epistemic_lifecycle` | 5 | 4 | 9 | 5 | 2 | 2 |
| **Totals** | **13** | **13** | **19** | **13** | **8** | **2** |

### 2.3 Domain Coverage Matrix

| Domain | Verbs in Registry | StateGraph Coverage | Reducer Coverage | SemTaxonomy V2 Coverage |
|---|---|---|---|---|
| `cbu` | 25 | 8 verb_ids (32%) | — | 3 verbs (create/read/update) |
| `entity` | 30 | 5 verb_ids (17%) | via entity_kyc_lifecycle | — |
| `deal` | 30 | 5 verb_ids (17%) | — | — |
| `fund` | ~15 | 5 verb_ids (33%) | — | — |
| `screening` | ~10 | 3 verb_ids (30%) | — | — |
| `ubo` | ~15 | 3 verb_ids (20%) | via ubo_epistemic_lifecycle | — |
| `document` | 7 | 3 verb_ids (43%) | — | — |
| `kyc` | 20 | — | via entity_kyc_lifecycle | — |
| Other (~1,100 verbs) | — | — | — | — |
| **Total** | **1,263** | **32 (2.5%)** | **~13 transitions** | **3 (0.24%)** |

---

## 3. Logic Traces

### 3.1 StateGraph: `walk_graph()` Determinism

**Input:** `(graph: &StateGraph, entity_context: &Value)`
**Output:** `GraphWalkResult` with `satisfied_nodes`, `frontier_nodes`, `valid_verbs`, `blocked_verbs`, `gate_status`

**Trace for CBU graph with `entity_context = {"entity_id": "abc", "name": "Test", "signals": {"has_kyc_case": true}}`:**

```
Step 1: Build node_map (BTreeMap — deterministic ordering)
  cbu.entry -> Entry
  cbu.active -> Milestone (satisfied_when: [has_kyc_case])
  cbu.gate -> Gate (satisfied_when: [all_cbu_prereqs_met])
  ubo.live -> Milestone (satisfied_when: [has_ubo_data])

Step 2: Evaluate node satisfaction
  cbu.entry -> NodeType::Entry → always true → SATISFIED
  cbu.active -> has_kyc_case = true → SATISFIED
  cbu.gate -> all_cbu_prereqs_met = missing/false → NOT SATISFIED
  ubo.live -> has_ubo_data = missing/false → NOT SATISFIED

Step 3: Compute frontier (edges where from SATISFIED, to NOT SATISFIED)
  edge cbu.entry → cbu.gate: from satisfied, to NOT satisfied → FRONTIER
  edge cbu.active → ubo.live: from satisfied, to NOT satisfied → FRONTIER

Step 4: For each frontier edge, evaluate conditions
  edge to cbu.gate: no conditions → VALID
    → verb_ids on this edge added to valid_verbs
  edge to ubo.live: no conditions → VALID
    → verb_ids on this edge added to valid_verbs

Step 5: Evaluate gates
  cbu.gate requires [cbu.entry] → cbu.entry is satisfied → gate all_met = true
```

**Determinism properties:**
- `BTreeMap` / `BTreeSet` used throughout — iteration order is deterministic
- `evaluate_signal()` is a pure function (reads only from `entity_context["signals"]`)
- `compute_relevance()` is deterministic (score = base + from_satisfied bonus + edge_type adjustment)
- Verb YAML loaded via `ConfigLoader::from_env()` — deterministic given same config files
- **Conclusion: walk_graph() is fully deterministic for identical inputs**

**Edge case — `evaluate_signal()` limitations:**
```rust
pub fn evaluate_signal(signal: &str, entity_context: &Value) -> bool {
    let signal_value = &entity_context["signals"][signal];
    signal_value.as_bool().unwrap_or_else(|| signal_value.as_i64().unwrap_or(0) > 0)
}
```
- Only supports `bool` and positive `i64` — no string equality, no comparison operators
- Missing signal → `Value::Null` → `as_bool()` = None, `as_i64()` = None → `0 > 0` → `false`
- Negative integer → `false` (correct for count semantics, surprising for general use)
- String values → always `false` (silently ignored)

### 3.2 Reducer: `reduce_slot()` Determinism

**Input:** `(pool, cbu_id, case_id, constellation_type, slot_path, sm)`
**Output:** `SlotReduceResult` with `effective_state`, `computed_state`, `override_info`, `blocked_verbs`

**Trace:**
```
Step 1: Build eval scope
  - Fetch overlay data from sm.reducer.overlay_sources (live DB queries)
  - Construct ScopeData from overlay rows

Step 2: Evaluate rules (first-match-wins)
  For each rule in sm.reducer.rules:
    For each condition in rule.conditions:
      evaluate(condition, eval_scope) → true/false
    If ALL conditions true → state = rule.transition_to, STOP

Step 3: Check active override
  SELECT FROM state_overrides WHERE cbu_id AND slot_path AND NOT revoked AND NOT expired
  If found → effective_state = override.override_state

Step 4: Compute blocked verbs
  For each transition in sm.transitions:
    If transition.from != effective_state → blocked (wrong state)
    For each condition in transition.conditions:
      If condition fails → blocked with reason
```

**Determinism properties:**
- Depends on live DB state (overlay queries) — NOT deterministic across time
- First-match-wins rule semantics — deterministic given identical overlay data
- Override lookup uses `LIMIT 1 ORDER BY created_at DESC` — deterministic tiebreak
- **Conclusion: deterministic within a single evaluation, but overlay-dependent**

### 3.3 SemTaxonomy V2: `step2_entity_state()` Determinism

**Input:** `(scope: &EntityScope, entity_state: Option<&Value>, valid_transitions: Option<&Value>)`
**Output:** Filters verb candidates based on `valid_transitions` JSON

**Trace:**
```
Step 1: If valid_transitions is None → pass all candidates through (no filtering)
Step 2: If present → parse JSON into Vec<ValidVerb> / Vec<BlockedVerb>
Step 3: Filter: only verbs whose FQN appears in valid_transitions are eligible
```

**Key observation:** `valid_transitions` comes from `research_cache["graph-walk"]` in `agent_service.rs` (line 1571). This means:
- If `discovery.graph-walk` was never invoked → `valid_transitions = None` → **no filtering at all**
- If invoked → stale cache data (no TTL, no invalidation mechanism)

---

## 4. Integration Assessment

### 4.1 StateGraph → Pipeline Path

The **only** consumer of `walk_graph()` is `DiscoveryGraphWalkOp` in `rust/src/domain_ops/discovery_ops.rs`. Its output is stored in `state.research_cache["graph-walk"]` and consumed in `agent_service.rs`:

```
User invokes "discovery.graph-walk" verb
    ↓
DiscoveryGraphWalkOp.execute()
    ↓
walk_graph() → GraphWalkResult
    ↓
Serialized to JSON → stored in session.research_cache["graph-walk"]
    ↓
(Later, on next chat turn)
    ↓
agent_service.rs:1571 → state.research_cache.get("graph-walk")
    ↓
semtaxonomy_v2::step2_entity_state() receives as valid_transitions
```

**Problems with this path:**
1. **Opt-in only** — user must explicitly run graph-walk first
2. **No cache invalidation** — entity state may change between graph-walk and next turn
3. **Untyped JSON** — `GraphWalkResult` serialized to `serde_json::Value`, deserialized with separate `ValidVerb`/`BlockedVerb` types in SemTaxonomy V2
4. **No automatic refresh** — no background job, no TTL, no staleness detection

### 4.2 Systems with ZERO StateGraph References

Confirmed via grep — these critical pipeline components have no awareness of StateGraph:

| Component | File | Impact |
|---|---|---|
| Orchestrator | `rust/src/agent/orchestrator.rs` | Main utterance pipeline ignores entity state |
| SessionVerbSurface | `rust/src/agent/verb_surface.rs` | Governance verb set ignores entity state |
| HybridVerbSearcher | `rust/src/mcp/verb_search.rs` | Semantic search returns ineligible verbs |
| IntentPipeline | `rust/src/mcp/intent_pipeline.rs` | Intent matching ignores entity state |
| RuntimeVerbRegistry | `rust/src/dsl_v2/runtime_registry.rs` | Base verb set is unconditional |
| ContextEnvelope | `rust/src/agent/context_envelope.rs` | SemReg filtering has no entity state input |

### 4.3 Reducer → Pipeline Path

The Reducer has better REST integration than StateGraph:
- `GET /api/cbu/:id/constellation` — hydrates slot states for UI
- `POST /api/session/:id/input` — constellation panel "Ask Why Blocked" submits prompts

But the Reducer also does NOT feed into the verb surface:
- `SessionVerbSurface` Step 5 (`lifecycle_state`) receives `entity_state: None` always
- The P1-D todo doc (prior version) confirms: "entity_state: None always, deferring to Phase 3"

### 4.4 Type System Fragmentation

Three separate `ValidVerb` / `BlockedVerb` type hierarchies exist:

| System | Valid Type | Blocked Type | Location |
|---|---|---|---|
| StateGraph | `stategraph::ValidVerb` (12 fields) | `stategraph::BlockedVerb` (7 fields) | `stategraph/mod.rs` |
| Reducer | `reducer::ast::BlockedVerb` (4 fields) | `reducer::ast::BlockedWhyResult` | `sem_reg/reducer/ast.rs` |
| SemTaxonomy V2 | Own `ValidVerb` (parsed from JSON) | Own `BlockedVerb` | `semtaxonomy_v2/mod.rs` |

These types are not interchangeable and require JSON serialization/deserialization to bridge.

---

## 5. Gap List

### G-01: StateGraph not wired into verb surface [P0 — Severity: Critical]

**What:** `SessionVerbSurface.compute_session_verb_surface()` has 8 steps. None consult StateGraph or entity state. Step 5 (lifecycle_state) is disabled (`entity_state: None` always).

**Impact:** Users see all verbs regardless of entity state. Blocked verbs are only discovered at execution time, not at discovery/selection time.

**Evidence:** `rust/src/agent/verb_surface.rs` — no import of `stategraph`, no `entity_state` parameter consumed in compute pipeline.

### G-02: `research_cache` indirection is fragile and untyped [P1 — Severity: High]

**What:** StateGraph results reach the chat pipeline only via `state.research_cache.get("graph-walk")` — a `HashMap<String, serde_json::Value>` with no typing, no TTL, no invalidation.

**Impact:** Stale graph-walk results may permit verbs that are no longer valid. Cache key is a magic string. Type errors between `stategraph::ValidVerb` and SemTaxonomy V2's `ValidVerb` fail at runtime, not compile time.

**Evidence:** `agent_service.rs:1571` — `let valid_transitions = state.research_cache.get("graph-walk").cloned();`

### G-03: No Terminal node handling in any stategraph YAML [P2 — Severity: Medium]

**What:** `NodeType::Terminal` is defined in the type system but zero stategraph YAML files use it. There is no code path that handles terminal state behavior (e.g., preventing further transitions, marking an entity as complete/closed).

**Impact:** Entities never reach a terminal state via the StateGraph engine. All graphs have open-ended progression.

**Evidence:** All 7 YAML files checked — only `entry` and `milestone` node types used. `NodeType::Gate` used only in `cbu.yaml`.

### G-04: `evaluate_signal()` supports only bool/i64 — no string comparison [P1 — Severity: High]

**What:** Signal evaluation coerces everything through `as_bool()` then `as_i64() > 0`. String signals (e.g., `"status": "approved"`) silently evaluate to `false`.

**Impact:** Cannot gate on string-valued entity state fields (status enums, lifecycle phases) — the most common state representation in the database. Forces all conditions to be pre-computed as boolean/numeric signals by the caller.

**Evidence:** `stategraph/mod.rs:262-267`

### G-05: StateGraph verb coverage is 2.5% of registry [P1 — Severity: High]

**What:** 32 verb_ids across 7 graphs out of 1,263 verbs in the registry. 15 domains have zero StateGraph coverage.

**Impact:** Even if StateGraph were fully integrated, 97.5% of verbs would pass through unfiltered. The graph definitions are far too sparse to serve as a comprehensive verb-gating mechanism.

**Evidence:** Coverage table (Section 2.3).

### G-06: Reducer `blocked_verbs_with_reasons()` can produce duplicate entries [P2 — Severity: Medium]

**What:** `blocked_verbs_with_reasons()` iterates all transitions and emits a `BlockedVerb` for each failed condition on each transition. If a verb appears in multiple transitions, it gets duplicate blocked entries with different reasons.

**Impact:** UI may display duplicate blocked-verb entries. Minor — cosmetic issue.

**Evidence:** `rust/src/sem_reg/reducer/verbs.rs` — `blocked_verbs_with_reasons()` has no dedup.

### G-07: `handle_state_derive_all()` hardcodes constellation name [P2 — Severity: Medium]

**What:** `handle_state_derive_all()` in `verbs.rs` hardcodes `"struct.lux.ucits.sicav"` as the constellation type when deriving all slot states.

**Impact:** Only works for Luxembourg UCITS SICAV structures. Other constellation types (PE, hedge, cross-border) require separate handling.

**Evidence:** `rust/src/sem_reg/reducer/verbs.rs` — search for `struct.lux.ucits.sicav`.

### G-08: Reducer overlay queries have no caching or batching [P2 — Severity: Medium]

**What:** Every call to `reduce_slot()` executes live DB queries against overlay sources. No query caching, no batch evaluation across slots.

**Impact:** For a constellation with 20 slots, each evaluation produces 20 x N_overlay_sources independent DB queries. Performance concern for constellation hydration.

**Evidence:** `rust/src/sem_reg/reducer/fetch.rs` — `fetch_slot_overlays()` and `fetch_slot_overlays_tx()` execute fresh queries per call.

### G-09: `DiscoveryGraphWalkOp` returns untyped JSON [P1 — Severity: High]

**What:** `DiscoveryGraphWalkOp` in `discovery_ops.rs` serializes `GraphWalkResult` to `serde_json::json!({...})` instead of a typed struct. This violates the project's "Type Safety First" rule (CLAUDE.md Non-Negotiable Rule #1).

**Impact:** No compile-time guarantees on the shape of graph-walk results. Field renames break silently.

**Evidence:** `rust/src/domain_ops/discovery_ops.rs` — `DiscoveryGraphWalkOp::execute()` returns `ExecutionResult::Record(serde_json::json!({...}))`.

### G-10: Three separate ValidVerb/BlockedVerb type hierarchies [P2 — Severity: Medium]

**What:** StateGraph, Reducer, and SemTaxonomy V2 each define their own verb eligibility types with different field sets. No shared trait or conversion between them.

**Impact:** Unifying the three systems into a single verb surface requires bridging three type hierarchies through JSON serialization.

**Evidence:** Section 4.4 type comparison table.

---

## 6. Findings Summary (Severity-Tagged)

| ID | Severity | Finding |
|---|---|---|
| **G-01** | P0 — Critical | StateGraph output is not consumed by SessionVerbSurface, orchestrator, or any component in the verb discovery pipeline. Entity state does not reduce the presented verb set. |
| **G-02** | P1 — High | `research_cache["graph-walk"]` is untyped, has no TTL, no invalidation, and is opt-in only. |
| **G-04** | P1 — High | `evaluate_signal()` cannot compare string values — silently returns false for status enums. |
| **G-05** | P1 — High | StateGraph covers only 32 of 1,263 verbs (2.5%). Insufficient for comprehensive gating. |
| **G-09** | P1 — High | `DiscoveryGraphWalkOp` uses untyped JSON, violating Type Safety First rule. |
| **G-03** | P2 — Medium | `NodeType::Terminal` defined but never used in any YAML — no terminal state handling. |
| **G-06** | P2 — Medium | Reducer can produce duplicate `BlockedVerb` entries for the same verb. |
| **G-07** | P2 — Medium | `handle_state_derive_all()` hardcodes `"struct.lux.ucits.sicav"`. |
| **G-08** | P2 — Medium | Reducer overlay queries have no caching — N queries per slot per evaluation. |
| **G-10** | P2 — Medium | Three separate ValidVerb/BlockedVerb type hierarchies with no shared trait. |

---

## 7. Recommendations

### Short-Term (Addresses P0)

1. **Wire entity state into SessionVerbSurface Step 5.** After `reduce_slot()` for the current session entity, pass the `effective_state` as `entity_state` in `VerbSurfaceContext`. This enables Step 5 to filter out verbs that the reducer would block, making the verb browser accurate.

2. **Replace `research_cache` with typed graph-walk context.** Define a `GraphWalkContext` struct that `DiscoveryGraphWalkOp` produces and that `step2_entity_state()` consumes directly, with a `computed_at: DateTime<Utc>` timestamp and a staleness threshold.

3. **Add typed result struct for `DiscoveryGraphWalkOp`.** Replace the `serde_json::json!({...})` return with a `#[derive(Serialize, Deserialize)] GraphWalkResultDto` in `ob-poc-types`, per the Type Safety First rule.

### Medium-Term (Addresses P1)

4. **Extend `evaluate_signal()` to support string comparison.** Add a `SignalCondition` variant for `Eq(String)` / `In(Vec<String>)` so that status-enum signals can be gated directly without pre-computation to booleans.

5. **Expand StateGraph YAML coverage.** Prioritize the 5 highest-traffic domains (kyc, entity, trading-profile, deal, ubo) — each graph should cover at least the CRUD verbs and lifecycle transition verbs for its entity type.

6. **Unify ValidVerb/BlockedVerb types.** Define a shared `VerbEligibility` trait in `ob-poc-types` that StateGraph, Reducer, and SemTaxonomy V2 types all implement, enabling a single verb surface to consume results from any system.

### Long-Term (Addresses P2)

7. **Automatic graph-walk on entity focus change.** When the session focuses on a new entity (via `view.cbu`, `view.drill`, etc.), automatically run the relevant StateGraph walk and cache the result in the session, removing the opt-in requirement.

8. **Add slot state change audit log.** Even if reducer state is derived (not stored), surface a `slot_state_transitions` event table that records when a slot's effective state changes. This enables auditing of "when did this step become COMPLETE" without re-evaluating the full state machine.

---

## Appendix A: File Inventory

### StateGraph Engine
| File | Purpose |
|---|---|
| `rust/src/stategraph/mod.rs` | Core engine: types, loader, validator, walker |
| `rust/config/stategraphs/cbu.yaml` | CBU graph (4 nodes, 4 edges, 1 gate) |
| `rust/config/stategraphs/deal.yaml` | Deal graph (2 nodes, 2 edges) |
| `rust/config/stategraphs/entity.yaml` | Entity graph (2 nodes, 3 edges) |
| `rust/config/stategraphs/fund.yaml` | Fund graph (3 nodes, 3 edges) |
| `rust/config/stategraphs/screening.yaml` | Screening graph (2 nodes, 2 edges) |
| `rust/config/stategraphs/ubo.yaml` | UBO graph (2 nodes, 2 edges) |
| `rust/config/stategraphs/document.yaml` | Document graph (2 nodes, 2 edges) |

### Reducer State Machine
| File | Purpose |
|---|---|
| `rust/src/sem_reg/reducer/mod.rs` | Module root, re-exports |
| `rust/src/sem_reg/reducer/verbs.rs` | Verb handlers: derive, diagnose, blocked_why |
| `rust/src/sem_reg/reducer/eval.rs` | Condition evaluator with caching |
| `rust/src/sem_reg/reducer/state_machine.rs` | YAML loader, ValidatedStateMachine |
| `rust/src/sem_reg/reducer/builtin.rs` | 2 builtin state machines |
| `rust/src/sem_reg/reducer/overrides.rs` | Manual state override CRUD |
| `rust/src/sem_reg/reducer/fetch.rs` | Overlay data fetching |
| `rust/config/sem_os_seeds/state_machines/entity_kyc_lifecycle.yaml` | 8-state KYC lifecycle |
| `rust/config/sem_os_seeds/state_machines/ubo_epistemic_lifecycle.yaml` | 5-state UBO lifecycle |

### SemTaxonomy V2 Pipeline
| File | Purpose |
|---|---|
| `rust/src/semtaxonomy_v2/mod.rs` | Pipeline orchestrator, three-step routing |
| `rust/src/semtaxonomy_v2/bridge.rs` | Legacy Sage -> CompilerInputEnvelope |
| `rust/src/semtaxonomy_v2/cbu_compiler.rs` | Narrow CBU compiler slice |
| `rust/src/semtaxonomy_v2/compiler.rs` | 6-phase IntentCompiler |
| `rust/src/semtaxonomy_v2/extraction.rs` | JSON extraction + validation |
| `rust/src/semtaxonomy_v2/phases/mod.rs` | Phase trait definitions |

### Integration Points
| File | Purpose |
|---|---|
| `rust/src/domain_ops/discovery_ops.rs` | `DiscoveryGraphWalkOp` — sole StateGraph consumer |
| `rust/src/api/agent_service.rs` | `research_cache["graph-walk"]` → `step2_entity_state()` |
| `rust/src/agent/orchestrator.rs` | Main pipeline — NO StateGraph references |
| `rust/src/agent/verb_surface.rs` | SessionVerbSurface — NO entity state input |
| `rust/src/session/view_state.rs` | ViewState — NO graph-walk integration |

## Appendix B: Query Evidence

```sql
-- No constellation/reducer/state_graph tables exist in the database
SELECT tablename FROM pg_tables
WHERE (tablename LIKE '%constellation%' OR tablename LIKE '%reducer%' OR tablename LIKE '%state_graph%')
  AND schemaname NOT IN ('pg_catalog', 'information_schema');
-- Results: (empty)

-- State override persistence exists
SELECT tablename FROM pg_tables WHERE tablename = 'state_overrides';
-- Results: state_overrides (in "ob-poc" schema)
```
