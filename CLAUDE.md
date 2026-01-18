# CLAUDE.md

> **Last reviewed:** 2026-01-18
> **Crates:** 14 Rust crates
> **Verbs:** 800+ across 103 YAML files
> **Migrations:** 34 schema migrations
> **Embeddings:** Candle local (384-dim, all-MiniLM-L6-v2)

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

---

## DSL Pipeline (Single Path)

**ALL DSL generation goes through this pipeline. No bypass paths.**

```
User says: "spin up a fund for Acme"
                    ↓
            verb_search tool
                    ↓
    ┌───────────────┴───────────────┐
    │     Search Priority (7-tier)   │
    │  1. User learned (exact)       │
    │  2. Global learned (exact)     │
    │  3. User semantic (pgvector)   │
    │  4. Global semantic (pgvector) │
    │  5. Blocklist check            │
    │  6. YAML invocation_phrases    │
    │  7. Cold start semantic        │
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

### Embeddings: Candle Local (Complete)

| Component | Value |
|-----------|-------|
| Framework | HuggingFace Candle (pure Rust) |
| Model | all-MiniLM-L6-v2 |
| Dimensions | 384 |
| Latency | 5-15ms |
| Storage | pgvector (IVFFlat) |
| API Key | Not required |

**No external API calls.** Embeddings are computed locally.

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

## Adding Verbs

> ⚠️ **Before writing verb YAML, read `docs/verb-definition-spec.md`**
> Serde structs are strict. Invalid YAML silently fails to load.

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
        invocation_phrases:        # Required for discovery
          - "create my thing"
          - "add new record"
        args:
          - name: name
            type: string
            required: true
            maps_to: name
```

### Verify

```bash
cargo x verbs check   # YAML matches DB
cargo x verbs lint    # Tiering rules
```

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

---

## Deprecated / Removed

| Removed | Replaced By |
|---------|-------------|
| `ViewMode` enum (5 modes) | Unit struct (always TRADING) |
| `OpenAIEmbedder` | `CandleEmbedder` (local) |
| `IntentExtractor` | MCP `verb_search` + `dsl_generate` |
| `AgentOrchestrator` | MCP pipeline |
| `verb_rag_metadata.rs` | YAML `invocation_phrases` + pgvector |
| `FeedbackLoop.generate_valid_dsl()` | MCP `dsl_generate` |

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
