# 035: REPL Session Implementation Plan

> **Status:** Ready to Implement
> **Depends on:** `034-repl-session-phased-execution.md` (design)
> **Estimated effort:** ~12-16 hours

## Overview

Implement the REPL session state machine and phased execution model designed in 034.

## Phase 1: Enhance Session Record (~3 hours)

### 1.1 Add SessionState enum

**File:** `rust/src/session/cbu_session.rs`

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SessionState {
    Empty,
    Scoped,
    Templated { confirmed: bool },
    Generated,
    Parsed,
    Resolving { remaining: usize },
    Ready,
    Executing { completed: usize, total: usize },
    Executed { success: bool },
}
```

### 1.2 Add scope and sheet fields to CbuSession

```rust
pub struct CbuSession {
    // Existing
    pub id: Uuid,
    pub name: Option<String>,
    pub state: CbuSessionState,  // cbu_ids: HashSet<Uuid>
    history: Vec<CbuSessionState>,
    future: Vec<CbuSessionState>,
    
    // NEW: State machine
    pub session_state: SessionState,
    
    // NEW: Scope layer
    pub scope: Option<GraphScope>,
    pub scope_dsl: Vec<String>,
    
    // NEW: Intent layer
    pub template_dsl: Option<String>,
    pub target_entity_type: Option<String>,
    pub intent_confirmed: bool,
    
    // NEW: Sheet layer
    pub sheet: Option<DslSheet>,
}
```

### 1.3 Add DslSheet and SessionDslStatement

**File:** `rust/src/session/dsl_sheet.rs` (new)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslSheet {
    pub id: Uuid,
    pub statements: Vec<SessionDslStatement>,
    pub validation: Option<ValidationResult>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDslStatement {
    pub index: usize,
    pub source: String,
    pub dag_depth: usize,
    pub produces: Option<String>,
    pub consumes: Vec<String>,
    pub resolved_args: HashMap<String, Uuid>,
    pub returned_pk: Option<Uuid>,
    pub status: StatementStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StatementStatus {
    Pending,
    Parsed,
    Resolved,
    Executing,
    Success,
    Failed { error: String },
    Skipped { blocked_by: usize },
}
```

### 1.4 Add state transition methods

```rust
impl CbuSession {
    pub fn set_scope(&mut self, scope_dsl: Vec<String>, scope: GraphScope) -> Result<()>;
    pub fn set_template(&mut self, template_dsl: String, target: String) -> Result<()>;
    pub fn confirm_intent(&mut self) -> Result<()>;
    pub fn set_generated(&mut self, sheet: DslSheet) -> Result<()>;
    pub fn set_parsed(&mut self, unresolved_count: usize) -> Result<()>;
    pub fn resolve_ref(&mut self, index: usize, uuid: Uuid) -> Result<()>;
    pub fn set_ready(&mut self) -> Result<()>;
    pub fn set_executing(&mut self, total: usize) -> Result<()>;
    pub fn mark_executed(&mut self, success: bool) -> Result<()>;
    pub fn reset_to_scoped(&mut self) -> Result<()>;
}
```

### 1.5 Update DB schema

**File:** `migrations/035_session_sheet.sql`

```sql
-- Add columns to sessions table
ALTER TABLE "ob-poc".sessions 
ADD COLUMN IF NOT EXISTS session_state TEXT DEFAULT 'empty',
ADD COLUMN IF NOT EXISTS scope JSONB,
ADD COLUMN IF NOT EXISTS scope_dsl TEXT[],
ADD COLUMN IF NOT EXISTS template_dsl TEXT,
ADD COLUMN IF NOT EXISTS sheet JSONB;

-- Index for querying by state
CREATE INDEX IF NOT EXISTS idx_sessions_state ON "ob-poc".sessions(session_state);
```

---

## Phase 2: DAG Phase Extraction (~2 hours)

### 2.1 Add ExecutionPhase struct

**File:** `rust/src/dsl_v2/topo_sort.rs`

```rust
#[derive(Debug, Clone)]
pub struct ExecutionPhase {
    pub depth: usize,
    pub statement_indices: Vec<usize>,
    pub produces: Vec<String>,
    pub consumes: Vec<String>,
}
```

### 2.2 Add compute_phases() to TopoSortResult

```rust
impl TopoSortResult {
    /// Extract execution phases from sorted statements
    pub fn compute_phases(&self, deps: &HashMap<usize, HashSet<usize>>) -> Vec<ExecutionPhase> {
        // Compute depth for each statement
        let depths = self.compute_depths(deps);
        
        // Group by depth
        let max_depth = depths.values().max().copied().unwrap_or(0);
        let mut phases: Vec<ExecutionPhase> = (0..=max_depth)
            .map(|d| ExecutionPhase {
                depth: d,
                statement_indices: vec![],
                produces: vec![],
                consumes: vec![],
            })
            .collect();
        
        for (idx, depth) in &depths {
            phases[*depth].statement_indices.push(*idx);
        }
        
        // Compute produces/consumes per phase
        self.annotate_phases(&mut phases);
        
        phases
    }
    
    fn compute_depths(&self, deps: &HashMap<usize, HashSet<usize>>) -> HashMap<usize, usize> {
        let mut depths = HashMap::new();
        for idx in 0..self.program.statements.len() {
            self.compute_depth_recursive(idx, deps, &mut depths, &mut HashSet::new());
        }
        depths
    }
    
    fn compute_depth_recursive(
        &self,
        idx: usize,
        deps: &HashMap<usize, HashSet<usize>>,
        depths: &mut HashMap<usize, usize>,
        visiting: &mut HashSet<usize>,
    ) -> usize {
        if let Some(&d) = depths.get(&idx) {
            return d;
        }
        if visiting.contains(&idx) {
            return 0; // Cycle - handled elsewhere
        }
        visiting.insert(idx);
        
        let max_dep_depth = deps.get(&idx)
            .map(|d| d.iter()
                .map(|&dep_idx| self.compute_depth_recursive(dep_idx, deps, depths, visiting) + 1)
                .max()
                .unwrap_or(0))
            .unwrap_or(0);
        
        visiting.remove(&idx);
        depths.insert(idx, max_dep_depth);
        max_dep_depth
    }
}
```

### 2.3 Expose deps from topological_sort

Currently `deps` is internal. Either:
- Return it in `TopoSortResult`, or
- Compute phases inside `topological_sort` before returning

Recommend adding to result:

```rust
pub struct TopoSortResult {
    pub program: Program,
    pub reordered: bool,
    pub index_map: Vec<usize>,
    pub lifecycle_diagnostics: Vec<PlannerDiagnostic>,
    // NEW
    pub phases: Vec<ExecutionPhase>,
}
```

---

## Phase 3: Phased Executor (~4 hours)

### 3.1 Add SheetExecutor

**File:** `rust/src/dsl_v2/sheet_executor.rs` (new)

```rust
pub struct SheetExecutor<'a> {
    pool: &'a PgPool,
    emitter: Option<SharedEmitter>,
}

impl<'a> SheetExecutor<'a> {
    pub fn new(pool: &'a PgPool, emitter: Option<SharedEmitter>) -> Self {
        Self { pool, emitter }
    }
    
    pub async fn execute_phased(
        &self,
        sheet: &mut DslSheet,
        phases: &[ExecutionPhase],
    ) -> Result<SheetExecutionResult> {
        let mut symbols: HashMap<String, Uuid> = HashMap::new();
        let mut results: Vec<StatementResult> = vec![];
        let started_at = Utc::now();
        let total = sheet.statements.len();
        
        let mut tx = self.pool.begin().await?;
        let mut phases_completed = 0;
        let mut overall_status = SheetStatus::Success;
        
        'phases: for phase in phases {
            for &stmt_idx in &phase.statement_indices {
                let stmt = &mut sheet.statements[stmt_idx];
                
                // Substitute symbols
                let resolved = match self.substitute_symbols(&stmt.source, &symbols) {
                    Ok(r) => r,
                    Err(e) => {
                        stmt.status = StatementStatus::Failed { error: e.to_string() };
                        overall_status = SheetStatus::Failed;
                        self.mark_downstream_skipped(sheet, stmt_idx, phases);
                        break 'phases;
                    }
                };
                
                // Execute
                stmt.status = StatementStatus::Executing;
                match self.execute_statement(&mut tx, &resolved).await {
                    Ok(result) => {
                        if let Some(ref symbol) = stmt.produces {
                            if let Some(pk) = result.produced_pk {
                                symbols.insert(symbol.clone(), pk);
                                stmt.returned_pk = Some(pk);
                            }
                        }
                        stmt.status = StatementStatus::Success;
                        results.push(StatementResult::success(stmt_idx, stmt, result));
                    }
                    Err(e) => {
                        stmt.status = StatementStatus::Failed { error: e.to_string() };
                        overall_status = SheetStatus::Failed;
                        results.push(StatementResult::failed(stmt_idx, stmt, e));
                        self.mark_downstream_skipped(sheet, stmt_idx, phases);
                        break 'phases;
                    }
                }
            }
            phases_completed += 1;
        }
        
        // Commit or rollback
        if overall_status == SheetStatus::Success {
            tx.commit().await?;
        } else {
            tx.rollback().await?;
        }
        
        Ok(SheetExecutionResult {
            session_id: sheet.id, // or pass separately
            sheet_id: sheet.id,
            overall_status,
            phases_completed,
            phases_total: phases.len(),
            statements: results,
            started_at,
            completed_at: Utc::now(),
        })
    }
    
    fn substitute_symbols(&self, source: &str, symbols: &HashMap<String, Uuid>) -> Result<String> {
        // Replace @symbol with UUID
        let mut result = source.to_string();
        for (symbol, uuid) in symbols {
            let pattern = format!("@{}", symbol);
            result = result.replace(&pattern, &format!("\"{}\"", uuid));
        }
        // Check for remaining unresolved @symbols
        if result.contains('@') {
            // Extract which symbols are missing
            return Err(anyhow!("Unresolved symbols in: {}", result));
        }
        Ok(result)
    }
    
    fn mark_downstream_skipped(
        &self, 
        sheet: &mut DslSheet, 
        failed_idx: usize,
        phases: &[ExecutionPhase],
    ) {
        // Find all statements that depend on the failed one (directly or transitively)
        // Mark them as Skipped
        for phase in phases {
            for &idx in &phase.statement_indices {
                if idx != failed_idx {
                    let stmt = &sheet.statements[idx];
                    if matches!(stmt.status, StatementStatus::Pending | StatementStatus::Parsed | StatementStatus::Resolved) {
                        // Check if this statement consumes something produced by failed
                        // For simplicity, mark all pending in later phases as skipped
                        sheet.statements[idx].status = StatementStatus::Skipped { blocked_by: failed_idx };
                    }
                }
            }
        }
    }
}
```

### 3.2 Add result types

**File:** `rust/src/dsl_v2/sheet_executor.rs` (continued)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetExecutionResult {
    pub session_id: Uuid,
    pub sheet_id: Uuid,
    pub overall_status: SheetStatus,
    pub phases_completed: usize,
    pub phases_total: usize,
    pub statements: Vec<StatementResult>,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SheetStatus {
    Success,
    Failed,
    RolledBack,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatementResult {
    pub index: usize,
    pub dag_depth: usize,
    pub source: String,
    pub status: StatementStatus,
    pub error: Option<StatementError>,
    pub returned_pk: Option<Uuid>,
    pub execution_time_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatementError {
    pub code: ErrorCode,
    pub message: String,
    pub detail: Option<String>,
    pub span: Option<Span>,
    pub blocked_by: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorCode {
    SyntaxError,
    UnresolvedSymbol,
    AmbiguousEntity,
    EntityNotFound,
    TypeMismatch,
    MissingRequired,
    InvalidArgument,
    DbConstraint,
    DbConnection,
    Timeout,
    Blocked,
    CyclicDependency,
}
```

---

## Phase 4: Error Reporting (~2 hours)

### 4.1 Add span tracking to AST

Ensure parser captures source spans for error highlighting.

**Check:** `rust/crates/dsl-core/src/parser.rs` - verify `Span` is populated.

### 4.2 Add error mapping

Map internal errors to `ErrorCode`:

```rust
impl From<sqlx::Error> for ErrorCode {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::Database(db_err) => {
                if db_err.constraint().is_some() {
                    ErrorCode::DbConstraint
                } else {
                    ErrorCode::DbConnection
                }
            }
            _ => ErrorCode::DbConnection,
        }
    }
}
```

### 4.3 Add audit table

**File:** `migrations/035_session_sheet.sql` (append)

```sql
CREATE TABLE IF NOT EXISTS "ob-poc".sheet_execution_audit (
    execution_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL,
    sheet_id UUID NOT NULL,
    source_dsl TEXT NOT NULL,
    dag_analysis JSONB,
    result JSONB NOT NULL,
    submitted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    submitted_by TEXT,
    
    CONSTRAINT fk_session FOREIGN KEY (session_id) 
        REFERENCES "ob-poc".sessions(id) ON DELETE CASCADE
);

CREATE INDEX idx_sheet_audit_session ON "ob-poc".sheet_execution_audit(session_id);
CREATE INDEX idx_sheet_audit_submitted ON "ob-poc".sheet_execution_audit(submitted_at);
```

---

## Phase 5: Wiring (~3 hours)

### 5.1 Add sheet submission endpoint

**File:** `rust/src/api/session_routes.rs`

```rust
/// POST /api/session/:id/sheet/submit
/// Submit DSL sheet for execution
pub async fn submit_sheet(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(request): Json<SubmitSheetRequest>,
) -> Result<Json<SheetExecutionResult>, ApiError> {
    // 1. Get session
    let mut session = state.session_manager.get_session(session_id).await?;
    
    // 2. Validate state (must be READY)
    if session.session_state != SessionState::Ready {
        return Err(ApiError::BadRequest("Session not ready for execution"));
    }
    
    // 3. Get sheet and phases
    let sheet = session.sheet.as_mut().ok_or(ApiError::BadRequest("No sheet"))?;
    let phases = &sheet.phases; // Computed during PARSED → READY transition
    
    // 4. Execute
    let executor = SheetExecutor::new(&state.pool, state.emitter.clone());
    let result = executor.execute_phased(sheet, phases).await?;
    
    // 5. Update session state
    session.mark_executed(result.overall_status == SheetStatus::Success)?;
    state.session_manager.save_session(&session).await?;
    
    // 6. Audit log
    audit_sheet_execution(&state.pool, &session, &result).await?;
    
    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
pub struct SubmitSheetRequest {
    pub confirm: bool,  // Must be true to execute
}
```

### 5.2 Add sheet generation endpoint

```rust
/// POST /api/session/:id/sheet/generate
/// Generate sheet from template × entity set
pub async fn generate_sheet(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<DslSheet>, ApiError> {
    let mut session = state.session_manager.get_session(session_id).await?;
    
    // Validate state
    if !matches!(session.session_state, SessionState::Templated { confirmed: true }) {
        return Err(ApiError::BadRequest("Intent not confirmed"));
    }
    
    // Generate template × entity set
    let template = session.template_dsl.as_ref().ok_or(ApiError::BadRequest("No template"))?;
    let entity_type = session.target_entity_type.as_ref().ok_or(ApiError::BadRequest("No target type"))?;
    let entities = session.state.cbu_ids.iter().collect::<Vec<_>>();
    
    let mut statements = vec![];
    for (idx, entity_id) in entities.iter().enumerate() {
        let populated = template.replace("@cbu", &format!("\"{}\"", entity_id));
        statements.push(SessionDslStatement {
            index: idx,
            source: populated,
            dag_depth: 0, // Will be computed after DAG analysis
            produces: None,
            consumes: vec!["cbu".to_string()],
            resolved_args: HashMap::new(),
            returned_pk: None,
            status: StatementStatus::Pending,
        });
    }
    
    let sheet = DslSheet {
        id: Uuid::new_v4(),
        statements,
        validation: None,
        created_at: Utc::now(),
    };
    
    session.sheet = Some(sheet.clone());
    session.session_state = SessionState::Generated;
    state.session_manager.save_session(&session).await?;
    
    Ok(Json(sheet))
}
```

### 5.3 Wire to agent routes

Modify existing agent flow to use session state machine:

```rust
// In agent_service.rs or agent_routes.rs
// When agent generates DSL, store as template
session.set_template(generated_dsl, target_entity_type)?;

// When user confirms
session.confirm_intent()?;

// Generate sheet
let sheet = generate_sheet_from_template(&session)?;
session.set_generated(sheet)?;

// Parse and validate
let (ast, unresolved) = parse_and_validate(&sheet)?;
session.set_parsed(unresolved.len())?;

// Resolve loop (entity picker)
while !session.sheet.unresolved.is_empty() {
    // User picks entity
    session.resolve_ref(ref_index, selected_uuid)?;
}

// Ready for execution
session.set_ready()?;

// User confirms run
let result = submit_sheet(&session)?;
```

---

## Testing

### Unit tests

- `topo_sort.rs`: Test `compute_phases()` with various dependency graphs
- `sheet_executor.rs`: Test phased execution, failure handling, symbol substitution
- `cbu_session.rs`: Test state transitions, validation

### Integration tests

- Full flow: scope → template → generate → parse → resolve → execute
- Failure scenarios: phase 0 fails, phase 1 fails, symbol missing
- Rollback verification: no partial state after failure

---

## Files to Create/Modify

| File | Action | Purpose |
|------|--------|---------|
| `rust/src/session/cbu_session.rs` | Modify | Add state machine, scope fields |
| `rust/src/session/dsl_sheet.rs` | Create | Sheet and statement types |
| `rust/src/session/mod.rs` | Modify | Export new types |
| `rust/src/dsl_v2/topo_sort.rs` | Modify | Add `compute_phases()` |
| `rust/src/dsl_v2/sheet_executor.rs` | Create | Phased execution |
| `rust/src/dsl_v2/mod.rs` | Modify | Export sheet executor |
| `rust/src/api/session_routes.rs` | Modify | Add sheet endpoints |
| `migrations/035_session_sheet.sql` | Create | Schema changes |

---

## Risks

| Risk | Mitigation |
|------|------------|
| Complex state machine | Clear state transition guards |
| Symbol substitution edge cases | Thorough unit tests |
| Transaction timeout on large sheets | Batch size limits, progress tracking |
| Concurrent session modification | Version field + optimistic locking |
