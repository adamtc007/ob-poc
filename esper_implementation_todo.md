# Esper Navigation System — Implementation TODO

**Reference**: `esper_navigation_architecture_v1.0.md`

**Approach**: Rip and replace. This is a clean-room implementation against the v1.0 spec.
Delete any existing `esper_*` code and start fresh.

---

## Pre-Flight Checklist

```bash
# Create workspace structure
mkdir -p esper/crates/{esper_snapshot,esper_core,esper_input,esper_compiler,esper_policy,esper_egui}
mkdir -p esper/config/policies

# Initialize workspace Cargo.toml
cat > esper/Cargo.toml << 'EOF'
[workspace]
resolver = "2"
members = [
    "crates/esper_snapshot",
    "crates/esper_core",
    "crates/esper_input",
    "crates/esper_compiler",
    "crates/esper_policy",
    "crates/esper_egui",
]

[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }
bitflags = "2.4"
rayon = "1.8"
hashbrown = "0.14"
egui = "0.27"
EOF
```

---

## Phase 1: esper_snapshot (Wire Format & ABI)

**Goal**: Define the shared types that both server (compiler) and client (egui) consume.
This is the contract from Section 6.

### 1.1 Sentinel Constants

```rust
// crates/esper_snapshot/src/lib.rs

/// Navigation index sentinel: no link
pub const NONE_IDX: u32 = u32::MAX;

/// Entity ID sentinel: no detail ref
pub const NONE_ID: u64 = 0;
```

### 1.2 Core Snapshot Types

```rust
// crates/esper_snapshot/src/types.rs

// TODO: Implement these structs per Section 5.3 and 6.1

pub struct SnapshotEnvelope {
    pub schema_version: u32,
    pub source_hash: u64,
    pub policy_hash: u64,
    pub created_at: u64,
    pub cbu_id: u64,
}

pub struct WorldSnapshot {
    pub envelope: SnapshotEnvelope,
    pub string_table: Vec<String>,
    pub chambers: Vec<ChamberSnapshot>,
}

pub struct ChamberSnapshot {
    pub id: u32,
    pub kind: ChamberKind,
    pub bounds: Rect,
    pub default_camera: CameraPreset,

    // SoA entity data — all Vec<T> must have same length N
    pub entity_ids: Vec<u64>,
    pub kind_ids: Vec<u16>,
    pub x: Vec<f32>,
    pub y: Vec<f32>,
    pub label_ids: Vec<u32>,
    pub detail_refs: Vec<u64>,

    // Navigation indices — use NONE_IDX for no link
    pub first_child: Vec<u32>,
    pub next_sibling: Vec<u32>,
    pub prev_sibling: Vec<u32>,

    pub doors: Vec<DoorSnapshot>,
    pub grid: GridSnapshot,
}

pub struct DoorSnapshot {
    pub id: u32,
    pub from_entity_idx: u32,
    pub target_chamber_id: u32,       // stable ID, not index
    pub target_entity_id: Option<u64>, // focus-on-arrival
    pub door_kind: DoorKind,
    pub label_id: u32,
    pub position: Vec2,
}

pub struct GridSnapshot {
    pub cell_size: f32,
    pub origin: Vec2,
    pub dims: (u32, u32),
    pub cell_ranges: Vec<(u32, u32)>,      // (start, count) per cell
    pub cell_entity_indices: Vec<u32>,     // chamber-local indices
}
```

### 1.3 Validation

```rust
// crates/esper_snapshot/src/validate.rs

// TODO: Implement validation per Section 6.3

impl ChamberSnapshot {
    /// Validate SoA invariants. Panics in debug, returns Result in release.
    pub fn validate(&self) -> Result<(), SnapshotError> {
        let n = self.entity_ids.len();
        
        // Parallel array length check
        assert_eq!(self.kind_ids.len(), n);
        assert_eq!(self.x.len(), n);
        assert_eq!(self.y.len(), n);
        assert_eq!(self.label_ids.len(), n);
        assert_eq!(self.detail_refs.len(), n);
        assert_eq!(self.first_child.len(), n);
        assert_eq!(self.next_sibling.len(), n);
        assert_eq!(self.prev_sibling.len(), n);
        
        // Index domain check
        for &idx in &self.first_child {
            if idx != NONE_IDX && idx as usize >= n {
                return Err(SnapshotError::IndexOutOfBounds);
            }
        }
        // ... similar for next_sibling, prev_sibling
        
        // Entity ID invariant: all > 0
        for &id in &self.entity_ids {
            if id == NONE_ID {
                return Err(SnapshotError::InvalidEntityId);
            }
        }
        
        Ok(())
    }
}
```

### 1.4 Grid Query

```rust
// crates/esper_snapshot/src/grid.rs

// TODO: Implement per Section 5.4

impl GridSnapshot {
    pub fn query_visible(&self, viewport: Rect) -> impl Iterator<Item = u32> + '_ {
        // Convert viewport to cell range
        // Iterate cells, yield entity indices
        todo!()
    }
    
    fn world_to_cell(&self, pos: Vec2) -> (u32, u32) {
        todo!()
    }
}
```

### 1.5 Serialization

```rust
// crates/esper_snapshot/src/serde.rs

// TODO: Implement deterministic serialization
// Consider: bincode, rkyv, or custom for WASM compatibility

impl WorldSnapshot {
    pub fn serialize(&self) -> Vec<u8> { todo!() }
    pub fn deserialize(bytes: &[u8]) -> Result<Self, SnapshotError> { todo!() }
}
```

**Deliverables**:
- [ ] `esper_snapshot/src/lib.rs` — re-exports
- [ ] `esper_snapshot/src/types.rs` — all snapshot structs
- [ ] `esper_snapshot/src/validate.rs` — SoA invariant checks
- [ ] `esper_snapshot/src/grid.rs` — spatial query
- [ ] `esper_snapshot/src/serde.rs` — serialization
- [ ] Unit tests for validation and grid query

---

## Phase 2: esper_core (Navigation Engine)

**Goal**: Implement the stack machine, verbs, state, and effects per Sections 4, 7, 10, 11.

### 2.1 Command Tokens

```rust
// crates/esper_core/src/token.rs

// TODO: Implement per Section 4.1

pub type VerbId = u16;
pub type EntityId = u64;
pub type DoorId = u32;
pub type ChamberId = u32;

pub enum CommandToken {
    Verb(VerbId),
    Macro(MacroId),
    Literal(f32),
    Entity(EntityId),
    Door(DoorId),
}
```

### 2.2 Verbs

```rust
// crates/esper_core/src/verb.rs

// TODO: Implement per Section 4.3

#[derive(Clone, Copy, Debug)]
pub enum Verb {
    // Spatial
    DiveInto(DoorId),
    PullBack,
    Surface,
    PanTo { x: f32, y: f32 },
    Zoom(f32),
    Enhance,
    Track(EntityId),
    Focus(EntityId),
    
    // Structural
    Ascend,
    Descend,
    DescendTo(NodeId),
    Next,
    Prev,
    First,
    Last,
    Expand,
    Collapse,
    Select(NodeId),
    Preview(NodeId),
    ClearPreview,
    Root,
}
```

### 2.3 Effect Flags

```rust
// crates/esper_core/src/effect.rs

// TODO: Implement per Section 11.1

use bitflags::bitflags;

bitflags! {
    #[derive(Clone, Copy, Debug)]
    pub struct EffectSet: u16 {
        const NONE                = 0;
        const CAMERA_CHANGED      = 1 << 0;
        const LOD_MODE_RESET      = 1 << 1;
        const CHAMBER_CHANGED     = 1 << 3;
        const MODE_CHANGED        = 1 << 4;
        const SNAP_TRANSITION     = 1 << 5;
        const TAXONOMY_CHANGED    = 1 << 6;
        const PREFETCH_DETAILS    = 1 << 7;
        const SCROLL_ADJUST       = 1 << 8;
        const PHASE_RESET         = 1 << 9;
        const PREVIEW_SET         = 1 << 10;
        const PREVIEW_CLEAR       = 1 << 11;
        const CONTEXT_PUSHED      = 1 << 12;
        const CONTEXT_POPPED      = 1 << 13;
    }
}
```

### 2.4 Drone State

```rust
// crates/esper_core/src/state.rs

// TODO: Implement per Sections 4.2 and 7.1

pub struct DroneState {
    pub mode: NavigationMode,
    pub current_chamber: ChamberId,
    pub context_stack: Vec<ContextFrame>,
    pub camera: CameraState,
    pub taxonomy: TaxonomyState,
    pub lod_state: LodState,
}

pub struct ContextFrame {
    pub chamber_id: ChamberId,
    pub camera: CameraState,
}

pub struct CameraState {
    pub target: Vec2,
    pub current: Vec2,  // for lerp
    pub zoom: f32,
}

pub struct TaxonomyState {
    pub focus_path: Vec<NodeId>,
    pub current_depth: usize,
    pub selection: Option<NodeId>,
    pub expanded: HashSet<NodeId>,
    pub scroll_offset: f32,
    pub phase: NavigationPhase,
    pub last_nav_tick: u64,
    pub focus_t: f32,
    pub preview_target: Option<NodeId>,
}

pub enum NavigationMode {
    Spatial,
    Structural,
}
```

### 2.5 Navigation Phase (Fly-Over)

```rust
// crates/esper_core/src/phase.rs

// TODO: Implement per Section 10.1

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NavigationPhase {
    Moving,
    Settling,
    Focused,
}

impl NavigationPhase {
    pub fn update(&mut self, tick: u64, last_nav: u64, dwell_ticks: u64) {
        let elapsed = tick.saturating_sub(last_nav);
        *self = if elapsed < dwell_ticks {
            NavigationPhase::Moving
        } else if elapsed < dwell_ticks + SETTLE_TICKS {
            NavigationPhase::Settling
        } else {
            NavigationPhase::Focused
        };
    }
}
```

### 2.6 Verb Execution

```rust
// crates/esper_core/src/execute.rs

// TODO: Implement verb execution per Section 4.3

impl DroneState {
    pub fn execute(&mut self, verb: Verb, world: &WorldSnapshot) -> Result<EffectSet, Fault> {
        match verb {
            Verb::DiveInto(door_id) => self.dive_into(door_id, world),
            Verb::PullBack => self.pull_back(),
            Verb::Surface => self.surface(),
            Verb::Next => self.next_sibling(world),
            Verb::Prev => self.prev_sibling(world),
            Verb::Ascend => self.ascend(world),
            Verb::Descend => self.descend(world),
            // ... etc
        }
    }
    
    fn dive_into(&mut self, door_id: DoorId, world: &WorldSnapshot) -> Result<EffectSet, Fault> {
        // Find door, push context, switch chamber
        todo!()
    }
    
    fn next_sibling(&mut self, world: &WorldSnapshot) -> Result<EffectSet, Fault> {
        // Use next_sibling[] array for O(1) lookup
        todo!()
    }
}
```

### 2.7 Fault Model

```rust
// crates/esper_core/src/fault.rs

// TODO: Implement per Section 4.4

#[derive(Debug)]
pub enum Fault {
    StackUnderflow,
    StackOverflow,
    ContextStackOverflow,
    DoorNotFound(DoorId),
    ChamberNotFound(ChamberId),
    EntityNotFound(EntityId),
    CyclicReference(ChamberId),
    UnknownVerb,
    TypeMismatch { expected: TokenKind, got: TokenKind },
    NotAuthorized { chamber: ChamberId, required_role: RoleId },
    MaxDepthExceeded,
    TokenQueueOverflow,
}
```

**Deliverables**:
- [ ] `esper_core/src/token.rs`
- [ ] `esper_core/src/verb.rs`
- [ ] `esper_core/src/effect.rs`
- [ ] `esper_core/src/state.rs`
- [ ] `esper_core/src/phase.rs`
- [ ] `esper_core/src/execute.rs`
- [ ] `esper_core/src/fault.rs`
- [ ] `esper_core/src/stack.rs` — context stack operations
- [ ] Unit tests for each verb, especially navigation index lookups

---

## Phase 3: esper_input (Unified Input)

**Goal**: Implement gesture recognition and verb mapping per Section 9.

### 3.1 Gesture Types

```rust
// crates/esper_input/src/gesture.rs

// TODO: Implement per Section 9.3

pub enum Gesture {
    Click(NodeId),
    DoubleClick(NodeId),
    ClickDoor(DoorId),
    HoverEnter(NodeId),
    HoverDwell(NodeId),
    HoverExit(NodeId),
    DragStart(Vec2),
    DragMove { delta: Vec2, velocity: Vec2 },
    DragEnd(Vec2),
}
```

### 3.2 Gesture Recognizer

```rust
// crates/esper_input/src/recognizer.rs

// TODO: Implement gesture state machine

pub struct GestureRecognizer {
    hover_target: Option<NodeId>,
    hover_start_tick: u64,
    drag_state: Option<DragState>,
    last_click_tick: u64,
    last_click_target: Option<NodeId>,
}

impl GestureRecognizer {
    pub fn process(&mut self, event: RawInputEvent, tick: u64) -> Option<Gesture> {
        // State machine: track hover duration, detect double-click, etc.
        todo!()
    }
}
```

### 3.3 Gesture to Verb Mapping

```rust
// crates/esper_input/src/mapping.rs

// TODO: Implement per Section 9.4

pub fn gesture_to_verb(gesture: Gesture, state: &DroneState) -> Option<Verb> {
    match gesture {
        Gesture::Click(node_id) => match state.mode {
            NavigationMode::Spatial => Some(Verb::Focus(node_id)),
            NavigationMode::Structural => Some(Verb::Select(node_id)),
        },
        Gesture::DoubleClick(node_id) => match state.mode {
            NavigationMode::Spatial => find_door(node_id).map(Verb::DiveInto),
            NavigationMode::Structural => Some(Verb::DescendTo(node_id)),
        },
        Gesture::HoverDwell(node_id) => Some(Verb::Preview(node_id)),
        Gesture::HoverExit(_) => Some(Verb::ClearPreview),
        Gesture::DragMove { delta, .. } => Some(Verb::PanTo { x: delta.x, y: delta.y }),
        Gesture::ClickDoor(door_id) => Some(Verb::DiveInto(door_id)),
        _ => None,
    }
}
```

### 3.4 Keyboard Mapping

```rust
// crates/esper_input/src/keyboard.rs

// TODO: Implement per Section 9.5

pub fn key_to_verb(key: KeyCode, modifiers: Modifiers) -> Option<Verb> {
    match key {
        KeyCode::Right | KeyCode::J | KeyCode::Tab => Some(Verb::Next),
        KeyCode::Left | KeyCode::K => Some(Verb::Prev),
        KeyCode::Down | KeyCode::L => Some(Verb::Descend),
        KeyCode::Up | KeyCode::H | KeyCode::Escape => Some(Verb::Ascend),
        KeyCode::Enter => Some(Verb::Select(/* current */)),
        KeyCode::E => Some(Verb::Expand),
        KeyCode::Space => Some(Verb::Enhance),
        _ => None,
    }
}
```

### 3.5 Voice Integration (Stub)

```rust
// crates/esper_input/src/voice.rs

// TODO: Stub for Phase 8

pub struct VoiceCommandQueue {
    pending: Option<Verb>,
    cancelled: bool,
}

impl VoiceCommandQueue {
    pub fn queue(&mut self, verb: Verb) { self.pending = Some(verb); }
    pub fn cancel(&mut self) { self.pending = None; self.cancelled = true; }
    pub fn take(&mut self) -> Option<Verb> { self.pending.take() }
}
```

**Deliverables**:
- [ ] `esper_input/src/gesture.rs`
- [ ] `esper_input/src/recognizer.rs`
- [ ] `esper_input/src/mapping.rs`
- [ ] `esper_input/src/keyboard.rs`
- [ ] `esper_input/src/voice.rs` (stub)
- [ ] Unit tests for gesture recognition state machine

---

## Phase 4: esper_compiler (World Compiler)

**Goal**: Implement server-side compilation per Section 5 and 13.

### 4.1 Compilation Pipeline

```rust
// crates/esper_compiler/src/lib.rs

// TODO: Implement per Section 5.2

use rayon::prelude::*;

pub fn compile_world(
    ast: &Ast,
    policy: &RenderPolicy,
) -> Result<WorldSnapshot, CompileError> {
    // Stage 0: Already parsed (AST input)
    
    // Stage 1: Build canonical graph
    let graph = build_canonical_graph(ast)?;
    
    // Stage 2: Chamberization
    let chamber_defs = chamberize(&graph, policy)?;
    
    // Stage 3-4: Parallel string interning
    let (string_table, chamber_defs) = intern_strings_parallel(&chamber_defs);
    
    // Stage 5: Parallel chamber compilation
    let chambers: Vec<ChamberSnapshot> = chamber_defs
        .par_iter()
        .map(|def| compile_chamber(def, policy, &string_table))
        .collect::<Result<Vec<_>, _>>()?;
    
    // Build envelope
    let envelope = SnapshotEnvelope {
        schema_version: 1,
        source_hash: hash_ast(ast),
        policy_hash: hash_policy(policy),
        created_at: now_unix(),
        cbu_id: ast.cbu_id,
    };
    
    Ok(WorldSnapshot { envelope, string_table, chambers })
}
```

### 4.2 Canonical Graph

```rust
// crates/esper_compiler/src/graph.rs

// TODO: Implement per Section 5.2 Stage 1

pub struct CanonicalGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

pub fn build_canonical_graph(ast: &Ast) -> Result<CanonicalGraph, CompileError> {
    // Walk AST, create nodes and edges
    todo!()
}
```

### 4.3 Chamberization

```rust
// crates/esper_compiler/src/chamber.rs

// TODO: Implement per Section 5.2 Stage 2

pub struct ChamberDef {
    pub id: u32,
    pub kind: ChamberKind,
    pub entities: Vec<EntityDef>,
    pub doors: Vec<DoorDef>,
}

pub fn chamberize(
    graph: &CanonicalGraph,
    policy: &RenderPolicy,
) -> Result<Vec<ChamberDef>, CompileError> {
    // Apply boundary heuristics from policy
    // Split into chambers when threshold exceeded
    todo!()
}
```

### 4.4 Two-Pass String Interning

```rust
// crates/esper_compiler/src/intern.rs

// TODO: Implement per Section 13.3

use rayon::prelude::*;
use hashbrown::HashSet;

pub fn intern_strings_parallel(
    chambers: &[ChamberDef],
) -> (Vec<String>, Vec<ChamberDefWithIds>) {
    // Pass 1: Collect unique strings (parallel)
    let all_strings: HashSet<String> = chambers
        .par_iter()
        .flat_map(|c| c.entities.par_iter().map(|e| e.label.clone()))
        .collect();
    
    // Build index (sequential, fast)
    let table: Vec<String> = all_strings.into_iter().collect();
    let index: HashMap<&str, u32> = table
        .iter()
        .enumerate()
        .map(|(i, s)| (s.as_str(), i as u32))
        .collect();
    
    // Pass 2: Replace strings with IDs (parallel)
    let updated = chambers
        .par_iter()
        .map(|c| replace_with_ids(c, &index))
        .collect();
    
    (table, updated)
}
```

### 4.5 Layout

```rust
// crates/esper_compiler/src/layout.rs

// TODO: Implement per Section 5.2 Stage 3

pub fn layout_chamber(def: &ChamberDef, policy: &RenderPolicy) -> LayoutResult {
    match def.kind {
        ChamberKind::Grid => layout_grid(def, policy),
        ChamberKind::Tree => layout_tree(def, policy),
        ChamberKind::Force => layout_force(def, policy),  // expensive
    }
}

fn layout_grid(def: &ChamberDef, policy: &RenderPolicy) -> LayoutResult {
    // Simple grid layout
    todo!()
}

fn build_navigation_indices(layout: &LayoutResult) -> NavigationIndices {
    // Compute first_child, next_sibling, prev_sibling arrays
    todo!()
}

fn build_spatial_index(layout: &LayoutResult, cell_size: f32) -> GridSnapshot {
    // Build grid cells for spatial query
    todo!()
}
```

### 4.6 Emit Snapshot

```rust
// crates/esper_compiler/src/emit.rs

// TODO: Implement per Section 5.2 Stage 5

pub fn compile_chamber(
    def: &ChamberDefWithIds,
    policy: &RenderPolicy,
    string_table: &[String],
) -> Result<ChamberSnapshot, CompileError> {
    let layout = layout_chamber(def, policy);
    let nav_indices = build_navigation_indices(&layout);
    let grid = build_spatial_index(&layout, policy.cell_size);
    
    // Build SoA arrays
    let n = def.entities.len();
    let mut entity_ids = Vec::with_capacity(n);
    let mut kind_ids = Vec::with_capacity(n);
    let mut x = Vec::with_capacity(n);
    let mut y = Vec::with_capacity(n);
    let mut label_ids = Vec::with_capacity(n);
    let mut detail_refs = Vec::with_capacity(n);
    
    for (i, e) in def.entities.iter().enumerate() {
        entity_ids.push(e.id);
        kind_ids.push(e.kind_id);
        x.push(layout.positions[i].x);
        y.push(layout.positions[i].y);
        label_ids.push(e.label_id);
        detail_refs.push(e.detail_ref.unwrap_or(NONE_ID));
    }
    
    let snapshot = ChamberSnapshot {
        id: def.id,
        kind: def.kind,
        bounds: layout.bounds,
        default_camera: policy.default_camera_for(def.kind),
        entity_ids,
        kind_ids,
        x,
        y,
        label_ids,
        detail_refs,
        first_child: nav_indices.first_child,
        next_sibling: nav_indices.next_sibling,
        prev_sibling: nav_indices.prev_sibling,
        doors: compile_doors(&def.doors),
        grid,
    };
    
    snapshot.validate()?;
    Ok(snapshot)
}
```

**Deliverables**:
- [ ] `esper_compiler/src/lib.rs` — main compile_world
- [ ] `esper_compiler/src/graph.rs` — canonical graph
- [ ] `esper_compiler/src/chamber.rs` — chamberization
- [ ] `esper_compiler/src/intern.rs` — two-pass interning
- [ ] `esper_compiler/src/layout.rs` — grid/tree/force layout
- [ ] `esper_compiler/src/emit.rs` — snapshot emission
- [ ] Benchmark tests comparing 1-thread vs 8-thread compilation

---

## Phase 5: esper_policy (Policy Parser)

**Goal**: Implement policy parsing per Section 16.

### 5.1 Policy Types

```rust
// crates/esper_policy/src/types.rs

pub struct RenderPolicy {
    pub version: u32,
    pub kind_schema: Vec<KindDef>,
    pub spatial_lod: SpatialLodConfig,
    pub structural_lod: StructuralLodConfig,
    pub chamber_configs: HashMap<String, ChamberConfig>,
}

pub struct KindDef {
    pub id: u16,
    pub name: String,
}

pub struct SpatialLodConfig {
    pub tiers: Vec<LodTier>,
    pub budgets: LodBudgets,
}

pub struct LodTier {
    pub name: String,
    pub zoom_max: f32,
    pub hysteresis: f32,
}
```

### 5.2 S-Expression Parser

```rust
// crates/esper_policy/src/parser.rs

// TODO: Parse policy s-expressions

pub fn parse_policy(input: &str) -> Result<RenderPolicy, PolicyError> {
    // Use nom or a simple recursive descent parser
    todo!()
}
```

### 5.3 Validation and Hashing

```rust
// crates/esper_policy/src/validate.rs

impl RenderPolicy {
    pub fn validate(&self) -> Result<(), PolicyError> {
        // Check kind mappings complete
        // Check LOD tier ordering
        // Check budgets non-negative
        todo!()
    }
    
    pub fn compute_hash(&self) -> u64 {
        // Canonicalize then hash
        let canonical = self.canonicalize();
        hash_bytes(&canonical)
    }
}
```

### 5.4 Hot Reload (Dev)

```rust
// crates/esper_policy/src/watch.rs

// TODO: File watcher for dev hot reload

pub struct PolicyWatcher {
    path: PathBuf,
    last_hash: u64,
}

impl PolicyWatcher {
    pub fn check_reload(&mut self) -> Option<RenderPolicy> {
        // Check mtime, reload if changed, validate, return if hash differs
        todo!()
    }
}
```

**Deliverables**:
- [ ] `esper_policy/src/types.rs`
- [ ] `esper_policy/src/parser.rs`
- [ ] `esper_policy/src/validate.rs`
- [ ] `esper_policy/src/watch.rs`
- [ ] Sample policy file: `config/policies/default.sexpr`

---

## Phase 6: esper_egui (Rendering)

**Goal**: Implement the RIP loop per Section 15 and 8.

### 6.1 Label Cache

```rust
// crates/esper_egui/src/label_cache.rs

// TODO: Implement per Section 8.4

use hashbrown::HashMap;

#[derive(Hash, Eq, PartialEq)]
pub struct LabelCacheKey {
    pub string_id: u32,
    pub max_width: u16,  // quantized to 10px
    pub lod: LodTier,
}

pub struct LabelCache {
    cache: HashMap<LabelCacheKey, Arc<Galley>>,
    lru: VecDeque<LabelCacheKey>,
    max_entries: usize,
}

impl LabelCache {
    pub fn get_or_shape(
        &mut self,
        key: LabelCacheKey,
        text: &str,
        fonts: &Fonts,
    ) -> Arc<Galley> {
        if let Some(galley) = self.cache.get(&key) {
            return galley.clone();
        }
        
        // Shape text (expensive!)
        let galley = Arc::new(fonts.layout_no_wrap(text, FontId::default(), Color32::WHITE));
        
        // Insert with LRU eviction
        self.insert(key, galley.clone());
        galley
    }
}
```

### 6.2 Spatial Renderer

```rust
// crates/esper_egui/src/spatial.rs

// TODO: Implement spatial mode rendering

pub fn render_spatial(
    ui: &mut Ui,
    chamber: &ChamberSnapshot,
    state: &DroneState,
    label_cache: &mut LabelCache,
    string_table: &[String],
) {
    // 1. Query visible entities
    let viewport = compute_viewport(ui, &state.camera);
    let visible: Vec<u32> = chamber.grid.query_visible(viewport).collect();
    
    // 2. Determine LOD for each
    let lod = compute_lod(state.camera.zoom, &state.lod_state);
    
    // 3. Apply budget caps
    let (icons, labels, fulls) = apply_budgets(&visible, lod, &state.lod_state.budgets);
    
    // 4. Render icons (cheap)
    for idx in &icons {
        draw_icon(ui, chamber, *idx);
    }
    
    // 5. Render labels (cached shaping)
    if state.taxonomy.phase != NavigationPhase::Moving {
        for idx in &labels {
            let label_id = chamber.label_ids[*idx as usize];
            let text = &string_table[label_id as usize];
            let galley = label_cache.get_or_shape(/* ... */);
            draw_label(ui, chamber, *idx, &galley);
        }
    }
    
    // 6. Render full cards
    for idx in &fulls {
        draw_full_card(ui, chamber, *idx, label_cache, string_table);
    }
}
```

### 6.3 Structural Renderer

```rust
// crates/esper_egui/src/structural.rs

// TODO: Implement structural mode rendering per Section 7

pub fn render_structural(
    ui: &mut Ui,
    chamber: &ChamberSnapshot,
    state: &DroneState,
    label_cache: &mut LabelCache,
    string_table: &[String],
) {
    // Render breadcrumb
    render_breadcrumb(ui, &state.taxonomy.focus_path, string_table);
    
    // Render current level
    let focus_idx = state.taxonomy.selection;
    let siblings = collect_siblings(chamber, focus_idx);
    
    // LOD by density
    let lod = structural_lod(siblings.len(), &state.lod_state);
    
    // Render siblings
    for (i, idx) in siblings.iter().enumerate() {
        let is_selected = Some(*idx) == focus_idx;
        let entity_lod = if is_selected { LodTier::Full } else { lod };
        render_entity(ui, chamber, *idx, entity_lod, label_cache, string_table);
    }
    
    // Render preview level (always icon)
    if let Some(selected) = focus_idx {
        let children = collect_children(chamber, selected);
        for idx in children {
            draw_icon(ui, chamber, idx);
        }
    }
}
```

### 6.4 Effect Handler

```rust
// crates/esper_egui/src/effect.rs

// TODO: Implement per Section 11.2

pub fn handle_effects(
    effects: EffectSet,
    state: &DroneState,
    label_cache: &mut LabelCache,
    transition: &mut TransitionState,
) {
    if effects.contains(EffectSet::CHAMBER_CHANGED) {
        label_cache.prepare_for_chamber(/* ... */);
    }
    if effects.contains(EffectSet::SNAP_TRANSITION) {
        transition.start(/* ... */);
    }
    if effects.contains(EffectSet::PHASE_RESET) {
        // Reset focus_t to 0
    }
}
```

### 6.5 Main RIP Loop

```rust
// crates/esper_egui/src/app.rs

// TODO: Implement main render loop

pub struct EsperApp {
    world: Arc<WorldSnapshot>,
    state: DroneState,
    label_cache: LabelCache,
    gesture_recognizer: GestureRecognizer,
    voice_queue: VoiceCommandQueue,
    transition: TransitionState,
}

impl eframe::App for EsperApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1. Process input → verbs
        let verbs = self.collect_verbs(ctx);
        
        // 2. Execute verbs
        for verb in verbs {
            match self.state.execute(verb, &self.world) {
                Ok(effects) => handle_effects(effects, &self.state, &mut self.label_cache, &mut self.transition),
                Err(fault) => log::warn!("Fault: {:?}", fault),
            }
        }
        
        // 3. Update animations
        self.transition.update(ctx.input(|i| i.time));
        self.state.taxonomy.phase.update(/* ... */);
        
        // 4. Render
        egui::CentralPanel::default().show(ctx, |ui| {
            let chamber = &self.world.chambers[self.state.current_chamber as usize];
            match self.state.mode {
                NavigationMode::Spatial => render_spatial(ui, chamber, &self.state, &mut self.label_cache, &self.world.string_table),
                NavigationMode::Structural => render_structural(ui, chamber, &self.state, &mut self.label_cache, &self.world.string_table),
            }
        });
        
        // 5. Request repaint if animating
        if self.transition.is_animating() || self.state.taxonomy.phase == NavigationPhase::Settling {
            ctx.request_repaint();
        }
    }
}
```

**Deliverables**:
- [ ] `esper_egui/src/label_cache.rs`
- [ ] `esper_egui/src/spatial.rs`
- [ ] `esper_egui/src/structural.rs`
- [ ] `esper_egui/src/effect.rs`
- [ ] `esper_egui/src/app.rs`
- [ ] Frame timing tests (must stay under 16ms)

---

## Phase 7: Snapshot Caching

**Goal**: Implement memory + disk caching per Section 14.

```rust
// crates/esper_compiler/src/cache.rs

// TODO: Implement per Section 14.2

pub struct SnapshotCache {
    memory: LruCache<SnapshotCacheKey, Arc<WorldSnapshot>>,
    disk_path: PathBuf,
}

impl SnapshotCache {
    pub async fn get_or_compile(
        &mut self,
        cbu_id: u64,
        ast: &Ast,
        policy: &RenderPolicy,
    ) -> Arc<WorldSnapshot> {
        let key = SnapshotCacheKey::compute(cbu_id, ast, policy);
        
        // 1. Memory hit
        if let Some(s) = self.memory.get(&key) {
            return s.clone();
        }
        
        // 2. Disk hit
        if let Some(s) = self.load_disk(&key).await {
            self.memory.put(key.clone(), s.clone());
            return s;
        }
        
        // 3. Compile
        let snapshot = Arc::new(compile_world(ast, policy).unwrap());
        self.memory.put(key.clone(), snapshot.clone());
        self.save_disk(&key, &snapshot).await;
        snapshot
    }
}
```

**Deliverables**:
- [ ] `esper_compiler/src/cache.rs`
- [ ] Cache invalidation tests

---

## Phase 8: Voice Integration

**Goal**: Integrate voice commands per Section 9.

```rust
// crates/esper_input/src/voice.rs

// TODO: Full implementation

pub struct VoiceProcessor {
    recognizer: SpeechRecognizer,  // platform-specific
    queue: VoiceCommandQueue,
    confirmation_delay: Duration,
}

impl VoiceProcessor {
    pub fn process_audio(&mut self, samples: &[f32]) -> Option<Verb> {
        if let Some(transcript) = self.recognizer.recognize(samples) {
            self.parse_command(&transcript)
        } else {
            None
        }
    }
    
    fn parse_command(&self, transcript: &str) -> Option<Verb> {
        match transcript.to_lowercase().as_str() {
            "select" => Some(Verb::Select(/* preview target */)),
            "right" | "next" => Some(Verb::Next),
            "left" | "previous" => Some(Verb::Prev),
            "down" | "descend" => Some(Verb::Descend),
            "up" | "back" | "ascend" => Some(Verb::Ascend),
            "expand" => Some(Verb::Expand),
            "enhance" => Some(Verb::Enhance),
            _ => None,
        }
    }
}
```

**Deliverables**:
- [ ] `esper_input/src/voice.rs` (full implementation)
- [ ] Voice → Verb mapping tests

---

## Phase 9: Serialization & Replay

**Goal**: Implement deterministic replay per NFR-6.

```rust
// crates/esper_core/src/replay.rs

pub struct NavigationLog {
    pub session_id: u64,
    pub snapshot_key: SnapshotCacheKey,
    pub events: Vec<TimestampedVerb>,
}

pub struct TimestampedVerb {
    pub tick: u64,
    pub verb: Verb,
}

impl NavigationLog {
    pub fn record(&mut self, tick: u64, verb: Verb) {
        self.events.push(TimestampedVerb { tick, verb });
    }
    
    pub fn replay(&self, world: &WorldSnapshot) -> DroneState {
        let mut state = DroneState::default();
        for event in &self.events {
            let _ = state.execute(event.verb, world);
        }
        state
    }
}
```

**Deliverables**:
- [ ] `esper_core/src/replay.rs`
- [ ] Determinism tests (replay produces identical state)

---

## Testing Checklist

### Unit Tests
- [ ] Snapshot SoA invariant validation
- [ ] Grid spatial query (empty, single cell, multi-cell)
- [ ] Navigation index traversal (siblings, children)
- [ ] Each verb returns correct EffectSet
- [ ] Gesture recognizer state machine
- [ ] Policy parsing and validation
- [ ] Two-pass string interning

### Integration Tests
- [ ] Full compile pipeline: AST → WorldSnapshot
- [ ] Snapshot round-trip serialization
- [ ] Cache hit/miss scenarios
- [ ] RIP loop under 16ms with 10k entities

### Benchmarks
- [ ] Compilation: 1-thread vs 8-thread
- [ ] Grid query: 1k, 10k, 100k entities
- [ ] Label cache: hit rate, eviction
- [ ] Frame time: spatial vs structural mode

---

## File Checklist (Complete Rip & Replace)

```
esper/
├── Cargo.toml
├── config/
│   └── policies/
│       └── default.sexpr
└── crates/
    ├── esper_snapshot/
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs
    │       ├── types.rs
    │       ├── validate.rs
    │       ├── grid.rs
    │       └── serde.rs
    ├── esper_core/
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs
    │       ├── token.rs
    │       ├── verb.rs
    │       ├── effect.rs
    │       ├── state.rs
    │       ├── phase.rs
    │       ├── execute.rs
    │       ├── fault.rs
    │       ├── stack.rs
    │       └── replay.rs
    ├── esper_input/
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs
    │       ├── gesture.rs
    │       ├── recognizer.rs
    │       ├── mapping.rs
    │       ├── keyboard.rs
    │       └── voice.rs
    ├── esper_compiler/
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs
    │       ├── graph.rs
    │       ├── chamber.rs
    │       ├── intern.rs
    │       ├── layout.rs
    │       ├── emit.rs
    │       └── cache.rs
    ├── esper_policy/
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs
    │       ├── types.rs
    │       ├── parser.rs
    │       ├── validate.rs
    │       └── watch.rs
    └── esper_egui/
        ├── Cargo.toml
        └── src/
            ├── lib.rs
            ├── label_cache.rs
            ├── spatial.rs
            ├── structural.rs
            ├── effect.rs
            └── app.rs
```

---

## Notes for Implementation

1. **Start with esper_snapshot** — everything depends on these types.

2. **Write validation tests early** — SoA invariants are the contract.

3. **Use `NONE_IDX` and `NONE_ID` consistently** — never mix `Option` in SoA arrays.

4. **Benchmark grid query** — this is called every frame.

5. **Label cache is critical** — cache miss = blown frame budget.

6. **Don't optimize force layout yet** — focus on grid/tree first.

7. **Policy parser can be simple** — s-expr is trivial to parse.

8. **Voice is Phase 8** — stub it and move on.

— End of TODO —
