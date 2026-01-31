# ESPER Navigation System — Implementation Plan

**Reference**: `esper_implementation_todo.md`, `esper_navigation_architecture_v1.0.md`  
**Approach**: Rip and replace. Clean-room implementation against the v1.0 spec.  
**Date**: 2026-01-31

---

## Executive Summary

This plan details the complete replacement of the existing navigation system in `ob-poc-graph` and `ob-poc-ui` with the new ESPER (Enhance, Scale, Pan, Explore, Review) navigation architecture. The implementation follows a **rip-and-replace** strategy as mandated by the refactoring rules.

### Why Rip-and-Replace

Per the `/refactor` skill:
- Files >500 lines with >30% changing → rip and replace
- Architectural refactor → rip and replace  
- Changing data flow patterns → rip and replace
- Unifying two parallel systems → rip and replace

The current navigation code spans 2000+ lines across `ob-poc-graph` with interleaved rendering, state management, and input handling. ESPER cleanly separates these concerns into 6 crates with well-defined contracts.

---

## egui Rules Compliance Checklist

**All phases MUST comply with these rules from `/egui` skill:**

| Rule | Enforcement |
|------|-------------|
| ❶ NO local state mirroring server data | `WorldSnapshot` is immutable, fetched from server |
| ❷ Actions return values, no callbacks | All widgets return `Option<Verb>`, not closures |
| ❸ Short lock, then render | `DroneState` extracted before render, no lock during paint |
| ❹ Process async first, render second | `process_async_results()` → `tick()` → `render()` |
| ❺ Server round-trip for mutations | Snapshot compilation is server-side, UI is read-only |
| ❻ Animation: update before ui, read in ui | `NavigationPhase.update(tick)` in update, read in render |

**Critical Animation Pattern:**
```rust
// WRONG - mutating in ui()
fn ui(&mut self, ui: &mut Ui) {
    self.spring.tick(dt);  // NO
}

// RIGHT - update before ui, read in ui
fn update(&mut self, dt: f32) {
    self.navigation_service.tick(dt);  // Physics here
}
fn ui(&self, ui: &mut Ui) {
    let pos = self.navigation_service.camera_pos();  // Read only
}
```

---

## Crate Dependency Graph

```
                    ┌──────────────────┐
                    │  esper_snapshot  │  ← Pure data types, no deps
                    └────────┬─────────┘
                             │
           ┌─────────────────┼─────────────────┐
           │                 │                 │
           ▼                 ▼                 ▼
    ┌─────────────┐   ┌─────────────┐   ┌─────────────┐
    │ esper_core  │   │esper_policy │   │esper_compiler│
    │(nav engine) │   │(policy defs)│   │(world build) │
    └──────┬──────┘   └──────┬──────┘   └─────────────┘
           │                 │                 ↑
           │                 │                 │
           └────────┬────────┘           (server-side only)
                    │
           ┌────────┴────────┐
           │                 │
           ▼                 ▼
    ┌─────────────┐   ┌─────────────┐
    │ esper_input │   │  esper_egui │
    │(gestures)   │   │(rendering)  │
    └─────────────┘   └─────────────┘
```

**Key insight**: `esper_compiler` is **server-side only** (compiles graph data → snapshot). The client only receives the compiled `WorldSnapshot` and renders it.

---

## Phase 1: esper_snapshot (Wire Format & ABI)

**Goal**: Define the shared types that both server (compiler) and client (egui) consume.

**Duration**: 2-3 days  
**Dependencies**: None (foundational)

### 1.1 Directory Structure

```
rust/crates/esper_snapshot/
├── Cargo.toml
└── src/
    ├── lib.rs           # Re-exports, sentinel constants
    ├── types.rs         # Core snapshot structs (SoA layout)
    ├── chamber.rs       # ChamberKind, ChamberSnapshot
    ├── door.rs          # DoorSnapshot, DoorKind
    ├── grid.rs          # GridSnapshot, spatial query
    ├── validate.rs      # SoA invariant checks
    └── serde.rs         # Deterministic serialization
```

### 1.2 Core Types (from TODO §1.2)

```rust
// Sentinel constants - CRITICAL for SoA navigation
pub const NONE_IDX: u32 = u32::MAX;  // No link in navigation array
pub const NONE_ID: u64 = 0;           // No entity ID

// Envelope for cache invalidation
pub struct SnapshotEnvelope {
    pub schema_version: u32,
    pub source_hash: u64,     // Hash of source AST
    pub policy_hash: u64,     // Hash of render policy
    pub created_at: u64,
    pub cbu_id: u64,
}

// Chamber = navigable region (CBU, entity cluster, etc.)
pub struct ChamberSnapshot {
    pub id: u32,
    pub kind: ChamberKind,
    pub bounds: Rect,
    pub default_camera: CameraPreset,
    
    // SoA entity data — ALL Vec<T> MUST have same length N
    pub entity_ids: Vec<u64>,
    pub kind_ids: Vec<u16>,
    pub x: Vec<f32>,
    pub y: Vec<f32>,
    pub label_ids: Vec<u32>,      // Index into string_table
    pub detail_refs: Vec<u64>,    // NONE_ID if no detail
    
    // Navigation indices — NONE_IDX for no link
    pub first_child: Vec<u32>,    // First child index
    pub next_sibling: Vec<u32>,   // Next sibling index
    pub prev_sibling: Vec<u32>,   // Previous sibling index
    
    pub doors: Vec<DoorSnapshot>,
    pub grid: GridSnapshot,
}
```

### 1.3 SoA Invariants (CRITICAL)

The Structure-of-Arrays layout is the contract. Validation MUST enforce:

```rust
impl ChamberSnapshot {
    pub fn validate(&self) -> Result<(), SnapshotError> {
        let n = self.entity_ids.len();
        
        // All parallel arrays same length
        assert_eq!(self.kind_ids.len(), n);
        assert_eq!(self.x.len(), n);
        assert_eq!(self.y.len(), n);
        assert_eq!(self.label_ids.len(), n);
        assert_eq!(self.detail_refs.len(), n);
        assert_eq!(self.first_child.len(), n);
        assert_eq!(self.next_sibling.len(), n);
        assert_eq!(self.prev_sibling.len(), n);
        
        // Navigation indices in bounds
        for &idx in &self.first_child {
            if idx != NONE_IDX && idx as usize >= n {
                return Err(SnapshotError::IndexOutOfBounds);
            }
        }
        // ... same for next_sibling, prev_sibling
        
        // Entity IDs non-zero
        for &id in &self.entity_ids {
            if id == NONE_ID {
                return Err(SnapshotError::InvalidEntityId);
            }
        }
        
        Ok(())
    }
}
```

### 1.4 Grid Query (Spatial Culling)

```rust
impl GridSnapshot {
    /// Query entities visible in viewport - O(cells touched)
    pub fn query_visible(&self, viewport: Rect) -> impl Iterator<Item = u32> + '_ {
        let (min_cell, max_cell) = self.viewport_to_cells(viewport);
        
        (min_cell.0..=max_cell.0)
            .flat_map(move |cx| (min_cell.1..=max_cell.1).map(move |cy| (cx, cy)))
            .flat_map(move |(cx, cy)| {
                let cell_idx = cy * self.dims.0 + cx;
                let (start, count) = self.cell_ranges[cell_idx as usize];
                (start..start + count).map(|i| self.cell_entity_indices[i as usize])
            })
    }
}
```

### 1.5 Deliverables Checklist

- [ ] `esper_snapshot/src/lib.rs` — sentinel constants, re-exports
- [ ] `esper_snapshot/src/types.rs` — SnapshotEnvelope, WorldSnapshot
- [ ] `esper_snapshot/src/chamber.rs` — ChamberSnapshot, ChamberKind
- [ ] `esper_snapshot/src/door.rs` — DoorSnapshot, DoorKind
- [ ] `esper_snapshot/src/grid.rs` — GridSnapshot, spatial query
- [ ] `esper_snapshot/src/validate.rs` — SoA invariant checks
- [ ] `esper_snapshot/src/serde.rs` — bincode serialization
- [ ] Unit tests: validation (all invariants), grid query (empty, single, multi-cell)
- [ ] Benchmark: grid query with 10k, 100k entities

---

## Phase 2: esper_core (Navigation Engine)

**Goal**: Implement the stack machine, verbs, state, and effects.

**Duration**: 3-4 days  
**Dependencies**: esper_snapshot

### 2.1 Directory Structure

```
rust/crates/esper_core/
├── Cargo.toml
└── src/
    ├── lib.rs           # Re-exports
    ├── verb.rs          # Verb enum (all navigation commands)
    ├── effect.rs        # EffectSet bitflags
    ├── state.rs         # DroneState, CameraState, TaxonomyState
    ├── phase.rs         # NavigationPhase (Moving/Settling/Focused)
    ├── execute.rs       # Verb execution → EffectSet
    ├── fault.rs         # Fault enum for errors
    ├── stack.rs         # Context stack operations
    └── replay.rs        # Navigation log replay
```

### 2.2 Verb Enum (Command Vocabulary)

```rust
#[derive(Clone, Copy, Debug)]
pub enum Verb {
    // === Spatial Navigation ===
    DiveInto(DoorId),           // Enter through door
    PullBack,                    // Exit current chamber
    Surface,                     // Return to parent context
    PanTo { x: f32, y: f32 },   // Pan camera
    Zoom(f32),                   // Zoom in/out
    Enhance,                     // Increase detail level
    Track(EntityId),             // Follow entity
    Focus(EntityId),             // Center on entity
    
    // === Structural Navigation (Taxonomy) ===
    Ascend,                      // Go to parent node
    Descend,                     // Go to first child
    DescendTo(NodeId),           // Go to specific child
    Next,                        // Next sibling
    Prev,                        // Previous sibling
    First,                       // First sibling
    Last,                        // Last sibling
    Expand,                      // Expand current node
    Collapse,                    // Collapse current node
    Select(NodeId),              // Select node
    Preview(NodeId),             // Preview node (hover state)
    ClearPreview,                // Clear preview
    Root,                        // Return to root
}
```

### 2.3 Effect Flags (Response Protocol)

```rust
use bitflags::bitflags;

bitflags! {
    #[derive(Clone, Copy, Debug)]
    pub struct EffectSet: u16 {
        const NONE             = 0;
        const CAMERA_CHANGED   = 1 << 0;   // Camera needs update
        const LOD_MODE_RESET   = 1 << 1;   // LOD tier changed
        const CHAMBER_CHANGED  = 1 << 2;   // Active chamber changed
        const MODE_CHANGED     = 1 << 3;   // Spatial ↔ Structural
        const SNAP_TRANSITION  = 1 << 4;   // Instant camera move
        const TAXONOMY_CHANGED = 1 << 5;   // Taxonomy selection changed
        const PREFETCH_DETAILS = 1 << 6;   // Prefetch detail data
        const SCROLL_ADJUST    = 1 << 7;   // Scroll position changed
        const PHASE_RESET      = 1 << 8;   // Reset navigation phase
        const PREVIEW_SET      = 1 << 9;   // Preview target set
        const PREVIEW_CLEAR    = 1 << 10;  // Preview target cleared
        const CONTEXT_PUSHED   = 1 << 11;  // Context stack grew
        const CONTEXT_POPPED   = 1 << 12;  // Context stack shrunk
    }
}
```

### 2.4 DroneState (Navigation State Machine)

**CRITICAL egui compliance**: DroneState is updated in `update()`, read-only in `ui()`.

```rust
pub struct DroneState {
    pub mode: NavigationMode,
    pub current_chamber: ChamberId,
    pub context_stack: Vec<ContextFrame>,  // Max depth enforced
    pub camera: CameraState,
    pub taxonomy: TaxonomyState,
    pub lod_state: LodState,
}

pub struct CameraState {
    pub target: Vec2,     // Where camera SHOULD be
    pub current: Vec2,    // Where camera IS (for lerp)
    pub zoom: f32,
}

pub struct TaxonomyState {
    pub focus_path: Vec<NodeId>,      // Breadcrumb path
    pub current_depth: usize,
    pub selection: Option<NodeId>,
    pub expanded: HashSet<NodeId>,
    pub scroll_offset: f32,
    pub phase: NavigationPhase,
    pub last_nav_tick: u64,
    pub focus_t: f32,                 // Animation progress [0,1]
    pub preview_target: Option<NodeId>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NavigationPhase {
    Moving,     // User actively navigating
    Settling,   // Brief pause, detail emerging
    Focused,    // Full detail visible
}
```

### 2.5 Verb Execution

```rust
impl DroneState {
    /// Execute a verb, returning effects to be handled
    /// 
    /// This is a PURE function - no side effects beyond state mutation.
    /// Effects are returned for the caller to handle.
    pub fn execute(&mut self, verb: Verb, world: &WorldSnapshot) -> Result<EffectSet, Fault> {
        match verb {
            Verb::DiveInto(door_id) => self.dive_into(door_id, world),
            Verb::PullBack => self.pull_back(),
            Verb::Next => self.next_sibling(world),
            Verb::Prev => self.prev_sibling(world),
            Verb::Ascend => self.ascend(world),
            Verb::Descend => self.descend(world),
            Verb::Focus(entity_id) => self.focus_entity(entity_id, world),
            Verb::Zoom(factor) => self.zoom(factor),
            Verb::PanTo { x, y } => self.pan_to(x, y),
            // ... all other verbs
        }
    }
    
    fn next_sibling(&mut self, world: &WorldSnapshot) -> Result<EffectSet, Fault> {
        let chamber = &world.chambers[self.current_chamber as usize];
        let current = self.taxonomy.selection.ok_or(Fault::EntityNotFound(0))?;
        
        // O(1) lookup via precomputed index
        let next_idx = chamber.next_sibling[current as usize];
        if next_idx == NONE_IDX {
            return Err(Fault::EntityNotFound(0));
        }
        
        self.taxonomy.selection = Some(next_idx);
        self.taxonomy.last_nav_tick = /* current tick */;
        self.taxonomy.phase = NavigationPhase::Moving;
        
        Ok(EffectSet::TAXONOMY_CHANGED | EffectSet::PHASE_RESET)
    }
}
```

### 2.6 Fault Model

```rust
#[derive(Debug)]
pub enum Fault {
    StackUnderflow,                         // Context stack empty on pop
    StackOverflow,                          // Context stack at max depth
    ContextStackOverflow,                   // Too many nested contexts
    DoorNotFound(DoorId),                   // Door doesn't exist
    ChamberNotFound(ChamberId),             // Chamber doesn't exist
    EntityNotFound(EntityId),               // Entity doesn't exist
    CyclicReference(ChamberId),             // Cycle in door graph
    UnknownVerb,                            // Unrecognized verb
    TypeMismatch { expected: TokenKind, got: TokenKind },
    NotAuthorized { chamber: ChamberId, required_role: RoleId },
    MaxDepthExceeded,                       // Taxonomy too deep
    TokenQueueOverflow,                     // Input queue full
}
```

### 2.7 Deliverables Checklist

- [ ] `esper_core/src/verb.rs` — all navigation verbs
- [ ] `esper_core/src/effect.rs` — EffectSet bitflags
- [ ] `esper_core/src/state.rs` — DroneState, CameraState, TaxonomyState
- [ ] `esper_core/src/phase.rs` — NavigationPhase with update logic
- [ ] `esper_core/src/execute.rs` — verb execution for all verbs
- [ ] `esper_core/src/fault.rs` — fault enum
- [ ] `esper_core/src/stack.rs` — context stack with max depth
- [ ] `esper_core/src/replay.rs` — NavigationLog for deterministic replay
- [ ] Unit tests: each verb returns correct EffectSet
- [ ] Unit tests: navigation index traversal (O(1) sibling, child access)
- [ ] Property test: replay produces identical state

---

## Phase 3: esper_input (Unified Input)

**Goal**: Implement gesture recognition and verb mapping.

**Duration**: 2 days  
**Dependencies**: esper_core

### 3.1 Directory Structure

```
rust/crates/esper_input/
├── Cargo.toml
└── src/
    ├── lib.rs           # Re-exports
    ├── gesture.rs       # Gesture enum
    ├── recognizer.rs    # Gesture state machine
    ├── mapping.rs       # Gesture → Verb mapping
    ├── keyboard.rs      # Key → Verb mapping
    └── voice.rs         # Voice command queue (stub → Phase 8)
```

### 3.2 Gesture Types

```rust
pub enum Gesture {
    Click(NodeId),                          // Single click on node
    DoubleClick(NodeId),                    // Double click (dive/descend)
    ClickDoor(DoorId),                      // Click on door
    HoverEnter(NodeId),                     // Mouse entered node
    HoverDwell(NodeId),                     // Hover duration threshold
    HoverExit(NodeId),                      // Mouse left node
    DragStart(Vec2),                        // Drag began
    DragMove { delta: Vec2, velocity: Vec2 }, // Drag in progress
    DragEnd(Vec2),                          // Drag ended
}
```

### 3.3 Gesture → Verb Mapping

**Context-aware mapping based on NavigationMode:**

```rust
pub fn gesture_to_verb(gesture: Gesture, state: &DroneState) -> Option<Verb> {
    match gesture {
        Gesture::Click(node_id) => match state.mode {
            NavigationMode::Spatial => Some(Verb::Focus(node_id)),
            NavigationMode::Structural => Some(Verb::Select(node_id)),
        },
        Gesture::DoubleClick(node_id) => match state.mode {
            NavigationMode::Spatial => find_door_for_entity(node_id, state)
                .map(Verb::DiveInto),
            NavigationMode::Structural => Some(Verb::DescendTo(node_id)),
        },
        Gesture::HoverDwell(node_id) => Some(Verb::Preview(node_id)),
        Gesture::HoverExit(_) => Some(Verb::ClearPreview),
        Gesture::DragMove { delta, .. } => Some(Verb::PanTo { 
            x: -delta.x, 
            y: -delta.y 
        }),
        Gesture::ClickDoor(door_id) => Some(Verb::DiveInto(door_id)),
        _ => None,
    }
}
```

### 3.4 Keyboard Mapping

```rust
pub fn key_to_verb(key: KeyCode, modifiers: Modifiers) -> Option<Verb> {
    match key {
        // Vim-style navigation
        KeyCode::J | KeyCode::Right | KeyCode::Tab => Some(Verb::Next),
        KeyCode::K | KeyCode::Left => Some(Verb::Prev),
        KeyCode::L | KeyCode::Down => Some(Verb::Descend),
        KeyCode::H | KeyCode::Up | KeyCode::Escape => Some(Verb::Ascend),
        
        // Selection/Expansion
        KeyCode::Enter => Some(Verb::Select(/* current */)),
        KeyCode::E => Some(Verb::Expand),
        KeyCode::C => Some(Verb::Collapse),
        KeyCode::Space => Some(Verb::Enhance),
        
        // Zoom
        KeyCode::Plus | KeyCode::Equals => Some(Verb::Zoom(1.2)),
        KeyCode::Minus => Some(Verb::Zoom(0.8)),
        KeyCode::Num0 => Some(Verb::Zoom(1.0)),  // Reset zoom
        
        _ => None,
    }
}
```

### 3.5 Voice Command Queue (Stub for Phase 8)

```rust
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

### 3.6 Deliverables Checklist

- [ ] `esper_input/src/gesture.rs` — Gesture enum
- [ ] `esper_input/src/recognizer.rs` — gesture state machine (hover dwell, double-click timing)
- [ ] `esper_input/src/mapping.rs` — gesture_to_verb function
- [ ] `esper_input/src/keyboard.rs` — key_to_verb function
- [ ] `esper_input/src/voice.rs` — stub for Phase 8
- [ ] Unit tests: gesture recognizer state transitions
- [ ] Unit tests: mode-aware mapping

---

## Phase 4: esper_compiler (World Compiler) — SERVER SIDE ONLY

**Goal**: Implement server-side compilation of graph data → WorldSnapshot.

**Duration**: 3-4 days  
**Dependencies**: esper_snapshot, esper_policy

**IMPORTANT**: This crate runs on the server only. It is NOT compiled to WASM.

### 4.1 Directory Structure

```
rust/crates/esper_compiler/
├── Cargo.toml
└── src/
    ├── lib.rs           # compile_world entry point
    ├── graph.rs         # CanonicalGraph from DB data
    ├── chamber.rs       # Chamberization logic
    ├── intern.rs        # Two-pass string interning
    ├── layout.rs        # Grid/tree/force layout
    ├── emit.rs          # ChamberSnapshot emission
    ├── cache.rs         # Memory + disk caching
    └── hash.rs          # Deterministic hashing
```

### 4.2 Compilation Pipeline

```rust
use rayon::prelude::*;

pub fn compile_world(
    graph_data: &CbuGraphData,  // From existing graph API
    policy: &RenderPolicy,
) -> Result<WorldSnapshot, CompileError> {
    // Stage 1: Build canonical graph
    let graph = build_canonical_graph(graph_data)?;
    
    // Stage 2: Chamberization (split by boundaries)
    let chamber_defs = chamberize(&graph, policy)?;
    
    // Stage 3-4: Two-pass string interning (parallel)
    let (string_table, chamber_defs) = intern_strings_parallel(&chamber_defs);
    
    // Stage 5: Parallel chamber compilation
    let chambers: Vec<ChamberSnapshot> = chamber_defs
        .par_iter()
        .map(|def| compile_chamber(def, policy, &string_table))
        .collect::<Result<Vec<_>, _>>()?;
    
    // Build envelope
    let envelope = SnapshotEnvelope {
        schema_version: 1,
        source_hash: hash_graph_data(graph_data),
        policy_hash: policy.compute_hash(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        cbu_id: graph_data.cbu_id,
    };
    
    Ok(WorldSnapshot { envelope, string_table, chambers })
}
```

### 4.3 Integration with Existing Graph API

The compiler takes `CbuGraphData` (from existing `GET /api/cbu/:id/graph`) and produces `WorldSnapshot`:

```rust
// Existing type from ob-poc-graph
pub struct CbuGraphData {
    pub cbu_id: Uuid,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    // ...
}

// New conversion
fn build_canonical_graph(data: &CbuGraphData) -> Result<CanonicalGraph, CompileError> {
    let nodes: Vec<CanonicalNode> = data.nodes.iter()
        .map(|n| CanonicalNode {
            id: n.entity_id.as_u64_pair().0, // Use first 64 bits of UUID
            kind_id: kind_to_id(&n.entity_type),
            label: n.display_name.clone(),
            position: None, // Layout will compute
            detail_ref: Some(n.entity_id.as_u64_pair().0),
        })
        .collect();
    
    let edges: Vec<CanonicalEdge> = data.edges.iter()
        .map(|e| CanonicalEdge {
            from: entity_id_to_u64(&e.from_id),
            to: entity_id_to_u64(&e.to_id),
            edge_type: edge_type_to_door_kind(&e.edge_type),
        })
        .collect();
    
    Ok(CanonicalGraph { nodes, edges })
}
```

### 4.4 Two-Pass String Interning

```rust
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

### 4.5 Server Endpoint

Add new endpoint to compile and cache snapshots:

```rust
// In rust/src/api/graph_routes.rs

/// GET /api/cbu/:id/snapshot
/// Returns compiled WorldSnapshot for client rendering
pub async fn get_cbu_snapshot(
    Path(cbu_id): Path<Uuid>,
    State(pool): State<PgPool>,
    State(cache): State<SnapshotCache>,
) -> Result<impl IntoResponse, ApiError> {
    // Fetch graph data (existing)
    let graph_data = fetch_cbu_graph(&pool, cbu_id).await?;
    
    // Load policy (cached)
    let policy = load_render_policy()?;
    
    // Get or compile snapshot (cached)
    let snapshot = cache.get_or_compile(cbu_id, &graph_data, &policy).await?;
    
    // Serialize (bincode is 10x smaller than JSON)
    let bytes = snapshot.serialize()?;
    
    Ok((
        [(header::CONTENT_TYPE, "application/octet-stream")],
        bytes,
    ))
}
```

### 4.6 Deliverables Checklist

- [ ] `esper_compiler/src/lib.rs` — compile_world entry point
- [ ] `esper_compiler/src/graph.rs` — CanonicalGraph from CbuGraphData
- [ ] `esper_compiler/src/chamber.rs` — chamberization with boundary detection
- [ ] `esper_compiler/src/intern.rs` — two-pass parallel string interning
- [ ] `esper_compiler/src/layout.rs` — grid and tree layout
- [ ] `esper_compiler/src/emit.rs` — ChamberSnapshot emission with navigation indices
- [ ] `esper_compiler/src/cache.rs` — LRU memory cache + optional disk cache
- [ ] `rust/src/api/graph_routes.rs` — `/api/cbu/:id/snapshot` endpoint
- [ ] Benchmark: 1-thread vs 8-thread compilation
- [ ] Integration test: compile existing CBU graph → valid snapshot

---

## Phase 5: esper_policy (Policy Parser)

**Goal**: Implement policy parsing for LOD configuration.

**Duration**: 1-2 days  
**Dependencies**: esper_snapshot

### 5.1 Directory Structure

```
rust/crates/esper_policy/
├── Cargo.toml
└── src/
    ├── lib.rs           # Re-exports
    ├── types.rs         # RenderPolicy, LodConfig
    ├── parser.rs        # S-expression parser
    ├── validate.rs      # Policy validation
    └── watch.rs         # Hot reload for dev
```

### 5.2 Policy Types

```rust
pub struct RenderPolicy {
    pub version: u32,
    pub kind_schema: Vec<KindDef>,        // Entity type definitions
    pub spatial_lod: SpatialLodConfig,    // Zoom-based LOD
    pub structural_lod: StructuralLodConfig, // Density-based LOD
    pub chamber_configs: HashMap<String, ChamberConfig>,
}

pub struct SpatialLodConfig {
    pub tiers: Vec<LodTier>,
    pub budgets: LodBudgets,
}

pub struct LodTier {
    pub name: String,       // "icon", "label", "full"
    pub zoom_max: f32,      // Max zoom for this tier
    pub hysteresis: f32,    // Prevents flicker at boundaries
}

pub struct LodBudgets {
    pub max_icons: u32,
    pub max_labels: u32,
    pub max_full: u32,
}
```

### 5.3 Sample Policy File

```
rust/config/policies/default.policy
```

```lisp
(policy
  :version 1
  
  (kinds
    (kind :id 1 :name "cbu" :icon "building")
    (kind :id 2 :name "entity" :icon "user")
    (kind :id 3 :name "product" :icon "package"))
  
  (spatial-lod
    (tier :name "icon" :zoom-max 0.3 :hysteresis 0.05)
    (tier :name "label" :zoom-max 0.7 :hysteresis 0.05)
    (tier :name "full" :zoom-max 999.0))
    
  (budgets
    :max-icons 500
    :max-labels 100
    :max-full 20)
    
  (chamber :kind "cbu"
    :layout "grid"
    :cell-size 100
    :default-zoom 1.0))
```

### 5.4 Deliverables Checklist

- [ ] `esper_policy/src/types.rs` — RenderPolicy, LodConfig, etc.
- [ ] `esper_policy/src/parser.rs` — s-expression parser (nom-based)
- [ ] `esper_policy/src/validate.rs` — policy validation + hash
- [ ] `esper_policy/src/watch.rs` — dev-only hot reload
- [ ] `rust/config/policies/default.policy` — default policy file
- [ ] Unit tests: parse → validate round-trip

---

## Phase 6: esper_egui (Rendering)

**Goal**: Implement the RIP (Request-Input-Present) loop with egui.

**Duration**: 4-5 days  
**Dependencies**: esper_snapshot, esper_core, esper_input, esper_policy

### 6.1 Directory Structure

```
rust/crates/esper_egui/
├── Cargo.toml
└── src/
    ├── lib.rs           # Re-exports, EsperWidget
    ├── label_cache.rs   # LRU cache for shaped text
    ├── spatial.rs       # Spatial mode rendering
    ├── structural.rs    # Structural mode rendering (taxonomy)
    ├── effect.rs        # Effect handler
    ├── transition.rs    # Camera/zoom transitions
    └── app.rs           # Main RIP loop (for standalone)
```

### 6.2 egui Integration Pattern

**CRITICAL: Follows egui rules exactly**

```rust
/// Esper navigation widget - embeddable in any egui app
pub struct EsperWidget {
    // Immutable data (from server)
    world: Option<Arc<WorldSnapshot>>,
    policy: Arc<RenderPolicy>,
    
    // Navigation state (updated in update(), read in ui())
    state: DroneState,
    
    // Rendering helpers
    label_cache: LabelCache,
    gesture_recognizer: GestureRecognizer,
    transition: TransitionState,
}

impl EsperWidget {
    /// Call BEFORE render - updates physics/animation
    /// 
    /// This is the ONLY place mutation happens.
    pub fn update(&mut self, dt: f32) {
        // Update navigation phase
        self.state.taxonomy.phase.update(
            current_tick(),
            self.state.taxonomy.last_nav_tick,
            DWELL_TICKS,
        );
        
        // Update camera lerp
        self.state.camera.current = self.state.camera.current.lerp(
            self.state.camera.target,
            CAMERA_LERP_SPEED * dt,
        );
        
        // Update transition
        self.transition.update(dt);
    }
    
    /// Process a verb command (from keyboard, gesture, voice)
    pub fn execute(&mut self, verb: Verb) -> Result<EffectSet, Fault> {
        let world = self.world.as_ref().ok_or(Fault::ChamberNotFound(0))?;
        let effects = self.state.execute(verb, world)?;
        self.handle_effects(effects);
        Ok(effects)
    }
    
    /// Render the widget - READ ONLY, no mutation
    pub fn ui(&self, ui: &mut egui::Ui) -> EsperResponse {
        let Some(world) = &self.world else {
            ui.label("Loading...");
            return EsperResponse::default();
        };
        
        let chamber = &world.chambers[self.state.current_chamber as usize];
        
        match self.state.mode {
            NavigationMode::Spatial => {
                self.render_spatial(ui, chamber, &world.string_table)
            }
            NavigationMode::Structural => {
                self.render_structural(ui, chamber, &world.string_table)
            }
        }
    }
}
```

### 6.3 Label Cache (Prevents Frame Budget Blowout)

```rust
#[derive(Hash, Eq, PartialEq)]
pub struct LabelCacheKey {
    pub string_id: u32,
    pub max_width: u16,   // Quantized to 10px buckets
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
        let galley = Arc::new(fonts.layout_no_wrap(
            text,
            FontId::default(),
            Color32::WHITE,
        ));
        
        // LRU eviction
        if self.cache.len() >= self.max_entries {
            if let Some(old_key) = self.lru.pop_front() {
                self.cache.remove(&old_key);
            }
        }
        
        self.cache.insert(key.clone(), galley.clone());
        self.lru.push_back(key);
        galley
    }
}
```

### 6.4 Spatial Renderer

```rust
fn render_spatial(
    &self,
    ui: &mut Ui,
    chamber: &ChamberSnapshot,
    string_table: &[String],
) -> EsperResponse {
    let painter = ui.painter();
    let viewport = self.compute_viewport(ui.available_rect());
    
    // Query visible entities (spatial index)
    let visible: Vec<u32> = chamber.grid.query_visible(viewport).collect();
    
    // Determine LOD for each
    let lod = self.compute_lod(self.state.camera.zoom);
    
    // Apply budgets
    let (icons, labels, fulls) = self.apply_budgets(&visible, lod);
    
    // Render icons (cheap)
    for &idx in &icons {
        let pos = self.world_to_screen(chamber.x[idx as usize], chamber.y[idx as usize]);
        let kind = chamber.kind_ids[idx as usize];
        draw_icon(painter, pos, kind);
    }
    
    // Render labels (only if not moving)
    if self.state.taxonomy.phase != NavigationPhase::Moving {
        for &idx in &labels {
            let label_id = chamber.label_ids[idx as usize];
            let text = &string_table[label_id as usize];
            // Use cache to avoid re-shaping
            let galley = self.label_cache.get_or_shape(/* ... */);
            draw_label(painter, pos, &galley);
        }
    }
    
    // Render full cards
    for &idx in &fulls {
        self.draw_full_card(ui, chamber, idx, string_table);
    }
    
    EsperResponse::default()
}
```

### 6.5 Structural Renderer (Taxonomy Browser)

```rust
fn render_structural(
    &self,
    ui: &mut Ui,
    chamber: &ChamberSnapshot,
    string_table: &[String],
) -> EsperResponse {
    let mut response = EsperResponse::default();
    
    // Breadcrumb bar
    self.render_breadcrumb(ui, &self.state.taxonomy.focus_path, string_table);
    
    // Current level items
    let focus_idx = self.state.taxonomy.selection;
    let siblings = self.collect_siblings(chamber, focus_idx);
    
    // LOD by density
    let lod = self.structural_lod(siblings.len());
    
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (i, idx) in siblings.iter().enumerate() {
            let is_selected = Some(*idx) == focus_idx;
            let entity_lod = if is_selected { LodTier::Full } else { lod };
            
            if let Some(verb) = self.render_entity_row(ui, chamber, *idx, entity_lod, is_selected, string_table) {
                response.verb = Some(verb);
            }
        }
    });
    
    // Preview pane (children of selected)
    if let Some(selected) = focus_idx {
        let children = self.collect_children(chamber, selected);
        ui.separator();
        for idx in children.iter().take(5) {
            draw_preview_icon(ui, chamber, *idx, string_table);
        }
    }
    
    response
}
```

### 6.6 Deliverables Checklist

- [ ] `esper_egui/src/lib.rs` — EsperWidget main struct
- [ ] `esper_egui/src/label_cache.rs` — LRU galley cache
- [ ] `esper_egui/src/spatial.rs` — spatial mode rendering
- [ ] `esper_egui/src/structural.rs` — taxonomy browser rendering
- [ ] `esper_egui/src/effect.rs` — effect handler (CHAMBER_CHANGED, etc.)
- [ ] `esper_egui/src/transition.rs` — camera/zoom animation
- [ ] Integration with ob-poc-ui: replace CbuGraphWidget with EsperWidget
- [ ] Frame timing tests: MUST stay under 16ms with 10k entities

---

## Phase 7: Migration from ob-poc-graph

**Goal**: Replace existing graph/navigation code with ESPER crates.

**Duration**: 2-3 days  
**Dependencies**: All ESPER crates complete

### 7.1 Files to DELETE (Rip)

These files will be completely removed:

```
rust/crates/ob-poc-graph/src/graph/
├── animation.rs       → Replaced by esper_egui/transition.rs
├── camera.rs          → Replaced by esper_core/state.rs
├── input.rs           → Replaced by esper_input/
├── lod.rs             → Replaced by esper_policy/
├── viewport.rs        → Replaced by esper_egui/spatial.rs
├── viewport_fit.rs    → Merged into esper_egui
├── galaxy.rs          → Replaced by esper_egui + chamber concept
├── cluster.rs         → Replaced by chamber concept
├── force_sim.rs       → Keep (may still be useful for layout)
└── render.rs          → Replaced by esper_egui
```

### 7.2 Files to MODIFY (Replace)

```
rust/crates/ob-poc-ui/src/
├── app.rs             → Replace CbuGraphWidget with EsperWidget
├── state.rs           → Remove galaxy_view, cluster_view (now in EsperWidget)
└── navigation.rs      → Simplify to just call EsperWidget.execute()
```

### 7.3 Migration Mapping

| Old Component | New Component |
|--------------|---------------|
| `CbuGraphWidget` | `EsperWidget` |
| `GalaxyView` | `esper_core::DroneState` + multi-chamber world |
| `ClusterView` | Single chamber with CBUs as entities |
| `TaxonomyState` | `esper_core::TaxonomyState` |
| `TradingMatrixState` | Chamber with trading matrix as structure |
| `graph_widget.zoom_in()` | `widget.execute(Verb::Zoom(1.2))` |
| `graph_widget.focus_entity()` | `widget.execute(Verb::Focus(id))` |
| `graph_widget.pan()` | `widget.execute(Verb::PanTo { x, y })` |

### 7.4 App.rs Changes

```rust
// BEFORE
pub struct App {
    pub state: AppState,
}

impl AppState {
    pub graph_widget: CbuGraphWidget,
    pub galaxy_view: GalaxyView,
    pub cluster_view: ClusterView,
    // ... 10+ navigation-related fields
}

// AFTER
pub struct App {
    pub state: AppState,
}

impl AppState {
    pub esper: EsperWidget,  // Single widget replaces all navigation
    // ... other non-navigation state
}
```

### 7.5 Verb Execution Unification

All navigation commands (voice, keyboard, agent) go through one path:

```rust
// In app.rs update()
#[cfg(target_arch = "wasm32")]
fn execute_navigation_verb(&mut self, verb: esper_core::Verb, source: Option<CommandSource>) {
    // Log to audit trail
    if !matches!(verb, Verb::None) {
        self.state.log_navigation(verb.to_dsl(), source);
    }
    
    // Execute via EsperWidget
    match self.state.esper.execute(verb) {
        Ok(effects) => {
            // Handle effects that need app-level action
            if effects.contains(EffectSet::PREFETCH_DETAILS) {
                self.fetch_entity_details(/* ... */);
            }
        }
        Err(fault) => {
            web_sys::console::warn_1(&format!("Navigation fault: {:?}", fault).into());
        }
    }
}
```

---

## Phase 8: Voice Integration (Full)

**Goal**: Complete voice command integration.

**Duration**: 1-2 days  
**Dependencies**: esper_input stub in place

### 8.1 Voice → Verb Parsing

```rust
impl VoiceProcessor {
    fn parse_command(&self, transcript: &str) -> Option<Verb> {
        match transcript.to_lowercase().as_str() {
            // Selection
            "select" | "choose" => Some(Verb::Select(/* preview target */)),
            "confirm" | "yes" => Some(Verb::Select(/* current */)),
            
            // Navigation
            "right" | "next" => Some(Verb::Next),
            "left" | "previous" | "back" => Some(Verb::Prev),
            "down" | "descend" | "drill" => Some(Verb::Descend),
            "up" | "ascend" | "surface" => Some(Verb::Ascend),
            
            // Actions
            "expand" | "open" => Some(Verb::Expand),
            "collapse" | "close" => Some(Verb::Collapse),
            "enhance" | "zoom in" => Some(Verb::Enhance),
            "zoom out" | "pull back" => Some(Verb::PullBack),
            
            // Cancel
            "cancel" | "stop" | "never mind" => {
                self.queue.cancel();
                None
            }
            
            _ => None,
        }
    }
}
```

---

## Phase 9: Testing & Benchmarks

**Goal**: Ensure correctness and performance.

**Duration**: 2 days  
**Dependencies**: All phases complete

### 9.1 Unit Tests

| Test | Module | Description |
|------|--------|-------------|
| SoA validation | esper_snapshot | All invariants enforced |
| Grid query | esper_snapshot | Empty, single cell, multi-cell |
| Navigation indices | esper_core | O(1) sibling/child access |
| Each verb | esper_core | Correct EffectSet returned |
| Gesture recognizer | esper_input | State machine transitions |
| Policy parsing | esper_policy | Parse → validate round-trip |
| String interning | esper_compiler | Parallel correctness |

### 9.2 Integration Tests

| Test | Description |
|------|-------------|
| Compile existing CBU | CbuGraphData → WorldSnapshot → valid |
| Snapshot round-trip | serialize → deserialize → identical |
| Full navigation flow | Load → Navigate → Render |
| Cache hit/miss | Memory and disk cache behavior |

### 9.3 Benchmarks

| Benchmark | Target |
|-----------|--------|
| Grid query 10k entities | < 1ms |
| Grid query 100k entities | < 10ms |
| Label cache hit rate | > 90% |
| Chamber compilation | < 100ms for 1k entities |
| Frame time (10k entities) | < 16ms |
| Frame time (spatial, moving) | < 8ms (labels skipped) |

---

## File Checklist (Complete Structure)

```
rust/crates/
├── esper_snapshot/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── types.rs
│       ├── chamber.rs
│       ├── door.rs
│       ├── grid.rs
│       ├── validate.rs
│       └── serde.rs
├── esper_core/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
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
├── esper_compiler/        # SERVER-SIDE ONLY
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── graph.rs
│       ├── chamber.rs
│       ├── intern.rs
│       ├── layout.rs
│       ├── emit.rs
│       ├── cache.rs
│       └── hash.rs
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
        ├── transition.rs
        └── app.rs
```

---

## Implementation Order

1. **esper_snapshot** (2-3 days) — Foundation, no dependencies
2. **esper_policy** (1-2 days) — Needed for compiler and renderer
3. **esper_core** (3-4 days) — Navigation engine
4. **esper_input** (2 days) — Can start parallel with esper_core
5. **esper_compiler** (3-4 days) — Server-side, needs snapshot + policy
6. **esper_egui** (4-5 days) — Final rendering layer
7. **Migration** (2-3 days) — Replace ob-poc-graph usage
8. **Voice full** (1-2 days) — Complete voice integration
9. **Testing** (2 days) — Unit, integration, benchmarks

**Total: ~20-25 days**

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Frame budget blown | Label cache, LOD budgets, skip labels during Moving phase |
| WASM size increase | esper_compiler is server-only, not in WASM bundle |
| Migration breaks | Keep old code until new is verified, switch via feature flag |
| Performance regression | Benchmark on each phase, compare to current |

---

## Notes for Implementation

1. **Start with esper_snapshot** — everything depends on these types.

2. **Write validation tests early** — SoA invariants are the contract.

3. **Use `NONE_IDX` and `NONE_ID` consistently** — never mix `Option` in SoA arrays.

4. **Benchmark grid query** — this is called every frame.

5. **Label cache is critical** — cache miss = blown frame budget.

6. **Don't optimize force layout yet** — focus on grid/tree first.

7. **Policy parser can be simple** — s-expr is trivial to parse with nom.

8. **Voice is Phase 8** — stub it and move on.

9. **egui rules are NON-NEGOTIABLE** — any violation will cause bugs.

---

**End of Implementation Plan**
