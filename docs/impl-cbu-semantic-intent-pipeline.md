# Implementation Spec: CBU CRUD via Semantic Intent Pipeline

## Overview

Re-implement CBU CRUD operations using the new semantic intent pipeline. The old `intent_pipeline` module has been removed. The new approach uses:

```
Agent JSON → Intent (validated) → Planner (deterministic) → DSL Source → Parser → Executor → DB
```

## Current State

### Working Components
- `dsl_v2/semantic_intent.rs` - Intent schema, validation, planning (7 tests passing)
- `dsl_v2/executor.rs` - DSL execution with `execute_dsl()` method
- `dsl_v2/custom_ops/mod.rs` - Custom operations (document.catalog, document.extract, entity.create)
- `database/cbu_service.rs` - Direct DB CRUD for CBUs
- `dsl_v2/verbs.rs` - Data-driven verb definitions including `cbu.create`, `cbu.assign-role`

### Test Harness
- `bin/test_semantic_intent.rs` - 6 tests passing, generates valid DSL

### Generated DSL Example (from test harness)
```clojure
(cbu.create :name "John Smith" :client-type "individual" :jurisdiction "UK" :as @cbu0)
(document.catalog :document-type "PASSPORT_GBR" :cbu-id @cbu0 :as @doc0)
(document.extract :document-id @doc0)
```

## Implementation Tasks

### Task 1: Create Integration Test Harness

Create `rust/src/bin/test_cbu_crud_pipeline.rs`:

```rust
//! CBU CRUD via Semantic Intent Pipeline - Integration Test
//!
//! Tests the FULL flow: JSON Intent → DSL → Executor → Database
//!
//! Run with: cargo run --bin test_cbu_crud_pipeline --features database
//!
//! Requires: DATABASE_URL environment variable

use ob_poc::dsl_v2::semantic_intent::{run_pipeline, IntentVocabulary};
use ob_poc::dsl_v2::executor::{DslExecutor, ExecutionContext};
use ob_poc::database::CbuService;
use sqlx::PgPool;

#[tokio::main]
async fn main() {
    // 1. Connect to DB
    // 2. For each test case:
    //    a. Create JSON intent
    //    b. Run pipeline to get DSL
    //    c. Execute DSL against DB
    //    d. Verify DB state using CbuService
    //    e. Clean up test data
}
```

**Test Cases:**
1. Create individual CBU with passport
2. Create corporate CBU with entities (director, UBO, signatory)  
3. Add document to existing CBU
4. Add entity role to existing CBU
5. Verify bindings resolve correctly (@cbu0, @doc0, @ent0)

### Task 2: Wire Executor to Pipeline

The pipeline currently stops at parse verification. Extend to execute:

In `semantic_intent.rs`, add or modify `run_pipeline()` to optionally execute:

```rust
pub struct PipelineResult {
    pub intent: KycIntent,
    pub validation_errors: Vec<ValidationError>,
    pub dsl_plan: Option<DslPlan>,
    pub parse_result: Option<ParseResult>,
    pub execution_results: Option<Vec<ExecutionResult>>, // NEW
}

// Add execution variant
pub async fn run_pipeline_with_execution(
    json: &str,
    vocab: &IntentVocabulary,
    executor: &DslExecutor,
) -> Result<PipelineResult, String> {
    // ... existing validation and planning ...
    
    // Execute DSL
    let mut ctx = ExecutionContext::new();
    let results = executor.execute_dsl(&plan.dsl_source, &mut ctx).await?;
    
    // Return context.symbols for binding verification
}
```

### Task 3: Verify Verb Compatibility

Check that generated DSL verbs exist in `verbs.rs` or `custom_ops`:

| Generated Verb | Location | Status |
|----------------|----------|--------|
| `cbu.create` | verbs.rs | ✅ Exists |
| `cbu.assign-role` | verbs.rs | ✅ Exists |
| `document.catalog` | custom_ops | ✅ Exists |
| `document.extract` | custom_ops | ✅ Exists |
| `document.link-entity` | verbs.rs | ✅ Exists |
| `entity.create` | custom_ops | ✅ Just added |

### Task 4: Fix Argument Mapping

The planner generates `:client-type` but `cbu.create` verb expects specific args. Verify mapping in `verbs.rs`:

```rust
// In VERB_DEFS for cbu.create
VerbDef {
    domain: "cbu",
    verb: "create",
    behavior: Behavior::Insert { table: "cbus" },
    required_args: &["name"],
    optional_args: &["description", "nature-purpose", "source-of-funds", "client-type", "jurisdiction"],
    ...
}
```

Check `mappings.rs` for column mappings:
- `:client-type` → `client_type`
- `:jurisdiction` → `jurisdiction`

### Task 5: Handle :as Binding in Executor

The executor must capture the returned UUID when `:as @binding` is present:

```rust
// In executor.rs execute_verb()
if let Some(binding) = verb_call.get_as_binding() {
    if let ExecutionResult::Uuid(id) = &result {
        ctx.bind(&binding, *id);
    }
}
```

Verify this works for:
- `cbu.create ... :as @cbu0` → binds CBU UUID
- `document.catalog ... :as @doc0` → binds document UUID
- `entity.create ... :as @ent0` → binds entity UUID

## File Locations

| File | Purpose |
|------|---------|
| `rust/src/dsl_v2/semantic_intent.rs` | Intent schema, validation, planning |
| `rust/src/dsl_v2/executor.rs` | DSL execution engine |
| `rust/src/dsl_v2/verbs.rs` | Data-driven verb definitions |
| `rust/src/dsl_v2/custom_ops/mod.rs` | Custom operations |
| `rust/src/dsl_v2/mappings.rs` | DSL arg → DB column mappings |
| `rust/src/database/cbu_service.rs` | Direct CBU database operations |
| `rust/src/bin/test_semantic_intent.rs` | Existing test harness (DSL generation only) |

## Success Criteria

1. `cargo run --bin test_cbu_crud_pipeline --features database` passes
2. CBU created in database with correct fields
3. Entities created and linked via `cbu_entity_roles` junction
4. Documents cataloged with correct `cbu_id` FK
5. All bindings (@cbu0, @doc0, @ent0) resolve correctly
6. Test cleans up after itself (delete test data)

## Notes

- The old `intent_pipeline` module was removed - do not reference it
- The old test files in `tests/` (cbu_document_crud_flow.rs, etc.) reference removed modules - ignore them
- Use `semantic_intent.rs` as the source of truth for the new approach
- Entity create uses the new `entity.create` custom op with `:type` parameter
- `cbu.assign-role` is the correct verb for linking entities to CBUs (not `cbu.link-entity`)

## Testing Command

```bash
# Run unit tests first
cargo test --features database --lib dsl_v2::semantic_intent

# Then integration test
DATABASE_URL=postgres://... cargo run --bin test_cbu_crud_pipeline --features database
```
