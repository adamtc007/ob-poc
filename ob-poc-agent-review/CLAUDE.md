# CLAUDE.md

This file provides guidance to Claude Code when working with this repository.

## Project Overview

**OB-POC** is a production-ready Ultimate Beneficial Ownership (UBO) and comprehensive onboarding system implementing a declarative DSL approach. The project uses **DSL-as-State** architecture where accumulated DSL documents serve as both state representation and audit trail.

## Core Architecture

### Single Canonical Path: Forth DSL CRUD Runtime
**All database operations flow through the Forth NOM parser/compiler runtime.**

```
DSL Document → NOM Parser → Forth VM → CRUD Statements → CrudExecutor → Database
```

No bypassing. No legacy shortcuts. Schema alignment happens in one place.

### DSL-as-State Pattern
- **State = Accumulated DSL Document**: Each case's state is its complete DSL document
- **Immutable Event Sourcing**: Operations append to DSL, creating immutable versions
- **Executable Documentation**: DSL is human-readable, machine-parseable, audit trail, and workflow

### AttributeID-as-Type Pattern
Variables typed by AttributeID (UUID) referencing universal dictionary:
```lisp
(kyc.collect @attr{3020d46f-472c-5437-9647-1b0682c35935} @attr{0af112fd-ec04-5938-84e8-6e5949db0b52})
```

## Database Schema

Target database: `data_designer` with `ob-poc` schema.

### Key Tables
```sql
"ob-poc".cbus                    -- Client Business Units (doc_id, document_name, cbu_id, etc.)
"ob-poc".dictionary              -- Universal attribute dictionary  
"ob-poc".attribute_values        -- Runtime attribute values
"ob-poc".document_catalog        -- Document management (doc_id, document_name, document_type_id, metadata, status)
"ob-poc".document_metadata       -- Extracted attributes (doc_id, attribute_id, value jsonb)
"ob-poc".document_relationships  -- Document links (primary_doc_id, related_doc_id, relationship_type)
"ob-poc".document_types          -- Document classifications
"ob-poc".dsl_instances           -- DSL instance storage
```

### Schema Column Mappings
The Rust code uses these actual column names:
- `doc_id` (not document_id)
- `document_name` (not document_code)
- `metadata` (not extracted_attributes)
- `status` (not verification_status)
- `value` jsonb (not extracted_value)
- `primary_doc_id`/`related_doc_id` (not source/target)

## Development Commands

```bash
cd rust/

# Build and test
cargo build --features="database"
cargo test --features="database" --lib
cargo clippy --features="database"

# Run CRUD flow integration tests (requires DATABASE_URL)
cargo test --test cbu_document_crud_flow --features="database" -- --ignored --nocapture

# Environment
export DATABASE_URL="postgresql:///data_designer?user=username"
```

## Key Components

### Forth Engine (`src/forth_engine/`)
- `kyc_vocab.rs` - DSL verb implementations that emit CRUD statements
- `mod.rs` - VM execution and sheet processing
- Verbs: `cbu.create`, `cbu.read`, `cbu.update`, `cbu.delete`, `document.catalog`, etc.

### Database Services (`src/database/`)
- `crud_executor.rs` - Executes CRUD statements against services
- `cbu_service.rs` - CBU table operations
- `document_service.rs` - Document catalog operations
- All SQL aligned with actual `data_designer` schema

### Parser (`src/parser/`)
- NOM-based S-expression parser
- UUID attribute references: `@attr{uuid}`
- Semantic references: `@attr.semantic.id`

## DSL Verbs

### CBU Operations
```lisp
(cbu.create :cbu-name "Name" :client-type "HEDGE_FUND" :jurisdiction "GB" :nature-purpose "Investment" :description "Details")
(cbu.read :cbu-id "uuid")
(cbu.update :cbu-id "uuid" :name "New Name")
(cbu.delete :cbu-id "uuid")
```

### Document Operations
```lisp
(document.catalog :doc-id "DOC-001" :doc-type "UK-PASSPORT")
(document.verify :doc-id "DOC-001" :status "verified")
(document.extract :doc-id "DOC-001" :attr-id "uuid")
(document.link :primary-doc "uuid1" :related-doc "uuid2" :type "SUPPORTS")
```

## Test Results

```bash
cargo test --features="database" --lib
# 111 passed; 0 failed; 4 ignored

cargo clippy --features="database"
# Zero warnings
```

## Architecture Principles

1. **Single Path**: All CRUD through Forth runtime - no bypasses
2. **Schema Alignment**: Rust structs match actual database columns exactly
3. **No Legacy**: Dead code removed, no "legacy compatibility" functions
4. **Clippy Clean**: Zero warnings, explicit allows for intentional patterns

## Directory Structure

```
ob-poc/
├── rust/src/
│   ├── forth_engine/     # Forth VM and DSL verb implementations
│   ├── database/         # Database services (aligned with data_designer)
│   ├── parser/           # NOM-based DSL parser
│   ├── ai/               # AI integration (OpenAI, Gemini)
│   └── ...
├── sql/                  # Database schemas
└── CLAUDE.md            # This file
```

## Current Status

- **Clippy**: Zero warnings
- **Tests**: 111 passed
- **Schema**: 100% aligned with `data_designer` database
- **Architecture**: Single canonical path through Forth runtime

---

**Last Updated**: 2025-11-22
