# Refactor: Rename forth_engine to dsl_runtime

**Created:** 2025-11-25  
**Status:** READY TO EXECUTE  
**Priority:** P2 — Housekeeping  
**Scope:** Directory rename, import updates, terminology cleanup  

---

## Rationale

The `forth_engine` name is misleading. The implementation:
- ❌ Has no Forth stack machine
- ❌ Has no Forth semantics (postfix, concatenative)
- ✅ Parses S-expressions → AST
- ✅ Tree walks AST → calls registered Rust functions
- ✅ Emits CrudStatements for execution

The name `dsl_runtime` accurately describes what it is: a generic DSL runtime that can host any vocabulary.

---

## Part 1: Directory Rename

```bash
# Rename the directory
mv rust/src/forth_engine rust/src/dsl_runtime
```

---

## Part 2: File Renames

| Old Path | New Path |
|----------|----------|
| `rust/src/dsl_runtime/vocab_registry.rs` | `rust/src/dsl_runtime/vocabulary.rs` |

```bash
mv rust/src/dsl_runtime/vocab_registry.rs rust/src/dsl_runtime/vocabulary.rs
```

---

## Part 3: Update mod.rs

**File:** `rust/src/dsl_runtime/mod.rs`

Change:
```rust
pub mod vocab_registry;
```

To:
```rust
pub mod vocabulary;
```

Update any internal references from `vocab_registry` to `vocabulary`.

---

## Part 4: Update All Imports

Search and replace in all `.rs` files under `rust/src/`:

| Find | Replace |
|------|---------|
| `forth_engine` | `dsl_runtime` |
| `vocab_registry` | `vocabulary` |

**Files likely affected:**
- `rust/src/lib.rs` or `rust/src/main.rs`
- `rust/src/database/crud_executor.rs`
- `rust/src/agentic/orchestrator.rs`
- `rust/src/agentic/rag_context.rs`
- `rust/src/agentic/llm_generator.rs`
- `rust/src/agentic/validation/pipeline.rs`
- Any test files

```bash
# Find all files with old references
grep -r "forth_engine" rust/src/
grep -r "vocab_registry" rust/src/

# Automated replacement (run from project root)
find rust/src -name "*.rs" -exec sed -i '' 's/forth_engine/dsl_runtime/g' {} \;
find rust/src -name "*.rs" -exec sed -i '' 's/vocab_registry/vocabulary/g' {} \;
```

---

## Part 5: Update Cargo.toml (if needed)

Check `rust/Cargo.toml` for any references to `forth_engine` in:
- Feature flags
- Binary targets
- Example targets

---

## Part 6: Terminology Updates in Code Comments

Optional but recommended — update comments and doc strings:

| Old Term | New Term |
|----------|----------|
| "Forth engine" | "DSL runtime" |
| "Forth stack" | (remove — no stack) |
| "Forth word" | "DSL word" or just "word" |
| "push_crud" | Consider renaming to `emit_crud` |

---

## Part 7: Update Documentation Files

Search and update any `.md` files that reference `forth_engine`:

```bash
grep -r "forth_engine" *.md
grep -r "forth_engine" docs/
```

Known files:
- `REFACTOR_FORTH_TO_DIRECT_RUNTIME.md` — update references
- `REFACTOR_AGENTIC_RAG_INTEGRATION.md` — update references
- `REVIEW_CBU_MODEL.md` — update references

---

## Part 8: Suggested Future Structure

After rename, consider splitting `words.rs` by domain:

```
rust/src/dsl_runtime/
├── mod.rs
├── parser.rs
├── ast.rs
├── value.rs
├── runtime.rs
├── vocabulary.rs
└── words/
    ├── mod.rs
    ├── cbu.rs         # cbu.* words
    ├── entity.rs      # entity.* words
    ├── document.rs    # document.* words
    ├── kyc.rs         # kyc.*, investigation.* words
    ├── ubo.rs         # ubo.* words
    ├── screening.rs   # screening.* words
    ├── risk.rs        # risk.* words
    ├── decision.rs    # decision.* words
    └── taxonomy.rs    # product.*, service.*, lifecycle-resource.* words
```

This is optional and can be done later — it's just organization.

---

## Part 9: Verification

After all changes:

```bash
# 1. Check for any remaining old references
grep -r "forth_engine" rust/
grep -r "vocab_registry" rust/

# 2. Compile
cd rust && cargo check

# 3. Run tests
cargo test

# 4. Verify imports resolve
cargo build
```

---

## Summary

| Step | Action |
|------|--------|
| 1 | `mv rust/src/forth_engine rust/src/dsl_runtime` |
| 2 | `mv rust/src/dsl_runtime/vocab_registry.rs rust/src/dsl_runtime/vocabulary.rs` |
| 3 | Update `mod.rs` |
| 4 | Find/replace all imports |
| 5 | Check `Cargo.toml` |
| 6 | Update comments (optional) |
| 7 | Update `.md` docs |
| 8 | Verify with `cargo check && cargo test` |

This is a safe mechanical refactor — no logic changes, just naming.
