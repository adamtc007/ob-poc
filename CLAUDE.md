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

```
┌─────────────────────────────────────────────────────────────────┐
│                     DSL Source Text                              │
│  (cbu.ensure :name "Fund" :jurisdiction "LU" :as @fund)         │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                   Parser (Nom) → AST                             │
│  rust/src/dsl_v2/parser.rs                                      │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                CSG Linter (Validation)                           │
│  - Verb existence, argument validation                          │
│  - Entity type constraints (passport→person, cert→company)      │
│  - Symbol resolution (@ref must be defined before use)          │
│  rust/src/dsl_v2/csg_linter.rs                                  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              Compiler → Execution Plan                           │
│  rust/src/dsl_v2/execution_plan.rs                              │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│        GenericCrudExecutor (YAML-driven)                         │
│  - Reads verb config from config/verbs.yaml                     │
│  - All 13 CRUD operations driven by YAML config                 │
│  - Custom ops via plugin pattern                                │
│  rust/src/dsl_v2/generic_executor.rs                            │
└─────────────────────────────────────────────────────────────────┘
```

### YAML-Driven Configuration

The DSL system is entirely YAML-driven. Adding new verbs requires editing YAML, not Rust code.

```
config/
├── verbs.yaml      # All verb definitions (1,500+ lines)
│                   # - Domain definitions
│                   # - Verb args with maps_to for DB columns
│                   # - CRUD operations (insert, update, delete, etc.)
│                   # - Plugin behaviors for custom ops
└── csg_rules.yaml  # Context-sensitive grammar rules
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
│   │   │   ├── generic_executor.rs # GenericCrudExecutor (13 CRUD ops)
│   │   │   ├── executor.rs         # DslExecutor (orchestrates execution)
│   │   │   ├── csg_linter.rs       # Context-sensitive validation
│   │   │   ├── execution_plan.rs   # AST → ExecutionPlan compiler
│   │   │   └── custom_ops/         # Plugin handlers for non-CRUD ops
│   │   ├── database/               # Repository pattern services
│   │   │   └── visualization_repository.rs  # Centralized visualization queries
│   │   ├── graph/                  # Graph visualization (single pipeline)
│   │   ├── graph/                  # Graph visualization (single pipeline)
│   │   │   ├── builder.rs          # CbuGraphBuilder (multi-layer graph)
│   │   │   └── types.rs            # GraphNode, GraphEdge, CbuGraph
│   │   ├── domains/                # Domain-specific logic
│   │   ├── mcp/                    # MCP server for Claude Desktop
│   │   ├── planner/                # DSL builder utilities
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

## Visualization Architecture

The UI follows a **single pipeline** pattern. Server returns flat graph data; UI owns filtering and layout.

```
┌─────────────────────────────────────────────────────────────────┐
│                    WASM UI (egui)                                │
│  CbuGraphWidget: filters by ViewMode, owns layout logic         │
│  rust/crates/ob-poc-ui/src/graph/                               │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              /api/cbu/:id/graph endpoint                         │
│  Returns COMPLETE graph (all layers)                            │
│  rust/src/api/graph_routes.rs                                   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    CbuGraphBuilder                               │
│  Loads all layers: Core, Custody, KYC, UBO, Services            │
│  rust/src/graph/builder.rs                                      │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                  VisualizationRepository                         │
│  SINGLE point of DB access for all visualization queries        │
│  rust/src/database/visualization_repository.rs                  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      PostgreSQL / Oracle                         │
└─────────────────────────────────────────────────────────────────┘
```

### View Modes (UI-side filtering)

| Mode | Layers Shown | Description |
|------|--------------|-------------|
| KYC/UBO | core, kyc, ubo | Entities, KYC status, ownership chains |
| Service Delivery | core, services | Entities + Products → Services → Resources |

### Key Design Principles

1. **Single Pipeline**: One endpoint (`/api/cbu/:id/graph`), one builder, UI filters by view mode.

2. **UI Owns Layout**: Server returns flat graph (nodes + edges). UI's `LayoutEngine` positions nodes based on roles and node types.

3. **Centralized DB Access**: All queries go through `VisualizationRepository`.

### Graph Layers

| Layer | Node Types | Description |
|-------|------------|-------------|
| core | cbu, entity | CBU and business entities with roles |
| kyc | verification, document | KYC status, document requests |
| ubo | entity (UBO-specific) | Ownership chains, control relationships |
| services | product, service, resource | Products → Services → Resource instances |

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
- `rust/crates/ob-poc-ui/src/app.rs` - Fetch/save logic with debounce
- `rust/crates/ob-poc-ui/src/graph/types.rs` - LayoutOverride, NodeOffset, NodeSizeOverride
- `rust/crates/ob-poc-ui/src/graph/input.rs` - Drag/resize handling


```bash
cd rust/

# Build
cargo build --features server --bin agentic_server        # Main server
cargo build --features cli,database --bin dsl_cli         # CLI tool
cargo build --features database                            # DSL library only
cargo build --features mcp --bin dsl_mcp                  # MCP server

# Run server (requires DATABASE_URL and ANTHROPIC_API_KEY)
DATABASE_URL="postgresql:///data_designer" \
ANTHROPIC_API_KEY="sk-..." \
./target/debug/agentic_server
# Open http://localhost:3000

# Test
cargo test --features database --lib                  # Unit tests (~143)
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
| `POST /api/session/:id/chat` | Send chat message |
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

## KYC & UBO DSL

The KYC case management and UBO domains manage entity-level investigations, screenings, ownership chains, and UBO determinations.

> **Note**: Screenings are now managed via the KYC Case model. Use `kyc-case.create` → `entity-workstream.create` → `case-screening.run` instead of the legacy `screening.*` verbs.

### UBO Verbs

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
| `ubo.trace-chains` | Trace all ownership chains to natural persons |
| `ubo.infer-chain` | Trace ownership chain upward from starting entity |
| `ubo.check-completeness` | Validate UBO determination completeness |
| `ubo.supersede-ubo` | Supersede UBO record with newer determination |
| `ubo.snapshot-cbu` | Capture point-in-time UBO state snapshot |
| `ubo.compare-snapshot` | Compare two UBO snapshots for changes |

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
**Schemas**: `ob-poc` (55 tables), `custody` (17 tables), `kyc` (11 tables)  
**Updated**: 2025-12-07

## Overview

This document describes the database schema used by the OB-POC KYC/AML onboarding system. The schema supports:

- **Core KYC/AML**: CBUs, entities, documents, screening, KYC investigations
- **Service Delivery**: Products, services, resource instances
- **Custody & Settlement**: Three-layer model (Universe → SSI → Booking Rules)
- **Investor Registry**: Fund share classes, holdings, and movements (Clearstream-style)
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

### control_relationships

Non-ownership control links between entities.

| Column | Type | Nullable | Default | Description |
|--------|------|----------|---------|-------------|
| control_id | uuid | NOT NULL | gen_random_uuid() | Primary key |
| controller_entity_id | uuid | NOT NULL | | FK to entities (who controls) |
| controlled_entity_id | uuid | NOT NULL | | FK to entities (who is controlled) |
| control_type | varchar(50) | NOT NULL | | BOARD_CONTROL, VOTING_RIGHTS, VETO_POWER, MANAGEMENT, TRUSTEE, PROTECTOR, OTHER |
| description | text | | | Description of control mechanism |
| effective_from | date | | | Start date |
| effective_to | date | | | End date |
| is_active | boolean | | true | Active record |
| evidence_doc_id | uuid | | | FK to document_catalog |
| created_at | timestamptz | | now() | |
| updated_at | timestamptz | | now() | |


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
| Core | 5 | cbus, entities, entity_types, roles, cbu_entity_roles |
| Entity Extensions | 4 | entity_proper_persons, entity_limited_companies, entity_partnerships, entity_trusts |
| Documents | 3 | document_catalog, document_types, document_attribute_mappings |
| Products/Services | 8 | products, services, service_delivery_map, cbu_resource_instances |
| Reference Data | 4 | master_jurisdictions, currencies, roles, dictionary |
| DSL/Execution | 6 | dsl_instances, dsl_instance_versions, dsl_execution_log, dsl_domains, dsl_examples |
| Onboarding | 4 | onboarding_requests, onboarding_products, service_option_definitions, service_option_choices |
| Attributes | 4 | attribute_registry, attribute_values_typed, attribute_dictionary, resource_attribute_requirements |
| Other | 13 | Various support tables |
| **ob-poc Total** | **51** | |
| **Custody** | **17** | cbu_instrument_universe, cbu_ssi, ssi_booking_rules, isda_agreements, csa_agreements |
| **KYC** | **11** | cases, entity_workstreams, red_flags, doc_requests, screenings, share_classes, holdings, movements |
| **Grand Total** | **79** | |

## Rebuilding the Schema

```bash
# Full schema rebuild
psql -d data_designer -f schema_export.sql

```

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
| `verbs_list` | List available DSL verbs (optionally by domain) |
| `schema_info` | Get entity types, roles, document types |

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

## Intent-to-DSL Assembly Pipeline

The `rust/src/dsl_v2/` module provides a **deterministic** DSL generation pipeline that minimizes AI variance. Unlike the agentic module which has Claude generate DSL text directly, this pipeline has Claude extract **structured intent** which is then assembled into valid DSL by Rust code.

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
│  ArgIntent lookups → ResolvedArg with real UUIDs/codes          │
│  - EntityLookup: "Apex Capital" → UUID                          │
│  - RefDataLookup: "director" → "DIRECTOR"                       │
│  rust/src/dsl_v2/entity_resolver.rs                             │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│         PHASE 3: DSL ASSEMBLY (Deterministic Rust)               │
│  Resolved intents → Valid DSL source code                       │
│  - Verb registry validates args                                 │
│  - Symbol tracking across batch                                 │
│  - Proper quoting (all strings quoted in DSL)                   │
│  rust/src/dsl_v2/assembler.rs                                   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│         PHASE 4: VALIDATION                                      │
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
├── assembler.rs        # DslAssembler, ArgResolver trait
├── entity_resolver.rs  # EntityGatewayResolver, needs_quoting()
└── prompts/
    └── general_intent_extraction.md  # Claude extraction prompt
```

### Usage Example

```rust
// 1. Extract intent from natural language
let extractor = IntentExtractor::from_env()?;
let batch = extractor.extract("Add John Smith as director of Apex Capital").await?;

// 2. Create resolver (connects to EntityGateway)
let resolver = EntityGatewayResolver::from_env().await?;

// 3. Assemble DSL
let mut assembler = DslAssembler::new();
let dsl = assembler.assemble_batch(&batch, &resolver)?;

// 4. Validate
let ast = parse_program(&dsl)?;
```

### Why This Design?

| Aspect | Agentic (text gen) | Intent Pipeline |
|--------|-------------------|-----------------|
| AI output | DSL source code | Structured JSON |
| Entity IDs | Can hallucinate | Resolved via EntityGateway |
| Validation | Post-hoc (retry loop) | Built into assembly |
| Determinism | Low (text varies) | High (templates) |
| Debugging | Parse error messages | Typed assembly errors |

**Key insight**: AI is good at understanding intent, but prone to syntax errors and hallucinating IDs. By having AI produce structured data and Rust produce DSL, we get the best of both.

## Environment Variables
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
| `cbu.delete` | Delete a CBU |
| `cbu.ensure` | Create or update a CBU by natural key |
| `cbu.list` | List CBUs with optional filters |
| `cbu.parties` | List all parties (entities with their roles) for a CBU |
| `cbu.read` | Read a CBU by ID |
| `cbu.remove-role` | Remove a specific role from an entity within a CBU |
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


```bash
DATABASE_URL="postgresql:///data_designer"
ANTHROPIC_API_KEY="sk-ant-..."
DSL_CONFIG_DIR="/path/to/config"  # Optional: override config location
```

## Adding New Verbs

To add a new verb, edit `rust/config/verbs.yaml`:

```yaml
domains:
  my_domain:
    verbs:
      my-verb:
        description: "What this verb does"
        behavior: crud
        crud:
          operation: insert  # insert, update, delete, upsert, select, etc.
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
