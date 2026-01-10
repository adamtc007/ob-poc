# CLAUDE.md

> **Last reviewed:** 2026-01-10
> **Verb count:** ~760 verbs across 97 YAML files
> **Custom ops:** 47 plugin handlers
> **Crates:** 13 fine-grained crates
> **Migrations:** 13 schema migrations (latest: 013_capital_structure_ownership.sql)
> **Pending:** DSL execution path consolidation (see ai-thoughts/015-consolidate-dsl-execution-path.md)

This file provides guidance to Claude Code when working with this repository.

---

## Deep Dive Documentation

**CLAUDE.md is the quick reference. Detailed docs are in /docs.**

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

**How to read:** Use `view docs/filename.md` before starting the task.

### Reference Documentation (read as needed)

| When working on... | Read this file | Contains |
|--------------------|----------------|----------|
| **Understanding WHY things work this way** | `docs/strategy-patterns.md` | Data philosophy, CBU/UBO/Trading concepts, Agent strategy, egui do's/don'ts |
| **Creating or modifying verb YAML** | `docs/verb-definition-spec.md` | **CRITICAL** - exact YAML structure, valid field values, common errors |
| **Entity model, schemas, relationships** | `docs/entity-model-ascii.md` | Full ERD, table relationships, identifier schemes, UBO flow, dual-use holdings |
| **DSL parser, compiler, executor** | `docs/dsl-verb-flow.md` | Pipeline stages, verb resolution, YAML structure, capture/interpolation, plugin handlers |
| **Agent pipeline, LLM integration** | `docs/agent-architecture.md` | Lexicon tokenizer, intent parsing, research macros, conductor mode, voice |
| **UI, graph viz, REPL commands** | `docs/repl-viewport.md` | 5-panel layout, shared state, graph interactions, taxonomy navigator, galaxy nav |

**START HERE for non-obvious concepts:**
- "Why is everything an Entity?" → `docs/strategy-patterns.md` §1
- "What's the difference between CBU and Entity?" → `docs/strategy-patterns.md` §1
- "How does UBO discovery work?" → `docs/strategy-patterns.md` §1
- "Why DSL instead of direct SQL?" → `docs/strategy-patterns.md` §2
- "What are Research Macros?" → `docs/strategy-patterns.md` §2
- "egui patterns and gotchas" → `docs/strategy-patterns.md` §3
- "Verb YAML not loading?" → `docs/verb-definition-spec.md` §5 (Common Errors)

**Trigger phrases (if you see these in a task, read the doc first):**
- "add verb", "new verb", "create verb", "verb YAML" → `docs/verb-definition-spec.md`
- "egui", "viewport", "immediate mode", "graph widget" → `docs/strategy-patterns.md` §3
- "entity model", "CBU", "UBO", "holdings" → `docs/strategy-patterns.md` §1
- "agent", "MCP", "research macro" → `docs/strategy-patterns.md` §2
- "investor register", "cap table", "shareholder", "control holder" → `ai-thoughts/018-investor-register-visualization.md`
- "institutional holder", "UBO chain", "look-through" → `ai-thoughts/018-investor-register-visualization.md`

**Working documents (TODOs, plans):**
- `ai-thoughts/015-consolidate-dsl-execution-path.md` - Unify DSL execution to single session-aware path
- `ai-thoughts/016-capital-structure-ownership-model.md` - Multi-class cap table, voting/economic rights, dilution
- `ai-thoughts/017-transactional-safety-complex-capital-verbs.md` - SERIALIZABLE + advisory locks for splits/exercises
- `ai-thoughts/018-investor-register-visualization.md` - Dual-mode visualization, threshold collapse, institutional look-through

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
│   ├── config/verbs/           # Verb YAML definitions (95 files, ~720 verbs)
│   │   ├── cbu.yaml            # CBU domain
│   │   ├── entity.yaml         # Entity domain
│   │   ├── custody/            # Custody subdomain
│   │   ├── kyc/                # KYC subdomain
│   │   └── registry/           # Investor registry
│   ├── crates/
│   │   ├── dsl-core/           # Parser, AST, compiler (NO DB dependency)
│   │   ├── ob-agentic/         # LLM agent for DSL generation
│   │   ├── ob-poc-web/         # Axum server + API
│   │   ├── ob-poc-ui/          # egui/WASM UI
│   │   └── ob-poc-graph/       # Graph visualization widget
│   ├── src/
│   │   ├── dsl_v2/             # DSL execution layer
│   │   │   ├── generic_executor.rs  # YAML-driven CRUD executor
│   │   │   ├── custom_ops/     # Plugin handlers (~44 files)
│   │   │   └── verb_registry.rs
│   │   ├── api/                # REST API routes
│   │   └── bin/
│   │       ├── dsl_api.rs      # Main Axum server
│   │       └── dsl_cli.rs      # CLI tool
│   └── xtask/                  # Build automation
├── migrations/                 # SQLx migrations (12 files)
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

### Verify Verbs Load

```bash
cargo x verify-verbs   # Shows parse errors for all YAML files
```

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
- Anything coupling Rust + SQL + YAML

**Pass 1:** Read files, explain the pipeline, state invariants.
**Pass 2:** Given that understanding, propose specific changes.

### When in Doubt

If uncertain about DSL semantics, CBU/UBO/KYC domain rules, or cross-crate boundaries:

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
| `ownership` | 15 | Holdings, control positions, snapshots |
| `dilution` | 10 | Options, warrants, convertibles, exercises |

**Full verb reference:** See YAML files in `rust/config/verbs/`

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

### Key Types

| Type | Location | Purpose |
|------|----------|---------|
| `InvestorRegisterView` | `rust/src/graph/investor_register.rs` | Full API response |
| `ControlHolderNode` | Same | Individual holder above threshold |
| `AggregateInvestorsNode` | Same | Collapsed "N other investors" node |
| `UboSummary` | Same | Pre-fetched UBO for institutional holders |
| `InvestorPanelState` | `ob-poc-ui/src/state.rs` | UI-only panel state |
| `InvestorPanelAction` | `ob-poc-ui/src/panels/investor_panel.rs` | Panel interaction actions |

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

## Key Files Reference

| What | Where |
|------|-------|
| Verb definitions | `rust/config/verbs/**/*.yaml` |
| Plugin handlers | `rust/src/dsl_v2/custom_ops/` |
| DSL parser | `rust/crates/dsl-core/src/parser.rs` |
| Generic executor | `rust/src/dsl_v2/generic_executor.rs` |
| API routes | `rust/src/api/` |
| Migrations | `migrations/*.sql` |
| Config types | `rust/crates/dsl-core/src/config/types.rs` |

---

*For detailed reference material, see the docs/ directory.*
