# TODO: Template & Batch Native DSL Integration (P1)

## ⛔ MANDATORY FIRST STEP

**Before writing ANY egui/WASM code, read `/EGUI-RULES.md` completely.**

---

## Objective

Make templates and batch execution first-class DSL constructs in the interactive REPL session, fully integrated with the execution pipeline (parse → compile → DAG → execute).

**Key Insight:** Batch execution must respect the topo-sorted DAG execution model. Each iteration runs the full pipeline with proper context isolation AND parent binding access.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  SESSION CONTEXT (parent)                                                   │
│  ├── shared_bindings: {@manco: uuid-1, @im: uuid-2}  ← Constant             │
│  ├── batch_results: BatchResultAccumulator            ← Accumulated         │
│  │   ├── iteration_bindings: [                                              │
│  │   │     {cbu: uuid-100, case: uuid-200},  // iteration 0                 │
│  │   │     {cbu: uuid-101, case: uuid-201},  // iteration 1                 │
│  │   │   ]                                                                  │
│  │   └── primary_entity_ids: [uuid-100, uuid-101, ...]                      │
│  └── primary_entity_type: "cbu"                                             │
│                                                                             │
│  PER ITERATION:                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  ExecutionContext (child)                                           │   │
│  │  ├── symbols: {@cbu: uuid-100}           ← Fresh per iteration      │   │
│  │  ├── parent_symbols: {@manco, @im}       ← Read from parent         │   │
│  │  └── batch_index: 0                                                 │   │
│  │                                                                     │   │
│  │  Pipeline: Expand → Parse → Compile → DAG → Execute                 │   │
│  │  Topo order respected within each iteration                         │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  POST-BATCH (:then clause):                                                 │
│  ├── Has access to batch_results.primary_entity_ids                         │
│  └── Can iterate all created PKs for follow-up operations                   │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## P1 Enhancements

### Enhancement 1: `template.invoke` Verb

Single template invocation within DSL.

#### DSL Syntax

```clojure
;; Invoke template with explicit params
(template.invoke 
  :id "onboard-fund-cbu"
  :params {:fund_entity @selected_fund
           :fund_entity.name "Allianz Dynamic"
           :manco_entity @manco
           :im_entity @im
           :jurisdiction "LU"}
  :as @result)

;; @cbu binding available after execution (from template outputs)
```

#### Implementation

**File:** `rust/src/dsl_v2/custom_ops/template_ops.rs` (new)

```rust
pub struct TemplateInvokeOp;

impl VerbExecutor for TemplateInvokeOp {
    fn verb_name(&self) -> &'static str {
        "template.invoke"
    }
    
    async fn execute(
        &self,
        args: &VerbCallArgs,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult, ExecutionError> {
        let template_id = args.get_string("id")?;
        let params: HashMap<String, String> = args.get_map("params")?;
        
        // Get template registry from context
        let registry = ctx.template_registry()
            .ok_or(ExecutionError::NoTemplateRegistry)?;
        
        let template = registry.get(&template_id)
            .ok_or_else(|| ExecutionError::TemplateNotFound(template_id.clone()))?;
        
        // Build expansion context from current execution context
        let exp_ctx = ExpansionContext {
            current_cbu: ctx.resolve_binding("cbu"),
            current_case: ctx.resolve_binding("case"),
            bindings: ctx.all_bindings_as_strings(),
            binding_types: ctx.all_binding_types(),
        };
        
        // Expand template
        let expansion = TemplateExpander::expand(template, &params, &exp_ctx);
        
        if !expansion.missing_params.is_empty() {
            return Err(ExecutionError::MissingTemplateParams {
                template_id,
                missing: expansion.missing_params,
            });
        }
        
        // Parse expanded DSL
        let ast = parse_program(&expansion.dsl)
            .map_err(|e| ExecutionError::TemplateParseError(template_id.clone(), e))?;
        
        // Compile to execution plan (DAG)
        let plan = compile(&ast)
            .map_err(|e| ExecutionError::TemplateCompileError(template_id.clone(), e))?;
        
        // Execute plan (nested execution, same context)
        let executor = DslExecutor::new(pool.clone());
        let results = executor.execute_plan(&plan, ctx).await?;
        
        Ok(ExecutionResult::TemplateExpanded {
            template_id,
            statements_executed: ast.statements.len(),
            outputs: expansion.outputs,
        })
    }
}
```

**Register verb:**

```rust
// In rust/src/dsl_v2/verb_registry.rs or custom_ops/mod.rs
registry.register("template", "invoke", Box::new(TemplateInvokeOp));
```

#### Tasks

- [ ] Create `rust/src/dsl_v2/custom_ops/template_ops.rs`
- [ ] Implement `TemplateInvokeOp`
- [ ] Add `template_registry()` accessor to `ExecutionContext`
- [ ] Register `template.invoke` verb
- [ ] Add `ExecutionResult::TemplateExpanded` variant
- [ ] Unit tests for single invocation
- [ ] Integration test with `onboard-fund-cbu`

---

### Enhancement 2a: `template.batch` Verb

Batch template execution over a query result set.

#### DSL Syntax

```clojure
;; First query entities via verb
(entity.query :type "fund" :name-like "Allianz%" :jurisdiction "LU" :as @funds)

;; Basic batch using the binding
(template.batch
  :id "onboard-fund-cbu"
  :source @funds                  ;; Binding from entity.query
  :bind-as "fund_entity"          ;; Each item binds to this param
  :shared {:manco_entity @manco
           :im_entity @im
           :jurisdiction "LU"}
  :on-error :continue             ;; :stop | :continue | :rollback
  :limit 10                       ;; Optional limit
  :as @batch_result)

;; Post-batch operations via verb
(batch.add-products :cbu-ids @batch_result :products ["CUSTODY" "FUND_ACCOUNTING"])
```

#### Implementation

**File:** `rust/src/dsl_v2/custom_ops/template_ops.rs`

```rust
pub struct TemplateBatchOp;

impl VerbExecutor for TemplateBatchOp {
    fn verb_name(&self) -> &'static str {
        "template.batch"
    }
    
    async fn execute(
        &self,
        args: &VerbCallArgs,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult, ExecutionError> {
        let template_id = args.get_string("id")?;
        let source_query = args.get_string("source")?;
        let bind_param = args.get_string("bind-as")?;
        let shared: HashMap<String, String> = args.get_map("shared")?;
        let on_error = args.get_enum_or("on-error", "continue")?;
        let limit = args.get_optional_usize("limit")?;
        let then_clause = args.get_optional_statements("then")?;
        
        // Resolve shared bindings to UUIDs
        let shared_bindings = resolve_shared_bindings(&shared, ctx, pool).await?;
        
        // Execute source query
        let rows: Vec<(Uuid, String)> = sqlx::query_as(&source_query)
            .fetch_all(pool)
            .await
            .map_err(|e| ExecutionError::QueryFailed(e.to_string()))?;
        
        let rows = match limit {
            Some(n) => rows.into_iter().take(n).collect(),
            None => rows,
        };
        
        // Load template
        let registry = ctx.template_registry()?;
        let template = registry.get(&template_id)?;
        
        // Create batch executor
        let batch_executor = BatchExecutor::new(
            pool.clone(),
            template.clone(),
            shared.clone(),
            shared_bindings.clone(),
        );
        
        // Execute batch with accumulation
        let batch_result = batch_executor
            .execute_batch(rows, bind_param, on_error)
            .await?;
        
        // Store accumulator in context for :then clause
        ctx.batch_results = Some(batch_result.accumulator.clone());
        
        // Execute :then clause if present
        if let Some(then_statements) = then_clause {
            execute_post_batch(&then_statements, &batch_result.accumulator, ctx, pool).await?;
        }
        
        Ok(ExecutionResult::BatchCompleted(batch_result))
    }
}
```

#### Tasks

- [ ] Implement `TemplateBatchOp` in `template_ops.rs`
- [ ] Implement `entity.query` verb (returns list of entity refs)
- [ ] Support binding reference as `:source` argument
- [ ] Implement `resolve_shared_bindings()` helper
- [ ] Register `template.batch` verb
- [ ] Unit tests for batch execution
- [ ] Integration test with Allianz funds

---

### Enhancement 2b: Batch Context Propagation (Critical)

Proper context hierarchy for batch execution respecting the DAG pipeline.

#### Problem

Each batch iteration must:
1. Run full pipeline (parse → compile → DAG → execute)
2. Have isolated local symbols (fresh @cbu per iteration)
3. Access parent symbols (shared @manco, @im)
4. Propagate created PKs to accumulator

#### Implementation

**File:** `rust/src/dsl_v2/executor.rs` (modify)

```rust
/// Execution context with parent hierarchy support
pub struct ExecutionContext {
    /// Symbols created in THIS execution scope
    pub symbols: HashMap<String, Uuid>,
    
    /// Symbol types (entity_type for each binding)
    pub symbol_types: HashMap<String, String>,
    
    /// Parent symbols (read-only, from parent context)
    pub parent_symbols: HashMap<String, Uuid>,
    
    /// Parent symbol types
    pub parent_symbol_types: HashMap<String, String>,
    
    /// Batch iteration index (None if not in batch)
    pub batch_index: Option<usize>,
    
    /// Accumulated results from batch iterations
    /// Only present in orchestrating context, not child contexts
    pub batch_results: Option<BatchResultAccumulator>,
    
    /// Template registry reference
    template_registry: Option<Arc<TemplateRegistry>>,
    
    /// Database pool reference
    pool: Option<PgPool>,
}

impl ExecutionContext {
    /// Resolve a binding, checking local then parent
    pub fn resolve_binding(&self, name: &str) -> Option<Uuid> {
        // 1. Local symbols (current scope)
        if let Some(pk) = self.symbols.get(name) {
            return Some(*pk);
        }
        
        // 2. Parent symbols (shared bindings)
        if let Some(pk) = self.parent_symbols.get(name) {
            return Some(*pk);
        }
        
        None
    }
    
    /// Get all bindings (local + parent) as string map
    pub fn all_bindings_as_strings(&self) -> HashMap<String, String> {
        let mut result: HashMap<String, String> = self.parent_symbols
            .iter()
            .map(|(k, v)| (k.clone(), v.to_string()))
            .collect();
        
        // Local overrides parent
        for (k, v) in &self.symbols {
            result.insert(k.clone(), v.to_string());
        }
        
        result
    }
    
    /// Create child context for batch iteration
    pub fn child_for_iteration(&self, index: usize) -> Self {
        Self {
            symbols: HashMap::new(),  // Fresh
            symbol_types: HashMap::new(),
            parent_symbols: self.effective_symbols(),  // Inherit
            parent_symbol_types: self.effective_symbol_types(),
            batch_index: Some(index),
            batch_results: None,  // Child doesn't accumulate
            template_registry: self.template_registry.clone(),
            pool: self.pool.clone(),
        }
    }
    
    /// Get effective symbols (local merged with parent)
    fn effective_symbols(&self) -> HashMap<String, Uuid> {
        let mut result = self.parent_symbols.clone();
        result.extend(self.symbols.clone());
        result
    }
}
```

**File:** `rust/src/dsl_v2/batch_executor.rs` (new)

```rust
//! Batch executor for template iteration with context propagation

use std::collections::HashMap;
use std::sync::Arc;
use sqlx::PgPool;
use uuid::Uuid;

use super::executor::{DslExecutor, ExecutionContext};
use super::{compile, parse_program};
use crate::templates::{ExpansionContext, TemplateDefinition, TemplateExpander};

/// Accumulated results from batch execution
#[derive(Debug, Clone, Default)]
pub struct BatchResultAccumulator {
    /// Per-iteration bindings: Vec<{binding_name → uuid}>
    pub iteration_bindings: Vec<HashMap<String, Uuid>>,
    
    /// Flattened primary entity IDs (e.g., all @cbu values)
    pub primary_entity_ids: Vec<Uuid>,
    
    /// Primary entity type (from template.primary_entity)
    pub primary_entity_type: String,
    
    /// Primary entity binding name (e.g., "cbu")
    pub primary_binding_name: String,
    
    /// Success count
    pub success_count: usize,
    
    /// Failure count
    pub failure_count: usize,
    
    /// Errors by iteration index
    pub errors: HashMap<usize, String>,
}

impl BatchResultAccumulator {
    pub fn new(template: &TemplateDefinition) -> Self {
        let (entity_type, binding_name) = template.primary_entity
            .as_ref()
            .map(|p| (p.entity_type.to_string(), p.param.clone()))
            .unwrap_or_else(|| ("unknown".to_string(), "result".to_string()));
        
        Self {
            primary_entity_type: entity_type,
            primary_binding_name: binding_name,
            ..Default::default()
        }
    }
    
    /// Get all primary entity IDs as a Vec for iteration
    pub fn cbu_ids(&self) -> &[Uuid] {
        &self.primary_entity_ids
    }
}

/// Orchestrates batch template execution
pub struct BatchExecutor {
    pool: PgPool,
    template: TemplateDefinition,
    shared_params: HashMap<String, String>,
    shared_bindings: HashMap<String, Uuid>,
}

impl BatchExecutor {
    pub fn new(
        pool: PgPool,
        template: TemplateDefinition,
        shared_params: HashMap<String, String>,
        shared_bindings: HashMap<String, Uuid>,
    ) -> Self {
        Self {
            pool,
            template,
            shared_params,
            shared_bindings,
        }
    }
    
    /// Execute batch with full pipeline per iteration
    pub async fn execute_batch(
        &self,
        items: Vec<(Uuid, String)>,  // (entity_id, name)
        bind_param: &str,
        on_error: &str,
    ) -> Result<BatchExecutionResult, ExecutionError> {
        let executor = DslExecutor::new(self.pool.clone());
        let mut accumulator = BatchResultAccumulator::new(&self.template);
        
        // Optional transaction for rollback mode
        let mut tx = if on_error == "rollback" {
            Some(self.pool.begin().await?)
        } else {
            None
        };
        
        for (index, (entity_id, name)) in items.iter().enumerate() {
            // Build iteration-specific params
            let mut params = self.shared_params.clone();
            params.insert(bind_param.to_string(), entity_id.to_string());
            params.insert(format!("{}.name", bind_param), name.clone());
            
            // Build expansion context
            let exp_ctx = ExpansionContext {
                bindings: self.shared_bindings
                    .iter()
                    .map(|(k, v)| (k.clone(), v.to_string()))
                    .collect(),
                ..Default::default()
            };
            
            // EXPAND template
            let expansion = TemplateExpander::expand(&self.template, &params, &exp_ctx);
            
            if !expansion.missing_params.is_empty() {
                accumulator.failure_count += 1;
                accumulator.errors.insert(index, format!(
                    "Missing params: {:?}",
                    expansion.missing_params.iter().map(|p| &p.name).collect::<Vec<_>>()
                ));
                
                if on_error == "stop" {
                    break;
                }
                continue;
            }
            
            // PARSE expanded DSL
            let ast = match parse_program(&expansion.dsl) {
                Ok(ast) => ast,
                Err(e) => {
                    accumulator.failure_count += 1;
                    accumulator.errors.insert(index, format!("Parse error: {:?}", e));
                    
                    if on_error == "stop" {
                        break;
                    }
                    continue;
                }
            };
            
            // COMPILE to execution plan (DAG/topo sort)
            let plan = match compile(&ast) {
                Ok(plan) => plan,
                Err(e) => {
                    accumulator.failure_count += 1;
                    accumulator.errors.insert(index, format!("Compile error: {:?}", e));
                    
                    if on_error == "stop" {
                        break;
                    }
                    continue;
                }
            };
            
            // Create CHILD context with parent bindings
            let mut child_ctx = ExecutionContext {
                symbols: HashMap::new(),  // Fresh for this iteration
                symbol_types: HashMap::new(),
                parent_symbols: self.shared_bindings.clone(),  // Shared
                parent_symbol_types: HashMap::new(),
                batch_index: Some(index),
                batch_results: None,
                template_registry: None,
                pool: Some(self.pool.clone()),
            };
            
            // EXECUTE plan (respects DAG order!)
            match executor.execute_plan(&plan, &mut child_ctx).await {
                Ok(_results) => {
                    // Propagate iteration bindings to accumulator
                    accumulator.iteration_bindings.push(child_ctx.symbols.clone());
                    
                    // Extract primary entity PK
                    if let Some(pk) = child_ctx.symbols.get(&accumulator.primary_binding_name) {
                        accumulator.primary_entity_ids.push(*pk);
                    }
                    
                    accumulator.success_count += 1;
                }
                Err(e) => {
                    accumulator.failure_count += 1;
                    accumulator.errors.insert(index, format!("Execution error: {}", e));
                    
                    match on_error {
                        "stop" => break,
                        "rollback" => {
                            if let Some(tx) = tx.take() {
                                tx.rollback().await?;
                            }
                            return Err(ExecutionError::BatchAborted {
                                at_index: index,
                                error: e.to_string(),
                                partial_results: accumulator,
                            });
                        }
                        _ => continue,  // "continue"
                    }
                }
            }
        }
        
        // Commit transaction if rollback mode succeeded
        if let Some(tx) = tx {
            tx.commit().await?;
        }
        
        Ok(BatchExecutionResult {
            accumulator,
            total_items: items.len(),
        })
    }
}

/// Result of batch execution
#[derive(Debug)]
pub struct BatchExecutionResult {
    pub accumulator: BatchResultAccumulator,
    pub total_items: usize,
}
```

#### Tasks

- [ ] Modify `ExecutionContext` to support parent hierarchy
- [ ] Add `resolve_binding()` with local → parent lookup
- [ ] Add `child_for_iteration()` factory method
- [ ] Create `rust/src/dsl_v2/batch_executor.rs`
- [ ] Implement `BatchResultAccumulator`
- [ ] Implement `BatchExecutor::execute_batch()`
- [ ] Add `ExecutionError::BatchAborted` variant
- [ ] Unit tests for context hierarchy
- [ ] Unit tests for binding resolution order
- [ ] Integration test: verify DAG order within iteration

---

### Enhancement 3: Session Batch State

Persist batch state in session for pause/resume/status.

#### Add to SessionContext

**File:** `rust/src/api/session.rs` (modify)

```rust
pub struct SessionContext {
    // ... existing fields ...
    
    /// Active batch execution state
    pub active_batch: Option<ActiveBatchState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveBatchState {
    /// Template being executed
    pub template_id: String,
    
    /// Source query (for resume/retry)
    pub source_query: String,
    
    /// Bind parameter name
    pub bind_param: String,
    
    /// Shared parameters
    pub shared_params: HashMap<String, String>,
    
    /// Resolved shared bindings
    pub shared_bindings: HashMap<String, Uuid>,
    
    /// Total items to process
    pub total_items: usize,
    
    /// Current position (0-indexed)
    pub current_index: usize,
    
    /// Remaining items (entity_id, name)
    pub remaining_items: Vec<(Uuid, String)>,
    
    /// Accumulated results
    pub results: BatchResultAccumulator,
    
    /// Error handling mode
    pub on_error: String,
    
    /// Batch status
    pub status: BatchStatus,
    
    /// Started at
    pub started_at: DateTime<Utc>,
    
    /// Paused at (if paused)
    pub paused_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BatchStatus {
    Running,
    Paused,
    Completed,
    Failed,
    Aborted,
}
```

#### REPL Commands

```clojure
;; Pause current batch
(batch.pause)

;; Resume paused batch
(batch.resume)

;; Continue N more items
(batch.continue :count 10)

;; Skip current item
(batch.skip)

;; Abort batch
(batch.abort)

;; Get batch status
(batch.status)
;; => {:template "onboard-fund-cbu"
;;     :status :running
;;     :progress {:current 47 :total 205 :success 45 :failed 2}
;;     :current_item "Allianz Dynamic Multi Asset Strategy"
;;     :elapsed "00:02:34"}
```

#### Tasks

- [ ] Add `ActiveBatchState` to `SessionContext`
- [ ] Add `BatchStatus` enum
- [ ] Implement `batch.pause` verb
- [ ] Implement `batch.resume` verb
- [ ] Implement `batch.continue` verb
- [ ] Implement `batch.skip` verb
- [ ] Implement `batch.abort` verb
- [ ] Implement `batch.status` verb
- [ ] Persist batch state on pause
- [ ] Restore batch state on resume
- [ ] Integration test: pause/resume cycle

---

### Enhancement 4: Post-Batch Operations

Access accumulated PKs in `:then` clause.

#### DSL Syntax

```clojure
;; :then clause has access to @batch_result
(template.batch
  :id "onboard-fund-cbu"
  :source (query "SELECT ...")
  :shared {:manco @manco}
  :as @batch_result
  :then [
    ;; Iterate all created CBUs
    (for-each [@cbu @batch_result.cbu_ids]
      (cbu.add-product :cbu-id @cbu :product "CUSTODY"))
    
    ;; Or use bulk endpoint
    (batch.add-products 
      :cbu-ids @batch_result.cbu_ids 
      :products ["CUSTODY" "FUND_ACCOUNTING"])
  ])
```

#### Implementation

```rust
/// Execute post-batch statements with accumulated results
pub async fn execute_post_batch(
    statements: &[Statement],
    accumulator: &BatchResultAccumulator,
    parent_ctx: &ExecutionContext,
    pool: &PgPool,
) -> Result<Vec<ExecutionResult>, ExecutionError> {
    // Create context with batch results injected
    let mut ctx = ExecutionContext {
        symbols: HashMap::new(),
        parent_symbols: parent_ctx.effective_symbols(),
        batch_results: Some(accumulator.clone()),
        ..Default::default()
    };
    
    // Inject batch result as accessible binding
    // @batch_result.cbu_ids → accumulator.primary_entity_ids
    ctx.set_batch_result_binding("batch_result", accumulator);
    
    // Parse and execute :then statements
    let executor = DslExecutor::new(pool.clone());
    
    let mut results = Vec::new();
    for stmt in statements {
        let plan = compile_statement(stmt)?;
        let result = executor.execute_plan(&plan, &mut ctx).await?;
        results.extend(result);
    }
    
    Ok(results)
}
```

#### Tasks

- [ ] Implement `execute_post_batch()` function
- [ ] Add `set_batch_result_binding()` to context
- [ ] Implement `@batch_result.cbu_ids` resolution
- [ ] Implement `for-each` construct (or reuse existing)
- [ ] Add `batch.add-products` bulk verb
- [ ] Integration test: batch + then clause

---

## File Summary

| File | Action | Purpose |
|------|--------|---------|
| `rust/src/dsl_v2/custom_ops/template_ops.rs` | Create | `template.invoke`, `template.batch` verbs |
| `rust/src/dsl_v2/batch_executor.rs` | Create | Batch orchestration with accumulation |
| `rust/src/dsl_v2/executor.rs` | Modify | Add parent_symbols, resolve_binding() |
| `rust/src/dsl_v2/custom_ops/batch_control_ops.rs` | Create | `batch.pause`, `batch.resume`, etc. |
| `rust/src/api/session.rs` | Modify | Add `ActiveBatchState` |
| `rust/src/dsl_v2/mod.rs` | Modify | Export new modules |
| `rust/src/dsl_v2/verb_registry.rs` | Modify | Register new verbs |

---

## Testing Plan

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_context_binding_resolution_local_first() {
        let mut ctx = ExecutionContext::new();
        ctx.parent_symbols.insert("manco".into(), uuid_1);
        ctx.symbols.insert("manco".into(), uuid_2);  // Local override
        
        assert_eq!(ctx.resolve_binding("manco"), Some(uuid_2));  // Local wins
    }
    
    #[test]
    fn test_context_binding_resolution_falls_back_to_parent() {
        let mut ctx = ExecutionContext::new();
        ctx.parent_symbols.insert("manco".into(), uuid_1);
        // No local "manco"
        
        assert_eq!(ctx.resolve_binding("manco"), Some(uuid_1));  // Parent
    }
    
    #[test]
    fn test_child_context_inherits_parent_bindings() {
        let mut parent = ExecutionContext::new();
        parent.symbols.insert("manco".into(), uuid_1);
        
        let child = parent.child_for_iteration(0);
        
        assert!(child.symbols.is_empty());  // Fresh
        assert_eq!(child.parent_symbols.get("manco"), Some(&uuid_1));  // Inherited
    }
    
    #[test]
    fn test_batch_accumulator_collects_pks() {
        // ... test that each iteration's @cbu is collected
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_template_batch_execution() {
    let pool = test_pool().await;
    seed_allianz_entities(&pool).await;
    
    let dsl = r#"
        (entity.query :type "fund" :name-like "Allianz%" :limit 5 :as @funds)
        (template.batch
          :id "onboard-fund-cbu"
          :source @funds
          :bind-as "fund_entity"
          :shared {:manco_entity "Allianz Global Investors GmbH"
                   :jurisdiction "LU"})
    "#;
    
    let result = execute_dsl(dsl, &pool).await.unwrap();
    
    assert_eq!(result.success_count, 5);
    assert_eq!(result.accumulator.primary_entity_ids.len(), 5);
    
    // Verify CBUs created
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cbus WHERE name ILIKE 'Allianz%'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 5);
}

#[tokio::test]
async fn test_template_batch_with_post_batch_verb() {
    let pool = test_pool().await;
    seed_allianz_entities(&pool).await;
    
    let dsl = r#"
        (entity.query :type "fund" :name-like "Allianz%" :limit 3 :as @funds)
        (template.batch
          :id "onboard-fund-cbu"
          :source @funds
          :bind-as "fund_entity"
          :shared {:manco_entity "Allianz Global Investors GmbH"}
          :as @batch)
        (batch.add-products :cbu-ids @batch :products ["CUSTODY"])
    "#;
    
    let result = execute_dsl(dsl, &pool).await.unwrap();
    
    // Verify products added
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM cbu_products cp 
         JOIN cbus c ON cp.cbu_id = c.cbu_id 
         WHERE c.name ILIKE 'Allianz%'"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    
    assert_eq!(count, 3);  // 3 CBUs × 1 product each
}
```

---

## Success Criteria

1. ✅ `(template.invoke ...)` works in REPL
2. ✅ `(template.batch ...)` executes N iterations
3. ✅ Each iteration runs full pipeline (parse → compile → DAG → execute)
4. ✅ Child context has fresh symbols, inherits parent bindings
5. ✅ Accumulated PKs available in `:then` clause
6. ✅ `batch.pause` / `batch.resume` work across session reconnect
7. ✅ Allianz 205 fund + 2 products test passes via DSL (not CLI)

---

## Implementation Questions (For Review)

These questions arose during codebase analysis. Please clarify before implementation proceeds.

---

### Question 1: Session State Architecture

**Context:** The existing `rust/src/api/session.rs` already has `TemplateExecutionContext` with:
- `TemplatePhase` enum (SelectingTemplate, CollectingSharedParams, CollectingBatchParams, Executing, Complete)
- `BatchItemResult` struct
- `TemplateParamKeySet` for collected entities
- `current_batch_index`, `batch_results`, etc.

The TODO proposes adding a NEW `ActiveBatchState` struct.

**Options:**

**(A) Extend existing `TemplateExecutionContext`**
- Add pause/resume fields (`paused_at`, `status: BatchStatus`)
- Add `remaining_items: Vec<(Uuid, String)>` for resume capability
- Add `source_query: String` for re-query on resume
- Pros: Single source of truth, leverages existing UI integration
- Cons: May conflate agent-driven template execution with DSL-native batch

**(B) Add new `ActiveBatchState` as proposed in TODO**
- Separate from `TemplateExecutionContext`
- Store in `SessionContext.active_batch: Option<ActiveBatchState>`
- Pros: Clean separation between agent workflow and DSL-native batch
- Cons: Two similar-but-different batch state structures

**(C) Hybrid approach**
- Use `TemplateExecutionContext` for agent-driven batch (MCP tools)
- Use `ActiveBatchState` for DSL-native batch (`template.batch` verb)
- Both share `BatchResultAccumulator` type
- Pros: Each use case has purpose-built state
- Cons: More types to maintain

**Recommendation:** Option C - the use cases are different enough to warrant separate state structures, but they should share the result accumulator type.

**Decision needed:** Which option?

---

### Question 2: `:then` Clause Implementation

**Context:** The `:then` clause requires parsing a list of statements as a verb argument:

```clojure
(template.batch
  :id "..."
  :then [
    (cbu.add-product :cbu-id @cbu :product "CUSTODY")
    (cbu.add-product :cbu-id @cbu :product "FUND_ACCOUNTING")
  ])
```

This is a **nested statement list** inside a verb call argument. The current parser handles:
- Literal values (strings, numbers, booleans)
- Symbol references (`@name`)
- Entity references (`("type" "search" "uuid")`)
- Lists of literals `["a" "b" "c"]`
- Maps `{:key "value"}`

But NOT lists of full VerbCalls as argument values.

**Options:**

**(A) Implement full `:then` clause support now**
- Extend parser to handle `AstNode::StatementList` as argument value
- Implement `execute_post_batch()` as shown in TODO
- Implement `@batch_result.cbu_ids` property access syntax
- Pros: Complete feature as designed
- Cons: Parser changes are non-trivial, property access syntax is new

**(B) Defer `:then` clause to Phase 2**
- Implement core batch execution first (`template.batch` without `:then`)
- Use separate verb call for post-batch operations:
  ```clojure
  (template.batch :id "..." :as @batch)
  (batch.add-products :cbu-ids @batch :products ["CUSTODY"])
  ```
- Pros: Faster to working batch execution, simpler parser
- Cons: Two-step workflow instead of single compound statement

**(C) Alternative syntax using string DSL in `:then`**
- `:then` takes a string that is parsed as DSL:
  ```clojure
  (template.batch
    :id "..."
    :then "(batch.add-products :cbu-ids @batch.cbu_ids :products [\"CUSTODY\"])")
  ```
- Pros: No parser changes for nested statements
- Cons: Ugly escaping, harder to write, less type-safe

**Recommendation:** Option B - defer `:then` to Phase 2. Get core batch working first.

**Decision needed:** Defer `:then` clause?

---

### Question 3: Error Handling Modes

**Context:** The TODO specifies three `:on-error` modes:

| Mode | Behavior |
|------|----------|
| `:continue` | Log error, continue to next item |
| `:stop` | Stop batch, return partial results |
| `:rollback` | Rollback ALL items on first error (requires wrapping in transaction) |

**Complexity:**

- `:continue` is straightforward
- `:stop` is straightforward
- `:rollback` requires:
  - Starting a transaction before batch begins
  - All N iterations run within that transaction
  - On error: rollback entire transaction
  - On success: commit
  - **Caveat:** Long-running transaction (177 items × ~10ms each = ~2 seconds holding transaction open)

**Options:**

**(A) Implement all three modes now**
- Full feature parity with TODO spec
- Transaction management for rollback mode
- Pros: Complete feature
- Cons: More complex, potential long transaction issues

**(B) Implement `:continue` and `:stop` only**
- Defer `:rollback` to Phase 2
- Document that rollback requires external transaction management
- Pros: Simpler, faster to implement
- Cons: Missing rollback capability

**(C) Implement all three, but warn on `:rollback` for large batches**
- Log warning if batch size > 50 and mode is rollback
- Suggest using `:stop` for large batches
- Pros: Full feature with guardrails
- Cons: Still has long transaction risk

**Recommendation:** Option A - implement all three. The 177-item Allianz test is a realistic use case, and 2 seconds is acceptable transaction duration.

**Decision needed:** Which error modes to implement?

---

### Question 4: Template Registry Access in ExecutionContext

**Context:** The `TemplateInvokeOp` and `TemplateBatchOp` need access to `TemplateRegistry` to look up templates by ID. Currently:

- `DslExecutor` has `pool: PgPool` and `custom_ops: CustomOperationRegistry`
- `ExecutionContext` has `symbols`, `audit_user`, `transaction_id`, `execution_id`
- `TemplateRegistry` is loaded at startup via `TemplateRegistry::load_from_dir()`

The custom operation receives `(verb_call, ctx, pool)` but NOT the template registry.

**Options:**

**(A) Add `template_registry: Option<Arc<TemplateRegistry>>` to ExecutionContext**
- Set during executor construction
- Custom ops access via `ctx.template_registry()`
- Pros: Clean access pattern
- Cons: ExecutionContext grows

**(B) Add `template_registry` to DslExecutor, pass to custom ops**
- Change custom op signature to include registry
- Pros: Explicit dependency
- Cons: Breaks existing custom op interface

**(C) Use global static (OnceLock) for TemplateRegistry**
- Similar to how `runtime_registry()` works
- Access via `template_registry()` function
- Pros: No signature changes, consistent with verb registry pattern
- Cons: Global state

**(D) Load template on-demand in custom op**
- Custom op calls `TemplateRegistry::load_from_dir()` when needed
- Pros: No architecture changes
- Cons: Inefficient (re-parses YAML each time), breaks caching

**Recommendation:** Option C - use global static. This matches the existing `runtime_registry()` pattern and requires no interface changes.

**Decision needed:** How should template ops access the registry?

---

### Question 5: Batch Source Query Syntax

**Context:** The TODO shows inline SQL as:

```clojure
(template.batch
  :source (query "SELECT entity_id, name FROM entities WHERE ...")
  ...)
```

The `(query "...")` syntax implies a pseudo-verb or special form. The parser would need to handle this.

**Options:**

**(A) Implement `query` as a special form in the parser**
- `(query "SQL")` produces `AstNode::Query(sql_string)`
- Custom handling in TemplateBatchOp
- Pros: Clean DSL syntax
- Cons: Parser changes

**(B) Accept SQL as plain string argument**
- `:source "SELECT entity_id, name FROM entities WHERE ..."`
- Parse and execute as raw SQL
- Pros: No parser changes
- Cons: Less type-safe, easier to confuse with other string args

**(C) Accept a reference to a pre-defined query**
- Queries defined in YAML config or database
- `:source :allianz-funds` or `:source "query:allianz-funds"`
- Pros: Safer (no arbitrary SQL), reusable
- Cons: Less flexible, more setup required

**(D) Accept entity_type + filter criteria**
- `:source {:entity-type "fund" :filter {:name-like "Allianz%"}}`
- Build SQL internally
- Pros: Safe, structured
- Cons: Limited expressiveness

**Recommendation:** Option B for Phase 1 (plain string), with Option C as future enhancement. Arbitrary SQL is needed for the Allianz test case.

**Decision needed:** Query source syntax?

---

## Summary of Recommended Decisions

| Question | Recommendation | Rationale |
|----------|---------------|-----------|
| 1. Session state | Option C (Hybrid) | Separate DSL-native from agent-driven |
| 2. `:then` clause | Option B (Defer) | Get core batch working first |
| 3. Error modes | Option A (All three) | Full feature, acceptable transaction duration |
| 4. Template registry | Option C (Global static) | Matches existing pattern |
| 5. Query syntax | **Verb pattern** | `entity.query` verb, no raw SQL |

---

## DECISIONS CONFIRMED (by architect review)

All recommendations above are **APPROVED**. Proceed with implementation.

### Additional Clarifications:

**Q1 - Session State:** Correct. `TemplateExecutionContext` is for agent-driven (MCP/chat) workflows. `ActiveBatchState` is for DSL-native `template.batch` verb. They share `BatchResultAccumulator`.

**Q2 - `:then` Clause:** Correct to defer. Use two-step workflow for Phase 1:
```clojure
(template.batch :id "..." :as @batch)
(batch.add-products :cbu-ids @batch :products ["CUSTODY"])
```

**Q3 - Error Modes:** Implement all three. **Important caveat:** If a `:rollback` mode batch is paused, do NOT hold transaction open. Convert to `:stop` behavior on pause (commit what's done, save remaining items for resume).

**Q4 - Template Registry:** Use global static `template_registry()` function, matching `runtime_registry()` pattern.

**Q5 - Query Syntax:** **NOT plain SQL.** Use verb pattern consistently. Query is a verb that returns entity list:
```clojure
;; Query via verb
(entity.query :type "fund" :name-like "Allianz%" :as @funds)

;; Batch consumes the binding
(template.batch :id "onboard-fund-cbu" :source @funds ...)
```
No exceptions to the verb pattern. All CRUD/query goes through verbs. This maintains consistency, auditability, and avoids SQL injection concerns.

---

## Implementation Order

Based on decisions above, implement in this order:

1. **Global template registry** (Q4) - Foundation for everything else
2. **ExecutionContext parent hierarchy** (Enhancement 2b) - Required for batch
3. **`entity.query` verb** - Returns list of entity refs for batch source
4. **`template.invoke` verb** (Enhancement 1) - Single invocation
5. **`BatchExecutor` + `BatchResultAccumulator`** (Enhancement 2a/2b) - Core batch
6. **`template.batch` verb** (Enhancement 2a) - DSL-native batch, consumes @binding from entity.query
7. **`ActiveBatchState` + pause/resume** (Enhancement 3) - Session persistence
8. **`batch.add-products` verb** (Enhancement 4 partial) - Post-batch bulk op
9. **Integration tests** - Allianz 205 fund test

**Defer to Phase 2:**
- `:then` clause with nested statements
- `for-each` iteration construct
- Progress streaming (WebSocket/SSE)

---

## References

- Current harness: `rust/src/bin/batch_test_harness.rs`
- Session state: `rust/src/api/session.rs`
- Executor: `rust/src/dsl_v2/executor.rs`
- Template expander: `rust/src/templates/expander.rs`
- EGUI rules: `/EGUI-RULES.md`
