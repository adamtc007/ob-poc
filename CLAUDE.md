# CLAUDE.md

This file provides guidance to Claude Code when working with this repository.

## Project Overview

**OB-POC** is a KYC/AML onboarding system using a declarative DSL. The DSL is the single source of truth for onboarding workflows.

## Architecture

```
DSL Source -> Verb Schema Validator -> Runtime -> CRUD Statements -> Database
```

### Key Concepts
- **Verb Schema**: 53 verbs across 8 domains with typed arguments
- **RefType Lookups**: Document types, roles, jurisdictions etc from DB
- **Symbol Binding**: @symbol references for data flow between verbs
- **LSP Server**: IDE integration for completions, hover, diagnostics

## Codebase Stats

- **54,500 lines of Rust** (src/ + crates/)
- **53 verbs** across 8 domains
- **PostgreSQL** backend with ob-poc schema

## Directory Structure

```
rust/
  src/
    forth_engine/
      schema/
        types.rs        # VerbDef, ArgSpec, SemType, RefType
        registry.rs     # VERB_REGISTRY (53 verbs)
        validator.rs    # Schema validation
        cache.rs        # SchemaCache for lookups
        verbs/          # Verb definitions by domain
          cbu.rs        # 9 verbs
          entity.rs     # 5 verbs
          document.rs   # 6 verbs
          kyc.rs        # 5 verbs
          screening.rs  # 7 verbs
          decision.rs   # 7 verbs
          monitoring.rs # 7 verbs
          attribute.rs  # 7 verbs
      runtime.rs        # DSL execution
      words.rs          # Word implementations
    database/           # DB services
    services/           # Business logic
    dsl_test_harness/   # Testing framework
  crates/
    dsl-lsp/            # LSP server (2,200 lines)
  Cargo.toml
```

## Commands

```bash
cd rust/

# Build
cargo build --features database
cargo check --features database

# Test
cargo test --features database --lib

# Clippy
cargo clippy --features database

# LSP server
cargo build -p dsl-lsp
./target/debug/dsl-lsp
```

## Verb Domains

| Domain | Verbs | Purpose |
|--------|-------|---------|
| cbu | 9 | Client Business Unit lifecycle |
| entity | 5 | Legal entity creation |
| document | 6 | Document management |
| kyc | 5 | Investigation and risk |
| screening | 7 | PEP/sanctions/adverse media |
| decision | 7 | Approval workflow |
| monitoring | 7 | Ongoing monitoring |
| attribute | 7 | Attribute management |

## DSL Example

```clojure
(cbu.ensure :cbu-name "Acme Fund" :jurisdiction "LU" :as @fund)

(entity.create-limited-company
  :name "Acme Holdings Ltd"
  :jurisdiction "GB"
  :as @company)

(cbu.attach-entity :cbu-id @fund :entity-id @company :role "InvestmentManager")

(document.request :document-type "CERT_OF_INCORP" :entity-id @company)

(screening.pep :entity-id @person)
(screening.sanctions :entity-id @company)

(decision.approve :investigation-id @inv :rationale "All checks passed")
```

## Database

Schema: ob-poc in PostgreSQL

Key tables:
- cbus - Client Business Units
- entities - Legal entities
- document_catalog - Documents
- document_types - Document type definitions
- roles - Entity roles
- master_jurisdictions - Jurisdiction codes
- screening_results - Screening outcomes
- decisions - Decision records

## Environment

```bash
export DATABASE_URL="postgresql:///data_designer"
```
