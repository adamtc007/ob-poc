# Template System Status

Updated: 2025-12-16

## Current State

All 12 templates:
- ✅ Load successfully
- ✅ Expand (with sample params showing "partial" - expected for missing required params)
- ✅ Parse successfully (12/12)
- ✅ Compile successfully (12/12)

**Key Fix Applied**: Templates now use plain strings (e.g., `:cbu-id "$cbu_name"`) instead of entity ref syntax. The enrichment pass converts these to `EntityRef` nodes based on verb YAML `lookup` config.

---

## Architecture Notes

### Two Execution Paths

1. **DAG/Compiler Path** (`compiler.rs` → `Op` → toposort → execute)
   - Used for complex multi-verb scripts needing dependency ordering
   - Compiler only handles ~20 hardcoded verbs
   - Shows "0 ops" for verbs not in compiler

2. **Direct Execution Path** (`execute_verb` → generic executor)
   - Used for single statement execution
   - Handles ALL verbs defined in YAML with `behavior: crud`
   - Verbs like `kyc-case.escalate`, `doc-request.create` work here

### Implication

The "0 ops" shown in harness for some templates is **not a bug**. Those verbs:
- Are defined in YAML with `behavior: crud`
- Execute correctly via GenericCrudExecutor
- Just don't have special Op generation in compiler (not needed)

---

## Verbs Without Compiler Ops (By Design)

These verbs work via generic executor but show "0 ops" in compiler:

| Verb | Reason |
|------|--------|
| `kyc-case.escalate` | Simple CRUD update |
| `kyc-case.set-risk-rating` | Simple CRUD update |
| `kyc-case.close` | Simple CRUD update |
| `doc-request.create` | Simple CRUD insert |
| `case-screening.review-hit` | Simple CRUD update |
| `ubo.trace-chains` | Plugin behavior (custom_ops) |

**No action needed** - these execute correctly.

---

## Remaining Gaps

### Gap 1: Compiler Doesn't Know All CRUD Verbs

The compiler's match statement only handles ~20 verbs explicitly. For DAG-based execution with many statements, this means:
- Statements with unknown verbs get skipped in Op generation
- DAG ordering may be incomplete

**Options**:
1. Add generic CRUD Op that the compiler emits for any `behavior: crud` verb
2. Keep current design - compiler for complex verbs, direct execution for simple ones

**Recommendation**: Add `Op::GenericCrud { verb, args }` variant for DAG completeness.

### Gap 2: Template Parameter Validation

The harness shows "missing params" correctly but doesn't validate:
- Param types match verb arg types
- Required params are actually required in verb definition

**Low priority** - templates work, just lacks strict validation.

### Gap 3: Bulk Execution Pattern

Templates generate DSL for one entity. For bulk operations:
- Agent queries for entities (e.g., "all funds in account X")
- Expands template N times with each entity
- Batch executes

**See**: `TODO-TEMPLATES.md` Phase 3 for bulk design.

---

## Testing

```bash
cd rust/

# Run template harness
cargo run --bin template_harness

# Verbose output
cargo run --bin template_harness -- --verbose

# With database execution (validates end-to-end)
DATABASE_URL="postgresql:///data_designer" \
cargo run --features database --bin template_harness -- --execute
```

---

## Template Inventory

| Template | Primary Entity | Ops | Status |
|----------|---------------|-----|--------|
| onboard-director | cbu | 5 | ✅ |
| onboard-signatory | cbu | 3 | ✅ |
| add-ownership | cbu | 1 | ✅ |
| register-ubo | cbu | 1 | ✅ |
| trace-chains | cbu | 0* | ✅ |
| create-kyc-case | cbu | 1 | ✅ |
| escalate-case | kyc_case | 0* | ✅ |
| approve-case | kyc_case | 0* | ✅ |
| run-entity-screening | kyc_case | 3 | ✅ |
| review-screening-hit | kyc_case | 0* | ✅ |
| catalog-document | cbu | 1 | ✅ |
| request-documents | kyc_case | 0* | ✅ |

*0 ops = verb handled by generic executor, not compiler
