# CLAUDE.md

> **Last reviewed:** 2026-01-14
> **Verb count:** 800+ verbs across 103 YAML files
> **Custom ops:** 51 plugin handlers
> **Crates:** 13 fine-grained crates
> **Migrations:** 31 schema migrations (latest: 031_economic_lookthrough.sql)
> **Investor Register:** ✅ Complete - Role profiles, fund vehicles, economic look-through
> **Feedback System:** ✅ Complete - Event capture + inspector + MCP tools
> **Session/View:** ✅ Implemented - Scopes, filters, ESPER verbs, history
> **CBU Session v2:** ✅ Complete - In-memory sessions, 9 MCP tools, REST API, undo/redo, persistence
> **Verb Tiering:** ✅ Complete - All verbs tagged with tier metadata
> **Verb Governance:** ✅ Complete - Mandatory metadata, single authoring surface, STANDARD lint enforcement
> **Viewport Scaling:** ✅ Complete - Force simulation + LOD thresholds scale to viewport size
> **CBU View Mode:** ✅ Complete - Single TRADING view mode as default, simplified ViewMode struct
> **CBU Container Rendering:** ✅ Complete - Container layout with status badge, external taxonomy positioning, attachment edges, taxonomy navigation
> **Entity Resolution:** ⚠️ In Progress - UX design + implementation plan done, UI wiring pending
> **Service Resource Pipeline:** ✅ Complete - Intent→Discovery→Attributes→Provisioning→Readiness + taxonomy viz

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
| **Entity Disambiguation UX** | `ai-thoughts/025-entity-disambiguation-ux.md` | ✅ **DONE** - Inline popup + batch modal design, voice refinement |
| **Feedback Inspector (impl)** | `ai-thoughts/026-implement-feedback-inspector.md` | ✅ **DONE** - Classifier, redactor, repro gen, audit trail, 6 MCP tools, REPL commands |
| **Entity Resolution Plan** | `ai-thoughts/026-entity-resolution-implementation-plan.md` | ⚠️ **IN PROGRESS** - Sub-session architecture, 4-phase implementation |
| **Trading Matrix Pivot** | `ai-thoughts/027-trading-matrix-canonical-pivot.md` | ✅ **DONE** - Types + linter + all verbs tagged with tier metadata |
| **Verb Lexicon Governance** | `ai-thoughts/028-verb-lexicon-governance.md` | ✅ **DONE** - Mandatory metadata, rip-and-replace, lint tiers, matrix-first enforcement |
| **Implement Verb Governance** | `ai-thoughts/029-implement-verb-governance.md` | ✅ **DONE** - 46 verbs reclassified, 15 new CA/plan-apply verbs, 6 proof tests |
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
- "investor register", "cap table", "shareholder", "control holder" → `ai-thoughts/018-*`, See Investor Register section
- "institutional holder", "UBO chain", "look-through" → `ai-thoughts/018-*`, `ai-thoughts/019-*`, See Investor Register section
- "nominee", "custodian", "fund of funds", "FoF", "master-feeder" → See Investor Register section
- "investor role", "holder classification", "UBO eligibility" → See Investor Register section
- "economic exposure", "diluted ownership", "indirect ownership" → See Investor Register section
- "fund vehicle", "SICAV", "RAIF", "SCSp", "FCP", "umbrella", "compartment" → See Investor Register section
- "look-through policy", "end investor", "beneficial owner" → See Investor Register section
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
- "entity resolution", "disambiguation", "unresolved ref", "batch resolve" → `ai-thoughts/025-entity-disambiguation-ux.md`, `ai-thoughts/026-entity-resolution-implementation-plan.md`
- "sub-session", "resolution modal", "inline popup" → `ai-thoughts/026-entity-resolution-implementation-plan.md`
- "verb governance", "verb lifecycle", "mandatory metadata" → `ai-thoughts/028-verb-lexicon-governance.md`
- "lint rules", "verb linter", "MINIMAL/BASIC/STANDARD" → `ai-thoughts/028-verb-lexicon-governance.md`
- "single authoring surface", "projection-only", "one commit path" → `ai-thoughts/028-verb-lexicon-governance.md`
- "pitch", "internal sell", "coalition", "bank-safe", "coexistence", "pilot" → `ai-thoughts/030-internal-pitch-strategy.md`
- "RAG", "vector", "qdrant", "embedding", "stack audit", "round-trip", "reconciliation" → `ai-thoughts/031-rag-cleanup-stack-audit.md`
- "corporate action", "CA policy", "election policy", "dividend", "rights issue", "proceeds SSI" → `ai-thoughts/032-corporate-actions-integration.md`
- "cbu session", "session persistence", "load cbu", "unload cbu", "session undo", "session redo" → See CBU Session v2 section below
- "force simulation", "viewport scaling", "LOD", "detail level", "graph density" → See Viewport Scaling section below
- "service resource", "service intent", "resource discovery", "provisioning", "readiness" → See Service Resource Pipeline section below
- "service taxonomy", "product service", "attribute satisfaction", "srdef" → See Service Resource Pipeline section below
- "container", "container_parent_id", "is_container", "entities inside CBU", "trading nodes outside", "status badge", "attachment edge", "taxonomy navigation", "external taxonomy" → See CBU Container Rendering section below

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
- `ai-thoughts/025-entity-disambiguation-ux.md` - ✅ DONE - Inline popup + batch modal design, Zed-style code actions, voice refinement
- `ai-thoughts/026-implement-feedback-inspector.md` - ✅ DONE - Classifier, redactor, repro gen, audit trail, 6 MCP tools
- `ai-thoughts/026-entity-resolution-implementation-plan.md` - ⚠️ **IN PROGRESS** - Sub-session architecture, parent-child context inheritance
- `ai-thoughts/027-trading-matrix-canonical-pivot.md` - ✅ DONE - Types, linter, all verbs tagged with tier metadata
- `ai-thoughts/028-verb-lexicon-governance.md` - ✅ DONE - Mandatory metadata, rip-and-replace, lint tiers, matrix-first enforcement
- `ai-thoughts/029-implement-verb-governance.md` - ✅ DONE - 46 verbs reclassified, 15 new verbs, idempotency tests
- `ai-thoughts/030-internal-pitch-strategy.md` - 📝 Strategic - Bank-safe positioning, coalition building, pilot slice definition
- `ai-thoughts/031-rag-cleanup-stack-audit.md` - 📝 **TODO** - Safe RAG cleanup + full round-trip audit (DB→verbs→Rust→DSL→agent→egui)
- `ai-thoughts/032-corporate-actions-integration.md` - 📝 **TODO** - CA integration: ISO 15022 compliant (53 CAEV codes), migration, Rust types, intent verbs, materialize (~16h)

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
│   ├── config/verbs/           # Verb YAML definitions (102 files, 781 verbs)
│   │   ├── cbu.yaml            # CBU domain
│   │   ├── entity.yaml         # Entity domain
│   │   ├── custody/            # Custody subdomain
│   │   ├── kyc/                # KYC subdomain
│   │   ├── registry/           # Investor registry
│   │   ├── research/           # Research workflows
│   │   └── agent/              # Agent mode verbs
│   ├── crates/
│   │   ├── dsl-core/           # Parser, AST, compiler (NO DB dependency)
│   │   ├── ob-agentic/         # LLM agent for DSL generation
│   │   ├── ob-poc-web/         # Axum server + API
│   │   ├── ob-poc-ui/          # egui/WASM UI
│   │   └── ob-poc-graph/       # Graph visualization widget
│   ├── src/
│   │   ├── dsl_v2/             # DSL execution layer
│   │   │   ├── generic_executor.rs  # YAML-driven CRUD executor
│   │   │   ├── custom_ops/     # Plugin handlers (48 files)
│   │   │   └── verb_registry.rs
│   │   ├── research/           # Research module
│   │   ├── agent/              # Agent controller
│   │   ├── api/                # REST API routes
│   │   └── bin/
│   │       ├── dsl_api.rs      # Main Axum server
│   │       └── dsl_cli.rs      # CLI tool
│   └── xtask/                  # Build automation
├── prompts/                    # LLM prompt templates
│   └── research/
├── migrations/                 # SQLx migrations (21 files)
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
cargo x verbs check         # Check verb YAML files are up-to-date with DB
cargo x verbs lint          # Lint verbs for tiering rule compliance
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
cargo x verbs check       # Check if YAML configs match DB (CI check)
cargo x verbs lint        # Check tiering rule violations
cargo x verbs compile     # Compile YAML → sync to database
cargo x verbs diagnostics # Show verbs with errors/warnings
```

### Verb Tiering System

Verbs are categorized by their role in the data flow:

| Tier | Purpose | Example |
|------|---------|---------|
| `reference` | Global reference data (catalogs) | `corporate-action:define-event-type` |
| `intent` | User-facing authoring operations | `trading-profile:add-market` |
| `projection` | Internal writes to operational tables | `_write-instrument` (internal) |
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

### Verb Governance (Enforced)

> **Full details:** `ai-thoughts/028-verb-lexicon-governance.md`, `ai-thoughts/029-implement-verb-governance.md`

**Linter Tiers (Buf-style):**

| Tier | Rules Enforced | CI Status |
|------|----------------|----------|
| `MINIMAL` | Required metadata fields present | ✅ Pass |
| `BASIC` | Naming conventions, create/ensure semantics | ✅ Pass |
| `STANDARD` | Single authoring surface, projection-only writes, one commit path | ✅ Pass |

**Run linter:**
```bash
cargo x verbs lint                    # Default: STANDARD tier
cargo x verbs lint --tier minimal     # Check only required fields
cargo x verbs lint --tier basic       # Check naming + semantics
```

**Hard Enforcement Rules:**

| Rule | Description | Violation = |
|------|-------------|-------------|
| S001 | If `source_of_truth: matrix`, no other verb can be `tier: intent` for same noun | Error |
| S002 | If `writes_operational: true`, must be `tier: projection` or `composite` | Error |
| S003 | If `tier: projection` + `writes_operational`, must be `internal: true` | Error |
| B001 | `create-*` must use `operation: insert` | Error |
| B002 | `ensure-*` must use `operation: upsert` | Error |
| B003 | `delete-*` on regulated nouns requires `dangerous: true` | Error |

### Deleted Verbs (Rip and Replace)

This is a POC - no deprecation period needed. The following verbs were **deleted** (not deprecated):

| Deleted From | Verbs Removed | Use Instead |
|--------------|---------------|-------------|
| `instruction-profile` | `assign-template`, `override-field`, `remove-assignment`, `bulk-assign` | `trading-profile.add-standing-instruction`, etc. |
| `cbu-custody` | `add-instrument`, `add-market`, `add-universe`, `remove-instrument` | `trading-profile.add-*` |
| `trade-gateway` | `assign-gateway`, `set-routing` | `trading-profile.add-gateway` |
| `settlement-chain` | `assign-chain` | `trading-profile.add-settlement-chain` |
| `pricing-config` | `set-source` | `trading-profile.set-pricing-source` |
| `tax-config` | `set-treatment` | `trading-profile.set-tax-treatment` |
| `corporate-action` | `set-preference` | `trading-profile.ca.set-election-policy` |

**Single authoring surface:** `trading-profile.*` is the ONLY way to configure CBU trading setup.

### Materialize Plan/Apply

The `trading-profile.materialize` composite now supports plan/apply separation:

| Verb | Purity | DB Writes | Use Case |
|------|--------|-----------|----------|
| `generate-materialization-plan` | Pure | None | Preview changes, dry-run |
| `apply-materialization-plan` | Transactional | Yes | Apply after review |
| `materialize` | Orchestrator | Yes | Convenience (generate + apply) |

**Idempotency guarantee:** Running materialize twice produces no changes on second run.

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
| `trading-profile` | 30 | Trading matrix configuration, CA policy, plan/apply |
| `corporate-action` | 8 | CA event types (ISO 15022), preferences, instruction windows |
| `capital` | 25 | Share classes, issuance, supply tracking |
| `ownership` | 20 | Holdings, control, coverage, computation |
| `dilution` | 10 | Options, warrants, convertibles, exercises |
| `agent` | 12 | Agent mode, checkpoints, task orchestration |
| `research.*` | 30+ | External source import, workflow, screening |
| `service-intent` | 3 | Service intent creation, listing |
| `resource-discovery` | 3 | Discover resources for services |
| `resource-attributes` | 2 | Attribute satisfaction tracking |
| `provisioning` | 3 | Request/track resource provisioning |
| `readiness` | 3 | CBU service readiness queries |

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

## Investor Register & Economic Look-Through Pipeline

> **Status:** ✅ Complete
> **Migrations:** 029-031 (role profiles, fund vehicles, economic look-through)
> **Verbs:** 35+ across 4 domains (investor-role, fund-vehicle, fund-compartment, economic-exposure)
> **Tests:** 7 integration tests in `rust/tests/investor_register_tests.rs`

### The Problem: Control vs Economic Ownership

Financial KYC/AML requires tracking two fundamentally different ownership concepts:

| Concept | Question | Scale | Use Case |
|---------|----------|-------|----------|
| **Control/Voting** | "Who controls decisions?" | 5-50 holders | Board composition, blocking rights, regulatory filings (13D/13G, TR-1) |
| **Economic** | "Who receives distributions?" | 100,000+ holders | AML exposure analysis, tax reporting, investor communications |

**The mismatch problem:**
- Control holders are few, named, and need individual graph nodes
- Economic holders are many, often anonymous, and need aggregation
- Traditional systems either show 50,000 nodes (unusable) or collapse everything (loses control insight)
- Institutional holders (pension funds, FoFs) aren't "proper persons" - they have their own UBO chains

**Domain jargon this solves:**
- *Nominee holding* - Custodian holds on behalf of beneficial owners, not UBO-eligible
- *Omnibus account* - Single account aggregating multiple underlying investors
- *Fund-of-funds (FoF)* - Intermediary fund that invests in other funds, requires look-through
- *Master-feeder structure* - Pooling vehicle where feeders invest in master, master holds assets
- *Intra-group treasury* - Group company moving cash between subsidiaries, not "real" external investor
- *Look-through* - Tracing through intermediaries to find ultimate beneficial owners
- *TA holding* - Transfer Agent record, administrative not beneficial
- *End investor* - Terminal holder who actually receives economic benefit

### Architecture: Temporal Role Profiles + Bounded Look-Through

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    Investor Register Architecture                            │
│                                                                              │
│  ┌───────────────────┐     ┌───────────────────┐     ┌──────────────────┐   │
│  │  Role Profiles    │     │  Fund Vehicles    │     │ Economic Edges   │   │
│  │  (kyc.investor_   │     │  (kyc.fund_       │     │ (v_economic_     │   │
│  │   role_profiles)  │     │   vehicles)       │     │  edges_direct)   │   │
│  │                   │     │                   │     │                  │   │
│  │  • Issuer-scoped  │     │  • SCSP, SICAV,   │     │  • pct_of_to     │   │
│  │  • Temporal       │     │    FCP, LP, etc   │     │  • usage_type    │   │
│  │  • UBO eligibility│     │  • Umbrella/      │     │  • Filtered by   │   │
│  │  • Look-through   │     │    compartments   │     │    role profile  │   │
│  │    policy         │     │  • Manager link   │     │                  │   │
│  └───────────────────┘     └───────────────────┘     └──────────────────┘   │
│           │                         │                         │              │
│           └─────────────────────────┼─────────────────────────┘              │
│                                     ▼                                        │
│                    ┌────────────────────────────────┐                        │
│                    │  fn_compute_economic_exposure  │                        │
│                    │  (Bounded recursive CTE)       │                        │
│                    │                                │                        │
│                    │  Stop conditions:              │                        │
│                    │  1. CYCLE_DETECTED             │                        │
│                    │  2. MAX_DEPTH (default 6)      │                        │
│                    │  3. BELOW_MIN_PCT (0.01%)      │                        │
│                    │  4. END_INVESTOR role          │                        │
│                    │  5. POLICY_NONE                │                        │
│                    │  6. NO_BO_DATA available       │                        │
│                    └────────────────────────────────┘                        │
│                                     │                                        │
│                                     ▼                                        │
│                    ┌────────────────────────────────┐                        │
│                    │     Dual-Mode Visualization    │                        │
│                    │                                │                        │
│                    │  Control: Individual nodes     │                        │
│                    │  Economic: Aggregated table    │                        │
│                    └────────────────────────────────┘                        │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Role Types and Look-Through Policies

**Role Types (`investor_role_profiles.role_type`):**

| Role | UBO Eligible | Look-Through | Description |
|------|--------------|--------------|-------------|
| `END_INVESTOR` | ✅ Yes | None | Terminal beneficial owner, proper person |
| `NOMINEE` | ❌ No | None | Custodial/nominee holding, not real owner |
| `OMNIBUS` | ❌ No | On-demand | Aggregated account, multiple underlying |
| `INTERMEDIARY_FOF` | ❌ No | On-demand | Fund-of-funds, invests in other funds |
| `MASTER_POOL` | ❌ No | Auto-if-data | Master fund in master-feeder |
| `INTRA_GROUP_POOL` | ❌ No | Auto-if-data | Group treasury/intercompany |
| `TREASURY` | ❌ No | None | Corporate treasury function |
| `CUSTODIAN` | ❌ No | None | Custodian bank holding |
| `OTHER` | Configurable | Configurable | Custom classification |

**Look-Through Policies (`lookthrough_policy`):**

| Policy | Behavior | Use Case |
|--------|----------|----------|
| `NONE` | Stop here, don't traverse | End investors, nominees |
| `ON_DEMAND` | Only traverse when explicitly requested | FoFs where BO data may exist |
| `AUTO_IF_DATA` | Traverse if `beneficial_owner_data_available=true` | Master pools with known feeders |
| `ALWAYS` | Always traverse regardless of data availability | Regulatory requirement |

### Temporal Versioning

Role profiles support point-in-time queries for mid-year reclassifications:

```sql
-- Same holder, different roles over time
INSERT INTO kyc.investor_role_profiles 
  (issuer_entity_id, holder_entity_id, role_type, effective_from)
VALUES
  ('fund-a', 'blackrock', 'END_INVESTOR', '2024-01-01'),  -- Started as end investor
  ('fund-a', 'blackrock', 'INTERMEDIARY_FOF', '2024-07-01'); -- Reclassified mid-year

-- Query as of March 2024 → END_INVESTOR
-- Query as of September 2024 → INTERMEDIARY_FOF
```

### Fund Vehicle Taxonomy

Luxembourg and international fund structures:

| Vehicle Type | Jurisdiction | Structure | Compartments |
|--------------|--------------|-----------|--------------|
| `SCSP` | Luxembourg | Société en Commandite Spéciale | No |
| `SICAV_RAIF` | Luxembourg | Reserved AIF (RAIF) | Yes |
| `SICAV_SIF` | Luxembourg | Specialized Investment Fund | Yes |
| `SICAV_UCITS` | Luxembourg | UCITS umbrella | Yes |
| `FCP` | Luxembourg | Fonds Commun de Placement | No |
| `LP` | Multiple | Limited Partnership | No |
| `LLC` | US | Limited Liability Company | No |
| `TRUST` | Multiple | Trust structure | No |
| `OEIC` | UK | Open-Ended Investment Company | Yes |
| `ETF` | Multiple | Exchange-Traded Fund | No |
| `REIT` | Multiple | Real Estate Investment Trust | No |

### Key DSL Verbs

**Investor Role Management:**
```
investor-role.set issuer="Allianz Fund I" holder="BlackRock" 
  role-type=INTERMEDIARY_FOF lookthrough-policy=ON_DEMAND

investor-role.mark-as-nominee issuer="Fund A" holder="State Street"

investor-role.read-as-of issuer="Fund A" holder="Investor X" as-of-date=2024-06-15
```

**Fund Vehicle Management:**
```
fund-vehicle.upsert fund-entity-id="Allianz SICAV" vehicle-type=SICAV_RAIF 
  is-umbrella=true domicile-country=LU

fund-compartment.upsert umbrella-fund-entity-id="Allianz SICAV" 
  compartment-code="EQUITY" compartment-name="Global Equity Fund"

share-class.link-to-compartment share-class-id=$class compartment-id=$compartment
```

**Economic Exposure Analysis:**
```
economic-exposure.compute root-entity-id="Fund A" max-depth=6 min-pct=0.01

economic-exposure.summary issuer-entity-id="Fund A" threshold-pct=5.0

issuer-control-config.upsert issuer-entity-id="Fund A" 
  disclosure-threshold-pct=5.0 control-threshold-pct=50.0
```

### API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/capital/:entity_id/economic-exposure` | Compute look-through exposure |
| `GET` | `/api/capital/:entity_id/investor-register` | Get investor register with role profiles |
| `POST` | `/api/capital/:entity_id/role-profile` | Set holder role profile |

### Key Files

| File | Purpose |
|------|---------|
| `migrations/029_investor_role_profiles.sql` | Role profiles table + upsert function |
| `migrations/030_fund_vehicles.sql` | Fund vehicles + compartments + views |
| `migrations/031_economic_lookthrough.sql` | Look-through functions + control config |
| `rust/config/verbs/registry/investor-role.yaml` | Role profile DSL verbs |
| `rust/config/verbs/registry/fund-vehicle.yaml` | Fund vehicle DSL verbs |
| `rust/config/verbs/registry/economic-exposure.yaml` | Look-through DSL verbs |
| `rust/src/api/capital_routes.rs` | REST API endpoints |
| `rust/tests/investor_register_tests.rs` | Integration tests |

### Integration with UBO Computation

The UBO sync trigger (`trg_sync_ubo_on_holding_change`) respects role profiles:

```sql
-- Holdings with usage_type='TA' → no UBO edge (transfer agent record)
-- Holdings where holder has is_ubo_eligible=false → no UBO edge
-- Holdings where holder role_type='NOMINEE' → no UBO edge
```

This prevents nominees, custodians, and administrative holdings from polluting the UBO graph while preserving them in the economic exposure view.

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

## CBU Session v2 (In-Memory Sessions)

> **Migration:** `023_sessions_persistence.sql`
> **Design:** Memory is truth, DB is backup

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     CbuSession (in-memory)                      │
│  - id: Uuid                                                     │
│  - cbu_ids: HashSet<Uuid>     ← Current loaded CBUs             │
│  - history: Vec<HashSet>      ← Undo stack                      │
│  - future: Vec<HashSet>       ← Redo stack                      │
└─────────────────────────────────────────────────────────────────┘
                              │
                    fire-and-forget (2s debounce)
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                   PostgreSQL (backup only)                      │
│  ob-poc.sessions table                                          │
│  - Auto-extends expiry on activity (7 days)                     │
│  - Graceful degradation if DB unavailable                       │
└─────────────────────────────────────────────────────────────────┘
```

### MCP Tools (9 tools)

| Tool | Description |
|------|-------------|
| `session_load_cbu` | Load a CBU into session |
| `session_load_jurisdiction` | Load all CBUs in jurisdiction |
| `session_load_galaxy` | Load all CBUs under apex entity |
| `session_unload_cbu` | Remove CBU from session |
| `session_clear` | Clear all CBUs |
| `session_undo` | Undo last action |
| `session_redo` | Redo undone action |
| `session_info` | Get session state |
| `session_list` | List all sessions |

### REST API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/api/cbu-session` | Create new session |
| `GET` | `/api/cbu-session` | List all sessions |
| `GET` | `/api/cbu-session/:id` | Get session by ID |
| `DELETE` | `/api/cbu-session/:id` | Delete session |
| `POST` | `/api/cbu-session/:id/load-cbu` | Load CBU |
| `POST` | `/api/cbu-session/:id/load-jurisdiction` | Load by jurisdiction |
| `POST` | `/api/cbu-session/:id/load-galaxy` | Load by apex entity |
| `POST` | `/api/cbu-session/:id/unload-cbu` | Unload CBU |
| `POST` | `/api/cbu-session/:id/clear` | Clear session |
| `POST` | `/api/cbu-session/:id/undo` | Undo |
| `POST` | `/api/cbu-session/:id/redo` | Redo |

### Key Files

| File | Purpose |
|------|---------|
| `rust/src/session/cbu_session.rs` | Core session struct + operations |
| `rust/src/session/mod.rs` | Module exports |
| `rust/src/mcp/tools.rs` | MCP tool definitions |
| `rust/src/mcp/handlers/core.rs` | MCP tool handlers |
| `rust/src/api/cbu_session_routes.rs` | REST API routes |

---

## Viewport Scaling (Graph Visualization)

Force simulation and LOD thresholds automatically scale to viewport dimensions.

### Force Simulation Scaling

```rust
// Reference viewport: 800x600 (480,000 px²)
impl ForceConfig {
    pub fn scale_to_viewport(&mut self, width: f32, height: f32) {
        // Boundary scales to 40% of smaller dimension
        let min_dim = width.min(height);
        self.boundary_radius = (min_dim * 0.4).max(200.0);
        
        // Repulsion scales with sqrt of area ratio
        let area_ratio = (width * height) / REFERENCE_VIEWPORT_AREA;
        let repulsion_scale = area_ratio.sqrt().clamp(0.7, 1.5);
        self.repulsion = 8000.0 * repulsion_scale;
    }
}
```

### LOD (Level of Detail) Scaling

```rust
impl LodConfig {
    pub fn for_viewport(width: f32, height: f32) -> Self {
        let area_ratio = (width * height) / REFERENCE_VIEWPORT_AREA;
        let scaled_base = 20.0 * area_ratio.sqrt();
        Self {
            density_base: scaled_base.clamp(10.0, 60.0),
            density_weight: 0.3,
            viewport_area: width * height,
        }
    }
}
```

**Effect:** Larger viewports show more detail (higher density thresholds).

### Key Files

| File | Purpose |
|------|---------|
| `rust/crates/ob-poc-graph/src/graph/force_sim.rs` | Force simulation + viewport scaling |
| `rust/crates/ob-poc-graph/src/graph/lod.rs` | LOD config + density thresholds |
| `rust/crates/ob-poc-graph/src/graph/galaxy.rs` | Resize detection + wiring |

---

## CBU View Mode (Simplified)

The CBU graph view has been simplified from 5 view modes to a single TRADING view mode.

### Before (Removed)
- `KYC_UBO` - KYC/UBO ownership view (was default)
- `SERVICE_DELIVERY` - Product/service delivery chain
- `PRODUCTS_ONLY` - Products without service expansion
- `BOARD_CONTROL` - Board/control relationships

### After (Current)
- `TRADING` - Trading network view showing:
  - CBU container node
  - Trading Profile
  - Instrument Matrix
  - Instrument classes (equities, bonds, derivatives, etc.)
  - Markets (exchanges)
  - Counterparty entities

### API Default Change

All graph API endpoints now default to `TRADING` view mode:
```
GET /api/cbu/{cbu_id}/graph              → TRADING (was KYC_UBO)
GET /api/cbu/{cbu_id}/graph?view_mode=TRADING  → Explicit
```

### ViewMode Struct

`ViewMode` is now a unit struct (not an enum):
```rust
// rust/crates/ob-poc-graph/src/graph/mod.rs
pub struct ViewMode;  // Single CBU/Trading view
```

### Key Files Changed

| File | Change |
|------|--------|
| `ob-poc-graph/src/graph/mod.rs` | ViewMode enum → unit struct |
| `ob-poc-ui/src/command.rs` | Consolidated voice triggers, keyboard "g" for graph |
| `api/graph_routes.rs` | Default changed from KYC_UBO to TRADING |
| `config/verbs/graph.yaml` | Default view-mode: TRADING |
| `config/verbs/view.yaml` | `view.cbu` mode: trading/ubo only |

---

## Service Resource Pipeline

The service resource pipeline handles the flow from product/service intent through resource discovery, attribute satisfaction, and provisioning to readiness.

### Pipeline Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     Service Resource Pipeline                                │
│                                                                              │
│  ┌───────────┐    ┌───────────┐    ┌───────────┐    ┌───────────┐           │
│  │  Intent   │ → │ Discovery │ → │ Attributes│ → │Provisioning│ → Ready   │
│  │           │    │           │    │           │    │           │           │
│  │ CBU wants │    │ Find SRDEF│    │ Check/    │    │ Request   │           │
│  │ a service │    │ resources │    │ satisfy   │    │ resources │           │
│  └───────────┘    └───────────┘    └───────────┘    └───────────┘           │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### DSL Domains (14 verbs total)

| Domain | Verbs | Description |
|--------|-------|-------------|
| `service-intent` | `create`, `list`, `get` | Express CBU intent for services |
| `resource-discovery` | `discover`, `list-for-service`, `get-definition` | Find SRDEF resources |
| `resource-attributes` | `check-satisfaction`, `record-satisfaction` | Track attribute completion |
| `provisioning` | `request`, `status`, `list-pending` | Request/track provisioning |
| `readiness` | `check`, `summary`, `blocking-resources` | Query CBU readiness |

### MCP Tools (10 tools)

| Tool | Description |
|------|-------------|
| `service_intent_create` | Create service intent for CBU |
| `service_intent_list` | List intents for CBU |
| `resource_discovery_discover` | Discover resources for service |
| `resource_discovery_list_for_service` | List resources for service |
| `resource_attributes_check` | Check attribute satisfaction |
| `resource_attributes_record` | Record attribute as satisfied |
| `provisioning_request` | Request resource provisioning |
| `provisioning_status` | Check provisioning status |
| `readiness_check` | Check CBU readiness for service |
| `readiness_summary` | Get readiness summary |

### REST API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/api/service-resource/intent` | Create service intent |
| `GET` | `/api/service-resource/intent/:cbu_id` | List intents for CBU |
| `POST` | `/api/service-resource/discover` | Discover resources |
| `GET` | `/api/service-resource/resources/:service_id` | List resources for service |
| `POST` | `/api/service-resource/attributes/check` | Check satisfaction |
| `POST` | `/api/service-resource/attributes/record` | Record satisfaction |
| `POST` | `/api/service-resource/provision` | Request provisioning |
| `GET` | `/api/service-resource/provision/:request_id` | Get provisioning status |
| `GET` | `/api/service-resource/readiness/:cbu_id` | Check readiness |
| `GET` | `/api/service-resource/readiness/:cbu_id/summary` | Get summary |

### Service Taxonomy Visualization

The UI includes a hierarchical browser for the service resource structure:

```
┌─────────────────────────────────────────────────────────────────┐
│  Browser Tabs: [Entities] [Trading] [Services]                  │
├─────────────────────────────────────────────────────────────────┤
│  ▼ CBU: Acme Fund                                               │
│    ▼ Product: Prime Brokerage                                   │
│      ▼ Service: Securities Lending                              │
│        ├─ Intent: intent-001 [ACTIVE]                          │
│        ▼ Resource: SRDEF-CUSTODY-ACCOUNT                       │
│          ▼ Attributes                                           │
│            ├─ [✓] account_number: "12345"                      │
│            └─ [✗] routing_code: (missing)                      │
│      ▼ Service: Margin Financing                                │
│        └─ ... (collapsed)                                       │
└─────────────────────────────────────────────────────────────────┘
```

**Node types:**
- `Root` - CBU container
- `Product` - Product category
- `Service` - Service under product
- `ServiceIntent` - Active intent
- `Resource` - SRDEF resource definition
- `AttributeCategory` - Attribute grouping
- `Attribute` - Individual attribute (✓/✗ status)
- `AttributeValue` - Source of satisfaction

**Status indicators:**
- `Ready` (green) - All attributes satisfied
- `Partial` (yellow) - Some attributes missing
- `Blocked` (red) - Critical attributes missing
- `Pending` (gray) - Awaiting provisioning

### Key Files

| File | Purpose |
|------|---------|
| `rust/config/verbs/service-pipeline.yaml` | DSL verb definitions (14 verbs) |
| `rust/src/dsl_v2/custom_ops/service_pipeline_ops.rs` | Plugin handlers |
| `rust/src/api/service_resource_routes.rs` | REST API routes |
| `rust/src/mcp/handlers/service_resource.rs` | MCP tool handlers |
| `rust/crates/ob-poc-graph/src/graph/service_taxonomy.rs` | Taxonomy tree widget |
| `rust/crates/ob-poc-ui/src/panels/service_taxonomy.rs` | UI panel wrapper |

---

## CBU Container Rendering

The CBU graph uses container rendering to visually group entities inside a CBU box, while trading infrastructure nodes appear outside.

### Container Field Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  Server (ConfigDrivenGraphBuilder)                                          │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ CBU Node:     is_container: true, contains_type: "entity"           │   │
│  │ Entity Nodes: container_parent_id: <cbu_id>                         │   │
│  │ Trading Nodes: container_parent_id: null (outside container)        │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│                                    ▼ API Response                           │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ UI (LayoutGraph)                                                     │   │
│  │ Uses container_parent_id to group nodes visually                     │   │
│  │ render_containers() draws box around grouped nodes                   │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Visual Result

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│  ┌─────────────────────────────────────────┐                               │
│  │         CBU: Acme Fund                  │    ┌──────────────────────┐   │
│  │  ┌─────────────┐  ┌─────────────┐       │    │  Trading Profile     │   │
│  │  │ Management  │  │ Investment  │       │    └──────────────────────┘   │
│  │  │ Company     │  │ Manager     │       │              │               │
│  │  └─────────────┘  └─────────────┘       │              ▼               │
│  │  ┌─────────────┐  ┌─────────────┐       │    ┌──────────────────────┐   │
│  │  │ Asset       │  │ Custodian   │       │    │  Instrument Matrix   │   │
│  │  │ Owner       │  │             │       │    └──────────────────────┘   │
│  │  └─────────────┘  └─────────────┘       │              │               │
│  └─────────────────────────────────────────┘              ▼               │
│       INSIDE CBU CONTAINER                     ┌───────────────────────┐   │
│                                                │  Equities │ Bonds     │   │
│                                                └───────────────────────┘   │
│                                                   OUTSIDE CONTAINER        │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Container Fields

| Field | Set On | Value | Purpose |
|-------|--------|-------|---------|
| `is_container` | CBU node | `true` | Marks CBU as a container |
| `contains_type` | CBU node | `"entity"` | Type of items inside |
| `container_parent_id` | Entity nodes | CBU UUID | Groups entities in CBU box |
| `container_parent_id` | Trading nodes | `null` | Renders outside the box |

### Container Header with Status Badge

The container header displays the CBU name and a status badge:

```rust
// Status badge colors (render.rs)
match status.to_lowercase().as_str() {
    "active" | "approved" => Color32::from_rgb(34, 197, 94),   // green-500
    "pending"             => Color32::from_rgb(250, 204, 21),  // yellow-400
    "blocked" | "rejected"=> Color32::from_rgb(239, 68, 68),   // red-500
    "draft"               => Color32::from_rgb(148, 163, 184), // slate-400
    _                     => Color32::from_rgb(148, 163, 184), // slate-400
}
```

### External Taxonomy Positioning

External taxonomies are positioned relative to the CBU container:

```
         ┌─────────────────────────────┐
         │      Instrument Matrix      │  ← ABOVE container
         │   InstrumentClass  Market   │
         └─────────────────────────────┘
                      │
              attachment edge
                      │
    ┌─────────────────────────────────────┐
    │           CBU Container             │
    │   [entities with roles/edges]       │
    └─────────────────────────────────────┘
                      │
              attachment edge
                      │
         ┌─────────────────────────────┐
         │       Product Matrix        │  ← BELOW container
         │    Products   Services      │
         └─────────────────────────────┘
```

**Layout function:** `position_external_taxonomies()` in `layout.rs`

### Attachment Edges

Attachment edges connect the CBU container to external taxonomies with connector circles at endpoints:

```rust
// render.rs - render_attachment_edges()
// Draws line from container bounds to taxonomy nodes
// With filled circles at both endpoints
```

### Taxonomy Navigation (Double-Click)

Double-clicking on taxonomy nodes triggers navigation:

| Node Type | Action | Enum Value |
|-----------|--------|------------|
| TradingProfile, InstrumentMatrix, InstrumentClass, Market | Navigate to Trading Matrix view | `TaxonomyNavigationAction::TradingMatrix` |
| Product, Service, Resource | Navigate to Service Taxonomy view | `TaxonomyNavigationAction::ServiceTaxonomy` |

**Usage in UI:**
```rust
// In app.rs or graph panel
if let Some(action) = graph_widget.take_taxonomy_navigation_action() {
    match action {
        TaxonomyNavigationAction::TradingMatrix => { /* switch to trading matrix panel */ }
        TaxonomyNavigationAction::ServiceTaxonomy => { /* switch to service taxonomy panel */ }
    }
}
```

### Key Files

| File | Purpose |
|------|---------|
| `rust/src/graph/config_driven_builder.rs` | Sets container fields server-side |
| `rust/src/graph/types.rs` | `LegacyGraphNode` struct with container fields |
| `rust/crates/ob-poc-graph/src/graph/layout.rs` | `position_external_taxonomies()` + container grouping |
| `rust/crates/ob-poc-graph/src/graph/render.rs` | `render_containers()`, `render_attachment_edges()`, status badge |
| `rust/crates/ob-poc-graph/src/graph/input.rs` | Double-click handling, `TaxonomyNavigationAction` |
| `rust/crates/ob-poc-graph/src/graph/mod.rs` | `TaxonomyNavigationAction` enum, `take_taxonomy_navigation_action()` |
| `rust/crates/ob-poc-graph/src/graph/types.rs` | `GraphNodeData` / `LayoutNode` with `status` field |
| `rust/crates/ob-poc-types/src/lib.rs` | Shared `GraphNode` type |

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

> **Last audited:** 2026-01-12
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
| `service_taxonomy.rs` | `ServiceTaxonomyPanelAction` | ✅ | Service tree expand, drilling |

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
