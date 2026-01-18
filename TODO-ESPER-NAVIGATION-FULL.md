# TODO: ESPER Navigation System - Full Implementation

**Priority**: HIGH (core user experience)  
**Estimated Effort**: 3-5 days  
**Created**: 2025-01-18  
**Status**: ✅ LARGELY COMPLETE (reviewed 2025-01-18)

---

## Implementation Status Summary

| Feature | Status | Notes |
|---------|--------|-------|
| **EsperRenderState** | ✅ Complete | All toggle methods: xray, peel, shadow, illuminate, red_flag_scan, black_hole |
| **Scale Navigation** | ✅ Complete | Universe/Galaxy/System/Planet/Surface/Core all wired |
| **Drill Navigation** | ✅ Complete | DrillThrough, SurfaceReturn wired to galaxy focus stack |
| **Camera Animation** | ✅ Complete | Spring-based fly_to/zoom_to in Camera2D |
| **GalaxyView** | ✅ Complete | Cluster rendering, force simulation, drill actions |
| **X-ray/Peel/Shadow** | ✅ Complete | Toggle methods + render integration |
| **Illuminate/RedFlag/BlackHole** | ✅ Complete | Toggle methods + render integration |
| **Temporal Navigation** | ⏸️ Stubbed | Requires historical snapshot backend |
| **Orbital Navigation** | ⏸️ Stubbed | Requires continuous camera orbit animation |

### What's Working Now

1. **Voice/Chat commands** route through NavigationVerb handlers in `app.rs`
2. **Scale transitions** update view_level and trigger appropriate view switches
3. **Galaxy view** renders at Universe level with cluster drill-down
4. **EsperRenderState** toggles work and affect node rendering (alpha, highlight)
5. **Camera spring animations** provide smooth transitions

### Deferred (Requires Backend Support)

- **TimeRewind/TimeSlice** - Need historical snapshot API
- **TimeTrail** - Need entity history API  
- **Orbit animation** - Need continuous camera rotation loop

---  

## Overview

Implement the Blade Runner "Esper" inspired navigation system that lets users fly through the CBU/Group ownership universe using voice commands and keyboard shortcuts.

### Spatial Hierarchy (The Universe)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  UNIVERSE (ScaleLevel::Universe)                                            │
│  ┌─────────────────────────────────────────────────────────────────────────┐│
│  │  All CBUs - the complete portfolio                                      ││
│  │  • GalaxyView renders clusters (jurisdictions, ManCos, risk bands)      ││
│  │  • Click cluster → drill to Galaxy                                      ││
│  └─────────────────────────────────────────────────────────────────────────┘│
│                              ↓ ScaleGalaxy / drill_into                     │
│  ┌─────────────────────────────────────────────────────────────────────────┐│
│  │  GALAXY (ScaleLevel::Galaxy)                                            ││
│  │  Group of related CBUs - jurisdiction, client book, segment             ││
│  │  • Connected via ownership/control edges                                ││
│  │  • KYC/UBO taxonomy links these "solar systems"                         ││
│  │  • Click CBU → drill to System                                          ││
│  └─────────────────────────────────────────────────────────────────────────┘│
│                              ↓ ScaleSystem / drill_into                     │
│  ┌─────────────────────────────────────────────────────────────────────────┐│
│  │  SYSTEM (ScaleLevel::System)                                            ││
│  │  Single CBU - the trading unit view                                     ││
│  │  • Entity graph with ownership/control relationships                    ││
│  │  • Service taxonomy (products subscribed)                               ││
│  │  • Click entity → drill to Planet                                       ││
│  └─────────────────────────────────────────────────────────────────────────┘│
│                              ↓ ScalePlanet / drill_into                     │
│  ┌─────────────────────────────────────────────────────────────────────────┐│
│  │  PLANET (ScaleLevel::Planet)                                            ││
│  │  Single Entity - focused inspection                                     ││
│  │  • FocusCard shows entity detail                                        ││
│  │  • Neighborhood context (connected entities)                            ││
│  │  • ScaleSurface → show attributes, ScaleCore → show JSON                ││
│  └─────────────────────────────────────────────────────────────────────────┘│
│                              ↓ ScaleSurface / ScaleCore                     │
│  ┌─────────────────────────────────────────────────────────────────────────┐│
│  │  SURFACE/CORE (ScaleLevel::Surface, ScaleLevel::Core)                   ││
│  │  Entity detail - attributes and raw data                                ││
│  │  • DetailLevel::Attributes → expanded attribute cards                   ││
│  │  • DetailLevel::Raw → JSON/raw data view                                ││
│  └─────────────────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Current State Analysis

### What Exists (✓)

| Component | Location | Status |
|-----------|----------|--------|
| `ViewState` struct | `src/session/view_state.rs` | ✓ All ESPER fields defined |
| `ViewState` methods | `src/session/view_state.rs` | ✓ `drill_into`, `surface_up`, `start_trace`, etc. |
| `NavigationVerb` enum | `crates/ob-poc-ui/src/command.rs` | ✓ Full verb set |
| `CbuSession` | `src/session/cbu_session.rs` | ✓ Clean CBU set + undo/redo |
| `Camera2D` | `crates/ob-poc-graph/src/graph/camera.rs` | ✓ Spring animation |
| `GalaxyView` | `crates/ob-poc-graph/src/graph/galaxy.rs` | ✓ Cluster rendering |
| `ScaleLevel` enum | `src/session/view_state.rs` | ✓ Universe→Galaxy→System→Planet→Surface→Core |

### What's Stubbed (needs wiring)

| Component | Location | Issue |
|-----------|----------|-------|
| Verb handlers | `crates/ob-poc-ui/src/app.rs` | 53 TODOs - handlers just log, don't update state |
| Scale transitions | `app.rs` | No camera fly-to on scale change |
| Drill navigation | `app.rs` | No nav_stack push/pop |
| Graph rendering | `ob-poc-graph` | Doesn't read ViewState ESPER fields |
| X-ray/Shadow modes | `graph/viewport.rs` | Toggle methods exist but not called |
| Temporal views | `app.rs` | No historical data loading |

---

## Implementation Plan

### Phase 1: Wire Scale Navigation (ScaleLevel transitions)

**Goal**: Voice command "zoom out to universe" actually changes the view.

#### 1.1 Update ViewState on Scale Verbs

**File**: `crates/ob-poc-ui/src/app.rs`

Replace stub handlers with actual ViewState updates:

```rust
NavigationVerb::ScaleUniverse => {
    // 1. Update ViewState
    self.state.view_state.surface_to_universe();
    
    // 2. Tell graph to show galaxy clusters
    self.state.graph_widget.set_view_level(ViewLevel::Universe);
    
    // 3. Trigger camera fly-to center
    self.state.graph_widget.camera_fly_to_center();
    
    // 4. Clear any focused CBU/entity
    self.state.focused_cbu_id = None;
    self.state.focused_entity_id = None;
    
    // 5. Request universe data if not loaded
    if self.state.needs_universe_data() {
        self.schedule_fetch(FetchRequest::Universe);
    }
}

NavigationVerb::ScaleGalaxy { segment } => {
    // 1. Push current state to nav stack
    self.state.view_state.push_nav_stack();
    
    // 2. Update scale level
    self.state.view_state.scale_level = ScaleLevel::Galaxy;
    
    // 3. If segment specified, filter to that cluster
    if let Some(seg) = segment {
        self.state.graph_widget.focus_cluster(&seg);
    }
    
    // 4. Trigger camera animation
    self.state.graph_widget.camera_fly_to_cluster(&segment);
}

NavigationVerb::ScaleSystem { cbu_id } => {
    if let Some(cbu_id_str) = cbu_id {
        if let Ok(cbu_uuid) = Uuid::parse_str(&cbu_id_str) {
            // 1. Push nav stack
            self.state.view_state.push_nav_stack();
            
            // 2. Update ViewState
            self.state.view_state.scale_level = ScaleLevel::System;
            self.state.view_state.focus_cbu_id = Some(cbu_uuid);
            
            // 3. Load CBU into session (adds to cbu_ids set)
            self.state.cbu_session.load_cbu(cbu_uuid);
            
            // 4. Switch graph to CBU view
            self.state.graph_widget.set_view_level(ViewLevel::Cbu);
            self.state.graph_widget.load_cbu(cbu_uuid);
            
            // 5. Fetch CBU data
            self.schedule_fetch(FetchRequest::Cbu { cbu_id: cbu_uuid });
        }
    }
}

NavigationVerb::ScalePlanet { entity_id } => {
    if let Some(eid_str) = entity_id {
        if let Ok(entity_uuid) = Uuid::parse_str(&eid_str) {
            // 1. Push nav stack
            self.state.view_state.push_nav_stack();
            
            // 2. Update ViewState
            self.state.view_state.scale_level = ScaleLevel::Planet;
            self.state.view_state.focus_entity_id = Some(entity_uuid);
            
            // 3. Tell graph to focus entity
            self.state.graph_widget.focus_entity(entity_uuid);
            
            // 4. Camera fly to entity position
            self.state.graph_widget.camera_fly_to_entity(entity_uuid);
            
            // 5. Show focus card
            self.state.show_focus_card = true;
        }
    }
}

NavigationVerb::ScaleSurface => {
    // Stay on same entity, but expand detail level
    self.state.view_state.detail_level = DetailLevel::Attributes;
    self.state.view_state.scale_level = ScaleLevel::Surface;
    
    // Graph should expand the focus card to show all attributes
    self.state.graph_widget.set_detail_level(DetailLevel::Attributes);
}

NavigationVerb::ScaleCore => {
    // Raw JSON view
    self.state.view_state.detail_level = DetailLevel::Raw;
    self.state.view_state.scale_level = ScaleLevel::Core;
    
    self.state.graph_widget.set_detail_level(DetailLevel::Raw);
}
```

#### 1.2 Add ViewLevel to GraphWidget

**File**: `crates/ob-poc-graph/src/graph/mod.rs`

```rust
/// What level of the spatial hierarchy we're rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewLevel {
    /// Galaxy view - clusters of CBUs
    #[default]
    Universe,
    /// Cluster view - group of related CBUs
    Cluster,
    /// CBU view - single trading unit
    Cbu,
    /// Entity view - single entity focused
    Entity,
}

impl GraphWidget {
    /// Current view level
    pub view_level: ViewLevel,
    
    /// Set view level and trigger appropriate rendering mode
    pub fn set_view_level(&mut self, level: ViewLevel) {
        if self.view_level != level {
            self.view_level = level;
            self.needs_layout = true;
            
            // Adjust rendering based on level
            match level {
                ViewLevel::Universe => {
                    self.show_galaxy = true;
                    self.show_cbu_graph = false;
                }
                ViewLevel::Cluster => {
                    self.show_galaxy = true;  // But filtered to cluster
                    self.show_cbu_graph = false;
                }
                ViewLevel::Cbu => {
                    self.show_galaxy = false;
                    self.show_cbu_graph = true;
                }
                ViewLevel::Entity => {
                    self.show_galaxy = false;
                    self.show_cbu_graph = true;
                    self.show_focus_card = true;
                }
            }
        }
    }
}
```

#### 1.3 Camera Fly-To Methods

**File**: `crates/ob-poc-graph/src/graph/mod.rs`

```rust
impl GraphWidget {
    /// Animate camera to center of universe view
    pub fn camera_fly_to_center(&mut self) {
        let center = Pos2::ZERO;  // Or compute from galaxy bounds
        self.camera.fly_to(center);
        self.camera.zoom_to_fit(self.galaxy_bounds(), self.viewport_rect);
    }
    
    /// Animate camera to a cluster position
    pub fn camera_fly_to_cluster(&mut self, cluster_id: &str) {
        if let Some(cluster) = self.galaxy_view.get_cluster(cluster_id) {
            let pos = cluster.position;
            self.camera.fly_to(pos);
            self.camera.zoom_to(1.5);  // Closer zoom for cluster
        }
    }
    
    /// Animate camera to a CBU position (within cluster)
    pub fn camera_fly_to_cbu(&mut self, cbu_id: Uuid) {
        if let Some(pos) = self.galaxy_view.get_cbu_position(cbu_id) {
            self.camera.fly_to(pos);
            self.camera.zoom_to(2.0);
        }
    }
    
    /// Animate camera to an entity position (within CBU graph)
    pub fn camera_fly_to_entity(&mut self, entity_id: Uuid) {
        if let Some(pos) = self.get_entity_position(entity_id) {
            self.camera.fly_to(pos);
            self.camera.zoom_to(2.5);
        }
    }
}
```

---

### Phase 2: Wire Drill Navigation (Depth axis)

**Goal**: "Drill through" navigates into subsidiaries, "surface" returns up.

#### 2.1 Drill Through Handler

**File**: `crates/ob-poc-ui/src/app.rs`

```rust
NavigationVerb::DrillThrough => {
    // Only works if we have a focused entity
    if let Some(entity_id) = self.state.view_state.focus_entity_id {
        // 1. Call ViewState drill_into
        self.state.view_state.drill_into(entity_id, DrillDirection::Down);
        
        // 2. Request children/subsidiaries data
        self.schedule_fetch(FetchRequest::EntityChildren { 
            entity_id,
            depth: 1,
            direction: DrillDirection::Down,
        });
        
        // 3. Update graph to show expanded children
        self.state.graph_widget.expand_entity(entity_id, DrillDirection::Down);
        
        // 4. Camera zoom in slightly
        self.state.graph_widget.camera.zoom_in(1.3);
    }
}

NavigationVerb::SurfaceReturn => {
    // 1. Pop nav stack via ViewState
    if self.state.view_state.surface_up() {
        // 2. Update graph view based on restored state
        let scale = self.state.view_state.scale_level;
        self.state.graph_widget.set_view_level(scale.into());
        
        // 3. If we have a focus CBU, re-center on it
        if let Some(cbu_id) = self.state.view_state.focus_cbu_id {
            self.state.graph_widget.camera_fly_to_cbu(cbu_id);
        }
        
        // 4. Camera zoom out
        self.state.graph_widget.camera.zoom_out(1.3);
    }
}
```

#### 2.2 X-Ray Mode

**File**: `crates/ob-poc-ui/src/app.rs`

```rust
NavigationVerb::Xray => {
    // Toggle x-ray mode
    let currently_on = self.state.view_state.xray_mode;
    
    if currently_on {
        self.state.view_state.disable_xray();
        self.state.graph_widget.viewport.set_xray_mode(false);
    } else {
        // X-ray outer layers (ownership shell)
        self.state.view_state.enable_xray(vec!["ownership".into(), "control".into()]);
        self.state.graph_widget.viewport.set_xray_mode(true);
    }
}
```

#### 2.3 Peel Layer

```rust
NavigationVerb::Peel => {
    // Peel outermost layer to reveal next
    let current_depth = self.state.view_state.peel_depth;
    
    // Determine next layer to peel based on depth
    let layer = match current_depth {
        0 => "ownership",
        1 => "control", 
        2 => "services",
        _ => return,  // Can't peel further
    };
    
    self.state.view_state.peel_layer(layer.into());
    self.state.graph_widget.viewport.set_peel_depth(current_depth + 1);
}
```

---

### Phase 3: Wire Trace/Highlight Modes

**Goal**: "Follow the rabbit", "illuminate risks", "shadow mode" work visually.

#### 3.1 Follow Rabbit (Trace Ownership to Terminus)

**File**: `crates/ob-poc-ui/src/app.rs`

```rust
NavigationVerb::FollowRabbit { from_entity } => {
    let start_entity = from_entity
        .and_then(|s| Uuid::parse_str(&s).ok())
        .or(self.state.view_state.focus_entity_id);
    
    if let Some(entity_id) = start_entity {
        // 1. Start trace in ViewState
        self.state.view_state.start_trace(TraceMode::Control, Some(entity_id));
        
        // 2. Request ownership chain to terminus
        self.schedule_fetch(FetchRequest::OwnershipChain {
            from_entity: entity_id,
            to_terminus: true,
            max_depth: 10,
        });
        
        // 3. Tell graph to highlight the trace path when data arrives
        self.state.graph_widget.pending_trace = Some(PendingTrace {
            mode: TraceMode::Control,
            from_entity: entity_id,
        });
    }
}
```

#### 3.2 Illuminate Aspect

```rust
NavigationVerb::Illuminate { aspect } => {
    let illuminate_aspect = match aspect.to_lowercase().as_str() {
        "risks" | "risk" => IlluminateAspect::Risks,
        "documents" | "docs" => IlluminateAspect::Documents,
        "screenings" | "screening" => IlluminateAspect::Screenings,
        "gaps" | "missing" => IlluminateAspect::Gaps,
        "pending" => IlluminateAspect::Pending,
        _ => {
            web_sys::console::warn_1(&format!("Unknown illuminate aspect: {}", aspect).into());
            return;
        }
    };
    
    // 1. Update ViewState
    self.state.view_state.illuminate(illuminate_aspect);
    
    // 2. Tell graph widget to highlight
    self.state.graph_widget.set_illuminate_aspect(Some(illuminate_aspect));
}
```

#### 3.3 Shadow Mode

```rust
NavigationVerb::Shadow => {
    let currently_on = self.state.view_state.shadow_mode;
    
    if currently_on {
        self.state.view_state.disable_shadow();
        self.state.graph_widget.viewport.set_shadow_mode(false);
    } else {
        self.state.view_state.enable_shadow(RiskThreshold::Medium);
        self.state.graph_widget.viewport.set_shadow_mode(true);
    }
}
```

#### 3.4 Red Flag Scan

```rust
NavigationVerb::RedFlagScan => {
    let currently_on = self.state.view_state.red_flag_scan_active;
    
    if currently_on {
        self.state.view_state.stop_red_flag_scan();
        self.state.graph_widget.clear_red_flags();
    } else {
        self.state.view_state.start_red_flag_scan(Some(RedFlagCategory::All));
        
        // Request red flag data
        self.schedule_fetch(FetchRequest::RedFlags {
            scope: self.current_scope(),
        });
        
        self.state.graph_widget.enable_red_flag_rendering();
    }
}
```

#### 3.5 Black Hole (Data Gaps)

```rust
NavigationVerb::BlackHole => {
    let currently_on = self.state.view_state.black_hole_mode;
    
    if currently_on {
        self.state.view_state.disable_black_holes();
        self.state.graph_widget.clear_gap_highlights();
    } else {
        self.state.view_state.enable_black_holes(Some(GapType::All));
        
        // Request gap analysis
        self.schedule_fetch(FetchRequest::GapAnalysis {
            scope: self.current_scope(),
        });
        
        self.state.graph_widget.enable_gap_rendering();
    }
}
```

---

### Phase 4: Wire Temporal Navigation

**Goal**: "Rewind to last year", "show timeline" load historical data.

#### 4.1 Time Rewind

```rust
NavigationVerb::TimeRewind { target_date } => {
    let date = target_date
        .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| {
            // Default: 1 year ago
            Utc::now().date_naive() - chrono::Duration::days(365)
        });
    
    // 1. Update ViewState
    self.state.view_state.set_historical_view(date);
    
    // 2. Request historical snapshot
    self.schedule_fetch(FetchRequest::HistoricalSnapshot {
        scope: self.current_scope(),
        as_of_date: date,
    });
    
    // 3. Show temporal indicator
    self.state.show_temporal_indicator = true;
    self.state.temporal_indicator_date = Some(date);
}
```

#### 4.2 Time Slice (Comparison)

```rust
NavigationVerb::TimeSlice { date1, date2 } => {
    let d1 = date1
        .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| Utc::now().date_naive() - chrono::Duration::days(365));
    
    let d2 = date2
        .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| Utc::now().date_naive());
    
    // 1. Update ViewState
    self.state.view_state.set_comparison_view(d1, d2);
    
    // 2. Request both snapshots
    self.schedule_fetch(FetchRequest::SnapshotComparison {
        scope: self.current_scope(),
        date1: d1,
        date2: d2,
    });
    
    // 3. Graph will render diff view when data arrives
    self.state.graph_widget.set_comparison_mode(true);
}
```

#### 4.3 Time Trail (Entity History)

```rust
NavigationVerb::TimeTrail { entity_id } => {
    let eid = entity_id
        .and_then(|s| Uuid::parse_str(&s).ok())
        .or(self.state.view_state.focus_entity_id);
    
    if let Some(entity_uuid) = eid {
        // 1. Update ViewState
        self.state.view_state.start_time_trail(entity_uuid);
        
        // 2. Request entity history
        self.schedule_fetch(FetchRequest::EntityHistory {
            entity_id: entity_uuid,
            max_events: 50,
        });
        
        // 3. Enable timeline rendering
        self.state.graph_widget.enable_time_trail(entity_uuid);
    }
}
```

---

### Phase 5: Wire Orbital Navigation

**Goal**: Orbit around entity, rotate view, flip perspective.

#### 5.1 Orbit

```rust
NavigationVerb::Orbit { entity_id } => {
    let eid = entity_id
        .and_then(|s| Uuid::parse_str(&s).ok())
        .or(self.state.view_state.focus_entity_id);
    
    if self.state.view_state.orbit_active {
        // Stop orbiting
        self.state.view_state.stop_orbit();
        self.state.graph_widget.stop_orbit();
    } else if let Some(entity_uuid) = eid {
        // Start orbiting
        self.state.view_state.start_orbit(entity_uuid, 1.0);
        self.state.graph_widget.start_orbit(entity_uuid);
    }
}
```

#### 5.2 Add Orbit to GraphWidget

**File**: `crates/ob-poc-graph/src/graph/mod.rs`

```rust
impl GraphWidget {
    /// Orbit animation state
    orbit_center: Option<Uuid>,
    orbit_angle: f32,
    orbit_speed: f32,
    orbit_radius: f32,
    
    pub fn start_orbit(&mut self, center: Uuid) {
        if let Some(pos) = self.get_entity_position(center) {
            self.orbit_center = Some(center);
            self.orbit_angle = 0.0;
            self.orbit_speed = 0.5;  // Radians per second
            self.orbit_radius = 200.0;  // Pixels from center
        }
    }
    
    pub fn stop_orbit(&mut self) {
        self.orbit_center = None;
    }
    
    /// Called in tick() to update orbit
    fn update_orbit(&mut self, dt: f32) {
        if let Some(center_id) = self.orbit_center {
            if let Some(center_pos) = self.get_entity_position(center_id) {
                self.orbit_angle += self.orbit_speed * dt;
                
                let orbit_pos = Pos2::new(
                    center_pos.x + self.orbit_radius * self.orbit_angle.cos(),
                    center_pos.y + self.orbit_radius * self.orbit_angle.sin(),
                );
                
                self.camera.set_target(orbit_pos);
            }
        }
    }
}
```

#### 5.3 Flip Perspective

```rust
NavigationVerb::Flip => {
    self.state.view_state.flip_perspective();
    
    // Toggle between top-down and bottom-up ownership view
    self.state.graph_widget.toggle_perspective();
}
```

---

### Phase 6: Graph Widget ESPER Integration

**Goal**: GraphWidget reads ViewState ESPER fields and renders accordingly.

#### 6.1 ESPER Render State

**File**: `crates/ob-poc-graph/src/graph/mod.rs`

```rust
/// ESPER-specific render state (derived from ViewState)
#[derive(Debug, Clone, Default)]
pub struct EsperRenderState {
    /// Current scale level
    pub scale_level: ScaleLevel,
    
    /// X-ray mode active
    pub xray_mode: bool,
    pub xray_layers: Vec<String>,
    
    /// Peel depth
    pub peel_depth: u8,
    pub peeled_layers: Vec<String>,
    
    /// Shadow mode
    pub shadow_mode: bool,
    pub shadow_threshold: RiskThreshold,
    
    /// Illuminate aspect
    pub illuminate_aspect: Option<IlluminateAspect>,
    
    /// Red flag scan
    pub red_flag_scan: bool,
    pub red_flags: Vec<Uuid>,  // Entity IDs with flags
    
    /// Black hole mode
    pub black_hole_mode: bool,
    pub gap_entities: Vec<Uuid>,  // Entity IDs with gaps
    
    /// Active trace
    pub trace_mode: Option<TraceMode>,
    pub trace_path: Vec<Uuid>,  // Entity IDs in trace
    
    /// Temporal
    pub temporal_mode: TemporalMode,
    pub comparison_data: Option<ComparisonData>,
}

impl GraphWidget {
    /// Update ESPER render state from ViewState
    pub fn sync_from_view_state(&mut self, view_state: &ViewState) {
        self.esper_state.scale_level = view_state.scale_level;
        self.esper_state.xray_mode = view_state.xray_mode;
        self.esper_state.xray_layers = view_state.xray_layers.clone();
        self.esper_state.peel_depth = view_state.peel_depth;
        self.esper_state.peeled_layers = view_state.peeled_layers.clone();
        self.esper_state.shadow_mode = view_state.shadow_mode;
        self.esper_state.shadow_threshold = view_state.shadow_threshold;
        self.esper_state.illuminate_aspect = view_state.illuminate_aspect;
        self.esper_state.red_flag_scan = view_state.red_flag_scan_active;
        self.esper_state.black_hole_mode = view_state.black_hole_mode;
        self.esper_state.trace_mode = view_state.trace_mode;
        self.esper_state.temporal_mode = view_state.temporal_mode;
    }
}
```

#### 6.2 ESPER-Aware Rendering

**File**: `crates/ob-poc-graph/src/graph/render.rs`

```rust
impl GraphWidget {
    fn render_node(&self, painter: &Painter, node: &GraphNode, screen_pos: Pos2) {
        let mut alpha = 1.0;
        let mut stroke_color = self.node_color(node);
        let mut fill_color = stroke_color.gamma_multiply(0.2);
        
        // X-ray mode: make outer layers transparent
        if self.esper_state.xray_mode {
            if self.is_outer_layer_node(node) {
                alpha = 0.3;
            }
        }
        
        // Shadow mode: dim non-risk items
        if self.esper_state.shadow_mode {
            if !self.meets_risk_threshold(node, self.esper_state.shadow_threshold) {
                alpha = 0.2;
            }
        }
        
        // Illuminate mode: highlight specific aspects
        if let Some(aspect) = self.esper_state.illuminate_aspect {
            if self.node_has_aspect(node, aspect) {
                stroke_color = Color32::GOLD;
                fill_color = Color32::GOLD.gamma_multiply(0.3);
            } else {
                alpha = 0.5;
            }
        }
        
        // Red flag scan: highlight flagged entities
        if self.esper_state.red_flag_scan {
            if self.esper_state.red_flags.contains(&node.entity_id) {
                stroke_color = Color32::RED;
                fill_color = Color32::RED.gamma_multiply(0.3);
            }
        }
        
        // Black hole mode: highlight gaps
        if self.esper_state.black_hole_mode {
            if self.esper_state.gap_entities.contains(&node.entity_id) {
                stroke_color = Color32::from_rgb(128, 0, 128);  // Purple for gaps
                fill_color = Color32::BLACK;  // "Black hole"
            }
        }
        
        // Trace mode: highlight trace path
        if self.esper_state.trace_mode.is_some() {
            if self.esper_state.trace_path.contains(&node.entity_id) {
                stroke_color = Color32::LIGHT_BLUE;
                fill_color = Color32::LIGHT_BLUE.gamma_multiply(0.4);
            }
        }
        
        // Apply alpha
        stroke_color = stroke_color.gamma_multiply(alpha);
        fill_color = fill_color.gamma_multiply(alpha);
        
        // Render with computed colors
        painter.circle(screen_pos, node.radius, fill_color, Stroke::new(2.0, stroke_color));
    }
}
```

---

### Phase 7: App.rs Integration

**Goal**: Main update loop syncs ViewState → GraphWidget.

#### 7.1 Update Loop Integration

**File**: `crates/ob-poc-ui/src/app.rs`

```rust
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1. Process any pending ESPER verb from voice/keyboard/click
        self.process_pending_navigation_verbs();
        
        // 2. Sync ViewState to GraphWidget (before rendering)
        self.state.graph_widget.sync_from_view_state(&self.state.view_state);
        
        // 3. Tick animations (camera, orbit, etc.)
        let dt = ctx.input(|i| i.stable_dt);
        self.state.graph_widget.tick(dt);
        
        // 4. Render based on current ViewLevel
        match self.state.view_state.scale_level {
            ScaleLevel::Universe | ScaleLevel::Galaxy => {
                self.render_galaxy_view(ctx);
            }
            ScaleLevel::System | ScaleLevel::Planet => {
                self.render_cbu_view(ctx);
            }
            ScaleLevel::Surface | ScaleLevel::Core => {
                self.render_entity_detail_view(ctx);
            }
        }
        
        // 5. Render overlays (temporal indicator, trace path, etc.)
        self.render_esper_overlays(ctx);
        
        // 6. Request repaint for animations
        if self.state.graph_widget.is_animating() {
            ctx.request_repaint();
        }
    }
}
```

---

## Testing Checklist

### Scale Navigation
- [ ] "Zoom out to universe" → GalaxyView with all clusters
- [ ] Click cluster → ScaleGalaxy with filtered CBUs
- [ ] "Show me CBU X" → ScaleSystem with single CBU graph
- [ ] Click entity → ScalePlanet with focus card
- [ ] "Show attributes" → ScaleSurface with expanded detail
- [ ] "Show raw data" → ScaleCore with JSON view

### Drill Navigation
- [ ] "Drill through" from entity → Shows subsidiaries
- [ ] "Surface" → Returns to previous level
- [ ] Nav stack properly tracks depth
- [ ] Camera animates smoothly on drill/surface

### Trace/Highlight
- [ ] "Follow the rabbit" → Traces ownership to UBO
- [ ] "Illuminate risks" → Risk entities glow
- [ ] "Shadow mode" → Non-risk items dimmed
- [ ] "Red flag scan" → PEP/Sanctions highlighted
- [ ] "Black holes" → Data gaps shown as voids

### Temporal
- [ ] "Rewind to 2023" → Historical snapshot loaded
- [ ] "Compare to last year" → Side-by-side diff
- [ ] "Show timeline" → Entity history trail

### Orbital
- [ ] "Orbit around X" → Camera circles entity
- [ ] "Stop" → Orbit ends
- [ ] "Flip" → Perspective inverted

---

## Files to Modify

| File | Changes |
|------|---------|
| `crates/ob-poc-ui/src/app.rs` | Wire all 53 ESPER verb handlers |
| `crates/ob-poc-graph/src/graph/mod.rs` | Add `ViewLevel`, `EsperRenderState`, camera methods |
| `crates/ob-poc-graph/src/graph/render.rs` | ESPER-aware node/edge rendering |
| `crates/ob-poc-graph/src/graph/viewport.rs` | X-ray, shadow, peel mode toggles |
| `crates/ob-poc-graph/src/graph/galaxy.rs` | Cluster focus, CBU position lookup |
| `src/session/view_state.rs` | Add any missing helper methods |

---

## Success Criteria

1. **Voice**: "Show me Luxembourg funds" → Galaxy filters to LU cluster
2. **Voice**: "Drill into ABC Fund" → Camera flies to CBU, graph loads
3. **Voice**: "Who controls this?" → Ownership chain traced to UBO
4. **Voice**: "Illuminate gaps" → Missing data shown as black holes
5. **Voice**: "Rewind to 2023" → Historical view with temporal indicator
6. **Keyboard**: Arrow keys navigate, Enter drills, Escape surfaces
7. **Mouse**: Click cluster → drill, scroll → zoom, drag → pan
8. **Animation**: All transitions smooth (spring-based camera)
9. **Performance**: 60fps maintained during navigation


---

## Phase 8: Semantic Fallback Integration

**Goal**: Trie misses fall back to Candle semantic search, with auto-learning.

### 8.1 The Pattern

```
Voice: "magnify that thing"
         │
         ▼
    ┌─────────┐
    │  Trie   │ ──HIT──► execute() immediately (0µs)
    └────┬────┘
         │ MISS
         ▼
    ┌───────────────┐
    │ Candle search │ ~~15ms (1-2 frames, acceptable)
    │ with timeout  │
    └───────┬───────┘
            │
    ┌───────┴───────┬────────────────┐
    │               │                │
    ▼               ▼                ▼
 >0.80          0.50-0.80         <0.50
 confidence     confidence        or timeout
    │               │                │
    ▼               ▼                ▼
 execute()     disambiguate()   escalate_to_chat()
    +              UI                │
 auto-learn    "Did you mean?"      ▼
 (add to trie)                   Chat session
```

### 8.2 Implementation

**File**: `crates/ob-poc-ui/src/app.rs` (or new `esper_intent.rs`)

```rust
/// Semantic fallback timeout - acceptable frame budget for misses
const SEMANTIC_TIMEOUT: Duration = Duration::from_millis(50);

/// Confidence thresholds
const AUTO_EXECUTE_THRESHOLD: f32 = 0.80;
const DISAMBIGUATION_THRESHOLD: f32 = 0.50;

impl App {
    /// Handle voice/text input with trie + semantic fallback
    fn handle_esper_intent(&mut self, phrase: &str) {
        // =====================================================
        // FAST PATH: Trie hit (90%+ of cases, <1µs)
        // =====================================================
        if let Some(esper_match) = self.registry.lookup(&phrase) {
            self.execute_esper_command(esper_match.command);
            return;
        }
        
        // =====================================================
        // SLOW PATH: Semantic fallback (trie miss, ~15ms)
        // Only fires for novel phrases
        // =====================================================
        let search_result = self.semantic_search_with_timeout(phrase, SEMANTIC_TIMEOUT);
        
        match search_result {
            Some(result) if result.confidence > AUTO_EXECUTE_THRESHOLD => {
                // High confidence: execute + auto-learn
                self.execute_esper_command(result.command.clone());
                
                // Add to trie so next time it's instant
                self.registry.add_learned_alias(phrase, &result.command_key);
                
                // Persist to DB (fire and forget)
                self.persist_learned_alias(phrase, &result.command_key, result.confidence);
            }
            
            Some(result) if result.confidence > DISAMBIGUATION_THRESHOLD => {
                // Ambiguous: show quick alternatives
                self.show_disambiguation_ui(phrase, result.alternatives);
            }
            
            _ => {
                // Total miss or timeout: escalate to chat
                self.escalate_to_chat(phrase);
            }
        }
    }
    
    /// Semantic search with hard timeout (blocks, but bounded)
    fn semantic_search_with_timeout(
        &self, 
        phrase: &str, 
        timeout: Duration
    ) -> Option<SemanticSearchResult> {
        // Use tokio::time::timeout or std::thread with timeout
        // This is the Candle embed + pgvector search
        
        let embedder = self.embedder.as_ref()?;
        
        // Blocking is OK here - it's only 15ms and only on trie miss
        let embedding = embedder.embed_blocking(phrase).ok()?;
        
        // Search navigation commands (not DSL verbs)
        self.search_esper_commands_by_embedding(&embedding, timeout)
    }
    
    /// Show quick disambiguation (not full chat)
    fn show_disambiguation_ui(&mut self, phrase: &str, alternatives: Vec<Alternative>) {
        // Modal or inline UI: "Did you mean?"
        // [Zoom In] [Pan Left] [Something else...]
        self.state.disambiguation = Some(DisambiguationState {
            original_phrase: phrase.to_string(),
            alternatives,
            selected: None,
        });
    }
    
    /// User selected from disambiguation
    fn handle_disambiguation_selection(&mut self, selected: &Alternative) {
        // Execute the selected command
        self.execute_esper_command(selected.command.clone());
        
        // Learn it (high confidence since user confirmed)
        let phrase = self.state.disambiguation.as_ref()
            .map(|d| d.original_phrase.clone())
            .unwrap_or_default();
        
        self.registry.add_learned_alias(&phrase, &selected.command_key);
        self.persist_learned_alias(&phrase, &selected.command_key, 0.95);
        
        // Clear disambiguation UI
        self.state.disambiguation = None;
    }
    
    /// Escalate to chat for complex/unclear intents
    fn escalate_to_chat(&mut self, phrase: &str) {
        // Open chat panel with context
        self.state.chat_open = true;
        self.state.chat_context = Some(ChatContext::UnresolvedIntent {
            phrase: phrase.to_string(),
            suggestion: "I didn't understand that navigation command. Can you rephrase?".into(),
        });
    }
}
```

### 8.3 Embedding ESPER Commands

Need to embed the ESPER command aliases for semantic search. Add to startup:

**File**: `src/agent/esper/registry.rs`

```rust
impl EsperCommandRegistry {
    /// Pre-compute embeddings for all builtin aliases
    pub async fn compute_embeddings(&mut self, embedder: &dyn Embedder) -> Result<()> {
        for (key, def) in &self.commands {
            // Embed all exact aliases
            for alias in &def.aliases.exact {
                let embedding = embedder.embed(alias).await?;
                self.alias_embeddings.insert(alias.clone(), EmbeddedAlias {
                    command_key: key.clone(),
                    embedding,
                });
            }
            
            // Also embed the canonical phrase (command description)
            if let Some(desc) = &def.description {
                let embedding = embedder.embed(desc).await?;
                self.description_embeddings.insert(key.clone(), embedding);
            }
        }
        Ok(())
    }
    
    /// Search by embedding (called on trie miss)
    pub fn search_by_embedding(
        &self, 
        query_embedding: &[f32], 
        top_k: usize
    ) -> Vec<SemanticMatch> {
        let mut matches: Vec<_> = self.alias_embeddings
            .iter()
            .map(|(alias, entry)| {
                let similarity = cosine_similarity(query_embedding, &entry.embedding);
                SemanticMatch {
                    command_key: entry.command_key.clone(),
                    matched_alias: alias.clone(),
                    confidence: similarity,
                }
            })
            .collect();
        
        matches.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        matches.truncate(top_k);
        matches
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    // Embeddings are L2-normalized, so dot product = cosine
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}
```

### 8.4 Self-Healing Vocabulary Flow

```
Day 1: User says "make it bigger"
       ├─ Trie: MISS
       ├─ Semantic: ZoomIn (0.84)
       ├─ Execute + Learn
       └─ Trie now has: "make it bigger" → ZoomIn

Day 2: User says "make it bigger"
       ├─ Trie: HIT (learned)
       └─ Execute instantly (<1µs)

Day 3: User says "embiggen"
       ├─ Trie: MISS
       ├─ Semantic: ZoomIn (0.78) ← similar to "make it bigger" embedding
       ├─ Disambiguation: "Did you mean Zoom In?"
       ├─ User confirms
       └─ Trie now has: "embiggen" → ZoomIn
```

### 8.5 Database Schema for Learned Aliases

**File**: `migrations/0XX_esper_learned_aliases.sql`

```sql
-- Learned ESPER navigation aliases (per-user vocabulary)
CREATE TABLE IF NOT EXISTS agent.esper_learned_aliases (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id),
    phrase TEXT NOT NULL,
    command_key TEXT NOT NULL,
    confidence REAL NOT NULL,
    source TEXT DEFAULT 'auto',  -- 'auto', 'disambiguation', 'chat', 'explicit'
    use_count INT DEFAULT 1,
    created_at TIMESTAMPTZ DEFAULT now(),
    last_used_at TIMESTAMPTZ DEFAULT now(),
    
    -- Embedding for semantic clustering
    embedding vector(384),
    
    UNIQUE(user_id, phrase)
);

-- Index for loading user's vocabulary at session start
CREATE INDEX idx_esper_aliases_user ON agent.esper_learned_aliases(user_id);

-- Index for semantic search fallback
CREATE INDEX idx_esper_aliases_embedding 
ON agent.esper_learned_aliases 
USING ivfflat (embedding vector_cosine_ops) WITH (lists = 50);
```

### 8.6 Startup: Load Learned Vocabulary

```rust
impl App {
    async fn load_user_vocabulary(&mut self, user_id: Uuid) -> Result<()> {
        let aliases: Vec<LearnedAlias> = sqlx::query_as(
            "SELECT phrase, command_key FROM agent.esper_learned_aliases 
             WHERE user_id = $1 
             ORDER BY use_count DESC 
             LIMIT 1000"
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        
        for alias in aliases {
            self.registry.add_learned_alias(&alias.phrase, &alias.command_key);
        }
        
        Ok(())
    }
}
```

---

## Phase 8 Testing Checklist

### Fast Path (Trie)
- [ ] "zoom in" → instant execution, no semantic search
- [ ] "show universe" → instant execution
- [ ] Known learned phrase → instant execution

### Slow Path (Semantic)
- [ ] Novel phrase → semantic search completes in <50ms
- [ ] High confidence hit → auto-execute + auto-learn
- [ ] Medium confidence → disambiguation UI appears
- [ ] Low confidence → chat escalation

### Learning Loop
- [ ] Auto-learned phrase → trie hit on next use
- [ ] Disambiguation selection → persisted to DB
- [ ] User vocabulary survives session restart

### Thresholds
- [ ] 0.85+ confidence → no hesitation, just works
- [ ] 0.70-0.85 → maybe add micro-confirmation?
- [ ] <0.50 → definitely needs help

---

## Files to Modify (Phase 8)

| File | Changes |
|------|---------|
| `crates/ob-poc-ui/src/app.rs` | `handle_esper_intent()` with fallback |
| `src/agent/esper/registry.rs` | `compute_embeddings()`, `search_by_embedding()` |
| `src/agent/learning/embedder.rs` | `embed_blocking()` for sync path |
| `migrations/0XX_esper_learned_aliases.sql` | NEW - learned alias storage |
| `crates/ob-poc-ui/src/disambiguation.rs` | NEW - quick "did you mean?" UI |

---

## Summary: The Complete Intent Pipeline

```
Voice/Text Input
       │
       ▼
┌──────────────────────────────────────────────────────────────┐
│  1. Trie Lookup (<1µs)                                       │
│     • Builtin aliases (50+ commands)                         │
│     • User's learned aliases (grows over time)               │
│     HIT → execute immediately                                │
└──────────────────────────┬───────────────────────────────────┘
                           │ MISS (novel phrase)
                           ▼
┌──────────────────────────────────────────────────────────────┐
│  2. Candle Semantic Search (~15ms, 1-2 frames)               │
│     • Embed phrase with all-MiniLM-L6-v2                     │
│     • Cosine similarity against command embeddings           │
│     • Return top-3 with confidence scores                    │
└──────────────────────────┬───────────────────────────────────┘
                           │
         ┌─────────────────┼─────────────────┐
         ▼                 ▼                 ▼
     >0.80             0.50-0.80          <0.50
   confidence         confidence         confidence
         │                 │                 │
         ▼                 ▼                 ▼
    Execute +         Quick UI:         Escalate:
    Auto-learn       "Did you mean?"    Chat session
         │                 │                 │
         └────────┬────────┘                 │
                  ▼                          │
         Add to trie                         │
         (next time = instant)               │
                                             ▼
                                    LLM clarification
                                    (async, out of band)
```

**Result**: 90%+ of commands execute in <1µs. Novel phrases work within 2 frames. Vocabulary self-improves with use.
