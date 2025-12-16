# TODO: Entity Resolution UI Implementation

## ⛔ MANDATORY FIRST STEP

**Before writing ANY code, read `/EGUI-RULES.md` completely.**

Then confirm you understand by checking these boxes mentally:
- [ ] NO local state mirroring server data
- [ ] Actions return values, NO callbacks
- [ ] Short lock, then render (never hold lock during UI)
- [ ] Process async FIRST in update(), render SECOND
- [ ] All mutations go through server round-trip

**If you find yourself writing `self.xxx.push()` or `self.xxx = ...` for server data inside a panel, STOP. You are violating the rules.**

---

## Reference Documents

## Reference Documents

| Document | Purpose | Priority |
|----------|---------|----------|
| **`EGUI-RULES.md`** | 5 non-negotiable rules - SHORT, READ FIRST | **⛔ MANDATORY** |
| `CLAUDE.md` - egui section | Full patterns, examples, rationale | Reference |
| `EGUI.md` | Full egui refactoring brief | Reference |
| `docs/ENTITY_RESOLUTION_UI.md` | Detailed design spec | Reference |
| `docs/ENTITY_RESOLUTION_UI_PROPOSAL.md` | Design decisions | Reference |

---

## ⚠️ CRITICAL: egui Architecture Compliance

**Before writing ANY UI code, re-read the egui section in CLAUDE.md.**

### Mandatory Patterns

```rust
// ✅ CORRECT: Server-first, fetch-render pattern
fn update(&mut self, ctx: &egui::Context) {
    // 1. Process async results FIRST
    self.process_async_results();
    
    // 2. Extract what we need from state (short lock)
    let resolution_data = {
        let state = self.async_state.lock().unwrap();
        state.resolution_session.clone()  // Clone and release lock
    };
    
    // 3. Render using extracted data (NO locks held)
    if let Some(session) = resolution_data {
        self.render_resolution_panel(ui, &session);
    }
}
```

### Forbidden Patterns

```rust
// ❌ WRONG: Local state mirroring server
struct ResolutionPanel {
    local_resolutions: HashMap<String, ResolvedRef>,  // NO! State drift
    is_dirty: bool,                                    // NO! Fighting egui
    cached_matches: Vec<EntityMatch>,                  // NO! Server caches
}

// ❌ WRONG: Lock held during UI rendering
fn render(&mut self, ui: &mut egui::Ui) {
    let state = self.async_state.lock().unwrap();  // Lock acquired
    for ref in &state.resolution_session.unresolved {
        ui.label(&ref.display);  // Lock STILL held - BAD
    }
}  // Lock released too late

// ❌ WRONG: Callbacks that capture &mut self
ui.button("Resolve").clicked().then(|| {
    self.resolve_entity(ref_id);  // Borrow checker nightmare
});
```

### The Pattern for Resolution Panel

```rust
// Resolution panel state (UI-only, ephemeral)
pub struct ResolutionPanelState {
    // Text buffers ONLY - the one exception for local mutable state
    pub search_buffers: HashMap<String, String>,
    
    // UI navigation (not server state)
    pub focused_ref_idx: Option<usize>,
    pub expanded_ref: Option<String>,
    
    // NO resolution data here - comes from server via async_state
}

// In App state
pub struct AppState {
    // Server data (fetched, never modified locally)
    pub session: Option<SessionStateResponse>,
    pub resolution: Option<ResolutionSessionResponse>,  // From /resolution/start
    
    // UI-only
    pub resolution_panel: ResolutionPanelState,
    
    // Async coordination
    pub async_state: Arc<Mutex<AsyncState>>,
}

// Actions return values, don't mutate directly
fn resolution_panel(ui: &mut egui::Ui, state: &ResolutionPanelState, session: &ResolutionSessionResponse) -> Option<ResolutionAction> {
    // Pure render function - returns what happened
    for (idx, unresolved) in session.unresolved.iter().enumerate() {
        if ui.button("Confirm").clicked() {
            return Some(ResolutionAction::Confirm { ref_id: unresolved.ref_id.clone() });
        }
        if ui.button("Change").clicked() {
            return Some(ResolutionAction::ReResolve { ref_id: unresolved.ref_id.clone() });
        }
    }
    None
}

// In update(), handle returned actions
if let Some(action) = resolution_panel(ui, &self.resolution_panel, &resolution) {
    match action {
        ResolutionAction::Confirm { ref_id } => {
            // POST to server, then refetch
            self.post_resolution_confirm(&ref_id);
        }
        ResolutionAction::ReResolve { ref_id } => {
            self.resolution_panel.expanded_ref = Some(ref_id);
        }
    }
}
```

---

## Feature Overview

### User Flow

```
1. Agent generates DSL with entity references
              │
2. POST /resolution/start → server extracts unresolved refs
              │
3. UI shows RESOLUTION PANEL (batch resolve)
   ├─ Agent suggestions pre-selected (one-click confirm)
   ├─ Grouped by confidence (green/yellow/red)
   └─ Keyboard navigation (Tab, Enter, 1-5)
              │
4. User resolves refs (or confirms agent suggestions)
              │
5. Panel transforms to REVIEW MODE
   ├─ Shows all resolutions with discriminators
   ├─ Flags warnings (multiple matches, inactive, etc.)
   └─ [✓ Correct] [↻ Change] [👁 View] per ref
              │
6. User spots error → [↻ Change] → re-search → back to review
              │
7. User satisfied → [Execute] → Summary gate → Execute
```

### Agent Integration

User can voice-prompt the agent at any stage:
- "Resolve all the Luxembourg entities"
- "Change the second one to the UK company"
- "Show me the other John Smith options"
- "Execute when ready"

Agent responds by either updating server state or guiding user to UI options.

---

## Phase 1: Backend API

### 1.1 Resolution Session State

Location: `rust/src/services/resolution_service.rs`

```rust
pub struct ResolutionSession {
    pub id: Uuid,
    pub session_id: Uuid,
    pub state: ResolutionState,
    
    /// Refs needing resolution
    pub unresolved: Vec<UnresolvedRef>,
    
    /// Auto-resolved (exact match, reference data)
    pub auto_resolved: Vec<ResolvedRef>,
    
    /// User resolutions (confirmed)
    pub resolved: HashMap<String, ResolvedRef>,
}

pub enum ResolutionState {
    Resolving,   // User picking entities
    Reviewing,   // All resolved, user reviewing
    Committed,   // Applied to AST
    Cancelled,
}

pub struct UnresolvedRef {
    pub ref_id: String,
    pub entity_type: String,
    pub entity_subtype: Option<String>,
    pub search_value: String,
    pub search_schema: SearchSchema,
    pub context: RefContext,
    
    /// Pre-fetched initial matches
    pub initial_matches: Vec<EntityMatch>,
    
    /// Agent's suggested resolution (if confident)
    pub agent_suggestion: Option<EntityMatch>,
    pub suggestion_reason: Option<String>,
    
    /// Review requirement level
    pub review_requirement: ReviewRequirement,
}

pub struct ResolvedRef {
    pub ref_id: String,
    pub entity_type: String,
    pub original_search: String,
    pub resolved_key: Uuid,
    pub display: String,
    
    /// For review panel - key discriminators
    pub discriminators: HashMap<String, String>,
    pub entity_status: EntityStatus,
    pub warnings: Vec<ResolutionWarning>,
    pub alternative_count: usize,
    pub confidence: f32,
    
    /// Review tracking
    pub reviewed: bool,
    pub changed_from_original: bool,
}

pub enum ReviewRequirement {
    Optional,     // Auto-resolved, high confidence
    Recommended,  // Warnings present
    Required,     // Low confidence, multiple close matches
}
```

### 1.2 API Endpoints

Location: `rust/src/api/resolution_routes.rs`

| Endpoint | Method | Request | Response |
|----------|--------|---------|----------|
| `/api/session/:id/resolution/start` | POST | - | `ResolutionSessionResponse` |
| `/api/session/:id/resolution/search` | POST | `SearchRequest` | `SearchResponse` |
| `/api/session/:id/resolution/select` | POST | `SelectRequest` | `SelectResponse` |
| `/api/session/:id/resolution/commit` | POST | - | `CommitResponse` |
| `/api/session/:id/resolution/cancel` | POST | - | `CancelResponse` |

**Types go in `ob-poc-types`** - shared by server and WASM.

```rust
// ob-poc-types/src/resolution.rs

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ResolutionSessionResponse {
    pub id: String,  // UUID as string for TS compat
    pub state: String,
    pub unresolved: Vec<UnresolvedRefResponse>,
    pub auto_resolved: Vec<ResolvedRefResponse>,
    pub resolved: Vec<ResolvedRefResponse>,
    pub summary: ResolutionSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ResolutionSummary {
    pub total_refs: usize,
    pub resolved_count: usize,
    pub warnings_count: usize,
    pub required_review_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SelectRequest {
    pub ref_id: String,
    pub resolved_key: String,  // UUID as string
}
```

### 1.3 Unit Tests

- [ ] Extract unresolved refs from AST
- [ ] Auto-resolve exact matches (reference data)
- [ ] Search with discriminators
- [ ] Select resolution updates state
- [ ] Commit writes to AST
- [ ] State transitions (Resolving → Reviewing → Committed)

---

## Phase 2: Resolution Panel UI (WASM/egui)

**⚠️ MUST follow patterns in CLAUDE.md egui section**

### 2.1 State Structure

Location: `ob-poc-ui/src/state.rs`

```rust
// Add to AppState
pub struct AppState {
    // ... existing ...
    
    /// Resolution session from server (fetched, never modified locally)
    pub resolution: Option<ResolutionSessionResponse>,
    
    /// UI-only state for resolution panel
    pub resolution_ui: ResolutionPanelUi,
}

/// UI-only ephemeral state (NOT server state)
pub struct ResolutionPanelUi {
    /// Search input buffers (ONLY local mutable state)
    pub search_buffers: HashMap<String, String>,
    
    /// Currently focused ref index
    pub focused_idx: usize,
    
    /// Expanded ref for re-resolution (None = collapsed)
    pub expanded_ref: Option<String>,
    
    /// View mode
    pub mode: ResolutionViewMode,
}

pub enum ResolutionViewMode {
    Resolving,  // Batch resolution
    Reviewing,  // Review all before execute
    Summary,    // Final confirmation
}
```

### 2.2 Panel Component

Location: `ob-poc-ui/src/panels/resolution_panel.rs`

```rust
/// Actions the panel can return (NOT callbacks)
pub enum ResolutionAction {
    Search { ref_id: String, query: String },
    Select { ref_id: String, entity_key: String },
    Confirm { ref_id: String },
    ReResolve { ref_id: String },
    ConfirmAll,
    Execute,
    Cancel,
}

/// Pure render function - receives state, returns actions
pub fn resolution_panel(
    ui: &mut egui::Ui,
    resolution: &ResolutionSessionResponse,
    ui_state: &mut ResolutionPanelUi,
) -> Option<ResolutionAction> {
    
    // Header with progress
    ui.horizontal(|ui| {
        ui.heading("Entity Resolution");
        let summary = &resolution.summary;
        ui.label(format!("{}/{} resolved", summary.resolved_count, summary.total_refs));
        if summary.warnings_count > 0 {
            ui.colored_label(egui::Color32::YELLOW, format!("⚠ {}", summary.warnings_count));
        }
    });
    
    ui.separator();
    
    // Render based on mode
    match ui_state.mode {
        ResolutionViewMode::Resolving => render_resolving_mode(ui, resolution, ui_state),
        ResolutionViewMode::Reviewing => render_review_mode(ui, resolution, ui_state),
        ResolutionViewMode::Summary => render_summary_mode(ui, resolution),
    }
}

fn render_resolving_mode(...) -> Option<ResolutionAction> {
    // Group by confidence
    let (auto, high_conf, needs_attention) = group_by_confidence(&resolution.unresolved);
    
    // Auto-resolved (collapsed)
    if !auto.is_empty() {
        ui.collapsing(format!("🟢 Auto-resolved ({})", auto.len()), |ui| {
            for resolved in auto {
                ui.label(format!("{} → {}", resolved.original_search, resolved.display));
            }
        });
    }
    
    // High confidence (agent suggestions, one-click confirm)
    if !high_conf.is_empty() {
        ui.label(format!("🟡 High confidence ({})", high_conf.len()));
        for unresolved in high_conf {
            if let Some(suggestion) = &unresolved.agent_suggestion {
                ui.horizontal(|ui| {
                    ui.label(&unresolved.search_value);
                    ui.label("→");
                    ui.label(&suggestion.display);
                    if ui.button("✓").clicked() {
                        return Some(ResolutionAction::Select {
                            ref_id: unresolved.ref_id.clone(),
                            entity_key: suggestion.id.clone(),
                        });
                    }
                });
            }
        }
    }
    
    // Needs attention (full search UI)
    for unresolved in needs_attention {
        render_ref_resolver(ui, unresolved, ui_state)
    }
    
    // Actions
    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("Confirm All High-Confidence").clicked() {
            return Some(ResolutionAction::ConfirmAll);
        }
        if ui.button("Cancel").clicked() {
            return Some(ResolutionAction::Cancel);
        }
    });
    
    None
}
```

### 2.3 Review Mode

```rust
fn render_review_mode(
    ui: &mut egui::Ui,
    resolution: &ResolutionSessionResponse,
    ui_state: &mut ResolutionPanelUi,
) -> Option<ResolutionAction> {
    
    ui.label("Review resolved entities before execution:");
    
    egui::ScrollArea::vertical().show(ui, |ui| {
        for resolved in &resolution.resolved {
            let is_expanded = ui_state.expanded_ref.as_ref() == Some(&resolved.ref_id);
            
            egui::Frame::none()
                .fill(if resolved.warnings.is_empty() { 
                    egui::Color32::from_gray(40) 
                } else { 
                    egui::Color32::from_rgb(60, 50, 30)  // Warning tint
                })
                .show(ui, |ui| {
                    // Header row
                    ui.horizontal(|ui| {
                        ui.strong(&resolved.original_search);
                        ui.label("→");
                        ui.label(&resolved.display);
                        
                        // Warning indicator
                        if !resolved.warnings.is_empty() {
                            ui.colored_label(egui::Color32::YELLOW, "⚠");
                        }
                    });
                    
                    // Discriminators (jurisdiction, DOB, etc.)
                    ui.horizontal(|ui| {
                        for (key, value) in &resolved.discriminators {
                            ui.small(format!("{}: {}", key, value));
                        }
                    });
                    
                    // Warnings
                    for warning in &resolved.warnings {
                        ui.colored_label(egui::Color32::YELLOW, format!("⚠ {}", warning.message));
                    }
                    
                    // Action buttons
                    ui.horizontal(|ui| {
                        if ui.button("✓ Correct").clicked() {
                            return Some(ResolutionAction::Confirm { 
                                ref_id: resolved.ref_id.clone() 
                            });
                        }
                        if ui.button("↻ Change").clicked() {
                            return Some(ResolutionAction::ReResolve { 
                                ref_id: resolved.ref_id.clone() 
                            });
                        }
                        if ui.button("👁 View").clicked() {
                            // Open entity detail (separate panel/modal)
                        }
                    });
                    
                    // Expanded re-resolution UI
                    if is_expanded {
                        render_ref_resolver(ui, /* convert to unresolved */, ui_state);
                    }
                });
        }
    });
    
    // Final actions
    ui.separator();
    ui.horizontal(|ui| {
        let can_execute = resolution.summary.required_review_count == 0;
        ui.add_enabled_ui(can_execute, |ui| {
            if ui.button("Execute").clicked() {
                return Some(ResolutionAction::Execute);
            }
        });
        if !can_execute {
            ui.small(format!("{} items require review", resolution.summary.required_review_count));
        }
    });
    
    None
}
```

### 2.4 Keyboard Navigation

```rust
fn handle_resolution_keyboard(
    ctx: &egui::Context,
    ui_state: &mut ResolutionPanelUi,
    resolution: &ResolutionSessionResponse,
) -> Option<ResolutionAction> {
    ctx.input(|i| {
        // Tab - next ref
        if i.key_pressed(egui::Key::Tab) && !i.modifiers.shift {
            ui_state.focused_idx = (ui_state.focused_idx + 1) % resolution.unresolved.len();
            return None;
        }
        
        // Shift+Tab - previous ref
        if i.key_pressed(egui::Key::Tab) && i.modifiers.shift {
            ui_state.focused_idx = ui_state.focused_idx.saturating_sub(1);
            return None;
        }
        
        // Enter - confirm current
        if i.key_pressed(egui::Key::Enter) {
            if let Some(unresolved) = resolution.unresolved.get(ui_state.focused_idx) {
                if let Some(suggestion) = &unresolved.agent_suggestion {
                    return Some(ResolutionAction::Select {
                        ref_id: unresolved.ref_id.clone(),
                        entity_key: suggestion.id.clone(),
                    });
                }
            }
        }
        
        // 1-5 - quick select match
        for (idx, key) in [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5].iter().enumerate() {
            if i.key_pressed(*key) {
                if let Some(unresolved) = resolution.unresolved.get(ui_state.focused_idx) {
                    if let Some(match_) = unresolved.initial_matches.get(idx) {
                        return Some(ResolutionAction::Select {
                            ref_id: unresolved.ref_id.clone(),
                            entity_key: match_.id.clone(),
                        });
                    }
                }
            }
        }
        
        // Ctrl+Enter - execute (if ready)
        if i.key_pressed(egui::Key::Enter) && i.modifiers.command {
            return Some(ResolutionAction::Execute);
        }
        
        // Escape - cancel
        if i.key_pressed(egui::Key::Escape) {
            return Some(ResolutionAction::Cancel);
        }
        
        None
    })
}
```

### 2.5 Async Integration

**Follow CLAUDE.md async pattern exactly:**

```rust
// In App::update()
fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    // 1. Process async results FIRST
    self.process_resolution_async();
    
    // 2. Extract state (short lock)
    let resolution = self.resolution.clone();
    
    // 3. Render and collect actions
    let action = if let Some(ref res) = resolution {
        resolution_panel(ui, res, &mut self.resolution_ui)
    } else {
        None
    };
    
    // 4. Handle actions (dispatch async, update UI state)
    if let Some(action) = action {
        self.handle_resolution_action(action);
    }
}

fn handle_resolution_action(&mut self, action: ResolutionAction) {
    match action {
        ResolutionAction::Search { ref_id, query } => {
            // Debounced search
            self.post_resolution_search(&ref_id, &query);
        }
        ResolutionAction::Select { ref_id, entity_key } => {
            // POST selection to server
            self.post_resolution_select(&ref_id, &entity_key);
            // Server will transition state if all resolved → refetch
        }
        ResolutionAction::Execute => {
            // POST commit, then execute DSL
            self.post_resolution_commit();
        }
        // ... etc
    }
}

fn process_resolution_async(&mut self) {
    let pending = {
        let mut state = self.async_state.lock().unwrap();
        state.pending_resolution.take()
    };  // Lock released
    
    if let Some(result) = pending {
        match result {
            Ok(resolution) => {
                self.resolution = Some(resolution);
                // Update UI mode based on server state
                if self.resolution.as_ref().map(|r| r.state.as_str()) == Some("reviewing") {
                    self.resolution_ui.mode = ResolutionViewMode::Reviewing;
                }
            }
            Err(e) => {
                // Show error in UI
            }
        }
    }
}
```

---

## Phase 3: Agent Integration

### 3.1 Agent Context

Agent needs visibility into resolution state:

```rust
pub struct AgentContext {
    // ... existing ...
    
    /// Current resolution session (if active)
    pub resolution: Option<ResolutionContextInfo>,
}

pub struct ResolutionContextInfo {
    pub state: String,
    pub unresolved_count: usize,
    pub resolved_count: usize,
    pub unresolved_summaries: Vec<String>,  // "Allianz Global (company)", etc.
}
```

### 3.2 Agent Commands

Agent can help with resolution via voice:

| User Says | Agent Action |
|-----------|--------------|
| "Resolve all" | Triggers batch auto-resolve for high-confidence |
| "Change the second one" | Expands ref #2 for re-resolution |
| "Use the Luxembourg company" | Searches with jurisdiction filter, selects |
| "Execute" | Commits and executes if all resolved |
| "Show me other options for John Smith" | Re-fetches matches for that ref |

### 3.3 ResolutionRequired Message Type

```rust
pub enum ChatResponseContent {
    Text(String),
    Dsl { source: String, status: DslStatus },
    ResolutionRequired {
        resolution_id: String,
        unresolved_count: usize,
        agent_message: String,  // "I've found 3 entities that need your confirmation"
        suggestions_made: bool,  // True if agent pre-resolved some
    },
}
```

---

## Phase 4: Enhancements

### 4.1 "Apply to Similar" for Bulk

```rust
pub struct BulkResolutionOption {
    pub pattern: BulkPattern,
    pub suggestion: EntityMatch,
    pub matching_refs: Vec<String>,  // ref_ids
}

pub enum BulkPattern {
    SameSearchValue(String),        // All refs with same search term
    SameEntityType(String),         // All company refs
    SameVerbArg { verb: String, arg: String },
}

// UI shows checkbox:
// ☑ Apply to 46 similar refs
```

### 4.2 Resolution Memory

```rust
pub struct ResolutionMemory {
    /// Session-level (always)
    pub session_mappings: HashMap<String, String>,  // search → uuid
    
    /// Persistent (opt-in)
    pub persistent_mappings: HashMap<String, PersistedMapping>,
}

// UI checkbox: ☑ Remember this mapping
```

### 4.3 Cross-Resolution Context

After resolving CBU in Luxembourg, boost Luxembourg entities for remaining refs.

---

## Verification Gates

### Gate 1: Backend API
```bash
cargo test --package ob-poc resolution::
# All resolution service tests pass

curl -X POST http://localhost:3000/api/session/$ID/resolution/start
# Returns ResolutionSessionResponse
```

### Gate 2: Basic UI
```
1. Generate DSL with unresolved refs
2. Resolution panel appears
3. Can search, select, confirm
4. State transitions to Reviewing
5. Can execute
```

### Gate 3: Review/Repair Loop
```
1. Complete resolution
2. Panel shows review mode with discriminators
3. Click [↻ Change] on a ref
4. Search UI expands inline
5. Select different entity
6. Panel updates
7. Execute
```

### Gate 4: Keyboard Navigation
```
Tab/Shift+Tab navigates refs
Enter confirms agent suggestion
1-5 selects match by number
Ctrl+Enter executes
```

### Gate 5: Agent Integration
```
User: "Change the second entity to the UK company"
Agent: Updates resolution → UI reflects change
User: "Execute"
Agent: Commits and executes
```

---

## File Checklist

```
# Types (ob-poc-types)
ob-poc-types/src/resolution.rs              [ ]

# Backend
rust/src/services/resolution_service.rs     [ ]
rust/src/api/resolution_routes.rs           [ ]
rust/src/api/mod.rs                         [ ] (add routes)

# UI (ob-poc-ui)
ob-poc-ui/src/state.rs                      [ ] (add resolution state)
ob-poc-ui/src/panels/resolution_panel.rs    [ ]
ob-poc-ui/src/panels/mod.rs                 [ ] (add panel)

# Tests
rust/src/services/resolution_service_test.rs [ ]
```

---

## Non-Goals (Out of Scope)

- Entity creation in resolution panel (separate flow)
- Resolution versioning/history
- Multi-session resolution sharing
- Offline resolution caching

---

## Checklist Before Starting

- [ ] Read CLAUDE.md egui section (MANDATORY)
- [ ] Read EGUI.md for async patterns
- [ ] Understand server-first architecture
- [ ] No local state mirroring server data
- [ ] Actions return values, no callbacks
- [ ] Short lock durations, never across UI rendering
