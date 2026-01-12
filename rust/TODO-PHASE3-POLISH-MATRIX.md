# Phase 3: Polish + Trading Matrix Wiring

## Overview

Two parallel tracks:
1. **Board Control Polish** - Complete the control view UX
2. **Trading Matrix Wiring** - Connect matrix to viewport navigation

---

# Architecture: Server/Client Split

## The Rule

| Layer | Owns | Persists |
|-------|------|----------|
| **Server** | Entity structs, cluster membership, graph topology, edges | DB + session |
| **Client (RIP)** | Camera, zoom, LOD rules, compression, force layout, animation | Ephemeral |

## Server Provides (Authoritative)

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
    pub entity_type: EntityType,   // Server decides this
    pub label: String,
    pub sublabel: Option<String>,
    pub cluster_id: Uuid,          // Which cluster I belong to
}
```

**Server never knows:** screen positions, zoom level, which LOD is rendering

## Client Computes (Ephemeral)

```rust
/// Camera into the universe
pub struct ViewportCamera {
    pub center: Vec2,
    pub zoom: f32,
    pub target_center: Vec2,   // For animation
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

**Client never invents:** entity relationships, cluster membership, graph structure

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

**Visual progression:**
```
Zoom out (compression > 0.8):    ◉₄₇           (Icon + count)
Zoom mid (0.5 - 0.8):            [LU 47]       (Label)
Zoom in (0.2 - 0.5):             [Luxembourg]  (Full)
                                 [47 CBUs    ]
Zoom max (< 0.2):                [Luxembourg ]  (Expanded)
                                 [47 CBUs    ]
                                 [Top: AGI(8)]
```

## Camera Animation (Inspect/Back)

```rust
fn handle_inspect(&mut self, entity_id: Uuid) {
    // Focus changes (for highlight + HUD)
    self.focus.set(entity_id);
    
    // Camera animates TO entity, zoomed in
    self.camera.target_center = self.get_node_position(entity_id);
    self.camera.target_zoom = 1.0;
    
    // Data unchanged - all clusters still loaded
}

fn handle_back(&mut self) {
    self.focus.back();
    self.camera.target_zoom = 0.3;  // Zoom out to context
}

fn update_camera(&mut self, dt: f32) {
    let speed = 5.0;
    self.camera.zoom += (self.camera.target_zoom - self.camera.zoom) * speed * dt;
    self.camera.center += (self.camera.target_center - self.camera.center) * speed * dt;
}
```

## Force Simulation (Already Exists)

`ob-poc-graph/src/graph/force_sim.rs` handles:
- Repulsion between nodes
- Center attraction  
- Cluster attraction (via `parent_id`)
- Zoom-responsive compression
- Boundary containment

**Missing:** Cluster-specific attraction. Add:
```rust
pub cluster_attraction: f32,  // Pull children to their parent node

// In calculate_forces():
if let Some(parent_id) = &node.parent_id {
    if let Some(parent) = self.get_node(parent_id) {
        let to_parent = parent.position - node.position;
        forces[i] += to_parent * self.config.cluster_attraction;
    }
}
```

---

# Focus Model Refactor (DO THIS FIRST)

## Kill the Enum

**DELETE this garbage:**
```rust
// OLD - enum hell with hardcoded variants
pub enum ViewportFocusState {
    CbuContainer { cbu: CbuRef, enhance_level: u8 },
    BoardControl { anchor: EntityRef, source_cbu: Option<CbuRef>, enhance_level: u8 },
    InstrumentMatrix { cbu: CbuRef, matrix: MatrixRef, matrix_enhance: u8, container_enhance: u8 },
    // ... more variants as features grow = unmaintainable
}
```

**REPLACE with data:**
```rust
/// Viewport focus is just a set of node UUIDs
/// Entity type metadata comes from the graph (server-side)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ViewportFocus {
    /// Currently focused nodes (0 = nothing, 1 = single, n = multi-select)
    pub nodes: HashSet<Uuid>,
    /// Navigation history for back
    #[serde(skip)]
    pub history: Vec<HashSet<Uuid>>,
    /// Enhance level (zoom depth) - applies to current focus
    pub enhance_level: u8,
}

impl ViewportFocus {
    pub fn clear(&mut self) {
        if !self.nodes.is_empty() {
            self.history.push(self.nodes.clone());
        }
        self.nodes.clear();
    }
    
    pub fn set(&mut self, id: Uuid) {
        if !self.nodes.is_empty() {
            self.history.push(self.nodes.clone());
        }
        self.nodes.clear();
        self.nodes.insert(id);
    }
    
    pub fn add(&mut self, id: Uuid) {
        self.nodes.insert(id);
    }
    
    pub fn remove(&mut self, id: Uuid) {
        self.nodes.remove(&id);
    }
    
    pub fn toggle(&mut self, id: Uuid) {
        if !self.nodes.remove(&id) {
            self.nodes.insert(id);
        }
    }
    
    pub fn back(&mut self) -> bool {
        if let Some(prev) = self.history.pop() {
            self.nodes = prev;
            true
        } else {
            false
        }
    }
    
    pub fn contains(&self, id: &Uuid) -> bool { self.nodes.contains(id) }
    pub fn is_empty(&self) -> bool { self.nodes.is_empty() }
    pub fn len(&self) -> usize { self.nodes.len() }
    
    pub fn single(&self) -> Option<Uuid> {
        if self.nodes.len() == 1 {
            self.nodes.iter().next().copied()
        } else {
            None
        }
    }
    
    pub fn enhance(&mut self, delta: i8) {
        self.enhance_level = (self.enhance_level as i8 + delta).max(0) as u8;
    }
}
```

## Graph Nodes Have Metadata

Server already sends entity type - use it:

```rust
/// Node in the graph (from server)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: Uuid,
    pub entity_type: EntityType,  // Server provides this
    pub label: String,
    pub sublabel: Option<String>,
    pub position: Option<Vec2>,
    // ... rendering hints
}

/// Entity types - matches server taxonomy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    // CBU domain
    Cbu,
    Fund,
    Subfund,
    ShareClass,
    
    // Legal entities
    LegalEntity,
    ManCo,
    InvestmentManager,
    Custodian,
    TransferAgent,
    
    // Control domain
    ControlAnchor,
    UltimateBeneficialOwner,
    
    // Trading domain
    InstrumentClass,
    AssetClass,
    Market,
    
    // Service providers
    ServiceProvider,
    
    // Persons
    NaturalPerson,
    
    // Catch-all
    Unknown,
}
```

## Rendering: Check Focus + Entity Type

```rust
fn render_node(&self, node: &GraphNode, ui: &mut egui::Ui) {
    let is_focused = self.focus.contains(&node.id);
    
    // Base node rendering
    let fill = self.node_color(&node.entity_type, is_focused);
    // ... render shape
    
    // Focus ring if selected
    if is_focused {
        self.render_focus_ring(ui, node.rect);
    }
}

fn render_hud(&self, ui: &mut egui::Ui, graph: &Graph) {
    // HUD based on what's focused
    let focused_nodes: Vec<&GraphNode> = graph.nodes.iter()
        .filter(|n| self.focus.contains(&n.id))
        .collect();
    
    if focused_nodes.is_empty() {
        return; // No HUD when nothing focused
    }
    
    if focused_nodes.len() == 1 {
        let node = focused_nodes[0];
        match node.entity_type {
            EntityType::Cbu | EntityType::Fund => self.render_cbu_hud(ui, node),
            EntityType::ControlAnchor | EntityType::ManCo => self.render_control_hud(ui, node),
            EntityType::InstrumentClass | EntityType::AssetClass => self.render_matrix_hud(ui, node),
            EntityType::NaturalPerson => self.render_person_hud(ui, node),
            _ => self.render_generic_hud(ui, node),
        }
    } else {
        // Multi-select HUD
        self.render_multi_select_hud(ui, &focused_nodes);
    }
}
```

## Navigation is Focus Manipulation

```rust
// Click handlers
fn on_node_click(&mut self, node_id: Uuid, modifiers: Modifiers) {
    if modifiers.shift {
        self.focus.toggle(node_id);  // Multi-select
    } else {
        self.focus.set(node_id);     // Single select (pushes history)
    }
}

fn on_background_click(&mut self) {
    self.focus.clear();
}

// Keyboard
fn on_key(&mut self, key: KeyCode) {
    match key {
        KeyCode::Escape => { self.focus.clear(); }
        KeyCode::Backspace => { self.focus.back(); }  // Navigate back
        KeyCode::Plus | KeyCode::Equals => { self.focus.enhance(1); }
        KeyCode::Minus => { self.focus.enhance(-1); }
        _ => {}
    }
}

// Voice/chat commands map to same operations
fn handle_command(&mut self, cmd: &AgentCommand, graph: &Graph) {
    match cmd {
        AgentCommand::Focus { entity_id } => {
            self.focus.set(*entity_id);
        }
        AgentCommand::Inspect { target } => {
            // Find node by name/type in graph
            if let Some(node) = graph.find_by_label(target) {
                self.focus.set(node.id);
            }
        }
        AgentCommand::Back => {
            self.focus.back();
        }
        AgentCommand::Enhance => {
            self.focus.enhance(1);
        }
        AgentCommand::Reduce => {
            self.focus.enhance(-1);
        }
        _ => {}
    }
}
```

## Portal Nodes

"Board Control →" is just a graph node with `entity_type: EntityType::ControlAnchor`:

```rust
// Server includes portal as a node in the graph
GraphNode {
    id: portal_uuid,
    entity_type: EntityType::ControlAnchor,
    label: "Board Control →".to_string(),
    sublabel: Some("via Allianz Global Investors GmbH".to_string()),
    // Portal has metadata about where it leads
    portal_target: Some(PortalTarget {
        graph_id: control_sphere_graph_id,
        anchor_entity: manco_uuid,
    }),
}

// Click on portal = load new graph + focus anchor
fn on_portal_click(&mut self, portal: &GraphNode) {
    if let Some(target) = &portal.portal_target {
        self.load_graph(target.graph_id);
        self.focus.set(target.anchor_entity);
    }
}
```

## Files to Change

| File | Action |
|------|--------|
| `ob-poc-types/src/viewport.rs` | DELETE `ViewportFocusState` enum, ADD `ViewportFocus` struct |
| `ob-poc-types/src/lib.rs` | ADD/update `EntityType` enum, `GraphNode` struct |
| `ob-poc-ui/src/state.rs` | Replace focus state with `ViewportFocus` |
| `ob-poc-ui/src/app.rs` | Update all match arms on focus state → use focus.contains() |
| `ob-poc-graph/src/graph/*.rs` | Update rendering to use new model |
| Server routes | Ensure `entity_type` is populated on all graph nodes |

## Migration

1. Add new `ViewportFocus` struct alongside old enum
2. Update rendering to check `focus.contains(id)` instead of enum match
3. Update click handlers to call `focus.set(id)`
4. Delete enum variants one by one as code migrates
5. Remove old enum entirely

---

# Track A: Board Control Polish

## A1: HUD for Control Entities

HUD renders based on focused node's `entity_type`, not enum variant:

```rust
fn render_control_hud(&self, ui: &mut egui::Ui, node: &GraphNode) {
    // Fetch board controller summary from graph metadata or session
    let summary = self.graph.get_control_summary(&node.id);
    
    egui::Frame::none()
        .fill(Color32::from_rgba_unmultiplied(20, 25, 30, 220))
        .inner_margin(12.0)
        .rounding(4.0)
        .show(ui, |ui| {
            ui.label(RichText::new("BOARD CONTROL").strong().color(Color32::from_rgb(100, 180, 255)));
            ui.label(&node.label);
            
            if let Some(s) = summary {
                ui.horizontal(|ui| {
                    ui.label("Method:");
                    ui.label(format!("{:?}", s.method));
                    let conf_color = match s.confidence {
                        ControlConfidence::High => Color32::GREEN,
                        ControlConfidence::Medium => Color32::YELLOW,
                        ControlConfidence::Low => Color32::from_rgb(255, 100, 100),
                    };
                    ui.label(RichText::new(format!("{}%", (s.score * 100.0) as u8)).color(conf_color));
                });
            }
            
            // Back link
            if ui.link("← Back").clicked() {
                // self.focus.back() called in parent
            }
        });
}
```

## A2: Enhance = Graph Depth

`focus.enhance_level` passed to server when fetching graph:

```rust
async fn fetch_graph_for_focus(&mut self) {
    if let Some(node_id) = self.focus.single() {
        let depth = self.focus.enhance_level;
        let graph = api::fetch_graph(node_id, depth).await;
        self.graph = graph;
    }
}
```

`+`/`-` keys call `focus.enhance(delta)` then refetch.

## A3: Back = History Pop

Already in `ViewportFocus::back()`. Wire to:
- `Backspace` key
- HUD "← Back" link
- "back" voice command

## A4: Evidence Panel

Collapsible panel when control entity focused:

```rust
fn render_evidence_panel(&self, ui: &mut egui::Ui, node: &GraphNode) {
    if let Some(explanation) = self.graph.get_explanation(&node.id) {
        ui.collapsing("Evidence", |ui| {
            for ev in &explanation.evidence_refs {
                ui.label(format!("{} {}", ev.source_icon(), ev.description));
            }
        });
        
        if !explanation.data_gaps.is_empty() {
            ui.collapsing("⚠ Missing Data", |ui| {
                for gap in &explanation.data_gaps {
                    ui.label(gap);
                }
            });
        }
    }
}
```
            }
        });
    }
}
```

---

# Track B: Trading Matrix Wiring

## B1: Matrix Click → Focus Set

Matrix nodes are graph nodes with `entity_type: InstrumentClass | AssetClass | Market`:

```rust
// In matrix panel render
if ui.selectable_label(selected, &node.name).clicked() {
    self.focus.set(node.id);  // That's it. Just set focus.
}
```

No enum variant needed. Focus is a UUID set.

## B2: Graph Highlights Focused Nodes

Already handled by new model:

```rust
fn render_node(&self, node: &GraphNode, ui: &mut egui::Ui) {
    let is_focused = self.focus.contains(&node.id);
    
    if is_focused {
        self.render_focus_ring(ui, node.rect);
    }
    // ... rest of rendering
}
```

Works for ANY node type - matrix, control, CBU, person.

## B3: Matrix HUD

Renders when focused node has matrix entity type:

```rust
fn render_matrix_hud(&self, ui: &mut egui::Ui, node: &GraphNode) {
    egui::Frame::none()
        .fill(Color32::from_rgba_unmultiplied(20, 25, 30, 220))
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.label(RichText::new("TRADING MATRIX").strong().color(Color32::from_rgb(100, 200, 100)));
            ui.label(&node.label);
            
            if let Some(stats) = self.graph.get_matrix_stats(&node.id) {
                ui.label(format!("Markets: {} | Currencies: {}", stats.markets, stats.currencies));
            }
            
            if ui.link("← Back").clicked() {
                // self.focus.back()
            }
        });
}
```

## B4: Inspect Command

```rust
AgentCommand::Inspect { target } => {
    // Find node by label/type
    if let Some(node) = self.graph.find_by_label(target) {
        self.focus.set(node.id);
    }
}
```

"inspect equity" → finds node labeled "Equity" → focus.set(equity_node.id)

No special matrix handling needed - all nodes are equal.

---

# Implementation Order

## Batch 0: Server/Client Types (2h)
- [ ] Add `Cluster` struct to ob-poc-types (id, cluster_type, anchor, nodes, edges)
- [ ] Add `ClusterType` enum (Cbu, ControlSphere, TradingMatrix)
- [ ] Ensure `GraphNode` has `cluster_id` field
- [ ] Add `NodeLOD` enum to ob-poc-graph (Icon, Label, Full, Expanded)
- [ ] Add `ViewportCamera` struct (center, zoom, target_center, target_zoom)

## Batch 1: Focus Model (2h)
- [ ] Add `ViewportFocus` struct (nodes: HashSet, history, enhance_level)
- [ ] DELETE `ViewportFocusState` enum
- [ ] Wire keyboard: Escape=clear, Backspace=back, +/-=enhance

## Batch 2: Force Sim + LOD (2h)
- [ ] Add `cluster_attraction` to ForceConfig
- [ ] Wire cluster attraction in `calculate_forces()` using `parent_id`
- [ ] Add `NodeLOD::for_compression()` function
- [ ] Wire LOD selection in node render based on zoom + density

## Batch 3: Camera Animation (2h)
- [ ] Add camera lerp in update loop
- [ ] "inspect" command → focus.set + camera.target_* animation
- [ ] "back" command → focus.back + camera zoom out

## Batch 4: Click Handlers (2h)
- [ ] Graph node click → focus.set(id)
- [ ] Shift+click → focus.add(id)
- [ ] Background click → focus.clear()
- [ ] Matrix panel click → focus.set(id)

## Batch 5: HUD Rendering (4h)
- [ ] render_hud() checks focused nodes
- [ ] Dispatch to type-specific HUD based on entity_type
- [ ] Control HUD with evidence
- [ ] Matrix HUD with stats
- [ ] Generic HUD for other types

---

# Success Criteria

- [ ] Server returns Cluster structs with typed nodes
- [ ] Client renders based on zoom → LOD rules
- [ ] "inspect X" → smooth zoom animation to X
- [ ] "back" → smooth zoom out, clusters still there
- [ ] Click any node → focus ring appears
- [ ] Shift+click → multi-select
- [ ] Escape → clear focus
- [ ] Backspace → back to previous focus
- [ ] +/- → enhance level changes, graph refetches
- [ ] HUD shows context for focused node based on entity_type
- [ ] No enum matching anywhere - just focus.contains(id)
- [ ] Nodes collapse to icons when zoomed out (LOD)
- [ ] Clusters orbit their anchor via force sim

---

# Files to Modify

| File | Action |
|------|--------|
| `ob-poc-types/src/lib.rs` | ADD `Cluster`, `ClusterType`, ensure `EntityType` complete |
| `ob-poc-types/src/viewport.rs` | DELETE enum, ADD `ViewportFocus`, `ViewportCamera` |
| `ob-poc-graph/src/graph/force_sim.rs` | ADD `cluster_attraction` to config + force calc |
| `ob-poc-graph/src/graph/lod.rs` | NEW - `NodeLOD` enum + selection logic |
| `ob-poc-graph/src/graph/viewport.rs` | Camera animation, HUD routing by entity_type |
| `ob-poc-ui/src/state.rs` | Use new `ViewportFocus`, `ViewportCamera` |
| `ob-poc-ui/src/app.rs` | Replace enum match, add camera update loop |
| `ob-poc-graph/src/graph/*.rs` | Use `focus.contains(id)` for highlight, LOD for render |
