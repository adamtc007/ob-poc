# CLAUDE.md

> **Last reviewed:** 2026-01-09
> **Verb count:** ~720 verbs across 94 YAML files
> **Custom ops:** 42 plugin handlers
> **Crates:** 13 fine-grained crates
> **Migrations:** 11 schema migrations (latest: 011_investor_register.sql)
> **Pending:** Investor Register + UBO integration (see ai-thoughts/013-investor-register-ubo-integration.md)

This file provides guidance to Claude Code when working with this repository.

---

## Deep Dive Documentation

**CLAUDE.md is the quick reference. For detailed architecture, see these docs:**

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

**BEFORE creating verb YAML:**
Always read `docs/verb-definition-spec.md` - the structs are strict and errors are silent.

**Working documents (TODOs, plans):**
- `ai-thoughts/013-investor-register-ubo-integration.md` - Investor register + UBO implementation plan

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
│   ├── config/verbs/           # Verb YAML definitions (94 files, ~720 verbs)
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
│   │   │   ├── custom_ops/     # Plugin handlers (~42 files)
│   │   │   └── verb_registry.rs
│   │   ├── api/                # REST API routes
│   │   └── bin/
│   │       ├── dsl_api.rs      # Main Axum server
│   │       └── dsl_cli.rs      # CLI tool
│   └── xtask/                  # Build automation
├── migrations/                 # SQLx migrations (11 files)
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

> **⚠️ CRITICAL:** Read `docs/verb-definition-spec.md` BEFORE creating verb YAML.
> Errors are SILENT - invalid YAML causes verbs to not load with no error message.

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
| `custody` | 40 | Settlement, safekeeping |
| `isda` | 12 | ISDA/CSA agreements |
| `screening` | 10 | Sanctions, PEP screening |
| `gleif` | 8 | GLEIF API integration |
| `trading-profile` | 15 | Trading matrix configuration |

**Full verb reference:** See YAML files in `rust/config/verbs/`

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
