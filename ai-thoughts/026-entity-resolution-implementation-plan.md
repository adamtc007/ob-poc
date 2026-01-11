# 026 Entity Resolution Implementation Plan

## Overview

Implement agent-driven entity disambiguation with sub-session architecture, integrating the full DSL pipeline from enrichment through resolution to execution.

**Key principle:** Resolution is a scoped agent conversation, not a form. The agent drives disambiguation through natural language in a sub-session window.

---

## Architecture Summary

```
Main Agent Chat
    │
    ▼
Agent generates DSL with unresolved EntityRefs
    │
    ▼ (detected via can_execute: false + unresolved_refs)
    │
Agent: "I found 2 entities to confirm."
       [Open Resolution Assistant →]
    │
    ▼ Push to window_stack (Layer 2)
    │
┌─────────────────────────────────────────────────────────────────┐
│  Resolution Sub-Session                                         │
│  - Inherits parent session context (symbols, CBU scope)         │
│  - Scoped agent conversation for disambiguation                 │
│  - Voice-friendly natural language refinement                   │
│  - Updates parent AST on completion                             │
└─────────────────────────────────────────────────────────────────┘
    │
    ▼ Pop from window_stack
    │
Main Agent Chat: "All resolved. Ready to execute?"
```

---

## Phase 1: Sub-Session Infrastructure (Backend)

**Goal:** Enable parent-child session relationships with context inheritance.

### 1.1 Session Model Changes

**File: `rust/src/api/session.rs`**

```rust
pub struct AgentSession {
    // ... existing fields ...
    
    /// Parent session ID (None for root sessions)
    pub parent_session_id: Option<Uuid>,
    
    /// Sub-session type (determines behavior)
    pub session_type: SessionType,
    
    /// Inherited symbols from parent (pre-populated on creation)
    pub inherited_symbols: HashMap<String, BoundValue>,
}

pub enum SessionType {
    /// Root session - full agent capabilities
    Root,
    /// Resolution sub-session - scoped to entity disambiguation
    Resolution { 
        unresolved_refs: Vec<UnresolvedRef>,
        parent_dsl_index: usize,  // Which DSL statement triggered this
    },
    /// Research sub-session - GLEIF/UBO discovery
    Research { 
        target_entity: Option<Uuid>,
    },
    /// Review sub-session - DSL review before execute
    Review {
        pending_dsl: String,
    },
}
```

### 1.2 Sub-Session Creation Endpoint

**File: `rust/src/api/agent_routes.rs`**

```rust
/// POST /api/session/:id/subsession
/// Creates a child session inheriting parent context
#[derive(Deserialize)]
pub struct CreateSubSessionRequest {
    pub session_type: SessionType,
}

#[derive(Serialize)]
pub struct CreateSubSessionResponse {
    pub session_id: Uuid,
    pub parent_id: Uuid,
    pub inherited_symbols: Vec<String>,  // Names only, for UI display
}

async fn create_subsession(
    State(state): State<AppState>,
    Path(parent_id): Path<Uuid>,
    Json(req): Json<CreateSubSessionRequest>,
) -> Result<Json<CreateSubSessionResponse>, StatusCode> {
    let parent = state.session_store.get(&parent_id)?;
    
    // Inherit symbols from parent's validation context
    let inherited = parent.context.known_symbols.clone();
    
    let child = AgentSession {
        id: Uuid::new_v4(),
        parent_session_id: Some(parent_id),
        session_type: req.session_type,
        inherited_symbols: inherited.clone(),
        state: SessionState::New,
        // ... other fields initialized ...
    };
    
    state.session_store.insert(child.id, child.clone());
    
    Ok(Json(CreateSubSessionResponse {
        session_id: child.id,
        parent_id,
        inherited_symbols: inherited.keys().cloned().collect(),
    }))
}
```

### 1.3 Symbol Inheritance in Validation

**File: `rust/src/dsl_v2/semantic_validator.rs`**

```rust
impl SemanticValidator {
    /// Pre-populate symbols from parent session context
    pub fn with_inherited_symbols(mut self, symbols: &HashMap<String, BoundValue>) -> Self {
        for (name, value) in symbols {
            self.validation_context.known_symbols.insert(name.clone(), value.clone());
        }
        self
    }
}
```

### 1.4 Sub-Session Merge on Completion

**File: `rust/src/api/agent_routes.rs`**

```rust
/// POST /api/session/:id/subsession/:child_id/complete
/// Merges child results back to parent and closes child
#[derive(Deserialize)]
pub struct CompleteSubSessionRequest {
    /// Resolved EntityRefs to apply to parent AST
    pub resolutions: HashMap<RefId, String>,  // ref_id → resolved_key
}

async fn complete_subsession(
    State(state): State<AppState>,
    Path((parent_id, child_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<CompleteSubSessionRequest>,
) -> Result<Json<()>, StatusCode> {
    let child = state.session_store.remove(&child_id)?;
    let mut parent = state.session_store.get_mut(&parent_id)?;
    
    // Apply resolutions to parent's pending AST
    for (ref_id, resolved_key) in req.resolutions {
        parent.apply_resolution(ref_id, resolved_key)?;
    }
    
    // Re-validate parent DSL with resolutions applied
    parent.revalidate()?;
    
    Ok(Json(()))
}
```

---

## Phase 2: Resolution Agent Tools (MCP)

**Goal:** Give the agent tools to drive resolution conversation.

### 2.1 New MCP Tools

**File: `rust/src/api/agent_routes.rs`**

Add to `generate_commands_help()` and implement handlers:

```rust
// Resolution workflow tools
resolution_start     - Start resolution for current DSL's unresolved refs
resolution_search    - Search for entity matches with discriminators
resolution_select    - Select a specific match for a ref
resolution_status    - Get current resolution progress
resolution_complete  - Commit resolutions and close sub-session

// Enhanced entity search with disambiguation
entity_disambiguate  - Present disambiguation options to user
```

### 2.2 Resolution Start Tool

**File: `rust/src/api/mcp_tools/resolution.rs`** (new)

```rust
/// MCP tool: resolution_start
/// Called by agent when DSL has unresolved refs
pub async fn resolution_start(
    session: &mut AgentSession,
    pool: &PgPool,
) -> Result<ResolutionStartResponse> {
    // Extract unresolved refs from session's pending AST
    let unresolved = session.extract_unresolved_refs()?;
    
    if unresolved.is_empty() {
        return Ok(ResolutionStartResponse {
            needed: false,
            message: "All entity references are resolved.".into(),
            refs: vec![],
        });
    }
    
    // For each ref, do initial fuzzy search
    let mut refs_with_matches = Vec::new();
    for uref in &unresolved {
        let matches = entity_gateway::search_fuzzy(
            &uref.entity_type,
            &uref.value,
            10,
        ).await?;
        
        refs_with_matches.push(UnresolvedRefWithMatches {
            ref_id: uref.ref_id.clone(),
            entity_type: uref.entity_type.clone(),
            search_value: uref.value.clone(),
            context_line: uref.context_line.clone(),
            matches,
        });
    }
    
    Ok(ResolutionStartResponse {
        needed: true,
        message: format!("Found {} entities to resolve.", unresolved.len()),
        refs: refs_with_matches,
    })
}
```

### 2.3 Agent Resolution Prompt

**File: `rust/config/prompts/resolution/disambiguate.md`** (new)

```markdown
You are helping the user resolve ambiguous entity references in their DSL.

Current unresolved reference:
- Entity type: {{entity_type}}
- Search value: "{{search_value}}"
- Context: {{context_line}}

Matches found:
{{#each matches}}
{{@index}}. {{display}} ({{score}}%)
   {{#if detail}}{{detail}}{{/if}}
{{/each}}

Ask the user which one they mean. Accept:
- Number selection: "1", "the first one"
- Refinement: "the UK one", "born in 1965"
- Voice-friendly: natural language descriptions

If the user provides refinement, search again with discriminators.
If the user selects, confirm and move to next unresolved ref.
If no matches, offer to create new entity.
```

### 2.4 Resolution Search with Discriminators

```rust
/// MCP tool: resolution_search
pub async fn resolution_search(
    ref_id: &RefId,
    query: &str,
    discriminators: &Discriminators,
) -> Result<Vec<EntityMatch>> {
    let mut request = SearchRequest {
        nickname: normalize_entity_type(&ref_id.entity_type),
        values: vec![query.to_string()],
        mode: SearchMode::Fuzzy as i32,
        limit: Some(10),
        discriminators: HashMap::new(),
    };
    
    // Apply discriminators from natural language parsing
    if let Some(nat) = &discriminators.nationality {
        request.discriminators.insert("nationality".into(), nat.clone());
    }
    if let Some(dob) = &discriminators.date_of_birth {
        request.discriminators.insert("date_of_birth".into(), dob.clone());
    }
    if let Some(jur) = &discriminators.jurisdiction {
        request.discriminators.insert("jurisdiction".into(), jur.clone());
    }
    
    let response = entity_gateway_client.search(request).await?;
    Ok(response.matches.into_iter().map(|m| m.into()).collect())
}
```

---

## Phase 3: Natural Language Discriminator Parsing

**Goal:** Parse "UK citizen, born 1965" into structured discriminators.

### 3.1 Discriminator Parser

**File: `rust/src/api/entity_routes.rs`**

```rust
/// POST /api/entity/parse-refinement
#[derive(Deserialize)]
pub struct ParseRefinementRequest {
    pub text: String,
    pub entity_type: String,
}

#[derive(Serialize)]
pub struct ParseRefinementResponse {
    pub discriminators: Discriminators,
    pub interpretation: String,
}

async fn parse_refinement(
    Json(req): Json<ParseRefinementRequest>,
) -> Result<Json<ParseRefinementResponse>, StatusCode> {
    let text = req.text.to_lowercase();
    let mut disc = Discriminators::default();
    let mut interp = Vec::new();
    
    // Nationality patterns
    if let Some(nat) = extract_nationality(&text) {
        disc.nationality = Some(nat.clone());
        interp.push(format!("nationality={}", nat));
    }
    
    // Birth year patterns: "born 1965", "born in 1965", "1965"
    if let Some(year) = extract_birth_year(&text) {
        disc.date_of_birth = Some(year.clone());
        interp.push(format!("dob={}", year));
    }
    
    // Jurisdiction patterns: "UK", "in Luxembourg", "from Germany"
    if let Some(jur) = extract_jurisdiction(&text) {
        disc.jurisdiction = Some(jur.clone());
        interp.push(format!("jurisdiction={}", jur));
    }
    
    // Role/company hints (fuzzy, not exact)
    if let Some(role) = extract_role_hint(&text) {
        disc.role_hint = Some(role.clone());
        interp.push(format!("role contains '{}'", role));
    }
    if let Some(company) = extract_company_hint(&text) {
        disc.company_hint = Some(company.clone());
        interp.push(format!("company contains '{}'", company));
    }
    
    Ok(Json(ParseRefinementResponse {
        discriminators: disc,
        interpretation: interp.join(", "),
    }))
}

fn extract_nationality(text: &str) -> Option<String> {
    // Pattern: "UK citizen", "British", "German national"
    let patterns = [
        (r"(?i)\b(uk|british|gb)\b", "GB"),
        (r"(?i)\b(us|american|usa)\b", "US"),
        (r"(?i)\b(german|germany|de)\b", "DE"),
        (r"(?i)\b(french|france|fr)\b", "FR"),
        (r"(?i)\b(luxembourg|lu)\b", "LU"),
        // ... more patterns
    ];
    for (pattern, code) in patterns {
        if regex::Regex::new(pattern).unwrap().is_match(text) {
            return Some(code.to_string());
        }
    }
    None
}

fn extract_birth_year(text: &str) -> Option<String> {
    // Pattern: "born 1965", "born in 1965", "dob 1965", just "1965" if 4 digits 19xx/20xx
    let re = regex::Regex::new(r"\b(19|20)\d{2}\b").unwrap();
    re.find(text).map(|m| m.as_str().to_string())
}
```

---

## Phase 4: UI Window Stack Integration

**Goal:** Implement sub-session windows with agent chat.

### 4.1 Window Stack State

**File: `rust/crates/ob-poc-ui/src/state.rs`**

```rust
pub struct AppState {
    // ... existing fields ...
    
    /// Window stack for modals and sub-sessions
    pub window_stack: Vec<WindowInstance>,
}

pub struct WindowInstance {
    pub id: WindowId,
    pub layer: u8,
    pub title: String,
    pub state: WindowState,
}

#[derive(Clone)]
pub enum WindowId {
    CbuSearch,
    EntityRefPopup { ref_index: usize },
    ResolutionSession { session_id: Uuid },
    ResearchSession { session_id: Uuid },
    ConfirmDialog { action: String },
}

pub enum WindowState {
    CbuSearch(CbuSearchState),
    Resolution(ResolutionSessionState),
    Research(ResearchSessionState),
    Confirm(ConfirmDialogState),
}

pub struct ResolutionSessionState {
    pub session_id: Uuid,
    pub parent_session_id: Uuid,
    pub messages: Vec<ChatMessage>,
    pub input_buffer: String,
    pub current_ref: Option<UnresolvedRefWithMatches>,
    pub pending_refs: Vec<UnresolvedRefWithMatches>,
    pub resolutions: HashMap<RefId, String>,
    pub voice_active: bool,
}
```

### 4.2 Window Stack Rendering

**File: `rust/crates/ob-poc-ui/src/app.rs`**

```rust
impl App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        // 1. Process async results
        self.process_async_results();
        
        // 2. Render Layer 0 (main panels)
        self.render_main_panels(ctx);
        
        // 3. Render window stack in order
        let mut actions = Vec::new();
        for (i, window) in self.state.window_stack.iter().enumerate() {
            if let Some(action) = self.render_window(ctx, window, i) {
                actions.push((i, action));
            }
        }
        
        // 4. Handle window actions
        for (idx, action) in actions {
            self.handle_window_action(idx, action);
        }
        
        // 5. ESC closes topmost
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.close_topmost_window();
        }
    }
    
    fn render_window(&self, ctx: &egui::Context, window: &WindowInstance, z_idx: usize) 
        -> Option<WindowAction> 
    {
        match &window.state {
            WindowState::Resolution(state) => {
                resolution_session_window(ctx, &window.title, state)
            }
            WindowState::CbuSearch(state) => {
                cbu_search_modal(ctx, state)
            }
            // ... other window types
        }
    }
    
    fn close_topmost_window(&mut self) {
        if let Some(window) = self.state.window_stack.pop() {
            // Cleanup based on window type
            match window.state {
                WindowState::Resolution(state) => {
                    // Cancel sub-session if not completed
                    self.cancel_subsession(state.session_id);
                }
                _ => {}
            }
        }
    }
}
```

### 4.3 Resolution Session Window

**File: `rust/crates/ob-poc-ui/src/panels/resolution_session.rs`** (new)

```rust
pub fn resolution_session_window(
    ctx: &egui::Context,
    title: &str,
    state: &mut ResolutionSessionState,
) -> Option<ResolutionAction> {
    let mut action = None;
    
    egui::Window::new(title)
        .collapsible(false)
        .resizable(true)
        .default_size(egui::Vec2::new(500.0, 600.0))
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            // Header with progress
            ui.horizontal(|ui| {
                if ui.button("← Back").clicked() {
                    action = Some(ResolutionAction::Cancel);
                }
                ui.separator();
                let resolved = state.resolutions.len();
                let total = resolved + state.pending_refs.len() + 
                    if state.current_ref.is_some() { 1 } else { 0 };
                ui.label(format!("Resolving {} of {} entities", resolved + 1, total));
            });
            
            ui.separator();
            
            // DSL context (current ref)
            if let Some(ref cref) = state.current_ref {
                ui.group(|ui| {
                    ui.label(egui::RichText::new("Context:").strong());
                    ui.monospace(&cref.context_line);
                });
            }
            
            ui.separator();
            
            // Chat messages
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    for msg in &state.messages {
                        render_chat_message(ui, msg);
                    }
                });
            
            ui.separator();
            
            // Input area
            ui.horizontal(|ui| {
                let response = ui.text_edit_singleline(&mut state.input_buffer);
                
                if ui.button("Send").clicked() || 
                   (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) 
                {
                    if !state.input_buffer.is_empty() {
                        action = Some(ResolutionAction::SendMessage {
                            message: state.input_buffer.clone(),
                        });
                        state.input_buffer.clear();
                    }
                }
                
                // Voice button
                let voice_icon = if state.voice_active { "🎤" } else { "🎙️" };
                if ui.button(voice_icon).clicked() {
                    action = Some(ResolutionAction::ToggleVoice);
                }
            });
        });
    
    action
}

pub enum ResolutionAction {
    SendMessage { message: String },
    ToggleVoice,
    Cancel,
    Complete,
}
```

---

## Phase 5: Agent Chat Integration

**Goal:** Wire sub-session chat to agent backend.

### 5.1 Sub-Session Chat Endpoint

**File: `rust/src/api/agent_routes.rs`**

```rust
/// POST /api/session/:parent_id/subsession/:child_id/chat
/// Chat in a sub-session context
async fn subsession_chat(
    State(state): State<AppState>,
    Path((parent_id, child_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, StatusCode> {
    let child = state.session_store.get_mut(&child_id)?;
    
    // Verify parent relationship
    if child.parent_session_id != Some(parent_id) {
        return Err(StatusCode::BAD_REQUEST);
    }
    
    // Process based on session type
    match &child.session_type {
        SessionType::Resolution { unresolved_refs, .. } => {
            process_resolution_chat(&mut child, &req, &state.pool).await
        }
        SessionType::Research { .. } => {
            process_research_chat(&mut child, &req, &state.pool).await
        }
        _ => Err(StatusCode::BAD_REQUEST),
    }
}

async fn process_resolution_chat(
    session: &mut AgentSession,
    req: &ChatRequest,
    pool: &PgPool,
) -> Result<Json<ChatResponse>, StatusCode> {
    let message = req.message.trim();
    
    // Check for selection patterns: "1", "the first one", "select 1"
    if let Some(selection) = parse_selection(message) {
        return handle_resolution_selection(session, selection).await;
    }
    
    // Check for refinement: "UK citizen", "born 1965"
    let discriminators = parse_refinement(message).await?;
    if discriminators.has_any() {
        return handle_resolution_refinement(session, &discriminators).await;
    }
    
    // Otherwise, use LLM to interpret
    let llm_response = agent_service::process_resolution_message(session, message).await?;
    
    Ok(Json(ChatResponse {
        message: llm_response.message,
        dsl: None,
        session_state: session.state.clone().into(),
        commands: llm_response.commands,
    }))
}
```

### 5.2 Resolution Selection Handler

```rust
async fn handle_resolution_selection(
    session: &mut AgentSession,
    selection: usize,
) -> Result<Json<ChatResponse>, StatusCode> {
    let SessionType::Resolution { ref mut unresolved_refs, .. } = session.session_type else {
        return Err(StatusCode::BAD_REQUEST);
    };
    
    // Get current ref being resolved
    let current = session.current_resolution_ref()?;
    
    // Validate selection
    if selection >= current.matches.len() {
        return Ok(Json(ChatResponse {
            message: format!("Please select 1-{}", current.matches.len()),
            ..Default::default()
        }));
    }
    
    let selected = &current.matches[selection];
    
    // Record resolution
    session.resolutions.insert(
        current.ref_id.clone(),
        selected.value.clone(),
    );
    
    // Move to next ref
    let message = if let Some(next) = session.advance_to_next_ref() {
        format!(
            "Selected: {} ({}%)\n\nNext: Which {}?\n{}",
            selected.display,
            (selected.score * 100.0) as u32,
            next.entity_type,
            format_matches(&next.matches),
        )
    } else {
        // All done
        session.state = SessionState::Complete;
        "All entities resolved. Ready to apply?".into()
    };
    
    Ok(Json(ChatResponse {
        message,
        session_state: session.state.clone().into(),
        commands: if session.state == SessionState::Complete {
            Some(vec![Command::ApplyResolutions])
        } else {
            None
        },
        ..Default::default()
    }))
}
```

---

## Phase 6: Inline Popup (Zed-style)

**Goal:** Quick resolution for single refs without full sub-session.

### 6.1 Inline Popup Component

**File: `rust/crates/ob-poc-ui/src/panels/entity_ref_popup.rs`** (new)

```rust
pub struct EntityRefPopupState {
    pub ref_index: usize,
    pub entity_type: String,
    pub search_value: String,
    pub query: String,
    pub results: Option<Vec<EntityMatch>>,
    pub searching: bool,
    pub selected_index: Option<usize>,
}

pub fn entity_ref_popup(
    ctx: &egui::Context,
    anchor_pos: egui::Pos2,
    state: &mut EntityRefPopupState,
) -> Option<EntityRefAction> {
    let mut action = None;
    
    egui::Area::new(egui::Id::new("entity_ref_popup"))
        .fixed_pos(anchor_pos)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .show(ui, |ui| {
                    ui.set_min_width(300.0);
                    ui.set_max_height(250.0);
                    
                    // Search input
                    let response = ui.text_edit_singleline(&mut state.query);
                    if response.changed() && state.query.len() >= 2 {
                        action = Some(EntityRefAction::Search {
                            query: state.query.clone(),
                        });
                    }
                    
                    // Handle keyboard navigation
                    if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                        state.selected_index = Some(
                            state.selected_index.map(|i| i + 1).unwrap_or(0)
                        );
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                        state.selected_index = state.selected_index.and_then(|i| i.checked_sub(1));
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        if let Some(idx) = state.selected_index {
                            if let Some(results) = &state.results {
                                if let Some(m) = results.get(idx) {
                                    action = Some(EntityRefAction::Select {
                                        ref_index: state.ref_index,
                                        resolved_key: m.value.clone(),
                                    });
                                }
                            }
                        }
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        action = Some(EntityRefAction::Close);
                    }
                    
                    ui.separator();
                    
                    // Results
                    if state.searching {
                        ui.spinner();
                    } else if let Some(results) = &state.results {
                        for (i, m) in results.iter().take(6).enumerate() {
                            let selected = state.selected_index == Some(i);
                            let response = ui.selectable_label(
                                selected,
                                format!("{} ({}%)", m.display, (m.score * 100.0) as u32),
                            );
                            if response.clicked() {
                                action = Some(EntityRefAction::Select {
                                    ref_index: state.ref_index,
                                    resolved_key: m.value.clone(),
                                });
                            }
                        }
                    }
                    
                    ui.separator();
                    
                    if ui.small_button("+ Create new...").clicked() {
                        action = Some(EntityRefAction::CreateNew {
                            entity_type: state.entity_type.clone(),
                            initial_name: state.search_value.clone(),
                        });
                    }
                });
        });
    
    action
}

pub enum EntityRefAction {
    Search { query: String },
    Select { ref_index: usize, resolved_key: String },
    CreateNew { entity_type: String, initial_name: String },
    Close,
}
```

### 6.2 DSL Editor Integration

**File: `rust/crates/ob-poc-ui/src/panels/dsl_editor.rs`**

```rust
// In DSL editor rendering, detect unresolved refs and show squiggly
fn render_dsl_with_diagnostics(ui: &mut Ui, dsl: &str, diagnostics: &[Diagnostic]) {
    // ... existing text rendering ...
    
    // For each unresolved ref diagnostic, add clickable region
    for diag in diagnostics.iter().filter(|d| d.code == "unresolved-ref") {
        let span = diag.span;
        let rect = text_rect_for_span(span);
        
        // Draw squiggly underline
        draw_squiggly(ui, rect, Color32::YELLOW);
        
        // Make clickable
        let response = ui.interact(rect, ui.id().with(span), Sense::click());
        if response.clicked() {
            // Open inline popup at this position
            return Some(DslEditorAction::OpenRefPopup {
                ref_index: diag.ref_index,
                anchor: rect.left_bottom(),
            });
        }
        
        // Ctrl+. shortcut when cursor is on squiggly
        if response.hovered() && ui.input(|i| i.modifiers.ctrl && i.key_pressed(Key::Period)) {
            return Some(DslEditorAction::OpenRefPopup {
                ref_index: diag.ref_index,
                anchor: rect.left_bottom(),
            });
        }
    }
}
```

---

## Phase 7: Voice Integration

**Goal:** Voice refinement in resolution sub-session.

### 7.1 Voice Command Routing

**File: `rust/crates/ob-poc-ui/src/voice_bridge.rs`**

```rust
pub fn dispatch_voice_command(
    command: &VoiceCommand,
    app_state: &mut AppState,
) -> Option<VoiceAction> {
    // Check if resolution sub-session is active
    if let Some(window) = app_state.window_stack.last() {
        if let WindowState::Resolution(ref state) = window.state {
            // Route to resolution session
            return dispatch_resolution_voice(command, state);
        }
    }
    
    // Otherwise route to main agent
    dispatch_main_voice(command, app_state)
}

fn dispatch_resolution_voice(
    command: &VoiceCommand,
    state: &ResolutionSessionState,
) -> Option<VoiceAction> {
    let transcript = &command.transcript.to_lowercase();
    
    // Selection patterns
    if let Some(num) = parse_ordinal(transcript) {
        // "first", "second", "one", "1"
        return Some(VoiceAction::ResolutionSelect { index: num - 1 });
    }
    
    // Confirmation patterns
    if matches_pattern(transcript, &["yes", "correct", "that's right", "confirm"]) {
        return Some(VoiceAction::ResolutionConfirm);
    }
    
    // Rejection patterns
    if matches_pattern(transcript, &["no", "wrong", "not that", "different"]) {
        return Some(VoiceAction::ResolutionReject);
    }
    
    // Otherwise treat as refinement
    Some(VoiceAction::ResolutionRefine {
        text: command.transcript.clone(),
    })
}
```

---

## Implementation Order

```
Week 1: Foundation
├── 1.1 Session model changes (parent_id, session_type)
├── 1.2 Sub-session creation endpoint
├── 1.3 Symbol inheritance in validator
└── 1.4 Sub-session merge/complete endpoint

Week 2: Agent Tools
├── 2.1 MCP tool definitions (resolution_*)
├── 2.2 resolution_start implementation
├── 2.3 Resolution prompt template
└── 2.4 resolution_search with discriminators

Week 3: Discriminator Parsing
├── 3.1 parse_refinement endpoint
├── 3.2 Nationality/DOB/jurisdiction extractors
├── 3.3 Role/company hint extractors
└── 3.4 Integration with resolution_search

Week 4: UI Window Stack
├── 4.1 WindowInstance types in state.rs
├── 4.2 Window stack rendering in app.rs
├── 4.3 Resolution session window component
└── 4.4 ESC/Back handling

Week 5: Agent Chat Integration
├── 5.1 Sub-session chat endpoint
├── 5.2 Resolution selection handler
├── 5.3 Refinement handler
└── 5.4 Completion/merge flow

Week 6: Inline Popup
├── 6.1 EntityRefPopup component
├── 6.2 DSL editor squiggly rendering
├── 6.3 Ctrl+. trigger
└── 6.4 Keyboard navigation

Week 7: Voice & Polish
├── 7.1 Voice command routing to sub-session
├── 7.2 Ordinal/confirmation parsing
├── 7.3 Auto-resolve on high confidence
└── 7.4 Edge cases and testing
```

---

## Files to Create

| File | Purpose |
|------|---------|
| `rust/src/api/mcp_tools/resolution.rs` | Resolution MCP tool implementations |
| `rust/src/api/mcp_tools/mod.rs` | MCP tools module |
| `rust/config/prompts/resolution/disambiguate.md` | Agent prompt for disambiguation |
| `rust/crates/ob-poc-ui/src/panels/resolution_session.rs` | Resolution sub-session window |
| `rust/crates/ob-poc-ui/src/panels/entity_ref_popup.rs` | Inline popup component |
| `rust/crates/ob-poc-ui/src/window_stack.rs` | Window stack management |

## Files to Modify

| File | Changes |
|------|---------|
| `rust/src/api/session.rs` | Add parent_id, session_type, inherited_symbols |
| `rust/src/api/agent_routes.rs` | Add subsession endpoints, update generate_commands_help |
| `rust/src/dsl_v2/semantic_validator.rs` | Add with_inherited_symbols() |
| `rust/src/api/entity_routes.rs` | Add parse-refinement endpoint |
| `rust/crates/ob-poc-ui/src/state.rs` | Add window_stack, WindowInstance types |
| `rust/crates/ob-poc-ui/src/app.rs` | Add window stack rendering loop |
| `rust/crates/ob-poc-ui/src/panels/dsl_editor.rs` | Add squiggly rendering, Ctrl+. trigger |
| `rust/crates/ob-poc-ui/src/voice_bridge.rs` | Add resolution voice routing |

---

## Success Criteria

1. [ ] Sub-session inherits parent symbols correctly
2. [ ] Resolution agent conversation resolves "John Smith" with voice refinement
3. [ ] Inline popup works with Ctrl+. and keyboard navigation
4. [ ] ESC closes topmost window consistently
5. [ ] Resolved refs update parent AST correctly
6. [ ] Voice "the first one" selects correctly in sub-session
7. [ ] "UK citizen, born 1965" parses to discriminators
8. [ ] All clippy clean, tests pass

---

## References

- Design doc: `ai-thoughts/025-entity-disambiguation-ux.md`
- Window stack rules: `docs/strategy-patterns.md` §3
- DSL pipeline: `docs/dsl-verb-flow.md`
- Agent architecture: `docs/agent-architecture.md`
- Existing modal: `rust/crates/ob-poc-ui/src/panels/cbu_search.rs`
