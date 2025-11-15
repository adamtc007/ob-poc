# OB-POC Database Schema

This directory contains the complete PostgreSQL schema for the OB-POC (Ultimate Beneficial Ownership - Proof of Concept) system.

## Quick Start

To set up a new database from scratch:

```bash
# 1. Create the complete schema (tables, indexes, constraints, functions)
psql -d your_database -f 00_master_schema.sql

# 2. Load seed data (optional but recommended)
psql -d your_database -f 01_seed_data.sql
```

## Files

### Active Scripts

- **00_master_schema.sql** - Complete schema definition
  - Generated directly from production database using `pg_dump`
  - Contains all 57 tables with complete structure
  - Includes all indexes, constraints, and foreign keys
  - Includes all functions and stored procedures
  - This is the single source of truth for schema structure

- **01_seed_data.sql** - Reference data and sample data
  - Dictionary attributes (76+ common financial attributes)
  - Sample CBUs (Client Business Units)
  - Entity types and roles
  - Essential reference data for development and testing

### Archive

The `archive/` directory contains historical migration scripts and old schema files. These are kept for reference only and should not be executed on new databases.

## Schema Overview

The ob-poc schema contains **57 tables** organized into functional areas:

### Core Business Objects
- `cbus` - Client Business Units
- `entities` - Legal entities (companies, partnerships, trusts, persons)
- `entity_proper_persons`, `entity_limited_companies`, `entity_partnerships`, `entity_trusts` - Entity type specializations
- `products`, `services` - Product and service definitions
- `ubo_registry` - Ultimate beneficial ownership tracking

### DSL Engine
- `dsl_instances` - DSL document instances
- `dsl_versions` - Version history
- `parsed_asts` - Compiled AST storage
- `dsl_execution_log` - Execution audit trail
- `dsl_domains`, `domain_vocabularies`, `verb_registry` - DSL grammar and vocabulary

### Attribute System
- `dictionary` - Universal attribute dictionary
- `attribute_registry` - Attribute type registry (UUID-based)
- `attribute_values` - Runtime attribute values
- `attribute_values_typed` - Typed attribute storage

### Document Management
- `document_catalog` - Document storage and metadata
- `document_metadata` - Cached extracted values
- `document_types` - Document classifications
- `document_relationships` - Document linkage

### Orchestration & Workflow
- `orchestration_sessions` - Workflow session tracking
- `orchestration_tasks` - Task management
- `orchestration_state_history` - State transitions
- `orchestration_domain_sessions` - Domain-specific sessions

### Supporting Tables
- `grammar_rules` - DSL grammar definitions
- `master_jurisdictions` - Jurisdiction reference data
- `master_entity_xref` - Entity cross-references
- `schema_changes` - Schema migration history
- `rag_embeddings` - AI/RAG vector storage
- Various relationship and mapping tables

## Key Features

### DSL-as-State Architecture
The database is designed to support a "DSL-as-State" pattern where accumulated DSL documents serve as both state representation and audit trail.

### AttributeID-as-Type Pattern
Variables in DSL are typed by AttributeID (UUID) referencing the universal dictionary, not primitive types. This provides:
- Type safety through dictionary validation
- Privacy classification (PII, PCI, PHI)
- Source/sink metadata
- Business domain context

### UUID Support
Full support for UUID-based attribute references with:
- Hybrid UUID + semantic ID approach
- Bidirectional resolution (O(1) HashMap)
- Runtime value binding with execution context
- Source tracking for audit trails

### Multi-Domain Support
Supports 7 operational domains with 70+ approved verbs:
- Core Operations (case management)
- Entity Management (registration, classification, identity)
- Product Operations (configuration, provisioning)
- KYC Operations (collection, verification, compliance)
- UBO Operations (ownership structures, calculations)
- Document Library (cataloging, extraction, lifecycle)
- ISDA Derivatives (master agreements, trades, margin)

## Maintenance

### Schema Updates

**IMPORTANT**: The master schema file (`00_master_schema.sql`) is generated from the production database. To update it:

```bash
# Extract current schema from database
pg_dump --schema-only --schema=ob-poc -d data_designer -U adamtc007 > 00_master_schema.sql
```

Do not manually edit the master schema file. Make changes to the database first, then regenerate the file.

### Adding Seed Data

To add new seed data, edit `01_seed_data.sql` directly. Use `INSERT ... ON CONFLICT DO NOTHING` for idempotent inserts.

## Database Connection

The system expects a `DATABASE_URL` environment variable:

```bash
export DATABASE_URL="postgresql://user:password@localhost/database_name"
```

For local development with default PostgreSQL settings:

```bash
export DATABASE_URL="postgresql:///data_designer?user=adamtc007"
```

## Testing

Test database connectivity from the Rust codebase:

```bash
cd ../rust
cargo run --bin test_db_connection --features="database"
```

## Schema Statistics

- **Total Tables**: 57
- **Schema Size**: ~4,400 lines of SQL
- **Seed Data**: ~275 lines
- **Attributes in Dictionary**: 76+ financial onboarding attributes
- **Sample CBUs**: Hedge funds and investment vehicles

## Version History

- **2025-11-15**: Consolidated to master schema file (generated from production DB)
- **2025-11-14**: UUID migration complete, document extraction system implemented
- **2025-11-13**: AI integration and attribute resolution
- **Previous**: Iterative migrations (archived)

## Architecture Documentation

For complete architectural details, see `/CLAUDE.md` in the repository root.

---

**Status**: Production-ready schema with comprehensive domain support
**Last Schema Export**: 2025-11-15
**Database Version**: PostgreSQL 14.19+
