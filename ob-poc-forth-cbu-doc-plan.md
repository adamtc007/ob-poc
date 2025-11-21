# ob-poc Forth DSL Integration Plan  
## Document Dictionary • Attribute Directory • CBU Builder • DSL CRUD

Target: **Zed + Claude dev sessions** implementing the next iteration of the `ob-poc` Forth DSL engine, wired into the existing Postgres schema and SQLx modules.

This plan assumes the current state of your repo (from the uploaded tarballs):

- Forth engine in `rust/src/forth_engine/*`
- Simplified `models` focused on the central **attribute dictionary**
- Database module in `rust/src/database/*` including `dictionary_service.rs`
- Database schema in `sql/ob-poc-schema.sql` with:
  - `attribute_values`, `document_types`, `document_metadata`, `document_catalog`, `document_relationships`
  - `cbus`, `cbu_entity_roles`, `entities`, `entity_types`, `proper_persons` (via migrations)
  - `crud_operations` for agentic CRUD logging
- SQL integration tests:
  - `sql/tests/uk_passport_agentic_crud_test.sql`
  - `sql/tests/cbu_assembly_agentic_crud_test.sql`
  - `sql/tests/comprehensive_entity_agentic_crud_test.sql`


---

## 1. Current Engine Capabilities (Quick Recap)

### 1.1 Forth Engine

Location: `rust/src/forth_engine/`

Core pieces:

- `ast.rs`
  - `DslSheet { id, domain, version, content }`
  - `Expr`:
    - `WordCall { name, args }`
    - `StringLiteral`, `IntegerLiteral`, `BoolLiteral`
    - `Keyword(String)` – `:case-id`, `:entity-id`, etc.
    - `AttributeRef(String)` – parsed from `@attr("KYC.LEI")`
    - `DocumentRef(String)` – parsed from `@doc("passport")`
  - `trait DslParser { fn parse(&self, sheet: &DslSheet) -> Result<Vec<Expr>, EngineError> }`

- `parser_nom.rs`
  - `NomKycParser` implementing `DslParser`
  - Clojure-style S-exprs, kebab-case verbs, supports keywords + @attr/@doc syntax

- `value.rs`
  - `AttributeId(pub String)`, `DocumentId(pub String)`
  - `Value::{Int, Str, Bool, Keyword, Attr(AttributeId), Doc(DocumentId)}`

- `env.rs`
  - `RuntimeEnv` holding:
    - `OnboardingRequestId(String)`
    - `case_id: Option<String>`
    - `attribute_cache: HashMap<AttributeId, Value>`
    - `document_cache: HashMap<DocumentId, Value>`
    - Optional `PgPool` behind `feature = "database"`
  - Helpers:
    - Attribute get/set, case-id minting
    - `generate_onboarding_template(...)` which emits a DSL `(case.create ...)` template

- `vocab.rs`
  - `WordId(pub usize)`
  - `WordImpl = Arc<dyn for<'a> Fn(&mut VM<'a>) + Send + Sync>`
  - `WordSpec { id, name, domain, stack_effect, impl_fn }`
  - `Vocab { name -> WordId, specs[WordId] }`

- `vm.rs`
  - Stack machine with:
    - `data_stack: Vec<Value>`
    - `return_stack: Vec<usize>`
    - `Program { instructions: Vec<Instruction> }`
  - `Instruction`:
    - `Op(CallWord(WordId) | Halt)`
    - `LitInt`, `LitStr`, `LitKeyword`, `AttrRef`, `DocRef`
  - `VM::step_with_logging(...)` for audit traces

- `compiler.rs`
  - `compile_expr(&Expr, &Vocab) -> Program`
  - For `WordCall`, compiles all args then emits `CallWord(word_id)`
  - For `AttributeRef`/`DocumentRef` currently just:
    - `AttributeId(name.to_string())`
    - `DocumentId(name.to_string())`
  - Placeholder for dictionary-driven resolution

- `kyc_vocab.rs`
  - Verbs across domains (`case.*`, `entity.*`, `products.*`, `kyc.*`, `compliance.*`, etc.)
  - Generic pattern:
    - keyword/value pairs on stack → `collect_keyword_pairs` → `process_pairs`
    - typically copy interesting values into `RuntimeEnv` and emit `AttributeId(key)` entries in `attribute_cache`

- `mod.rs`
  - `ExecutionResult { logs, case_id, success, version }`
  - `execute_sheet(...)` (non-DB) → currently returns only `Vec<String>` logs
  - `execute_sheet_with_db(...)` (DB) → `ExecutionResult`, leaving DB calls to the caller’s `DatabaseManager` for now

---

## 2. Target Capabilities

We want to layer on **three major capabilities**:

1. **Document dictionary & document flows**
   - Map document types (e.g. `passport`) to required attributes
   - Drive “solicit Passport for proper person ID” DSL flows
   - Persist extracted values into `attribute_values` and `document_metadata` / `document_catalog`

2. **Attribute directory integration**
   - Fully wire `AttributeId` to the dictionary in `"ob-poc".dictionary`
   - Use dictionary metadata to:
     - Validate attribute values
     - Drive UI/agent prompts
     - Drive extraction from documents

3. **CBU Builder + DSL CRUD**
   - CBU = primary client model backing onboarding & UBO
   - DSL-based CRUD for:
     - creating/updating CBUs (`cbus`)
     - linking entities/roles (`cbu_entity_roles`, `entities`, `proper_persons`, etc.)
     - logging all operations via `crud_operations`
   - This becomes the “state machine” backing CBU assembly, fully agent-driven.

The goal: **Claude in Zed** can:

- Read this plan
- Navigate to the indicated Rust modules
- Implement missing pieces step-by-step, guided by the existing SQL tests.

---

## 3. Phase Plan (High-Level)

We’ll do this in five phases, each of which is “Claude-friendly”:

1. **Phase 1 – Firm up RuntimeEnv & DB adapters**
2. **Phase 2 – Attribute directory integration**
3. **Phase 3 – Document dictionary & Passport flows**
4. **Phase 4 – CBU builder DSL & services**
5. **Phase 5 – DSL CRUD framework for CBU & related assets**

Each phase can be run in a dedicated Zed Claude session.

---

## 4. Phase 1 – Firm up `RuntimeEnv` & DB adapters

### 4.1 Create dedicated DB service modules

In `rust/src/database/` add:

1. `attribute_values_service.rs`
   - Responsible for `"ob-poc".attribute_values`
   - Functions:
     - `get_attribute_value(cbu_id, dsl_version, attribute_id)`
     - `set_attribute_value(cbu_id, dsl_version, attribute_id, value_json)`
     - `upsert_attribute_values(...)` for batch operations
   - Typed models for rows (or use `sqlx::query!` inline).

2. `document_service.rs`
   - Responsible for:
     - `"ob-poc".document_types`
     - `"ob-poc".document_metadata`
     - `"ob-poc".document_catalog`
     - `"ob-poc".document_relationships`
   - Functions:
     - `get_document_type_by_code(type_code) -> DocumentType`
     - `create_document_catalog_entry(...) -> doc_id`
     - `set_document_metadata(doc_id, attribute_id, value_json)`
     - `link_documents(primary_doc_id, related_doc_id, relationship_type)`  

3. `cbu_service.rs`
   - Responsible for:
     - `"ob-poc".cbus`
     - `"ob-poc".cbu_entity_roles`
     - `"ob-poc".entities` / `proper_persons` / partnerships / companies / trusts
   - Functions:
     - `create_cbu(name, description, nature_purpose) -> cbu_id`
     - `get_cbu_by_name(name) -> cbu_id`
     - `attach_entity_to_cbu(cbu_id, entity_id, role_id)`
     - helper: `ensure_role_exists(name) -> role_id`

4. `crud_service.rs`
   - Wraps `"ob-poc".crud_operations`
   - Functions:
     - `log_crud_operation(operation_type, asset_type, entity_table_name, generated_dsl, ai_instruction, affected_records, ai_provider, ai_model)`

These services will be called from the Forth **word implementations**.

### 4.2 Extend `RuntimeEnv`

In `forth_engine/env.rs`:

- Add fields:

  ```rust
  pub struct RuntimeEnv {
      pub request_id: OnboardingRequestId,
      pub cbu_id: Option<Uuid>,          // link DSL run to a CBU
      pub entity_id: Option<Uuid>,       // for current entity/proper person
      pub attribute_cache: HashMap<AttributeId, Value>,
      pub document_cache: HashMap<DocumentId, Value>,
      #[cfg(feature = "database")]
      pub pool: PgPool,
  }
  ```

- Add methods:

  ```rust
  impl RuntimeEnv {
      pub fn set_cbu_id(&mut self, id: Uuid) { self.cbu_id = Some(id); }
      pub fn ensure_cbu_id(&self) -> Result<Uuid, VmError> { ... }

      pub fn set_entity_id(&mut self, id: Uuid) { self.entity_id = Some(id); }
      pub fn ensure_entity_id(&self) -> Result<Uuid, VmError> { ... }
  }
  ```

- Update `execute_sheet_with_db` to **optionally** take a `cbu_id` and seed it into `RuntimeEnv`.

This gives the Forth words enough context to write to the correct CBU, entity, and attribute/doc tables.

---

## 5. Phase 2 – Attribute Directory Integration

Currently, `AttributeId` is a `String`, and the compiler simply wraps the name.

We want:

- Attribute **names** in DSL (e.g. `@attr("KYC.LEI")`) to line up with `"ob-poc".dictionary.attribute_id`.
- Optionally validate values against `dictionary_service`’s rules.

### 5.1 Resolution strategy

In `compiler.rs`:

- Replace stub:

  ```rust
  fn resolve_attribute_id(name: &str) -> Result<AttributeId, CompileError> {
      Ok(AttributeId(name.to_string()))
  }
  ```

with a pluggable resolution:

- Introduce a trait in `forth_engine`:

  ```rust
  pub trait AttributeResolver: Send + Sync {
      fn resolve(&self, name: &str) -> Result<AttributeId, CompileError>;
  }
  ```

- Update `compile_expr` to take an `Arc<dyn AttributeResolver>` (or pass it via `RuntimeEnv` if easier).
- Provide a default implementation in `database::dictionary_service`:

  ```rust
  pub struct DbAttributeResolver { pool: PgPool }

  impl AttributeResolver for DbAttributeResolver {
      fn resolve(&self, name: &str) -> Result<AttributeId, CompileError> {
          // SELECT attribute_id FROM "ob-poc".dictionary WHERE attribute_id = $1
          // or by name/alias as needed
      }
  }
  ```

In Zed, Claude can wire `DbAttributeResolver` to the dictionary table using existing `dictionary_service.rs` types.

### 5.2 Attribute Forth words

Add or refine verbs in `kyc_vocab.rs` (or a separate `attribute_vocab.rs`):

- `(attr.require @attr("KYC.LEI"))`
  - VM: ensure attribute exists in dictionary; if `attribute_values` has no row for this CBU/version, raise `VmError::MissingAttribute`.

- `(attr.set @attr("KYC.LEI") "5493001KJTIIGC8Y1R12")`
  - VM:
    - `ensure_cbu_id()`
    - Write to `"ob-poc".attribute_values` via `attribute_values_service`.

- `(attr.validate @attr("KYC.LEI") "value")`
  - VM:
    - Call `dictionary_service::validate_value(attribute_id, value)` if you want extra rigor.

**Idea:** For now keep `AttributeId` as `String` wrapping dictionary `attribute_id` text; you don’t need numeric IDs to get this working.

---

## 6. Phase 3 – Document Dictionary & Passport Flows

Goal: Use the **document dictionary** and document tables to drive flows like:

> “Solicit Passport for Proper Person ID” → DSL → DB.

### 6.1 Model the Document Dictionary

Use `"ob-poc".document_types`:

- `type_code` – e.g. `"UK-PASSPORT"`
- `required_attributes` – JSONB of dictionary attribute IDs that must be present:
  - e.g. `["KYC.PASSPORT_NUMBER", "KYC.FULL_NAME", "KYC.ADDRESS", "KYC.NATIONALITY"]`

In `document_service.rs` define:

```rust
pub struct DocumentType {
    pub type_id: Uuid,
    pub type_code: String,
    pub display_name: String,
    pub category: String,
    pub required_attributes: serde_json::Value,
}

pub async fn get_document_type_by_code(pool: &PgPool, code: &str) -> Result<DocumentType>;
```

### 6.2 Forth verbs for documents

Extend `kyc_vocab.rs` or add `document_vocab.rs` with verbs like:

1. `(document.catalog :document-type "UK-PASSPORT" :entity-id "..." :file-hash "..." :storage-key "..." :mime-type "application/pdf")`

   - Implementation:
     - Parse keyword/value pairs.
     - Use `document_service::create_document_catalog_entry(...)` to insert a row into `"ob-poc".document_catalog`.
     - Cache `DocumentId` in `RuntimeEnv.document_cache` (e.g. keyed by `:document-id` or `:entity-id`).

2. `(document.link-to-cbu :cbu-id "..." :document-id "..." :relationship-type "IDENTITY_PROOF")`

   - Implementation:
     - Use `RuntimeEnv.ensure_cbu_id()` or provided `:cbu-id` to find CBU.
     - Use `document_service::link_documents` if linking multiple docs, or create a CBU-doc linking table if needed (or reuse `orchestration_tasks` / `document_relationships`).

3. `(document.extract-attributes :document-id "..." :document-type "UK-PASSPORT")`

   - For now, simulate extraction by:
     - Taking a JSON blob (e.g. from some external extractor) and:
     - Writing each attribute into:
       - `document_metadata` (doc_id, attribute_id, value)
       - `attribute_values` (cbu_id, attribute_id, value) if we know the CBU
   - Later, you can integrate a real extraction pipeline.

4. `(document.require @doc("UK-PASSPORT") @attr("KYC.PASSPORT_NUMBER") @attr("KYC.FULL_NAME") ...)`

   - Implementation:
     - Look up the document type’s `required_attributes` from `DocumentType`.
     - Ensure there is at least one document of this type linked to the CBU and that the required attributes are present in `document_metadata` / `attribute_values`.

### 6.3 Align with `uk_passport_agentic_crud_test.sql`

Use that SQL file as **acceptance criteria**:

- Each DSL word should correspond to one or more SQL ops already shown in the test.
- You can even embed the DSL in comments next to the SQL and gradually migrate to actual DSL-driven execution.

Example DSL sketch for step 1 of the passport test:

```lisp
(document.catalog
  :document-id "uk-passport-agentic-001"
  :document-type "UK-PASSPORT"
  :entity-id "PROPER_PERSON:John-Smith"
  :issuer "UK-HO"
  :title "UK Passport - John Smith"
  :confidentiality-level "restricted")

(document.extract-attributes
  :document-id "uk-passport-agentic-001"
  :document-type "UK-PASSPORT"
  @attr("KYC.PASSPORT_NUMBER") "123456789"
  @attr("KYC.FULL_NAME") "John Smith"
  @attr("KYC.ADDRESS") "123 Baker Street, London, UK"
  @attr("KYC.NATIONALITY") "GB")
```

Claude can implement words from this shape.

---

## 7. Phase 4 – CBU Builder DSL & Services

CBU builder = the **heart** of the client model.

We already have tables:

- `"ob-poc".cbus` – main CBU records
- `"ob-poc".cbu_entity_roles` – linking CBUs to entity IDs and roles
- `"ob-poc".entities`, `"ob-poc".entity_types`
- `"ob-poc".proper_persons` and other entity subtype tables

And SQL tests:

- `cbu_assembly_agentic_crud_test.sql`
- `comprehensive_entity_agentic_crud_test.sql`

### 7.1 CBU DB service

In `database/cbu_service.rs`:

- Models:

  ```rust
  pub struct Cbu {
      pub cbu_id: Uuid,
      pub name: String,
      pub description: Option<String>,
      pub nature_purpose: Option<String>,
  }

  pub struct Role {
      pub role_id: Uuid,
      pub name: String,
  }
  ```

- Functions:

  ```rust
  pub async fn create_cbu(pool: &PgPool, name: &str, description: Option<&str>, nature_purpose: Option<&str>) -> Result<Uuid>;

  pub async fn get_cbu_by_name(pool: &PgPool, name: &str) -> Result<Option<Cbu>>;

  pub async fn ensure_role(pool: &PgPool, name: &str, description: &str) -> Result<Uuid>;

  pub async fn attach_entity_to_cbu(pool: &PgPool, cbu_id: Uuid, entity_id: Uuid, role_id: Uuid) -> Result<()>;
  ```

Claude can mirror the patterns used in the SQL tests to ensure correct semantics.

### 7.2 CBU builder DSL verbs

In `forth_engine/kyc_vocab.rs` (or a new `cbu_vocab.rs`) add verbs like:

1. `(cbu.create :cbu-name "Acme Fund" :description "Hedge fund" :nature_purpose "Global macro")`
   - VM:
     - Use `cbu_service::create_cbu` to insert a row.
     - Set `RuntimeEnv.cbu_id`.
     - Record core data as attributes (e.g. `KYC.CBU_NAME`) via `attribute_values_service`.

2. `(cbu.attach-entity :entity-id "..." :role "BENEFICIAL_OWNER")`
   - VM:
     - `ensure_cbu_id()`
     - `ensure_role("BENEFICIAL_OWNER")`
     - `attach_entity_to_cbu(cbu_id, entity_id, role_id)`

3. `(cbu.attach-proper-person :person-name "John Smith" :role "BENEFICIAL_OWNER")`
   - VM:
     - Lookup or create `proper_person` & `entities` row for John Smith.
     - Attach via `cbu_entity_roles`.

4. `(cbu.finalize :cbu-id "..." :status "READY_FOR_KYC")`
   - VM:
     - Optionally update CBU state (add field or new table if needed).
     - Maybe enqueue tasks in `orchestration_tasks`.

### 7.3 Align with CBU SQL tests

Use `cbu_assembly_agentic_crud_test.sql` as your truth table:

- For each step, add a comment with the DSL that *should* represent the SQL.
- Then implement Forth verbs that execute the SQL instead of the script.

Example DSL for a simple CBU with one company and one beneficial owner:

```lisp
(cbu.create
  :cbu-name "ACME HOLDING LTD"
  :description "Holding company for ACME group"
  :nature_purpose "Equity investment holding")

(cbu.attach-entity
  :entity-id "COMPANY:ACME-HOLDING-LTD"
  :role "PRIMARY_OPERATING_ENTITY")

(cbu.attach-proper-person
  :person-name "John Smith"
  :role "BENEFICIAL_OWNER")
```

---

## 8. Phase 5 – DSL CRUD Framework

The schema includes `"ob-poc".crud_operations` with fields:

- `operation_type` – `"CREATE" | "READ" | "UPDATE" | "DELETE"`
- `asset_type` – `'CBU', 'PROPER_PERSON', 'TRUST', 'ATTRIBUTE', 'DOCUMENT', ...`
- `entity_table_name`
- `generated_dsl`
- `ai_instruction`
- `affected_records` (JSONB)
- `execution_status`, `ai_confidence`, `ai_provider`, `ai_model`

This is perfect for **agentic CRUD logging**.

### 8.1 Crud service

In `database/crud_service.rs`:

```rust
pub enum AssetType { Cbu, ProperPerson, Trust, Attribute, Document }

pub enum OperationType { Create, Read, Update, Delete }

pub async fn log_crud_operation(
    pool: &PgPool,
    operation_type: OperationType,
    asset_type: AssetType,
    entity_table_name: &str,
    generated_dsl: &str,
    ai_instruction: &str,
    affected_records: serde_json::Value,
    ai_provider: Option<&str>,
    ai_model: Option<&str>,
) -> Result<Uuid>;
```

### 8.2 CRUD Forth verbs

Add verbs like:

1. `(crud.begin :operation-type "CREATE" :asset-type "CBU")`
   - VM:
     - Pushes some marker or sets a flag in `RuntimeEnv` indicating an active CRUD session.

2. `(crud.commit :entity-table "cbus" :ai-instruction "..." :ai-provider "OPENAI" :ai-model "gpt-5.1")`
   - VM:
     - Gathers context (e.g. `cbu_id`, updated records, current DSL sheet).
     - Calls `crud_service::log_crud_operation`.

3. Integrate CRUD logging into CBU/document verbs:
   - After successful `cbu.create`, call `crud_service::log_crud_operation` automatically.
   - After `document.catalog`, do the same.

This gives you what you asked for:

> DSL CRUD – that an agent can build – to handle CBU builder state changes – connected to the DB SQLx module.

Claude can then generate DSL “CRUD scripts” that both **effect state** and **log themselves** in `crud_operations`.

---

## 9. Zed + Claude Implementation Checklist

You can run these as discrete “sessions” with Claude in Zed.

**Session 1 – DB Services & Env**

- [ ] Create `database/attribute_values_service.rs`
- [ ] Create `database/document_service.rs`
- [ ] Create `database/cbu_service.rs`
- [ ] Create `database/crud_service.rs`
- [ ] Extend `RuntimeEnv` with `cbu_id`, `entity_id`, and `pool` wiring
- [ ] Update `execute_sheet_with_db` to populate `RuntimeEnv` correctly

**Session 2 – Attribute Directory Integration**

- [ ] Define `AttributeResolver` trait in `forth_engine`
- [ ] Implement `DbAttributeResolver` using `dictionary_service`
- [ ] Wire resolver into `compiler::compile_expr`
- [ ] Add `attr.require`, `attr.set`, `attr.validate` words in `kyc_vocab.rs` or `attribute_vocab.rs`
- [ ] Add small in-memory tests for attribute flows

**Session 3 – Document Dictionary & Passport**

- [ ] Flesh out `document_service` for document types, catalog, metadata
- [ ] Implement `document.catalog`, `document.link-to-cbu`, `document.extract-attributes`, `document.require`
- [ ] Mirror the behaviour of `uk_passport_agentic_crud_test.sql`
- [ ] Add an integration test in Rust that uses the Forth engine plus a test DB

**Session 4 – CBU Builder**

- [ ] Implement `cbu_service` functions aligning with `cbus` and `cbu_entity_roles`
- [ ] Add `cbu.create`, `cbu.attach-entity`, `cbu.attach-proper-person`, `cbu.finalize` verbs
- [ ] Re-express parts of `cbu_assembly_agentic_crud_test.sql` as DSL and test via Forth engine

**Session 5 – DSL CRUD**

- [ ] Implement `crud_service::log_crud_operation`
- [ ] Add `crud.begin` / `crud.commit` / auto-logging into key verbs
- [ ] Ensure all CBU and document operations write to `crud_operations`
- [ ] Add a small report query to show CRUD history for a CBU

---

## 10. Example End-to-End DSL Sketch

Here’s a sketch tying it all together for a simple onboarding:

```lisp
; Step 1: Create CBU
(cbu.create
  :cbu-name "ACME HOLDING LTD"
  :description "Holding company for ACME group"
  :nature_purpose "Equity investment holding")

; Step 2: Register proper person + attach as UBO
(entity.register
  :entity-id "PROPER_PERSON:John-Smith"
  :entity-type "PROPER_PERSON"
  :name "John Smith")

(cbu.attach-proper-person
  :person-name "John Smith"
  :role "BENEFICIAL_OWNER")

; Step 3: Catalog passport & extract attributes
(document.catalog
  :document-id "uk-passport-John-Smith-001"
  :document-type "UK-PASSPORT"
  :entity-id "PROPER_PERSON:John-Smith"
  :issuer "UK-HO"
  :title "UK Passport - John Smith"
  :confidentiality-level "restricted")

(document.extract-attributes
  :document-id "uk-passport-John-Smith-001"
  :document-type "UK-PASSPORT"
  @attr("KYC.PASSPORT_NUMBER") "123456789"
  @attr("KYC.FULL_NAME") "John Smith"
  @attr("KYC.ADDRESS") "123 Baker Street, London, UK"
  @attr("KYC.NATIONALITY") "GB")

; Step 4: Enforce KYC requirements
(attr.require @attr("KYC.PASSPORT_NUMBER"))
(attr.require @attr("KYC.FULL_NAME"))
(attr.require @attr("KYC.ADDRESS"))

; Step 5: Log CRUD
(crud.begin :operation-type "CREATE" :asset-type "CBU")
(crud.commit
  :entity-table "cbus"
  :ai-instruction "Onboard ACME holding with John Smith as UBO using passport"
  :ai-provider "OPENAI"
  :ai-model "gpt-5.1")
```

This is the target **shape** your agents can aim for; the Forth engine + DB services then make it real.

---

## 11. How to Use This Plan

1. Save this file into your repo, e.g.:

   ```text
   rust/docs/ob-poc-forth-cbu-doc-plan.md
   ```

2. In Zed, open the file and the relevant modules (e.g. `rust/src/forth_engine/*`, `rust/src/database/*`).

3. Start a Claude session with a prompt like:

   > “This is the ob-poc Forth DSL integration plan. Implement Session 1 tasks, modifying the files in this repo accordingly. Keep changes minimal and aligned with the existing schema and tests.”

4. Iterate session-by-session until all five phases are in place.

At that point, you’ll have:

- A **Forth-based DSL runtime** that knows about:
  - the attribute dictionary
  - the document dictionary and extraction flows
  - the CBU builder and entity/role graph
- A **DSL CRUD layer** that logs everything into `crud_operations` for full agentic auditability.

That’s your “CBU + KYC + Document + Attribute” core, ready for serious demos and real onboarding workflows.


---

## 12. Addendum – DSL CRUD as the Structural Bridge (Sink/Source Aware)

**Refinement of intent:**

- DSL CRUD is **not just logging**; it is the **agentic integration surface** to *all* data state:
  - CBUs
  - Entities (proper persons, companies, trusts, etc.)
  - Documents
  - Atomic attributes
- All entities and CBU attributes are represented in the **attribute dictionary**, but:
  - CBU / entity structures live **above** those atomic attributes.
  - The attribute & document dictionaries already encode **Sink** and **Source** semantics.
- This Sink/Source linkage must be:
  - Baked into the **DSL CRUD design**
  - Reflected in the **Forth vocab + nom parser layers**
  - Incorporated into the **attribute & document dictionary services**

This addendum changes the *ordering* and the *requirements* of the plan:

### 12.1 New Phase 0 – Sink/Source Semantics & Structural Bridge

Before Phase 1, introduce:

**Phase 0 – Define & Wire Sink/Source Semantics**

Tasks:

1. **Clarify dictionary model for Sink/Source**

   In the attribute and document dictionary schema (and corresponding Rust types), ensure:

   - Each attribute has:
     - `source_flag` (or equivalent) – indicates this attribute is a **producer** of data (e.g. extracted from a document, provided by user).
     - `sink_flag` – indicates this attribute is a **consumer** / target for persisted state (e.g. `KYC.CBU_NAME`, `KYC.UBO_GRAPH`, etc.).
   - Each document type’s metadata includes:
     - Which attributes are **produced** by that document (sources).
     - Which attributes must be **consumed** / satisfied by that document (sinks).

   Claude should:
   - Verify existing columns in the dictionary tables (attribute + document) that represent Sink/Source semantics.
   - Create/update Rust structs in `dictionary_service.rs` / `document_service.rs` to expose these flags explicitly.

2. **Add Sink/Source-aware helper APIs**

   In `dictionary_service.rs` and `document_service.rs`, add functions like:

   ```rust
   fn get_sink_attributes_for_asset(asset_type: AssetType) -> Vec<AttributeId>;
   fn get_source_attributes_for_doc_type(doc_type: &str) -> Vec<AttributeId>;
   ```

   Asset types:
   - `CBU`, `ENTITY`, `PROPER_PERSON`, `DOCUMENT`, etc.

3. **Expose Sink/Source in RuntimeEnv**

   In `forth_engine::env::RuntimeEnv` add:

   ```rust
   pub struct RuntimeEnv {
       // existing fields...
       pub sink_attributes: HashSet<AttributeId>,
       pub source_attributes: HashSet<AttributeId>,
   }
   ```

   And helpers:

   ```rust
   impl RuntimeEnv {
       pub fn is_sink(&self, attr_id: &AttributeId) -> bool { ... }
       pub fn is_source(&self, attr_id: &AttributeId) -> bool { ... }
   }
   ```

   Then, in `execute_sheet_with_db`, prime these sets based on:
   - CBU type / onboarding context
   - Document types involved
   - Any explicit hints passed into the engine

4. **Forth vocab hooks for Sink/Source**

   Modify / extend attribute & document verbs so they consult Sink/Source:

   - `attr.set`:
     - If `attr_id` is **not a sink** for this asset, log a warning or raise a `VmError::InvalidSink`.
   - `document.extract-attributes`:
     - Only writes attributes that are marked as **source** for that document type.
   - `attr.require`:
     - Prefer to require only attributes that are **sinks** for the CBU/entity context.

5. **Integrate Sink/Source into DSL CRUD**

   Update the **Phase 5 – DSL CRUD** design so that each CRUD operation:

   - Contains the set of **sink attributes** that the operation is responsible for.
   - Optionally lists **source documents/attributes** that populated those sinks.
   - Can therefore be used to reconstruct *why* a particular CBU/entity state exists (traceability).

   In `crud_service.rs`, add fields to `log_crud_operation` calls:

   ```rust
   affected_sinks: Vec<AttributeId>,
   contributing_sources: Vec<AttributeId>,
   ```

   (stored as JSONB).

### 12.2 Bridging CBU Structure and Atomic Attributes

The **bridge** between high-level CBU structure and atomic attributes should be:

- CBU/Entity **schemas** (which define which attributes are sinks for that structure).
- The **dictionary** (attributes + documents) marking each attribute as sink/source.
- DSL CRUD + Forth words that:
  - Operate at the **CBU/Entity level** (e.g. `(cbu.create ...)`, `(cbu.attach-entity ...)`).
  - Internally:
    - Resolve which attributes are sinks for the CBU/entity/role.
    - Write to `attribute_values` only for those sink attributes.
    - Use document flows to populate sources → sinks (e.g. passport → KYC.PASSPORT_NUMBER).

Concrete steps to add to existing phases:

- **Phase 2 (Attribute Directory Integration)**
  - MUST:
    - Use Sink/Source flags from Phase 0.
    - Ensure `attr.set` / `attr.require` behaviours respect those flags.
- **Phase 3 (Document Dictionary & Passport)**
  - MUST:
    - For `document.extract-attributes`, only treat attributes as valid sources if flagged as such.
- **Phase 4 (CBU Builder)**
  - MUST:
    - When creating/attaching entities/roles, load the **sink attribute set** for the CBU + entity + role, and:
      - Use that set to drive which attributes the CBU builder DSL should populate.
- **Phase 5 (DSL CRUD)**
  - MUST:
    - Log, per operation, the **sinks** it touched and the **sources** used, based on dictionary Sink/Source flags and RuntimeEnv’s tracking.

### 12.3 Implication for Implementation Order

Because this Sink/Source linkage is foundational, the recommended **revised order** is:

1. **Phase 0 – Sink/Source Semantics & Structural Bridge** (this addendum)
2. Phase 1 – DB Services & RuntimeEnv wiring
3. Phase 2 – Attribute Directory Integration (now Sink/Source aware)
4. Phase 3 – Document Dictionary & Passport flows (Sink/Source aware)
5. Phase 4 – CBU Builder DSL & services (uses sink sets to shape required attributes)
6. Phase 5 – DSL CRUD (logs sink/source usage per operation)

Claude should treat Phase 0 as **non-optional** and complete it before any deeper integration work.
