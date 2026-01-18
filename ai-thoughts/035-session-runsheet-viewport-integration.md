# Session = Run Sheet = Viewport Scope

> **Status:** Design Complete
> **Priority:** High - Foundational for REPL-to-Viewport UX
> **Created:** 2026-01-18

## Problem Statement

The current session architecture has three loosely coupled concepts that should be a single unified model:

1. **Session** - Holds CBU IDs, bindings, modes, and accumulated context
2. **Run Sheet** - DSL statements with execution status (implicit in `assembled_dsl`, `pending`)
3. **Viewport Scope** - What the user is looking at in the graph visualization

These need to be a **closed feedback loop**:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    CLOSED FEEDBACK LOOP                                     │
│                                                                              │
│   REPL Panel ──────────────────────► Session (Server)                       │
│        │                                   │                                │
│        │  "add Aviva Lux funds"           │  Entity Scope Updated           │
│        │                                   ▼                                │
│        │                           ┌───────────────┐                        │
│        │                           │ Target Universe│                       │
│        │                           │ Entity Scope   │                       │
│        │                           │ Run Sheet      │                       │
│        │                           │ State Stack    │                       │
│        │                           └───────┬───────┘                        │
│        │                                   │                                │
│        │                                   │ Viewport subscribes            │
│        │                                   ▼                                │
│   REPL Panel ◄──────────────────── Viewport (UI)                           │
│        │                                   │                                │
│        └───────────────────────────────────┘                                │
│                    60fps Updates                                            │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Key Insight: LLM Does Intent Classification, Not DSL Generation

The agent does NOT generate DSL syntax. Instead:

1. **LLM** does **intent classification** - picks from verb/template vocabulary
2. **Templates** are macros that expand to multi-statement DSL
3. **DSL pipeline** handles parsing, validation, execution

This means templates need the same `invocation_phrases` / `invocation_hints` metadata as verbs.

## Target State Architecture

### 1. Target Universe (Anchor Point)

Every session starts with the user declaring a target universe. This is the **anchor** for all subsequent operations.

```rust
/// The declared operating scope for this session
/// Set at session start, refined over time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetUniverse {
    /// Description of the universe (e.g., "Aviva Lux funds", "BlackRock EMEA portfolio")
    pub description: String,
    
    /// How the universe was defined
    pub definition: UniverseDefinition,
    
    /// When this universe was declared
    pub declared_at: DateTime<Utc>,
    
    /// User who declared it (for audit)
    pub declared_by: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UniverseDefinition {
    /// All CBUs under an apex entity (e.g., "Aviva Group")
    Galaxy { apex_entity_id: Uuid, apex_name: String },
    
    /// CBUs matching a filter (e.g., "Luxembourg funds")
    Filtered { filter: GraphFilters, description: String },
    
    /// Explicit list of CBUs
    Explicit { cbu_ids: Vec<Uuid>, names: Vec<String> },
    
    /// Single CBU focus
    SingleCbu { cbu_id: Uuid, cbu_name: String },
    
    /// Entire book (all CBUs the user has access to)
    FullBook,
}
```

### 2. Entity Scope (Active Working Set)

The entity scope is the **current** set of entities being operated on. It's a refinement of the target universe.

```rust
/// Current entity scope - what the viewport shows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityScope {
    /// CBU IDs currently in scope
    pub cbu_ids: HashSet<Uuid>,
    
    /// Entity IDs currently in scope (entities within CBUs)
    pub entity_ids: HashSet<Uuid>,
    
    /// How entities enter/exit scope
    pub expansion_mode: ExpansionMode,
    
    /// Filters applied to narrow scope
    pub filters: GraphFilters,
    
    /// Focal point (if zoomed to specific entity)
    pub focal_entity_id: Option<Uuid>,
    
    /// Zoom level (affects which entities are visible)
    pub zoom_level: ZoomLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExpansionMode {
    /// Only explicitly added entities
    Explicit,
    /// N hops from focal entity
    Neighborhood { hops: u8 },
    /// Full graph of CBU
    FullCbu,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ZoomLevel {
    Universe,   // All CBUs as dots
    Galaxy,     // Cluster/segment view
    System,     // Single CBU solar system
    Planet,     // Single entity focus
    Surface,    // High zoom detail
    Core,       // Deepest zoom - attributes visible
}
```

### 3. Run Sheet (DSL Statement Ledger)

The run sheet tracks all DSL statements with their status and affected entities.

```rust
/// A single DSL statement in the run sheet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSheetEntry {
    /// Unique ID for this entry
    pub id: Uuid,
    
    /// Index in run sheet (0-based, order matters)
    pub index: usize,
    
    /// The DSL statement
    pub statement: Statement,
    
    /// Current status
    pub status: RunSheetStatus,
    
    /// When this entry was created
    pub created_at: DateTime<Utc>,
    
    /// When status last changed
    pub status_changed_at: DateTime<Utc>,
    
    /// Entity IDs affected by this statement (populated after execution)
    pub affected_entities: Vec<Uuid>,
    
    /// Bindings created by this statement (e.g., @cbu, @manco)
    pub created_bindings: Vec<String>,
    
    /// Error message if status is Failed
    pub error: Option<String>,
    
    /// User-facing display string (no UUIDs)
    pub display_dsl: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RunSheetStatus {
    /// Parsed, awaiting user confirmation
    Draft,
    /// User confirmed, ready to execute
    Ready,
    /// Currently executing
    Executing,
    /// Successfully executed
    Executed,
    /// User declined
    Cancelled,
    /// Execution failed
    Failed,
}

/// The full run sheet - ledger of all DSL for this session
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RunSheet {
    /// All entries in order
    pub entries: Vec<RunSheetEntry>,
    
    /// Current position (what user is looking at)
    pub cursor_position: usize,
    
    /// Execution high-water mark (last executed index)
    pub executed_through: Option<usize>,
}

impl RunSheet {
    /// Add a new draft entry
    pub fn add_draft(&mut self, statement: Statement, display_dsl: String) -> Uuid {
        let id = Uuid::new_v4();
        let index = self.entries.len();
        self.entries.push(RunSheetEntry {
            id,
            index,
            statement,
            status: RunSheetStatus::Draft,
            created_at: Utc::now(),
            status_changed_at: Utc::now(),
            affected_entities: vec![],
            created_bindings: vec![],
            error: None,
            display_dsl,
        });
        id
    }
    
    /// Get pending (non-terminal) entries
    pub fn pending_entries(&self) -> Vec<&RunSheetEntry> {
        self.entries.iter()
            .filter(|e| matches!(e.status, RunSheetStatus::Draft | RunSheetStatus::Ready))
            .collect()
    }
    
    /// Get all affected entity IDs (from executed entries)
    pub fn all_affected_entities(&self) -> HashSet<Uuid> {
        self.entries.iter()
            .filter(|e| e.status == RunSheetStatus::Executed)
            .flat_map(|e| e.affected_entities.iter().copied())
            .collect()
    }
}
```

### 4. Session State Stack (Navigation History)

The state stack enables back/forward/go-to-start/go-to-last navigation with smooth viewport transitions.

```rust
/// A snapshot of session state at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSnapshot {
    /// Unique ID for this snapshot
    pub id: Uuid,
    
    /// When this snapshot was taken
    pub timestamp: DateTime<Utc>,
    
    /// Target universe at this point
    pub target_universe: TargetUniverse,
    
    /// Entity scope at this point
    pub entity_scope: EntityScope,
    
    /// Run sheet cursor position
    pub run_sheet_position: usize,
    
    /// View state for viewport animation
    pub view_state: ViewState,
    
    /// What action created this snapshot (for display)
    pub action_label: String,
}

/// Session state stack for navigation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStateStack {
    /// All snapshots
    states: Vec<SessionSnapshot>,
    
    /// Current position in stack (0 = oldest)
    position: usize,
    
    /// Max snapshots to keep (rolling window)
    max_size: usize,
}

impl SessionStateStack {
    pub fn new(max_size: usize) -> Self {
        Self {
            states: Vec::new(),
            position: 0,
            max_size,
        }
    }
    
    /// Push a new snapshot (truncates forward history if in middle)
    pub fn push(&mut self, snapshot: SessionSnapshot) {
        // Truncate forward history
        if self.position < self.states.len().saturating_sub(1) {
            self.states.truncate(self.position + 1);
        }
        
        // Add new snapshot
        self.states.push(snapshot);
        
        // Enforce max size
        while self.states.len() > self.max_size {
            self.states.remove(0);
        }
        
        self.position = self.states.len().saturating_sub(1);
    }
    
    /// Go back N steps, returns the target snapshot
    pub fn back(&mut self, n: usize) -> Option<&SessionSnapshot> {
        if n == 0 || self.position == 0 {
            return None;
        }
        self.position = self.position.saturating_sub(n);
        self.states.get(self.position)
    }
    
    /// Go forward N steps
    pub fn forward(&mut self, n: usize) -> Option<&SessionSnapshot> {
        if n == 0 {
            return None;
        }
        let target = self.position + n;
        if target >= self.states.len() {
            return None;
        }
        self.position = target;
        self.states.get(self.position)
    }
    
    /// Go to start (first snapshot)
    pub fn go_to_start(&mut self) -> Option<&SessionSnapshot> {
        if self.states.is_empty() {
            return None;
        }
        self.position = 0;
        self.states.first()
    }
    
    /// Go to last (most recent snapshot)
    pub fn go_to_last(&mut self) -> Option<&SessionSnapshot> {
        if self.states.is_empty() {
            return None;
        }
        self.position = self.states.len() - 1;
        self.states.last()
    }
    
    /// Current snapshot
    pub fn current(&self) -> Option<&SessionSnapshot> {
        self.states.get(self.position)
    }
    
    /// Check if can go back
    pub fn can_go_back(&self) -> bool {
        self.position > 0
    }
    
    /// Check if can go forward
    pub fn can_go_forward(&self) -> bool {
        self.position + 1 < self.states.len()
    }
}
```

### 5. View State (for Viewport Animation)

The view state captures what the viewport needs to render smoothly.

```rust
/// View state for viewport animation (60fps updates)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewState {
    /// Camera position
    pub camera: CameraState,
    
    /// Which nodes are expanded
    pub expanded_nodes: HashSet<Uuid>,
    
    /// Which nodes are selected
    pub selected_nodes: HashSet<Uuid>,
    
    /// Highlight state (for trace, search results)
    pub highlights: Vec<HighlightState>,
    
    /// Filter state (what's visible vs hidden)
    pub visibility: VisibilityState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraState {
    pub center_x: f32,
    pub center_y: f32,
    pub zoom: f32,
    pub rotation: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighlightState {
    pub node_ids: Vec<Uuid>,
    pub edge_ids: Vec<(Uuid, Uuid)>,
    pub color: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisibilityState {
    pub hidden_nodes: HashSet<Uuid>,
    pub hidden_edge_types: HashSet<String>,
    pub layer_visibility: HashMap<String, bool>,
}
```

## Integration: Unified Session

All these pieces come together in the session:

```rust
/// Enhanced AgentSession with unified state model
pub struct AgentSession {
    // ... existing fields ...
    
    // === NEW: Unified State Model ===
    
    /// Target universe - declared at session start
    pub target_universe: Option<TargetUniverse>,
    
    /// Current entity scope (viewport subscription target)
    pub entity_scope: EntityScope,
    
    /// Run sheet - DSL statement ledger
    pub run_sheet: RunSheet,
    
    /// State stack for navigation
    pub state_stack: SessionStateStack,
    
    /// Current view state (for viewport)
    pub view_state: ViewState,
}
```

## Flow: Session → Viewport Subscription

```
1. User declares target universe
   "Work on Aviva Lux funds"
   
   → Session.target_universe = Galaxy { apex: "Aviva" }
   → Session.entity_scope.cbu_ids = [cbu1, cbu2, cbu3]
   → Push snapshot to state_stack
   
2. Viewport subscribes to session
   
   viewport.entity_scope = session.entity_scope.clone()
   viewport.render(entity_scope)
   
3. User refines scope via voice
   "Focus on Fund 9"
   
   → Agent generates: (session.set-cbu :cbu-id @fund_9)
   → Executor runs, updates Session.entity_scope
   → Push snapshot to state_stack
   → Viewport receives update, animates transition
   
4. User says "back"
   
   → state_stack.back(1)
   → Restore previous snapshot
   → Viewport animates to previous state
```

## Gap Analysis

### What Exists Today

| Component | Status | Location |
|-----------|--------|----------|
| Session struct | ✅ Exists | `session.rs` |
| CBU IDs tracking | ✅ Exists | `SessionContext.cbu_ids` |
| Bindings | ✅ Exists | `SessionContext.bindings` |
| Pending DSL | ✅ Exists | `AgentSession.pending` |
| Assembled DSL | ✅ Exists | `AgentSession.assembled_dsl` |
| View state | ⚠️ Partial | `SessionContext.view_state`, `scope` |
| Session scope | ⚠️ Partial | `SessionScope` type exists |
| Taxonomy stack | ✅ Exists | `SessionContext.taxonomy_stack` |
| Zoom level | ⚠️ Partial | `zoom_level` field, not unified |
| Viewport state | ⚠️ Partial | `ViewportState` type exists |

### What's Missing

| Component | Gap | Priority |
|-----------|-----|----------|
| **TargetUniverse** | Doesn't exist - need to add | High |
| **EntityScope** | Partial - scattered fields, not unified | High |
| **RunSheet** | Doesn't exist - DSL status tracking is ad-hoc | High |
| **SessionStateStack** | Doesn't exist - no undo/navigation | High |
| **ViewState** for animation | Partial - `ViewportState` exists but not complete | Medium |
| Template `invocation_phrases` | Templates don't have LLM metadata | High |
| Template-as-verb loading | Templates not in LLM tool vocabulary | High |
| Viewport subscription | UI doesn't subscribe to session scope changes | High |

### Files That Need Changes

| File | Changes Needed |
|------|----------------|
| `rust/src/api/session.rs` | Add TargetUniverse, EntityScope, RunSheet, SessionStateStack |
| `rust/crates/dsl-core/src/config/types.rs` | Add `invocation_phrases` to template config |
| `rust/src/dsl_v2/verb_registry.rs` | Load templates into vocabulary alongside verbs |
| `rust/src/api/agent_service.rs` | Use template invocation for intent matching |
| `rust/crates/ob-poc-ui/src/state.rs` | Add session scope subscription |
| `rust/crates/ob-poc-ui/src/app.rs` | Wire viewport to session.entity_scope |
| `rust/src/api/session_routes.rs` | Add navigation endpoints (back, forward, etc.) |

## Implementation Plan

### Phase 1: Core Types (Server)

1. Add `TargetUniverse` struct to `session.rs`
2. Add unified `EntityScope` struct
3. Add `RunSheet` with `RunSheetEntry`
4. Add `SessionStateStack` with navigation methods
5. Add `ViewState` for animation sync

### Phase 2: Template-as-Verb

1. Add `invocation_phrases` to template YAML schema
2. Update verb registry to load templates alongside verbs
3. Update agent intent matching to include templates
4. Templates expand to DSL source → normal pipeline

### Phase 3: Run Sheet Integration

1. Replace `assembled_dsl` with `run_sheet`
2. Track statement status in run sheet entries
3. Populate affected_entities after execution
4. Add run sheet position to snapshots

### Phase 4: Navigation & State Stack

1. Push snapshot after each scope-changing operation
2. Add navigation verbs: `session.back`, `session.forward`, `session.go-to-start`
3. Add navigation API endpoints
4. Wire REPL keyboard shortcuts

### Phase 5: Viewport Subscription (UI)

1. Add `session_scope` subscription channel in UI state
2. Viewport checks scope changes each frame
3. Animate transitions between scope states
4. Sync zoom/expanded_nodes bidirectionally

## Success Criteria

1. **Target Universe Anchor**: First chat message establishes target universe
2. **Closed Loop**: REPL → Session → Viewport → REPL (visual feedback)
3. **Navigation**: "back", "back 3", "forward", "go to start" all work
4. **60fps Transitions**: Scope changes animate smoothly
5. **Templates in Vocabulary**: LLM can select templates via intent classification
6. **Run Sheet Visibility**: User can see DSL statements with status in REPL panel

## Out of Scope (This Document)

- Entity resolution modal UX (covered in 025, 026)
- ESPER voice commands (covered in plan file)
- Research agent loop (covered in 020)
- Investor register visualization (covered in 018)
