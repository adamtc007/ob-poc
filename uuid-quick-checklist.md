# UUID Integration Checklist - ob-poc

**Critical Issue:** UUID parser exists but AttributeService doesn't use the resolver  
**Result:** `@attr{uuid}` won't work until fixed

## ⚡ Quick Fix (Do This First!)

### File: `rust/src/services/attribute_service.rs`

1. **Add import:**
```rust
use crate::domains::attributes::resolver::{AttributeResolver, ResolutionError};
```

2. **Add field to struct (line ~24):**
```rust
resolver: AttributeResolver,  // ADD THIS
```

3. **Initialize in constructor (line ~57):**
```rust
resolver: AttributeResolver::new(),  // ADD THIS
```

4. **Fix extract_attr_ref method (line ~301):**
```rust
Value::AttrUuid(uuid) => {
    // REPLACE format!("uuid:{}", uuid) WITH:
    match self.resolver.uuid_to_semantic(uuid) {
        Ok(semantic_id) => Some(semantic_id),
        Err(e) => {
            log::error!("Failed to resolve UUID {}: {}", uuid, e);
            None
        }
    }
}
```

5. **Add to error enum:**
```rust
#[error("UUID resolution error: {0}")]
Resolution(String),
```

**Test:** `cargo test --lib attribute_service`

---

## Then Complete These Tasks

### Task 1: Source Executors
- [ ] Create `rust/src/domains/attributes/sources/mod.rs`
- [ ] Create `rust/src/domains/attributes/sources/default.rs`
- [ ] Create `rust/src/domains/attributes/sources/document_extraction.rs`
- [ ] Add `pub mod sources;` to `rust/src/domains/attributes/mod.rs`

### Task 2: Value Binder
- [ ] Create `rust/src/execution/value_binder.rs`
- [ ] Add `pub mod value_binder;` to `rust/src/execution/mod.rs`

### Task 3: DSL Executor
- [ ] Create/update `rust/src/execution/dsl_executor.rs`
- [ ] Implement UUID extraction from AST
- [ ] Wire up value binding

### Task 4: Testing
- [ ] Create `rust/tests/uuid_e2e_test.rs`
- [ ] Run: `cargo test uuid_e2e_test --features database`

---

## Verification

```bash
# After fixing AttributeService
cargo build --features database
cargo test --lib

# After all tasks
cargo test --features database
```

**Success:** When `@attr{3020d46f-472c-5437-9647-1b0682c35935}` resolves to `"attr.identity.first_name"` and fetches/stores values.
