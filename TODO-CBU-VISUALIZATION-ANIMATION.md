# TODO: CBU Visualization - Interactive Animation System

## ⛔ MANDATORY FIRST STEP

**Read these files:**
- `/EGUI-RULES.md` - Non-negotiable UI patterns (especially Rule 3: short lock, then render)
- `/TODO-CBU-VISUALIZATION-ENHANCEMENT.md` - Visual enhancement plan (do Phase 1 types first)
- `/docs/CBU_UNIVERSE_VISUALIZATION_SPEC.md` - Force simulation concepts

---

## Context

This is **incremental work**. We're building toward a game-engine-quality visualization:
- 60fps smooth animations
- Fly-over, zoom, drill-down navigation
- Expand/collapse composite structures
- Walk up/down taxonomies
- Prune/add nodes dynamically

**Think:** Google Earth for CBU data. Smooth flight between views, semantic zoom levels, drill into structures.

---

## Vision

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│    [ZOOMED OUT: Universe View]                                              │
│                                                                             │
│         ○ ○    ○      ← CBUs as distant stars                               │
│      ○     ○ ○   ○                                                          │
│        ○  ●  ○        ← Selected CBU pulses                                 │
│                                                                             │
│    ══════════════════════════════════════════════════════════════════════  │
│    │  ZOOM IN (smooth 500ms flight)                                     │  │
│    ▼                                                                        │
│                                                                             │
│    [ZOOMED IN: CBU Solar System]                                            │
│                                                                             │
│              ┌─────┐                                                        │
│              │ UBO │ ← Natural persons orbit outer ring                     │
│              └──┬──┘                                                        │
│         ┌──────┴──────┐                                                     │
│         │   ManCo    │ ← Intermediaries middle ring                        │
│         └──────┬──────┘                                                     │
│         ┌──────┴──────┐                                                     │
│         │    CBU     │ ← CBU at center (sun)                               │
│         └──────┬──────┘                                                     │
│    ┌─────┬─────┼─────┬─────┐                                               │
│    │Prod1│Prod2│Prod3│Prod4│ ← Products inner ring                         │
│    └──┬──┴──┬──┴──┬──┴──┬──┘                                               │
│       │     │     │     │                                                   │
│    [Collapsed: "4 Services"]  ← Click to expand                            │
│                                                                             │
│    ══════════════════════════════════════════════════════════════════════  │
│    │  DRILL DOWN into "Investor Register" entity                        │  │
│    ▼                                                                        │
│                                                                             │
│    [DRILLED: Investor Register Detail]                                      │
│                                                                             │
│    ┌─────────────────────────────────────────────────────────────────────┐ │
│    │                    INVESTOR REGISTER                                │ │
│    │                    (12,847 investors)                               │ │
│    │                                                                     │ │
│    │   ┌────┐ ┌────┐ ┌────┐ ┌────┐ ┌────┐ ┌────┐ ┌────┐                │ │
│    │   │Inv1│ │Inv2│ │Inv3│ │Inv4│ │Inv5│ │... │ │+12k│                │ │
│    │   └────┘ └────┘ └────┘ └────┘ └────┘ └────┘ └────┘                │ │
│    │                                                                     │ │
│    │   [Search: ________]  [Filter: Country ▼]  [Sort: AUM ▼]          │ │
│    └─────────────────────────────────────────────────────────────────────┘ │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Architecture: Animation-First Design

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  ANIMATION STATE (lives in UI, not server)                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  struct AnimationState {                                                    │
│      // Camera                                                              │
│      camera_pos: Vec2,           // Current camera position                 │
│      camera_target: Vec2,        // Where camera is flying to               │
│      camera_zoom: f32,           // Current zoom level (0.1 - 10.0)         │
│      camera_zoom_target: f32,    // Target zoom level                       │
│                                                                             │
│      // Node animations                                                     │
│      node_positions: HashMap<String, AnimatedVec2>,  // Interpolating pos   │
│      node_scales: HashMap<String, AnimatedF32>,      // Expand/collapse     │
│      node_opacity: HashMap<String, AnimatedF32>,     // Fade in/out         │
│                                                                             │
│      // View state                                                          │
│      current_view: ViewState,     // Universe | CBU | DrillDown             │
│      drill_stack: Vec<DrillContext>, // Breadcrumb for back navigation      │
│      expanded_nodes: HashSet<String>, // Which composites are expanded      │
│                                                                             │
│      // Timing                                                              │
│      last_frame: Instant,                                                   │
│      delta_time: f32,                                                       │
│  }                                                                          │
│                                                                             │
│  struct AnimatedVec2 {                                                      │
│      current: Vec2,                                                         │
│      target: Vec2,                                                          │
│      velocity: Vec2,             // For spring physics                      │
│  }                                                                          │
│                                                                             │
│  struct AnimatedF32 {                                                       │
│      current: f32,                                                          │
│      target: f32,                                                           │
│      velocity: f32,                                                         │
│  }                                                                          │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                              │
                              │ Each frame (60fps)
                              ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  ANIMATION LOOP                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  fn update(&mut self, ctx: &egui::Context) {                               │
│      // 1. Calculate delta time                                             │
│      let now = Instant::now();                                              │
│      let dt = (now - self.last_frame).as_secs_f32();                       │
│      self.last_frame = now;                                                 │
│                                                                             │
│      // 2. Update all animations (spring physics)                           │
│      self.animation_state.tick(dt);                                         │
│                                                                             │
│      // 3. Render with current animated values                              │
│      self.render(ctx);                                                      │
│                                                                             │
│      // 4. Request repaint if any animation in progress                     │
│      if self.animation_state.is_animating() {                              │
│          ctx.request_repaint();  // Keep 60fps loop going                  │
│      }                                                                      │
│  }                                                                          │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Phase 1: Animation Foundation (Start Here)

### 1.1 Spring Physics Module

**File:** `crates/ob-poc-ui/src/animation.rs`

```rust
//! Spring-based animation system for smooth 60fps transitions
//! 
//! Uses critically-damped spring physics for natural motion.
//! All animations converge smoothly without overshoot.

/// Spring configuration
#[derive(Clone, Copy)]
pub struct SpringConfig {
    /// Stiffness (higher = faster)
    pub stiffness: f32,
    /// Damping ratio (1.0 = critically damped, <1 = bouncy, >1 = sluggish)
    pub damping: f32,
}

impl SpringConfig {
    /// Fast, snappy animation (UI responses)
    pub const FAST: Self = Self { stiffness: 300.0, damping: 1.0 };
    /// Medium animation (camera moves)
    pub const MEDIUM: Self = Self { stiffness: 150.0, damping: 1.0 };
    /// Slow, cinematic animation (view transitions)
    pub const SLOW: Self = Self { stiffness: 80.0, damping: 1.0 };
    /// Bouncy animation (attention-grabbing)
    pub const BOUNCY: Self = Self { stiffness: 200.0, damping: 0.6 };
}

/// Animated f32 value with spring physics
#[derive(Clone)]
pub struct SpringF32 {
    current: f32,
    target: f32,
    velocity: f32,
    config: SpringConfig,
}

impl SpringF32 {
    pub fn new(initial: f32) -> Self {
        Self::with_config(initial, SpringConfig::MEDIUM)
    }
    
    pub fn with_config(initial: f32, config: SpringConfig) -> Self {
        Self {
            current: initial,
            target: initial,
            velocity: 0.0,
            config,
        }
    }
    
    /// Set new target (animation begins)
    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }
    
    /// Jump immediately to value (no animation)
    pub fn set_immediate(&mut self, value: f32) {
        self.current = value;
        self.target = value;
        self.velocity = 0.0;
    }
    
    /// Update animation (call each frame with delta time)
    pub fn tick(&mut self, dt: f32) {
        // Spring physics: F = -kx - cv
        let displacement = self.current - self.target;
        let spring_force = -self.config.stiffness * displacement;
        let damping_force = -self.config.damping * 2.0 * self.config.stiffness.sqrt() * self.velocity;
        let acceleration = spring_force + damping_force;
        
        self.velocity += acceleration * dt;
        self.current += self.velocity * dt;
        
        // Snap to target if close enough
        if (self.current - self.target).abs() < 0.001 && self.velocity.abs() < 0.001 {
            self.current = self.target;
            self.velocity = 0.0;
        }
    }
    
    /// Current animated value
    pub fn get(&self) -> f32 {
        self.current
    }
    
    /// Is animation in progress?
    pub fn is_animating(&self) -> bool {
        (self.current - self.target).abs() > 0.001 || self.velocity.abs() > 0.001
    }
}

/// Animated Vec2 (position, offset, etc.)
#[derive(Clone)]
pub struct SpringVec2 {
    pub x: SpringF32,
    pub y: SpringF32,
}

impl SpringVec2 {
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            x: SpringF32::new(x),
            y: SpringF32::new(y),
        }
    }
    
    pub fn set_target(&mut self, x: f32, y: f32) {
        self.x.set_target(x);
        self.y.set_target(y);
    }
    
    pub fn tick(&mut self, dt: f32) {
        self.x.tick(dt);
        self.y.tick(dt);
    }
    
    pub fn get(&self) -> (f32, f32) {
        (self.x.get(), self.y.get())
    }
    
    pub fn is_animating(&self) -> bool {
        self.x.is_animating() || self.y.is_animating()
    }
}
```

### 1.2 Camera System

```rust
/// Camera for pan/zoom navigation
pub struct Camera {
    /// Camera center position in world coordinates
    pub position: SpringVec2,
    /// Zoom level (1.0 = normal, 2.0 = 2x magnification)
    pub zoom: SpringF32,
    /// Viewport size in screen pixels
    pub viewport: (f32, f32),
}

impl Camera {
    pub fn new(viewport_width: f32, viewport_height: f32) -> Self {
        Self {
            position: SpringVec2::new(0.0, 0.0),
            zoom: SpringF32::with_config(1.0, SpringConfig::MEDIUM),
            viewport: (viewport_width, viewport_height),
        }
    }
    
    /// Fly camera to focus on a point
    pub fn fly_to(&mut self, x: f32, y: f32) {
        self.position.set_target(x, y);
    }
    
    /// Zoom to level (animated)
    pub fn zoom_to(&mut self, level: f32) {
        self.zoom.set_target(level.clamp(0.1, 10.0));
    }
    
    /// Fly to point AND zoom (e.g., drill down)
    pub fn fly_to_zoom(&mut self, x: f32, y: f32, zoom: f32) {
        self.fly_to(x, y);
        self.zoom_to(zoom);
    }
    
    /// Convert world coordinates to screen coordinates
    pub fn world_to_screen(&self, world_x: f32, world_y: f32) -> (f32, f32) {
        let (cam_x, cam_y) = self.position.get();
        let zoom = self.zoom.get();
        let (vw, vh) = self.viewport;
        
        let screen_x = (world_x - cam_x) * zoom + vw / 2.0;
        let screen_y = (world_y - cam_y) * zoom + vh / 2.0;
        
        (screen_x, screen_y)
    }
    
    /// Convert screen coordinates to world coordinates
    pub fn screen_to_world(&self, screen_x: f32, screen_y: f32) -> (f32, f32) {
        let (cam_x, cam_y) = self.position.get();
        let zoom = self.zoom.get();
        let (vw, vh) = self.viewport;
        
        let world_x = (screen_x - vw / 2.0) / zoom + cam_x;
        let world_y = (screen_y - vh / 2.0) / zoom + cam_y;
        
        (world_x, world_y)
    }
    
    pub fn tick(&mut self, dt: f32) {
        self.position.tick(dt);
        self.zoom.tick(dt);
    }
    
    pub fn is_animating(&self) -> bool {
        self.position.is_animating() || self.zoom.is_animating()
    }
}
```

### 1.3 Tasks

- [ ] Create `crates/ob-poc-ui/src/animation.rs`
- [ ] Implement `SpringF32` with critically-damped spring physics
- [ ] Implement `SpringVec2` for 2D positions
- [ ] Implement `Camera` with `fly_to()`, `zoom_to()`, coordinate transforms
- [ ] Add `tick()` call in main update loop
- [ ] Add `ctx.request_repaint()` when animating
- [ ] Test: smooth zoom in/out with scroll wheel
- [ ] Test: smooth pan with middle-mouse drag

---

## Phase 2: Drill Down / Expand-Collapse

### 2.1 Composite Node Concept

Some nodes are **composite** - they represent collections that can be expanded:

| Node Type | Composite? | Drill-Down Shows |
|-----------|------------|------------------|
| Investor Register | Yes | Individual investors (paginated) |
| Product | Yes | Services under this product |
| Service | Yes | Resources under this service |
| Fund Family | Yes | Individual funds |
| ManCo | Maybe | Funds managed by this ManCo |
| UBO (Person) | No | Leaf node |

### 2.2 Drill Context Stack

```rust
/// Tracks where we are in the drill-down hierarchy
pub struct DrillStack {
    stack: Vec<DrillContext>,
}

#[derive(Clone)]
pub struct DrillContext {
    /// Node ID we drilled into
    pub node_id: String,
    /// Label for breadcrumb
    pub label: String,
    /// Camera state before drill (for back navigation)
    pub camera_snapshot: CameraSnapshot,
    /// View mode at this level
    pub view_mode: ViewMode,
}

#[derive(Clone)]
pub struct CameraSnapshot {
    pub position: (f32, f32),
    pub zoom: f32,
}

impl DrillStack {
    pub fn new() -> Self {
        Self { stack: vec![] }
    }
    
    /// Drill into a node
    pub fn push(&mut self, node_id: String, label: String, camera: &Camera, view_mode: ViewMode) {
        self.stack.push(DrillContext {
            node_id,
            label,
            camera_snapshot: CameraSnapshot {
                position: camera.position.get(),
                zoom: camera.zoom.get(),
            },
            view_mode,
        });
    }
    
    /// Go back one level
    pub fn pop(&mut self) -> Option<DrillContext> {
        self.stack.pop()
    }
    
    /// Get breadcrumb trail
    pub fn breadcrumbs(&self) -> Vec<&str> {
        self.stack.iter().map(|c| c.label.as_str()).collect()
    }
    
    /// Current depth
    pub fn depth(&self) -> usize {
        self.stack.len()
    }
    
    /// At root level?
    pub fn is_root(&self) -> bool {
        self.stack.is_empty()
    }
}
```

### 2.3 Expand/Collapse Animation

```rust
/// State for expandable nodes
pub struct ExpandState {
    /// Which nodes are expanded
    expanded: HashSet<String>,
    /// Animation progress per node (0.0 = collapsed, 1.0 = expanded)
    progress: HashMap<String, SpringF32>,
}

impl ExpandState {
    pub fn toggle(&mut self, node_id: &str) {
        if self.expanded.contains(node_id) {
            self.expanded.remove(node_id);
            if let Some(p) = self.progress.get_mut(node_id) {
                p.set_target(0.0);
            }
        } else {
            self.expanded.insert(node_id.to_string());
            self.progress
                .entry(node_id.to_string())
                .or_insert_with(|| SpringF32::with_config(0.0, SpringConfig::FAST))
                .set_target(1.0);
        }
    }
    
    pub fn is_expanded(&self, node_id: &str) -> bool {
        self.expanded.contains(node_id)
    }
    
    /// Get interpolated expand progress (for animation)
    pub fn get_progress(&self, node_id: &str) -> f32 {
        self.progress.get(node_id).map(|p| p.get()).unwrap_or(0.0)
    }
    
    pub fn tick(&mut self, dt: f32) {
        for p in self.progress.values_mut() {
            p.tick(dt);
        }
    }
}
```

### 2.4 Tasks

- [ ] Add `is_composite` field to `GraphNode` (set by builder)
- [ ] Add `child_count` field for composite nodes
- [ ] Implement `DrillStack` for navigation history
- [ ] Implement `ExpandState` for expand/collapse
- [ ] Add expand/collapse icon to composite nodes
- [ ] Animate children appearing/disappearing on expand
- [ ] Add breadcrumb UI for drill navigation
- [ ] Back button / ESC to pop drill stack
- [ ] Double-click on composite = drill down
- [ ] Single-click expand icon = expand in place

---

## Phase 3: Semantic Zoom Levels

### 3.1 Level of Detail (LOD)

Different zoom levels show different detail:

| Zoom Level | What's Visible | Node Rendering |
|------------|----------------|----------------|
| 0.1 - 0.3 | Universe (all CBUs) | Dots with color |
| 0.3 - 0.7 | CBU clusters | Small icons, no labels |
| 0.7 - 1.5 | Single CBU | Full nodes with labels |
| 1.5 - 3.0 | Detail view | Nodes + sublabels + status |
| 3.0+ | Drill view | Full detail + child counts |

### 3.2 LOD Renderer

```rust
pub enum LodLevel {
    Universe,   // Dots
    Cluster,    // Icons
    Normal,     // Full nodes
    Detail,     // Nodes + status
    Drill,      // Full detail
}

impl LodLevel {
    pub fn from_zoom(zoom: f32) -> Self {
        match zoom {
            z if z < 0.3 => Self::Universe,
            z if z < 0.7 => Self::Cluster,
            z if z < 1.5 => Self::Normal,
            z if z < 3.0 => Self::Detail,
            _ => Self::Drill,
        }
    }
}

impl GraphRenderer {
    fn render_node_lod(&self, ui: &mut egui::Ui, node: &GraphNode, lod: LodLevel, screen_pos: (f32, f32)) {
        match lod {
            LodLevel::Universe => {
                // Just a colored dot
                let color = self.node_category_color(node);
                let radius = 3.0 + node.importance.unwrap_or(0.5) * 5.0;
                ui.painter().circle_filled(
                    egui::pos2(screen_pos.0, screen_pos.1),
                    radius,
                    color,
                );
            }
            LodLevel::Cluster => {
                // Small icon, maybe label on hover
                let size = 20.0;
                let icon = self.node_icon(node);
                // Draw icon...
            }
            LodLevel::Normal => {
                // Full node with label
                self.render_node_full(ui, node, screen_pos, false);
            }
            LodLevel::Detail => {
                // Full node with sublabel and status
                self.render_node_full(ui, node, screen_pos, true);
            }
            LodLevel::Drill => {
                // Full detail + expand hint for composites
                self.render_node_full(ui, node, screen_pos, true);
                if node.is_composite {
                    self.render_expand_hint(ui, node, screen_pos);
                }
            }
        }
    }
}
```

### 3.3 Tasks

- [ ] Implement `LodLevel` enum
- [ ] Add `from_zoom()` thresholds
- [ ] Create LOD-specific node renderers
- [ ] Smooth transitions between LOD levels
- [ ] Labels fade in/out based on zoom
- [ ] Edge rendering LOD (hide at universe level)

---

## Phase 4: Taxonomy Walk (Up/Down)

### 4.1 Concept

User can "walk" the taxonomy:
- **Products view:** CBU → Products → Services → Resources
- **UBO view:** CBU → Fund → ManCo → HoldCo → UBO
- **Pruning:** Hide branches, focus on specific paths

### 4.2 Taxonomy Navigator

```rust
pub struct TaxonomyNavigator {
    /// Current focus node
    focus_node: Option<String>,
    /// Visible levels (depth from focus)
    visible_up: i32,    // How many levels above focus to show
    visible_down: i32,  // How many levels below focus to show
    /// Hidden branches (pruned)
    hidden_branches: HashSet<String>,
}

impl TaxonomyNavigator {
    /// Focus on a specific node
    pub fn focus(&mut self, node_id: &str) {
        self.focus_node = Some(node_id.to_string());
    }
    
    /// Walk up the hierarchy (show parent)
    pub fn walk_up(&mut self) {
        self.visible_up += 1;
    }
    
    /// Walk down the hierarchy (show children)
    pub fn walk_down(&mut self) {
        self.visible_down += 1;
    }
    
    /// Prune a branch (hide it and descendants)
    pub fn prune(&mut self, node_id: &str) {
        self.hidden_branches.insert(node_id.to_string());
    }
    
    /// Restore a pruned branch
    pub fn restore(&mut self, node_id: &str) {
        self.hidden_branches.remove(node_id);
    }
    
    /// Should this node be visible given current navigation?
    pub fn is_visible(&self, node_id: &str, depth_from_focus: i32) -> bool {
        if self.hidden_branches.contains(node_id) {
            return false;
        }
        
        if depth_from_focus < 0 {
            // Above focus
            depth_from_focus.abs() <= self.visible_up
        } else {
            // Below focus
            depth_from_focus <= self.visible_down
        }
    }
}
```

### 4.3 Tasks

- [ ] Implement `TaxonomyNavigator`
- [ ] Add keyboard shortcuts: Up/Down arrow to walk
- [ ] Add prune button on node context menu
- [ ] Visual indicator for pruned branches (faded with "+" icon)
- [ ] Animate nodes appearing/disappearing as levels change
- [ ] Smooth camera follow as focus changes

---

## Phase 5: Gestures and Controls

### 5.1 Input Mapping

| Input | Action |
|-------|--------|
| Scroll wheel | Zoom in/out (centered on cursor) |
| Middle-drag | Pan camera |
| Left-click | Select node |
| Double-click | Drill into composite / Focus on node |
| Right-click | Context menu |
| ESC | Back (pop drill stack) |
| Arrow keys | Walk taxonomy |
| +/- | Expand/collapse levels |
| Space | Fit all to view |
| F | Fly to selected |
| H | Home (reset view) |

### 5.2 Gesture Handler

```rust
pub struct GestureHandler {
    /// Last mouse position (for drag)
    last_mouse_pos: Option<egui::Pos2>,
    /// Is middle button held?
    is_panning: bool,
    /// Double-click timer
    last_click_time: Option<Instant>,
    last_click_node: Option<String>,
}

impl GestureHandler {
    pub fn handle_input(
        &mut self,
        ui: &egui::Ui,
        camera: &mut Camera,
        graph: &CbuGraph,
        drill_stack: &mut DrillStack,
    ) -> Option<GraphAction> {
        let response = ui.interact(
            ui.available_rect_before_wrap(),
            ui.id().with("graph_canvas"),
            egui::Sense::click_and_drag(),
        );
        
        // Scroll = zoom
        if let Some(scroll) = ui.input(|i| i.scroll_delta.y).filter(|&s| s != 0.0) {
            let zoom_factor = 1.0 + scroll * 0.001;
            let new_zoom = camera.zoom.get() * zoom_factor;
            
            // Zoom toward cursor position
            if let Some(cursor) = ui.input(|i| i.pointer.hover_pos()) {
                let (world_x, world_y) = camera.screen_to_world(cursor.x, cursor.y);
                camera.zoom_to(new_zoom);
                // Adjust position to keep cursor point stable
                // (math for zoom-to-point)
            } else {
                camera.zoom_to(new_zoom);
            }
        }
        
        // Middle-drag = pan
        if response.dragged_by(egui::PointerButton::Middle) {
            let delta = response.drag_delta();
            let zoom = camera.zoom.get();
            let (cx, cy) = camera.position.get();
            camera.position.set_immediate(
                cx - delta.x / zoom,
                cy - delta.y / zoom,
            );
        }
        
        // Double-click = drill
        if response.double_clicked() {
            if let Some(node) = self.node_at_cursor(ui, camera, graph) {
                if node.is_composite {
                    return Some(GraphAction::DrillInto(node.id.clone()));
                }
            }
        }
        
        // Keyboard
        ui.input(|i| {
            if i.key_pressed(egui::Key::Escape) {
                return Some(GraphAction::DrillBack);
            }
            if i.key_pressed(egui::Key::Space) {
                return Some(GraphAction::FitAll);
            }
        });
        
        None
    }
}

pub enum GraphAction {
    Select(String),
    DrillInto(String),
    DrillBack,
    Expand(String),
    Collapse(String),
    FitAll,
    FlyTo(String),
    Prune(String),
    Restore(String),
}
```

### 5.3 Tasks

- [ ] Implement scroll-to-zoom with cursor centering
- [ ] Implement middle-drag pan
- [ ] Implement double-click drill
- [ ] Add keyboard shortcuts
- [ ] Add context menu (right-click)
- [ ] Implement "Fit All" (calculate bounding box, fly to show all)
- [ ] Implement "Fly To" selected node

---

## Phase 6: Async Data Loading for Drill-Down

### 6.1 Problem

When drilling into "Investor Register" with 12,000 investors, we can't load all at once.

### 6.2 Solution: Lazy Loading with Placeholder

```rust
pub struct LazyGraphData {
    /// Loaded data
    loaded_nodes: HashMap<String, GraphNode>,
    /// Placeholder for unloaded children
    pending_loads: HashSet<String>,
    /// Currently loading
    loading: HashSet<String>,
}

impl LazyGraphData {
    /// Request children of a node (async load)
    pub fn request_children(&mut self, parent_id: &str, runtime: &Runtime) {
        if self.loading.contains(parent_id) {
            return; // Already loading
        }
        
        self.loading.insert(parent_id.to_string());
        
        // Spawn async task
        let parent_id = parent_id.to_string();
        runtime.spawn(async move {
            // Load from server...
            let children = api::get_node_children(&parent_id).await;
            // Send back to UI thread via channel
        });
    }
    
    /// For 12,000 investors: virtual scrolling
    pub fn get_children_paginated(
        &mut self,
        parent_id: &str,
        offset: usize,
        limit: usize,
    ) -> (Vec<&GraphNode>, usize) {
        // Return visible page + total count
    }
}
```

### 6.3 Tasks

- [ ] Add `get_node_children` API endpoint
- [ ] Implement lazy loading with loading indicator
- [ ] Virtual scrolling for large child lists
- [ ] Placeholder nodes while loading
- [ ] Cancel loading if user navigates away

---

## Implementation Order

1. **Phase 1: Animation Foundation** ← START HERE
   - Spring physics
   - Camera with fly/zoom
   - Basic 60fps loop

2. **Phase 5: Gestures** (partial)
   - Scroll-to-zoom
   - Pan
   - Basic selection

3. **Phase 3: Semantic Zoom**
   - LOD levels
   - Labels fade

4. **Phase 2: Drill Down**
   - Drill stack
   - Expand/collapse

5. **Phase 4: Taxonomy Walk**
   - Up/down navigation
   - Prune

6. **Phase 6: Async Loading**
   - Large datasets

---

## Success Criteria

- [ ] Camera movement feels smooth and responsive (60fps)
- [ ] Zoom follows cursor (not center of screen)
- [ ] Drill-down animates smoothly (zoom + load)
- [ ] Expand/collapse animates children
- [ ] Labels appear/disappear based on zoom (no jarring)
- [ ] Large graphs (1000+ nodes) still performant
- [ ] Back navigation restores exact previous view
- [ ] Keyboard navigation feels natural

---

## References

- EGUI animation: https://docs.rs/egui/latest/egui/struct.Context.html#method.request_repaint
- Spring physics: https://www.youtube.com/watch?v=KPoeNZZ6H4s (GDC talk on springs)
- Game camera: https://www.gamasutra.com/blogs/ItayKeren/20150511/243083/Scroll_Back_The_Theory_and_Practice_of_Cameras_in_SideScrollers.php
- Semantic zoom: https://observablehq.com/@d3/zoom

---

*Animation-first visualization for game-engine-quality CBU exploration.*
