# OB-POC Database Schema

This directory contains the shared PostgreSQL schema used by both the Go and Rust implementations of the OB-POC (Onboarding Proof of Concept) system.

## Schema Structure

The database uses the `ob-poc` schema and is organized into several key areas:

### Core Tables

- **`dsl_ob`** - Immutable, versioned DSL files
- **`cbus`** - Client Business Unit definitions
- **`products`** - Core product definitions
- **`services`** - Services offered with or without products

### Entity Management

- **`entities`** - Central entity registry
- **`entity_types`** - Different types of entities (Trust, Corporation, etc.)
- **`entity_*`** - Specific entity type tables:
  - `entity_trusts`
  - `entity_limited_companies`
  - `entity_partnerships`
  - `entity_proper_persons`

### Dictionary & Attributes

- **`dictionary`** - Master data dictionary (central pillar)
- **`attribute_values`** - Runtime values for onboarding instances
- **`prod_resources`** - Production resources

### Grammar & Vocabulary

- **`grammar_rules`** - Database-stored EBNF grammar definitions
- **`domain_vocabularies`** - Database-stored DSL verbs by domain
- **`verb_registry`** - Global verb registry for conflict detection
- **`vocabulary_audit`** - Vocabulary change tracking

### Product Requirements

- **`product_requirements`** - DSL operations and attributes required per product
- **`entity_product_mappings`** - Compatibility matrix for entity types and products
- **`product_workflows`** - Generated workflows for specific product-entity combinations

## File Organization

- **`00_init_schema.sql`** - Main schema definition
- **`01_schema.sql`** - Legacy schema (to be deprecated)
- **`02_seed_attributes.sql`** - Legacy seed data (to be deprecated)
- **`03_seed_dictionary_attributes.sql`** - Dictionary seed data
- **`04_seed_hedge_fund_1940act_cbus.sql`** - Hedge fund and 1940 Act CBU seed data
- **`05_seed_simplified_cbus.sql`** - Simplified CBU seed data
- **`migrations/`** - Database migration files

## Usage

### Initial Setup

```bash
# Create database and schema
psql -d your_database -f sql/00_init_schema.sql

# Load seed data
psql -d your_database -f sql/03_seed_dictionary_attributes.sql
psql -d your_database -f sql/04_seed_hedge_fund_1940act_cbus.sql
psql -d your_database -f sql/05_seed_simplified_cbus.sql
```

### Migrations

Run migrations in order:
```bash
psql -d your_database -f sql/migrations/001_standardize_cbu_id_uuid.sql
psql -d your_database -f sql/migrations/002_fix_foreign_key_constraints.sql
psql -d your_database -f sql/migrations/003_runtime_api_endpoints.sql
psql -d your_database -f sql/migrations/004_rename_individuals_to_proper_persons.sql
```

## Schema Design Principles

1. **Immutable DSL Storage** - DSL files are stored as immutable versions
2. **Entity-Centric Design** - All entities are registered in a central table with type-specific extensions
3. **Flexible Attributes** - Rich JSONB metadata for sources and sinks
4. **Product Requirements** - Configurable product-entity compatibility matrix
5. **Audit Trail** - Comprehensive change tracking for compliance

## Integration

Both Go and Rust codebases should reference this shared schema:

- Go applications should update their database connection to use schema `ob-poc`
- Rust applications should use the same schema name in their database configurations
- All SQL queries should be qualified with the schema name or use `SET search_path`

## Migration from Legacy Schema

The legacy files (`01_schema.sql`, `02_seed_attributes.sql`) will be deprecated once both codebases are fully migrated to use the new schema structure.
