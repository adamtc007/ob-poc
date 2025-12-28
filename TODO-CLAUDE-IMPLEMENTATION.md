# TODO: Unified DSL Execution Model

**For:** Claude (MCP Agent Implementation)
**Context:** Session management + DSL pipeline integration

---

## The Core Insight

All DSL execution uses ONE uniform model. No special cases.

```
DSL contains:     @symbol references (never literal UUIDs)
Bindings contain: symbol → UUID(s)

Cardinality 0  →  Draft (unresolved, valid REPL state, can't execute yet)
Cardinality 1  →  Singleton execution  
Cardinality N  →  Batch expansion (N instances, atomic transaction)
```

**No templates. No macros. No batch mode flag.** Just substitution based on binding cardinality.

---

## Example Flows

### Singleton (N=1)
```
Agent submits:
  dsl: (cbu.add-product :cbu-id @target :product "CUSTODY")
  bindings: { "target": ["uuid-123"] }

Pipeline:
  → Substitutes @target → uuid-123
  → Executes 1 statement
```

### Batch (N=46)
```
Agent submits:
  dsl: (cbu.add-product :cbu-id @target :product "CUSTODY")
  bindings: { "target": ["uuid-1", "uuid-2", ..., "uuid-46"] }

Pipeline:
  → Generates 46 statements (one per UUID)
  → Executes ALL in one atomic transaction
```

### Draft (N=0) - Valid REPL State
```
Agent submits:
  dsl: (cbu.assign-role :cbu-id @cbu :entity-id @person :role "DIRECTOR")
  bindings: { "cbu": ["uuid-123"], "person": [] }  ← empty = unresolved

Pipeline:
  → Returns: Draft state, @person unresolved
  → Agent can later call: bind("person", [uuid-456])
  → Then execute when ready
```

---

## Files to Create/Modify

| File | Action | Purpose |
|------|--------|---------|
| `rust/src/dsl_v2/domain_context.rs` | CREATE | Track active domain/entity |
| `rust/src/dsl_v2/submission.rs` | CREATE | Uniform submission model |
| `rust/src/dsl_v2/mod.rs` | MODIFY | Export new modules |
| `rust/src/dsl_v2/executor.rs` | MODIFY | Add execute_submission() |
| `rust/src/api/session.rs` | MODIFY | Integrate with SessionContext |
| `rust/src/mcp/tools.rs` | MODIFY | Update execute tool |
| `rust/src/mcp/session.rs` | MODIFY | Bridge to SessionStore |

---

## Part 1: Domain Context

**File:** `rust/src/dsl_v2/domain_context.rs`

Tracks "where we are" in multi-step workflows. Supports push/pop for nested operations.

```rust
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ActiveDomain {
    #[default]
    None,
    Cbu,
    KycCase,
    OnboardingRequest,
    EntityWorkstream,
    UboGraph,
    TradingProfile,
    Contract,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterationContext {
    pub index: usize,
    pub iteration_key: String,
    pub source_entity_id: Uuid,
    pub source_entity_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DomainContextFrame {
    domain: ActiveDomain,
    cbu_id: Option<Uuid>,
    case_id: Option<Uuid>,
    request_id: Option<Uuid>,
    entity_id: Option<Uuid>,
    push_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DomainContext {
    pub active_domain: ActiveDomain,
    pub active_cbu_id: Option<Uuid>,
    pub active_cbu_name: Option<String>,
    pub active_case_id: Option<Uuid>,
    pub active_request_id: Option<Uuid>,
    pub active_entity_id: Option<Uuid>,
    pub iteration: Option<IterationContext>,
    
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    context_stack: Vec<DomainContextFrame>,
}

impl DomainContext {
    pub fn new() -> Self { Self::default() }
    
    pub fn for_cbu(cbu_id: Uuid, name: Option<String>) -> Self {
        Self {
            active_domain: ActiveDomain::Cbu,
            active_cbu_id: Some(cbu_id),
            active_cbu_name: name,
            ..Default::default()
        }
    }
    
    pub fn push_domain(&mut self, domain: ActiveDomain, reason: impl Into<String>) {
        let frame = DomainContextFrame {
            domain: self.active_domain,
            cbu_id: self.active_cbu_id,
            case_id: self.active_case_id,
            request_id: self.active_request_id,
            entity_id: self.active_entity_id,
            push_reason: reason.into(),
        };
        self.context_stack.push(frame);
        self.active_domain = domain;
    }
    
    pub fn pop_domain(&mut self) -> bool {
        if let Some(frame) = self.context_stack.pop() {
            self.active_domain = frame.domain;
            self.active_cbu_id = frame.cbu_id;
            self.active_case_id = frame.case_id;
            self.active_request_id = frame.request_id;
            self.active_entity_id = frame.entity_id;
            true
        } else {
            false
        }
    }
    
    pub fn stack_depth(&self) -> usize { self.context_stack.len() }
    pub fn in_batch_iteration(&self) -> bool { self.iteration.is_some() }
    
    pub fn enter_iteration(&mut self, index: usize, key: String, entity_id: Uuid, entity_type: String) {
        self.iteration = Some(IterationContext {
            index, iteration_key: key, source_entity_id: entity_id, source_entity_type: entity_type,
        });
    }
    
    pub fn exit_iteration(&mut self) { self.iteration = None; }
    
    pub fn child_for_iteration(&self, index: usize, key: String, entity_id: Uuid, entity_type: String) -> Self {
        Self {
            active_domain: self.active_domain,
            active_cbu_id: self.active_cbu_id,
            active_cbu_name: self.active_cbu_name.clone(),
            active_case_id: self.active_case_id,
            active_request_id: self.active_request_id,
            active_entity_id: None,
            iteration: Some(IterationContext {
                index, iteration_key: key, source_entity_id: entity_id, source_entity_type: entity_type,
            }),
            context_stack: Vec::new(),
        }
    }
}
```

---

## Part 2: Symbol Binding

**File:** `rust/src/dsl_v2/submission.rs`

```rust
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SymbolBinding {
    pub ids: Vec<Uuid>,
    #[serde(default)]
    pub names: Vec<String>,
    #[serde(default)]
    pub entity_type: Option<String>,
}

impl SymbolBinding {
    pub fn unresolved() -> Self { Self::default() }
    
    pub fn singleton(id: Uuid) -> Self {
        Self { ids: vec![id], names: vec![], entity_type: None }
    }
    
    pub fn singleton_named(id: Uuid, name: String) -> Self {
        Self { ids: vec![id], names: vec![name], entity_type: None }
    }
    
    pub fn multiple(ids: Vec<Uuid>) -> Self {
        Self { ids, names: vec![], entity_type: None }
    }
    
    pub fn multiple_named(items: Vec<(Uuid, String)>) -> Self {
        let (ids, names) = items.into_iter().unzip();
        Self { ids, names, entity_type: None }
    }
    
    pub fn with_type(mut self, t: String) -> Self {
        self.entity_type = Some(t);
        self
    }
    
    pub fn len(&self) -> usize { self.ids.len() }
    pub fn is_empty(&self) -> bool { self.ids.is_empty() }
    pub fn is_unresolved(&self) -> bool { self.ids.is_empty() }
    pub fn is_singleton(&self) -> bool { self.ids.len() == 1 }
    pub fn is_multiple(&self) -> bool { self.ids.len() > 1 }
    
    pub fn id(&self) -> Uuid {
        assert!(self.is_singleton());
        self.ids[0]
    }
    
    pub fn add(&mut self, id: Uuid, name: Option<String>) {
        self.ids.push(id);
        if let Some(n) = name { self.names.push(n); }
    }
    
    pub fn remove(&mut self, id: Uuid) -> bool {
        if let Some(idx) = self.ids.iter().position(|i| *i == id) {
            self.ids.remove(idx);
            if idx < self.names.len() { self.names.remove(idx); }
            true
        } else { false }
    }
}
```

---

## Part 3: DSL Submission

**File:** `rust/src/dsl_v2/submission.rs` (continue)

```rust
use super::ast::{Statement, AstNode, Literal, VerbCall, Argument};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslSubmission {
    pub statements: Vec<Statement>,
    pub bindings: HashMap<String, SymbolBinding>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SubmissionState {
    Draft { unresolved: Vec<String> },
    Ready,
    ReadyWithWarning { message: String, iterations: usize, total_ops: usize },
    TooLarge { message: String, suggestion: String },
}

#[derive(Debug, Clone)]
pub struct SubmissionLimits {
    pub warn_iterations: usize,    // 100
    pub max_iterations: usize,     // 10_000
    pub warn_total_ops: usize,     // 500
    pub max_total_ops: usize,      // 50_000
    pub chunk_size: usize,         // 100
}

impl Default for SubmissionLimits {
    fn default() -> Self {
        Self {
            warn_iterations: 100,
            max_iterations: 10_000,
            warn_total_ops: 500,
            max_total_ops: 50_000,
            chunk_size: 100,
        }
    }
}

impl DslSubmission {
    pub fn new(statements: Vec<Statement>) -> Self {
        Self { statements, bindings: HashMap::new() }
    }
    
    pub fn bind(mut self, symbol: impl Into<String>, binding: SymbolBinding) -> Self {
        self.bindings.insert(symbol.into(), binding);
        self
    }
    
    pub fn bind_one(mut self, symbol: impl Into<String>, id: Uuid) -> Self {
        self.bindings.insert(symbol.into(), SymbolBinding::singleton(id));
        self
    }
    
    pub fn bind_many(mut self, symbol: impl Into<String>, ids: Vec<Uuid>) -> Self {
        self.bindings.insert(symbol.into(), SymbolBinding::multiple(ids));
        self
    }
    
    pub fn set_binding(&mut self, symbol: &str, binding: SymbolBinding) {
        self.bindings.insert(symbol.to_string(), binding);
    }
    
    pub fn add_to_binding(&mut self, symbol: &str, id: Uuid, name: Option<String>) {
        self.bindings.entry(symbol.to_string())
            .or_insert_with(SymbolBinding::unresolved)
            .add(id, name);
    }
    
    pub fn remove_from_binding(&mut self, symbol: &str, id: Uuid) -> bool {
        self.bindings.get_mut(symbol).map(|b| b.remove(id)).unwrap_or(false)
    }
    
    /// Symbols referenced in DSL
    pub fn symbols_in_dsl(&self) -> Vec<String> {
        let mut symbols = vec![];
        for stmt in &self.statements {
            collect_symbols(stmt, &mut symbols);
        }
        symbols.sort();
        symbols.dedup();
        symbols
    }
    
    /// Symbols with cardinality 0
    pub fn unresolved_symbols(&self) -> Vec<String> {
        self.symbols_in_dsl().into_iter()
            .filter(|s| self.bindings.get(s).map(|b| b.is_unresolved()).unwrap_or(true))
            .collect()
    }
    
    pub fn has_unresolved(&self) -> bool { !self.unresolved_symbols().is_empty() }
    pub fn is_resolved(&self) -> bool { self.unresolved_symbols().is_empty() }
    
    /// Find symbol with cardinality > 1 (error if multiple)
    pub fn iteration_symbol(&self) -> Result<Option<String>, SubmissionError> {
        let multi: Vec<_> = self.bindings.iter()
            .filter(|(_, b)| b.is_multiple())
            .map(|(k, _)| k.clone())
            .collect();
        match multi.len() {
            0 => Ok(None),
            1 => Ok(Some(multi.into_iter().next().unwrap())),
            _ => Err(SubmissionError::MultipleIterationSymbols(multi)),
        }
    }
    
    pub fn is_batch(&self) -> bool {
        self.bindings.values().any(|b| b.is_multiple())
    }
    
    pub fn iteration_count(&self) -> usize {
        self.bindings.values().map(|b| b.len()).max().unwrap_or(1).max(1)
    }
    
    pub fn total_operations(&self) -> usize {
        self.iteration_count() * self.statements.len()
    }
    
    /// Get current state
    pub fn state(&self, limits: &SubmissionLimits) -> SubmissionState {
        let unresolved = self.unresolved_symbols();
        if !unresolved.is_empty() {
            return SubmissionState::Draft { unresolved };
        }
        
        let iterations = self.iteration_count();
        let total_ops = self.total_operations();
        
        if iterations > limits.max_iterations {
            return SubmissionState::TooLarge {
                message: format!("{} items exceeds max {}", iterations, limits.max_iterations),
                suggestion: "Refine selection".into(),
            };
        }
        if total_ops > limits.max_total_ops {
            return SubmissionState::TooLarge {
                message: format!("{} ops exceeds max {}", total_ops, limits.max_total_ops),
                suggestion: "Reduce items or ops".into(),
            };
        }
        if iterations > limits.warn_iterations || total_ops > limits.warn_total_ops {
            return SubmissionState::ReadyWithWarning {
                message: format!("{} items × {} ops = {}", iterations, self.statements.len(), total_ops),
                iterations,
                total_ops,
            };
        }
        SubmissionState::Ready
    }
    
    pub fn can_execute(&self, limits: &SubmissionLimits) -> bool {
        matches!(self.state(limits), SubmissionState::Ready | SubmissionState::ReadyWithWarning { .. })
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum SubmissionError {
    #[error("Unresolved symbols: {0:?}")]
    UnresolvedSymbols(Vec<String>),
    #[error("Multiple iteration symbols: {0:?}")]
    MultipleIterationSymbols(Vec<String>),
    #[error("Execution error: {0}")]
    ExecutionError(String),
}

fn collect_symbols(stmt: &Statement, out: &mut Vec<String>) {
    if let Statement::VerbCall(vc) = stmt {
        for arg in &vc.arguments {
            collect_symbols_node(&arg.value, out);
        }
    }
}

fn collect_symbols_node(node: &AstNode, out: &mut Vec<String>) {
    match node {
        AstNode::Symbol(s) => out.push(s.clone()),
        AstNode::List(items) => items.iter().for_each(|n| collect_symbols_node(n, out)),
        _ => {}
    }
}
```

---

## Part 4: Expansion

**File:** `rust/src/dsl_v2/submission.rs` (continue)

```rust
#[derive(Debug)]
pub struct ExpandedSubmission {
    pub iterations: Vec<IterationStatements>,
    pub is_batch: bool,
    pub total_statements: usize,
}

#[derive(Debug, Clone)]
pub struct IterationStatements {
    pub index: usize,
    pub iteration_key: Option<IterationKey>,
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub struct IterationKey {
    pub symbol: String,
    pub id: Uuid,
    pub name: Option<String>,
}

impl DslSubmission {
    pub fn expand(&self) -> Result<ExpandedSubmission, SubmissionError> {
        let unresolved = self.unresolved_symbols();
        if !unresolved.is_empty() {
            return Err(SubmissionError::UnresolvedSymbols(unresolved));
        }
        
        let iter_symbol = self.iteration_symbol()?;
        
        // Fixed bindings (singletons)
        let fixed: HashMap<String, Uuid> = self.bindings.iter()
            .filter(|(_, b)| b.is_singleton())
            .map(|(k, b)| (k.clone(), b.id()))
            .collect();
        
        let iterations = match iter_symbol {
            None => {
                vec![IterationStatements {
                    index: 0,
                    iteration_key: None,
                    statements: substitute_all(&self.statements, &fixed),
                }]
            }
            Some(symbol) => {
                let binding = self.bindings.get(&symbol).unwrap();
                binding.ids.iter().enumerate().map(|(idx, id)| {
                    let mut bindings = fixed.clone();
                    bindings.insert(symbol.clone(), *id);
                    IterationStatements {
                        index: idx,
                        iteration_key: Some(IterationKey {
                            symbol: symbol.clone(),
                            id: *id,
                            name: binding.names.get(idx).cloned(),
                        }),
                        statements: substitute_all(&self.statements, &bindings),
                    }
                }).collect()
            }
        };
        
        let total = iterations.iter().map(|i| i.statements.len()).sum();
        Ok(ExpandedSubmission { is_batch: iter_symbol.is_some(), iterations, total_statements: total })
    }
}

fn substitute_all(statements: &[Statement], bindings: &HashMap<String, Uuid>) -> Vec<Statement> {
    statements.iter().map(|s| substitute_statement(s, bindings)).collect()
}

fn substitute_statement(stmt: &Statement, bindings: &HashMap<String, Uuid>) -> Statement {
    match stmt {
        Statement::VerbCall(vc) => Statement::VerbCall(VerbCall {
            domain: vc.domain.clone(),
            verb: vc.verb.clone(),
            arguments: vc.arguments.iter()
                .map(|arg| Argument {
                    key: arg.key.clone(),
                    value: substitute_node(&arg.value, bindings),
                })
                .collect(),
            binding: vc.binding.clone(),
        }),
        other => other.clone(),
    }
}

fn substitute_node(node: &AstNode, bindings: &HashMap<String, Uuid>) -> AstNode {
    match node {
        AstNode::Symbol(s) => {
            bindings.get(s).map(|id| AstNode::Literal(Literal::Uuid(*id)))
                .unwrap_or_else(|| node.clone())
        }
        AstNode::List(items) => {
            AstNode::List(items.iter().map(|n| substitute_node(n, bindings)).collect())
        }
        _ => node.clone(),
    }
}
```

---

## Part 5: Executor Integration

**File:** `rust/src/dsl_v2/executor.rs` (ADD to existing)

```rust
use super::submission::{DslSubmission, ExpandedSubmission, SubmissionError, SubmissionLimits};
use super::domain_context::DomainContext;

#[derive(Debug, Serialize)]
pub struct SubmissionResult {
    pub iterations: Vec<IterationResult>,
    pub is_batch: bool,
    pub total_executed: usize,
}

#[derive(Debug, Serialize)]
pub struct IterationResult {
    pub index: usize,
    pub success: bool,
    pub bindings: HashMap<String, Uuid>,
    pub error: Option<String>,
}

impl DslExecutor {
    /// Unified entry point for all DSL execution
    pub async fn execute_submission(
        &self,
        submission: &DslSubmission,
        domain_ctx: &mut DomainContext,
        limits: &SubmissionLimits,
    ) -> Result<SubmissionResult, SubmissionError> {
        let expanded = submission.expand()?;
        
        tracing::info!(
            is_batch = expanded.is_batch,
            iterations = expanded.iterations.len(),
            total = expanded.total_statements,
            "Executing submission"
        );
        
        // Execute atomically
        let mut tx = self.pool.begin().await
            .map_err(|e| SubmissionError::ExecutionError(e.to_string()))?;
        
        let mut results = vec![];
        
        for iteration in &expanded.iterations {
            if let Some(ref key) = iteration.iteration_key {
                domain_ctx.enter_iteration(
                    iteration.index,
                    key.name.clone().unwrap_or_else(|| key.id.to_string()),
                    key.id,
                    key.symbol.clone(),
                );
            }
            
            let mut exec_ctx = ExecutionContext::from_domain(domain_ctx);
            match self.execute_statements_in_tx(&iteration.statements, &mut exec_ctx, &mut tx).await {
                Ok(bindings) => {
                    results.push(IterationResult {
                        index: iteration.index,
                        success: true,
                        bindings,
                        error: None,
                    });
                }
                Err(e) => {
                    tx.rollback().await.ok();
                    return Err(SubmissionError::ExecutionError(format!(
                        "Iteration {} failed: {}", iteration.index, e
                    )));
                }
            }
            
            if iteration.iteration_key.is_some() {
                domain_ctx.exit_iteration();
            }
        }
        
        tx.commit().await
            .map_err(|e| SubmissionError::ExecutionError(e.to_string()))?;
        
        Ok(SubmissionResult {
            is_batch: expanded.is_batch,
            total_executed: expanded.total_statements,
            iterations: results,
        })
    }
}
```

---

## Part 6: MCP Tool Update

**File:** `rust/src/mcp/tools.rs` (MODIFY execute tool)

```rust
#[derive(Debug, Deserialize)]
pub struct ExecuteDslInput {
    pub dsl: String,
    
    /// Bindings: symbol → UUID string, array of UUIDs, or empty array (unresolved)
    #[serde(default)]
    pub bindings: HashMap<String, serde_json::Value>,
    
    #[serde(default)]
    pub session_id: Option<String>,
    
    /// Skip warning confirmation
    #[serde(default)]
    pub confirmed: bool,
}

#[derive(Debug, Serialize)]
pub struct ExecuteDslOutput {
    pub state: SubmissionState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<SubmissionResult>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unresolved: Vec<String>,
    pub submission_id: String,
}

pub async fn handle_execute_dsl(input: ExecuteDslInput, state: &ToolState) -> Result<ExecuteDslOutput> {
    let program = parse_program(&input.dsl)?;
    let mut submission = DslSubmission::new(program.statements);
    
    // Parse bindings
    for (symbol, value) in input.bindings {
        let binding = parse_binding_value(value)?;
        submission.set_binding(&symbol, binding);
    }
    
    let limits = SubmissionLimits::default();
    let sub_state = submission.state(&limits);
    
    match &sub_state {
        SubmissionState::Draft { unresolved } => {
            // Store draft, return for resolution
            let sub_id = store_submission(&submission, state).await?;
            Ok(ExecuteDslOutput {
                state: sub_state,
                result: None,
                unresolved: unresolved.clone(),
                submission_id: sub_id,
            })
        }
        SubmissionState::TooLarge { .. } => {
            Ok(ExecuteDslOutput {
                state: sub_state,
                result: None,
                unresolved: vec![],
                submission_id: String::new(),
            })
        }
        SubmissionState::ReadyWithWarning { .. } if !input.confirmed => {
            let sub_id = store_submission(&submission, state).await?;
            Ok(ExecuteDslOutput {
                state: sub_state,
                result: None,
                unresolved: vec![],
                submission_id: sub_id,
            })
        }
        SubmissionState::Ready | SubmissionState::ReadyWithWarning { .. } => {
            // Execute!
            let mut domain_ctx = get_domain_context(input.session_id.as_deref(), state).await?;
            let executor = DslExecutor::new(state.pool.clone());
            let result = executor.execute_submission(&submission, &mut domain_ctx, &limits).await?;
            
            Ok(ExecuteDslOutput {
                state: SubmissionState::Ready,
                result: Some(result),
                unresolved: vec![],
                submission_id: String::new(),
            })
        }
    }
}

/// Bind tool - resolve symbols on pending submission
#[derive(Debug, Deserialize)]
pub struct BindInput {
    pub submission_id: String,
    pub symbol: String,
    pub ids: Vec<String>,
    #[serde(default)]
    pub names: Vec<String>,
}

pub async fn handle_bind(input: BindInput, state: &ToolState) -> Result<ExecuteDslOutput> {
    let mut submission = get_submission(&input.submission_id, state).await?;
    
    let ids: Vec<Uuid> = input.ids.iter()
        .map(|s| Uuid::parse_str(s))
        .collect::<Result<_, _>>()?;
    
    let binding = if input.names.is_empty() {
        SymbolBinding::multiple(ids)
    } else {
        SymbolBinding::multiple_named(ids.into_iter().zip(input.names).collect())
    };
    
    submission.set_binding(&input.symbol, binding);
    store_submission(&submission, state).await?;
    
    Ok(ExecuteDslOutput {
        state: submission.state(&SubmissionLimits::default()),
        result: None,
        unresolved: submission.unresolved_symbols(),
        submission_id: input.submission_id,
    })
}

fn parse_binding_value(value: serde_json::Value) -> Result<SymbolBinding> {
    match value {
        serde_json::Value::Null | serde_json::Value::Array(ref a) if a.is_empty() => {
            Ok(SymbolBinding::unresolved())
        }
        serde_json::Value::String(s) => {
            Ok(SymbolBinding::singleton(Uuid::parse_str(&s)?))
        }
        serde_json::Value::Array(arr) => {
            let mut ids = vec![];
            let mut names = vec![];
            for item in arr {
                match item {
                    serde_json::Value::String(s) => ids.push(Uuid::parse_str(&s)?),
                    serde_json::Value::Object(obj) => {
                        if let Some(id) = obj.get("id").and_then(|v| v.as_str()) {
                            ids.push(Uuid::parse_str(id)?);
                            if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
                                names.push(name.to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(SymbolBinding { ids, names, entity_type: None })
        }
        _ => Ok(SymbolBinding::unresolved()),
    }
}
```

---

## Part 7: Module Exports

**File:** `rust/src/dsl_v2/mod.rs`

```rust
pub mod domain_context;
pub mod submission;

pub use domain_context::{ActiveDomain, DomainContext, IterationContext};
pub use submission::{
    DslSubmission, SymbolBinding, SubmissionState, SubmissionLimits,
    ExpandedSubmission, SubmissionError, IterationStatements, IterationKey,
};
```

---

## Summary

| Cardinality | State | Behavior |
|-------------|-------|----------|
| 0 | Draft | Valid REPL state. Bind symbols later. |
| 1 | Ready | Singleton execution |
| N | Ready | Batch: N iterations, atomic transaction |
| N > 10,000 | TooLarge | Reject, suggest refinement |

**Agent workflow:**
1. Generate DSL with @symbols
2. Submit with bindings (0, 1, or N UUIDs per symbol)
3. If Draft → bind symbols via separate calls
4. If Ready → executes atomically
5. If Warning → confirm then execute

**No special cases.** Cardinality drives everything.

---

## Verification

```bash
cd /Users/adamtc007/Developer/ob-poc/rust
cargo check --lib
cargo test domain_context -- --nocapture
cargo test submission -- --nocapture
```
