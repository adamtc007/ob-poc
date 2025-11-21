# CBU Document-Directed DSL CRUD Execution Plan  
## (DSL.CBU.MODEL → DSL.CRUD.CBU.TEMPLATE → DSL.CRUD.CBU → Forth → CrudExecutor → DB)

Date: 2025-11-21  
Owner: Adam Cearns  
Context: ob-poc / CBU onboarding + UBO platform

Goal: **Make CBU the reference implementation** of “document-directed DSL CRUD execution”, wired end-to-end through:

- CBU Model DSL (`DSL.CBU.MODEL`) – business spec
- CBU CRUD Templates (`DSL.CRUD.CBU.TEMPLATE`) – generated recipes
- CBU CRUD Sheets (`DSL.CRUD.CBU`) – concrete runs
- Forth engine – parses & emits CRUD IR
- CrudExecutor + Rust services – maps IR to DB using SQLx
- PostgreSQL schema – canonical storage & relationships
- Dictionary (with `source` / `sink`) – attribute semantics & routing

Once this pattern is solid for CBU, we can reuse it for other domains (ISDA, fund lifecycle, etc.).

---

## 1. Concepts and Document Types

### 1.1 CBU Model Document (`DSL.CBU.MODEL`)

**Role:** Business specification of the CBU in DSL form.

Contains:

- Model identity: `:id`, `:version`, `:description`, `:applies-to`
- Attribute groups as **chunks**:
  - `core`, `contact`, `kyc`, etc.
  - Each chunk has `required` and `optional` attributes.
- States and transitions:
  - Stable states (`Proposed`, `PendingKYC`, `Active`, etc.)
  - Transition verbs (`cbu.submit`, `cbu.approve`, `cbu.close`…)
  - For each transition:
    - `:verb`
    - `:from`, `:to`
    - `:chunks` that must be complete
    - optional `:preconditions` (attributes that must be present)

Attributes are always referenced as dictionary IDs:

```lisp
@attr("CBU.LEGAL_NAME")
@attr("CBU.LEGAL_JURISDICTION")
@attr("KYC.LEI")
```

No SQL, no DB table names in this DSL.

**DB representation:**

- DSL text in `dsl_instances` (`domain_name = 'CBU-MODEL'` or similar).
- Document in `document_catalog` with `type_code = 'DSL.CBU.MODEL'`.
- Metadata in `document_metadata` describing id, version, model applicability, etc.

---

### 1.2 CBU CRUD Template Document (`DSL.CRUD.CBU.TEMPLATE`)

**Role:** Parametrised “recipe” for creating/updating a CBU instance according to a given model.

Examples:

- A template for initial CBU creation (`Proposed → PendingKYC` transition) that includes `core` + `contact` chunks.
- A template for KYC completion (`PendingKYC → Active`) that includes `kyc` chunk.

Template DSL might look like:

```lisp
(cbu.create
  :cbu-name             "{{CBU.LEGAL_NAME}}"
  :description          "{{CBU.NATURE_PURPOSE}}"
  :nature-purpose       "{{CBU.NATURE_PURPOSE}}"
  :jurisdiction         "{{CBU.LEGAL_JURISDICTION}}"
  :registered-address   "{{CBU.REGISTERED_ADDRESS}}"
  :primary-contact-email "{{CBU.PRIMARY_CONTACT_EMAIL}}")
```

- Placeholders (`{{ATTRIBUTE_ID}}`) are either:
  - Filled by agent/UI before execution, or
  - Left as `VAR_ATTRIBUTE_ID` markers the agent must resolve.

**DB representation:**

- DSL text in `dsl_instances` (domain `CBU-CRUD-TEMPLATE`).
- Document in `document_catalog` (`type_code = 'DSL.CRUD.CBU.TEMPLATE'`).
- `document_relationships` linking template document to:
  - CBU model doc (`DSL.CBU.MODEL`) it was generated from.

---

### 1.3 CBU CRUD Sheet (`DSL.CRUD.CBU`)

**Role:** Actual DSL that Forth will execute for a specific CBU or onboarding case.

Example:

```lisp
(cbu.create
  :cbu-name "ACME HOLDING LTD"
  :description "Holding company for ACME group"
  :nature-purpose "Equity investment holding"
  :jurisdiction "GB"
  :registered-address "123 Baker Street, London, UK"
  :primary-contact-email "ops@acme-holding.example")

(cbu.attach-proper-person
  :person-name "John Smith"
  :role "BeneficialOwner")
```

- All required attributes from relevant chunks must either:
  - Be present here, or
  - Already exist in `attribute_values` (from previous runs/documents).

**DB representation:**

- DSL text in `dsl_instances` (`domain_name = 'CBU-CRUD'`).
- Document in `document_catalog` (`type_code = 'DSL.CRUD.CBU'`).
- `document_metadata` includes:
  - CBU model id / version
  - template id (if created from template)
  - case id / business_reference
- `document_relationships` link:
  - the CRUD sheet to its template (`DSL.CRUD.CBU.TEMPLATE`)
  - the CRUD sheet to the target CBU (once created)

---

## 2. Implementation Phases

We’ll implement the CBU document-directed DSL CRUD stack in **incremental phases**, but everything builds on the canonical architecture in `ob-poc-master-architecture-v1.md` (DB is master, dictionary is master, Rust is mapping, Forth is execution engine).

### Phase 1 – CBU Model DSL (Spec Layer)

**Goal:** Implement the `DSL.CBU.MODEL` spec end-to-end (parse, validate, store), including attribute chunks and transitions.

#### 1.1 `cbu_model_dsl` module

Files:

- `rust/src/cbu_model_dsl/mod.rs`
- `rust/src/cbu_model_dsl/ast.rs`
- `rust/src/cbu_model_dsl/parser.rs`
- `rust/src/cbu_model_dsl/ebnf.rs`
- `rust/src/cbu_model_dsl/service.rs`

#### 1.2 AST Design

In `ast.rs`:

```rust
pub struct CbuModel {
    pub id: String,
    pub version: String,
    pub description: Option<String>,
    pub applies_to: Vec<String>,
    pub chunks: Vec<CbuAttributeChunk>,
    pub states: CbuStateMachine,
    pub roles: Vec<CbuRoleSpec>,
}

pub struct CbuAttributeChunk {
    pub name: String,        // "core", "contact", "kyc"
    pub required: Vec<String>, // attribute_ids (e.g. "CBU.LEGAL_NAME")
    pub optional: Vec<String>,
}

pub struct CbuStateMachine {
    pub initial: String,
    pub finals: Vec<String>,
    pub states: Vec<CbuState>,
    pub transitions: Vec<CbuTransition>,
}

pub struct CbuState {
    pub name: String,
    pub description: Option<String>,
}

pub struct CbuTransition {
    pub from: String,
    pub to: String,
    pub verb: String,               // e.g. "cbu.submit"
    pub chunks: Vec<String>,        // e.g. ["core", "contact"]
    pub preconditions: Vec<String>, // attribute_ids
}

pub struct CbuRoleSpec {
    pub name: String,  // "BeneficialOwner"
    pub min: u32,
    pub max: Option<u32>,
}
```

#### 1.3 Parser & EBNF

In `ebnf.rs` define `CBU_MODEL_EBNF` consistent with the S-expression shape above.

In `parser.rs` implement `CbuModelParser` using `nom`:

```rust
pub struct CbuModelParser;

impl CbuModelParser {
    pub fn parse_str(input: &str) -> Result<CbuModel, CbuModelError> {
        // parse (cbu-model ...) into CbuModel
    }
}
```

#### 1.4 Validation against dictionary

In `service.rs`:

```rust
pub struct CbuModelService {
    pub dictionary: DictionaryService,
}

impl CbuModelService {
    pub async fn validate_model(&self, model: &CbuModel) -> Result<(), CbuModelError> {
        // For every attribute in chunks + preconditions:
        //   - dictionary.resolve_attribute(attr_id)
        //   - ensure sink includes "CBU"

        Ok(())
    }

    pub fn get_chunk(&self, model: &CbuModel, name: &str) -> Option<&CbuAttributeChunk> { ... }

    pub fn find_transition_by_verb(&self, model: &CbuModel, verb: &str) -> Option<&CbuTransition> { ... }
}
```

#### 1.5 Model storage as document + DSL instance

When a new CBU model DSL is created/updated:

- Store text in `dsl_instances` (`domain_name = 'CBU-MODEL'`).
- Create a `document_catalog` entry of type `DSL.CBU.MODEL` via `DocumentService`.
- Add `document_metadata` entries like:
  - `KYC.DSL.MODEL_ID`, `KYC.DSL.VERSION`, `KYC.DSL.DOMAIN = "CBU"`.

---

### Phase 2 – CBU CRUD Template Generation (`DSL.CRUD.CBU.TEMPLATE`)

**Goal:** For each CBU model, generate one or more CRUD templates describing how to populate chunks in specific transitions.

#### 2.1 Template generator module

Add `rust/src/cbu_crud_template/{mod.rs, service.rs}`.

Example struct:

```rust
pub struct CbuCrudTemplate {
    pub id: String,
    pub model_id: String,
    pub transition_verb: String,
    pub content: String, // DSL text with placeholders
}
```

#### 2.2 Template generation from model

For each `CbuTransition` in a `CbuModel`:

- Collect all chunks referenced by `transition.chunks`.
- For each chunk:
  - Emit DSL keyword/value pairs for each attribute:
    - `:cbu-name` for `CBU.LEGAL_NAME`
    - `:jurisdiction` for `CBU.LEGAL_JURISDICTION`
    - etc., according to a mapping function.

Example mapping function:

```rust
fn map_attr_to_dsl_keyword(attr_id: &str) -> String {
    match attr_id {
        "CBU.LEGAL_NAME" => ":cbu-name".into(),
        "CBU.LEGAL_JURISDICTION" => ":jurisdiction".into(),
        "CBU.NATURE_PURPOSE" => ":nature-purpose".into(),
        "CBU.REGISTERED_ADDRESS" => ":registered-address".into(),
        // ...
        _ => format!(":{}", attr_id.replace('.', '-').to_lowercase()),
    }
}
```

Then assemble a template DSL like:

```lisp
(cbu.create
  :cbu-name "{{CBU.LEGAL_NAME}}"
  :jurisdiction "{{CBU.LEGAL_JURISDICTION}}"
  :nature-purpose "{{CBU.NATURE_PURPOSE}}"
  :registered-address "{{CBU.REGISTERED_ADDRESS}}"
  :primary-contact-email "{{CBU.PRIMARY_CONTACT_EMAIL}}")
```

#### 2.3 Store templates as documents

For each generated template:

- Insert DSL into `dsl_instances` (domain `CBU-CRUD-TEMPLATE`).
- Create `document_catalog` row with `type_code = 'DSL.CRUD.CBU.TEMPLATE'`.
- Link to model via `document_relationships` (e.g., relationship type `"MODEL_FOR"`).

---

### Phase 3 – CBU CRUD Instance Creation (`DSL.CRUD.CBU`)

**Goal:** Instantiate a template for a particular CBU onboarding request, generating a concrete CRUD sheet.

#### 3.1 Generator API

Add a function, e.g. in `cbu_crud_template::service`:

```rust
pub async fn instantiate_crud_from_template(
    template_doc_id: Uuid,
    model: &CbuModel,
    initial_values: HashMap<String, String>, // optional prefilled values
    dsl_repo: &DslRepository,
    doc_svc: &DocumentService,
) -> Result<(Uuid /*dsl_instance_id*/, Uuid /*doc_id*/)> {
    // 1. Load template DSL content
    // 2. Replace placeholders with initial_values where available
    // 3. Save as DSL instance (domain 'CBU-CRUD')
    // 4. Create document_catalog entry type 'DSL.CRUD.CBU'
    // 5. Link CRUD doc to template + model via document_relationships
    // 6. Return ids
}
```

#### 3.2 Attribute linkage

When you instantiate the CRUD doc, you already know:

- Which model (`CbuModel`)
- Which transition (`transition_verb`)
- Which chunks required

You can write `document_metadata`:

- `KYC.DSL.MODEL_ID` = model id
- `KYC.DSL.TRANSITION_VERB` = `"cbu.submit"`
- `KYC.DSL.CHUNKS` = `["core", "contact"]`

This metadata is later used to **understand which “chunk” a given value came from**.

---

### Phase 4 – Forth Execution of DSL.CRUD.CBU

**Goal:** Execute `DSL.CRUD.CBU` in Forth, producing CRUD IR only (no SQL).

#### 4.1 Forth verbs for CBU CRUD

In `forth_engine` (likely `kyc_vocab.rs` or equivalent vocab module), implement:

- `cbu.create`
- `cbu.update`
- `cbu.change-state`
- `cbu.attach-entity`
- `cbu.attach-proper-person`

Each word:

1. Pops keyword/value pairs from the stack.
2. Normalizes them into a `HashMap<String, Value>` where keys are **DSL field names** (e.g., `"cbu-name"`, `"jurisdiction"`).
3. Constructs `CrudStatement::DataCreate` or `DataUpdate` with:
   - `asset: "CBU"` or `"CBU_RELATIONSHIP"`
   - `values: HashMap<String, Value>`
4. Pushes `CrudStatement` into `RuntimeEnv.pending_crud`.

No DB calls. No direct knowledge of `cbus`/`attribute_values` table structure.

#### 4.2 Forth + RuntimeEnv

Ensure `RuntimeEnv` includes:

- `pending_crud: Vec<CrudStatement>`
- `cbu_id: Option<Uuid>`
- `cbu_model_id: Option<String>` (optional)
- `cbu_state: Option<String>`

Forth execution completes with a populated `RuntimeEnv` containing the IR.

---

### Phase 5 – CrudExecutor: Map DSL CRUD → DB Using CBU Model

**Goal:** Take `CrudStatement`s from `RuntimeEnv.pending_crud` and apply them to DB using services, dictionary, and CBU Model DSL.

#### 5.1 Execution orchestrator

After Forth execution (in `forth_engine::execute_with_database` or similar):

```rust
let (env, crud_statements) = forth_engine::execute_sheet_collect_crud(...)?;

// ExecutionContext includes CbuModel loaded from DSL.CBU.MODEL doc
let exec_ctx = ExecutionContext {
    cbu_model: Some(loaded_model),
    // other context: business_reference, case_id, etc.
};

for stmt in &crud_statements {
    crud_executor.execute_with_context(stmt, &exec_ctx).await?;
}
```

#### 5.2 CBU-specific mapping in CrudExecutor

In `CrudExecutor` implement:

```rust
pub async fn execute_create_cbu(
    &self,
    create: &DataCreate,
    ctx: &ExecutionContext,
) -> Result<CrudExecutionResult> {
    // 1. Look up CbuModel from ctx
    let model = ctx.cbu_model.as_ref().ok_or(...)?;

    // 2. Split DSL values into:
    //    - core CBU fields (for `cbus`)
    //    - attribute values (for `attribute_values`)
    let (cbu_fields, attr_values) = self.split_cbu_values(create, model)?;

    // 3. Create CBU row
    let cbu_id = self.cbu_service.create_cbu(&self.pool, &cbu_fields).await?;

    // 4. For each attribute value:
    //    - Use dictionary to confirm sink includes "CBU"
    //    - Upsert into attribute_values via AttributeValuesService
    //    - Include `source` json with DSL CRUD doc id + chunk info
    self.attribute_values_service
        .upsert_for_cbu_from_dsl_crud(cbu_id, attr_values, &ctx.dsl_doc_source())
        .await?;

    // 5. Return result (include cbu_id, etc.)
}
```

`split_cbu_values` uses:

- `CbuModel.chunks` to know which attribute ids belong to which chunk.
- A mapping function to map DSL field names to attribute ids (`map_attr_to_dsl_keyword` inverse).

---

### Phase 6 – Attribute Source Linkage for Chunks

**Goal:** Make it explicit which “chunk” and which **document** contributed each attribute value.

#### 6.1 Source payload in AttributeValuesService

Extend `AttributeValuesService` with a helper:

```rust
pub async fn upsert_for_cbu_from_dsl_crud(
    &self,
    cbu_id: Uuid,
    values: Vec<(AttributeId, JsonValue, String /* chunk_name */)>,
    dsl_doc_source: &DslDocSource, // contains dsl_instance_id, doc_id, model_id, template_id
) -> Result<()> {
    for (attr_id, value, chunk_name) in values {
        let source = json!({
            "type": "DSL.CRUD.CBU",
            "dsl_instance_id": dsl_doc_source.dsl_instance_id,
            "document_id": dsl_doc_source.document_id,
            "model_id": dsl_doc_source.model_id,
            "template_id": dsl_doc_source.template_id,
            "chunk": chunk_name,
        });

        self.upsert_for_cbu(
            cbu_id,
            dsl_doc_source.dsl_version,
            attr_id,
            value,
            "user-input", // or "compiled", depending on context
            source,
            Utc::now(),
        )
        .await?;
    }

    Ok(())
}
```

Now, `attribute_values.source` fully expresses:

- DSL CRUD document (which one)
- CBU model + template
- Which chunk of the CBU the attribute belongs to

#### 6.2 Later: Document extraction as another source

Once passport / KYC doc extraction is wired:

- A parallel helper like `upsert_for_cbu_from_document` uses:

```json
source = {
  "type": "DOCUMENT",
  "document_id": "...",
  "doc_type": "UK-PASSPORT",
  "extraction_pipeline": "v1.0"
}
```

and `state = "extracted"`.

Both flows (DSL CRUD vs documents) write into **the same attribute_values table**, but with different sources and states.

---

### Phase 7 – CRUD Logging

Ensure `CrudService::log_crud_operation`:

- For CBU CRUD executions:
  - Logs:
    - associated DSL CRUD document id
    - associated CBU id
    - attribute chunks touched
    - state transitions
- This gives you a fully explainable “why does this CBU look like this?” story, tracing back through DSL CRUD documents and any other source docs.

---

## 3. Summary & Usage

Once implemented, your pipeline looks like:

1. **Domain Architect** defines/updates CBU Model DSL (`DSL.CBU.MODEL`).  
2. **Template generator** produces CRUD templates (`DSL.CRUD.CBU.TEMPLATE`) per model/transition.  
3. For each onboarding case:
   - A **CRUD instance** (`DSL.CRUD.CBU`) is instantiated from a template.  
   - This doc describes concrete state changes for a particular CBU in business language.  
4. **Forth** executes `DSL.CRUD.CBU` → produces CRUD IR only.  
5. **CrudExecutor** uses:
   - CBU Model DSL (spec)
   - Dictionary (sink/source)
   - Services (CbuService, EntityService, AttributeValuesService, DocumentService)  
   to apply changes to DB via SQLx.  
6. **Attribute values** carry rich `source` metadata linking:
   - Back to the CRUD document  
   - Back to CBU chunks  
   - Later, back to real-world documents.

This is strictly more powerful and transparent than stored procedures:  
You get **spec as document**, **CRUD as document**, and **execution as code**, all tied by explicit source metadata and a canonical relational schema.

