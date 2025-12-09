# Opus TODO Integration Review

This document reviews the Opus 4.5 generated `TODO_dsl_repl_dataflow.md` against the current codebase to identify:
1. What already exists
2. What needs to be added
3. Integration points and potential conflicts
4. Recommended changes to the Opus TODO

---

## Executive Summary

The Opus TODO is **well-aligned** with the codebase architecture. Key findings:

| Area | Status | Notes |
|------|--------|-------|
| Phase 1: verbs.yaml extension | **Ready** | Clean integration point |
| Phase 2: BindingContext | **New module needed** | No conflicts |
| Phase 3: Dataflow validation | **Extends existing** | Add to `csg_linter.rs` |
| Phase 4: Storage | **New tables** | Clean addition |
| Phase 5: REPL | **New binary** | Uses existing executor |
| Phase 6: LSP | **Extends existing** | Need to update context.rs |
| Phase 7: Agent | **Replaces hardcoded** | Major cleanup |
| Phase 8: Testing | **New tests** | Uses existing test patterns |

**No fundamental disconnects found.** Minor adjustments documented below.

---

## Phase 1: Verb Registry Extension

### Current State

**File:** `rust/src/dsl_v2/config/types.rs`

Current `VerbConfig`:
```rust
pub struct VerbConfig {
    pub description: String,
    pub behavior: VerbBehavior,
    pub crud: Option<CrudConfig>,
    pub handler: Option<String>,
    pub args: Vec<ArgConfig>,
    pub returns: Option<ReturnsConfig>,
}
```

Current `ArgConfig` already has:
```rust
pub struct ArgConfig {
    pub name: String,
    pub arg_type: ArgType,
    pub required: bool,
    pub maps_to: Option<String>,
    pub lookup: Option<LookupConfig>,  // ← Already has entity_type!
    // ...
}
```

### Integration Notes

1. **`LookupConfig.entity_type` already exists** - This is the `ref_type` Opus proposes. We can use this directly.

2. **`produces`/`consumes` need to be added** to `VerbConfig`:
   ```rust
   // ADD to VerbConfig
   pub produces: Option<VerbProduces>,
   #[serde(default)]
   pub consumes: Vec<VerbConsumes>,
   ```

3. **Opus's `ArgDefinition.ref_type`** maps to our existing `ArgConfig.lookup.entity_type`. No new field needed.

### Recommended Change to Opus TODO

```diff
- pub struct ArgDefinition {
-     pub ref_type: Option<String>,   // NEW: for @ref validation
- }
+ // Use existing: ArgConfig.lookup.entity_type
+ // No new field needed - extract from lookup config
```

---

## Phase 2: Binding Context

### Current State

**File:** `rust/src/api/session.rs`

We already have:
```rust
pub struct BoundEntity {
    pub id: Uuid,
    pub entity_type: String,      // "cbu", "entity", "case"
    pub display_name: String,
}

pub struct SessionContext {
    pub bindings: HashMap<String, BoundEntity>,
    // ...
}
```

### Integration Notes

1. **`BoundEntity` is similar to Opus's `BindingInfo`** but:
   - Missing `subtype` (e.g., "proper_person")
   - Missing `resolved` flag (lookup vs create)
   - Missing `source_sheet_id`

2. **Recommend: Extend `BoundEntity`** rather than create new struct:
   ```rust
   pub struct BoundEntity {
       pub id: Uuid,
       pub entity_type: String,
       pub subtype: Option<String>,      // NEW
       pub display_name: String,
       pub resolved: bool,               // NEW: true if from lookup
       pub source_sheet_id: Option<Uuid>, // NEW
   }
   ```

3. **`BindingContext` as separate module** is cleaner. Create `rust/src/dsl_v2/binding_context.rs` as Opus suggests.

### Recommended Change to Opus TODO

```diff
- pub struct BindingInfo { ... }
+ // Extend existing BoundEntity in session.rs OR
+ // Create BindingInfo in dsl_v2/binding_context.rs and use for validation
+ // Keep BoundEntity for session/LLM context (simpler interface)
```

---

## Phase 3: Dataflow Validation

### Current State

**File:** `rust/src/dsl_v2/csg_linter.rs`

Current linter has:
- `CsgLinter::lint()` - main entry point
- `validate_args()` - arg validation
- `validate_verb()` - verb existence
- `LintError` enum

### Integration Notes

1. **Add `DataflowError` variants** to `LintError` or create separate enum
2. **Add `validate_dataflow()` function** as Opus suggests
3. **Call from existing lint pipeline**

Current AST has `VerbCall.binding` - need method to extract binding ref from arg:

```rust
impl AstNode {
    /// If this is a SymbolRef (@name), return the name
    pub fn as_binding_ref(&self) -> Option<&str> {
        match self {
            AstNode::SymbolRef(name) => Some(name),
            _ => None,
        }
    }
}
```

**This already exists!** Check `ast.rs`:

```rust
pub enum AstNode {
    // ...
    SymbolRef(String),  // @name references
    // ...
}
```

### Recommended Change to Opus TODO

None - aligns well. Just need to check `AstNode::SymbolRef` variant exists (it does).

---

## Phase 4: Storage

### Current State

No existing `cbu_dsl_state` table. The DSL is executed and results stored in domain tables (cbus, entities, etc.).

### Integration Notes

1. **New concept:** Separating "executed DSL" from "pending DSL"
2. **New tables needed** - clean addition, no conflicts
3. **Consider:** Do we want to store DSL per CBU, or globally?

### Recommended Change to Opus TODO

Add consideration for:
- Migration strategy (what about existing CBUs?)
- Backfill script to populate `executed_dsl` from existing data?

---

## Phase 5: REPL

### Current State

**File:** `rust/src/bin/dsl_cli.rs` - existing CLI

Commands: `generate`, `custody`, `parse`, `validate`, `plan`, `execute`, `verbs`, `examples`, `demo`

### Integration Notes

1. **New binary `ob-dsl`** is clean - doesn't conflict with `dsl_cli`
2. **Reuses existing components:**
   - `DslExecutor` from `executor.rs`
   - `RuntimeVerbRegistry` from `runtime_registry.rs`
   - Parser from `parser.rs`

3. **REPL-specific features are new:**
   - Session state management
   - `:commit`, `:rollback` commands
   - Line-by-line validation

### Recommended Change to Opus TODO

Consider merging into `dsl_cli` as a subcommand:
```bash
dsl_cli repl --cbu <uuid>
```

Rather than separate binary. Keeps tooling consolidated.

---

## Phase 6: LSP Integration

### Current State

**Files:**
- `rust/crates/dsl-lsp/src/analysis/context.rs` - document analysis
- `rust/crates/dsl-lsp/src/handlers/completion.rs` - completions
- `rust/crates/dsl-lsp/src/server.rs` - LSP server

### Integration Notes

1. **`context.rs` needs extension** for binding tracking
2. **Completion handler** needs dataflow-aware ranking
3. **Diagnostics** need dataflow errors

Current LSP completion loads from EntityGateway for reference data. Need to also track document bindings.

### Recommended Change to Opus TODO

1. Integrate with existing `EntityGateway` for entity lookups
2. Add `executed_bindings` loading via new state repository
3. Keep backward compatible - documents without CBU context still work

---

## Phase 7: Agent Integration

### Current State

**File:** `rust/src/api/agent_routes.rs`

Hardcoded prompts at:
- Lines 1270-1420: `generate_dsl` system prompt
- Lines 1750-1850: `generate_with_tools` prompt

Problems identified:
- Line 1350-51: `@cbu` used without definition
- Examples are static, not context-aware

### Integration Notes

1. **Remove hardcoded examples** - major cleanup
2. **Dynamic context builder** replaces static prompts
3. **Session bindings** passed to LLM

Current session system already tracks bindings:
```rust
// agent_routes.rs:377-382
let session_bindings_for_llm: Vec<String> = {
    session.context.bindings_for_llm()
};
```

But this only includes **executed** bindings, not pending ones from current generation.

### Recommended Change to Opus TODO

1. Also track **pending bindings** from DSL in current message
2. Extract bindings from generated DSL before next turn
3. Update `SessionContext` to track pending bindings

---

## Phase 8: Testing

### Current State

**Files:**
- `rust/tests/db_integration.rs` - DB integration tests
- `rust/tests/scenarios/*.dsl` - DSL scenario files

### Integration Notes

Opus test patterns align with existing:
- Unit tests in module `tests` submodule
- Integration tests in `rust/tests/`

### Recommended Change to Opus TODO

Add scenario tests for dataflow validation:
```
rust/tests/scenarios/
├── dataflow_valid.dsl       # Valid binding order
├── dataflow_invalid.dsl     # Invalid references
└── dataflow_reorder.dsl     # Tests topological sort
```

---

## Critical Integration Points

### 1. VerbConfig Extension

```rust
// rust/src/dsl_v2/config/types.rs

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VerbConfig {
    // ... existing fields ...
    
    // NEW: Dataflow metadata
    #[serde(default)]
    pub produces: Option<VerbProduces>,
    #[serde(default)]
    pub consumes: Vec<VerbConsumes>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VerbProduces {
    #[serde(rename = "type")]
    pub produced_type: String,
    pub subtype: Option<String>,
    #[serde(default)]
    pub resolved: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VerbConsumes {
    pub arg: String,
    #[serde(rename = "type")]
    pub consumed_type: String,
    #[serde(default = "default_true")]
    pub required: bool,
}
```

### 2. RuntimeVerbRegistry Extension

```rust
// rust/src/dsl_v2/runtime_registry.rs

impl RuntimeVerbRegistry {
    // NEW methods
    pub fn get_produces(&self, domain: &str, verb: &str) -> Option<&VerbProduces> {
        self.get(domain, verb)?.produces.as_ref()
    }
    
    pub fn get_consumes(&self, domain: &str, verb: &str) -> &[VerbConsumes] {
        self.get(domain, verb)
            .map(|v| v.consumes.as_slice())
            .unwrap_or(&[])
    }
    
    pub fn get_arg_ref_type(&self, domain: &str, verb: &str, arg: &str) -> Option<&str> {
        self.get(domain, verb)?
            .args
            .iter()
            .find(|a| a.name == arg)?
            .lookup
            .as_ref()?
            .entity_type
            .as_deref()
    }
}
```

### 3. Binding Type Derivation

Use existing `lookup.entity_type` from `ArgConfig`:

```yaml
# verbs.yaml - existing pattern
args:
  - name: cbu-id
    type: uuid
    required: true
    lookup:
      table: cbus
      entity_type: cbu        # ← This is the ref_type
      search_key: name
      primary_key: cbu_id
```

For `consumes`, we reference this same entity_type:

```yaml
# verbs.yaml - new pattern
consumes:
  - arg: cbu-id              # references the arg above
    type: cbu                # must match lookup.entity_type
    required: true
```

---

## Potential Conflicts

### 1. Session vs BindingContext

Two similar structures:
- `session.rs::BoundEntity` - for LLM context
- `binding_context.rs::BindingInfo` - for validation

**Resolution:** Keep both. `BindingInfo` is internal validation struct, `BoundEntity` is API-facing.

### 2. Existing Lint Errors

Current `LintError` in `csg_linter.rs`. Opus adds `DataflowError`.

**Resolution:** Either:
- Add `DataflowError` variants to `LintError`, or
- Return `Vec<LintError>` and `Vec<DataflowError>` separately

Recommend: Add to `LintError` for unified error handling.

### 3. AST Mutations

Opus assumes AST can store `resolved_pk` after execution:
```rust
if let Some(pk) = stmt.resolved_pk {
```

Current AST doesn't have `resolved_pk` field.

**Resolution:** Two options:
1. Add `resolved_pk: Option<Uuid>` to `VerbCall`
2. Keep AST immutable, return bindings separately (cleaner)

Recommend: Option 2 - keep AST immutable, return bindings from executor.

---

## Summary of Changes Needed

### New Files
- `rust/src/dsl_v2/binding_context.rs`
- `rust/src/dsl_v2/topo_sort.rs`
- `rust/src/database/dsl_state_repository.rs`
- `rust/src/repl/mod.rs` and `session.rs`
- `rust/src/agentic/context_builder.rs`
- Migration SQL for `cbu_dsl_state` tables

### Modified Files
- `rust/src/dsl_v2/config/types.rs` - add `VerbProduces`, `VerbConsumes`
- `rust/src/dsl_v2/runtime_registry.rs` - add helper methods
- `rust/src/dsl_v2/csg_linter.rs` - add dataflow validation
- `rust/src/api/agent_routes.rs` - remove hardcoded prompts
- `rust/src/api/session.rs` - extend `BoundEntity`
- `rust/crates/dsl-lsp/src/analysis/context.rs` - add binding tracking
- `rust/crates/dsl-lsp/src/handlers/completion.rs` - dataflow-aware
- `rust/config/verbs.yaml` - add produces/consumes to all verbs

### Unchanged Files
- `rust/src/dsl_v2/parser.rs` - no changes needed
- `rust/src/dsl_v2/executor.rs` - minor changes for binding return
- `rust/src/dsl_v2/ast.rs` - no changes needed (keep immutable)

---

## Implementation Order

1. **Phase 1** - Extend types.rs and runtime_registry.rs (foundation)
2. **Phase 2** - Create binding_context.rs (needed by Phase 3)
3. **Phase 3** - Add dataflow validation to linter (core logic)
4. **Phase 7** - Update agent_routes.rs (immediate user-facing fix)
5. **Phase 4** - Add storage tables (needed for REPL)
6. **Phase 5** - Build REPL (uses all above)
7. **Phase 6** - LSP integration (polish)
8. **Phase 8** - Testing throughout

Start with Phase 1-3 and Phase 7 in parallel - they don't depend on each other and both provide immediate value.
