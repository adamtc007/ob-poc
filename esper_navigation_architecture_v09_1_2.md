<!-- Draft v0.9.1.2 generated 2026-01-31 -->

<!-- Draft v0.9.1.1 generated 2026-01-31 -->

<!-- Draft v0.9.1 generated 2026-01-31 -->

ARCHITECTURE DESIGN DOCUMENT

Esper Navigation System

Stack-Machine Driven Visualization for Complex Data Hierarchies

Enterprise Onboarding Platform — OB-POC Project

```
Status: DRAFT FOR PEER REVIEW
```

Version: 0.9 (Consolidated)

Date: 2026-01-31

Revision History

| Ver | Date | Changes |
| --- | --- | --- |
| 0.1 | 2026-01-30 | Initial: stack machine, chambers, verbs |
| 0.2 | 2026-01-30 | Tokenization boundary, fault model, TRACK semantics |
| 0.3 | 2026-01-30 | Render Policy: LOD, hysteresis, budgets |
| 0.4 | 2026-01-30 | RenderScratch, complexity fixes, resolution modes |
| 0.5 | 2026-01-30 | World Compiler: Domain→IR, SoA layout, TaxonomySchema |
| 0.6 | 2026-01-31 | Text rendering perf, structural navigation, LOD rationale |
| 0.7 | 2026-01-31 | Unified input model, fly-over, verb↔RIP interface, sync scenarios |
| 0.8 | 2026-01-31 | rayon parallelism, snapshot caching, frame budget |
| 0.9 | 2026-01-31 | CONSOLIDATED: All content from v0.5-v0.8 with proper section order |

# 1. Executive Summary

This document describes the architecture for a visualization navigation system (codename: Esper) designed for intuitive exploration of deeply nested, variable-scale data structures within the Enterprise Onboarding Platform.

The system addresses a fundamental challenge in financial services compliance: navigating ownership hierarchies spanning from high-level fund structures down to individual beneficial owners, where intermediate structures may contain vastly different amounts of data.

## 1.1 Key Design Decisions

Stack Machine Architecture: Navigation commands are verbs that manipulate a context stack. Deterministic, replayable.

Unified Input Model: All input sources (keyboard, mouse, voice) emit verbs. Mouse overrides voice.

Fly-Over Navigation: Icons while moving, detail on dwell. 60fps guaranteed during rapid navigation.

World Compiler: Domain model compiles server-side to flat IR. Snapshots cached.

Structure of Arrays: Entity data uses SoA layout for cache-friendly iteration.

Server-Side Parallelism: rayon for chamber processing, two-pass string interning.

## 1.2 Performance Targets

```
┌─────────────────────────────────────────────────────────────────┐
```

│  Target                        Constraint                       │

```
│  ──────                        ──────────                       │
```

│  Frame rate                    60fps (16.67ms budget)           │

│  Navigation response           < 16ms                           │

│  DSL → visible                 < 200ms (cached: < 20ms)         │

│  Label shaping (cached)        < 0.5ms for 200 labels           │

│  Entities supported            100,000+                         │

│  Compile speedup (8 threads)   3-6× for large taxonomies        │

```
└─────────────────────────────────────────────────────────────────┘
```

# 2. Problem Statement

## 2.1 The TARDIS Problem

Traditional visualization approaches fail when data structures are "bigger on the inside" — where child nodes contain orders of magnitude more data than their parents suggest.

| Structure Level | Entity Count | Complexity |
| --- | --- | --- |
| Fund Complex Overview | 5-20 | Low |
| LP Structure Detail | 50-200 | Medium |
| Instrument Matrix | 10,000-50,000 | Extreme |

Solution: Discrete chambers with independent coordinate spaces, hard-cut transitions, context stack for navigation history.

## 2.2 Two Navigation Modes

Spatial Mode: Pan, zoom, camera-driven LOD. For CBU overviews, general graphs.

Structural Mode: Ascend, descend, sibling-traverse. For taxonomies, hierarchies. Full-screen, level-focused.

Spatial Mode (CBU)              Structural Mode (Taxonomy)

```
┌─────────────────┐             ┌─────────────────────────┐
```

│    ●            │             │ Breadcrumb: All > Equity │

```
│   /|\    ●      │             ├─────────────────────────┤
```

│  ● ● ●  /|\     │             │ [●Eq] [●Fix] [●Deriv]   │

│        ● ● ●    │             │   ↓                      │

│                 │             │ [●Lg] [●Mid] [●Sm]       │

│  Pan/Zoom/LOD   │             │   ↓                      │

```
└─────────────────┘             │ ● ● ● ● ● (preview)      │
                                └─────────────────────────┘
       DIVE-INTO ──────────────────────▶
       ◀────────────────────── PULL-BACK / ASCEND from root
```

## 2.3 Requirements

### Functional Requirements

FR-1: Navigate hierarchical structures of arbitrary depth without scale distortion.

FR-2: Support discrete context transitions between incompatible scale domains.

FR-3: Maintain navigation history enabling return to any previous context.

FR-4: Support spatial (pan, zoom) and semantic (drill-into, follow-relationship) navigation.

FR-5: Enable compound navigation via composable commands.

FR-6: Produce deterministic, replayable navigation sequences for audit.

FR-7: Provide flicker-free LOD transitions with configurable presentation.

FR-8: Support automatic (zoom-driven) and manual (ENHANCE) LOD control.

FR-9: Compile domain model to render-optimized IR server-side.

### Non-Functional Requirements

NFR-1: O(1) navigation command execution regardless of world size.

NFR-2: Sub-16ms frame rendering (60fps target).

NFR-3: Support 100,000+ entities with visible-set rendering.

NFR-4: No new heap allocations in frame loop after warm-up.

NFR-5: Serializable state with explicit versioning.

NFR-6: Deterministic replay across machines and frame rates.

NFR-7: Configurable render policy without recompilation.

NFR-8: Flat IR format optimized for cache-friendly traversal.

NFR-9: All navigation/LOD tuning parameters must be externalized (policy/config), inspectable at runtime, and changeable without code changes. In dev, changes apply via hot-reload; in prod, changes apply via policy artifact rollout (no recompilation).

NFR-10: The active `(source_hash, policy_hash, schema_version)` must be observable in both server logs and UI debug overlay for reproducibility and performance triage.

# 3. Conceptual Model

## 3.1 The Cave System Metaphor

The data structure is a static cave system. The UI is a drone flying through it. Start mile-high, dive into a foxhole, explore a chamber, follow tunnels to connected chambers, then return to the surface.

Terrain Immutability: The cave system (data) does not change during flight. New server data constitutes a new cave system.

Discrete Chambers: Each sub-structure has its own coordinate space and scale.

Navigation Stack: The drone maintains a record of chambers visited.

## 3.2 Compilation Model

The system follows a compiler architecture:

```
┌─────────────────────────────────────────────────────────────────┐
```

│                                                                 │

│   Domain Model          World Compiler         Render IR       │

│   (Source)              (Server-side)          (Object File)   │

```
│   ───────────           ──────────────         ─────────────   │
```

│                                                                 │

│   DSL / JSON    ──▶    Parse + Validate   ──▶  WorldSnapshot   │

│   Nested structs       Chamberize              Flat arrays     │

│   Rich semantics       Layout                  Interned strings│

│   Audit trail          Build indices           Grid cells      │

│                                                                 │

```
└─────────────────────────────────────────────────────────────────┘
```

                              │

                              ▼

```
┌─────────────────────────────────────────────────────────────────┐
```

│                                                                 │

│   UI (Runtime)                                                  │

```
│   ────────────                                                  │
```

│                                                                 │

│   Load WorldSnapshot  ──▶  RIP Loop  ──▶  egui shapes          │

│   (deserialize once)      (per frame)    (visible set only)    │

│                                                                 │

```
└─────────────────────────────────────────────────────────────────┘
```

The AST (NOM-parsed DSL) is the "source code". The WorldSnapshot is the "object file". The UI consumes only the object file, never the source.

# 4. Stack Machine Architecture

## 4.1 Tokenization Boundary

The navigation core consumes typed tokens, never strings:

```
enum CommandToken {
```

    Verb(VerbId),

    Macro(MacroId),

    Literal(f32),

    Entity(EntityId),

    Door(DoorId),

    Relationship(RelationshipKind),

```
}
```

```
enum VerbId {
```

    DiveInto, PullBack, Surface,

    PanTo, Zoom, Enhance,

    Track, Focus,

```
    // Structural navigation
```

    Ascend, Descend, Next, Prev, First, Last,

    Expand, Collapse, Select, Preview,

```
}
```

## 4.2 Dual-Stack Architecture

```
┌─────────────────────────────────────────────────┐
```

│ Data Stack           │ Context Stack           │

│ (operands)           │ (navigation history)    │

```
├──────────────────────┼─────────────────────────┤
```

│ Literal(100.0)       │ Surface @ (0,0,1.0)     │

│ Door(0x0017)         │ Fund_A @ (500,300,2.0)  │

│                      │ ← current chamber       │

```
└──────────────────────┴─────────────────────────┘
```

## 4.3 Core Verbs

| Verb | Stack Effect | Description |
| --- | --- | --- |
| DIVE-INTO | ( door -- ) | Push context, enter target chamber |
| PULL-BACK | ( -- ) | Pop context, restore previous camera |
| SURFACE | ( -- ) | Clear context, return to root |
| PAN-TO | ( x y -- ) | Set camera target position |
| ZOOM | ( factor -- ) | Multiply target zoom |
| ENHANCE | ( -- ) | Cycle LOD in manual mode |
| TRACK | ( rel -- ) | Plan path, emit DIVE-INTO sequence |
| FOCUS | ( entity -- ) | Spotlight specific entity |

## 4.4 Fault Model

```
enum Fault {
```

    StackUnderflow, StackOverflow, ContextStackOverflow,

    DoorNotFound(DoorId), ChamberNotFound(ChamberId),

    EntityNotFound(EntityId), CyclicReference(ChamberId),

    UnknownVerb, TypeMismatch { expected: TokenKind, got: TokenKind },

    NotAuthorized { chamber: ChamberId, required_role: RoleId },

    MaxDepthExceeded, TokenQueueOverflow,

```
}
```

```
enum ExecutionMode { Strict, Interactive }
```

# 5. World Compiler

The World Compiler transforms the rich domain model into a flat, render-optimized intermediate representation. This compilation happens server-side, ensuring single source of truth.

## 5.1 Why Flatten?

The RIP loop needs to do only these things, cheaply:

1. Pick chamber

2. Query visible IDs (spatial index)

3. Choose LOD from policy

4. Draw primitives (batched)

A nested JSON blob forces the UI to traverse tree structures per frame. Flattening turns that into: visible_ids → arrays → draw.

## 5.2 Compilation Pipeline

Stage 0: Parse DSL → AST (NOM). Semantic validation.

Stage 1: Build canonical graph (nodes + edges).

Stage 2: Chamberization (apply boundary heuristics).

Stage 3: Layout (compute positions, build spatial index).

Stage 4: Intern (string table, numeric IDs).

Stage 5: Emit WorldSnapshot (flat SoA).

## 5.3 ChamberSnapshot: Structure of Arrays

Entity data uses SoA layout for cache-friendly traversal. Parallel arrays share the same indexing:

```
struct ChamberSnapshot {
    id: u32,
    kind: ChamberKind,
    bounds: Rect,
    default_camera: CameraPreset,
```

```
    // Entity data: SoA layout (parallel arrays, same length)
```

    entity_ids: Vec<u64>,        // [N] server entity IDs

    kind_ids: Vec<u16>,          // [N] index into kinds table

    x: Vec<f32>,                 // [N] world X position

    y: Vec<f32>,                 // [N] world Y position

    label_ids: Vec<u32>,         // [N] index into string_table

    detail_refs: Vec<u64>,         // [N] optional detail link (0 = none)

```
    // Navigation indices: O(1) traversal
```

    first_child: Vec<u32>,       // Index of first child

    next_sibling: Vec<u32>,      // Index of next sibling

    prev_sibling: Vec<u32>,      // Index of prev sibling

```
    // Doors to other chambers
    doors: Vec<DoorSnapshot>,
```

```
    // Spatial index (precomputed)
    grid: GridSnapshot,
}
```

Why SoA instead of AoS? Cache Efficiency: When iterating to draw icons, we only touch x, y, kind_ids. WASM Friendly: Maps to typed arrays. SIMD Potential: Float arrays can be vectorized.

### 5.3.1 Snapshot Contract (Client/Server ABI)

This section is the **hard compatibility boundary** between server compilation and client rendering. If this contract holds, the RIP can remain simple and fast, and server-side evolution stays safe.

#### Invariants
- **Parallel-array length**: every SoA array in `ChamberSnapshot` has the same length **N**.
- **Index domain**: `first_child`, `next_sibling`, `prev_sibling`, and any grid references are **chamber-local** indices in `[0..N)`, unless explicitly set to `NONE`.
- **Stable identity**: `entity_ids[i]` is the stable server-side entity identifier for the element at index `i`. Indices may change between snapshots; `entity_ids` must not.
- **Determinism**: given identical `(source_hash, policy_hash, schema_version)`, the serialized snapshot bytes are deterministic (stable ordering + stable layout rules).

#### Sentinel values (no `Option` required)
To keep snapshots **WASM typed-array friendly** and cache-friendly:
- `NONE_IDX: u32 = u32::MAX` — used by navigation indices to mean “no link”.
- `NONE_ID: u64 = 0` — used by `detail_refs[i]` to mean “no detail”.

#### Grid and spatial references
- `GridSnapshot.cell_entity_indices[]` stores chamber-local indices that must be valid `[0..N)`; no sentinel values inside grid cells.
- A chamber’s `bounds` must fully contain all `(x[i], y[i])` positions, so the client can clip and cull safely.

#### Doors and cross-chamber references
- `DoorSnapshot.target_chamber_id` is a **stable chamber identifier**, not a chamber-local index.
- Doors may optionally include `target_entity_id` (server ID) for “focus-on-arrival” behavior. This avoids brittle index-based coupling across chambers.

#### Strings and labels
- `label_ids[i]` indexes into an interned `string_table`.
- Client is allowed to render:
  - **icon-only** with no string access (MOVING)
  - **cached shaped label** if available (SETTLING)
  - **full shaping** only when phase/budget allows (FOCUSED)

- **Client-evaluated policy** must never change the meaning of snapshot indices/IDs; it may only change presentation (LOD, budgets, phase gating, text shaping decisions).

#### Compatibility and schema evolution
- `schema_version` is required in the snapshot envelope.
- Additive changes must be backward compatible (new fields optional with defaults).
- Breaking changes require a schema bump and must invalidate caches.

## 5.4 GridSnapshot: Pre-built Spatial Index

```
struct GridSnapshot {
    cell_size: f32,
    origin: Vec2,
    dims: (u32, u32),
```

```
    // Flattened cell data
```

    cell_ranges: Vec<(u32, u32)>,    // [cells] (start, count)

    cell_entity_indices: Vec<u32>,   // indices, sorted

```
}
```

```
impl GridSnapshot {
    fn query_visible(&self, viewport: Rect) -> impl Iterator<Item = u32> + '_ {
        let min_cell = self.world_to_cell(viewport.min);
        let max_cell = self.world_to_cell(viewport.max);
```

```
        (min_cell.y..=max_cell.y).flat_map(move |row| {
            (min_cell.x..=max_cell.x).flat_map(move |col| {
                let cell_idx = row * self.dims.0 + col;
                let (start, count) = self.cell_ranges[cell_idx as usize];
```

                self.cell_entity_indices[start as usize..(start+count) as usize]

                    .iter().copied()

```
            })
        })
    }
}
```

## 5.5 DoorSnapshot

```
struct DoorSnapshot {
    id: u32,
```

    from_entity_idx: u32,     // index into parent chamber

    to_chamber: u32,          // target ChamberSnapshot id

    door_kind: DoorKind,      // visual style

    label_id: u32,            // index into string_table

```
    position: Vec2,
}
```

# 6. Structural Navigation

Taxonomies require structural navigation, not spatial. Navigate the hierarchy itself.

## 6.1 Structural State

```
struct TaxonomyState {
```

    focus_path: Vec<NodeId>,      // [root, equity, large_cap]

    current_depth: usize,         // Which level has focus

    selection: Option<NodeId>,    // Selected node at current level

    expanded: HashSet<NodeId>,    // Nodes with inline expansion

    scroll_offset: f32,           // Horizontal scroll at level

```
    // Fly-over state
    phase: NavigationPhase,
    last_nav_tick: u64,
```

    focus_t: f32,                 // 0.0 = icons, 1.0 = full detail

```
    preview_target: Option<NodeId>,
}
```

## 6.2 Structural Verbs

| Verb | Effect | Description |
| --- | --- | --- |
| ASCEND | ( -- ) | Move focus to parent level |
| DESCEND | ( -- ) | Move focus into selected node |
| NEXT | ( -- ) | Select next sibling |
| PREV | ( -- ) | Select previous sibling |
| FIRST | ( -- ) | Select first sibling |
| LAST | ( -- ) | Select last sibling |
| EXPAND | ( -- ) | Show children inline |
| COLLAPSE | ( -- ) | Hide children |
| ROOT | ( -- ) | Jump to taxonomy root |

## 6.3 Structural LOD

LOD in structural mode is driven by density and focus, not camera zoom:

```
(defstructural-lod
  (context-above icon)           // Levels above focus: always icon
  (preview-below icon)           // Preview level: always icon
  (focus-level
    (when (> nodes 50) icon)     // Dense: icons only
    (when (> nodes 10) label)    // Medium: labels
    (else full))                 // Sparse: full cards
  (selected full))               // Selected node: always full
```

Three tiers: Icon (dot + badge), Label (name + count), Full (multi-line card).

# 7. Text Rendering Performance

Text shaping is the primary frame budget constraint. Understanding this justifies the entire LOD and budget architecture.

## 7.1 Why Text is Expensive

```
┌─────────────────────────────────────────────────────────────────┐
```

│  Step             Cost          Operations                     │

```
│  ────             ────          ──────────                     │
```

│  1. Font lookup   Cheap         Glyph ID lookup                │

│  2. SHAPING       EXPENSIVE     Unicode norm, ligatures,       │

│                                 kerning, BiDi, complex scripts │

│  3. LAYOUT        EXPENSIVE     Measure, line break, truncate  │

│  4. Rasterize     Medium        Beziers → pixels (cached)      │

│  5. Render        Cheap         Draw textured quads            │

```
└─────────────────────────────────────────────────────────────────┘
```

## 7.2 Cost Measurements

| Operation | Time | Impact |
| --- | --- | --- |
| Shape + layout 1 label | 50-200 µs | Per unique string |
| Shape 200 labels (uncached) | 10-40 ms | EXCEEDS 16ms budget |
| Draw 200 cached Galleys | 0.2 ms | Just vertices |
| Draw 200 icons | 0.1 ms | Trivial |

Shaping 200 labels without caching blows the entire 16ms frame budget. Icons are essentially free.

## 7.3 LOD Budget Rationale

Budget caps exist primarily to limit text shaping cost:

LOD        Budget       Text Cost       Rationale

```
───        ──────       ─────────       ─────────
```

Icon       unlimited    None            No text, draw millions

Label      250          1 line/entity   250 cached Galleys = OK

Extended   80           3 lines/entity  More shaping, tighter cap

Full       20           6+ lines        Most text, strictest cap

## 7.4 Label Cache Architecture

Shaped text (Galleys) cached by (string_id, max_width, lod). Cache hit = O(1):

```
struct LabelCache {
    cache: HashMap<LabelCacheKey, Arc<Galley>>,
    lru: LruList<LabelCacheKey>,
```

    max_entries: usize,  // ~500-1000

```
}
```

```
`#[derive(Hash, Eq, PartialEq)]`
struct LabelCacheKey {
```

    string_id: u32,       // Interned string index

    max_width: u16,       // Quantized to 10px buckets

```
    lod: LodTier,
}
```

## 7.5 Server/Client Split

```
┌─────────────────────────────────────────────────────────────────┐
```

│  SERVER                          WASM UI                       │

```
│  ──────                          ───────                       │
```

│  Intern strings (dedupe)         Shape strings → Galleys       │

│  Assign string_ids               Cache Galleys (LRU)           │

│  Send compact IDs                Draw cached Galleys           │

│                                                                 │

│  12,847 instruments              Cache ~500 Galleys (~2MB)     │

│  → ~2,000 unique labels          Only shape what's visible     │

│  → ~40KB string table            Amortize over frames          │

```
└─────────────────────────────────────────────────────────────────┘
```

Server cannot pre-shape because font/DPI/locale vary by client.

# 8. Unified Input Model

## 8.1 Input Priority

```
┌─────────────────────────────────────────────────────────────────┐
```

│  PRIORITY (highest to lowest)                                  │

```
│  ────────────────────────────                                  │
```

│                                                                 │

│  1. MOUSE / POINTER                                            │

│     └── User wants precision NOW                               │

│     └── Overrides any in-flight voice command                  │

│     └── Immediate visual feedback (hover, click)               │

│                                                                 │

│  2. KEYBOARD                                                   │

│     └── Power user rapid navigation                            │

│     └── Discrete commands (NEXT, PREV, DESCEND)                │

│     └── Vim-style muscle memory                                │

│                                                                 │

│  3. VOICE                                                      │

│     └── Hands-free, eyes-on-screen                             │

│     └── Higher latency (recognition time)                      │

│     └── Cancellable by pointer/keyboard                        │

```
└─────────────────────────────────────────────────────────────────┘
```

Rationale: When a user reaches for the mouse, they need accuracy fast. Mouse always wins.

## 8.2 Single Verb Interface

All input sources emit verbs. The verb layer is the single interface to state:

```
┌──────────┐
│ Keyboard │───┐
└──────────┘   │
               │         ┌─────────────┐      ┌─────────────┐
┌──────────┐   │         │             │      │             │
│  Voice   │───┼────────▶│ Verb Queue  │─────▶│ DroneState  │
└──────────┘   │         │             │      │             │
               │         └─────────────┘      └─────────────┘
┌──────────┐   │               ▲
│  Mouse   │───┘               │
└──────────┘                   │
```

     │                         │

```
     └─────── gesture ────────▶│
```

               recognizer

## 8.3 Gesture Recognition

```
enum Gesture {
    // Discrete
```

    Click(NodeId),

    DoubleClick(NodeId),

    ClickDoor(DoorId),

```
    // Hover
```

    HoverEnter(NodeId),

    HoverDwell(NodeId),     // Held for threshold

    HoverExit(NodeId),

```
    // Continuous
```

    DragStart(Vec2),

    DragMove(Vec2, Vec2),   // (delta, velocity)

    DragEnd(Vec2),          // Final velocity for momentum

```
}
```

## 8.4 Gesture to Verb Mapping

```
fn gesture_to_verb(gesture: Gesture, state: &DroneState) -> Option<Verb> {
    match gesture {
        Gesture::Click(node_id) => match state.mode {
            Spatial => Some(Verb::Focus(node_id)),
            Structural => Some(Verb::Select(node_id)),
        },
        Gesture::DoubleClick(node_id) => match state.mode {
            Spatial => find_door(node_id).map(Verb::DiveInto),
            Structural => Some(Verb::DescendTo(node_id)),
        },
        Gesture::HoverDwell(node_id) => Some(Verb::Preview(node_id)),
        Gesture::HoverExit(_) => Some(Verb::ClearPreview),
        Gesture::DragMove(delta, _) => Some(Verb::Pan(delta)),
        Gesture::ClickDoor(door_id) => Some(Verb::DiveInto(door_id)),
        _ => None,
    }
}
```

## 8.5 Complete Verb Table

| Verb | Keyboard | Mouse | Voice | Effect |
| --- | --- | --- | --- | --- |
| Select | Enter | Click | "select" | Set selection |
| Next | →, j, Tab | — | "right" | Sibling +1 |
| Prev | ←, k | — | "left" | Sibling -1 |
| Descend | ↓, l | Double-click | "down" | Into children |
| Ascend | ↑, h, Esc | — | "up", "back" | To parent |
| Preview | — | Hover dwell | — | Transient detail |
| Expand | e | — | "expand" | Inline children |
| Enhance | Space | — | "enhance" | Cycle LOD |

# 9. Fly-Over Navigation

When inspecting large structures, users need to move fast. LOD responds to movement, not just zoom.

## 9.1 Navigation Phase Model

```
enum NavigationPhase {
```

    Moving,     // Just navigated — icons only, fast

    Settling,   // Dwell timer started — animating to detail

    Focused,    // Dwell complete — full detail visible

```
}
```

## 9.2 Phase Transitions

```
┌─────────────────────────────────────────────────────────────────┐
```

│  User action        Phase          focus_t      Visual         │

```
│  ───────────        ─────          ───────      ──────         │
```

│                                                                 │

│  (start)            Moving         0.0          All icons      │

│     │                                                           │

│     │ ~0.5s dwell                                              │

│     ▼                                                           │

│  (pause)            Settling       0.0 → 1.0    Zoom to select │

│     │                              (animating)                  │

│     │ ~0.2s animation                                          │

│     ▼                                                           │

│  (focused)          Focused        1.0          Full detail    │

│     │                                                           │

│     │ User presses NEXT                                        │

│     ▼                                                           │

│  NEXT verb          Moving         1.0 → 0.0    Snap to icons  │

```
└─────────────────────────────────────────────────────────────────┘
```

## 9.3 Visual Behavior

MOVING (rapid navigation):

```
┌─────────────────────────────────────────────────────────────────┐
```

│  [●]  [●]  [●]  [●]  [●]  [●]  [●]  [●]  [●]  [●]  [●]  [●]   │

│                 ▲                                               │

│            selection (icon only)                               │

```
└─────────────────────────────────────────────────────────────────┘
```

FOCUSED (after dwell):

```
┌─────────────────────────────────────────────────────────────────┐
│  [●]  [●]  [●Fin]  ╔═══════════════════╗  [●Ene]  [●]  [●]    │
```

│                    ║ ● Technology      ║                       │

```
│                    ║   ─────────────   ║                       │
```

│                    ║   247 instruments ║                       │

│                    ║   38% of total    ║                       │

```
│                    ╚═══════════════════╝                       │
```

│                           ▲                                     │

│                    selection (full), siblings (label)          │

```
└─────────────────────────────────────────────────────────────────┘
```

## 9.4 Preview vs Selection

Preview is transient (from hover). Selection is persistent (from click/keyboard).

SELECTION                        PREVIEW

```
─────────                        ───────
```

• Persistent until changed       • Clears on hover-exit

• Drives phase / focus_t         • Doesn't affect phase

• Set by click or keyboard       • Set by hover dwell

• One at a time                  • Can coexist with selection

## 9.5 Fly-Over Eliminates Shaping During Movement

Zero text shaping while navigating. Only shape when settled:

Phase         Selection LOD    Siblings LOD    Shaping Load

```
─────         ─────────────    ────────────    ────────────
```

Moving        Icon             Icon            ZERO

Settling      Icon → Full      Icon → Label    Gradual

Focused       Full             Label           Stable (cached)

Rapid NEXT-NEXT-NEXT navigation: zero shaping, 60fps guaranteed.

# 10. Verb ↔ RIP Interface

Every verb returns explicit effects that the render loop must handle.

## 10.1 EffectSet Flags

```
bitflags! {
    struct EffectSet: u16 {
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

## 10.2 Effect Handler

```
fn handle_verb_effects(
    effects: EffectSet,
```

    state: &DroneState,

    label_cache: &mut LabelCache,

    detail_cache: &mut DetailCache,

    transition: &mut TransitionState,

```
) {
    if effects.contains(CHAMBER_CHANGED) {
```

        label_cache.reserve_for_chamber(current_chamber);

```
    }
    if effects.contains(SNAP_TRANSITION) {
```

        transition.start(snap_config);

```
    }
    if effects.contains(PREFETCH_DETAILS) {
```

        detail_cache.prefetch(collect_refs(state));

```
    }
    if effects.contains(SCROLL_ADJUST) {
```

        adjust_scroll_for_selection(state);

```
    }
}
```

## 10.3 Bidirectional Contract

VERB reads:                     VERB writes:

```
───────────                     ────────────
```

WorldSnapshot (immutable)       DroneState

├── chambers[]                  ├── current_chamber

├── doors[]                     ├── context_stack

├── first_child[]               ├── camera.*

├── next_sibling[]              ├── mode

├── prev_sibling[]              ├── lod_state.*

└── default_cameras             └── taxonomy.*

Returns: EffectSet → render loop actions

# 11. Input Synchronization Scenarios

These scenarios validate that all input methods work together correctly:

## 11.1 Mouse Override Voice

1. User says "go to Technology"

2. Voice command queued (pending confirmation)

3. User clicks on "Financials" (needs precision NOW)

4. Voice command CANCELLED

```
5. Verb::Select("Financials") executed
```

6. Selection = Financials, scroll animates

## 11.2 Hover During Keyboard Navigation

1. User presses NEXT → selection = "Finance"

2. User hovers mouse over "Healthcare"

3. State: selection = Finance, preview = Healthcare

4. Visual: Finance = selected (full), Healthcare = preview (full)

5. User presses NEXT → selection = "Healthcare"

6. Preview clears (selection now matches hover)

## 11.3 Voice Confirm Hover

1. User hovers over "Technology" → preview shows

2. User says "select"

```
3. preview_target used → Verb::Select("Technology")
```

4. selection = Technology, preview clears

## 11.4 Pan Then Keyboard

1. User drags to pan through list

2. selection_mode = Derived (center of viewport)

3. User presses NEXT

4. selection_mode = Explicit

5. Selection moves discretely, scroll follows

# 12. Server Compilation Performance

For large taxonomies, server-side parallelism is essential.

## 12.1 Pipeline Stages

| Stage | Time | Parallel? | Notes |
| --- | --- | --- | --- |
| DB Fetch | 10-100ms | Async | tokio |
| Parse AST (NOM) | 1-5ms | No | Sequential |
| Canonical Graph | 2-5ms | No | Tree-dependent |
| Chamberization | 1-2ms | No | Graph-dependent |
| String Interning | 1-5ms | Two-pass | rayon, no locks |
| Layout (grid/tree) | 1-20ms | Per-chamber | rayon |
| Layout (force) | 100-5000ms | YES | Critical path |
| Navigation Indices | 5-20ms | Per-chamber | rayon |
| Spatial Index | 5-20ms | Per-chamber | rayon |

Key insight: After chamberization, chambers are independent. Stages 5-9 parallelize.

## 12.2 Rayon Parallelism

```
use rayon::prelude::*;
```

```
pub fn compile_world(ast: &Ast, policy: &ChamberPolicy) -> Result<WorldSnapshot> {
    // SEQUENTIAL: Parse and chamberize (fast, tree-dependent)
    let graph = build_canonical_graph(ast)?;
    let chamber_defs = chamberize(&graph, policy)?;
```

```
    // PARALLEL: String interning (two-pass, no contention)
    let (string_table, chamber_defs) = intern_strings_parallel(&chamber_defs);
```

```
    // PARALLEL: Process each chamber independently
    let chambers: Vec<ChamberSnapshot> = chamber_defs
```

        .par_iter()                    // <── rayon parallel iterator

        .map(|def| compile_chamber(def, policy))

        .collect();

    Ok(WorldSnapshot { string_table, chambers, .. })

```
}
```

## 12.3 Two-Pass String Interning

Concurrent interning has lock contention. Two-pass avoids it:

```
fn intern_strings_parallel(chambers: &[ChamberDef]) -> (StringTable, Vec<ChamberWithIds>) {
    // Pass 1: Collect unique strings (parallel)
    let all_strings: HashSet<String> = chambers.par_iter()
```

        .flat_map(|c| c.entities.par_iter().map(|e| e.label.clone()))

        .collect();

```
    // Build index (sequential, fast)
    let table: Vec<String> = all_strings.into_iter().collect();
    let index: HashMap<&str, u32> = table.iter()
```

        .enumerate().map(|(i, s)| (s.as_str(), i as u32)).collect();

```
    // Pass 2: Replace strings with IDs (parallel)
    let updated = chambers.par_iter()
```

        .map(|c| replace_with_ids(c, &index))

        .collect();

```
    (StringTable(table), updated)
}
```

## 12.4 Compilation Benchmarks

| Scenario | Entities | 1 Thread | 8 Threads | Speedup |
| --- | --- | --- | --- | --- |
| Small CBU (grid) | 200 | 12ms | 12ms | 1×* |
| Medium CBU (grid) | 2,000 | 45ms | 20ms | 2.3× |
| Large taxonomy | 12,847 | 180ms | 55ms | 3.3× |
| Huge taxonomy | 50,000 | 650ms | 180ms | 3.6× |
| Force layout | 5,000 | 3200ms | 520ms | 6.2× |

* Below threshold: sequential faster (no rayon overhead)

# 13. Snapshot Caching

Compiled snapshots are deterministic. Cache them to eliminate redundant compilation.

## 13.1 Cache Key

```
`#[derive(Hash, Eq, PartialEq)]`
struct SnapshotCacheKey {
```

    cbu_id: u64,            // Which CBU

    source_hash: u64,       // Hash of source AST

    policy_hash: u64,       // Hash of chamber policy

    schema_version: u32,    // Snapshot format version

```
}
```

## 13.2 Cache Implementation

```
struct SnapshotCache {
```

    memory: LruCache<SnapshotCacheKey, Arc<WorldSnapshot>>,  // Hot

    disk_path: PathBuf,                                      // Cold

```
}
```

```
impl SnapshotCache {
```

    async fn get_or_compile(&self, cbu_id: u64, ast: &Ast, policy: &ChamberPolicy)

```
        -> Arc<WorldSnapshot>
    {
        let key = SnapshotCacheKey::compute(cbu_id, ast, policy);
```

```
        // 1. Memory cache (~1ms)
```

        if let Some(s) = self.memory.get(&key) { return s.clone(); }

```
        // 2. Disk cache (~5-10ms)
        if let Some(s) = self.load_disk(&key).await {
```

            self.memory.put(key, s.clone());

            return s;

```
        }
```

```
        // 3. Compile (~50-200ms)
        let snapshot = Arc::new(compile_world(ast, policy)?);
```

        self.memory.put(key, snapshot.clone());

        self.save_disk(&key, &snapshot).await;

        snapshot

```
    }
}
```

## 13.3 Invalidation

Source Data Change: DSL command modifies entities → invalidate by cbu_id.

Policy Change: Chamber policy updated → invalidate all (policy_hash changed).

Schema Upgrade: New snapshot format → invalidate all.

# 14. Client Frame Budget

## 14.1 16.67ms Budget Breakdown

```
┌─────────────────────────────────────────────────────────────────┐
```

│  Phase                  Budget      Notes                       │

```
│  ─────                  ──────      ─────                       │
```

│  1. Process verbs       < 1ms       Usually 0-2 verbs/frame    │

│  2. Update animations   < 1ms       camera lerp, focus_t       │

│  3. Visible query       < 2ms       Grid spatial index         │

│  4. LOD + budget        < 2ms       Policy lookup, maybe sort  │

│  5. Text shaping        < 3ms       Cache hits only*           │

│  6. Render              < 5ms       Icons, labels, cards       │

│  7. egui overhead       < 2ms       Layout, paint              │

```
│  ────────────────────────────────                               │
```

│  TOTAL                  ≈15ms       2ms headroom               │

```
└─────────────────────────────────────────────────────────────────┘
```

* Fly-over mode: 0ms shaping during Moving phase

## 14.2 What Breaks the Budget

Uncached shaping: 200 labels × 100µs = 20ms. EXCEEDS 16ms. Fly-over uses icons while moving.

Force sort every frame: Sort 10k entities × O(n log n) = ~5ms. Only sort when budget exceeded.

Full LOD on too many entities: 50 full cards × detailed layout = 10ms+. Budget caps prevent this.

# 15. Policy Configuration (Runtime, Hot-Reloadable)

Render Policy is **data-driven** and **reloadable at runtime**. This is essential while the visual grammar (icons, LOD thresholds, chamberization defaults, layout templates) is still evolving.

The policy is treated as an input to compilation:
- Server compiles `source_struct + render_policy` into a `WorldSnapshot`.
- The snapshot includes `policy_hash` so the client and caches can verify exact policy alignment.

## 15.1 Goals
- Change icons / layout / LOD thresholds **without rebuilding binaries**.
- Keep the client RIP loop **policy-agnostic** (it consumes resolved numeric IDs).
- Preserve determinism: policy changes must produce a new `policy_hash` and therefore a new snapshot.

## 15.2 Policy Format

Policy is expressed as an s-expression (or JSON/YAML equivalent), versioned and validated:

```
(chamber-policy :version 1

  ;; Entity kinds
  (defkind-schema
    (kind 0 CBU)
    (kind 1 LegalEntity)
    (kind 2 TaxonomyNode)
    (kind 3 Instrument))

  ;; Spatial LOD (zoom-driven)
  (defspatial-lod
    (tiers
      (icon     :zoom-max 0.8  :hysteresis 0.08)
      (label    :zoom-max 1.5  :hysteresis 0.10)
      (full     :zoom-max 999  :hysteresis 0.15))
    (budgets :icon unlimited :label 250 :full 20))

  ;; Flyover config
  (defchamber InstrumentMatrix :mode structural
    :flyover
      (dwell-ticks 30)
      (settle-duration 0.2)
      (moving  :selection-lod icon :siblings-lod icon)
      (focused :selection-lod full :siblings-lod label))
)
```

## 15.3 Loading, Validation, and Hashing

Policy is loaded by the server World Compiler at runtime:
1. Parse + validate against a policy schema.
2. Canonicalize (stable ordering, whitespace-insensitive).
3. Compute `policy_hash = hash(canonical_policy_bytes)`.

Validation should catch:
- Unknown kinds or missing kind mappings
- Invalid LOD tier ordering
- Impossible budgets (e.g. negative)
- Unknown chamber names / template references

## 15.4 Snapshot Linkage and Cache Keys

`policy_hash` must be part of:
- Snapshot envelope metadata
- Snapshot cache key (already included in Section 13)

**Rule**: if policy changes, caches must miss automatically and trigger a recompile.

## 15.5 Hot Reload (Dev) and Change Control (Prod)

**Development**
- Policy watcher reloads the policy file.
- Any change invalidates the snapshot cache (by `policy_hash`) and recompiles on demand.
- Telemetry/logging should print `(source_hash, policy_hash, schema_version)` for reproducibility.

**Production**
- Policy changes should be deployed as versioned artifacts.
- Rollout can be staged (A/B) by policy version if desired.
- Snapshots can be pre-warmed for common CBUs to avoid “first-hit” compile latency.

## 15.6 Client Responsibilities

For performance, the client RIP should not need to parse policy:
- Snapshots should already contain resolved numeric fields (`kind_id`, `icon_id`, LOD thresholds if embedded).
- Client only performs:
  - visible query (GridSnapshot)
  - LOD gating (phase + zoom)
  - cached shaping when allowed
  - draw batching

Optional: client may display the current `policy_hash` in a debug overlay for support and reproducibility.

## 15.7 Tuning Workflow (How parameters get fine-tuned)

The entire point of runtime policy is to support rapid iteration on **LOD**, **phase timing**, and **budgets** while preserving deterministic snapshots.

Recommended workflow:
1. **Change policy** (file edit or UI debug panel writes to policy).
2. Server validates + canonicalizes → new `policy_hash`.
3. Cache miss triggers recompile → new `WorldSnapshot`.
4. Client loads snapshot and displays `(source_hash, policy_hash)` in a debug overlay.
5. Performance counters confirm frame budget compliance.

Key tunables (initial set):
- LOD thresholds: `zoom-max` per tier + hysteresis
- Fly-over: `dwell-ticks`, `settle-duration`, `focus_t` easing
- Budgets: label/full counts; per-frame shaping budget (ms)
- Grid: `cell_size` (spatial query cost trade-off)
- Label cache: `max_entries`, width quantization buckets
- Structural mode density thresholds (icon/label/full cutovers)

# 16. Implementation

## 15.8 Policy Coverage Table

This table enumerates the **tuning knobs** that are expected to be adjusted during performance and UX refinement, and where each knob is applied (server compiler vs snapshot vs client RIP vs verb engine).

> **Rule of thumb**
> - **Bake into snapshot**: anything that affects structure/layout/indexing/IDs or would be expensive/branchy per-frame.
> - **Runtime (client)**: anything that affects budgets, timing, and thresholds that you’ll tweak frequently.

| Category | Policy key (suggested path) | Applied in | Baked into snapshot? | Hot-reload effect | Validation / Notes |
|---|---|---:|---:|---|---|
| **Schema / versioning** | `policy.version` | Server | Yes (envelope metadata) | Cache miss + recompile | Must be monotonic when breaking schema |
|  | `policy.name` / `policy.variant` | Server | Yes (envelope metadata) | Cache miss + recompile | Useful for rollout/A-B |
|  | `policy.canonicalization` | Server | Yes (hash) | Cache miss + recompile | Stable ordering/normalization required |
| **Kinds → visuals** | `kinds[kind_id].name` | Server | Optional | Cache miss + recompile | For debugging / overlay |
|  | `kinds[kind_id].icon_id` | Server | **Yes** | Cache miss + recompile | Resolves `icon_id[i]` in SoA |
|  | `kinds[kind_id].color_id` (optional) | Server | Optional | Cache miss + recompile | Prefer palette IDs not RGB |
| **Roles / styling** | `roles[role_id].badge_icon_id` | Server | Optional | Cache miss + recompile | Keep `role_mask` in snapshot; resolve badge later if needed |
| **Chamberization** | `chambers[*].enabled` | Server | Yes | Cache miss + recompile | Structural change |
|  | `chambers[*].membership.rules` | Server | Yes | Cache miss + recompile | Deterministic rules only |
|  | `chambers[*].door.enabled` | Server | Yes | Cache miss + recompile | Doors affect navigation graph |
| **Layout selection** | `chambers[*].layout.kind` (`grid/tree/radial/force`) | Server | **Yes** | Cache miss + recompile | Force layout must be seeded + stable |
|  | `chambers[*].layout.params.*` | Server | **Yes** | Cache miss + recompile | e.g. row/col spacing, radial radius |
|  | `chambers[*].layout.seed` | Server | Yes | Cache miss + recompile | Needed for deterministic force layouts |
| **Spatial index** | `chambers[*].grid.cell_size` | Server | **Yes** | Cache miss + recompile | Cell size trades query cost vs memory |
|  | `chambers[*].grid.max_cells` (optional) | Server | Yes | Cache miss + recompile | Prevent pathological memory growth |
| **LOD tiers (zoom)** | `lod.tiers[icon|label|full].zoom_max` | Client | No (or optional) | Immediate behavior change | If you bake, it forces recompile; keep runtime unless you pre-tokenize atoms by tier |
|  | `lod.tiers[*].hysteresis` | Client | No | Immediate | Prevents flicker at thresholds |
|  | `lod.manual_cycle_order` | Client/Verbs | No | Immediate | For ENHANCE “step through” behavior |
| **LOD budgets (counts)** | `budgets.label_budget_count` | Client | No | Immediate | Cap number of labels drawn/shaped |
|  | `budgets.full_budget_count` | Client | No | Immediate | Cap full detail nodes |
|  | `budgets.icons_unlimited` (bool) | Client | No | Immediate | Usually true |
| **LOD budgets (time)** | `budgets.shape_budget_ms_per_frame` | Client | No | Immediate | More robust than count-only caps |
|  | `budgets.visible_query_budget_ms` (optional) | Client | No | Immediate | If needed for worst-case chambers |
| **Navigation phases** | `phases.moving.duration_ms` (optional) | Client/Verbs | No | Immediate | Often implicit (while velocity > eps) |
|  | `phases.settling.duration_ms` | Client/Verbs | No | Immediate | Controls when shaping becomes allowed |
|  | `phases.focused.min_dwell_ms` | Client/Verbs | No | Immediate | Avoid “thrash” shaping |
| **Fly-over** | `flyover.dwell_ticks` | Verbs | No | Immediate | Drives auto focus cadence |
|  | `flyover.settle_duration_s` | Verbs | No | Immediate | Camera easing time |
|  | `flyover.easing` (`smoothstep`, `cubic`, etc.) | Verbs | No | Immediate | Deterministic pure fn |
|  | `flyover.mode_defaults[spatial|structural].*` | Verbs | No | Immediate | Separate tuning per mode |
| **Camera motion** | `camera.pan_speed` | Verbs | No | Immediate | Affects transition feel |
|  | `camera.zoom_speed` | Verbs | No | Immediate | Ditto |
|  | `camera.snap_epsilon` | Verbs | No | Immediate | When “arrived” is true |
|  | `camera.focus_padding` | Verbs | No | Immediate | Extra space around target |
| **Selection + focus rules** | `focus.selection_priority` | Verbs | No | Immediate | e.g. selected > hovered > recent |
|  | `focus.neighbor_ring_size` | Verbs/RIP | No | Immediate | “siblings/nearby also show label” |
|  | `focus.prefetch_radius_cells` | Client | No | Immediate | How much to prefetch around focus |
| **Text / labels** | `labels.priority_rules` | Server or Client | Optional | Depends | If rule affects `label_id` assignment, bake server-side; if it’s draw-time filtering, keep client |
|  | `labels.min_zoom_for_any_text` | Client | No | Immediate | Hard gate for MOVING |
|  | `label_cache.max_entries` | Client | No | Immediate | Cache size |
|  | `label_cache.width_quantization` | Client | No | Immediate | Stabilizes caching across slight width changes |
|  | `label_cache.eviction` (`lru`, `clock`) | Client | No | Immediate | Implementation-specific |
| **Structural mode density** | `structural.density_cutover.icon_only` | Client | No | Immediate | When density too high, force icon mode |
|  | `structural.density_cutover.labels` | Client | No | Immediate | Allow labels at lower density |
|  | `structural.max_labels_per_cluster` | Client | No | Immediate | Prevents explosions |
| **Detail / expansion** | `details.enabled` | Server/Client | Optional | Depends | If detail graph is compiled into snapshot → server |
|  | `details.max_prefetch_per_second` | Client | No | Immediate | Keep network sane |
|  | `details.none_id` (=0) | Contract | Yes | N/A | Must match Snapshot Contract sentinel |
| **Doors / cross-chamber** | `doors.enabled` | Server | Yes | Cache miss + recompile | Structural |
|  | `doors.arrival_behavior` (`focus_target`, `keep_zoom`) | Verbs | No | Immediate | Affects transition semantics |
| **Observability** | `debug.overlay.enabled` | Client | No | Immediate | Dev-only typically |
|  | `debug.overlay.show_hashes` | Client | No | Immediate | Show `(source_hash, policy_hash, schema_version)` |
|  | `metrics.stage_timings.enabled` | Server | No | Immediate | Perf counters on compiler |
| **Safety clamps** | `clamps.max_nodes_visible` | Client | No | Immediate | Hard fallback |
|  | `clamps.max_chambers_loaded` | Client | No | Immediate | Avoid memory blowups |
|  | `clamps.max_snapshot_bytes` | Server | Yes | Cache miss + recompile | Refuse pathological snapshots |

### Minimal “must-have knobs” for the first tuning cycle
1. `lod.tiers.*.zoom_max`  
2. `lod.tiers.*.hysteresis`  
3. `budgets.label_budget_count`  
4. `budgets.full_budget_count`  
5. `budgets.shape_budget_ms_per_frame`  
6. `flyover.dwell_ticks`  
7. `flyover.settle_duration_s`  
8. `camera.snap_epsilon`  
9. `chambers[*].grid.cell_size` *(server, baked)*  
10. `label_cache.max_entries`


## 16.1 Crate Structure

esper/

├── crates/

│   ├── esper_input/         # Unified input handling

│   │   ├── gesture.rs       # Mouse → Gesture

│   │   ├── voice.rs         # Voice → Command

│   │   └── keyboard.rs      # Key → Verb

│   │

│   ├── esper_core/          # Navigation engine

│   │   ├── verb.rs          # All verbs

│   │   ├── effect.rs        # EffectSet flags

│   │   ├── state.rs         # DroneState, TaxonomyState

│   │   ├── phase.rs         # NavigationPhase, fly-over

│   │   └── stack.rs         # Context stack

│   │

│   ├── esper_compiler/      # World Compiler (server)

│   │   ├── graph.rs         # Canonical graph

│   │   ├── chamber.rs       # Chamberization

│   │   ├── layout.rs        # Layout + parallelism

│   │   ├── intern.rs        # Two-pass string interning

│   │   └── emit.rs          # WorldSnapshot serialization

│   │

│   ├── esper_snapshot/      # Flat IR types (shared)

│   ├── esper_policy/        # Policy s-expr → Rust

│   │

│   └── esper_egui/          # Rendering

│       ├── label_cache.rs   # Text shaping cache

│       ├── spatial.rs       # Spatial renderer

│       ├── structural.rs    # Structural renderer

│       └── effect.rs        # Effect handler

│

└── config/policies/

## 16.2 Phase Plan

Phase 1: Core verbs, state, effect flags

Phase 2: Gesture recognizer, verb mapping

Phase 3: Fly-over phase model, focus_t interpolation

Phase 4: World Compiler, SoA snapshots, rayon parallelism

Phase 5: Label cache, text rendering

Phase 6: Policy system (schema + loader + canonicalization + hashing)
- Parse + validate policy at runtime
- Compute `policy_hash` from canonical bytes
- Integrate with snapshot caching (policy_hash in key)
- Dev hot-reload watcher (file change → invalidate cache → recompile on demand)

Phase 6.1: Tuning & observability harness
- Server metrics per stage (compile timings, snapshot bytes)
- Client debug overlay: show `(source_hash, policy_hash, schema_version)`, phase, LOD tier counts, cache hit rates
- Optional dev “policy panel” (sliders) that writes an override policy file or sends an update to server

Phase 7: Snapshot caching (memory + disk)

Phase 8: Voice integration

Phase 9: Serialization, replay

## 16.3 Tuning & Experimentation (TODO Pack)

This section exists to ensure that **all parameters you expect to tweak during testing** are:
- externalized (policy/config)
- observable (debug overlay + logs)
- safe to change (validated)
- reproducible (hashes + deterministic snapshot)

### 16.3.1 TODO — Policy schema coverage
- [ ] Define a formal policy schema with versioning (policy `:version`).
- [ ] Add validation errors with precise paths (e.g. `flyover.dwell-ticks must be >= 0`).
- [ ] Canonicalize policy (stable ordering) before hashing.

### 16.3.2 TODO — Hot reload in dev
- [ ] File watcher for `config/policies/*.sexp` (or chosen format).
- [ ] On change: reload + validate; if valid, update active policy and invalidate snapshot cache by new `policy_hash`.
- [ ] If invalid: keep last-known-good policy active and surface error in logs + UI overlay.

### 16.3.3 TODO — End-to-end tunables (minimum set)
**Fly-over + phases**
- [ ] `dwell-ticks`, `settle-duration`, easing function for `focus_t`.
- [ ] Mode-specific defaults (spatial vs structural).

**LOD thresholds + hysteresis**
- [ ] `zoom-max` per tier; hysteresis per tier.
- [ ] Manual LOD cycling order for ENHANCE.

**Budgets**
- [ ] `label_budget_count`, `full_budget_count`
- [ ] `shape_budget_ms_per_frame` (client-side) — cap shaping time rather than entity count alone.

**Spatial index**
- [ ] `grid.cell_size` per chamber kind (matrix vs CBU graph).

**Text**
- [ ] `label_cache.max_entries`, width quantization buckets, eviction policy.

**Structural mode density cutovers**
- [ ] thresholds for icon/label/full per level density.

### 16.3.4 TODO — Debug overlay (client)
- [ ] Display: `source_hash`, `policy_hash`, `schema_version`.
- [ ] Display: current `NavigationPhase`, `focus_t`, manual/auto LOD mode.
- [ ] Display: visible counts per LOD tier (icons/labels/full).
- [ ] Display: label cache hit rate + current entries.
- [ ] Display: per-frame timings (visible query ms, shaping ms, paint ms) in dev builds.

### 16.3.5 TODO — Metrics + regression tests (server)
- [ ] Structured timing spans per stage (graph/chamberize/layout/intern/emit/compress).
- [ ] Snapshot size metrics (string table bytes, chamber bytes, total bytes).
- [ ] Determinism test: same `(source_hash, policy_hash, schema_version)` → same snapshot hash (run nightly / CI).

### 16.3.6 Acceptance Criteria
- [ ] A policy change (e.g. label budget 250→120) takes effect **without recompilation** and results in a new `policy_hash`.
- [ ] UI debug overlay shows the new hash and visible-tier counts change as expected.
- [ ] During rapid NEXT/NEXT navigation, shaping time remains ~0ms (MOVING phase).
- [ ] Snapshots remain deterministic for identical inputs.

# 17. Open Questions

Q1: Should chambers support lazy loading for 50k+ entities?

Q2: Voice command confirmation delay — how long before commit?

Q3: Haptic feedback for touch navigation?

Q4: Accessibility: screen reader integration?

Q5: Multi-select for batch inspection?

Q6: Collaborative inspection — show where other users are looking?

Q7: Should the compiler support incremental re-chamberization?

Q8: JSON vs CBOR/MessagePack for snapshot transport?

Q9: SIMD for viewport culling (WASM SIMD support)?

Q10: Barnes-Hut for O(n log n) force layout?

# 18. Appendix

## 18.1 Anti-Patterns

✗ Shaping text in render loop without cache.

✗ Sorting every frame unconditionally.

✗ Allocating in frame loop.

✗ Full LOD on > 25 entities.

✗ Nested iteration over 10k+ entities.

## 18.2 Patterns

✓ Icons while moving, detail on dwell.

✓ Pre-compute indices at compile time.

✓ LRU cache for shaped text.

✓ Two-pass interning to avoid locks.

✓ Rayon par_iter for chamber processing.

## 18.3 Glossary

| Term | Definition |
| --- | --- |
| Verb | Atomic navigation command. All input emits verbs. |
| EffectSet | Flags returned by verb indicating what renderer must do. |
| Fly-Over | Movement-aware LOD: icons while moving, detail on dwell. |
| Phase | Moving → Settling → Focused. Drives LOD interpolation. |
| focus_t | 0.0 = icons, 1.0 = full detail. Animated. |
| Preview | Transient detail from hover. Doesn't change selection. |
| Selection | Persistent focus. Set by click/keyboard. |
| Galley | egui's pre-shaped text layout. Cached for reuse. |
| Label Cache | LRU cache of Galleys. Amortizes shaping cost. |
| World Compiler | Server-side: AST → flat IR with rayon parallelism. |
| SoA | Structure of Arrays: parallel arrays for cache efficiency. |
| TaxonomySnapshot | Flat IR for structural navigation with nav indices. |
| Two-Pass Intern | Collect strings, then assign IDs. No lock contention. |

— End of Document —
