# EGUI MANDATORY RULES - READ BEFORE ANY UI CODE

## ⛔ STOP

**Do NOT write any egui/WASM UI code until you have read and understood these rules.**

**Violations cause: frozen UI, state drift, impossible-to-debug bugs, 60fps rendering disasters.**

---

## The 5 Non-Negotiable Rules

### Rule 1: NO LOCAL STATE MIRRORING SERVER DATA

```rust
// ❌ FORBIDDEN - will cause state drift
struct MyPanel {
    messages: Vec<Message>,           // NO - this is server data
    entities: HashMap<Uuid, Entity>,  // NO - this is server data
    is_dirty: bool,                   // NO - fighting the model
    cached_results: Vec<SearchResult>,// NO - server caches, not UI
}

// ✅ REQUIRED - UI state is ONLY navigation/buffers
struct MyPanel {
    search_buffer: String,            // YES - text user is typing
    selected_idx: Option<usize>,      // YES - UI navigation
    expanded_section: Option<String>, // YES - UI state
}
```

**Server data lives in `AppState.session`, `AppState.resolution`, etc. - fetched from server, NEVER mutated locally.**

---

### Rule 2: ACTIONS RETURN VALUES, NO CALLBACKS

```rust
// ❌ FORBIDDEN - borrow checker hell, spaghetti logic
if ui.button("Save").clicked() {
    self.save_data();  // Mutating self inside UI code
    self.refresh();    // More mutation
}

// ✅ REQUIRED - pure function returns what happened
fn my_panel(ui: &mut egui::Ui, data: &Data) -> Option<Action> {
    if ui.button("Save").clicked() {
        return Some(Action::Save);
    }
    if ui.button("Delete").clicked() {
        return Some(Action::Delete { id: data.id.clone() });
    }
    None
}

// In update(), AFTER rendering:
if let Some(action) = my_panel(ui, &data) {
    match action {
        Action::Save => self.post_save(),
        Action::Delete { id } => self.post_delete(&id),
    }
}
```

---

### Rule 3: SHORT LOCK, THEN RENDER

```rust
// ❌ FORBIDDEN - lock held during entire render
fn update(&mut self, ctx: &egui::Context) {
    let state = self.async_state.lock().unwrap();  // Lock acquired
    
    egui::CentralPanel::default().show(ctx, |ui| {
        for item in &state.items {  // Lock STILL held
            ui.label(&item.name);   // Lock STILL held - BAD
        }
    });
}  // Lock finally released - TOO LATE

// ✅ REQUIRED - extract, release, then render
fn update(&mut self, ctx: &egui::Context) {
    // 1. Short lock - extract what we need
    let items = {
        let state = self.async_state.lock().unwrap();
        state.items.clone()  // Clone and release
    };  // Lock released HERE
    
    // 2. Render with extracted data - NO lock held
    egui::CentralPanel::default().show(ctx, |ui| {
        for item in &items {
            ui.label(&item.name);
        }
    });
}
```

---

### Rule 4: PROCESS ASYNC FIRST, RENDER SECOND

```rust
// ❌ FORBIDDEN - async results ignored or processed mid-render
fn update(&mut self, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        // Rendering...
        self.check_async();  // NO - don't do this mid-render
    });
}

// ✅ REQUIRED - process async at START of update()
fn update(&mut self, ctx: &egui::Context) {
    // 1. FIRST: Process any pending async results
    self.process_async_results();
    
    // 2. SECOND: Extract state
    let data = self.extract_render_data();
    
    // 3. THIRD: Render
    egui::CentralPanel::default().show(ctx, |ui| {
        // Pure rendering using extracted data
    });
    
    // 4. FOURTH: Handle actions from render
    // (dispatch new async operations)
}
```

---

### Rule 5: SERVER ROUND-TRIP FOR ALL MUTATIONS

```rust
// ❌ FORBIDDEN - optimistic local update
fn handle_action(&mut self, action: Action) {
    match action {
        Action::AddItem(item) => {
            self.items.push(item);  // NO - local mutation
            self.post_to_server();  // Then sync? State drift!
        }
    }
}

// ✅ REQUIRED - server is source of truth
fn handle_action(&mut self, action: Action) {
    match action {
        Action::AddItem(item) => {
            // 1. Set loading state
            self.set_loading(true);
            
            // 2. POST to server
            self.post_add_item(&item);
            
            // 3. Server responds → process_async_results() updates state
            // 4. Next frame renders new state from server
        }
    }
}
```

---

## The Mental Model

```
┌─────────────────────────────────────────────────────────────────┐
│                    egui runs at 60fps                           │
│                                                                 │
│   Every frame:                                                  │
│   1. process_async_results()     ← Server data arrives here     │
│   2. extract state (short lock)                                 │
│   3. render UI (pure function of state)                         │
│   4. handle returned actions     ← User actions dispatched here │
│                                                                 │
│   State flows: Server → AppState → UI → Actions → Server        │
│                                                                 │
│   UI NEVER modifies server data. UI only:                       │
│   - Reads server data (via AppState)                            │
│   - Returns actions (via return values)                         │
│   - Manages ephemeral UI state (buffers, selection, navigation) │
└─────────────────────────────────────────────────────────────────┘
```

---

## Pre-Coding Checklist

Before writing ANY egui panel code, answer these questions:

1. **What server data does this panel display?**
   - This data comes from `AppState.xxx` (fetched from server)
   - This data is NEVER mutated by the panel

2. **What UI-only state does this panel need?**
   - Text buffers (what user is typing)
   - Selection/navigation state
   - Expanded/collapsed sections
   - NOTHING ELSE

3. **What actions can the user take?**
   - Define an `enum Action { ... }`
   - Panel returns `Option<Action>`
   - Parent handles actions, dispatches to server

4. **How does data get to this panel?**
   - Extracted from `AppState` at start of `update()`
   - Passed as `&Data` parameter to panel function
   - NOT by locking inside the panel

---

## Quick Reference

| Want to... | Do this | NOT this |
|------------|---------|----------|
| Store server data | `AppState.xxx` | Panel struct field |
| Store user typing | `panel.search_buffer: String` | - |
| Handle button click | `return Some(Action::X)` | `self.do_thing()` |
| Read async result | `process_async_results()` at top of update | Check inside render |
| Update server data | POST → wait → refetch | Mutate locally |
| Access shared state | Extract, drop lock, use | Hold lock during render |

---

## If You're Unsure

**Ask:** "Is this state from the server?"
- Yes → It goes in `AppState`, fetched via async, NEVER mutated by UI
- No → It's UI ephemeral state (buffers, navigation), can live in panel struct

**Ask:** "Am I mutating something when a button is clicked?"
- Yes → STOP. Return an action instead. Handle it outside the panel.
- No → Good.

**Ask:** "Am I holding a lock while rendering?"
- Yes → STOP. Extract what you need, drop the lock, then render.
- No → Good.

---

## Reference

Full details in `CLAUDE.md` → "egui State Management & Best Practices" section.

**These rules are non-negotiable. Violations will cause bugs that are extremely difficult to debug.**
