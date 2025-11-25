# DSL Implementation Review Plan

**Created:** 2025-11-25  
**Purpose:** Systematic review of DSL implementation in small, session-safe chunks  
**Status:** 🟡 IN PROGRESS

---

## Architecture Overview

The DSL implementation is split into two major subsystems:

### Subsystem A: Forth/NOM Parser-Compiler-Runtime
Location: `rust/src/forth_engine/`
- `parser_nom.rs` - NOM-based S-expression parser
- `compiler.rs` - AST to bytecode compilation
- `vm.rs` - Stack-based virtual machine
- `vocab.rs` - Core vocabulary/word definitions
- `kyc_vocab.rs` - Domain-specific KYC vocabulary
- `value.rs` - Runtime value types
- `env.rs` - Execution environment
- `errors.rs` - Error types

### Subsystem B: DSL Source Editing & Generation
Location: `rust/src/dsl_source/`
- `agentic/` - RAG + LLM generation (partially implemented)
- `generator/` - Template-based DSL generation
- `context/` - Vocabulary and attribute context
- `validation/` - Pre-parse validation pipeline
- `editor/` - Formatting utilities

### Integration Layer: DSL.CRUD → SQLX
Location: `rust/src/database/`
- `crud_executor.rs` - DSL verb → SQLX execution
- `crud_service.rs` - CRUD service abstraction
- Various `*_service.rs` - Entity-specific CRUD

Location: `rust/src/taxonomy/`
- `crud_operations.rs` - Taxonomy CRUD verbs
- `crud_ast.rs` - CRUD-specific AST nodes

---

## Review Chunks

### CHUNK 1: Forth Engine Core (Parser)
**Files:** `forth_engine/parser_nom.rs`, `forth_engine/ast.rs`, `forth_engine/ebnf.rs`
**Time Est:** 1 session
**Status:** ✅ COMPLETE

**Review Goals:**
- [x] Trace NOM combinator chain for S-expression parsing
- [x] Verify kebab-case identifier handling
- [x] Check error recovery and position tracking
- [x] Document AST node types produced

**Findings:**

#### AST Structure (ast.rs)
Clean enum-based design with appropriate variants:
```rust
Expr {
    WordCall { name, args }  // (verb arg1 arg2) - core S-expr
    StringLiteral(String)
    IntegerLiteral(i64)
    FloatLiteral(f64)
    BoolLiteral(bool)
    Keyword(String)          // :keyword (keeps colon)
    DottedKeyword(Vec<String>) // :a.b.c -> ["a","b","c"]
    AttributeRef(String)     // @attr{uuid}
    DocumentRef(String)      // @doc{uuid}
    ListLiteral(Vec<Expr>)   // [a b c]
    MapLiteral(Vec<(String, Expr)>) // {:k v} (strips colon from key)
    Comment(String)          // ;; text
}
```

#### Parser Combinator Chain (parser_nom.rs)
```
parse() -> many0(parse_expr)
  parse_expr -> delimited(ws, alt(comment | s_expr | atom), ws)
    parse_s_expr -> delimited('(', word_call, ')')
      parse_word_call -> symbol + many0(expr)  // recursive
    parse_atom -> alt(
      number | bool | string | dotted_kw | keyword |
      attr_ref | doc_ref | list | map
    )
```

#### STRENGTHS
1. **Kebab-case handled correctly** - `parse_symbol` allows `-` in identifiers
2. **Dotted keywords work** - `:customer.address.city` parses to `["customer", "address", "city"]`
3. **Dual syntax for refs** - Both `@attr{uuid}` and legacy `@attr("uuid")` supported
4. **Escape sequences** - Strings handle `\n`, `\r`, `\t`, `\\`, `\"`
5. **List separators flexible** - Both `[1 2 3]` and `[1, 2, 3]` work
6. **Good test coverage** - Unit tests for each atom type

#### ISSUES FOUND

**CRITICAL: No position tracking**
- `Expr` has no span/location info
- Parse errors are just strings: `EngineError::Parse(String)`
- Makes debugging DSL sheets painful for users
- Later stages can't report "error at line 42, col 8"

**MEDIUM: EBNF spec drift**
Parser supports features not in `ebnf.rs`:
- FloatLiteral (EBNF only has INTEGER)
- ListLiteral `[...]`
- MapLiteral `{:k v}`
- Comment `;;`
- DottedKeyword
- New @attr{uuid} brace syntax

**MEDIUM: Symbol parsing too permissive**
```rust
parse_symbol starts with: alpha1 | "_" | "-" | "." | ">"
```
This allows symbols starting with `-` or `.` which may conflict with other syntax.

**LOW: Keyword vs Map key inconsistency**
- `parse_keyword` returns `":name"` (with colon)
- `parse_map_key` returns `"name"` (stripped colon)
- May cause confusion in downstream code

**LOW: Bool parsing greedy**
`tag("true")` might partially match `truename` before failing.
Not a real bug (NOM backtracks) but worth noting.

**INFO: Comments preserved in AST**
`Expr::Comment` nodes are returned. Compiler/VM must skip them.

#### Architecture Pattern: Layered Parsing
`cbu_model_parser.rs` shows the correct pattern:
1. NOM parser → generic `Vec<Expr>` AST
2. Domain parser → typed model (e.g., `CbuModel`)

This is good separation - keeps NOM parser reusable.

**Refactoring TODOs:**

1. **[P1] Add Spanned<Expr> wrapper** - Track byte offsets for error reporting
   ```rust
   pub struct Spanned<T> {
       pub node: T,
       pub span: Range<usize>,
   }
   ```

2. **[P2] Update EBNF spec** - Document all actually-supported syntax

3. **[P3] Tighten symbol start chars** - Remove `.` and `-` from valid start characters

4. **[P3] Normalize keyword representation** - Decide: always with colon or always without

---

### CHUNK 2: Forth Engine Core (Compiler + VM)
**Files:** `forth_engine/compiler.rs`, `forth_engine/vm.rs`, `forth_engine/value.rs`, `forth_engine/kyc_vocab.rs`
**Time Est:** 1 session
**Status:** ✅ COMPLETE

**Review Goals:**
- [x] Trace compilation from AST → bytecode/threaded code
- [x] Understand stack discipline and value types
- [x] Document word invocation mechanism
- [x] Check for S-expr vs postfix alignment issues (flagged in prior review)

**Findings:**

#### Compilation Flow (compiler.rs)
```
compile_sheet(exprs, vocab)
  └─→ for each expr: compile_expr_inner()
        WordCall { name, args }:
          1. Compile args left-to-right (push onto instruction stream)
          2. Emit CallWord(word_idx) 
        Literals: Emit LitInt/LitStr/LitKeyword/etc.
        AttrRef/DocRef: Resolve to ID, emit AttrRef/DocRef instruction
  └─→ Append Halt
  └─→ validate_stack_effects() - compile-time stack checking
```

#### Instruction Set (vm.rs)
```rust
enum Instruction {
    Op(OpCode),           // CallWord(idx), Halt
    LitInt(i64),
    LitFloat(f64),
    LitStr(String),
    LitKeyword(String),
    LitDottedKeyword(Vec<String>),
    LitList(Vec<Instruction>),  // Nested!
    LitMap(Vec<(String, Vec<Instruction>)>),  // Nested!
    AttrRef(AttributeId),
    DocRef(DocumentId),
}
```

#### VM Execution Model
- `data_stack: VecDeque<Value>` - main operand stack
- `return_stack: VecDeque<usize>` - unused (no subroutines yet)
- Words are `Arc<dyn Fn(&mut VM) -> Result<(), VmError>>` closures
- Word lookup by index into `vocab.specs[word_idx]`

#### S-EXPR vs POSTFIX RESOLUTION
**Prior concern was UNFOUNDED** - the design is intentional and correct:

```
DSL:     (cbu.create :name "Test" :type "Corp")
Compiles to:
  LitKeyword(":name")
  LitStr("Test")
  LitKeyword(":type")
  LitStr("Corp")
  CallWord(cbu.create)

At runtime, stack is: [:name, "Test", :type, "Corp"] (bottom to top)
Word pops pairs via pop_keyword_value() in LIFO order:
  -> (:type, "Corp"), then (:name, "Test")
```

This is **Clojure-style named arguments**, not traditional Forth postfix.
The `collect_keyword_pairs()` helper gathers N pairs regardless of order.

#### Value Types (value.rs)
```rust
enum Value {
    Int(i64), Float(f64), Str(String), Bool(bool),
    Keyword(String), DottedKeyword(Vec<String>),
    Attr(AttributeId), Doc(DocumentId),
    List(Vec<Value>), Map(Vec<(String, Value)>),
}
```
Plus `CrudStatement` enum: DataCreate, DataRead, DataUpdate, DataDelete
- These are the bridge to SQLX - words emit CrudStatements to `vm.env`

#### STRENGTHS
1. **Compile-time stack effect validation** - catches underflow early
2. **Clean word abstraction** - `WordSpec` with name, domain, stack_effect, impl_fn
3. **CrudStatement emission** - words produce DB operations, not raw SQL
4. **Domain organization** - words grouped by domain (case, entity, kyc, cbu, etc.)
5. **63 words defined** - comprehensive vocabulary for KYC/UBO domain

#### ISSUES FOUND

**CRITICAL: LitList execution is buggy**
```rust
// vm.rs line ~130
for item_instructions in items.chunks(1) {  // WRONG: assumes 1 instr per item
    for instr in item_instructions {
        self.execute_instruction(instr)?;
        if let Some(val) = self.data_stack.pop_back() {
            values.push(val);
        }
    }
}
```
If a list item compiles to multiple instructions (e.g., nested S-expr),
this will break. Should evaluate each item's full instruction sequence.

**MEDIUM: Bool compiles to Int, but Value has Bool**
```rust
// compiler.rs
Expr::BoolLiteral(b) => instructions.push(Instruction::LitInt(if *b { 1 } else { 0 })),
```
Should emit `LitBool` (which doesn't exist) or use `Value::Bool` properly.
Currently `true` becomes `Value::Int(1)` at runtime.

**MEDIUM: WordId indices are hardcoded**
```rust
WordSpec { id: WordId(31), name: "cbu.create" ... }
WordSpec { id: WordId(32), name: "cbu.read" ... }
```
If you reorder or insert words, indices break. Should auto-assign.

**MEDIUM: Variable-arity words break stack validation**
`collect_keyword_pairs()` silently breaks on underflow:
```rust
Err(VmError::StackUnderflow { .. }) => { break; }  // Swallows error
```
This means `stack_effect` validation is inaccurate for words that
accept variable numbers of pairs. Compile-time check may pass
but runtime may not have enough args.

**LOW: No type checking at compile time**
All type errors surface at runtime. Could add typed stack simulation.

**LOW: Return stack unused**
`return_stack` is allocated but never used. Dead code.

**Refactoring TODOs:**

1. **[P1] Fix LitList execution** - Properly evaluate nested instruction sequences
   ```rust
   Instruction::LitList(items) => {
       let mut values = Vec::new();
       for item_instrs in items {  // items should be Vec<Vec<Instruction>>
           let val = self.eval_instructions(item_instrs)?;
           values.push(val);
       }
       self.data_stack.push_back(Value::List(values));
   }
   ```

2. **[P1] Add LitBool instruction** - Preserve bool type through compilation

3. **[P2] Auto-assign WordId** - Remove manual index assignment
   ```rust
   impl Vocab {
       pub fn new(specs: Vec<WordSpec>) -> Self {
           let specs: Vec<_> = specs.into_iter().enumerate()
               .map(|(i, mut s)| { s.id = WordId(i); s }).collect();
           // ...
       }
   }
   ```

4. **[P2] Strict vs lenient word modes** - Flag words that truly accept
   variable args vs those that require exact count

5. **[P3] Remove unused return_stack** or implement subroutine calls

---

### CHUNK 3: Forth Engine Vocabulary System
**Files:** `forth_engine/vocab.rs`, `forth_engine/kyc_vocab.rs`, `forth_engine/env.rs`, `vocabulary/*.rs`, `sql/migrations/013-015`
**Time Est:** 1 session  
**Status:** ✅ COMPLETE

**Review Goals:**
- [x] Document all registered words/verbs
- [x] Trace word lookup and dispatch
- [x] Check vocabulary extensibility mechanism
- [x] Map KYC domain words to CRUD operations

**Findings:**

#### Two Vocabulary Systems Exist (CRITICAL ARCHITECTURE GAP)

**System 1: Hardcoded Rust Vocabulary (`forth_engine/kyc_vocab.rs`)**
```rust
pub fn kyc_orch_vocab() -> Vocab {
    let specs = vec![
        WordSpec { id: WordId(0), name: "case.create", ... },
        // ... 63 words total
    ];
    Vocab::new(specs)
}
```
- 63 words across 10 domains: case, entity, kyc, compliance, ubo, document, cbu, crud, attr, product/service/lifecycle-resource
- Each word has `stack_effect: (inputs, outputs)` for compile-time validation
- Implementation via `Arc<dyn Fn(&mut VM) -> Result<(), VmError>>`

**System 2: Database Vocabulary Registry (`vocabulary/*.rs` + SQL)**
```sql
CREATE TABLE vocabulary_registry (
    vocab_id UUID PRIMARY KEY,
    verb_name VARCHAR(100) UNIQUE,
    domain VARCHAR(50),
    signature TEXT,           -- :entity-type STRING :name STRING
    parameter_schema JSONB,   -- JSON Schema for validation
    examples JSONB,           -- For RAG retrieval
    usage_count INTEGER,      -- Usage tracking
    ...
);
```
- Designed for RAG context retrieval
- Has rich metadata: signatures, examples, parameter schemas
- **MARKED AS DEAD CODE** - `#[allow(dead_code)]` everywhere

#### RuntimeEnv (env.rs) - Execution Context
```rust
pub struct RuntimeEnv {
    pub request_id: OnboardingRequestId,
    pub cbu_id: Option<Uuid>,
    pub entity_id: Option<Uuid>,
    pub attribute_cache: HashMap<AttributeId, Value>,
    pub pending_crud: Vec<CrudStatement>,  // <-- KEY: CRUD output buffer
    pub cbu_model: Option<CbuModel>,       // State machine validation
    pub cbu_state: Option<String>,
    pub sink_attributes: HashSet<Uuid>,
    pub source_attributes: HashSet<Uuid>,
    // ...
}
```
Key methods:
- `push_crud(stmt)` - Words emit CrudStatements here
- `take_pending_crud()` - Executor drains and processes
- `set_cbu_model()` / `is_valid_transition()` - State machine validation

#### Word → CrudStatement Flow
```
DSL: (cbu.create :cbu-name "Test" :client-type "Corp")
  ↓
VM calls word_cbu_create()
  ↓
collect_keyword_pairs() from stack
  ↓
vm.env.push_crud(CrudStatement::DataCreate {
    asset: "CBU",
    values: { "cbu-name": "Test", "client-type": "Corp" }
})
```

#### STRENGTHS
1. **Clean separation**: Words emit abstract CrudStatements, not raw SQL
2. **State machine validation**: CBU Model can enforce valid transitions
3. **Source/Sink tracking**: Attributes tagged for data flow analysis
4. **Extensible word pattern**: `typed_word()` helper for consistent implementation
5. **DB vocabulary has rich metadata**: Signatures, examples, JSON schemas for RAG

#### ISSUES FOUND

**CRITICAL: Two vocabulary systems are disconnected**
- Rust `kyc_orch_vocab()` is the ONLY vocab used at runtime
- DB `vocabulary_registry` is seeded but NEVER QUERIED
- No bridge to load DB vocab into Rust `Vocab` struct
- RAG context system can't leverage the rich DB metadata

**CRITICAL: Rust vocab lacks metadata for RAG**
- No signatures (what args does each word expect?)
- No examples (what does valid DSL look like?)
- No parameter schemas (how to validate before parse?)
- Only has: name, domain, stack_effect, impl_fn

**MEDIUM: Vocabulary registry is entirely dead code**
```rust
#[allow(dead_code)]
pub(crate) fn register_verb(...) { ... }
```
Entire `vocabulary/` module is unused.

**MEDIUM: Word implementations have duplicated patterns**
Every word does:
```rust
let pairs = collect_keyword_pairs(vm, N)?;
process_pairs(vm, &pairs);
let values = pairs.into_iter().map(|(k,v)| { ... }).collect();
vm.env.push_crud(CrudStatement::DataCreate { ... });
```
Should be abstracted into a macro or builder.

**LOW: asset names are inconsistent**
- `"CBU"` vs `"Product"` vs `"DOCUMENT"` vs `"LifecycleResource"`
- Should match table names exactly or have explicit mapping

**Refactoring TODOs:**

1. **[P1] Bridge DB vocab to Rust runtime** - Load vocabulary_registry at startup
   ```rust
   impl Vocab {
       pub async fn from_database(pool: &PgPool) -> Result<Self, Error> {
           let rows = sqlx::query_as!(VocabRow, "SELECT * FROM vocabulary_registry WHERE is_active")
               .fetch_all(pool).await?;
           // Build WordSpec from each row
           // Generate impl_fn dynamically or map to known implementations
       }
   }
   ```

2. **[P1] Add metadata to WordSpec for RAG**
   ```rust
   pub struct WordSpec {
       pub id: WordId,
       pub name: String,
       pub domain: String,
       pub stack_effect: (usize, usize),
       pub impl_fn: WordImpl,
       // NEW:
       pub signature: String,           // ":cbu-name STRING :client-type STRING"
       pub parameter_schema: JsonValue, // JSON Schema
       pub examples: Vec<String>,       // Example DSL snippets
   }
   ```

3. **[P2] Create word implementation macro**
   ```rust
   crud_word!(cbu_create, "CBU", DataCreate, 5);  // asset, op, num_pairs
   ```

4. **[P2] Normalize asset names** - Create enum or constant mapping
   ```rust
   pub enum Asset { Cbu, Product, Service, LifecycleResource, Document, ... }
   ```

5. **[P3] Remove dead code or implement vocab registry integration**

---

### CHUNK 4: DSL.CRUD Vocabulary Definition & Executor
**Files:** `taxonomy/crud_operations.rs`, `taxonomy/crud_ast.rs`, `taxonomy/dsl_parser.rs`, `database/crud_executor.rs`
**Time Est:** 1 session
**Status:** ✅ COMPLETE

**Review Goals:**
- [x] Document all CRUD verbs (create, read, update, delete variants)
- [x] Map verb signatures to expected arguments
- [x] Check AST nodes for CRUD operations
- [x] Verify DSL parser handles CRUD syntax
- [x] Trace CrudStatement → SQLX execution

**Findings:**

#### Three-Layer CRUD Architecture

```
 Layer 1: Forth VM Words (kyc_vocab.rs)
   │ cbu.create, product.read, etc.
   │ Emit CrudStatement to RuntimeEnv.pending_crud
   ▼
 Layer 2: CrudStatement IR (value.rs)
   │ DataCreate { asset, values }
   │ DataRead { asset, where_clause, select, limit }
   │ DataUpdate { asset, where_clause, values }
   │ DataDelete { asset, where_clause }
   ▼
 Layer 3: CrudExecutor (crud_executor.rs)
   │ Routes by asset type to domain services
   │ CbuService, ProductService, EntityService, etc.
   ▼
 Layer 4: Domain Services (database/*.rs)
   │ SQLX queries, transactions, audit logging
   ▼
 PostgreSQL
```

#### TaxonomyCrudStatement - Alternative AST (taxonomy/crud_ast.rs)
**PARALLEL SYSTEM** to Forth VM's CrudStatement:
```rust
enum TaxonomyCrudStatement {
    CreateProduct(CreateProduct),
    ReadProduct(ReadProduct),
    // ... 20+ variants
    QueryWorkflow(QueryWorkflow),
    GenerateCompleteDsl(GenerateCompleteDsl),
}
```
Used by `TaxonomyDslParser` and `TaxonomyCrudOperations` - **separate from Forth engine**.

#### TaxonomyDslParser (taxonomy/dsl_parser.rs)
**Dual-mode parser:**
1. **S-expression mode**: `(product.create :code "X" :name "Y")` → Uses NomDslParser
2. **Natural language mode**: "Create product X called Y" → Regex extraction

Maps to TaxonomyCrudStatement, NOT Forth VM execution.

#### CrudExecutor - The Real Bridge (crud_executor.rs)
**This is the critical integration point:**
```rust
impl CrudExecutor {
    pub async fn execute(&self, stmt: &CrudStatement) -> Result<CrudExecutionResult> {
        match stmt {
            CrudStatement::DataCreate(create) => self.execute_create(create).await,
            // ...
        }
    }
    
    async fn execute_create(&self, create: &DataCreate) -> Result<...> {
        match create.asset.as_str() {
            "CBU" => { self.cbu_service.create_cbu(...).await }
            "Product" => { self.product_service.create_product(...).await }
            "DOCUMENT" => { self.document_service.create_document(...).await }
            // ... 10+ asset types
        }
    }
}
```

**Key features:**
- State machine validation via `execute_all_with_env()`
- Model-aware value splitting for CBU attributes
- Source tracking for audit trail
- Supports: CBU, ENTITY, PROPER_PERSON, DOCUMENT, DOCUMENT_METADATA, Product, Service, LifecycleResource

#### STRENGTHS
1. **Clean IR design** - CrudStatement is a minimal, serializable representation
2. **Service delegation** - Executor doesn't embed SQL, routes to services
3. **State validation** - Can check CBU Model before executing transitions
4. **Attribute splitting** - Separates core fields from attribute_values
5. **Audit logging** - TaxonomyCrudOperations logs all operations
6. **Flexible asset routing** - Easy to add new entity types

#### ISSUES FOUND

**CRITICAL: Two parallel CRUD systems don't integrate**
- `TaxonomyCrudStatement` (taxonomy module) vs `CrudStatement` (forth_engine)
- `TaxonomyDslParser` → `TaxonomyCrudStatement` → `TaxonomyCrudOperations`
- `NomDslParser` → `Forth VM` → `CrudStatement` → `CrudExecutor`
- These are completely separate execution paths!

**CRITICAL: TaxonomyDslParser natural language is brittle**
```rust
fn identify_operation(input: &str) -> Operation {
    if input.contains("create") { ... }  // Regex-based, no ML
}
```
This is NOT the agentic RAG approach - it's keyword matching.

**MEDIUM: Asset name inconsistency across layers**
- Forth words emit: `"CBU"`, `"Product"`, `"DOCUMENT"`
- Executor handles: `"CBU"`, `"Product" | "PRODUCT"`, `"DOCUMENT"`
- Some case-insensitive, some not

**MEDIUM: Missing CRUD operations**
- No LIST operation in CrudStatement (only READ with optional limit)
- No UPSERT operation
- No batch operations

**LOW: Dead code in TaxonomyDslParser**
```rust
fn parse_service_create_dsl(_parts: &[&str]) -> Result<...> {
    Err(anyhow!("Service create DSL parsing not yet implemented"))
}
```

**Refactoring TODOs:**

1. **[P1] Unify CRUD systems** - Choose one path:
   - Option A: Remove TaxonomyCrudStatement, use only Forth VM path
   - Option B: Make TaxonomyDslParser emit CrudStatement for executor
   - Option C: Add adapter between the two

2. **[P1] Replace NL parser with agentic approach** - TaxonomyDslParser's
   regex matching should be replaced with LLM + RAG:
   ```rust
   // Instead of:
   fn identify_operation(input: &str) -> Operation {
       if input.contains("create") { ... }
   }
   // Use:
   async fn generate_dsl(prompt: &str, rag_context: &RagContext) -> String {
       // LLM generates valid DSL from prompt + context
   }
   ```

3. **[P2] Normalize asset names** - Create canonical enum:
   ```rust
   pub enum AssetType {
       Cbu, Entity, ProperPerson, Document, Product, Service, LifecycleResource
   }
   impl AssetType {
       fn from_str(s: &str) -> Option<Self> { ... }  // case-insensitive
   }
   ```

4. **[P2] Add batch CRUD operations** - For efficiency:
   ```rust
   CrudStatement::DataCreateBatch { asset, values: Vec<HashMap> }
   ```

5. **[P3] Remove dead NL parser stubs** or implement them

---

### CHUNK 5: CRUD Executor → SQLX Integration
**Files:** `database/crud_executor.rs`, `database/crud_service.rs`
**Time Est:** 1 session
**Status:** ⬜ NOT STARTED

**Review Goals:**
- [ ] Trace from DSL.CRUD verb → executor → SQLX query
- [ ] Document transaction handling
- [ ] Check error propagation from DB to DSL runtime
- [ ] Verify all CRUD verbs have executor implementations

**Findings:**
```
(to be updated)
```

**Refactoring TODOs:**
```
(to be updated)
```

---

### CHUNK 6: Entity-Specific CRUD Services
**Files:** `database/cbu_service.rs`, `database/entity_service.rs`, `database/product_service.rs`, `database/service_service.rs`, `database/lifecycle_resource_service.rs`
**Time Est:** 1 session
**Status:** ⬜ NOT STARTED

**Review Goals:**
- [ ] Document CRUD operations per entity type
- [ ] Check consistency of service interfaces
- [ ] Verify SQLX query patterns
- [ ] Note any missing operations

**Findings:**
```
(to be updated)
```

**Refactoring TODOs:**
```
(to be updated)
```

---

### CHUNK 7: DSL Source - Template Generator
**Files:** `dsl_source/generator/*.rs`, `dsl_source/generator/domains/*.rs`
**Time Est:** 1 session
**Status:** ⬜ NOT STARTED

**Review Goals:**
- [ ] Document template generation approach
- [ ] Map domain generators (cbu, product, service, lifecycle_resource)
- [ ] Check template → valid DSL output
- [ ] Note coverage gaps

**Findings:**
```
(to be updated)
```

**Refactoring TODOs:**
```
(to be updated)
```

---

### CHUNK 8: DSL Source - Agentic/RAG (Partial Implementation)
**Files:** `dsl_source/agentic/*.rs`
**Time Est:** 1 session
**Status:** ⬜ NOT STARTED

**Review Goals:**
- [ ] Document current RAG context implementation status
- [ ] Check LLM generator scaffolding
- [ ] Identify missing components for deterministic DSL generation
- [ ] Map to AGENTIC_DSL_PLAN.md requirements

**Findings:**
```
(to be updated)
```

**Refactoring TODOs:**
```
(to be updated)
```

---

### CHUNK 9: Context & Validation Pipeline
**Files:** `dsl_source/context/*.rs`, `dsl_source/validation/*.rs`
**Time Est:** 1 session
**Status:** ⬜ NOT STARTED

**Review Goals:**
- [ ] Document vocabulary context retrieval
- [ ] Check attribute context integration
- [ ] Trace validation pipeline stages
- [ ] Verify pre-parse validation catches common errors

**Findings:**
```
(to be updated)
```

**Refactoring TODOs:**
```
(to be updated)
```

---

### CHUNK 10: End-to-End Integration Trace
**Files:** E2E test binaries, `services/agentic_*.rs`
**Time Est:** 1 session
**Status:** ⬜ NOT STARTED

**Review Goals:**
- [ ] Trace: User prompt → RAG context → Template/LLM → DSL source → Parser → Compiler → VM → CRUD Executor → SQLX → DB
- [ ] Document integration points
- [ ] Identify gaps in the pipeline
- [ ] Check error handling across boundaries

**Findings:**
```
(to be updated)
```

**Refactoring TODOs:**
```
(to be updated)
```

---

## SQL Schema Reference

**Key Migrations for CRUD:**
- `012_taxonomy_crud_support.sql` - CRUD operation support
- `013_vocabulary_registry.sql` - Vocabulary persistence
- `014_enhance_dsl_instances.sql` - DSL instance tracking
- `015_seed_vocabulary.sql` - Vocabulary seed data

**Master Schema:** `sql/00_MASTER_SCHEMA_CONSOLIDATED.sql`

---

## Summary Findings

### Critical Issues
```
(to be updated after review chunks complete)
```

### Architecture Gaps
```
(to be updated after review chunks complete)
```

### Recommended Refactoring Priority
```
(to be updated after review chunks complete)
```

---

## Session Log

| Date | Chunk | Status | Notes |
|------|-------|--------|-------|
| 2025-11-25 | Plan Created | ✅ | Initial plan with 10 chunks |
| 2025-11-25 | Chunk 1 | ✅ | Parser review complete. Critical: no span tracking. Medium: EBNF drift. |
| 2025-11-25 | Chunk 2 | ✅ | Compiler+VM complete. S-expr/postfix NOT an issue. Critical: LitList bug. Medium: Bool→Int, hardcoded WordIds. |
| 2025-11-25 | Chunk 3 | ✅ | Vocab system complete. **CRITICAL: Two disconnected vocab systems** - Rust hardcoded vs DB registry. DB vocab is dead code. |
| 2025-11-25 | Chunk 4 | ✅ | CRUD systems complete. **CRITICAL: Two parallel CRUD paths** (TaxonomyCrudStatement vs CrudStatement). TaxonomyDslParser NL is regex, not agentic. |

---

## Quick Reference Commands

**Resume Review:**
```
Read DSL_REVIEW_PLAN.md, find first ⬜ NOT STARTED chunk, execute review
```

**After Session:**
```
Update chunk status to ✅ COMPLETE, add findings, add refactoring TODOs
```
