# CLAUDE.md

> **Last reviewed:** 2026-01-30
> **Crates:** 17 Rust crates (includes ob-poc-macros)
> **Verbs:** 537 canonical verbs (V2 schema), 10,160 intent patterns (DB-sourced)
> **Migrations:** 58 schema migrations
> **Embeddings:** Candle local (384-dim, BGE-small-en-v1.5) - 10,160 patterns vectorized
> **V2 Schema Pipeline:** ✅ Complete - Canonical YAML → registry.json → server startup → embeddings
> **Navigation:** ✅ Unified - All prompts go through IntentPipeline (view.*/session.* verbs)
> **Multi-CBU Viewport:** ✅ Complete - Scope graph endpoint, execution refresh
> **REPL Session/Phased Execution:** ✅ Complete - See `ai-thoughts/035-repl-session-implementation-plan.md`
> **Candle Semantic Pipeline:** ✅ Complete - DB source of truth, populate_embeddings binary
> **Agent Pipeline:** ✅ Unified - One path, all input → LLM intent → DSL (no special cases)
> **Solar Navigation (038):** ✅ Complete - ViewState, NavigationHistory, orbit navigation
> **Nav UX Messages:** ✅ Complete - NavReason codes, NavSuggestion, standardized error copy
> **Promotion Pipeline (043):** ✅ Complete - Quality-gated pattern promotion with collision detection
> **Teaching Mechanism (044):** ✅ Complete - Direct phrase→verb mapping for trusted sources
> **Verb Search Test Harness:** ✅ Complete - Full pipeline sweep, safety-first policy, `cargo x test-verbs`
> **Client Group Resolver (048):** ✅ Complete - Two-stage alias→group→anchor resolution for session scope
> **Workflow Task Queue (049):** ✅ Complete - Async task return path, document entity, requirement guards
> **Transactional Execution (050):** ✅ Complete - Atomic execution, advisory locks, expansion audit
> **CustomOp Auto-Registration (051):** ✅ Complete - `#[register_custom_op]` macro, inventory-based registration
> **Staged Runbook REPL (054):** ✅ Complete - Anti-hallucination staging, entity resolution, DAG ordering
> **Client Group Research Integration (055):** ✅ Complete - GLEIF import → client_group_entity staging → CBU creation with role mapping
> **REPL Viewport Feedback Loop (056):** ✅ Complete - Scope propagation in execute_runbook triggers UI refresh
> **Verb Disambiguation UI (057):** ✅ Complete - Ambiguous verb selection with gold-standard learning signals
> **Unified Architecture (058):** ✅ Complete - Operator vocabulary, macro lint, constraint cascade, DAG navigation, phonetic matching
> **Playbook System (059):** ✅ Complete - YAML playbooks, marked-yaml source mapping, LSP validation, xtask CLI
> **LSP Alignment (060):** ✅ Complete - UTF-16 encoding, multiline context, tree-sitter binding rule, rename handler
> **LSP Test Harness (063):** ✅ Complete - 150+ parser tests, golden file validation, syntax edge cases
> **Macro Vocabulary (063):** ✅ Complete - party.yaml macros, macro/implementation separation documented
> **CBU Structure Macros (064):** ✅ Complete - M1-M18 jurisdiction macros, document bundles, placeholder entities, role cardinality, wizard UI

This is the root project guide for Claude Code. Domain-specific details are in annexes.

---

## Quick Start

```bash
cd rust/

# Pre-commit (fast)
cargo x pre-commit          # Format + clippy + unit tests

# Full check
cargo x check --db          # Include database integration tests

# Deploy (UI development)
cargo x deploy              # Full: WASM + server + start
cargo x deploy --skip-wasm  # Skip WASM rebuild

# Run server directly
DATABASE_URL="postgresql:///data_designer" cargo run -p ob-poc-web
```

---

## Non-Negotiable Implementation Rules

These rules are **mandatory** for all code changes. No exceptions.

### 1. Type Safety First

**Never use untyped JSON (`serde_json::json!`) for structured data.** Always define typed structs.

```rust
// ❌ WRONG - Untyped, no compile-time guarantees
Ok(ExecutionResult::Record(serde_json::json!({
    "groups_created": row.0,
    "memberships_created": row.1,
})))

// ✅ CORRECT - Typed struct with Serialize/Deserialize
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeriveGroupsResult {
    pub groups_created: i32,
    pub memberships_created: i32,
}

let result = DeriveGroupsResult {
    groups_created: row.0,
    memberships_created: row.1,
};
Ok(ExecutionResult::Record(serde_json::to_value(result)?))
```

**Where to define types:**
- Domain result types → `ob-poc-types` crate (e.g., `manco_group.rs`, `trading_matrix.rs`)
- DSL config types → `dsl-core/src/config/types.rs`
- API response types → Near the route handler or in a shared `types.rs`

**Benefits:**
- Compile-time field name checking
- Refactoring safety (rename field → compiler finds all uses)
- IDE autocomplete and documentation
- Consistent serialization (snake_case via serde)

### 2. Consistent Return Types

YAML verb definitions use `ReturnTypeConfig` enum. Map these to typed Rust structs:

| YAML `returns.type` | Rust Pattern |
|---------------------|--------------|
| `uuid` | `ExecutionResult::Uuid(uuid)` |
| `record` | `ExecutionResult::Record(serde_json::to_value(typed_struct)?)` |
| `record_set` | `ExecutionResult::RecordSet(vec_of_typed_structs.iter().map(serde_json::to_value).collect())` |
| `affected` | `ExecutionResult::Affected(count)` |
| `void` | `ExecutionResult::Void` |

### 3. Option<T> for Nullable/Optional Values

Use `Option<T>` consistently:

```rust
// ✅ CORRECT - Explicit optionality
pub struct ControlChainNode {
    pub entity_id: Uuid,                      // Required
    pub controlled_by_entity_id: Option<Uuid>, // Optional (None for root)
    pub voting_pct: Option<Decimal>,          // Optional
}

// ❌ WRONG - Sentinel values or silent nulls
pub voting_pct: Decimal,  // What does 0.0 mean? Missing or zero?
```

### 4. Error Types over Panics

Use `Result<T, E>` and `?` operator. Never `.unwrap()` in production code paths.

```rust
// ❌ WRONG
let value = map.get("key").unwrap();

// ✅ CORRECT
let value = map.get("key").ok_or_else(|| anyhow!("Missing key"))?;
```

### 5. Re-export Types at Module Boundary

When a module uses types from another crate, re-export them for consumers:

```rust
// In domain_ops/manco_ops.rs
pub use ob_poc_types::manco_group::{
    DeriveGroupsResult, BridgeRolesResult, ControlChainNode, // ...
};
```

---

## Agent Chat `/commands` Help

The agent chat includes a built-in help system. Type `/commands` to see available operations with natural language examples.

**Chat Commands:**
| Command | Description |
|---------|-------------|
| `/commands` | Full command reference with natural language examples |
| `/verbs` | List all DSL verbs |
| `/verbs <domain>` | Verbs for specific domain (e.g., `/verbs kyc`, `/verbs session`) |
| `/help` | Quick help |

**Categories in `/commands`:**

| Section | Content |
|---------|---------|
| **DSL Operations** | `dsl_validate`, `dsl_execute`, `dsl_generate` with NL examples |
| **Session & Navigation** | Load/unload CBUs, undo/redo, scope management |
| **CBU & Entity** | Create, search, roles, products |
| **View & Zoom (ESPER)** | "enhance", "drill", "surface", "follow the money" |
| **KYC & UBO** | Case management, ownership discovery, screening |
| **Trading Profile & Custody** | Instruments, markets, SSIs |
| **Research Macros** | GLEIF lookup, UBO research, approval flow |
| **Learning & Feedback** | Pattern learning, corrections, semantic status |
| **Promotion Pipeline** | Quality-gated auto-promotion with thresholds |
| **Teaching** | Direct phrase→verb mapping for trusted sources |
| **Workflow & Templates** | Workflow instances, template expansion |
| **Batch Operations** | Multi-entity template application |

**Example Natural Language → DSL:**

```
User: "create a fund for Acme Corp"
→ dsl_generate
→ (cbu.create :name "Acme Corp" :type FUND)

User: "load the Allianz book"  
→ (session.load-galaxy :apex-name "Allianz")

User: "who owns this company?"
→ (ubo.discover :entity-id @entity)

User: "teach: 'spin up a fund' = cbu.create"
→ (agent.teach :phrase "spin up a fund" :verb "cbu.create")
```

**Key file:** `rust/src/api/agent_routes.rs` - `generate_commands_help()`

---

## Core Architecture: CBU-Centric Model

**CBU (Client Business Unit) is the atomic unit.** Everything resolves to sets of CBUs.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         SESSION = Set<CBU>                                   │
│                                                                              │
│   Universe (all CBUs)                                                        │
│     └── Book (commercial client's CBUs: Allianz, BlackRock)                 │
│          └── CBU (single trading unit)                                       │
│               ├── TRADING view (default) - instruments, counterparties      │
│               └── UBO view (KYC mode) - ownership/control taxonomy          │
│                    └── Entity drill-down (within UBO context only)          │
│                                                                              │
│   Group structure cross-links CBUs via ownership/control edges              │
│   Clusters/galaxies are DERIVED from these edges, not stored                │
└─────────────────────────────────────────────────────────────────────────────┘
```

### ViewMode (Simplified)

`ViewMode` is a unit struct - always TRADING (CBU view). The old multi-mode enum (`KYC_UBO`, `SERVICE_DELIVERY`, `PRODUCTS_ONLY`, `BOARD_CONTROL`) was removed.

```rust
// Current implementation (graph/mod.rs)
pub struct ViewMode;  // Always "TRADING"
```

For KYC/UBO work, use `view.cbu :mode ubo` which switches to ownership taxonomy within the CBU.

### GraphScope (How Sessions Resolve)

```rust
pub enum GraphScope {
    Empty,                                    // Initial state
    SingleCbu { cbu_id, cbu_name },          // Single CBU focus
    Book { apex_entity_id, apex_name },      // All CBUs under apex
    Jurisdiction { code },                    // All CBUs in jurisdiction
    EntityNeighborhood { entity_id, hops },  // N hops from entity (UBO context)
    Custom { description },                   // Custom filter
}
```

### Astro Scale Levels (Zoom/Layout)

These are **UI zoom levels using CBU and group structures**, not session scope changes:

| Level | What You See | Zoom Action |
|-------|--------------|-------------|
| Universe | All CBUs as dots | Zoom out to see everything |
| Cluster/Galaxy | Segment view (by apex/jurisdiction) | Group CBUs by ownership |
| System | Single CBU expanded | Focus on one CBU |
| Planet | Entity within CBU | Drill into CBU's entities |
| Surface/Core | Deep detail | Max zoom on entity attributes |

---

## Domain Annexes

| When working on... | Read this annex | Contains |
|--------------------|-----------------|----------|
| **Semantic pipeline** | `docs/agent-semantic-pipeline.md` | Candle embeddings, 7-tier search, latency analysis |
| **Agent/MCP pipeline** | `docs/agent-architecture.md` | Intent extraction, MCP tools, learning loop |
| **Session & navigation** | `docs/session-visualization-architecture.md` | Scopes, filters, ESPER verbs, history |
| **Data model (CBU/Entity/UBO)** | `docs/strategy-patterns.md` §1 | Why CBU is a lens, UBO discovery, holdings |
| **Verb authoring** | `docs/verb-definition-spec.md` | YAML structure, valid values, common errors |
| **egui/UI patterns** | `docs/strategy-patterns.md` §3 | Immediate mode, action enums, lock patterns |
| **Entity model & schema** | `docs/entity-model-ascii.md` | Full ERD, table relationships |
| **DSL pipeline** | `docs/dsl-verb-flow.md` | Parser, compiler, executor, plugins |
| **Research workflows** | `docs/research-agent-annex.md` | GLEIF, agent mode, invocation phrases |

### AI-Thoughts (Design Decisions)

| Topic | Document | Status |
|-------|----------|--------|
| Group/UBO ownership | `ai-thoughts/019-group-taxonomy-intra-company-ownership.md` | ✅ Done |
| Research workflows | `ai-thoughts/020-research-workflows-external-sources.md` | ✅ Done |
| Source loaders | `ai-thoughts/021-pluggable-research-source-loaders.md` | ✅ Done |
| Event infrastructure | `ai-thoughts/023a-event-infrastructure.md` | ✅ Done |
| Feedback inspector | `ai-thoughts/023b-feedback-inspector.md` | ✅ Done |
| Entity disambiguation | `ai-thoughts/025-entity-disambiguation-ux.md` | ✅ Done |
| Trading matrix pivot | `ai-thoughts/027-trading-matrix-canonical-pivot.md` | ✅ Done |
| Verb governance | `ai-thoughts/028-verb-lexicon-governance.md` | ✅ Done |
| Entity resolution wiring | `ai-thoughts/033-entity-resolution-wiring-plan.md` | ✅ Done |
| REPL state model | `ai-thoughts/034-repl-state-model-dsl-agent-protocol.md` | ⚠️ In Progress |
| Session-runsheet-viewport | `ai-thoughts/035-session-runsheet-viewport-integration.md` | ✅ Done |
| Session rip-and-replace | `ai-thoughts/036-session-rip-and-replace.md` | ✅ Done |
| Solar navigation | `ai-thoughts/038-solar-navigation-unified-design.md` | ✅ Done |

---

## DSL Pipeline (Single Path)

**ALL DSL generation goes through this pipeline. No bypass paths.**

```
User says: "spin up a fund for Acme"
                    ↓
            verb_search tool
                    ↓
    ┌───────────────┴───────────────┐
    │     Search Priority (6-tier)   │
    │  1. User learned (exact)       │
    │  2. Global learned (exact)     │
    │  3. User semantic (pgvector)   │
    │  4. Global semantic (pgvector) │
    │  5. Blocklist check            │
    │  6. Global semantic fallback   │
    └───────────────┬───────────────┘
                    ↓
            Top match: cbu.create
                    ↓
            dsl_generate tool
                    ↓
    LLM extracts args as JSON (NOT DSL)
                    ↓
    Deterministic DSL assembly
                    ↓
            (cbu.create :name "Acme")
                    ↓
            dsl_execute tool
```

### Why This Matters: LLM Removed from Semantic Loop

**Before Candle:** Verb discovery required LLM call (200-500ms, network, API cost).
**After Candle:** Verb discovery is pure Rust (5-15ms, local, free).

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  VERB DISCOVERY (100% Rust, NO LLM)                                         │
│                                                                              │
│  User: "set up an ISDA"                                                     │
│      ↓                                                                       │
│  Candle embed (local)          5-15ms   ← Pure Rust, no network             │
│      ↓                                                                       │
│  pgvector similarity           <1ms     ← In-memory index                   │
│      ↓                                                                       │
│  Result: isda.create           ────────── Total: 6-16ms                     │
│                                                                              │
├─────────────────────────────────────────────────────────────────────────────┤
│  ARG EXTRACTION (LLM still needed)                                          │
│                                                                              │
│  LLM extracts JSON args        200-500ms  ← Only LLM call in pipeline       │
│      ↓                                                                       │
│  DSL builder (Rust)            1-5ms                                        │
│      ↓                                                                       │
│  (isda.create :counterparty "Goldman" :governing-law "NY")                  │
└─────────────────────────────────────────────────────────────────────────────┘
```

| Metric | Before (OpenAI) | After (Candle) | Improvement |
|--------|-----------------|----------------|-------------|
| Embedding latency | 100-300ms | 5-15ms | **10-20x faster** |
| Network required | Yes | No | **Offline capable** |
| Cost per embed | $0.00002 | $0 | **100% savings** |
| LLM calls per query | 2 (discover + extract) | 1 (extract only) | **50% reduction** |

> **Full details:** `docs/agent-semantic-pipeline.md`

### Intent Pipeline Fixes (042)

> ✅ **IMPLEMENTED (2026-01-21)**: Full correctness fixes for verb search, entity resolution, and ambiguity detection.

**Issues Fixed:**

| Issue | Problem | Fix |
|-------|---------|-----|
| **C** | Unresolved refs lacked metadata | Parse → enrich → canonical walker with `entity_type`, `search_column`, `ref_id` |
| **K** | List/map commit broke (shared span) | Commit by `ref_id` (includes `:list_index` suffix), `dsl_hash` guard |
| **J/D** | LIMIT 1 prevented ambiguity detection | Top-k semantic search + `normalize_candidates()` + `AMBIGUITY_MARGIN = 0.05` |
| **I** | Two competing global sources | Union `agent.invocation_phrases` + `verb_pattern_embeddings`, dedupe by verb |
| **G** | Embedding computed 4x per search | Compute once at top of `search()`, pass through |
| **H** | Hardcoded 0.5 threshold | Added `fallback_threshold` (0.70), `semantic_threshold` (0.78) |

**Search Priority (Updated):**

```
1. User-specific learned (exact) - score 1.0
2. Global learned (exact) - score 1.0
3. User-specific learned (semantic, top-k=3)
4. [REMOVED] - merged into step 6
5. Blocklist filter
6. Global semantic - UNION of learned + cold-start patterns (top-k)
   → normalize_candidates(): dedupe by verb, sort desc, truncate
   → Final blocklist filter across all candidates
```

**Ambiguity Detection:**

```rust
const AMBIGUITY_MARGIN: f32 = 0.05;

pub enum VerbSearchOutcome {
    Matched(VerbSearchResult),           // Clear winner
    Ambiguous { top, runner_up, margin }, // Need clarification
    NoMatch,                              // Below threshold
}

// Pipeline returns NeedsClarification early, does NOT call LLM
```

**Entity Resolution (ref_id for Lists/Maps):**

```rust
// Each list item gets unique ref_id: "stmt_idx:start-end:list_index"
// Commit endpoint uses ref_id, not span, to target exact EntityRef

POST /api/dsl/resolve-by-ref-id
{
  "session_id": "...",
  "ref_id": "0:40-80:0",        // First item in list
  "resolved_key": "uuid-...",
  "dsl_hash": "a1b2c3d4..."     // Optimistic concurrency
}
```

**Key Files:**
- `rust/src/mcp/intent_pipeline.rs` - Pipeline with `IntentArgValue`, fail-early
- `rust/src/mcp/verb_search.rs` - `normalize_candidates()`, top-k, single embed
- `rust/crates/dsl-core/src/ast.rs` - `find_unresolved_ref_locations()` uses stored `ref_id`
- `rust/src/api/agent_routes.rs` - `resolve_by_ref_id` endpoint
- `intent-pipeline-fixes-todo.md` - Full spec with 12-point PR review rubric

### Embeddings: Candle Local (BGE Retrieval Model)

| Component | Value |
|-----------|-------|
| Framework | HuggingFace Candle (pure Rust) |
| Model | BAAI/bge-small-en-v1.5 |
| Dimensions | 384 |
| Latency | 5-15ms |
| Storage | pgvector (IVFFlat) |
| API Key | Not required |
| Mode | Asymmetric retrieval (query→target) |

**BGE vs MiniLM:**
- **BGE** is retrieval-optimized: user queries search against stored verb patterns
- Uses CLS token pooling (not mean pooling)
- Queries prefixed with instruction: `"Represent this sentence for searching relevant passages: "`
- Targets (verb patterns) stored without prefix
- Scores cluster higher (0.6-1.0) than MiniLM (0.3-1.0), thresholds recalibrated

**API Distinction:**
```rust
// User input - searching for verbs
embedder.embed_query("load the allianz book")

// Verb patterns - stored in DB
embedder.embed_target("load galaxy by apex name")
```

### Candle Semantic Pipeline - DB as Source of Truth

> ✅ **IMPLEMENTED (2026-01-19)**: Full pipeline operational with DB-sourced patterns.

**Architecture:**

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    SOURCE OF TRUTH: dsl_verbs.intent_patterns               │
│                                                                              │
│   YAML invocation_phrases                                                    │
│         │                                                                    │
│         ▼                                                                    │
│   VerbSyncService.sync_all_with_phrases() [server startup]                  │
│         │                                                                    │
│         ▼                                                                    │
│   dsl_verbs.intent_patterns (923 verbs, 7334 patterns)                      │
│         │                                                                    │
│         ▼                                                                    │
│   populate_embeddings [binary]                                               │
│   - Reads from v_verb_intent_patterns view                                  │
│   - BGE embeds each pattern (384-dim, embed_target mode)                    │
│   - Inserts to verb_pattern_embeddings with phonetic codes                  │
│         │                                                                    │
│         ▼                                                                    │
│   verb_pattern_embeddings (7500+ patterns with vectors)                     │
│         │                                                                    │
│         ▼                                                                    │
│   HybridVerbSearcher.search_global_semantic()                               │
│   - pgvector cosine similarity                                               │
│   - Returns ranked verb matches                                              │
│                                                                              │
│   Learning loop:                                                             │
│   PatternLearner → add_learned_pattern() → dsl_verbs.intent_patterns        │
│                         ↑                                                    │
│   (Re-run populate_embeddings to pick up new patterns)                      │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Key Files:**

| File | Purpose |
|------|---------|
| `rust/src/database/verb_service.rs` | `VerbService` - centralized verb DB access |
| `rust/src/mcp/verb_search.rs` | `HybridVerbSearcher` - semantic search (uses VerbService) |
| `rust/src/session/verb_sync.rs` | `VerbSyncService` - syncs YAML to DB |
| `rust/crates/ob-semantic-matcher/src/bin/populate_embeddings.rs` | Populates verb_pattern_embeddings |
| `rust/crates/ob-semantic-matcher/src/feedback/learner.rs` | Learning loop |
| `migrations/037_candle_pipeline_complete.sql` | DB schema |

**Database Tables:**

| Table | Purpose | Records |
|-------|---------|---------|
| `ob-poc.dsl_verbs` | Verb definitions + intent_patterns | 923 verbs |
| `ob-poc.verb_pattern_embeddings` | Patterns with Candle embeddings | 7500+ |
| `ob-poc.v_verb_intent_patterns` | View that flattens intent_patterns array | — |
| `agent.user_learned_phrases` | Per-user learned patterns | Runtime |
| `agent.phrase_blocklist` | Blocked verb mappings | Runtime |

**Startup Sync:**

On server startup (`ob-poc-web/src/main.rs`):
1. Load verb YAML config
2. `VerbSyncService.sync_all_with_phrases()` syncs verbs AND invocation_phrases to `dsl_verbs.yaml_intent_patterns`
3. `populate_embeddings` must be run separately to create vectors
4. Background learning task spawns (30s delay, then 6hr interval)

**Populating Embeddings:**

> ⚠️ **CRITICAL:** After adding/modifying verb YAML, you MUST run `populate_embeddings` or the new verbs won't be discoverable via semantic search. Server startup syncs YAML → DB but does NOT create embeddings.

```bash
# Run after YAML changes or first setup
DATABASE_URL="postgresql:///data_designer" \
  cargo run --release --package ob-semantic-matcher --bin populate_embeddings

# Use --force if patterns exist but need re-embedding (e.g., after model change)
DATABASE_URL="postgresql:///data_designer" \
  cargo run --release --package ob-semantic-matcher --bin populate_embeddings -- --force
```

Options:
- `--bootstrap`: Generate patterns for verbs without intent_patterns (use sparingly)
- `--force`: Re-embed all patterns even if already present (use after model change)

**Delta Loading (Fast Incremental):**

`populate_embeddings` uses delta loading by default - only processes patterns WHERE embedding IS NULL. This makes teaching and incremental updates very fast:

```
Full re-embed (7,414 patterns): ~74 seconds
Delta embed (6 new patterns):   ~0.07 seconds
```

After teaching new phrases, just run `populate_embeddings` without `--force` to pick up only the new patterns.

**Search Priority (HybridVerbSearcher):**

All DB access goes through `VerbService` (no direct sqlx calls).

1. User learned exact → `agent.user_learned_phrases`
2. Global learned exact → In-memory LearnedData
3. User semantic → pgvector on user_learned_phrases
4. Global semantic → pgvector on `verb_pattern_embeddings`
5. Blocklist check → Filter blocked verbs
6. **Global semantic fallback** ← Lower threshold, wider net

**Pattern Sources (v_verb_intent_patterns view):**
- `yaml_intent_patterns` - YAML invocation_phrases, overwritten on startup
- `intent_patterns` - Learned patterns, preserved across restarts

**Learning Loop:**

Automatic learning via background task (or MCP tools):
1. `learning_analyze` identifies high-frequency unmatched phrases
2. `learning_apply` promotes candidates to `dsl_verbs.intent_patterns`
3. `populate_embeddings` creates vectors for new patterns
4. Future queries match the learned phrases

**MCP Tools:**
- `learning_analyze` - Find patterns worth learning (days_back, min_occurrences)
- `learning_apply` - Apply a specific pattern → verb mapping
- `embeddings_status` - Check embedding coverage stats

**Coverage Stats:**

```sql
SELECT * FROM "ob-poc".v_verb_embedding_stats;
-- total_verbs: 938
-- verbs_with_patterns: 938
-- verbs_with_yaml_patterns: 935   -- From YAML invocation_phrases
-- verbs_with_learned_patterns: 15 -- From learning loop
-- total_embeddings: 7928
-- unique_verbs_embedded: 977
```

---

## Structured Onboarding Pipeline (ob-agentic)

The `ob-agentic` crate provides a **three-layer pipeline** for converting natural language custody onboarding requests into validated DSL. This is distinct from the single-verb MCP pipeline above—it handles complex multi-entity onboarding workflows.

**Crate:** `rust/crates/ob-agentic/` (no database dependency)

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    STRUCTURED ONBOARDING PIPELINE                            │
│                                                                              │
│  User: "Set up a PE fund trading US equities with IRS with Morgan Stanley"  │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  LAYER 1: INTENT EXTRACTION (LLM)                                   │    │
│  │  IntentExtractor.extract() → OnboardingIntent                       │    │
│  │                                                                     │    │
│  │  Output: {                                                          │    │
│  │    client: { name: "PE Fund", type: "fund", jurisdiction: "US" },  │    │
│  │    instruments: [EQUITY, OTC_IRS],                                 │    │
│  │    markets: [{ code: XNYS, currencies: [USD] }],                   │    │
│  │    otc_counterparties: [{ name: "Morgan Stanley", law: "NY" }]     │    │
│  │  }                                                                  │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                              │                                               │
│                              ▼                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  LAYER 2: REQUIREMENT PLANNING (Deterministic Rust)                 │    │
│  │  RequirementPlanner::plan(intent) → OnboardingPlan                  │    │
│  │                                                                     │    │
│  │  • Classify pattern (SimpleEquity / MultiMarket / WithOtc)         │    │
│  │  • Plan CBU creation                                                │    │
│  │  • Plan entity lookups (counterparties)                            │    │
│  │  • Derive universe (instruments × markets × currencies)            │    │
│  │  • Derive SSIs (settlement routes)                                 │    │
│  │  • Derive booking rules (priority-ordered)                         │    │
│  │  • Plan ISDAs + CSAs (if OTC)                                      │    │
│  │                                                                     │    │
│  │  NO LLM - pure business logic expansion                            │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                              │                                               │
│                              ▼                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  LAYER 3: DSL GENERATION (LLM)                                      │    │
│  │  DslGenerator.generate(plan) → DSL source string                    │    │
│  │                                                                     │    │
│  │  • System prompt: DSL syntax, verb schemas, pattern example        │    │
│  │  • User prompt: structured plan (CBU, entities, universe, rules)   │    │
│  │  • LLM renders plan as s-expression DSL                            │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                              │                                               │
│                              ▼                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  VALIDATION & RETRY (FeedbackLoop)                                  │    │
│  │  FeedbackLoop.generate_valid_dsl(plan)                              │    │
│  │                                                                     │    │
│  │  • AgentValidator checks syntax                                    │    │
│  │  • If invalid: collect errors → ask LLM to fix                     │    │
│  │  • Retry up to max_retries                                         │    │
│  │  • Return ValidatedDsl { source, attempts, validation }            │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                              │                                               │
│                              ▼                                               │
│                    dsl-core parser → execution                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Key Types

| Type | Module | Purpose |
|------|--------|---------|
| `OnboardingIntent` | `ob_agentic::intent` | Structured NL extraction (client, instruments, markets, counterparties) |
| `IntentResult` | `ob_agentic::intent` | Either `Clear(intent)` or `NeedsClarification(request)` |
| `OnboardingPlan` | `ob_agentic::planner` | Complete requirements (CBU, entities, universe, SSIs, rules, ISDAs) |
| `OnboardingPattern` | `ob_agentic::patterns` | Classification: `SimpleEquity`, `MultiMarket`, `WithOtc` |
| `DslGenerator` | `ob_agentic::generator` | LLM-based DSL rendering from plan |
| `FeedbackLoop` | `ob_agentic::feedback` | Retry loop with error correction |
| `ValidatedDsl` | `ob_agentic::feedback` | Final output with attempt count |

### Onboarding Patterns

| Pattern | Criteria | Complexity |
|---------|----------|------------|
| `SimpleEquity` | Single market, single currency, no OTC | CBU + profile + 1 SSI + basic rules |
| `MultiMarket` | Multiple markets or cross-currency | Multiple SSIs + complex routing rules |
| `WithOtc` | Has OTC counterparties | Adds entities + ISDAs + CSAs + collateral SSI |

### OnboardingPlan Structure

```rust
pub struct OnboardingPlan {
    pub pattern: OnboardingPattern,       // Classification
    pub cbu: CbuPlan,                     // CBU creation spec
    pub entities: Vec<EntityPlan>,        // Counterparty lookups
    pub universe: Vec<UniverseEntry>,     // What client trades
    pub ssis: Vec<SsiPlan>,               // Settlement identifiers  
    pub booking_rules: Vec<BookingRulePlan>, // Routing (priority-ordered)
    pub isdas: Vec<IsdaPlan>,             // OTC agreements
}

// Universe entry: instruments × markets × currencies
pub struct UniverseEntry {
    pub instrument_class: String,         // EQUITY, OTC_IRS
    pub market: Option<String>,           // XNYS, None for OTC
    pub currencies: Vec<String>,          // USD, EUR
    pub settlement_types: Vec<String>,    // DVP, FOP
    pub counterparty_var: Option<String>, // @morgan for OTC
}

// Booking rules derived from universe
pub struct BookingRulePlan {
    pub priority: u32,                    // 10, 15, 50, 100
    pub instrument_class: Option<String>,
    pub market: Option<String>,
    pub currency: Option<String>,
    pub counterparty_var: Option<String>,
    pub ssi_variable: String,             // @ssi-xnys-usd
}
```

### Call Stack

```rust
// 1. Extract intent from NL (async, LLM)
let extractor = IntentExtractor::from_env()?;
let result = extractor.extract("Set up PE fund...").await?;

match result {
    IntentResult::NeedsClarification(req) => {
        // Show req.ambiguity.question to user
        // Call extract_with_clarification() with their choice
    }
    IntentResult::Clear(intent) => {
        // 2. Plan requirements (sync, deterministic)
        let plan = RequirementPlanner::plan(&intent);
        
        // 3. Generate validated DSL (async, LLM with retry)
        let feedback = FeedbackLoop::from_env(3)?;
        let validated = feedback.generate_valid_dsl(&plan).await?;
        
        // validated.source contains DSL ready for dsl-core
    }
}
```

### Lexicon Pipeline (Alternative Path)

The `ob_agentic::lexicon` module provides a **formal grammar** approach as an alternative to LLM-based intent extraction:

```
User Input → Tokenizer (lexicon + EntityGateway) → Nom Parser → IntentAst → Plan → DSL
```

| Module | Purpose |
|--------|---------|
| `lexicon::Tokenizer` | Classifies words against YAML lexicon + DB entities |
| `lexicon::parse_tokens` | Nom grammar parser → `IntentAst` |
| `lexicon::intent_to_plan` | AST → `Plan` with `SemanticAction` |
| `lexicon::render_plan` | Plan → DSL source string |

This path is deterministic end-to-end (no LLM) but requires the lexicon to cover the input vocabulary.

### Key Files

| File | Purpose |
|------|---------|
| `rust/crates/ob-agentic/src/intent.rs` | `OnboardingIntent`, `IntentResult`, `ClarificationRequest` |
| `rust/crates/ob-agentic/src/planner.rs` | `RequirementPlanner::plan()`, `OnboardingPlan` |
| `rust/crates/ob-agentic/src/generator.rs` | `DslGenerator`, `IntentExtractor` |
| `rust/crates/ob-agentic/src/feedback.rs` | `FeedbackLoop`, `ValidatedDsl` |
| `rust/crates/ob-agentic/src/patterns.rs` | `OnboardingPattern` enum |
| `rust/crates/ob-agentic/src/validator.rs` | `AgentValidator` for DSL syntax |
| `rust/crates/ob-agentic/src/lexicon/` | Formal grammar tokenizer + parser |
| `rust/crates/ob-agentic/src/schemas/` | Verb schemas + reference data for prompts |

### When to Use Which Pipeline

| Pipeline | Use Case |
|----------|----------|
| **MCP Pipeline** (verb_search → dsl_generate) | Single-verb commands, navigation, CRUD operations |
| **ob-agentic Pipeline** (Intent → Plan → DSL) | Complex custody onboarding with multiple entities, SSIs, booking rules |
| **Lexicon Pipeline** | Deterministic parsing when vocabulary is constrained |

---

## V2 Verb Schema Pipeline (057)

> ✅ **IMPLEMENTED (2026-01-27)**: Canonical V2 YAML schema with deterministic phrase generation and compiled registry.

The V2 schema pipeline provides a canonical verb definition format with:
- Inline args (HashMap-style) instead of nested arrays
- Deterministic invocation phrase generation (no LLM)
- Compiled registry.json artifact for fast server startup
- Alias collision detection for disambiguation

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    V2 SCHEMA PIPELINE                                        │
│                                                                              │
│  V1 YAML (config/verbs/*.yaml)                                              │
│         │                                                                    │
│         ▼                                                                    │
│  cargo x verbs migrate-v2                                                   │
│         │                                                                    │
│         ├─► V2 YAML (config/verb_schemas/generated/*.yaml)                  │
│         │   - Canonical format with inline args                             │
│         │   - Generated invocation_phrases (deterministic)                  │
│         │                                                                    │
│         └─► Lint pass (0 errors, 0 warnings required)                       │
│                                                                              │
│  cargo x verbs build-registry                                               │
│         │                                                                    │
│         ▼                                                                    │
│  registry.json (config/verb_schemas/registry.json)                          │
│         │   - 534 canonical verbs                                            │
│         │   - 480 aliases (short forms)                                      │
│         │   - 76 alias collisions (for disambiguation)                       │
│         │                                                                    │
│         ▼                                                                    │
│  Server startup (ob-poc-web)                                                │
│         │   - Auto-detects registry.json                                    │
│         │   - Loads V2Registry with invocation phrases                      │
│         │   - Syncs to dsl_verbs.yaml_intent_patterns                       │
│         │                                                                    │
│         ▼                                                                    │
│  populate_embeddings                                                        │
│         │   - Delta loading (only new patterns)                             │
│         │   - 10,160 total patterns vectorized                              │
│         │                                                                    │
│         ▼                                                                    │
│  HybridVerbSearcher (semantic search ready)                                 │
└─────────────────────────────────────────────────────────────────────────────┘
```

### V2 Schema Format

```yaml
# config/verb_schemas/generated/cbu.yaml
cbu.create:
  verb: cbu.create
  domain: cbu
  action: create
  description: "Create a new Client Business Unit"
  behavior: plugin
  
  # Inline args (HashMap style, not nested arrays)
  args:
    name: { type: str, required: true, description: "CBU name" }
    type: { type: str, required: false, valid_values: [FUND, MANDATE, SEGREGATED] }
    jurisdiction: { type: str, required: false }
  
  # Positional sugar for common patterns
  positional_sugar: [name]
  
  # Auto-generated invocation phrases
  invocation_phrases:
    - "create cbu"
    - "add cbu"
    - "new client business unit"
    - "register trading unit"
```

### Commands

```bash
cd rust/

# Migrate V1 → V2 (dry-run first)
cargo x verbs migrate-v2 --dry-run
cargo x verbs migrate-v2

# Lint V2 schemas (must pass with 0 errors/warnings)
cargo x verbs lint-v2

# Build compiled registry
cargo x verbs build-registry

# After server startup, populate embeddings
DATABASE_URL="postgresql:///data_designer" \
  cargo run --release --package ob-semantic-matcher --bin populate_embeddings
```

### Key Files

| File | Purpose |
|------|---------|
| `rust/xtask/src/verb_migrate.rs` | Migration logic, phrase generation |
| `rust/src/dsl_v2/v2_registry.rs` | V2Registry loader, type conversion |
| `rust/src/session/verb_sync.rs` | `sync_v2_invocation_phrases()` |
| `rust/crates/ob-poc-web/src/main.rs` | V2 auto-detection on startup |
| `config/verb_schemas/generated/` | V2 YAML output directory |
| `config/verb_schemas/registry.json` | Compiled registry artifact |

### Phrase Generation (Deterministic)

Phrases are generated using synonym dictionaries - no LLM required:

```rust
// Verb synonyms (CRUD operations)
"create" → ["add", "new", "make", "register"]
"list"   → ["show", "get all", "display", "enumerate"]
"update" → ["edit", "modify", "change"]
"delete" → ["remove", "drop"]

// Domain nouns
"cbu"    → ["cbu", "client business unit", "trading unit"]
"entity" → ["entity", "company", "person"]
"fund"   → ["fund", "investment fund", "portfolio"]
```

Each verb gets 3+ phrases combining verb synonyms with domain nouns.

### Alias Collisions

Short aliases like `create`, `list`, `delete` map to multiple verbs:

```
'create' → ["cbu.create", "entity.create", "user.create", ...]
'delete' → ["entity.delete", "cbu.delete", "identifier.remove", ...]
```

These are intentional - the semantic search uses full phrases to disambiguate, and the UI can show disambiguation options when the user types a bare alias.

---

## Promotion Pipeline (Staged Pattern Learning)

The **PromotionService** provides quality-gated pattern promotion with collision detection. Runs as part of the background learning task (every 6 hours).

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    PROMOTION PIPELINE                                        │
│                                                                              │
│  User interaction → outcome recorded                                        │
│      │                                                                       │
│      ▼                                                                       │
│  FeedbackService.record_outcome_with_dsl()                                  │
│      │                                                                       │
│      └── Triggers agent.record_learning_signal() for strong signals         │
│              │                                                               │
│              ▼                                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  QUALITY GATES (SQL function)                                        │    │
│  │  • Word count: 3-15 words                                            │    │
│  │  • Stopword ratio: <70%                                              │    │
│  │  • Not already in verb_pattern_embeddings                           │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│              │                                                               │
│              ▼                                                               │
│  agent.learning_candidates (tracks success_count, total_count)              │
│              │                                                               │
│              │ (background job every 6 hours)                               │
│              ▼                                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  PROMOTION THRESHOLDS                                                │    │
│  │  • occurrence_count >= 5                                             │    │
│  │  • success_rate >= 0.80                                              │    │
│  │  • age >= 24 hours (cool-down)                                       │    │
│  │  • collision_safe = true (semantic check)                            │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│              │                                                               │
│              ▼                                                               │
│  agent.apply_promotion() → dsl_verbs.intent_patterns                        │
│              │                                                               │
│              ▼                                                               │
│  (Run populate_embeddings to enable semantic matching)                      │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Key Files:**

| File | Purpose |
|------|---------|
| `rust/crates/ob-semantic-matcher/src/feedback/promotion.rs` | PromotionService |
| `rust/crates/ob-semantic-matcher/src/feedback/service.rs` | FeedbackService (triggers learning signals) |
| `rust/src/agent/learning/background.rs` | Background job integration |
| `migrations/043_feedback_loop_promotion.sql` | Schema, functions, views |

**Database Tables:**

| Table | Purpose |
|-------|---------|
| `agent.learning_candidates` | Staged candidates with success tracking |
| `agent.stopwords` | Words filtered from patterns |
| `agent.phrase_blocklist` | Rejected patterns |
| `agent.learning_audit` | Audit trail of promotions/rejections |

**Views:**

| View | Purpose |
|------|---------|
| `agent.v_top_pending_candidates` | Top 100 pending by occurrence |
| `agent.v_candidate_pipeline` | Pipeline status summary |
| `agent.v_learning_health_weekly` | Weekly health metrics |

**MCP Tools:**

| Tool | Description |
|------|-------------|
| `promotion_run_cycle` | Run full promotion pipeline manually |
| `promotion_candidates` | List candidates ready for auto-promotion |
| `promotion_review_queue` | List candidates needing manual review |
| `promotion_approve` | Manually approve a candidate |
| `promotion_reject` | Reject and add to blocklist |
| `promotion_health` | Weekly health metrics |
| `promotion_pipeline_status` | Pipeline summary by status |

**Thresholds:**

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| `min_occurrences` | 5 | Enough signal, not one-off |
| `min_success_rate` | 0.80 | 4/5 successful uses |
| `min_age_hours` | 24 | Cool-down for burst patterns |
| `collision_threshold` | 0.92 | Prevent verb confusion |

---

## Teaching Mechanism (Direct Pattern Learning)

The **Teaching Tools** allow trusted sources (admin, Claude) to directly add phrase→verb mappings without going through the candidate promotion pipeline. This bypasses quality gates for immediate effect.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    TEACHING vs PROMOTION                                     │
│                                                                              │
│  TEACHING (trusted, immediate)           PROMOTION (earned, gated)          │
│  ─────────────────────────────           ──────────────────────────         │
│  teach_phrase("spin up", "cbu.create")   User interactions → signals        │
│      │                                       │                               │
│      ▼                                       ▼                               │
│  agent.teach_phrase() SQL function       learning_candidates (staged)       │
│      │                                       │                               │
│      ├── Validates verb exists               ├── Quality gates              │
│      ├── Normalizes phrase                   ├── Success rate >= 80%        │
│      ├── Adds to intent_patterns             ├── Age >= 24 hours            │
│      └── Audits to teaching_audit            └── Collision check            │
│      │                                       │                               │
│      ▼                                       ▼                               │
│  dsl_verbs.intent_patterns              dsl_verbs.intent_patterns           │
│      │                                       │                               │
│      └──────────────┬────────────────────────┘                              │
│                     ▼                                                        │
│           populate_embeddings                                                │
│                     │                                                        │
│                     ▼                                                        │
│           verb_pattern_embeddings (semantic search enabled)                  │
└─────────────────────────────────────────────────────────────────────────────┘
```

**MCP Tools:**

| Tool | Description |
|------|-------------|
| `teach_phrase` | Directly add phrase→verb mapping |
| `unteach_phrase` | Remove a taught mapping (with audit) |
| `teaching_status` | View recently taught patterns and stats |

**Usage Examples:**

```json
// Teach a new phrase
{"tool": "teach_phrase", "phrase": "spin up a fund", "verb": "cbu.create"}

// Teach with custom source
{"tool": "teach_phrase", "phrase": "who owns this", "verb": "ubo.discover", "source": "admin_manual"}

// Remove a taught phrase
{"tool": "unteach_phrase", "phrase": "spin up a fund", "reason": "too_generic"}

// Check teaching status
{"tool": "teaching_status", "limit": 20, "include_stats": true}
```

**Key Files:**

| File | Purpose |
|------|---------|
| `migrations/044_agent_teaching.sql` | Schema, functions, views |
| `rust/src/mcp/handlers/core.rs` | MCP tool handlers |
| `rust/src/mcp/tools.rs` | Tool definitions |

**Database Functions:**

| Function | Purpose |
|----------|---------|
| `agent.teach_phrase(phrase, verb, source)` | Add pattern with validation |
| `agent.teach_phrases_batch(json_array)` | Bulk teaching |
| `agent.unteach_phrase(phrase, verb, reason)` | Remove with audit |
| `agent.get_taught_pending_embeddings()` | Patterns needing embedding |

**Views:**

| View | Purpose |
|------|---------|
| `agent.v_recently_taught` | Recent patterns with source |
| `agent.v_teaching_stats` | Totals, today, this week |

**After Teaching:**

Always run `populate_embeddings` after teaching to enable semantic matching:

```bash
DATABASE_URL="postgresql:///data_designer" \
  cargo run --release --package ob-semantic-matcher --bin populate_embeddings
```

---

## Verb Disambiguation UI (057)

> ✅ **IMPLEMENTED (2026-01-27)**: Interactive disambiguation when verb search returns multiple high-confidence matches.

When the semantic verb search returns multiple verbs with similar scores (within `AMBIGUITY_MARGIN = 0.05`), the system presents an interactive disambiguation UI instead of guessing.

**Flow:**

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  User: "load the book"                                                      │
│      │                                                                       │
│      ▼                                                                       │
│  HybridVerbSearcher.search() → VerbSearchOutcome::Suggest                   │
│      │   (top: session.load-galaxy @ 0.82, runner_up: session.load-cbu @ 0.79)
│      │                                                                       │
│      ▼                                                                       │
│  ChatResponse.verb_disambiguation = Some(VerbDisambiguationRequest {        │
│      original_input: "load the book",                                       │
│      options: [session.load-galaxy, session.load-cbu, ...]                  │
│  })                                                                          │
│      │                                                                       │
│      ▼                                                                       │
│  UI shows disambiguation card with clickable verb buttons                   │
│      │                                                                       │
│      ├─► User clicks verb → POST /select-verb                               │
│      │       → Gold-standard learning signal (confidence=0.95)              │
│      │       → Phrase variants generated for robust learning                │
│      │       → Continue with selected verb                                  │
│      │                                                                       │
│      └─► User cancels/times out (30s) → POST /abandon-disambiguation        │
│              → Negative signals for all candidates                          │
│              → Clear disambiguation state                                   │
└─────────────────────────────────────────────────────────────────────────────┘
```

**API Endpoints:**

| Endpoint | Purpose |
|----------|---------|
| `POST /api/session/:id/select-verb` | User selected a verb from disambiguation |
| `POST /api/session/:id/abandon-disambiguation` | User cancelled or timed out |

**Types (`ob-poc-types`):**

```rust
pub struct VerbDisambiguationRequest {
    pub original_input: String,
    pub options: Vec<VerbOption>,
}

pub struct VerbOption {
    pub verb_fqn: String,      // e.g., "session.load-galaxy"
    pub description: String,
    pub score: f32,
    pub example: Option<String>,
}

pub struct VerbSelectionRequest {
    pub selected_verb: String,
    pub original_input: String,
    pub all_candidates: Vec<VerbCandidate>,
}

pub enum AbandonReason {
    Cancelled,      // User clicked cancel
    Timeout,        // 30-second timeout
    NewInput,       // User started typing new input
}
```

**UI State (`VerbDisambiguationState`):**

```rust
pub struct VerbDisambiguationState {
    pub active: bool,
    pub request: Option<VerbDisambiguationRequest>,
    pub original_input: String,
    pub shown_at: Option<f64>,  // Timestamp for timeout
    pub loading: bool,
}
```

**Key Files:**

| File | Purpose |
|------|---------|
| `rust/crates/ob-poc-types/src/lib.rs` | Type definitions |
| `rust/src/api/agent_routes.rs` | `/select-verb`, `/abandon-disambiguation` endpoints |
| `rust/src/api/agent_service.rs` | `handle_verb_selection()`, wires to ChatResponse |
| `rust/crates/ob-poc-ui/src/state.rs` | `VerbDisambiguationState` |
| `rust/crates/ob-poc-ui/src/panels/repl.rs` | `render_verb_disambiguation_card()` |
| `rust/crates/ob-poc-ui/src/app.rs` | Action handling, timeout check |

**Learning Signal Generation:**

When user selects a verb:
1. **Gold-standard signal** (confidence=0.95) for selected phrase→verb
2. **Phrase variants** generated via `generate_phrase_variants()`:
   - Original phrase
   - Lowercase normalized
   - Common prefix variations ("please", "can you", "I want to")
3. **Negative signals** for rejected candidates

---

## Operator Macro Vocabulary (058)

> ✅ **IMPLEMENTED (2026-01-28)**: Business-friendly vocabulary layer over technical DSL.

The Operator Macro system provides business-friendly terms (structure, case, mandate) that map to technical DSL verbs (cbu, kyc-case, trading-profile). This enables the UI verb picker to show operator-friendly labels.

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  OPERATOR VOCABULARY LAYER                                                   │
│                                                                              │
│  User types: "set up structure"                                             │
│      │                                                                       │
│      ▼                                                                       │
│  HybridVerbSearcher.search() - Tier 0: Macro Search (HIGHEST PRIORITY)      │
│      │                                                                       │
│      ├─► Exact FQN match (structure.setup) → score 1.0                      │
│      ├─► Exact label match ("Set up Structure") → score 1.0                 │
│      └─► Fuzzy label/desc match → score 0.95                                │
│      │                                                                       │
│      ▼                                                                       │
│  Macro found → Expands to DSL: (session.set-structure :structure-id ...)    │
│                                                                              │
│  Display Noun Translation:                                                   │
│  - cbu → structure                                                          │
│  - kyc-case → case                                                          │
│  - trading-profile → mandate                                                │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Components

| Component | File | Purpose |
|-----------|------|---------|
| Macro Definitions | `rust/src/macros/definition.rs` | Type definitions for operator macros |
| Macro Registry | `rust/src/macros/registry.rs` | Load/index macros from YAML, build taxonomy |
| Display Nouns | `rust/src/api/display_nouns.rs` | Translate internal→operator vocabulary |
| Verb Search Integration | `rust/src/mcp/verb_search.rs` | Tier 0 macro search in `HybridVerbSearcher` |

### Session Context Verbs

New primitive verbs for macro expansion targets:

| Verb | Purpose | Handler |
|------|---------|---------|
| `session.set-structure` | Set current CBU context | `SessionSetStructureOp` |
| `session.set-case` | Set current KYC case context | `SessionSetCaseOp` |
| `session.set-mandate` | Set current trading profile context | `SessionSetMandateOp` |

### API Endpoints

| Endpoint | Purpose |
|----------|---------|
| `GET /api/verbs/taxonomy` | Hierarchical macro taxonomy for UI verb picker |
| `GET /api/verbs/:fqn/schema` | Full macro definition (args, prereqs, expansion) |

### Verb Search Priority (Updated)

```
0. Operator macros (business vocabulary) - score 1.0 exact, 0.95 fuzzy  ← NEW
1. User-specific learned (exact) - score 1.0
2. Global learned (exact) - score 1.0
3. User-specific learned (semantic) - score 0.8-0.99
4. [REMOVED]
5. Blocklist filter
6. Global semantic (cold start) - score 0.55-0.95
7. Phonetic fallback (typo handling)
```

### Display Noun Mapping

| Internal Term | Operator Term |
|---------------|---------------|
| `cbu` | `structure` |
| `cbu_id` | `structure_id` |
| `kyc-case` | `case` |
| `kyc_case_id` | `case_id` |
| `trading-profile` | `mandate` |
| `trading_profile_id` | `mandate_id` |

### Macro Lint Rules

Key lint rules enforced by `cargo x verbs lint-macros`:

| Rule | Severity | Description |
|------|----------|-------------|
| `MACRO011` | Error | UI fields required (label, description, target_label) |
| `MACRO012` | Error | Forbidden tokens in UI (cbu, entity_ref, etc.) |
| `MACRO042` | Error | No raw `entity_ref` - use `structure_ref`, `party_ref` |
| `MACRO043` | Error | `kinds:` only under `internal:` block |
| `MACRO044` | Error | Enums must use `${arg.X.internal}` in expansion |
| `MACRO063` | Error | Variable grammar validation (only `${arg.*}`, `${scope.*}`, `${session.*}`) |
| `MACRO080a-c` | Warning | UX friction (missing autofill, picker, short description) |

### Operator Enum Keys vs Internal Tokens

UI keys are not the same as internal tokens - expansion MUST use `${arg.X.internal}`:

| UI Key | Label (user sees) | Internal Token |
|--------|-------------------|----------------|
| `pe` | "Private Equity" | `private-equity` |
| `sicav` | "SICAV" | `sicav` |
| `gp` | "General Partner" | `general-partner` |
| `manco` | "Management Company" | `manco` |

---

## Legal Contracts & Onboarding Gate (045)

> ✅ **IMPLEMENTED (2026-01-22)**: Legal contracts with product-level rate cards and CBU subscription gate.

**Data Model:**

```
┌─────────────────────────────────────────────────────────────────────┐
│                     legal_contracts                                  │
│  contract_id, client_label, effective_date, status                  │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              │ 1:N
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                contract_products                                     │
│  (contract_id, product_code) = PK                                    │
│  rate_card_id                                                        │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              │ 1:N (FK enforced onboarding gate)
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                cbu_subscriptions                                     │
│  (cbu_id, contract_id, product_code) = PK                           │
│  CBU can only subscribe if contract covers that product             │
└─────────────────────────────────────────────────────────────────────┘
```

**Key Concept:** CBU onboarding requires contract+product subscription. No contract = no onboarding.

**client_label:**
- Added to `entities` and `cbus` tables as searchable client shorthand
- "allianz" → all Allianz entities/CBUs, "blackrock" → all BlackRock entities/CBUs
- Used by `session.load-cluster :client "allianz"` for client-scoped loading

**Contract Verbs:**

| Verb | Purpose |
|------|---------|
| `contract.create` | Create legal contract for client |
| `contract.add-product` | Add product with rate card |
| `contract.subscribe` | Subscribe CBU to contract+product (onboarding gate) |
| `contract.list-subscriptions` | List CBU subscriptions |
| `contract.for-client` | Get active contract by client label |

**DSL Examples:**

```clojure
;; Create contract with products
(contract.create :client "allianz" :reference "MSA-2024-001" :effective-date "2024-01-01" :as @contract)
(contract.add-product :contract-id @contract :product "CUSTODY" :rate-card-id @rate)

;; Subscribe CBU (onboarding)
(contract.subscribe :cbu-id @cbu :contract-id @contract :product "CUSTODY")

;; Load client's CBUs
(session.load-cluster :client "allianz" :jurisdiction "LU")
```

**Key Files:**

| File | Purpose |
|------|---------|
| `migrations/045_legal_contracts.sql` | Schema, views, seed data |
| `rust/config/verbs/contract.yaml` | 14 contract verbs |
| `rust/src/domain_ops/session_ops.rs` | `load-cluster` uses client_label |

---

## Client Group Resolver (048)

> ✅ **IMPLEMENTED (2026-01-23)**: Two-stage alias→group→anchor resolution for session scope selection.

The Client Group Resolver enables natural language scope selection (e.g., "allianz" → load all Allianz CBUs) through a two-stage resolution process:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    TWO-STAGE RESOLUTION                                      │
│                                                                              │
│  User: "allianz"                                                            │
│      │                                                                       │
│      ▼                                                                       │
│  Stage 1: Alias → ClientGroupId                                             │
│      │   "allianz" → exact/semantic match → Allianz Global Investors        │
│      │   (group_id: 11111111-1111-1111-1111-111111111111)                   │
│      │                                                                       │
│      ▼                                                                       │
│  Stage 2: ClientGroupId → AnchorEntityId                                    │
│      │   group_id + role (governance_controller) + jurisdiction (optional) │
│      │   → Allianz Global Investors Holdings GmbH (entity_id)              │
│      │                                                                       │
│      ▼                                                                       │
│  session.load-cluster uses anchor_entity_id to find all CBUs               │
│  under the client's ownership hierarchy                                     │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Data Model:**

| Table | Purpose |
|-------|---------|
| `client_group` | Virtual client groups (Allianz, Aviva, BlackRock) |
| `client_group_alias` | Searchable aliases with embeddings |
| `client_group_alias_embedding` | Versioned embeddings per alias |
| `client_group_anchor` | Role-based anchors per jurisdiction |

**Anchor Roles:**

| Role | Use Case | Default For |
|------|----------|-------------|
| `ultimate_parent` | UBO discovery, ownership tracing | `ubo.*` verbs |
| `governance_controller` | Session scope, CBU loading | `session.*`, `cbu.*` verbs |
| `book_controller` | Regional operations | - |
| `operating_controller` | Day-to-day operations | `contract.*` verbs |
| `regulatory_anchor` | Compliance, KYC | `kyc.*` verbs |

**DSL Usage:**

```clojure
;; Load by client name (two-stage resolution)
(session.load-cluster :client <Allianz>)

;; With jurisdiction filter
(session.load-cluster :client <Allianz> :jurisdiction "LU")

;; Direct entity (bypasses client group resolution)
(session.load-cluster :apex-entity-id "uuid-...")
```

**Key Files:**

| File | Purpose |
|------|---------|
| `migrations/048_client_group_seed.sql` | Schema + bootstrap data |
| `rust/crates/ob-semantic-matcher/src/client_group_resolver.rs` | Resolution logic |
| `rust/src/domain_ops/session_ops.rs` | `SessionLoadClusterOp` handler |
| `rust/config/verbs/session.yaml` | `:client` arg with lookup config |
| `rust/tests/client_group_integration.rs` | Full integration tests |

**Bootstrap Data:**

| Group | Canonical Name | Anchors |
|-------|----------------|---------|
| Allianz | Allianz Global Investors | Allianz SE (UP), AGI Holdings GmbH (GC) |
| Aviva | Aviva Investors | Aviva plc (UP), Aviva Investors Global (GC) |
| BlackRock | BlackRock | BlackRock Inc (UP), BLK Fund Advisors (GC) |
| Aberdeen | Aberdeen Standard Investments | abrdn plc (UP, GC) |

**Populate Embeddings:**

After adding client groups or aliases:

```bash
DATABASE_URL="postgresql:///data_designer" \
  cargo run --release --package ob-semantic-matcher --bin populate_embeddings -- --client-groups
```

---

## Workflow Task Queue & Document Entity (049)

> ✅ **IMPLEMENTED (2026-01-24)**: Queue-based async task return path with document as first-class entity.

**Problem Solved:** Workflows can emit tasks (e.g., "solicit passport") but had no mechanism to receive results and resume. The task queue provides the return path.

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  OUTBOUND: Workflow emits task                                              │
│  Blocker detected → resolution DSL → workflow_pending_tasks                 │
│  → External system (Camunda, portal, email) receives task_id + callback URL │
└─────────────────────────────────────────────────────────────────────────────┘
                              │
                    External system works...
                              │
┌─────────────────────────────────────────────────────────────────────────────┐
│  INBOUND: Task completion webhook                                           │
│  POST /api/workflow/task-complete → task_result_queue                       │
│  TaskQueueListener → try_advance(workflow_instance)                         │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Three-Layer Document Model

| Layer | Table | Purpose |
|-------|-------|---------|
| A: Requirement | `document_requirements` | What we NEED from entity (exists before upload) |
| B: Document | `documents` | Logical identity (stable reference) |
| C: Version | `document_versions` | Each submission (immutable, verification status) |

### Requirement State Machine

```
missing → requested → received → in_qa → verified
                                    ↓
                               rejected → (retry with new version)
                                    
waived (manual override)    expired (validity lapsed)
```

### Rejection Reason Codes

Standardized codes for document QA rejection with client messaging:

| Category | Codes | Next Action |
|----------|-------|-------------|
| Quality | `UNREADABLE`, `CUTOFF`, `GLARE`, `LOW_RESOLUTION` | Re-upload |
| Mismatch | `WRONG_DOC_TYPE`, `WRONG_PERSON`, `SAMPLE_DOC` | Upload correct doc |
| Validity | `EXPIRED`, `NOT_YET_VALID`, `UNDATED` | Upload valid doc |
| Data | `DOB_MISMATCH`, `NAME_MISMATCH`, `ADDRESS_MISMATCH` | Verify details |
| Authenticity | `SUSPECTED_ALTERATION`, `INCONSISTENT_FONTS` | Escalate |

### CargoRef URI Scheme

Typed reference URIs for document/entity tracking:

```
document://ob-poc/{uuid}     # Document identity
version://ob-poc/{uuid}      # Specific version (preferred for callbacks)
entity://ob-poc/{uuid}       # Entity reference
screening://ob-poc/{uuid}    # Screening result
external://{system}/{id}     # Vendor passthrough
```

### Bundle Payload Format

External systems return bundles (even for single documents):

```json
{
  "task_id": "uuid",
  "status": "completed",
  "idempotency_key": "vendor-event-12345",
  "items": [
    { "cargo_ref": "version://ob-poc/...", "doc_type": "passport", "status": "completed" }
  ]
}
```

Listener uses atomic CTE-based queue pop (`FOR UPDATE SKIP LOCKED`) with deduplication by `(task_id, idempotency_key)`.

### Key Tables

| Table | Purpose |
|-------|---------|
| `rejection_reason_codes` | Reference data for QA rejection reasons |
| `document_requirements` | What documents are needed per entity/workflow |
| `documents` | Logical document identity |
| `document_versions` | Immutable submissions with verification status |
| `workflow_pending_tasks` | Outbound task tracking |
| `task_result_queue` | Inbound results (ephemeral, deleted after processing) |
| `task_result_dlq` | Dead letter queue for failed processing |
| `workflow_task_events` | Permanent audit trail |

### API Endpoints

| Endpoint | Purpose |
|----------|---------|
| `POST /api/workflow/task-complete` | External callback webhook (bundle payload) |
| `POST /api/documents` | Create document |
| `POST /api/documents/:id/versions` | Upload version, returns `cargo_ref` |
| `GET /api/documents/:id` | Get document with status |
| `POST /api/documents/:doc_id/versions/:version_id/verify` | QA approve |
| `POST /api/documents/:doc_id/versions/:version_id/reject` | QA reject with code |
| `GET /api/requirements` | List requirements (with filters) |

### DSL Verbs

| Verb | Purpose |
|------|---------|
| `document.solicit` | Request document from entity |
| `document.solicit-set` | Request multiple documents |
| `document.verify` | QA approves version |
| `document.reject` | QA rejects with reason code |
| `requirement.create` | Initialize requirement |
| `requirement.waive` | Manual override (skip requirement) |

### Workflow Guard Integration

Guards check **requirement status**, not raw document existence:

```yaml
states:
  awaiting_identity:
    requirements:
      - type: requirement_satisfied
        doc_type: passport
        min_state: verified       # Must be QA-approved
        subject: $entity_id
        max_age_days: 90          # Recency check on satisfied_at
```

### Key Files

| File | Purpose |
|------|---------|
| `migrations/049_workflow_task_queue_documents.sql` | Schema (all tables) |
| `rust/crates/ob-workflow/src/listener.rs` | Queue listener with retry/DLQ |
| `rust/crates/ob-workflow/src/cargo_ref.rs` | CargoRef URI scheme |
| `rust/crates/ob-workflow/src/document.rs` | Document types, RequirementState |
| `rust/src/api/workflow_routes.rs` | HTTP endpoints |
| `rust/config/verbs/document.yaml` | 7 document verbs |
| `rust/config/verbs/requirement.yaml` | 5 requirement verbs |

---

## Transactional Execution & Advisory Locking (050)

> ✅ **IMPLEMENTED (2026-01-24)**: Atomic execution with PostgreSQL advisory locks, deterministic template expansion, and audit trail.

**Problem Solved:** Multi-statement DSL batches need transactional guarantees. Without locking, concurrent sessions can corrupt shared entities. The expansion stage derives locks deterministically from DSL structure.

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  DSL EXECUTION PIPELINE WITH EXPANSION                                       │
│                                                                              │
│  Source DSL: (cbu.create ...) (entity.create ...) (trading-profile.create)  │
│      │                                                                       │
│      ▼                                                                       │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  EXPANSION STAGE (deterministic)                                     │    │
│  │  • Expand template verbs (behavior: template)                        │    │
│  │  • Derive lock set from entity references                            │    │
│  │  • Determine batch_policy: atomic vs best_effort                     │    │
│  │  • Generate ExpansionReport for audit                                │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│      │                                                                       │
│      ├─── batch_policy = atomic ──────────────────────────┐                 │
│      │                                                     ▼                 │
│      │    ┌───────────────────────────────────────────────────────────┐     │
│      │    │  ATOMIC EXECUTOR                                          │     │
│      │    │  • Acquire advisory locks (sorted, no deadlock)           │     │
│      │    │  • Single transaction wraps all statements                │     │
│      │    │  • Any failure → full rollback                            │     │
│      │    │  • Result: Committed | RolledBack | LockContention        │     │
│      │    └───────────────────────────────────────────────────────────┘     │
│      │                                                                       │
│      └─── batch_policy = best_effort ─────────────────────┐                 │
│                                                            ▼                 │
│           ┌───────────────────────────────────────────────────────────┐     │
│           │  BEST-EFFORT EXECUTOR                                     │     │
│           │  • No locking required                                    │     │
│           │  • Execute statements independently                       │     │
│           │  • Continue on failure, aggregate errors                  │     │
│           │  • Result: { succeeded: [...], failed: [...] }            │     │
│           └───────────────────────────────────────────────────────────┘     │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Batch Policy

| Policy | Lock Acquisition | Failure Behavior | Use Case |
|--------|------------------|------------------|----------|
| `atomic` | Advisory locks (sorted by entity_id) | Full rollback | Multi-entity operations needing consistency |
| `best_effort` | None | Continue, aggregate errors | Independent operations, bulk imports |

**Policy Determination:**
- `atomic`: Template verbs with `batch_policy: atomic`, or DSL touching multiple related entities
- `best_effort`: Independent statements, bulk operations, read-only queries

### Advisory Locks

PostgreSQL `pg_advisory_xact_lock` provides session-scoped locking:

```rust
pub struct DerivedLock {
    pub entity_type: String,  // "cbu", "entity", "trading_profile"
    pub entity_id: Uuid,      // Target entity
    pub access: LockAccess,   // Read or Write
}

// Lock key derivation: deterministic i64 from (entity_type, entity_id) using DefaultHasher
// Locks acquired in sorted order (entity_id) to prevent deadlocks
// Released automatically when transaction commits/rolls back
```

### Error Aggregation

Errors grouped by root cause for clear diagnostics:

| ErrorCause | Description |
|------------|-------------|
| `EntityDeleted` | Entity was deleted mid-execution |
| `EntityNotFound` | Referenced entity doesn't exist |
| `VersionConflict` | Optimistic lock failure |
| `ConstraintViolation` | FK/unique constraint failed |

`FailureTiming` distinguishes `PreExisting` (bad input) vs `MidExecution` (race condition).

### Expansion Safety Rules

- **Expansion output is UNTRUSTED** - validate expanded macros like hostile input
- **Max expansion steps: 100** - prevents runaway recursive macros
- **No macro recursion** - primitives only in expansion output

### Execution Results

```rust
pub enum AtomicExecutionResult {
    Committed {
        results: Vec<ExecutionResult>,
        lock_stats: LockStats,
    },
    RolledBack {
        cause: RollbackCause,
        partial_results: Vec<ExecutionResult>,
    },
    LockContention {
        contested_locks: Vec<DerivedLock>,
        retry_after_ms: Option<u64>,
    },
}

pub struct BestEffortExecutionResult {
    pub succeeded: Vec<(usize, ExecutionResult)>,  // (statement_index, result)
    pub failed: Vec<(usize, ExecutionError)>,       // (statement_index, error)
    pub skipped: Vec<usize>,                        // Skipped due to dependency
}
```

### Expansion Report (Audit Trail)

Every DSL execution produces an `ExpansionReport` persisted to `ob-poc.expansion_reports`:

| Field | Description |
|-------|-------------|
| `expansion_id` | Unique ID for this expansion |
| `session_id` | Session that triggered execution |
| `source_digest` | SHA-256 of canonical source DSL |
| `expanded_dsl_digest` | SHA-256 of expanded DSL |
| `batch_policy` | `atomic` or `best_effort` |
| `derived_lock_set` | JSON array of locks derived |
| `template_digests` | Templates used (name, version, hash) |
| `invocations` | Template invocation details |
| `diagnostics` | Warnings/errors during expansion |

### Key Files

| File | Purpose |
|------|---------|
| `migrations/050_expansion_audit.sql` | Schema for expansion_reports |
| `rust/src/dsl_v2/expansion/engine.rs` | Template expansion, lock derivation |
| `rust/src/dsl_v2/expansion/policy.rs` | Batch policy determination |
| `rust/src/dsl_v2/executor.rs` | `execute_plan_atomic_with_locks`, `execute_plan_best_effort` |
| `rust/src/dsl_v2/locking.rs` | Advisory lock acquisition/release |
| `rust/src/database/expansion_audit.rs` | ExpansionAuditRepository |
| `rust/src/api/agent_routes.rs` | Session execution with expansion |
| `rust/src/mcp/handlers/core.rs` | MCP dsl_execute with expansion |

### Database Table

```sql
CREATE TABLE "ob-poc".expansion_reports (
    expansion_id UUID PRIMARY KEY,
    session_id UUID NOT NULL,
    source_digest VARCHAR(64) NOT NULL,
    expanded_dsl_digest VARCHAR(64) NOT NULL,
    expanded_statement_count INTEGER NOT NULL,
    batch_policy VARCHAR(20) NOT NULL CHECK (batch_policy IN ('atomic', 'best_effort')),
    derived_lock_set JSONB NOT NULL DEFAULT '[]',
    template_digests JSONB NOT NULL DEFAULT '[]',
    invocations JSONB NOT NULL DEFAULT '[]',
    diagnostics JSONB NOT NULL DEFAULT '[]',
    expanded_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

---


---

## Playbook System (059)

> ✅ **IMPLEMENTED (2026-01-28)**: YAML-based playbook specs with LSP validation, source mapping, and CLI tooling.

**Purpose:** Define reusable multi-step workflows as YAML playbooks that lower to DSL statements with slot substitution.

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    PLAYBOOK PIPELINE                                         │
│                                                                              │
│  .playbook.yaml file                                                        │
│         │                                                                    │
│         ▼                                                                    │
│  playbook-core::parse_playbook()                                            │
│         │   - Pass 1: serde_yaml → PlaybookSpec (typed)                     │
│         │   - Pass 2: marked-yaml → SourceMap (line:col positions)          │
│         │                                                                    │
│         ▼                                                                    │
│  playbook-lower::lower_playbook()                                           │
│         │   - Substitutes ${slot} references                                │
│         │   - Reports missing required slots                                │
│         │   - Generates DSL statements                                      │
│         │                                                                    │
│         ▼                                                                    │
│  DSL statements ready for execution                                         │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Playbook Spec Format

```yaml
id: setup-pe-fund
version: 1
name: "Set Up PE Fund Structure"
slots:
  fund_name:
    type: string
    required: true
  fund_type:
    type: string
    required: false
    default: "private-equity"
steps:
  - id: create-fund
    verb: cbu.create
    args:
      name: "${fund_name}"
      kind: "${fund_type}"
  - id: assign-gp
    verb: cbu-role.assign
    after: [create-fund]
    args:
      cbu_id: "${create-fund.result}"
      role: general-partner
```

### Crates

| Crate | Purpose |
|-------|---------|
| `playbook-core` | Spec types, parser with marked-yaml source mapping |
| `playbook-lower` | Slot state management, DSL lowering |

### Parser (Two-Pass with marked-yaml)

```rust
// Pass 1: Typed deserialization
let spec: PlaybookSpec = serde_yaml::from_str(source)?;

// Pass 2: AST walk with source positions
let node = marked_yaml::parse_yaml(0, source)?;
let marker = verb_node.span().start()?;  // Marker{line, col}
```

No string scanning or regex - pure AST traversal with exact source positions.

### LSP Support

- **File detection:** `.playbook.yaml`, `.playbook.yml`
- **Diagnostics:** Parse errors, missing slots reported with exact positions
- **Debouncing:** 100ms delay before analysis (avoids thrashing)
- **Completion:** Verb completions from macro registry

### CLI Tool

```bash
# Check playbook(s)
cargo run -p xtask -- playbook-check <files> [-v|--verbose]

# Example
cargo run -p xtask -- playbook-check data/playbooks/*.playbook.yaml -v
```

### Zed Extension

Extension updated to recognize `.playbook.yaml` files:
- Syntax highlighting via tree-sitter-yaml
- Language server integration for diagnostics
- Verb completion in `verb:` context

### Key Files

| File | Purpose |
|------|---------|
| `rust/crates/playbook-core/src/spec.rs` | PlaybookSpec, SlotSpec, StepSpec |
| `rust/crates/playbook-core/src/parser.rs` | Two-pass parser with marked-yaml |
| `rust/crates/playbook-core/src/source_map.rs` | SourceSpan, StepSpan |
| `rust/crates/playbook-lower/src/lower.rs` | lower_playbook(), MissingSlot |
| `rust/crates/playbook-lower/src/slots.rs` | SlotState, SlotValue |
| `rust/crates/dsl-lsp/src/handlers/playbook.rs` | LSP playbook analysis |
| `rust/crates/dsl-lsp/zed-extension/languages/playbook/` | Zed language config |
| `rust/xtask/src/main.rs` | PlaybookCheck command |

---

## Zed Extension: DSL Tree-sitter Grammar (Dev Install)

> **Critical for local development** - Installing the custom DSL grammar in Zed.
> This was painful to get working. Follow these steps EXACTLY.

### Directory Structure

```
rust/crates/dsl-lsp/
├── zed-extension/           # Extension root (select THIS for dev install)
│   ├── extension.toml       # Must be at root of selected directory
│   ├── Cargo.toml           # Rust extension for LSP launch
│   ├── src/lib.rs           # LSP command provider
│   ├── languages/
│   │   ├── dsl/
│   │   │   ├── config.toml      # grammar = "dsl" (must match [grammars.dsl])
│   │   │   ├── highlights.scm
│   │   │   ├── brackets.scm
│   │   │   ├── indents.scm
│   │   │   ├── outline.scm
│   │   │   ├── textobjects.scm
│   │   │   ├── overrides.scm
│   │   │   └── runnables.scm
│   │   └── playbook/
│   │       └── config.toml      # grammar = "yaml"
│   └── snippets/
│       └── dsl.json
└── tree-sitter-dsl/         # Grammar source
    ├── grammar.js
    ├── package.json         # tree-sitter-cli devDependency
    ├── src/parser.c         # Generated - run `npx tree-sitter generate`
    └── tree-sitter-dsl.wasm # Generated - run `npx tree-sitter build --wasm`
```

### Installation Steps

1. **Upgrade tree-sitter CLI** (0.22+ required for Docker-free WASM build):
   ```bash
   cd rust/crates/dsl-lsp/tree-sitter-dsl
   npm install tree-sitter-cli@latest --save-dev
   ```

2. **Generate grammar artifacts:**
   ```bash
   npx tree-sitter generate
   npx tree-sitter build --wasm   # Downloads wasi-sdk automatically, no Docker
   ```

3. **Verify config alignment:**
   - `languages/dsl/config.toml`: `grammar = "dsl"` (NOT "clojure")
   - `extension.toml`: `[grammars.dsl]` section exists with proper config

4. **Clean any stale grammar cache** (if reinstalling):
   ```bash
   rm -rf rust/crates/dsl-lsp/zed-extension/grammars/
   ```

5. **Install in Zed:**
   - Open Extensions panel (Cmd+Shift+X or `zed: extensions`)
   - Click **"Install Dev Extension"**
   - Select: `rust/crates/dsl-lsp/zed-extension/` (the directory with `extension.toml`)

6. **Verify:**
   - Open any `.dsl` file
   - Status bar should show "DSL" (not Clojure/Plain Text)
   - Syntax highlighting should work (verb names, keywords, symbols)

### extension.toml Grammar Config (EXACT FORMAT)

```toml
# For local development - file:// URL to REPO ROOT + path to grammar
# BOTH repository AND rev are REQUIRED (even for file://)
[grammars.dsl]
repository = "file:///Users/adamtc007/Developer/ob-poc"
rev = "c50cd396ac861ee21c8db56de664ed55b3a9b1f0"  # Must be actual commit SHA
path = "rust/crates/dsl-lsp/tree-sitter-dsl"

# For publishing - use https:// with commit SHA + path
[grammars.dsl]
repository = "https://github.com/your-org/ob-poc"
rev = "abc123def456"  # Must be commit SHA, NOT "main" or "HEAD"
path = "rust/crates/dsl-lsp/tree-sitter-dsl"
```

**To get current commit SHA:**
```bash
git rev-parse HEAD
```

### Common Failure Modes (Battle-Tested)

| Problem | Cause | Fix |
|---------|-------|-----|
| `.dsl` shows as Clojure | `grammar = "clojure"` in config.toml | Change to `grammar = "dsl"` |
| No highlighting | Grammar name mismatch | `config.toml` grammar must match `[grammars.X]` key |
| "missing field `rev`" | `rev` not specified | Add `rev = "<commit-sha>"` (required even for file://) |
| "pathspec 'HEAD' did not match" | `rev = "HEAD"` doesn't work | Use actual commit SHA from `git rev-parse HEAD` |
| "grammar directory already exists" | Stale cache | Delete `zed-extension/grammars/` and reinstall |
| "Failed to fetch grammar" | Wrong file:// path | Point to repo root, use `path` for subdirectory |
| "parser.c missing" | Grammar not generated | Run `npx tree-sitter generate` |
| WASM build needs Docker | Old tree-sitter CLI (<0.22) | `npm install tree-sitter-cli@latest` |
| Extension not found | Wrong folder selected | Must select folder containing `extension.toml` |
| TOML parse error with curly quotes | Copy-paste from docs | Replace `""` with `"`, `''` with `'` |
| "expected newline" in TOML | Malformed string escaping | Check `autoclose_before` and bracket strings |

### config.toml Gotchas

**NEVER use curly/smart quotes** - they break TOML parsing:
```toml
# WRONG - curly quotes from copy-paste
autoclose_before = "]"'"
brackets = [{ start = """, end = """ }]

# CORRECT - straight quotes
autoclose_before = "]'\""
brackets = [{ start = "\"", end = "\"" }]
```

### Debug

1. Open Zed log: `zed: open log` command
2. Search for "grammar", "extension", "TOML", or "dsl"
3. Look for specific error messages

Common log patterns:
- `compiled grammar dsl` = SUCCESS
- `TOML parse error at line X` = config.toml syntax error
- `missing field 'rev'` = extension.toml needs rev
- `failed to checkout revision` = wrong rev or stale cache

### Key Files

| File | Purpose |
|------|---------|
| `zed-extension/extension.toml` | Extension manifest + grammar registration |
| `zed-extension/languages/dsl/config.toml` | Language config (grammar binding) |
| `zed-extension/languages/playbook/config.toml` | Playbook YAML config |
| `zed-extension/languages/dsl/highlights.scm` | Syntax highlighting queries |
| `tree-sitter-dsl/grammar.js` | Grammar definition |
| `tree-sitter-dsl/package.json` | tree-sitter-cli version (keep at 0.22+) |
| `tree-sitter-dsl/src/parser.c` | Generated parser (commit this) |
| `tree-sitter-dsl/tree-sitter-dsl.wasm` | Compiled WASM (Zed compiles this itself) |

### After Making Changes

If you modify the grammar or extension config:
1. Re-run `npx tree-sitter generate` (if grammar.js changed)
2. Delete `zed-extension/grammars/` to clear cache
3. Re-run "Install Dev Extension" in Zed
4. Check `zed: open log` for errors

---

## DSL Language Server (dsl-lsp)

> ✅ **IMPLEMENTED (060-063)**: Full LSP with completions, hover, rename, diagnostics, code actions, and EntityGateway integration.

**Crate:** `rust/crates/dsl-lsp/` — Language Server Protocol implementation for the Onboarding DSL.

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    DSL LANGUAGE SERVER                                       │
│                                                                              │
│  Editor (Zed/VS Code)         LSP Server (tower-lsp)        External        │
│  ───────────────────          ──────────────────────        ────────        │
│                                                                              │
│  .dsl file opened    ───►    did_open()                                     │
│                               │                                              │
│                               ├─► parse_with_v2() (dsl-core)                │
│                               ├─► Extract symbols (@bindings)               │
│                               ├─► LspValidator.validate()                   │
│                               ├─► analyse_and_plan() (DAG)    ◄─── ob-poc  │
│                               └─► publish_diagnostics()                     │
│                                                                              │
│  User types          ───►    did_change() [debounced 100ms]                 │
│                               └─► Re-analyze + re-publish                   │
│                                                                              │
│  Ctrl+Space          ───►    completion()                                   │
│                               ├─► detect_completion_context()               │
│                               ├─► complete_verbs / keywords / symbols       │
│                               └─► EntityGateway lookup  ◄─── gRPC          │
│                                                                              │
│  Hover               ───►    hover()                                        │
│                               └─► Verb docs, symbol info, error suggestions │
│                                                                              │
│  F2 Rename           ───►    rename()                                       │
│                               └─► Find all symbol refs, apply edits         │
│                                                                              │
│  Cmd+.               ───►    code_action()                                  │
│                               ├─► Implicit creates (PlanningOutput)         │
│                               ├─► Reorder suggestions                       │
│                               └─► Entity "did you mean?" fixes              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### LSP Capabilities

| Capability | Handler | Features |
|------------|---------|----------|
| **Diagnostics** | `diagnostics.rs` | Syntax errors, semantic validation, DAG warnings |
| **Completion** | `completion.rs` | Verbs, `:keywords`, `@symbols`, entity lookups, next-step suggestions |
| **Hover** | `hover.rs` | Verb documentation, symbol info, error suggestions |
| **Go to Definition** | `goto_definition.rs` | Jump to `@symbol` definitions |
| **Find References** | `goto_definition.rs` | Find all uses of `@symbol` |
| **Rename** | `rename.rs` | Rename symbols across document |
| **Signature Help** | `signature.rs` | Verb argument hints while typing |
| **Document Symbols** | `symbols.rs` | Outline view of verbs and bindings |
| **Code Actions** | `code_actions.rs` | Quick fixes, implicit creates, reordering |

### Completion Contexts

The LSP detects context and routes to appropriate completer:

| Context | Trigger | Completions |
|---------|---------|-------------|
| `VerbName` | After `(` | All verbs from registry |
| `Keyword` | After verb name | Valid `:args` for that verb |
| `KeywordValue` | After `:keyword` | Enum values, entity lookups |
| `SymbolRef` | After `@` | Defined `@bindings` in document |
| `None` | Empty line | DAG-based next-step suggestions |

### Entity Gateway Integration

For entity completion (e.g., `:counterparty <...>`), the LSP connects to EntityGateway via gRPC:

```bash
# Environment variables
ENTITY_GATEWAY_URL=http://localhost:50051   # Default: [::1]:50051
DSL_CONFIG_DIR=/path/to/config/verbs/       # Verb YAML directory
```

If EntityGateway is unavailable, LSP falls back to syntax-only mode.

### Code Actions

Three sources of code actions from `PlanningOutput`:

| Source | Example Action |
|--------|----------------|
| `synthetic_steps` | "Create @cbu with cbu.create" (implicit binding) |
| `was_reordered` | "Reorder statements for dependencies" |
| `SemanticDiagnostic.suggestions` | "Did you mean 'John Smith'?" |

### Document Analysis Pipeline

```
DSL Source
    │
    ▼
parse_with_v2() ──► DocumentState { text, expressions, symbol_defs, symbol_refs }
    │
    ▼
LspValidator.validate() ──► SemanticDiagnostics (entity resolution errors)
    │
    ▼
analyse_and_plan() ──► PlanningOutput { phases, synthetic_steps, was_reordered }
    │
    ▼
publish_diagnostics() ──► Editor shows errors/warnings
```

### Shared Validation with Server

The LSP uses the **same validation code** as the server:

```rust
// Both LSP and server use:
ob_poc::parse_program()           // Parser
ob_poc::LspValidator              // Semantic validation
ob_poc::planning_facade           // DAG analysis
ob_poc::RuntimeVerbRegistry       // Verb metadata
```

This ensures LSP diagnostics match server behavior 100%.

### Playbook Support

Files matching `*.playbook.yaml` / `*.playbook.yml` get specialized handling:

- Parse with `playbook-core`
- Lower with `playbook-lower`
- Report missing required slots as warnings
- Validate verb references

### Running the LSP

```bash
# Direct execution
cargo run --release -p dsl-lsp

# Zed auto-launches via extension config
# VS Code: configure in settings.json
```

### Logging

LSP logs to file (stdout reserved for protocol):

```bash
tail -f /tmp/dsl-lsp.log

# Control verbosity
DSL_LSP=trace cargo run -p dsl-lsp   # Max verbosity
DSL_LSP=warn cargo run -p dsl-lsp    # Errors only
```

### Key Files

| File | Purpose |
|------|---------|
| `src/main.rs` | Entry point, stdio setup |
| `src/server.rs` | Core LSP state machine, lifecycle |
| `src/analysis/document.rs` | `DocumentState` - parsed document representation |
| `src/analysis/context.rs` | `detect_completion_context()` |
| `src/handlers/completion.rs` | Completion logic with sub-completers |
| `src/handlers/diagnostics.rs` | Validation pipeline |
| `src/handlers/code_actions.rs` | Code action generation |
| `src/handlers/hover.rs` | Hover information |
| `src/handlers/rename.rs` | Symbol rename |
| `src/encoding.rs` | UTF-16/UTF-8 position conversion |
| `src/entity_client.rs` | gRPC client for EntityGateway |

### Performance

| Optimization | Implementation |
|--------------|----------------|
| Debouncing | 100ms delay on `did_change` |
| Incremental sync | Clients send deltas, not full text |
| Lazy EntityGateway | Only connects when completion needs entities |
| Lazy planning | Only runs if no parse errors |
| Caching | Verb registry and macro registry cached in `OnceLock` |

---

## Staged Runbook REPL (054)

> ✅ **IMPLEMENTED (2026-01-25)**: Anti-hallucination execution model with staged commands, entity resolution, DAG ordering.

**Problem Solved:** Agent could hallucinate entity UUIDs or execute commands without user confirmation. The staged runbook provides a deterministic bridge from natural language → resolved UUIDs → safe execution.

### Architecture

```
User prompt: "Show me Irish funds"
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│  STAGE (no execution)                                           │
│  runbook_stage tool                                             │
│  • Parse DSL                                                    │
│  • Resolve entity arguments → UUIDs via DB search               │
│  • Stage command in staged_runbook table                        │
│  • Emit CommandStaged / ResolutionAmbiguous / StageFailed       │
└─────────────────────────────────────────────────────────────────┘
    │
    ▼ (user reviews, may pick from ambiguous, may remove commands)
    │
    ▼ (explicit "run" / "execute" / "commit")
┌─────────────────────────────────────────────────────────────────┐
│  RUN (server-side ready gate)                                   │
│  runbook_run tool                                               │
│  • Validate all commands resolved                               │
│  • Compute DAG order                                            │
│  • Execute in order                                             │
│  • Emit per-command results                                     │
└─────────────────────────────────────────────────────────────────┘
```

### Non-Negotiable Invariants

1. **No side-effects** unless user explicitly says `run/execute/commit`
2. **No invented UUIDs** - All UUIDs from DB resolution or picker validation
3. **Picker validation** - `runbook_pick` entity_ids must match stored candidates
4. **Server-side ready gate** - `runbook_run` rejects if not all resolved
5. **DAG ordering** - Dependencies detected and reordered transparently

### MCP Tools

| Tool | Description |
|------|-------------|
| `runbook_stage` | Stage a command (parse + resolve, no execute) |
| `runbook_pick` | Select from ambiguous candidates |
| `runbook_show` | Show current runbook state |
| `runbook_preview` | Preview with readiness check |
| `runbook_remove` | Remove a staged command |
| `runbook_run` | Execute (explicit user confirmation required) |
| `runbook_abort` | Clear all staged commands |

### Key Files

| File | Purpose |
|------|---------|
| `migrations/054_staged_runbook.sql` | Schema (staged_runbook, staged_command, etc.) |
| `rust/src/repl/staged_runbook.rs` | Rust types (StagedRunbook, ResolutionStatus) |
| `rust/src/repl/repository.rs` | DB access layer |
| `rust/src/repl/resolver.rs` | EntityArgResolver (shorthand → UUID) |
| `rust/src/repl/dag_analyzer.rs` | Dependency detection, topological sort |
| `rust/src/repl/service.rs` | RunbookService (stage/pick/run/show/abort) |
| `rust/src/mcp/handlers/runbook.rs` | MCP tool handlers |
| `rust/config/verbs/runbook.yaml` | Verb definitions |

### Database Tables

| Table | Purpose |
|-------|---------|
| `staged_runbook` | Session-scoped runbook container |
| `staged_command` | Individual DSL commands with resolution status |
| `staged_command_entity` | Resolved entity footprint |
| `staged_command_candidate` | Picker candidates for ambiguous resolution |

---

## Client Group Research Integration (055)

> ✅ **IMPLEMENTED (2026-01-26)**: GLEIF import populates client_group_entity staging tables, then CBU creation with GLEIF role mapping.

**Problem Solved:** Research (GLEIF import) and onboarding (CBU creation) were coupled. Now they are separate concerns:
1. **Research phase**: GLEIF import → entities + `client_group_entity` staging
2. **Onboarding phase**: `cbu.create-from-client-group` → CBUs with role mapping

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  PHASE 1: RESEARCH (GLEIF Import)                                           │
│                                                                              │
│  (gleif.import-tree :entity-id <Allianz SE> :depth 3)                       │
│      │                                                                       │
│      ├─► entities table (all LEI entities)                                  │
│      ├─► entity_funds (with gleif_category: FUND, GENERAL)                  │
│      ├─► entity_parent_relationships (corporate hierarchy)                  │
│      └─► client_group_entity + client_group_entity_roles                    │
│              │                                                               │
│              └─► Roles: SUBSIDIARY, ULTIMATE_PARENT (corporate hierarchy)   │
└─────────────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  PHASE 2: ONBOARDING (CBU Creation)                                         │
│                                                                              │
│  (cbu.create-from-client-group :group-id <Allianz> :gleif-category "FUND")  │
│      │                                                                       │
│      ├─► Query client_group_entity WHERE gleif_category = 'FUND'            │
│      │                                                                       │
│      ├─► Create CBU per entity                                               │
│      │                                                                       │
│      └─► Assign CBU entity roles via GLEIF→CBU role mapping:                │
│              • GLEIF category FUND → ASSET_OWNER                            │
│              • Group role ULTIMATE_PARENT → HOLDING_COMPANY                 │
│              • Group role SUBSIDIARY → SUBSIDIARY                           │
│              • Optional: MANAGEMENT_COMPANY, INVESTMENT_MANAGER             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### GLEIF Role → CBU Role Mapping

| GLEIF Source | CBU Entity Role | Rationale |
|--------------|-----------------|-----------|
| `gleif_category = 'FUND'` | `ASSET_OWNER` | Fund entity owns its trading unit |
| `group_role = 'ULTIMATE_PARENT'` | `HOLDING_COMPANY` | Top of corporate hierarchy |
| `group_role = 'SUBSIDIARY'` | `SUBSIDIARY` | Corporate subsidiary |
| (default) | `ASSET_OWNER` | Safe default for fund onboarding |

### Key Verbs

| Verb | Description |
|------|-------------|
| `gleif.import-tree` | Import corporate hierarchy from GLEIF into entities + client_group_entity |
| `cbu.create-from-client-group` | Create CBUs from staged entities with role mapping |

### DSL Usage

```clojure
;; Phase 1: Research - import corporate hierarchy
(gleif.import-tree :entity-id <Allianz SE> :depth 3 :as @import)

;; Phase 2: Onboard - create CBUs for Luxembourg FUNDs
(cbu.create-from-client-group 
  :group-id <Allianz>
  :gleif-category "FUND"
  :jurisdiction-filter "LU"
  :manco-entity-id <AGI Holdings GmbH>)

;; Dry-run to preview
(cbu.create-from-client-group 
  :group-id <Allianz>
  :gleif-category "FUND"
  :dry-run true
  :limit 10)
```

### Key Tables

| Table | Purpose |
|-------|---------|
| `client_group_entity` | Staging: entities discovered via research |
| `client_group_entity_roles` | Staging: GLEIF roles (SUBSIDIARY, ULTIMATE_PARENT) |
| `entity_funds.gleif_category` | GLEIF entity classification (FUND, GENERAL) |
| `cbu_entity_roles` | Production: CBU membership roles |

### Key Files

| File | Purpose |
|------|---------|
| `migrations/055_client_group_research.sql` | Schema for client_group_entity_roles |
| `rust/src/gleif/enrichment.rs` | GLEIF tree import with relationship tracking |
| `rust/src/domain_ops/gleif_ops.rs` | `GleifImportTreeOp` - links to client_group_entity |
| `rust/src/domain_ops/cbu_ops.rs` | `CbuCreateFromClientGroupOp` - CBU creation with role mapping |
| `rust/config/verbs/cbu.yaml` | `create-from-client-group` verb definition |

---

## Verb Search Test Harness

Comprehensive test harness for verifying semantic matching after teaching new phrases or tuning thresholds.

### Quick Start

```bash
# Run all verb search tests
cargo x test-verbs

# Specific test categories
cargo x test-verbs --taught       # Only taught phrase tests
cargo x test-verbs --hard-negatives  # Dangerous confusion detection
cargo x test-verbs --sweep        # Threshold calibration sweep

# Explore a specific query
cargo x test-verbs --explore "load the allianz book"
```

### Test Categories

| Category | Purpose |
|----------|---------|
| `taught` | Verify taught phrases match expected verbs |
| `session` | Session management verbs (load, unload, undo) |
| `cbu` | CBU lifecycle verbs |
| `hard_negative` | Dangerous confusions (delete vs archive) |
| `safety_first` | Verbs where ambiguity is acceptable |
| `edge` | Edge cases (garbage input, ambiguous queries) |

### Threshold Sweep (Full Pipeline)

The sweep tests both retrieval AND decision thresholds:

```bash
# Quick sweep (3 combinations)
cargo x test-verbs --sweep

# Full sweep (180 combinations) - comprehensive exploration
DATABASE_URL="postgresql:///data_designer" \
  cargo test --features database --test verb_search_integration test_threshold_sweep_full -- --ignored --nocapture
```

**Output:**
```
sem_thr  fall_thr margin top1_hit%   ambig%    wrong no_match
   0.85     0.78   0.05      85.0       5.0        0        2
   0.88     0.78   0.05      80.0       8.0        0        4
   ...
✓ BEST (0 wrong): semantic=0.85 fallback=0.78 margin=0.05 → 85.0% top-1
```

### Safety-First Policy

For dangerous verb pairs (delete/archive, approve/submit), the harness uses `ExpectedOutcome::MatchedOrAmbiguous`:

- **Matched(correct)** → Pass (correct verb selected)
- **Ambiguous** → Pass (disambiguation UI triggered - safe)
- **Matched(wrong)** → FAIL (dangerous confusion)

This prevents optimizing for overconfident behavior on safety-critical verbs.

### Key Files

| File | Purpose |
|------|---------|
| `rust/tests/verb_search_integration.rs` | Test harness implementation |
| `rust/xtask/src/main.rs` | `cargo x test-verbs` command |

---

## Session Verbs

Session manages which CBUs are loaded. Focus/camera is client-side.

| Verb | Purpose |
|------|---------|
| `session.load-cbu` | Add single CBU to session |
| `session.load-jurisdiction` | Load all CBUs in jurisdiction |
| `session.load-galaxy` | Load all CBUs under apex entity |
| `session.unload-cbu` | Remove CBU from session |
| `session.clear` | Clear all CBUs |
| `session.undo` / `session.redo` | History navigation |
| `session.info` / `session.list` | Query session state |

### View Verbs (ESPER Navigation)

| Verb | Description |
|------|-------------|
| `view.universe` | All CBUs with optional filters |
| `view.book` | CBUs for commercial client |
| `view.cbu` | Focus single CBU (mode: trading/ubo) |
| `view.drill` | Drill into entity (within UBO taxonomy) |
| `view.surface` | Surface up from drill |
| `view.trace` | Follow threads (money/control/risk) |
| `view.xray` | Show hidden layers |
| `view.refine` | Add/remove filters |

---

## Navigation (Unified Intent Pipeline)

All user input—including navigation commands—goes through the unified `IntentPipeline`. Navigation phrases match to `view.*` and `session.*` verbs via semantic search. There is no separate ESPER path.

### Architecture

```
User: "zoom in" / "load the allianz book" / "show ownership"
    │
    ▼
IntentPipeline.process()
    │
    ├─ HybridVerbSearcher.search()
    │       │
    │       ├─ Exact match (learned phrases)
    │       └─ Semantic search (BGE embeddings)
    │
    ├─ Top match: view.drill / session.load-galaxy / control.build-graph
    │
    └─ LLM extracts args → DSL generated → Execute
```

### Key Files

| File | Purpose |
|------|---------|
| `rust/src/mcp/intent_pipeline.rs` | Unified intent processing |
| `rust/src/mcp/verb_search.rs` | `HybridVerbSearcher` - semantic + exact match |
| `rust/src/api/agent_service.rs` | `AgentService.process_chat()` entry point |
| `rust/config/verbs/view.yaml` | Navigation verbs (view.drill, view.surface, etc.) |
| `rust/config/verbs/session.yaml` | Session verbs (session.load-galaxy, etc.) |

### Navigation Verbs

| Verb | Purpose | Example Phrases |
|------|---------|-----------------|
| `view.universe` | Show all CBUs | "show universe", "zoom out to all" |
| `view.cbu` | Focus single CBU | "focus on this cbu", "show cbu details" |
| `view.drill` | Drill into entity | "drill down", "go deeper" |
| `view.surface` | Surface back up | "go back", "zoom out" |
| `session.load-galaxy` | Load CBUs under apex | "load the allianz book" |
| `session.load-cbu` | Load single CBU | "load cbu acme fund" |
| `session.clear` | Clear session | "clear", "start fresh" |
| `session.undo` / `session.redo` | History navigation | "undo", "redo" |

### Verb Search Thresholds (BGE Asymmetric Mode)

BGE uses **asymmetric retrieval**: queries get an instruction prefix (`"Represent this sentence for searching relevant passages: "`), targets don't. This produces **lower** similarity scores than symmetric (target-to-target) comparison:

- **Symmetric (target→target):** scores 0.6-1.0 (same-embedding comparison, like DB tests)
- **Asymmetric (query→target):** scores 0.5-0.8 (instruction-prefixed query vs raw target)

| Threshold | Value | Purpose |
|-----------|-------|---------|
| `fallback_threshold` | 0.55 | Retrieval cutoff for DB queries (must be low enough to retrieve candidates) |
| `semantic_threshold` | 0.65 | Decision gate for accepting match |
| `blocklist_threshold` | 0.80 | Collision detection |

> **Warning:** If you see "No matching verbs found" for queries that should match, check that asymmetric thresholds are being used. The old symmetric thresholds (0.70/0.78) will reject valid asymmetric matches.

### Adding Navigation Phrases

Add to the verb's `invocation_phrases` in YAML, then run `populate_embeddings`:

```yaml
# rust/config/verbs/view.yaml
drill:
  description: "Drill into entity detail"
  invocation_phrases:
    - "drill down"
    - "go deeper"
    - "zoom into"
    - "enhance"      # Add new phrase here
```

```bash
DATABASE_URL="postgresql:///data_designer" \
  cargo run --release --package ob-semantic-matcher --bin populate_embeddings
```

---

## Adding Verbs

> ⚠️ **Before writing verb YAML, read `docs/verb-definition-spec.md`**
> Serde structs are strict. Invalid YAML silently fails to load.

### Verb Behaviors

| Behavior | Execution | Use Case |
|----------|-----------|----------|
| `crud` | Generic executor, single DB operation | Simple CRUD |
| `plugin` | Custom Rust handler | Complex logic |
| `template` | Expands to multi-statement DSL | Workflows, macros |

### Plugin Verb Implementation (CustomOperation)

Plugin verbs require a Rust `CustomOperation` implementation. Use the `#[register_custom_op]` macro for automatic registration via the inventory crate.

```rust
use ob_poc_macros::register_custom_op;

#[register_custom_op]
pub struct MyDomainCreateOp;

#[async_trait]
impl CustomOperation for MyDomainCreateOp {
    fn domain(&self) -> &'static str { "my-domain" }
    fn verb(&self) -> &'static str { "create" }
    fn rationale(&self) -> &'static str { "Complex validation logic" }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Implementation
    }
}
```

**Key points:**
- Macro generates `inventory::submit!` for automatic registration at startup
- Domain/verb must match YAML definition with `behavior: plugin`
- `rationale()` documents why plugin (not CRUD) is needed
- Test coverage: `test_plugin_verb_coverage` verifies all YAML plugin verbs have ops

**Verify coverage:**
```bash
cargo test --lib -- test_plugin_verb_coverage
```

### Quick Example (CRUD)

```yaml
# rust/config/verbs/my_domain.yaml
domains:
  my_domain:
    verbs:
      create:
        description: "Create a record"
        behavior: crud
        crud:
          operation: insert
          table: my_table
          schema: ob-poc
        metadata:
          tier: intent
          source_of_truth: operational
        invocation_phrases:        # Required for LLM discovery
          - "create my thing"
          - "add new record"
        args:
          - name: name
            type: string
            required: true
            maps_to: name
```

### Template Verbs (Macros)

Templates are **first-class verbs** that expand to multi-statement DSL. They enable the LLM to generate complex workflows without training on DSL syntax.

**Key insight:** LLM does INTENT CLASSIFICATION (picks the right template), not DSL generation. Templates emit DSL source that flows through the normal pipeline.

```
User: "Research Aviva group and onboard their Lux funds"
       │
       ▼
┌─────────────────────────────────────────────────────────────┐
│  LLM: Intent Classification                                 │
│  verb: "onboarding.research-group"                         │
│  lookups: { root-entity: "Aviva" }                         │
│  params: { jurisdiction: "LU" }                            │
└─────────────────────────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────────────────────────┐
│  Entity Resolution (same as singleton verbs)               │
│  "Aviva" → UUID via EntityGateway                          │
└─────────────────────────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────────────────────────┐
│  Template Expansion → DSL Source                           │
│                                                             │
│  (gleif.import-tree :entity-id "uuid-..." :as @import)     │
│  (bulk.create-cbu :filter "type=FUND AND jur=LU")          │
│  (bulk.add-products :cbu-set @created :products [...])     │
└─────────────────────────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────────────────────────┐
│  Normal DSL Pipeline (unchanged)                           │
│  Parse → Compile → Session → Execute                       │
└─────────────────────────────────────────────────────────────┘
```

**Template Verb Example:**

```yaml
domains:
  onboarding:
    description: "High-level onboarding workflows"
    
    invocation_hints:
      - "onboard"
      - "bulk"
      - "research group"
    
    verbs:
      research-group:
        description: "Import group from GLEIF and bulk onboard funds"
        behavior: template
        
        invocation_phrases:
          - "research a group"
          - "import corporate hierarchy"
          - "bulk onboard funds from GLEIF"
        
        metadata:
          tier: composite
          source_of_truth: external
          scope: global
          noun: group_onboarding
        
        args:
          - name: root-entity
            type: uuid
            required: true
            description: "Group apex entity"
            lookup:
              table: entities
              entity_type: entity
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          
          - name: jurisdiction
            type: string
            required: false
            default: "LU"
            valid_values: [LU, IE, DE, UK, US]
          
          - name: products
            type: list
            required: false
            default: [EQUITY, FIXED_INCOME]
        
        # Template body - expands to DSL source
        template:
          body: |
            (gleif.import-tree :entity-id $root-entity :direction BOTH :as @import)
            (bulk.create-cbu 
              :filter "source=gleif_import AND type=FUND AND jurisdiction=$jurisdiction"
              :as @created_cbus)
            (bulk.add-products 
              :cbu-set @created_cbus 
              :products $products)
```

**Template vs Singleton:**

| Aspect | Singleton Verb | Template Verb |
|--------|---------------|---------------|
| Invocation | `(cbu.assign-role ...)` | `(onboarding.research-group ...)` |
| YAML location | Same (`config/verbs/`) | Same (`config/verbs/`) |
| Intent matching | `invocation_phrases` | `invocation_phrases` (identical) |
| Entity resolution | `lookup:` on args | `lookup:` on args (identical) |
| Execution | Single operation | Expands → multiple statements |

**Why templates are verbs:**
- Single vocabulary for LLM tool (verbs + templates together)
- Same entity resolution via `lookup:` config
- Same linting and governance rules apply
- User can invoke directly: `(onboarding.research-group :root <Aviva>)`

### Verify

```bash
cargo x verbs check   # YAML matches DB
cargo x verbs lint    # Tiering rules
```

### Entity Resolution in DSL

Entity references in DSL use the `<entity_name>` syntax and are resolved via EntityGateway before execution.

**3-Stage Compiler Model:**
```
DSL Source → Parse (Syntax) → Compile (Semantics) → Execute
                                    │
                                    ▼
                         Detect unresolved <entity> refs
                                    │
                                    ▼
                         Resolution Modal (if ambiguous)
                                    │
                                    ▼
                         Substitute UUIDs → Execute
```

**Resolution config comes from verb YAML:**
```yaml
args:
  - name: entity-id
    type: uuid
    lookup:
      table: entities
      entity_type: entity
      schema: ob-poc
      search_key: name                    # Simple key
      # OR composite s-expression:
      # search_key: "(search_name (nationality :selectivity 0.7) (date_of_birth :selectivity 0.95))"
      primary_key: entity_id
```

**Resolution flow (simplified 2-hop):**
1. Agent generates DSL with `<BlackRock>` entity ref
2. Parser produces AST with `EntityRef { value: "BlackRock", resolved_key: None }`
3. `ChatResponse.unresolved_refs` carries refs directly to UI (no sub-session API call)
4. UI opens resolution modal via `pending_unresolved_refs` in `AsyncState`
5. User selects → `resolved_key` populated with UUID
6. Execute with fully resolved AST

> **Note:** Legacy `needs_resolution_check` flag and `start_resolution()` API removed. Resolution state is inline in session, not a sub-session.

### Two Session Models (Intentionally Separate)

| Model | API | Purpose | Scope Storage |
|-------|-----|---------|---------------|
| `AgentSession` | `/api/session/*` | Full agent workflow (chat, DSL, execution) | `context.cbu_ids` |
| `CbuSession` | `/api/cbu-session/*` | Standalone scope navigation (load/undo/redo) | `state.cbu_ids` (HashSet) |

**Primary workflow:** Use DSL verbs (`session.load-galaxy`, etc.) which update `AgentSession.context.cbu_ids` after execution. This is the integrated path that feeds the viewport.

**Standalone REST:** The `/api/cbu-session/*` endpoints are independent and don't sync to `AgentSession`. Use them for direct REST integration without the agent/DSL layer.

> **Warning:** Don't mix the two models. Pick one per client integration.

### Session = Run Sheet = Viewport Scope

The session is the **single source of truth** that ties the REPL, DSL execution, and viewport together.

> **Full design:** `ai-thoughts/035-session-runsheet-viewport-integration.md`, `ai-thoughts/036-session-rip-and-replace.md`

**Unified Session Types (`rust/src/api/session.rs`):**
- `AgentSession` - Single source of truth for agent conversations
- `ServerRunSheet` - DSL statement ledger with per-statement status (replaces legacy `assembled_dsl`, `pending`, `executed_results`)
- `ServerRunSheetEntry` - Individual DSL statement with status, AST, bindings, affected entities

**RunSheet Status (`DslStatus`):**
- `Draft` - Just added, not yet validated
- `Ready` - Validated, ready to execute
- `Executing` - Currently running
- `Executed` - Successfully completed
- `Failed` - Execution error
- `Cancelled` - User cancelled (undo/clear)

**Unified Session Types (`rust/src/session/unified.rs`):**
- `UnifiedSession` - High-level session combining scope + view state
- `TargetUniverse` - Book/Galaxy/Jurisdiction scope
- `EntityScope` - Active CBU set being worked on  
- `StateStack` - Navigation history (back/forward/go-to-start/go-to-last)
- `ViewState` - Viewport rendering state
- `ResolutionState` - Inline resolution (not a sub-session)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         SESSION = UNIVERSE OF WORK                          │
│                                                                             │
│  Session ID: abc-123                                                        │
│  Entity Scope: [CBU-1, CBU-2, CBU-3, ...]  ← Drives viewport               │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  Run Sheet (accumulated DSL statements)                              │   │
│  │                                                                       │   │
│  │  ✓ (gleif.import-tree :entity-id "aviva" :as @import)                │   │
│  │  ✓ (bulk.create-cbu :filter "..." :as @cbus) → [CBU-1..50]          │   │
│  │  ⏳ (bulk.add-products :cbu-set @cbus :products [...])               │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                              │                                              │
│                              ▼                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  Viewport shows session.entity_scope                                 │   │
│  │  Updates real-time as DSL executes                                   │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
```

**The closed feedback loop:**
- **REPL Panel** shows run sheet with per-statement status
- **Session** tracks entity IDs created/modified by each statement
- **Viewport** subscribes to `session.entity_scope` and shows those CBUs/entities
- **User sees** graph update as DSL executes

**Key fields:**
- `session.entity_scope.cbu_ids` - CBUs being worked on (viewport universe)
- `session.run_sheet.entries[]` - DSL statements with status + affected entities
- `entry.status` - Draft/Ready/Executing/Executed/Failed/Cancelled
- `entry.bindings` - Symbols produced (@cbu, @created_cbus)
- `entry.affected_entities` - Entity UUIDs modified by this statement

**Multi-CBU Viewport Refresh (Scope Graph):**

Session scope is set via `session.load-*` verbs, then DSL execution modifies those CBUs, and the viewport refreshes:

```
1. User sets scope: "Load Allianz Lux book"
    │
    ▼
(session.load-galaxy :apex-entity-id "allianz-se")
    │
    └─► CbuSession.load_many([cbu-1..cbu-50])
            │
            └─► Synced to context.cbu_ids after execution
    │
    ▼
2. User modifies CBUs: "Add custody accounts to all"
    │
    ▼
(bulk.add-custody-account :cbu-set @cbus ...)
    │
    └─► entry.affected_entities = [entity-1..entity-200]
    │
    ▼
3. UI fetches combined graph:
    │
    ▼
GET /api/session/:id/scope-graph
    │
    ▼
MultiCbuGraphResponse {
    graph: CbuGraph,              // Combined graph for all 50 CBUs
    cbu_ids: Vec<Uuid>,           // CBUs in scope
    affected_entity_ids: Vec<Uuid> // 200 entities modified
}
    │
    ▼
Viewport updates with combined graph
```

**Key files:**
- `rust/src/api/graph_routes.rs` - `get_session_scope_graph()` endpoint
- `rust/src/api/agent_routes.rs` - `exec_ctx.take_pending_cbu_session()` sync to `context.cbu_ids`
- `rust/crates/ob-poc-ui/src/app.rs` - `fetch_scope_graph()` method
- `rust/crates/ob-poc-ui/src/state.rs` - `pending_scope_graph`, `needs_scope_graph_refetch`

---

## REPL Session & Phased Execution

> **Full design:** `ai-thoughts/034-repl-session-phased-execution.md`

The agent REPL session is a **state machine** that handles multi-CBU bulk operations:

```
EMPTY → SCOPED → TEMPLATED → GENERATED → PARSED → READY → EXECUTED
         │                                  │        │
         │ scope_dsl                        │        │ user confirms "Run?"
         │ + derived CBU set                │        │
         └──────────────────────────────────┘        │
                                             resolve loop (picker)
```

**Key concepts:**

| Concept | Description |
|---------|-------------|
| **Scope derivation** | "Allianz Lux book" → queries GROUP structure → derives CBU set |
| **Template × CBU set** | Single verb template expanded for each CBU in scope |
| **DAG phases** | Statements grouped by dependency depth (0 = creators, 1 = needs phase 0, etc.) |
| **Two tollgates** | Pre-compile (reorder) and pre-run (phase extraction) using same DAG |
| **Phased execution** | Execute phase 0, collect PKs, substitute into phase 1, repeat |

**Entity type hierarchy** (verb target determines attachment point):

```
Universe → Book → CBU → TradingProfile → Product
                     → Entity (roles)
                     → ISDA → Counterparty
                     → KycCase
```

**Session record stores:**
- `scope_dsl` - DSL that derived the scope
- `template_dsl` - Unpopulated verb template  
- `sheet.statements[]` - Populated DSL with `dag_depth` per statement
- `returned_pk` - PKs collected after each phase executes

**Failure handling:**
- Single transaction wrapping all phases
- Failure → rollback → no partial state
- Downstream statements marked SKIPPED (blocked by failed dependency)
- POC recovery: trash session, start fresh (idempotent verbs are safe)

**`@session_cbus` iteration:**
Template batch operations can use `@session_cbus` as source to iterate over all CBUs in session scope:
```clojure
(template.batch :id "add-custody" :source @session_cbus :bind-as "cbu_id" ...)
```

---

## Code Patterns

### Config Struct Pattern

For types with many optional parameters, use a builder-style config struct:

```rust
// Instead of many constructors:
// fn new(pool: PgPool) -> Self
// fn with_sessions(pool: PgPool, sessions: SessionStore) -> Self
// fn with_all_sessions(...) -> Self

// Use config struct with builder:
pub struct ToolHandlersConfig {
    pub pool: PgPool,
    pub sessions: Option<SessionStore>,
    pub cbu_sessions: Option<CbuSessionStore>,
    pub learned_data: Option<SharedLearnedData>,
    pub embedder: Option<SharedEmbedder>,
}

// Usage:
let handlers = ToolHandlersConfig::new(pool)
    .with_sessions(sessions)
    .with_embedder(embedder)
    .build();
```

See `rust/src/mcp/handlers/core.rs` for the `ToolHandlersConfig` pattern.

### Centralized DB Access

All database access should go through service modules in `rust/src/database/`:
- `VerbService` for verb discovery and semantic search
- `VisualizationRepository` for graph/viewport queries
- `SessionRepository` for session persistence
- `GenerationLogRepository` for training data

No direct `sqlx::query` calls outside of service modules.

---

## Key Directories

```
ob-poc/
├── rust/
│   ├── config/verbs/           # 103 YAML verb definitions
│   ├── crates/
│   │   ├── dsl-core/           # Parser, AST, compiler (no DB)
│   │   ├── dsl-lsp/            # LSP server + Zed extension + tree-sitter grammar
│   │   ├── ob-agentic/         # Onboarding pipeline (Intent→Plan→DSL)
│   │   ├── ob-poc-macros/      # Proc macros (#[register_custom_op], #[derive(IdType)])
│   │   ├── ob-poc-ui/          # egui/WASM UI
│   │   └── ob-poc-graph/       # Graph visualization
│   └── src/
│       ├── dsl_v2/             # DSL execution
│       │   ├── custom_ops/     # Plugin handlers
│       │   └── generic_executor.rs
│       ├── domain_ops/         # CustomOperation implementations (~300+ ops)
│       ├── session/            # Session state
│       ├── graph/              # Graph builders
│       └── api/                # REST routes
├── migrations/                 # SQLx migrations
├── docs/                       # Architecture docs
└── ai-thoughts/                # ADRs and design docs
```

---

## Environment Variables

```bash
# Required
DATABASE_URL="postgresql:///data_designer"

# LLM (for agent chat, not embeddings)
AGENT_BACKEND=anthropic
ANTHROPIC_API_KEY="sk-ant-..."

# Optional
DSL_CONFIG_DIR="/path/to/config"
BRAVE_SEARCH_API_KEY="..."       # Research macros
```

---

## Database Practices

### SQLx Compile-Time Verification

```bash
# After schema changes
psql -d data_designer -f your_migration.sql
cd rust && cargo sqlx prepare --workspace
cargo build  # Catches type mismatches
```

### Type Mapping

| PostgreSQL | Rust |
|------------|------|
| UUID | `Uuid` |
| TIMESTAMPTZ | `DateTime<Utc>` |
| INTEGER | `i32` |
| BIGINT | `i64` |
| NUMERIC | `BigDecimal` |
| NULLABLE | `Option<T>` |

---

## Error Handling

**Never use `.unwrap()` in production paths.** Use:
- `?` operator
- `.ok_or_else(|| anyhow!(...))`
- `let Some(x) = ... else { continue }`
- `match` / `if let`

---

## Trigger Phrases

When you see these in a task, read the corresponding annex first:

| Phrase | Read |
|--------|------|
| "add verb", "create verb", "verb YAML" | `docs/verb-definition-spec.md` |
| "egui", "viewport", "immediate mode" | `docs/strategy-patterns.md` §3 |
| "entity model", "CBU", "UBO", "holdings" | `docs/strategy-patterns.md` §1 |
| "agent", "MCP", "verb_search" | `docs/agent-architecture.md` |
| "session", "scope", "navigation" | `docs/session-visualization-architecture.md` |
| "ESPER", "drill", "trace", "xray" | `docs/session-visualization-architecture.md` |
| "investor register", "look-through" | `ai-thoughts/018-investor-register-visualization.md` |
| "GROUP", "ownership graph" | `ai-thoughts/019-group-taxonomy-intra-company-ownership.md` |
| "sheet", "phased execution", "DAG" | `ai-thoughts/035-repl-session-implementation-plan.md` |
| "solar navigation", "ViewState", "orbit", "nav_history" | `ai-thoughts/038-solar-navigation-unified-design.md` |
| "intent pipeline", "ambiguity", "normalize_candidates", "ref_id" | CLAUDE.md §Intent Pipeline Fixes (042) |
| "macro", "operator vocabulary", "structure.setup", "constraint cascade" | CLAUDE.md §Operator Vocabulary & Macros (058) |
| "onboarding pipeline", "RequirementPlanner", "OnboardingPlan", "ob-agentic" | CLAUDE.md §Structured Onboarding Pipeline |
| "LSP", "language server", "completion", "diagnostics", "dsl-lsp" | CLAUDE.md §DSL Language Server |

---

## Operator Vocabulary & Macros (058)

> ✅ **IMPLEMENTED (2026-01-28)**: Operator vocabulary layer, macro expansion, constraint cascade, DAG navigation.

**Problem Solved:** Implementation jargon (CBU, entity_ref, trading-profile) leaked to operators. Intent matching at 80%. Now operators see business terms (structure, party, mandate) and matching is 95%+.

### Architecture

```
Operator: "Set up a PE structure for Allianz"
    │
    ├─► Macro: structure.setup :type pe :name "..."
    │       │
    │       └─► Expands to: (cbu.create :kind private-equity :name "...")
    │
    └─► Constraint Cascade:
            client: Allianz → structure_type: PE → scope entity search
```

### Operator Domains (Macro Namespaces)

| Operator Domain | Wraps | Purpose |
|-----------------|-------|---------|
| `structure.*` | `cbu.*` | Fund structure operations |
| `party.*` | `entity.*` + `cbu-role.*` | People/orgs in roles |
| `case.*` | `kyc-case.*` | KYC case management |
| `mandate.*` | `trading-profile.*` | Investment mandates |

### Display Nouns (UI Never Shows Internal IDs)

| Internal | UI Shows |
|----------|----------|
| `cbu` | Structure / Client Unit |
| `entity` | Party |
| `trading-profile` | Mandate |
| `kyc-case` | Case |

### Macro Schema Example

> **IMPORTANT:** All macro YAML uses **kebab-case** for field names (`target-label`, `mode-tags`, `expands-to`).

```yaml
structure.setup:
  kind: macro
  id: structure.setup                    # Explicit ID (optional, defaults to YAML key)
  tier: composite                        # primitive | composite | template
  aliases: [setup-structure, new-fund]   # Alternative names
  taxonomy:
    domain: structure
    category: fund-setup
  docs-bundle: docs.bundle.ucits-baseline
  required-roles:
    - role: depositary
      cardinality: one
      entity-kinds: [bank, custodian]
  optional-roles:
    - role: prime-broker
      cardinality: zero-or-more
  ui:
    label: "Set up Structure"
    description: "Create a structure in the current client scope"
    target-label: "Structure"
  target:
    operates-on: client-ref
    produces: structure-ref
  args:
    required:
      structure-type:
        type: enum
        ui-label: "Type"
        values:
          - key: pe
            label: "Private Equity"
            internal: private-equity   # CRITICAL: UI key ≠ internal token
          - key: sicav
            label: "SICAV"
            internal: sicav
        default-key: pe
      depositary:
        type: party-ref
        ui-label: "Depositary"
        required-if: "structure-type = sicav"  # Conditional requirement
        placeholder-if-missing: true           # Auto-create placeholder entity
  expands-to:
    - verb: cbu.create
      args:
        kind: "${arg.structure-type.internal}"  # Uses internal token
        name: "${arg.name}"
```

### Advanced Macro Features (064)

**Conditional Expansion (`when:`):**
```yaml
expands-to:
  - when: "jurisdiction = LU"
    then:
      - verb: cbu.create
        args: { kind: "sicav", jurisdiction: "LU" }
    else:
      - verb: cbu.create
        args: { kind: "ucits" }
```

**Iteration (`foreach:`):**
```yaml
expands-to:
  - foreach: role
    in: "${required-roles}"
    do:
      - verb: party.assign
        args:
          structure-id: "${@structure}"
          role: "${role.role}"
          party-ref: "${role.party}"
```

**Condition Operators:**
- `var = value` - equality
- `var != value` - inequality
- `var in [a, b, c]` - membership
- `not: <condition>` - negation
- `any-of: [<cond1>, <cond2>]` - OR
- `all-of: [<cond1>, <cond2>]` - AND

**Conditional Requirements (`required-if:`):**
```yaml
depositary:
  type: party-ref
  required-if: "structure-type = sicav"  # Required only for SICAV
  
prime-broker:
  type: party-ref
  required-if:
    any-of:
      - "structure-type = hedge"
      - "has-prime-broker = true"
```

**Placeholder Entities (`placeholder-if-missing: true`):**
When arg is missing but `placeholder-if-missing: true`, expander generates:
```lisp
(entity.ensure-or-placeholder :kind "depositary" :ref "depositary-placeholder")
```

**Key Files:**
| File | Purpose |
|------|---------|
| `rust/src/dsl_v2/macros/schema.rs` | Schema types (`MacroSchema`, `WhenStep`, `ForEachStep`) |
| `rust/src/dsl_v2/macros/conditions.rs` | Condition evaluation for `when:` |
| `rust/src/dsl_v2/macros/expander.rs` | Macro expansion with When/ForEach/placeholder |
| `config/verb_schemas/macros/*.yaml` | Macro definitions (kebab-case) |

### Macro Lint (`cargo x verbs lint-macros`)

| Rule | Severity | What |
|------|----------|------|
| `MACRO011` | Error | UI fields required (label, description, target_label) |
| `MACRO012` | Error | Forbidden tokens in UI (cbu, entity_ref, etc.) |
| `MACRO042` | Error | No raw `entity_ref` - use `structure_ref`, `party_ref` |
| `MACRO043` | Error | `kinds:` only under `internal:` block |
| `MACRO044` | Error | Enums must use `${arg.X.internal}` in expansion |

### Constraint Cascade

```
1. CLIENT → Entities: 10,000 → 500
2. STRUCTURE TYPE → Structures: 500 → 50
3. VERB SCHEMA → Entity kinds for args
4. ENTITY → Search ONLY within scope
```

Session fields: `client`, `structure_type`, `current_structure`, `current_case`

### DAG Navigation

```
● structure.setup           ← START (ready)
  ├─► ○ structure.assign-role :role gp    (needs: setup)
  ├─► ○ structure.assign-role :role im    (needs: setup)
  └─► ○ case.open                         (needs: setup)
        └─► ○ case.approve                (needs: submit)

● = ready   ○ = blocked   ✓ = done
```

Prereq conditions: `VerbCompleted`, `AnyOf`, `StateExists`, `FactExists`

### Role Validation (Server-Side)

```yaml
role:
  type: enum
  values:
    - key: gp
      internal: general-partner
      valid-for: [pe, hedge]  # GP fails on SICAV
    - key: manco
      internal: manco
      valid-for: [sicav]      # ManCo fails on PE
```

### Key Files

| File | Purpose |
|------|---------|
| `rust/src/lint/macro_lint.rs` | MACRO000-MACRO080 lint rules |
| `rust/src/dsl_v2/macros/` | Macro registry, expander, schemas |
| `rust/src/session/unified.rs` | `UnifiedSession` with cascade context |
| `rust/src/mcp/macro_integration.rs` | DAG readiness, prereq checking |
| `config/verb_schemas/macros/` | Macro YAML definitions |

---

## Deprecated / Removed

| Removed | Replaced By |
|---------|-------------|
| `ViewMode` enum (5 modes) | Unit struct (always TRADING) |
| `OpenAIEmbedder` | `CandleEmbedder` (local) |
| `all-MiniLM-L6-v2` model | `bge-small-en-v1.5` (retrieval-optimized) |
| `embed()` / `embed_batch()` | `embed_query()` / `embed_target()` (asymmetric) |
| `IntentExtractor` (old MCP path) | MCP `verb_search` + `dsl_generate` |
| `AgentOrchestrator` | MCP pipeline |
| `verb_rag_metadata.rs` | YAML `invocation_phrases` + pgvector |
| `FeedbackLoop.generate_valid_dsl()` | MCP `dsl_generate` |
| `VerbPhraseIndex` (YAML phrase matcher) | `VerbService` + pgvector semantic search |
| Direct sqlx in `HybridVerbSearcher` | `VerbService` centralized DB access |

---

## Domain Quick Reference

| Domain | Verbs | Purpose |
|--------|-------|---------|
| `cbu` | 25 | Client Business Unit lifecycle |
| `entity` | 30 | Natural/legal person management |
| `session` | 16 | Scope, navigation, history |
| `view` | 15 | ESPER navigation, filters |
| `trading-profile` | 30 | Trading matrix, CA policy |
| `kyc` | 20 | KYC case management |
| `investor` | 15 | Investor register, holdings |
| `custody` | 40 | Settlement, safekeeping |
| `gleif` | 15 | LEI lookup, hierarchy import |
| `research.*` | 30+ | External source workflows |
| `contract` | 14 | Legal contracts, rate cards, CBU subscriptions |
| `document` | 7 | Document solicitation, verification, rejection |
| `requirement` | 5 | Document requirements, waiver, status |
