# CBU DSL Document-Directed CRUD – Full E2E Test Harness (Option C)

This document is the **single source of instructions for Claude** to:

1. Apply a tiny remaining cleanup fix (no sneaky direct SQL in Forth/RuntimeEnv).  
2. Implement a **full CBU CRUD integration test**, exercising the entire pipeline:

   ```text
   CBU Model DSL (DSL.CBU.MODEL)
      → CBU CRUD Template (DSL.CRUD.CBU.TEMPLATE)
      → CBU CRUD Instance (DSL.CRUD.CBU)
      → Forth Engine (execute_sheet_into_env)
      → RuntimeEnv.pending_crud (CRUD IR)
      → CrudExecutor.execute_all
      → SQLx / PostgreSQL
   ```

3. Assert that **create / update / delete** all work via this flow.

We explicitly chose **Option C**:  
> *Use `execute_sheet_into_env` to run Forth and then call `CrudExecutor` manually in the test.*

---

## Part 0 – One Mandatory Cleanup Fix

### 0.1 Remove direct SQL from `RuntimeEnv` (`forth_engine/env.rs`)

**File:** `rust/src/forth_engine/env.rs`

Find and **delete** this method (if it still exists):

```rust
#[allow(dead_code)]
pub async fn load_attribute(&mut self, id: &AttributeId) -> Result<Option<Value>, sqlx::Error> {
    if let Some(pool) = &self.pool {
        let case_id = self.case_id.as_deref().unwrap_or("");

        let row = sqlx::query_as::<_, (String,)>(
            r#"
            SELECT attribute_value
            FROM "ob-poc".attribute_values
            WHERE attribute_id = $1::uuid AND entity_id = $2
            "#,
        )
        .bind(&id.0)
        .bind(case_id)
        .fetch_optional(pool)
        .await?;
        // ...
    }
}
```

**Why:**

- It uses the **old `attribute_values` schema** (`attribute_value`, `entity_id`).  
- It violates the architecture: **Forth/RuntimeEnv must not issue SQL**.  
- The correct way to load attributes is via `AttributeValuesService` and `CrudExecutor`, not from the env.

Do **not** replace this method; just **remove it**.

---

## Part 1 – DB Seeding for CBU CRUD Tests

Create the following seed SQL files under:

```text
sql/tests/seeds/
  cbu_dictionary.sql
  cbu_document_types.sql
  cbu_entity_types_and_roles.sql
```

### 1.1 `sql/tests/seeds/cbu_dictionary.sql`

Minimal dictionary entries for the CBU attributes used in the model and CRUD:

```sql
-- Minimal CBU attribute dictionary for tests

INSERT INTO "ob-poc".dictionary (attribute_id, name, sink, source)
VALUES
  ('CBU.LEGAL_NAME',           'Legal Name',           '{"sink": ["CBU"]}', '{}' ),
  ('CBU.LEGAL_JURISDICTION',   'Legal Jurisdiction',   '{"sink": ["CBU"]}', '{}' ),
  ('CBU.NATURE_PURPOSE',       'Nature Purpose',       '{"sink": ["CBU"]}', '{}' ),
  ('CBU.REGISTERED_ADDRESS',   'Registered Address',   '{"sink": ["CBU"]}', '{}' ),
  ('CBU.PRIMARY_CONTACT_EMAIL','Primary Contact Email','{"sink": ["CBU"]}', '{}' )
ON CONFLICT (attribute_id) DO NOTHING;
```

---

### 1.2 `sql/tests/seeds/cbu_document_types.sql`

Document types for CBU model, templates, and CRUD sheets:

```sql
-- Document types for CBU model + CRUD DSLs

INSERT INTO "ob-poc".document_types (type_id, type_code, display_name, category)
VALUES
  (gen_random_uuid(), 'DSL.CBU.MODEL',           'CBU Model DSL',          'DSL'),
  (gen_random_uuid(), 'DSL.CRUD.CBU.TEMPLATE',   'CBU CRUD Template DSL',  'DSL'),
  (gen_random_uuid(), 'DSL.CRUD.CBU',            'CBU CRUD Instance DSL',  'DSL')
ON CONFLICT (type_code) DO NOTHING;
```

---

### 1.3 `sql/tests/seeds/cbu_entity_types_and_roles.sql`

Entity types & roles needed for CBU CRUD:

```sql
-- Entity types (just enough for CBU CRUD tests)

INSERT INTO "ob-poc".entity_types (entity_type_id, type_name)
VALUES
  (gen_random_uuid(), 'PROPER_PERSON'),
  (gen_random_uuid(), 'COMPANY')
ON CONFLICT (type_name) DO NOTHING;

-- Roles (e.g. Beneficial Owner)

INSERT INTO "ob-poc".roles (role_id, role_name)
VALUES
  (gen_random_uuid(), 'BeneficialOwner')
ON CONFLICT (role_name) DO NOTHING;
```

---

## Part 2 – Test DSL Assets (Inline in Rust Test)

### 2.1 Test CBU Model DSL (`DSL.CBU.MODEL`)

We define this in the test as a `String`:

```rust
fn test_cbu_model_dsl() -> String {
    r#"(cbu-model
  :id "CBU.TEST.GENERIC"
  :version "1.0"
  :description "Test CBU model for integration harness"
  :applies-to ["Fund" "SPV" "CorporateClient"]

  (attributes
    (chunk "core"
      (required
        @attr("CBU.LEGAL_NAME")
        @attr("CBU.LEGAL_JURISDICTION")
        @attr("CBU.NATURE_PURPOSE"))
      (optional))

    (chunk "contact"
      (required
        @attr("CBU.REGISTERED_ADDRESS")
        @attr("CBU.PRIMARY_CONTACT_EMAIL"))
      (optional)))

  (states
    :initial "Proposed"
    :final ["Closed"]
    (state "Proposed"   :description "Draft CBU")
    (state "PendingKYC" :description "Awaiting KYC clearance")
    (state "Active"     :description "Live CBU")
    (state "Closed"     :description "Closed CBU"))

  (transitions
    (-> "Proposed"   "PendingKYC"
        :verb "cbu.submit"
        :chunks ["core" "contact"])
    (-> "PendingKYC" "Active"
        :verb "cbu.approve"
        :chunks ["core" "contact"])
    (-> "Active"     "Closed"
        :verb "cbu.close"
        :chunks []))

  (roles
    (role "BeneficialOwner" :min 1 :max 10))
)"#.to_string()
}
```

---

### 2.2 CBU CRUD DSL sheet (`DSL.CRUD.CBU`)

We generate the sheet in the test, so we can inject `cbu-id` once it’s known:

```rust
use uuid::Uuid;

fn build_cbu_crud_sheet(cbu_id: Option<Uuid>) -> String {
    let base = r#"
(cbu.create
  :cbu-name "ACME HOLDING LTD"
  :description "Holding company for ACME group"
  :nature-purpose "Equity investment holding"
  :jurisdiction "GB"
  :registered-address "123 Baker Street, London, UK"
  :primary-contact-email "ops@acme-holding.example")
"#;

    let mut sheet = base.to_string();

    if let Some(id) = cbu_id {
        let id_str = id.to_string();
        let updates = format!(
            r#"
(cbu.update
  :cbu-id "{id}"
  :nature-purpose "Equity holding and treasury services")

(cbu.update
  :cbu-id "{id}"
  :registered-address "456 Fleet Street, London, UK")

(cbu.delete
  :cbu-id "{id}"
  :reason "Test teardown")
"#,
            id = id_str,
        );
        sheet.push_str(&updates);
    }

    sheet
}
```

This gives us:

- A **create-only DSL** when `cbu_id` is `None`
- A **create + update + delete DSL** when `Some(cbu_id)` is passed.

---

## Part 3 – Public Forth Entry: `execute_sheet_into_env`

We chose **Option C**: drive execution using a variant that gives us back the **RuntimeEnv** (`pending_crud`) rather than going straight to the DB inside Forth.

### 3.1 Add a helper in `forth_engine/mod.rs` (if not present)

**File:** `rust/src/forth_engine/mod.rs`

Add a public helper that wraps your existing internal function (`execute_sheet_internal_with_env` or equivalent):

```rust
use crate::forth_engine::env::RuntimeEnv;
use crate::forth_engine::parser_nom::NomKycParser;
use crate::forth_engine::kyc_vocab::kyc_orch_vocab;
use crate::forth_engine::vm::VM;
use crate::dsl::DslSheet;
use std::sync::Arc;

use crate::database::DslRepository;
use crate::database::DictionaryDatabaseService;
use crate::cbu_model_dsl::CbuModelService;
use sqlx::PgPool;

/// Public helper that executes a DSL sheet and returns (ExecutionResult, RuntimeEnv)
/// without applying CRUD via CrudExecutor.
///
/// - Forth: parse + compile + run
/// - RuntimeEnv: contains pending_crud, cbu_model, etc.
#[cfg(feature = "database")]
pub fn execute_sheet_into_env(
    sheet: &DslSheet,
    pool: Option<PgPool>,
) -> Result<(ExecutionResult, RuntimeEnv), EngineError> {
    execute_sheet_internal_with_env(sheet, pool)
}
```

If `execute_sheet_internal_with_env` is not currently public/available, Claude should expose it appropriately.

This function must:

- Parse & execute the DSL using Forth
- Populate `RuntimeEnv.pending_crud` with `CrudStatement`s
- Return `(ExecutionResult, RuntimeEnv)`.

---

## Part 4 – Rust Integration Test: `rust/tests/cbu_document_crud_flow.rs`

Create:

```text
rust/tests/cbu_document_crud_flow.rs
```

and implement **this exact flow**:

- Seed DB
- Parse & validate CBU Model DSL
- Save as DSL + document
- Generate templates
- Save a template as DSL + document
- Instantiate CRUD sheet (instance)
- Run CRUD DSL through Forth via `execute_sheet_into_env`
- Extract `pending_crud` from `env`
- Call `CrudExecutor::execute_all` with those statements
- Assert DB state

### 4.1 Test skeleton

```rust
use std::collections::HashMap;

use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    cbu_model_dsl::{parser::CbuModelParser, service::CbuModelService},
    cbu_crud_template::service::CbuCrudTemplateService,
    database::{
        CrudExecutor, DatabaseConfig, DictionaryDatabaseService, DocumentService, DslRepository,
    },
    forth_engine::{DslSheet, execute_sheet_into_env},
};

async fn setup_test_pool() -> PgPool {
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set for integration tests");

    let config = DatabaseConfig {
        database_url,
        max_connections: 5,
        connection_timeout: std::time::Duration::from_secs(5),
        idle_timeout: None,
    };

    config.create_pool().await.expect("Failed to create PgPool")
}

async fn seed_db(pool: &PgPool) {
    // Optionally: sqlx::migrate!() if you use migrations
    // sqlx::migrate!("./sql/migrations").run(pool).await.unwrap();

    sqlx::query(include_str!("../sql/tests/seeds/cbu_dictionary.sql"))
        .execute(pool)
        .await
        .unwrap();

    sqlx::query(include_str!("../sql/tests/seeds/cbu_document_types.sql"))
        .execute(pool)
        .await
        .unwrap();

    sqlx::query(include_str!("../sql/tests/seeds/cbu_entity_types_and_roles.sql"))
        .execute(pool)
        .await
        .unwrap();
}

// Bring in the two helper fns from Part 2:
fn test_cbu_model_dsl() -> String { /* as above */ }
fn build_cbu_crud_sheet(cbu_id: Option<Uuid>) -> String { /* as above */ }

#[tokio::test]
async fn cbu_document_directed_crud_full_lifecycle_via_forth_and_crudexecutor() {
    let pool = setup_test_pool().await;
    seed_db(&pool).await;

    // 1. Parse and validate CBU model DSL
    let model_dsl = test_cbu_model_dsl();
    let parser = CbuModelParser;
    let model = parser.parse_str(&model_dsl).expect("Failed to parse CBU model DSL");

    let dict_service = DictionaryDatabaseService::new(pool.clone());
    let dsl_repo = DslRepository::new(pool.clone());
    let model_service = CbuModelService::new(pool.clone(), dict_service, dsl_repo.clone());

    model_service
        .validate_model(&model)
        .await
        .expect("CBU model validation failed");

    let (_model_instance_id, model_doc_id) = model_service
        .save_model_as_document(&model_dsl, &model)
        .await
        .expect("Failed to save CBU model as document");

    // 2. Generate CRUD templates from model and save one
    let template_service = CbuCrudTemplateService::new(pool.clone());
    let templates = template_service.generate_templates(&model);
    assert!(!templates.is_empty(), "Expected at least one template");
    let template = &templates[0];

    let (_template_instance_id, template_doc_id) = template_service
        .save_template_as_document(template)
        .await
        .expect("Failed to save CBU CRUD template");

    // 3. Instantiate CRUD sheet from template with initial values
    let mut initial_values = HashMap::new();
    initial_values.insert("CBU.LEGAL_NAME".into(), "ACME HOLDING LTD".into());
    initial_values.insert("CBU.LEGAL_JURISDICTION".into(), "GB".into());
    initial_values.insert(
        "CBU.NATURE_PURPOSE".into(),
        "Equity investment holding".into(),
    );
    initial_values.insert(
        "CBU.REGISTERED_ADDRESS".into(),
        "123 Baker Street, London, UK".into(),
    );
    initial_values.insert(
        "CBU.PRIMARY_CONTACT_EMAIL".into(),
        "ops@acme-holding.example".into(),
    );

    let (crud_instance_id, crud_doc_id) = template_service
        .instantiate_crud_from_template(template_doc_id, initial_values)
        .await
        .expect("Failed to instantiate CBU CRUD sheet");

    // 4. Execute CRUD sheet via Forth (create only, no cbu-id)
    let crud_sheet_initial = build_cbu_crud_sheet(None);
    let sheet_initial = DslSheet {
        id: crud_instance_id.to_string(),
        domain: "cbu".to_string(),
        version: "1.0".to_string(),
        content: crud_sheet_initial,
    };

    let (exec_result_initial, env_initial) =
        execute_sheet_into_env(&sheet_initial, Some(pool.clone()))
            .expect("Forth execution (initial) failed");

    assert!(
        exec_result_initial.success,
        "Initial Forth execution for CBU.create failed"
    );
    assert!(
        !env_initial.pending_crud.is_empty(),
        "Expected pending_crud statements after initial Forth execution"
    );

    // 5. Apply CRUD IR via CrudExecutor for initial create
    let crud_executor = CrudExecutor::new(pool.clone());
    let results_initial = crud_executor
        .execute_all(&env_initial.pending_crud)
        .await
        .expect("CrudExecutor initial execution failed");

    assert!(
        !results_initial.is_empty(),
        "Expected at least one CrudExecutionResult for initial create"
    );

    // 6. Look up created CBU via CbuService
    let cbu_service = crate::database::CbuService::new(pool.clone());
    let cbu = cbu_service
        .get_cbu_by_name("ACME HOLDING LTD")
        .await
        .expect("Failed to load CBU by name")
        .expect("CBU not found after create via Forth+CrudExecutor");
    let cbu_id = cbu.cbu_id;

    // 7. Execute update + delete via Forth + CrudExecutor using CBU ID
    let crud_sheet_updates = build_cbu_crud_sheet(Some(cbu_id));
    let sheet_updates = DslSheet {
        id: crud_instance_id.to_string(),
        domain: "cbu".to_string(),
        version: "1.0".to_string(),
        content: crud_sheet_updates,
    };

    let (exec_result_updates, env_updates) =
        execute_sheet_into_env(&sheet_updates, Some(pool.clone()))
            .expect("Forth execution (updates) failed");

    assert!(
        exec_result_updates.success,
        "Forth execution for update/delete failed"
    );
    assert!(
        !env_updates.pending_crud.is_empty(),
        "Expected pending_crud statements after updates"
    );

    let results_updates = crud_executor
        .execute_all(&env_updates.pending_crud)
        .await
        .expect("CrudExecutor update/delete execution failed");
    assert!(
        !results_updates.is_empty(),
        "Expected CrudExecutionResult for updates/delete"
    );

    // 8. Assertions on final DB state:
    //    a) CBU exists (or has expected 'closed' status if your schema supports it)
    let cbu_after = cbu_service
        .get_cbu_by_id(cbu_id)
        .await