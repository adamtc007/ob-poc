# 034: REPL Session & Phased Execution Architecture

> **Status:** Design Complete
> **Date:** 2025-01-18
> **Scope:** Session state machine, DAG phase extraction, sheet execution model

## Problem Statement

The agent REPL needs to handle multi-CBU bulk operations with proper dependency ordering:

1. User declares scope ("Allianz Lux book") → derives CBU set
2. User declares intent ("add custody product") → generates DSL template
3. Template × CBU set → populated DSL sheet
4. Sheet must execute in correct order (creators before consumers)
5. Failures must be captured and reported per-statement

## Key Insight: Session Loop

The session is a **recursive feedback loop**:

```
User declares scope → Session updates → Agent has context → 
Agent generates DSL → DSL updates session → Viewport reflects → 
User refines scope → Loop continues
```

The session context must be available to:
- Agent (for generating appropriate DSL)
- MCP tools (for bulk operations)
- Executor (for `@session_cbus` iteration)

## Architecture

### Session State Machine

```
┌──────────┐    scope_set     ┌──────────┐   intent_confirmed  ┌──────────┐
│  EMPTY   │ ───────────────► │  SCOPED  │ ──────────────────► │ TEMPLATED│
│          │                  │          │                     │          │
│ No CBUs  │ ◄─────────────── │ CBU set  │ ◄─────────────────  │ Intent   │
└──────────┘    scope_clear   │ derived  │    scope_change     │ captured │
                              └──────────┘                     └──────────┘
                                                                    │
                                                                    │ generate
                                                                    ▼
┌──────────┐   execute_ok    ┌──────────┐   all_resolved    ┌──────────┐
│ EXECUTED │ ◄────────────── │  READY   │ ◄──────────────── │ PARSED   │
│          │                 │          │                   │          │
│ Results  │                 │ Validated│   ◄───────────    │ AST +    │
│ stored   │                 │ Phases   │   resolve_loop    │ unresolv │
└──────────┘                 └──────────┘                   └──────────┘
```

### Session Record Structure

```rust
pub struct AgentReplSession {
    pub id: Uuid,
    pub state: SessionState,
    
    // SCOPE LAYER
    pub scope: Option<GraphScope>,           // Book, Jurisdiction, etc.
    pub scope_dsl: Vec<String>,              // DSL that derived the scope
    pub entity_sets: HashMap<EntityType, Vec<Uuid>>,  // CBU → [uuid...]
    
    // INTENT LAYER  
    pub template_dsl: Option<String>,        // Unpopulated template
    pub target_entity_type: Option<EntityType>,
    pub intent_confirmed: bool,
    
    // SHEET LAYER
    pub sheet: Option<DslSheet>,             // Populated statements with dag_depth
    
    // EXECUTION LAYER
    pub execution_result: Option<SheetExecutionResult>,
    
    // HISTORY
    history: Vec<SessionSnapshot>,
    future: Vec<SessionSnapshot>,
}

pub struct DslSheet {
    pub statements: Vec<SessionDslStatement>,
    pub validation: ValidationResult,
}

pub struct SessionDslStatement {
    pub index: usize,
    pub source: String,
    pub dag_depth: usize,              // 0 = no deps, 1 = needs depth 0, etc.
    pub produces: Option<String>,       // @symbol this creates
    pub consumes: Vec<String>,          // @symbols this needs
    pub resolved_args: HashMap<String, Uuid>,
    pub returned_pk: Option<Uuid>,
    pub status: StatementStatus,
}
```

### Two Tollgates (Same DAG Tool)

```
TOLLGATE 1: PRE-COMPILE (reorder)
════════════════════════════════
Input:  Raw DSL text (any order)
DAG:    Analyze @symbol flow, detect cycles, REORDER
Output: Sorted DSL text + dag_depth per statement

If cycles → STOP, error
If ok → proceed to compile


TOLLGATE 2: PRE-RUN (phase extraction)
══════════════════════════════════════
Input:  Compiled AST (resolved, validated)  
DAG:    Same analysis, EXTRACT PHASES for execution
Output: Vec<Phase> grouped by depth

User confirms "Run?" → Execute phases in order
```

### DAG Phase Extraction

The DAG computes depth for each statement:

```
depth = max(depth of dependencies) + 1
(or 0 if no dependencies)

Statements:
  0: (cbu.ensure :name "Fund I" :as @cbu)           → depth 0
  1: (entity.ensure :name "Barclays" :as @cp)       → depth 0
  2: (trading-profile.add-product :cbu-id @cbu ...) → depth 1
  3: (cbu.assign-role :cbu-id @cbu :entity-id @cp)  → depth 1
  4: (isda.create :cbu-id @cbu :counterparty @cp)   → depth 1
  5: (isda.add-csa :isda-id @isda ...)              → depth 2

Phases:
  Phase 0: [0, 1]    ← creators, no deps
  Phase 1: [2, 3, 4] ← need @cbu, @cp from phase 0
  Phase 2: [5]       ← needs @isda from phase 1
```

### Phased Execution

```rust
pub async fn execute_phased(
    dag: &DagAnalysis,
    pool: &PgPool,
) -> Result<SheetExecutionResult> {
    let mut symbols: HashMap<String, Uuid> = HashMap::new();
    let tx = pool.begin().await?;
    
    for phase in &dag.phases {
        for stmt in &phase.statements {
            // Substitute @symbols from previous phases
            let resolved = substitute_symbols(stmt, &symbols)?;
            
            // Execute
            let result = execute(&tx, resolved).await?;
            
            // Capture produced symbol
            if let Some(symbol) = &stmt.produces {
                symbols.insert(symbol.clone(), result.pk);
            }
        }
    }
    
    tx.commit().await?;
    // Only now persist PKs to session
}
```

### Entity Type Hierarchy (Fixed Tree)

The DAG validates attachments against the hierarchy:

```
Universe
  └── Book (apex entity)
        └── CBU
              ├── TradingProfile
              │     ├── Product       ← "add custody" attaches HERE
              │     ├── InstrumentClass
              │     ├── Market
              │     └── Gateway
              ├── Entity (roles)
              │     ├── AssetOwner
              │     ├── ManCo
              │     └── Custodian
              ├── ISDA
              │     └── Counterparty
              └── KycCase
```

Verb's target determines where it attaches. DAG ensures chain is complete.

### Error Reporting

```rust
pub struct SheetExecutionResult {
    pub session_id: Uuid,
    pub sheet_id: Uuid,
    pub overall_status: SheetStatus,
    pub phases_completed: usize,
    pub phases_total: usize,
    pub statements: Vec<StatementResult>,
}

pub struct StatementResult {
    pub index: usize,
    pub dag_depth: usize,
    pub source: String,
    pub status: StatementStatus,  // Success, Failed, Skipped
    pub error: Option<StatementError>,
    pub returned_pk: Option<Uuid>,
}

pub struct StatementError {
    pub code: ErrorCode,
    pub message: String,
    pub span: Option<Span>,        // For UI highlighting
    pub blocked_by: Option<usize>, // Index that caused block
}
```

## Failure Modes & Recovery

| Failure | Handling |
|---------|----------|
| Cycle detected | DAG rejects at Tollgate 1 |
| Unbound symbol | Resolver error, picker loop |
| Phase N fails | Rollback entire tx, report which stmt |
| Downstream blocked | Mark SKIPPED, report blocked_by |
| Session corrupted | User trashes session, starts fresh |

**Self-protecting:** If creator fails, no PK returned → downstream can't substitute → natural block.

**POC Recovery:** Idempotent verbs + tx rollback + session restart = always recoverable.

**Production (future):** Circuit breakers, data consistency checks, reconciliation tools.

## Existing Infrastructure

| Component | Location | Purpose |
|-----------|----------|---------|
| DAG/TopoSort | `rust/src/dsl_v2/topo_sort.rs` | Dependency analysis, reordering |
| Event emitter | `rust/src/events/emitter.rs` | Fire-and-forget event capture |
| DslEvent | `rust/src/events/types.rs` | Success/failure per verb |
| Executor emit | `rust/src/dsl_v2/executor.rs:890` | Already wired |
| CbuSession | `rust/src/session/cbu_session.rs` | CBU set management |

## Implementation Plan

### Phase 1: Session Record Enhancement
- Add `scope_dsl`, `template_dsl`, `sheet` fields to session
- Add `dag_depth` to statement storage
- Add `SessionState` enum with transitions

### Phase 2: DAG Phase Extraction  
- Add `compute_phases()` to `TopoSortResult`
- Group statements by depth
- Return `Vec<ExecutionPhase>`

### Phase 3: Phased Executor
- Execute phases in order
- Collect symbols between phases
- Single transaction wrapping all phases

### Phase 4: Error Reporting
- `SheetExecutionResult` struct
- Per-statement status with span info
- Blocked statement tracking

### Phase 5: Wiring
- Connect session state machine to agent routes
- Connect phased executor to sheet submission
- UI displays phase progress and errors

## Key Files to Modify

| File | Changes |
|------|---------|
| `rust/src/session/cbu_session.rs` | Add scope_dsl, sheet, state machine |
| `rust/src/dsl_v2/topo_sort.rs` | Add `compute_phases()` |
| `rust/src/dsl_v2/executor.rs` | Add phased execution path |
| `rust/src/api/agent_routes.rs` | Wire session state transitions |
| `rust/config/verbs/session.yaml` | Verify scope verbs complete |

## Out of Scope (Production)

- Circuit breaker for repeated failures
- Data consistency pre-checks
- Reconciliation tooling
- Alerting on failure patterns
- Partial execution recovery
