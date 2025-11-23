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

### Agentic DSL Generation Pipeline
**Natural language → DSL source generation with RAG context:**

```
User NL Input → RagContextProvider → LlmDslGenerator → ValidationPipeline → DSL Source
                      ↓                      ↓                    ↓
              (vocabulary,            (Anthropic API)      (syntax, semantic,
               examples,                                   business rules)
               attributes)
```

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
"ob-poc".cbus                    -- Client Business Units
"ob-poc".dictionary              -- Universal attribute dictionary
"ob-poc".attribute_values        -- Runtime attribute values
"ob-poc".document_catalog        -- Document management
"ob-poc".document_metadata       -- Extracted attributes
"ob-poc".document_relationships  -- Document links
"ob-poc".document_types          -- Document classifications
"ob-poc".dsl_instances           -- DSL instance storage (enhanced for RAG)
"ob-poc".vocabulary_registry     -- DSL verb registry for RAG
"ob-poc".taxonomy_crud_log       -- Taxonomy operation audit log
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
export ANTHROPIC_API_KEY="sk-ant-..."  # For LLM generation
```

## Key Components

### Forth Engine (`src/forth_engine/`)
- `parser_nom.rs` - Canonical NOM-based S-expression parser (single source of truth)
- `kyc_vocab.rs` - DSL verb implementations that emit CRUD statements
- `cbu_model_parser.rs` - CBU model DSL parsing (wraps NomDslParser)
- `mod.rs` - VM execution and sheet processing
- Verbs: `cbu.create`, `cbu.read`, `cbu.update`, `cbu.delete`, `document.catalog`, etc.

### Database Services (`src/database/`)
- `crud_executor.rs` - Executes CRUD statements against services
- `cbu_service.rs` - CBU table operations
- `document_service.rs` - Document catalog operations
- All SQL aligned with actual `data_designer` schema

### Agentic Services (`src/services/`)

#### RAG and LLM Generation (NEW)
- `rag_context_provider.rs` - Retrieves vocabulary, examples, attributes for LLM context
- `llm_dsl_generator.rs` - Anthropic Claude API integration with retry loop
- `validation_pipeline.rs` - Multi-stage validation (syntax, semantic, business rules)

#### DSL Source Generation
- `agentic_dsl_crud.rs` - DSL source generation from natural language
- `agentic_complete.rs` - Extended DSL generation for entities, roles, CBUs

#### Attribute Pipeline
- `source_executor.rs` - **Attribute value sourcing from documents/database**
  - `SourceExecutor` trait for pluggable attribute sources
  - `CompositeSourceExecutor` orchestrates multiple sources
  - Used by `attribute_lifecycle.rs` for value binding
  - Called from `execution/value_binder.rs`
- `sink_executor.rs` - Attribute value persistence
- `attribute_executor.rs` - Attribute operation execution
- `attribute_lifecycle.rs` - Full attribute lifecycle management

#### Taxonomy Operations
- `taxonomy_crud.rs` - **Product/Service/Resource CRUD operations**
  - `TaxonomyCrudService` for taxonomy management
  - Creates/reads/updates/deletes products, services, resources
  - Manages onboarding configurations
  - Logs operations to `taxonomy_crud_log`

#### Document Services
- `document_catalog_source.rs` - Document catalog operations
- `document_extraction_service.rs` - Document attribute extraction
- `document_type_detector.rs` - Document type classification
- `extraction_service.rs` - General extraction service
- `real_document_extraction_service.rs` - Production extraction implementation

### Taxonomy (`src/taxonomy/`)
- `dsl_parser.rs` - TaxonomyDslParser for NL preprocessing
- `crud_operations.rs` - Low-level taxonomy CRUD operations

### Execution (`src/execution/`)
- `value_binder.rs` - Binds attribute values using SourceExecutor chain

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

### KYC Operations
```lisp
(kyc.declare-entity :entity-type "PERSON" :name "John Doe" :data {})
(kyc.obtain-document :document-type "passport" :from @entity(id))
(kyc.verify-document :document-id @doc(id) :verification-method "ocr")
```

## SQL Migrations

### Agentic Generation Support
- `013_vocabulary_registry.sql` - DSL verb registry for RAG
- `014_enhance_dsl_instances.sql` - RAG columns (operation_type, confidence, etc.)
- `015_seed_vocabulary.sql` - Core vocabulary (KYC, CBU, document, attribute verbs)

## Architecture Principles

1. **Single Parser**: All DSL parsing through NomDslParser in forth_engine
2. **RAG-Enhanced Generation**: LLM uses vocabulary + examples + attributes for context
3. **Validation Pipeline**: Multi-stage validation with retry loop
4. **Schema Alignment**: Rust structs match actual database columns exactly
5. **No Legacy**: Dead code removed, no compatibility hacks

## Directory Structure

```
ob-poc/
├── rust/src/
│   ├── forth_engine/     # Forth VM, NOM parser, DSL verb implementations
│   ├── database/         # Database services (aligned with data_designer)
│   ├── services/         # Agentic generation, attribute pipeline, taxonomy
│   ├── taxonomy/         # Taxonomy management
│   ├── execution/        # Value binding, attribute execution
│   ├── domains/          # Domain-specific attribute sources
│   ├── ai/               # AI integration (OpenAI, Gemini)
│   └── ...
├── sql/
│   ├── migrations/       # Database migrations including agentic support
│   └── ...
└── CLAUDE.md            # This file
```

## Current Status

- **Architecture**: Single canonical NOM parser + RAG-enhanced generation
- **Schema**: 100% aligned with `data_designer` database
- **Agentic**: RAG context provider, LLM generator, validation pipeline implemented

---

**Last Updated**: 2025-11-23
