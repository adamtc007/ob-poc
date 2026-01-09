# TODO: Esper DSL + Render Pipeline - Corrected Architecture

> **Priority:** CRITICAL - UAT Blocking
> **Status:** Clarified two-layer split

---

## Architecture: Server DSL vs WASM Render

```
┌─────────────────────────────────────────────────────────────────┐
│                    SERVER (Rust API)                            │
│  DSL Executor → ViewState → JSON response                       │
│  view.yaml verbs: view.universe, view.cbu, view.drill, etc.    │
│  These SET the session's view state (WHAT to show)             │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ HTTP API (JSON)
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    WASM (egui)                                  │
│  NavigationVerb → local rendering state                         │
│  Xray, Peel, Shadow, RedFlagScan, BlackHole, Illuminate        │
│  These control HOW the current view is RENDERED                │
└─────────────────────────────────────────────────────────────────┘
```

---

## Layer 1: Server DSL Verbs (Require Round-Trip)

These verbs change **WHAT data** is displayed. They hit the server, query the database, return new ViewState.

| Verb | Purpose | Server Action |
|------|---------|---------------|
| `view.universe` | Show all CBUs | Fetch CBU list |
| `view.cbu` | Focus on CBU | Fetch CBU + members |
| `view.entity` | Focus on entity | Fetch entity + relationships |
| `view.drill` | Show children/parents | Fetch hierarchy level |
| `view.surface` | Return to top | Clear drill state |
| `view.who-controls` | Control chain | Fetch ownership path |
| `view.follow-money` | Financial trace | Fetch financial relationships |
| `view.time-rewind` | Historical view | Fetch point-in-time data |
| `view.time-slice` | Compare dates | Fetch two snapshots |

**Implementation:** In `rust/src/dsl_v2/custom_ops/view_ops.rs` - server-side handlers that query DB and return JSON.

---

## Layer 2: WASM Render Modes (Local State, No Round-Trip)

These verbs change **HOW data is rendered**. They're local toggles in the egui WASM client.

| NavigationVerb | Render Effect | Local State Change |
|----------------|---------------|-------------------|
| `Xray` | Transparency mode | `graph_widget.xray_mode = true` |
| `Peel` | Hide outer layer | `graph_widget.peel_depth += 1` |
| `Shadow` | Dim non-risk entities | `graph_widget.shadow_mode = true` |
| `RedFlagScan` | Highlight risk flags | `graph_widget.red_flag_highlight = true` |
| `BlackHole` | Show data gaps | `graph_widget.show_gaps = true` |
| `Illuminate` | Glow specific aspect | `graph_widget.illuminate_aspect = Some(aspect)` |
| `DepthIndicator` | Show depth rings | `graph_widget.show_depth = true` |
| `CrossSection` | Cut-through view | `graph_widget.cross_section = Some(axis, pos)` |
| `Orbit` | Camera orbit mode | `graph_widget.orbit_mode = true` |
| `Tilt` | Dimension emphasis | `graph_widget.tilt = Some(dim, amount)` |
| `Flip` | Perspective flip | `graph_widget.flipped = !flipped` |

**Implementation:** In `rust/crates/ob-poc-ui/src/app.rs` - local state mutations, no HTTP calls.

---

## WASM RenderState Struct

**File:** `rust/crates/ob-poc-ui/src/render_state.rs`

```rust
/// Local render state - affects HOW data is drawn, not WHAT data
/// No server round-trips for these toggles
#[derive(Debug, Clone, Default)]
pub struct RenderState {
    // === Layer Visibility ===
    pub xray_mode: bool,
    pub xray_layers: Vec<String>,
    pub peel_depth: u8,
    
    // === Highlighting ===
    pub shadow_mode: bool,
    pub shadow_threshold: RiskThreshold,
    pub red_flag_highlight: bool,
    pub red_flag_category: Option<RedFlagCategory>,
    pub show_gaps: bool,          // BlackHole mode
    pub gap_type: Option<GapType>,
    pub illuminate_aspect: Option<IlluminateAspect>,
    
    // === Overlays ===
    pub show_depth_indicator: bool,
    pub cross_section: Option<CrossSection>,
    
    // === Camera/Perspective ===
    pub orbit_mode: bool,
    pub orbit_speed: f32,
    pub tilt: Option<(String, f32)>,  // (dimension, amount)
    pub flipped: bool,
}

#[derive(Debug, Clone)]
pub struct CrossSection {
    pub axis: CrossSectionAxis,
    pub position: f32,  // 0.0 to 1.0
}

impl RenderState {
    pub fn toggle_xray(&mut self) {
        self.xray_mode = !self.xray_mode;
    }
    
    pub fn peel_layer(&mut self) {
        self.peel_depth += 1;
    }
    
    pub fn unpeel_layer(&mut self) {
        self.peel_depth = self.peel_depth.saturating_sub(1);
    }
    
    pub fn toggle_shadow(&mut self, threshold: Option<RiskThreshold>) {
        self.shadow_mode = !self.shadow_mode;
        if let Some(t) = threshold {
            self.shadow_threshold = t;
        }
    }
    
    pub fn toggle_red_flags(&mut self, category: Option<RedFlagCategory>) {
        self.red_flag_highlight = !self.red_flag_highlight;
        self.red_flag_category = category;
    }
    
    pub fn toggle_gaps(&mut self, gap_type: Option<GapType>) {
        self.show_gaps = !self.show_gaps;
        self.gap_type = gap_type;
    }
    
    pub fn set_illuminate(&mut self, aspect: Option<IlluminateAspect>) {
        self.illuminate_aspect = aspect;
    }
    
    pub fn toggle_depth_indicator(&mut self) {
        self.show_depth_indicator = !self.show_depth_indicator;
    }
    
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
```

---

## Wire NavigationVerb to RenderState

**File:** `rust/crates/ob-poc-ui/src/app.rs`

```rust
impl App {
    /// Handle NavigationVerb - decide if server call or local render toggle
    fn handle_navigation(&mut self, verb: NavigationVerb) {
        match verb {
            // === SERVER DSL VERBS (require round-trip) ===
            NavigationVerb::ScaleUniverse => {
                self.execute_server_dsl("(view.universe)");
            }
            NavigationVerb::ScaleBook { client_name } => {
                self.execute_server_dsl(&format!("(view.book :client \"{}\")", client_name));
            }
            NavigationVerb::ScaleSystem { cbu_id } => {
                if let Some(id) = cbu_id {
                    self.execute_server_dsl(&format!("(view.cbu :cbu-id \"{}\")", id));
                }
            }
            NavigationVerb::ScalePlanet { entity_id } => {
                if let Some(id) = entity_id {
                    self.execute_server_dsl(&format!("(view.entity :entity-id \"{}\")", id));
                }
            }
            NavigationVerb::DrillThrough => {
                self.execute_server_dsl("(view.drill :direction \"down\")");
            }
            NavigationVerb::SurfaceReturn => {
                self.execute_server_dsl("(view.surface)");
            }
            NavigationVerb::WhoControls { entity_id } => {
                let dsl = match entity_id {
                    Some(id) => format!("(view.who-controls :entity-id \"{}\")", id),
                    None => "(view.who-controls)".to_string(),
                };
                self.execute_server_dsl(&dsl);
            }
            NavigationVerb::FollowMoney { from_entity } => {
                let dsl = match from_entity {
                    Some(id) => format!("(view.follow-money :from-entity \"{}\")", id),
                    None => "(view.follow-money)".to_string(),
                };
                self.execute_server_dsl(&dsl);
            }
            NavigationVerb::TimeRewind { target_date } => {
                let dsl = match target_date {
                    Some(d) => format!("(view.time-rewind :target-date \"{}\")", d),
                    None => "(view.time-rewind)".to_string(),
                };
                self.execute_server_dsl(&dsl);
            }
            
            // === LOCAL RENDER TOGGLES (no server round-trip) ===
            NavigationVerb::Xray => {
                self.render_state.toggle_xray();
                self.request_repaint();
            }
            NavigationVerb::Peel => {
                self.render_state.peel_layer();
                self.request_repaint();
            }
            NavigationVerb::Shadow => {
                self.render_state.toggle_shadow(None);
                self.request_repaint();
            }
            NavigationVerb::RedFlagScan => {
                self.render_state.toggle_red_flags(None);
                self.request_repaint();
            }
            NavigationVerb::BlackHole => {
                self.render_state.toggle_gaps(None);
                self.request_repaint();
            }
            NavigationVerb::Illuminate { aspect } => {
                let asp = parse_illuminate_aspect(&aspect);
                self.render_state.set_illuminate(asp);
                self.request_repaint();
            }
            NavigationVerb::DepthIndicator => {
                self.render_state.toggle_depth_indicator();
                self.request_repaint();
            }
            NavigationVerb::CrossSection => {
                // Toggle cross-section mode
                if self.render_state.cross_section.is_some() {
                    self.render_state.cross_section = None;
                } else {
                    self.render_state.cross_section = Some(CrossSection {
                        axis: CrossSectionAxis::Ownership,
                        position: 0.5,
                    });
                }
                self.request_repaint();
            }
            NavigationVerb::Orbit { entity_id } => {
                self.render_state.orbit_mode = !self.render_state.orbit_mode;
                // entity_id could be used to set orbit center
                self.request_repaint();
            }
            NavigationVerb::Tilt { dimension } => {
                if self.render_state.tilt.is_some() {
                    self.render_state.tilt = None;
                } else {
                    self.render_state.tilt = Some((dimension, 0.5));
                }
                self.request_repaint();
            }
            NavigationVerb::Flip => {
                self.render_state.flipped = !self.render_state.flipped;
                self.request_repaint();
            }
            
            // === CONTEXT MODE (could be either, let's make it local) ===
            NavigationVerb::SetContext { mode } => {
                self.context_mode = parse_context_mode(&mode);
                self.request_repaint();
            }
            
            _ => {
                tracing::warn!("Unhandled NavigationVerb: {:?}", verb);
            }
        }
    }
    
    /// Execute DSL on server and update view state from response
    fn execute_server_dsl(&mut self, dsl: &str) {
        // Spawn async HTTP call to server
        let dsl_owned = dsl.to_string();
        let ctx = self.ctx.clone();
        
        wasm_bindgen_futures::spawn_local(async move {
            match api::execute_dsl(&dsl_owned).await {
                Ok(response) => {
                    // Update view state from server response
                    // This will trigger re-render with new data
                }
                Err(e) => {
                    tracing::error!("DSL execution failed: {}", e);
                }
            }
        });
    }
}
```

---

## Graph Renderer Uses RenderState

**File:** `rust/crates/ob-poc-ui/src/graph/renderer.rs`

```rust
impl GraphRenderer {
    pub fn render(&self, painter: &egui::Painter, view_state: &ViewState, render_state: &RenderState) {
        for node in &self.nodes {
            let mut alpha = 1.0;
            let mut scale = 1.0;
            let mut highlight_color = None;
            
            // === Apply render state effects ===
            
            // X-ray mode: make outer shells transparent
            if render_state.xray_mode {
                if node.is_outer_shell() {
                    alpha = 0.3;
                }
            }
            
            // Peel: hide layers beyond peel depth
            if node.layer_depth() < render_state.peel_depth {
                continue; // Don't render peeled layers
            }
            
            // Shadow mode: dim non-risk entities
            if render_state.shadow_mode {
                if !node.has_risk_flags() {
                    alpha = 0.2;
                }
            }
            
            // Red flag highlight
            if render_state.red_flag_highlight {
                if node.has_risk_flags() {
                    highlight_color = Some(Color32::RED);
                    scale = 1.1;
                }
            }
            
            // Black hole: show gaps as voids
            if render_state.show_gaps {
                if node.has_data_gaps() {
                    self.render_gap_indicator(painter, node);
                }
            }
            
            // Illuminate aspect
            if let Some(aspect) = &render_state.illuminate_aspect {
                if node.has_aspect(aspect) {
                    highlight_color = Some(Color32::GOLD);
                    alpha = 1.0;  // Full brightness
                } else {
                    alpha = 0.4;  // Dim others
                }
            }
            
            // Depth indicator overlay
            if render_state.show_depth_indicator {
                self.render_depth_ring(painter, node);
            }
            
            // Cross section: hide entities on wrong side of cut
            if let Some(cs) = &render_state.cross_section {
                if !node.is_on_visible_side(cs) {
                    continue;
                }
            }
            
            // Flip perspective
            let pos = if render_state.flipped {
                node.flipped_position()
            } else {
                node.position()
            };
            
            // Finally render the node
            self.render_node(painter, node, pos, alpha, scale, highlight_color);
        }
    }
}
```

---

## Summary: What Goes Where

| Verb | Layer | Needs Server? | Implementation Location |
|------|-------|---------------|------------------------|
| `view.universe` | Server DSL | YES | `view_ops.rs` |
| `view.cbu` | Server DSL | YES | `view_ops.rs` |
| `view.entity` | Server DSL | YES | `view_ops.rs` |
| `view.drill` | Server DSL | YES | `view_ops.rs` |
| `view.surface` | Server DSL | YES | `view_ops.rs` |
| `view.who-controls` | Server DSL | YES | `view_ops.rs` |
| `view.follow-money` | Server DSL | YES | `view_ops.rs` |
| `view.time-rewind` | Server DSL | YES | `view_ops.rs` |
| `Xray` | Render toggle | NO | `app.rs` → `RenderState` |
| `Peel` | Render toggle | NO | `app.rs` → `RenderState` |
| `Shadow` | Render toggle | NO | `app.rs` → `RenderState` |
| `RedFlagScan` | Render toggle | NO | `app.rs` → `RenderState` |
| `BlackHole` | Render toggle | NO | `app.rs` → `RenderState` |
| `Illuminate` | Render toggle | NO | `app.rs` → `RenderState` |
| `DepthIndicator` | Render toggle | NO | `app.rs` → `RenderState` |
| `CrossSection` | Render toggle | NO | `app.rs` → `RenderState` |
| `Orbit` | Render toggle | NO | `app.rs` → `RenderState` |
| `Tilt` | Render toggle | NO | `app.rs` → `RenderState` |
| `Flip` | Render toggle | NO | `app.rs` → `RenderState` |

---

## Files to Create/Modify

| File | Action | Purpose |
|------|--------|---------|
| `rust/crates/ob-poc-ui/src/render_state.rs` | CREATE | RenderState struct |
| `rust/crates/ob-poc-ui/src/app.rs` | MODIFY | Handle NavigationVerb correctly |
| `rust/crates/ob-poc-ui/src/graph/renderer.rs` | MODIFY | Apply RenderState in render loop |
| `rust/src/dsl_v2/custom_ops/view_ops.rs` | CREATE | Server-side DSL handlers |
| `rust/config/verbs/view.yaml` | EXTEND | Server DSL verb definitions |

---

## Acceptance Criteria

### Local Render Toggles
- [ ] `Xray` toggles transparency locally, no server call
- [ ] `Shadow` dims non-risk entities locally
- [ ] `RedFlagScan` highlights risks locally
- [ ] `BlackHole` shows gaps locally
- [ ] All render toggles take effect immediately (no loading)

### Server DSL Verbs
- [ ] `view.drill` fetches children from server
- [ ] `view.entity` fetches entity detail from server
- [ ] `view.who-controls` fetches control chain from server
- [ ] Server responses update ViewState
- [ ] UI re-renders with new data

### Integration
- [ ] Agent can say "show x-ray" → local toggle
- [ ] Agent can say "drill into Allianz" → server call
- [ ] Both work seamlessly in same conversation
