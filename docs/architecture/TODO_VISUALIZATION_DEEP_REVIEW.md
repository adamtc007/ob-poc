# TODO: Visualization Deep Review

**Created**: 2026-01-05  
**Status**: Pre-Production Readiness  
**Priority**: HIGH  

---

## Executive Summary

This document provides a comprehensive architectural review of the ob-poc visualization system, covering session scope, selection, filtering, view modes (KYC/UBO vs Trading/CBU), and auto-layout/zoom mechanics. The review identifies gaps, inconsistencies, and proposes solutions.

---

## 1. CURRENT ARCHITECTURE OVERVIEW

### 1.1 Stack Layers

```
┌─────────────────────────────────────────────────────────────────────┐
│                     CLIENT (egui/WASM)                              │
│  - Receives pre-positioned nodes from server                        │
│  - Renders immediately (no local layout logic)                      │
│  - Viewport state (zoom, pan) managed locally                       │
└─────────────────────────────────────────────────────────────────────┘
                                 │
                                 │ HTTP/JSON
                                 ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     SERVER (Axum/Rust)                              │
├─────────────────────────────────────────────────────────────────────┤
│  graph/                                                             │
│  ├── types.rs         EntityGraph, GraphNode, edges, enums          │
│  ├── layout.rs        LayoutEngine - position computation          │
│  ├── filters.rs       GraphFilterOps - visibility control          │
│  ├── viewport.rs      ViewportContext - zoom/pan state             │
│  ├── view_model.rs    GraphViewModel - DSL verb output             │
│  ├── builder.rs       CbuGraphBuilder - DB → graph construction    │
│  └── query_engine.rs  GraphQueryEngine - graph.* verb executor     │
├─────────────────────────────────────────────────────────────────────┤
│  session/                                                           │
│  ├── view_state.rs    ViewState - "it" the session sees            │
│  ├── scope.rs         SessionScope - load strategies               │
│  └── research_context TaxonomyStack for fractal navigation         │
├─────────────────────────────────────────────────────────────────────┤
│  taxonomy/                                                          │
│  └── TaxonomyBuilder  Builds tree from database                     │
├─────────────────────────────────────────────────────────────────────┤
│  dsl_v2/custom_ops/                                                 │
│  ├── view_ops.rs      view.* verbs (universe, book, cbu, etc.)     │
│  └── ubo_graph_ops.rs graph.* verbs                                 │
└─────────────────────────────────────────────────────────────────────┘
```

### 1.2 Two Parallel Graph Systems

**PROBLEM**: There are two graph type systems in coexistence:

| System | Files | Purpose | Status |
|--------|-------|---------|--------|
| **EntityGraph** (new) | `types.rs` lines 1-900 | Unified UBO/CBU graph with typed edges | Partially implemented |
| **CbuGraph** (legacy) | `types.rs` lines 1900-2500 | Older flat node/edge structure | Still in use |

The layout engine (`layout.rs`) still operates on `CbuGraph`, not `EntityGraph`.

---

## 2. SESSION SCOPE ANALYSIS

### 2.1 Scope Definitions (GraphScope enum)

```rust
pub enum GraphScope {
    Empty,                              // Initial state
    SingleCbu { cbu_id, cbu_name },     // One CBU
    Book { apex_entity_id, apex_name }, // All CBUs under ownership apex
    Jurisdiction { code },              // All in jurisdiction
    EntityNeighborhood { entity_id, hops }, // N-hop neighborhood
    Custom { description },             // Free-form
}
```

### 2.2 Load Strategies (LoadStatus enum)

```rust
pub enum LoadStatus {
    Full,                   // All data in memory (< 1000 entities)
    SummaryOnly {           // Expand on demand
        expandable_nodes: Vec<ExpandableNode>
    },
    Windowed {              // Data around focal point
        center_entity_id: Uuid,
        loaded_hops: u32,
        total_reachable: usize,
    },
}
```

### 2.3 Scope Selection Flow

```
User: "view.cbu :cbu-id @alpha :mode trading"
        │
        ▼
ViewCbuOp.execute()
        │
        ├── Build TaxonomyContext::CbuTrading { cbu_id }
        ├── TaxonomyBuilder.build(pool) → TaxonomyNode
        ├── ViewState::from_taxonomy(taxonomy, context)
        │
        ▼
ViewState {
    stack: TaxonomyStack,      // Fractal navigation history
    taxonomy: TaxonomyNode,    // Current tree
    context: TaxonomyContext,  // What built this
    selection: Vec<Uuid>,      // The "those" after refinements
    ...
}
```

### 2.4 GAPS IN SCOPE HANDLING

| Gap | Description | Impact |
|-----|-------------|--------|
| **Scope ↔ ViewMode disconnect** | ViewState uses TaxonomyContext, LayoutEngine uses ViewMode enum | Mismatch between scope and rendering |
| **No scope persistence** | Scope is ephemeral per request | Session loses scope on reconnect |
| **Book scope incomplete** | `load_book_graph` in EntityGraph but CbuGraphBuilder doesn't support it | Can't view entire ownership book |
| **Windowed loading stub** | LoadStatus::Windowed defined but no actual windowed loading | Large datasets fail |

---

## 3. SELECTION & FILTERING ANALYSIS

### 3.1 Filter Types (GraphFilters)

```rust
pub struct GraphFilters {
    pub prong: ProngFilter,              // Ownership/Control/Both
    pub jurisdictions: Option<Vec<String>>,
    pub fund_types: Option<Vec<String>>,
    pub entity_types: Option<Vec<EntityType>>,
    pub as_of_date: NaiveDate,           // Temporal filter
    pub min_ownership_pct: Option<Decimal>,
    pub path_only: bool,                 // Show only path to cursor
}
```

### 3.2 Refinement System (ViewState)

```rust
pub enum Refinement {
    Include { filter: Filter },  // Keep only matching
    Exclude { filter: Filter },  // Remove matching
    Add { ids: Vec<Uuid> },      // Add specific IDs
    Remove { ids: Vec<Uuid> },   // Remove specific IDs
}
```

### 3.3 Selection Flow

```
1. ViewState::from_taxonomy() → selection = taxonomy.all_ids()
2. User: "except under 100M"
3. ViewState::refine(Refinement::Exclude { filter })
4. recompute_selection() → selection.retain(|id| !filter.matches(node))
5. selection now excludes small funds
```

### 3.4 GAPS IN FILTERING

| Gap | Description | Impact |
|-----|-------------|--------|
| **Filter taxonomy mismatch** | ViewState uses Filter enum, GraphFilters uses different structure | Two filter systems don't interop |
| **No role category filter** | GraphFilters lacks RoleCategory filter | Can't filter by OwnershipChain vs TradingExecution |
| **Filter not applied to layout** | LayoutEngine ignores filters, positions all nodes | Hidden nodes still take space |
| **No filter UI binding** | No DSL verb to add/remove filters dynamically | User must restart scope |
| **Prong filter incomplete** | ProngFilter exists but edge visibility not fully wired | Control edges always visible |

---

## 4. VIEW MODES: KYC/UBO vs TRADING/CBU

### 4.1 View Modes Defined

```rust
pub enum ViewMode {
    KycUbo,         // Entity hierarchy by role, ownership chains
    UboOnly,        // Pure ownership/control - no roles, no products
    ProductsOnly,   // CBU → Products only
    ServiceDelivery,// Products → Services → Resources
    Trading,        // CBU as container with trading entities
}
```

### 4.2 Role Category → Layout Behavior Mapping

```rust
impl RoleCategory {
    pub fn layout_behavior(&self) -> LayoutBehavior {
        match self {
            OwnershipChain | OwnershipControl => PyramidUp,
            ControlChain => Overlay,
            FundStructure | InvestmentVehicle => TreeDown,
            FundManagement => Satellite,
            TrustRoles => Radial,
            ServiceProvider => FlatBottom,
            TradingExecution | FundOperations | Distribution => FlatRight,
            InvestorChain | Financing => PyramidDown,
            RelatedParty => Peripheral,
            Both => Overlay,
        }
    }
}
```

### 4.3 Current Tier Layout (KYC/UBO Vertical)

```
Tier 0: CBU (center)
Tier 1: Products (center-right)
Tier 2: PyramidUp/Down (ownership/control) - SHELL left, PERSON right
Tier 3: TreeDown/Overlay (fund structure, service providers)
Tier 4: Satellite/Radial (trading, trust roles)
Tier 5: FlatBottom/FlatRight (investor chain)
Tier 6: Peripheral (related parties)
```

### 4.4 CRITICAL PROBLEMS WITH VIEW MODE RENDERING

| Problem | Description | Severity |
|---------|-------------|----------|
| **Role-based tier placement is naive** | All PyramidUp nodes in Tier 2, regardless of depth | HIGH |
| **No ownership depth consideration** | UBO at depth 4 placed same tier as direct shareholder | HIGH |
| **SHELL/PERSON split too rigid** | Forces left/right split even when not appropriate | MEDIUM |
| **No actual relationship routing** | Edges drawn as straight lines between tier positions | HIGH |
| **Trading view incomplete** | Just a grid of entities, no semantic layout | MEDIUM |
| **InvestmentVehicle role not wired** | RoleCategory exists but not in builder | MEDIUM |

### 4.5 What KYC/UBO View SHOULD Show

```
                    ┌─────────────────┐
                    │   Natural       │  ← Terminus (depth 0)
                    │   Person UBO    │
                    └────────┬────────┘
                             │ 75%
                    ┌────────┴────────┐
                    │  Holding Co A   │  ← Intermediate (depth 1)
                    │  (Luxembourg)   │
                    └────────┬────────┘
                             │ 100%
                    ┌────────┴────────┐
                    │  Holding Co B   │  ← Intermediate (depth 2)
                    │  (Cayman)       │
                    └────────┬────────┘
                             │ 100%
            ┌────────────────┼────────────────┐
            │                │                │
    ┌───────┴───────┐ ┌──────┴──────┐ ┌──────┴──────┐
    │   Fund A      │ │   Fund B    │ │   Fund C    │  ← CBU subjects
    └───────────────┘ └─────────────┘ └─────────────┘
            │                │                │
            └────────────────┼────────────────┘
                             │
                    ┌────────┴────────┐
                    │       CBU       │  ← The client
                    └─────────────────┘
```

**Current layout places ALL of these in Tier 2** because they all have `OwnershipChain` role category.

### 4.6 What Trading/CBU View SHOULD Show

```
                    ┌─────────────────────────────────────┐
                    │              CBU CONTAINER          │
                    │                                     │
                    │   ┌─────────┐    ┌─────────┐       │
                    │   │ Asset   │───▶│ Invest  │       │
                    │   │ Owner   │    │ Manager │       │
                    │   └────┬────┘    └────┬────┘       │
                    │        │              │             │
                    │        ▼              ▼             │
                    │   ┌─────────────────────────┐      │
                    │   │    Trading Matrix       │      │
                    │   │  ┌─────┬─────┬─────┐   │      │
                    │   │  │ EQ  │ FI  │ FX  │   │      │
                    │   │  └─────┴─────┴─────┘   │      │
                    │   └─────────────────────────┘      │
                    │                                     │
                    │   ┌─────────┐    ┌─────────┐       │
                    │   │ Prime   │    │ Custod- │       │
                    │   │ Broker  │    │ ian     │       │
                    │   └─────────┘    └─────────┘       │
                    │                                     │
                    └─────────────────────────────────────┘
                                     │
                    ┌────────────────┼────────────────┐
                    ▼                ▼                ▼
            ┌─────────────┐  ┌─────────────┐  ┌─────────────┐
            │  ETF Pool   │  │  Unit Trust │  │  OEIC       │
            │  (external) │  │  (external) │  │  (external) │
            └─────────────┘  └─────────────┘  └─────────────┘
            InvestmentVehicle - pooled funds AO invests in
```

---

## 5. AUTO-LAYOUT ENGINE ANALYSIS

### 5.1 Current Layout Algorithm

```rust
fn layout_kyc_ubo_vertical(&self, graph: &mut CbuGraph) {
    // 1. Categorize nodes by LayoutBehavior
    for node in graph.nodes {
        match get_layout_behavior(node) {
            PyramidUp | PyramidDown => tier_2.push(node),
            TreeDown | Overlay => tier_3.push(node),
            Satellite | Radial => tier_4.push(node),
            FlatBottom | FlatRight => tier_5.push(node),
            Peripheral => tier_6.push(node),
        }
    }
    
    // 2. Split each tier by SHELL/PERSON
    for tier in [tier_2, tier_3, tier_4, tier_5] {
        tier_shell = tier.filter(is_shell);
        tier_person = tier.filter(is_person);
    }
    
    // 3. Position: shells left, persons right
    layout_tier_left(&tier_2_shell, 2, y);
    layout_tier_right(&tier_2_person, 2, y);
    // ...
}
```

### 5.2 Layout Helper Functions

```rust
fn layout_tier_left(nodes, tier, y) {
    start_x = SHELL_MARGIN_LEFT;  // 100.0
    for (i, node) in nodes.enumerate() {
        node.x = start_x + i * NODE_SPACING_X;
        node.y = y;
    }
}

fn layout_tier_right(nodes, tier, y) {
    total_width = nodes.len() * NODE_SPACING_X;
    start_x = max(CANVAS_WIDTH - MARGIN - total_width, CANVAS_WIDTH / 2);
    for (i, node) in nodes.enumerate() {
        node.x = start_x + i * NODE_SPACING_X;
        node.y = y;
    }
}
```

### 5.3 LAYOUT ENGINE DEFICIENCIES

| Deficiency | Description | Required Fix |
|------------|-------------|--------------|
| **No hierarchy awareness** | Doesn't follow ownership/control edges | Need tree layout algorithm |
| **No edge routing** | Straight lines cause overlaps | Need edge bundling/orthogonal routing |
| **Fixed tier spacing** | 120px between all tiers | Need dynamic spacing based on content |
| **No grouping/containers** | Can't group subfunds under umbrella | Need compound node support |
| **No force-directed option** | Only tier-based layout | Add force-directed for exploration |
| **No semantic zoom** | Same detail at all zoom levels | Collapse nodes at distance |
| **No layout caching** | Recomputed every request | Cache layouts, invalidate on change |
| **Canvas size hardcoded** | 1200px width assumed | Need responsive canvas |

---

## 6. VIEWPORT & ZOOM ANALYSIS

### 6.1 Current Viewport System

```rust
pub struct ViewportContext {
    pub zoom: f32,                      // 0.1 - 2.0
    pub zoom_name: ZoomName,            // Overview/Standard/Detail
    pub pan_offset: (f32, f32),         // Pixels from center
    pub canvas_size: (f32, f32),        // Canvas dimensions
    pub visible_entities: HashSet<Uuid>,// Currently visible
    pub off_screen: OffScreenSummary,   // What's not visible
    pub is_default: bool,               // Has user adjusted?
}

pub enum ZoomName {
    Overview,   // 0.1 - 0.3: See entire structure
    Standard,   // 0.3 - 0.7: Normal working view
    Detail,     // 0.7 - 2.0: Close-up with all labels
}
```

### 6.2 Viewport Commands

```rust
impl ViewportContext {
    fn pan(&mut self, direction: PanDirection, amount: f32);
    fn zoom_in(&mut self);      // * 1.25, max 2.0
    fn zoom_out(&mut self);     // / 1.25, min 0.1
    fn fit_all(&mut self);      // Reset to 0.25, (0,0)
    fn center_on(&mut self, x: f32, y: f32);
    fn center_on_entity(&mut self, entity_id, graph);
}
```

### 6.3 VIEWPORT GAPS

| Gap | Description | Impact |
|-----|-------------|--------|
| **No LOD (Level of Detail)** | Same rendering at all zoom levels | Cluttered at overview zoom |
| **No semantic zoom** | Zoom doesn't affect what's visible | Just scales pixels |
| **No fit-to-selection** | Can only fit all | Can't zoom to filtered subset |
| **Off-screen hints weak** | Just counts, no navigation | User can't quickly navigate to off-screen |
| **No animation** | Zoom/pan is instant | Jarring UX |
| **Viewport not persisted** | Lost on reload | User must re-navigate each time |

---

## 7. PROPOSED ARCHITECTURE CHANGES

### 7.1 Unified Graph System

**Action**: Migrate from `CbuGraph` to `EntityGraph` completely.

```rust
// BEFORE: Two systems
pub type CbuGraph = LegacyCbuGraph;  // layout.rs uses this
pub struct EntityGraph { ... }        // types.rs defines this

// AFTER: One system
pub struct EntityGraph {
    // Unified structure with all capabilities
}

// layout.rs updated to use EntityGraph
impl LayoutEngine {
    pub fn layout(&self, graph: &mut EntityGraph) { ... }
}
```

### 7.2 Hierarchical Layout Algorithm

**Replace tier-based with proper tree layout**:

```rust
pub enum LayoutAlgorithm {
    /// Tier-based (current) - for flat views
    Tiered { config: TieredConfig },
    
    /// Tree layout (new) - for ownership chains
    Tree { 
        root_at: TreeRoot,          // Top, Bottom, Left, Right
        sibling_spacing: f32,
        level_spacing: f32,
        compact: bool,              // Minimize width
    },
    
    /// Force-directed (new) - for exploration
    Force {
        repulsion: f32,
        attraction: f32,
        iterations: u32,
        constrain_to_roles: bool,   // Keep role groups together
    },
    
    /// Radial (new) - for trust structures
    Radial {
        center_entity: Uuid,
        ring_spacing: f32,
    },
    
    /// Sugiyama/DAG (new) - for complex graphs
    Sugiyama {
        layer_spacing: f32,
        node_spacing: f32,
        minimize_crossings: bool,
    },
}
```

### 7.3 View Mode → Layout Algorithm Mapping

```rust
impl ViewMode {
    pub fn default_algorithm(&self) -> LayoutAlgorithm {
        match self {
            ViewMode::KycUbo | ViewMode::UboOnly => LayoutAlgorithm::Tree {
                root_at: TreeRoot::Top,       // UBOs at top
                sibling_spacing: 160.0,
                level_spacing: 120.0,
                compact: false,
            },
            ViewMode::Trading => LayoutAlgorithm::Tiered {
                config: TieredConfig::trading_default(),
            },
            ViewMode::FundStructure => LayoutAlgorithm::Tree {
                root_at: TreeRoot::Top,       // Umbrella at top
                sibling_spacing: 180.0,
                level_spacing: 100.0,
                compact: true,
            },
            ViewMode::ServiceDelivery => LayoutAlgorithm::Sugiyama {
                layer_spacing: 100.0,
                node_spacing: 150.0,
                minimize_crossings: true,
            },
        }
    }
}
```

### 7.4 Edge Routing System

```rust
pub struct EdgeRouter {
    pub algorithm: EdgeRoutingAlgorithm,
}

pub enum EdgeRoutingAlgorithm {
    /// Straight lines (current)
    Direct,
    
    /// Right-angle paths avoiding nodes
    Orthogonal {
        bend_penalty: f32,
        crossing_penalty: f32,
    },
    
    /// Curved Bezier paths
    Curved {
        curvature: f32,
    },
    
    /// Bundle edges with similar sources/targets
    Bundled {
        bundle_strength: f32,
    },
}

impl EdgeRouter {
    pub fn route(&self, graph: &EntityGraph) -> Vec<EdgePath> {
        // Returns paths for each edge, not just start/end points
    }
}

pub struct EdgePath {
    pub edge_id: Uuid,
    pub points: Vec<(f32, f32)>,  // Control points
    pub path_type: PathType,      // Line, Quadratic, Cubic
}
```

### 7.5 Semantic Zoom System

```rust
pub struct SemanticZoomLevel {
    pub min_zoom: f32,
    pub max_zoom: f32,
    pub visible_node_types: Vec<NodeType>,
    pub visible_edge_types: Vec<EdgeType>,
    pub label_detail: LabelDetail,
    pub collapse_threshold: usize,  // Collapse groups > N nodes
}

pub enum LabelDetail {
    None,           // No labels
    Name,           // Just name
    NameAndType,    // Name + entity type
    Full,           // All details
}

impl ViewportContext {
    pub fn get_semantic_level(&self) -> SemanticZoomLevel {
        match self.zoom_name {
            ZoomName::Overview => SemanticZoomLevel {
                visible_node_types: vec![Cbu, Entity],  // No products/services
                label_detail: LabelDetail::Name,
                collapse_threshold: 5,
            },
            ZoomName::Standard => SemanticZoomLevel {
                visible_node_types: vec![Cbu, Entity, Product],
                label_detail: LabelDetail::NameAndType,
                collapse_threshold: 15,
            },
            ZoomName::Detail => SemanticZoomLevel {
                visible_node_types: all_types(),
                label_detail: LabelDetail::Full,
                collapse_threshold: 100,
            },
        }
    }
}
```

### 7.6 Compound Node / Grouping

```rust
pub struct CompoundNode {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,      // For nesting
    pub children: Vec<Uuid>,          // Child node IDs
    pub group_type: GroupType,
    pub layout: GroupLayout,
    pub collapsed: bool,
    
    // Bounds computed from children
    pub bounds: Option<Rect>,
}

pub enum GroupType {
    UmbrellaFund { umbrella_id: Uuid },
    Jurisdiction { code: String },
    RoleCategory { category: RoleCategory },
    OwnershipTier { depth: u32 },
    Custom { label: String },
}

pub enum GroupLayout {
    /// Children stacked vertically
    Vertical,
    /// Children in a row
    Horizontal,
    /// Children in a grid
    Grid { columns: usize },
    /// Children placed by their own positions
    Free,
}
```

---

## 8. DSL VERB ADDITIONS

### 8.1 View Control Verbs

```yaml
# verbs/view.yaml additions

view.set-mode:
  domain: view
  description: "Change view mode for current scope"
  behavior: PLUGIN
  arguments:
    - name: mode
      type: STRING
      required: true
      values: [KYC_UBO, UBO_ONLY, TRADING, FUND_STRUCTURE, SERVICE_DELIVERY]
  output: ViewOpResult

view.set-layout:
  domain: view
  description: "Change layout algorithm"
  behavior: PLUGIN
  arguments:
    - name: algorithm
      type: STRING
      required: true
      values: [TIERED, TREE, FORCE, RADIAL, SUGIYAMA]
    - name: config
      type: JSON
      required: false
  output: LayoutResult

view.set-orientation:
  domain: view
  description: "Change layout orientation"
  behavior: PLUGIN
  arguments:
    - name: orientation
      type: STRING
      required: true
      values: [VERTICAL, HORIZONTAL]
  output: ViewOpResult
```

### 8.2 Filter Verbs

```yaml
# verbs/view.yaml additions

view.filter-add:
  domain: view
  description: "Add a filter refinement"
  behavior: PLUGIN
  arguments:
    - name: filter-type
      type: STRING
      required: true
      values: [JURISDICTION, STATUS, FUND_TYPE, ROLE_CATEGORY, MIN_OWNERSHIP]
    - name: values
      type: LIST
      required: true
  output: ViewOpResult

view.filter-remove:
  domain: view
  description: "Remove a filter by type"
  behavior: PLUGIN
  arguments:
    - name: filter-type
      type: STRING
      required: true
  output: ViewOpResult

view.filter-clear:
  domain: view
  description: "Clear all filters"
  behavior: PLUGIN
  output: ViewOpResult
```

### 8.3 Navigation Verbs

```yaml
# verbs/view.yaml additions

view.focus:
  domain: view
  description: "Focus on a specific entity, centering viewport"
  behavior: PLUGIN
  arguments:
    - name: entity-id
      type: UUID
      required: true
    - name: expand-depth
      type: INTEGER
      required: false
      default: 2
  output: ViewOpResult

view.expand:
  domain: view
  description: "Expand a collapsed group or entity"
  behavior: PLUGIN
  arguments:
    - name: node-id
      type: UUID
      required: true
  output: ViewOpResult

view.collapse:
  domain: view
  description: "Collapse a node into a summary"
  behavior: PLUGIN
  arguments:
    - name: node-id
      type: UUID
      required: true
  output: ViewOpResult

view.zoom-to-fit:
  domain: view
  description: "Zoom viewport to fit current selection or all"
  behavior: PLUGIN
  arguments:
    - name: target
      type: STRING
      required: false
      values: [ALL, SELECTION, FILTERED]
      default: ALL
  output: ViewportState
```

---

## 9. IMPLEMENTATION PLAN

### Phase 1: Unify Graph Types (2 days)

1. **Migrate LayoutEngine to EntityGraph**
   - Update `layout.rs` to accept `&mut EntityGraph`
   - Map LegacyGraphNode fields to EntityGraph fields
   - Remove CbuGraph/LegacyGraphNode after migration

2. **Update CbuGraphBuilder**
   - Output EntityGraph instead of CbuGraph
   - Wire up typed edges (OwnershipEdge, ControlEdge, etc.)
   - Populate `depth_from_terminus` during build

3. **Update GraphQueryEngine**
   - Use EntityGraph throughout
   - Return EntityGraph from queries

### Phase 2: Tree Layout for UBO View (3 days)

1. **Implement TreeLayoutEngine**
   - Walk ownership edges to build tree structure
   - Handle cycles (mark as "complex structure")
   - Compute positions top-down (UBOs at apex)

2. **Handle Multi-Parent Nodes**
   - Entity owned by multiple parents
   - Options: duplicate node, draw to first parent + crosslinks, or use DAG layout

3. **Integrate with ViewMode**
   - `ViewMode::UboOnly` uses TreeLayoutEngine
   - `ViewMode::KycUbo` uses TreeLayoutEngine + role overlays

### Phase 3: Edge Routing (2 days)

1. **Implement OrthogonalRouter**
   - A* pathfinding avoiding node rectangles
   - Minimize bends and crossings

2. **Edge Path Representation**
   - Return control points, not just endpoints
   - Client renders paths (lines, beziers)

3. **Edge Bundling (optional)**
   - Group edges with common source/target regions

### Phase 4: Semantic Zoom (2 days)

1. **Define Zoom Levels**
   - Overview, Standard, Detail configurations
   - Node visibility rules per level

2. **Implement Collapse Logic**
   - Auto-collapse groups at zoom-out
   - Expand on zoom-in or click

3. **Label LOD**
   - No labels at overview
   - Short labels at standard
   - Full details at detail

### Phase 5: Grouping / Compound Nodes (2 days)

1. **Implement CompoundNode**
   - Parent-child relationships
   - Bounds computation from children

2. **Umbrella Fund Grouping**
   - Auto-group subfunds under umbrella
   - Jurisdiction grouping option

3. **Manual Grouping**
   - DSL verb to create custom groups
   - Persist group definitions

### Phase 6: Filter Integration (1 day)

1. **Wire Filters to Layout**
   - Filtered-out nodes excluded from layout
   - Re-layout on filter change

2. **Filter DSL Verbs**
   - `view.filter-add`, `view.filter-remove`, `view.filter-clear`

3. **Filter Persistence**
   - Store active filters in session
   - Restore on reconnect

---

## 10. VALIDATION CRITERIA

### Layout Quality Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Node overlaps | 0 | Count of overlapping node rectangles |
| Edge crossings | < 10% of edges | Count of edge intersections |
| Aspect ratio | 0.5 - 2.0 | Canvas width / height |
| Edge length variance | < 50% | StdDev / Mean of edge lengths |
| Parent-child alignment | 80%+ | Children centered under parents |

### Performance Targets

| Operation | Target Latency | Max Graph Size |
|-----------|----------------|----------------|
| Initial layout | < 500ms | 1000 nodes |
| Filter update | < 100ms | 1000 nodes |
| Zoom/pan | < 16ms (60 FPS) | Any |
| Edge routing | < 1s | 500 edges |

### UX Criteria

- [ ] UBO view shows clear ownership hierarchy from apex to CBU
- [ ] Trading view shows CBU as container with trading roles inside
- [ ] User can navigate from UBO view to Trading view without losing context
- [ ] Zoom to overview shows entire structure, labels readable
- [ ] Zoom to detail shows all attributes, no overlaps
- [ ] Filtering reduces visual clutter, layout adjusts
- [ ] Back/forward navigation works intuitively

---

## 11. APPENDIX: FILE LOCATIONS

| File | Purpose | Lines |
|------|---------|-------|
| `rust/src/graph/types.rs` | All graph types (EntityGraph + Legacy) | ~2500 |
| `rust/src/graph/layout.rs` | LayoutEngine implementation | ~880 |
| `rust/src/graph/filters.rs` | GraphFilterOps trait + FilterBuilder | ~450 |
| `rust/src/graph/viewport.rs` | ViewportContext, zoom handling | ~440 |
| `rust/src/graph/view_model.rs` | GraphViewModel for DSL output | ~650 |
| `rust/src/graph/builder.rs` | CbuGraphBuilder from DB | ~1100 |
| `rust/src/session/view_state.rs` | ViewState, Refinement, selection | ~800 |
| `rust/src/session/scope.rs` | SessionScope, LoadStatus | ~380 |
| `rust/src/dsl_v2/custom_ops/view_ops.rs` | view.* verb handlers | ~1160 |

---

## 12. RELATED TODOS

- [ ] TODO_VERB_CONTRACT_LAYER.md - Verb execution contracts
- [ ] TODO_GHOST_PERSON_STATE.md - Ghost entity handling
- [ ] TODO_DOCUMENT_ATTRIBUTE_CATALOGUE_COMPLETION.md - Data gaps
- [ ] TODO_INVESTMENT_VEHICLE_ROLE.md - Pooled fund role (not yet created)

---

**Next Action**: Begin Phase 1 - Unify Graph Types
