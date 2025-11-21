# CBU DSL CRUD v1 – Code Cleanup Checklist for Claude
## (Pre-functional Cleanup Before Filling Data / Behaviour Gaps)

Repository snapshot: `cbu-document-directed-crud-v1`  
Context: This checklist is the **“clean house first”** pass before we start tightening functional behaviour and data flows (CBU model enforcement, attribute source linkage, doc-driven updates, etc.).

You MUST follow this in order, and you MUST NOT reintroduce direct SQL inside Forth or parsing layers.

---

## 0. Ground Rules

1. **DB schema is canonical.**  
   - Do NOT change tables or columns in this cleanup pass.
   - Only adjust Rust / DSL code to be consistent and clean.

2. **No new features.**  
   - Do NOT add new verbs, new IR variants, or new DB fields.
   - Focus on code organization, dead code removal, consistent patterns.

3. **No direct SQL in Forth or template logic.**  
   - After this cleanup, the only modules allowed to contain SQLx are:
     - `rust/src/database/*.rs` (domain services + repositories).
   - Forth engine and DSL modules must be free of SQLx.

4. **Don’t change public behaviour yet.**  
   - All functional and data-path changes (e.g. wiring DocumentService into CrudExecutor) will be done in a later “implementation” pass.

---

## 1. Clean Up `database/mod.rs` (DB Wiring & Comments)

**Files:**

- `rust/src/database/mod.rs`

### 1.1 Remove outdated/incorrect comments

- There is a legacy comment suggesting:

  > “Forth engine now handles database operations through RuntimeEnv with direct SQL queries…”

- This is **wrong** relative to current architecture (CrudExecutor + services).  
  **Action:**
  - Update module-level docs to say:

    - `CrudExecutor` and domain services own all DB interactions.
    - Forth/RuntimeEnv never call SQL directly.

### 1.2 Ensure all required services are declared

- `mod.rs` should:

  - `pub mod cbu_service;`
  - `pub mod entity_service;`
  - `pub mod cbu_entity_roles_service;`
  - `pub mod attribute_values_service;`
  - `pub mod document_service;`
  - `pub mod crud_service;`
  - `pub mod dsl_repository;`
  - (and any other domain services referenced by `CrudExecutor`)

**Action:**

- Audit `mod.rs` against actual `.rs` files in `rust/src/database`.
- Remove any `pub mod` entries pointing to non-existent or legacy modules.
- Add missing `pub mod` entries for real, current services.

**Do not** edit SQL in this step—just the module wiring and comments.

---

## 2. `cbu_model_dsl/*` – Finish Structure & Remove Stubs

**Files:**

- `rust/src/cbu_model_dsl/ast.rs`
- `rust/src/cbu_model_dsl/parser.rs`
- `rust/src/cbu_model_dsl/ebnf.rs`
- `rust/src/cbu_model_dsl/service.rs` (may be partial or missing)

### 2.1 Remove or complete placeholder methods

- Check `ast.rs` / `parser.rs` for:
  - TODO comments
  - placeholder “unimplemented!” / `unreachable!()`
  - partial parsing branches

**Action:**

- Replace any placeholder logic with either:
  - Working parser / AST helpers (if trivial), **or**
  - Clear `CbuModelError` returns with meaningful error messages (if not yet implemented).

The goal: no “mystery panics” or abandoned TODOs in this module.

### 2.2 Ensure `CbuModelService` exists and has a minimal, non-broken API

- `service.rs` should at minimum define:

  ```rust
  pub struct CbuModelService {
      pub dictionary: DictionaryService,
  }

  impl CbuModelService {
      pub async fn validate_model(&self, model: &CbuModel) -> Result<(), CbuModelError> { /* stub OK */ }

      pub fn get_chunk(&self, model: &CbuModel, name: &str) -> Option<&CbuAttributeChunk> { /* delegate to AST */ }

      pub fn find_transition_by_verb(&self, model: &CbuModel, verb: &str) -> Option<&CbuTransition> { /* delegate */ }
  }
  ```

For this **cleanup pass**, it’s acceptable if `validate_model` is a TODO internally, as long as:

- It compiles.
- It returns a `Result` (we’ll fill it in later).
- It doesn’t panic.

---

## 3. `cbu_crud_template/*` – Separate Template Logic from Raw SQL

**Files:**

- `rust/src/cbu_crud_template/mod.rs`
- `rust/src/cbu_crud_template/service.rs`

### 3.1 Extract direct SQL into Document/Dsl Repository services

Right now, `cbu_crud_template` is mixing:

- Business logic (template creation/instantiation), and
- Infrastructure (querying `document_catalog`, `document_metadata` via raw SQLx).

**Goal:** After cleanup, `cbu_crud_template` should NOT contain raw SQLx queries.

**Action:**

1. Identify all SQLx calls inside `cbu_crud_template/*`:
   - Reads from `"ob-poc".document_catalog`
   - Reads or writes JSON `metadata`
   - Any DSL instance creation via raw `INSERT`

2. For each such call:
   - Move the query into:
     - `DocumentService` **or**
     - `DslRepository`
   - Replace the raw SQL in `cbu_crud_template` with calls to those services.

Example transformation:

```rust
// BEFORE (inside cbu_crud_template)
let (template_name, metadata): (String, serde_json::Value) =
    sqlx::query_as(...).fetch_one(&self.pool).await?;

// AFTER
let template = self.document_service.get_dsl_crud_template(template_doc_id).await?;
let template_name = template.name;
let metadata = template.metadata;
```

For now, it’s fine if these new service methods are simple wrappers around existing SQL—they just need to centralise it.

### 3.2 Remove dead helper functions

- Scan `cbu_crud_template/service.rs` for functions not referenced anywhere:
  - e.g. experimental `generate_default_template_for_model` that is never called.
- **Action:** Either:
  - Remove them, **or**
  - Clearly mark them as `#[allow(dead_code)]` **and** add a short doc comment `/// reserved for future use (ISDA etc.)`.

No large orphan functions left hanging.

---

## 4. `database/crud_executor.rs` – Enforce “Orchestrator Only”

**File:**

- `rust/src/database/crud_executor.rs`

### 4.1 Remove or encapsulate any raw SQL

- Search `crud_executor.rs` for `sqlx::query` or any direct SQL usage.

**Action:**

- If you find any:
  - Move them into the appropriate service (`CbuService`, `EntityService`, `AttributeValuesService`, etc.).
  - Replace the direct calls with use of that service.

Example:

```rust
// BEFORE in CrudExecutor
let row = sqlx::query!(r#"SELECT state FROM "ob-poc".cbus WHERE cbu_id = $1"#, cbu_id)
    .fetch_one(&self.pool)
    .await?;

// AFTER:
let state = self.cbu_service.get_state(cbu_id).await?;
```

For cleanup, it’s enough to stub a minimal `get_state` method into `CbuService`.

### 4.2 Remove commented-out legacy code

- Look for:
  - Old `execute_*` variants commented out.
  - Leftover `println!` debug hacks, `dbg!` usage, etc.

**Action:**

- Delete commented-out branches that refer to:
  - Legacy DB access from Forth.
  - Old asset models not part of the master blueprint.

### 4.3 Keep the match skeleton clean

The main `execute` function should be:

```rust
match stmt {
    CrudStatement::DataCreate(create) => self.execute_create(create).await,
    CrudStatement::DataRead(read) => self.execute_read(read).await,
    CrudStatement::DataUpdate(update) => self.execute_update(update).await,
    CrudStatement::DataDelete(delete) => self.execute_delete(delete).await,
    _ => Err(anyhow!("Unsupported CRUD statement type")),
}
```

**Action:**

- Remove temporary `panic!`, `todo!` arms or random `_ => Ok(default)` branches left over from experimentation.

If something is truly not implemented yet, keep a clear `Unsupported` error message.

---

## 5. `database/crud_service.rs` – Ensure It’s Used Consistently

**File:**

- `rust/src/database/crud_service.rs`

### 5.1 No dead API surface

- Check for functions that are never called (e.g. `log_crud_operation_detailed`, `partial_log_*`).

**Action:**

- Remove them or fold them into a single `log_crud_operation` function that:
  - Accepts a single well-defined struct (e.g. `CrudLogPayload`).
  - Writes one row into `crud_operations`.

### 5.2 Ensure `CrudExecutor` actually logs

This is a small “peek into functionality”, but important for cleanup:

- Verify that `CrudExecutor::execute_*` actually calls `CrudService::log_crud_operation` everywhere a CRUD statement is processed.

If not:

- Add a TODO comment at call sites:

  ```rust
  // TODO: wire CrudService::log_crud_operation once payload shape is final
  ```

We’ll fill this in in the next (functional) pass.

---

## 6. `forth_engine/env.rs` – Confirm Shape & Remove Noise

**File:**

- `rust/src/forth_engine/env.rs`

### 6.1 Confirm all fields are used or clearly reserved

Fields likely include:

- `request_id`
- `pool` (when DB feature enabled)
- `cbu_id`, `entity_id`
- `cbu_model`
- `sink_attributes`, `source_attributes`
- `current_transition_verb`, `current_chunks`
- `pending_crud`

**Action:**

- For any field that is genuinely not used yet, add a short comment:

  ```rust
  /// reserved for model-driven validation in CBU CRUD v2
  pub current_transition_verb: Option<String>,
  ```

- Remove fields that are clearly obsolete and will never be used.

### 6.2 Remove direct-DB temptation

If `RuntimeEnv` has methods that directly call SQL (e.g. `fn load_cbu_from_db(&self, ...)`), which it shouldn’t:

- Move them into services and strip them from env.

After this pass, `RuntimeEnv` should only hold state and collections, not DB access.

---

## 7. Forth Vocab / DSL Glue – Remove Legacy SQL Hooks

**Files to inspect:**

- `rust/src/forth_engine/mod.rs`
- Any vocab modules (e.g. `kyc_vocab.rs`, `cbu_vocab.rs`, etc., if present in your full repo)

Even if they’re not fully visible in this tar, in your real repo you should:

### 7.1 Ensure no vocab function uses SQLx

- Search for `sqlx::` under `rust/src/forth_engine`.

**Action:**

- If any Forth word calls SQL:
  - Move that logic into `CrudExecutor` / services.
  - Replace with IR generation and `env.pending_crud.push(...)`.

### 7.2 Remove leftover “direct DB” comments

- Any comments suggesting:

  - “This word hits the DB”
  - “Temporary direct DB call”

…should be removed or updated to match the new pattern.

---

## 8. Dead Code Sweep: Repository-Wide

Run a **dead code sweep** (manual + clippy):

### 8.1 Manual pass

- Scan all new modules added for CBU work:
  - `cbu_model_dsl/*`
  - `cbu_crud_template/*`
  - Any new CBU-related services

Look for:

- Old prototype functions not referenced anymore.
- Half-refactored helpers that still point at old entity layouts.

**Action:**

- Either:
  - Delete them, **or**
  - Move them into a clearly labelled `scratch` or `experimental` module (not in the main path).

### 8.2 Clippy / compiler hints

Locally (outside this prompt), you should run:

```bash
cargo clippy --all-targets --all-features
```

Then:

- For warnings about `dead_code`, either:
  - Remove the symbol, **or**
  - Intentionally `#[allow(dead_code)]` with a precise doc comment.

For now, just focus on eliminating obvious dead, unused, or confusing bits.

---

## 9. Final Step for This Cleanup Pass: Compile and Stabilise

After performing the above:

1. Ensure the project **compiles** with features used in ob-poc:
   - `cargo build --all-features`
2. Ensure `sqlx` macros (if used) pass checks:
   - `cargo sqlx prepare` (if you’re using that workflow).
3. Do **not** worry yet if behaviour is incomplete (e.g., no document → attribute source wiring) — that’s for the next “implementation” pass.

The point of *this* document is to:

- Remove dead branches
- Centralise DB logic into services
- Ensure Forth & DSL layers are clean
- Make CrudExecutor the obvious single “DB bridge” entry point

…so that the **next functional / data gap pass** has solid ground to stand on.

