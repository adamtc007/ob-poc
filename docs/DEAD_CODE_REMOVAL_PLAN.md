# Dead Code Removal: Legacy DSL Static Definitions

**Document for Claude Code**  
**Goal**: Remove ~3,300 lines of static Rust code now replaced by YAML-driven configuration

---

## Executive Summary

The YAML-driven DSL infrastructure is complete and working:
- `config/verbs.yaml` defines all verbs
- `RuntimeVerbRegistry` loads verbs at runtime
- `GenericCrudExecutor` executes all 13 CRUD operations
- `execute_verb_generic()` provides the new execution path

The old static definitions are now **dead code** that must be removed to:
1. Prevent confusion (two sources of truth)
2. Avoid hallucinations (AI reading stale code)
3. Reduce maintenance burden
4. Enforce the YAML-first architecture

---

## Phase 1: Identify All Dead Code

### 1.1 Files to DELETE Entirely

| File | Lines | Reason |
|------|-------|--------|
| `rust/src/dsl_v2/verbs.rs` | 1,299 | `STANDARD_VERBS` array replaced by `verbs.yaml` |
| `rust/src/dsl_v2/mappings.rs` | 1,424 | Column mappings replaced by `maps_to` in YAML |

### 1.2 Functions to REMOVE from `executor.rs`

These execute_* methods are replaced by `GenericCrudExecutor`:

| Function | Approx Lines | Replacement |
|----------|--------------|-------------|
| `execute_insert()` | 50 | `GenericCrudExecutor::execute_insert()` |
| `execute_select()` | 60 | `GenericCrudExecutor::execute_select()` |
| `execute_update()` | 50 | `GenericCrudExecutor::execute_update()` |
| `execute_delete()` | 20 | `GenericCrudExecutor::execute_delete()` |
| `execute_upsert()` | 60 | `GenericCrudExecutor::execute_upsert()` |
| `execute_link()` | 60 | `GenericCrudExecutor::execute_link()` |
| `execute_unlink()` | 25 | `GenericCrudExecutor::execute_unlink()` |
| `execute_role_link()` | 50 | `GenericCrudExecutor::execute_role_link()` |
| `execute_role_unlink()` | 40 | `GenericCrudExecutor::execute_role_unlink()` |
| `execute_list_by_fk()` | 20 | `GenericCrudExecutor::execute_list_by_fk()` |
| `execute_list_parties()` | 40 | `GenericCrudExecutor::execute_list_parties()` |
| `execute_select_with_join()` | 30 | `GenericCrudExecutor::execute_select_with_join()` |
| `execute_entity_create()` | 80 | `GenericCrudExecutor::execute_entity_create()` |
| **Total** | **~585** | |

Also remove from `executor.rs`:
- `bind_value_to_query()` - duplicated in generic_executor
- `bind_value_to_query_regular()` - duplicated in generic_executor
- `row_to_json()` - duplicated in generic_executor (keep ONE copy)
- `BindValue` enum - duplicated in generic_executor as `SqlValue`
- `qualified_table()` helper - no longer needed
- `SCHEMA` constant - schema now in YAML config

### 1.3 Code to REMOVE from `verb_registry.rs`

| Item | Reason |
|------|--------|
| `use super::verbs::{Behavior, VerbDef, STANDARD_VERBS}` | verbs.rs being deleted |
| CRUD verb loading loop in `build()` | Now loads from RuntimeVerbRegistry |
| `infer_arg_type()` function | Arg types explicit in YAML |
| `crud_def: Option<&'static VerbDef>` field | VerbDef being deleted |
| `crud_behavior: Option<Behavior>` field | Behavior enum being deleted |

### 1.4 Code to REMOVE from `mod.rs`

```rust
// REMOVE these lines:
pub mod mappings;
pub mod verbs;

// REMOVE these re-exports:
pub use mappings::{get_table_mappings, resolve_column, ColumnMapping, DbType, TableMappings};
pub use verbs::{
    domains, find_verb, verb_count, verbs_for_domain, Behavior, VerbDef, STANDARD_VERBS,
};
```

### 1.5 Update Imports in Other Files

Search for and update any file importing from deleted modules:

```bash
# Find all files importing from verbs.rs or mappings.rs
grep -r "use.*verbs::" rust/src/
grep -r "use.*mappings::" rust/src/
grep -r "find_verb" rust/src/
grep -r "STANDARD_VERBS" rust/src/
grep -r "resolve_column" rust/src/
grep -r "get_pk_column" rust/src/
grep -r "get_table_mappings" rust/src/
```

Known files that import these:
- `executor.rs` - remove imports, rewire to use generic_executor
- `verb_registry.rs` - remove imports, load from RuntimeVerbRegistry
- `semantic_validator.rs` - may need updates if it uses mappings
- `csg_linter.rs` - check for verb lookups

---

## Phase 2: Rewire `execute_verb()` to Use Generic Executor

### 2.1 Current Flow (Legacy)

```rust
// executor.rs - current execute_verb()
pub async fn execute_verb(&self, vc: &VerbCall, ctx: &mut ExecutionContext) -> Result<ExecutionResult> {
    // Check unified registry for custom ops
    let unified_verb = registry().get(&vc.domain, &vc.verb);
    if let Some(uv) = unified_verb {
        if uv.behavior == VerbBehavior::CustomOp {
            // Route to custom_ops
            return self.custom_ops.get(&vc.domain, &vc.verb)?.execute(...);
        }
    }
    
    // Look up static verb definition (FROM verbs.rs - BEING DELETED)
    let verb_def = find_verb(&vc.domain, &vc.verb)?;
    
    // Match on Behavior enum (FROM verbs.rs - BEING DELETED)
    match &verb_def.behavior {
        Behavior::Insert { table } => self.execute_insert(...),
        Behavior::Select { table } => self.execute_select(...),
        // ... etc
    }
}
```

### 2.2 New Flow (YAML-Driven)

```rust
// executor.rs - new execute_verb()
pub async fn execute_verb(&self, vc: &VerbCall, ctx: &mut ExecutionContext) -> Result<ExecutionResult> {
    // Look up verb in RuntimeVerbRegistry (loaded from YAML)
    let runtime_verb = self.runtime_registry.get(&vc.domain, &vc.verb)
        .ok_or_else(|| anyhow!("Unknown verb: {}.{}", vc.domain, vc.verb))?;
    
    // Check if plugin (custom op)
    if let RuntimeBehavior::Plugin(handler) = &runtime_verb.behavior {
        if let Some(op) = self.custom_ops.get(&vc.domain, &vc.verb) {
            return op.execute(vc, ctx, &self.pool).await;
        }
        return Err(anyhow!("Plugin {} has no handler", handler));
    }
    
    // Convert VerbCall args to HashMap<String, JsonValue>
    let resolved_args = self.resolve_args_to_json(&vc.arguments, ctx)?;
    
    // Execute via GenericCrudExecutor
    self.generic_executor.execute(runtime_verb, &resolved_args).await
        .map(|r| r.to_legacy())
}
```

### 2.3 New Method: `resolve_args_to_json()`

```rust
/// Convert AST arguments to JSON values for generic executor
fn resolve_args_to_json(
    &self,
    args: &[Argument],
    ctx: &ExecutionContext,
) -> Result<HashMap<String, JsonValue>> {
    let mut result = HashMap::new();
    
    for arg in args {
        let key = arg.key.canonical();
        let value = self.ast_value_to_json(&arg.value, ctx)?;
        result.insert(key, value);
    }
    
    Ok(result)
}

/// Convert AST Value to serde_json::Value
fn ast_value_to_json(&self, value: &Value, ctx: &ExecutionContext) -> Result<JsonValue> {
    match value {
        Value::String(s) => Ok(json!(s)),
        Value::Integer(i) => Ok(json!(i)),
        Value::Decimal(d) => Ok(json!(d.to_string())),
        Value::Boolean(b) => Ok(json!(b)),
        Value::Null => Ok(JsonValue::Null),
        Value::Reference(name) => {
            let uuid = ctx.resolve(name)
                .ok_or_else(|| anyhow!("Unresolved reference: @{}", name))?;
            Ok(json!(uuid.to_string()))
        }
        Value::AttributeRef(uuid) => Ok(json!(uuid.to_string())),
        Value::DocumentRef(uuid) => Ok(json!(uuid.to_string())),
        Value::List(items) => {
            let arr: Result<Vec<JsonValue>> = items.iter()
                .map(|v| self.ast_value_to_json(v, ctx))
                .collect();
            Ok(JsonValue::Array(arr?))
        }
        Value::Map(map) => {
            let obj: Result<serde_json::Map<String, JsonValue>> = map.iter()
                .map(|(k, v)| Ok((k.clone(), self.ast_value_to_json(v, ctx)?)))
                .collect();
            Ok(JsonValue::Object(obj?))
        }
        Value::NestedCall(_) => {
            bail!("NestedCall should be compiled, not resolved at runtime")
        }
    }
}
```

### 2.4 Update `DslExecutor` Struct

```rust
pub struct DslExecutor {
    #[cfg(feature = "database")]
    pool: PgPool,
    #[cfg(feature = "database")]
    custom_ops: CustomOperationRegistry,
    #[cfg(feature = "database")]
    generic_executor: GenericCrudExecutor,
    #[cfg(feature = "database")]
    runtime_registry: Arc<RuntimeVerbRegistry>,  // ADD THIS
}

impl DslExecutor {
    #[cfg(feature = "database")]
    pub fn new(pool: PgPool, runtime_registry: Arc<RuntimeVerbRegistry>) -> Self {
        Self {
            generic_executor: GenericCrudExecutor::new(pool.clone()),
            pool,
            custom_ops: CustomOperationRegistry::new(),
            runtime_registry,  // ADD THIS
        }
    }
}
```

---

## Phase 3: Update Dependent Code

### 3.1 `verb_registry.rs` Changes

The `UnifiedVerbRegistry` should now delegate to `RuntimeVerbRegistry`:

```rust
// BEFORE: Loads from static STANDARD_VERBS
fn build() -> Self {
    for crud_verb in STANDARD_VERBS.iter() {
        // ... build from static array
    }
}

// AFTER: Loads from RuntimeVerbRegistry
fn build(runtime_registry: &RuntimeVerbRegistry) -> Self {
    let mut verbs = HashMap::new();
    
    for verb in runtime_registry.all_verbs() {
        let unified = UnifiedVerbDef {
            domain: verb.domain.clone(),
            verb: verb.verb.clone(),
            description: verb.description.clone(),
            args: verb.args.iter().map(|a| ArgDef {
                name: a.name.clone(),
                arg_type: format!("{:?}", a.arg_type),
                required: a.required,
                description: String::new(),
            }).collect(),
            behavior: match &verb.behavior {
                RuntimeBehavior::Crud(_) => VerbBehavior::Crud,
                RuntimeBehavior::Plugin(_) => VerbBehavior::CustomOp,
            },
            custom_op_id: match &verb.behavior {
                RuntimeBehavior::Plugin(h) => Some(h.clone()),
                _ => None,
            },
        };
        verbs.insert(verb.full_name.clone(), unified);
    }
    
    // ... rest of build
}
```

### 3.2 `semantic_validator.rs` Changes

Check if it uses:
- `resolve_column()` from mappings.rs → Remove or inline
- `find_verb()` from verbs.rs → Use RuntimeVerbRegistry
- `VerbDef` type → Use RuntimeVerb

### 3.3 `csg_linter.rs` Changes

Check if it references:
- Verb lookups → Use RuntimeVerbRegistry
- Column mappings → Use RuntimeVerb.args[].maps_to

### 3.4 Update All `DslExecutor::new()` Call Sites

Every place that creates a `DslExecutor` needs to pass the registry:

```rust
// BEFORE
let executor = DslExecutor::new(pool);

// AFTER
let config = ConfigLoader::from_env().load_verbs()?;
let registry = Arc::new(RuntimeVerbRegistry::from_config(&config));
let executor = DslExecutor::new(pool, registry);
```

Search for call sites:
```bash
grep -r "DslExecutor::new" rust/src/
```

---

## Phase 4: Execution Checklist

### Step 1: Preparation
- [ ] Create a new branch: `git checkout -b remove-legacy-dsl-static`
- [ ] Run `cargo test` - all 137 tests should pass
- [ ] Run `cargo clippy` - should be clean

### Step 2: Delete Files
- [ ] `rm rust/src/dsl_v2/verbs.rs`
- [ ] `rm rust/src/dsl_v2/mappings.rs`
- [ ] `cargo build` - will fail with missing imports

### Step 3: Fix `mod.rs`
- [ ] Remove `pub mod verbs;`
- [ ] Remove `pub mod mappings;`
- [ ] Remove re-exports for deleted types
- [ ] `cargo build` - will fail with missing types in other files

### Step 4: Fix `verb_registry.rs`
- [ ] Remove `use super::verbs::{...}`
- [ ] Remove `crud_def` and `crud_behavior` fields from `UnifiedVerbDef`
- [ ] Update `build()` to take `&RuntimeVerbRegistry`
- [ ] Remove `infer_arg_type()` function
- [ ] Update global registry initialization
- [ ] `cargo build` - will fail with executor issues

### Step 5: Fix `executor.rs`
- [ ] Remove `use super::verbs::{find_verb, Behavior, VerbDef}`
- [ ] Remove `use super::mappings::{...}`
- [ ] Remove `SCHEMA` constant
- [ ] Remove `qualified_table()` function
- [ ] Remove all `execute_*` methods (13 total)
- [ ] Remove `BindValue` enum (use SqlValue from generic_executor)
- [ ] Remove `bind_value_to_query*` functions
- [ ] Remove duplicate `row_to_json()` (keep one in generic_executor)
- [ ] Add `runtime_registry: Arc<RuntimeVerbRegistry>` field
- [ ] Update `new()` to accept registry parameter
- [ ] Add `resolve_args_to_json()` method
- [ ] Add `ast_value_to_json()` method
- [ ] Rewrite `execute_verb()` to use generic executor
- [ ] `cargo build` - will fail at call sites

### Step 6: Fix Call Sites
- [ ] Search: `grep -r "DslExecutor::new" rust/src/`
- [ ] Update each call site to pass RuntimeVerbRegistry
- [ ] `cargo build` - should pass now

### Step 7: Fix Any Remaining Issues
- [ ] `semantic_validator.rs` - fix any imports
- [ ] `csg_linter.rs` - fix any imports
- [ ] Any other files found by compiler errors

### Step 8: Verify
- [ ] `cargo build` - passes
- [ ] `cargo clippy` - clean
- [ ] `cargo test` - all 137 tests pass
- [ ] Manual smoke test: run a DSL command through MCP

### Step 9: Commit
- [ ] `git add -A`
- [ ] `git commit -m "Remove legacy static DSL definitions (~3,300 lines)"`
- [ ] Update documentation if needed

---

## Phase 5: What to KEEP

### Keep in `executor.rs`
- `ExecutionContext` struct
- `ExecutionResult` enum
- `ReturnType` enum
- `ResolvedValue` enum (still used for internal resolution)
- `resolve_args()` method (convert to JSON variant)
- `resolve_value()` method (convert to JSON variant)
- `validate_args()` method (update to use RuntimeVerb)
- `execute_plan()` method
- `execute_dsl()` method
- `execute_verb_generic()` method (becomes the main path)

### Keep in `verb_registry.rs`
- `VerbBehavior` enum (Crud, CustomOp, Composite)
- `ArgDef` struct
- `UnifiedVerbDef` struct (simplified)
- `UnifiedVerbRegistry` struct (loads from RuntimeVerbRegistry)
- `registry()` global accessor

### Keep in `generic_executor.rs`
- Everything - this is the new implementation

### Keep in `runtime_registry.rs`
- Everything - this is the YAML loader

### Keep in `config/`
- Everything - this is the source of truth

---

## Summary

### Lines Removed
| Source | Lines |
|--------|-------|
| `verbs.rs` (deleted) | 1,299 |
| `mappings.rs` (deleted) | 1,424 |
| `executor.rs` (methods) | ~585 |
| `verb_registry.rs` (cleanup) | ~100 |
| **Total** | **~3,400** |

### Lines Added
| Source | Lines |
|--------|-------|
| `executor.rs` (new methods) | ~60 |
| `verb_registry.rs` (new loader) | ~30 |
| **Total** | **~90** |

### Net Change
**~3,300 lines removed** from Rust codebase

### Result
- Single source of truth: `config/verbs.yaml`
- No static arrays to maintain
- No duplicate type definitions
- Clean separation: YAML for config, Rust for execution
