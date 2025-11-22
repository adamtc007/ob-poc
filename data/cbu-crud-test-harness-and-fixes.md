# CBU DSL Document-Directed CRUD – Test Harness & Fixes for Claude

This file is the **single source of instructions for Claude** to:

1. Apply the last small cleanup fixes you asked for (no crevices).
2. Implement a **full CBU CRUD integration test harness** driven by a **CBU DSL document**:
   - `DSL.CBU.MODEL` → `DSL.CRUD.CBU.TEMPLATE` → `DSL.CRUD.CBU`
   - Forth → CrudExecutor → SQLx DB.

You can say in Zed:

> “Implement everything in `rust/docs/cbu-crud-test-harness-and-fixes.md` exactly.”

---

## Part 0 – Tiny Mandatory Fixes Before Tests

These are the last bits of “clean house” required so the harness doesn’t accidentally use old paths.

### 0.1 Remove direct SQL from `RuntimeEnv` (`forth_engine/env.rs`)

**File:** `rust/src/forth_engine/env.rs`

Find and **delete** the method:

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

- It uses the **old attribute_values schema** (`attribute_value`, `entity_id`).
- It violates the architecture: **no SQL in Forth/RuntimeEnv**.
- It’s marked `dead_code` – better to delete than to risk reuse.

No replacement needed in this step: attribute loading will come via `AttributeValuesService` and `CrudExecutor` when truly needed.

---

### 0.2 (Optional Strictness) Keep `CbuModelService` as domain service

**File:** `rust/src/cbu_model_dsl/service.rs`

This module uses `sqlx::query*` to:

- Check dictionary entries,
- Persist model as DSL/document.

For the test harness, this is acceptable (it’s acting as a domain service).  
You **do not** need to move this SQL into `database/` right now unless you want maximal purity.

If you want to be strict later, we can refactor this into:

- `DictionaryDatabaseService`
- `DslRepository`
- `DocumentService`

For now, **do not change it** unless absolutely necessary.

---

## Part 1 – DB Seeding for CBU CRUD Harness

Create seed SQL files under:

```text
sql/tests/seeds/
  cbu_dictionary.sql
  cbu_document_types.sql
  cbu_entity_types_and_roles.sql
```

### 1.1 `sql/tests/seeds/cbu_dictionary.sql`

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

We define the test model inline as a `&str` in the test:

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

We’ll generate it dynamically in the test, so we can inject the concrete `cbu-id` we get from the create operation:

```rust
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
            id = id_str
        );
        sheet.push_str(&updates);
    }

    sheet
}
```

---

## Part 3 – Rust Integration Test: `rust/tests/cbu_document_crud_flow.rs`

Create:

```text
rust/tests/cbu_document_crud_flow.rs
```

And populate it with the test implementation from the previous answer (Claude should paste the full code, adjusting module paths to the actual crate name and existing APIs).

---

## Part 4 – What You Tell Claude

Once this file is present in your repo at:

```text
rust/docs/cbu-crud-test-harness-and-fixes.md
```

You can say:

> “Implement everything in `rust/docs/cbu-crud-test-harness-and-fixes.md` exactly:
>  - Apply the cleanup change in Part 0
>  - Create the SQL seed files in Part 1
>  - Add the test file in Part 3  
>  Make sure the test compiles and passes against a local test database.”

That’s it — this document is the **one file** that instructs Claude to implement the CBU DSL document-driven CRUD test harness, plus the tiny cleanup fix I flagged.

