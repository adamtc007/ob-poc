# OB-POC: Ultimate Beneficial Ownership Multi-Language Implementation

A multi-language proof-of-concept for modeling Ultimate Beneficial Ownership (UBO) discovery workflows and comprehensive onboarding processes in financial institutions.

## Overview

This project implements a declarative DSL system for:
- Entity relationship modeling
- Document-based evidence tracking  
- Multi-source data with conflict resolution
- UBO calculation algorithms
- Comprehensive onboarding workflows across multiple business domains
- Shared data dictionary and grammar system
- Audit trail generation

## Project Structure

This repository contains multiple language implementations of the UBO/onboarding system:

```
ob-poc/
├── rust/              # Rust implementation (DSL engine and compiler)
│   ├── src/           # Rust source code
│   ├── examples/      # Rust-specific examples
│   ├── Cargo.toml     # Rust dependencies
│   └── bacon.toml     # Auto-rebuild configuration
├── sql/               # Database schemas
│   ├── 01_schema.sql  # PostgreSQL schema
│   └── 02_seed_*.sql  # Seed data
├── docs/              # Shared documentation
│   └── ...            # Grammar specs, architecture docs
├── data/              # Shared reference data
└── README.md          # This file
```

## Implementations

### Rust Implementation (`/rust/`)

The Rust implementation serves as the core DSL engine and compiler:

- **EDN-Style Syntax**: Uses Clojure-inspired keywords for readability
- **S-Expression Structure**: Homoiconic syntax for easy parsing
- **Graph-Based Modeling**: Models ownership structures as property graphs
- **Multi-Domain Support**: Extensible to KYC, contracting, invoicing, etc.
- **PostgreSQL Integration**: Soft schema with JSON AST storage

**Quick Start:**
```bash
cd rust/
cargo build
cargo run --bin cli examples/zenith_capital_ubo.dsl

# Auto-rebuild on save
bacon
```

### Go Implementation (`/go/`)

The original Go POC is retained for demos and mocks only. All orchestration and database persistence have moved to the consolidated Rust `DslManager`. Direct DB access from Go has been removed.

Note on schema naming: The canonical PostgreSQL schema is `"ob-poc"`. Any legacy references to `"dsl-ob-poc"` exist only in migration scripts to normalize existing databases.

## System Architecture

### Multi-Domain Onboarding System

The system supports multiple business domains with shared infrastructure:

**Back Office:**
- KYC/AML (Know Your Customer/Anti-Money Laundering)
- Contracting (Contract management and legal processes)
- Invoicing (Billing and invoice management)

**Front Office:**
- Account Opening (New account setup and onboarding)
- Trade & Instruction Capture (Trading operations)
- Data Delivery (Market data and information delivery)

**Data Management:**
- Data Operations (Data management and analytics)

### Shared Components

- **EBNF Grammar**: Unified grammar for all domains with dialect support
- **Data Dictionary**: Shared attribute catalog (`customer.first_name`, `kyc.risk_rating`, etc.)
- **Verb Library**: Common operations (validate, collect, transform, check, etc.)
- **PostgreSQL Schema**: Soft schema design with JSON AST storage

## Database Schema

The system uses PostgreSQL with a comprehensive schema supporting:

- **Grammar Management**: EBNF grammars with versioning and composition
- **Domain Organization**: Hierarchical business domain structure
- **Shared Vocabulary**: Common attributes, verbs, and data types
- **DSL Compilation**: AST storage and execution tracking
- **Audit Trail**: Complete change tracking and compliance logging

**Setup:**
```bash
# Create database and schema
psql -d your_database -f sql/01_schema.sql
psql -d your_database -f sql/02_seed_attributes.sql
```

## DSL Syntax Example

```clojure
(define-kyc-investigation "zenith-capital-ubo-discovery"
  :target-entity "company-zenith-spv-001"
  :jurisdiction "KY"
  :ubo-threshold 25.0

  ;; Declare entities with shared data dictionary attributes
  (declare-entity
    :node-id "company-zenith-spv-001" 
    :label Company
    :properties {
      :legal-name "Zenith Capital Partners LP"
      :registration-number "KY-123456"
      :jurisdiction "KY"
    })

  ;; Use shared verbs for operations
  (validate customer.email_primary "john@example.com")
  (collect kyc.risk_rating :from "risk-engine")
  (check compliance.fatca_status :equals "NON_US")

  ;; Model ownership relationships
  (create-edge
    :from "alpha-holdings-sg"
    :to "company-zenith-spv-001"
    :type HAS_OWNERSHIP
    :properties {
      :percent 45.0
      :share-class "Class A Ordinary"
    }
    :evidenced-by ["doc-cayman-registry-001"])
)
```

## Development

### Rust Development

```bash
cd rust/

# Quick checks
./dev-check.sh

# Full development workflow
./dev-commit.sh "Your commit message"

# Auto-rebuild on save
bacon
```

### Shared Development

- Database schema changes go in `/sql/`
- Grammar definitions in `/docs/`
- Cross-language examples in project root

## Key Features

### Multi-Language Support
- Compare implementations across Rust and Go
- Shared database schema and grammar definitions
- Language-specific optimizations and patterns

### Domain-Specific Languages
- Unified EBNF grammar with dialect support
- Business area specific vocabularies
- Shared data dictionary across all domains

### Compliance & Audit
- Complete audit trail for all operations
- Document-based evidence tracking
- Regulatory reporting support (CRS, FATCA)

### Extensibility
- Pluggable grammar extensions
- Configurable business rules
- Multi-source data integration

## Architecture Philosophy

### Keywords as First-Class Citizens
Following Rich Hickey's EDN philosophy:
- **Self-Documenting**: `:customer.first_name` is immediately recognizable
- **Functional**: Keywords can look themselves up: `(:name entity)`
- **Namespace Safe**: `:compliance/ubo-threshold` prevents naming conflicts
- **Data as Code**: Same reader parses DSL and data structures

### Soft Schema Design
- ASTs stored as JSON for flexibility
- Schema evolution without migrations
- Language-agnostic data interchange

### Multi-Domain Modeling
- Shared vocabulary prevents data silos
- Cross-domain workflows (KYC → Account Opening → Trading)
- Consistent audit and compliance patterns

## Contributing

1. The project uses Rust as the primary implementation language
2. Follow the Rust development workflow described above
3. Update shared schemas in `/sql/` for database changes
4. Update documentation for grammar or architecture changes
5. Add cross-language examples for new features

## License

MIT License - see LICENSE file for details.

## Why This Architecture?

This multi-language approach allows us to:
- **Compare Approaches**: Evaluate different language paradigms for DSL implementation
- **Share Knowledge**: Common schema and grammar definitions across teams
- **Optimize Separately**: Language-specific performance and ecosystem advantages
- **Reduce Risk**: Multiple implementations provide fallback options
- **Enable Choice**: Teams can choose the implementation that best fits their needs

Perfect for a complex domain where different teams may have different language preferences, but everyone needs to work with the same business rules and data models.
