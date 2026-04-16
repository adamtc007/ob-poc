# DSL Pipeline, Verb Search & Intent Resolution — Detailed Annex

> This annex covers the DSL pipeline, verb search tiers, embeddings, intent resolution,
> disambiguation, teaching/promotion, scenarios, AffinityGraph, and discovery.
> For the high-level overview see the root `CLAUDE.md`.

---

## DSL Pipeline (Single Path)

ALL user input flows through one unified pipeline:

```
User Utterance
  → IntentPipeline.process_with_scope()
      → SemOsContextEnvelope (CCIR — allowed verbs)
      → SessionVerbSurface (multi-layer governance)
      → HybridVerbSearcher (10-tier search, pre-constrained by allowed verbs)
      → LLM arg extraction (Anthropic, 200–500ms)
      → Deterministic DSL assembly
      → TOCTOU recheck
      → Execution
```

**Key files:**

| File | Purpose |
|------|---------|
| `rust/src/agent/orchestrator.rs` | Unified entry point — `handle_utterance()` |
| `rust/src/mcp/intent_pipeline.rs` | `IntentPipeline`, `StructuredIntent`, `PipelineResult` |
| `rust/src/mcp/verb_search.rs` | `HybridVerbSearcher`, `VerbSearchResult`, `VerbSearchOutcome` |
| `rust/src/agent/sem_os_context_envelope.rs` | `SemOsContextEnvelope`, `PrunedVerb`, `PruneReason` |
| `rust/src/agent/verb_surface.rs` | `SessionVerbSurface`, `SurfaceVerb`, `VerbSurfaceFailPolicy` |
| `rust/src/mcp/scenario_index.rs` | Tier -2A: `ScenarioIndex`, `ScenarioDefYaml`, `ResolvedRoute` |
| `rust/src/mcp/macro_index.rs` | Tier -2B: `MacroIndex`, `MacroResolveOutcome` |
| `rust/src/mcp/compound_intent.rs` | `CompoundSignals` (11 signal types) |
| `rust/src/mcp/sequence_validator.rs` | Validates macro sequences |
| `rust/src/mcp/enrichment.rs` | `EntityEnricher`, `EntityContext` |

---

## Boundary Status Update (2026-04-16)

The DSL capability is still less cleanly standalone than Sem OS, but the tooling boundary has started to tighten around explicit facades rather than deep module exposure.

- `dsl-core` remains the pure parser/compiler kernel
- `dsl-lsp` now presents a narrower root-level surface (`DslLanguageServer`, encoding helpers, entity lookup types, and a single `analyze_document` seam for tests)
- `dsl-lsp` handler, analysis, and server module trees are no longer intended public API

The remaining structural issue is not in `dsl-lsp`; it is the broad `ob_poc::dsl_v2` public shape, which still mixes compiler, runtime, planning, and tooling concerns. That needs a later capability-boundary pass rather than another hygiene sweep.

---

## Orchestrator Stages

**File:** `rust/src/agent/orchestrator.rs`

**Entry:** `handle_utterance(context, utterance)`

| Stage | Action |
|-------|--------|
| 0 | Scope resolution (session context) |
| 1 | Entity linking (LookupService) + Sage intent (observation plane) |
| 1.5 | Sage classification (read / write / ambiguous polarity) |
| 2 | Sem OS context resolution → `SemOsContextEnvelope` |
| 2.5 | `SessionVerbSurface` computation (multi-layer governance) |
| 3 | `HybridVerbSearcher` with allowed-verb pre-constraint |
| 4 | LLM arg extraction (only if needed) |
| 5 | DSL assembly + validation |
| 6 | TOCTOU recheck → execution |

**`PreparedTurnContext` (internal stage carrier):**
```rust
struct PreparedTurnContext {
    lookup_result: Option<LookupResult>,
    dominant_entity_name: Option<String>,
    dominant_entity_kind: Option<String>,
    entity_candidates: Vec<String>,
    sem_reg_verb_names: Option<Vec<String>>,
    envelope: SemOsContextEnvelope,
    surface: SessionVerbSurface,
    composite_state: Option<GroupCompositeState>,
}
```

**Environment gates:**
```bash
OBPOC_STRICT_SINGLE_PIPELINE=true   # PolicyGate enforcement (default: true)
OBPOC_STRICT_SEMREG=true            # SemReg fail-closed (default: true)
OBPOC_ALLOW_RAW_EXECUTE=false       # Block direct DSL bypass
```

---

## 10-Tier Search Priority

```
Tier -2A:  ScenarioIndex              score 0.97   journey-level compound intent
Tier -2B:  MacroIndex                 score 0.96   operator macro search
Tier -0.5: ConstellationVerbIndex     state-gated noun+action lookup from live constellation
Tier  0:   Operator Macros            score 1.0 exact / 0.95 fuzzy
Tier  1:   User Learned Exact         score 1.0
Tier  2:   Global Learned Exact       score 1.0
Tier  3:   User Learned Semantic      pgvector + BGE (threshold 0.80)
Tier  5:   Blocklist Filter           semantic collision detection (threshold 0.80)
Tier  6:   Global Semantic            UNION learned + cold-start patterns (threshold 0.65)
Tier  7:   Phonetic Fallback          dmetaphone (score 0.80)
```

### HybridVerbSearcher

```rust
pub struct HybridVerbSearcher {
    verb_service: Option<Arc<VerbService>>,
    learned_data: Option<SharedLearnedData>,
    embedder: Option<SharedEmbedder>,        // CandleEmbedder BGE-small-en-v1.5
    macro_registry: Option<Arc<MacroRegistry>>,
    lexicon: Option<SharedLexicon>,          // fast lexical search (runs before semantic)
    macro_index: Option<Arc<MacroIndex>>,    // Tier -2B
    scenario_index: Option<Arc<ScenarioIndex>>,  // Tier -2A
    semantic_threshold: f32,    // 0.65
    fallback_threshold: f32,    // 0.55
    blocklist_threshold: f32,   // 0.80
}
```

**Builder chain:**
`.new()` → `.with_embedder()` → `.with_macro_registry()` → `.with_lexicon()`
→ `.with_macro_index()` → `.with_scenario_index()`

**Ambiguity detection:** if `(top.score - runner_up.score) < 0.05` → `Ambiguous`

**`VerbSearchOutcome` variants:**
```rust
pub enum VerbSearchOutcome {
    Matched(VerbSearchResult),
    Ambiguous { top, runner_up, margin: f32 },
    Suggest { candidates: Vec<VerbSearchResult> },
    NoMatch,
}
```

---

## ConstellationVerbIndex (Tier -0.5)

**Replaced:** ECIR / NounIndex (Tier -1) — removed in favor of ConstellationVerbIndex.

The ConstellationVerbIndex provides a two-way (noun, action_stem) to verb lookup built from the live hydrated constellation's state-gated available verbs. It is consulted at Tier -0.5 in HybridVerbSearcher, after scenario/macro tiers but before embedding tiers. Unlike the old static NounIndex, this index reflects the actual constellation state and workspace pack constraints.

**Key file:** `rust/src/agent/constellation_verb_index.rs`

---

## ScenarioIndex — Journey-Level Intent (Tier -2A)

**File:** `rust/src/mcp/scenario_index.rs`
**Config:** `rust/config/scenario_index.yaml` (16 scenarios)

**Scoring ledger:**
```
compound_action (onboard, set up, establish):  +4
jurisdiction found:                             +4
structure noun (sicav, icav, LP):              +3
phase noun (KYC, screening):                   +2
quantifier (three, multiple):                  +2
macro metadata match:                          +3
single-verb cue (penalty):                     -6
```

**Hard gates:**
- G1: Compound signal required (`requires.any_of` or `requires.all_of`)
- G2: Mode compatibility (scenario modes must overlap active mode)
- G3: Minimum score ≥ 8

**Route types:**
```rust
pub enum ScenarioRouteYaml {
    Macro { macro_fqn: String },
    MacroSequence { macros: Vec<String> },
    MacroSelector { select_on: String, options: Vec<SelectorOption>, then: Vec<String> },
}
```

Coverage: Luxembourg (SICAV, UCITS, AIF, RAIF), Ireland (ICAV), UK (OEIC, AUT, ACS, SCSp), KYC/screening workflows (3 macro_sequence routes).

---

## MacroIndex — Operator Macro Search (Tier -2B)

**File:** `rust/src/mcp/macro_index.rs`

**Scoring:**
```
Exact FQN:           +10
Exact label:         +8
Alias/phrase:        +6
Jurisdiction match:  +3
Mode match:          +2
Noun overlap:        +2
Target kind match:   +2
Mismatch penalty:    -999
```

**Hard gates:**
- M1: Mode compatibility (mode_tags must overlap if specified)
- M2: Min score ≥ 6
- M3: Disambiguation band Δ ≤ 2 → return multiple candidates

**Deterministic features:** FQN-to-jurisdiction mapping (LU, IE, UK, US, DE), FQN-to-structure-type extraction.

---

## Compound Intent Extraction

**File:** `rust/src/mcp/compound_intent.rs`

`CompoundSignals` has 11 signal types extracted from the utterance:
- Compound outcome verbs: `onboard`, `set up`, `establish`, `spin up`, `configure`
- Structure nouns: `sicav`, `icav`, `ucits`, `aif`, `raif`, `lp`, `llp`, `slp`, `sca`, `sarl`, `fund`
- Phase nouns: `kyc`, `screening`, `due diligence`, `onboarding`, `compliance`, `aml`, `cdd`, `edd`
- Quantifier patterns: `three`, `four`, `multiple`, `several`, `all`, `each`, `every`, `both`

Signals are extracted before tier routing. When compound signals are present, Tier -2A (ScenarioIndex) evaluates first.

---

## Embeddings — CandleEmbedder & BGE-small-en-v1.5

**File:** `rust/src/agent/learning/embedder.rs`

| Property | Value |
|----------|-------|
| Model | `BGE-small-en-v1.5` |
| Dimensions | 384 |
| Engine | Candle (local, no API) |
| Cache | `~/.cache/huggingface/` (~130MB) |
| Speed | 5–15ms per embedding |
| Mode | Asymmetric: queries get instruction prefix, targets don't |

**API:**
```rust
pub async fn embed_query(&self, text: &str) -> Result<Embedding>   // with prefix
pub async fn embed_target(&self, text: &str) -> Result<Embedding>  // no prefix
pub async fn embed_batch_queries(&self, texts: &[&str]) -> Result<Vec<Embedding>>
pub async fn embed_batch_targets(&self, texts: &[&str]) -> Result<Vec<Embedding>>
// Blocking variants also available
```

**Thresholds (calibrated for BGE asymmetric):**
```
semantic_threshold (decision gate):  0.65
fallback_threshold (retrieval):      0.55
blocklist_threshold (collision):     0.80
```

**Database pattern storage:**
- `dsl_verbs.yaml_intent_patterns` — from YAML `invocation_phrases` (overwritten on startup)
- `dsl_verbs.intent_patterns` — learned from user feedback (preserved across restarts)
- View `v_verb_intent_patterns` = UNION of both
- Table `verb_pattern_embeddings` — 384-dim vectors indexed with pgvector

**Populate embeddings** (required after YAML verb changes):
```bash
cargo x verbs compile && \
DATABASE_URL="postgresql:///data_designer" \
  cargo run --release -p ob-semantic-matcher --bin populate_embeddings
# --force to re-embed all (e.g., after model change)
```

**Current inventory:** 1,418 verbs, ~23,405 total embeddings. Intent hit rate: 52.1% first-attempt, 85.3% two-attempt.

---

## Intent Disambiguation

**Disambiguation triggers when:** `(top.score - runner_up.score) < 0.05`

**`VerbOption` enrichment in Ambiguous responses:**
- `verb_kind` — domain classification
- `differentiation` — what makes this verb distinct
- `entity_context` — current entity state relevant to the choice
- `constellation_slot` — where in the workflow this verb fits

**Entity context structure:**
```rust
pub struct EntityContext {
    pub nationality: Option<String>,
    pub date_of_birth: Option<String>,
    pub jurisdiction: Option<String>,
    pub registration_number: Option<String>,
    pub roles: Vec<RoleContext>,
    pub ownership: Vec<OwnershipContext>,
    pub created_at: String,
    pub last_activity: String,
}
```

---

## Teaching & Promotion Pipeline

**File:** `rust/src/mcp/handlers/learning_tools.rs`

| Operation | Function | Purpose |
|-----------|----------|---------|
| Teach phrase | `teach_phrase(phrase, verb, confidence, tags)` | Promote to learned patterns |
| Block phrase | `intent_block(phrase, blocked_verb, reason, scope, expires)` | Semantic collision detection |
| Bulk import | `learning_import(source, format, scope, dry_run)` | YAML/JSON/CSV bulk load |
| Approve candidates | `learning_approve_candidates()` | Accept learning recommendations |
| Reject candidates | `learning_reject_candidates()` | Mark as noise |
| Stats | `intent_learning_stats()` | Coverage and confidence metrics |

Blocklist entries are embedded in target mode and checked at Tier 5 against incoming queries. Scope: `global` or `user_specific`. Expiry supported.

---

## SemOsContextEnvelope — CCIR Output

**File:** `rust/src/agent/sem_os_context_envelope.rs`

```rust
pub struct SemOsContextEnvelope {
    pub allowed_verbs: HashSet<String>,
    pub allowed_verb_contracts: Vec<VerbCandidateSummary>,
    pub pruned_verbs: Vec<PrunedVerb>,
    pub fingerprint: AllowedVerbSetFingerprint,   // "v1:<sha256>"
    pub evidence_gaps: Vec<String>,
    pub governance_signals: Vec<GovernanceSignalSummary>,
    pub snapshot_set_id: Option<String>,
    pub computed_at: DateTime<Utc>,
    pub resolution_stage: ResolutionStage,
    pub discovery_surface: Option<DiscoverySurface>,
    pub grounded_action_surface: Option<GroundedActionSurface>,
    // deny_all, unavailable: private — use #[cfg(test)] test_with_verbs() for tests
}

pub enum PruneReason {
    AbacDenied { actor_role: String, required: String },
    EntityKindMismatch { verb_kinds: Vec<String>, subject_kind: String },
    TierExcluded { tier: String, reason: String },
    TaxonomyNoOverlap { verb_taxonomies: Vec<String> },
    PreconditionFailed { precondition: String },
    AgentModeBlocked { mode: String },
    PolicyDenied { policy_fqn: String, reason: String },
}
```

**Fingerprint:** `"v1:<hex>"` — SHA-256 of sorted allowed verb FQNs (distinct from `vs1:` surface fingerprint).

**TOCTOU recheck:**
```rust
pub enum TocTouResult {
    StillAllowed,
    AllowedButDrifted { new_fingerprint: AllowedVerbSetFingerprint },
    Denied { verb_fqn: String, new_fingerprint: AllowedVerbSetFingerprint },
}
```

---

## CCIR — Context Resolution (12-Step Pure Pipeline)

**File:** `rust/crates/sem_os_core/src/context_resolution.rs`

Pure scoring/ranking logic. No DB access — all data pre-loaded.

**Request:**
```rust
pub struct ContextResolutionRequest {
    pub subject: SubjectRef,           // CaseId | EntityId | DocumentId | TaskId | ViewId
    pub intent_summary: Option<String>,
    pub raw_utterance: Option<String>,
    pub actor: ActorContext,
    pub goals: Vec<String>,
    pub constraints: ResolutionConstraints,
    pub evidence_mode: EvidenceMode,   // Strict | Normal | Exploratory | Governance
    pub point_in_time: Option<DateTime<Utc>>,
    pub entity_kind: Option<String>,
    pub entity_confidence: Option<f64>,
    pub discovery: DiscoveryContext,
}
```

**Response includes:**
- `candidate_verbs: Vec<VerbCandidate>` — ranked by context score
- `required_preconditions: Vec<PreconditionStatus>` — what must be true
- `disambiguation_questions: Vec<DisambiguationPrompt>`
- `policy_verdicts: Vec<PolicyVerdict>`
- `grounded_action_surface: Option<GroundedActionSurface>`
- `evidence: EvidenceSummary`

---

## SessionVerbSurface — Multi-Layer Governance

**File:** `rust/src/agent/verb_surface.rs`

**6-step pipeline:**

| Step | Filter | Description |
|------|--------|-------------|
| 1 | Registry | All registered verbs |
| 2 | AgentMode | Filter by active agent mode |
| 3 | Scope + Workflow (merged) | Group scope + workflow phase gates |
| 4 | SemReg CCIR | Envelope allowed-verb set |
| 5 | Lifecycle | Current entity/CBU state |
| 6 | Rank + CompositeStateBias | Score and sort |

**FailClosed default** (~30 safe-harbor verbs from domains: `agent`, `audit`, `focus`, `registry`, `schema`, `session`, `view`).

**Dual fingerprints:**
- `vs1:<hex>` — surface fingerprint (SessionVerbSurface)
- `v1:<hex>` — SemReg fingerprint (AllowedVerbSetFingerprint from CCIR)

```rust
pub enum VerbSurfaceFailPolicy {
    FailClosed,  // ~30 safe-harbor verbs (default)
    FailOpen,    // full registry tagged "ungoverned" (dev-only)
}

pub struct FilterSummary {
    pub total_registry: usize,
    pub after_agent_mode: usize,
    pub after_workflow: usize,
    pub after_group_scope: usize,
    pub after_semreg: usize,
    pub after_lifecycle: usize,
    pub after_actor: usize,
    pub final_count: usize,
}
```

**NO_GROUP_ALLOWED_DOMAINS** (always available without a loaded group):
`agent`, `audit`, `client-group`, `focus`, `gleif`, `onboarding`, `registry`, `schema`, `session`, `view`

---

## AffinityGraph & Diagram Generation

**Builder:** `rust/crates/sem_os_core/src/affinity/builder.rs`

**5-pass construction:**

| Pass | Input | Output |
|------|-------|--------|
| 1 | VerbContracts | Forward edges (produces/consumes/crud_mapping) |
| 2 | EntityTypeDefs | Entity↔table bimaps |
| 3 | AttributeDefs | Reverse edges |
| 4 | DerivationSpecs | Lineage edges |
| 5 | RelationshipTypeDefs | Entity↔entity relationships |

Output: bidirectional indexes
- `verb_to_data: HashMap<verb_fqn, Vec<edge_idx>>`
- `data_to_verb: HashMap<data_key, Vec<edge_idx>>`

**Cache:** `rust/src/domain_ops/affinity_graph_cache.rs` — keyed on latest active snapshot epoch second.

### Diagram Model & Mermaid Renderer

**Files:**
- `rust/crates/sem_os_core/src/diagram/model.rs` — pure value types (`DiagramModel`, `DiagramEntity`, `DiagramRelationship`, `GovernanceLevel`)
- `rust/crates/sem_os_core/src/diagram/mermaid.rs` — Mermaid syntax generation

**Render modes:**
1. `erDiagram` — entity blocks + relationships + cardinality
2. `verb_flow` — `graph LR` (verb → data asset flow)
3. `domain_map` — `graph TD` (domain subgraphs)

**GovernanceLevel:**
```rust
pub enum GovernanceLevel {
    Full,     // entity_type_fqn + all verbs mapped
    Partial,  // some verbs mapped
    None,     // no registry coverage
}
```

---

## Discovery Pipeline

**File:** `rust/src/domain_ops/discovery_ops.rs`

- Read-only discovery surface over existing entity search
- Lightweight operational context queries (state, polarity, lane assignment)
- Validated against SemOS discovery surface before session mutation
- `current_state` extracted from composite state and fed to lifecycle filter (was `None` — now fixed)

---

## Configuration Files

| File | Purpose | Entries |
|------|---------|---------|
| `rust/config/scenario_index.yaml` | 16 journey scenarios | Tier -2A routes |
| `rust/config/verb_schemas/macros/screening.yaml` | 4 party-level screening macros (ad-hoc, pre-workstream) | Tier -2B candidates |
| `rust/config/verb_schemas/macros/screening-ops.yaml` | 3 workstream-level screening macros (KYC case context) | Tier -2B candidates |
| `rust/config/verb_schemas/macros/kyc-workflow.yaml` | 3 KYC workflow macros | Tier -2B candidates |
| `rust/config/macro_search_overrides.yaml` | Search aliases for macros (screening, structure, party, KYC) | MacroIndex overrides |

---

## Test Fixtures & Harness

| File | Purpose |
|------|---------|
| `rust/tests/fixtures/intent_test_utterances.toml` | Intent test corpus (133 cases) |
| `rust/tests/fixtures/tier2_test_cases.toml` | Tier -2 cases (43: 17 scenario + 14 macro_match + 5 blocker + 7 sequence) |

**Intent hit rate (current):**
- First-attempt: 78.2%
- Two-attempt: 99.4%

**Scenario harness:**
```bash
cargo x harness list
cargo x harness run --all
cargo x harness run --suite scenarios/suites/governance_strict.yaml
cargo x harness dump --scenario direct_dsl_denied_viewer
```
