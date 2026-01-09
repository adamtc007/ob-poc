# egui UI Development

Before writing any egui/WASM UI code, review the 5 non-negotiable rules in CLAUDE.md → "egui State Management & Best Practices".

## The 5 Rules

1. **NO local state mirroring server data** - use AppState.session (fetched), not panel.messages
2. **Actions return values, no callbacks** - return Some(Action::Save), don't self.save_data()
3. **Short lock, then render** - extract data, drop lock, then render
4. **Process async first, render second** - process_async_results() at top of update()
5. **Server round-trip for mutations** - POST → wait → refetch, not optimistic updates

## Animation Rule (Critical for Galaxy Navigation)

```rust
// WRONG - mutating in ui()
fn ui(&mut self, ui: &mut Ui) {
    self.spring.tick(dt);  // NO - don't mutate in render
    self.camera.fly_to(target);  // NO - don't trigger transitions here
}

// RIGHT - update before ui, read in ui
fn update(&mut self, dt: f32) {
    self.navigation_service.tick(dt);  // Physics here
}

fn ui(&self, ui: &mut Ui) {
    let pos = self.navigation_service.camera_pos();  // Read only
    painter.circle(pos, radius, color);  // Render only
}
```

## Widget Pattern

```rust
// WRONG - state in widget, callback
impl MyWidget {
    fn ui(&mut self, ui: &mut Ui) {
        if ui.button("Save").clicked() {
            self.save_data();  // NO callback
        }
    }
}

// RIGHT - stateless widget, returns action
impl MyWidget {
    fn ui(&self, ui: &mut Ui, data: &Data) -> Option<MyAction> {
        if ui.button("Save").clicked() {
            return Some(MyAction::Save);  // Caller handles
        }
        None
    }
}
```

## Files to Understand
- rust/crates/ob-poc-ui/src/app.rs - Main app struct and update loop
- rust/crates/ob-poc-ui/src/state.rs - AppState, AsyncState, TextBuffers
- rust/crates/ob-poc-ui/src/panels/ - Panel implementations
- rust/crates/ob-poc-graph/src/graph/animation.rs - Spring physics

## Common Traps

| Trap | Why It Hurts | Fix |
|------|--------------|-----|
| State in widget | Disappears each frame | Put in AppState/Service |
| Async in ui() | Blocks render | Trigger externally, read Option<T> |
| Callbacks | Can't test, hard to trace | Return actions |
| Layout queries | Don't know size until drawn | Use previous frame or fixed |
| Retained mode thinking | "Widget remembers" | It doesn't. Pass state in. |
| Hardcoded enum in struct Default | Ignores enum's `#[default]` | Use `EnumType::default()` |

## Default Value Rule

When an enum has `#[default]` attribute, **never hardcode a variant** in a struct's `Default` impl:

```rust
// WRONG - hardcoded value ignores enum's #[default]
impl Default for PanelState {
    fn default() -> Self {
        Self {
            layout: LayoutMode::FourPanel,  // BUG: ignores LayoutMode's #[default]
        }
    }
}

// RIGHT - respect the enum's default
impl Default for PanelState {
    fn default() -> Self {
        Self {
            layout: LayoutMode::default(),  // Uses enum's #[default] attribute
        }
    }
}

// BEST - derive Default if all fields have Default
#[derive(Default)]
pub struct PanelState {
    pub layout: LayoutMode,  // Automatically uses LayoutMode::default()
}
```

This prevents the bug where changing an enum's `#[default]` has no effect because struct impls override it.

Read CLAUDE.md section "egui State Management & Best Practices" for full details.
