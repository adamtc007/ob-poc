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
│   │   ├── visualization/          # Server-side visualization builders
│   │   │   ├── kyc_builder.rs      # KYC/UBO tree builder
│   │   │   └── service_builder.rs  # Service delivery tree builder
│   │   ├── graph/                  # Graph visualization
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
├── sql/
│   ├── seeds/                      # Seed data SQL files
│   └── tests/                      # SQL test fixtures
├── docs/
│   ├── DATABASE_SCHEMA.md          # Complete schema reference
│   └── DSL_TEST_SCENARIOS.md       # Test scenario documentation
├── schema_export.sql               # Full DDL for database rebuild
└── CLAUDE.md                       # This file
```

## Visualization Architecture

The UI follows a **server-side rendering / dumb UI** pattern. All data assembly happens on the server; the UI receives JSON and renders it.

```
┌─────────────────────────────────────────────────────────────────┐
│                         UI (Dumb Client)                         │
│  Receives JSON, renders trees/graphs, no business logic         │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Visualization Builders                        │
│  KycTreeBuilder, ServiceTreeBuilder, CbuGraphBuilder            │
│  Assemble view models from repository data                      │
│  rust/src/visualization/, rust/src/graph/                       │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                  VisualizationRepository                         │
│  SINGLE point of DB access for all visualization queries        │
│  Enables Oracle migration - SQL isolated to one file            │
│  rust/src/database/visualization_repository.rs                  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      PostgreSQL / Oracle                         │
└─────────────────────────────────────────────────────────────────┘
```

### Key Design Principles

1. **Centralized DB Access**: All visualization queries go through `VisualizationRepository`. No direct `sqlx::query!` calls in builders or API routes.

2. **View Models**: Repository returns typed view structs (e.g., `CbuView`, `EntityView`, `GraphEntityView`). Builders transform these into UI-ready structures.

3. **Database Portability**: Isolating SQL to one file enables future Oracle migration without touching visualization logic.

### Query Categories in VisualizationRepository

| Category | Methods | Used By |
|----------|---------|--------|
| CBU | `list_cbus`, `get_cbu`, `get_cbu_for_tree` | Tree builders, dropdowns |
| Entity | `get_entity`, `get_entities_by_role`, `get_officers` | KYC tree |
| Graph Core | `get_graph_entities` | Graph builder |
| Graph Custody | `get_universes`, `get_ssis`, `get_booking_rules`, `get_isdas`, `get_csas` | Graph builder |
| Graph KYC | `get_kyc_statuses`, `get_document_requests`, `get_graph_screenings` | Graph builder |
| Graph UBO | `get_ubos`, `get_ownerships`, `get_graph_controls` | Graph builder |
| Graph Services | `get_resource_instances` | Graph builder |
| MCP | `get_cbu_basic`, `get_cbu_entities`, `get_entity_types`, etc. | MCP handlers |


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

## Verb Domains

| Domain | Purpose |
|--------|---------|
| cbu | Client Business Unit lifecycle (ensure, assign-role, etc.) |
| entity | Dynamic verbs from entity_types (create-proper-person, create-limited-company) |
| document | Document catalog, request, extract |
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

## Database

**Database**: `data_designer` on PostgreSQL 17

Four schemas:
- **ob-poc**: KYC/AML domain (105 tables)
- **custody**: Settlement & custody (18 tables)
- **kyc**: Investor registry (3 tables: share_classes, holdings, movements)
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
