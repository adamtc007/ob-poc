# OB-POC: Taxonomy-Driven Layout & Session Context Implementation

## Overview

Transform viewport rendering from force-directed chaos to deterministic, 
taxonomy-driven layouts with unified session context shared between REPL and viewport.

**Key Outcomes:**
- Predictable, repeatable layouts based on taxonomy rules
- Mass-based view pivoting (astro ↔ pyramid)
- Single source of truth session context
- Explicit blast radius for agent operations
- Floating entity handling (Option<T> unknowns)

**Related Slash Commands:** (restart Claude Code session after creation)
- `/project:session-context` - Implement session context types
- `/project:mass-calc` - Implement mass calculation
- `/project:layout-config` - Implement layout configuration types
- `/project:layout-pyramid` - Implement pyramid layout engine
- `/project:layout-solar` - Implement solar system layout engine
- `/project:layout-matrix` - Implement matrix layout engine
- `/project:view-transitions` - Implement view transitions and density rules
- `/project:egui-renderer` - Implement egui graph renderer
- `/project:repl-navigation` - Implement REPL navigation commands

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                      SESSION CONTEXT                            │
│                   (Single Source of Truth)                      │
│                                                                 │
│  scope: "allianz.trading.cbu_germany"                          │
│  mass: 847 (computed)                                          │
│  view_mode: HybridDrilldown (derived from mass)                │
│  focal_entity: Option<EntityId>                                │
│  selected: Vec<EntityId>                                       │
│  version: u64 (increments on change)                           │
│                                                                 │
│              ┌─────────────┴─────────────┐                     │
│              ▼                           ▼                      │
│         REPL                        VIEWPORT                    │
│   (observes + mutates)         (observes + mutates)            │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                    LAYOUT PIPELINE                              │
│                                                                 │
│  SemanticGraph + TaxonomyConfig + Mass → LayoutEngine          │
│                                              │                  │
│                                              ▼                  │
│                                       PositionedGraph          │
│                                              │                  │
│                                              ▼                  │
│                                       egui Renderer            │
└─────────────────────────────────────────────────────────────────┘
```

---

## Phase 1: Foundation — Session Context & Shared State

### 1.1 Define Core Session Types
**File:** `rust/crates/ob-poc-types/src/session/context.rs`
**Command:** `/project:session-context`

**Types to implement:**
```rust
pub struct SessionContext {
    pub scope: ScopePath,
    pub graph: Arc<SemanticGraph>,
    pub mass: StructMass,
    pub focal_entity: Option<EntityId>,
    pub selected: Vec<EntityId>,
    pub filters: FilterSet,
    pub view_mode: ViewMode,
    pub version: u64,
    pub updated_at: Instant,
}

pub struct ScopePath {
    pub segments: Vec<ScopeSegment>,
}

pub struct ScopeSegment {
    pub name: String,
    pub entity_type: String,
    pub entity_id: Option<EntityId>,
    pub mass: u32,
}
```

**Acceptance Criteria:**
- [ ] SessionContext holds all session state
- [ ] ScopePath supports hierarchical navigation (display(), parent(), push())
- [ ] Version increments on any mutation
- [ ] All types are Clone + Send + Sync safe

---

### 1.2 Implement SessionManager
**File:** `rust/crates/ob-poc-types/src/session/manager.rs`
**Command:** `/project:session-context`

**Implementation:**
```rust
pub struct SessionManager {
    context: Arc<RwLock<SessionContext>>,
    change_tx: watch::Sender<SessionContext>,
    change_rx: watch::Receiver<SessionContext>,
}

impl SessionManager {
    pub fn current(&self) -> SessionContext;
    pub fn subscribe(&self) -> watch::Receiver<SessionContext>;
    pub fn navigate(&self, new_scope: ScopePath, graph: SemanticGraph);
    pub fn set_focus(&self, entity: Option<EntityId>);
    pub fn drill_down(&self, entity_id: EntityId, subgraph: SemanticGraph);
    pub fn navigate_up(&self, parent_graph: SemanticGraph);
    pub fn apply_filter(&self, filter: Filter);
}
```

**Acceptance Criteria:**
- [ ] Thread-safe read/write access (parking_lot::RwLock)
- [ ] Change notifications via tokio::sync::watch channel
- [ ] REPL and Viewport can both subscribe
- [ ] Mutations are atomic and versioned

---

### 1.3 Wire Session to REPL
**File:** `rust/crates/ob-poc-ui/src/repl/state.rs`

**Add to ReplState:**
```rust
pub struct ReplState {
    session: Arc<SessionManager>,
    // ... existing fields
}

impl ReplState {
    pub fn render_context_header(&self) -> String;
    pub fn confirm_blast_radius(&self, operation: &str) -> BlastRadiusCheck;
}
```

**Acceptance Criteria:**
- [ ] REPL prompt shows current scope path
- [ ] Mass and entity counts visible
- [ ] Blast radius confirmation for large operations (mass > threshold)
- [ ] Header updates when session context changes

---

### 1.4 Wire Session to Viewport
**File:** `rust/crates/ob-poc-ui/src/egui/viewport_state.rs`

**Add to ViewportState:**
```rust
pub struct ViewportState {
    session: Arc<SessionManager>,
    change_rx: watch::Receiver<SessionContext>,
    last_context_version: u64,
    // ... existing fields
}
```

**Acceptance Criteria:**
- [ ] Viewport observes session changes via watch channel
- [ ] Layout recomputes on context change
- [ ] Transitions animate smoothly
- [ ] No polling — event-driven updates

---

## Phase 2: Mass Calculation

### 2.1 Define Mass Types
**File:** `rust/crates/ob-poc-types/src/layout/mass.rs`
**Command:** `/project:mass-calc`

```rust
#[derive(Debug, Clone)]
pub struct StructMass {
    pub total: u32,
    pub breakdown: MassBreakdown,
    pub density: f32,
    pub depth: u32,
    pub complexity: f32,
}

#[derive(Debug, Clone, Default)]
pub struct MassBreakdown {
    pub cbus: u32,
    pub persons: u32,
    pub holdings: u32,
    pub edges: u32,
    pub floating: u32,
}

#[derive(Debug, Deserialize)]
pub struct MassWeights {
    pub cbu: u32,        // e.g., 100
    pub person: u32,     // e.g., 10
    pub holding: u32,    // e.g., 20
    pub edge: u32,       // e.g., 5
    pub floating: u32,   // e.g., 15
}

#[derive(Debug, Deserialize)]
pub struct MassThresholds {
    pub astro_threshold: u32,   // e.g., 500
    pub hybrid_threshold: u32,  // e.g., 100
}
```

**Acceptance Criteria:**
- [ ] Mass computed from graph structure
- [ ] Floating entities detected (no structural edges)
- [ ] Weights configurable via MassWeights
- [ ] suggested_view() returns appropriate ViewMode

---

### 2.2 Mass Computation
**Command:** `/project:mass-calc`

```rust
impl StructMass {
    pub fn compute(graph: &SemanticGraph, weights: &MassWeights) -> Self {
        // 1. Count entities by type
        // 2. Identify floating nodes (no structural edges)
        // 3. Compute weighted total
        // 4. Calculate depth and complexity
    }
    
    pub fn suggested_view(&self, thresholds: &MassThresholds) -> ViewMode {
        if self.total > thresholds.astro_threshold {
            ViewMode::AstroOverview
        } else if self.total > thresholds.hybrid_threshold {
            ViewMode::HybridDrilldown
        } else if self.breakdown.cbus > 1 {
            ViewMode::MultiCbuDetail
        } else {
            ViewMode::SingleCbuPyramid
        }
    }
}
```

---

## Phase 3: Taxonomy Layout Configuration

### 3.1 YAML Config Files
**Directory:** `rust/config/taxonomies/`
**Command:** `/project:layout-config`

**Create files:**
- `ubo_ownership.yaml` - Pyramid layout for UBO structures
- `entity_universe.yaml` - Solar system layout for large datasets
- `trading_instruments.yaml` - Matrix layout for instrument grids

**Example: ubo_ownership.yaml**
```yaml
taxonomy: ubo_ownership

layout:
  strategy: pyramid
  direction: top_down
  level_spacing: 120
  sibling_spacing: 80
  pyramid_expansion: 1.2

rank_rules:
  ULTIMATE_BENEFICIAL_OWNER:
    rank: 0
    anchor: top_center
  INTERMEDIATE_HOLDING:
    rank: derived
  SUBJECT_ENTITY:
    rank: leaf
  PERSON:
    rank: contextual

edge_topology:
  OWNS:
    direction: above_to_below
    rank_delta: 1
  CONTROLS:
    direction: above_to_below
    rank_delta: 1
  RELATED_TO:
    direction: horizontal
    rank_delta: 0

floating_zone:
  position: right_gutter
  layout: vertical_stack
  max_width: 200
  label: "Unlinked Persons"
```

**Example: entity_universe.yaml**
```yaml
taxonomy: entity_universe

layout:
  strategy: solar_system
  center: focal_entity
  ring_spacing: 100
  rotation: organic
  
  rings:
    - name: core
      filter: { role_in: ["UBO", "DIRECTOR", "SIGNATORY"] }
      radius: 150
    - name: inner
      filter: { edge_type_in: ["OWNS", "CONTROLS"] }
      radius: 280
    - name: outer
      filter: { edge_type_in: ["RELATED_TO", "ASSOCIATED"] }
      radius: 420
    - name: asteroid_belt
      filter: floating
      radius: 580
      style: scattered
```

---

### 3.2 Rust Config Types
**File:** `rust/crates/ob-poc-types/src/layout/config.rs`
**Command:** `/project:layout-config`

```rust
#[derive(Debug, Deserialize)]
pub struct TaxonomyLayoutConfig {
    pub taxonomy: String,
    pub layout: LayoutSpec,
    pub rank_rules: HashMap<String, RankRule>,
    pub edge_topology: HashMap<String, EdgeTopology>,
    pub floating_zone: Option<FloatingZoneSpec>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "strategy", rename_all = "snake_case")]
pub enum LayoutStrategy {
    Pyramid(PyramidConfig),
    SolarSystem(SolarSystemConfig),
    Matrix(MatrixConfig),
    ForceDirected(ForceConfig),
}

#[derive(Debug, Deserialize)]
pub struct PyramidConfig {
    pub direction: Direction,
    pub level_spacing: f32,
    pub sibling_spacing: f32,
    pub pyramid_expansion: f32,
    pub level_assignment: LevelAssignment,
}

#[derive(Debug, Deserialize)]
pub struct SolarSystemConfig {
    pub center: CenterSelection,
    pub ring_spacing: f32,
    pub rings: Vec<RingDefinition>,
    pub rotation: RotationStrategy,
}

#[derive(Debug, Deserialize)]
pub struct RingDefinition {
    pub name: String,
    pub filter: RingFilter,
    pub radius: f32,
    pub style: Option<RingStyle>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RingFilter {
    RoleIn(Vec<String>),
    EdgeTypeIn(Vec<String>),
    HopDistance(u32),
    Floating,
    Custom(String),
}
```

**Acceptance Criteria:**
- [ ] All YAML fields map to Rust types
- [ ] Serde deserializes without errors
- [ ] Config loader caches loaded configs
- [ ] Hot-reload support for development

---

## Phase 4: Layout Engine Implementation

### 4.1 Layout Engine Trait
**File:** `rust/crates/ob-poc-types/src/layout/engine.rs`

```rust
pub trait LayoutEngine {
    fn layout(
        &self,
        ast: &SemanticGraph,
        config: &TaxonomyLayoutConfig,
        viewport: Rect,
    ) -> PositionedGraph;
}

pub struct PositionedGraph {
    pub nodes: Vec<PositionedNode>,
    pub edges: Vec<PositionedEdge>,
    pub floating_zone: Option<FloatingZoneLayout>,
    pub bounds: Rect,
}

pub struct PositionedNode {
    pub id: NodeId,
    pub position: Pos2,
    pub size: Vec2,
    pub rank: Option<u32>,
    pub ring: Option<String>,
    pub style: NodeStyle,
    pub is_floating: bool,
}

pub struct PositionedEdge {
    pub source: NodeId,
    pub target: NodeId,
    pub path: Vec<Pos2>,
    pub style: EdgeStyle,
}
```

---

### 4.2 Pyramid Layout (Sugiyama-style)
**File:** `rust/crates/ob-poc-ui/src/layout/pyramid.rs`
**Command:** `/project:layout-pyramid`

**Algorithm:**
1. `assign_levels()` - Use rank_rules, support Fixed/Derived/Leaf/Contextual
2. `partition_floating()` - Separate nodes without structural edges
3. `order_within_levels()` - Barycentric method for crossing minimization
4. `assign_coordinates()` - Pyramid expansion per level
5. `layout_floating_zone()` - Stack in gutter
6. `route_edges()` - Straight for adjacent, splines for skipped levels

**Acceptance Criteria:**
- [ ] Nodes assigned to correct levels by rank rules
- [ ] Floating nodes separated to gutter zone
- [ ] Pyramid widens at each level per expansion factor
- [ ] Edges route cleanly without crossing nodes

---

### 4.3 Solar System Layout
**File:** `rust/crates/ob-poc-ui/src/layout/solar_system.rs`
**Command:** `/project:layout-solar`

**Algorithm:**
1. `select_center()` - By CenterSelection enum
2. `assign_to_rings()` - Match nodes to RingDefinition filters
3. `position_on_ring()` - Angular distribution by RingStyle
4. `layout_asteroid_belt()` - Floating entities with scattered style
5. `route_orbital_edges()` - Curved beziers for cross-ring

**Acceptance Criteria:**
- [ ] Center node at viewport center
- [ ] Nodes on correct rings by filter
- [ ] Asteroid belt for floating entities
- [ ] Curved edges between rings

---

### 4.4 Matrix Layout
**File:** `rust/crates/ob-poc-ui/src/layout/matrix.rs`
**Command:** `/project:layout-matrix`

**Algorithm:**
1. Assign nodes to cells by rows_by/cols_by attributes
2. Compute row/column sizes
3. Position nodes in grid cells
4. Route edges as orthogonal paths

---

### 4.5 Layout Dispatcher
**File:** `rust/crates/ob-poc-ui/src/layout/dispatcher.rs`

```rust
pub struct TaxonomyLayoutDispatcher {
    pyramid: PyramidLayout,
    solar_system: SolarSystemLayout,
    matrix: MatrixLayout,
    force_directed: ForceDirectedLayout,
}

impl LayoutEngine for TaxonomyLayoutDispatcher {
    fn layout(&self, ast: &SemanticGraph, config: &TaxonomyLayoutConfig, viewport: Rect) -> PositionedGraph {
        match &config.layout.strategy {
            LayoutStrategy::Pyramid(cfg) => self.pyramid.layout(ast, cfg, viewport),
            LayoutStrategy::SolarSystem(cfg) => self.solar_system.layout(ast, cfg, viewport),
            LayoutStrategy::Matrix(cfg) => self.matrix.layout(ast, cfg, viewport),
            LayoutStrategy::ForceDirected(cfg) => self.force_directed.layout(ast, cfg, viewport),
        }
    }
}
```

---

## Phase 5: Density Rules & View Transitions

### 5.1 Density Rule Evaluation
**File:** `rust/crates/ob-poc-ui/src/view/density.rs`
**Command:** `/project:view-transitions`

```rust
#[derive(Debug, Deserialize)]
pub struct DensityRule {
    pub threshold: DensityThreshold,
    pub mode: ViewMode,
    pub node_rendering: NodeRenderMode,
    pub expand_taxonomy: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum DensityThreshold {
    GreaterThan { gt: u32, entity_type: String },
    LessThan { lt: u32, entity_type: String },
    Range { min: u32, max: u32, entity_type: String },
    Single,
}

pub fn evaluate_density_rules(
    visible: &VisibleEntities,
    rules: &[DensityRule],
) -> ViewMode;
```

---

### 5.2 View Transitions
**File:** `rust/crates/ob-poc-ui/src/view/transition.rs`
**Command:** `/project:view-transitions`

```rust
pub struct ViewTransition {
    pub from_layout: PositionedGraph,
    pub to_layout: PositionedGraph,
    pub from_mode: ViewMode,
    pub to_mode: ViewMode,
    pub progress: f32,
    pub started_at: Instant,
}

pub fn interpolate_layouts(
    from: &PositionedGraph,
    to: &PositionedGraph,
    t: f32,
) -> PositionedGraph;

pub fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}
```

**Acceptance Criteria:**
- [ ] Nodes morph smoothly between positions
- [ ] New nodes fade in, removed nodes fade out
- [ ] Debounce prevents flip-flopping during zoom

---

## Phase 6: egui Renderer Integration

### 6.1 Graph Renderer
**File:** `rust/crates/ob-poc-ui/src/egui/graph_renderer.rs`
**Command:** `/project:egui-renderer`

```rust
pub fn render_positioned_graph(
    ui: &mut egui::Ui,
    graph: &PositionedGraph,
    interaction_state: &mut GraphInteractionState,
) {
    // 1. Draw edges (Painter)
    // 2. Draw floating zone background
    // 3. Draw nodes (ui.put)
    // 4. Handle interactions
}
```

---

### 6.2 Scope Indicator Overlay
**File:** `rust/crates/ob-poc-ui/src/egui/overlays.rs`

```rust
pub fn render_scope_indicator(
    ui: &mut egui::Ui,
    session: &SessionContext,
) {
    // Breadcrumb: allianz › trading › germany
    // Stats: Mass: 847 | 12 CBUs | 43 persons
    // Mode: HybridDrilldown
}
```

---

### 6.3 Blast Radius Visualization

When REPL has pending operation:
- Highlight in-scope nodes
- Dim out-of-scope nodes
- Show count badge
- Pulse animation

---

## Phase 7: REPL Commands

### 7.1 Navigation Commands
**File:** `rust/crates/ob-poc-ui/src/repl/commands/navigation.rs`
**Command:** `/project:repl-navigation`

| Command | Description |
|---------|-------------|
| `cd <path>` | Navigate to scope (relative: `cd ..`, `cd ../trading`) |
| `pwd` | Print current scope with mass |
| `ls` | List entities in current scope |
| `focus <id>` | Set focal entity |
| `select <ids>` | Multi-select for batch ops |
| `filter <expr>` | Apply filter |
| `clear-filter` | Remove filters |

---

### 7.2 View Commands

| Command | Description |
|---------|-------------|
| `view` | Show current mode + reason |
| `view astro\|pyramid\|matrix` | Force view mode |
| `view auto` | Return to mass-based |
| `zoom <level>` | Set viewport zoom |
| `center [id]` | Center on entity |

---

## Phase 8: Configuration

### 8.1 Master Config
**File:** `rust/config/layout_config.yaml`

```yaml
mass_weights:
  cbu: 100
  person: 10
  holding: 20
  edge: 5
  floating: 15

mass_thresholds:
  astro_threshold: 500
  hybrid_threshold: 100

density_rules:
  - threshold: { gt: 20, entity_type: visible_cbu }
    mode: astro_overview
    node_rendering: compact_dot
    
  - threshold: { min: 5, max: 20, entity_type: visible_cbu }
    mode: hybrid_drilldown
    node_rendering: expanded_taxonomy
    
  - threshold: { lt: 5, entity_type: visible_cbu }
    mode: full_detail
    node_rendering: full_taxonomy_pyramid

transitions:
  duration_ms: 400
  debounce_ms: 300
  easing: ease_out_cubic

visibility:
  min_node_size_px: 20
```

---

## Implementation Order

```
Phase 1: Session Context (FOUNDATION)
├── 1.1 Session Types
├── 1.2 SessionManager
├── 1.3 Wire to REPL
└── 1.4 Wire to Viewport

Phase 2: Mass Calculation
├── 2.1 Mass Types
└── 2.2 Computation + suggested_view()

Phase 3: Layout Configuration
├── 3.1 YAML Files
├── 3.2 Rust Types
└── 3.3 Config Loader

Phase 4: Layout Engines
├── 4.1 Trait Definition
├── 4.2 Pyramid Layout ◄── Most common, implement first
├── 4.3 Solar System Layout
├── 4.4 Matrix Layout
└── 4.5 Dispatcher

Phase 5: View Transitions
├── 5.1 Density Rules
├── 5.2 Interpolation
└── 5.3 Debouncing

Phase 6: egui Renderer
├── 6.1 Graph Renderer
├── 6.2 Scope Overlay
└── 6.3 Blast Radius

Phase 7: REPL Commands
├── 7.1 Navigation
└── 7.2 View Control

Phase 8: Config Tuning
```

---

## Testing Strategy

### Unit Tests
- Mass calculation with various graph shapes
- Rank assignment by different rules
- Density threshold evaluation
- Layout coordinate calculation

### Integration Tests
- Session sync between REPL and Viewport
- Layout determinism (same input → same output)
- Config loading and validation

### Visual Regression Tests
- Golden screenshots for each layout type
- Transition animation frames

---

## Definition of Done

**Per Phase:**
- [ ] Code compiles without warnings
- [ ] Unit tests pass
- [ ] Integration tests pass
- [ ] REPL and Viewport stay in sync

**Final Acceptance:**
- [ ] User navigates scope in REPL → viewport updates
- [ ] User clicks node in viewport → REPL shows context
- [ ] Mass determines view mode automatically
- [ ] Transitions animate smoothly
- [ ] Blast radius always visible and accurate
- [ ] Same data + same config = same visual output (deterministic)
- [ ] Floating entities have explicit home (not random placement)
