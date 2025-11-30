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
│                    PostgreSQL (ob-poc)                          │
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
│                   DslExecutor                                    │
│  - Type-aware binding (String→Date, UUID, Decimal)              │
│  - Symbol table management                                       │
│  - CRUD generation and execution                                │
│  rust/src/dsl_v2/executor.rs                                    │
└─────────────────────────────────────────────────────────────────┘
```

### Key Components

| Component | Location | Purpose |
|-----------|----------|---------|
| Web UI | `src/ui/` | Server-rendered HTML pages |
| Agentic Server | `src/bin/agentic_server.rs` | Main HTTP server |
| Agent Routes | `src/api/agent_routes.rs` | Claude API integration |
| Parser | `src/dsl_v2/parser.rs` | Nom-based S-expression parser |
| AST | `src/dsl_v2/ast.rs` | Program, Statement, VerbCall, Value types |
| Verb Registry | `src/dsl_v2/verb_registry.rs` | 53+ verbs across 8 domains |
| CSG Linter | `src/dsl_v2/csg_linter.rs` | Context-sensitive validation |
| Compiler | `src/dsl_v2/execution_plan.rs` | AST → ExecutionPlan |
| Executor | `src/dsl_v2/executor.rs` | Plan execution with DB |
| Templates | `src/templates/` | Pre-built DSL templates |
| MCP Server | `src/mcp/` | Claude Desktop integration |
| LSP Server | `crates/dsl-lsp/` | IDE integration |

## Directory Structure

```
rust/
├── src/
│   ├── ui/                     # Server-rendered UI
│   │   ├── mod.rs              # Module exports
│   │   ├── pages.rs            # HTML generation
│   │   └── routes.rs           # Axum routes
│   ├── api/                    # REST API routes
│   │   ├── agent_routes.rs     # /api/agent/* (generate, validate)
│   │   ├── session_routes.rs   # /api/session/* (chat, execute)
│   │   └── template_routes.rs  # /api/templates/*
│   ├── dsl_v2/                 # Core DSL implementation
│   │   ├── mod.rs              # Module exports
│   │   ├── ast.rs              # AST types
│   │   ├── parser.rs           # Nom parser
│   │   ├── verb_registry.rs    # Unified verb registry
│   │   ├── verbs.rs            # Standard verb definitions
│   │   ├── mappings.rs         # DSL key → DB column mappings
│   │   ├── csg_linter.rs       # CSG validation
│   │   ├── execution_plan.rs   # Compiler
│   │   ├── executor.rs         # Database executor
│   │   ├── custom_ops/         # Non-CRUD operations
│   │   └── ref_resolver.rs     # Reference type resolution
│   ├── templates/              # DSL templates
│   │   ├── registry.rs         # Template definitions
│   │   └── renderer.rs         # Template → DSL
│   ├── mcp/                    # MCP server for Claude Desktop
│   │   ├── server.rs           # JSON-RPC server loop
│   │   ├── handlers.rs         # Tool implementations
│   │   ├── tools.rs            # Tool definitions
│   │   └── protocol.rs         # MCP protocol types
│   ├── database/               # Database services
│   ├── services/               # Business logic
│   └── bin/
│       ├── agentic_server.rs   # Main server binary
│       ├── dsl_cli.rs          # CLI tool
│       └── dsl_mcp.rs          # MCP server binary
├── crates/
│   └── dsl-lsp/                # LSP server
├── tests/
│   ├── db_integration.rs       # Database integration tests
│   ├── db_cli_test.sh          # CLI integration tests
│   ├── mcp_test.sh             # MCP protocol tests
│   └── scenarios/              # DSL test scenarios
│       ├── valid/              # 8 valid scenarios
│       └── error/              # 5 error scenarios
└── Cargo.toml
```

## Running the Server

```bash
cd rust/

# Build the server
cargo build --features server --bin agentic_server

# Run (requires DATABASE_URL and ANTHROPIC_API_KEY)
DATABASE_URL="postgresql:///data_designer" \
ANTHROPIC_API_KEY="your-key" \
./target/debug/agentic_server

# Open in browser
open http://localhost:3000
```

## Commands

```bash
cd rust/

# Build
cargo build --features server --bin agentic_server   # Main server
cargo build --features database                       # DSL library only
cargo build --features mcp --bin dsl_mcp             # MCP server
cargo build -p dsl-lsp                               # LSP server

# Test
cargo test --features database --lib              # 115 unit tests
cargo test --features database --test db_integration  # 10 DB tests
./tests/db_cli_test.sh                            # 11 CLI tests
./tests/scenarios/run_tests.sh                    # 13 DSL scenarios
./tests/mcp_test.sh                               # 12 MCP protocol tests

# Run CLI
./target/debug/dsl_cli lint file.dsl
./target/debug/dsl_cli execute file.dsl

# Clippy
cargo clippy --features server
cargo clippy --features database
cargo clippy --features mcp
```

## API Endpoints

### Web UI
| Endpoint | Description |
|----------|-------------|
| `GET /` | Agent session UI (Chat / DSL Editor / Results) |
| `GET /verbs` | Verb reference page |

### Agent DSL Generation
| Endpoint | Description |
|----------|-------------|
| `POST /api/agent/generate` | Generate DSL from natural language |
| `POST /api/agent/validate` | Validate DSL syntax/semantics |
| `GET /api/agent/domains` | List available DSL domains |
| `GET /api/agent/vocabulary` | Get vocabulary (optionally by domain) |
| `GET /api/agent/health` | Health check |

### Session Management
| Endpoint | Description |
|----------|-------------|
| `POST /api/session` | Create new session |
| `GET /api/session/:id` | Get session state |
| `DELETE /api/session/:id` | Delete session |
| `POST /api/session/:id/chat` | Send chat message |
| `POST /api/session/:id/execute` | Execute accumulated DSL |

### Templates
| Endpoint | Description |
|----------|-------------|
| `GET /api/templates` | List all templates |
| `GET /api/templates/:id` | Get template details |
| `POST /api/templates/:id/render` | Render template to DSL |

### DSL Viewer
| Endpoint | Description |
|----------|-------------|
| `GET /api/dsl/list` | List DSL instances |
| `GET /api/dsl/show/:ref` | Get latest DSL version |
| `GET /api/dsl/show/:ref/:ver` | Get specific version |
| `GET /api/dsl/history/:ref` | Get version history |

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
(document.catalog :cbu-id @fund :doc-type "CERT_OF_INCORP" :title "Acme Holdings Certificate")

;; Screening
(screening.pep :entity-id @john)
(screening.sanctions :entity-id @company)
```

## Verb Domains

| Domain | Verbs | Purpose |
|--------|-------|---------|
| cbu | 9 | Client Business Unit lifecycle |
| entity | 15+ | Dynamic verbs from entity_types table |
| document | 6 | Document catalog, request, extract |
| screening | 3 | PEP, sanctions, adverse-media |
| kyc | 2 | Investigation initiate, decide |
| ubo | 2 | Calculate, validate ownership |
| resource | 5 | Resource instance create, set-attr, activate, suspend, decommission |
| delivery | 3 | Service delivery record, complete, fail |

## Database Schema

See `docs/DATABASE_SCHEMA.md` for complete schema reference.

### Core Tables

| Table | Purpose |
|-------|---------|
| cbus | Client Business Units |
| entities | Base entity table (Class Table Inheritance) |
| entity_types | Entity type definitions |
| entity_proper_persons | Natural person extension |
| entity_limited_companies | Company extension |
| cbu_entity_roles | Entity-CBU-Role relationships |
| roles | Role definitions |
| document_catalog | Document records |
| document_types | Document type definitions |
| screenings | Screening records |
| cbu_resource_instances | Delivered resource instances (accounts, connections) |
| resource_instance_attributes | Typed attribute values for instances |
| service_delivery_map | Service delivery tracking (CBU → Product → Service → Instance) |

## MCP Server Tools

The MCP server (for Claude Desktop integration) exposes 8 tools:

| Tool | Description |
|------|-------------|
| `dsl_validate` | Parse and validate DSL source |
| `dsl_execute` | Execute DSL against database |
| `dsl_plan` | Show execution plan |
| `cbu_get` | Get CBU with entities, roles, documents |
| `cbu_list` | List/search CBUs |
| `entity_get` | Get entity details |
| `verbs_list` | List available DSL verbs |
| `schema_info` | Get entity types, roles, document types |

## Environment Variables

```bash
# Required for server
export DATABASE_URL="postgresql:///data_designer"
export ANTHROPIC_API_KEY="your-anthropic-key"

# Alternative LLM (optional)
export OPENAI_API_KEY="your-openai-key"
```

## Test Coverage

| Suite | Tests | Description |
|-------|-------|-------------|
| Unit tests | 115 | Core DSL functionality |
| DB integration | 10 | Database round-trips |
| CLI integration | 11 | End-to-end CLI tests |
| DSL scenarios | 13 | Valid (8) + error (5) scenarios |
| MCP protocol | 12 | MCP server JSON-RPC tests |
| **Total** | **161** | |

## Reference Docs

- `docs/DATABASE_SCHEMA.md` - Complete database schema reference
- `docs/DSL_TEST_SCENARIOS.md` - Test scenario documentation
