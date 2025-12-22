# CLAUDE.md

This file provides guidance to Claude Code when working with this repository.

## Project Overview

**OB-POC** is a KYC/AML onboarding system using a declarative DSL. The DSL is the single source of truth for onboarding workflows.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                   Web UI (localhost:3000)                       │
│  Server-rendered HTML with embedded JS/CSS                      │
│  Three panels: Chat | DSL Editor | Results                      │
│  rust/src/ui/                                                   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                   Agentic Server (Axum)                         │
│  rust/src/bin/agentic_server.rs                                 │
│  - /api/agent/generate → Claude API → DSL                       │
│  - /api/session/* → Session management                          │
│  - /api/templates/* → Template rendering                        │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                     DSL Pipeline                                 │
│  Parser (Nom) → CSG Linter → Compiler → Executor                │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                  PostgreSQL 17 (data_designer)                  │
│            Extensions: uuid-ossp, pg_trgm, pgvector             │
└─────────────────────────────────────────────────────────────────┘
```

### DSL Pipeline Detail

The pipeline is split into fast local stages (parse, enrich) and slower network stages (validate, execute):

```
┌─────────────────────────────────────────────────────────────────┐
│                     DSL Source Text                              │
│  (cbu.ensure :name "Fund" :jurisdiction "LU" :as @fund)         │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              Stage 1: Parser (Nom) → Raw AST                     │
│  ~16µs for 10 statements - instant keystroke feedback           │
│  rust/src/dsl_v2/parser.rs                                      │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              Stage 2: Enrichment → EntityRef AST                 │
│  Adds entity_type, search_column from YAML config               │
│  EntityRef { resolved_key: None } = unresolved (valid state)    │
│  rust/src/dsl_v2/enrichment.rs                                  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              Stage 3: Semantic Validation (DB/gRPC)              │
│  Batch resolves EntityRefs via EntityGateway (~6x speedup)      │
│  EntityRef { resolved_key: Some(uuid) } = resolved              │
│  rust/src/dsl_v2/semantic_validator.rs                          │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              Stage 4: Compiler → Execution Plan                  │
│  rust/src/dsl_v2/execution_plan.rs                              │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│        Stage 5: GenericCrudExecutor (YAML-driven)                │
│  - Reads verb config from config/verbs/*.yaml                   │
│  - All 14 CRUD operations driven by YAML config                 │
│  - Custom ops via plugin pattern                                │
│  rust/src/dsl_v2/generic_executor.rs                            │
└─────────────────────────────────────────────────────────────────┘
```

### AST Types (rust/src/dsl_v2/ast.rs)

| Type | Description |
|------|-------------|
| `Program` | Root node containing statements |
| `Statement` | VerbCall or Comment |
| `VerbCall` | `(domain.verb :key value :as @binding)` |
| `Argument` | Key-value pair with span |
| `AstNode` | Literal, SymbolRef, EntityRef, List, Map, Nested |
| `EntityRef` | External reference needing resolution |
| `SymbolRef` | `@name` binding reference |
| `Span` | Source location (start, end byte offsets) |

**Key Design: EntityRef as Valid Intermediate State**

```rust
// Unresolved - valid state for draft saving
EntityRef { entity_type: "cbu", value: "Apex Fund", resolved_key: None }

// Resolved - ready for execution
EntityRef { entity_type: "cbu", value: "Apex Fund", resolved_key: Some("uuid...") }
```

This enables:
- Saving DSL with unresolved references (draft mode)
- Incremental resolution (resolve entities one at a time)
- Offline editing (parse/enrich without DB)

### Resolution Mode (YAML lookup config)

Each argument with a `lookup:` block can specify how the UI should resolve it:

```yaml
args:
  - name: jurisdiction
    lookup:
      entity_type: jurisdiction
      resolution_mode: reference  # < 100 items - autocomplete dropdown

  - name: cbu-id
    lookup:
      entity_type: cbu
      resolution_mode: entity     # growing table - search modal
```

| Mode | Use Case | UI Behavior |
|------|----------|-------------|
| `reference` | Roles, jurisdictions, currencies | Autocomplete dropdown |
| `entity` | CBUs, people, funds, cases | Search modal with refinement |

### Composite Search Keys (S-Expression Syntax)

For tables with 100k+ records (e.g., persons, companies), a simple name search returns too many matches. The `search_key` config uses **s-expression syntax** to define composite keys with discriminators:

```yaml
# Simple search key (just a column name - backwards compatible)
lookup:
  entity_type: cbu
  search_key: name
  primary_key: cbu_id

# Composite search key with discriminators (s-expression)
lookup:
  entity_type: proper_person
  search_key: "(search_name (date_of_birth :selectivity 0.95) (nationality :selectivity 0.7))"
  primary_key: entity_id
  resolution_mode: entity

# With global options
lookup:
  entity_type: entity
  search_key: "(name (jurisdiction :selectivity 0.8) :min-confidence 0.85 :tier composite)"
  primary_key: entity_id
```

**S-Expression Syntax**:

```
(primary_field discriminator1 discriminator2 ... :option value)

Where discriminator is either:
  - field_name                           ; simple field, default selectivity
  - (field_name :selectivity 0.95)       ; field with selectivity
  - (field_name :from-arg dob)           ; maps to DSL argument name
```

**Examples**:

| S-Expression | Meaning |
|--------------|---------|
| `name` | Simple search on name column |
| `(search_name date_of_birth)` | Search name, narrow by DOB |
| `(search_name (dob :selectivity 0.95))` | Name + DOB with 95% selectivity |
| `(name (nationality :selectivity 0.7) :min-confidence 0.9)` | Name + nationality, require 90% confidence |

**Resolution Tiers** (tried in order):

| Tier | Requires | Confidence | Performance |
|------|----------|------------|-------------|
| `exact` | source_system + source_id | 1.0 | O(1) |
| `composite` | name + discriminators | 0.95 | O(log n) |
| `contextual` | name + scope (CBU, case) | 0.85 | O(log n) |
| `fuzzy` | name only | varies | O(n) |

**Discriminator Selectivity**:
- 1.0 = unique identifier (source_id)
- 0.95 = nearly unique (name + dob)
- 0.7 = helpful but not unique (nationality)

### Search Engine Architecture

The EntityGateway includes a **mini DSL interpreter** for search queries. Search schemas are parsed from verb YAML, and runtime queries are s-expressions with filled-in values.

```
┌─────────────────────────────────────────────────────────────────┐
│                   Verb YAML (design time)                        │
│  search_key: "(search_name (dob :selectivity 0.95))"            │
└─────────────────────────────────────────────────────────────────┘
                              │ parse
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                   SearchSchema                                   │
│  primary_field: "search_name"                                   │
│  discriminators: [{field: "dob", selectivity: 0.95}]            │
│  rust/crates/entity-gateway/src/search_expr.rs                  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                   SearchQuery (runtime)                          │
│  primary_value: "John Smith"                                    │
│  discriminators: {"dob": "1980-01-15"}                          │
└─────────────────────────────────────────────────────────────────┘
                              │ execute
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                   SearchEngine                                   │
│  1. Search primary field in Tantivy index                       │
│  2. Score matches by discriminator similarity                   │
│  3. Return ranked results with confidence scores                │
│  rust/crates/entity-gateway/src/search_engine.rs                │
└─────────────────────────────────────────────────────────────────┘
```

**Key Types** (`rust/crates/entity-gateway/src/`):

| Type | File | Purpose |
|------|------|---------|
| `SearchExpr` | search_expr.rs | Parsed s-expression AST |
| `SearchSchema` | search_expr.rs | Schema definition from verb YAML |
| `SearchQuery` | search_expr.rs | Runtime query with values |
| `SearchEngine` | search_engine.rs | Interpreter that executes queries |
| `SearchResult` | search_engine.rs | Ranked matches with confidence |
| `RankedMatch` | search_engine.rs | Individual match with score |

**Search Algorithm**:
1. Parse schema from verb YAML `search_key` field
2. At runtime, receive query with primary value + optional discriminators
3. Search primary field using Tantivy fuzzy matching
4. For each match, compute discriminator scores
5. Combine scores: `final = primary_score * Π(1 - selectivity * (1 - match))`
6. Filter by min_confidence, return ranked results

## Centralized Entity Lookup Architecture

All entity lookup and resolution flows through the **EntityGateway** gRPC service. This ensures consistent fuzzy search behavior across the entire system.

```
┌─────────────────────────────────────────────────────────────────┐
│                    All Entity Lookups                            │
│  LSP Completions | Semantic Validator | Executor | Agent Routes │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                   EntityGateway (gRPC)                           │
│  rust/crates/entity-gateway/                                    │
│  Port 50051 | In-memory Tantivy indexes                         │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      PostgreSQL                                  │
│  Periodic refresh (300s) from reference tables                  │
└─────────────────────────────────────────────────────────────────┘
```

### Resolution Rule

| Input | Action |
|-------|--------|
| Have UUID primary key | Direct SQL lookup (no EntityGateway) |
| Have name/partial match | EntityGateway fuzzy search → UUID |

### Consumers

| Consumer | File | Usage |
|----------|------|-------|
| LSP Completions | `rust/crates/dsl-lsp/src/handlers/completion.rs` | Autocomplete for entity references |
| Semantic Validator | `rust/src/dsl_v2/semantic_validator.rs` | Resolve EntityRef values |
| Gateway Resolver | `rust/src/dsl_v2/gateway_resolver.rs` | CSG linter reference validation |
| Generic Executor | `rust/src/dsl_v2/generic_executor.rs` | Runtime entity lookup with SQL fallback |
| Agent Routes | `rust/src/api/agent_routes.rs` | Tool-use entity lookup |

### Batch Resolution Optimization

The semantic validator uses **batch resolution** for ~6x speedup:

```
Without batch: 30 refs × 1 gRPC call each = 30 round trips
With batch:    30 refs grouped by 5 RefTypes = 5 gRPC calls
```

**How it works:**
1. `validate()` calls `batch_resolve_all_refs()` after parsing
2. Collects all EntityRefs by RefType in single AST pass
3. Makes one gRPC `SearchRequest` per RefType (batch of values)
4. Stores results in `RefCache: HashMap<(RefType, String), ResolveResult>`
5. `validate_argument_value()` checks cache before individual `resolve()`

**Key types:**
```rust
pub type RefCache = HashMap<(RefType, String), ResolveResult>;
```

This optimization is transparent - validation semantics unchanged, just faster.

### Web UI Architecture

> **📘 See also:** `EGUI.md` for the full egui/WASM refactoring brief and implementation guide.

The web UI is split into three crates:
- `rust/crates/ob-poc-web/` - Axum server serving static files and API endpoints
- `rust/crates/ob-poc-ui/` - Pure egui/WASM application (main UI)
- `rust/crates/ob-poc-graph/` - Reusable graph widget (used by ob-poc-ui)

The UI uses a 4-panel layout with multiple layout modes (FourPanel, EditorFocus, GraphFocus, GraphFullSize).

**Key features:**
- 4 view modes: KYC_UBO, SERVICE_DELIVERY, CUSTODY, PRODUCTS_ONLY
- Multiple layout orientations
- Node drag/resize with layout persistence
- Entity search and resolution via EntityGateway
- YAML-driven token system for entity visual styling

### Token System

The UI uses a YAML-driven token configuration system for consistent entity visualization.

**Module:** `rust/crates/ob-poc-ui/src/tokens/`

| File | Purpose |
|------|---------|
| `types.rs` | Core types: TokenDefinition, TokenVisual, InteractionRules |
| `registry.rs` | TokenRegistry with YAML loading and alias resolution |
| `default_tokens.yaml` | Token definitions for all entity types |

**Key concepts:**
- **TokenDefinition**: Visual config (colors, icon, size) + interaction rules + detail template
- **TokenRegistry**: Loads from embedded YAML, supports type aliases (e.g., `proper_person` → `entity`)
- **TokenVisual**: 2D egui styling with status-based color mapping

**Usage:**
```rust
// Get token definition for an entity type
let token = state.token_registry.get("cbu");
let color = token.visual.status_color32(Some("ACTIVE"));
```

## Shared Types Crate (ob-poc-types)

The `ob-poc-types` crate is the **single source of truth** for all API types crossing HTTP boundaries.

### Type Boundaries

```
┌──────────────────┐         ┌──────────────────┐
│  Rust Server     │  JSON   │  TypeScript      │
│  (Axum)          │ ◄─────► │  (HTML panels)   │
└──────────────────┘         └──────────────────┘
         │
         │ JSON
         ▼
┌──────────────────┐
│  Rust WASM       │
│  (Graph)         │
└──────────────────┘

Plus: TS ◄──CustomEvent──► WASM (just entity IDs)
```

### Rules

1. **All API types live in `ob-poc-types`** - No inline struct definitions in handlers
2. **Server wins** - UI types must match what server sends, not the other way around
3. **Use `#[derive(TS)]`** for TypeScript generation via ts-rs
4. **Tagged enums only**: `#[serde(tag = "type")]` for TypeScript discriminated unions
5. **UUIDs as Strings** in API types for TypeScript compatibility
6. **CustomEvent payloads**: Keep simple - just IDs as strings

### Workflow

1. Edit types in `rust/crates/ob-poc-types/src/lib.rs`
2. Run `cargo test --package ob-poc-types export_bindings` to regenerate TypeScript
3. Copy bindings: `cp rust/crates/ob-poc-types/bindings/*.ts rust/crates/ob-poc-web/static/ts/generated/`
4. Fix lint: `cd rust/crates/ob-poc-web/static && npx eslint ts/generated/*.ts --fix`

### Key Types

| Type | Boundary | Usage |
|------|----------|-------|
| `CreateSessionRequest/Response` | Server↔TS | Session creation |
| `SessionStateResponse` | Server→TS | Session state with `active_cbu` and `bindings` |
| `BindEntityRequest/Response` | Server↔TS | Bind entity to session context |
| `BoundEntityInfo` | Server→TS | Entity info (id, name, entity_type) in session state |
| `ChatRequest/Response` | Server↔TS | Chat messages |
| `ChatStreamEvent` | Server→TS (SSE) | Streaming chat events |
| `ExecuteRequest/Response` | Server↔TS | DSL execution |
| `CbuGraphResponse` | Server↔WASM | Graph data |
| `LoadCbuEvent`, `EntitySelectedEvent` | TS↔WASM | CustomEvent payloads |

### Example: Adding a New API Type

```rust
// In ob-poc-types/src/lib.rs
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MyNewRequest {
    pub id: String,  // UUID as string for TS
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional_field: Option<String>,
}

// Tagged enum for TypeScript discrimination
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum MyEvent {
    Success { data: String },
    Error { message: String },
}
```

## Code Statistics

As of 2025-12-17:

| Language | Files | Lines |
|----------|-------|-------|
| Rust | 238 | ~106,000 |
| SQL | 11 | ~50,000 |
| YAML Config | 74 | ~16,000 |
| Markdown | 45 | ~25,000 |
| TypeScript/JS | 16 | ~3,700 |
| **Total** | **543** | **~200,000** |

### YAML-Driven Configuration

The DSL system is entirely YAML-driven. Adding new verbs requires editing YAML, not Rust code.

```
config/
├── verbs/                    # Verb definitions (split into multiple files)
│   ├── _meta.yaml           # Meta configuration
│   ├── cbu.yaml             # CBU domain verbs
│   ├── entity.yaml          # Entity domain verbs
│   ├── delivery.yaml        # Delivery domain verbs
│   ├── document.yaml        # Document domain verbs
│   ├── product.yaml         # Product domain verbs
│   ├── screening.yaml       # Screening domain verbs
│   ├── service.yaml         # Service domain verbs
│   ├── service-resource.yaml # Service resource verbs
│   ├── ubo.yaml             # UBO domain verbs
│   ├── custody/             # Custody-related domains
│   │   ├── cbu-custody.yaml # CBU custody operations
│   │   ├── entity-settlement.yaml
│   │   └── isda.yaml        # ISDA/CSA agreements
│   ├── kyc/                 # KYC case management
│   │   ├── kyc-case.yaml
│   │   ├── entity-workstream.yaml
│   │   ├── case-screening.yaml
│   │   ├── red-flag.yaml
│   │   ├── doc-request.yaml
│   │   └── case-event.yaml
│   ├── observation/         # Evidence model
│   │   ├── observation.yaml
│   │   ├── allegation.yaml
│   │   └── discrepancy.yaml
│   ├── registry/            # Investor registry
│   │   ├── share-class.yaml
│   │   ├── holding.yaml
│   │   └── movement.yaml
│   ├── reference/           # Market reference data
│   │   ├── market.yaml
│   │   ├── instrument-class.yaml
│   │   ├── security-type.yaml
│   │   └── subcustodian.yaml
│   └── refdata/             # Classification reference data
│       ├── jurisdiction.yaml
│       ├── currency.yaml
│       ├── role.yaml
│       ├── client-type.yaml
│       ├── case-type.yaml
│       ├── screening-type.yaml
│       ├── risk-rating.yaml
│       ├── settlement-type.yaml
│       └── ssi-type.yaml
└── csg_rules.yaml           # Context-sensitive grammar rules
```

**Key YAML structures:**
- `behavior: crud` → GenericCrudExecutor handles it
- `behavior: plugin` → Routes to custom_ops handler
- `maps_to:` → DSL arg name → DB column mapping
- `dynamic_verbs:` → Generated from DB tables (e.g., entity.create-*)

## Directory Structure

```
ob-poc/
├── rust/
│   ├── config/                     # YAML configuration (source of truth)
│   │   ├── verbs.yaml              # All verb definitions
│   │   └── csg_rules.yaml          # Validation rules
│   ├── src/
│   │   ├── ui/                     # Server-rendered UI (pages.rs, routes.rs)
│   │   ├── api/                    # REST API routes
│   │   │   ├── agent_routes.rs     # /api/agent/* (generate, validate)
│   │   │   ├── session_routes.rs   # /api/session/* (chat, execute)
│   │   │   └── template_routes.rs  # /api/templates/*
│   │   ├── dsl_v2/                 # Core DSL implementation
│   │   │   ├── parser.rs           # Nom-based S-expression parser
│   │   │   ├── ast.rs              # Program, Statement, VerbCall, Value
│   │   │   ├── config/             # YAML config types and loader
│   │   │   │   ├── types.rs        # Serde structs for verbs.yaml
│   │   │   │   └── loader.rs       # ConfigLoader (from env or path)
│   │   │   ├── runtime_registry.rs # RuntimeVerbRegistry (loads from YAML)
│   │   │   ├── verb_registry.rs    # UnifiedVerbRegistry (wraps runtime)
│   │   │   ├── entity_deps.rs      # Unified entity dependency DAG
│   │   │   ├── ops.rs              # Op enum (primitive operations)
│   │   │   ├── compiler.rs         # AST → Ops compiler
│   │   │   ├── dag.rs              # DAG builder + toposort
│   │   │   ├── diagnostics.rs      # Unified diagnostic types for LSP
│   │   │   ├── execution_result.rs # StepResult enum + ExecutionResults
│   │   │   ├── repl_session.rs     # REPL state with undo support
│   │   │   ├── planning_facade.rs  # Central analyse_and_plan() entrypoint
│   │   │   ├── generic_executor.rs # GenericCrudExecutor (14 CRUD ops)
│   │   │   ├── executor.rs         # DslExecutor (orchestrates execution)
│   │   │   ├── csg_linter.rs       # Context-sensitive validation
│   │   │   ├── execution_plan.rs   # AST → ExecutionPlan compiler
│   │   │   └── custom_ops/         # Plugin handlers for non-CRUD ops
│   │   ├── database/               # Repository pattern services
│   │   │   └── visualization_repository.rs  # Centralized visualization queries
│   │   ├── graph/                  # Graph visualization (single pipeline)
│   │   │   ├── builder.rs          # CbuGraphBuilder (multi-layer graph)
│   │   │   └── types.rs            # GraphNode, GraphEdge, CbuGraph
│   │   ├── domains/                # Domain-specific logic
│   │   ├── mcp/                    # MCP server for Claude Desktop
│   │   └── bin/
│   │       ├── agentic_server.rs   # Main server binary
│   │       ├── dsl_cli.rs          # CLI tool
│   │       └── dsl_mcp.rs          # MCP server binary
│   ├── crates/dsl-lsp/             # LSP server
│   └── tests/
│       ├── db_integration.rs       # Database integration tests
│       └── scenarios/              # DSL test scenarios (8 valid, 5 error)
├── docs/
│   └── DSL_TEST_SCENARIOS.md       # Test scenario documentation
├── schema_export.sql               # Full DDL for database rebuild
└── CLAUDE.md                       # This file
```

## Web UI Architecture

The UI uses a **hybrid architecture**: HTML/TypeScript panels for text content, WASM/egui for the interactive graph.

```
┌─────────────────────────────────────────────────────────────────┐
│                    ob-poc-web (Axum Server)                      │
│  Port 3000 - serves HTML + static files + API                   │
│  rust/crates/ob-poc-web/                                        │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────┬─────────────┐                                  │
│  │ Chat Panel  │ DSL Panel   │  ← HTML/TypeScript               │
│  │ (HTML/TS)   │ (HTML/TS)   │                                  │
│  ├─────────────┼─────────────┤                                  │
│  │ Graph       │ AST Panel   │  ← Graph is WASM/egui            │
│  │ (WASM)      │ (HTML/TS)   │                                  │
│  └─────────────┴─────────────┘                                  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    ob-poc-graph (WASM Component)                 │
│  Interactive CBU graph with drag/zoom                           │
│  rust/crates/ob-poc-graph/                                      │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│     /api/cbu/:id/graph?view_mode=KYC_UBO&orientation=VERTICAL   │
│  Returns graph with pre-computed x,y positions                  │
│  rust/src/api/graph_routes.rs                                   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    LayoutEngine + CbuGraphBuilder                │
│  Server-side layout and graph construction                      │
│  rust/src/graph/layout.rs, builder.rs                           │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      PostgreSQL                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Key Crates

| Crate | Purpose |
|-------|---------|
| `ob-poc-web` | Axum server: HTML pages, API routes, static files |
| `ob-poc-graph` | WASM/egui graph component embedded in HTML |
| `ob-poc-types` | Shared API types (single source of truth) |

### View Modes

| Mode | Layers Shown | Description |
|------|--------------|-------------|
| KYC_UBO | core, kyc, ubo | Entities, KYC status, ownership chains |
| SERVICE_DELIVERY | core, services | Entities + Products → Services → Resources |
| CUSTODY | core, custody | Markets, SSIs, Booking Rules |

### Layout Orientations

| Orientation | Description |
|-------------|-------------|
| VERTICAL | Top-to-bottom flow (default). Tiers flow downward, SHELL/PERSON split left/right |
| HORIZONTAL | Left-to-right flow. Tiers flow rightward, SHELL/PERSON split top/bottom |

### Key Design Principles

1. **Single Pipeline**: One endpoint (`/api/cbu/:id/graph`), one builder, server computes layout.

2. **Server-Side Layout**: `LayoutEngine` computes x, y positions based on view mode and orientation. UI just renders.

3. **Centralized DB Access**: All queries go through `VisualizationRepository`.

### Graph Layers

| Layer | Node Types | Description |
|-------|------------|-------------|
| core | cbu, entity | CBU and business entities with roles |
| kyc | verification, document | KYC status, document requests |
| ubo | entity (UBO-specific) | Ownership chains, control relationships |
| services | product, service, resource | Products → Services → Resource instances |

### Graph DSL Domain

The `graph.*` DSL verbs provide programmatic access to graph queries. Unlike CRUD verbs, graph verbs use `RuntimeBehavior::GraphQuery` and are executed by `GraphQueryExecutor`.

**Architecture:**

```
┌─────────────────────────────────────────────────────────────────┐
│                    DSL: (graph.view :cbu-id @cbu)               │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              GraphQueryExecutor (graph_executor.rs)              │
│  Routes to GraphQueryEngine based on GraphQueryOperation        │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              GraphQueryEngine (query_engine.rs)                  │
│  - execute_view, execute_focus, execute_filter                  │
│  - execute_path, execute_ancestors, execute_descendants         │
│  - execute_compare, execute_group_by, execute_find_connected    │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              GraphViewModel (view_model.rs)                      │
│  UI-ready output: nodes, edges, groups, paths, stats            │
└─────────────────────────────────────────────────────────────────┘
```

**Key Files:**

| File | Purpose |
|------|---------|
| `config/verbs/graph.yaml` | Graph verb definitions |
| `src/dsl_v2/graph_executor.rs` | DSL → GraphQueryEngine bridge |
| `src/graph/query_engine.rs` | Query execution (BFS/DFS, filtering) |
| `src/graph/view_model.rs` | GraphViewModel, GraphFilter, GraphPath |
| `src/graph/types.rs` | EdgeType, NodeType, LayerType enums |

**EdgeType Categories:**

| Category | Types |
|----------|-------|
| Core | HasRole |
| Ownership & Control | Owns, Controls, TrustSettlor, TrustTrustee, TrustBeneficiary, TrustProtector |
| Fund Structure | ManagedBy, AdministeredBy, CustodiedBy, UsesProduct, FeederTo, InvestsIn, Contains, ShareClassOf |
| Custody | RoutesTo, Matches, CoveredBy, SecuredBy, SettlesAt, SubcustodianOf |
| KYC | Requires, Validates, VerifiedBy, Contradicts |
| Services | Delivers, BelongsTo, ProvisionedFor |
| Delegation | DelegatesTo |

### Graph Visualization Modules

The graph visualization (`rust/crates/ob-poc-graph/`) includes several specialized modules:

| Module | File | Description |
|--------|------|-------------|
| `animation` | `animation.rs` | Spring physics (SpringF32, SpringVec2) for smooth 60fps transitions |
| `astronomy` | `astronomy.rs` | Universe ↔ Solar System view transitions with fade animations |
| `ontology` | `ontology.rs` | Entity type hierarchy browser with expand/collapse |
| `camera` | `camera.rs` | Camera2D with animated fly_to/zoom_to methods |
| `input` | `input.rs` | Mouse/keyboard input handling, scroll-to-zoom |
| `lod` | `lod.rs` | Level of detail rendering based on zoom level |

### Entity Type Ontology Browser

The `ontology` module provides a hierarchical view of entity types with counts:

```
ENTITY (root)
├── SHELL (Legal Vehicles)
│   ├── LIMITED_COMPANY, FUND, TRUST, PARTNERSHIP, LLC
├── PERSON (Natural Persons)
│   ├── PROPER_PERSON, UBO, CONTROL_PERSON
└── SERVICE_LAYER (Services)
    ├── PRODUCT, SERVICE, RESOURCE
```

**Key Types:**

| Type | Description |
|------|-------------|
| `EntityTypeOntology` | Complete type hierarchy with root `TypeNode` |
| `TypeNode` | Tree node with type_code, label, children, matching_entities, total_count |
| `TaxonomyState` | Expand/collapse state with spring animations |
| `TypeBrowserAction` | Actions: ToggleExpand, SelectType, FilterToType, ExpandAll, CollapseAll |

**Usage:**
```rust
// Create ontology and populate from graph
let mut ontology = EntityTypeOntology::new();
ontology.populate_counts(&layout_graph);

// Render browser (returns action, EGUI-RULES compliant)
let action = render_type_browser(ui, &ontology, &taxonomy_state, max_height);
match action {
    TypeBrowserAction::SelectType { type_code } => { /* highlight entities */ }
    TypeBrowserAction::FilterToType { type_code } => { /* filter view */ }
    _ => {}
}
```

## egui State Management & Best Practices

> **⛔ STOP: Read the 5 rules below before writing ANY egui/WASM code. Violations cause frozen UI, state drift, and impossible-to-debug bugs.**

### The 5 Non-Negotiable Rules (Quick Reference)

| Rule | Do This | NOT This |
|------|---------|----------|
| **1. No local state mirroring server data** | `AppState.session` (fetched) | `panel.messages: Vec<Message>` |
| **2. Actions return values, no callbacks** | `return Some(Action::Save)` | `self.save_data()` in button handler |
| **3. Short lock, then render** | Extract data, drop lock, then render | Hold lock during entire render |
| **4. Process async first, render second** | `process_async_results()` at top of `update()` | Check async mid-render |
| **5. Server round-trip for mutations** | POST → wait → refetch | Optimistic local update |

**If unsure:** Is this data from the server? → It goes in `AppState`, never mutated by UI.

The UI uses egui in immediate mode with server-first state management. These patterns are **mandatory** for all egui code in this project.

### Philosophy: Why These Patterns Exist

egui is an **immediate mode** GUI. This is fundamentally different from React, Vue, or any retained mode framework. Understanding this difference is critical.

**Retained Mode (React, etc.):**
```
Component Tree (persistent)
     │
     ├── State lives IN components
     ├── Changes trigger re-render of subtree
     ├── Virtual DOM diffs old vs new
     └── Minimal DOM updates applied
```

**Immediate Mode (egui):**
```
Every frame (60fps):
     │
     ├── Read current state
     ├── Paint everything from scratch
     ├── Check what user clicked/typed
     ├── Return interactions
     └── Caller decides what to do
     
     (No component tree. No persistence. No diffing.)
```

**Why this matters:**

1. **There are no components** - Functions paint UI and return. They don't "exist" between frames. Trying to give them persistent state creates bugs.

2. **State must live OUTSIDE the UI** - egui functions are pure: `(state) -> painted pixels + interactions`. State lives in your `App` struct, not in widgets.

3. **60fps means "just refetch" is cheap** - In React, you optimize to avoid re-renders. In egui, you're already re-rendering 60x/second. Fetching fresh server state fits naturally.

4. **Callbacks are an anti-pattern** - In React, `onClick={() => setState(...)}` is idiomatic. In egui, callbacks capture `&mut self` and create borrow checker nightmares. Return values are the answer.

5. **"Dirty" flags fight the model** - egui assumes everything redraws every frame. Adding dirty tracking means you're building a retained mode system on top of immediate mode. Just let it redraw.

**The mental model:**

```
┌─────────────────────────────────────────────────────────────────┐
│                         YOUR APP                                 │
│                                                                  │
│   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐        │
│   │   Server    │───▶│  AppState   │───▶│    egui     │        │
│   │   (truth)   │    │  (mirror)   │    │  (painter)  │        │
│   └─────────────┘    └─────────────┘    └─────────────┘        │
│         ▲                   │                   │               │
│         │                   │                   ▼               │
│         │                   │            ┌─────────────┐        │
│         │                   │            │  Response   │        │
│         │                   │            │  (clicks,   │        │
│         │                   │            │   typing)   │        │
│         │                   │            └──────┬──────┘        │
│         │                   │                   │               │
│         │                   ▼                   ▼               │
│         │            ┌─────────────────────────────┐            │
│         └────────────│      Action Handler         │            │
│                      │  (POST to server, refetch)  │            │
│                      └─────────────────────────────┘            │
└─────────────────────────────────────────────────────────────────┘

The loop:
1. Server has truth
2. AppState mirrors server (via fetch)
3. egui paints AppState
4. User interacts → Response
5. Handler POSTs to server
6. Refetch → AppState updated
7. Next frame paints new state
```

**Why server-first works perfectly with egui:**

| egui Reality | Server-First Response |
|--------------|----------------------|
| Redraws everything every frame | So "just fetch and paint" is natural |
| No component state | So state lives on server, no sync needed |
| Returns interactions, doesn't handle them | So POST to server, let IT handle logic |
| No diffing/optimization | So refetch entire state, paint it fresh |

**The key insight:** egui's "limitation" (no persistent state) becomes a strength when paired with server-first architecture. You CAN'T have state drift if the UI has no state. Every frame is a fresh, accurate render of server truth.

**When you're tempted to add local state, ask:**
- "Is this text the user is actively typing?" → Yes: TextBuffers (the ONE exception)
- "Is this data that came from the server?" → No local state. Fetch it.
- "Am I caching to avoid re-fetching?" → Don't. Let the server/EntityGateway cache.
- "Am I tracking dirty/changed flags?" → Don't. POST action, then refetch.

### Core Principle: Server is the Single Source of Truth

```
┌─────────────────────────────────────────────────────────────────┐
│                    APPROVED STATE MODEL                          │
├─────────────────────────────────────────────────────────────────┤
│  SERVER DATA (fetched via API, NEVER modified locally)          │
│  ├── session: Option<SessionStateResponse>                      │
│  ├── graph_data: Option<CbuGraphData>                           │
│  ├── validation: Option<ValidationResponse>                     │
│  └── execution: Option<ExecuteResponse>                         │
├─────────────────────────────────────────────────────────────────┤
│  UI-ONLY STATE (ephemeral, not persisted)                       │
│  ├── buffers: TextBuffers (draft text being edited)             │
│  ├── view_mode: ViewMode                                        │
│  ├── selected_entity_id: Option<String>                         │
│  └── camera: Camera2D (pan/zoom)                                │
├─────────────────────────────────────────────────────────────────┤
│  ASYNC COORDINATION                                              │
│  └── async_state: Arc<Mutex<AsyncState>>                        │
└─────────────────────────────────────────────────────────────────┘
```

### Anti-Patterns (NEVER DO)

```rust
// ❌ WRONG: Local state that mirrors server data
pub struct AppState {
    local_messages: Vec<Message>,           // NO - server owns messages
    is_dirty: bool,                         // NO - refetch instead
    cached_entities: HashMap<Uuid, Entity>, // NO - server owns entities
    last_known_cbu: Option<Cbu>,           // NO - fetch from session
}

// ❌ WRONG: Modifying server data locally
fn handle_new_message(&mut self, msg: Message) {
    self.session.messages.push(msg);  // NO - this will drift from server
}

// ❌ WRONG: Complex sync logic
if self.is_dirty && self.last_sync > 5.0 {
    self.sync_to_server();  // NO - creates race conditions
}
```

### Approved Patterns

#### Pattern 1: Action → Server → Refetch → Render

Every user action that changes data follows this flow:

```rust
// ✅ CORRECT: Server roundtrip for mutations
fn handle_execute(&mut self, ctx: &egui::Context) {
    let dsl = self.buffers.dsl_editor.clone();
    let async_state = Arc::clone(&self.async_state);
    let ctx = ctx.clone();
    
    // 1. Set loading state
    {
        let mut state = async_state.lock().unwrap();
        state.executing = true;
    }
    
    // 2. POST to server
    spawn_local(async move {
        let result = api::execute_dsl(&dsl).await;
        
        // 3. Store result for next frame
        if let Ok(mut state) = async_state.lock() {
            state.pending_execution = Some(result);
            state.executing = false;
        }
        
        // 4. Trigger repaint
        ctx.request_repaint();
    });
}

// 5. In update(), process pending and refetch dependents
fn process_pending(&mut self) {
    if let Some(result) = self.async_state.lock().unwrap().pending_execution.take() {
        self.execution = Some(result);
        self.refetch_session();  // Server tells us new state
        self.refetch_graph();    // Graph may have changed
    }
}
```

#### Pattern 2: TextBuffers are the ONLY Local Mutable State

```rust
// ✅ CORRECT: Explicit text buffer struct
#[derive(Default)]
pub struct TextBuffers {
    pub chat_input: String,      // Draft message being typed
    pub dsl_editor: String,      // DSL being edited
    pub search_query: String,    // Entity search input
    pub dsl_dirty: bool,         // For "unsaved changes" warning ONLY
}

// Usage in UI
fn chat_input(ui: &mut Ui, buffers: &mut TextBuffers) {
    let response = ui.text_edit_singleline(&mut buffers.chat_input);
    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        // Submit to server, then clear
        submit_chat(&buffers.chat_input);
        buffers.chat_input.clear();
    }
}
```

#### Pattern 3: Async State Coordination

```rust
// ✅ CORRECT: Centralized async state
#[derive(Default)]
pub struct AsyncState {
    // Pending results (written by spawn_local, read by update loop)
    pub pending_session: Option<Result<SessionStateResponse, String>>,
    pub pending_graph: Option<Result<CbuGraphData, String>>,
    pub pending_validation: Option<Result<ValidationResponse, String>>,
    pub pending_execution: Option<Result<ExecuteResponse, String>>,
    
    // Loading flags (for spinners)
    pub loading_session: bool,
    pub loading_graph: bool,
    pub loading_chat: bool,
    pub executing: bool,
    
    // Trigger flags (set by actions, processed ONCE in update loop)
    // These coordinate when to refetch - NOT dirty tracking
    pub needs_graph_refetch: bool,
    pub pending_cbu_id: Option<Uuid>,
    
    // Error display
    pub last_error: Option<String>,
}

// Process at start of each frame
fn process_async_results(&mut self) {
    let mut state = self.async_state.lock().unwrap();
    
    if let Some(result) = state.pending_session.take() {
        state.loading_session = false;
        match result {
            Ok(session) => {
                // Sync DSL editor if server has content and we're not dirty
                if let Some(ref dsl) = session.pending_dsl {
                    if !self.buffers.dsl_dirty {
                        self.buffers.dsl_editor = dsl.clone();
                    }
                }
                self.session = Some(session);
            }
            Err(e) => state.last_error = Some(e),
        }
    }
    
    // ... similar for other pending results
}

// ✅ CRITICAL: Central update loop processes trigger flags ONCE
fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    // STEP 1: Process any pending async results
    self.state.process_async_results();

    // STEP 2: Handle trigger flags (SINGLE CENTRAL PLACE)
    // All graph/session refetches happen here, AFTER all state changes
    if let Some(cbu_id) = self.state.take_pending_graph_refetch() {
        self.state.fetch_graph(cbu_id);
    }

    // STEP 3: Render panels (they return actions, don't mutate state directly)
    // ...
}

// Actions SET flags, they don't fetch directly
pub fn set_view_mode(&mut self, mode: ViewMode) {
    self.view_mode = mode;
    self.graph_widget.set_view_mode(mode);
    // Set flag - actual fetch happens in update() after all state changes
    if let Ok(mut state) = self.async_state.lock() {
        state.needs_graph_refetch = true;
    }
}

pub fn select_cbu(&mut self, cbu_id: Uuid) {
    self.session_id = Some(cbu_id);
    // Set flags - actual fetch happens in update()
    if let Ok(mut state) = self.async_state.lock() {
        state.pending_cbu_id = Some(cbu_id);
        state.needs_graph_refetch = true;
    }
}
```

#### Pattern 4: Graph Widget State Isolation

The graph widget owns ONLY rendering state, not business data:

```rust
// ✅ CORRECT: Widget owns camera/interaction, not data
pub struct CbuGraphWidget {
    // Rendering state (widget-owned)
    camera: Camera2D,
    input_state: InputState,
    
    // Data from server (set via set_data, never modified)
    raw_data: Option<CbuGraphData>,
    layout_graph: Option<LayoutGraph>,  // Computed from raw_data
    
    // View filtering (affects what's shown, not the data)
    view_mode: ViewMode,
}

impl CbuGraphWidget {
    // ✅ Data flows IN, never modified
    pub fn set_data(&mut self, data: CbuGraphData) {
        self.raw_data = Some(data);
        self.recompute_layout();  // Pure computation, no side effects
    }
    
    // ✅ Events flow OUT via return values, not callbacks
    pub fn selected_entity_changed(&mut self) -> Option<String> {
        self.input_state.take_selection_change()
    }
}
```

#### Pattern 5: No Callbacks, Use Return Values

```rust
// ❌ WRONG: Callback-based event handling
impl CbuGraphWidget {
    pub fn on_entity_selected(&mut self, callback: impl Fn(String)) {
        // Creates lifetime/ownership nightmares
    }
}

// ✅ CORRECT: Return value based
impl CbuGraphWidget {
    pub fn ui(&mut self, ui: &mut egui::Ui) -> GraphResponse {
        // ... render ...
        GraphResponse {
            selected_entity: self.input_state.take_selection(),
            hovered_entity: self.input_state.hovered.clone(),
            needs_repaint: self.camera_is_animating(),
        }
    }
}

// Caller handles the response
fn update(&mut self, ctx: &egui::Context) {
    let response = self.graph_widget.ui(ui);
    if let Some(entity_id) = response.selected_entity {
        self.selected_entity_id = Some(entity_id);
    }
}
```

#### Pattern 6: Loading States

```rust
// ✅ CORRECT: Explicit loading UI
fn render_panel(&mut self, ui: &mut egui::Ui) {
    let loading = self.async_state.lock().unwrap().loading_graph;
    
    if loading {
        ui.centered_and_justified(|ui| {
            ui.spinner();
            ui.label("Loading...");
        });
        return;
    }
    
    let Some(ref data) = self.graph_data else {
        ui.centered_and_justified(|ui| {
            ui.label("Select a CBU to view");
        });
        return;
    };
    
    // Render actual content
    self.render_graph(ui, data);
}
```

### File Organization

```
rust/crates/ob-poc-ui/src/
├── lib.rs              # WASM entry point only
├── app.rs              # App struct, update() loop, layout
├── state.rs            # AppState, AsyncState, TextBuffers (state definitions)
├── api.rs              # HTTP client (get, post, SSE helpers)
├── panels/             # UI panels (each receives &mut AppState)
│   ├── mod.rs
│   ├── chat.rs         # fn chat_panel(ui: &mut Ui, state: &mut AppState)
│   ├── dsl_editor.rs
│   ├── results.rs
│   └── toolbar.rs
└── widgets/            # Reusable widgets (pure, no AppState access)
    ├── mod.rs
    ├── status_badge.rs # fn kyc_badge(ui: &mut Ui, status: &KycStatus)
    └── message.rs      # fn render_message(ui: &mut Ui, msg: &Message)
```

### Key Rules Summary

| Rule | Rationale |
|------|-----------|
| Server owns all business data | Prevents drift, single source of truth |
| TextBuffers is the only local mutable state | Explicit about what UI owns |
| Actions go through server, then refetch | No local state sync bugs |
| Widgets return events, don't use callbacks | Simpler ownership, no lifetime issues |
| AsyncState coordinates all async ops | Centralized, predictable async handling |
| Loading states are explicit | User always knows what's happening |
| No `is_dirty` flags for sync | Refetch is simpler and more reliable |

### Common Mistakes and Fixes

| Mistake | Fix |
|---------|-----|
| Storing `Vec<Message>` locally | Fetch from `session.messages` each frame |
| `if changed { sync() }` patterns | POST action, then refetch |
| Callbacks for widget events | Return `Option<Event>` from widget |
| `Arc<Mutex<>>` everywhere | Only for `AsyncState`, everything else is owned |
| Caching entity lookups | Let server/EntityGateway handle caching |
| Local "optimistic updates" | Wait for server confirmation, show spinner |

### WASM/egui Debugging in Chrome

#### Essential Setup (MUST have in lib.rs)

```rust
#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    // Converts panics to readable stack traces in console
    console_error_panic_hook::set_once();
    
    // Routes tracing macros to browser console
    tracing_wasm::set_as_global_default();
    
    tracing::info!("WASM module initialized");
    Ok(())
}
```

Without `console_error_panic_hook`, panics show as cryptic "unreachable executed" errors.

#### Build for Debugging

```bash
# Development build with debug info and source maps
wasm-pack build --target web --dev

# This generates:
# - pkg/ob_poc_ui.wasm      (unoptimized, with debug info)
# - pkg/ob_poc_ui.wasm.map  (source map for Rust debugging)
```

**Never debug release builds** - optimizations break line correlation.

#### Chrome DevTools Setup

1. Open DevTools (F12)
2. Settings (⚙️) → Experiments → Enable **"WebAssembly Debugging: DWARF support"**
3. Restart DevTools
4. Sources panel → Add workspace folder (your project root)
5. `.rs` files appear under `wasm://` - set breakpoints directly in Rust

#### Tracing for State Debugging

Use `tracing`, not `println!` or `log::info!`:

```rust
use tracing::{debug, info, warn, error};

fn process_async_results(&mut self) {
    let state = self.async_state.lock().unwrap();
    
    // Structured logging with field inspection
    debug!(?state.loading_session, ?state.loading_graph, "async state check");
    
    if let Some(ref err) = state.last_error {
        error!(%err, "async operation failed");
    }
}
```

#### egui Built-in Debug Tools

```rust
// Toggle inspection UI with F1 (dev builds only)
fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    #[cfg(debug_assertions)]
    if ctx.input(|i| i.key_pressed(egui::Key::F1)) {
        self.show_debug = !self.show_debug;
    }
    
    #[cfg(debug_assertions)]
    if self.show_debug {
        egui::Window::new("Debug").show(ctx, |ui| {
            // Async state visibility
            let state = self.async_state.lock().unwrap();
            ui.label(format!("loading_session: {}", state.loading_session));
            ui.label(format!("loading_graph: {}", state.loading_graph));
            ui.label(format!("executing: {}", state.executing));
            if let Some(ref err) = state.last_error {
                ui.colored_label(egui::Color32::RED, err);
            }
            
            ui.separator();
            
            // egui's built-in inspection
            ctx.inspection_ui(ui);
        });
    }
}
```

#### Network Tab is Your Friend

With server-first architecture, **every user action should produce a network request**:

| User Action | Expected Network Activity |
|-------------|--------------------------|
| Select CBU | `GET /api/cbu/:id/graph` |
| Send chat | `POST /api/session/:id/chat` (or SSE stream) |
| Execute DSL | `POST /api/session/:id/execute` then refetch |
| Change view mode | `GET /api/cbu/:id/graph?view_mode=...` |

If state seems wrong:
1. Check Network tab - did the request fire?
2. Check response - is server returning expected data?
3. Check console - any tracing output showing state update?

#### Common WASM Debugging Issues

| Symptom | Likely Cause | Fix |
|---------|--------------|-----|
| "unreachable executed" | Panic without hook | Add `console_error_panic_hook::set_once()` |
| No `.rs` files in Sources | Missing source map | Build with `--dev`, check `.wasm.map` exists |
| Breakpoints don't hit | Release build or wrong workspace | Rebuild with `--dev`, add correct folder |
| State not updating | Async result not processed | Check `process_async_results()` called in `update()` |
| UI frozen | `lock().unwrap()` deadlock | Never hold lock across await points |
| "recursive mutex" | Locking same mutex twice | Extract data from lock, drop lock, then use data |

## Zed Extension (DSL Syntax Highlighting)

The project includes a Zed editor extension for DSL syntax highlighting located at `rust/crates/dsl-lsp/zed-extension/`.

### Extension Structure

```
rust/crates/dsl-lsp/zed-extension/
├── extension.toml          # Extension manifest
├── extension.wasm          # Compiled WASM extension
├── Cargo.toml              # Rust crate for extension logic
├── src/lib.rs              # Extension entry point
├── languages/dsl/
│   ├── config.toml         # Language configuration
│   ├── highlights.scm      # Syntax highlighting queries
│   └── indents.scm         # Indentation rules
└── grammars/               # Tree-sitter grammar (cloned by Zed)
```

### Installing the Dev Extension

1. Open Zed
2. Open Command Palette (`Cmd+Shift+P`)
3. Run "zed: install dev extension"
4. Select the `rust/crates/dsl-lsp/zed-extension/` directory
5. Files with `.dsl`, `.obl`, or `.onboard` extensions will now have syntax highlighting

### Key Configuration Files

**extension.toml** - Extension manifest:
```toml
id = "onboarding-dsl"
name = "Onboarding DSL"
version = "0.1.0"
schema_version = 1
languages = ["languages/dsl"]

[grammars.clojure]
repository = "https://github.com/sogaiu/tree-sitter-clojure"
rev = "e43eff80d17cf34852dcd92ca5e6986d23a7040f"
```

**languages/dsl/config.toml** - Language settings:
```toml
name = "DSL"
grammar = "clojure"
path_suffixes = ["dsl", "obl", "onboard"]
line_comments = [";"]
```

### Grammar Notes

The extension uses `tree-sitter-clojure` as the grammar since the DSL uses S-expression syntax similar to Clojure/Lisp. The `highlights.scm` file maps clojure node types to highlight groups:

- `sym_lit` → function names (verbs)
- `kwd_lit` → keywords (`:arg-name`)
- `str_lit` → strings
- `num_lit` → numbers
- `derefing_lit` → symbol references (`@name`)

### Troubleshooting

If the extension fails to load, check Zed logs:
```bash
tail -100 ~/Library/Logs/Zed/Zed.log | grep -i "dsl\|error\|language"
```

Common issues:
- **"failed to compile grammar"**: Delete `grammars/` directory and reinstall
- **"Invalid node type"**: `highlights.scm` or `indents.scm` uses wrong node names for the grammar
- **Language not recognized**: Check `path_suffixes` in `config.toml`

## EntityGateway (LSP Autocomplete Backend)

The EntityGateway is a gRPC service providing fast fuzzy search for LSP autocomplete. It replaces direct database lookups with an in-memory Tantivy index for sub-millisecond response times.

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     Zed Editor                                   │
│  User types: (cbu.ensure :jurisdiction "Lu                      │
└─────────────────────────────────────────────────────────────────┘
                              │ LSP completion request
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                   DSL Language Server                            │
│  rust/crates/dsl-lsp/                                           │
│  Maps keyword → EntityGateway nickname                          │
└─────────────────────────────────────────────────────────────────┘
                              │ gRPC SearchRequest
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                   EntityGateway Service                          │
│  rust/crates/entity-gateway/                                    │
│  Port: 50051 (default)                                          │
│  In-memory Tantivy indexes per entity type                      │
└─────────────────────────────────────────────────────────────────┘
                              │ Periodic refresh (300s)
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      PostgreSQL                                  │
│  Reference tables: roles, jurisdictions, currencies, etc.       │
└─────────────────────────────────────────────────────────────────┘
```

### Running EntityGateway

```bash
cd rust/crates/entity-gateway
DATABASE_URL="postgresql:///data_designer" cargo run --release
```

The service loads all configured entities from the database on startup and refreshes every 5 minutes.

### Configuration

**Config file**: `rust/crates/entity-gateway/config/entity_index.yaml`

Each entity defines:
- `nickname`: Lookup key used by LSP (e.g., "role", "jurisdiction")
- `source_table`: Database table to query
- `return_key`: Column to return as the token (UUID or code)
- `search_keys`: Columns to index for search
- `index_mode`: `trigram` (fuzzy substring) or `exact` (prefix match)
- `display_template`: How to format results (e.g., `{first_name} {last_name}`)

### Index Modes

| Mode | Use Case | Example |
|------|----------|---------|
| `trigram` | Names, descriptions | "gold" → "Goldberg, Sarah" |
| `exact` | Codes, enums | "dir" → "DIRECTOR" |

### Configured Entities (18 total)

**Trigram mode** (fuzzy name search):
- `person`, `legal_entity`, `entity`, `cbu`, `fund`, `product`, `service`

**Exact mode** (code/enum lookup):
- `role`, `jurisdiction`, `currency`, `client_type`, `case_type`
- `screening_type`, `risk_rating`, `settlement_type`, `ssi_type`
- `instrument_class`, `market`

### LSP Keyword Mapping

The LSP maps DSL keywords to EntityGateway nicknames:

| DSL Keyword | Nickname |
|-------------|----------|
| `:cbu-id` | cbu |
| `:entity-id`, `:owner-entity-id`, etc. | entity |
| `:role` | role |
| `:jurisdiction` | jurisdiction |
| `:currency`, `:cash-currency` | currency |
| `:client-type` | client_type |
| `:instrument-class` | instrument_class |
| `:market` | market |

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `ENTITY_GATEWAY_URL` | `http://[::1]:50051` | gRPC endpoint for LSP |
| `DATABASE_URL` | (required) | PostgreSQL connection string |

## Commands

### Layout Persistence

Users can customize node positions (drag) and sizes (shift+drag) in the graph visualization. These layout overrides are persisted per CBU and view mode.

**Database Table**: `"ob-poc".cbu_layout_overrides`

| Column | Type | Description |
|--------|------|-------------|
| cbu_id | UUID | CBU identifier |
| user_id | UUID | User identifier (default: nil UUID for shared) |
| view_mode | TEXT | "KYC_UBO" or "SERVICE_DELIVERY" |
| positions | JSONB | Array of `{node_id, dx, dy}` offsets from template |
| sizes | JSONB | Array of `{node_id, w, h}` size overrides |

**API Endpoints**:
- `GET /api/cbu/:id/layout?view_mode=KYC_UBO` - Fetch saved layout
- `POST /api/cbu/:id/layout?view_mode=KYC_UBO` - Save layout overrides

**UI Behavior**:
- Drag node: Moves node, stores offset from template base position
- Shift+drag node: Resizes node container
- Debounced save: Changes saved after 1 second of inactivity
- Race condition handling: UI waits for both graph AND layout to load before rendering

**Key Implementation Files**:
- `rust/src/database/visualization_repository.rs` - Layout CRUD operations
- `rust/src/api/graph_routes.rs` - Layout API endpoints
- `rust/crates/ob-poc-graph/src/graph/` - Graph widget with drag/resize handling

## xtask Development Automation

The `xtask` crate provides type-safe, cross-platform build automation. All commands are invoked via `cargo x <command>`.

### Quick Reference

```bash
cd rust/

# Development workflow
cargo x pre-commit          # Format + clippy + unit tests (fast)
cargo x check               # Compile + clippy + tests
cargo x check --db          # Include database integration tests

# Individual tasks
cargo x fmt                 # Format code
cargo x fmt --check         # Check formatting only
cargo x clippy              # Run clippy on all feature combinations
cargo x clippy --fix        # Auto-fix clippy warnings
cargo x test                # Run all tests
cargo x test --lib          # Unit tests only (faster)
cargo x test --filter foo   # Filter by test name

# Build
cargo x build               # Build all binaries (debug)
cargo x build --release     # Build all binaries (release)
cargo x wasm                # Build WASM components only
cargo x wasm --release      # Build WASM in release mode

# Deploy (recommended for UI development)
cargo x deploy              # Full deploy: WASM + server + start
cargo x deploy --release    # Release builds
cargo x deploy --skip-wasm  # Skip WASM rebuild (faster if only Rust changed)
cargo x deploy --no-run     # Build only, don't start server
cargo x deploy --port 8080  # Custom port

# Utilities
cargo x schema-export       # Export DB schema to schema_export.sql
cargo x ts-bindings         # Generate TypeScript bindings from ob-poc-types
cargo x dsl-tests           # Run DSL test scenarios
cargo x serve               # Start web server (port 3000)
cargo x serve --port 8080   # Custom port

# CI
cargo x ci                  # Full pipeline: fmt, clippy, test, build
```

### Available Commands

| Command | Description |
|---------|-------------|
| `check` | Compile + clippy + tests (add `--db` for integration tests) |
| `clippy` | Run clippy on all feature combinations (database, server, mcp, cli) |
| `test` | Run tests (`--lib` for unit only, `--db` for integration, `--filter` to match) |
| `fmt` | Format code (`--check` to verify only) |
| `build` | Build all binaries (`--release` for optimized) |
| `wasm` | Build WASM component (ob-poc-ui) to static/wasm/ |
| `deploy` | Full deploy: WASM + server build + start (`--skip-wasm`, `--no-run`, `--release`) |
| `serve` | Start web server (`--port` to customize) |
| `schema-export` | Export database schema to `schema_export.sql` |
| `ts-bindings` | Generate TypeScript bindings from ob-poc-types |
| `dsl-tests` | Run DSL test scenarios via `tests/scenarios/run_tests.sh` |
| `ci` | Full CI pipeline: format check, clippy, tests, build |
| `pre-commit` | Fast pre-commit hook: format, clippy, unit tests |
| `batch-import` | Import Allianz funds as CBUs using template pipeline |
| `batch-clean` | Delete Allianz CBUs via cascade delete DSL verb |
| `gleif-import` | Import funds from GLEIF API by search term (`--search`, `--limit`, `--dry-run`) |

## Allianz Batch Import Test Case

This is a **real-world production test** of the full DSL template → execution pipeline using 177 Luxembourg-domiciled Allianz funds.

### Overview

The test validates the complete onboarding pipeline:
1. **Template expansion** - `onboard-fund-cbu.yaml` template with shared + batch params
2. **DSL generation** - Template → DSL source text with parameter substitution
3. **DSL execution** - Parse → Compile → Execute via GenericCrudExecutor
4. **Role assignment** - Each CBU gets 3 roles: ASSET_OWNER, MANAGEMENT_COMPANY, INVESTMENT_MANAGER
5. **Cascade delete** - Full cleanup via `cbu.delete-cascade` plugin handler

### Data Source

Scraped from Allianz Global Investors Luxembourg fund registry:
- **Source file**: `scrapers/allianz/output/allianz-lu-2025-12-17.json`
- **Seed SQL**: `data/allianzgi_seed/seed.sql`
- **Entity count**: 178 entities (177 funds + 1 ManCo)

### Commands

```bash
cd rust/

# Full import cycle (177 funds → 177 CBUs with roles)
cargo x batch-import

# Limited test run
cargo x batch-import --limit 5

# Dry run - show DSL without executing
cargo x batch-import --dry-run --verbose

# Clean all Allianz CBUs
cargo x batch-clean

# Clean with limit
cargo x batch-clean --limit 10

# Dry run cleanup
cargo x batch-clean --dry-run

# GLEIF API import (fetch funds from GLEIF by search term)
cargo x gleif-import --search "Allianz Global Investors"

# GLEIF import with limit
cargo x gleif-import --search "Allianz Global Investors" --limit 10

# GLEIF dry run
cargo x gleif-import --search "Allianz Global Investors" --dry-run
```

### GLEIF Import

The `gleif-import` command fetches fund data from the GLEIF API and imports into `entity_funds`:

| Feature | Description |
|---------|-------------|
| API pagination | Fetches all pages (100 records per page) |
| Upsert by LEI | Updates existing records, creates new ones |
| Name matching | Matches existing entities by name to backfill LEI |
| GLEIF metadata | Stores status, category, registration, addresses |

**Example run:**
```
Fetched 334 records from GLEIF API
Matched existing: 134
Created new: 196
Updated with GLEIF: 134
```

### Performance Results

| Operation | Count | Time | Rate |
|-----------|-------|------|------|
| Full import | 177 CBUs | 1.64s | 108 CBUs/sec |
| Full cleanup | 177 CBUs | ~3s | ~60 CBUs/sec |

### What Gets Created

For each Allianz fund entity, the template creates:

```clojure
;; 1. CBU with fund name and jurisdiction
(cbu.ensure :name "$fund_entity.name" :jurisdiction "LU" :client-type "fund" :as @cbu)

;; 2. ASSET_OWNER role (fund owns itself)
(cbu.assign-role :cbu-id @cbu :entity-id "$fund_entity" :role "ASSET_OWNER")

;; 3. MANAGEMENT_COMPANY role (shared ManCo)
(cbu.assign-role :cbu-id @cbu :entity-id "$manco_entity" :role "MANAGEMENT_COMPANY")

;; 4. INVESTMENT_MANAGER role (shared IM)
(cbu.assign-role :cbu-id @cbu :entity-id "$im_entity" :role "INVESTMENT_MANAGER")
```

### Key Files

| File | Purpose |
|------|---------|
| `rust/src/bin/batch_test_harness.rs` | CLI for batch template execution |
| `rust/config/verbs/templates/fund/onboard-fund-cbu.yaml` | Template definition |
| `rust/src/templates/expander.rs` | Template param substitution (including `$param.property`) |
| `rust/src/dsl_v2/custom_ops/cbu_ops.rs` | `cbu.delete-cascade` handler |
| `data/allianzgi_seed/seed.sql` | Entity seed data |

### Verification Queries

After import:
```sql
-- Count CBUs created
SELECT COUNT(*) FROM "ob-poc".cbus WHERE name ILIKE 'Allianz%';
-- Expected: 177

-- Check role distribution
SELECT r.name, COUNT(*) 
FROM "ob-poc".cbu_entity_roles cer
JOIN "ob-poc".roles r ON cer.role_id = r.role_id
JOIN "ob-poc".cbus c ON cer.cbu_id = c.cbu_id
WHERE c.name ILIKE 'Allianz%'
GROUP BY r.name;
-- Expected: ASSET_OWNER=177, MANAGEMENT_COMPANY=177, INVESTMENT_MANAGER=177
```

### Why This Test Matters

1. **Real data scale** - 177 entities is realistic for a fund manager's book
2. **Template pipeline validation** - Proves template → DSL → execution works end-to-end
3. **No direct DB access** - All operations go through DSL verbs (YAML-driven)
4. **Repeatable** - Clean + reimport cycle validates idempotency
5. **Performance baseline** - ~100 CBUs/sec is the benchmark for batch operations

### Manual Commands (Legacy)

```bash
cd rust/

# Build
cargo build -p ob-poc-web                                 # Web server
cargo build --features cli,database --bin dsl_cli         # CLI tool
cargo build --features database                            # DSL library only
cargo build --features mcp --bin dsl_mcp                  # MCP server

# Run web server (requires DATABASE_URL)
DATABASE_URL="postgresql:///data_designer" \
cargo run -p ob-poc-web
# Open http://localhost:3000

# Test
cargo test --features database --lib                  # Unit tests (~273)
cargo test --features database --test db_integration  # DB tests
./tests/scenarios/run_tests.sh                        # DSL scenarios
./tests/mcp_test.sh                                   # MCP protocol tests

# Clippy (all features)
cargo clippy --features server
cargo clippy --features database
cargo clippy --features mcp
```

## Tracing / Debug Logging

The DSL executor supports structured logging via the `tracing` crate. Logging is **off by default**.

```bash
# Debug level - shows step execution, verb routing, SQL queries
RUST_LOG=ob_poc::dsl_v2=debug ./target/debug/dsl_cli execute -f file.dsl

# Trace level - includes SQL bind values and row counts
RUST_LOG=ob_poc::dsl_v2=trace ./target/debug/dsl_cli execute -f file.dsl

# Save trace output to file
RUST_LOG=ob_poc::dsl_v2=debug ./target/debug/dsl_cli execute -f file.dsl 2> trace.log
```

| Level | Output |
|-------|--------|
| `info` | Config loading, high-level events |
| `debug` | Step execution, verb routing, generated SQL |
| `trace` | SQL bind values, row counts (very verbose) |

## Error Handling Guidelines

The DSL execution pipeline uses proper `Result` error handling throughout. **Never use `.unwrap()` or `.expect()` in production code paths** - these cause server panics.

### Panic-Free Patterns

| Pattern | Use Case |
|---------|----------|
| `?` operator | Propagate errors up the call stack |
| `.ok_or_else(\|\| anyhow!(...))` | Convert Option to Result with context |
| `let Some(x) = ... else { continue }` | Skip missing items in loops |
| `.unwrap_or_else(\|_\| default)` | Provide fallback for non-critical values |
| `match` / `if let` | Explicit handling of all cases |

### Audited Areas (Panic-Free)

These production code paths have been audited and are panic-free:

| Module | Status |
|--------|--------|
| `main.rs` (server startup) | ✅ Returns Result, graceful errors |
| `generic_executor.rs` | ✅ All lookups return Result |
| `custom_ops/*.rs` | ✅ Plugin handlers return Result |
| `trading_profile.rs` | ✅ Uses `let Some() else` pattern |
| `ubo_analysis.rs` | ✅ BigDecimal from constant, no parse |
| `semantic_context.rs` | ✅ Valid placeholder URLs |
| `csg_linter.rs` | ✅ Valid placeholder URLs |

### Acceptable `.unwrap()` Locations

- **Test code** (`#[test]`, `#[cfg(test)]`) - panics are expected for failures
- **Static constants** with `.expect("static value")` - compile-time provable
- **After explicit check** - `if x.is_none() { return } x.unwrap()` (prefer `let Some()`)

## DSL CLI (dsl_cli)

The CLI provides headless access to the full DSL pipeline, including AI-powered generation.

### Build

```bash
cd rust/
cargo build --features cli,database --bin dsl_cli --release
```

### Commands Overview

| Command | Description |
|---------|-------------|
| `generate` | Generate DSL from natural language using Claude AI |
| `custody` | Generate custody onboarding DSL (agentic workflow with pattern classification) |
| `parse` | Parse DSL source into AST (no validation) |
| `validate` | Validate DSL source (parse + CSG lint) |
| `plan` | Compile DSL to execution plan (parse + lint + compile) |
| `execute` | Execute DSL against the database |
| `verbs` | List available verbs and their schemas |
| `examples` | Show example DSL programs |
| `demo` | Run a built-in demo scenario |

### Global Options

```bash
-o, --format <FORMAT>  # Output format: json, text, pretty (default)
-q, --quiet            # Suppress non-essential output
```

### Generate Command (AI-Powered)

Generate DSL from natural language instructions using Claude AI.

```bash
# Basic generation
dsl_cli generate -i "Create a fund called Pacific Growth in Luxembourg"

# Generate and execute immediately
dsl_cli generate -i "Onboard Apex Capital as a US hedge fund" --execute

# Generate and save to file
dsl_cli generate -i "Create corporate with John Smith as UBO" -o output.dsl

# Focus on specific domain
dsl_cli generate -i "Provision custody account" --domain service-resource

# JSON output for scripting
dsl_cli generate -i "Create a trust in Jersey" --format json

# Pipe instruction from stdin
echo "Create a fund in Ireland" | dsl_cli generate
```

**Options:**
- `-i, --instruction <TEXT>` - Natural language instruction (or reads from stdin)
- `--execute` - Execute generated DSL after validation
- `--db-url <URL>` - Database URL (required with --execute, or use DATABASE_URL env)
- `--domain <DOMAIN>` - Focus generation on specific domain (cbu, entity, service-resource, etc.)
- `-o, --output <FILE>` - Save generated DSL to file

**Environment Variables:**
- `ANTHROPIC_API_KEY` - Required for generation
- `DATABASE_URL` - Required for --execute

### Validate Command

Validate DSL syntax and semantics without execution.

```bash
# Validate from file
dsl_cli validate -f program.dsl

# Validate from stdin
echo '(cbu.ensure :name "Test" :jurisdiction "US")' | dsl_cli validate

# With context
dsl_cli validate -f program.dsl --client-type fund --jurisdiction LU

# JSON output
dsl_cli validate -f program.dsl --format json
```

### Plan Command

Compile DSL to execution plan (shows what would execute).

```bash
# Show execution plan
dsl_cli plan -f program.dsl

# JSON output for inspection
dsl_cli plan -f program.dsl --format json
```

### Execute Command

Execute DSL against the database.

```bash
# Execute DSL file
dsl_cli execute -f program.dsl --db-url postgresql:///data_designer

# Dry run (show plan without executing)
dsl_cli execute -f program.dsl --dry-run

# Execute from stdin
echo '(cbu.ensure :name "Test Fund" :jurisdiction "LU" :client-type "fund")' | \
  dsl_cli execute --db-url postgresql:///data_designer

# JSON output with results
dsl_cli execute -f program.dsl --format json
```

### Verbs Command

List available DSL verbs.

```bash
# List all verbs
dsl_cli verbs

# Filter by domain
dsl_cli verbs --domain cbu
dsl_cli verbs --domain entity
dsl_cli verbs --domain service-resource

# Verbose with full schema
dsl_cli verbs --domain cbu --verbose

# JSON output
dsl_cli verbs --format json
```

### Examples Command

Show example DSL programs.

```bash
# All examples
dsl_cli examples

# By category
dsl_cli examples onboarding
dsl_cli examples documents
dsl_cli examples entities
dsl_cli examples custody
```

### Full Pipeline Example

```bash
# 1. Generate DSL from natural language
dsl_cli generate -i "Onboard Pacific Fund as a Luxembourg fund with custody account" -o pacific.dsl

# 2. Validate the generated DSL
dsl_cli validate -f pacific.dsl

# 3. View execution plan
dsl_cli plan -f pacific.dsl

# 4. Execute (dry run first)
dsl_cli execute -f pacific.dsl --dry-run

# 5. Execute for real
dsl_cli execute -f pacific.dsl

# Or do it all in one command:
dsl_cli generate -i "Onboard Pacific Fund as a Luxembourg fund" --execute
```

### Scripting with JSON Output

```bash
# Generate and parse with jq
dsl_cli generate -i "Create a fund" --format json | jq '.dsl'

# Check if execution succeeded
dsl_cli execute -f program.dsl --format json | jq '.success'

# Get created bindings
dsl_cli execute -f program.dsl --format json | jq '.bindings'
```

## API Endpoints

| Endpoint | Description |
|----------|-------------|
| `GET /` | Agent session UI |
| `GET /verbs` | Verb reference page |
| `POST /api/agent/generate` | Generate DSL from natural language |
| `POST /api/agent/generate-with-tools` | Generate DSL with Claude tool_use (looks up real IDs) |
| `POST /api/agent/validate` | Validate DSL syntax/semantics |
| `POST /api/session` | Create new session |
| `GET /api/session/:id` | Get session state (includes active_cbu, bindings) |
| `POST /api/session/:id/chat` | Send chat message |
| `POST /api/session/:id/bind` | Bind entity to session (e.g., active CBU) |
| `POST /api/session/:id/execute` | Execute DSL |
| `GET /api/templates` | List templates |
| `GET /api/dsl/list` | List DSL instances |

### Tool-Use Generation Endpoint

The `/api/agent/generate-with-tools` endpoint uses Claude's tool calling feature to look up real database entities before generating DSL. This prevents UUID hallucination.

**Available tools:**
- `lookup_cbu` - Find CBU by name
- `lookup_entity` - Find entity by name
- `lookup_product` - Find product by name  
- `list_cbus` - List all CBUs

**Example:**
```bash
curl -X POST http://localhost:3000/api/agent/generate-with-tools \
  -H "Content-Type: application/json" \
  -d '{"instruction": "Add Custody product to Apex Capital"}'
```

Claude will:
1. Call `lookup_cbu` with "Apex Capital" to verify it exists
2. Generate DSL using the confirmed CBU name

### Batch Operations Endpoint

The `/api/batch/add-products` endpoint provides server-side DSL generation for bulk operations. This is the "flexible macro" pattern - agent issues high-level commands, server handles deterministic DSL generation.

**When to use batch endpoints vs templates:**
- **Templates:** Complex multi-step workflows with business logic, needs human review
- **Batch endpoints:** Deterministic, repetitive operations (single verb × N items)

**Example:**
```bash
curl -X POST http://localhost:3000/api/batch/add-products \
  -H "Content-Type: application/json" \
  -d '{"cbu_ids": ["uuid1", "uuid2", ...], "products": ["CUSTODY", "FUND_ACCOUNTING"]}'
```

**Response:**
```json
{
  "total_operations": 354,
  "success_count": 354,
  "failure_count": 0,
  "duration_ms": 638,
  "results": [{"cbu_id": "...", "product": "CUSTODY", "success": true, "services_added": 19}, ...]
}
```

**Performance:** 354 operations in ~640ms (vs ~15 minutes with sequential LLM calls).

The server generates DSL like `(cbu.add-product :cbu-id "uuid" :product "CODE")` for each combination, executes via the standard DSL pipeline, and returns aggregated results.

### Session Management Architecture

Sessions are managed server-side with a shared in-memory store. The UI sends state changes to the server and receives updates.

```
┌─────────────────────────────────────────────────────────────────┐
│                    Browser (TypeScript)                          │
│  - ChatPanel creates session on init                            │
│  - App binds CBU selection to session                           │
│  - Auto-recovery on 404 (server restart)                        │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Shared SessionStore                           │
│  Arc<RwLock<HashMap<Uuid, AgentSession>>>                       │
│  - Single store shared across all routers                       │
│  - agent_routes and web app use same store                      │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    AgentSession                                  │
│  - messages: Vec<Message>                                       │
│  - context.active_cbu: Option<BoundEntity>                      │
│  - context.bindings: HashMap<String, BoundEntity>               │
│  - pending/ast state for DSL execution                          │
└─────────────────────────────────────────────────────────────────┘
```

**Key endpoints:**

| Endpoint | Purpose |
|----------|---------|
| `POST /api/session` | Create new session, returns `session_id` |
| `GET /api/session/:id` | Get session state with `active_cbu` and `bindings` |
| `POST /api/session/:id/bind` | Bind entity (CBU, etc.) to session context |
| `POST /api/session/:id/chat` | Chat with agent (uses `active_cbu` in prompt) |

**Session auto-recovery:** When the UI gets a 404 on bind (e.g., after server restart), it automatically recreates the session and retries.

**Active CBU context:** When a CBU is bound to the session, the agent receives it in the system prompt, enabling context-aware responses like "Add a director to [CBU Name]".

## DSL Syntax

```clojure
;; Create a CBU and bind to @fund
(cbu.ensure :name "Acme Fund" :jurisdiction "LU" :client-type "FUND" :as @fund)

;; Create entities with type-specific verbs
(entity.create-limited-company :name "Acme Holdings Ltd" :jurisdiction "LU" :as @company)
(entity.create-proper-person :first-name "John" :last-name "Smith" :date-of-birth "1980-01-15" :as @john)

;; Assign roles to link entities to CBU
(cbu.assign-role :cbu-id @fund :entity-id @john :role "DIRECTOR")
(cbu.assign-role :cbu-id @fund :entity-id @company :role "PRINCIPAL")

;; Document operations
(document.catalog :cbu-id @fund :doc-type "PASSPORT" :title "John Smith Passport")

;; KYC Case with workstreams and screenings
(kyc-case.create :cbu-id @fund :case-type "NEW_CLIENT" :as @case)
(entity-workstream.create :case-id @case :entity-id @john :as @ws-john)
(entity-workstream.create :case-id @case :entity-id @company :as @ws-company)
(case-screening.run :workstream-id @ws-john :screening-type "PEP")
(case-screening.run :workstream-id @ws-company :screening-type "SANCTIONS")

;; Service resource instance lifecycle
(service-resource.provision :cbu-id @fund :resource-type "CUSTODY_ACCOUNT" :instance-url "https://..." :as @account)
(service-resource.set-attr :instance-id @account :attr "account_number" :value "ACC-12345")
(service-resource.activate :instance-id @account)
```

### LookupRef Triplet Pattern

For arguments that reference existing database entities, the DSL supports a **triplet pattern** that captures the entity type, human-readable search key, and resolved primary key:

```clojure
;; Triplet syntax: (ref_type search_key primary_key)
;; - ref_type: Entity type from verb YAML definition (e.g., "proper_person", "role", "jurisdiction")
;; - search_key: Human-readable identifier displayed in UI
;; - primary_key: Resolved UUID or code (or nil if unresolved)

;; Example: Resolved entity reference
(cbu.assign-role :entity-id ("proper_person" "John Smith" "550e8400-e29b-41d4-a716-446655440000"))

;; Example: Unresolved reference (needs resolution via EntityGateway)
(cbu.assign-role :entity-id ("proper_person" "John Smith" nil))

;; Example: Reference data (codes instead of UUIDs)
(cbu.assign-role :role ("role" "DIRECTOR" "DIRECTOR"))
```

**How it works:**
1. **UI Autocomplete**: User types partial name → EntityGateway fuzzy search → returns matches
2. **Selection**: User selects match → UI stores triplet with resolved primary_key
3. **Validation**: On reload, semantic validator confirms primary_key still exists
4. **Execution**: Executor uses primary_key for database operations

**Verb YAML configuration** drives the expected `entity_type` for each argument:

```yaml
args:
  - name: entity-id
    type: uuid
    required: true
    maps_to: entity_id
    lookup:
      table: entities
      schema: ob-poc
      entity_type: entity        # ← Becomes ref_type in triplet
      search_key: name
      primary_key: entity_id
```

**Supported entity types**: `cbu`, `entity`, `proper_person`, `limited_company`, `product`, `service`, `document`, `role`, `jurisdiction`, `currency`, `kyc_case`, `workstream`, `share_class`, `holding`, `movement`, `ssi`, `market`, `instrument_class`, etc.

## Verb Domains

| Domain | Purpose |
|--------|---------|
| cbu | Client Business Unit lifecycle (ensure, assign-role, etc.) |
| entity | Dynamic verbs from entity_types (create-proper-person, create-limited-company) |
| document | Document catalog, request, extract, extract-to-observations |
| screening | Legacy PEP, sanctions checks (use case-screening instead) |
| kyc | Legacy KYC verbs (use kyc-case domain instead) |
| ubo | Ownership chains, control relationships, UBO registry |
| service-resource | Service resource type CRUD + instance provision, set-attr, activate, suspend, decommission |
| delivery | Service delivery record, complete, fail |
| cbu-custody | Custody & settlement: universe, SSI, booking rules |
| share-class | Fund share class master data (ISIN, NAV, fees, liquidity) |
| holding | Investor positions in share classes |
| movement | Subscription, redemption, transfer transactions |
| kyc-case | KYC case lifecycle (create, status, escalate, close) |
| entity-workstream | Per-entity workstream within KYC case |
| red-flag | Risk indicators and issues (raise, mitigate, waive) |
| doc-request | Document collection and verification |
| case-screening | Screenings within KYC workstreams |
| case-event | Audit trail for case activities |
| allegation | Client allegations - unverified claims that start KYC |
| observation | Attribute observations from various sources |
| discrepancy | Conflicts between attribute observations |
| threshold | Risk-based document requirements (derive, evaluate, check-entity) |
| rfi | Request for Information batch operations (generate, check-completion, list-by-case) |
| product | Product catalog CRUD (create, update, list) |
| service | Service catalog CRUD (create, update, list) |
| instrument-class | CFI-based instrument classification reference data |
| security-type | SMPG/ALERT security type taxonomy |
| market | ISO 10383 MIC market reference data |
| subcustodian | Subcustodian network relationships |
| isda | ISDA master agreements and product coverage |
| entity-settlement | Entity BIC/LEI settlement identity |
| fund | Fund structure operations (umbrella, subfund, share class, master-feeder, FoF) |
| control | Control relationships distinct from ownership (voting, board, veto) |
| delegation | Service provider delegation chains (ManCo to sub-advisor) |

## KYC Case Management DSL

The KYC case management system provides a complete workflow for client onboarding and periodic review, with automatic rule-based risk detection.

### Case State Machine

```
INTAKE → DISCOVERY → ASSESSMENT → REVIEW → APPROVED/REJECTED
                                    ↓
                                 BLOCKED (if hard stops)
```

### Entity Workstream States

```
PENDING → COLLECT → VERIFY → SCREEN → ASSESS → COMPLETE
                                 ↓
                          ENHANCED_DD (if PEP/high-risk)
                                 ↓
                              BLOCKED (if sanctions match)
```

### KYC Case Verbs

| Verb | Description |
|------|-------------|
| `kyc-case.create` | Create new KYC case for a CBU |
| `kyc-case.update-status` | Update case status |
| `kyc-case.escalate` | Escalate to higher authority |
| `kyc-case.assign` | Assign analyst/reviewer |
| `kyc-case.set-risk-rating` | Set case risk rating |
| `kyc-case.close` | Close case (approved/rejected/withdrawn) |

### Entity Workstream Verbs

| Verb | Description |
|------|-------------|
| `entity-workstream.create` | Create workstream for entity |
| `entity-workstream.update-status` | Update workstream status |
| `entity-workstream.block` | Block with reason |
| `entity-workstream.complete` | Mark as complete |
| `entity-workstream.set-enhanced-dd` | Flag for enhanced due diligence |
| `entity-workstream.set-ubo` | Mark entity as UBO |

### Red Flag Verbs

| Verb | Description |
|------|-------------|
| `red-flag.raise` | Raise new red flag |
| `red-flag.mitigate` | Mark as mitigated |
| `red-flag.waive` | Waive with justification |
| `red-flag.dismiss` | Dismiss as false positive |
| `red-flag.set-blocking` | Set as blocking the case |

### Rules Engine

The KYC system includes a YAML-driven rules engine that automatically triggers actions based on events.

**Configuration**: `rust/config/rules.yaml`

**Supported Events**: `workstream.created`, `screening.completed`, `doc-request.received`, `red-flag.raised`, `case.created`, `scheduled`

**Action Types**: `raise-red-flag`, `block-workstream`, `escalate-case`, `set-enhanced-dd`, `require-document`, `log-event`

### KYC Schema (kyc.* tables)

| Table | Purpose |
|-------|---------|
| cases | Main KYC case for a CBU |
| entity_workstreams | Per-entity work items within case |
| red_flags | Risk indicators and issues |
| doc_requests | Document requirements per workstream |
| screenings | Sanctions/PEP/adverse media checks |
| case_events | Audit trail of all activities |
| rule_executions | Audit log of rule engine runs |


## Adversarial Verification Module

The verification module implements a game-theoretic "Trust But Verify → Distrust And Verify" model where every piece of information is treated as a CLAIM that must be VERIFIED. The standard: "Would this process catch a sophisticated liar?"

### Architecture

```
rust/src/verification/
├── mod.rs              # Module exports
├── types.rs            # Claim, Evidence, Inconsistency types
├── confidence.rs       # ConfidenceCalculator with weighted aggregation
├── patterns.rs         # PatternDetector (circular ownership, layering, nominees)
├── evasion.rs          # EvasionDetector (behavioral analysis)
├── registry.rs         # GLEIF/Companies House verification stubs
```

### Confidence Calculation

The `ConfidenceCalculator` aggregates evidence from multiple sources with weighted scoring:

| Source Type | Base Weight | Description |
|-------------|-------------|-------------|
| GovernmentRegistry | 0.95 | Company house, LEI registry |
| RegulatedEntity | 0.90 | Banks, regulated FIs |
| AuditedFinancial | 0.85 | Audited financial statements |
| Document | 0.70 | Extracted from documents |
| ThirdParty | 0.60 | Third-party data providers |
| Screening | 0.75 | Screening service results |
| System | 0.80 | System-derived values |
| Manual | 0.50 | Manual analyst entry |
| Allegation | 0.30 | Client claims (low trust) |

**Modifiers:**
- Authoritative source bonus: +20%
- Corroboration bonus: +10% per corroborating source (max +30%)
- Recency decay: Exponential with 365-day half-life
- Inconsistency penalty: -5% to -25% per inconsistency by severity
- Pattern penalty: -5% to -40% per detected pattern by severity

**Confidence Bands:**

| Band | Score | Meaning |
|------|-------|---------|
| VERIFIED | ≥0.80 | High confidence, verified |
| PROVISIONAL | ≥0.60 | Acceptable with caveats |
| SUSPECT | ≥0.40 | Requires investigation |
| REJECTED | <0.40 | Insufficient evidence |

### Pattern Detection

The `PatternDetector` identifies adversarial patterns in ownership structures:

| Pattern | Detection Method | Severity |
|---------|------------------|----------|
| CircularOwnership | DFS cycle detection | CRITICAL |
| Layering | Chain depth > 5 entities | HIGH |
| NomineeUsage | Nominee/trustee role patterns | MEDIUM |
| OpacityJurisdiction | BVI, Cayman, etc. | MEDIUM |
| RegistryMismatch | GLEIF vs claims differ | HIGH |
| OwnershipGaps | Ownership < 100% | MEDIUM |

### Evasion Detection

The `EvasionDetector` analyzes doc_request history for behavioral red flags:

| Signal | Description | Severity |
|--------|-------------|----------|
| RepeatedDelays | Multiple deadline extensions | MEDIUM |
| SelectiveResponse | Answers some, ignores others | HIGH |
| DocumentQualityIssues | Blurry scans, partial docs | MEDIUM |
| LowCompletionRate | < 50% document fulfillment | HIGH |
| HighRejectionRate | > 30% documents rejected | HIGH |

**Evasion Score Classification:**
- < 0.3: LOW_RISK (proceed normally)
- 0.3 - 0.6: MEDIUM_RISK (enhanced scrutiny)
- 0.6 - 0.8: HIGH_RISK (escalate to senior analyst)
- ≥ 0.8: CRITICAL_RISK (escalate to MLRO)

### Database Tables

| Table | Purpose |
|-------|---------|
| detected_patterns | Pattern detection audit trail |
| verification_challenges | Challenge/response workflow |
| verification_escalations | Risk-based escalation routing |

### Verification Verbs

| Verb | Type | Description |
|------|------|-------------|
| `verify.detect-patterns` | plugin | Run adversarial pattern detection on CBU |
| `verify.detect-evasion` | plugin | Analyze doc_request history for evasion |
| `verify.calculate-confidence` | plugin | Aggregate confidence for entity |
| `verify.get-status` | plugin | Comprehensive verification status report |
| `verify.verify-against-registry` | plugin | Check against GLEIF/Companies House |
| `verify.assert` | plugin | Declarative confidence gate |
| `verify.challenge` | crud | Raise formal challenge |
| `verify.respond-to-challenge` | crud | Record client response |
| `verify.resolve-challenge` | crud | Resolve challenge |
| `verify.escalate` | crud | Route to higher authority |
| `verify.resolve-escalation` | crud | Record escalation decision |
| `verify.list-challenges` | crud | List challenges for CBU/case |

### Example: Verification Flow

```clojure
;; 1. Run pattern detection on CBU
(verify.detect-patterns :cbu-id @fund :case-id @case)

;; 2. Analyze for evasion behavior
(verify.detect-evasion :case-id @case)

;; 3. Calculate confidence for an entity
(verify.calculate-confidence :entity-id @director)

;; 4. Get comprehensive status
(verify.get-status :cbu-id @fund)

;; 5. Verify against external registry
(verify.verify-against-registry :entity-id @company :registry "GLEIF")

;; 6. Assert minimum confidence gate
(verify.assert :cbu-id @fund :min-confidence 0.70 :fail-action "block")

;; 7. Raise a challenge if issues found
(verify.challenge
  :cbu-id @fund
  :entity-id @director
  :challenge-type "LOW_CONFIDENCE"
  :reason "Confidence score 0.45 below threshold"
  :severity "HIGH")
```


## KYC Observation Model

The observation model implements evidence-based KYC verification. Instead of storing a single "truth" per attribute, it captures multiple observations from various sources and reconciles them.

### The Observation Triangle

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         CLIENT ALLEGATIONS                                   │
│  "The client claims..." (unverified starting point)                         │
│  Source: Onboarding form, KYC questionnaire, email                          │
└────────────────────────────────────────┬────────────────────────────────────┘
                                         │
                                         │ verification
                                         ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                       ATTRIBUTE OBSERVATIONS                                 │
│  Multiple observations per attribute from different sources                  │
│  Each with: source_type, source_document, confidence, is_authoritative      │
└────────────────────────────────────────┬────────────────────────────────────┘
                                         │
                            ┌────────────┴────────────┐
                            │                         │
                            ▼                         ▼
              ┌─────────────────────┐   ┌─────────────────────┐
              │   SOURCE DOCUMENTS  │   │   SINK DOCUMENTS    │
              │   (extraction)      │   │   (fulfillment)     │
              │   Passport PROVIDES │   │   Identity REQUIRES │
              │   name, DOB, etc.   │   │   passport as proof │
              └─────────────────────┘   └─────────────────────┘
```

### Key Tables

| Table | Purpose |
|-------|---------|
| attribute_observations | Multiple observations per attribute with source provenance |
| client_allegations | Client's unverified claims (KYC starting point) |
| document_attribute_links | Bidirectional: which docs provide/require which attrs |
| observation_discrepancies | Conflicts detected between observations |

### Allegation Verbs

| Verb | Description |
|------|-------------|
| `allegation.record` | Record client allegation about an attribute |
| `allegation.verify` | Mark allegation verified by observation |
| `allegation.contradict` | Mark allegation contradicted by evidence |
| `allegation.mark-partial` | Mark allegation partially verified |
| `allegation.list-by-entity` | List allegations for an entity |
| `allegation.list-pending` | List pending allegations for CBU |

### Observation Verbs

| Verb | Description |
|------|-------------|
| `observation.record` | Record attribute observation |
| `observation.record-from-document` | Record observation extracted from document |
| `observation.supersede` | Supersede observation with newer one |
| `observation.list-for-entity` | List all observations for entity |
| `observation.list-for-attribute` | List observations of specific attribute |
| `observation.get-current` | Get current best observation |
| `observation.reconcile` | Compare observations and auto-create discrepancies |
| `observation.verify-allegations` | Batch verify pending allegations with observations |

### Discrepancy Verbs

| Verb | Description |
|------|-------------|
| `discrepancy.record` | Record discrepancy between observations |
| `discrepancy.resolve` | Resolve a discrepancy |
| `discrepancy.escalate` | Escalate discrepancy for review |
| `discrepancy.list-open` | List open discrepancies |

### Example: KYC Verification Flow

```clojure
;; 1. Record client allegation
(allegation.record
  :cbu-id @fund
  :entity-id @john
  :attribute-id "attr.identity.full_name"
  :value {"first": "John", "last": "Smith"}
  :display-value "John Smith"
  :source "ONBOARDING_FORM"
  :case-id @case
  :as @allegation-name)

;; 2. Extract observation from passport
(observation.record-from-document
  :entity-id @john
  :document-id @passport
  :attribute "attr.identity.full_name"
  :value "John A Smith"
  :extraction-method "MRZ"
  :confidence 0.95
  :as @obs-passport)

;; 3. Verify allegation (acceptable variation)
(allegation.verify
  :allegation-id @allegation-name
  :observation-id @obs-passport
  :result "ACCEPTABLE_VARIATION"
  :notes "Middle initial difference acceptable")

;; 4. Get current best value
(observation.get-current
  :entity-id @john
  :attribute "attr.identity.full_name")
```

## Service Resource Taxonomy

The service resource taxonomy provides a three-level hierarchy for managing onboarding deliverables:

```
┌─────────────────────────────────────────────────────────────────┐
│  PRODUCT                                                         │
│  What the client buys (e.g., "Prime Brokerage", "Fund Admin")   │
│  ob-poc.products                                                │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼ (M:N via product_services)
┌─────────────────────────────────────────────────────────────────┐
│  SERVICE                                                         │
│  Logical capability delivered (e.g., "Trade Settlement",        │
│  "Asset Safekeeping", "NAV Calculation")                        │
│  ob-poc.services                                                │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼ (M:N via service_resource_capabilities)
┌─────────────────────────────────────────────────────────────────┐
│  SERVICE RESOURCE TYPE                                           │
│  Technical system/platform that delivers the service            │
│  (e.g., "DTCC Settlement System", "Custody Account")            │
│  ob-poc.service_resource_types                                  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼ (1:N)
┌─────────────────────────────────────────────────────────────────┐
│  CBU RESOURCE INSTANCE                                           │
│  Actual provisioned artifact for a specific CBU                 │
│  (e.g., "Acme Fund's custody account at State Street")          │
│  ob-poc.cbu_resource_instances                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Attribute Management

Resource instances have typed attributes defined in a unified registry:

| Table | Purpose |
|-------|---------|
| attribute_registry | Unified attribute dictionary (all domains) |
| resource_attribute_requirements | Required/optional attrs per resource type |
| resource_instance_attributes | Actual values set on instances |

### Service Resource Verbs

| Verb | Description |
|------|-------------|
| `service-resource.provision` | Create resource instance for CBU (auto-derives service_id) |
| `service-resource.set-attr` | Set attribute value on instance |
| `service-resource.validate-attrs` | Validate all required attributes are set |
| `service-resource.activate` | Activate instance (validates required attrs first) |
| `service-resource.suspend` | Suspend active instance |
| `service-resource.decommission` | Permanently decommission instance |

### Example: Provision and Configure

```clojure
;; Provision a custody account (service_id auto-derived from capabilities)
(service-resource.provision
  :cbu-id @fund
  :resource-type "CUSTODY_ACCT"
  :instance-url "https://custody.bank.com/accounts/12345"
  :as @custody)

;; Set required attributes
(service-resource.set-attr :instance-id @custody :attr "account_number" :value "ACC-12345")
(service-resource.set-attr :instance-id @custody :attr "custodian_bic" :value "CITIUS33")

;; Validate before activation
(service-resource.validate-attrs :instance-id @custody)

;; Activate (will fail if required attrs missing)
(service-resource.activate :instance-id @custody)
```

## Custody & Settlement DSL

The `cbu-custody` domain implements a three-layer model for settlement instruction routing, aligned with SWIFT/ISO standards and ALERT-style booking logic.

### Three-Layer Model

```
┌─────────────────────────────────────────────────────────────────┐
│  Layer 1: UNIVERSE                                              │
│  What does the CBU trade?                                       │
│  - Instrument classes (EQUITY, GOVT_BOND, CORP_BOND, ETF)       │
│  - Markets (XNYS, XLON, XFRA, etc.)                            │
│  - Currencies, settlement types                                 │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Layer 2: SSI DATA                                              │
│  Pure account information (no routing logic)                    │
│  - Safekeeping account + BIC                                    │
│  - Cash account + BIC + currency                                │
│  - PSET BIC (place of settlement)                              │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Layer 3: BOOKING RULES                                         │
│  ALERT-style routing: trade characteristics → SSI              │
│  - Priority-based matching (lower = more specific)              │
│  - Wildcard support (NULL = match any)                          │
│  - Specificity scoring for tie-breaking                         │
└─────────────────────────────────────────────────────────────────┘
```

### Custody Verbs

| Verb | Type | Description |
|------|------|-------------|
| `cbu-custody.add-universe` | CRUD | Define tradeable instrument/market combination |
| `cbu-custody.list-universe` | CRUD | List CBU's trading universe |
| `cbu-custody.create-ssi` | CRUD | Create Standing Settlement Instruction |
| `cbu-custody.activate-ssi` | CRUD | Set SSI status to ACTIVE |
| `cbu-custody.suspend-ssi` | CRUD | Set SSI status to SUSPENDED |
| `cbu-custody.list-ssis` | CRUD | List CBU's SSIs |
| `cbu-custody.add-agent-override` | CRUD | Add intermediary agent to SSI settlement chain |
| `cbu-custody.list-agent-overrides` | CRUD | List agent overrides for an SSI |
| `cbu-custody.add-booking-rule` | CRUD | Add ALERT-style routing rule |
| `cbu-custody.list-booking-rules` | CRUD | List CBU's booking rules |
| `cbu-custody.update-rule-priority` | CRUD | Change rule priority |
| `cbu-custody.deactivate-rule` | CRUD | Deactivate a booking rule |
| `cbu-custody.validate-booking-coverage` | Plugin | Validate rules cover universe |
| `cbu-custody.derive-required-coverage` | Plugin | Calculate required coverage |
| `cbu-custody.lookup-ssi` | Plugin | Find SSI for trade characteristics |
| `cbu-custody.setup-ssi` | Plugin | Bulk import SSIs from SSI_ONBOARDING document |

### Example: Full Custody Setup

```clojure
;; Create CBU
(cbu.ensure :name "Pension Fund" :jurisdiction "US" :client-type "FUND" :as @fund)

;; Layer 1: Define trading universe
(cbu-custody.add-universe
  :cbu-id @fund
  :instrument-class "EQUITY"
  :market "XNYS"
  :currencies ["USD"]
  :settlement-types ["DVP"])

(cbu-custody.add-universe
  :cbu-id @fund
  :instrument-class "EQUITY"
  :market "XLON"
  :currencies ["GBP" "USD"]
  :settlement-types ["DVP"])

;; Layer 2: Create SSIs
(cbu-custody.create-ssi
  :cbu-id @fund
  :name "US Safekeeping"
  :type "SECURITIES"
  :safekeeping-account "SAFE-001"
  :safekeeping-bic "BABOROCP"
  :cash-account "CASH-001"
  :cash-bic "BABOROCP"
  :cash-currency "USD"
  :pset-bic "DTCYUS33"
  :effective-date "2024-12-01"
  :as @ssi-us)

(cbu-custody.activate-ssi :ssi-id @ssi-us)

;; Layer 3: Booking rules
(cbu-custody.add-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-us
  :name "US Equity DVP"
  :priority 10
  :instrument-class "EQUITY"
  :market "XNYS"
  :currency "USD"
  :settlement-type "DVP")

;; Fallback rule (lower specificity)
(cbu-custody.add-booking-rule
  :cbu-id @fund
  :ssi-id @ssi-us
  :name "USD Fallback"
  :priority 50
  :currency "USD")

;; Validate coverage
(cbu-custody.validate-booking-coverage :cbu-id @fund)

;; Lookup SSI for a trade
(cbu-custody.lookup-ssi
  :cbu-id @fund
  :instrument-class "EQUITY"
  :market "XNYS"
  :currency "USD"
  :settlement-type "DVP")
```

### Reference Data

**Instrument Classes** (CFI-based):
- `EQUITY` - Common/preferred stock
- `GOVT_BOND` - Government debt
- `CORP_BOND` - Corporate debt
- `ETF` - Exchange-traded funds
- `FUND` - Mutual funds

**Markets** (ISO 10383 MIC):
- `XNYS` - NYSE
- `XNAS` - NASDAQ
- `XLON` - London
- `XPAR` - Euronext Paris
- `XFRA` - Frankfurt

**Settlement Types**:
- `DVP` - Delivery vs Payment
- `FOP` - Free of Payment
- `RVP` - Receive vs Payment

## Trading Matrix (IM Assignment, Pricing, SLA)

The Trading Matrix is a core onboarding pillar that enables **traceability from trade execution back to contractual terms**. It answers: "For this trade, which Investment Manager was responsible, what pricing applies, and are we meeting our SLA commitments?"

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         TRADE EVENT                              │
│  Instrument: AAPL, Market: XNYS, Currency: USD                  │
└─────────────────────────────────────────────────────────────────┘
                              │
          ┌───────────────────┼───────────────────┐
          ▼                   ▼                   ▼
┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐
│  IM Assignment   │ │  Pricing Config  │ │  SLA Commitment  │
│  "Who executed?" │ │  "What rate?"    │ │  "Did we meet?"  │
└──────────────────┘ └──────────────────┘ └──────────────────┘
          │                   │                   │
          ▼                   ▼                   ▼
┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐
│ cbu_im_assign    │ │ cbu_pricing_cfg  │ │ cbu_sla_commit   │
│ - scope matching │ │ - priority-based │ │ - per-metric     │
│ - NULL=any       │ │ - tiered rates   │ │ - thresholds     │
└──────────────────┘ └──────────────────┘ └──────────────────┘
```

### Database Tables (custody schema)

| Table | Purpose |
|-------|---------|
| `investment_managers` | IM entity master with regulatory status |
| `cbu_im_assignments` | CBU → IM assignment with scope (instrument/market/currency) |
| `pricing_templates` | Reusable pricing configurations |
| `pricing_tiers` | Volume/value-based tier thresholds |
| `cbu_pricing_configs` | CBU-specific pricing with template + overrides |
| `sla_templates` | Standard SLA definitions (T+1, 99.9% uptime) |
| `sla_metrics` | Individual metrics within templates |
| `cbu_sla_commitments` | CBU-specific SLA commitments |
| `sla_breaches` | Recorded SLA breaches with remediation |
| `cash_sweep_configs` | Cash sweep/investment configurations |

### Traceability Chain

For any trade, the system can trace:

```
Trade (AAPL, XNYS, USD)
    │
    ├── IM Assignment → Goldman Sachs Asset Management
    │   └── Scope: instrument_class=EQUITY, market=XNYS, currency=USD
    │
    ├── Pricing Config → "US Equity Standard" template
    │   ├── Base rate: 2.5 bps
    │   └── Tier: Volume > $10M → 1.8 bps
    │
    └── SLA Commitment → "Trade Settlement T+1"
        ├── Target: 99.5%
        ├── Actual: 99.2%
        └── Breach: Recorded with remediation plan
```

### Scope Matching (IM Assignment)

IM assignments use **NULL=any** semantics for flexible scope matching:

```sql
-- Find IM for: instrument_class=EQUITY, market=XNYS, currency=USD
SELECT * FROM custody.cbu_im_assignments
WHERE cbu_id = $1
  AND status = 'active'
  AND (instrument_class_id IS NULL OR instrument_class_id = $2)
  AND (market_id IS NULL OR market_id = $3)
  AND (currency IS NULL OR currency = $4)
ORDER BY
  -- Most specific first (fewer NULLs = higher priority)
  (instrument_class_id IS NOT NULL)::int +
  (market_id IS NOT NULL)::int +
  (currency IS NOT NULL)::int DESC
LIMIT 1;
```

### Priority-Based Pricing

Pricing configs use explicit priority for deterministic selection:

```clojure
;; Higher priority (lower number) wins
(pricing-config.assign :cbu-id @fund :template-code "US_EQUITY_PREMIUM"
  :instrument-class "EQUITY" :market "XNYS" :priority 10)

(pricing-config.assign :cbu-id @fund :template-code "EQUITY_STANDARD"
  :instrument-class "EQUITY" :priority 50)  ;; Fallback
```

### SLA Breach Workflow

```
Measurement → Compare to Target → Breach Detected → Record → Remediate → Close
     │                                    │
     └── cbu_sla_commitments.target_value │
                                          ▼
                               sla_breaches table:
                               - breach_severity
                               - root_cause_category
                               - remediation_plan
                               - remediation_due_date
                               - escalated_to
```

### Example: Full Trading Matrix Setup

```clojure
;; 1. Create Investment Manager entity
(entity.create-limited-company :name "Goldman Sachs Asset Management"
  :jurisdiction "US" :as @gsam)

;; 2. Register as Investment Manager
(investment-manager.create :entity-id @gsam :regulatory-status "SEC_REGISTERED"
  :aum-usd 2000000000000 :as @im)

;; 3. Assign IM to CBU for US Equities
(investment-manager.assign :cbu-id @fund :im-id @im
  :instrument-class "EQUITY" :market "XNYS" :currency "USD")

;; 4. Create pricing template
(pricing-config.create-template :code "US_EQUITY_STD" :name "US Equity Standard"
  :fee-type "TRANSACTION" :base-rate 0.00025 :as @pricing-tpl)

;; 5. Add volume tier
(pricing-config.add-tier :template-id @pricing-tpl :tier-name "High Volume"
  :min-value 10000000 :rate-adjustment -0.00007)

;; 6. Assign pricing to CBU
(pricing-config.assign :cbu-id @fund :template-id @pricing-tpl
  :instrument-class "EQUITY" :priority 10)

;; 7. Create SLA template
(sla.create-template :code "TRADE_SETTLE_T1" :name "Trade Settlement T+1"
  :category "SETTLEMENT" :as @sla-tpl)

;; 8. Add metric to template
(sla.add-metric :template-id @sla-tpl :code "SETTLE_RATE"
  :name "Settlement Success Rate" :target-value 99.5
  :measurement-unit "PERCENTAGE" :measurement-frequency "DAILY")

;; 9. Commit SLA to CBU
(sla.commit :cbu-id @fund :template-id @sla-tpl :effective-date "2024-01-01")

;; 10. Query: Find IM for a trade
(investment-manager.find-for-trade :cbu-id @fund
  :instrument-class "EQUITY" :market "XNYS" :currency "USD")

;; 11. Query: Find pricing for instrument
(pricing-config.find-for-instrument :cbu-id @fund
  :instrument-class "EQUITY" :market "XNYS")

;; 12. Query: List open SLA breaches
(sla.list-open-breaches :cbu-id @fund)
```

## Trading Profile DSL

The `trading-profile` domain provides a document-centric approach to CBU trading configuration. A single JSONB document is the source of truth, which is then materialized to operational tables.

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                  Trading Profile Document                        │
│  YAML/JSON source of truth for CBU trading configuration        │
│  Stored in: ob-poc.cbu_trading_profiles                         │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Materialization                               │
│  Document → Operational Tables (atomic sync)                    │
│  - custody.cbu_instrument_universe                              │
│  - custody.cbu_ssi                                              │
│  - custody.ssi_booking_rules                                    │
│  - custody.isda_agreements + csa_agreements                     │
└─────────────────────────────────────────────────────────────────┘
```

### Trading Profile Verbs

| Verb | Description |
|------|-------------|
| `trading-profile.import` | Import trading profile from YAML file |
| `trading-profile.get-active` | Get active profile for a CBU |
| `trading-profile.activate` | Activate a draft profile (supersedes previous) |
| `trading-profile.materialize` | Sync document to operational tables |
| `trading-profile.validate` | Validate document without importing |

### Document Structure

```yaml
# Key sections of a trading profile document
universe:
  base_currency: EUR
  allowed_currencies: [EUR, USD, GBP]
  allowed_markets:
    - mic: XETR          # ISO 10383 MIC code
      currencies: [EUR]
      settlement_types: [DVP]
  instrument_classes:
    - class_code: EQUITY
      is_held: true
      is_traded: true

standing_instructions:
  CUSTODY:
    - name: DE_EQUITY_SSI
      mic: XETR
      currency: EUR
      custody_account: "DE-DEPOT-001"
      custody_bic: "DEUTDEFF"
  OTC_COLLATERAL:
    - name: GS_COLLATERAL_SSI
      counterparty:
        type: LEI
        value: "W22LROWP2IHZNBB6K528"
      currency: USD

booking_rules:
  - name: "German Equities"
    priority: 10
    match:
      mic: XETR
      instrument_class: EQUITY
    ssi_ref: DE_EQUITY_SSI

isda_agreements:
  - counterparty:
      type: LEI
      value: "W22LROWP2IHZNBB6K528"
    agreement_date: "2020-03-15"
    governing_law: ENGLISH
    csa:
      csa_type: VM
      collateral_ssi_ref: GS_COLLATERAL_SSI  # Reference pattern
```

### EntityRef Pattern

Entity references use a type+value pattern for resolution at materialization time:

```yaml
counterparty:
  type: LEI      # LEI, BIC, NAME, or UUID
  value: "W22LROWP2IHZNBB6K528"
```

Resolution checks (in order for LEI):
1. `ob-poc.entity_funds.lei`
2. `ob-poc.entity_manco.lei`
3. `custody.entity_settlement_identity.lei`

### CSA Reference Pattern

CSA collateral SSIs use a reference pattern instead of inline definition:

```yaml
# In standing_instructions.OTC_COLLATERAL:
- name: GS_COLLATERAL_SSI
  counterparty: { type: LEI, value: "..." }
  currency: USD
  custody_account: "COLL-GS-001"

# In isda_agreements[].csa:
csa:
  csa_type: VM
  collateral_ssi_ref: GS_COLLATERAL_SSI  # References SSI by name
```

### Key Files

| File | Purpose |
|------|---------|
| `rust/src/trading_profile/types.rs` | Document type definitions |
| `rust/src/trading_profile/resolve.rs` | EntityRef → UUID resolution |
| `rust/src/trading_profile/validate.rs` | SSI reference validation |
| `rust/src/dsl_v2/custom_ops/trading_profile.rs` | Verb implementations |
| `rust/config/seed/trading_profiles/` | Example YAML profiles |

## KYC & UBO DSL

The KYC case management and UBO domains manage entity-level investigations, screenings, ownership chains, and UBO determinations.

> **Note**: Screenings are now managed via the KYC Case model. Use `kyc-case.create` → `entity-workstream.create` → `case-screening.run` instead of the legacy `screening.*` verbs.

### UBO Graph Architecture

The UBO system uses a **clean separation** between structural graph data and KYC workflow state:

```
┌─────────────────────────────────────────────────────────────────┐
│                 entity_relationships (ob-poc schema)             │
│  STRUCTURAL GRAPH - Facts about the world (CBU-agnostic)         │
│  - Ownership relationships (A owns X% of B)                      │
│  - Control relationships (A controls B via board/voting)         │
│  - Trust roles (settlor, protector, beneficiary)                │
│  - Columns: from_entity_id, to_entity_id, relationship_type,    │
│             percentage, effective_to                             │
│  - Used by: UBO, Onboarding, Trading, Visualization             │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ FK reference
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│           cbu_relationship_verification (ob-poc schema)          │
│  KYC VERIFICATION STATE - Per-CBU verification workflow          │
│  - cbu_id + relationship_id (unique per CBU)                    │
│  - alleged_percentage, observed_percentage                       │
│  - proof_document_id → document_catalog                         │
│  - status: unverified → alleged → pending → proven/disputed     │
│  - Used by: KYC convergence workflow                            │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ Separate concern
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                entity_workstreams (kyc schema)                   │
│  KYC CASE STATE - Per-entity investigation within a case         │
│  - is_ubo: boolean (derived from graph analysis)                │
│  - ownership_percentage: computed from chains                   │
│  - risk_rating: from screening/assessment                       │
│  - status: PENDING → COLLECT → VERIFY → SCREEN → COMPLETE       │
└─────────────────────────────────────────────────────────────────┘
```

**Key Tables:**

| Table | Schema | Purpose |
|-------|--------|---------|
| `entity_relationships` | ob-poc | Structural graph - ownership/control/trust relationships |
| `cbu_relationship_verification` | ob-poc | CBU-specific verification state for relationships |
| `entity_workstreams` | kyc | Per-entity KYC workflow state within a case |
| `proofs` | ob-poc | Evidence documents linked to verification |

**Convergence Model (cbu_relationship_verification.status):**

| Status | Meaning |
|--------|---------|
| `unverified` | Relationship exists, not yet part of KYC |
| `alleged` | Client claims this relationship exists |
| `pending` | Proof document linked, awaiting verification |
| `proven` | Verified by authoritative source |
| `disputed` | Conflicting evidence found |
| `waived` | Verification waived with justification |

**Key Design Principle:** Relationships are facts about the world (in `entity_relationships`). Verification is CBU-specific (in `cbu_relationship_verification`). This allows the same relationship to have different verification status across different CBUs.

**UBO Verb Pattern:**
```clojure
;; 1. Add ownership (creates structural relationship)
(ubo.add-ownership :owner-entity-id @person :owned-entity-id @fund :percentage 60)

;; 2. Allege for CBU context (creates verification record with status=alleged)
(ubo.allege :cbu-id @cbu :relationship-id @rel :percentage 60)

;; 3. Link proof document (status → pending)
(ubo.link-proof :verification-id @verif :proof-id @doc)

;; 4. Verify the relationship (status → proven)
(ubo.verify :verification-id @verif)
```

### UBO Verbs

**Note:** UBO chain tracing operations (`ubo.trace-chains`, `ubo.infer-chain`) now include **control relationships** alongside ownership relationships. This aligns with AML/KYC regulatory guidance where a person may be a beneficial owner through control (voting rights, board control, veto powers) even without direct ownership percentage.

| Verb | Description |
|------|-------------|
| `ubo.add-ownership` | Add ownership relationship |
| `ubo.update-ownership` | Update ownership percentage |
| `ubo.end-ownership` | End ownership relationship |
| `ubo.list-owners` | List owners of entity |
| `ubo.list-owned` | List entities owned by entity |
| `ubo.register-ubo` | Register UBO determination |
| `ubo.verify-ubo` | Mark UBO as verified |
| `ubo.list-ubos` | List UBOs for CBU |
| `ubo.list-by-subject` | List UBOs for subject entity |
| `ubo.discover-owner` | Discover potential UBOs from documents, registry, or screening |
| `ubo.trace-chains` | Trace all ownership AND control chains to natural persons |
| `ubo.infer-chain` | Trace ownership/control chain upward from starting entity |
| `ubo.check-completeness` | Validate UBO determination completeness |
| `ubo.supersede-ubo` | Supersede UBO record with newer determination |
| `ubo.snapshot-cbu` | Capture point-in-time UBO state snapshot |
| `ubo.compare-snapshot` | Compare two UBO snapshots for changes |

**Chain output includes:**
- `relationship_types`: Array showing each hop type (OWNERSHIP, VOTING_RIGHTS, BOARD_APPOINTMENT, etc.)
- `has_control_path`: Boolean indicating if chain includes control relationships
- `ubo_type`: OWNERSHIP, CONTROL, or OWNERSHIP_AND_CONTROL

### Example: Full KYC Case Flow

```clojure
;; Create CBU and entities
(cbu.create :name "Acme Corp" :jurisdiction "GB" :client-type "corporate" :as @cbu)
(entity.create-limited-company :cbu-id @cbu :name "Acme Ltd" :as @company)
(entity.create-proper-person :cbu-id @cbu :first-name "John" :last-name "Smith" :as @ubo)
(cbu.assign-role :cbu-id @cbu :entity-id @ubo :role "BENEFICIAL_OWNER" :ownership-percentage 100)

;; Create KYC case
(kyc-case.create :cbu-id @cbu :case-type "NEW_CLIENT" :as @case)

;; Create workstreams for entities requiring KYC
(entity-workstream.create :case-id @case :entity-id @company :as @ws-company)
(entity-workstream.create :case-id @case :entity-id @ubo :discovery-reason "BENEFICIAL_OWNER" :is-ubo true :as @ws-ubo)

;; Run screenings
(case-screening.run :workstream-id @ws-ubo :screening-type "PEP" :as @pep)
(case-screening.run :workstream-id @ws-ubo :screening-type "SANCTIONS" :as @sanctions)
(case-screening.run :workstream-id @ws-company :screening-type "SANCTIONS")

;; Complete screenings with results
(case-screening.complete :screening-id @pep :status "CLEAR" :result-summary "No matches")
(case-screening.complete :screening-id @sanctions :status "CLEAR" :result-summary "No matches")

;; Complete workstreams and case
(entity-workstream.update-status :workstream-id @ws-ubo :status "COMPLETE")
(entity-workstream.update-status :workstream-id @ws-company :status "COMPLETE")
(kyc-case.update-status :case-id @case :status "APPROVED")
```

### Example: UBO Chain

```clojure
;; Build ownership chain: Person → HoldCo → Fund
(ubo.add-ownership :owner-entity-id @person :owned-entity-id @holdco :percentage 100 :ownership-type "DIRECT" :as @own1)
(ubo.add-ownership :owner-entity-id @holdco :owned-entity-id @fund-entity :percentage 60 :ownership-type "DIRECT" :as @own2)

;; Register UBO determination
(ubo.register-ubo :cbu-id @fund :subject-entity-id @fund-entity :ubo-person-id @person :relationship-type "OWNER" :qualifying-reason "OWNERSHIP_25PCT" :ownership-percentage 60 :workflow-type "ONBOARDING")

;; Verify UBO
(ubo.verify-ubo :ubo-id @ubo1 :verification-status "VERIFIED" :risk-rating "LOW")
```

## CBU Entity Graph Model

The CBU (Client Business Unit) is an **artificial focal point** - it has no real-world presence. It exists purely to connect real entities (funds, investment managers, management companies) via roles, and to attach products.

### Conceptual Model

```
CBU (artificial focal point - no real presence)
 │
 ├── ASSET_OWNER role → Fund Entity (the actual fund, same name as CBU)
 │                         │
 │                         └── 100% owned by → ManCo
 │                                               │
 │                                               └── ownership chain to UBOs...
 │
 ├── INVESTMENT_MANAGER role → IM Company (shared across many CBUs)
 │
 ├── MANCO role → Management Company (shared across many CBUs)
 │
 └── Products → Custody, Fund Accounting (linked directly to CBU)
```

**Key principles:**
- **CBU is artificial** - a context/focal point to group related entities
- **Fund is the ASSET_OWNER** - the real fund entity inherits the CBU name
- **IM/ManCo are shared** - same entity can serve multiple CBUs
- **Products link to CBU** - not to the fund or IM entities
- **UBO chains** flow from Fund → ManCo → natural persons

### Entity Categories

| Category | Description | Examples |
|----------|-------------|----------|
| `SHELL` | Legal vehicles that can own/be owned | Limited Company, Partnership, Trust, SICAV |
| `PERSON` | Natural persons (always leaf nodes) | Directors, UBOs, Signatories |

**Column**: `entity_types.entity_category` (VARCHAR(20))

### Graph Structure

```
CBU (artificial focal point)
 │
 ├── ASSET_OWNER → Fund SICAV (same name as CBU)
 │
 ├── MANCO → ManCo S.à r.l. (shared)
 │    └── PERSON (Director)
 │
 ├── INVESTMENT_MANAGER → IM GmbH (shared)
 │    └── PERSON (Portfolio Manager)
 │
 └── Products → [Custody, Fund Accounting]
```

**Key rules:**
- CBU is NOT an entity - it's an artificial grouping/context
- Fund entity attached via ASSET_OWNER role carries the CBU name
- Same IM/ManCo entities serve multiple CBUs
- Products link to CBU, not to fund or IM entities
- UBO tracing follows ownership from Fund entity upward

### Connection Types

| Table | From | To | Purpose |
|-------|------|-----|---------|
| `cbu_entity_roles` | CBU | Entity | Assigns functional roles within CBU context |
| `entity_relationships` | Entity | Entity | Ownership, control, trust_role relationships (unified table) |
| `cbu_relationship_verification` | CBU | Relationship | CBU-specific verification state for relationships |

### Role Categories

Roles describe an entity's **function** within a CBU, not ownership structure:

| Category | Priority | Roles |
|----------|----------|-------|
| OWNERSHIP_CONTROL | 100 | BENEFICIAL_OWNER, SHAREHOLDER, PRINCIPAL, SETTLOR, PROTECTOR |
| BOTH | 50 | DIRECTOR, AUTHORIZED_SIGNATORY, POWER_OF_ATTORNEY |
| TRADING_EXECUTION | 10 | ASSET_OWNER, INVESTMENT_MANAGER, PORTFOLIO_MANAGER, TRADER |

**View**: `v_cbu_entity_with_roles` sorts entities by role priority (ownership at top, trading at bottom).

### Fund Ownership: Management Shares vs Investor Shares

Funds use a **dual share class structure** to separate control from economic participation:

```
┌─────────────────────────────────────────────────────────────────┐
│  MANAGEMENT SHARES (class_category: CORPORATE)                  │
│  - Owned by: Fund sponsor/ManCo (e.g., BlackRock ManCo)        │
│  - Purpose: Voting rights and control of fund vehicle          │
│  - Economic value: Nominal (often €1 total)                    │
│  - Tradeable: No - permanently held by sponsor                 │
│  - Rights: Appoint directors, approve providers, amend docs    │
└─────────────────────────────────────────────────────────────────┘
                              vs
┌─────────────────────────────────────────────────────────────────┐
│  INVESTOR SHARES (class_category: FUND)                         │
│  - Owned by: Retail and institutional investors                │
│  - Purpose: Economic participation in fund returns             │
│  - Economic value: Full NAV participation                      │
│  - Tradeable: Yes - subscribed/redeemed daily/weekly           │
│  - Rights: Limited voting (usually only on liquidation)        │
└─────────────────────────────────────────────────────────────────┘
```

**How this maps to data model:**

```
CBU: "Luxembourg Growth Fund"
├── SHELL: "LuxGrowth SICAV" (the fund vehicle)
│   └── share_classes:
│       ├── "Management Shares" (class_category: CORPORATE)
│       │   └── held by ManCo → gives ASSET_OWNER role
│       └── "Class A EUR" (class_category: FUND)
│           └── held by investors → tracked in holdings table
│
├── SHELL: "BlackRock Luxembourg ManCo S.à r.l."
│   └── roles: MANAGEMENT_COMPANY, ASSET_OWNER
│   └── holds: 100% of Management Shares
│
└── PERSON: "John Smith" (Director of SICAV)
    └── roles: DIRECTOR
```

**Key insight:** The ManCo's 100% ownership of management shares is what justifies their `ASSET_OWNER` role in `cbu_entity_roles`. The role describes *function*, while the share holding describes the *legal basis* for that function.

**Database columns:**
- `share_classes.class_category`: `CORPORATE` (management/voting) vs `FUND` (investor/NAV)
- `share_classes.entity_id`: FK to the issuing SHELL (SICAV/fund vehicle)
- `holdings`: Tracks who owns which shares (ManCo owns management, investors own fund shares)

## Investor Registry DSL

## Threshold Decision Matrix

The `threshold` domain provides risk-based document requirements that determine what documentation is needed based on entity roles and risk bands.

### Threshold Tables

| Table | Purpose |
|-------|---------|
| threshold_factors | Risk factors and their weights |
| risk_bands | Risk band definitions (LOW, MEDIUM, HIGH, VERY_HIGH) |
| threshold_requirements | Per-risk-band attribute requirements |
| requirement_acceptable_docs | Document types that satisfy requirements |
| screening_requirements | Screening requirements per risk band |

### Requirement → Acceptable Documents Mapping

Each threshold requirement maps to document types that can satisfy it:

| Attribute | Acceptable Documents (by priority) |
|-----------|-----------------------------------|
| `identity` | PASSPORT, NATIONAL_ID, DRIVERS_LICENSE |
| `address` | UTILITY_BILL, BANK_STATEMENT |
| `date_of_birth` | PASSPORT, NATIONAL_ID, DRIVERS_LICENSE, BIRTH_CERTIFICATE |
| `nationality` | PASSPORT, NATIONAL_ID, BIRTH_CERTIFICATE |
| `ownership_percentage` | REGISTER_OF_SHAREHOLDERS, SHARE_CERTIFICATE, OWNERSHIP_CHART, PSC_REGISTER, UBO_DECLARATION |
| `source_of_funds` | SOURCE_OF_FUNDS, BANK_STATEMENT, PROOF_OF_PAYMENT, INVESTMENT_PORTFOLIO |
| `source_of_wealth` | SOURCE_OF_WEALTH, NET_WORTH_STATEMENT, TAX_RETURN, AUDITED_ACCOUNTS |
| `tax_residence` | TAX_RESIDENCY_CERT, TAX_RETURN, W9, W8_BEN, CRS_SELF_CERT, FATCA_SELF_CERT |


### Threshold Verbs

| Verb | Description |
|------|-------------|
| `threshold.derive` | Compute risk band from entity factors |
| `threshold.evaluate` | Evaluate requirements for entity based on risk band |
| `threshold.check-entity` | Check if entity meets all threshold requirements |

### Example: Threshold-Based Requirements

```clojure
;; Derive risk band for entity
(threshold.derive :cbu-id @fund :entity-id @ubo :as @risk-result)

;; Evaluate what requirements apply
(threshold.evaluate :cbu-id @fund :entity-id @ubo :risk-band "HIGH")

;; Check if entity meets all requirements
(threshold.check-entity :cbu-id @fund :entity-id @ubo)
```

## RFI (Request for Information) System

The `rfi` domain manages batch document requests based on threshold requirements. It extends the existing `kyc.doc_requests` table rather than creating separate tables.

### RFI Verbs

| Verb | Description |
|------|-------------|
| `rfi.generate` | Generate doc_requests from threshold requirements for a case |
| `rfi.check-completion` | Check document completion status for a case |
| `rfi.list-by-case` | List all doc_requests for a case |

### Example: RFI Generation

```clojure
;; Generate document requests based on threshold requirements
(rfi.generate :case-id @case :risk-band "HIGH" :as @batch-id)

;; Check completion status
(rfi.check-completion :case-id @case)

;; List all requests for the case
(rfi.list-by-case :case-id @case)
```


The `share-class`, `holding`, and `movement` domains implement a Clearstream-style investor registry for fund share classes.

### Share Class Verbs

| Verb | Description |
|------|-------------|
| `share-class.create` | Create new share class for fund CBU |
| `share-class.ensure` | Upsert share class by ISIN |
| `share-class.update-nav` | Update NAV and date |
| `share-class.read` | Read share class by ID |
| `share-class.list` | List share classes for fund |
| `share-class.close` | Close to new subscriptions |

### Holding Verbs

| Verb | Description |
|------|-------------|
| `holding.create` | Create investor holding |
| `holding.ensure` | Upsert holding by share class + investor |
| `holding.update-units` | Update position units |
| `holding.read` | Read holding by ID |
| `holding.list-by-share-class` | List holdings for share class |
| `holding.list-by-investor` | List holdings for investor |
| `holding.close` | Mark holding inactive |

### Movement Verbs

| Verb | Description |
|------|-------------|
| `movement.subscribe` | Record subscription |
| `movement.redeem` | Record redemption |
| `movement.transfer-in` | Record incoming transfer |
| `movement.transfer-out` | Record outgoing transfer |
| `movement.confirm` | Confirm pending movement |
| `movement.settle` | Mark as settled |
| `movement.cancel` | Cancel pending movement |
| `movement.list-by-holding` | List movements for holding |
| `movement.read` | Read movement by ID |

### Example: Fund Share Class Setup

```clojure
;; Create fund CBU with commercial client reference
(entity.create-limited-company :name "Blackrock Inc" :jurisdiction "US" :as @head-office)
(cbu.ensure :name "Luxembourg Growth Fund" :jurisdiction "LU" :client-type "FUND" 
  :commercial-client-entity-id @head-office :as @fund)

;; Create fund entity (legal issuer of shares)
(entity.create-limited-company :name "Luxembourg Growth Fund SICAV" :jurisdiction "LU" :as @fund-entity)

;; Create share classes with issuing entity
(share-class.create :cbu-id @fund :entity-id @fund-entity :name "Class A EUR" :isin "LU0123456789" 
  :currency "EUR" :class-category "FUND" :nav-per-share 100.00 :management-fee-bps 150 
  :minimum-investment 10000.00 :subscription-frequency "Daily" :redemption-frequency "Weekly" 
  :redemption-notice-days 5 :as @class-a)

(share-class.create :cbu-id @fund :entity-id @fund-entity :name "Class I USD" :isin "LU9876543210" 
  :currency "USD" :class-category "FUND" :nav-per-share 1000.00 :management-fee-bps 75 
  :minimum-investment 1000000.00 :as @class-i)

;; Create corporate share class (for ManCo ownership tracking)
(entity.create-limited-company :name "Fund Management Co" :jurisdiction "LU" :as @manco)
(share-class.create :cbu-id @fund :entity-id @manco :name "Ordinary Shares" 
  :currency "EUR" :class-category "CORPORATE" :as @manco-shares)

;; Create investor entity
(entity.create-limited-company :name "Pension Fund ABC" :jurisdiction "US" :as @investor)

;; Create holding
(holding.create :share-class-id @class-a :investor-entity-id @investor :as @holding)

;; Record subscription
(movement.subscribe :holding-id @holding :units 1000 :price-per-unit 100.00 :amount 100000.00
  :trade-date "2024-01-15" :settlement-date "2024-01-17" :reference "SUB-2024-001")

;; Confirm and settle
(movement.confirm :movement-id @sub1)
(movement.settle :movement-id @sub1)

;; Update holding position
(holding.update-units :holding-id @holding :units 1000 :cost-basis 100000.00)

;; Update NAV
(share-class.update-nav :share-class-id @class-a :nav-per-share 102.50 :nav-date "2024-01-31")

;; Record redemption
(movement.redeem :holding-id @holding :units 500 :price-per-unit 102.50 :amount 51250.00
  :trade-date "2024-02-01" :reference "RED-2024-001")
```



## Database Schema Reference

**Database**: `data_designer` on PostgreSQL 17  
**Schemas**: `ob-poc` (83 tables), `custody` (17 tables), `kyc` (12 tables), `public` (10 tables)  
**Updated**: 2025-12-11

## Overview

This document describes the database schema used by the OB-POC KYC/AML onboarding system. The schema supports:

- **Core KYC/AML**: CBUs, entities, documents, screening, KYC investigations
- **Service Delivery**: Products, services, resource instances
- **Custody & Settlement**: Three-layer model (Universe → SSI → Booking Rules)
- **Investor Registry**: Fund share classes, holdings, and movements (Clearstream-style)
- **Fund Structure**: Umbrella funds, sub-funds, share classes, master-feeder, FoF investments
- **Control & Delegation**: Control relationships (separate from ownership), service provider delegation chains
- **Evidence & Proofs**: CBU evidence, UBO evidence, snapshots for audit trails
- **Decision Support**: Case evaluation snapshots, red-flag scoring, decision thresholds
- **Agentic DSL Generation**: The `rust/src/agentic/` module generates DSL that creates records in these tables

## Core Tables

### cbus (Client Business Units)

The central entity representing a client relationship.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| cbu_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| name | varchar(255) | NOT NULL | | Client name |
| description | text | | | Description |
| nature_purpose | text | | | Nature and purpose of business |
| source_of_funds | text | | | Source of funds |
| client_type | varchar(100) | | | FUND, CORPORATE, INDIVIDUAL, etc. |
| jurisdiction | varchar(50) | | | Primary jurisdiction code |
| risk_context | jsonb | | '{}' | Risk assessment context |
| onboarding_context | jsonb | | '{}' | Onboarding workflow context |
| semantic_context | jsonb | | '{}' | AI/semantic context |
| embedding | vector | | | pgvector embedding |
| commercial_client_entity_id | uuid | YES | | FK to entities - head office that contracted with bank |
| product_id | uuid | YES | | FK to products - primary product for this CBU |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

### entities (Base Entity Table)

Base table for all entity types (Class Table Inheritance pattern).

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| entity_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| entity_type_id | uuid | NOT NULL | | FK to entity_types |
| external_id | varchar(255) | | | External system reference |
| name | varchar(255) | NOT NULL | | Display name |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

### entity_types (Entity Type Registry)

Defines available entity types and their extension tables.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| entity_type_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| name | varchar(255) | NOT NULL | | Display name |
| type_code | varchar(100) | | | Code for DSL verbs (e.g., 'proper_person') |
| table_name | varchar(255) | NOT NULL | | Extension table name |
| description | text | | | |
| parent_type_id | uuid | | | For type hierarchy |
| type_hierarchy_path | text[] | | | Ancestor path |
| semantic_context | jsonb | | '{}' | AI context |
| embedding | vector | | | |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

## Entity Extension Tables

### entity_proper_persons (Natural Persons)

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| proper_person_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| entity_id | uuid | | | FK to entities |
| first_name | varchar(255) | NOT NULL | | |
| last_name | varchar(255) | NOT NULL | | |
| middle_names | varchar(255) | | | |
| date_of_birth | date | | | |
| nationality | varchar(100) | | | |
| residence_address | text | | | |
| id_document_type | varchar(100) | | | |
| id_document_number | varchar(100) | | | |
| search_name | text | | | Computed search field |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

### entity_limited_companies (Companies)

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| limited_company_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| entity_id | uuid | | | FK to entities |
| company_name | varchar(255) | NOT NULL | | |
| registration_number | varchar(100) | | | |
| jurisdiction | varchar(100) | | | |
| incorporation_date | date | | | |
| registered_address | text | | | |
| business_nature | text | | | |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

### entity_partnerships

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| partnership_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| entity_id | uuid | | | FK to entities |
| partnership_name | varchar(255) | NOT NULL | | |
| partnership_type | varchar(100) | | | LP, LLP, GP, etc. |
| jurisdiction | varchar(100) | | | |
| formation_date | date | | | |
| principal_place_business | text | | | |
| partnership_agreement_date | date | | | |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

### entity_trusts

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| trust_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| entity_id | uuid | | | FK to entities |
| trust_name | varchar(255) | NOT NULL | | |
| trust_type | varchar(100) | | | Discretionary, Fixed, etc. |
| jurisdiction | varchar(100) | NOT NULL | | |
| establishment_date | date | | | |
| trust_deed_date | date | | | |
| trust_purpose | text | | | |
| governing_law | varchar(100) | | | |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

### entity_funds (Fund Entities)

Extension table for umbrella funds, sub-funds, standalone funds, master and feeder funds.

| Column | Type | Description |
|--------|------|-------------|
| entity_id | uuid | PK, FK to entities |
| lei | varchar(20) | Legal Entity Identifier (unique index) |
| isin_base | varchar(12) | Base ISIN |
| fund_structure_type | text | SICAV, ICAV, OEIC, VCC, UNIT_TRUST, FCP |
| fund_type | text | UCITS, AIF, HEDGE_FUND, PRIVATE_EQUITY |
| regulatory_status | text | UCITS, AIF, RAIF, PART_II, UNREGULATED |
| parent_fund_id | uuid | FK to entities (umbrella for subfund) |
| master_fund_id | uuid | FK to entities (master for feeder) |
| jurisdiction | varchar(10) | Domicile |
| base_currency | varchar(3) | |
| investment_objective | text | |
| gleif_legal_form_id | varchar(10) | ELF code (e.g., UDY2) |
| gleif_registered_as | varchar(100) | Registry identifier |
| gleif_registered_at | varchar(20) | Registration authority code |
| gleif_category | varchar(20) | FUND, GENERAL, BRANCH, SOLE_PROPRIETOR |
| gleif_status | varchar(20) | ACTIVE, INACTIVE |
| gleif_corroboration_level | varchar(30) | FULLY_CORROBORATED, PARTIALLY_CORROBORATED |
| gleif_managing_lou | varchar(20) | LEI of the Local Operating Unit |
| gleif_last_update | timestamptz | Last update from GLEIF API |
| legal_address_city | varchar(100) | Legal address city |
| legal_address_country | varchar(2) | Legal address ISO country code |
| hq_address_city | varchar(100) | HQ address city |
| hq_address_country | varchar(2) | HQ address ISO country code |

### entity_share_classes (Share Classes)

| Column | Type | Description |
|--------|------|-------------|
| entity_id | uuid | PK, FK to entities |
| parent_fund_id | uuid | FK to entities (sub-fund) |
| isin | varchar(12) | ISIN (unique) |
| share_class_type | text | INSTITUTIONAL, RETAIL, SEED, FOUNDER, CLEAN |
| distribution_type | text | ACC (accumulating), DIST (distributing), FLEX |
| currency | varchar(3) | |
| is_hedged | boolean | |
| management_fee_bps | integer | Management fee in basis points |
| minimum_investment | decimal | |

### entity_manco (Management Companies)

| Column | Type | Description |
|--------|------|-------------|
| entity_id | uuid | PK, FK to entities |
| lei | varchar(20) | Legal Entity Identifier |
| manco_type | text | UCITS_MANCO, AIFM, DUAL_AUTHORIZED |
| authorized_jurisdiction | varchar(10) | |
| can_manage_ucits | boolean | |
| can_manage_aif | boolean | |
| passported_jurisdictions | text[] | Array of jurisdiction codes |

## Fund Structure Tables

### fund_structure

Structural containment relationships (umbrella→subfund, subfund→shareclass, master→feeder).

| Column | Type | Description |
|--------|------|-------------|
| structure_id | uuid | Primary key |
| parent_entity_id | uuid | FK to entities |
| child_entity_id | uuid | FK to entities |
| relationship_type | text | CONTAINS, MASTER_FEEDER |
| effective_from | date | |
| effective_to | date | NULL = current |

### fund_investments

Fund-of-Funds investment relationships.

| Column | Type | Description |
|--------|------|-------------|
| investment_id | uuid | Primary key |
| investor_entity_id | uuid | The FoF |
| investee_entity_id | uuid | Underlying fund |
| percentage_of_investor_nav | decimal | % of FoF NAV |
| percentage_of_investee_aum | decimal | % of underlying AUM |
| investment_type | text | DIRECT, VIA_SHARE_CLASS, SIDE_POCKET |
| investment_date | date | |
| redemption_date | date | NULL = still invested |

### entity_relationships

Unified table for all entity-to-entity relationships (ownership, control, trust roles).

| Column | Type | Description |
|--------|------|-------------|
| relationship_id | uuid | Primary key |
| from_entity_id | uuid | Owner/controller entity |
| to_entity_id | uuid | Owned/controlled entity |
| relationship_type | varchar(30) | 'ownership', 'control', 'trust_role' |
| percentage | decimal(5,2) | Ownership percentage (NULL for non-ownership) |
| control_type | varchar(30) | For control: board_member, executive, voting_rights, etc. |
| trust_role | varchar(30) | For trust: settlor, trustee, beneficiary, protector |
| effective_from | date | Start date |
| effective_to | date | End date (NULL = active) |
| source | varchar(100) | Data source reference |

### cbu_relationship_verification

CBU-specific verification state for relationships (KYC convergence workflow).

| Column | Type | Description |
|--------|------|-------------|
| verification_id | uuid | Primary key |
| cbu_id | uuid | FK to cbus |
| relationship_id | uuid | FK to entity_relationships |
| alleged_percentage | decimal(5,2) | What client claims |
| observed_percentage | decimal(5,2) | What proof shows |
| proof_document_id | uuid | FK to document_catalog |
| status | varchar(20) | unverified, alleged, pending, proven, disputed, waived |
| discrepancy_notes | text | Notes on conflicts |
| resolved_at | timestamptz | When resolved |

### delegation_relationships

Service provider delegation chains (ManCo to sub-advisor, administrator to sub-contractor).

| Column | Type | Description |
|--------|------|-------------|
| delegation_id | uuid | Primary key |
| delegator_entity_id | uuid | Who delegates |
| delegate_entity_id | uuid | Who receives delegation |
| delegation_scope | text | INVESTMENT_MANAGEMENT, RISK_MANAGEMENT, PORTFOLIO_ADMINISTRATION, DISTRIBUTION, TRANSFER_AGENCY |
| applies_to_cbu_id | uuid | FK to cbus (optional - may be firm-wide) |
| contract_doc_id | uuid | FK to document_catalog |
| effective_from | date | |
| effective_to | date | |

## Role Management

### roles

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| role_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| name | varchar(255) | NOT NULL | | DIRECTOR, UBO, SHAREHOLDER, etc. |
| description | text | | | |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

### cbu_entity_roles (CBU-Entity-Role Junction)

Links entities to CBUs with specific roles.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| cbu_entity_role_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| cbu_id | uuid | NOT NULL | | FK to cbus |
| entity_id | uuid | NOT NULL | | FK to entities |
| role_id | uuid | NOT NULL | | FK to roles |
| created_at | timestamptz | | now() | |

## Document Management

### document_types

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| type_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| type_code | varchar(100) | NOT NULL | | PASSPORT, CERT_OF_INCORP, etc. |
| display_name | varchar(200) | NOT NULL | | |
| category | varchar(100) | NOT NULL | | IDENTITY, CORPORATE, FINANCIAL |
| domain | varchar(100) | | | |
| description | text | | | |
| required_attributes | jsonb | | '{}' | |
| applicability | jsonb | | '{}' | Entity type applicability |
| semantic_context | jsonb | | '{}' | |
| embedding | vector | | | |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

### document_catalog

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| doc_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| document_id | uuid | | gen_random_uuid() | Business ID |
| cbu_id | uuid | | | FK to cbus |
| document_type_id | uuid | | | FK to document_types |
| document_type_code | varchar(100) | | | Denormalized type code |
| document_name | varchar(255) | | | |
| file_hash_sha256 | text | | | |
| storage_key | text | | | S3/storage reference |
| file_size_bytes | bigint | | | |
| mime_type | varchar(100) | | | |
| source_system | varchar(100) | | | |
| status | varchar(50) | | 'active' | |
| extraction_status | varchar(50) | | 'PENDING' | |
| extracted_data | jsonb | | | AI-extracted data |
| extraction_confidence | numeric | | | |
| last_extracted_at | timestamptz | | | |
| metadata | jsonb | | '{}' | |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

## Screening & KYC

### screenings

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| screening_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| investigation_id | uuid | | | FK to kyc_investigations |
| entity_id | uuid | NOT NULL | | FK to entities |
| screening_type | varchar(50) | NOT NULL | | PEP, SANCTIONS, ADVERSE_MEDIA |
| databases | jsonb | | | Databases searched |
| lists | jsonb | | | Specific lists |
| include_rca | boolean | | false | Include relatives/close associates |
| search_depth | varchar(20) | | | |
| languages | jsonb | | | |
| status | varchar(50) | | 'PENDING' | |
| result | varchar(50) | | | CLEAR, HIT, INCONCLUSIVE |
| match_details | jsonb | | | |
| resolution | varchar(50) | | | TRUE_MATCH, FALSE_POSITIVE |
| resolution_rationale | text | | | |
| screened_at | timestamptz | | now() | |
| reviewed_by | varchar(255) | | | |
| resolved_by | varchar(255) | | | |
| resolved_at | timestamptz | | | |

### kyc_investigations

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| investigation_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| cbu_id | uuid | | | FK to cbus |
| investigation_type | varchar(50) | NOT NULL | | INITIAL, PERIODIC, TRIGGER |
| risk_rating | varchar(20) | | | LOW, MEDIUM, HIGH |
| regulatory_framework | jsonb | | | |
| ubo_threshold | numeric | | 10.0 | |
| investigation_depth | integer | | 5 | |
| status | varchar(50) | | 'INITIATED' | |
| deadline | date | | | |
| outcome | varchar(50) | | | |
| notes | text | | | |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |
| completed_at | timestamptz | | | |

### kyc_decisions

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| decision_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| cbu_id | uuid | NOT NULL | | FK to cbus |
| investigation_id | uuid | | | FK to kyc_investigations |
| decision | varchar(50) | NOT NULL | | APPROVE, REJECT, CONDITIONAL |
| decision_authority | varchar(100) | | | |
| rationale | text | | | |
| decided_by | varchar(255) | | | |
| decided_at | timestamptz | | now() | |
| effective_date | date | | CURRENT_DATE | |
| review_date | date | | | |

### entity_kyc_status

Per-entity KYC status within a CBU context.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| status_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| entity_id | uuid | NOT NULL | | FK to entities |
| cbu_id | uuid | NOT NULL | | FK to cbus |
| kyc_status | varchar(50) | NOT NULL | | NOT_STARTED, IN_PROGRESS, PENDING_REVIEW, APPROVED, REJECTED, EXPIRED |
| risk_rating | varchar(20) | | | LOW, MEDIUM, HIGH, PROHIBITED |
| reviewer | varchar(255) | | | Reviewer email/ID |
| notes | text | | | Status notes |
| next_review_date | date | | | Scheduled review date |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

**Unique constraint**: (entity_id, cbu_id)

## Observation Model (KYC Evidence)

The observation model captures the reality of KYC: multiple sources may provide different observations about the same attribute. Allegations from clients are verified against documentary evidence.

### client_allegations

What the client claims about their entities.

| Column | Type | Description |
|--------|------|-------------|
| allegation_id | uuid | Primary key |
| cbu_id | uuid | FK to cbus |
| entity_id | uuid | FK to entities |
| attribute_id | uuid | FK to attribute_registry |
| alleged_value | jsonb | The claimed value |
| allegation_source | varchar(50) | ONBOARDING_FORM, KYC_QUESTIONNAIRE, EMAIL, VERBAL, API, DOCUMENT |
| verification_status | varchar(30) | PENDING, VERIFIED, CONTRADICTED, PARTIAL, UNVERIFIABLE, WAIVED |
| verification_result | varchar(30) | EXACT_MATCH, ACCEPTABLE_VARIATION, MATERIAL_DISCREPANCY |
| verified_by_observation_id | uuid | FK to attribute_observations |

### attribute_observations

Evidence from authoritative sources (documents, screening, third parties).

| Column | Type | Description |
|--------|------|-------------|
| observation_id | uuid | Primary key |
| entity_id | uuid | FK to entities |
| attribute_id | uuid | FK to attribute_registry |
| value_text/number/boolean/date/json | varied | Exactly one value column set |
| source_type | varchar(30) | DOCUMENT, SCREENING, THIRD_PARTY, SYSTEM, DERIVED, MANUAL |
| source_document_id | uuid | FK to document_catalog (required if source_type=DOCUMENT) |
| confidence | numeric(3,2) | 0.00-1.00 confidence score |
| is_authoritative | boolean | Primary source for this attribute |
| status | varchar(30) | ACTIVE, SUPERSEDED, DISPUTED, WITHDRAWN, REJECTED |

### observation_discrepancies

Conflicts between observations requiring resolution.

| Column | Type | Description |
|--------|------|-------------|
| discrepancy_id | uuid | Primary key |
| entity_id | uuid | FK to entities |
| attribute_id | uuid | FK to attribute_registry |
| observation_1_id | uuid | FK to attribute_observations |
| observation_2_id | uuid | FK to attribute_observations |
| discrepancy_type | varchar(30) | VALUE_MISMATCH, SPELLING_VARIATION, CONTRADICTORY |
| severity | varchar(20) | INFO, LOW, MEDIUM, HIGH, CRITICAL |
| resolution_status | varchar(30) | OPEN, INVESTIGATING, RESOLVED, ESCALATED |
| accepted_observation_id | uuid | FK to observation chosen as correct |

## Products & Services

### Reference Data Summary (as of 2025-12-03)

| Entity | Count |
|--------|-------|
| Products | 7 |
| Services | 30 |
| Service Resource Types | 22 |
| Product-Service Mappings | 32 |

**Products**: Alternatives, Collateral Management, Custody, Fund Accounting, Markets FX, Middle Office, Transfer Agency

**Service Resource Types**: ALTS_GENEVA, ALTS_PRADO, APAC_CLEAR, CA_PLATFORM, COLLATERAL_GLOBAL1, CUSTODY_ACCT, CUSTODY_GSP, CUSTODY_IMMS, CUSTODY_SMARTSTREAM, CUSTODY_SWIFT, DTCC_SETTLE, EUROCLEAR, FA_EAGLE, FA_INVESTONE, IBOR_SYSTEM, INVESTOR_LEDGER, NAV_ENGINE, PNL_ENGINE, REPORTING_HUB, RUFUS_TA, SETTLE_ACCT, SWIFT_CONN


### products

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| product_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| name | varchar(255) | NOT NULL | | |
| product_code | varchar(50) | | | |
| product_category | varchar(100) | | | |
| regulatory_framework | varchar(100) | | | |
| description | text | | | |
| min_asset_requirement | numeric | | | |
| is_active | boolean | | true | |
| metadata | jsonb | | | |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

### services

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| service_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| name | varchar(255) | NOT NULL | | |
| service_code | varchar(50) | | | |
| service_category | varchar(100) | | | |
| description | text | | | |
| sla_definition | jsonb | | | |
| is_active | boolean | | true | |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

## Resource Instance Taxonomy

### cbu_resource_instances

Delivered resource instances (accounts, connections, etc.).

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| instance_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| cbu_id | uuid | NOT NULL | | FK to cbus |
| product_id | uuid | | | FK to products |
| service_id | uuid | | | FK to services |
| resource_type_id | uuid | | | FK to service_resources |
| instance_url | varchar(1024) | NOT NULL | | Resource locator |
| instance_identifier | varchar(255) | | | External ID |
| instance_name | varchar(255) | | | Display name |
| instance_config | jsonb | | '{}' | Configuration |
| status | varchar(50) | NOT NULL | 'PENDING' | PENDING, ACTIVE, SUSPENDED, DECOMMISSIONED |
| requested_at | timestamptz | | now() | |
| provisioned_at | timestamptz | | | |
| activated_at | timestamptz | | | |
| decommissioned_at | timestamptz | | | |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

### resource_instance_attributes

Typed attribute values for resource instances.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| value_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| instance_id | uuid | NOT NULL | | FK to cbu_resource_instances |
| attribute_id | uuid | NOT NULL | | FK to attribute_registry |
| value_text | varchar | | | Text value |
| value_number | numeric | | | Numeric value |
| value_boolean | boolean | | | Boolean value |
| value_date | date | | | Date value |
| value_timestamp | timestamptz | | | Timestamp value |
| value_json | jsonb | | | JSON value |
| state | varchar(50) | | 'proposed' | proposed, confirmed, superseded |
| source | jsonb | | | Source metadata |
| observed_at | timestamptz | | now() | |

### service_delivery_map

Tracks service delivery to CBUs.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| delivery_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| cbu_id | uuid | NOT NULL | | FK to cbus |
| product_id | uuid | NOT NULL | | FK to products |
| service_id | uuid | NOT NULL | | FK to services |
| instance_id | uuid | | | FK to cbu_resource_instances |
| service_config | jsonb | | '{}' | |
| delivery_status | varchar(50) | | 'PENDING' | PENDING, IN_PROGRESS, DELIVERED, FAILED |
| requested_at | timestamptz | | now() | |
| started_at | timestamptz | | | |
| delivered_at | timestamptz | | | |
| failed_at | timestamptz | | | |
| failure_reason | text | | | |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

## Reference Data

### master_jurisdictions

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| jurisdiction_code | varchar(10) | NOT NULL | | Primary key (e.g., 'LU', 'IE') |
| jurisdiction_name | varchar(200) | NOT NULL | | |
| country_code | varchar(3) | NOT NULL | | ISO country code |
| region | varchar(100) | | | |
| regulatory_framework | varchar(100) | | | |
| entity_formation_allowed | boolean | | true | |
| offshore_jurisdiction | boolean | | false | |
| regulatory_authority | varchar(300) | | | |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |

## Custody Schema (`custody`)

The custody schema implements a three-layer model for settlement instruction routing.

### Layer 1: Universe Tables

#### cbu_instrument_universe

Defines what instruments a CBU trades.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| universe_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| cbu_id | uuid | NOT NULL | | FK to cbus |
| instrument_class_id | uuid | NOT NULL | | FK to instrument_classes |
| market_id | uuid | | | FK to markets |
| currencies | varchar(3)[] | NOT NULL | '{}' | Supported currencies |
| settlement_types | varchar(10)[] | | '{DVP}' | DVP, FOP, RVP |
| counterparty_entity_id | uuid | | | For OTC counterparty-specific |
| is_held | boolean | | true | Holds positions |
| is_traded | boolean | | true | Actively trades |
| is_active | boolean | | true | Active record |
| effective_date | date | NOT NULL | CURRENT_DATE | |

### Layer 2: SSI Tables

#### cbu_ssi (Standing Settlement Instructions)

Account information for settlement.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| ssi_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| cbu_id | uuid | NOT NULL | | FK to cbus |
| ssi_name | varchar(100) | NOT NULL | | Display name |
| ssi_type | varchar(20) | NOT NULL | | SECURITIES, CASH, COLLATERAL |
| safekeeping_account | varchar(35) | | | Securities account |
| safekeeping_bic | varchar(11) | | | Custodian BIC |
| safekeeping_account_name | varchar(100) | | | Account name |
| cash_account | varchar(35) | | | Cash account |
| cash_account_bic | varchar(11) | | | Cash agent BIC |
| cash_currency | varchar(3) | | | Settlement currency |
| pset_bic | varchar(11) | | | Place of settlement BIC |
| status | varchar(20) | | 'PENDING' | PENDING, ACTIVE, SUSPENDED |
| effective_date | date | NOT NULL | | Start date |
| expiry_date | date | | | End date |
| source | varchar(20) | | 'MANUAL' | MANUAL, SWIFT, DTCC |

### Layer 3: Booking Rules

#### ssi_booking_rules

ALERT-style routing rules matching trade characteristics to SSIs.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| rule_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| cbu_id | uuid | NOT NULL | | FK to cbus |
| ssi_id | uuid | NOT NULL | | FK to cbu_ssi |
| rule_name | varchar(100) | NOT NULL | | Display name |
| priority | integer | NOT NULL | 50 | Lower = higher priority |
| instrument_class_id | uuid | | | NULL = any |
| security_type_id | uuid | | | NULL = any |
| market_id | uuid | | | NULL = any |
| currency | varchar(3) | | | NULL = any |
| settlement_type | varchar(10) | | | NULL = any |
| counterparty_entity_id | uuid | | | For OTC |
| specificity_score | integer | | | Generated: counts non-NULL criteria |
| is_active | boolean | | true | |
| effective_date | date | NOT NULL | CURRENT_DATE | |

### Reference Tables

#### instrument_classes

CFI-based instrument classification.

| Column | Type | Description |
|--------|------|-------------|
| class_id | uuid | Primary key |
| class_code | varchar(20) | EQUITY, GOVT_BOND, CORP_BOND, ETF |
| cfi_prefix | varchar(6) | CFI code prefix |
| description | text | |
| smpg_category | varchar(50) | SMPG/ALERT category |

#### markets

ISO 10383 MIC codes.

| Column | Type | Description |
|--------|------|-------------|
| market_id | uuid | Primary key |
| mic | varchar(4) | XNYS, XLON, XNAS |
| market_name | varchar(100) | |
| country_code | varchar(2) | |
| currency | varchar(3) | Primary currency |
| csd_bic | varchar(11) | CSD BIC |

#### security_types

SMPG/ALERT security type taxonomy.

| Column | Type | Description |
|--------|------|-------------|
| security_type_id | uuid | Primary key |
| type_code | varchar(30) | |
| instrument_class_id | uuid | FK to instrument_classes |
| description | text | |
| smpg_code | varchar(10) | |

#### currencies

ISO 4217 currency codes.

| Column | Type | Description |
|--------|------|-------------|
| currency_code | varchar(3) | Primary key (USD, EUR, GBP) |
| currency_name | varchar(50) | |
| decimals | integer | Decimal places |
| is_active | boolean | |

### Supporting Tables

| Table | Purpose |
|-------|---------|
| cbu_ssi_agent_override | Override receiving/delivering agents |
| entity_settlement_identity | BIC/LEI for entity settlement |
| entity_ssi | Entity-level SSIs (vs CBU-level) |
| subcustodian_network | Subcustodian relationships |
| instruction_types | Settlement instruction types |
| instruction_paths | Settlement message routing |
| isda_agreements | ISDA master agreements |
| isda_product_coverage | Products under ISDA |
| isda_product_taxonomy | OTC product classification |
| csa_agreements | Credit support annexes |
| cfi_codes | Full CFI code reference |

## KYC Schema (`kyc`)

The kyc schema implements both KYC case management and a Clearstream-style investor registry.

### KYC Case Management

#### cases

Central table for KYC investigation cases.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| case_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| cbu_id | uuid | NOT NULL | | FK to cbus |
| status | varchar(30) | NOT NULL | 'INTAKE' | INTAKE, DISCOVERY, ASSESSMENT, REVIEW, APPROVED, REJECTED, BLOCKED, WITHDRAWN, EXPIRED |
| escalation_level | varchar(30) | NOT NULL | 'STANDARD' | STANDARD, SENIOR_COMPLIANCE, EXECUTIVE, BOARD |
| risk_rating | varchar(20) | | | LOW, MEDIUM, HIGH, VERY_HIGH, PROHIBITED |
| assigned_analyst_id | uuid | | | Assigned analyst |
| assigned_reviewer_id | uuid | | | Assigned reviewer |
| opened_at | timestamptz | NOT NULL | now() | Case opened timestamp |
| closed_at | timestamptz | | | Case closed timestamp |
| sla_deadline | timestamptz | | | SLA deadline |
| last_activity_at | timestamptz | | now() | Last activity timestamp |
| case_type | varchar(30) | | 'NEW_CLIENT' | NEW_CLIENT, PERIODIC_REVIEW, EVENT_DRIVEN, REMEDIATION |
| notes | text | | | Case notes |

**Indexes**: case_id (PK), cbu_id, status, assigned_analyst_id

#### entity_workstreams

Per-entity work items within a case.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| workstream_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| case_id | uuid | NOT NULL | | FK to cases |
| entity_id | uuid | NOT NULL | | FK to entities |
| status | varchar(30) | NOT NULL | 'PENDING' | PENDING, COLLECT, VERIFY, SCREEN, ASSESS, COMPLETE, BLOCKED, ENHANCED_DD |
| discovery_source_workstream_id | uuid | | | FK to self - parent workstream that discovered this entity |
| discovery_reason | varchar(100) | | | Why entity was discovered |
| risk_rating | varchar(20) | | | Entity risk rating |
| risk_factors | jsonb | | '[]' | Array of risk factors |
| created_at | timestamptz | NOT NULL | now() | |
| started_at | timestamptz | | | Work started |
| completed_at | timestamptz | | | Work completed |
| blocked_at | timestamptz | | | When blocked |
| blocked_reason | text | | | Why blocked |
| requires_enhanced_dd | boolean | | false | Enhanced due diligence required |
| is_ubo | boolean | | false | Is this entity a UBO |
| ownership_percentage | numeric(5,2) | | | Ownership percentage if applicable |
| discovery_depth | integer | | 1 | Depth in ownership chain |

**Unique constraint**: (case_id, entity_id)
**Indexes**: case_id, entity_id, status, discovery_source_workstream_id

#### red_flags

Risk indicators raised during KYC.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| red_flag_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| case_id | uuid | NOT NULL | | FK to cases |
| workstream_id | uuid | | | FK to entity_workstreams (optional) |
| flag_type | varchar(50) | NOT NULL | | Type of red flag |
| severity | varchar(20) | NOT NULL | | SOFT, ESCALATE, HARD_STOP |
| status | varchar(20) | NOT NULL | 'OPEN' | OPEN, UNDER_REVIEW, MITIGATED, WAIVED, BLOCKING, CLOSED |
| description | text | NOT NULL | | Description of the flag |
| source | varchar(50) | | | Source system/rule |
| source_reference | text | | | Reference ID in source |
| raised_at | timestamptz | NOT NULL | now() | When raised |
| raised_by | uuid | | | Who raised it |
| reviewed_at | timestamptz | | | When reviewed |
| reviewed_by | uuid | | | Who reviewed |
| resolved_at | timestamptz | | | When resolved |
| resolved_by | uuid | | | Who resolved |
| resolution_type | varchar(30) | | | How resolved |
| resolution_notes | text | | | Resolution details |
| waiver_approved_by | uuid | | | Who approved waiver |
| waiver_justification | text | | | Waiver justification |

**Indexes**: case_id, workstream_id, flag_type, severity, status

#### doc_requests

Document collection requests per workstream.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| request_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| workstream_id | uuid | NOT NULL | | FK to entity_workstreams |
| doc_type | varchar(50) | NOT NULL | | Document type code |
| status | varchar(20) | NOT NULL | 'REQUIRED' | REQUIRED, REQUESTED, RECEIVED, UNDER_REVIEW, VERIFIED, REJECTED, WAIVED, EXPIRED |
| required_at | timestamptz | NOT NULL | now() | When requirement created |
| requested_at | timestamptz | | | When requested from client |
| due_date | date | | | Due date for document |
| received_at | timestamptz | | | When received |
| reviewed_at | timestamptz | | | When reviewed |
| verified_at | timestamptz | | | When verified |
| document_id | uuid | | | FK to document_catalog |
| reviewer_id | uuid | | | Who reviewed |
| rejection_reason | text | | | Why rejected |
| verification_notes | text | | | Verification notes |
| is_mandatory | boolean | | true | Is document mandatory |
| priority | varchar(10) | | 'NORMAL' | Document priority |

**Indexes**: workstream_id, doc_type, status, due_date

#### screenings

Screening requests and results per workstream.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| screening_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| workstream_id | uuid | NOT NULL | | FK to entity_workstreams |
| screening_type | varchar(30) | NOT NULL | | SANCTIONS, PEP, ADVERSE_MEDIA, CREDIT, CRIMINAL, REGULATORY, CONSOLIDATED |
| provider | varchar(50) | | | Screening provider |
| status | varchar(20) | NOT NULL | 'PENDING' | PENDING, RUNNING, CLEAR, HIT_PENDING_REVIEW, HIT_CONFIRMED, HIT_DISMISSED, ERROR, EXPIRED |
| requested_at | timestamptz | NOT NULL | now() | When requested |
| completed_at | timestamptz | | | When completed |
| expires_at | timestamptz | | | When expires |
| result_summary | varchar(100) | | | Brief result |
| result_data | jsonb | | | Full result data |
| match_count | integer | | 0 | Number of matches |
| reviewed_by | uuid | | | Who reviewed |
| reviewed_at | timestamptz | | | When reviewed |
| review_notes | text | | | Review notes |
| red_flag_id | uuid | | | FK to red_flags if hit raised flag |

**Indexes**: workstream_id, screening_type, status

#### case_events

Audit trail for case activities.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| event_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| case_id | uuid | NOT NULL | | FK to cases |
| workstream_id | uuid | | | FK to entity_workstreams (optional) |
| event_type | varchar(50) | NOT NULL | | Event type |
| event_data | jsonb | | '{}' | Event payload |
| actor_id | uuid | | | Who performed action |
| actor_type | varchar(20) | | 'USER' | USER, SYSTEM, RULE |
| occurred_at | timestamptz | NOT NULL | now() | When occurred |
| comment | text | | | Optional comment |

**Indexes**: case_id, workstream_id, event_type, occurred_at DESC

#### rule_executions

Audit log for rules engine executions.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| execution_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| case_id | uuid | NOT NULL | | FK to cases |
| workstream_id | uuid | | | FK to entity_workstreams (optional) |
| rule_name | varchar(100) | NOT NULL | | Rule that was evaluated |
| trigger_event | varchar(50) | NOT NULL | | Event that triggered rule |
| condition_matched | boolean | NOT NULL | | Whether conditions matched |
| actions_executed | jsonb | | '[]' | Actions that were executed |
| context_snapshot | jsonb | | '{}' | Context at time of execution |
| executed_at | timestamptz | NOT NULL | now() | When executed |

#### approval_requests

Escalation and approval workflow.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| approval_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| case_id | uuid | NOT NULL | | FK to cases |
| workstream_id | uuid | | | FK to entity_workstreams (optional) |
| request_type | varchar(50) | NOT NULL | | Type of approval needed |
| requested_by | varchar(255) | | | Who requested |
| requested_at | timestamptz | NOT NULL | now() | When requested |
| approver | varchar(255) | | | Who approved/rejected |
| decision | varchar(20) | | | APPROVED, REJECTED, PENDING |
| decision_at | timestamptz | | | When decided |
| comments | text | | | Decision comments |

### KYC Case Views

#### v_case_summary

Aggregated case view with counts.

```sql
SELECT c.*, 
       COUNT(DISTINCT w.workstream_id) as workstream_count,
       COUNT(DISTINCT r.red_flag_id) FILTER (WHERE r.status = 'OPEN') as open_flags,
       MIN(c.sla_deadline) as next_deadline
FROM kyc.cases c
LEFT JOIN kyc.entity_workstreams w ON c.case_id = w.case_id
LEFT JOIN kyc.red_flags r ON c.case_id = r.case_id
GROUP BY c.case_id
```

#### v_workstream_detail

Workstream view with entity details.

```sql
SELECT w.*, e.name as entity_name, et.name as entity_type
FROM kyc.entity_workstreams w
JOIN entities e ON w.entity_id = e.entity_id
JOIN entity_types et ON e.entity_type_id = et.entity_type_id
```

### Investor Registry

### share_classes

Fund share class master data.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| id | uuid | NOT NULL | uuid_generate_v4() | Primary key |
| cbu_id | uuid | NOT NULL | | FK to cbus (the fund) |
| entity_id | uuid | YES | | FK to entities - legal entity that issues this share class |
| name | varchar(255) | NOT NULL | | Share class name (e.g., "Class A EUR") |
| isin | varchar(12) | | | ISIN code |
| currency | char(3) | NOT NULL | 'EUR' | Share class currency |
| class_category | varchar(20) | NO | 'FUND' | CORPORATE = company ownership, FUND = investment fund |
| fund_type | varchar(50) | | | HEDGE_FUND, UCITS, AIFMD, PRIVATE_EQUITY, REIT |
| fund_structure | varchar(50) | | | OPEN_ENDED, CLOSED_ENDED |
| investor_eligibility | varchar(50) | | | RETAIL, PROFESSIONAL, QUALIFIED |
| nav_per_share | numeric(20,6) | | | Current NAV |
| nav_date | date | | | NAV valuation date |
| management_fee_bps | integer | | | Management fee in basis points |
| performance_fee_bps | integer | | | Performance fee in basis points |
| high_water_mark | boolean | | false | Performance fee uses high water mark |
| hurdle_rate | numeric(5,2) | | | Hurdle rate for performance fee |
| subscription_frequency | varchar(50) | | | Daily, Weekly, Monthly |
| redemption_frequency | varchar(50) | | | Daily, Weekly, Monthly |
| redemption_notice_days | integer | | | Notice period for redemptions |
| lock_up_period_months | integer | | | Lock-up period for hedge funds |
| gate_percentage | numeric(5,2) | | | Redemption gate percentage |
| minimum_investment | numeric(20,2) | | | Minimum investment amount |
| status | varchar(50) | NOT NULL | 'active' | active, closed |
| created_at | timestamptz | NOT NULL | now() | |
| updated_at | timestamptz | NOT NULL | now() | |

**Unique constraint**: (cbu_id, isin)

### holdings

Investor positions in share classes.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| id | uuid | NOT NULL | uuid_generate_v4() | Primary key |
| share_class_id | uuid | NOT NULL | | FK to share_classes |
| investor_entity_id | uuid | NOT NULL | | FK to entities (the investor) |
| units | numeric(20,6) | NOT NULL | 0 | Number of units held |
| cost_basis | numeric(20,2) | | | Total cost basis |
| acquisition_date | date | | | Initial acquisition date |
| status | varchar(50) | NOT NULL | 'active' | active, closed |
| created_at | timestamptz | NOT NULL | now() | |
| updated_at | timestamptz | NOT NULL | now() | |

**Unique constraint**: (share_class_id, investor_entity_id)

### movements

Subscription, redemption, and transfer transactions.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| id | uuid | NOT NULL | uuid_generate_v4() | Primary key |
| holding_id | uuid | NOT NULL | | FK to holdings |
| movement_type | varchar(50) | NOT NULL | | subscription, redemption, transfer_in, transfer_out, dividend, adjustment |
| units | numeric(20,6) | NOT NULL | | Number of units |
| price_per_unit | numeric(20,6) | | | Price at transaction |
| amount | numeric(20,2) | | | Total amount |
| currency | char(3) | NOT NULL | 'EUR' | Transaction currency |
| trade_date | date | NOT NULL | | Trade date |
| settlement_date | date | | | Settlement date |
| status | varchar(50) | NOT NULL | 'pending' | pending, confirmed, settled, cancelled, failed |
| reference | varchar(100) | | | External reference |
| notes | text | | | Transaction notes |
| created_at | timestamptz | NOT NULL | now() | |
| updated_at | timestamptz | NOT NULL | now() | |

**Check constraints**:
- movement_type IN ('subscription', 'redemption', 'transfer_in', 'transfer_out', 'dividend', 'adjustment')
- status IN ('pending', 'confirmed', 'settled', 'cancelled', 'failed')


## Table Count by Category

| Category | Tables | Examples |
|----------|--------|----------|
| Core | 6 | cbus, entities, entity_types, roles, cbu_entity_roles, cbu_change_log |
| Entity Extensions | 4 | entity_proper_persons, entity_limited_companies, entity_partnerships, entity_trusts |
| Documents | 4 | document_catalog, document_types, document_attribute_mappings, document_attribute_links |
| Products/Services | 8 | products, services, service_delivery_map, cbu_resource_instances |
| Reference Data | 6 | master_jurisdictions, currencies, roles, dictionary, risk_bands, client_types |
| DSL/Execution | 10 | dsl_instances, dsl_instance_versions, dsl_execution_log, dsl_domains, dsl_sessions |
| Onboarding | 4 | onboarding_requests, onboarding_products, service_option_definitions, service_option_choices |
| Attributes | 5 | attribute_registry, attribute_values_typed, attribute_dictionary, attribute_observations, client_allegations |
| Evidence/Proofs | 4 | cbu_evidence, ubo_evidence, ubo_snapshots, ubo_snapshot_comparisons |
| Decision Support | 3 | case_decision_thresholds, case_evaluation_snapshots, redflag_score_config |
| UBO | 3 | ubo_registry, entity_relationships, cbu_relationship_verification |
| Thresholds | 4 | threshold_factors, threshold_requirements, requirement_acceptable_docs, screening_requirements |
| Other | 32 | Various support tables |
| **ob-poc Total** | **83** | |
| **Custody** | **17** | cbu_instrument_universe, cbu_ssi, ssi_booking_rules, isda_agreements, csa_agreements |
| **KYC** | **12** | cases, entity_workstreams, red_flags, doc_requests, screenings, share_classes, holdings, movements |
| **Public** | **10** | rules, rule_versions, business_attributes, derived_attributes, credentials_vault |
| **Grand Total** | **122** | |

## Rebuilding the Schema

```bash
# Full schema rebuild
psql -d data_designer -f schema_export.sql

```

## Workflow Orchestration

The workflow module provides stateful orchestration for KYC, UBO, and onboarding processes. Workflows are defined in YAML and executed by a state machine engine.

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           WORKFLOW ENGINE                                   │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │
│  │  Workflow   │  │   State     │  │ Transition  │  │  Blocker    │        │
│  │ Definition  │  │  Tracker    │  │   Guard     │  │  Resolver   │        │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘        │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                           DSL EXECUTION                                     │
│              Workflow emits DSL → Executor runs → Results fed back          │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Key Concepts

| Concept | Description |
|---------|-------------|
| **WorkflowDefinition** | YAML-defined state machine with states, transitions, guards |
| **WorkflowInstance** | Running instance of a workflow for a specific subject (CBU, entity, case) |
| **Guard** | Condition that must be met before a transition can occur |
| **Blocker** | Actionable item preventing advancement (includes resolution DSL verb) |
| **Auto-transition** | Automatic state change when guard passes |
| **Manual transition** | Requires explicit user action |

### Module Structure

```
rust/src/workflow/
├── mod.rs              # Module exports
├── definition.rs       # WorkflowDefinition, parsing YAML
├── state.rs            # WorkflowInstance, StateTransition, Blocker types
├── guards.rs           # GuardEvaluator (uses requirements + custom guards)
├── requirements.rs     # RequirementEvaluator (YAML-driven checks)
├── engine.rs           # WorkflowEngine core logic
└── repository.rs       # Database persistence

rust/config/workflows/
└── kyc_onboarding.yaml # KYC onboarding workflow definition
```

### YAML-Driven Requirements

Guards are now data-driven from YAML requirements. Add new requirements by editing YAML, not Rust code.

**Requirement Types:**

| Type | YAML Example | What It Checks |
|------|--------------|----------------|
| `role_count` | `{type: role_count, role: DIRECTOR, min: 1}` | Minimum entities with role |
| `all_entities_screened` | `{type: all_entities_screened}` | All linked entities screened |
| `document_set` | `{type: document_set, documents: [CERT_OF_INC, REG_OF_DIRS]}` | CBU-level docs present |
| `per_entity_document` | `{type: per_entity_document, entity_type: DIRECTOR, documents: [PASSPORT]}` | Docs per entity with role |
| `ownership_complete` | `{type: ownership_complete, threshold: 100}` | Ownership sums to threshold |
| `all_ubos_verified` | `{type: all_ubos_verified}` | All UBOs verified/proven |
| `no_open_alerts` | `{type: no_open_alerts}` | No unresolved screening hits |
| `case_checklist_complete` | `{type: case_checklist_complete}` | All doc requests fulfilled |

**Example YAML:**
```yaml
requirements:
  ENTITY_COLLECTION:
    - type: role_count
      role: DIRECTOR
      min: 1
      description: At least one director
    - type: role_count
      role: AUTHORIZED_SIGNATORY
      min: 1
  
  DOCUMENT_COLLECTION:
    - type: document_set
      documents:
        - CERTIFICATE_OF_INCORPORATION
        - REGISTER_OF_DIRECTORS
    - type: per_entity_document
      entity_type: DIRECTOR
      documents:
        - PASSPORT
```

**How It Works:**
1. Engine calls `GuardEvaluator.evaluate_for_transition()`
2. Evaluator gets requirements for target state from YAML
3. `RequirementEvaluator` checks each requirement against DB
4. If transition has a named guard, also evaluates that
5. Returns combined blockers from requirements + custom guard

### Blocker Types

| Type | Description | Resolution Verb |
|------|-------------|-----------------|
| `MissingRole` | Required role not assigned | `cbu.assign-role` |
| `MissingDocument` | Required document not present | `document.catalog` |
| `PendingScreening` | Entity needs screening | `case-screening.run` |
| `UnresolvedAlert` | Screening hit needs review | `case-screening.review-hit` |
| `IncompleteOwnership` | Ownership doesn't sum to 100% | `ubo.add-ownership` |
| `UnverifiedUbo` | UBO needs verification | `ubo.verify-ubo` |
| `ManualApprovalRequired` | Analyst approval needed | `kyc-case.update-status` |

### KYC Onboarding Workflow States

```
INTAKE → ENTITY_COLLECTION → SCREENING → DOCUMENT_COLLECTION → UBO_DETERMINATION → REVIEW → APPROVED
                                                                                          ↓
                                                                                      REJECTED
                                                                                          ↓
                                                                                    REMEDIATION
```

| State | Description | Guard to Exit |
|-------|-------------|---------------|
| `INTAKE` | Initial data gathering | Auto (no guard) |
| `ENTITY_COLLECTION` | Collecting directors, UBOs, signatories | `entities_complete` |
| `SCREENING` | Running AML/PEP/sanctions screening | `screening_complete` |
| `DOCUMENT_COLLECTION` | Gathering required documents | `documents_complete` |
| `UBO_DETERMINATION` | Calculating and verifying beneficial owners | `ubo_complete` |
| `REVIEW` | Analyst review of complete package | `review_approved` / `review_rejected` |
| `APPROVED` | Onboarding complete (terminal) | - |
| `REJECTED` | Onboarding rejected (terminal) | - |
| `REMEDIATION` | Issues found, need correction | - |

### Example Usage

```rust
use ob_poc::workflow::{WorkflowEngine, WorkflowLoader};

// Load workflow definitions
let definitions = WorkflowLoader::load_from_dir(Path::new("config/workflows"))?;
let engine = WorkflowEngine::new(pool, definitions);

// Start a workflow for a CBU
let instance = engine.start_workflow("kyc_onboarding", "cbu", cbu_id, None).await?;

// Get current status with blockers
let status = engine.get_status(instance.instance_id).await?;
println!("State: {}, Blockers: {:?}", status.current_state, status.blockers);

// Try to advance (evaluates guards, auto-transitions if possible)
let instance = engine.try_advance(instance.instance_id).await?;

// Manual transition (e.g., analyst approval)
let instance = engine.transition(
    instance.instance_id,
    "APPROVED",
    Some("analyst@example.com".to_string()),
    Some("All requirements met".to_string())
).await?;
```

### Database Tables

| Table | Purpose |
|-------|---------|
| `workflow_instances` | Running workflow instances with state, history, blockers |
| `workflow_audit_log` | Audit trail of all state transitions |

## MCP Server Tools

For Claude Desktop integration. The MCP server (`dsl_mcp`) provides tools for DSL generation and execution.

### Core DSL Tools

| Tool | Description |
|------|-------------|
| `dsl_validate` | Parse and validate DSL syntax/semantics |
| `dsl_execute` | Execute DSL against database (with dry_run option) |
| `dsl_plan` | Show execution plan without running |
| `dsl_lookup` | **Look up real database IDs** - prevents UUID hallucination |
| `dsl_complete` | Get completions for verbs, domains, products, roles |
| `dsl_signature` | Get verb signature with parameters and types |

### Data Access Tools

| Tool | Description |
|------|-------------|
| `cbu_get` | Get CBU with entities, roles, documents, screenings |
| `cbu_list` | List/search CBUs with filtering |
| `entity_get` | Get entity details with relationships |
| `entity_search` | **Smart entity search with disambiguation** - fuzzy search with enriched context |
| `verbs_list` | List available DSL verbs (optionally by domain) |
| `schema_info` | Get entity types, roles, document types |

### Key Tool: `entity_search`

The `entity_search` tool provides intelligent entity lookup with rich context for disambiguation. It uses EntityGateway for fuzzy search and enriches results with roles, relationships, and dates.

```json
// Basic search
{"entity_type": "person", "query": "John Smith"}

// Search with conversation context for smarter resolution
{
  "entity_type": "person",
  "query": "John",
  "conversation_hints": {
    "mentioned_roles": ["DIRECTOR"],
    "mentioned_cbu": "Apex Fund",
    "mentioned_nationality": "US"
  }
}
```

**Response includes:**
- `matches[]` - Enriched results with disambiguation labels
- `resolution` - Suggested action: `auto_resolve`, `ask_user`, `suggest_create`, or `need_more_info`
- `confidence` - Resolution confidence: `high`, `medium`, `low`, or `none`

**Entity context returned for each match:**
- Nationality, DOB (for persons)
- Jurisdiction, registration number (for companies)
- Roles and CBU associations
- Ownership relationships
- Created/updated timestamps

**Disambiguation labels** are human-readable summaries like:
- `"John Smith (US) b.1975 - Director at Apex Fund"`
- `"Holdings Ltd (LU) #B123456 - 2 roles"`

### Key Tool: `dsl_lookup`

The `dsl_lookup` tool is critical for preventing UUID hallucination. **Always use this tool before generating DSL that references existing entities.**

```json
// Example: Look up a CBU by name
{"lookup_type": "cbu", "search": "Apex"}

// Example: Look up entities of a specific type
{"lookup_type": "entity", "filters": {"entity_type": "proper_person"}}

// Example: Look up products
{"lookup_type": "product"}
```

Supported lookup types: `cbu`, `entity`, `document`, `product`, `service`, `kyc_case`

### Key Tool: `dsl_signature`

Get full parameter information for any verb:

```json
{"verb": "cbu.add-product"}
// Returns: parameters with types, required flags, descriptions, and example usage
```

### MCP Module Structure

```
rust/src/mcp/
├── mod.rs              # Module exports
├── tools.rs            # Tool schema definitions (JSON Schema)
├── handlers.rs         # Tool implementation handlers
├── types.rs            # Agent-friendly response types
├── enrichment.rs       # EntityEnricher - fetches rich context for disambiguation
└── resolution.rs       # ResolutionStrategy - determines auto-resolve vs ask user
```

**Key Types:**

| Type | Module | Description |
|------|--------|-------------|
| `EntityEnricher` | enrichment | Fetches roles, ownership, dates from DB |
| `EntityContext` | enrichment | Rich context (nationality, DOB, jurisdiction, roles) |
| `ResolutionStrategy` | resolution | Analyzes matches to suggest action |
| `ResolutionConfidence` | resolution | High, Medium, Low, None |
| `SuggestedAction` | resolution | AutoResolve, AskUser, SuggestCreate, NeedMoreInfo |
| `ConversationContext` | resolution | Hints from conversation (roles, CBU, nationality) |

### Template Tools

Templates are pre-built DSL patterns for common multi-step operations. They capture domain lifecycle patterns and expand to reviewable DSL source text.

| Tool | Description |
|------|-------------|
| `template_list` | List/search templates by tag, blocker, workflow state, or text |
| `template_get` | Get full template details with params, effects, and DSL body |
| `template_expand` | Expand template to DSL source text with parameter substitution |

**Usage Flow:**

```json
// 1. Find templates that resolve a blocker
{"tool": "template_list", "args": {"blocker": "missing_role:DIRECTOR"}}

// 2. Get template details
{"tool": "template_get", "args": {"template_id": "onboard-director"}}

// 3. Expand with parameters
{"tool": "template_expand", "args": {
  "template_id": "onboard-director",
  "cbu_id": "uuid-of-cbu",
  "case_id": "uuid-of-case",
  "params": {
    "name": "John Smith",
    "date_of_birth": "1975-03-15",
    "nationality": "GB"
  }
}}
// Returns: { "dsl": "(let [person ...]...)", "complete": true, ... }
```

**Template Expansion:**
- Parameters resolved in order: explicit → session context → defaults
- Returns `missing_params` with prompts if required params not provided
- Returns reviewable/editable DSL source text

### Batch Execution Tools

Batch tools enable template-driven iteration over entity sets. They operate on the **shared UI SessionStore** - the single source of truth for all session state.

| Tool | Description |
|------|-------------|
| `batch_start` | Start batch mode with a template, initialize key sets |
| `batch_add_entities` | Add resolved entities to a parameter's key set |
| `batch_confirm_keyset` | Mark a key set as complete, advance phase |
| `batch_set_scalar` | Set a scalar (non-entity) parameter value |
| `batch_get_state` | Get current batch execution state |
| `batch_expand_current` | Expand template for current batch item |
| `batch_record_result` | Record success/failure for current item, advance index |
| `batch_skip_current` | Skip current item with reason |
| `batch_cancel` | Cancel batch execution, return to chat mode |

**Architecture:**

```
┌─────────────────────────────────────────────────────────────────┐
│              UI Server Session (api/session.rs)                  │
│              SessionStore = Arc<RwLock<HashMap<Uuid, AgentSession>>>
│              - AgentSession.context.template_execution           │
│              - AgentSession.context.mode                         │
└─────────────────────────────────────────────────────────────────┘
                              │
          ┌───────────────────┼───────────────────┐
          ▼                   ▼                   ▼
┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐
│   Web API Routes │ │    MCP Tools     │ │     egui UI      │
│ state.sessions   │ │ self.sessions    │ │ fetch from API   │
└──────────────────┘ └──────────────────┘ └──────────────────┘
```

**Usage Modes:**

| Mode | Constructor | Batch Tools |
|------|-------------|-------------|
| Standalone MCP (Claude Desktop) | `ToolHandlers::new(pool)` | Error - no session store |
| Integrated (web server) | `ToolHandlers::with_sessions(pool, sessions)` | Full functionality |

**Batch Workflow:**
1. Agent selects template based on user intent
2. `batch_start` initializes template context in session
3. Agent collects entities via `batch_add_entities` (batch params iterate, shared params are constant)
4. `batch_confirm_keyset` marks each param complete
5. `batch_expand_current` generates DSL for current batch item
6. User reviews/edits DSL, then executes
7. `batch_record_result` tracks outcome, advances to next item
8. Repeat until all items processed or `batch_cancel`

## Template System

Templates capture domain lifecycle patterns - chained verb sequences that accomplish business goals. They serve as prompt enhancement for agent DSL generation.

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         USER / AGENT                            │
│  "Add John Doe as director to Apex Fund"                        │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Template Selection                           │
│  1. Parse intent: action=add_director, person="John Doe"        │
│  2. Check if person exists → NOT FOUND (needs creation)         │
│  3. Select template: onboard-director                           │
│  4. Check params: missing dob, nationality → prompt user        │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Template Expansion                           │
│  Substitute parameters → DSL SOURCE TEXT                        │
│  Agent can REVIEW and EDIT before execution                     │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Normal DSL Pipeline                          │
│  Parse → Entity Resolution → Validate → Execute                 │
└─────────────────────────────────────────────────────────────────┘
```

### Directory Structure

```
rust/config/templates/
├── director/
│   └── onboard-director.yaml
├── signatory/
│   └── onboard-signatory.yaml
├── ubo/
│   ├── add-ownership.yaml
│   ├── trace-chains.yaml
│   └── register-ubo.yaml
├── screening/
│   ├── run-entity-screening.yaml
│   └── review-screening-hit.yaml
├── documents/
│   ├── catalog-document.yaml
│   └── request-documents.yaml
└── case/
    ├── create-kyc-case.yaml
    ├── escalate-case.yaml
    └── approve-case.yaml
```

### Template YAML Schema

```yaml
template: onboard-director
version: 1

metadata:
  name: Onboard Director
  summary: Add a natural person as director with full KYC setup
  description: |
    Creates person entity, assigns DIRECTOR role, creates workstream,
    requests documents, and initiates screening.
  when_to_use:
    - Adding a new director who doesn't exist in the system
    - Workflow blocker shows "missing_role:DIRECTOR"
  when_not_to_use:
    - Person already exists (use cbu.assign-role directly)
  effects:
    - New person entity created
    - DIRECTOR role assigned to CBU
    - Entity workstream created in KYC case
  next_steps:
    - Upload/catalog documents for the person
    - Review screening results if hits found

tags:
  - director
  - person
  - role
  - kyc

workflow_context:
  applicable_workflows:
    - kyc_onboarding
  applicable_states:
    - ENTITY_COLLECTION
  resolves_blockers:
    - missing_role:DIRECTOR

params:
  cbu_id:
    type: cbu_ref
    required: true
    source: session
  name:
    type: string
    required: true
    prompt: "Director's full legal name"
    example: "John Smith"
  date_of_birth:
    type: date
    required: true
    prompt: "Date of birth"
  nationality:
    type: country_code
    required: true
    prompt: "Nationality (ISO 2-letter code)"

body: |
  (let [person (entity.create-proper-person
                 :name "$name"
                 :date-of-birth "$date_of_birth"
                 :nationality "$nationality")]
    (cbu.assign-role :cbu "$cbu_id" :entity person :role DIRECTOR)
    (entity-workstream.create :case "$case_id" :entity person)
    (case-screening.run :case "$case_id" :entity person))

outputs:
  person:
    type: entity_ref
    description: Created person entity ID
```

### Rust Module Structure

```
rust/src/templates/
├── mod.rs              # Module exports
├── definition.rs       # TemplateDefinition, ParamDefinition, WorkflowContext
├── registry.rs         # TemplateRegistry with multi-index lookup
├── expander.rs         # TemplateExpander for parameter substitution
└── error.rs            # TemplateError enum
```

### Key Types

| Type | Description |
|------|-------------|
| `TemplateDefinition` | Full template with metadata, params, body, outputs |
| `TemplateMetadata` | Name, summary, when_to_use, effects, next_steps |
| `ParamDefinition` | Type, required, source, default, prompt, validation |
| `WorkflowContext` | Applicable workflows, states, resolves_blockers |
| `TemplateRegistry` | Indexes by tag, blocker, workflow_state |
| `TemplateExpander` | Parameter substitution with context resolution |
| `ExpansionResult` | DSL text, filled_params, missing_params, outputs |

### Parameter Resolution Order

1. **Explicit params** - Values provided in the expand call
2. **Session context** - current_cbu, current_case from session
3. **Default values** - From param definition (supports `$other_param` refs, `today`)

### Available Templates

| Template | Description | Resolves Blockers |
|----------|-------------|-------------------|
| `onboard-director` | Add new director with full KYC | `missing_role:DIRECTOR` |
| `onboard-signatory` | Add authorized signatory | `missing_role:AUTHORIZED_SIGNATORY` |
| `add-ownership` | Record ownership relationship | `incomplete_ownership` |
| `trace-chains` | Trace ownership to natural persons | `ubo_not_traced` |
| `register-ubo` | Register identified UBO | `ubo_not_registered` |
| `run-entity-screening` | Run AML/PEP/sanctions screening | `screening_required` |
| `review-screening-hit` | Disposition a screening hit | `unresolved_alert` |
| `catalog-document` | Catalog received document | `missing_document` |
| `request-documents` | Create document requests | `docs_not_requested` |
| `create-kyc-case` | Initialize KYC case | `no_kyc_case` |
| `escalate-case` | Escalate to higher authority | `escalation_required` |
| `approve-case` | Final case approval | `pending_approval` |

### Template Test Harness

The `template_harness` binary validates all templates through the DSL pipeline.

**Usage:**

```bash
cd rust/

# Basic run - load, expand, parse, compile all templates
cargo run --bin template_harness

# Verbose - show expanded DSL for each template
cargo run --bin template_harness -- --verbose

# JSON output for scripting
cargo run --bin template_harness -- --json

# Execute against database (requires DATABASE_URL)
DATABASE_URL="postgresql:///data_designer" \
cargo run --features database --bin template_harness -- --execute

# Custom templates directory
cargo run --bin template_harness -- --templates-dir /path/to/templates
```

**CLI Options:**

| Flag | Description |
|------|-------------|
| `--verbose`, `-v` | Show expanded DSL for each template |
| `--json` | Output results as JSON |
| `--execute`, `-e` | Execute DSL against database |
| `--templates-dir`, `-d` | Override templates directory path |

**Pipeline:**

```
Load templates → Expand with sample params → Parse DSL → Compile → (Execute)
```

**Module:** `rust/src/templates/harness.rs`

| Type | Description |
|------|-------------|
| `TemplateTestResult` | Per-template result with expansion/parse/compile/execute status |
| `HarnessResult` | Aggregate results with summary stats |
| `get_sample_params()` | Sample parameters for all 12 templates |
| `run_harness()` | Full pipeline with optional DB execution |
| `run_harness_no_db()` | Pipeline without database execution |

## Agentic DSL Generation

The `rust/src/agentic/` module provides AI-powered DSL generation from natural language, specifically optimized for custody onboarding scenarios.

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     USER REQUEST                                 │
│  "Onboard BlackRock for US and UK equities with IRS to Goldman" │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              PHASE 1: INTENT EXTRACTION (Claude API)            │
│  Natural language → OnboardingIntent struct                     │
│  rust/src/agentic/generator.rs (IntentExtractor)               │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              PHASE 2: PATTERN CLASSIFICATION (Deterministic)    │
│  OnboardingIntent → OnboardingPattern                          │
│  - SimpleEquity: Single market, single currency                │
│  - MultiMarket: Multiple markets or cross-currency             │
│  - WithOtc: OTC derivatives requiring ISDA/CSA                 │
│  rust/src/agentic/patterns.rs                                  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              PHASE 3: REQUIREMENT PLANNING (Deterministic Rust) │
│  Intent → OnboardingPlan with:                                  │
│  - CBU details, entity lookups                                  │
│  - Universe entries (market × instrument × currency)            │
│  - SSI requirements                                             │
│  - Booking rules with priorities and fallbacks                  │
│  - ISDA/CSA requirements for OTC                               │
│  rust/src/agentic/planner.rs (RequirementPlanner)              │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              PHASE 4: DSL GENERATION (Claude API)               │
│  OnboardingPlan → DSL source code                               │
│  Full verb schemas included in context                          │
│  Pattern-specific few-shot examples                             │
│  rust/src/agentic/generator.rs (DslGenerator)                  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              PHASE 5: VALIDATION + RETRY LOOP                   │
│  Parse → CSG Lint → Compile                                     │
│  If errors: feed back to Claude (max 3 retries)                │
│  rust/src/agentic/validator.rs, feedback.rs                    │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              PHASE 6: EXECUTION (Optional)                      │
│  Execute validated DSL against database                         │
│  Return created entity UUIDs                                    │
└─────────────────────────────────────────────────────────────────┘
```

### Module Structure

```
rust/src/agentic/
├── mod.rs              # Module exports
├── intent.rs           # OnboardingIntent, ClientIntent, MarketIntent, etc.
├── patterns.rs         # OnboardingPattern enum (SimpleEquity, MultiMarket, WithOtc)
├── planner.rs          # RequirementPlanner - deterministic business logic
├── generator.rs        # IntentExtractor & DslGenerator (Claude API)
├── validator.rs        # AgentValidator - wraps existing parser/linter
├── feedback.rs         # FeedbackLoop - retry logic
├── orchestrator.rs     # AgentOrchestrator - coordinates full pipeline
├── prompts/
│   └── intent_extraction_system.md   # Claude prompt for intent extraction
├── schemas/
│   ├── custody_verbs.md              # Verb reference for DSL generation
│   └── reference_data.md             # Markets, BICs, currencies
└── examples/
    ├── simple_equity.dsl             # Single market example
    ├── multi_market.dsl              # Multi-market with cross-currency
    └── with_otc.dsl                  # OTC with ISDA/CSA
```

### CLI Usage (custody command)

```bash
# Generate custody DSL from natural language
dsl_cli custody -i "Set up Apex Capital for US equity trading"

# Show plan without generating DSL
dsl_cli custody -i "Onboard fund for US, UK, Germany equities" --plan-only

# Generate and execute against database
dsl_cli custody -i "Onboard TestFund for US equities" --execute

# Save to file
dsl_cli custody -i "..." -o output.dsl

# JSON output for scripting
dsl_cli custody -i "..." --format json
```

### Pattern Examples

**SimpleEquity** - Single market, single currency:
```
"Set up Apex Capital for US equity trading"
→ 1 universe entry, 1 SSI, 3 booking rules
```

**MultiMarket** - Multiple markets or cross-currency:
```
"Onboard Global Fund for UK and Germany equities with USD cross-currency"
→ 2 universe entries, 4 SSIs, 8 booking rules
```

**WithOtc** - OTC derivatives with ISDA/CSA:
```
"Onboard Pacific Fund for US equities plus IRS exposure to Morgan Stanley under NY law ISDA with VM"
→ Entity lookup, universe, SSIs, booking rules, ISDA, coverage, CSA
```

### Key Design Decisions

**No Vector DB**: Direct schema inclusion in prompts. The bounded domain (~30 verbs) fits easily in context - no probabilistic retrieval needed.

**Deterministic Planning**: Business logic for deriving SSIs and booking rules is pure Rust code, not AI. Only intent extraction and DSL generation use Claude.

**Pattern-Based Generation**: Classification enables pattern-specific few-shot examples and complexity scaling.

**Retry Loop**: Validation failures feed back to Claude with error messages for self-correction (max 3 attempts).

## DAG Execution Layer

The DSL execution system uses a Terraform-style execution model with dependency-aware ordering:

```
Source → Parse → AST → Enrich → Compile → Ops → DAG → Toposort → Execute
```

### Key Modules

| Module | File | Purpose |
|--------|------|---------|
| `Op` enum | `ops.rs` | Primitive operations (EnsureEntity, SetFK, LinkRole, etc.) |
| Compiler | `compiler.rs` | Transforms AST VerbCalls → Op sequence |
| DAG Builder | `dag.rs` | Builds dependency graph, performs toposort, detects cycles |
| Diagnostics | `diagnostics.rs` | Unified diagnostic types for LSP (Severity, DiagnosticCode, SourceSpan) |
| Execution Results | `execution_result.rs` | StepResult enum + ExecutionResults accumulator |
| REPL Session | `repl_session.rs` | Tracks executed blocks with undo support for incremental execution |
| Planning Facade | `planning_facade.rs` | Central `analyse_and_plan()` entrypoint |

### Op Types

```rust
pub enum Op {
    EnsureEntity { entity_type, key, attrs, source_stmt },
    SetFK { source, field, target, source_stmt },
    LinkRole { cbu, entity, role, source_stmt },
    AddOwnership { owner, owned, percentage, ownership_type, source_stmt },
    RegisterUBO { cbu, subject, ubo_person, qualifying_reason, source_stmt },
    UpsertDoc { doc_type, key, content, source_stmt },
    CreateCase { cbu, case_type, source_stmt },
    RunScreening { entity, screening_type, source_stmt },
    Materialize { source, sections, force, source_stmt },
    RequireRef { ref_type, value, source_stmt },
    // ... more variants
}
```

Each Op declares its dependencies via `dependencies()` and what it produces via `produces()`. The DAG builder uses this to determine execution order.

### Planning Facade

The `analyse_and_plan()` function is the central entrypoint for DSL analysis:

```rust
pub fn analyse_and_plan(input: PlanningInput) -> PlanningOutput {
    // 1. Parse DSL source → Program
    // 2. Compile to Ops
    // 3. Build DAG and toposort
    // 4. Detect cycles, collect diagnostics
    // 5. Return plan + diagnostics (even with errors, for LSP)
}
```

**PlanningInput** includes:
- `source: &str` - DSL source text
- `registry: Arc<RuntimeVerbRegistry>` - Verb definitions
- `executed_bindings: Option<&BindingContext>` - From REPL session
- `implicit_create_mode: ImplicitCreateMode` - Disabled/Enabled/Silent

**PlanningOutput** includes:
- `program: Program` - Parsed AST
- `diagnostics: Vec<Diagnostic>` - All errors/warnings/hints
- `plan: Option<PlannedExecution>` - Topologically sorted ops
- `was_reordered: bool` - True if source order differs from execution order

### REPL Session State

For incremental REPL execution, `ReplSession` tracks previously executed blocks:

```rust
let mut session = ReplSession::new();

// Execute first block
session.append_executed(program1, bindings1, types1);

// Next block can reference @bindings from previous blocks
let ctx = session.binding_context();
assert!(ctx.has("fund"));

// Undo last block
session.undo();
```

### Batch Resolution

The `gateway_resolver.rs` includes `batch_resolve()` for performance:

```rust
// Instead of 30 sequential gRPC calls (150ms)
// Batch by RefType: 5 calls (25ms) = 6x speedup
let results = resolver.batch_resolve(RefType::Entity, &values).await?;
```

## Intent Extraction Pipeline

The `rust/src/dsl_v2/` module includes an intent extraction system that uses Claude to extract structured intent from natural language, which is then used to generate DSL.

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     USER REQUEST                                 │
│  "Add John Smith as director of Apex Capital"                   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│         PHASE 1: INTENT EXTRACTION (Claude API)                  │
│  Natural language → DslIntentBatch (structured JSON)            │
│  AI extracts WHAT to do, not HOW to write DSL                   │
│  rust/src/dsl_v2/intent_extractor.rs                            │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│         PHASE 2: ENTITY RESOLUTION (EntityGateway)               │
│  ArgIntent lookups → Resolved UUIDs/codes                       │
│  - EntityLookup: "Apex Capital" → UUID                          │
│  - RefDataLookup: "director" → "DIRECTOR"                       │
│  rust/src/dsl_v2/gateway_resolver.rs                            │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│         PHASE 3: VALIDATION                                      │
│  Parse → CSG Lint → Ready for execution                         │
│  rust/src/dsl_v2/parser.rs, csg_linter.rs                       │
└─────────────────────────────────────────────────────────────────┘
```

### Key Types

**DslIntent** - A single DSL action:
```rust
pub struct DslIntent {
    pub verb: Option<String>,       // e.g., "cbu.assign-role"
    pub action: String,             // e.g., "assign" (for inference)
    pub domain: String,             // e.g., "cbu"
    pub args: HashMap<String, ArgIntent>,
    pub bind_as: Option<String>,    // Symbol binding
    pub source_text: Option<String>,
}
```

**ArgIntent** - Argument value types:
```rust
pub enum ArgIntent {
    Literal { value: serde_json::Value },           // Direct value
    SymbolRef { symbol: String },                   // @previously-bound
    EntityLookup { search_text: String, entity_type: Option<String> },
    RefDataLookup { search_text: String, ref_type: String },
}
```

### Module Structure

```
rust/src/dsl_v2/
├── intent.rs           # DslIntent, ArgIntent, DslIntentBatch
├── intent_extractor.rs # IntentExtractor (Claude API client)
├── gateway_resolver.rs # EntityGateway gRPC client for resolution
└── prompts/
    └── general_intent_extraction.md  # Claude extraction prompt
```

### Why This Design?

| Aspect | Agentic (text gen) | Intent Pipeline |
|--------|-------------------|-----------------|
| AI output | DSL source code | Structured JSON |
| Entity IDs | Can hallucinate | Resolved via EntityGateway |
| Validation | Post-hoc (retry loop) | Built into resolution |
| Determinism | Low (text varies) | High (structured) |

**Key insight**: AI is good at understanding intent, but prone to syntax errors and hallucinating IDs. By having AI produce structured data and using EntityGateway for resolution, we get reliable entity lookup.

### Two-Pass Resolution with Display Feedback Loop

The agent chat pipeline uses a **two-pass architecture** to maintain both human-readable display names and resolved UUIDs:

```
┌─────────────────────────────────────────────────────────────────┐
│  PASS 1: Intent Extraction + Unresolved Lookups                  │
│  LLM extracts VerbIntents with lookups containing search_text   │
│  e.g., EntityLookup { search_text: "Apex Fund", entity_type: "cbu" }
│  This is the "user intent" - no UUIDs yet                       │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  PASS 2: Resolution + Source Update Feedback                     │
│  EntityGateway resolves lookups → UUIDs                         │
│  BUT: We preserve the display_name alongside resolved_id        │
│                                                                  │
│  ParamValue::ResolvedEntity {                                   │
│      display_name: "Apex Fund",    // for user display          │
│      resolved_id: uuid,            // for execution             │
│  }                                                               │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  DUAL DSL GENERATION (from same intents)                         │
│                                                                  │
│  exec_dsl: (cbu.add-product :cbu-id "550e8400-..." :product "CUSTODY")
│            ↳ Used for parsing, validation, execution            │
│                                                                  │
│  user_dsl: (cbu.add-product :cbu-id "Apex Fund" :product "CUSTODY")
│            ↳ Displayed in chat UI, stored in session            │
└─────────────────────────────────────────────────────────────────┘
```

**Why this matters**: The semantic resolution phase (Pass 2) "fixes up" unresolved references, but we need to feed that resolution back to update the source representation for display. Without this feedback loop, users see UUIDs instead of entity names.

**Key types** (`rust/src/api/`):

| Type | File | Purpose |
|------|------|---------|
| `ParamValue::ResolvedEntity` | intent.rs | Carries both display_name and resolved_id |
| `ResolvedEntityLookup` | agent_service.rs | Resolution result with both values |
| `to_user_dsl_string()` | intent.rs | Renders display_name for user DSL |
| `build_user_dsl_program()` | dsl_builder.rs | Builds complete user-friendly DSL |

**Flow in agent_service.rs**:
1. `resolve_lookups()` → returns `HashMap<String, ResolvedEntityLookup>` with display names
2. `inject_resolved_ids()` → creates `ParamValue::ResolvedEntity` preserving both values
3. `build_dsl_program()` → exec_dsl with UUIDs
4. `build_user_dsl_program()` → user_dsl with display names
5. `build_response()` → stores user_dsl in session for chat display

## Environment Variables

```bash
# Required
DATABASE_URL="postgresql:///data_designer"

# LLM Backend Selection (default: anthropic)
AGENT_BACKEND=anthropic   # or "openai"

# Anthropic (required if AGENT_BACKEND=anthropic)
ANTHROPIC_API_KEY="sk-ant-..."
ANTHROPIC_MODEL="claude-sonnet-4-20250514"  # optional override

# OpenAI (required if AGENT_BACKEND=openai)
OPENAI_API_KEY="sk-..."
OPENAI_MODEL="gpt-4.1"  # optional, default: gpt-4.1

# Optional
DSL_CONFIG_DIR="/path/to/config"  # override config location
ENTITY_GATEWAY_URL="http://[::1]:50051"  # EntityGateway gRPC endpoint
```

## LLM Backend Architecture

The agentic DSL generation supports switchable LLM backends via the `AGENT_BACKEND` environment variable.

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                   Application Code                               │
│  (DslGenerator, IntentExtractor, AgentOrchestrator, etc.)       │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                   LlmClient Trait                                │
│  rust/src/agentic/llm_client.rs                                 │
│  - chat(system, user) → String                                  │
│  - chat_json(system, user) → String (JSON mode)                 │
└─────────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┴───────────────┐
              │                               │
              ▼                               ▼
┌─────────────────────────┐     ┌─────────────────────────┐
│    AnthropicClient      │     │     OpenAiClient        │
│  anthropic_client.rs    │     │   openai_client.rs      │
│  Claude API             │     │   OpenAI API            │
└─────────────────────────┘     └─────────────────────────┘
```

### Key Files

| File | Purpose |
|------|---------|
| `rust/src/agentic/backend.rs` | `AgentBackend` enum (Anthropic, OpenAi) |
| `rust/src/agentic/llm_client.rs` | `LlmClient` trait definition |
| `rust/src/agentic/anthropic_client.rs` | Anthropic Claude implementation |
| `rust/src/agentic/openai_client.rs` | OpenAI GPT implementation |
| `rust/src/agentic/client_factory.rs` | `create_llm_client()` factory |

### Usage

```rust
use crate::agentic::{create_llm_client, LlmClient};

// Create client based on AGENT_BACKEND env var
let client = create_llm_client()?;

// Use for chat
let response = client.chat(&system_prompt, &user_prompt).await?;

// Use for JSON output (OpenAI uses json_object mode)
let json_response = client.chat_json(&system_prompt, &user_prompt).await?;

// Check provider
println!("Using: {} ({})", client.provider_name(), client.model_name());
```

### Switching Backends

```bash
# Use Anthropic Claude (default)
export AGENT_BACKEND=anthropic
export ANTHROPIC_API_KEY=sk-ant-...

# Use OpenAI GPT
export AGENT_BACKEND=openai
export OPENAI_API_KEY=sk-...
export OPENAI_MODEL=gpt-4.1  # or gpt-4o, gpt-4.1-mini
```

### Notes

- The `generate_dsl_with_tools` endpoint still uses Anthropic-specific tool_use (no OpenAI equivalent yet)
- Chat session endpoints with tool use remain Anthropic-only
- Basic DSL generation (`/api/agent/generate`) works with both backends

## Complete DSL Verb Reference

This section provides a complete reference of all DSL verbs organized by domain.

### allegation

Client allegations - unverified claims that start the KYC process

| Verb | Description |
|------|-------------|
| `allegation.contradict` | Mark allegation as contradicted by evidence |
| `allegation.list-by-entity` | List allegations for an entity |
| `allegation.list-pending` | List pending allegations for a CBU |
| `allegation.mark-partial` | Mark allegation as partially verified |
| `allegation.record` | Record a client allegation about an entity attribute |
| `allegation.verify` | Mark allegation as verified by an observation |

### case-event

Audit trail for KYC case activities

| Verb | Description |
|------|-------------|
| `case-event.list-by-case` | List events for a case |
| `case-event.log` | Log a case event |

### case-screening

Sanctions, PEP, and adverse media screening for KYC workstreams

| Verb | Description |
|------|-------------|
| `case-screening.complete` | Record screening completion |
| `case-screening.list-by-workstream` | List screenings for a workstream |
| `case-screening.review-hit` | Review a screening hit |
| `case-screening.run` | Initiate a screening |

### cbu

Client Business Unit operations

| Verb | Description |
|------|-------------|
| `cbu.assign-role` | Assign a role to an entity within a CBU |
| `cbu.create` | Create a new Client Business Unit |
| `cbu.decide` | Record KYC/AML decision (APPROVED/REJECTED/REFERRED) for CBU collective state |
| `cbu.delete` | Delete a CBU |
| `cbu.ensure` | Create or update a CBU by natural key |
| `cbu.list` | List CBUs with optional filters |
| `cbu.parties` | List all parties (entities with their roles) for a CBU |
| `cbu.read` | Read a CBU by ID |
| `cbu.remove-role` | Remove a specific role from an entity within a CBU |
| `cbu.show` | Show full CBU structure with entities, roles, documents, screenings |
| `cbu.update` | Update a CBU |

### cbu-custody

CBU custody operations: Universe, SSIs, and Booking Rules

| Verb | Description |
|------|-------------|
| `cbu-custody.activate-ssi` | Activate an SSI |
| `cbu-custody.add-agent-override` | Add intermediary agent to SSI settlement chain |
| `cbu-custody.add-booking-rule` | Add ALERT-style booking rule for SSI routing |
| `cbu-custody.add-universe` | Declare what a CBU trades (instrument class + market + currencies) |
| `cbu-custody.create-ssi` | Create a Standing Settlement Instruction (pure account data) |
| `cbu-custody.deactivate-rule` | Deactivate a booking rule |
| `cbu-custody.derive-required-coverage` | Compare universe to booking rules, find gaps |
| `cbu-custody.list-agent-overrides` | List agent overrides for an SSI |
| `cbu-custody.list-booking-rules` | List booking rules for a CBU |
| `cbu-custody.list-ssis` | List SSIs for a CBU |
| `cbu-custody.list-universe` | List CBU's traded universe |
| `cbu-custody.lookup-ssi` | Find SSI for given trade characteristics (simulate ALERT lookup) |
| `cbu-custody.setup-ssi` | Bulk import SSIs from SSI_ONBOARDING document |
| `cbu-custody.suspend-ssi` | Suspend an SSI |
| `cbu-custody.update-rule-priority` | Update booking rule priority |
| `cbu-custody.validate-booking-coverage` | Validate that all universe entries have matching booking rules |

### delivery

Service delivery tracking operations

| Verb | Description |
|------|-------------|
| `delivery.complete` | Mark a service delivery as complete |
| `delivery.fail` | Mark a service delivery as failed |
| `delivery.record` | Record a service delivery for a CBU |

### discrepancy

Observation discrepancies - conflicts between attribute observations

| Verb | Description |
|------|-------------|
| `discrepancy.escalate` | Escalate a discrepancy |
| `discrepancy.list-open` | List open discrepancies |
| `discrepancy.record` | Record a discrepancy between observations |
| `discrepancy.resolve` | Resolve a discrepancy |

### doc-request

Document collection and verification for KYC workstreams

| Verb | Description |
|------|-------------|
| `doc-request.create` | Create a document request |
| `doc-request.list-by-workstream` | List document requests for a workstream |
| `doc-request.mark-requested` | Mark document as formally requested |
| `doc-request.receive` | Record document received |
| `doc-request.reject` | Reject document |
| `doc-request.verify` | Verify document as valid |
| `doc-request.waive` | Waive document requirement |

### document

Document catalog and extraction operations

| Verb | Description |
|------|-------------|
| `document.catalog` | Catalog a document for an entity within a CBU |
| `document.extract` | Extract attributes from a cataloged document |
| `document.extract-to-observations` | Extract document data and create observations |

### entity

Entity management operations

| Verb | Description |
|------|-------------|
| `entity.create-limited-company` | Create a limited company entity |
| `entity.create-partnership-limited` | Create a limited partnership entity |
| `entity.create-proper-person` | Create a natural person entity |
| `entity.create-trust-discretionary` | Create a discretionary trust entity |
| `entity.ensure-limited-company` | Create or update a limited company (idempotent by name) |
| `entity.ensure-partnership-limited` | Create or update a limited partnership (idempotent by name) |
| `entity.ensure-proper-person` | Create or update a natural person (idempotent by name) |
| `entity.ensure-trust-discretionary` | Create or update a discretionary trust (idempotent by name) |
| `entity.delete` | Delete an entity (cascades to type extension) |
| `entity.list` | List entities with optional filters |
| `entity.read` | Read an entity by ID |
| `entity.update` | Update an entity's base fields |

### entity-settlement

Entity settlement identity and SSIs (counterparty data from ALERT)

| Verb | Description |
|------|-------------|
| `entity-settlement.add-ssi` | Add counterparty SSI (from ALERT or manual) |
| `entity-settlement.set-identity` | Set primary settlement identity for an entity |

### entity-workstream

Per-entity workstream within a KYC case

| Verb | Description |
|------|-------------|
| `entity-workstream.block` | Block workstream with reason |
| `entity-workstream.complete` | Mark workstream as complete |
| `entity-workstream.create` | Create a new entity workstream |
| `entity-workstream.list-by-case` | List workstreams for a case |
| `entity-workstream.read` | Read workstream details |
| `entity-workstream.set-enhanced-dd` | Flag workstream for enhanced due diligence |
| `entity-workstream.set-ubo` | Mark workstream entity as UBO |
| `entity-workstream.update-status` | Update workstream status |

### fund

Fund structure operations (umbrella, sub-fund, share class hierarchy)

| Verb | Description |
|------|-------------|
| `fund.ensure-umbrella` | Create or update an umbrella fund (idempotent by name) |
| `fund.ensure-subfund` | Create or update a sub-fund/compartment (idempotent by name) |
| `fund.ensure-share-class` | Create or update a share class (idempotent by ISIN) |

### graph

Graph visualization and traversal operations for CBU entity networks

| Verb | Description |
|------|-------------|
| `graph.view` | Get full graph visualization for a CBU with view mode filtering |
| `graph.focus` | Focus on a specific entity with configurable neighborhood depth |
| `graph.filter` | Filter graph by node types, edge types, layers, or attributes |
| `graph.group-by` | Group nodes by entity type, tier, role, or custom criteria |
| `graph.path` | Find path(s) between two entities in the graph |
| `graph.find-connected` | Find all entities connected to a given entity within depth |
| `graph.ancestors` | Trace ownership/control chain upward to natural persons |
| `graph.descendants` | Trace ownership/control chain downward from an entity |
| `graph.compare` | Compare two graph snapshots to detect structural changes |

### holding

Investor position management in share classes

| Verb | Description |
|------|-------------|
| `holding.close` | Close a holding (mark as inactive) |
| `holding.create` | Create a new investor holding in a share class |
| `holding.ensure` | Ensure investor holding exists (upsert) |
| `holding.list-by-investor` | List holdings for an investor across all share classes |
| `holding.list-by-share-class` | List holdings for a share class |
| `holding.read` | Read a holding by ID |
| `holding.update-units` | Update holding units (for position adjustments) |

### instrument-class

Instrument class with industry taxonomy mappings

| Verb | Description |
|------|-------------|
| `instrument-class.ensure` | Create or update instrument class with CFI/SMPG/ISDA mappings |
| `instrument-class.list` | List instrument classes with filters |
| `instrument-class.read` | Read instrument class by code |

### isda

ISDA and CSA agreement management for OTC derivatives

| Verb | Description |
|------|-------------|
| `isda.add-coverage` | Add instrument class coverage to ISDA |
| `isda.add-csa` | Add CSA (Credit Support Annex) to ISDA |
| `isda.create` | Create ISDA agreement with counterparty |
| `isda.list` | List ISDA agreements for CBU |

### kyc-case

KYC case lifecycle management

| Verb | Description |
|------|-------------|
| `kyc-case.assign` | Assign case to analyst and/or reviewer |
| `kyc-case.close` | Close the case |
| `kyc-case.create` | Create a new KYC case for a CBU |
| `kyc-case.escalate` | Escalate case to higher authority |
| `kyc-case.list-by-cbu` | List cases for a CBU |
| `kyc-case.read` | Read case details |
| `kyc-case.set-risk-rating` | Set case risk rating |
| `kyc-case.update-status` | Update case status |

### market

Market/Exchange reference data

| Verb | Description |
|------|-------------|
| `market.ensure` | Create or update market reference |
| `market.list` | List markets |
| `market.read` | Read market by MIC |

### movement

Fund subscription, redemption, and transfer transactions

| Verb | Description |
|------|-------------|
| `movement.cancel` | Cancel a pending movement |
| `movement.confirm` | Confirm a pending movement |
| `movement.list-by-holding` | List movements for a holding |
| `movement.read` | Read a movement by ID |
| `movement.redeem` | Record a redemption (investor selling units) |
| `movement.settle` | Mark a movement as settled |
| `movement.subscribe` | Record a subscription (investor buying units) |
| `movement.transfer-in` | Record an incoming transfer of units |
| `movement.transfer-out` | Record an outgoing transfer of units |

### observation

Attribute observations from various sources

| Verb | Description |
|------|-------------|
| `observation.get-current` | Get current best observation for an attribute |
| `observation.list-for-attribute` | List observations of a specific attribute for an entity |
| `observation.list-for-entity` | List all observations for an entity |
| `observation.reconcile` | Compare observations for an attribute and auto-create discrepancies |
| `observation.record` | Record an attribute observation |
| `observation.record-from-document` | Record observation extracted from a document |
| `observation.supersede` | Supersede an observation with a newer one |
| `observation.verify-allegations` | Batch verify pending allegations against observations |

### product

Product catalog operations (read-only - products are reference data)

| Verb | Description |
|------|-------------|
| `product.list` | List products with optional filters |
| `product.read` | Read a product by ID or code |

### red-flag

Risk indicators and issues requiring attention

| Verb | Description |
|------|-------------|
| `red-flag.dismiss` | Dismiss red flag as false positive |
| `red-flag.list-by-case` | List red flags for a case |
| `red-flag.list-by-workstream` | List red flags for a workstream |
| `red-flag.mitigate` | Mark red flag as mitigated |
| `red-flag.raise` | Raise a new red flag |
| `red-flag.set-blocking` | Set red flag as blocking the case |
| `red-flag.waive` | Waive red flag with justification |

### rfi

Request for Information - batch document request operations using kyc.doc_requests

| Verb | Description |
|------|-------------|
| `rfi.check-completion` | Check document completion status for a case |
| `rfi.generate` | Generate doc_requests from threshold requirements for a case |
| `rfi.list-by-case` | List all doc_requests for a case |

### screening

Entity screening operations (PEP, sanctions, adverse media)

| Verb | Description |
|------|-------------|
| `screening.adverse-media` | Run adverse media screening |
| `screening.pep` | Run PEP (Politically Exposed Persons) screening |
| `screening.sanctions` | Run sanctions list screening |

### security-type

SMPG/ALERT security type codes

| Verb | Description |
|------|-------------|
| `security-type.ensure` | Create or update ALERT security type |
| `security-type.list` | List security types for an instrument class |

### service

Service catalog operations (read-only - services are reference data)

| Verb | Description |
|------|-------------|
| `service.list` | List services with optional filters |
| `service.list-by-product` | List services for a product |
| `service.read` | Read a service by ID or code |

### service-resource

Service resource type (read-only) and instance operations

| Verb | Description |
|------|-------------|
| `service-resource.activate` | Activate a service resource instance |
| `service-resource.decommission` | Decommission a service resource instance |
| `service-resource.list` | List service resource types with optional filters |
| `service-resource.list-attributes` | List attribute requirements for a service resource type |
| `service-resource.list-by-service` | List service resource types for a service |
| `service-resource.provision` | Provision a service resource instance for a CBU |
| `service-resource.read` | Read a service resource type by ID or code |
| `service-resource.set-attr` | Set an attribute value on a service resource instance |
| `service-resource.suspend` | Suspend a service resource instance |
| `service-resource.validate-attrs` | Validate that all required attributes are set for a resource instance |

### share-class

Fund share class management and investor registry (Clearstream-style)

| Verb | Description |
|------|-------------|
| `share-class.close` | Close a share class to new subscriptions |
| `share-class.create` | Create a new share class for a fund CBU |
| `share-class.ensure` | Create or update share class by ISIN |
| `share-class.list` | List share classes for a fund |
| `share-class.read` | Read a share class by ID |
| `share-class.update-nav` | Update NAV for a share class |

### subcustodian

Bank's sub-custodian network (Omgeo Institution Network)

| Verb | Description |
|------|-------------|
| `subcustodian.ensure` | Create or update sub-custodian entry for market/currency |
| `subcustodian.list-by-market` | List sub-custodian entries for a market |
| `subcustodian.lookup` | Find sub-custodian for market/currency |

### threshold

KYC threshold computation and evaluation

| Verb | Description |
|------|-------------|
| `threshold.check-entity` | Check single entity against requirements |
| `threshold.derive` | Compute KYC requirements based on CBU risk factors |
| `threshold.evaluate` | Check if CBU meets threshold requirements |

### ubo

UBO ownership and control chain management

| Verb | Description |
|------|-------------|
| `ubo.add-ownership` | Add ownership relationship between entities |
| `ubo.calculate` | Calculate ultimate beneficial ownership chain |
| `ubo.check-completeness` | Check if UBO determination is complete for a CBU |
| `ubo.close-ubo` | Close a UBO record (no longer a UBO) |
| `ubo.compare-snapshot` | Compare two UBO snapshots to detect changes |
| `ubo.discover-owner` | Discover potential UBOs from document extraction or registry lookup |
| `ubo.end-ownership` | End an ownership relationship |
| `ubo.infer-chain` | Infer ownership chain from known relationships |
| `ubo.list-by-subject` | List UBOs for a subject entity |
| `ubo.list-owned` | List entities owned by an entity (what does this entity own) |
| `ubo.list-owners` | List owners of an entity (who owns this entity) |
| `ubo.list-snapshots` | List UBO snapshots for a CBU |
| `ubo.list-ubos` | List UBOs for a CBU |
| `ubo.register-ubo` | Register a UBO determination for a CBU |
| `ubo.snapshot-cbu` | Capture a point-in-time snapshot of UBO state for a CBU |
| `ubo.supersede-ubo` | Supersede a UBO record with a newer determination |
| `ubo.trace-chains` | Trace all ownership chains to natural persons for a CBU |
| `ubo.update-ownership` | Update ownership percentage or end date |
| `ubo.verify-ubo` | Mark a UBO as verified |

### verify

Adversarial verification for KYC (game-theoretic "Distrust And Verify" model)

| Verb | Description |
|------|-------------|
| `verify.assert` | Declarative confidence gate (blocks if below threshold) |
| `verify.calculate-confidence` | Aggregate confidence for entity from observations |
| `verify.challenge` | Raise formal verification challenge |
| `verify.detect-evasion` | Analyze doc_request history for evasion signals |
| `verify.detect-patterns` | Run adversarial pattern detection on CBU |
| `verify.escalate` | Route challenge to higher authority |
| `verify.get-status` | Comprehensive verification status report |
| `verify.list-challenges` | List challenges for CBU/case |
| `verify.list-escalations` | List escalations for CBU/case |
| `verify.respond-to-challenge` | Record client response to challenge |
| `verify.resolve-challenge` | Resolve challenge (accept/reject/waive) |
| `verify.resolve-escalation` | Record escalation decision |
| `verify.verify-against-registry` | Check entity against GLEIF/Companies House |

### investment-manager

Investment Manager assignment and trade routing

| Verb | Description |
|------|-------------|
| `investment-manager.assign` | Assign an investment manager to a CBU |
| `investment-manager.set-scope` | Configure scope (instrument classes, markets, currencies) |
| `investment-manager.link-connectivity` | Link IM to connectivity resource (FIX, SWIFT) |
| `investment-manager.list` | List IM assignments for a CBU |
| `investment-manager.suspend` | Suspend an IM assignment |
| `investment-manager.terminate` | Terminate an IM assignment |
| `investment-manager.find-for-trade` | Find IM for given trade characteristics (plugin) |

### pricing-config

Pricing source configuration for instrument valuation

| Verb | Description |
|------|-------------|
| `pricing-config.set` | Configure pricing source for instrument class/market |
| `pricing-config.link-resource` | Link pricing config to data feed resource |
| `pricing-config.list` | List pricing configurations for a CBU |
| `pricing-config.remove` | Remove a pricing configuration |
| `pricing-config.deactivate` | Deactivate a pricing configuration |
| `pricing-config.find-for-instrument` | Find pricing source for instrument (plugin) |

### sla

SLA commitment, measurement, and breach management

| Verb | Description |
|------|-------------|
| `sla.commit` | Create SLA commitment from template |
| `sla.bind-to-product` | Bind SLA to a product |
| `sla.bind-to-service` | Bind SLA to a service |
| `sla.bind-to-resource` | Bind SLA to a resource instance |
| `sla.override-target` | Override template target for specific commitment |
| `sla.list-commitments` | List SLA commitments for a CBU |
| `sla.record-measurement` | Record an SLA measurement |
| `sla.list-measurements` | List measurements for a commitment |
| `sla.report-breach` | Report an SLA breach |
| `sla.update-remediation` | Update breach remediation status |
| `sla.escalate-breach` | Escalate a breach |
| `sla.close-breach` | Close a resolved breach |
| `sla.list-open-breaches` | List open breaches for a CBU (plugin) |
| `sla.list-templates` | List available SLA templates |
| `sla.list-metrics` | List available SLA metrics |
| `sla.get-template` | Get SLA template details |
| `sla.get-commitment` | Get commitment details with measurements |

### cash-sweep

Cash sweep and STIF configuration

| Verb | Description |
|------|-------------|
| `cash-sweep.configure` | Configure cash sweep for a CBU |
| `cash-sweep.link-resource` | Link sweep config to cash management resource |
| `cash-sweep.list` | List sweep configurations for a CBU |
| `cash-sweep.update-threshold` | Update sweep threshold |
| `cash-sweep.update-timing` | Update sweep timing |
| `cash-sweep.change-vehicle` | Change sweep vehicle (STIF, MMF) |
| `cash-sweep.suspend` | Suspend sweep configuration |
| `cash-sweep.reactivate` | Reactivate sweep configuration |
| `cash-sweep.remove` | Remove sweep configuration |

### Reference Data Domains

The following domains manage reference/master data used throughout the system.

#### case-type

KYC case type reference data (NEW_CLIENT, PERIODIC_REVIEW, etc.)

| Verb | Description |
|------|-------------|
| `case-type.ensure` | Create or update a case type |
| `case-type.read` | Read a case type by code |
| `case-type.list` | List all case types |
| `case-type.deactivate` | Deactivate a case type |

#### client-type

Client type reference data (FUND, CORPORATE, etc.)

| Verb | Description |
|------|-------------|
| `client-type.ensure` | Create or update a client type |
| `client-type.read` | Read a client type by code |
| `client-type.list` | List all client types |
| `client-type.deactivate` | Deactivate a client type |

#### currency

Currency reference data (USD, EUR, GBP, etc.)

| Verb | Description |
|------|-------------|
| `currency.ensure` | Create or update a currency |
| `currency.read` | Read a currency by ISO code |
| `currency.list` | List all currencies |
| `currency.deactivate` | Deactivate a currency |

#### jurisdiction

Jurisdiction reference data (US, GB, LU, etc.)

| Verb | Description |
|------|-------------|
| `jurisdiction.ensure` | Create or update a jurisdiction |
| `jurisdiction.read` | Read a jurisdiction by code |
| `jurisdiction.list` | List all jurisdictions |
| `jurisdiction.delete` | Delete a jurisdiction |

#### risk-rating

Risk rating reference data (LOW, MEDIUM, HIGH, etc.)

| Verb | Description |
|------|-------------|
| `risk-rating.ensure` | Create or update a risk rating |
| `risk-rating.read` | Read a risk rating by code |
| `risk-rating.list` | List all risk ratings |
| `risk-rating.deactivate` | Deactivate a risk rating |

#### role

Entity role reference data (DIRECTOR, UBO, SHAREHOLDER, etc.)

| Verb | Description |
|------|-------------|
| `role.ensure` | Create or update a role |
| `role.read` | Read a role by name |
| `role.list` | List all roles |
| `role.delete` | Delete a role |

#### screening-type

Screening type reference data (PEP, SANCTIONS, ADVERSE_MEDIA, etc.)

| Verb | Description |
|------|-------------|
| `screening-type.ensure` | Create or update a screening type |
| `screening-type.read` | Read a screening type by code |
| `screening-type.list` | List all screening types |
| `screening-type.deactivate` | Deactivate a screening type |

#### settlement-type

Settlement type reference data (DVP, FOP, RVP, etc.)

| Verb | Description |
|------|-------------|
| `settlement-type.ensure` | Create or update a settlement type |
| `settlement-type.read` | Read a settlement type by code |
| `settlement-type.list` | List all settlement types |
| `settlement-type.deactivate` | Deactivate a settlement type |

#### ssi-type

SSI type reference data (SECURITIES, CASH, COLLATERAL)

| Verb | Description |
|------|-------------|
| `ssi-type.ensure` | Create or update an SSI type |
| `ssi-type.read` | Read an SSI type by code |
| `ssi-type.list` | List all SSI types |
| `ssi-type.deactivate` | Deactivate an SSI type |


## Adding New Verbs

To add a new verb, edit the appropriate file in `rust/config/verbs/`:

```yaml
domains:
  my_domain:
    verbs:
      my-verb:
        description: "What this verb does"
        behavior: crud
        crud:
          operation: insert  # insert, update, delete, upsert, select, entity_create, entity_upsert, etc.
          table: my_table
          schema: ob-poc
          returning: my_id
        args:
          - name: my-arg
            type: string
            required: true
            maps_to: my_column  # DB column name
        returns:
          type: uuid
          capture: true
```

No Rust code changes required for standard CRUD operations.

## Unified Entity Dependency DAG

The system uses a **unified entity dependency DAG** to manage dependencies between entity types during onboarding and execution planning. This replaces the older resource-specific dependency graph with a single, config-driven model.

### Database Table: `entity_type_dependencies`

```sql
CREATE TABLE "ob-poc".entity_type_dependencies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_type VARCHAR(100) NOT NULL,      -- e.g., "resource_instance", "kyc_case"
    source_subtype VARCHAR(100),            -- e.g., "CUSTODY_ACCT", "NEW_CLIENT"
    target_type VARCHAR(100) NOT NULL,      -- e.g., "cbu", "entity"
    target_subtype VARCHAR(100),            -- optional subtype constraint
    dependency_kind VARCHAR(20) NOT NULL,   -- REQUIRED, OPTIONAL, LIFECYCLE
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT now()
);
```

**Key rows:**
| source_type | source_subtype | target_type | kind |
|-------------|----------------|-------------|------|
| resource_instance | CUSTODY_ACCT | cbu | REQUIRED |
| resource_instance | SWIFT_CONN | resource_instance:CUSTODY_ACCT | REQUIRED |
| kyc_case | * | cbu | REQUIRED |
| fund | * | cbu | REQUIRED |

### Core Types (`rust/src/dsl_v2/entity_deps.rs`)

```rust
/// Key for matching entity types with optional subtype
pub struct EntityTypeKey {
    pub entity_type: String,
    pub subtype: Option<String>,
}

/// A dependency edge in the graph
pub struct EntityDep {
    pub source: EntityTypeKey,
    pub target: EntityTypeKey,
    pub kind: DependencyKind,
}

/// Centralized registry loaded from DB
pub struct EntityDependencyRegistry {
    deps_by_source: HashMap<EntityTypeKey, Vec<EntityDep>>,
    deps_by_target: HashMap<EntityTypeKey, Vec<EntityDep>>,
    known_types: HashSet<EntityTypeKey>,
}

impl EntityDependencyRegistry {
    /// Load from database
    pub async fn load(pool: &PgPool) -> Result<Self, sqlx::Error>;
    
    /// Get dependencies of a type (what does X depend on?)
    pub fn dependencies_of(&self, entity_type: &str, subtype: Option<&str>) -> Vec<&EntityDep>;
    
    /// Get dependents of a type (what depends on X?)
    pub fn dependents_of(&self, entity_type: &str, subtype: Option<&str>) -> Vec<&EntityDep>;
}

/// Entity instance for topological sorting
pub struct EntityInstance {
    pub id: String,
    pub entity_type: String,
    pub subtype: Option<String>,
    pub depends_on: Vec<String>,  // IDs of dependencies
}

/// Topological sort with parallel stage detection
pub fn topological_sort_unified(
    instances: &[EntityInstance]
) -> Result<TopoSortUnifiedResult, TopoSortUnifiedError>;
```

### Topological Sort Result

```rust
pub struct TopoSortUnifiedResult {
    pub stages: Vec<Vec<String>>,  // Parallel execution stages
    pub order: Vec<String>,        // Linear order (stages flattened)
}
```

Instances in the same stage have no inter-dependencies and can be processed in parallel.

### Usage in Onboarding

The onboarding custom_ops use the unified DAG for resource provisioning order:

```rust
// rust/src/dsl_v2/custom_ops/onboarding.rs
let registry = EntityDependencyRegistry::load(pool).await?;

// Build EntityInstance list from resources to provision
let instances: Vec<EntityInstance> = resources.iter().map(|r| {
    let deps = registry.dependencies_of("resource_instance", Some(&r.resource_code));
    EntityInstance {
        id: r.instance_id.to_string(),
        entity_type: "resource_instance".to_string(),
        subtype: Some(r.resource_code.clone()),
        depends_on: /* resolve dep IDs */,
    }
}).collect();

// Get execution order
let result = topological_sort_unified(&instances)?;
for stage in result.stages {
    // Process stage in parallel
}
```

### Migration from ResourceDependencyGraph

The legacy `ResourceDependencyGraph` in `onboarding.rs` is deprecated. New code should use:

| Old API | New API |
|---------|---------|
| `ResourceDependencyGraph::new()` | `EntityDependencyRegistry::load(pool).await` |
| `graph.add_dependency()` | Use DB table `entity_type_dependencies` |
| `graph.topological_sort()` | `topological_sort_unified(&instances)` |

## Database Development Practices

### ⛔ MANDATORY: SQLx Compile-Time Verification

When making ANY database schema changes, you MUST reconcile the database schema with Rust types.

**Why this matters:**

SQLx performs compile-time verification against the actual PostgreSQL schema. This catches type mismatches, missing columns, and schema drift that would:
- Compile fine in Java/Hibernate
- Pass mocked unit tests
- Fail at RUNTIME in production
- Or worse: silently corrupt data

**With SQLx + Rust, these errors are caught at COMPILE TIME - before any code runs.**

### Verification Workflow

After ANY schema change (new table, altered column, new index):

```bash
# 1. Apply your migration
psql -d data_designer -f your_migration.sql

# 2. Regenerate SQLx offline data
cd rust
cargo sqlx prepare --workspace

# 3. Build and verify - this will catch mismatches
cargo build

# 4. Fix any type mismatches between:
#    - PostgreSQL schema (source of truth)
#    - SQLx query macros (query_as!, query!)
#    - Rust struct definitions
```

### Common Mismatches to Watch For

| PostgreSQL Type | Rust Type | Notes |
|-----------------|-----------|-------|
| `UUID` | `Uuid` (from uuid crate) | Not `String` |
| `TIMESTAMPTZ` | `DateTime<Utc>` (from chrono) | Not `NaiveDateTime` |
| `VARCHAR(n)` / `TEXT` | `String` | |
| `INTEGER` | `i32` | Not `i64` |
| `BIGINT` | `i64` | Not `i32` |
| `NUMERIC` / `DECIMAL` | `BigDecimal` or `rust_decimal::Decimal` | Not `f64` for money |
| `BOOLEAN` | `bool` | |
| `JSONB` | `serde_json::Value` or typed struct | |
| `column NULLABLE` | `Option<T>` | Missing `Option` = runtime panic |
| `column NOT NULL` | `T` (not Option) | |

### Evidence: This Works

During development, SQLx compile-time checks discovered multiple type mismatches between the database schema and Rust code. Every error found would have:

- ✅ Compiled in Java/Hibernate  
- ✅ Passed mocked unit tests
- ❌ Failed at runtime in production

**This is not theoretical. This is concrete evidence that compile-time schema verification catches real bugs.**

### Schema Change Checklist

- [ ] Migration SQL written and reviewed
- [ ] Migration applied to local database
- [ ] `cargo sqlx prepare --workspace` run
- [ ] `cargo build` passes (no type mismatches)
- [ ] Relevant Rust structs updated if needed
- [ ] Tests pass with real database (not mocks)

### Why Not Hibernate/ORM?

Traditional ORMs like Hibernate:
- Validate schema at runtime (when first query runs)
- Allow string column names that don't exist (`@Column(name = "ammount")` - typo compiles fine)
- Type coercion can hide mismatches until production
- Mocked tests bypass all schema validation

SQLx:
- Validates at compile time against real database
- Typos in column names = compile error
- Type mismatches = compile error
- Cannot deploy code with schema drift

**The "complexity" of Rust/SQLx pays for itself in correctness.**

## Agent Workflow (Conductor Mode)

This repository uses a **conductor pattern** for agent interactions. The full contract is in `CONDUCTOR_MODE.md`. Key principles:

### Operating Principles

1. **Scope is explicit** - Only modify files mentioned or obviously related. ASK before touching others.

2. **Plan → Confirm → Edit** - Before editing:
   - Summarize what you've read in 3-7 bullets
   - Propose a short numbered plan (3-6 steps)
   - WAIT for explicit approval before changing code

3. **Small, reviewable diffs** - Prefer many small coherent changes over one giant diff.

### Editing Rules

1. **Preserve invariants** - Do not change public types, DSL grammars, or DB schemas unless explicitly asked. State invariants before touching them.

2. **Be explicit about uncertainty** - If unsure how something works, say so. Prefer tests/assertions/questions over silent guessing.

3. **No surprise deletions** - List call sites, classify (runtime vs test-only), explain why safe. Propose and await confirmation.

4. **Tests first** - For behavior changes, adjust or add tests first.

### High-Risk Areas (Two-Pass Required)

For these areas, always do a **read-only analysis pass** before proposing edits:

- DSL → AST → execution → DB transitions
- Call graph / dead code analysis  
- UBO graph logic / ownership prongs
- Anything coupling Rust + Go + SQL + JSON Schema

**Pass 1 (read-only):** Read files, explain the pipeline, state invariants, identify what would break.

**Pass 2 (tightly scoped edit):** Given that understanding, only change the specific seam.

### When in Doubt

If uncertain about DSL semantics, CBU/UBO/KYC domain rules, graph invariants, or cross-crate boundaries:

1. Stop
2. Explain the uncertainty
3. Ask for clarification or propose options
4. Wait for guidance

Never silently "guess and commit" on complex domain logic.


---

## Current Priority: Phase 3 Agent Intelligence Baseline Completion

**Status:** In Progress  
**TODO File:** `TODO-PHASE3-BASELINE-COMPLETION.md`

### What's Done
- Intent classifier with pattern matching
- Entity extractor with region/hierarchy expansion
- DSL generator with parameter mappings
- Pipeline orchestration with session management
- Configuration files (taxonomy, entity types, mappings)
- Evaluation dataset (40+ test cases)

### Critical Gaps to Fix

1. **Wire Execution** (Phase 3.1)
   - `pipeline.rs` execution is stubbed
   - Need to connect to actual `DslExecutor`
   - Add async support and transactions

2. **Complete Parameter Mappings** (Phase 3.2)
   - ~15 verbs in taxonomy have no mappings
   - Add to `config/agent/parameter_mappings.yaml`
   - Add missing entity types

3. **LLM Fallback** (Phase 3.3)
   - Currently 100% pattern matching
   - Need semantic fallback when patterns fail

4. **Evaluation Harness** (Phase 3.4)
   - Dataset exists but no runner
   - Build CLI to measure accuracy

### Key Files

```
rust/src/agentic/
├── pipeline.rs           # Main orchestration (needs execution wiring)
├── intent_classifier.rs  # Pattern-based classification
├── entity_extractor.rs   # Entity extraction with expansion
├── dsl_generator.rs      # Intent+entities → DSL
├── taxonomy.rs           # Loads intent_taxonomy.yaml
├── market_regions.rs     # Region expansion config
├── instrument_hierarchy.rs # Instrument category expansion

rust/config/agent/
├── intent_taxonomy.yaml      # All intents and trigger phrases
├── entity_types.yaml         # Entity patterns and normalization
├── parameter_mappings.yaml   # Intent → DSL parameter mapping (INCOMPLETE)
├── market_regions.yaml       # European → [XLON, XETR, ...]
├── instrument_hierarchy.yaml # Fixed Income → [GOVT_BOND, CORP_BOND]
├── evaluation_dataset.yaml   # Golden test cases
```

### Success Criteria
- All 25+ intents can generate valid DSL
- Generated DSL executes against database
- Evaluation passes at 85%+ accuracy
- Demo: "Add BlackRock for European equities via CTM" → DSL → Execution → Confirmation

### Execution Order
1. Week 1: Wire execution + complete parameter mappings
2. Week 2: Evaluation harness + gap fixing
3. Week 3: LLM fallback + query implementation
4. Week 4: Polish and documentation

