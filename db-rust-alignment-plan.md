# DB ⇄ Rust/Forth/DSL Alignment Plan  
_based on `COMPLETE_SCHEMA_RUST_REVIEW.md`_

Date: 2025-11-21  
Scope: Align PostgreSQL schema with the **new Rust/Forth/DSL architecture**, using `COMPLETE_SCHEMA_RUST_REVIEW.md` as the mismatch catalogue.

---

## 0. Goals & Principles

**Goals**

1. Eliminate all schema ↔ Rust mismatches identified in `COMPLETE_SCHEMA_RUST_REVIEW.md`.
2. Make the **Forth/DSL/CRUD architecture** the *conceptual* source of truth, but:
3. Treat the **current normalized schema** (especially dictionary, document, CBU/entity topology) as the *persistence* baseline unless we explicitly decide otherwise.
4. End with:
   - A clean **attribute pipeline**: dictionary → DSL / Forth → CRUD → `attribute_values`.
   - A clean **CBU/entity topology**: `cbus`, `entities`, `proper_persons`, `cbu_entity_roles`.
   - A clean **document layer**: `document_types`, `document_catalog`, `document_metadata`, etc.
   - A clean **DSL storage & logging**: `dsl_instances`, `dsl_ast_snapshots`, `crud_operations`, `agent_verb_usage`.

**Key Principles**

- **No more half-stubs** in Rust: every DB table Rust touches gets a dedicated service / struct.
- **Sink/Source semantics** from the dictionary are authoritative for where attributes live.
- **DSL is state, DB is structure**:
  - DSL represents *what happened / what should happen*.
  - DB represents *the canonical state* and *audit trail*.

**Input Artifacts**

- `COMPLETE_SCHEMA_RUST_REVIEW.md` (this file) — the mismatch report
- `rust/src/forth_engine/*`
- `rust/src/database/*`
- `rust/src/parser/ast.rs` (Crud IR)
- `rust/src/execution/*`
- `rust/src/models/*`

Claude: **always cross-check specific column names and types against `COMPLETE_SCHEMA_RUST_REVIEW.md` when implementing.**

---

## 1. Decide Canonical Model Per Domain

We don’t blindly change DB or Rust; we decide per domain:

1. **Attribute dictionary & attribute_values**
   - Canonical: **new attribute pipeline design** described in the mismatch report (cbu-anchored, versioned, jsonb).
   - DB schema is *mostly* correct here; Rust is lagging.

2. **Entities / CBUs / Roles**
   - Canonical: **normalized relational model** in DB (`entities`, `entity_types`, `proper_persons`, `cbus`, `cbu_entity_roles`).
   - Rust should adapt to this topology rather than flatten it.

3. **DSL storage (`dsl_instances`, `dsl_ast_snapshots`)**
   - Canonical: DB schema in mismatch report (versioning, status).
   - Rust `DslRepository` must match.

4. **Document tables (`document_catalog`, `document_types`, `document_metadata`, `document_relationships`)**
   - Canonical: DB model (rich doc metadata, AI extraction support).
   - Rust and Forth must plug into these, not reinvent.

5. **CRUD logging (`crud_operations`, `agent_verb_usage`)**
   - Canonical: DB tables as given (they’re designed for agentic CRUD).
   - Rust should treat them as the audit sink.

---

## 2. Workstream A – Attribute Pipeline Alignment

### A.1 Attribute Values Table

Mismatch (from the report):

- DB: `attribute_values` has roughly:
  - `cbu_id`, `dsl_version`, `state`, `value` (jsonb), `observed_at`, unique `(cbu_id, dsl_version, attribute_id)` [exact schema in report].
- Rust (currently) expects columns like:
  ```rust
  INSERT INTO attribute_values (attribute_id, entity_id, attribute_value, value_type, created_at)
  ```
  and treats `entity_id` as string, `attribute_value` as text, etc.

**Decision:** **DB design is canonical**; Rust must be updated.

**Tasks for Claude:**

1. **Update Rust models & services**
   - In `rust/src/database/attribute_values_service.rs` (or add if missing):
     - Define an `AttributeValueRow` struct that matches the DB schema exactly (types + column names) from `COMPLETE_SCHEMA_RUST_REVIEW.md`.
     - Implement something like:
       ```rust
       pub async fn upsert_for_cbu(
           &self,
           cbu_id: Uuid,
           dsl_version: i32,
           state: &str,
           attr_id: Uuid,
           value: serde_json::Value,
           observed_at: DateTime<Utc>,
       ) -> Result<(), sqlx::Error>;
       ```
       and appropriate `get_*` helpers.
   - In `rust/src/database/dsl_repository.rs` and any other callers:
     - Replace any use of legacy columns (`entity_id`, `attribute_value`, `value_type`, `created_at`) with the correct ones (`cbu_id`, `value`, `observed_at`, `dsl_version`, `state`) as per the schema.

2. **Wire Forth/CRUD to the new attribute pipeline**
   - In `forth_engine/kyc_vocab.rs` (or a dedicated `attribute_vocab.rs`):
     - Ensure `set-attribute` and `require-attribute` verbs:
       - Work on dictionary-backed `AttributeId`.
       - Do **not** call SQL directly. They should:
         - Manipulate the VM stack and `RuntimeEnv`.
         - Append `CrudStatement`s (e.g. `DataUpdate` or a dedicated `SetAttribute` IR) into `RuntimeEnv.pending_crud`.
   - In `CrudExecutor`:
     - For `DataCreate` / `DataUpdate` or dedicated attribute-IR:
       - Use `AttributeValuesService` to persist to `attribute_values` with the correct `cbu_id`, `dsl_version`, `state`, `observed_at`.

3. **Tests**
   - Add integration tests that:
     - Apply a small DSL sheet that sets attributes for a CBU.
     - Confirm the correct rows appear in `attribute_values`, including `dsl_version`, `state`, and JSON `value`.

---

## 3. Workstream B – Entities, CBUs, and Roles

The review highlights mismatches around:

- `entities`
- `entity_types`
- `proper_persons`
- `cbus`
- `cbu_entity_roles`

And notes Rust sometimes bypasses `cbu_entity_roles` or treats `entities` too generically.

**Decision:** **Keep the DB topology as canonical** (it models real-world entities & roles well); adjust Rust.

**Tasks for Claude:**

1. **Create explicit domain services**

Create or refine:

- `rust/src/database/cbu_service.rs`:
  ```rust
  pub struct CbuService;

  impl CbuService {
      pub async fn create_cbu(&self, pool: &PgPool, fields: &NewCbuFields) -> Result<Uuid>;
      pub async fn get_cbu_by_business_ref(&self, pool: &PgPool, ref_: &str) -> Result<Option<CbuRow>>;
      // etc.
  }
  ```

- `rust/src/database/entity_service.rs`:
  ```rust
  pub struct EntityService;

  impl EntityService {
      pub async fn create_entity(&self, pool: &PgPool, fields: &NewEntityFields) -> Result<Uuid>;
      pub async fn create_proper_person(&self, pool: &PgPool, fields: &NewProperPersonFields) -> Result<(Uuid, Uuid)>;
  }
  ```

- `rust/src/database/cbu_entity_roles_service.rs`:
  ```rust
  pub struct CbuEntityRolesService;

  impl CbuEntityRolesService {
      pub async fn attach_entity_to_cbu(
          &self,
          pool: &PgPool,
          cbu_id: Uuid,
          entity_id: Uuid,
          role_id: Uuid,
      ) -> Result<()>;

      pub async fn get_entities_for_cbu(
          &self,
          pool: &PgPool,
          cbu_id: Uuid,
      ) -> Result<Vec<CbuEntityRoleRow>>;
  }
  ```

Make sure every struct’s fields (and types) match the DB schema in `COMPLETE_SCHEMA_RUST_REVIEW.md` exactly.

2. **Align CrudExecutor with this topology**

In `execution/crud_executor.rs` (or equivalent):

- For CRUD assets like `"CBU"`, `"PROPER_PERSON"`, `"ENTITY"`:
  - **Do NOT** insert straight into `entities` or `cbus` by hand.
  - Instead:
    - Use `CbuService` to create CBUs.
    - Use `EntityService` to create `entities` + `proper_persons`.
    - Use `CbuEntityRolesService` to create relationships in `cbu_entity_roles`.
- This ensures the code reflects the DB’s normalized structure, rather than duplicating logic.

3. **Defer semantic role/cardinality constraints to higher layer**

- Do **not** try to enforce “min 1 BeneficialOwner” or similar constraints at the DB-call level.
- That will be driven by the forthcoming **CBU Model DSL** (`DSL.CBU`) spec, not by raw SQL functions.
- For now, just ensure the relationships are stored correctly.

---

## 4. Workstream C – DSL Storage (`dsl_instances`, `dsl_ast_snapshots`)

The mismatch report notes structural differences for DSL storage tables.

**Decision:** unify Rust to match the DB schema verbatim.

**Tasks:**

1. **Align `DslRepository` to `dsl_instances`**

- In `rust/src/database/dsl_repository.rs`:
  - Create `DslInstanceRow` mirroring all columns of `dsl_instances` from the mismatch report:
    - `instance_id uuid`
    - `domain_name varchar(100)`
    - `business_reference varchar(255)`
    - `current_version integer`
    - `status varchar(50)`
    - etc.
  - Update existing methods:
    - `save_dsl_instance_with_ast(...)`
    - `load_by_business_reference(...)`
    - any others
  - Ensure you read/write:
    - `instance_id` as the primary key.
    - `status` consistently (e.g. `"draft"`, `"active"`, `"archived"`).

2. **Align AST snapshot usage**

- For `dsl_ast_snapshots` (as defined in the review doc):
  - Define `DslAstSnapshotRow` with correct columns: `snapshot_id`, `instance_id`, `dsl_version`, `ast_json`, `created_at`, etc.
  - Ensure you write snapshots with a stable link to `instance_id`.

---

## 5. Workstream D – Document Layer Alignment

The mismatch report lists rich document-layer tables:

- `document_types`
- `document_catalog`
- `document_metadata`
- `document_relationships`

Rust currently only uses a subset or uses incorrect column expectations.

**Decision:** treat the document schema as the canonical representation of all documents, including DSL ones (e.g. `DSL.CBU.MODEL`, `DSL.CRUD.CBU`).

**Tasks:**

1. **Introduce `DocumentService`**

Create `rust/src/database/document_service.rs`:

```rust
pub struct DocumentTypeRow { /* mirror document_types */ }
pub struct DocumentCatalogRow { /* mirror document_catalog */ }
pub struct DocumentMetadataRow { /* mirror document_metadata */ }

pub struct DocumentService;

impl DocumentService {
    pub async fn get_document_type_by_code(&self, pool: &PgPool, code: &str)
        -> Result<DocumentTypeRow>;

    pub async fn create_document(
        &self,
        pool: &PgPool,
        doc_type_code: &str,
        business_ref: &str,
        title: &str,
        created_by: &str,
    ) -> Result<Uuid>;

    pub async fn set_document_metadata(
        &self,
        pool: &PgPool,
        document_id: Uuid,
        attribute_id: Uuid,
        value: serde_json::Value,
    ) -> Result<()>;

    pub async fn link_documents(
        &self,
        pool: &PgPool,
        from_id: Uuid,
        to_id: Uuid,
        relationship_type: &str,
    ) -> Result<()>;
}
```

Ensure all fields and types line up with the DB schema.

2. **Use the document layer for DSL as-document**

- For DSL documents:
  - `DSL.CBU.MODEL`
  - `DSL.CRUD.CBU`
  - etc.

Use `DocumentService::create_document` and `set_document_metadata` to:
- Register DSL sheets as documents in `document_catalog`.
- Store metadata like DSL version, hash, domain, etc.

3. **Align any existing “document” Rust code**

- Any `document_*` Rust modules must be updated to use this service instead of ad-hoc SQL.

---

## 6. Workstream E – CRUD Logging & Agent Metrics

Tables in the report:

- `crud_operations`
- `agent_verb_usage`
- Possibly other agent-related tables.

These form the **audit trail** for agentic DSL CRUD.

**Tasks:**

1. **CRUD logging service**

Create `rust/src/database/crud_service.rs`:

```rust
pub enum OperationType { Create, Read, Update, Delete }
pub enum AssetType { Cbu, ProperPerson, Entity, Document /* etc */ }

pub struct CrudOperationRow { /* mirror crud_operations */ }

pub struct CrudService;

impl CrudService {
    pub async fn log_crud_operation(
        &self,
        pool: &PgPool,
        op_type: OperationType,
        asset_type: AssetType,
        entity_table_name: &str,
        generated_dsl: &str,
        ai_instruction: &str,
        affected_records: serde_json::Value,
        affected_sinks: Option<serde_json::Value>,
        contributing_sources: Option<serde_json::Value>,
        ai_provider: Option<&str>,
        ai_model: Option<&str>,
        execution_status: &str,
        selection_reasoning: Option<&str>,
        confidence_reported: Option<f32>,
        execution_success: bool,
        user_feedback: Option<&str>,
        correction_applied: Option<&str>,
        preceding_verbs: Option<&[String]>,
        workflow_stage: Option<&str>,
    ) -> Result<Uuid>;
}
```

Fields should exactly match `crud_operations` (names, types).

2. **Integrate into `CrudExecutor`**

- After each successful CRUD execution:
  - Build the log payload:
    - `generated_dsl` from the DSL instance.
    - `ai_instruction`, `ai_provider`, `ai_model` from context.
    - `affected_records` summarising primary keys or business refs.
    - `affected_sinks` / `contributing_sources` from dictionary + RuntimeEnv (if tracked).
  - Call `CrudService::log_crud_operation`.

3. **Agent verb usage**

Create `AgentVerbUsageService` for `agent_verb_usage`:

- Each time a Forth verb is chosen/executed by an agent:
  - Log a record with verb name, asset type, success, confidence, etc.
- This is mostly analytics; focus on correct column mapping.

---

## 7. Workstream F – Forth/RuntimeEnv <-> DB Boundaries

Goal: enforce clean layering.

**Target pattern:**

- Forth VM:
  - Pure stack + `RuntimeEnv` manipulation.
  - No direct SQLx calls.
  - Produces IR (`CrudStatement`s) + metadata in `RuntimeEnv`.

- Execution/CRUD layer:
  - Reads IR & context.
  - Talks to domain services and DB.
  - Logs to `crud_operations`.

**Tasks:**

1. **Add IR buffer to `RuntimeEnv`**

In `forth_engine/env.rs`:

```rust
pub struct RuntimeEnv {
    // existing fields…
    pub pending_crud: Vec<CrudStatement>,
}
```

2. **Refactor Forth verbs**

- Identify any Forth words doing direct DB or tightly coupled to DB structs.
- Refactor them to:
  - Pop arguments from stack.
  - Construct appropriate `CrudStatement` (e.g. `DataCreate { asset: "CBU", values }`).
  - Push into `env.pending_crud`.
- Only VM stack + env changes inside Forth; DB is downstream.

3. **Central orchestrator**

Where you currently call Forth execution:

```rust
let mut env = RuntimeEnv::new(request_id);
forth_engine::execute_sheet_into_env(&sheet, &mut env)?; // variant that retains env

for stmt in env.pending_crud {
    crud_executor.execute(stmt, &exec_ctx).await?;
}
```

This centralises DB schema sensitivity in a single, testable place (`CrudExecutor` + services).

---

## 8. Workstream G – Migrations & Cleanup

Once code is aligned to the schema in `COMPLETE_SCHEMA_RUST_REVIEW.md`, you can consider cleanup migrations.

**Rules for Claude:**

1. **No destructive migrations until:**
   - All Rust `sqlx::query!` / `query_as!` compile with the new structs.
   - Integration tests pass against a DB that matches the current review schema.

2. **When cleaning legacy columns/tables:**
   - Search the Rust tree for references first.
   - Remove or refactor those references.
   - Only then write goose migrations to drop/rename columns or tables.

3. **Document each migration** with reference back to the mismatch report section (“See COMPLETE_SCHEMA_RUST_REVIEW.md §X.Y”).

---

## 9. Recommended Implementation Order (Claude Sessions)

**Session 1 – Attribute Pipeline**
- Align `attribute_values` usage (A.1).
- Introduce/align `AttributeValuesService`.
- Fix Rust callers & add tests.

**Session 2 – CBU/Entity/Role Topology**
- Implement `CbuService`, `EntityService`, `CbuEntityRolesService` using the schema from the review.
- Update `CrudExecutor` to use them.

**Session 3 – DSL Storage**
- Fix `DslRepository` uses of `dsl_instances` and `dsl_ast_snapshots`.
- Add tests that create/load DSL instances.

**Session 4 – Document Layer**
- Implement `DocumentService` and refactor existing code to use it.
- Hook basic Forth “document” verbs to IR + CRUD.

**Session 5 – CRUD Logging & Agent Metrics**
- Implement `CrudService` for `crud_operations` and `AgentVerbUsageService`.
- Integrate into `CrudExecutor` and/or Forth orchestration.

**Session 6 – Forth Cleanup**
- Add `pending_crud` to `RuntimeEnv`.
- Refactor Forth verbs away from direct DB usage.
- Ensure full path: Forth → IR → CrudExecutor → DB.

**Session 7 – End-to-End Tests**
- Add integration tests for a simple onboarding scenario:
  - CBU creation.
  - Proper person attachment.
  - A few attributes and a dummy document.
- Assert on:
  - `cbus`
  - `entities` / `proper_persons`
  - `cbu_entity_roles`
  - `attribute_values`
  - `document_catalog` / `document_metadata`
  - `crud_operations`

---

## 10. Final Reminder for Claude

When implementing this plan:

- Treat `COMPLETE_SCHEMA_RUST_REVIEW.md` as the **source of truth** for table/column definitions.
- Do **not** invent new schema without explicit reason; align Rust to DB first.
- Keep layers clean:
  - Forth engine: DSL → IR, no SQL.
  - CrudExecutor + services: IR → SQLx.
  - Dictionary: source/sink semantics, attribute identity.
- Favour small, testable steps per session over huge refactors.

This plan is meant to be the **one-time, production-grade alignment** so that future changes live “above” it in clean DSLs and domain specs, not tangled in schema mismatches.
