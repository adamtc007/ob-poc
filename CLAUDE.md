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
├── sql/
│   ├── seeds/                      # Seed data SQL files
│   └── tests/                      # SQL test fixtures
├── docs/
│   ├── DATABASE_SCHEMA.md          # Complete schema reference
│   └── DSL_TEST_SCENARIOS.md       # Test scenario documentation
├── schema_export.sql               # Full DDL for database rebuild
└── CLAUDE.md                       # This file
```

## Commands

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
cargo test --features database --lib                  # Unit tests (~118)
cargo test --features database --test db_integration  # DB tests
./tests/scenarios/run_tests.sh                        # DSL scenarios
./tests/mcp_test.sh                                   # MCP protocol tests

# Clippy (all features)
cargo clippy --features server
cargo clippy --features database
cargo clippy --features mcp
```

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
| `POST /api/agent/validate` | Validate DSL syntax/semantics |
| `POST /api/session` | Create new session |
| `POST /api/session/:id/chat` | Send chat message |
| `POST /api/session/:id/execute` | Execute DSL |
| `GET /api/templates` | List templates |
| `GET /api/dsl/list` | List DSL instances |

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

;; Screening
(screening.pep :entity-id @john)
(screening.sanctions :entity-id @company)

;; Service resource instance lifecycle
(service-resource.provision :cbu-id @fund :resource-type "CUSTODY_ACCOUNT" :instance-url "https://..." :as @account)
(service-resource.set-attr :instance-id @account :attr "account_number" :value "ACC-12345")
(service-resource.activate :instance-id @account)
```

## Verb Domains

| Domain | Purpose |
|--------|---------|
| cbu | Client Business Unit lifecycle (ensure, assign-role, etc.) |
| entity | Dynamic verbs from entity_types (create-proper-person, create-limited-company) |
| document | Document catalog, request, extract |
| screening | PEP, sanctions, adverse-media checks |
| kyc | Investigation initiate, decide |
| ubo | Calculate, validate ownership |
| service-resource | Service resource type CRUD + instance provision, set-attr, activate, suspend, decommission |
| delivery | Service delivery record, complete, fail |
| cbu-custody | Custody & settlement: universe, SSI, booking rules |

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
| `cbu-custody.add-booking-rule` | CRUD | Add ALERT-style routing rule |
| `cbu-custody.list-booking-rules` | CRUD | List CBU's booking rules |
| `cbu-custody.update-rule-priority` | CRUD | Change rule priority |
| `cbu-custody.deactivate-rule` | CRUD | Deactivate a booking rule |
| `cbu-custody.validate-booking-coverage` | Plugin | Validate rules cover universe |
| `cbu-custody.derive-required-coverage` | Plugin | Calculate required coverage |
| `cbu-custody.lookup-ssi` | Plugin | Find SSI for trade characteristics |

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

## Database

**Database**: `data_designer` on PostgreSQL 17

Three schemas:
- **ob-poc**: KYC/AML domain (103 tables)
- **custody**: Settlement & custody (18 tables)
- **public**: Runtime API endpoints

See `docs/DATABASE_SCHEMA.md` for complete schema. Rebuild with:
```bash
psql -d data_designer -f schema_export.sql
```

## MCP Server Tools

For Claude Desktop integration:

| Tool | Description |
|------|-------------|
| `dsl_validate` | Parse and validate DSL |
| `dsl_execute` | Execute DSL against database |
| `cbu_get` | Get CBU with entities, roles, documents |
| `cbu_list` | List/search CBUs |
| `verbs_list` | List available DSL verbs |
| `schema_info` | Get entity types, roles, document types |

## Environment Variables

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
