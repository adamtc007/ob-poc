# ob-poc Master Architecture & Alignment Blueprint  
### Rust · Forth/DSL · PostgreSQL · Dictionary · Documents · CBU Model

Date: 2025-11-21  
Owner: Adam Cearns (Solution Architect)  
Scope: **Prevent future drift** between Rust/Forth/DSL implementation and PostgreSQL schema by defining clear canonical sources of truth, layers, and mapping rules.

---

## 0. Canonical Hierarchy (“Who is Master of What?”)

We had regressions where DB tables were rolled back to older shapes, breaking Rust refactors. This section makes **non-negotiable** decisions about what is canonical (“master”) and what must adapt (“derived”).

### 0.1 Canonical Sources of Truth

**1. Canonical Data Model (Storage & Relationships) → PostgreSQL Schema**  
- Tables like: `entities`, `entity_types`, `proper_persons`, `cbus`, `cbu_entity_roles`,  
  `attribute_values`, `document_*`, `dsl_instances`, `crud_operations`, etc.  
- These define the **persistent, auditable, relational** model.
- Any DSL / Rust change that contradicts this must be considered **wrong** unless we explicitly design and migrate the schema.

**2. Canonical Attribute Semantics → Attribute Dictionary**  
- Dictionary tables describe:  
  - Attribute identity (`attribute_id`)  
  - Meaning (semantics, descriptions)  
  - `source` / `sink` metadata (where values come from, where they should be persisted).
- The dictionary is **master** for:  
  - Which attributes exist.  
  - Which assets (CBU / ENTITY / DOCUMENT / DSL-DOC) they apply to.  
  - How they should be used in KYC/Onboarding.

**3. Canonical Business Flow / Execution Logic → DSL & Forth + CRUD IR**  
- High-level intent is expressed as DSL sheets (S-exprs).
- Forth engine interprets DSL and produces **CRUD IR** (`CrudStatement`) + runtime metadata.
- This layer is **master of “what should happen”**, but not of storage shape.

**4. Canonical Orchestration Layer → Rust RuntimeEnv + CrudExecutor + Services**  
- Rust is **responsible for bridging** DSL intent to DB reality.
- It **must not** assume schema; it must be coded against the canonical schema and dictionary.

### 0.2 Non-Canonical (Derived) Layers

- **Forth VM**:  
  - Executes DSL, manipulates stacks, builds IR.
  - It is not allowed to embed DB schema knowledge.
- **DSL Field Names & Shapes**:  
  - They are UX/projection concerns.
  - They are mapped to canonical DB schema via Rust.

### 0.3 Summary Matrix

| Concern                          | Canonical Layer        | Derived / Must Adapt         |
|----------------------------------|------------------------|------------------------------|
| Tables, columns, FKs             | PostgreSQL schema      | Rust, DSL                    |
| Attribute semantics, sink/source | Dictionary             | Forth, CrudExecutor          |
| Execution verbs & flows          | DSL + Forth            | Rust services, DB (via CRUD) |
| Mapping logic                    | Rust (RuntimeEnv/CRUD) | Forth, DSL field names       |
| Audit & agent telemetry          | DB (`crud_operations`) | Rust logging                 |

> **Rule:** If Rust/Forth and DB disagree, DB + Dictionary win. Rust/Forth **must be changed**, not DB, unless we explicitly design and migrate the database.

---

## 1. Data Model Overview (DB Canonical)

This is a conceptual map of the key DB domains you care about: CBU, Entities, Attributes, Documents, DSL, CRUD logs.

### 1.1 CBU & Entities

- `cbus`  
  - Represents a **Client Business Unit** (CBU), the core client model for onboarding/UBO.
  - Columns (simplified):  
    - `cbu_id uuid PK`  
    - `name text` (user-visible name; DSL field `cbu-name` maps here)  
    - `description text`  
    - `nature_purpose text`  
    - Possibly `created_at`, `updated_at`, etc.

- `entity_types`  
  - Lookup for entity categories (`PROPER_PERSON`, `COMPANY`, `TRUST`, etc.).

- `entities`  
  - Generic abstraction of parties (companies, trusts, persons).  
  - `entity_type_id` FK to `entity_types` (FK is canonical).

- `proper_persons`  
  - Person-specific details for entities of type `PROPER_PERSON`.

- `cbu_entity_roles`  
  - Join table: **which entities play which roles on which CBU**.  
  - FKs: `cbu_id`, `entity_id`, `role_id` (FK to roles table).

### 1.2 Attributes

- `dictionary` tables (or equivalent) define `DictionaryAttribute` with:  
  - `attribute_id` (uuid or text)  
  - `name`, `description`  
  - `source` jsonb (where values can come from – documents, systems, DSL)  
  - `sink` jsonb (which assets persist this attribute – e.g., CBU, ENTITY, DOCUMENT).

- `attribute_values`  
  - Canonical model for attribute storage:  
    - `cbu_id uuid`  
    - `dsl_version integer`  
    - `attribute_id uuid`  
    - `value jsonb`  
    - `state text` (e.g., `proposed`, `confirmed`, `derived`)  
    - `source jsonb` (doc ref, extraction method, etc.)  
    - `observed_at timestamp`  
  - Primary unique index: `(cbu_id, dsl_version, attribute_id)`.

### 1.3 Documents

- `document_types`  
  - Document classification (e.g., `UK-PASSPORT`, `DSL.CBU.MODEL`, `DSL.CRUD.CBU`).

- `document_catalog`  
  - Registered documents, including DSL sheets when treated as documents.

- `document_metadata`  
  - Attribute-value metadata about documents (e.g., passport number, issue date, DSL hash).

- `document_relationships`  
  - Links between documents or between documents and CBUs/entities.

### 1.4 DSL & Audit

- `dsl_instances`  
  - Persistent identity & metadata for DSL sheets.  
  - `instance_id uuid PK`  
  - `domain_name` (e.g., `KYC-ORCH`, `CBU-CRUD`)  
  - `business_reference` (CBU name, case ref, etc.)  
  - `current_version`, `status`.

- `dsl_ast_snapshots`  
  - Versioned AST snapshots of DSL instances for audit / replay.

- `crud_operations`  
  - Log of each CRUD action derived from DSL/Forth.

- `agent_verb_usage`  
  - Analytics on which verbs agents use and how successfully.

---

## 2. Execution Stack Overview (Rust + Forth + DSL)

### 2.1 Layers

1. **DSL Sheets (S-expr)**  
   - Authored by agents/tools (Forth-friendly S-expressions).

2. **Forth Engine (`forth_engine/*`)**  
   - Parses DSL to AST (via `NomKycParser` or similar).  
   - Compiles AST → `Program` (threaded code).  
   - Executes in `VM`, manipulating `Value` stack + `RuntimeEnv`.  
   - **Outputs**:  
     - `RuntimeEnv` with `pending_crud: Vec<CrudStatement>` and attribute/doc caches.

3. **CRUD IR (`CrudStatement`)**  
   - A Rust-level intermediate representation:  
     - `DataCreate`, `DataUpdate`, `DataDelete`, `DataRead` for assets like `"CBU"`, `"PROPER_PERSON"`, `"DOCUMENT"`, `"ATTRIBUTE"`.

4. **CrudExecutor + Domain Services**  
   - `CrudExecutor::execute(CrudStatement, ExecutionContext)`  
   - Calls: `CbuService`, `EntityService`, `CbuEntityRolesService`, `AttributeValuesService`, `DocumentService`, etc.

5. **DB**  
   - SQLx-based domain services persist to canonical tables.

### 2.2 CBU Model DSL (`DSL.CBU.MODEL`)

Separately, you have a **specification DSL** that describes:

- What attributes define a CBU (pick lists by group).  
- What states a CBU can be in.  
- What transitions (verbs) are legal between states.  
- What roles/entities are required and their cardinality.

This is purely **documentation/spec** (no SQL), validated against the dictionary, and stored as a document type `DSL.CBU.MODEL` + DSL instance.

Execution DSL (Forth) uses this spec to validate and shape allowed CRUD IR, but does **not** derive the schema from it.

---

## 3. Concrete Canonical / Mapping Decisions

### 3.1 CBU Table <-> DSL Fields

**Canonical:** DB table `cbus`.

- DB columns:  
  - `name` (canonical)  
  - `description`  
  - `nature_purpose`  

**DSL field names:**

- `:cbu-name`    → maps to DB `name`  
- `:description` → maps to DB `description`  
- `:nature-purpose` → DB `nature_purpose`  
- `:jurisdiction`, `:client-type`, `:status` → **not new columns** in `cbus`; they become attributes:
  - DSL fields → attribute IDs in dictionary (e.g., `CBU.JURISDICTION`, `CBU.CLIENT_TYPE`, `CBU.STATUS`)  
  - Those are persisted into `attribute_values` using `AttributeValuesService`.

**Rule:**  
If the field is semantically “core identity / narrative” (name, description, nature, etc.), it lives in `cbus`.  
If it is a more detailed property/flag, it lives in `attribute_values` via dictionary.

### 3.2 Attribute Values Model

**Canonical:** DB table `attribute_values` (cbu-centric, jsonb).  
Old Rust/DSL model with `entity_id`, `attribute_value text`, `value_type` is deprecated.

**Rust responsibilities:**

- Use `cbu_id`, `dsl_version`, `attribute_id`, `value jsonb`, `state`, `source`, `observed_at` exactly as DB defines.
- Provide helper methods to:
  - Read/write a typed attribute value in Rust `Value` form.  
  - Set `source` (e.g., `"UK-PASSPORT:doc-uuid"`) and `state` appropriately.

**Forth responsibilities:**

- When a DSL expression like:
  ```lisp
  (attr.set @attr("CBU.JURISDICTION") "GB")
  ```
  is executed, the Forth word should:
  - Turn it into a `CrudStatement` (e.g., `DataUpdate` for asset `"ATTRIBUTE"` or `"CBU"`).  
  - Include attribute identifier and the `Value` to be persisted.  
  - Not know anything about `attribute_values` columns.

### 3.3 Entities / Entity Types / Proper Persons

**Canonical:** normalized DB schema (`entities`, `entity_types`, `proper_persons`).

**DSL-level:**

```lisp
(entity.register
  :entity-type "PROPER_PERSON"
  :name "John Smith")
```

**Rust mapping:**

- `EntityService` must:
  - Resolve `"PROPER_PERSON"` → `entity_type_id` via `entity_types`.  
  - Insert into `entities` with proper FK.  
  - If a `PROPER_PERSON`, also create a row in `proper_persons` and link back to `entities`.

**CBU Entity Roles:**

- DSL:
  ```lisp
  (cbu.attach-entity
    :entity-ref "John Smith"
    :role "BeneficialOwner")
  ```

- DB:
  - Use `roles` table to resolve `"BeneficialOwner"` → `role_id`.  
  - Insert into `cbu_entity_roles(cbu_id, entity_id, role_id)`.

Again: DSL names → resolved via Rust/mapping into FK-driven DB.

### 3.4 dsl_instances – `business_reference` vs `case_id`

- `business_reference` (DB): long-lived, business-level identifier (e.g., “CBU:ACME-HOLDING” or “CASE:KYC-2025-001”).  
- `case_id` (Rust RuntimeEnv): transient execution context ID (unique per run, for logs & traces).

**Rule:**
- Do **not** rename or unify them.  
- Use `business_reference` in DB; keep `case_id` inside `RuntimeEnv` and log to telemetry if needed.

### 3.5 Documents

**Canonical:** `document_*` tables as-is.  
All documents, including DSL sheets used as documents, must fit into this model:

- `document_types` defines types like `DSL.CBU.MODEL`, `DSL.CRUD.CBU`, `UK-PASSPORT`.  
- `document_catalog` registers each physical/logical document.  
- `document_metadata` holds attribute-values about documents.  
- `document_relationships` links documents and/or documents-to-CBUs/entities.

No DB migration required here — only Rust services and DSL alignment.

---

## 4. Rust Layer Responsibilities (Production-Grade)

Rust is the **execution and mapping layer**. It must **stabilize** around the canonical DB schema and dictionary, not fight it.

### 4.1 Services to Own DB Responsibilities

Introduce/refine services, each deeply aware of DB schema:

- `CbuService`
- `EntityService`
- `ProperPersonService` (or part of EntityService)
- `CbuEntityRolesService`
- `AttributeValuesService`
- `DocumentService`
- `DslRepository`
- `CrudService` (for `crud_operations`)
- `AgentVerbUsageService`

Each service exposes typed methods, hides SQLx details from Forth/DSL layers, and aligns exactly with DB columns and types.

### 4.2 CrudExecutor

`CrudExecutor` is the **bridge from CRUD IR to services**:

- Accepts `CrudStatement` + `ExecutionContext` (which can include CBU model spec, dictionary references, case_id, etc.).
- Delegates to the appropriate service(s).  
- Logs results via `CrudService`.  

CrudExecutor **must not** embed SQL; it orchestrates services.

### 4.3 RuntimeEnv & Forth

`RuntimeEnv` holds execution context for Forth:

- `request_id` / `case_id`
- `cbu_id`, `entity_id` (optional)
- `attribute_cache`, `document_cache`
- `pending_crud: Vec<CrudStatement>`

Forth words manipulate the stack and `RuntimeEnv` only, building IR. They **never** touch DB.

---

## 5. DSL Layers: Spec vs Execution

### 5.1 CBU Model DSL (`DSL.CBU.MODEL`)

- Lives in new module: `rust/src/cbu_model_dsl/*`.  
- AST: `CbuModel`, `CbuStateMachine`, `CbuAttributesSpec`, etc.  
- Parser: `CbuModelParser::parse_str(&str) -> CbuModel`.  
- Validated by `CbuModelService` against dictionary (attributes exist, sink includes CBU).  
- Stored as:
  - DSL instance in `dsl_instances`  
  - Document in `document_catalog` with type `DSL.CBU.MODEL`
- Used by CrudExecutor to:
  - Check attributes used in CRUD for a CBU are valid members of the model.  
  - Enforce state machine constraints.

### 5.2 Execution DSL (Forth / CRUD Sheets)

- These are the actual **operational** DSL files, e.g.:

  ```lisp
  (cbu.create
    :cbu-name "ACME HOLDING LTD"
    :description "Holding company"
    :nature-purpose "Equity holding"
    :jurisdiction "GB")

  (cbu.attach-proper-person
    :person-name "John Smith"
    :role "BeneficialOwner")
  ```

- Forth words (`cbu.create`, `cbu.attach-proper-person`, `attr.set`, `document.catalog`, etc.):
  - Parse arguments
  - Build `CrudStatement`s and push to `env.pending_crud`

- After Forth execution, `CrudExecutor` runs each statement, validated against CBU Model DSL + dictionary + DB invariants.

---

## 6. Drift Prevention: How We Stop Reverting to Old Tables

Previously, changes reverted to an older DB shape and broke Rust. This architecture must include **guardrails**:

### 6.1 Single Source for Schema

- The DB schema and `COMPLETE_SCHEMA_RUST_REVIEW.md` are the **only accepted references** for table structure.
- Rust code must use `sqlx::query!` macros or equivalent so compile-time checks catch mismatches.

### 6.2 Migrations Discipline

- No manual DB edits.  
- All schema changes go through:

  1. Update schema definition / docs.  
  2. Update Rust structs & services.  
  3. Update migrations via goose.  
  4. Run tests.  
  5. Apply migrations only when tests pass.

### 6.3 CI / Gate

Short-term (local), use `cargo sqlx prepare` or similar to check queries against DB.  
Longer term, add CI to block merges if:

- SQLx offline checks fail.  
- Core end-to-end tests (CBU create & attach, attribute writes, document flows, CRUD logs) fail.

---

## 7. Implementation Roadmap (Claude’s Checklist)

1. **Align Rust structs & services to DB schema**  
   - Fix all mismatches noted in `COMPLETE_SCHEMA_RUST_REVIEW.md`.  
   - Implement `*Service` modules for CBU, Entities, Attributes, Documents, CRUD logs.

2. **Introduce `pending_crud` in `RuntimeEnv` & refactor Forth words**  
   - Remove any SQLx or schema assumptions from Forth.  
   - Make all verbs generate CRUD IR only.

3. **Implement `CrudExecutor` as the only path from IR → DB**  
   - Wire it to services and logging.

4. **Add CBU Model DSL & validation**  
   - Implement parsing, AST, validation, storage, and hooking into `ExecutionContext`.

5. **Write integration tests to lock behaviour**  
   - DSL → Forth → IR → CrudExecutor → DB.  
   - Validate results in: `cbus`, `entities`, `cbu_entity_roles`, `attribute_values`, `document_*`, `crud_operations`.

This master blueprint is what you and Claude should follow to avoid ever falling back to old table structures or ad-hoc Rust mappings again.
