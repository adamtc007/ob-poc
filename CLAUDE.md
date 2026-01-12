# CLAUDE.md

> **Last reviewed:** 2026-01-11
> **Verb count:** ~816 verbs across 105+ YAML files
> **Custom ops:** 55+ plugin handlers
> **Crates:** 13 fine-grained crates
> **Migrations:** 20 schema migrations (latest: 020_trading_profile_materialization.sql)
> **Feedback System:** ✅ Complete - Event capture + inspector + MCP tools
> **Session/View:** ✅ Implemented - Scopes, filters, ESPER verbs, history
> **Verb Tiering:** ⚠️ Partial - Types + linter done, only 5/40 verb files tagged

This file provides guidance to Claude Code when working with this repository.

---

## Deep Dive Documentation

**CLAUDE.md is the quick reference. Detailed docs are in /docs and /ai-thoughts.**

### ⚠️ MANDATORY READING (Claude MUST read these before certain tasks)

| Task | MUST read first | Why |
|------|-----------------|-----|
| Creating/modifying verb YAML | `docs/verb-definition-spec.md` | Serde structs are strict, errors are silent |
| Working on egui/viewport | `docs/strategy-patterns.md` §3 | Immediate mode patterns are non-obvious |
| Understanding CBU/UBO/Entity | `docs/strategy-patterns.md` §1 | Data model is unconventional |
| Agent/MCP integration | `docs/strategy-patterns.md` §2 | LLM→DSL pattern is specific |
| Capital structure/ownership | `ai-thoughts/016-capital-structure-ownership-model.md` | Multi-class cap table design |
| Complex capital verbs (split/exercise) | `ai-thoughts/017-transactional-safety-complex-capital-verbs.md` | Transaction safety patterns |
| Investor register visualization | `ai-thoughts/018-investor-register-visualization.md` | Dual-mode display, institutional look-through |
| **GROUP/UBO ownership model** | `ai-thoughts/019-group-taxonomy-intra-company-ownership.md` | ✅ **DONE** - UBO computed not stored, coverage model |
| **Research workflows & agent** | `ai-thoughts/020-research-workflows-external-sources.md` | ✅ **DONE** - Bounded non-determinism, orchestration |
| **Source loaders (GLEIF/CH/SEC)** | `ai-thoughts/021-pluggable-research-source-loaders.md` | ✅ **DONE** - SourceLoader trait, 3 loaders, 15 handlers |
| **Luxembourg loader** | `ai-thoughts/022-luxembourg-source-loader.md` | PARKED - CSSF (testable), RCS/RBE (stub pattern) |
| **Event Infrastructure (design)** | `ai-thoughts/023a-event-infrastructure.md` | ✅ **DONE** - Always-on, zero-overhead event capture from DSL pipeline + session logging |
| **Feedback Inspector (design)** | `ai-thoughts/023b-feedback-inspector.md` | ✅ **DONE** - On-demand failure analysis, classification, repro generation, audit trail, MCP interface |
| **Event Infrastructure (impl)** | `ai-thoughts/025-implement-event-infrastructure.md` | ✅ **DONE** - Lock-free emitter, drain task, session logger |
| **Feedback Inspector (impl)** | `ai-thoughts/026-implement-feedback-inspector.md` | ✅ **DONE** - Classifier, redactor, repro gen, audit trail, 6 MCP tools, REPL commands |
| **Trading Matrix Pivot** | `ai-thoughts/027-trading-matrix-canonical-pivot.md` | ⚠️ **IN PROGRESS** - Types + linter done, 35/40 verb files need tagging |
| **Research/agent quick reference** | `docs/research-agent-annex.md` | Invocation phrases, confidence thresholds, agent loop |

> **DEPRECATED:** `TODO-semantic-intent-matching.md` - replaced by 023 unified learning system

**How to read:** Use `view docs/filename.md` or `view ai-thoughts/filename.md` before starting the task.

### Reference Documentation (read as needed)

| When working on... | Read this file | Contains |
|--------------------|----------------|----------|
| **Understanding WHY things work this way** | `docs/strategy-patterns.md` | Data philosophy, CBU/UBO/Trading concepts, Agent strategy, egui do's/don'ts |
| **Session & visualization architecture** | `docs/session-visualization-architecture.md` | Session scopes, view verbs, ESPER navigation, filters, history, active CBU set |
| **Creating or modifying verb YAML** | `docs/verb-definition-spec.md` | **CRITICAL** - exact YAML structure, valid field values, common errors |
| **Entity model, schemas, relationships** | `docs/entity-model-ascii.md` | Full ERD, table relationships, identifier schemes, UBO flow, dual-use holdings |
| **DSL parser, compiler, executor** | `docs/dsl-verb-flow.md` | Pipeline stages, verb resolution, YAML structure, capture/interpolation, plugin handlers |
| **Agent pipeline, LLM integration** | `docs/agent-architecture.md` | Lexicon tokenizer, intent parsing, research macros, conductor mode, voice |
| **UI, graph viz, REPL commands** | `docs/repl-viewport.md` | 5-panel layout, shared state, graph interactions, taxonomy navigator, galaxy nav |
| **Research workflows, agent mode** | `docs/research-agent-annex.md` | Invocation phrases, confidence routing, checkpoint UI, pluggable sources |
| **Architecture & economics** | `docs/architecture/intent-driven-onboarding.md` | DSL-as-state philosophy, refactoring economics, Rust vs Java TCO, LLM productivity |

**START HERE for non-obvious concepts:**
- "Why is everything an Entity?" → `docs/strategy-patterns.md` §1
- "What's the difference between CBU and Entity?" → `docs/strategy-patterns.md` §1
- "How does UBO discovery work?" → `docs/strategy-patterns.md` §1, `ai-thoughts/019-*`
- "Why DSL instead of direct SQL?" → `docs/strategy-patterns.md` §2
- "What are Research Macros?" → `docs/strategy-patterns.md` §2, `ai-thoughts/020-*`
- "egui patterns and gotchas" → `docs/strategy-patterns.md` §3
- "Verb YAML not loading?" → `docs/verb-definition-spec.md` §5 (Common Errors)
- "How does the agent work?" → `docs/research-agent-annex.md`, `ai-thoughts/020-*`
- "UBO computed vs stored?" → `ai-thoughts/019-*`
- "What are invocation phrases?" → `docs/research-agent-annex.md`

**Trigger phrases (if you see these in a task, read the doc first):**
- "add verb", "new verb", "create verb", "verb YAML" → `docs/verb-definition-spec.md`
- "egui", "viewport", "immediate mode", "graph widget" → `docs/strategy-patterns.md` §3
- "entity model", "CBU", "UBO", "holdings" → `docs/strategy-patterns.md` §1
- "agent", "MCP", "research macro" → `docs/research-agent-annex.md`, `ai-thoughts/020-*`
- "investor register", "cap table", "shareholder", "control holder" → `ai-thoughts/018-*`
- "institutional holder", "UBO chain", "look-through" → `ai-thoughts/018-*`, `ai-thoughts/019-*`
- "GROUP", "ownership graph", "coverage", "gaps" → `ai-thoughts/019-*`
- "research", "GLEIF", "Companies House", "external source" → `docs/research-agent-annex.md`, `ai-thoughts/021-*`
- "checkpoint", "confidence", "disambiguation" → `docs/research-agent-annex.md`
- "agent mode", "resolve gaps", "chain research" → `docs/research-agent-annex.md`
- "invocation phrases", "agent triggers" → `docs/research-agent-annex.md`
- "SourceLoader", "source registry", "API client" → `ai-thoughts/021-*`
- "SEC EDGAR", "13D", "13G", "CIK" → `ai-thoughts/021-*`
- "PSC", "beneficial owner", "control holder" → `ai-thoughts/021-*`
- "session scope", "galaxy", "book", "navigation history" → `docs/session-visualization-architecture.md`
- "view verb", "esper", "drill", "surface", "trace" → `docs/session-visualization-architecture.md`
- "active CBU set", "multi-CBU selection" → `docs/session-visualization-architecture.md`
- "zoom animation", "astro", "landing", "taxonomy stack" → `docs/session-visualization-architecture.md`
- "refactor", "rename verb", "delete verb", "deprecate", "cleanup" → `ai-thoughts/027-*`, `docs/architecture/intent-driven-onboarding.md`
- "trading matrix", "instrument taxonomy", "materialize", "canonical" → `ai-thoughts/027-*`

**Working documents (TODOs, plans):**
- `ai-thoughts/015-consolidate-dsl-execution-path.md` - Unify DSL execution to single session-aware path
- `ai-thoughts/016-capital-structure-ownership-model.md` - Multi-class cap table, voting/economic rights, dilution
- `ai-thoughts/017-transactional-safety-complex-capital-verbs.md` - SERIALIZABLE + advisory locks for splits/exercises
- `ai-thoughts/018-investor-register-visualization.md` - Dual-mode visualization, threshold collapse, institutional look-through
- `ai-thoughts/019-group-taxonomy-intra-company-ownership.md` - ✅ DONE - GROUP taxonomy, UBO computation, coverage model
- `ai-thoughts/020-research-workflows-external-sources.md` - ✅ DONE - Research agent, bounded non-determinism, orchestration
- `ai-thoughts/021-pluggable-research-source-loaders.md` - ✅ DONE - SourceLoader trait, GLEIF/CH/SEC loaders, 15 handlers
- `ai-thoughts/022-luxembourg-source-loader.md` - PARKED - Luxembourg CSSF/RCS/RBE, stub pattern for subscription sources
- `ai-thoughts/023a-event-infrastructure.md` - ✅ DONE - Always-on event capture, zero DSL impact, session logging
- `ai-thoughts/023b-feedback-inspector.md` - ✅ DONE - On-demand analysis, repro generation, audit trail, MCP server
- `ai-thoughts/025-implement-event-infrastructure.md` - ✅ DONE - Lock-free emitter, drain task, session logger
- `ai-thoughts/026-implement-feedback-inspector.md` - ✅ DONE - Classifier, redactor, repro gen, audit trail, 6 MCP tools
- `ai-thoughts/027-trading-matrix-canonical-pivot.md` - ⚠️ **IN PROGRESS** - Types + linter done, 35/40 verb files need tier metadata

---

## Project Overview

**OB-POC** is a KYC/AML onboarding system using a declarative DSL. The DSL is the single source of truth for onboarding workflows.

```
User/Agent → DSL Source → Parser → Compiler → Executor → PostgreSQL
                                      ↓
                              YAML verb definitions
```

**Key insight:** LLM does DISCOVERY (what to do), DSL does EXECUTION (how to do it deterministically).

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                   Web UI (localhost:3000)                       │
│  ob-poc-ui (egui/WASM) + ob-poc-web (Axum)                     │
│  5-panel layout: Context | Chat | DSL | Graph | Results         │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                   DSL Pipeline (dsl-core crate)                 │
│  Parser (Nom) → Compiler → Executor → Database                  │
│  YAML verbs define operations - no Rust code for standard CRUD  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                  PostgreSQL 17 (data_designer)                  │
│  Schemas: ob-poc, kyc, custody, instruments, teams              │
│  Extensions: uuid-ossp, pg_trgm, pgvector                       │
└─────────────────────────────────────────────────────────────────┘
```

---

## Directory Structure

```
ob-poc/
├── rust/
│   ├── config/verbs/           # Verb YAML definitions (105+ files, ~820 verbs)
│   │   ├── cbu.yaml            # CBU domain
│   │   ├── entity.yaml         # Entity domain
│   │   ├── custody/            # Custody subdomain
│   │   ├── kyc/                # KYC subdomain
│   │   ├── registry/           # Investor registry
│   │   ├── research/           # Research workflows (NEW)
│   │   └── agent/              # Agent mode verbs (NEW)
│   ├── crates/
│   │   ├── dsl-core/           # Parser, AST, compiler (NO DB dependency)
│   │   ├── ob-agentic/         # LLM agent for DSL generation
│   │   ├── ob-poc-web/         # Axum server + API
│   │   ├── ob-poc-ui/          # egui/WASM UI
│   │   └── ob-poc-graph/       # Graph visualization widget
│   ├── src/
│   │   ├── dsl_v2/             # DSL execution layer
│   │   │   ├── generic_executor.rs  # YAML-driven CRUD executor
│   │   │   ├── custom_ops/     # Plugin handlers (~55 files)
│   │   │   └── verb_registry.rs
│   │   ├── research/           # Research module (NEW)
│   │   ├── agent/              # Agent controller (NEW)
│   │   ├── api/                # REST API routes
│   │   └── bin/
│   │       ├── dsl_api.rs      # Main Axum server
│   │       └── dsl_cli.rs      # CLI tool
│   └── xtask/                  # Build automation
├── prompts/                    # LLM prompt templates (NEW)
│   └── research/
├── migrations/                 # SQLx migrations (16 files)
├── docs/                       # Architecture documentation
├── ai-thoughts/                # ADRs and working docs
└── CLAUDE.md                   # This file
```

### Crate Architecture

| Crate | DB Required | Purpose |
|-------|-------------|---------|
| `dsl-core` | No | Pure parser, AST, compiler - works offline |
| `ob-agentic` | No | LLM intent extraction |
| `ob-poc-ui` | No | Pure egui/WASM UI - fetches data via HTTP |
| `ob-poc-web` | Yes | Axum server handles all DB operations |
| `entity-gateway` | Yes | gRPC entity resolution with Tantivy indexes |

---

## Commands

### Development Workflow (xtask)

```bash
cd rust/

# Pre-commit (fast)
cargo x pre-commit          # Format + clippy + unit tests

# Full check
cargo x check               # Compile + clippy + tests
cargo x check --db          # Include database integration tests

# Build
cargo x build               # Debug build
cargo x build --release     # Release build
cargo x wasm                # Build WASM components

# Deploy (recommended for UI development)
cargo x deploy              # Full: WASM + server + start
cargo x deploy --skip-wasm  # Skip WASM rebuild

# Utilities
cargo x verify-verbs        # Check all verb YAML files parse correctly
cargo x schema-export       # Export DB schema
cargo x dsl-tests           # Run DSL test scenarios
```

### Direct Cargo Commands

```bash
# Run web server
DATABASE_URL="postgresql:///data_designer" cargo run -p ob-poc-web

# Test
cargo test --features database --lib                  # Unit tests
cargo test --features database --test db_integration  # DB tests

# Clippy
cargo clippy --features server
cargo clippy --features database
```

### Tracing / Debug Logging

```bash
# Debug level - shows step execution, verb routing, SQL
RUST_LOG=ob_poc::dsl_v2=debug ./target/debug/dsl_cli execute -f file.dsl

# Trace level - includes SQL bind values (very verbose)
RUST_LOG=ob_poc::dsl_v2=trace ./target/debug/dsl_cli execute -f file.dsl

# Agent/research debugging
RUST_LOG=ob_poc::agent=debug,ob_poc::research=debug ./target/debug/dsl_cli
```

---

## Adding New Verbs

> **⚠️ STOP: Before writing ANY verb YAML, you MUST:**
> 1. Run `view docs/verb-definition-spec.md` and read it
> 2. Understand the exact struct fields and valid enum values
> 3. Errors are SILENT - invalid YAML causes verbs to not load with no error message
>
> **This is not optional.** The Rust serde structs are strict. Field names, enum values,
> and nesting must be exact. Past failures occurred because Claude guessed at structure.

### Quick Example (CRUD)

```yaml
# rust/config/verbs/my_domain.yaml
domains:
  my_domain:
    description: "My domain operations"
    verbs:
      create:
        description: "Create a new record"
        behavior: crud                    # MUST be 'crud' or 'plugin'
        crud:
          operation: insert               # insert|upsert|update|delete|select
          table: my_table
          schema: ob-poc
          returning: id
        args:
          - name: name                    # kebab-case in DSL
            type: string                  # string|uuid|integer|decimal|boolean|date
            required: true
            maps_to: name                 # snake_case SQL column
        returns:
          type: uuid
          capture: true
```

**No Rust code changes required for standard CRUD operations.**

### Plugin Handler (Custom Logic)

```yaml
my-complex-operation:
  description: "Does something complex"
  behavior: plugin
  handler: MyComplexOperationOp    # Must match Rust struct name
  args:
    - name: entity-id
      type: uuid
      required: true
      lookup:                      # Enables name→UUID resolution
        table: entities
        schema: ob-poc
        search_key: name
        primary_key: entity_id
```

Then implement in `rust/src/dsl_v2/custom_ops/`:

```rust
pub struct MyComplexOperationOp;

#[async_trait]
impl CustomOp for MyComplexOperationOp {
    async fn execute(&self, ctx: &OpContext, args: &OpArgs) -> Result<OpResult> {
        // Custom logic here
    }
}
```

### Adding Invocation Phrases (for Agent)

For verbs that should be triggered by natural language, see `docs/research-agent-annex.md`.

```yaml
my-domain:
  invocation_hints:           # Domain-level hints
    - "my domain"
    
  verbs:
    my-verb:
      invocation_phrases:     # Verb-level phrases
        - "do the thing"
        - "perform my action"
```

### Verify Verbs Load

```bash
cargo x verify-verbs   # Shows parse errors for all YAML files
cargo x verbs lint     # Check tiering rule violations
```

### Verb Tiering System

Verbs are categorized by their role in the data flow:

| Tier | Purpose | Example |
|------|---------|---------|
| `reference` | Global reference data (catalogs) | `corporate-action:define-event-type` |
| `intent` | User-facing authoring operations | `trading-profile:add-market` |
| `projection` | Internal writes to operational tables | `cbu-custody:add-universe` (deprecated) |
| `diagnostics` | Read-only queries and validation | `cbu-custody:list-ssis` |
| `composite` | Multi-table orchestration | `trading-profile:materialize` |

**Source of Truth (domain-specific):**
- `matrix` - Trading profile JSONB document
- `entity` - Entity graph (entity_relationships table)
- `workflow` - Case/KYC state machine
- `external` - External APIs (GLEIF, Companies House, SEC)
- `register` - Capital structure (fund/investor holdings)
- `catalog` - Reference data (seeded lookup tables)
- `session` - Ephemeral UI state
- `document` - Document catalog
- `operational` - Derived/projected tables

**Verb Metadata Example:**
```yaml
my-verb:
  description: "Do something"
  behavior: crud
  metadata:
    tier: intent                    # reference|intent|projection|diagnostics|composite
    source_of_truth: matrix         # matrix|catalog|operational|session
    scope: cbu                      # global|cbu
    writes_operational: false       # true if writes to operational tables
    internal: false                 # true if not for direct user invocation
    noun: trading_profile           # domain object being operated on
    tags: [authoring, draft]        # free-form tags
  # ... rest of verb definition
```

**Linting Rules (universal, domain-agnostic):**
- T007: All verbs should have metadata (warning)
- T002: Projection verbs must be `internal: true`
- T006: Diagnostics verbs must be read-only

---

## Database Development Practices

### ⛔ MANDATORY: SQLx Compile-Time Verification

When making ANY database schema changes:

```bash
# 1. Apply migration
psql -d data_designer -f your_migration.sql

# 2. Regenerate SQLx offline data
cd rust
cargo sqlx prepare --workspace

# 3. Build - catches type mismatches at compile time
cargo build
```

**Why:** SQLx performs compile-time verification against the actual PostgreSQL schema. Type mismatches that would pass in Hibernate/mocked tests are caught here.

### Type Mapping

| PostgreSQL | Rust | Notes |
|------------|------|-------|
| `UUID` | `Uuid` | Not `String` |
| `TIMESTAMPTZ` | `DateTime<Utc>` | Not `NaiveDateTime` |
| `INTEGER` | `i32` | Not `i64` |
| `BIGINT` | `i64` | |
| `NUMERIC` | `BigDecimal` | Not `f64` for money |
| `NULLABLE` | `Option<T>` | Missing = runtime panic |

### Schema Change Checklist

- [ ] Migration SQL written and reviewed
- [ ] Migration applied to local database
- [ ] `cargo sqlx prepare --workspace` run
- [ ] `cargo build` passes (no type mismatches)
- [ ] Relevant Rust structs updated if needed

---

## Error Handling Guidelines

**Never use `.unwrap()` or `.expect()` in production code paths** - these cause server panics.

### Panic-Free Patterns

| Pattern | Use Case |
|---------|----------|
| `?` operator | Propagate errors up the call stack |
| `.ok_or_else(\|\| anyhow!(...))` | Convert Option to Result with context |
| `let Some(x) = ... else { continue }` | Skip missing items in loops |
| `match` / `if let` | Explicit handling of all cases |

### Acceptable `.unwrap()` Locations

- Test code (`#[test]`, `#[cfg(test)]`)
- Static constants with `.expect("static value")`
- After explicit check (prefer `let Some()`)

---

## Environment Variables

```bash
# Required
DATABASE_URL="postgresql:///data_designer"

# LLM Backend
AGENT_BACKEND=anthropic          # or "openai"
ANTHROPIC_API_KEY="sk-ant-..."
ANTHROPIC_MODEL="claude-sonnet-4-20250514"

# Optional
DSL_CONFIG_DIR="/path/to/config"
ENTITY_GATEWAY_URL="http://[::1]:50051"
BRAVE_SEARCH_API_KEY="..."       # For research macros

# Database Pool (production)
DATABASE_POOL_MAX=50
DATABASE_POOL_MIN=5
```

---

## Agent Workflow (Conductor Mode)

When working as an AI assistant on this codebase:

### Operating Principles

1. **Scope is explicit** - Only modify files mentioned or obviously related. ASK before touching others.

2. **Plan → Confirm → Edit** - Before editing:
   - Summarize what you've read in 3-7 bullets
   - Propose a short numbered plan (3-6 steps)
   - WAIT for explicit approval before changing code

3. **Small, reviewable diffs** - Prefer many small coherent changes over one giant diff.

### High-Risk Areas (Two-Pass Required)

For these areas, always do a **read-only analysis pass** before proposing edits:

- DSL → AST → execution → DB transitions
- UBO graph logic / ownership calculations
- Research agent loop / checkpoint handling
- Anything coupling Rust + SQL + YAML

**Pass 1:** Read files, explain the pipeline, state invariants.
**Pass 2:** Given that understanding, propose specific changes.

### When in Doubt

If uncertain about DSL semantics, CBU/UBO/KYC domain rules, research workflow patterns, or cross-crate boundaries:

1. Stop
2. Explain the uncertainty
3. Ask for clarification
4. Wait for guidance

Never silently "guess and commit" on complex domain logic.

---

## Domain Quick Reference

| Domain | Verbs | Purpose |
|--------|-------|---------|
| `cbu` | 25 | Client Business Unit lifecycle |
| `entity` | 30 | Natural/legal person management |
| `kyc` | 20 | KYC case management |
| `investor` | 15 | Investor register, holdings |
| `session` | 16 | Scope management, navigation, bookmarks |
| `custody` | 40 | Settlement, safekeeping |
| `isda` | 12 | ISDA/CSA agreements |
| `screening` | 10 | Sanctions, PEP screening |
| `gleif` | 15 | GLEIF LEI lookup, hierarchy import |
| `bods` | 9 | BODS 0.4 UBO discovery, import/export |
| `trading-profile` | 15 | Trading matrix configuration |
| `capital` | 25 | Share classes, issuance, supply tracking |
| `ownership` | 20 | Holdings, control, coverage, computation |
| `dilution` | 10 | Options, warrants, convertibles, exercises |
| `agent` | 12 | Agent mode, checkpoints, task orchestration |
| `research.*` | 30+ | External source import, workflow, screening |

**Full verb reference:** See YAML files in `rust/config/verbs/`
**Research/agent details:** See `docs/research-agent-annex.md`

---

## Investor Register Visualization

The investor register uses a **dual-mode visualization** to handle the scale difference between control holders (5-50) and economic investors (potentially 100,000+).

### Visualization Modes

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  CONTROL VIEW (Taxonomy Graph)              ECONOMIC VIEW (Table Panel)     │
│  ─────────────────────────────              ─────────────────────────────   │
│                                                                              │
│  Individual nodes for:                      Aggregate node expands to:       │
│  • >5% voting/economic                      • Breakdown by investor type     │
│  • Board appointment rights                 • Paginated searchable table     │
│  • Veto rights                              • Filter by type/status/country  │
│  • Any special rights                       • Export capability              │
│                                                                              │
│  ┌──────────────┐  ┌──────────────┐        ┌─────────────────────────────┐  │
│  │ AllianzGI    │  │ Sequoia      │        │ 📊 4,847 other investors    │  │
│  │ 35.2% ⚡     │  │ 22.1% 🪑    │        │    (22.0% economic)         │  │
│  │ [View UBOs]  │  │ [View LPs]   │        │    [Click to expand]        │  │
│  └──────────────┘  └──────────────┘        └─────────────────────────────┘  │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Institutional Look-Through

Shareholders can be institutions (not proper persons). The visualization supports drilling into their UBO structure:

| Field | Purpose |
|-------|---------|
| `is_terminal` | `true` = proper person (end of chain), `false` = institution |
| `has_ubo_structure` | Institution has navigable ownership structure |
| `cbu_id` | Link to institution's CBU graph (if onboarded) |
| `known_ubos` | Pre-fetched UBO summary (max 5) |
| `chain_depth` | Levels to reach all proper persons |
| `ubo_discovery_status` | COMPLETE, PARTIAL, PENDING, NOT_REQUIRED |

### Threshold Rules

Configured per issuer in `kyc.issuer_control_config`:

| Threshold | Default | Effect |
|-----------|---------|--------|
| `disclosure_threshold_pct` | 5% | Above = individual node |
| `material_threshold_pct` | 10% | Highlighted |
| `significant_threshold_pct` | 25% | ⚡ indicator |
| `control_threshold_pct` | 50% | ⚡ + control edge |

**Any holder with board/veto rights appears as individual node regardless of percentage.**

---

## Session State Management

Session = Intent Scope = Visual State = Operation Target. **They are the same thing.**

> **Full details:** `docs/session-visualization-architecture.md`

### Scope Hierarchy

```
Universe (all CBUs)
  └── Book (commercial client: Allianz, BlackRock)
       └── Filtered Book (jurisdiction, fund type, status)
            └── Single CBU
                 └── Entity Neighborhood (N hops)
```

**Session Verbs (session.yaml):**
| Verb | Purpose |
|------|---------|
| `session.set-galaxy` | All CBUs under apex entity |
| `session.set-book` | Filtered subset (jurisdictions, cbu-types) |
| `session.set-cbu` | Single CBU focus |
| `session.set-jurisdiction` | All CBUs in a jurisdiction |
| `session.set-neighborhood` | N hops from focal entity |
| `session.back` / `session.forward` | History navigation |

### Scope Filters

**GraphFilters (graph/types.rs):**
```rust
pub struct GraphFilters {
    pub jurisdictions: Option<Vec<String>>,   // LU, IE, DE
    pub fund_types: Option<Vec<String>>,      // EQUITY, FIXED_INCOME
    pub entity_types: Option<Vec<EntityType>>,
    pub same_manco_id: Option<Uuid>,          // Same management company
    pub same_sicav_id: Option<Uuid>,          // Same SICAV umbrella
    pub min_ownership_pct: Option<Decimal>,
    pub prong: ProngFilter,                   // Both, OwnershipOnly, ControlOnly
    pub as_of_date: NaiveDate,
    pub path_only: bool,
}
```

### Active CBU Set (0..n)

Multi-CBU selection for batch operations:
```rust
pub struct SessionScopeState {
    pub active_cbu_ids: Option<Vec<Uuid>>,  // 0..n CBUs
    // ...
}
```

**Verbs:** `session.add-cbu`, `session.remove-cbu`, `session.clear-cbu-set`, `session.list-active-cbus`

### History / Back-Forward Navigation

**Database (019_session_navigation_history.sql):**
- `session_scope_history` table stores snapshots
- `history_position` column tracks current position
- PL/pgSQL functions: `push_scope_history()`, `navigate_back()`, `navigate_forward()`

**Edge cases handled:** Empty session, no prior history, at end of forward stack → returns `navigated: false`

### Astro Navigation (Scale Levels)

```rust
pub enum ViewLevel {
    Universe,   // All CBUs as dots
    Cluster,    // Segment/galaxy view
    System,     // Single CBU (solar system)
    Planet,     // Single entity focus
    Surface,    // High zoom detail
    Core,       // Deepest zoom
}
```

**Voice/chat triggers:** "universe", "book allianz", "galaxy", "system", "planet", "surface", "core"

### ESPER View Verbs

| Verb | Description |
|------|-------------|
| `view.drill` | Drill into entity (up/down) |
| `view.surface` | Surface up from drill |
| `view.trace` | Follow threads (money/control/risk/documents/alerts) |
| `view.xray` | Show hidden layers (custody/ubo/services) |
| `view.peel` | Remove outer layer |
| `view.illuminate` | Highlight aspect |

### Fractal Zoom (TaxonomyStack)

**API Endpoints:**
- `POST /api/session/:id/taxonomy/zoom-in`
- `POST /api/session/:id/taxonomy/zoom-out`
- `POST /api/session/:id/taxonomy/back-to`
- `GET /api/session/:id/taxonomy/breadcrumbs`

**Transitions (view/transition.rs):**
- `LayoutTransition` - Smooth interpolation with easing
- `EsperTransition` - Stepped Blade Runner-style enhance
- Presets: `QUICK` (0.2s), `STANDARD` (0.35s), `DRAMATIC` (0.5s)

### REPL `:verbs` Command

```
:verbs           → List all domains with verb counts
:verbs kyc       → List KYC domain verbs with args
:verbs session   → List session verbs
```

**MCP equivalent:** `verbs_list` tool with `domain` parameter

---

## Key Files Reference

| What | Where |
|------|-------|
| Verb definitions | `rust/config/verbs/**/*.yaml` |
| Plugin handlers | `rust/src/dsl_v2/custom_ops/` |
| DSL parser | `rust/crates/dsl-core/src/parser.rs` |
| Generic executor | `rust/src/dsl_v2/generic_executor.rs` |
| Agent controller | `rust/src/agent/controller.rs` |
| Research handlers | `rust/src/research/` |
| Prompt templates | `prompts/research/` |
| API routes | `rust/src/api/` |
| Migrations | `migrations/*.sql` |
| Config types | `rust/crates/dsl-core/src/config/types.rs` |

---

## egui Pattern Compliance Checklist

> **Last audited:** 2026-01-11
> **Status:** ✅ All panels compliant with documented patterns

### Core Rules (from `docs/strategy-patterns.md` §3)

| Rule | Description | Status |
|------|-------------|--------|
| **State External** | All state stored in `AppState`, never in widgets | ✅ |
| **Actions Return Values** | Panels return `Option<Action>` enums, no callbacks | ✅ |
| **Short Lock Windows** | Lock async_state, extract data, release, then render | ✅ |
| **No Async in update()** | All async via `spawn_local` + channels | ✅ |
| **Window Stack** | Modals use `WindowStack`, ESC closes topmost | ✅ |

### Panel Action Pattern (MANDATORY)

Every panel that handles user interactions MUST:

```rust
// 1. Define action enum in panel module
pub enum MyPanelAction {
    None,
    DoSomething { param: String },
    Close,
}

// 2. Panel returns Option<Action>, NEVER mutates AppState directly
pub fn my_panel(ui: &mut Ui, data: &MyPanelData) -> Option<MyPanelAction> {
    let mut action = None;
    
    if ui.button("Click").clicked() {
        action = Some(MyPanelAction::DoSomething { 
            param: "value".to_string() 
        });
    }
    
    action  // Return, don't handle here
}

// 3. App::update() calls handler AFTER rendering
fn handle_my_panel_action(&mut self, action: Option<MyPanelAction>) {
    let Some(action) = action else { return };
    match action {
        MyPanelAction::DoSomething { param } => { /* mutate state */ }
        MyPanelAction::Close => { /* close panel */ }
        MyPanelAction::None => {}
    }
}
```

**Why:** Separates rendering from mutation, prevents borrow conflicts, enables action composition.

### Async State Pattern (MANDATORY)

```rust
// ✅ CORRECT: Short lock, extract, release, then use
let (loading, data) = {
    let guard = state.async_state.lock().unwrap();
    (guard.loading_chat, guard.pending_data.clone())
};  // Lock released here
// Now safe to render with extracted data

// ❌ WRONG: Holding lock across render
let guard = state.async_state.lock().unwrap();
ui.label(&guard.some_field);  // Still holding lock!
```

### Window Stack Layers

| Layer | Purpose | Modal | Examples |
|-------|---------|-------|----------|
| 0 | Base panels | No | Chat, DSL Editor, Graph, Results |
| 1 | Slide-in panels | No | Entity Detail, Container Browse |
| 2 | Modals | Yes | Resolution, CBU Search, Confirmation |
| 3 | Toasts | No | Notifications, Errors |

**ESC closes topmost modal (layer 2+), never base panels.**

### Text Buffer Pattern

```rust
// state.rs - UI-only mutable state
pub struct TextBuffers {
    pub chat_input: String,      // Chat message being composed
    pub dsl_editor: String,      // DSL source being edited
    pub entity_search: String,   // Search query
    pub dsl_dirty: bool,         // For "unsaved changes" warning ONLY
}

// ✅ CORRECT: Bind to buffer
TextEdit::singleline(&mut state.buffers.chat_input)

// ❌ WRONG: Temporary string (resets every frame!)
TextEdit::singleline(&mut String::new())
```

### Panels That Follow All Patterns (Reference)

| Panel | Actions | Async | Notes |
|-------|---------|-------|-------|
| `chat.rs` | Via AppState | ✅ Short lock | Focus management on chat complete |
| `dsl_editor.rs` | `DslEditorAction` | ✅ | Clear/Validate/Execute actions |
| `resolution.rs` | `ResolutionPanelAction` | ✅ | Sub-session modal with voice |
| `cbu_search.rs` | `CbuSearchAction` | ✅ | Focus on just_opened flag |
| `context.rs` | `ContextPanelAction` | ✅ | Stage focus, context selection |
| `taxonomy.rs` | `TaxonomyPanelAction` | ✅ | Fractal navigation zoom |
| `trading_matrix.rs` | `TradingMatrixPanelAction` | ✅ | Node selection, navigation |
| `investor_register.rs` | `InvestorRegisterAction` | ✅ | Aggregate expand, drill-down |
| `toolbar.rs` | `ToolbarAction` | ✅ | CBU select, layout, view mode |

### Anti-Patterns to Avoid

```rust
// ❌ NEVER: Callback-based event handling
ui.button("Click").on_click(|| self.do_thing());  // Not egui API anyway

// ❌ NEVER: Mutating state during render
if ui.button("Save").clicked() {
    self.state.save();  // Borrow conflict!
}

// ❌ NEVER: Async in update()
async fn update(&mut self) {
    let data = fetch().await;  // BLOCKS UI THREAD
}

// ❌ NEVER: Long lock windows
let guard = self.async_state.lock().unwrap();
// ... 50 lines of rendering while holding lock ...

// ❌ NEVER: Ad-hoc Window::new outside WindowStack for modals
Window::new("My Modal")...  // Use WindowStack.push() instead
```

### Voice Integration Pattern

Voice commands flow through unified dispatcher → resolution modal when active:

```rust
// app.rs - Route voice to resolution when modal active
fn process_voice_commands(&mut self) {
    let resolution_active = self.state.resolution_ui.voice_active
        && self.state.window_stack.has(&WindowType::Resolution);
    
    for cmd in take_pending_voice_commands() {
        if resolution_active {
            self.process_resolution_voice_command(&cmd.transcript, cmd.confidence);
        } else {
            // Normal voice command dispatch
        }
    }
}
```

---

*For detailed reference material, see the docs/ directory and ai-thoughts/ working documents.*
