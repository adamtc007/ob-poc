# Questions on DB-Rust Alignment Plan

## Clarifications Needed Before Implementation

### 1. CBU Table - Conflicting Information

The plan states "cbu is correct" but:
- Current DB has: `name`, `description`, `nature_purpose`
- DSL/Forth uses: `cbu-name`, `client-type`, `jurisdiction`, `status`

**Question:** Should we:
a) Keep current DB columns and map DSL fields in Rust (e.g., `cbu-name` → `name`)?
b) Migrate DB to match DSL field names exactly?
c) Something else?

---

### 2. Attribute Values - Two Different Models

**Current DB model:**
```sql
attribute_values (
    cbu_id uuid,
    dsl_version integer,
    attribute_id uuid,
    value jsonb,
    state text,
    source jsonb,
    observed_at timestamp
)
```

**DSL/Rust expects:**
```sql
attribute_values (
    entity_id varchar,
    attribute_id uuid,
    attribute_value text,
    value_type varchar
)
```

**Question:** Which model is canonical?
- The plan says "DB design is canonical" for attributes
- But also says "DSL/Forth is master"
- These are fundamentally different models (jsonb vs text, cbu_id vs entity_id)

---

### 3. Entities Table - FK vs String Type

**Current DB:** `entity_type_id uuid` (FK to entity_types table)
**DSL expects:** `entity_type varchar` (direct string)

**Question:** Should we:
a) Keep normalized FK design and look up type strings in Rust?
b) Denormalize to match DSL's direct string approach?

---

### 4. CBU_ENTITY_RELATIONSHIP vs cbu_entity_roles

**Current DB:** `cbu_entity_roles` with `role_id uuid` FK to roles table
**DSL expects:** `role varchar` as direct string

**Question:** Same as above - normalize or denormalize?

---

### 5. dsl_instances - case_id vs business_reference

The plan mentions both terms. 

**Question:** Are these the same field with different names, or different concepts?

---

### 6. Document Tables

The plan mentions document alignment but doesn't specify if document tables need migration.

**Question:** Should document tables (`document_catalog`, `document_types`, etc.) be left as-is or migrated?

---

## Summary

The core question is: **What is actually canonical?**

Option A: DSL field names/types are canonical → Migrate DB to match
Option B: DB normalized design is canonical → Map in Rust code
Option C: Hybrid approach → Specify exactly which tables follow which rule

Please clarify before I proceed with implementation.
