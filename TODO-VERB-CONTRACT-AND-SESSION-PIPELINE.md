# TODO: Verb Contract Layer + Unified Session Pipeline

## Overview

Two related problems exist that create "side doors" in the execution pipeline:

1. **Verb Definition Side Door**: YAML verb definitions are interpreted at runtime and could theoretically change behavior. Plans don't reference a deterministic artifact.

2. **Session State Side Door**: `ExecutionContext.current_selection` is a COPY of `ViewState.selection`. They can diverge. The `CustomOperation` trait doesn't receive `UnifiedSessionContext`.

This TODO addresses BOTH to create a truly unified pipeline.

---

## Part A: Verb Contract Layer (YAML → CompiledVerbSchema → Plan/Execute)

### Goal

Insert an explicit, versioned contract artifact between YAML verb definitions and runtime execution:
- Parse + canonicalize YAML
- Compile YAML → deterministic `CompiledVerbSchema` (+ diagnostics)
- Persist compiled contracts keyed by `(verb_name, schema_version, yaml_hash)`
- Planner emits plans that reference `contract_id`
- Executor executes strictly via compiled contracts (no YAML dependency)

This makes behavior reproducible, auditable, and debuggable.

---

### Step A.1 — DB Schema (goose migrations)

**File:** `migrations/20260104_0001_dsl_compiled_verbs_up.sql`

```sql
-- +goose Up
-- Contract artifacts for verb YAML compilation.

CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE IF NOT EXISTS dsl_compiled_verbs (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

  verb_name TEXT NOT NULL,
  schema_version INTEGER NOT NULL,

  -- SHA-256 of canonicalized YAML structure (not raw YAML text)
  yaml_hash BYTEA NOT NULL,

  -- SHA-256 of compiled contract json bytes
  compiled_hash BYTEA NOT NULL,

  -- Compiled contract artifact (normalized, runnable semantics)
  compiled_json JSONB NOT NULL,

  -- Diagnostics emitted by compiler (errors/warnings)
  diagnostics_json JSONB NOT NULL,

  -- Fully expanded defaults + resolved configuration (for debugging)
  effective_config_json JSONB NOT NULL,

  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_dsl_compiled_verbs_key
  ON dsl_compiled_verbs (verb_name, schema_version, yaml_hash);

CREATE INDEX IF NOT EXISTS ix_dsl_compiled_verbs_yaml_hash
  ON dsl_compiled_verbs (yaml_hash);
```

**File:** `migrations/20260104_0001_dsl_compiled_verbs_down.sql`

```sql
-- +goose Down
DROP TABLE IF EXISTS dsl_compiled_verbs;
```

**Acceptance:**
- [ ] Migration runs cleanly
- [ ] Unique constraint prevents duplicates
- [ ] Can query by `(verb_name, schema_version, yaml_hash)`

---

### Step A.2 — Rust Types

**File:** `rust/src/verb_contract/types.rs`

```rust
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type Sha256 = [u8; 32];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledVerbSchema {
    // Identity
    pub contract_id: Uuid,
    pub verb_name: String,
    pub schema_version: u32,
    pub yaml_hash: Sha256,
    pub compiled_hash: Sha256,

    // Normalized runnable semantics
    pub params: Vec<CompiledParam>,
    pub binding: BindingRule,
    pub execution: ExecutionTarget,
    pub outputs: OutputContract,

    // Debug/audit
    pub effective_config: serde_json::Value,
    pub diagnostics: Diagnostics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledParam {
    pub name: String,
    pub ty: ParamType,
    pub cardinality: Cardinality,
    pub default_value: Option<serde_json::Value>,
    pub coercions: Vec<CoercionRule>,
    pub docs: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Cardinality {
    ZeroOrOne,
    ExactlyOne,
    ZeroOrMany,
    OneOrMany,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParamType {
    String,
    Bool,
    I64,
    F64,
    Uuid,
    AttributeId,
    EntityRef,
    Json,
    Enum { values: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CoercionRule {
    StringToUuid,
    StringToAttributeId,
    StringToEnum { values: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindingRule {
    pub requires_entity_resolution: bool,
    pub search_key_template: Option<String>,
    pub min_confidence: Option<f32>,
    pub tier_policy: Option<TierPolicy>,
    /// Session scope requirement (Universe, Book, Cbu, etc.)
    pub scope_requirement: Option<ScopeRequirement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScopeRequirement {
    /// Verb operates on universe (all accessible CBUs)
    Universe,
    /// Verb requires a client/book scope
    Book { client_binding: String },
    /// Verb requires a single CBU scope
    Cbu { cbu_binding: String },
    /// Verb operates on current selection
    Selection,
    /// No scope requirement
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierPolicy {
    pub preferred_tier: Option<String>,
    pub allow_fallback: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionTarget {
    DbSql { statement_name: String },
    RpcCall { service: String, method: String },
    InternalFn { function: String },
    /// Custom operation (plugin)
    Plugin { handler: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputContract {
    pub return_type: Option<ParamType>,
    pub exports: Vec<ExportedValue>,
    /// Does this verb modify session view state?
    pub modifies_view: bool,
    /// Does this verb modify session selection?
    pub modifies_selection: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedValue {
    pub name: String,
    pub ty: ParamType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostics {
    pub errors: Vec<Diagnostic>,
    pub warnings: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    pub code: String,
    pub message: String,
    pub path: String,
    pub hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiagnosticLevel {
    Error,
    Warning,
}
```

**Acceptance:**
- [ ] Types serialize/deserialize cleanly (JSONB)
- [ ] `ScopeRequirement` captures session scope semantics
- [ ] `OutputContract.modifies_view/modifies_selection` flags session-modifying verbs

---

### Step A.3 — Canonical YAML Hash

**File:** `rust/src/verb_contract/canonical.rs`

```rust
use sha2::{Digest, Sha256};
use serde_json::Value as J;
use serde_yaml::Value as Y;

pub fn yaml_hash(yaml: &Y) -> [u8; 32] {
    let canonical_json = yaml_to_canonical_json(yaml);
    let bytes = canonical_bytes(&canonical_json);
    sha256(&bytes)
}

pub fn yaml_to_canonical_json(yaml: &Y) -> J {
    let json: J = serde_json::to_value(yaml).expect("yaml->json");
    normalize_json(json)
}

fn normalize_json(v: J) -> J {
    match v {
        J::Object(map) => {
            let mut keys: Vec<_> = map.keys().cloned().collect();
            keys.sort();
            let mut out = serde_json::Map::new();
            for k in keys {
                let child = map.get(&k).cloned().unwrap_or(J::Null);
                out.insert(k, normalize_json(child));
            }
            J::Object(out)
        }
        J::Array(arr) => J::Array(arr.into_iter().map(normalize_json).collect()),
        other => other,
    }
}

pub fn canonical_bytes(v: &J) -> Vec<u8> {
    serde_json::to_vec(v).expect("json->bytes")
}

pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}
```

**Acceptance:**
- [ ] Same YAML semantics (different whitespace/key order) → same hash
- [ ] Unit tests prove stability

---

### Step A.4 — Contract Compiler

**File:** `rust/src/verb_contract/compiler.rs`

```rust
use uuid::Uuid;
use serde_yaml::Value as Y;
use crate::verb_contract::types::*;
use crate::verb_contract::canonical;

pub const CONTRACT_SCHEMA_VERSION: u32 = 1;

pub fn compile_verb(verb_name: &str, yaml_def: &Y) -> CompiledVerbSchema {
    let yaml_hash = canonical::yaml_hash(yaml_def);
    let mut diagnostics = Diagnostics { errors: vec![], warnings: vec![] };

    // TODO: Parse yaml_def fields
    let effective_config = serde_json::to_value(yaml_def).unwrap_or(serde_json::Value::Null);

    // Extract params, binding, execution, outputs from YAML
    let params = compile_params(yaml_def, &mut diagnostics);
    let binding = compile_binding(yaml_def, &mut diagnostics);
    let execution = compile_execution(yaml_def, &mut diagnostics);
    let outputs = compile_outputs(yaml_def, &mut diagnostics);

    let temp_contract = CompiledVerbSchema {
        contract_id: Uuid::new_v4(),
        verb_name: verb_name.to_string(),
        schema_version: CONTRACT_SCHEMA_VERSION,
        yaml_hash,
        compiled_hash: [0u8; 32],
        params,
        binding,
        execution,
        outputs,
        effective_config,
        diagnostics,
    };

    let compiled_json = serde_json::to_value(&temp_contract).expect("contract->json");
    let compiled_bytes = canonical::canonical_bytes(&compiled_json);
    let compiled_hash = canonical::sha256(&compiled_bytes);

    CompiledVerbSchema { compiled_hash, ..temp_contract }
}

fn compile_params(yaml: &Y, diag: &mut Diagnostics) -> Vec<CompiledParam> {
    // TODO: Extract args from YAML, validate types, build CompiledParam
    vec![]
}

fn compile_binding(yaml: &Y, diag: &mut Diagnostics) -> BindingRule {
    // TODO: Extract lookup config, resolution mode, scope requirements
    BindingRule {
        requires_entity_resolution: false,
        search_key_template: None,
        min_confidence: None,
        tier_policy: None,
        scope_requirement: None,
    }
}

fn compile_execution(yaml: &Y, diag: &mut Diagnostics) -> ExecutionTarget {
    // TODO: Map behavior (crud/plugin/graph_query) to ExecutionTarget
    ExecutionTarget::InternalFn { function: "TODO".into() }
}

fn compile_outputs(yaml: &Y, diag: &mut Diagnostics) -> OutputContract {
    // TODO: Extract returns config, detect view-modifying verbs
    OutputContract {
        return_type: None,
        exports: vec![],
        modifies_view: false,
        modifies_selection: false,
    }
}
```

**Acceptance:**
- [ ] Compiles all existing verb YAMLs
- [ ] Produces meaningful diagnostics for invalid fixtures
- [ ] Detects view-modifying verbs (view.universe, view.book, etc.)

---

### Step A.5 — Repository Layer

**File:** `rust/src/verb_contract/repo.rs`

```rust
use sqlx::{PgPool, Row};
use uuid::Uuid;
use crate::verb_contract::types::*;

pub async fn get_compiled_verb(
    db: &PgPool,
    verb_name: &str,
    schema_version: i32,
    yaml_hash: &[u8],
) -> sqlx::Result<Option<CompiledVerbSchema>> {
    let row = sqlx::query(
        r#"
        SELECT id, compiled_json
        FROM dsl_compiled_verbs
        WHERE verb_name = $1 AND schema_version = $2 AND yaml_hash = $3
        "#,
    )
    .bind(verb_name)
    .bind(schema_version)
    .bind(yaml_hash)
    .fetch_optional(db)
    .await?;

    Ok(row.map(|r| {
        let json: serde_json::Value = r.get("compiled_json");
        serde_json::from_value(json).expect("valid contract")
    }))
}

pub async fn upsert_compiled_verb(
    db: &PgPool,
    contract: &CompiledVerbSchema,
) -> sqlx::Result<Uuid> {
    let compiled_json = serde_json::to_value(contract).expect("contract->json");
    let diagnostics_json = serde_json::to_value(&contract.diagnostics).expect("diag->json");
    let effective_config_json = contract.effective_config.clone();

    let row = sqlx::query(
        r#"
        INSERT INTO dsl_compiled_verbs
          (verb_name, schema_version, yaml_hash, compiled_hash, compiled_json, diagnostics_json, effective_config_json)
        VALUES
          ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (verb_name, schema_version, yaml_hash)
        DO UPDATE SET
          compiled_hash = EXCLUDED.compiled_hash,
          compiled_json = EXCLUDED.compiled_json,
          diagnostics_json = EXCLUDED.diagnostics_json,
          effective_config_json = EXCLUDED.effective_config_json
        RETURNING id
        "#,
    )
    .bind(&contract.verb_name)
    .bind(contract.schema_version as i32)
    .bind(&contract.yaml_hash[..])
    .bind(&contract.compiled_hash[..])
    .bind(&compiled_json)
    .bind(&diagnostics_json)
    .bind(&effective_config_json)
    .fetch_one(db)
    .await?;

    Ok(row.get("id"))
}
```

**Acceptance:**
- [ ] First boot compiles + inserts
- [ ] Subsequent boot finds in DB and skips compile (unless schema_version changes)

---

### Step A.6 — Planner References Contracts

**Changes to planner:**

```rust
// Plan step now includes contract reference
pub struct PlanStep {
    pub verb_contract_id: Uuid,
    pub verb_name: String,
    pub yaml_hash: [u8; 32],
    pub schema_version: u32,
    // ... existing fields
}
```

**Acceptance:**
- [ ] Serialized plan includes `verb_contract_id` per step
- [ ] Planner validates args against contract (cardinality, types, coercions)
- [ ] Planning failures are deterministic and diagnostic-rich

---

### Step A.7 — Executor Uses Contracts

Executor must not consult YAML:
- Load contract by `verb_contract_id`
- Dispatch via `ExecutionTarget`
- Apply coercions exactly as compiled
- Enforce binding rules

**Acceptance:**
- [ ] Execution works if YAML files are missing (contracts in DB)
- [ ] Historic plans can re-run as long as referenced contract rows exist

---

## Part B: Unified Session Pipeline (Close the State Side Door)

### Problem Statement

Currently:
- `CustomOperation::execute()` receives `ExecutionContext` only
- View operations create `ViewState` locally, copy selection to `ExecutionContext`, then discard ViewState
- `UnifiedSessionContext.view` is never set by view operations
- Visualization and REPL see different state

### Goal

Single source of truth:
```
UnifiedSessionContext
├── view: ViewState         ← THE canonical "it"
│   └── selection: Vec<Uuid>  ← THE canonical selection
└── execution: ExecutionContext
    └── (NO separate selection - derives from view)
```

---

### Step B.1 — Change CustomOperation Trait

**File:** `rust/src/dsl_v2/custom_ops/mod.rs`

```rust
/// Trait for custom operations
#[async_trait]
pub trait CustomOperation: Send + Sync {
    fn domain(&self) -> &'static str;
    fn verb(&self) -> &'static str;
    fn rationale(&self) -> &'static str;

    /// Does this operation modify session view state?
    fn modifies_view(&self) -> bool { false }

    /// Execute the custom operation
    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut OperationContext,  // ← NEW: unified context
        pool: &PgPool,
    ) -> Result<ExecutionResult>;
}

/// Unified operation context - provides access to session state
pub struct OperationContext<'a> {
    /// Execution context for DSL bindings
    pub exec: &'a mut ExecutionContext,
    /// Session context for view state (optional for non-view ops)
    pub session: Option<&'a mut UnifiedSessionContext>,
}

impl<'a> OperationContext<'a> {
    /// Get current view state (if session available)
    pub fn view(&self) -> Option<&ViewState> {
        self.session.as_ref().and_then(|s| s.view.as_ref())
    }

    /// Get mutable view state
    pub fn view_mut(&mut self) -> Option<&mut ViewState> {
        self.session.as_mut().and_then(|s| s.view.as_mut())
    }

    /// Set view state (for view.* operations)
    pub fn set_view(&mut self, view: ViewState) {
        if let Some(session) = &mut self.session {
            // Sync selection to execution context
            self.exec.current_selection = Some(view.selection.clone());
            session.view = Some(view);
        }
    }

    /// Get selection (from view, not from exec)
    pub fn selection(&self) -> &[Uuid] {
        self.view()
            .map(|v| v.selection.as_slice())
            .unwrap_or(&[])
    }
}
```

**Acceptance:**
- [ ] `OperationContext` provides unified access
- [ ] View operations use `set_view()` to store ViewState
- [ ] Selection always derives from ViewState

---

### Step B.2 — Update View Operations

**File:** `rust/src/dsl_v2/custom_ops/view_ops.rs`

```rust
impl CustomOperation for ViewUniverseOp {
    fn modifies_view(&self) -> bool { true }

    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut OperationContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Build taxonomy context from args
        let taxonomy_ctx = if let Some(client_id) = get_uuid_arg(verb_call, "client", ctx.exec) {
            TaxonomyContext::Book { client_id }
        } else {
            TaxonomyContext::Universe
        };

        // Build taxonomy from database
        let rules = taxonomy_ctx.to_rules();
        let taxonomy = TaxonomyBuilder::new(rules).build(pool).await?;

        // Create view state
        let mut view = ViewState::from_taxonomy(taxonomy, taxonomy_ctx);

        // Apply filters as refinements
        // ... existing filter logic ...

        // CRITICAL: Store view state in session (not just copy selection)
        ctx.set_view(view.clone());

        let result = ViewOpResult::from_view_state(&view);
        Ok(ExecutionResult::Record(serde_json::to_value(&result)?))
    }
}
```

**Acceptance:**
- [ ] View operations call `ctx.set_view()`
- [ ] ViewState is stored in UnifiedSessionContext
- [ ] Selection is synced automatically

---

### Step B.3 — Remove ExecutionContext.current_selection

After B.1 and B.2, `ExecutionContext.current_selection` becomes redundant. Either:

**Option A:** Keep as read-only cache (synced from ViewState)
```rust
impl ExecutionContext {
    /// Get selection (read-only, synced from session view)
    pub fn selection(&self) -> &[Uuid] {
        self.current_selection.as_deref().unwrap_or(&[])
    }
    
    // Remove set_selection() - only OperationContext.set_view() can modify
}
```

**Option B:** Remove entirely, always go through session
```rust
// ExecutionContext no longer has selection
// All selection access goes through OperationContext.selection()
```

**Acceptance:**
- [ ] No side door for selection modification
- [ ] Single source of truth in ViewState

---

### Step B.4 — Wire Executor to Session

**File:** `rust/src/dsl_v2/executor.rs`

```rust
impl DslExecutor {
    /// Execute with session context (for view-modifying operations)
    pub async fn execute_with_session(
        &self,
        program: &AstNode,
        exec_ctx: &mut ExecutionContext,
        session: &mut UnifiedSessionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let mut op_ctx = OperationContext {
            exec: exec_ctx,
            session: Some(session),
        };
        self.execute_internal(program, &mut op_ctx, pool).await
    }

    /// Execute without session context (standalone REPL)
    pub async fn execute(
        &self,
        program: &AstNode,
        exec_ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let mut op_ctx = OperationContext {
            exec: exec_ctx,
            session: None,
        };
        self.execute_internal(program, &mut op_ctx, pool).await
    }
}
```

**Acceptance:**
- [ ] UI/visualization path uses `execute_with_session()`
- [ ] Standalone REPL can still use `execute()` (no view state)
- [ ] Both paths use same internal execution logic

---

## Part C: Integration

### The Unified Pipeline

```
┌─────────────────────────────────────────────────────────────────────┐
│                         AGENT/UI/VOICE                              │
│                              │                                      │
│                              ▼                                      │
│                    "Show me Allianz book"                           │
└─────────────────────────────────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────────┐
│                         DSL PARSER                                  │
│                              │                                      │
│                              ▼                                      │
│               (view.book :client @allianz)                          │
└─────────────────────────────────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────────┐
│                         PLANNER                                     │
│                              │                                      │
│         Resolves: view.book → CompiledVerbSchema                    │
│         Validates: args against contract                            │
│         Emits: PlanStep with contract_id                            │
└─────────────────────────────────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────────┐
│                         EXECUTOR                                    │
│                              │                                      │
│         Loads: CompiledVerbSchema by contract_id                    │
│         Creates: OperationContext with session                      │
│         Dispatches: ViewBookOp.execute()                            │
└─────────────────────────────────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      VIEW OPERATION                                 │
│                              │                                      │
│         Builds: TaxonomyBuilder.build(pool)                         │
│         Creates: ViewState::from_taxonomy()                         │
│         Stores: ctx.set_view(view) → UnifiedSessionContext          │
└─────────────────────────────────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────────┐
│               UNIFIED SESSION CONTEXT                               │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │ view: ViewState                                               │  │
│  │   ├── stack: TaxonomyStack (fractal zoom)                     │  │
│  │   ├── taxonomy: TaxonomyNode (current tree)                   │  │
│  │   ├── selection: Vec<Uuid>  ← SINGLE SOURCE OF TRUTH          │  │
│  │   ├── refinements: Vec<Refinement>                            │  │
│  │   └── pending: Option<PendingOperation>                       │  │
│  └───────────────────────────────────────────────────────────────┘  │
│  execution: ExecutionContext                                        │
│    └── (reads selection from view, no independent copy)             │
└─────────────────────────────────────────────────────────────────────┘
                               │
                ┌──────────────┴──────────────┐
                ▼                              ▼
┌───────────────────────────┐  ┌───────────────────────────┐
│      VISUALIZATION        │  │        DSL REPL           │
│                           │  │                           │
│  Reads: session.view      │  │  Reads: session.view      │
│  Renders: taxonomy        │  │  Binds: @_selection       │
│  Shows: selection         │  │  Operates: on selection   │
└───────────────────────────┘  └───────────────────────────┘
```

### Key Invariants

1. **One ViewState** - stored in `UnifiedSessionContext.view`
2. **One Selection** - `ViewState.selection` is canonical
3. **One Contract per Verb** - `CompiledVerbSchema` in DB
4. **One Pipeline** - all paths go through same executor

---

## Implementation Order

### Phase 1: Session Pipeline Fix (Part B)
1. [ ] B.1 - Create `OperationContext`
2. [ ] B.2 - Update view operations to use `set_view()`
3. [ ] B.3 - Clean up `ExecutionContext.current_selection`
4. [ ] B.4 - Wire executor to session

### Phase 2: Verb Contract Layer (Part A)
1. [ ] A.1 - DB migration
2. [ ] A.2 - Rust types
3. [ ] A.3 - Canonical hash
4. [ ] A.4 - Compiler
5. [ ] A.5 - Repository
6. [ ] A.6 - Planner integration
7. [ ] A.7 - Executor integration

### Phase 3: Integration Testing
1. [ ] Verify view.* verbs store ViewState
2. [ ] Verify REPL and visualization see same state
3. [ ] Verify plans reference contract_id
4. [ ] Verify execution reproducibility

---

## Files to Create/Modify

### New Files
| File | Purpose |
|------|---------|
| `migrations/20260104_0001_dsl_compiled_verbs_up.sql` | DB schema |
| `migrations/20260104_0001_dsl_compiled_verbs_down.sql` | Rollback |
| `rust/src/verb_contract/mod.rs` | Module root |
| `rust/src/verb_contract/types.rs` | CompiledVerbSchema types |
| `rust/src/verb_contract/canonical.rs` | YAML hashing |
| `rust/src/verb_contract/compiler.rs` | YAML → contract |
| `rust/src/verb_contract/repo.rs` | DB persistence |

### Modified Files
| File | Changes |
|------|---------|
| `rust/src/dsl_v2/custom_ops/mod.rs` | `OperationContext`, trait change |
| `rust/src/dsl_v2/custom_ops/view_ops.rs` | Use `ctx.set_view()` |
| `rust/src/dsl_v2/executor.rs` | `execute_with_session()`, contract lookup |
| `rust/src/dsl_v2/planning_facade.rs` | Contract resolution |
| `rust/src/session/mod.rs` | Integration points |

---

## Done Definition

- [ ] Every plan step references a compiled verb contract
- [ ] Executor runs solely from compiled contracts  
- [ ] View operations store ViewState in UnifiedSessionContext
- [ ] Selection has single source of truth (ViewState)
- [ ] REPL and visualization see identical state
- [ ] Can answer: "Which YAML produced this behavior?" via yaml_hash
- [ ] Can answer: "What is the current selection?" unambiguously
