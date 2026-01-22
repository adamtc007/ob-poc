# CLAUDE.md

> **Last reviewed:** 2026-01-22
> **Crates:** 14 Rust crates
> **Verbs:** 938 verbs, 7414 intent patterns (DB-sourced)
> **Migrations:** 44 schema migrations
> **Embeddings:** Candle local (384-dim, BGE-small-en-v1.5) - 7414 patterns vectorized
> **ESPER Navigation:** ✅ Complete - 48 commands, trie + semantic fallback
> **Multi-CBU Viewport:** ✅ Complete - Scope graph endpoint, execution refresh
> **REPL Session/Phased Execution:** ✅ Complete - See `ai-thoughts/035-repl-session-implementation-plan.md`
> **Candle Semantic Pipeline:** ✅ Complete - DB source of truth, populate_embeddings binary
> **Agent Pipeline:** ✅ Unified - One path, all input → LLM intent → DSL (no special cases)
> **Solar Navigation (038):** ✅ Complete - ViewState, NavigationHistory, orbit navigation
> **Nav UX Messages:** ✅ Complete - NavReason codes, NavSuggestion, standardized error copy
> **Promotion Pipeline (043):** ✅ Complete - Quality-gated pattern promotion with collision detection
> **Teaching Mechanism (044):** ✅ Complete - Direct phrase→verb mapping for trusted sources

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
| Candle embeddings | `docs/TODO-CANDLE-MIGRATION.md` | ✅ Complete |
| Candle pipeline | `docs/TODO-CANDLE-PIPELINE-CONSOLIDATION.md` | ✅ Complete |
| Intent pipeline fixes | `ai-thoughts/Intent-Pipeline-Fixes-todo.md` | ✅ Complete |
| Promotion pipeline | `TODO-feedback-loop-promotion.md` | ✅ Complete |

### Active TODOs

| Topic | Document | Status |
|-------|----------|--------|
| **ESPER Navigation** | `TODO-ESPER-NAVIGATION-FULL.md` | ✅ Complete - Trie + semantic fallback |

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
| **H** | Hardcoded 0.5 threshold | Added `fallback_threshold` (0.65), `semantic_threshold` (0.80) |

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

## ESPER Navigation System

Blade Runner-inspired voice/chat navigation with trie-based instant lookup and semantic fallback.

### Architecture

```
User: "make it bigger"
    │
    ├─ Trie Lookup (O(k)) ──► HIT → Execute instantly (<1μs)
    │       │
    │       └─ MISS + Semantic Ready?
    │               │
    │               └─ embed_blocking() ~10ms
    │                       │
    │                       └─ SemanticIndex.search()
    │                               │
    │                               ├─ confidence ≥ 0.80 → Auto-execute + Learn
    │                               ├─ 0.50-0.80 → Disambiguation UI
    │                               └─ < 0.50 → Fall through to DSL
    │
    ▼
AgentCommand::ZoomIn { factor: Some(2.0) }
```

### Key Files

| File | Purpose |
|------|---------|
| `rust/config/esper-commands.yaml` | 48 command definitions with aliases |
| `rust/src/agent/esper/registry.rs` | Trie + semantic index + lookup |
| `rust/src/agent/esper/warmup.rs` | Pre-compute embeddings at startup |
| `rust/src/agent/esper/config.rs` | YAML schema types |
| `rust/src/api/agent_service.rs` | `handle_esper_command()` integration |

### Command Categories

| Category | Commands | Examples |
|----------|----------|----------|
| **Zoom** | `zoom_in`, `zoom_out`, `zoom_fit` | "enhance", "zoom in 2x", "fit to screen" |
| **Scale Navigation** | `scale_universe` → `scale_core` | "universe", "galaxy", "land on", "core sample" |
| **Depth Navigation** | `drill_through`, `surface_return`, `xray`, `peel` | "drill through", "x-ray", "peel back" |
| **Temporal** | `time_rewind`, `time_play`, `time_slice` | "rewind to 2023", "show changes" |
| **Investigation** | `follow_the_money`, `who_controls`, `red_flag_scan` | "follow the money", "red flag scan" |
| **Context** | `context_review`, `context_investigation` | "board review", "investigation mode" |

### Semantic Fallback (Phase 8)

On trie miss, falls back to BGE embeddings (query→target retrieval):

| Threshold | Action |
|-----------|--------|
| ≥ 0.88 | Auto-execute + persist learned alias |
| 0.50-0.88 | Show disambiguation UI |
| < 0.50 | No match, fall through to DSL pipeline |

**Verb Search Thresholds (BGE-calibrated):**

| Threshold | Value | Purpose |
|-----------|-------|---------|
| `semantic_threshold` | 0.88 | High-confidence auto-match |
| `fallback_threshold` | 0.78 | Lower bar for suggestions |
| `blocklist_threshold` | 0.85 | Collision detection |

### Adding New Phrases

Edit `rust/config/esper-commands.yaml`:

```yaml
zoom_in:
  canonical: "enhance"
  response: "Enhancing..."
  agent_command:
    type: ZoomIn
    params: { factor: extract }
  aliases:
    exact: ["enhance", "closer"]      # Must match exactly
    contains: ["make it bigger"]      # Substring match
    prefix: ["zoom in "]              # Prefix + extract rest
```

No Rust code changes needed for new phrases.

### MCP Tools

| Tool | Description |
|------|-------------|
| `esper_list` | List all 48 commands with aliases |
| `esper_lookup` | Test what command a phrase maps to |
| `esper_add_alias` | Add learned alias (persists to DB) |
| `esper_reload` | Reload config without restart |

### Trigger Phrases (for Claude)

When you see these in a task, you're working on ESPER:
- "esper", "navigation command", "voice command"
- "enhance", "zoom", "drill", "surface", "xray"
- "semantic fallback", "trie lookup", "learned alias"
- "AgentCommand", "EsperCommandRegistry"

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
│   │   ├── dsl-lsp/            # Language Server for DSL
│   │   ├── ob-agentic/         # LLM agent
│   │   ├── ob-poc-ui/          # egui/WASM UI
│   │   └── ob-poc-graph/       # Graph visualization
│   └── src/
│       ├── dsl_v2/             # DSL execution
│       │   ├── custom_ops/     # Plugin handlers
│       │   └── generic_executor.rs
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
| "intent pipeline", "ambiguity", "normalize_candidates", "ref_id" | `intent-pipeline-fixes-todo.md` |

---

## Deprecated / Removed

| Removed | Replaced By |
|---------|-------------|
| `ViewMode` enum (5 modes) | Unit struct (always TRADING) |
| `OpenAIEmbedder` | `CandleEmbedder` (local) |
| `all-MiniLM-L6-v2` model | `bge-small-en-v1.5` (retrieval-optimized) |
| `embed()` / `embed_batch()` | `embed_query()` / `embed_target()` (asymmetric) |
| `IntentExtractor` | MCP `verb_search` + `dsl_generate` |
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
