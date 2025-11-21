# Schema Mismatch Report: Rust Code vs PostgreSQL Database

Generated: 2025-11-21

## Executive Summary

The Rust DSL code has diverged from the PostgreSQL database schema during refactoring. The database schema appears to be the correct/canonical version. This document details all mismatches requiring Rust code updates.

---

## Critical Mismatches

### 1. attribute_values Table

**Database Schema (CORRECT):**
```sql
attribute_values (
    av_id uuid PRIMARY KEY,
    cbu_id uuid NOT NULL REFERENCES cbus(cbu_id),
    dsl_ob_id uuid,
    dsl_version integer NOT NULL,
    attribute_id uuid NOT NULL REFERENCES dictionary(attribute_id),
    value jsonb NOT NULL,
    state text NOT NULL DEFAULT 'resolved',
    source jsonb,
    observed_at timestamp with time zone
)
UNIQUE(cbu_id, dsl_version, attribute_id)
```

**Rust Code Expects:**
```rust
// In dsl_repository.rs save_attribute()
INSERT INTO attribute_values (attribute_id, entity_id, attribute_value, value_type, created_at)
VALUES ($1::uuid, $2, $3, $4, NOW())
```

**Mismatches:**
- entity_id -> should be cbu_id (uuid, not string)
- attribute_value -> should be value (jsonb)
- value_type -> not needed, value is jsonb
- created_at -> should be observed_at
- Missing: dsl_version (required), state

**Files to Update:**
- rust/src/database/dsl_repository.rs - save_attribute(), save_execution_transactionally()
- rust/src/database/attribute_values_service.rs

---

### 2. entities Table

**Database Schema (CORRECT):**
```sql
entities (
    entity_id uuid PRIMARY KEY,
    entity_type_id uuid NOT NULL REFERENCES entity_types(entity_type_id),
    external_id varchar(255),
    name varchar(255) NOT NULL,
    created_at timestamp with time zone,
    updated_at timestamp with time zone
)
```

**Rust Code Expects:**
```rust
// In crud_executor.rs execute_create_tx()
INSERT INTO entities (entity_id, entity_type, legal_name, jurisdiction, status, created_at, updated_at)
```

**Mismatches:**
- entity_type -> should be entity_type_id (uuid FK to entity_types)
- legal_name -> should be name
- jurisdiction -> column does not exist
- status -> column does not exist

**Files to Update:**
- rust/src/database/crud_executor.rs - CBU_ENTITY_RELATIONSHIP and CBU_PROPER_PERSON handlers

---

### 3. dsl_instances Table

**Database Schema (CORRECT):**
```sql
dsl_instances (
    instance_id uuid PRIMARY KEY,
    domain_name varchar(100) NOT NULL,
    business_reference varchar(255) NOT NULL,
    current_version integer NOT NULL DEFAULT 1,
    status varchar(50) NOT NULL DEFAULT 'CREATED',
    created_at timestamp with time zone,
    updated_at timestamp with time zone,
    metadata jsonb
)
UNIQUE(domain_name, business_reference)
```

**Rust Code (DslRepository) - ALREADY UPDATED:**
The dsl_repository.rs has been updated to use correct columns but some queries may still be wrong.

---

### 4. cbu_entity_roles Table (for entity relationships)

**Database Schema:**
```sql
cbu_entity_roles (
    cbu_entity_role_id uuid PRIMARY KEY,
    cbu_id uuid NOT NULL REFERENCES cbus(cbu_id),
    entity_id uuid NOT NULL REFERENCES entities(entity_id),
    role_id uuid REFERENCES roles(role_id),
    created_at timestamp with time zone
)
```

**Rust Code Issue:**
The CrudExecutor tries to insert into entities directly for CBU_ENTITY_RELATIONSHIP, but should use cbu_entity_roles to link CBUs to entities with roles.

---

### 5. dictionary Table

**Database Schema (CORRECT):**
```sql
dictionary (
    attribute_id uuid PRIMARY KEY,
    name varchar NOT NULL,
    long_description text,
    group_id varchar NOT NULL,
    mask varchar,
    domain varchar,
    vector text,
    source jsonb,
    sink jsonb,
    created_at timestamp with time zone,
    updated_at timestamp with time zone
)
```

---

## Document Tables

### document_catalog
- 33 columns for document management
- Handles versioning, AI extraction, compliance
- Uses document_type_id FK to document_types

### document_types
- 22 columns defining document type metadata
- Has expected_attribute_ids and validation_attribute_ids arrays

### document_usage
- Tracks document usage in DSL workflows
- Links to dsl_version_id and cbu_id
- Records verb_used, verification_result, confidence_score

---

## Files Requiring Updates

### High Priority:

1. **rust/src/database/crud_executor.rs**
   - Fix entities INSERT (use entity_type_id, name)
   - Use cbu_entity_roles for relationships

2. **rust/src/database/dsl_repository.rs**
   - Fix save_attribute() - use correct columns
   - Fix save_execution_transactionally()

3. **rust/src/database/attribute_values_service.rs**
   - Update all queries to match actual schema

### Medium Priority:

4. **rust/src/forth_engine/kyc_vocab.rs**
5. **rust/src/database/cbu_service.rs**
6. **rust/src/cbu_model_dsl/service.rs**

---

## Test Command

After fixes:
```bash
cd rust && cargo run --bin cbu_live_test --features database
```
