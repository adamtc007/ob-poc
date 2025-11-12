# OB-POC SQL Directory

This directory contains the essential SQL files for the OB-POC database system. The complete, canonical schema is defined in the root-level `ob-poc-schema.sql` file.

## Quick Start

### 1. Create Schema
The canonical schema file contains all table definitions:
```bash
psql -d your_database -f ../ob-poc-schema.sql
```

### 2. Load Seed Data
Load essential seed data in order:
```bash
# Universal attribute dictionary (76+ attributes)
psql -d your_database -f sql/seed_dictionary_attributes.sql

# Client Business Units and entity types
psql -d your_database -f sql/seed_cbus.sql
```

### 3. Apply Migrations (Optional)
If needed, apply incremental migrations:
```bash
psql -d your_database -f sql/migrations/001_*.sql
psql -d your_database -f sql/migrations/002_*.sql
# etc...
```

## File Descriptions

### Essential Files

- **`seed_dictionary_attributes.sql`** - Universal attribute dictionary with 76+ financial onboarding attributes
  - Contains AttributeID-as-Type mappings
  - Includes KYC, entity, document, and investment attributes
  - Required for DSL operations

- **`seed_cbus.sql`** - Client Business Unit seed data
  - Entity types (Partnership, Corporation, Proper Person, Trust)
  - Standard roles (General Partner, Investment Manager, etc.)
  - Sample CBU definitions for testing

### Migration Files

- **`migrations/`** - Incremental schema changes
  - Applied after main schema creation
  - Numbered for sequential application
  - Contains historical schema evolution

## Schema Architecture

The database implements a **DSL-as-State** architecture with:

- **55+ Tables** - Complete business domain coverage
- **AttributeID-as-Type** - Universal dictionary pattern
- **AI Integration** - Natural language to DSL conversion
- **Multi-Domain Support** - 7 operational domains with 70+ verbs

For complete architecture documentation, see `../ob-poc-architecture.md`.

## Data Model Overview

### Core Tables
- `cbus` - Client Business Units
- `dictionary` - Universal attribute dictionary
- `attribute_values` - Runtime attribute values
- `entities` - Central entity registry

### Specialized Areas
- `dsl_*` - DSL management and versioning
- `entity_*` - Entity type-specific tables
- `document_*` - Document library (V3.1)
- `ubo_registry` - Ultimate beneficial ownership

## Environment Variables

```bash
# Required for database operations
export DATABASE_URL="postgresql://user:password@localhost/database_name"

# Optional: AI integration
export OPENAI_API_KEY="your-openai-api-key"
export GEMINI_API_KEY="your-gemini-api-key"
```

## Testing Connection

```bash
# From Rust codebase
cargo run --bin test_db_connection --features="database"

# Direct PostgreSQL test
psql $DATABASE_URL -c "SELECT count(*) FROM \"ob-poc\".dictionary;"
```

## Archive

Historical SQL files have been moved to `sql_archive/` to reduce confusion. These files represent the development evolution but are no longer needed for system operation.

---

**Status**: Production-ready schema with comprehensive seed data
**Schema Version**: V3.1
**Last Updated**: 2025-01-14