# Semantic OS End-to-End Coverage Reconciliation Review

**Date:** 2026-03-18
**Scope:** Read-only analysis of implemented semantic pipeline coverage
**Method:** Evidence-based codebase inspection across 4 pillars

---

## 1. Executive Summary

### Overall Assessment

The Semantic OS has **robust infrastructure** — the plumbing from utterance ingress through verb search, compilation, advisory locking, and execution is well-engineered and fail-safe. However, the **semantic content** that populates that infrastructure is materially incomplete. The system has a sophisticated 12-step context resolution pipeline, a 22-constellation map library, a 16-scenario journey index, and ~1,263 verb definitions — but the **joins between these layers** are sparse, inconsistent, and in several cases broken by vocabulary mismatches.

**Headline verdict:** The infrastructure supports reliable end-to-end execution for ~40-45% of valid business utterances. The remaining ~55-60% fall into black holes caused by:
- Discovery structures that are nearly empty (1 of ~8 needed universe seeds, 1 of ~22 constellation family routes)
- Entity kind vocabulary mismatches between pillars (6 confirmed inconsistencies)
- Verb metadata gaps (harm_class, subject_kinds, phase_tags) that prevent governance filtering from working correctly
- The Sage/Coder fast path operating at 5% accuracy, forcing fallback to a legacy pipeline at 43%

**The four pillars are individually partially complete but materially fragmented in their alignment.**

---

## 2. Pillar-by-Pillar Assessment

### 2A. Entities

**Current State:** The system defines entity kinds across 4 independent vocabularies that do not fully align: (1) verb YAML `subject_kinds`, (2) `noun_index.yaml` `entity_type_fqn`, (3) scanner `domain_to_subject_kind()` heuristic, (4) constellation slot `entity_kinds`.

**Strengths:**
- Core entities (`cbu`, `entity`, `deal`, `trading-profile`, `document`, `billing-profile`) are consistent across noun_index, subject_kinds, and domain_to_subject_kind heuristic
- Entity linking service provides snapshot-based mention extraction with evidence trails
- 9 taxonomy seeds provide classification hierarchies for entity types, jurisdictions, risk tiers, document classes

**Weaknesses:**
- **6 confirmed vocabulary mismatches** between pillars:
  - `fund` vs `entity.fund` (verb YAML uses bare `fund`; context_resolution tests expect dotted `entity.fund`)
  - `kyc_case` (noun_index underscore) vs `kyc-case` (domain heuristic hyphen)
  - `investor` (noun_index) vs `investor-register` (domain heuristic)
  - `company`, `legal_entity`, `person`, `client` exist in verb subject_kinds but are unreachable via `domain_to_subject_kind()` — only present when explicitly declared in YAML
  - `organization` exists only in constellation slot `entity_kinds` — not in any verb contract or search index
  - `person` in constellation slots uses a different vocabulary from verb subject_kinds

**Notable Gaps:**
- 15+ entity types in noun_index (`isda`, `settlement-chain`, `sla`, `pricing-config`, `control`, `ownership`, `capital`, `tollgate`, `tax-config`, `graph`, `team`, `role`, `attribute`, `lifecycle`, `client-group`) have no corresponding `subject_kinds` declarations on any verb — they are discoverable at Tier -1 (ECIR) but cannot be used by context_resolution to narrow the verb surface
- Semantic Registry object types (`attribute_def`, `entity_type_def`, `verb_contract`, etc.) bypass entity-kind filtering entirely (`subject_kinds: []`)

**Confidence Level:** HIGH — findings are based on exhaustive source inspection of noun_index.yaml, scanner.rs, context_resolution.rs, all verb YAML files with subject_kinds, and constellation map seeds.

---

### 2B. DSL Verbs / Actions

**Current State:** ~1,263 verbs across ~136 YAML files covering ~25+ domains. 608 plugin verbs with 580 registered CustomOp implementations. Startup verification (`verify_plugin_verb_coverage`) prevents serving with unimplemented plugin verbs.

**Strengths:**
- Core business domains (cbu, entity, kyc, ubo, trading-profile, deal, billing, gleif, ownership) have complete YAML + CustomOp + SemReg scan coverage
- CRUD domains (fund, contract, isda, ssi, holding, etc.) work through the generic executor without requiring CustomOps
- 15,940 embeddings provide semantic search coverage for all 1,263 verbs
- Verb phrase generation auto-produces ≥8 invocation phrases per verb

**Weaknesses:**
- **`client.yaml` — 12 plugin verbs with no CustomOp implementation.** Uses a non-standard YAML schema (`plugin:` field instead of `behavior: plugin`). Either these verbs never load (best case) or load and fail at runtime (worst case). This is a **critical gap** if client-facing portal operations are intended to work.
- **`harm_class` — populated on only 2 of 136 YAML files.** PolicyGate cannot make harm-level routing decisions. Destructive verbs (`cbu.delete`, `entity.delete`, `case.reject`) are not flagged.
- **`subject_kinds` — declared on only 24 of 136 YAML files.** The scanner's 3-level fallback heuristic covers ~60% of domains. The remaining ~40% get incorrect or empty subject_kinds, causing entity-kind filtering to be either wrong or a no-op.
- **`phase_tags` — missing on `state`, `constellation`, `bpmn` domains.** These verbs will not appear in any workflow-scoped session (semos-onboarding, semos-kyc, etc.), making them undiscoverable through the Semantic OS Tab.
- **`access-review.yaml.disabled` — 8 verbs silently unavailable.** No implementation exists.
- **Governed tier — only 3 domains** (`kyc-case`, `entity-workstream`, `ubo.registry`). Business-sensitive domains (deal, billing, cbu lifecycle) operate at Operational tier without governed approval gates.

**Notable Gaps:**
- Macro verb reference validation happens only via `cargo x verbs lint-macros` (offline), not at startup. A macro referencing a non-existent primitive verb will fail at runtime, not at boot.
- The `VerbContractBody.action_class` field is mostly unpopulated, limiting the AffinityGraph's ability to classify verb impacts.

**Confidence Level:** HIGH — based on exhaustive grep of `#[register_custom_op]`, all verb YAML files, scanner.rs fallback logic, and runtime_registry.rs.

---

### 2C. Constellations / Discovery Structures

**Current State:** The discovery infrastructure is fully implemented in Rust (universe/family/constellation types, scoring, grounding thresholds, DiscoverySurface rendering) but the **seed content** is nearly empty.

**Strengths:**
- 22 constellation maps covering LU (3), IE (3), UK (6), US (4), cross-border (2) fund structures — comprehensive jurisdiction coverage
- 16 scenario index entries enabling compound utterance → macro resolution with deterministic scoring
- ~54 macros across 18 YAML files covering structure setup, KYC workflows, screening, party management, UBO discovery
- 4 REPL V2 packs (session-bootstrap, book-setup, kyc-case, onboarding-request)
- All constellation slots have uniform verb surfaces (ensure, assign, search, add, show) with state machine references

**Weaknesses (CRITICAL):**
- **1 universe seed** (`client_lifecycle` with only `onboarding` domain). No universes for KYC-standalone, data management, stewardship, trading/mandate, custody, or deal workflows.
- **1 constellation family** (`fund_onboarding` routing only to IE UCITS ICAV). **21 of 22 constellation maps are completely unreachable through the family discovery path.** The `DiscoverySurface` machinery works correctly but has almost nothing to match against.
- **10 of 22 constellations have no scenario index entry** (UK ACS/AUT/LTAF/LLP/PE-LP, US closed-end/ETF/Delaware-LP, IE AIF/Hedge). These structures are not discoverable via compound utterances.
- **Pack coverage is minimal** — `book-setup` only covers LU SICAV and UK OEIC. No packs for US, IE, AIF/RAIF, hedge, PE, cross-border, stewardship, or data management workflows.
- **The `registry.discover-dsl` verb** uses token-overlap Jaccard scoring (not embeddings) against verb invocation phrases — a fundamentally different discovery mechanism from the `HybridVerbSearcher` (BGE asymmetric embeddings). The two paths may produce different rankings for the same utterance.

**Notable Gaps:**
- The generic-fund-onboarding scenario selector routes to only 4 default structures (LU→SICAV, IE→ICAV, UK→OEIC, US→40Act-open-end). All other fund types require the user to name the specific structure.
- No state machine for mandate lifecycle or deal lifecycle — these entities have no constellation-backed semantic surface.

**Confidence Level:** HIGH — based on exhaustive reading of all universe, family, constellation, scenario, macro, and pack YAML seed files plus the discovery pipeline source code.

---

### 2D. DAG / Planning / Execution

**Current State:** The execution pipeline is the strongest pillar. Advisory locking, compiled runbooks, TOCTOU recheck, and SemReg pre-constraint are all production-hardened.

**Strengths:**
- `execute_runbook_with_pool` is the sole execution entry point — no bypass paths
- Advisory locks on write_set entity IDs prevent concurrent mutation races
- Compiled runbooks are immutable and content-addressed
- TOCTOU recheck catches verb revocation between intent resolution and execution
- Macro expansion has fixpoint convergence with cycle detection (max_depth=8, max_steps=500)
- `verify_plugin_verb_coverage` prevents startup with unimplemented handlers
- Three-level graceful degradation: NLCI compiler → CoderEngine → IntentPipeline

**Weaknesses:**
- **InMemoryRunbookStore is the default** — parked runbooks (awaiting BPMN callbacks) do not survive server restarts. The `bpmn_correlations` Postgres table can outlive the in-memory state it references.
- **REPL V2 and chat path maintain separate runbook state.** A session using `kind=utterance` cannot see or resume `kind=repl_v2` runbook entries. The two orchestrators share scope (`context.cbu_ids`) but not execution history.
- **Legacy compile-on-the-fly REPL entries** bypass advisory lock acquisition and write_set derivation, creating a race condition window for bulk operations in concurrent sessions.
- **Macro verb reference validation is offline only** (`cargo x verbs lint-macros`). Startup `verify_plugin_verb_coverage` checks plugin verb→CustomOp binding but not macro expansion→primitive verb existence.

**Notable Gaps:**
- The proposal engine (`ProposalEngine`) generates next-step proposals but only within the REPL V2 path. The chat path (`legacy_handle_utterance`) has no multi-step planning capability — each turn is a single verb.
- The constellation panel's "Ask Agent" / "Ask Why Blocked" buttons submit structured prompts through the full utterance pipeline, but if the resulting verb is ABAC-excluded, the UI has no feedback loop to explain why the slot is blocked.

**Confidence Level:** HIGH — based on reading orchestrator.rs, runbook/compiler.rs, runbook/executor.rs, expander.rs, and proposal_engine.rs.

---

## 3. Cross-Pillar Reconciliation Findings

### 3.1 Entities Without Adequate Verbs

| Entity Kind | In Noun Index | Has Verb subject_kinds | Verbs Execute | Gap |
|---|:---:|:---:|:---:|---|
| `client-group` | ✅ | ❌ | Partially (session.load-cluster) | No verbs declare `subject_kinds: [client-group]`; entity-kind filtering cannot narrow to this type |
| `isda` | ✅ | ❌ | ✅ (CRUD) | Verbs exist but no subject_kinds → context_resolution cannot use entity kind to scope |
| `settlement-chain` | ✅ | ❌ | ✅ (CRUD) | Same as isda |
| `sla` | ✅ | ❌ | ✅ (CRUD) | Same |
| `tollgate` | ✅ | ❌ | ✅ (plugin) | Constellation slot references tollgate but no verb declares subject_kinds for it |
| `capital` | ✅ | ❌ | Partial | Noun index routes utterances to capital.* verbs but entity-kind filtering is a no-op |
| `organization` | ❌ (constellation only) | ❌ | N/A | Exists only as a constellation slot discriminator; no semantic pipeline can route to it |

### 3.2 Verbs Without Adequate Entity Support

| Verb Domain | Has CustomOps | Entity Resolution | Gap |
|---|:---:|:---:|---|
| `client.*` (12 verbs) | ❌ | N/A | **Critical:** Plugin verbs with no implementation; likely never load due to non-standard YAML schema |
| `state.*` (8 verbs) | ✅ | ❌ | No subject_kinds, no phase_tags → not discoverable in workflow sessions, no entity-kind scoping |
| `constellation.*` (2 verbs) | ✅ | ❌ | No subject_kinds, no phase_tags → constellation verbs themselves are not discoverable through Sem OS workflow scoping |
| `access-review.*` (8 verbs) | ❌ | ❌ | File disabled; entirely non-functional |

### 3.3 Capabilities Without Discovery Exposure

| Capability | Runtime Exists | Scenario/Macro | Universe/Family | Pack |
|---|:---:|:---:|:---:|:---:|
| UK ACS fund setup | ✅ constellation map | ❌ no scenario | ❌ | ❌ |
| UK AUT fund setup | ✅ constellation map | ❌ | ❌ | ❌ |
| UK LTAF fund setup | ✅ constellation map | ❌ | ❌ | ❌ |
| UK Manager LLP | ✅ constellation map | ❌ | ❌ | ❌ |
| UK PE LP | ✅ constellation map | ❌ | ❌ | ❌ |
| US Closed-End fund | ✅ constellation map | ❌ | ❌ | ❌ |
| US ETF | ✅ constellation map | ❌ | ❌ | ❌ |
| US Delaware LP | ✅ constellation map | ❌ | ❌ | ❌ |
| IE AIF ICAV | ✅ constellation map | ❌ | ❌ | ❌ |
| IE Hedge ICAV | ✅ constellation map | ❌ | ❌ | ❌ |
| Deal lifecycle | ✅ (42 verbs) | ❌ | ❌ | ❌ |
| Billing lifecycle | ✅ (14 verbs) | ❌ | ❌ | ❌ |
| Trading profile setup | ✅ (~32 verbs) | ❌ | ❌ | ❌ |
| Stewardship workflow | ✅ (23 MCP tools) | ❌ | ❌ | ❌ |
| Data management authoring | ✅ (7 governance verbs) | ❌ | ❌ | ❌ |

### 3.4 Discoverable Intents Without Executable DAG Support

| Discoverable Path | Execution Support | Gap |
|---|---|---|
| Compound utterance "Full KYC onboarding" → `macro_sequence [case.open, screening.full, kyc.collect-documents]` | ✅ Sequence expansion produces multi-macro runbook | None — well-connected |
| Compound "Set up Luxembourg SICAV" → `struct.lux.ucits.sicav` macro | ✅ Macro expansion → multi-step DSL | None |
| Sem OS discovery "What are you onboarding?" (entry question) | Partial: selection stored but next turn must complete grounding | Extra turn required; if user abandons the selection flow, session is stuck in discovery stage |
| Generic utterance "set up a fund" → generic-fund-onboarding selector → jurisdiction prompt | ✅ DecisionPacket routes to correct constellation | None for covered jurisdictions; gap for uncovered fund types |

### 3.5 Planner/Runtime Paths Without Discovery Support

| Runtime Path | Discovery Reachable | Gap |
|---|---|---|
| BPMN durable workflow (WorkflowDispatcher → Parked → JobWorker → Signal) | Only via `bpmn.start` verb — no natural language discovery | Users cannot say "start a durable workflow"; must know verb FQN or use DSL directly |
| REPL V2 pack-guided execution | Only via `kind=repl_v2` input type | Chat users cannot access pack-guided multi-step planning; REPL is a separate UI mode |
| State reducer overrides (`state.set-override`, `state.clear-override`) | Verb exists but no phase_tags → excluded from workflow sessions | Cannot be discovered through Sem OS Tab workflows |

---

## 4. Black Holes and Failure Modes

### BH-1: Discovery Bootstrap Dead End (CRITICAL)

**Failure point:** Sem OS discovery stage → universe/family matching
**Why:** Only 1 universe (onboarding) and 1 family (fund_onboarding/IE) are seeded. Any non-onboarding intent (KYC standalone, data management, stewardship, deal, billing) enters discovery and finds no matching universe → `DiscoverySurface.matched_universes = []` → UI shows no actionable options.
**Symptom:** User in Semantic OS Tab selects "KYC" workflow → session enters discovery stage → no universes match → `grounding_readiness: NotReady` → no entry questions shown → user is stuck.
**Severity:** CRITICAL — affects all non-onboarding Sem OS Tab workflows.
**Domain:** All except fund onboarding.

### BH-2: Entity Kind Vocabulary Mismatch (HIGH)

**Failure point:** Entity linking returns kind → `filter_and_rank_verbs(entity_kind)` → verb subject_kinds comparison
**Why:** 6 confirmed mismatches between entity linker vocabulary and verb subject_kinds vocabulary (fund/entity.fund, kyc_case/kyc-case, investor/investor-register, company/person/legal_entity not in domain heuristic, organization constellation-only).
**Symptom:** User works with a `fund` entity → entity linker resolves kind `fund` → verb surface correctly filters to fund.* verbs. But if entity linker returns `entity.fund` (dotted) → no verb matches → `NoAllowedVerbs`. Or: user works with a KYC case → entity linker returns `kyc_case` (underscore) → verb subject_kinds uses `kyc-case` (hyphen) → no match.
**Severity:** HIGH — intermittent failures depending on entity linker output format.
**Domain:** fund, kyc, investor, gleif (company/legal_entity entities).

### BH-3: Constellation Family Routing Gap (HIGH)

**Failure point:** Sem OS discovery → family matching → constellation selection
**Why:** Only 1 of 22 constellations is reachable through the family routing path (IE UCITS ICAV). All others require either: (a) scenario index compound utterance match, or (b) direct macro FQN knowledge.
**Symptom:** User in discovery flow answers "objective: fund setup, jurisdiction: LU" → family `fund_onboarding` has no LU selection rule → `matched_constellations = []` → user cannot select a constellation → stuck in discovery.
**Severity:** HIGH — blocks guided constellation discovery for 21 of 22 structures.
**Domain:** All fund structure setup except IE UCITS ICAV.

### BH-4: Unimplemented Client Verbs (HIGH)

**Failure point:** Verb search → `client.*` verb match → CustomOp dispatch
**Why:** 12 plugin verbs in `client.yaml` use a non-standard schema format with no `#[register_custom_op]` implementation.
**Symptom:** If these verbs load: user utterance matches `client.submit-document` → compile succeeds → execution fails with "No CustomOp found for client/submit-document". If they don't load: the verbs are invisible but the YAML file's existence suggests intended capability.
**Severity:** HIGH if verbs are loadable; MEDIUM if YAML schema prevents loading.
**Domain:** Client portal operations.

### BH-5: Sage/Coder Accuracy Floor (MEDIUM)

**Failure point:** Sage classification → Coder verb resolution → fast path gate
**Why:** GATE 5 measurement shows Sage+Coder at 5.22% vs legacy pipeline at 43.28%. The fast path fires only for `Structure + Read` polarity with high/medium confidence and no missing args — a narrow slice of traffic.
**Symptom:** Most utterances bypass the Sage fast path entirely, falling to `legacy_handle_utterance` where `sage_enabled = false`. The new classification system provides near-zero benefit for the majority of traffic.
**Severity:** MEDIUM — the fallback to legacy pipeline works, but the investment in Sage/Coder is not yet yielding returns.
**Domain:** All.

### BH-6: Phase Tag Exclusion (MEDIUM)

**Failure point:** `compute_session_verb_surface()` → workflow phase filter → verb excluded
**Why:** `state.*`, `constellation.*`, `bpmn.*` verbs have no `phase_tags` → excluded from all workflow-scoped sessions.
**Symptom:** User in "semos-onboarding" workflow asks "show the constellation" → `constellation.hydrate` has no phase_tags → excluded from verb surface → `NoMatch`. User must exit Sem OS Tab and use chat to access these verbs.
**Severity:** MEDIUM — workaround exists (use chat), but Sem OS Tab experience is incomplete.
**Domain:** state, constellation, bpmn.

### BH-7: Harm Class Absence (LOW-MEDIUM)

**Failure point:** PolicyGate → harm-level routing
**Why:** Only 2 of 136 YAML files have `harm_class` populated. Destructive verbs are not flagged.
**Symptom:** `cbu.delete` is treated identically to `cbu.list` by the governance pipeline. No extra confirmation or escalation is triggered for destructive operations.
**Severity:** LOW-MEDIUM — the verb still executes correctly, but governance posture is weaker than intended.
**Domain:** All domains with destructive verbs.

---

## 5. Coverage Matrix

| Domain Area | Entity Kinds | Verb Surface | Discovery (Scenario/Macro) | Discovery (Universe/Family) | Pack Support | DAG/Execution | E2E Status |
|---|---|---|---|---|---|---|---|
| **CBU lifecycle** | ✅ Consistent | ✅ Complete | ✅ Scenarios + macros | ❌ Family gap (only IE) | ✅ book-setup (LU/UK only) | ✅ Robust | **Partial** — discovery routing incomplete |
| **Entity management** | ⚠️ 3 kinds not in heuristic | ✅ Complete | ✅ Via structure macros | ❌ Family gap | ❌ | ✅ | **Partial** — entity kind mismatches |
| **KYC case** | ⚠️ kyc_case/kyc-case mismatch | ✅ Governed | ✅ 3 KYC scenarios | ❌ No KYC universe | ✅ kyc-case pack | ✅ | **Partial** — vocabulary mismatch |
| **UBO discovery** | ✅ | ✅ Governed | ✅ Via macros | ❌ | ❌ | ✅ | **Partial** — no discovery bootstrap |
| **Screening** | ⚠️ No subject_kinds | ✅ Complete | ✅ screening.full scenario | ❌ No universe | ❌ | ✅ | **Partial** |
| **Deal lifecycle** | ✅ Consistent | ✅ Complete (42 verbs) | ❌ No scenarios/macros | ❌ No universe | ❌ No pack | ✅ | **Missing** — no discovery path |
| **Billing** | ⚠️ No subject_kinds | ✅ Complete (14 verbs) | ❌ | ❌ | ❌ | ✅ | **Missing** — no discovery path |
| **Trading profile** | ✅ Consistent | ✅ Complete (~32 verbs) | ❌ No scenarios/macros | ❌ No universe | ❌ No pack | ✅ | **Missing** — no discovery path |
| **Fund structures** | ⚠️ fund/entity.fund mismatch | ✅ CRUD | ✅ 8 structure scenarios | ⚠️ 1/22 family route | ⚠️ 2 templates | ✅ | **Partial** — family routing critical gap |
| **GLEIF/research** | ✅ | ✅ Complete | ✅ Via macros | ❌ | ❌ | ✅ | **Partial** |
| **Contract** | ⚠️ No subject_kinds | ✅ CRUD (14 verbs) | ❌ | ❌ | ❌ | ✅ | **Missing** — no discovery path |
| **Custody (ISDA/SSI)** | ⚠️ No subject_kinds | ✅ CRUD | ❌ | ❌ | ❌ | ✅ | **Missing** — no discovery path |
| **Stewardship** | N/A (registry ops) | ✅ 23 MCP tools | ❌ | ❌ No universe | ❌ No pack | ✅ | **Missing** — no discovery path |
| **Data management** | N/A (registry ops) | ✅ 7 governance verbs | ❌ | ❌ No universe | ❌ No pack | ✅ | **Missing** — no discovery path |
| **Client portal** | ❌ | ❌ 12 unimplemented | ❌ | ❌ | ❌ | ❌ | **Non-functional** |
| **State reducers** | ❌ No subject_kinds | ✅ 8 plugin verbs | ❌ | ❌ | ❌ | ✅ | **Execution works; undiscoverable** |
| **BPMN durable** | N/A (process-level) | ✅ 5 control verbs | ❌ | ❌ | ❌ | ✅ | **Execution works; undiscoverable** |

---

## 6. Priority Gaps

### Critical Gaps (blocking reliable agent-driven workflows)

| # | Gap | Pillars | Impact |
|---|---|---|---|
| C1 | **Universe/family seed coverage** — 1 of ~8 needed universes, 1 of ~22 family routes | Discovery | All non-IE-onboarding Sem OS workflows are dead-ended |
| C2 | **Entity kind vocabulary mismatches** — 6 confirmed cross-pillar inconsistencies | Entity ↔ Verb | Intermittent verb filtering failures for fund, KYC, investor, company entities |
| C3 | **Client verbs unimplemented** — 12 plugin verbs without CustomOps | Verb ↔ Execution | Client portal operations non-functional |

### Important but Non-Blocking Gaps

| # | Gap | Pillars | Impact |
|---|---|---|---|
| I1 | **Scenario coverage** — 10 of 22 constellations have no scenario entry | Discovery | These fund structures require exact naming to discover |
| I2 | **Phase tags missing** on state/constellation/bpmn domains | Verb ↔ Discovery | Verbs excluded from workflow-scoped sessions |
| I3 | **Subject_kinds sparse** — 24 of 136 YAML files declare them | Entity ↔ Verb | Context resolution entity-kind filtering is a no-op for ~40% of verbs |
| I4 | **Pack coverage** — 4 packs cover ~3 of 22 structure types | Discovery ↔ DAG | REPL V2 guided workflows only work for LU SICAV, UK OEIC |
| I5 | **Deal/billing/trading-profile have zero discovery path** | Discovery | 88 verbs are only reachable via direct utterance matching, not through any guided flow |
| I6 | **harm_class absent** on 134 of 136 YAML files | Verb ↔ Governance | PolicyGate cannot distinguish destructive from read-only operations |

### Later-Stage Refinement

| # | Gap | Pillars | Impact |
|---|---|---|---|
| L1 | **Sage/Coder accuracy** at 5.22% | Pipeline | Fast path rarely fires; legacy pipeline handles majority of traffic |
| L2 | **REPL V2 / chat path divergence** | DAG | Sessions cannot transfer between guided (REPL) and freeform (chat) modes |
| L3 | **InMemoryRunbookStore** as default | Execution | Parked runbooks lost on restart |
| L4 | **Macro reference validation offline only** | Execution | Broken macro expansions surface at runtime, not at boot |
| L5 | **Constellation panel blocking reason** not surfaced in UI | Discovery ↔ Execution | Users see "blocked" status with no explanation |
| L6 | **access-review.yaml.disabled** — 8 verbs with unclear intent | Verb | Dead code or planned feature; should be decided |

---

## 7. Recommended TODO Plan

### Phase 1: Vocabulary Alignment (closes C2, I3)

**Objective:** Establish a single canonical entity-kind vocabulary across all pillars.

| Task | Pillar(s) | Impact | Complexity |
|---|---|---|---|
| 1a. Create `entity_kind_canonical.yaml` mapping file: define the authoritative kind strings used everywhere | Entity | Foundation for all other fixes | Low |
| 1b. Fix `kyc_case` → `kyc-case` in noun_index.yaml (or vice versa — pick one) | Entity ↔ Verb | Fixes KYC entity-kind filtering | Low |
| 1c. Fix `investor` → `investor-register` in noun_index.yaml (or vice versa) | Entity ↔ Verb | Fixes investor verb scoping | Low |
| 1d. Add `company`, `legal_entity`, `person`, `client`, `fund` to `domain_to_subject_kind()` explicit mappings | Entity ↔ Verb | Ensures scanner produces correct kinds for all important domains | Low |
| 1e. Align constellation slot `entity_kinds` vocabulary with canonical kinds (e.g., `organization` → `company` or add bridge) | Entity ↔ Discovery | Constellation slots use resolvable entity kinds | Low |
| 1f. Add `subject_kinds` declarations to the 20 most important verb YAML files that lack them (screening, billing, state, constellation, bpmn, contract, isda, ssi, etc.) | Verb | Context resolution can narrow verb surface for these domains | Medium |

**Sequencing:** Do 1a first (canonical reference), then 1b-1e in parallel, then 1f.

### Phase 2: Discovery Seed Backfill (closes C1, I1, I4, I5)

**Objective:** Populate the discovery pipeline with enough seed content to support all major workflow domains.

| Task | Pillar(s) | Impact | Complexity |
|---|---|---|---|
| 2a. Add universe seeds for: `kyc_lifecycle`, `deal_lifecycle`, `stewardship`, `data_management`, `trading_mandate` | Discovery | Sem OS Tab workflows for KYC, deal, stewardship, data mgmt, mandates become functional | Medium |
| 2b. Add constellation family seeds with selection rules for all 22 constellation maps, grouped by jurisdiction | Discovery | All fund structures become reachable through guided discovery | Medium |
| 2c. Add scenario index entries for the 10 uncovered constellations (UK ACS/AUT/LTAF/LLP/PE-LP, US closed-end/ETF/Delaware-LP, IE AIF/Hedge) | Discovery | Compound utterances can discover all fund structures | Low |
| 2d. Add non-fund scenario entries for deal, billing, trading-profile domains (e.g., "set up a deal", "create billing profile", "configure mandate") | Discovery | Major business domains become discoverable via compound utterances | Medium |
| 2e. Add packs for US fund setup, IE fund setup, cross-border setup, deal lifecycle, stewardship workflow | Discovery ↔ DAG | REPL V2 guided workflows cover all major domains | High |

**Sequencing:** 2a and 2c can run in parallel. 2b depends on 2a. 2d and 2e can run in parallel with 2a-2c.

### Phase 3: Verb Metadata Enrichment (closes I2, I6)

**Objective:** Complete the verb metadata required for governance routing and workflow scoping.

| Task | Pillar(s) | Impact | Complexity |
|---|---|---|---|
| 3a. Add `phase_tags` to `state.*`, `constellation.*`, `bpmn.*` verb YAML files | Verb ↔ Discovery | These verbs appear in workflow-scoped sessions | Low |
| 3b. Add `harm_class` to all destructive verbs (delete, terminate, reject, revoke, close, cascade) across all domains | Verb ↔ Governance | PolicyGate can enforce harm-level routing decisions | Medium |
| 3c. Add `phase_tags` to deal, billing, trading-profile verbs (e.g., `phase_tags: [commercial]` for deal, `phase_tags: [onboarding, commercial]` for billing) | Verb ↔ Discovery | These verbs appear in the correct workflow-scoped sessions | Medium |

**Sequencing:** 3a-3c can run in parallel. Depends on Phase 1 canonical vocabulary being established.

### Phase 4: Implementation Cleanup (closes C3, L4, L6)

**Objective:** Resolve non-functional verbs and offline-only validation gaps.

| Task | Pillar(s) | Impact | Complexity |
|---|---|---|---|
| 4a. Decide on `client.yaml` — either implement `client_ops.rs` or remove the YAML file | Verb ↔ Execution | Eliminates non-functional verb surface | Medium (if implement) / Low (if remove) |
| 4b. Add macro verb reference validation to startup `verify_plugin_verb_coverage` or to a pre-deployment gate | Execution | Broken macro expansions caught before runtime | Medium |
| 4c. Decide on `access-review.yaml.disabled` — implement or permanently remove | Verb | Eliminates dead code ambiguity | Low |

### Phase 5: Pipeline Accuracy (addresses L1, L2)

**Objective:** Improve the Sage/Coder fast path accuracy and resolve chat/REPL divergence.

| Task | Pillar(s) | Impact | Complexity |
|---|---|---|---|
| 5a. Expand NLCI compiler entity coverage beyond `cbu|entity|kyc-case|ubo` to include deal, trading-profile, contract | Pipeline | More utterances can use the fast path | High |
| 5b. Improve StateGraph verb edges for entity types with empty `valid_verbs` | Pipeline | Sage Step 2 produces actionable verb narrowing for more entity types | High |
| 5c. Design session mode bridge allowing chat↔REPL transitions without losing runbook state | DAG | Users can switch between freeform and guided modes | High |

**Sequencing:** Phase 5 is post-stabilization. Phases 1-3 should be complete first.

---

## 8. Final Judgement

**How close is the current Semantic OS to supporting reliable end-to-end agent utterance → REPL/DSL execution without major semantic black holes?**

**Answer: Approximately 40-45% of the way there for guided workflows; approximately 60-65% for direct utterance-to-verb matching.**

The **execution infrastructure** is production-grade: advisory locking, compiled runbooks, TOCTOU recheck, SemReg pre-constraint, and fail-safe degradation chains are all correctly implemented and well-tested. If a verb is correctly resolved and compiled, it will execute reliably.

The **intent resolution pipeline** (HybridVerbSearcher with 10 tiers) is reasonably effective for single-verb utterances — the 43% first-attempt accuracy with 71% two-attempt accuracy on the 134-case harness demonstrates this. For compound utterances, the ScenarioIndex + MacroIndex add deterministic routing for the 16 authored scenarios.

The **critical gap is in the semantic discovery layer** — the universes, families, and constellation routing that should guide users from vague business intent to specific executable capability. With 1 universe, 1 family route, and 10 constellation gaps, the Sem OS Tab and guided discovery workflows are largely non-functional outside the narrow IE UCITS ICAV onboarding path. This means the system works well when users already know approximately what verb they want (direct utterance matching) but fails when they need semantic guidance to discover available capability.

The **entity vocabulary mismatches** (6 confirmed) create intermittent failures that are difficult to diagnose — the system may work correctly for one entity type but fail silently for another due to string comparison mismatches between pillars.

**In concrete terms:**
- "Load the Allianz book" → ✅ works reliably (session.load-galaxy, well-connected)
- "Set up a Luxembourg SICAV" → ✅ works (compound utterance → scenario → macro → execution)
- "Open a KYC case for this entity" → ⚠️ works if entity kind resolves correctly; may fail on kyc_case/kyc-case mismatch
- "Help me with KYC" (via Sem OS Tab) → ❌ discovery dead-end (no KYC universe seeded)
- "Set up a deal for Allianz" → ⚠️ works via direct verb matching; no guided discovery path
- "Onboard a UK LTAF" → ❌ no scenario, no family route, no pack — requires exact macro FQN knowledge
- "Submit a client document" → ❌ client verbs unimplemented

**The highest-leverage remediation is Phase 2 (discovery seed backfill) — it turns the fully-implemented but nearly-empty discovery infrastructure into a functional semantic guidance layer. Phase 1 (vocabulary alignment) is the prerequisite to prevent the new seeds from inheriting the existing vocabulary inconsistencies.**
