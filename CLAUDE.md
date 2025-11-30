# CLAUDE.md

This file provides guidance to Claude Code when working with this repository.

## Project Overview

**OB-POC** is a KYC/AML onboarding system using a declarative DSL. The DSL is the single source of truth for onboarding workflows.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     DSL Source Text                              │
│  (cbu.create :name "Fund" :jurisdiction "LU" :as @fund)         │
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
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    PostgreSQL (ob-poc schema)                    │
└─────────────────────────────────────────────────────────────────┘
```

### Key Components

| Component | File | Purpose |
|-----------|------|---------|
| Parser | `dsl_v2/parser.rs` | Nom-based S-expression parser |
| AST | `dsl_v2/ast.rs` | Program, Statement, VerbCall, Value types |
| Verb Registry | `dsl_v2/verb_registry.rs` | 53+ verbs across 8 domains |
| CSG Linter | `dsl_v2/csg_linter.rs` | Context-sensitive validation |
| Compiler | `dsl_v2/execution_plan.rs` | AST → ExecutionPlan |
| Executor | `dsl_v2/executor.rs` | Plan execution with DB |
| MCP Server | `mcp/` | Claude integration (8 tools) |
| LSP Server | `crates/dsl-lsp/` | IDE integration |

## Directory Structure

```
rust/
├── src/
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
│   ├── mcp/                    # MCP server for Claude
│   │   ├── server.rs           # JSON-RPC server loop
│   │   ├── handlers.rs         # Tool implementations
│   │   ├── tools.rs            # Tool definitions
│   │   └── protocol.rs         # MCP protocol types
│   ├── database/               # Database services
│   ├── services/               # Business logic
│   ├── intent/                 # Semantic intent schema
│   └── bin/
│       ├── dsl_cli.rs          # CLI tool
│       └── dsl_mcp.rs          # MCP server binary
├── crates/
│   └── dsl-lsp/                # LSP server
├── tests/
│   ├── db_integration.rs       # Database integration tests
│   ├── db_cli_test.sh          # CLI integration tests
│   └── scenarios/              # DSL test scenarios
│       ├── valid/              # 8 valid scenarios
│       └── error/              # 5 error scenarios
└── Cargo.toml
```

## Commands

```bash
cd rust/

# Build
cargo build --features database
cargo build --features mcp --bin dsl_mcp
cargo build -p dsl-lsp

# Test
cargo test --features database --lib              # 115 unit tests
cargo test --features database --test db_integration  # 10 DB tests
./tests/db_cli_test.sh                            # 11 CLI tests
./tests/scenarios/run_tests.sh                    # 13 DSL scenarios

# Run
./target/debug/dsl_cli lint file.dsl
./target/debug/dsl_cli execute file.dsl
DATABASE_URL="postgresql:///data_designer" ./target/debug/dsl_mcp

# Clippy
cargo clippy --features database
cargo clippy --features mcp
```

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

## Database Schema (ob-poc)

### Core Tables

| Table | Purpose | Key Columns |
|-------|---------|-------------|
| cbus | Client Business Units | cbu_id, name, jurisdiction, client_type |
| entities | Legal entities | entity_id, entity_type_id, name |
| entity_types | Entity type definitions | entity_type_id, type_code, name |
| cbu_entity_roles | Entity-CBU relationships | cbu_id, entity_id, role_id |
| roles | Role definitions | role_id, name |
| document_catalog | Documents | doc_id, cbu_id, document_type_code, status |
| document_types | Document type definitions | type_id, type_code, display_name |
| screenings | Screening records | screening_id, entity_id, screening_type, status |

### Key Relationships

```
cbus ←── cbu_entity_roles ──→ entities
              │
              ▼
            roles

entities ──→ entity_types

document_catalog ──→ document_types
```

## MCP Server Tools

The MCP server exposes 8 tools for Claude integration:

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

## Environment

```bash
export DATABASE_URL="postgresql:///data_designer"
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

### Running MCP Protocol Tests

```bash
cd rust/
./tests/mcp_test.sh
```

Tests cover: initialize, tools/list, all 8 tools, and full execution scenario.

## Reference Docs

- `docs/DATABASE_SCHEMA.md` - Complete database schema reference
- `docs/DSL_TEST_SCENARIOS.md` - Test scenario documentation
- `docs/DSL_CONFIG_DRIVEN_ARCHITECTURE.md` - Future: YAML-driven config
