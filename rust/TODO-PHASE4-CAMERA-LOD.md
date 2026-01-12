# Phase 4: Camera + LOD + Cluster Architecture

## The Split

| Layer | Owns | Persists |
|-------|------|----------|
| **Server** | Entity structs, cluster membership, graph topology, edges | DB + session |
| **Client (RIP)** | Camera, zoom, LOD rules, compression, force layout, animation | Ephemeral |

---

## Server Types (Authoritative)

```rust
/// Cluster = logical grouping (CBU ecosystem, control sphere, etc.)
pub struct Cluster {
    pub id: Uuid,
    pub cluster_type: ClusterType,
    pub anchor: Uuid,              // The "sun" - central entity
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

pub enum ClusterType {
    Cbu,            // Fund + services + participants
    ControlSphere,  // Ownership chain up to UBOs
    TradingMatrix,  // Instruments + markets
}

pub struct GraphNode {
    pub id: Uuid,
    pub entity_type: EntityType,
    pub label: String,
    pub sublabel: Option<String>,
    pub cluster_id: Uuid,
}
```

---

## Client Types (Ephemeral)

```rust
/// Camera into the universe
pub struct ViewportCamera {
    pub center: Vec2,
    pub zoom: f32,
    pub target_center: Vec2,
    pub target_zoom: f32,
}

/// What's selected
pub struct ViewportFocus {
    pub nodes: HashSet<Uuid>,
    pub history: Vec<HashSet<Uuid>>,
    pub enhance_level: u8,
}

/// Level of detail based on zoom + density
pub enum NodeLOD {
    Icon,      // Dot + count badge
    Label,     // Shape + 2-char code
    Full,      // Full name + sublabel
    Expanded,  // Full + inline detail
}
```

---

## LOD Rules

```rust
impl NodeLOD {
    pub fn for_compression(compression: f32, node_count: usize) -> Self {
        let density_factor = (node_count as f32 / 20.0).min(1.0);
        let effective = compression + (density_factor * 0.3);
        
        match effective {
            c if c > 0.8 => NodeLOD::Icon,
            c if c > 0.5 => NodeLOD::Label,
            c if c > 0.2 => NodeLOD::Full,
            _ => NodeLOD::Expanded,
        }
    }
}
```

**Visual:**
```
compression > 0.8:    ◉₄₇           (Icon)
0.5 - 0.8:            [LU 47]       (Label)
0.2 - 0.5:            [Luxembourg]  (Full)
                      [47 CBUs    ]
< 0.2:                [Luxembourg ]  (Expanded)
                      [47 CBUs    ]
                      [Top: AGI(8)]
```

---

## Camera Animation

```rust
fn handle_inspect(&mut self, entity_id: Uuid) {
    self.focus.set(entity_id);
    self.camera.target_center = self.get_node_position(entity_id);
    self.camera.target_zoom = 1.0;
    // Data unchanged - all clusters still loaded
}

fn handle_back(&mut self) {
    self.focus.back();
    self.camera.target_zoom = 0.3;
}

fn update_camera(&mut self, dt: f32) {
    let speed = 5.0;
    self.camera.zoom += (self.camera.target_zoom - self.camera.zoom) * speed * dt;
    self.camera.center += (self.camera.target_center - self.camera.center) * speed * dt;
}
```

---

## Force Sim Enhancement

Add cluster-specific attraction (children orbit their anchor):

```rust
// Add to ForceConfig
pub cluster_attraction: f32,

// In calculate_forces()
if let Some(parent_id) = &node.parent_id {
    if let Some(parent) = self.get_node(parent_id) {
        let to_parent = parent.position - node.position;
        forces[i] += to_parent * self.config.cluster_attraction;
    }
}
```

---

# Implementation Order

## Batch 1: Types (2h)
- [ ] Add `Cluster`, `ClusterType` to ob-poc-types
- [ ] Add `ViewportCamera` struct
- [ ] Add `NodeLOD` enum to ob-poc-graph
- [ ] Ensure `GraphNode` has `cluster_id`

## Batch 2: Force Sim (2h)
- [ ] Add `cluster_attraction` to ForceConfig
- [ ] Wire attraction in `calculate_forces()` using `parent_id`

## Batch 3: LOD Rendering (2h)
- [ ] Add `NodeLOD::for_compression()` 
- [ ] Wire LOD selection in node render
- [ ] Icon/Label/Full/Expanded render paths

## Batch 4: Camera (2h)
- [ ] Add camera lerp in update loop
- [ ] "inspect" → focus.set + animate to target
- [ ] "back" → focus.back + zoom out

---

# Success Criteria

- [ ] Server returns Cluster structs
- [ ] Zoom out → nodes collapse to icons
- [ ] Zoom in → nodes expand to full detail
- [ ] "inspect X" → smooth zoom to X
- [ ] "back" → smooth zoom out, all clusters still there
- [ ] Clusters orbit their anchor (force sim)

---

# Files

| File | Action |
|------|--------|
| `ob-poc-types/src/lib.rs` | ADD `Cluster`, `ClusterType` |
| `ob-poc-types/src/viewport.rs` | ADD `ViewportCamera` |
| `ob-poc-graph/src/graph/lod.rs` | NEW - `NodeLOD` + render dispatch |
| `ob-poc-graph/src/graph/force_sim.rs` | ADD `cluster_attraction` |
| `ob-poc-ui/src/app.rs` | ADD camera update loop |
