# TODO: DSL Transactional Execution, Expansion, and Entity Locking

## Overview

This TODO implements true transactional execution for DSL plans, deterministic template expansion with audit trails, and entity-level advisory locking to prevent mid-batch race conditions.

**The Problem We're Solving:**
```
Session A: LINK person_123 → 50 CBUs (batch)
  T0-T20: Verbs 1-20 succeed (edges created)
  T21: Session B deletes person_123
  T22-T50: Verbs 21-50 fail (person deleted)
  Result: 20/50 partial state — inconsistent, confusing
```

**The Solution:**
- **Phase 1**: Transaction plumbing (execute entire plan in one tx)
- **Phase 2**: Expansion stage + audit trail (deterministic template expansion)
- **Phase 3**: Advisory locks (prevent concurrent modification)
- **Phase 4**: Error aggregation (sensible reporting of cascaded failures)

---

## Dependencies

```
Phase 1: TX Plumbing (FOUNDATION) ← START HERE
    │
    ▼
Phase 2: Expansion Stage ──────────────────┐
    │                                      │
    ▼                                      │
Phase 3: Advisory Locks ◄──────────────────┘
    │
    ▼
Phase 4: Error Aggregation (can be parallel with 2-3)
```

---

# PHASE 1: Transaction Plumbing

> **Scope**: Narrow. Execution path only.
> **Goal**: Execute N DSL statements in ONE real Postgres transaction (commit/rollback).
> **Prerequisite for**: Everything else.

## Current State (from repo snapshot)

- `rust/src/dsl_v2/executor.rs` begins a tx but `execute_verb_in_tx()` is TODO and currently calls pool path
- `rust/src/dsl_v2/generic_executor.rs` uses `&self.pool` directly for all DB operations
- There is `Arc<dyn CustomOperation>`; cannot make it generic over `Executor` without object-safety issues

## Guardrails (DO NOT)

- ❌ Do NOT refactor the entire database layer or every repository
- ❌ Do NOT change the DSL grammar
- ❌ Do NOT introduce generic `Executor` on `dyn CustomOperation` (object safety)
- ❌ Do NOT add new features unrelated to TX plumbing (locking comes in Phase 3)
- ❌ Do NOT "fallback to pool" from tx path — atomic execution must not silently run outside tx
- ✅ DO make minimal mechanical changes to make the existing "tx wrapper" real

---

## 1.1 Make GenericCrudExecutor Support Transactions

**File**: `rust/src/dsl_v2/generic_executor.rs`

**Current Issue**: 
Module calls `.execute(&self.pool)` / `.fetch_one(&self.pool)` directly.
Even if caller starts a tx, CRUD verbs still auto-commit per statement.

### 1.1.1 Add Internal Generic Implementation

Add an internal helper that works with any sqlx Executor:

```rust
/// Internal implementation that works with any Executor
async fn execute_with_exec<'e, E>(
    &self,
    exec: E,
    verb: &RuntimeVerb,
    args: &HashMap<String, serde_json::Value>,
) -> anyhow::Result<GenericExecutionResult>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres> + Copy,
{
    // Move existing execute() logic here
    // All DB calls use `exec` instead of `&self.pool`
}
```

### 1.1.2 Add Transaction Entrypoint

```rust
/// Execute verb within an existing transaction
pub async fn execute_in_tx(
    &self,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    verb: &RuntimeVerb,
    args: &HashMap<String, serde_json::Value>,
) -> anyhow::Result<GenericExecutionResult> {
    self.execute_with_exec(&mut **tx, verb, args).await
}
```

### 1.1.3 Refactor Existing execute() as Thin Wrapper

```rust
/// Execute verb with auto-commit (existing behavior preserved)
pub async fn execute(
    &self,
    verb: &RuntimeVerb,
    args: &HashMap<String, serde_json::Value>,
) -> anyhow::Result<GenericExecutionResult> {
    self.execute_with_exec(&self.pool, verb, args).await
}
```

### 1.1.4 Thread `exec` Through All DB-Touching Helpers

Find and refactor every place in `generic_executor.rs` that uses `&self.pool`:

| Helper Function | Change Required |
|-----------------|-----------------|
| `execute_with_bindings()` | Add `exec: E` parameter |
| `execute_with_bindings_multi()` | Add `exec: E` parameter |
| `execute_non_query()` | Add `exec: E` parameter |
| `resolve_lookup()` | Add `exec: E` parameter |
| Any `fetch_one` / `fetch_optional` / `fetch_all` | Use `exec` not `&self.pool` |
| Entity type ID lookups | Use `exec` |

**Pattern replacement:**
```rust
// BEFORE
.fetch_one(&self.pool)
.fetch_optional(&self.pool)
.execute(&self.pool)

// AFTER
.fetch_one(exec)
.fetch_optional(exec)
.execute(exec)
```

### 1.1.5 Handle Borrow Issues

You may need `exec: E` to be `Copy`. If that causes trouble:
- Restructure by passing `&mut *tx` or `&self.pool` directly
- Thread `exec` as needed
- Keep it minimal and consistent

### Acceptance Criteria 1.1
- [ ] `generic_executor.rs` has no remaining `.execute(&self.pool)` on write paths
- [ ] `generic_executor.rs` has no remaining `.fetch_*(&self.pool)` on paths used by CRUD verbs
- [ ] `execute()` preserves existing auto-commit behavior
- [ ] `execute_in_tx()` uses provided transaction
- [ ] Code compiles without borrow checker errors

---

## 1.2 Implement execute_verb_in_tx() in DSL Executor

**File**: `rust/src/dsl_v2/executor.rs`

**Current Issue**: 
`execute_verb_in_tx()` is defined but ignores `tx` and calls `execute_verb()` which uses pool path.

### 1.2.1 Implement Transactional Path for Generic CRUD

In `execute_verb_in_tx(vc, ctx, tx)`:

```rust
pub async fn execute_verb_in_tx(
    &self,
    vc: &VerbCall,
    ctx: &mut ExecutionContext,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> anyhow::Result<ExecutionResult> {
    // 1. Same verb resolution as execute_verb()
    let runtime_verb = self.find_runtime_verb(&vc.domain, &vc.verb)?;
    
    // 2. Same argument building as execute_verb()
    let json_args = self.build_json_args(vc, ctx)?;
    
    // 3. Check if custom operation
    if let Some(op) = self.find_custom_op(&vc.domain, &vc.verb) {
        // Route to custom op's tx method (see 1.3)
        return op.execute_in_tx(vc, ctx, tx).await;
    }
    
    // 4. Generic CRUD path - use tx
    let result = self.generic_executor
        .execute_in_tx(tx, &runtime_verb, &json_args)
        .await?;
    
    // 5. Preserve returns.capture behavior (same as execute_verb)
    self.capture_returns(ctx, &result)?;
    
    Ok(result.into())
}
```

### 1.2.2 Key Points

- Use same verb resolution and argument-building logic as `execute_verb()`
- If verb resolves to generic CRUD → call `self.generic_executor.execute_in_tx(tx, ...)`
- Preserve existing "returns.capture" behavior exactly as in `execute_verb()`
- **DO NOT** call `execute_verb()` internally — that defeats the purpose

### Acceptance Criteria 1.2
- [ ] `execute_verb_in_tx()` no longer calls `execute_verb()` internally
- [ ] Generic CRUD verbs execute within provided transaction
- [ ] Custom operations route to `execute_in_tx()` trait method
- [ ] Return value capture works identically to pool path

---

## 1.3 Extend CustomOperation Trait

**File**: `rust/src/dsl_v2/custom_ops/mod.rs` (or wherever `CustomOperation` trait is defined)

### 1.3.1 Add Default Transactional Method

```rust
#[async_trait]
pub trait CustomOperation: Send + Sync {
    // Existing methods unchanged...
    fn domain(&self) -> &str;
    fn verb(&self) -> &str;
    
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> anyhow::Result<ExecutionResult>;
    
    /// Execute within a transaction. Default returns error.
    /// Custom ops must explicitly implement to support atomic execution.
    async fn execute_in_tx(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> anyhow::Result<ExecutionResult> {
        anyhow::bail!(
            "Custom operation {}.{} does not support transactional execution. \
             Implement execute_in_tx() to enable atomic batch execution.",
            self.domain(),
            self.verb()
        )
    }
}
```

### 1.3.2 Route Custom Ops Through TX Path

In `execute_verb_in_tx()`, when verb is custom operation:
- Call `op.execute_in_tx(vc, ctx, tx).await`
- If default impl returns error ("does not support transactional execution"), **let it propagate**
- **DO NOT** fallback to pool path from tx path — atomic execution must not silently auto-commit

### Acceptance Criteria 1.3
- [ ] `CustomOperation` trait has `execute_in_tx()` with default error impl
- [ ] Atomic execution fails clearly for unsupported custom ops
- [ ] No silent fallback to auto-commit from transactional path

---

## 1.4 Add Atomic Plan Execution Entrypoint

**File**: `rust/src/dsl_v2/executor.rs`

### 1.4.1 Implement execute_plan_atomic()

```rust
/// Execute entire plan in a single transaction (all-or-nothing)
pub async fn execute_plan_atomic(
    &self,
    plan: &ExecutionPlan,
    ctx: &mut ExecutionContext,
) -> anyhow::Result<AtomicExecutionResult> {
    // 1. Begin transaction
    let mut tx = self.pool.begin().await?;
    
    // 2. (Placeholder for future advisory locks - DO NOT implement yet)
    // self.acquire_locks(&mut tx, &plan.derived_locks).await?;
    
    // 3. Execute all steps in transaction
    let mut step_results = Vec::with_capacity(plan.steps.len());
    
    for (index, step) in plan.steps.iter().enumerate() {
        match self.execute_verb_in_tx(&step.verb_call, ctx, &mut tx).await {
            Ok(result) => {
                step_results.push(StepResult::Success { index, result });
            }
            Err(error) => {
                // 4a. Any failure → rollback everything
                tx.rollback().await?;
                
                return Ok(AtomicExecutionResult::RolledBack {
                    failed_at_step: index,
                    error: error.to_string(),
                    completed_steps: step_results,
                });
            }
        }
    }
    
    // 4b. All succeeded → commit
    tx.commit().await?;
    
    Ok(AtomicExecutionResult::Committed {
        step_results,
    })
}
```

### 1.4.2 Result Types

```rust
#[derive(Debug)]
pub enum AtomicExecutionResult {
    Committed {
        step_results: Vec<StepResult>,
    },
    RolledBack {
        failed_at_step: usize,
        error: String,
        completed_steps: Vec<StepResult>,
    },
}

#[derive(Debug)]
pub struct StepResult {
    pub index: usize,
    pub result: ExecutionResult,
}
```

### 1.4.3 Preserve Existing execute_plan()

**Do not modify `execute_plan()`**. It remains the auto-commit (best-effort) path for backward compatibility.

### 1.4.4 Handle Idempotency Logic

Handle idempotency logic the same way as current plan execution, but ensure DB writes use the tx path.

### Acceptance Criteria 1.4
- [ ] `execute_plan_atomic()` begins transaction before first step
- [ ] All steps execute within same transaction
- [ ] Any step failure triggers rollback of ALL prior steps
- [ ] Success commits entire batch
- [ ] `execute_plan()` unchanged (backward compatible)
- [ ] Idempotency logic preserved

---

## 1.5 Regression Test: Prove Rollback Works

**File**: `rust/tests/tx_atomic_rollback.rs` (or under existing test module conventions)

### Test Implementation

```rust
#[tokio::test]
async fn test_atomic_rollback_on_failure() {
    // Setup: get executor and clean test state
    let executor = setup_test_executor().await;
    let mut ctx = ExecutionContext::new();
    
    // Build plan with TWO steps:
    // Step 1: Valid CRUD verb that INSERTs something
    // Step 2: Invalid verb (bad domain/verb OR guaranteed-to-fail args)
    let plan = ExecutionPlan {
        steps: vec![
            // Step 1: Insert a test entity (will succeed)
            PlanStep {
                verb_call: VerbCall {
                    domain: "entity".into(),
                    verb: "create".into(),
                    args: vec![
                        ("name".into(), Value::String("test_rollback_entity".into())),
                        ("type".into(), Value::String("test".into())),
                    ],
                },
            },
            // Step 2: This MUST fail (nonexistent domain/verb)
            PlanStep {
                verb_call: VerbCall {
                    domain: "nonexistent".into(),
                    verb: "will_fail".into(),
                    args: vec![],
                },
            },
        ],
    };
    
    // Execute atomically
    let result = executor.execute_plan_atomic(&plan, &mut ctx).await;
    
    // Assert: execution returned RolledBack (or Err)
    match result {
        Ok(AtomicExecutionResult::RolledBack { failed_at_step, .. }) => {
            assert_eq!(failed_at_step, 1, "Should fail at step 1 (0-indexed)");
        }
        Ok(AtomicExecutionResult::Committed { .. }) => {
            panic!("Should not have committed - step 2 should fail");
        }
        Err(e) => {
            // Also acceptable - error propagated
        }
    }
    
    // Assert: Step 1's insert was ROLLED BACK
    let entity_exists = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM entities WHERE name = 'test_rollback_entity')"
    )
    .fetch_one(&executor.pool)
    .await
    .unwrap();
    
    assert_eq!(entity_exists, Some(false), 
        "Entity from step 1 should NOT exist after rollback");
}
```

### Test Behavior

| Condition | Expected |
|-----------|----------|
| Before this refactor | Test FAILS (step 1 committed despite step 2 failure) |
| After this refactor | Test PASSES (step 1 rolled back with step 2) |

### Acceptance Criteria 1.5
- [ ] Test exists and is runnable
- [ ] Test FAILS on current codebase (proves the problem exists)
- [ ] Test PASSES after Phase 1 complete (proves fix works)

---

## Phase 1 Validation Checklist

Before moving to Phase 2:

- [ ] `cargo test` passes (all existing tests)
- [ ] `execute_verb_in_tx()` no longer calls `execute_verb()` internally
- [ ] `generic_executor.rs` has no `.execute(&self.pool)` on CRUD write paths
- [ ] `generic_executor.rs` has no `.fetch_*(&self.pool)` on CRUD paths
- [ ] Regression test proves rollback works
- [ ] No new DSL syntax added
- [ ] No broad database repository refactor
- [ ] `execute_plan()` unchanged (backward compatibility)

---

# PHASE 2: Deterministic Template Expansion

> **Prerequisite**: Phase 1 complete
> **Scope**: Add expansion stage that compiles templates to atomic s-expressions
> **Goal**: Deterministic, auditable expansion with ExpansionReport

## Guardrails

- ❌ Do NOT add new s-expression grammar constructs
- ❌ Do NOT make expansion depend on DB state (must be pure)
- ✅ DO preserve existing execution semantics unless metadata opts into atomic

---

## 2.1 Create Expansion Module

**Files to create**:
- `rust/src/dsl_v2/expansion/mod.rs`
- `rust/src/dsl_v2/expansion/types.rs`
- `rust/src/dsl_v2/expansion/engine.rs`

### 2.1.1 Expansion Types (types.rs)

```rust
use serde::{Serialize, Deserialize};
use uuid::Uuid;

/// Complete record of template expansion for audit/replay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpansionReport {
    pub expansion_id: Uuid,
    
    /// Hash of pre-expanded DSL (canonical)
    pub source_digest: String,
    
    /// Hashes of each template definition used
    pub template_digests: Vec<TemplateDigest>,
    
    /// Details of each template invocation
    pub invocations: Vec<TemplateInvocationReport>,
    
    /// Total statements after expansion
    pub expanded_statement_count: usize,
    
    /// Hash of expanded DSL (canonical)
    pub expanded_dsl_digest: String,
    
    /// Locks inferred from metadata + args
    pub derived_lock_set: Vec<LockKey>,
    
    /// Batch policy (atomic | best_effort)
    pub batch_policy: BatchPolicy,
    
    /// Warnings and errors during expansion
    pub diagnostics: Vec<ExpansionDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateDigest {
    pub name: String,
    pub version: String,
    pub digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateInvocationReport {
    pub name: String,
    pub args_json: serde_json::Value,
    pub policy: TemplatePolicy,
    pub origin_span: Option<SpanRef>,
    pub expanded_range: ExpandedRange,
    
    /// Maps expanded statement index → template item index
    pub per_item_origins: Vec<PerItemOrigin>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpandedRange {
    pub start_index: usize,
    pub end_index_exclusive: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerItemOrigin {
    pub expanded_statement_index: usize,
    pub template_item_index: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum BatchPolicy {
    Atomic,
    #[default]
    BestEffort,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TemplatePolicy {
    pub batch_policy: BatchPolicy,
    pub locking: Option<LockingPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockingPolicy {
    pub mode: LockMode,
    pub timeout_ms: Option<u64>,
    pub targets: Vec<LockTarget>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum LockMode {
    Try,   // Non-blocking, fail fast if locked
    Block, // Wait for lock (with optional timeout)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockTarget {
    pub arg: String,        // arg name in verb call
    pub entity_type: String,
    pub access: LockAccess,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum LockAccess {
    Read,
    Write,
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct LockKey {
    pub entity_type: String,
    pub entity_id: String,
    pub access: LockAccess,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpansionDiagnostic {
    pub level: DiagnosticLevel,
    pub message: String,
    pub path: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DiagnosticLevel {
    Error,
    Warning,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanRef {
    pub file: Option<String>,
    pub start: usize,
    pub end: usize,
}
```

### 2.1.2 Expansion Engine (engine.rs)

```rust
use super::types::*;
use sha2::{Sha256, Digest};

pub struct ExpansionOutput {
    pub expanded_dsl: String,
    pub report: ExpansionReport,
}

/// Expand templates deterministically (PURE - no DB calls)
pub fn expand_templates(
    source_dsl: &str,
    template_registry: &TemplateRegistry,
) -> Result<ExpansionOutput, ExpansionError> {
    let expansion_id = Uuid::new_v4();
    let source_digest = hash_canonical(source_dsl);
    
    // Parse and identify template invocations
    let parsed = parse_for_expansion(source_dsl)?;
    
    let mut expanded_statements = Vec::new();
    let mut invocations = Vec::new();
    let mut template_digests = Vec::new();
    let mut derived_locks = Vec::new();
    let mut batch_policy = BatchPolicy::BestEffort;
    let mut diagnostics = Vec::new();
    
    for node in parsed.nodes {
        match node {
            ParsedNode::TemplateInvocation(invocation) => {
                // Look up template
                let template = template_registry.get(&invocation.name)?;
                
                // Record template digest (for audit)
                if !template_digests.iter().any(|d| d.name == template.name) {
                    template_digests.push(TemplateDigest {
                        name: template.name.clone(),
                        version: template.version.clone(),
                        digest: hash_canonical(&template.definition),
                    });
                }
                
                // Expand template with provided args
                let start_index = expanded_statements.len();
                let expanded = expand_single_template(&template, &invocation.args)?;
                
                // Track per-item origins
                let per_item_origins: Vec<_> = expanded.iter()
                    .enumerate()
                    .map(|(i, _)| PerItemOrigin {
                        expanded_statement_index: start_index + i,
                        template_item_index: i,
                    })
                    .collect();
                
                expanded_statements.extend(expanded);
                
                // Record invocation
                invocations.push(TemplateInvocationReport {
                    name: invocation.name.clone(),
                    args_json: invocation.args.clone(),
                    policy: template.policy.clone(),
                    origin_span: invocation.span.clone(),
                    expanded_range: ExpandedRange {
                        start_index,
                        end_index_exclusive: expanded_statements.len(),
                    },
                    per_item_origins,
                });
                
                // Derive locks from template policy + args
                if let Some(ref locking) = template.policy.locking {
                    derived_locks.extend(
                        derive_locks_from_policy(locking, &invocation.args)?
                    );
                }
                
                // Escalate to atomic if any template requests it
                if matches!(template.policy.batch_policy, BatchPolicy::Atomic) {
                    batch_policy = BatchPolicy::Atomic;
                }
            }
            ParsedNode::AtomicStatement(stmt) => {
                // Pass through unchanged
                expanded_statements.push(stmt);
            }
        }
    }
    
    // Sort locks to prevent deadlocks (CRITICAL)
    derived_locks.sort_by(|a, b| {
        (&a.entity_type, &a.entity_id, &a.access)
            .cmp(&(&b.entity_type, &b.entity_id, &b.access))
    });
    derived_locks.dedup();
    
    // Build expanded DSL string
    let expanded_dsl = statements_to_dsl(&expanded_statements);
    let expanded_dsl_digest = hash_canonical(&expanded_dsl);
    
    Ok(ExpansionOutput {
        expanded_dsl,
        report: ExpansionReport {
            expansion_id,
            source_digest,
            template_digests,
            invocations,
            expanded_statement_count: expanded_statements.len(),
            expanded_dsl_digest,
            derived_lock_set: derived_locks,
            batch_policy,
            diagnostics,
        },
    })
}

/// Hash content canonically (stable across runs)
fn hash_canonical(content: &str) -> String {
    let canonical = canonicalize_whitespace(content);
    let hash = Sha256::digest(canonical.as_bytes());
    hex::encode(hash)
}

fn canonicalize_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Derive concrete lock keys from policy + runtime args
fn derive_locks_from_policy(
    policy: &LockingPolicy,
    args: &serde_json::Value,
) -> Result<Vec<LockKey>, ExpansionError> {
    let mut locks = Vec::new();
    
    for target in &policy.targets {
        let entity_id = args
            .get(&target.arg)
            .and_then(|v| v.as_str())
            .ok_or_else(|| ExpansionError::MissingLockArg(target.arg.clone()))?;
        
        // Validate it looks like a UUID
        if Uuid::parse_str(entity_id).is_err() {
            return Err(ExpansionError::InvalidLockArg {
                arg: target.arg.clone(),
                value: entity_id.to_string(),
            });
        }
        
        locks.push(LockKey {
            entity_type: target.entity_type.clone(),
            entity_id: entity_id.to_string(),
            access: target.access,
        });
    }
    
    Ok(locks)
}

#[derive(Debug, thiserror::Error)]
pub enum ExpansionError {
    #[error("Template not found: {0}")]
    TemplateNotFound(String),
    
    #[error("Missing lock argument: {0}")]
    MissingLockArg(String),
    
    #[error("Invalid lock argument {arg}: {value} is not a valid UUID")]
    InvalidLockArg { arg: String, value: String },
    
    #[error("Parse error: {0}")]
    ParseError(String),
}
```

### Acceptance Criteria 2.1
- [ ] Expansion types serialize/deserialize cleanly
- [ ] ExpansionReport maps any expanded statement back to template + item index
- [ ] Expanding same input twice produces identical output and identical digests
- [ ] Expansion is pure (no DB calls)
- [ ] Lock set is sorted (deadlock prevention)

---

## 2.2 Add YAML Metadata for Batch Policy and Locking

**Files**: `rust/config/verbs/*.yaml`

### 2.2.1 Extend Verb YAML Schema

Add optional `policy` block:

```yaml
# Example: rust/config/verbs/link_director.yaml
domain: person
verb: link_as_director
description: Link person to entity as director

args:
  - name: person_id
    type: uuid
    required: true
  - name: entity_id
    type: uuid
    required: true

# NEW: Policy block (optional)
policy:
  batch: atomic          # atomic | best_effort (default: best_effort)
  locking:
    mode: try            # try | block (default: try)
    timeout_ms: 200      # optional, only used with mode: block
    targets:
      - arg: person_id
        entity_type: person
        access: write
      - arg: entity_id
        entity_type: entity
        access: write
```

### 2.2.2 Add RuntimePolicy to RuntimeVerb

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimePolicy {
    #[serde(default)]
    pub batch: BatchPolicy,
    pub locking: Option<LockingPolicy>,
}

// In RuntimeVerb struct, add:
pub policy: RuntimePolicy,
```

### 2.2.3 Defaults

- `batch`: `best_effort` (preserves current behavior)
- `locking`: `None` (no locks by default)

### Acceptance Criteria 2.2
- [ ] All existing verb YAML loads without changes (backward compatible)
- [ ] Verbs without `policy` block get defaults (best_effort, no locking)
- [ ] Verbs with `policy` block load correctly
- [ ] Policy available at runtime in RuntimeVerb

---

## 2.3 Deterministic Expansion Test

**File**: `rust/tests/expansion_determinism.rs`

```rust
#[test]
fn test_expansion_is_deterministic() {
    let source = r#"
        @template link_directors {
            person_id: "uuid-123",
            targets: ["cbu-1", "cbu-2", "cbu-3"]
        }
    "#;
    
    let registry = setup_test_registry();
    
    // Expand twice
    let result1 = expand_templates(source, &registry).unwrap();
    let result2 = expand_templates(source, &registry).unwrap();
    
    // Assert identical output
    assert_eq!(result1.expanded_dsl, result2.expanded_dsl);
    assert_eq!(result1.report.source_digest, result2.report.source_digest);
    assert_eq!(result1.report.expanded_dsl_digest, result2.report.expanded_dsl_digest);
    assert_eq!(result1.report.expanded_statement_count, result2.report.expanded_statement_count);
}
```

### Acceptance Criteria 2.3
- [ ] Test passes
- [ ] Same input always produces byte-identical output
- [ ] Same input always produces identical digests

---

# PHASE 3: Advisory Locks

> **Prerequisite**: Phase 1 and 2 complete
> **Scope**: Implement PostgreSQL advisory locks within transactions
> **Goal**: Prevent concurrent modification of entities during batch execution

---

## 3.1 DB Advisory Lock Helpers

**File**: `rust/src/database/locks.rs`

```rust
use sqlx::{Postgres, Transaction};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Derive stable i64 key from entity type + UUID
/// Uses deterministic hashing - same input always produces same key
pub fn lock_key(entity_type: &str, entity_id: &str) -> i64 {
    let mut hasher = DefaultHasher::new();
    entity_type.hash(&mut hasher);
    entity_id.hash(&mut hasher);
    hasher.finish() as i64
}

/// Acquire advisory lock (blocks until available)
/// Lock automatically released when transaction ends
pub async fn advisory_xact_lock(
    tx: &mut Transaction<'_, Postgres>,
    key: i64,
) -> sqlx::Result<()> {
    sqlx::query!("SELECT pg_advisory_xact_lock($1)", key)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Try to acquire advisory lock (non-blocking)
/// Returns true if acquired, false if already held by another session
pub async fn try_advisory_xact_lock(
    tx: &mut Transaction<'_, Postgres>,
    key: i64,
) -> sqlx::Result<bool> {
    let result = sqlx::query_scalar!(
        "SELECT pg_try_advisory_xact_lock($1)",
        key
    )
    .fetch_one(&mut **tx)
    .await?;
    
    Ok(result.unwrap_or(false))
}

/// Acquire multiple locks in sorted order (deadlock prevention)
pub async fn acquire_locks(
    tx: &mut Transaction<'_, Postgres>,
    locks: &[LockKey],
    mode: LockMode,
) -> Result<LockAcquisitionResult, LockError> {
    let mut acquired = Vec::new();
    let start = std::time::Instant::now();
    
    // Locks MUST be sorted to prevent deadlocks
    // Caller should pre-sort, but we ensure here
    let mut sorted_locks = locks.to_vec();
    sorted_locks.sort_by(|a, b| {
        (&a.entity_type, &a.entity_id).cmp(&(&b.entity_type, &b.entity_id))
    });
    
    for lock in &sorted_locks {
        let key = lock_key(&lock.entity_type, &lock.entity_id);
        
        match mode {
            LockMode::Try => {
                if !try_advisory_xact_lock(tx, key).await? {
                    return Err(LockError::Contention {
                        entity_type: lock.entity_type.clone(),
                        entity_id: lock.entity_id.clone(),
                        acquired_so_far: acquired,
                    });
                }
            }
            LockMode::Block => {
                advisory_xact_lock(tx, key).await?;
            }
        }
        
        acquired.push(lock.clone());
    }
    
    Ok(LockAcquisitionResult {
        acquired,
        wait_time_ms: start.elapsed().as_millis() as u64,
    })
}

#[derive(Debug)]
pub struct LockAcquisitionResult {
    pub acquired: Vec<LockKey>,
    pub wait_time_ms: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum LockError {
    #[error("Lock contention on {entity_type}:{entity_id}")]
    Contention {
        entity_type: String,
        entity_id: String,
        acquired_so_far: Vec<LockKey>,
    },
    
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}
```

### Acceptance Criteria 3.1
- [ ] Lock key derivation is deterministic and stable
- [ ] `advisory_xact_lock` blocks until available
- [ ] `try_advisory_xact_lock` returns immediately with bool
- [ ] Locks auto-released on tx commit/rollback
- [ ] Sorted acquisition prevents deadlocks

---

## 3.2 Integrate Locks into Atomic Execution

**File**: `rust/src/dsl_v2/executor.rs`

Update `execute_plan_atomic()` to accept optional expansion report:

```rust
pub async fn execute_plan_atomic(
    &self,
    plan: &ExecutionPlan,
    ctx: &mut ExecutionContext,
    expansion_report: Option<&ExpansionReport>,
) -> anyhow::Result<AtomicExecutionResult> {
    let mut tx = self.pool.begin().await?;
    
    // Acquire locks if expansion report has them
    let lock_result = if let Some(report) = expansion_report {
        if !report.derived_lock_set.is_empty() {
            let mode = match report.batch_policy {
                BatchPolicy::Atomic => LockMode::Try,  // Fail fast for atomic
                BatchPolicy::BestEffort => LockMode::Try,
            };
            match acquire_locks(&mut tx, &report.derived_lock_set, mode).await {
                Ok(result) => Some(result),
                Err(LockError::Contention { entity_type, entity_id, .. }) => {
                    tx.rollback().await?;
                    return Ok(AtomicExecutionResult::LockContention {
                        entity_type,
                        entity_id,
                    });
                }
                Err(e) => return Err(e.into()),
            }
        } else {
            None
        }
    } else {
        None
    };
    
    // Execute all steps in transaction
    let mut step_results = Vec::new();
    
    for (index, step) in plan.steps.iter().enumerate() {
        match self.execute_verb_in_tx(&step.verb_call, ctx, &mut tx).await {
            Ok(result) => {
                step_results.push(StepResult::Success { index, result });
            }
            Err(error) => {
                tx.rollback().await?;
                
                return Ok(AtomicExecutionResult::RolledBack {
                    failed_at_step: index,
                    error: error.to_string(),
                    completed_steps: step_results,
                    locks_held: lock_result.map(|r| r.acquired).unwrap_or_default(),
                });
            }
        }
    }
    
    tx.commit().await?;
    
    Ok(AtomicExecutionResult::Committed {
        step_results,
        locks_held: lock_result.map(|r| r.acquired).unwrap_or_default(),
    })
}

#[derive(Debug)]
pub enum AtomicExecutionResult {
    Committed {
        step_results: Vec<StepResult>,
        locks_held: Vec<LockKey>,
    },
    RolledBack {
        failed_at_step: usize,
        error: String,
        completed_steps: Vec<StepResult>,
        locks_held: Vec<LockKey>,
    },
    LockContention {
        entity_type: String,
        entity_id: String,
    },
}
```

### Acceptance Criteria 3.2
- [ ] Locks acquired after tx begin, before first step
- [ ] Lock contention returns clear error (not generic failure)
- [ ] Locks released on commit or rollback
- [ ] No locks acquired if expansion report is None or has empty lock set

---

## 3.3 Concurrency Test

**File**: `rust/tests/tx_lock_contention.rs`

```rust
#[tokio::test]
async fn test_atomic_lock_prevents_concurrent_modification() {
    let pool = setup_test_pool().await;
    
    // Create a person entity
    let person_id = create_test_person(&pool, "Hans Müller").await;
    
    // Session A: Start atomic batch linking person to CBUs
    // This should acquire lock on person_id
    let session_a = tokio::spawn({
        let pool = pool.clone();
        let person_id = person_id.clone();
        async move {
            let executor = DslExecutor::new(pool);
            let mut ctx = ExecutionContext::new();
            let plan = build_link_plan(&person_id, 10);
            let report = ExpansionReport {
                derived_lock_set: vec![LockKey {
                    entity_type: "person".into(),
                    entity_id: person_id.clone(),
                    access: LockAccess::Write,
                }],
                batch_policy: BatchPolicy::Atomic,
                ..Default::default()
            };
            
            // Add small delay to simulate work
            tokio::time::sleep(Duration::from_millis(100)).await;
            
            executor.execute_plan_atomic(&plan, &mut ctx, Some(&report)).await
        }
    });
    
    // Give Session A time to acquire lock
    tokio::time::sleep(Duration::from_millis(20)).await;
    
    // Session B: Attempt to delete the person (should fail due to lock)
    let session_b = tokio::spawn({
        let pool = pool.clone();
        let person_id = person_id.clone();
        async move {
            let executor = DslExecutor::new(pool);
            let mut ctx = ExecutionContext::new();
            let plan = build_delete_plan(&person_id);
            let report = ExpansionReport {
                derived_lock_set: vec![LockKey {
                    entity_type: "person".into(),
                    entity_id: person_id.clone(),
                    access: LockAccess::Write,
                }],
                batch_policy: BatchPolicy::Atomic,
                ..Default::default()
            };
            
            executor.execute_plan_atomic(&plan, &mut ctx, Some(&report)).await
        }
    });
    
    // Wait for both
    let (result_a, result_b) = tokio::join!(session_a, session_b);
    
    // Session A should succeed
    assert!(matches!(
        result_a.unwrap().unwrap(),
        AtomicExecutionResult::Committed { .. }
    ));
    
    // Session B should have failed due to lock contention
    assert!(matches!(
        result_b.unwrap().unwrap(),
        AtomicExecutionResult::LockContention { .. }
    ));
}
```

### Acceptance Criteria 3.3
- [ ] Test demonstrates lock prevents concurrent modification
- [ ] No partial state (the 20/50 problem) under contention
- [ ] Clear error message on lock contention

---

# PHASE 4: Error Aggregation

> **Can run in parallel with Phase 2-3**
> **Scope**: Sensible reporting when cascaded failures occur
> **Goal**: Group errors by root cause, not individual verb failures

---

## 4.1 Error Aggregation Types

**File**: `rust/src/dsl_v2/errors.rs`

```rust
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::Serialize;

/// Aggregated execution errors grouped by root cause
#[derive(Debug, Clone, Serialize, Default)]
pub struct ExecutionErrors {
    pub by_cause: HashMap<ErrorCause, CausedErrors>,
    pub total_failed: usize,
    pub total_succeeded: usize,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize)]
pub enum ErrorCause {
    EntityDeleted { entity_id: String },
    EntityNotFound { entity_id: String },
    VersionConflict { entity_id: String },
    PermissionDenied { resource: String },
    ValidationFailed { rule: String },
    Other { code: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct CausedErrors {
    pub cause: ErrorCause,
    pub details: CauseDetails,
    pub affected_verbs: Vec<AffectedVerb>,
    pub count: usize,
    pub timing: Option<FailureTiming>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CauseDetails {
    pub entity_name: Option<String>,
    pub deleted_by: Option<String>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub hint: String,
    pub recoverable: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AffectedVerb {
    pub index: usize,
    pub verb: String,
    pub domain: String,
    pub target: Option<String>,
}

/// Detect if failure happened mid-execution (race condition)
#[derive(Debug, Clone, Serialize)]
pub enum FailureTiming {
    /// Entity was already deleted when batch started
    PreExisting {
        deleted_at: DateTime<Utc>,
        batch_started_at: DateTime<Utc>,
    },
    /// Entity was deleted DURING batch execution
    MidExecution {
        deleted_at: DateTime<Utc>,
        first_success_at: DateTime<Utc>,
        first_failure_at: DateTime<Utc>,
        succeeded_before_delete: usize,
    },
}

impl ExecutionErrors {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn record(&mut self, verb_index: usize, domain: &str, verb: &str, result: &VerbResult) {
        match result {
            VerbResult::Success { .. } | VerbResult::NoOp { .. } => {
                self.total_succeeded += 1;
            }
            VerbResult::Failed { error, .. } => {
                self.total_failed += 1;
                
                let cause = error.to_cause();
                
                let entry = self.by_cause
                    .entry(cause.clone())
                    .or_insert_with(|| CausedErrors {
                        cause,
                        details: error.to_details(),
                        affected_verbs: vec![],
                        count: 0,
                        timing: None,
                    });
                
                entry.count += 1;
                entry.affected_verbs.push(AffectedVerb {
                    index: verb_index,
                    verb: verb.to_string(),
                    domain: domain.to_string(),
                    target: error.target_entity(),
                });
            }
        }
    }
    
    /// Analyze timing to detect mid-execution deletes
    pub fn analyze_timing(&mut self, verb_results: &[VerbResult], batch_started_at: DateTime<Utc>) {
        for errors in self.by_cause.values_mut() {
            if let ErrorCause::EntityDeleted { entity_id } = &errors.cause {
                // Find first success and first failure referencing this entity
                let first_success_at = verb_results.iter()
                    .filter(|r| r.references_entity(entity_id) && r.is_success())
                    .filter_map(|r| r.executed_at())
                    .min();
                    
                let first_failure_at = verb_results.iter()
                    .filter(|r| r.references_entity(entity_id) && r.is_failure())
                    .filter_map(|r| r.executed_at())
                    .min();
                
                if let (Some(success_t), Some(failure_t)) = (first_success_at, first_failure_at) {
                    if success_t < failure_t {
                        // Had successes before failures = mid-execution delete
                        let succeeded_count = verb_results.iter()
                            .filter(|r| r.references_entity(entity_id) && r.is_success())
                            .count();
                        
                        errors.timing = Some(FailureTiming::MidExecution {
                            deleted_at: errors.details.deleted_at.unwrap_or(failure_t),
                            first_success_at: success_t,
                            first_failure_at: failure_t,
                            succeeded_before_delete: succeeded_count,
                        });
                    }
                }
            }
        }
    }
    
    /// Generate human-readable summary
    pub fn summary(&self) -> String {
        if self.by_cause.is_empty() {
            return format!("✓ {} succeeded", self.total_succeeded);
        }
        
        let mut lines = vec![format!(
            "{} succeeded, {} failed ({} unique causes)",
            self.total_succeeded,
            self.total_failed,
            self.by_cause.len()
        )];
        
        for errors in self.by_cause.values() {
            lines.push(format!(
                "\n❌ {} operations failed: {}",
                errors.count,
                errors.details.hint
            ));
            
            // Show timing if mid-execution race detected
            if let Some(FailureTiming::MidExecution { succeeded_before_delete, .. }) = &errors.timing {
                lines.push(format!(
                    "   ⚠️ TIMING: Entity deleted MID-EXECUTION ({} ops succeeded before deletion)",
                    succeeded_before_delete
                ));
            }
            
            // Show first few affected verbs
            for verb in errors.affected_verbs.iter().take(3) {
                lines.push(format!("   • {}.{} (step {})", verb.domain, verb.verb, verb.index));
            }
            
            if errors.affected_verbs.len() > 3 {
                lines.push(format!("   ... +{} more", errors.affected_verbs.len() - 3));
            }
        }
        
        lines.join("\n")
    }
    
    pub fn is_empty(&self) -> bool {
        self.by_cause.is_empty()
    }
}
```

### Acceptance Criteria 4.1
- [ ] Errors grouped by root cause, not individual failures
- [ ] Mid-execution timing detected (race condition detection)
- [ ] Summary output is human-readable and actionable
- [ ] 50 failures from same cause show as "1 cause, 50 affected"

---

## 4.2 Integrate Error Aggregation into Best-Effort Execution

**File**: `rust/src/dsl_v2/executor.rs`

Update best-effort execution to use aggregation:

```rust
#[derive(Debug)]
pub struct BestEffortExecutionResult {
    pub verb_results: Vec<VerbResult>,
    pub errors: ExecutionErrors,
    pub status: BatchStatus,
}

#[derive(Debug)]
pub enum BatchStatus {
    AllSucceeded,
    PartialSuccess,
    AllFailed,
}

pub async fn execute_plan(
    &self,
    plan: &ExecutionPlan,
    ctx: &mut ExecutionContext,
) -> anyhow::Result<BestEffortExecutionResult> {
    let batch_started_at = Utc::now();
    let mut errors = ExecutionErrors::new();
    let mut verb_results = Vec::new();
    
    for (index, step) in plan.steps.iter().enumerate() {
        let result = self.execute_verb(&step.verb_call, ctx).await;
        
        errors.record(index, &step.verb_call.domain, &step.verb_call.verb, &result);
        verb_results.push(result);
    }
    
    // Analyze timing for any EntityDeleted errors
    errors.analyze_timing(&verb_results, batch_started_at);
    
    let status = if errors.total_failed == 0 {
        BatchStatus::AllSucceeded
    } else if errors.total_succeeded == 0 {
        BatchStatus::AllFailed
    } else {
        BatchStatus::PartialSuccess
    };
    
    Ok(BestEffortExecutionResult {
        verb_results,
        errors,
        status,
    })
}
```

### Acceptance Criteria 4.2
- [ ] Best-effort execution produces aggregated errors
- [ ] Timing analysis detects mid-execution deletes
- [ ] Results include both individual verb results AND aggregated summary

---

# PHASE 5: Wire Into REPL Execution Path

**File**: `rust/src/api/agent_routes.rs` (or equivalent)

---

## 5.1 Update execute_session_dsl()

```rust
pub async fn execute_session_dsl(
    session_id: Uuid,
    raw_dsl: &str,
    executor: &DslExecutor,
    template_registry: &TemplateRegistry,
) -> Result<ExecutionResponse, ApiError> {
    
    // 1. Expand templates (pure, deterministic)
    let ExpansionOutput { expanded_dsl, report } = 
        expand_templates(raw_dsl, template_registry)?;
    
    // 2. Parse expanded DSL
    let ast = parse_program(&expanded_dsl)?;
    
    // 3. Semantic validation
    validate_semantics(&ast)?;
    
    // 4. Compile to execution plan
    let plan = compile_plan(&ast)?;
    
    // 5. Execute based on batch policy
    let result = match report.batch_policy {
        BatchPolicy::Atomic => {
            let atomic_result = executor
                .execute_plan_atomic(&plan, &mut ctx, Some(&report))
                .await?;
            ExecutionOutcome::Atomic(atomic_result)
        }
        BatchPolicy::BestEffort => {
            let best_effort_result = executor
                .execute_plan(&plan, &mut ctx)
                .await?;
            ExecutionOutcome::BestEffort(best_effort_result)
        }
    };
    
    // 6. Persist expansion report for audit
    persist_expansion_report(&report, session_id).await?;
    
    // 7. Return response with aggregated errors
    Ok(ExecutionResponse {
        status: result.status(),
        summary: result.error_summary(),
        expansion_id: report.expansion_id,
        step_count: plan.steps.len(),
        // ...
    })
}

enum ExecutionOutcome {
    Atomic(AtomicExecutionResult),
    BestEffort(BestEffortExecutionResult),
}
```

### Acceptance Criteria 5.1
- [ ] Expansion happens before parsing
- [ ] Batch policy from expansion report determines execution mode
- [ ] Expansion report persisted for audit
- [ ] Errors returned with aggregated summary

---

## 5.2 Persist Expansion Report

**Table**: `dsl_expansions`

```sql
CREATE TABLE dsl_expansions (
    expansion_id UUID PRIMARY KEY,
    session_id UUID NOT NULL,
    
    source_digest TEXT NOT NULL,
    expanded_digest TEXT NOT NULL,
    
    report_json JSONB NOT NULL,
    
    batch_policy TEXT NOT NULL,
    lock_count INTEGER NOT NULL DEFAULT 0,
    statement_count INTEGER NOT NULL DEFAULT 0,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_dsl_expansions_session ON dsl_expansions(session_id, created_at DESC);
CREATE INDEX idx_dsl_expansions_source ON dsl_expansions(source_digest);
```

### Acceptance Criteria 5.2
- [ ] Every expansion persisted before execution
- [ ] Report JSON queryable for audit
- [ ] Can replay "what did this template expand into?"

---

# Validation Checklist (All Phases)

## Phase 1 Complete When:
- [ ] `cargo test` passes
- [ ] Rollback regression test passes
- [ ] `execute_verb_in_tx()` no longer calls `execute_verb()` internally
- [ ] No `&self.pool` on CRUD write paths in generic_executor.rs
- [ ] `execute_plan()` unchanged (backward compat)

## Phase 2 Complete When:
- [ ] Expansion is deterministic (same input → same output)
- [ ] ExpansionReport serializes correctly
- [ ] YAML policy metadata loads
- [ ] Expansion pure (no DB calls)

## Phase 3 Complete When:
- [ ] Advisory locks acquired in tx
- [ ] Lock contention test passes
- [ ] No deadlocks (sorted acquisition)
- [ ] Locks released on commit/rollback

## Phase 4 Complete When:
- [ ] Errors grouped by root cause
- [ ] Mid-execution timing detected
- [ ] Summary is human-readable
- [ ] 50 same-cause failures show as 1 cause

## Phase 5 Complete When:
- [ ] REPL uses expansion stage
- [ ] Batch policy routes to correct executor
- [ ] Expansion report persisted
- [ ] Aggregated errors in response

---

# Done Definition

The "20/50 partial state" problem is solved when:

1. **With `batch: atomic`**: All 50 succeed or all 50 rollback (no partial)
2. **With `batch: best_effort`**: Partial allowed, but errors aggregated sensibly
3. **With locking**: Concurrent delete blocked until batch completes
4. **Audit trail**: ExpansionReport shows exactly what happened and why

---

# Execution Order for Claude Code

```
1. Phase 1 (TX Plumbing) ← START HERE, foundation for everything
   1.1 → 1.2 → 1.3 → 1.4 → 1.5 (regression test)

2. Phase 4 (Error Aggregation) ← Can do in parallel, no dependencies
   4.1 → 4.2

3. Phase 2 (Expansion Stage)
   2.1 → 2.2 → 2.3 (determinism test)

4. Phase 3 (Advisory Locks) ← Requires Phase 1 + 2
   3.1 → 3.2 → 3.3 (concurrency test)

5. Phase 5 (REPL Integration) ← Ties everything together
   5.1 → 5.2
```

---

# Notes (Future Work, Not Part of This TODO)

- Some custom ops will need real tx support; add them incrementally by implementing `execute_in_tx()` in those ops
- Lock timeout support for `mode: block` may need custom Postgres logic
- Expansion report UI visualization
- Lock contention metrics/monitoring
