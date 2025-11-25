# Refactor: Replace Forth Stack Machine with Direct AST Runtime

**Created:** 2025-11-25  
**Status:** TODO  
**Estimated Effort:** 2-4 hours with Claude Code  
**Risk:** Medium (changes execution core, but logic preserved)

---

## Executive Summary

The current DSL execution uses a Forth-style stack machine (compiler → bytecode → VM → stack operations). This is unnecessary complexity for S-expression DSL with named arguments. 

**Replace with:** Direct AST interpretation — parse → walk → dispatch to Rust functions.

**Result:** ~400 lines deleted, simpler debugging, same functionality.

---

## Current Architecture (To Remove)

```
NomDslParser → Expr AST
      ↓
compiler.rs: compile_sheet() → Vec<Instruction>
      ↓
vm.rs: VM { data_stack, return_stack, ip } executes instructions
      ↓
Words pop args from stack via collect_keyword_pairs()
```

## Target Architecture

```
NomDslParser → Expr AST
      ↓
runtime.rs: Runtime::execute_sheet() walks AST directly
      ↓
Words receive args as &[Arg] slice, no stack
```

---

## Files to DELETE

These files will be completely removed:

```
rust/src/forth_engine/compiler.rs    # ~150 lines - bytecode generation
rust/src/forth_engine/vm.rs          # ~180 lines - stack machine
```

## Files to MODIFY

### 1. `rust/src/forth_engine/mod.rs`

**Remove:**
```rust
pub mod compiler;
pub mod vm;
```

**Add:**
```rust
pub mod runtime;
```

**Update re-exports** to remove `compile_sheet`, `VM`, `Program`, `Instruction`, `OpCode`.

---

### 2. Create NEW: `rust/src/forth_engine/runtime.rs`

```rust
//! Direct AST Runtime for DSL Execution
//!
//! Replaces the Forth stack machine with direct interpretation.
//! S-expressions with named arguments don't need stack threading.

use crate::forth_engine::ast::Expr;
use crate::forth_engine::env::RuntimeEnv;
use crate::forth_engine::errors::EngineError;
use crate::forth_engine::value::{AttributeId, DocumentId, Value};
use std::collections::HashMap;

/// Parsed argument: keyword + value pair
#[derive(Debug, Clone)]
pub struct Arg {
    pub key: String,
    pub value: Value,
}

/// Word function signature - receives args directly, no stack
pub type WordFn = fn(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError>;

/// Word entry with metadata for RAG/agent context
pub struct WordEntry {
    pub name: &'static str,
    pub domain: &'static str,
    pub func: WordFn,
    pub signature: &'static str,
    pub description: &'static str,
    pub examples: &'static [&'static str],
}

/// Direct AST runtime - no stack machine
pub struct Runtime {
    vocab: HashMap<&'static str, WordEntry>,
}

impl Runtime {
    /// Create runtime with registered vocabulary
    pub fn new(words: Vec<WordEntry>) -> Self {
        let mut vocab = HashMap::new();
        for entry in words {
            vocab.insert(entry.name, entry);
        }
        Self { vocab }
    }

    /// Execute a DSL sheet (multiple expressions)
    pub fn execute_sheet(&self, exprs: &[Expr], env: &mut RuntimeEnv) -> Result<(), EngineError> {
        for expr in exprs {
            self.execute_expr(expr, env)?;
        }
        Ok(())
    }

    /// Execute a single expression
    fn execute_expr(&self, expr: &Expr, env: &mut RuntimeEnv) -> Result<(), EngineError> {
        match expr {
            Expr::WordCall { name, args } => {
                let entry = self.vocab.get(name.as_str())
                    .ok_or_else(|| EngineError::UnknownWord(name.clone()))?;
                
                let parsed_args = self.extract_args(args)?;
                (entry.func)(&parsed_args, env)
            }
            Expr::Comment(_) => Ok(()), // Skip comments
            _ => Err(EngineError::Parse(format!(
                "Top-level expression must be a word call, got: {:?}", expr
            ))),
        }
    }

    /// Extract keyword-value pairs from argument list
    fn extract_args(&self, args: &[Expr]) -> Result<Vec<Arg>, EngineError> {
        let mut result = Vec::new();
        let mut iter = args.iter().peekable();
        
        while let Some(expr) = iter.next() {
            match expr {
                Expr::Keyword(key) => {
                    let value_expr = iter.next().ok_or_else(|| {
                        EngineError::Parse(format!("Keyword {} missing value", key))
                    })?;
                    result.push(Arg {
                        key: key.clone(),
                        value: self.expr_to_value(value_expr)?,
                    });
                }
                // Allow non-keyword args for positional parameters (rare)
                _ => {
                    result.push(Arg {
                        key: format!("_pos_{}", result.len()),
                        value: self.expr_to_value(expr)?,
                    });
                }
            }
        }
        Ok(result)
    }

    /// Convert AST Expr to runtime Value
    fn expr_to_value(&self, expr: &Expr) -> Result<Value, EngineError> {
        match expr {
            Expr::StringLiteral(s) => Ok(Value::Str(s.clone())),
            Expr::IntegerLiteral(i) => Ok(Value::Int(*i)),
            Expr::FloatLiteral(f) => Ok(Value::Float(*f)),
            Expr::BoolLiteral(b) => Ok(Value::Bool(*b)),
            Expr::Keyword(k) => Ok(Value::Keyword(k.clone())),
            Expr::DottedKeyword(parts) => Ok(Value::DottedKeyword(parts.clone())),
            Expr::AttributeRef(id) => Ok(Value::Attr(AttributeId(id.clone()))),
            Expr::DocumentRef(id) => Ok(Value::Doc(DocumentId(id.clone()))),
            Expr::ListLiteral(items) => {
                let values: Result<Vec<_>, _> = items.iter()
                    .map(|e| self.expr_to_value(e))
                    .collect();
                Ok(Value::List(values?))
            }
            Expr::MapLiteral(pairs) => {
                let converted: Result<Vec<_>, _> = pairs.iter()
                    .map(|(k, v)| Ok((k.clone(), self.expr_to_value(v)?)))
                    .collect();
                Ok(Value::Map(converted?))
            }
            Expr::WordCall { .. } => {
                // Nested word calls - for now, error
                // Could support if words return values
                Err(EngineError::Parse(
                    "Nested word calls not yet supported as values".into()
                ))
            }
            Expr::Comment(_) => Err(EngineError::Parse(
                "Comment cannot be used as value".into()
            )),
        }
    }

    /// Get word entry for RAG context building
    pub fn get_word(&self, name: &str) -> Option<&WordEntry> {
        self.vocab.get(name)
    }

    /// Get all words in a domain for RAG context
    pub fn get_domain_words(&self, domain: &str) -> Vec<&WordEntry> {
        self.vocab.values()
            .filter(|w| w.domain == domain)
            .collect()
    }

    /// Get all domains
    pub fn get_domains(&self) -> Vec<&'static str> {
        let mut domains: Vec<_> = self.vocab.values()
            .map(|w| w.domain)
            .collect();
        domains.sort();
        domains.dedup();
        domains
    }

    /// Get all word names (for validation)
    pub fn get_all_word_names(&self) -> Vec<&'static str> {
        self.vocab.keys().copied().collect()
    }
}

/// Helper trait for argument extraction
pub trait ArgList {
    fn require_string(&self, key: &str) -> Result<String, EngineError>;
    fn get_string(&self, key: &str) -> Option<String>;
    fn require_int(&self, key: &str) -> Result<i64, EngineError>;
    fn get_int(&self, key: &str) -> Option<i64>;
    fn require_uuid(&self, key: &str) -> Result<uuid::Uuid, EngineError>;
    fn get_uuid(&self, key: &str) -> Option<uuid::Uuid>;
    fn get_list(&self, key: &str) -> Option<Vec<Value>>;
    fn get_value(&self, key: &str) -> Option<&Value>;
}

impl ArgList for [Arg] {
    fn require_string(&self, key: &str) -> Result<String, EngineError> {
        self.get_string(key)
            .ok_or_else(|| EngineError::MissingArgument(key.into()))
    }

    fn get_string(&self, key: &str) -> Option<String> {
        self.iter()
            .find(|a| a.key == key)
            .and_then(|a| match &a.value {
                Value::Str(s) => Some(s.clone()),
                _ => None,
            })
    }

    fn require_int(&self, key: &str) -> Result<i64, EngineError> {
        self.get_int(key)
            .ok_or_else(|| EngineError::MissingArgument(key.into()))
    }

    fn get_int(&self, key: &str) -> Option<i64> {
        self.iter()
            .find(|a| a.key == key)
            .and_then(|a| match &a.value {
                Value::Int(i) => Some(*i),
                _ => None,
            })
    }

    fn require_uuid(&self, key: &str) -> Result<uuid::Uuid, EngineError> {
        let s = self.require_string(key)?;
        uuid::Uuid::parse_str(&s)
            .map_err(|e| EngineError::Parse(format!("Invalid UUID for {}: {}", key, e)))
    }

    fn get_uuid(&self, key: &str) -> Option<uuid::Uuid> {
        self.get_string(key)
            .and_then(|s| uuid::Uuid::parse_str(&s).ok())
    }

    fn get_list(&self, key: &str) -> Option<Vec<Value>> {
        self.iter()
            .find(|a| a.key == key)
            .and_then(|a| match &a.value {
                Value::List(items) => Some(items.clone()),
                _ => None,
            })
    }

    fn get_value(&self, key: &str) -> Option<&Value> {
        self.iter()
            .find(|a| a.key == key)
            .map(|a| &a.value)
    }
}

// Error variant needed - add to errors.rs
// EngineError::UnknownWord(String)
// EngineError::MissingArgument(String)
```

---

### 3. Update `rust/src/forth_engine/errors.rs`

**Add variants:**
```rust
#[derive(Debug, Error)]
pub enum EngineError {
    // ... existing variants ...

    #[error("Unknown word: '{0}'")]
    UnknownWord(String),

    #[error("Missing required argument: '{0}'")]
    MissingArgument(String),
}
```

**Remove** any `CompileError` references if they become unused.

---

### 4. Create NEW: `rust/src/forth_engine/words.rs`

Refactored word implementations. **Pattern for each word:**

**OLD (stack-based):**
```rust
fn word_cbu_create(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 5)?;
    process_pairs(vm, &pairs);
    let values = pairs.into_iter().map(...).collect();
    vm.env.push_crud(CrudStatement::DataCreate { ... });
    Ok(())
}
```

**NEW (direct args):**
```rust
pub fn cbu_create(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    let name = args.require_string(":cbu-name")
        .or_else(|_| args.require_string(":name"))?;
    let client_type = args.get_string(":client-type");
    let jurisdiction = args.get_string(":jurisdiction");
    let nature_purpose = args.get_string(":nature-purpose");
    let description = args.get_string(":description");

    let mut values = HashMap::new();
    values.insert("cbu-name".into(), Value::Str(name));
    if let Some(v) = client_type { values.insert("client-type".into(), Value::Str(v)); }
    if let Some(v) = jurisdiction { values.insert("jurisdiction".into(), Value::Str(v)); }
    if let Some(v) = nature_purpose { values.insert("nature-purpose".into(), Value::Str(v)); }
    if let Some(v) = description { values.insert("description".into(), Value::Str(v)); }

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "CBU".into(),
        values,
    }));

    Ok(())
}
```

**Full file structure:**
```rust
//! Word implementations for DSL vocabulary
//!
//! Each function: fn(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError>

use crate::forth_engine::errors::EngineError;
use crate::forth_engine::env::RuntimeEnv;
use crate::forth_engine::runtime::{Arg, ArgList};
use crate::forth_engine::value::{CrudStatement, DataCreate, DataRead, DataUpdate, DataDelete, Value};
use std::collections::HashMap;

// ============================================================================
// CBU DOMAIN
// ============================================================================

pub fn cbu_create(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    // ... implementation
}

pub fn cbu_read(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    // ... implementation
}

pub fn cbu_update(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    // ... implementation
}

pub fn cbu_delete(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    // ... implementation
}

pub fn cbu_list(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    // ... implementation
}

pub fn cbu_attach_entity(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    // ... implementation
}

pub fn cbu_attach_proper_person(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    // ... implementation
}

pub fn cbu_finalize(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    // ... implementation
}

// ============================================================================
// ENTITY DOMAIN
// ============================================================================

pub fn entity_register(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    // ... implementation
}

// ... etc for all 63 words

// ============================================================================
// HELPER: Convert args to CRUD values HashMap
// ============================================================================

fn args_to_crud_values(args: &[Arg]) -> HashMap<String, Value> {
    args.iter()
        .map(|a| {
            let key = a.key.trim_start_matches(':').to_string();
            (key, a.value.clone())
        })
        .collect()
}
```

---

### 5. Create NEW: `rust/src/forth_engine/vocab_registry.rs`

Build the runtime with all words registered:

```rust
//! Vocabulary registry - builds Runtime with all words

use crate::forth_engine::runtime::{Runtime, WordEntry};
use crate::forth_engine::words;

/// Create the standard DSL runtime with all vocabulary
pub fn create_standard_runtime() -> Runtime {
    Runtime::new(vec![
        // ============ CBU Domain ============
        WordEntry {
            name: "cbu.create",
            domain: "cbu",
            func: words::cbu_create,
            signature: ":cbu-name STRING :client-type STRING? :jurisdiction STRING? :nature-purpose STRING? :description STRING?",
            description: "Create a new Client Business Unit",
            examples: &[
                r#"(cbu.create :cbu-name "AcmeFund" :client-type "HEDGE_FUND" :jurisdiction "GB")"#,
            ],
        },
        WordEntry {
            name: "cbu.read",
            domain: "cbu",
            func: words::cbu_read,
            signature: ":cbu-id UUID",
            description: "Read a CBU by ID",
            examples: &[
                r#"(cbu.read :cbu-id "550e8400-e29b-41d4-a716-446655440000")"#,
            ],
        },
        WordEntry {
            name: "cbu.update",
            domain: "cbu",
            func: words::cbu_update,
            signature: ":cbu-id UUID :name STRING? :status STRING?",
            description: "Update a CBU",
            examples: &[
                r#"(cbu.update :cbu-id "..." :name "NewName")"#,
            ],
        },
        WordEntry {
            name: "cbu.delete",
            domain: "cbu",
            func: words::cbu_delete,
            signature: ":cbu-id UUID",
            description: "Delete a CBU",
            examples: &[
                r#"(cbu.delete :cbu-id "...")"#,
            ],
        },
        // ... continue for all 63 words
        
        // ============ Document Domain ============
        WordEntry {
            name: "document.catalog",
            domain: "document",
            func: words::document_catalog,
            signature: ":doc-id STRING :doc-type STRING",
            description: "Catalog a document for processing",
            examples: &[
                r#"(document.catalog :doc-id "DOC-001" :doc-type "UK-PASSPORT")"#,
            ],
        },
        // ... etc
    ])
}
```

---

### 6. Update `rust/src/forth_engine/kyc_vocab.rs`

**Option A:** Delete entirely, move word impls to `words.rs`

**Option B:** Keep as reference during migration, then delete

---

### 7. Update all call sites

Search for usages of:
- `compile_sheet` → Replace with `runtime.execute_sheet`
- `VM::new` → Remove
- `vm.step_with_logging` → Replace with direct execution
- `kyc_orch_vocab()` → Replace with `create_standard_runtime()`

**Key files to update:**
- `rust/src/bin/cbu_live_test.rs`
- `rust/src/bin/e2e_cbu_flow_test.rs`
- `rust/src/services/agentic_complete.rs`
- `rust/src/services/agentic_dsl_crud.rs`
- `rust/src/test_harness.rs`
- Any tests in `rust/src/forth_engine/`

---

## Migration Pattern for Call Sites

**OLD:**
```rust
use crate::forth_engine::{compile_sheet, VM, kyc_vocab::kyc_orch_vocab};

let vocab = Arc::new(kyc_orch_vocab());
let parser = NomDslParser::new();
let exprs = parser.parse(&dsl_source)?;
let program = Arc::new(compile_sheet(&exprs, &vocab)?);

let mut env = RuntimeEnv::new(request_id);
let mut vm = VM::new(program, vocab, &mut env);

while let Some(log) = vm.step_with_logging()? {
    debug!("{}", log);
}

let crud_stmts = env.take_pending_crud();
```

**NEW:**
```rust
use crate::forth_engine::{runtime::Runtime, vocab_registry::create_standard_runtime};

let runtime = create_standard_runtime();
let parser = NomDslParser::new();
let exprs = parser.parse(&dsl_source)?;

let mut env = RuntimeEnv::new(request_id);
runtime.execute_sheet(&exprs, &mut env)?;

let crud_stmts = env.take_pending_crud();
```

---

## Files to Delete After Migration

Once all tests pass:

```bash
rm rust/src/forth_engine/compiler.rs
rm rust/src/forth_engine/vm.rs
# After moving word impls:
rm rust/src/forth_engine/kyc_vocab.rs
```

---

## Verification Steps

1. **Compile check:** `cargo check` passes
2. **Unit tests:** `cargo test` passes
3. **Integration test:** `cargo run --bin e2e_cbu_flow_test` works
4. **Manual test:** Execute a sample DSL sheet, verify CRUD output matches old behavior

---

## Claude Code Instructions

When implementing this refactor:

1. **Start with runtime.rs** - Create the new runtime module first
2. **Add error variants** - Update errors.rs before words.rs
3. **Create words.rs** - Port word implementations one domain at a time
4. **Create vocab_registry.rs** - Wire up all words
5. **Update mod.rs** - Change exports
6. **Update call sites** - One file at a time, run `cargo check` after each
7. **Run tests** - Verify behavior unchanged
8. **Delete old files** - Only after everything works

---

## Rollback Plan

If issues arise:
- Old files are in git history
- Can restore `compiler.rs`, `vm.rs`, `kyc_vocab.rs` from HEAD~1

---

## Post-Refactor Cleanup

After successful migration:

1. **Delete TaxonomyCrudStatement system** - It's now redundant
   - `rust/src/taxonomy/crud_ast.rs` 
   - `rust/src/taxonomy/crud_operations.rs`
   - `rust/src/taxonomy/dsl_parser.rs` (the NL regex parser)
   
2. **Delete dead vocabulary code**
   - `rust/src/vocabulary/vocab_registry.rs` (the unused DB-backed one)
   
3. **Update DSL_REVIEW_PLAN.md** - Mark refactor complete

---

## RAG Integration Opportunity

The new `WordEntry` struct includes metadata for RAG:
- `signature` - Argument specification
- `description` - What the word does  
- `examples` - Valid DSL snippets

This can feed directly into RAG context:
```rust
fn build_rag_context(runtime: &Runtime, domain: &str) -> String {
    let words = runtime.get_domain_words(domain);
    words.iter()
        .map(|w| format!(
            "Word: {}\nSignature: {}\nDescription: {}\nExample: {}\n",
            w.name, w.signature, w.description, w.examples.first().unwrap_or(&"")
        ))
        .collect::<Vec<_>>()
        .join("\n---\n")
}
```

This replaces the need to query the DB `vocabulary_registry` table.
