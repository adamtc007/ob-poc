# TODO: ESPER Viewport Navigation Modal

## Summary

Create a **Viewport Navigation Modal** - a specialized agent chat window (like the entity resolution modal) that handles viewport/visualization commands. When user input contains trigger words like "view", "show", "inspect", "visualize", route to this modal instead of main chat.

The modal executes DSL verbs from the existing `view.*` domain via MCP tools.

---

## Existing DSL Verbs (view.yaml)

These verbs are **already registered** and available via MCP. The modal should execute them:

### ESPER Visual Effects
| Verb | Purpose | Example |
|------|---------|---------|
| `view.xray` | Transparent layers, see through structure | `(view.xray :layers [custody ubo])` |
| `view.peel` | Remove outer layer to reveal inner | `(view.peel :layer custody)` |
| `view.shadow` | Dim non-risk items | `(view.shadow :threshold high)` |
| `view.illuminate` | Highlight specific aspect | `(view.illuminate :aspect risks)` |
| `view.red-flag` | Scan and highlight red flags | `(view.red-flag :category pep)` |
| `view.black-holes` | Show data gaps | `(view.black-holes :type documents)` |

### Navigation
| Verb | Purpose | Example |
|------|---------|---------|
| `view.drill` | Drill into entity (up/down) | `(view.drill :entity-id $e :direction down)` |
| `view.surface` | Surface up from drill | `(view.surface :levels 1)` |
| `view.trace` | Follow money/control/risk threads | `(view.trace :mode money)` |
| `view.zoom-in` | Zoom into taxonomy node | `(view.zoom-in :node-id $uuid)` |
| `view.zoom-out` | Zoom out to parent | `(view.zoom-out)` |
| `view.back-to` | Jump to breadcrumb level | `(view.back-to :depth 0)` |

### Scope & Filtering  
| Verb | Purpose | Example |
|------|---------|---------|
| `view.universe` | View all CBUs with filters | `(view.universe :jurisdiction [LU])` |
| `view.book` | View client's CBU book | `(view.book :client "Apex")` |
| `view.cbu` | Focus single CBU | `(view.cbu :cbu-id $uuid :mode trading)` |
| `view.refine` | Add filter to current view | `(view.refine :include {:jurisdiction "LU"})` |
| `view.select` | Set explicit selection | `(view.select :all true)` |
| `view.clear` | Clear refinements | `(view.clear)` |
| `view.layout` | Change layout strategy | `(view.layout :mode galaxy)` |

### Introspection
| Verb | Purpose | Example |
|------|---------|---------|
| `view.status` | Get current view state | `(view.status)` |
| `view.breadcrumbs` | Get navigation breadcrumbs | `(view.breadcrumbs)` |
| `view.selection-info` | Get selection details | `(view.selection-info)` |

---

## Trigger Words

Open viewport navigation modal when user message contains:

**Strong triggers (always open modal):**
- `view`, `show`, `visualize`, `display`, `inspect`, `examine`, `explore`
- `xray`, `x-ray`, `peel`, `illuminate`, `shadow`, `highlight`
- `trace`, `drill`, `surface`, `zoom`, `focus`
- `red flag`, `black hole`, `gaps`, `risks`

**Pattern triggers (verb + noun):**
- Verbs: `show`, `display`, `view`, `see`, `find`, `trace`
- Nouns: `graph`, `tree`, `chain`, `structure`, `ownership`, `ubo`, `hierarchy`, `subsidiaries`, `risks`, `flags`, `gaps`

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      MAIN CHAT                                   │
│                                                                 │
│  User: "show me the ownership chain for Apex Capital"          │
│        ↓ trigger: "show" + "ownership chain"                   │
│                                                                 │
│  ┌───────────────────────────────────────────────────────────┐ │
│  │         VIEWPORT NAVIGATION MODAL (Layer 2)               │ │
│  │                                                           │ │
│  │  Context: CBU: Apex Capital Fund I                       │ │
│  │  📍 Universe › LU Funds › Apex Capital                   │ │
│  │                                                           │ │
│  │  [👁 X-Ray] [🌑 Shadow] [🚩 Flags] [🕳 Gaps] [🧅 Peel]   │ │
│  │  ─────────────────────────────────────────────────────   │ │
│  │                                                           │ │
│  │  🤖 Tracing ownership chain upward...                    │ │
│  │     ⚙ (view.trace :mode ownership :from-entity $apex)    │ │
│  │                                                           │ │
│  │  🤖 Found 4 levels: Apex Fund → Apex Mgmt → Apex Hold   │ │
│  │     → John Smith (UBO, 45%)                              │ │
│  │                                                           │ │
│  │  ─────────────────────────────────────────────────────   │ │
│  │  [▶] [_Navigate: "highlight the risks"____________] [Send]│ │
│  │                                                           │ │
│  │  [Done] [Continue Exploring]                             │ │
│  └───────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

---

## Implementation Tasks

### Task 1: Add WindowType::ViewportNavigation

**File:** `rust/crates/ob-poc-ui/src/state.rs`

Add to `WindowType` enum:
```rust
ViewportNavigation,
```

Add to `WindowData` enum:
```rust
ViewportNavigation {
    context: ViewportNavigationContext,
    messages: Vec<ViewportChatMessage>,
    active_effects: Vec<String>,
},
```

Add structs:
```rust
#[derive(Clone, Debug, Default)]
pub struct ViewportNavigationContext {
    pub cbu_id: Option<Uuid>,
    pub cbu_name: Option<String>,
    pub entity_id: Option<Uuid>,
    pub entity_name: Option<String>,
    pub view_mode: String,
    pub breadcrumbs: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct ViewportChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub dsl_executed: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Default, Clone)]
pub struct ViewportNavigationUi {
    pub chat_buffer: String,
    pub show_quick_actions: bool,
}
```

Add field to `AppState`:
```rust
pub viewport_nav_ui: ViewportNavigationUi,
```

---

### Task 2: Create Trigger Detection

**File:** `rust/crates/ob-poc-ui/src/navigation/trigger_detection.rs` (NEW)

```rust
/// Check if message should open viewport navigation modal
pub fn should_open_viewport_modal(message: &str) -> bool {
    let msg = message.to_lowercase();
    
    // Strong triggers
    let triggers = [
        "view ", "show ", "visualize", "display ", "inspect", "examine", "explore",
        "xray", "x-ray", "peel", "illuminate", "shadow", "highlight",
        "trace", "drill", "surface", "zoom", "focus",
        "red flag", "black hole", "gaps ", "risks",
        "ownership chain", "ubo chain", "control chain",
    ];
    
    triggers.iter().any(|t| msg.contains(t))
}
```

---

### Task 3: Create Viewport Navigation Panel

**File:** `rust/crates/ob-poc-ui/src/panels/viewport_navigation.rs` (NEW)

Similar structure to `resolution_panel.rs`:
- Header with context (CBU, entity, breadcrumbs)
- ESPER effect toggle buttons (X-Ray, Shadow, Flags, Gaps, Peel)
- Chat message history with DSL shown
- Quick action buttons (Drill Down, Surface Up, Zoom Fit, Clear)
- Chat input with Send button
- Done / Continue buttons

Return `ViewportNavigationAction` enum:
```rust
pub enum ViewportNavigationAction {
    None,
    Close,
    SendMessage { message: String },
    ExecuteDsl { dsl: String },
    ToggleEffect { effect: String },
    QuickAction { action: QuickNavAction },
}
```

---

### Task 4: Integrate in Chat Handler

**File:** `rust/crates/ob-poc-ui/src/app.rs`

In chat submit handler, check trigger:
```rust
if trigger_detection::should_open_viewport_modal(&message) {
    self.open_viewport_navigation_modal(message);
    return;
}
```

`open_viewport_navigation_modal()`:
1. Build `ViewportNavigationContext` from current state
2. Push `WindowEntry` with `WindowType::ViewportNavigation`
3. Add initial user message to modal's message list

---

### Task 5: Process Viewport Navigation Actions

**File:** `rust/crates/ob-poc-ui/src/app.rs`

In main update loop, handle `ViewportNavigationAction`:

- `SendMessage`: Call viewport agent API (new endpoint or use existing chat with viewport context flag)
- `ExecuteDsl`: Execute DSL via existing `dsl_execute` MCP tool
- `ToggleEffect`: Execute appropriate `view.*` verb:
  - "xray" → `(view.xray)`
  - "shadow" → `(view.shadow)`
  - "red-flag" → `(view.red-flag :category all)`
  - "black-holes" → `(view.black-holes :type all)`
  - "peel" → `(view.peel)`
- `QuickAction`: Map to DSL:
  - `DrillDown` → `(view.drill :direction down)`
  - `SurfaceUp` → `(view.surface)`
  - `ZoomFit` → `(view.zoom-out)` or fit-to-screen
  - `ClearEffects` → `(view.clear)`

---

### Task 6: Add Viewport Agent Endpoint (Optional)

**File:** `rust/src/api/viewport.rs` (NEW - optional)

If needed, create `/api/viewport/navigate` endpoint that:
1. Takes natural language query + context
2. Translates to DSL commands using simple pattern matching or LLM
3. Executes DSL
4. Returns result with executed DSL for display

Alternatively, reuse existing `/api/session/:id/chat` with a `viewport_mode: true` flag.

---

### Task 7: Wire EsperRenderState (Visual Effects)

**File:** `rust/crates/ob-poc-ui/src/state.rs`

Add `esper_state: EsperRenderState` to `AppState`.

**File:** `rust/crates/ob-poc-graph/src/graph/render.rs`

Pass `esper_state` to renderer, apply:
- Alpha dimming for xray/shadow modes
- Highlight rings for red-flag/black-holes
- Layer hiding for peel mode

**Note:** `EsperRenderState` already exists in `ob-poc-graph/src/graph/viewport.rs` with:
- `toggle_xray()`, `toggle_peel()`, `toggle_shadow()`, etc.
- `get_node_alpha(is_focused, depth, has_flag, has_gap) -> (alpha, highlight)`

---

## File Summary

| File | Action |
|------|--------|
| `rust/crates/ob-poc-ui/src/state.rs` | Add WindowType, WindowData, UI state, EsperRenderState |
| `rust/crates/ob-poc-ui/src/navigation/trigger_detection.rs` | NEW - trigger word detection |
| `rust/crates/ob-poc-ui/src/panels/viewport_navigation.rs` | NEW - modal panel UI |
| `rust/crates/ob-poc-ui/src/panels/mod.rs` | Export viewport_navigation |
| `rust/crates/ob-poc-ui/src/app.rs` | Integrate trigger check, modal handling |
| `rust/crates/ob-poc-graph/src/graph/render.rs` | Apply EsperRenderState effects |
| `rust/src/api/viewport.rs` | OPTIONAL - viewport agent endpoint |

---

## Acceptance Criteria

1. ✅ Typing "show ownership chain" opens viewport modal (not main chat)
2. ✅ Modal shows current context (CBU, entity, breadcrumbs)
3. ✅ ESPER effect buttons toggle visual modes
4. ✅ Chat messages show executed DSL commands
5. ✅ Quick actions execute appropriate view.* verbs
6. ✅ "Done" closes modal and returns to main chat
7. ✅ Visual effects render in graph (xray dims, flags highlight, etc.)

---

## Reference Files

- DSL verbs: `rust/config/verbs/view.yaml`
- EsperRenderState: `rust/crates/ob-poc-graph/src/graph/viewport.rs`
- Resolution modal (pattern): `rust/crates/ob-poc-ui/src/panels/resolution_panel.rs`
- Window stack: `rust/crates/ob-poc-ui/src/state.rs` (WindowStack, WindowEntry)
