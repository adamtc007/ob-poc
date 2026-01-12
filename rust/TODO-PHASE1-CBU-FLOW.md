# Phase 1: Core CBU Selection Flow

## Objective
Make "select allianz lux cbu" work end-to-end:
1. User types in chat
2. Agent returns disambiguation request (multiple CBU matches)
3. Resolution modal opens automatically
4. User picks one
5. Session updates with bound CBU
6. Viewport shows CBU graph with focus state
7. Trading matrix loads

## CRITICAL: Follow EGUI-RULES.md
- Panels return `Option<Action>` enums, NO callbacks
- Server data is read-only, never mutated locally
- UI-only state in dedicated structs
- Single async_state lock, extract data, release, then render

---

## Task 1: ChatResponse → Resolution Modal Trigger

### Problem
`ChatResponse` includes `disambiguation_request: Option<DisambiguationRequest>` but `app.rs` ignores it.

### Files to Modify

#### 1.1 `crates/ob-poc-ui/src/state.rs`

Add to `AsyncState` struct (around line 600):
```rust
/// Disambiguation request from agent that needs user resolution
pub pending_disambiguation: Option<ob_poc_types::DisambiguationRequest>,
```

#### 1.2 `crates/ob-poc-ui/src/app.rs`

In the `send_chat_message` spawn_local callback (around line 3300), after processing `chat_response.commands`:

```rust
// Handle disambiguation request - opens resolution modal
if let Some(disambig) = chat_response.disambiguation_request {
    web_sys::console::log_1(
        &format!(
            "send_chat: disambiguation requested for {} items",
            disambig.items.len()
        )
        .into(),
    );
    state.pending_disambiguation = Some(disambig);
}
```

#### 1.3 `crates/ob-poc-ui/src/app.rs`

In `process_async_results()` (around line 1050), add handler for pending_disambiguation:

```rust
// Process pending disambiguation request
if let Some(disambig) = state.pending_disambiguation.take() {
    // Convert to resolution modal format and open
    self.open_disambiguation_modal(disambig);
}
```

#### 1.4 `crates/ob-poc-ui/src/app.rs`

Add new method to `AppState`:

```rust
/// Open disambiguation modal for entity resolution
fn open_disambiguation_modal(&mut self, disambig: ob_poc_types::DisambiguationRequest) {
    use crate::state::{WindowData, WindowEntry, WindowType};
    
    // Create window entry for resolution modal
    let window = WindowEntry {
        id: format!("disambig-{}", uuid::Uuid::new_v4()),
        window_type: WindowType::Resolution,
        layer: 2, // Modal layer
        modal: true,
        data: Some(WindowData::Disambiguation {
            request: disambig.clone(),
            current_item_index: 0,
            search_results: None,
        }),
    };
    
    self.window_stack.push(window);
    
    // Initialize search buffer with first item's search text
    if let Some(first_item) = disambig.items.first() {
        self.resolution_ui.search_buffer = first_item.search_text.clone();
        // Auto-trigger search for first item
        self.search_disambiguation_matches(&first_item.search_text, &first_item.entity_type);
    }
}
```

#### 1.5 `crates/ob-poc-ui/src/state.rs`

Add `WindowData::Disambiguation` variant to `WindowData` enum (around line 380):

```rust
#[derive(Clone, Debug)]
pub enum WindowData {
    Resolution {
        subsession_id: String,
        current_ref_index: usize,
        total_refs: usize,
        matches: Vec<crate::panels::resolution::EntityMatchDisplay>,
    },
    /// Disambiguation from agent chat - simpler than full resolution
    Disambiguation {
        request: ob_poc_types::DisambiguationRequest,
        current_item_index: usize,
        search_results: Option<Vec<ob_poc_types::EntityMatchOption>>,
    },
    ContainerBrowse {
        entity_id: String,
        entity_type: String,
        browse_nickname: Option<String>,
    },
}
```

---

## Task 2: Disambiguation Modal UI

### Problem
Need a modal that shows matches and lets user select one.

### Files to Modify

#### 2.1 Create `crates/ob-poc-ui/src/panels/disambiguation.rs`

```rust
//! Disambiguation Modal Panel
//!
//! Shows entity matches from agent and lets user pick one.
//! Simpler than full resolution - just pick from pre-searched results.
//!
//! Follows EGUI-RULES:
//! - Returns Option<DisambiguationAction>, no callbacks
//! - Data passed in, not mutated

use egui::{Color32, RichText, ScrollArea, TextEdit, Ui, Vec2};
use ob_poc_types::{DisambiguationRequest, EntityMatchOption};

/// Actions from disambiguation modal
#[derive(Clone, Debug)]
pub enum DisambiguationAction {
    /// User selected a match
    Select {
        item_index: usize,
        entity_id: String,
        entity_type: String,
        display_name: String,
    },
    /// User wants to search with different text
    Search { query: String },
    /// User wants to skip this item
    Skip,
    /// User cancelled entire disambiguation
    Cancel,
    /// User closed modal
    Close,
}

/// Data needed to render disambiguation modal
pub struct DisambiguationModalData<'a> {
    pub request: &'a DisambiguationRequest,
    pub current_item_index: usize,
    pub matches: Option<&'a [EntityMatchOption]>,
    pub searching: bool,
}

/// Render disambiguation modal
/// Returns action if user interacted
pub fn disambiguation_modal(
    ctx: &egui::Context,
    search_buffer: &mut String,
    data: &DisambiguationModalData<'_>,
) -> Option<DisambiguationAction> {
    let mut action: Option<DisambiguationAction> = None;
    
    let current_item = data.request.items.get(data.current_item_index)?;
    let total_items = data.request.items.len();
    
    egui::Window::new("Select Entity")
        .collapsible(false)
        .resizable(true)
        .default_size(Vec2::new(500.0, 400.0))
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            // Header with progress
            ui.horizontal(|ui| {
                ui.heading("Resolve Reference");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("✕").clicked() {
                        action = Some(DisambiguationAction::Close);
                    }
                    ui.label(
                        RichText::new(format!("{} of {}", data.current_item_index + 1, total_items))
                            .small()
                            .color(Color32::GRAY),
                    );
                });
            });
            
            ui.separator();
            ui.add_space(4.0);
            
            // What we're resolving
            ui.horizontal(|ui| {
                ui.label("Looking for:");
                ui.label(
                    RichText::new(&current_item.search_text)
                        .strong()
                        .color(Color32::YELLOW),
                );
                ui.label(
                    RichText::new(format!("[{}]", current_item.entity_type))
                        .small()
                        .color(Color32::LIGHT_BLUE),
                );
            });
            
            // Search refinement
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("Refine:");
                let response = TextEdit::singleline(search_buffer)
                    .desired_width(300.0)
                    .hint_text("Type to search...")
                    .show(ui);
                
                if response.response.changed() && search_buffer.len() >= 2 {
                    action = Some(DisambiguationAction::Search {
                        query: search_buffer.clone(),
                    });
                }
                
                if data.searching {
                    ui.spinner();
                }
            });
            
            ui.add_space(8.0);
            ui.separator();
            
            // Matches
            ui.label(RichText::new("Matches:").small().color(Color32::GRAY));
            
            ScrollArea::vertical()
                .max_height(200.0)
                .show(ui, |ui| {
                    if let Some(matches) = data.matches {
                        if matches.is_empty() {
                            ui.label(
                                RichText::new("No matches found")
                                    .color(Color32::GRAY)
                                    .italics(),
                            );
                        } else {
                            for (idx, m) in matches.iter().enumerate() {
                                if let Some(select_action) = render_match_row(
                                    ui,
                                    idx,
                                    m,
                                    data.current_item_index,
                                    &current_item.entity_type,
                                ) {
                                    action = Some(select_action);
                                }
                            }
                        }
                    } else {
                        ui.label(
                            RichText::new("Searching...")
                                .color(Color32::GRAY)
                                .italics(),
                        );
                    }
                });
            
            ui.add_space(8.0);
            ui.separator();
            
            // Footer buttons
            ui.horizontal(|ui| {
                if ui.button("Skip").clicked() {
                    action = Some(DisambiguationAction::Skip);
                }
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Cancel").clicked() {
                        action = Some(DisambiguationAction::Cancel);
                    }
                });
            });
        });
    
    action
}

fn render_match_row(
    ui: &mut Ui,
    index: usize,
    m: &EntityMatchOption,
    item_index: usize,
    entity_type: &str,
) -> Option<DisambiguationAction> {
    let mut action: Option<DisambiguationAction> = None;
    
    let score_color = if m.score > 0.9 {
        Color32::from_rgb(100, 200, 100)
    } else if m.score > 0.7 {
        Color32::from_rgb(200, 180, 80)
    } else {
        Color32::from_rgb(180, 140, 100)
    };
    
    egui::Frame::default()
        .fill(Color32::from_rgb(45, 50, 55))
        .inner_margin(8.0)
        .rounding(4.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Index hint
                ui.label(
                    RichText::new(format!("{}.", index + 1))
                        .small()
                        .color(Color32::GRAY),
                );
                
                // Select button
                if ui.button("Select").clicked() {
                    action = Some(DisambiguationAction::Select {
                        item_index,
                        entity_id: m.id.clone(),
                        entity_type: entity_type.to_string(),
                        display_name: m.display.clone(),
                    });
                }
                
                // Display name
                ui.label(RichText::new(&m.display).strong());
                
                // Detail (jurisdiction, etc.)
                if let Some(ref detail) = m.detail {
                    ui.label(
                        RichText::new(detail)
                            .small()
                            .color(Color32::LIGHT_GRAY),
                    );
                }
                
                // Score on right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{:.0}%", m.score * 100.0))
                            .small()
                            .color(score_color),
                    );
                });
            });
        });
    
    ui.add_space(2.0);
    action
}
```

#### 2.2 `crates/ob-poc-ui/src/panels/mod.rs`

Add module:
```rust
pub mod disambiguation;
pub use disambiguation::*;
```

#### 2.3 `crates/ob-poc-ui/src/app.rs`

Add rendering of disambiguation modal in the main render loop (in `update()` around line 800):

```rust
// Render disambiguation modal if active
if let Some(window) = self.window_stack.find_by_type(WindowType::Resolution) {
    if let Some(WindowData::Disambiguation { ref request, current_item_index, ref search_results }) = window.data {
        let searching = {
            self.async_state.lock().map(|s| s.loading_disambiguation).unwrap_or(false)
        };
        
        let data = crate::panels::disambiguation::DisambiguationModalData {
            request,
            current_item_index,
            matches: search_results.as_deref(),
            searching,
        };
        
        if let Some(action) = crate::panels::disambiguation::disambiguation_modal(
            ctx,
            &mut self.resolution_ui.search_buffer,
            &data,
        ) {
            self.handle_disambiguation_action(action);
        }
    }
}
```

---

## Task 3: Disambiguation Selection → Session Bind

### Problem
When user selects an entity, need to:
1. Call `POST /api/session/{id}/bind` 
2. Close modal
3. Trigger session refresh

### Files to Modify

#### 3.1 `crates/ob-poc-ui/src/app.rs`

Add handler method:

```rust
/// Handle disambiguation modal action
fn handle_disambiguation_action(&mut self, action: crate::panels::DisambiguationAction) {
    use crate::panels::DisambiguationAction;
    
    match action {
        DisambiguationAction::Select { item_index: _, entity_id, entity_type, display_name } => {
            // Close modal
            self.window_stack.close_by_type(WindowType::Resolution);
            
            // Bind entity to session
            if let Some(session_id) = self.session_id {
                self.bind_entity_to_session(session_id, &entity_id, &entity_type, &display_name);
            }
        }
        
        DisambiguationAction::Search { query } => {
            // Get current item's entity type
            if let Some(window) = self.window_stack.find_by_type(WindowType::Resolution) {
                if let Some(WindowData::Disambiguation { ref request, current_item_index, .. }) = window.data {
                    if let Some(item) = request.items.get(current_item_index) {
                        self.search_disambiguation_matches(&query, &item.entity_type);
                    }
                }
            }
        }
        
        DisambiguationAction::Skip => {
            // Move to next item or close if done
            self.advance_disambiguation_item();
        }
        
        DisambiguationAction::Cancel | DisambiguationAction::Close => {
            self.window_stack.close_by_type(WindowType::Resolution);
        }
    }
}

/// Bind an entity to the session and trigger refresh
fn bind_entity_to_session(&mut self, session_id: Uuid, entity_id: &str, entity_type: &str, display_name: &str) {
    let entity_uuid = match Uuid::parse_str(entity_id) {
        Ok(id) => id,
        Err(e) => {
            web_sys::console::error_1(&format!("Invalid entity UUID: {}", e).into());
            return;
        }
    };
    
    let async_state = Arc::clone(&self.async_state);
    let ctx = self.ctx.clone();
    let entity_type = entity_type.to_string();
    let display_name = display_name.to_string();
    
    {
        let mut state = self.async_state.lock().unwrap();
        state.loading_session = true;
    }
    
    spawn_local(async move {
        let result = crate::api::bind_entity(
            session_id,
            entity_uuid,
            &entity_type,
            &display_name,
        ).await;
        
        if let Ok(mut state) = async_state.lock() {
            state.loading_session = false;
            
            match result {
                Ok(_response) => {
                    web_sys::console::log_1(
                        &format!("Bound {} {} to session", entity_type, display_name).into()
                    );
                    // Trigger full session refresh to get updated context
                    state.pending_session_refetch = true;
                }
                Err(e) => {
                    state.last_error = Some(format!("Failed to bind entity: {}", e));
                }
            }
        }
        
        if let Some(ctx) = ctx {
            ctx.request_repaint();
        }
    });
}

/// Search for disambiguation matches
fn search_disambiguation_matches(&mut self, query: &str, entity_type: &str) {
    let Some(session_id) = self.session_id else { return };
    
    let async_state = Arc::clone(&self.async_state);
    let ctx = self.ctx.clone();
    let query = query.to_string();
    let entity_type = entity_type.to_string();
    
    {
        let mut state = self.async_state.lock().unwrap();
        state.loading_disambiguation = true;
    }
    
    spawn_local(async move {
        // Use entity search API
        let result = crate::api::search_entities(&query, &entity_type, 10).await;
        
        if let Ok(mut state) = async_state.lock() {
            state.loading_disambiguation = false;
            state.pending_disambiguation_results = Some(result);
        }
        
        if let Some(ctx) = ctx {
            ctx.request_repaint();
        }
    });
}

/// Advance to next disambiguation item or close if done
fn advance_disambiguation_item(&mut self) {
    if let Some(window) = self.window_stack.find_by_type_mut(WindowType::Resolution) {
        if let Some(WindowData::Disambiguation { ref request, ref mut current_item_index, ref mut search_results }) = window.data {
            *current_item_index += 1;
            if *current_item_index >= request.items.len() {
                // All items processed, close modal
                self.window_stack.close_by_type(WindowType::Resolution);
            } else {
                // Clear results and start search for next item
                *search_results = None;
                if let Some(item) = request.items.get(*current_item_index) {
                    self.resolution_ui.search_buffer = item.search_text.clone();
                    self.search_disambiguation_matches(&item.search_text, &item.entity_type);
                }
            }
        }
    }
}
```

#### 3.2 `crates/ob-poc-ui/src/state.rs`

Add to `AsyncState`:

```rust
/// Loading disambiguation search results
pub loading_disambiguation: bool,

/// Pending disambiguation search results
pub pending_disambiguation_results: Option<Result<Vec<ob_poc_types::EntityMatchOption>, String>>,

/// Flag to trigger session refetch after bind
pub pending_session_refetch: bool,
```

#### 3.3 `crates/ob-poc-ui/src/api.rs`

Add entity search API function:

```rust
/// Search for entities by type
pub async fn search_entities(
    query: &str,
    entity_type: &str,
    limit: u32,
) -> Result<Vec<ob_poc_types::EntityMatchOption>, String> {
    let encoded_query = js_sys::encode_uri_component(query);
    let encoded_type = js_sys::encode_uri_component(entity_type);
    
    #[derive(Deserialize)]
    struct SearchResponse {
        matches: Vec<ob_poc_types::EntityMatchOption>,
    }
    
    let response: SearchResponse = get(&format!(
        "/api/entity/search?q={}&type={}&limit={}",
        encoded_query, encoded_type, limit
    )).await?;
    
    Ok(response.matches)
}
```

---

## Task 4: Session Bind → Graph + Matrix Auto-Fetch

### Problem
After binding a CBU, need to automatically:
1. Fetch CBU graph data
2. Fetch trading matrix
3. Update viewport focus state

### Files to Modify

#### 4.1 `crates/ob-poc-ui/src/app.rs`

In `process_async_results()`, add handler for session refetch flag:

```rust
// Handle pending session refetch (after bind)
if state.pending_session_refetch {
    state.pending_session_refetch = false;
    drop(state); // Release lock before calling methods
    self.refetch_session();
    self.fetch_session_context();
    return; // Process other results next frame
}
```

#### 4.2 `crates/ob-poc-ui/src/app.rs`

Modify `process_async_results()` session context handler to auto-fetch graph and matrix:

```rust
// Process session context
if let Some(result) = state.pending_session_context.take() {
    state.loading_session_context = false;
    match result {
        Ok(context) => {
            // Apply viewport_state to graph widget if present
            if let Some(ref viewport_state) = context.viewport_state {
                self.graph_widget.apply_viewport_state(viewport_state);
            }
            
            // If we have an active CBU, auto-fetch graph and trading matrix
            if let Some(ref cbu) = context.cbu {
                let cbu_id = Uuid::parse_str(&cbu.id).ok();
                if let Some(cbu_id) = cbu_id {
                    // Check if we need to fetch graph (CBU changed or no data)
                    let needs_graph = self.graph_data.as_ref()
                        .map(|g| g.cbu_id != cbu.id)
                        .unwrap_or(true);
                    
                    if needs_graph {
                        web_sys::console::log_1(
                            &format!("Auto-fetching graph for CBU {}", cbu.name).into()
                        );
                        self.fetch_cbu_graph(cbu_id);
                    }
                    
                    // Check if we need to fetch trading matrix
                    let needs_matrix = self.trading_matrix.as_ref()
                        .map(|m| m.document.cbu_id != cbu.id)
                        .unwrap_or(true);
                    
                    if needs_matrix {
                        web_sys::console::log_1(
                            &format!("Auto-fetching trading matrix for CBU {}", cbu.name).into()
                        );
                        self.fetch_trading_matrix(cbu_id);
                    }
                }
            }
            
            self.session_context = Some(context);
        }
        Err(e) => state.last_error = Some(format!("Session context fetch failed: {}", e)),
    }
}
```

---

## Task 5: Session → Viewport Focus State Sync

### Problem
When session has a bound CBU, viewport focus should be set to `CbuContainer`.

### Files to Modify

#### 5.1 `crates/ob-poc-graph/src/cbu_graph_widget.rs`

Modify `apply_viewport_state()` to actually apply focus state:

```rust
/// Apply viewport state from session
pub fn apply_viewport_state(&mut self, viewport_state: &ob_poc_types::ViewportState) {
    // Apply camera state
    self.camera.pan.x = viewport_state.camera.x;
    self.camera.pan.y = viewport_state.camera.y;
    self.camera.zoom = viewport_state.camera.zoom;
    
    // Apply focus state
    self.focus_manager = viewport_state.focus.clone();
    
    // Apply view type
    self.view_type = viewport_state.view_type;
    
    // Apply confidence threshold
    self.confidence_threshold = viewport_state.confidence_threshold;
    
    // Apply filters
    self.filters = viewport_state.filters.clone();
}
```

#### 5.2 `rust/src/api/session.rs`

Ensure `SessionContext` updates `viewport_state` when CBU is bound:

In the bind handler (or wherever `active_cbu` is set), add:

```rust
// When binding a CBU, initialize viewport focus
if binding_name == "cbu" {
    let cbu_id = Uuid::parse_str(&entity_id)?;
    
    // Set viewport focus to the bound CBU
    self.context.viewport_state = Some(ViewportState {
        focus: FocusManager {
            state: ViewportFocusState::CbuContainer {
                cbu: CbuRef::new(cbu_id),
                enhance_level: 0,
            },
            focus_stack: Vec::new(),
            focus_mode: FocusMode::default(),
            view_memory: HashMap::new(),
        },
        view_type: CbuViewType::Structure,
        camera: CameraState::default(),
        confidence_threshold: 0.0,
        filters: ViewportFilters::default(),
    });
}
```

---

## Task 6: Window Stack Helper Methods

### Problem
Need helper methods to find and manipulate windows in stack.

### Files to Modify

#### 6.1 `crates/ob-poc-ui/src/state.rs`

Add methods to `WindowStack`:

```rust
impl WindowStack {
    /// Find a window by type (immutable)
    pub fn find_by_type(&self, window_type: WindowType) -> Option<&WindowEntry> {
        self.windows.iter().find(|w| w.window_type == window_type)
    }
    
    /// Find a window by type (mutable)
    pub fn find_by_type_mut(&mut self, window_type: WindowType) -> Option<&mut WindowEntry> {
        self.windows.iter_mut().find(|w| w.window_type == window_type)
    }
    
    /// Close all windows of a given type
    pub fn close_by_type(&mut self, window_type: WindowType) {
        self.windows.retain(|w| w.window_type != window_type);
    }
    
    /// Push a new window onto the stack
    pub fn push(&mut self, window: WindowEntry) {
        self.windows.push(window);
    }
    
    /// Check if any modal is active
    pub fn has_modal(&self) -> bool {
        self.windows.iter().any(|w| w.modal)
    }
}
```

---

## Task 7: ob-poc-types Additions

### Files to Modify

#### 7.1 `crates/ob-poc-types/src/lib.rs`

Ensure these types exist (add if missing):

```rust
/// Request for entity disambiguation from agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisambiguationRequest {
    /// Items needing disambiguation
    pub items: Vec<DisambiguationItem>,
    /// Optional message from agent
    pub message: Option<String>,
}

/// Single item needing disambiguation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisambiguationItem {
    /// Parameter name (e.g., "cbu", "entity")
    pub param_name: String,
    /// Search text from user input
    pub search_text: String,
    /// Entity type being searched
    pub entity_type: String,
    /// Pre-searched matches (if agent already searched)
    #[serde(default)]
    pub matches: Vec<EntityMatchOption>,
}

/// Entity match option for disambiguation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityMatchOption {
    /// Entity UUID
    pub id: String,
    /// Display name
    pub display: String,
    /// Additional detail (jurisdiction, etc.)
    #[serde(default)]
    pub detail: Option<String>,
    /// Match score 0.0-1.0
    pub score: f32,
}
```

#### 7.2 `crates/ob-poc-types/src/lib.rs`

Add to `ChatResponse`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub message: String,
    #[serde(default)]
    pub commands: Option<Vec<AgentCommand>>,
    /// DSL generated by agent
    #[serde(default)]
    pub dsl: Option<String>,
    /// Disambiguation request if agent needs user to resolve entities
    #[serde(default)]
    pub disambiguation_request: Option<DisambiguationRequest>,
}
```

---

## Verification Steps

After implementation, test the complete flow:

1. **Start fresh session**: `cargo run` and open UI

2. **Type ambiguous CBU reference**: 
   ```
   select allianz lux
   ```

3. **Verify modal opens**: Should see disambiguation modal with matches

4. **Select one**: Click "Select" on desired match

5. **Verify graph loads**: CBU graph should appear in viewport

6. **Verify matrix loads**: Trading matrix panel should populate

7. **Verify console logs**:
   ```
   send_chat: disambiguation requested for 1 items
   Bound cbu Allianz Luxembourg Fund 1 to session
   Auto-fetching graph for CBU Allianz Luxembourg Fund 1
   Auto-fetching trading matrix for CBU Allianz Luxembourg Fund 1
   Applied viewport_state from session: focus=CbuContainer { cbu: CbuRef(...), enhance_level: 0 }
   ```

---

## Files Changed Summary

| File | Changes |
|------|---------|
| `ob-poc-types/src/lib.rs` | Add `DisambiguationRequest`, `DisambiguationItem`, `EntityMatchOption`, update `ChatResponse` |
| `ob-poc-ui/src/state.rs` | Add `AsyncState` fields, `WindowData::Disambiguation`, `WindowStack` methods |
| `ob-poc-ui/src/api.rs` | Add `search_entities()` |
| `ob-poc-ui/src/panels/mod.rs` | Add `disambiguation` module |
| `ob-poc-ui/src/panels/disambiguation.rs` | New file - modal UI |
| `ob-poc-ui/src/app.rs` | Add modal trigger, handler, bind logic, auto-fetch |
| `ob-poc-graph/src/cbu_graph_widget.rs` | Expand `apply_viewport_state()` |
| `rust/src/api/session.rs` | Initialize viewport_state on CBU bind |

---

## Anti-Patterns to Avoid

1. **NO STUBS**: Every handler must be complete
2. **NO LOCAL ENTITY CACHE**: Always fetch from server
3. **NO CALLBACKS**: Panels return `Option<Action>`
4. **NO DIRECT STATE MUTATION**: Use pending_ fields in AsyncState
5. **NO BLOCKING**: All API calls via spawn_local

## Success Criteria

- [ ] Type "select allianz" in chat
- [ ] Modal appears with CBU matches
- [ ] Click select → modal closes
- [ ] Graph panel shows CBU structure
- [ ] Trading matrix panel shows CBU's trading config
- [ ] Viewport focus ring appears on CBU node
- [ ] Console shows no errors or "TODO" warnings
