# Semantic OS Reconciliation Review — Findings Report

> **Date:** 2026-03-19
> **Scope:** Read-only reconciliation of Semantic OS implementation coverage across four pillars
> **Method:** Codebase inspection of entity/subject model, verb/DSL, constellation/discovery, and DAG/planning/execution layers

---

## 1. Executive Summary

The Semantic OS implementation is **structurally complete across all four pillars** with strong end-to-end wiring from utterance through execution. The DAG/planning/execution layer is the most mature (~95%). The discovery/constellation layer is well-authored but has a key join-point gap where bootstrap surfaces are computed but not rendered to users. The entity/subject model has all building blocks but **entity-kind filtering is not threaded through verb search** — the single most impactful gap.

**Overall assessment: 80–85% end-to-end coverage.**

**Headline risks:**
1. **Entity-kind not filtering verb search** — the biggest semantic black hole. Entity linking produces `dominant_entity_kind`, verb contracts declare `subject_kinds`, but `HybridVerbSearcher` ignores both.
2. **Discovery bootstrap surface is dead code at runtime** — SemOS computes `entry_questions`/`grounding_readiness` but these never reach the UI.
3. **~68% of verbs lack `subject_kinds`** — 402 of ~1,263 have `subject_kinds` populated, meaning most verbs are unconstrained by entity kind.

---

## 2. Pillar-by-Pillar Assessment

### A. Entity/Subject Model — Confidence: HIGH (80%)

**Strengths:**
- Canonical entity kinds with alias canonicalization (5 kinds: `kyc-case`, `client-group`, `company`, `person`, `investor`) in `rust/src/entity_kind.rs` + `rust/config/entity_kind_canonical.yaml`
- NounIndex provides deterministic Tier -1 noun→verb resolution with 99 mapped nouns (`rust/src/mcp/noun_index.rs`, `rust/config/noun_index.yaml`)
- Entity linking service has kind-constrained scoring (+0.15 boost / -0.20 penalty) in `rust/src/entity_linking/resolver.rs`
- LookupService implements verb-first ordering (verb → expected_kinds → entity resolution) in `rust/src/lookup/service.rs`
- Subject kind derivation has 3-level fallback chain in `rust/crates/sem_os_obpoc_adapter/src/scanner.rs`

**Weaknesses:**
- Only 5 canonical entity kinds — `fund`, `cbu`, `trading-profile`, `document`, `contract` are missing as first-class kinds
- `subject_kinds` populated on only ~32% of verbs — 68% of verbs accept any entity kind
- `entity_kind` field on `ContextResolutionRequest` is defined but **not consumed** in Step 5 of `resolve_context()`
- `HybridVerbSearcher.search()` has no `entity_kind` parameter — cannot filter by `subject_kinds` at search time
- `PruneReason::EntityKindMismatch` exists in `sem_os_context_envelope.rs` but is never populated
- `OrchestratorContext.pre_sage_entity_kind` is set but never used downstream

### B. Verb/DSL Coverage — Confidence: HIGH (90%)

**Quantitative Summary:**
| Metric | Count | Notes |
|--------|-------|-------|
| Verb YAML files | 131 | `rust/config/verbs/` |
| Total verbs | ~1,263 | |
| `behavior: plugin` | 608 | Majority |
| `behavior: crud` | 493 | GenericExecutor handles these |
| `behavior: graph` | 9 | |
| `behavior: durable` | 3 | BPMN-routed |
| Registered `#[register_custom_op]` | 599 | ~98% plugin coverage |
| Files with `invocation_phrases` | 1,075 | ~85% |
| Files with `phase_tags` | 1,081 | ~85% |
| Verbs with `subject_kinds` | 402 | **~32%** |
| Macro definitions | 84 | 18 YAML files in `config/verb_schemas/macros/` |

**Strengths:**
- 599 registered CustomOps covering ~98% of plugin verbs
- GenericExecutor handles all CRUD verbs automatically
- RuntimeVerbRegistry (`src/dsl_v2/runtime_registry.rs`) with 30 public functions
- 84 macros across 18 files provide composite verb chains
- Invocation phrases on ~85% of verbs enable semantic discovery

**Weaknesses:**
- ~9 plugin verbs may lack CustomOp registrations (599 ops vs 608 plugin verbs)
- ~15% of verbs lack invocation_phrases — invisible to semantic search
- Template verbs (behavior: template) count is very low

### C. Constellation/Discovery — Confidence: MEDIUM (70%)

**Quantitative Summary:**
| Artifact | Count | Location |
|----------|-------|----------|
| Constellation maps | 18 | `config/sem_os_seeds/constellation_maps/` |
| Constellation families | 3 | `config/sem_os_seeds/constellation_families/` |
| Universe seeds | 5 | `config/sem_os_seeds/universes/` |
| Scenario routes | 25 macro_fqn targets | `config/scenario_index.yaml` |
| Pack manifests | 4 | `config/packs/` |
| Domain metadata | 3,111 lines / 306 tables | `config/sem_os_seeds/domain_metadata.yaml` |

**Strengths:**
- 18 constellation maps cover fund structures across LU, IE, UK, US jurisdictions with detailed slot definitions (verbs, entity_kinds, cardinality, state_machines, overlays)
- 3 constellation families with condition-based selection rules (jurisdiction × structure_type → target constellation)
- 5 universe seeds with utterance signals, candidate entity kinds, and required grounding inputs
- 25 scenario routes in ScenarioIndex with macro_fqn targets — all resolve to real macros
- Discovery ops implement `discovery.graph-walk` and `schema.generate-discovery-map`

**Weaknesses:**
- **Discovery bootstrap surface (`entry_questions`, `grounding_readiness`) is computed by SemOS but never rendered in ChatResponse** — `SemOsContextEnvelope.discovery_surface` is populated but `ChatResponse` has no `discovery_bootstrap` field
- Only 3 constellation families (fund, cross-border, manager) — KYC, trading, billing domains have no constellation coverage
- Constellation maps reference verbs in slot definitions but there's no compile-time validation that these verbs exist
- Universe seeds have `required_grounding_inputs` but no runtime enforcement
- Only `onboarding-request` pack has a meaningful verb chain; others are thin

### D. DAG/Planning/Execution — Confidence: VERY HIGH (95%)

**Key Implementation Surfaces:**
| Component | File | LOC | Status |
|-----------|------|-----|--------|
| Agent orchestrator | `src/agent/orchestrator.rs` | 4,379 | Complete |
| Intent pipeline | `src/mcp/intent_pipeline.rs` | 2,011 | Complete |
| Verb search | `src/mcp/verb_search.rs` | 1,832 | Complete |
| Macro index | `src/mcp/macro_index.rs` | 1,181 | Complete |
| Scenario index | `src/mcp/scenario_index.rs` | 1,044 | Complete |
| Noun index | `src/mcp/noun_index.rs` | 1,013 | Complete |
| Verb surface | `src/agent/verb_surface.rs` | 1,006 | Complete |
| Context envelope | `src/agent/sem_os_context_envelope.rs` | 620 | Complete |

**Strengths:**
- Full DAG-aware topological sort with cycle detection (`src/dsl_v2/topo_sort.rs`)
- Runbook compilation pipeline with immutable artifacts (`src/runbook/compiler.rs`)
- Advisory lock acquisition sorted by entity_id (no deadlock) (`src/dsl_v2/locking.rs`)
- Preconditions engine with scope/prior/entity constraints and "why not" suggestions (`src/repl/preconditions.rs`)
- Macro expansion with fixpoint iteration, cycle detection, provenance labels (`src/dsl_v2/macros/expander.rs`)
- Two execution bridges: sync `DslStepExecutor`, durable `DslExecutorV2StepExecutor` (`src/runbook/step_executor_bridge.rs`)
- BPMN WorkflowDispatcher with queue-based resilience (`src/bpmn_integration/dispatcher.rs`)
- 7-state REPL V2 orchestrator fully wired (`src/repl/orchestrator_v2.rs`)
- SessionVerbSurface consolidates all governance layers into single queryable type (`src/agent/verb_surface.rs`)
- SemOsContextEnvelope with structured PruneReason, fingerprint, TOCTOU recheck (`src/agent/sem_os_context_envelope.rs`)
- All execution through `execute_runbook()` (INV-3 enforced)

**Weaknesses:**
- Phase execution is sequential (parallel not yet implemented)
- Macro expansion output is not re-validated against verb contracts post-expansion
- Reordering diagnostics (`was_reordered`) exist but aren't exposed in API

---

## 3. Cross-Pillar Reconciliation Findings

### 3.1 Entities without Verbs
- **`fund`** appears as `entity_kind` in constellation maps but is not a canonical entity kind in `entity_kind_canonical.yaml`. Fund structures are modeled as CBUs.
- **`document`** entities exist in `documents` table with full lifecycle but `document` is not a canonical entity kind — entity-kind-constrained verb selection will never route to document verbs based on entity kind.

### 3.2 Verbs without Entity Support
- **~860 verbs (68%)** have empty `subject_kinds` — they accept any entity kind. Even when entity-kind filtering is implemented, most verbs will pass through unfiltered.
- **`state.*` verbs** have `subject_kinds` populated (8 entries). Most other domain verbs do not.

### 3.3 Capabilities without Discovery Exposure
| Domain | Verb Count | Discovery Path | Gap |
|--------|-----------|---------------|-----|
| Trading | 30 verbs | None | No constellation family, pack, or scenario |
| Billing | 14 verbs | None | No constellation family, pack, or scenario |
| Deal | 30 verbs | None | No constellation family, pack, or scenario |
| Contract | 14 verbs | None | No constellation family, pack, or scenario |
| Custody | 40 verbs | None | No constellation family, pack, or scenario |
| Document | 7 verbs | None | No constellation family, pack, or scenario |

These domains are **functionally complete** (verbs work) but **semantically undiscoverable** through structured workflows.

### 3.4 Discoverable Intents without Validation
- Constellation maps reference verbs in slot definitions (e.g., `verb: entity.ensure-or-placeholder`) — these verbs are generally executable. However, **`state_machine` references** in constellation slots (e.g., `state_machine: entity_kyc_lifecycle`) may not all have corresponding stategraph YAML definitions.
- ScenarioIndex routes to macro FQNs like `screening.full` and `kyc.full-review` — these macros exist and expand to executable verb chains.

### 3.5 Runtime Dependencies Not Supplied by Semantic Layers
- Macro expansion produces DSL strings, but post-expansion **constraint validation** against verb contracts is missing. A macro could produce invalid argument values that pass compilation but fail at execution.
- Constellation slot `placeholder_detection: name_match` implies entity search capabilities, but entity search requires EntityGateway to be running — no fallback exists.

---

## 4. Black Holes and Failure Modes

### BH-1: Entity Kind Not Filtering Verb Search
- **Chain position:** Entity Linking → Verb Selection
- **Root cause:** `HybridVerbSearcher.search()` has no `entity_kind` parameter. Even though 402 verbs declare `subject_kinds`, search never uses them.
- **Symptom:** User says "screen this company" → verb search returns company-screening AND person-screening verbs equally because entity kind "company" isn't used to filter.
- **Severity:** **CRITICAL** — affects every utterance where entity kind matters
- **Evidence:** `src/mcp/verb_search.rs` — `search()` signature. `src/agent/orchestrator.rs` — `pre_sage_entity_kind` set but unused.

### BH-2: Discovery Bootstrap Dead Code
- **Chain position:** Verb Selection → Discovery Surface → UI
- **Root cause:** `SemOsContextEnvelope.discovery_surface` is populated but `ChatResponse` has no field for it.
- **Symptom:** New session with no grounding → user gets generic prompt instead of guided entry questions from universe seed.
- **Severity:** **MODERATE** — affects onboarding UX, not functional correctness
- **Evidence:** `src/agent/sem_os_context_envelope.rs:50-80` (DiscoverySurface struct populated). `src/api/agent_routes.rs` (ChatResponse lacks `discovery_bootstrap` field).

### BH-3: 68% of Verbs Accept Any Entity Kind
- **Chain position:** Entity → Verb Selection
- **Root cause:** Only 402/1,263 verbs have `subject_kinds` populated.
- **Symptom:** Even when entity-kind filtering is implemented, most verbs pass through unfiltered, reducing disambiguation value.
- **Severity:** **MODERATE** — reduces intent resolution precision
- **Evidence:** `grep -r 'subject_kinds:' config/verbs/ | wc -l` = 402.

### BH-4: Trading/Billing/Deal/Contract Discovery Gap
- **Chain position:** Discovery → Plan Formation
- **Root cause:** No constellation families, packs, or scenarios cover these domains.
- **Symptom:** User asks "set up billing for this fund" → semantic search finds the verb, but there's no guided discovery or pack-scoped workflow.
- **Severity:** **LOW-MODERATE** — verbs are executable but not discoverable through structured paths.

### BH-5: Missing Canonical Entity Kinds
- **Chain position:** Entity Resolution → Subject Classification
- **Root cause:** Only 5 canonical kinds exist. `fund`, `document`, `contract`, `trading-profile` are not canonical.
- **Symptom:** Constellation maps reference `entity_kind: "fund"` but `entity_kind_canonical.yaml` doesn't recognize it — entity-kind routing won't benefit.
- **Severity:** **LOW**

### BH-6: Entity Kind Confidence Lost Between Linking & SemOS
- **Chain position:** Entity Linking → SemOS Context Resolution
- **Root cause:** `DominantEntity.score` is computed but not passed to `resolve_sem_reg_verbs()`. SemOS receives `entity_kind: Option<&str>` only, no confidence.
- **Symptom:** Ambiguous entity resolutions (score 0.5) treated identically to confident ones (score 0.95).
- **Severity:** **LOW**
- **Evidence:** `src/agent/orchestrator.rs:338-350, 2879`

### BH-7: Macro Post-Expansion Not Re-Validated
- **Chain position:** Plan/DAG → Execution
- **Root cause:** Macro expander substitutes `${arg.X.internal}` but expanded DSL is not re-validated against verb contracts.
- **Symptom:** Macro produces invalid argument values → passes compilation → fails at execution with cryptic error.
- **Severity:** **LOW** — rare in practice but no defense in depth.

### BH-8: FailClosed Safe-Harbor Not Audited Against harm_class
- **Chain position:** Verb Surface → Execution
- **Root cause:** Safe-harbor verb set (~30 verbs) is hardcoded, not validated against `VerbContractBody.harm_class`.
- **Symptom:** If a dangerous verb were accidentally added to safe-harbor, no startup validation would catch it.
- **Severity:** **LOW** — current set is correct, but lacks ongoing guard.

---

## 5. Coverage Matrix

| Domain Area | Entity/Subject | Verb/DSL | Discovery/Constellation | DAG/Plan/Execute | E2E Status |
|------------|:-:|:-:|:-:|:-:|:--|
| **CBU/Structure** | ✅ | ✅ 25 verbs | ✅ 18 maps, 3 families | ✅ | **Complete** |
| **Entity/Party** | ✅ | ✅ 30 verbs | ✅ Via constellation slots | ✅ | **Complete** |
| **KYC/Screening** | ✅ | ✅ 20+ verbs | ✅ 3 scenarios, universe | ✅ | **Complete** |
| **Session/Navigation** | ✅ | ✅ 16 verbs | ✅ session-bootstrap pack | ✅ | **Complete** |
| **UBO/Ownership** | ⚠️ | ✅ 8+ verbs | ⚠️ Via KYC constellation | ✅ | **Partial** |
| **Trading Profile** | ❌ | ✅ 30 verbs | ❌ No discovery | ✅ | **Partial** |
| **Custody/Settlement** | ❌ | ✅ 40 verbs | ❌ No discovery | ✅ | **Partial** |
| **Deal Record** | ❌ | ✅ 30 verbs | ❌ No discovery | ✅ | **Partial** |
| **Billing** | ❌ | ✅ 14 verbs | ❌ No discovery | ✅ | **Partial** |
| **Contract** | ❌ | ✅ 14 verbs | ❌ No discovery | ✅ | **Partial** |
| **Document** | ❌ | ✅ 7 verbs | ❌ No discovery | ✅ | **Partial** |
| **GLEIF/Research** | ⚠️ | ✅ 15+ verbs | ⚠️ Scenario routes only | ✅ | **Partial** |
| **Registry/SemOS** | ✅ | ✅ 32+ tools | ✅ Semantic OS tab | ✅ | **Complete** |
| **Stewardship** | ✅ | ✅ 23 tools | ✅ Show Loop | ✅ | **Complete** |

**Legend:** ✅ = implemented and connected | ⚠️ = partial | ❌ = missing

---

## 6. End-to-End Join Point Status

| Join Point | Status | Key Risk |
|-----------|--------|----------|
| Utterance → Intent (Sage) | ✅ Connected | None — Sage mandatory |
| Intent → Entity (LookupService) | ✅ Connected | Confidence score lost (BH-6) |
| Entity → Verb (HybridVerbSearcher) | ⚠️ **Incomplete** | **entity_kind not threaded (BH-1)** |
| Verb → Discovery (SemOS) | ⚠️ **Incomplete** | **Bootstrap surface dead code (BH-2)** |
| Request → Plan (Runbook) | ✅ Connected | Post-expansion validation missing (BH-7) |
| Plan → Execution (Bridges) | ✅ Connected | None |
| Result → Completion (Session) | ✅ Connected | Park reason not exposed to UI |

---

## 7. Final Judgement

**80–85% complete.** The architecture is sound, the pipeline is connected end-to-end, and the DAG/planning/execution layer is production-grade. The main risk is the **entity-kind→verb-selection join** where all building blocks exist but the final wiring is missing. For the **CBU/structure onboarding + KYC** primary use case, coverage is **~95% complete**. For secondary domains (trading, billing, deal, contract, custody), verbs are executable but **semantically undiscoverable** through structured workflows.

The gap between "architecture complete" and "end-to-end reliable" is approximately **2–3 focused work items**: entity-kind threading, discovery surface wiring, and canonical kind expansion. None require architectural changes.
