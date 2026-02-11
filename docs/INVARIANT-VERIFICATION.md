# Invariant Verification Matrix

> Generated: 2026-02-07
> Scope: TODO-2 Architecture Pipeline — Pack-Scoped Intent Resolution
> Feature flag: `vnext-repl`

This document maps each architectural invariant to the enforcing code (file, function, line) and the test(s) that verify it.

---

## Pipeline Invariants (P-1 through P-5)

### P-1: One Context Object

**Rule:** A single canonical context object (`ContextStack`) holds all session-derived state. `MatchContext` is an adapter DTO derived from it.

| Artifact | File | Line | Role |
|----------|------|------|------|
| `ContextStack` struct | `repl/context_stack.rs` | 45 | Canonical 7-layer context |
| `MatchContext` struct | `repl/types.rs` | 18 | Adapter DTO for IntentMatcher trait |
| `from_runbook_with_router()` | `repl/context_stack.rs` | 78 | Pure fold constructor |
| `build_match_context()` | `repl/orchestrator_v2.rs` | 1711 | Adapter: ContextStack -> MatchContext |
| `build_context_stack()` | `repl/orchestrator_v2.rs` | 1723 | Delegates to session.build_context_stack() |

**Tests:**
- `context_stack::tests::test_from_runbook_empty` (context_stack.rs:1310)
- `context_stack::tests::test_from_runbook_with_scope` (context_stack.rs:1323)
- `context_stack::tests::test_from_runbook_with_pack` (context_stack.rs:1339)

---

### P-2: One Scoring Pipeline

**Rule:** All verb matching passes through `search_with_context()` which applies pack scoring. No bypass paths.

| Artifact | File | Line | Role |
|----------|------|------|------|
| `search_with_context()` | `repl/intent_matcher.rs` | 50 | Trait method — single search entry point |
| `apply_pack_scoring()` | `repl/scoring.rs` | 57 | Pack boost/penalty/domain affinity |
| `apply_ambiguity_policy()` | `repl/scoring.rs` | 150 | 4-outcome disambiguation |
| `match_verb_with_context()` | `repl/intent_service.rs` | — | Phase 2 path (IntentService) |
| `match_verb_for_input()` | `repl/orchestrator_v2.rs` | 1928 | Phase 1 fallback also calls search_with_context |
| `dual_decode()` | `repl/scoring.rs` | 185 | Cross-pack joint scoring |

**Tests:**
- `scoring::tests::test_pack_scoring_boosts_allowed_verb` (scoring.rs)
- `scoring::tests::test_ambiguity_policy_*` (scoring.rs, 4 tests)
- `scoring::tests::test_dual_decode_*` (scoring.rs, 6 tests)

---

### P-3: Session State Is Runbook Fold

**Rule:** All derived state comes from `ContextStack::from_runbook()`. Deprecated mutable fields (`client_context`, `journey_context`) are write-only persistence bridges.

| Artifact | File | Line | Role |
|----------|------|------|------|
| `from_runbook_with_router()` | `repl/context_stack.rs` | 78 | Pure fold — reads only Runbook |
| `derive_session_state()` | `repl/context_stack.rs` | 116 | Folds runbook entries -> DerivedScope |
| `DerivedScope` struct | `repl/context_stack.rs` | 105 | Replaces ClientContext |
| `PackContext` struct | `repl/context_stack.rs` | 175 | Replaces JourneyContext |
| `#[deprecated] client_context` | `repl/session_v2.rs` | 35 | Write-only persistence bridge |
| `#[deprecated] journey_context` | `repl/session_v2.rs` | 40 | Write-only persistence bridge |
| `set_client_context()` | `repl/session_v2.rs` | 125 | Bridge: writes canonical runbook.client_group_id FIRST |
| `activate_pack()` | `repl/session_v2.rs` | 135 | Bridge: writes canonical staged_pack FIRST |
| `build_context_stack()` | `repl/session_v2.rs` | 189 | Delegates to from_runbook_with_router (no deprecated reads) |

**Verification:**
- `build_match_context()` at orchestrator_v2.rs:1711 reads ONLY from ContextStack
- Zero logic reads of `client_context`/`journey_context` in V2 code paths
- All 18 deprecated field accesses are marked `#[allow(deprecated)]` for serialization/migration only

**Tests:**
- `context_stack::tests::test_from_runbook_*` (context_stack.rs:1310-1362)

---

### P-4: Preconditions Filter

**Rule:** Verbs with `precondition_checks` in lifecycle config are filtered before scoring. Unmet preconditions remove candidates and optionally suggest prerequisite verbs.

| Artifact | File | Line | Role |
|----------|------|------|------|
| `Preconditions` struct | `repl/preconditions.rs` | 33 | Parsed precondition rules |
| `EligibilityMode` enum | `repl/preconditions.rs` | 22 | Executable vs Plan mode |
| `parse_preconditions()` | `repl/preconditions.rs` | 74 | Parses "key:value" format strings |
| `preconditions_met()` | `repl/preconditions.rs` | 127 | Pure function: checks all precondition types |
| `filter_by_preconditions()` | `repl/preconditions.rs` | 244 | Batch filter with FilterStats |
| `executed_verbs` field | `repl/context_stack.rs` | — | HashSet derived from runbook fold |
| `staged_verbs` field | `repl/context_stack.rs` | — | HashSet derived from runbook fold |
| Wiring (Phase 2) | `repl/intent_service.rs` | — | filter_by_preconditions after search_with_context |
| Wiring (Phase 1) | `repl/orchestrator_v2.rs` | — | filter_by_preconditions in fallback path |

**Tests:**
- `preconditions::tests::test_parse_*` (preconditions.rs, 3 tests)
- `preconditions::tests::test_requires_scope_*` (preconditions.rs, 2 tests)
- `preconditions::tests::test_requires_prior_*` (preconditions.rs, 2 tests)
- `preconditions::tests::test_forbids_prior_*` (preconditions.rs)
- `preconditions::tests::test_plan_mode_*` (preconditions.rs)
- `preconditions::tests::test_empty_preconditions_*` (preconditions.rs)
- `preconditions::tests::test_filter_*` (preconditions.rs, 3 tests)
- `preconditions::tests::test_suggest_*` (preconditions.rs)
- Golden corpus: `tests/golden_corpus/preconditions.yaml` (15 entries)

---

### P-5: Decision Log Captures Everything

**Rule:** Every turn produces a `DecisionLog` with full context for replay tuning.

| Artifact | File | Line | Role |
|----------|------|------|------|
| `DecisionLog` struct | `repl/decision_log.rs` | 84 | Per-turn audit record (12 fields) |
| `VerbDecision` struct | `repl/decision_log.rs` | 173 | Verb match details + scores |
| `PreconditionFilterLog` | `repl/decision_log.rs` | 197 | Before/after counts + removed verbs |
| `PreconditionRemovedVerb` | `repl/decision_log.rs` | 203 | Individual removal with reasons |
| `ScoringConfig` struct | `repl/decision_log.rs` | 24 | Replay-tunable constants |
| `GoldenTestCase` struct | `repl/decision_log.rs` | — | Corpus entry for replay |
| `GoldenCorpusReport` | `repl/decision_log.rs` | — | Batch replay results |

**Tests:**
- `golden_corpus_test::test_corpus_total_at_least_50` — validates >= 50 entries (currently 102)
- `golden_corpus_test::test_category_coverage` — minimum 5 entries per category
- `golden_corpus_test::test_no_duplicate_ids` — no cross-file ID collisions

---

## Execution Invariants (E-1 through E-8)

### E-1: No Verb Search Without Context

**Rule:** Every verb search receives both `MatchContext` and `ContextStack`.

| Artifact | File | Line | Role |
|----------|------|------|------|
| `search_with_context()` signature | `repl/intent_matcher.rs` | 50 | Requires `&MatchContext` + `&ContextStack` |
| `propose_for_input()` | `repl/orchestrator_v2.rs` | 1734 | Builds context before search |
| `match_verb_for_input()` | `repl/orchestrator_v2.rs` | 1928 | Builds context before search |

---

### E-2: Pack Scoring Always Applied

**Rule:** `apply_pack_scoring()` runs on every candidate set before outcome determination.

| Artifact | File | Line | Role |
|----------|------|------|------|
| `apply_pack_scoring()` call | `repl/intent_matcher.rs` | 71 | Called inside search_with_context default impl |
| `apply_pack_scoring()` impl | `repl/scoring.rs` | 57 | Mutates candidates in-place |

---

### E-3: Ambiguity Detection with MARGIN

**Rule:** Candidates within `MARGIN` of the top score trigger disambiguation, never silent overconfidence.

| Artifact | File | Line | Role |
|----------|------|------|------|
| `MARGIN` constant | `repl/scoring.rs` | 44 | 0.05 |
| `STRONG_THRESHOLD` constant | `repl/scoring.rs` | — | 0.70 |
| `ABSOLUTE_FLOOR` constant | `repl/scoring.rs` | — | 0.55 |
| `AmbiguityOutcome` enum | `repl/scoring.rs` | 123 | Confident/Proposed/Ambiguous/NoMatch |
| `apply_ambiguity_policy()` | `repl/scoring.rs` | 150 | 4-outcome decision gate |

---

### E-4: Entity Resolution Never Invents

**Rule:** `build_candidate_universe()` only returns entities from database search. No synthetic entities.

| Artifact | File | Line | Role |
|----------|------|------|------|
| `build_candidate_universe()` | `repl/entity_resolution.rs` | — | Returns only DB-sourced candidates |
| `resolve_with_context()` | `repl/entity_resolution.rs` | — | Single resolution path in V2 |
| `EntityResolutionMethod` enum | `repl/decision_log.rs` | 272 | Focus/Search/UserSelection/Unresolved |

---

### E-5: Templates Deterministically Extracted

**Rule:** Template step hints are derived from runbook state, not LLM.

| Artifact | File | Line | Role |
|----------|------|------|------|
| `TemplateStepHint` struct | `repl/context_stack.rs` | 358 | Deterministic hint from template position |
| `derive_template_hint()` | `repl/context_stack.rs` | 414 | Fold over runbook entries |
| `try_deterministic_extraction()` | `repl/deterministic_extraction.rs` | — | 4-strategy priority (carry-forward, focus, answers, canonicalization) |
| `detect_multi_intent()` | `repl/deterministic_extraction.rs` | — | Conjunctive pattern splitting |
| `TEMPLATE_STEP_BOOST` constant | `repl/scoring.rs` | — | 0.15 boost for template-matching verbs |

---

### E-6: Decision Log Per Turn

**Rule:** Every orchestrator turn creates a `DecisionLog` entry.

| Artifact | File | Line | Role |
|----------|------|------|------|
| `DecisionLog.turn` field | `repl/decision_log.rs` | 84 | Sequential turn numbering |
| Construction in orchestrator | `repl/orchestrator_v2.rs` | — | Created in all process() handler paths |

---

### E-7: Golden Corpus >= 50

**Rule:** Golden corpus maintained at 50+ entries for regression detection.

| Artifact | File | Line | Role |
|----------|------|------|------|
| 8 YAML files | `tests/golden_corpus/*.yaml` | — | 102 entries across 8 files |
| `test_corpus_total_at_least_50` | `tests/golden_corpus_test.rs` | — | CI gate: asserts >= 50 |
| `test_category_coverage` | `tests/golden_corpus_test.rs` | — | >= 5 entries per category |
| Replay tuner CLI | `xtask/src/replay_tuner.rs` | — | run/sweep/compare/report commands |

---

### E-8: Server Wiring Complete

**Rule:** V2 REPL endpoints are registered and functional.

| Artifact | File | Line | Role |
|----------|------|------|------|
| `ReplOrchestratorV2` struct | `repl/orchestrator_v2.rs` | 99 | Full orchestrator with all components |
| `process()` entry point | `repl/orchestrator_v2.rs` | 271 | Unified dispatch for all input types |
| V2 route handlers | `api/repl_routes_v2.rs` | — | POST /session, POST /input, GET /session/:id |
| V2 route registration | `crates/ob-poc-web/src/main.rs` | 349-365 | Axum router nesting |

---

## Verification Commands

```bash
# Build
cargo build --features vnext-repl

# Clippy (zero warnings)
cargo clippy --features vnext-repl -- -D warnings

# Unit tests (320 pass)
cargo test --features vnext-repl --lib -- repl::

# Golden corpus CI (5 pass)
cargo test --test golden_corpus_test

# Dead code scan
grep -rn "BootstrapState" --include="*.rs" src/  # 0 hits expected
```
