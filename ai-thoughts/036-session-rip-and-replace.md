# Session Management Rip-and-Replace

> **Status:** In Progress
> **Priority:** High - Foundational cleanup
> **Created:** 2026-01-18
> **Approach:** Clean-slate replacement, not incremental patching

## Why Rip-and-Replace

The current session management has accumulated complexity:

| Problem | Current State | Impact |
|---------|---------------|--------|
| **Scattered state** | `SessionContext`, `SessionState`, `SubSessionType`, `BatchContext`, `TemplateExecutionContext` all partially overlap | Hard to reason about what's authoritative |
| **Flag soup** | `needs_resolution_check`, `pending_resolution`, `loading_resolution`, `searching_resolution` | Race conditions, unclear state machine |
| **Window stack complexity** | `WindowStack`, `WindowType`, `WindowData::Resolution`, `WindowData::Disambiguation` | Modal state spread across structures |
| **Resolution indirection** | Chat → flag → API call → response → another flag → modal | 5+ hops to show a modal |
| **No navigation** | No back/forward/undo capability | Users can't explore safely |

**LLM observation:** Incremental fixes to tangled code often introduce new bugs. Clean implementations with clear contracts work better.

## Target Architecture

### Single Source of Truth: `UnifiedSession`

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         UnifiedSession                                       │
│                                                                              │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐              │
│  │ TargetUniverse  │  │   EntityScope   │  │    RunSheet     │              │
│  │                 │  │                 │  │                 │              │
│  │ What user       │  │ Current working │  │ DSL statements  │              │
│  │ declared at     │  │ set (CBU IDs,   │  │ with status     │              │
│  │ session start   │  │ filters, zoom)  │  │ (draft/exec/err)│              │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘              │
│                                                                              │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐              │
│  │   StateStack    │  │   ViewState     │  │   Resolution    │              │
│  │                 │  │                 │  │   (Optional)    │              │
│  │ History for     │  │ Camera, zoom,   │  │                 │              │
│  │ back/forward    │  │ expanded nodes  │  │ Inline, not     │              │
│  │ navigation      │  │ for viewport    │  │ sub-session     │              │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘              │
│                                                                              │
│  Messages: Vec<ChatMessage>  (conversation history)                         │
│  Bindings: HashMap<String, BoundEntity>  (symbol table)                     │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Direct Flow: Chat → Resolution Modal

```
BEFORE (5+ hops):
  ChatResponse 
    → pending_chat 
    → process_async_results 
    → needs_resolution_check = true
    → update() checks flag
    → start_resolution API call
    → pending_resolution
    → process_async_results
    → window_stack.push(Resolution)
    → modal renders

AFTER (2 hops):
  ChatResponse { unresolved_refs: Some(refs) }
    → state.modal = Some(Modal::Resolution { refs })
    → modal renders
```

## Implementation Plan

### Phase 1: New Types (Server-Side)

**File:** `rust/src/session/unified.rs` (NEW)

```rust
//! Unified Session State
//!
//! Single source of truth for session state.
//! Replaces: SessionContext, SessionState, SubSessionType, BatchContext

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// The unified session - single source of truth
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedSession {
    // === Identity ===
    pub id: Uuid,
    pub user_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    
    // === Target Universe (anchor) ===
    /// Declared at session start, refined over time
    pub target_universe: Option<TargetUniverse>,
    
    // === Entity Scope (current working set) ===
    pub entity_scope: EntityScope,
    
    // === Run Sheet (DSL ledger) ===
    pub run_sheet: RunSheet,
    
    // === State Stack (navigation history) ===
    pub state_stack: StateStack,
    
    // === View State (for viewport sync) ===
    pub view_state: ViewState,
    
    // === Resolution (inline, optional) ===
    /// When Some, UI should show resolution modal
    pub resolution: Option<ResolutionState>,
    
    // === Conversation ===
    pub messages: Vec<ChatMessage>,
    
    // === Symbol Table ===
    pub bindings: HashMap<String, BoundEntity>,
}

/// Target universe - the anchor for this session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetUniverse {
    pub description: String,
    pub definition: UniverseDefinition,
    pub declared_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UniverseDefinition {
    /// All CBUs under an apex entity
    Galaxy { apex_entity_id: Uuid, apex_name: String },
    /// CBUs matching filters
    Filtered { filter_description: String, cbu_ids: Vec<Uuid> },
    /// Explicit list
    Explicit { cbu_ids: Vec<Uuid>, names: Vec<String> },
    /// Single CBU focus
    SingleCbu { cbu_id: Uuid, cbu_name: String },
    /// Full book (all accessible CBUs)
    FullBook,
}

/// Current entity scope - what the viewport shows
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EntityScope {
    /// CBU IDs in scope
    pub cbu_ids: HashSet<Uuid>,
    /// Entity IDs in scope (within CBUs)
    pub entity_ids: HashSet<Uuid>,
    /// Focal entity (if zoomed)
    pub focal_entity_id: Option<Uuid>,
    /// Current zoom level
    pub zoom_level: ZoomLevel,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ZoomLevel {
    #[default]
    Universe,
    Galaxy,
    System,
    Planet,
    Surface,
    Core,
}

/// Run sheet - DSL statement ledger
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RunSheet {
    pub entries: Vec<RunSheetEntry>,
    pub cursor: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSheetEntry {
    pub id: Uuid,
    pub dsl_source: String,
    pub display_dsl: String,
    pub status: EntryStatus,
    pub created_at: DateTime<Utc>,
    pub affected_entities: Vec<Uuid>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum EntryStatus {
    #[default]
    Draft,
    Ready,
    Executing,
    Executed,
    Cancelled,
    Failed,
}

/// State stack for navigation
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StateStack {
    snapshots: Vec<StateSnapshot>,
    position: usize,
    max_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub entity_scope: EntityScope,
    pub run_sheet_cursor: usize,
    pub action_label: String,
}

impl StateStack {
    pub fn new(max_size: usize) -> Self {
        Self {
            snapshots: Vec::new(),
            position: 0,
            max_size: if max_size == 0 { 100 } else { max_size },
        }
    }
    
    pub fn push(&mut self, scope: EntityScope, cursor: usize, label: String) {
        // Truncate forward history
        if self.position + 1 < self.snapshots.len() {
            self.snapshots.truncate(self.position + 1);
        }
        
        self.snapshots.push(StateSnapshot {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            entity_scope: scope,
            run_sheet_cursor: cursor,
            action_label: label,
        });
        
        // Enforce max size
        while self.snapshots.len() > self.max_size {
            self.snapshots.remove(0);
        }
        
        self.position = self.snapshots.len().saturating_sub(1);
    }
    
    pub fn back(&mut self, n: usize) -> Option<&StateSnapshot> {
        if n == 0 || self.position == 0 {
            return None;
        }
        self.position = self.position.saturating_sub(n);
        self.snapshots.get(self.position)
    }
    
    pub fn forward(&mut self, n: usize) -> Option<&StateSnapshot> {
        let target = self.position + n;
        if target >= self.snapshots.len() {
            return None;
        }
        self.position = target;
        self.snapshots.get(self.position)
    }
    
    pub fn go_to_start(&mut self) -> Option<&StateSnapshot> {
        if self.snapshots.is_empty() {
            return None;
        }
        self.position = 0;
        self.snapshots.first()
    }
    
    pub fn go_to_last(&mut self) -> Option<&StateSnapshot> {
        if self.snapshots.is_empty() {
            return None;
        }
        self.position = self.snapshots.len() - 1;
        self.snapshots.last()
    }
    
    pub fn current(&self) -> Option<&StateSnapshot> {
        self.snapshots.get(self.position)
    }
    
    pub fn can_go_back(&self) -> bool {
        self.position > 0
    }
    
    pub fn can_go_forward(&self) -> bool {
        self.position + 1 < self.snapshots.len()
    }
}

/// View state for viewport sync
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ViewState {
    pub camera_x: f32,
    pub camera_y: f32,
    pub zoom: f32,
    pub expanded_nodes: HashSet<Uuid>,
    pub selected_nodes: HashSet<Uuid>,
}

/// Inline resolution state (not a sub-session)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionState {
    /// Refs needing resolution
    pub refs: Vec<UnresolvedRef>,
    /// Current index being resolved
    pub current_index: usize,
    /// Resolutions made so far
    pub resolutions: HashMap<String, ResolvedRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnresolvedRef {
    pub ref_id: String,
    pub entity_type: String,
    pub search_value: String,
    pub context_line: String,
    pub search_keys: Vec<SearchKeyField>,
    pub discriminators: Vec<DiscriminatorField>,
    pub initial_matches: Vec<EntityMatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchKeyField {
    pub name: String,
    pub label: String,
    pub value: Option<String>,
    pub is_primary: bool,
    pub field_type: FieldType,
    pub enum_values: Option<Vec<EnumValue>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscriminatorField {
    pub name: String,
    pub label: String,
    pub value: Option<String>,
    pub field_type: FieldType,
    pub enum_values: Option<Vec<EnumValue>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    #[default]
    Text,
    Enum,
    Date,
    Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumValue {
    pub code: String,
    pub display: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityMatch {
    pub id: String,
    pub display: String,
    pub score: f32,
    pub details: Option<String>,
    pub entity_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedRef {
    pub ref_id: String,
    pub resolved_key: String,
    pub display: String,
}

/// Chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: Uuid,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub dsl: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Agent,
    System,
}

/// Bound entity in symbol table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundEntity {
    pub id: Uuid,
    pub entity_type: String,
    pub display_name: String,
}

impl Default for UnifiedSession {
    fn default() -> Self {
        Self::new()
    }
}

impl UnifiedSession {
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            user_id: Uuid::nil(),
            created_at: now,
            updated_at: now,
            target_universe: None,
            entity_scope: EntityScope::default(),
            run_sheet: RunSheet::default(),
            state_stack: StateStack::new(100),
            view_state: ViewState::default(),
            resolution: None,
            messages: Vec::new(),
            bindings: HashMap::new(),
        }
    }
    
    /// Set target universe (first user declaration)
    pub fn set_target_universe(&mut self, definition: UniverseDefinition, description: String) {
        self.target_universe = Some(TargetUniverse {
            description,
            definition,
            declared_at: Utc::now(),
        });
        self.updated_at = Utc::now();
    }
    
    /// Add CBU to scope
    pub fn add_cbu(&mut self, cbu_id: Uuid) {
        self.entity_scope.cbu_ids.insert(cbu_id);
        self.updated_at = Utc::now();
    }
    
    /// Add DSL entry to run sheet
    pub fn add_dsl(&mut self, dsl_source: String, display_dsl: String) -> Uuid {
        let id = Uuid::new_v4();
        self.run_sheet.entries.push(RunSheetEntry {
            id,
            dsl_source,
            display_dsl,
            status: EntryStatus::Draft,
            created_at: Utc::now(),
            affected_entities: Vec::new(),
            error: None,
        });
        self.run_sheet.cursor = self.run_sheet.entries.len() - 1;
        self.updated_at = Utc::now();
        id
    }
    
    /// Push current state to history
    pub fn push_state(&mut self, action_label: &str) {
        self.state_stack.push(
            self.entity_scope.clone(),
            self.run_sheet.cursor,
            action_label.to_string(),
        );
    }
    
    /// Navigate back
    pub fn back(&mut self, n: usize) -> bool {
        if let Some(snapshot) = self.state_stack.back(n) {
            self.entity_scope = snapshot.entity_scope.clone();
            self.run_sheet.cursor = snapshot.run_sheet_cursor;
            self.updated_at = Utc::now();
            true
        } else {
            false
        }
    }
    
    /// Navigate forward
    pub fn forward(&mut self, n: usize) -> bool {
        if let Some(snapshot) = self.state_stack.forward(n) {
            self.entity_scope = snapshot.entity_scope.clone();
            self.run_sheet.cursor = snapshot.run_sheet_cursor;
            self.updated_at = Utc::now();
            true
        } else {
            false
        }
    }
    
    /// Start resolution workflow
    pub fn start_resolution(&mut self, refs: Vec<UnresolvedRef>) {
        self.resolution = Some(ResolutionState {
            refs,
            current_index: 0,
            resolutions: HashMap::new(),
        });
        self.updated_at = Utc::now();
    }
    
    /// Complete resolution
    pub fn complete_resolution(&mut self) -> Option<HashMap<String, ResolvedRef>> {
        self.resolution.take().map(|r| r.resolutions)
    }
    
    /// Add user message
    pub fn add_user_message(&mut self, content: String) -> Uuid {
        let id = Uuid::new_v4();
        self.messages.push(ChatMessage {
            id,
            role: MessageRole::User,
            content,
            timestamp: Utc::now(),
            dsl: None,
        });
        self.updated_at = Utc::now();
        id
    }
    
    /// Add agent message
    pub fn add_agent_message(&mut self, content: String, dsl: Option<String>) -> Uuid {
        let id = Uuid::new_v4();
        self.messages.push(ChatMessage {
            id,
            role: MessageRole::Agent,
            content,
            timestamp: Utc::now(),
            dsl,
        });
        self.updated_at = Utc::now();
        id
    }
    
    /// Set binding
    pub fn set_binding(&mut self, name: &str, entity: BoundEntity) {
        self.bindings.insert(name.to_string(), entity);
        self.updated_at = Utc::now();
    }
}
```

### Phase 2: API Response Types

**File:** `rust/crates/ob-poc-types/src/session.rs` (NEW or replace)

The API types mirror the server types but are `#[derive(Serialize, Deserialize)]` for HTTP boundary.

```rust
// Key addition: ChatResponse includes resolution directly

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub message: String,
    pub dsl: Option<String>,
    pub commands: Option<Vec<AgentCommand>>,
    
    /// When present, UI should show resolution modal immediately
    /// No separate API call needed
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<ResolutionPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionPayload {
    pub refs: Vec<UnresolvedRefResponse>,
    pub total: usize,
}
```

### Phase 3: UI State Simplification

**File:** `rust/crates/ob-poc-ui/src/state.rs` (REPLACE core structs)

```rust
/// Simplified app state
pub struct AppState {
    // === Server data (fetched, never mutated locally) ===
    pub session_id: Option<Uuid>,
    pub session: Option<SessionResponse>,
    pub graph: Option<CbuGraphData>,
    
    // === UI-only state ===
    pub buffers: TextBuffers,
    pub modal: Option<Modal>,
    pub selected_entity: Option<String>,
    
    // === Widgets ===
    pub graph_widget: CbuGraphWidget,
    
    // === Async ===
    pub async_rx: AsyncReceiver,
}

/// Single modal enum (not a stack)
#[derive(Clone)]
pub enum Modal {
    Resolution(ResolutionModal),
    CbuSearch(CbuSearchModal),
    Confirmation(ConfirmationModal),
}

/// Resolution modal state
#[derive(Clone)]
pub struct ResolutionModal {
    pub refs: Vec<UnresolvedRefResponse>,
    pub current_index: usize,
    pub search_query: String,
    pub search_results: Option<Vec<EntityMatchResponse>>,
    pub resolutions: HashMap<String, String>,
}

/// Async channel (replaces Arc<Mutex<AsyncState>>)
pub struct AsyncReceiver {
    rx: std::sync::mpsc::Receiver<AsyncResult>,
}

pub enum AsyncResult {
    Session(Result<SessionResponse, String>),
    Graph(Result<CbuGraphData, String>),
    Chat(Result<ChatResponse, String>),
    ResolutionSearch(Result<Vec<EntityMatchResponse>, String>),
    Execution(Result<ExecuteResponse, String>),
}
```

### Phase 4: Direct Chat → Modal Flow

**File:** `rust/crates/ob-poc-ui/src/app.rs`

```rust
// In chat response handler - SIMPLE

fn handle_chat_response(&mut self, response: ChatResponse) {
    // Add message to UI
    self.state.add_chat_message(MessageRole::Agent, &response.message);
    
    // If DSL, add to buffer
    if let Some(ref dsl) = response.dsl {
        self.state.buffers.dsl_editor = dsl.clone();
    }
    
    // If resolution needed, open modal DIRECTLY
    if let Some(resolution) = response.resolution {
        self.state.modal = Some(Modal::Resolution(ResolutionModal {
            refs: resolution.refs,
            current_index: 0,
            search_query: String::new(),
            search_results: None,
            resolutions: HashMap::new(),
        }));
    }
    
    // Handle commands
    if let Some(commands) = response.commands {
        for cmd in commands {
            self.handle_command(cmd);
        }
    }
}
```

## Files to Delete

After migration, remove:

| File | Reason |
|------|--------|
| Most of `rust/src/api/session.rs` | Replaced by `unified.rs` |
| `SubSessionType`, `ResolutionSubSession` | Resolution is inline |
| `BatchContext`, `TemplateExecutionContext` | Simplified into RunSheet |
| `WindowStack`, `WindowType`, `WindowData` | Single Modal enum |
| `AsyncState` flag soup | Channel-based async |

## Files to Keep (Modified)

| File | Changes |
|------|---------|
| `rust/src/api/agent_service.rs` | Use `UnifiedSession`, return `resolution` in `ChatResponse` |
| `rust/crates/ob-poc-types/src/lib.rs` | Update `ChatResponse`, add `ResolutionPayload` |
| `rust/crates/ob-poc-ui/src/panels/resolution.rs` | Simplify to read from `Modal::Resolution` |

## Migration Order

1. **Create `unified.rs`** with new types (doesn't break anything)
2. **Add `ResolutionPayload` to `ChatResponse`** in types crate
3. **Update `AgentService`** to populate `resolution` field
4. **Update UI state** to use `Modal` enum
5. **Update UI app** with direct chat → modal flow
6. **Delete legacy code** after verifying everything works

## Success Criteria

1. **Chat → Resolution in 2 hops** (response → modal)
2. **Back/forward navigation works** via StateStack
3. **No flag soup** - no `needs_*`, `pending_*` for resolution
4. **Single modal state** - not a window stack
5. **Server types match API types** - no conversion layer needed
