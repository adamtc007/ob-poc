# TODO: ESPER Visual Effects Wiring

## Overview

ESPER navigation verbs exist but produce **no visual effect** because:
1. `EsperRenderState` exists in `ob-poc-graph` but isn't used by the UI
2. DSL verb handlers don't update any render state
3. Graph renderer doesn't apply alpha/highlight/peel effects

```
Current Flow (broken):
  view.xray → DSL executes → returns success → NOTHING CHANGES VISUALLY

Required Flow:
  view.xray → DSL executes → updates EsperRenderState → graph re-renders with effects
```

---

## Gap Analysis

| Component | Status | Location |
|-----------|--------|----------|
| `EsperRenderState` struct | ✅ Complete | `ob-poc-graph/src/graph/viewport.rs` |
| Toggle methods (`toggle_xray`, etc.) | ✅ Complete | Same file |
| Alpha calculation (`get_node_alpha`) | ✅ Complete | Same file |
| `EsperRenderState` in `AppState` | ❌ **MISSING** | `ob-poc-ui/src/state.rs` |
| DSL handlers update render state | ❌ **MISSING** | `dsl_v2/custom_ops/view_ops.rs` |
| Graph renderer uses `EsperRenderState` | ❌ **MISSING** | `ob-poc-graph/src/graph/render.rs` |
| Session sync for ESPER state | ❌ **MISSING** | Server-side persistence |

---

## Phase 1: Add EsperRenderState to AppState (30min)

### 1.1 Import and Add Field

**File:** `rust/crates/ob-poc-ui/src/state.rs`

```rust
use ob_poc_graph::EsperRenderState;

pub struct AppState {
    // ... existing fields ...
    
    // =========================================================================
    // ESPER RENDER STATE (local visual modes, no server round-trip)
    // =========================================================================
    /// ESPER visual effect state (xray, peel, shadow, illuminate, etc.)
    /// Updated by DSL verb handlers, consumed by graph renderer
    pub esper_state: EsperRenderState,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            // ... existing defaults ...
            esper_state: EsperRenderState::new(),
        }
    }
}
```

### 1.2 Add Reset on CBU Change

When the user switches CBUs, reset ESPER state:

```rust
// In CBU selection handler:
pub fn on_cbu_changed(&mut self, new_cbu_id: Uuid) {
    // ... existing logic ...
    
    // Reset ESPER visual modes
    self.esper_state.reset();
}
```

---

## Phase 2: Wire DSL Verb Handlers (1.5h)

The ESPER verbs need to update `EsperRenderState` via the session or return actions that the UI processes.

### 2.1 Option A: Session-Scoped State (Recommended)

Store `EsperRenderState` in the session context so DSL verbs can update it:

**File:** `rust/src/api/session.rs` (or session state module)

```rust
use ob_poc_graph::EsperRenderState;

pub struct SessionState {
    // ... existing fields ...
    
    /// ESPER visual render state (persisted per-session)
    pub esper_state: EsperRenderState,
}
```

**File:** `rust/src/dsl_v2/custom_ops/view_ops.rs`

Update each ESPER verb to modify session state:

```rust
/// Handler for view.xray
pub struct ViewXrayOp;

#[async_trait]
impl CustomOperation for ViewXrayOp {
    fn domain(&self) -> &'static str { "view" }
    fn verb(&self) -> &'static str { "xray" }
    fn rationale(&self) -> &'static str { 
        "Toggles X-ray mode - dims non-focused elements to see through structure" 
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Get alpha argument if provided
        let alpha: Option<f32> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "alpha")
            .and_then(|a| a.value.as_f64().map(|v| v as f32));

        // Update session's ESPER state
        if let Some(session) = ctx.session_mut() {
            if let Some(alpha) = alpha {
                session.esper_state.enable_xray(alpha);
            } else {
                session.esper_state.toggle_xray();
            }
        }

        let enabled = ctx.session()
            .map(|s| s.esper_state.xray_enabled)
            .unwrap_or(false);

        Ok(ExecutionResult::success(json!({
            "mode": "xray",
            "enabled": enabled,
            "alpha": ctx.session().map(|s| s.esper_state.xray_alpha).unwrap_or(0.3)
        })))
    }
}

/// Handler for view.peel
pub struct ViewPeelOp;

#[async_trait]
impl CustomOperation for ViewPeelOp {
    fn domain(&self) -> &'static str { "view" }
    fn verb(&self) -> &'static str { "peel" }
    fn rationale(&self) -> &'static str { 
        "Peels outer layers to reveal inner structure" 
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let depth: Option<u8> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "depth")
            .and_then(|a| a.value.as_u64().map(|v| v as u8));

        if let Some(session) = ctx.session_mut() {
            if let Some(depth) = depth {
                session.esper_state.set_peel_depth(depth);
            } else {
                session.esper_state.toggle_peel();
            }
        }

        let (enabled, current_depth) = ctx.session()
            .map(|s| (s.esper_state.peel_enabled, s.esper_state.peel_depth))
            .unwrap_or((false, 0));

        Ok(ExecutionResult::success(json!({
            "mode": "peel",
            "enabled": enabled,
            "depth": current_depth
        })))
    }
}

/// Handler for view.shadow
pub struct ViewShadowOp;

#[async_trait]
impl CustomOperation for ViewShadowOp {
    fn domain(&self) -> &'static str { "view" }
    fn verb(&self) -> &'static str { "shadow" }
    fn rationale(&self) -> &'static str { 
        "Dims non-relevant entities to focus on target" 
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let alpha: Option<f32> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "alpha")
            .and_then(|a| a.value.as_f64().map(|v| v as f32));

        if let Some(session) = ctx.session_mut() {
            if let Some(alpha) = alpha {
                session.esper_state.enable_shadow(alpha);
            } else {
                session.esper_state.toggle_shadow();
            }
        }

        let enabled = ctx.session()
            .map(|s| s.esper_state.shadow_enabled)
            .unwrap_or(false);

        Ok(ExecutionResult::success(json!({
            "mode": "shadow",
            "enabled": enabled
        })))
    }
}

/// Handler for view.illuminate
pub struct ViewIlluminateOp;

#[async_trait]
impl CustomOperation for ViewIlluminateOp {
    fn domain(&self) -> &'static str { "view" }
    fn verb(&self) -> &'static str { "illuminate" }
    fn rationale(&self) -> &'static str { 
        "Highlights/glows a specific aspect (ownership, control, risk)" 
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use ob_poc_graph::IlluminateAspect;
        
        let aspect_str: Option<&str> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "aspect")
            .and_then(|a| a.value.as_str());

        let aspect = match aspect_str {
            Some("ownership") => IlluminateAspect::Ownership,
            Some("control") => IlluminateAspect::Control,
            Some("risk") => IlluminateAspect::Risk,
            Some("documents") => IlluminateAspect::Documents,
            Some("kyc") => IlluminateAspect::KycStatus,
            _ => IlluminateAspect::Ownership,
        };

        if let Some(session) = ctx.session_mut() {
            session.esper_state.toggle_illuminate(aspect);
        }

        let enabled = ctx.session()
            .map(|s| s.esper_state.illuminate_enabled)
            .unwrap_or(false);

        Ok(ExecutionResult::success(json!({
            "mode": "illuminate",
            "enabled": enabled,
            "aspect": format!("{:?}", aspect)
        })))
    }
}

/// Handler for view.red-flag
pub struct ViewRedFlagOp;

#[async_trait]
impl CustomOperation for ViewRedFlagOp {
    fn domain(&self) -> &'static str { "view" }
    fn verb(&self) -> &'static str { "red-flag" }
    fn rationale(&self) -> &'static str { 
        "Highlights entities with risk indicators or anomalies" 
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use ob_poc_graph::RedFlagCategory;
        
        let category_str: Option<&str> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "category")
            .and_then(|a| a.value.as_str());

        let category = match category_str {
            Some("high-risk") => Some(RedFlagCategory::HighRisk),
            Some("pending-kyc") => Some(RedFlagCategory::PendingKyc),
            Some("sanctions") => Some(RedFlagCategory::Sanctions),
            Some("pep") => Some(RedFlagCategory::Pep),
            Some("adverse-media") => Some(RedFlagCategory::AdverseMedia),
            Some("ownership-gaps") => Some(RedFlagCategory::OwnershipGaps),
            Some("all") => Some(RedFlagCategory::All),
            None => None, // Toggle all
            _ => None,
        };

        if let Some(session) = ctx.session_mut() {
            session.esper_state.toggle_red_flag_scan(category);
        }

        let enabled = ctx.session()
            .map(|s| s.esper_state.red_flag_scan_enabled)
            .unwrap_or(false);

        Ok(ExecutionResult::success(json!({
            "mode": "red-flag",
            "enabled": enabled,
            "category": category_str
        })))
    }
}

/// Handler for view.black-holes
pub struct ViewBlackHolesOp;

#[async_trait]
impl CustomOperation for ViewBlackHolesOp {
    fn domain(&self) -> &'static str { "view" }
    fn verb(&self) -> &'static str { "black-holes" }
    fn rationale(&self) -> &'static str { 
        "Highlights entities with missing or incomplete data" 
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use ob_poc_graph::GapType;
        
        let gap_type_str: Option<&str> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "type")
            .and_then(|a| a.value.as_str());

        let gap_type = match gap_type_str {
            Some("documents") => Some(GapType::MissingDocuments),
            Some("ownership") => Some(GapType::IncompleteOwnership),
            Some("screening") => Some(GapType::MissingScreening),
            Some("expired") => Some(GapType::ExpiredData),
            Some("all") => Some(GapType::All),
            None => None,
            _ => None,
        };

        if let Some(session) = ctx.session_mut() {
            session.esper_state.toggle_black_hole(gap_type);
        }

        let enabled = ctx.session()
            .map(|s| s.esper_state.black_hole_enabled)
            .unwrap_or(false);

        Ok(ExecutionResult::success(json!({
            "mode": "black-holes",
            "enabled": enabled,
            "gap_type": gap_type_str
        })))
    }
}
```

### 2.2 Add ESPER State to Session Watch Response

**File:** `rust/src/api/session.rs`

The session watch endpoint should include ESPER state so the UI can sync:

```rust
#[derive(Serialize)]
pub struct WatchSessionResponse {
    // ... existing fields ...
    
    /// ESPER visual mode state
    pub esper_state: EsperRenderStateDto,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct EsperRenderStateDto {
    pub xray_enabled: bool,
    pub xray_alpha: f32,
    pub peel_enabled: bool,
    pub peel_depth: u8,
    pub shadow_enabled: bool,
    pub shadow_alpha: f32,
    pub illuminate_enabled: bool,
    pub illuminate_aspect: String,
    pub red_flag_scan_enabled: bool,
    pub red_flag_category: Option<String>,
    pub black_hole_enabled: bool,
    pub black_hole_gap_type: Option<String>,
}

impl From<&EsperRenderState> for EsperRenderStateDto {
    fn from(state: &EsperRenderState) -> Self {
        Self {
            xray_enabled: state.xray_enabled,
            xray_alpha: state.xray_alpha,
            peel_enabled: state.peel_enabled,
            peel_depth: state.peel_depth,
            shadow_enabled: state.shadow_enabled,
            shadow_alpha: state.shadow_alpha,
            illuminate_enabled: state.illuminate_enabled,
            illuminate_aspect: format!("{:?}", state.illuminate_aspect),
            red_flag_scan_enabled: state.red_flag_scan_enabled,
            red_flag_category: state.red_flag_category.map(|c| format!("{:?}", c)),
            black_hole_enabled: state.black_hole_enabled,
            black_hole_gap_type: state.black_hole_gap_type.map(|g| format!("{:?}", g)),
        }
    }
}
```

### 2.3 UI Sync from Session Watch

**File:** `rust/crates/ob-poc-ui/src/app.rs`

When session watch returns, update local ESPER state:

```rust
// In session watch handler:
fn handle_session_watch_response(&mut self, response: WatchSessionResponse) {
    // ... existing logic ...
    
    // Sync ESPER state from server
    self.state.esper_state.xray_enabled = response.esper_state.xray_enabled;
    self.state.esper_state.xray_alpha = response.esper_state.xray_alpha;
    self.state.esper_state.peel_enabled = response.esper_state.peel_enabled;
    self.state.esper_state.peel_depth = response.esper_state.peel_depth;
    self.state.esper_state.shadow_enabled = response.esper_state.shadow_enabled;
    self.state.esper_state.shadow_alpha = response.esper_state.shadow_alpha;
    self.state.esper_state.illuminate_enabled = response.esper_state.illuminate_enabled;
    self.state.esper_state.red_flag_scan_enabled = response.esper_state.red_flag_scan_enabled;
    self.state.esper_state.black_hole_enabled = response.esper_state.black_hole_enabled;
    // ... etc
}
```

---

## Phase 3: Graph Renderer Uses EsperRenderState (1.5h)

### 3.1 Pass EsperRenderState to Renderer

**File:** `rust/crates/ob-poc-graph/src/graph/render.rs`

Modify the render function signature to accept ESPER state:

```rust
/// Render the CBU graph with ESPER visual effects applied
pub fn render_cbu_graph(
    ui: &mut Ui,
    graph_data: &CbuGraphData,
    widget_state: &mut CbuGraphWidget,
    esper_state: &EsperRenderState,  // NEW PARAMETER
    screen_rect: Rect,
) -> GraphWidgetAction {
    // ... existing setup ...
    
    // Render nodes with ESPER effects
    for node in &graph_data.nodes {
        render_node_with_esper(
            painter,
            node,
            widget_state,
            esper_state,
            camera,
        );
    }
    
    // ... rest of rendering ...
}
```

### 3.2 Apply Visual Effects to Node Rendering

**File:** `rust/crates/ob-poc-graph/src/graph/render.rs`

```rust
/// Render a single node with ESPER visual effects applied
fn render_node_with_esper(
    painter: &Painter,
    node: &GraphNodeData,
    widget_state: &CbuGraphWidget,
    esper_state: &EsperRenderState,
    camera: &Camera,
) {
    // Determine if this node is focused
    let is_focused = widget_state.selected_node.as_ref() == Some(&node.id)
        || widget_state.hovered_node.as_ref() == Some(&node.id);
    
    // Get node depth (for peel mode) - calculate from ownership chain depth
    let depth = node.metadata.get("depth")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u8;
    
    // Check for risk flags (for red-flag mode)
    let has_red_flag = node.risk_level.as_ref()
        .map(|r| r == "high" || r == "medium")
        .unwrap_or(false)
        || node.metadata.get("has_screening_hit").and_then(|v| v.as_bool()).unwrap_or(false);
    
    // Check for data gaps (for black-hole mode)
    let has_gap = node.metadata.get("missing_documents").and_then(|v| v.as_bool()).unwrap_or(false)
        || node.metadata.get("incomplete_ownership").and_then(|v| v.as_bool()).unwrap_or(false)
        || node.metadata.get("missing_screening").and_then(|v| v.as_bool()).unwrap_or(false);
    
    // Calculate alpha and highlight from ESPER state
    let (alpha, should_highlight) = esper_state.get_node_alpha(
        is_focused,
        depth,
        has_red_flag,
        has_gap,
    );
    
    // Skip rendering if fully transparent (peel mode hiding)
    if alpha < 0.01 {
        return;
    }
    
    // Get base colors
    let base_fill = get_node_fill_color(node);
    let base_stroke = get_node_stroke_color(node);
    
    // Apply alpha
    let fill = apply_alpha(base_fill, alpha);
    let stroke_color = apply_alpha(base_stroke, alpha);
    
    // Calculate screen position
    let screen_pos = camera.world_to_screen(node.position);
    let screen_radius = node.radius * camera.zoom;
    
    // Render highlight ring if flagged
    if should_highlight {
        let highlight_color = if esper_state.red_flag_scan_enabled && has_red_flag {
            Color32::from_rgb(239, 68, 68) // red-500
        } else if esper_state.black_hole_enabled && has_gap {
            Color32::from_rgb(139, 92, 246) // violet-500
        } else {
            Color32::from_rgb(250, 204, 21) // yellow-400
        };
        
        // Pulsing glow effect
        let time = ui.input(|i| i.time) as f32;
        let pulse = (time * 3.0).sin() * 0.5 + 0.5;
        let glow_alpha = (pulse * 100.0) as u8;
        let glow = Color32::from_rgba_unmultiplied(
            highlight_color.r(), highlight_color.g(), highlight_color.b(), glow_alpha
        );
        
        painter.circle_filled(screen_pos, screen_radius + 8.0, glow);
        painter.circle_stroke(
            screen_pos,
            screen_radius + 4.0,
            Stroke::new(2.0, highlight_color),
        );
    }
    
    // Render illuminate glow if enabled
    if esper_state.illuminate_enabled {
        let should_illuminate = match esper_state.illuminate_aspect {
            IlluminateAspect::Ownership => node.metadata.get("has_ownership_edge")
                .and_then(|v| v.as_bool()).unwrap_or(false),
            IlluminateAspect::Control => node.metadata.get("is_controller")
                .and_then(|v| v.as_bool()).unwrap_or(false),
            IlluminateAspect::Risk => has_red_flag,
            IlluminateAspect::Documents => node.metadata.get("has_documents")
                .and_then(|v| v.as_bool()).unwrap_or(false),
            IlluminateAspect::KycStatus => node.metadata.get("kyc_complete")
                .and_then(|v| v.as_bool()).unwrap_or(false),
            IlluminateAspect::Custom => false,
        };
        
        if should_illuminate {
            let glow_color = Color32::from_rgba_unmultiplied(147, 197, 253, 80); // blue glow
            painter.circle_filled(screen_pos, screen_radius + 6.0, glow_color);
        }
    }
    
    // Render node body
    painter.circle_filled(screen_pos, screen_radius, fill);
    painter.circle_stroke(screen_pos, screen_radius, Stroke::new(2.0, stroke_color));
    
    // Render label (also with alpha)
    if alpha > 0.3 {
        let label_color = apply_alpha(Color32::WHITE, alpha);
        painter.text(
            Pos2::new(screen_pos.x, screen_pos.y + screen_radius + 12.0),
            egui::Align2::CENTER_TOP,
            &node.label,
            egui::FontId::proportional(11.0),
            label_color,
        );
    }
}

/// Apply alpha to a color
fn apply_alpha(color: Color32, alpha: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(
        color.r(),
        color.g(),
        color.b(),
        (color.a() as f32 * alpha) as u8,
    )
}
```

### 3.3 Apply Effects to Edge Rendering

```rust
/// Render edge with ESPER effects
fn render_edge_with_esper(
    painter: &Painter,
    edge: &GraphEdgeData,
    nodes: &HashMap<String, &GraphNodeData>,
    esper_state: &EsperRenderState,
    camera: &Camera,
) {
    let source = nodes.get(&edge.source);
    let target = nodes.get(&edge.target);
    
    if source.is_none() || target.is_none() {
        return;
    }
    
    let source = source.unwrap();
    let target = target.unwrap();
    
    // Get alpha for both endpoints
    let source_alpha = esper_state.get_node_alpha(false, 0, false, false).0;
    let target_alpha = esper_state.get_node_alpha(false, 0, false, false).0;
    let edge_alpha = source_alpha.min(target_alpha);
    
    if edge_alpha < 0.01 {
        return;
    }
    
    // Get edge color
    let base_color = get_edge_color(edge);
    let color = apply_alpha(base_color, edge_alpha);
    
    // Illuminate ownership edges if in ownership mode
    let stroke_width = if esper_state.illuminate_enabled 
        && esper_state.illuminate_aspect == IlluminateAspect::Ownership
        && edge.edge_type == "ownership" {
        3.0
    } else {
        1.5
    };
    
    let source_pos = camera.world_to_screen(source.position);
    let target_pos = camera.world_to_screen(target.position);
    
    painter.line_segment(
        [source_pos, target_pos],
        Stroke::new(stroke_width, color),
    );
}
```

### 3.4 Update Caller to Pass EsperRenderState

**File:** `rust/crates/ob-poc-ui/src/app.rs` (or wherever graph is rendered)

```rust
// In render function:
if let Some(ref graph_data) = state.graph_data {
    let action = render_cbu_graph(
        ui,
        graph_data,
        &mut state.graph_widget,
        &state.esper_state,  // Pass ESPER state
        graph_rect,
    );
    
    // Handle action...
}
```

---

## Phase 4: ESPER Mode Indicator HUD (30min)

### 4.1 Show Active Modes

Display which ESPER modes are active in the viewport HUD:

**File:** `rust/crates/ob-poc-graph/src/graph/viewport.rs`

```rust
/// Render ESPER mode indicator badges
pub fn render_esper_mode_badges(
    ui: &mut Ui,
    esper_state: &EsperRenderState,
    rect: Rect,
) {
    if !esper_state.any_mode_active() {
        return;
    }
    
    let painter = ui.painter();
    let mut x = rect.min.x + 8.0;
    let y = rect.center().y;
    let badge_height = 20.0;
    
    let active_modes: Vec<(&str, &str, Color32)> = vec![
        ("XRAY", "👁", Color32::from_rgb(147, 197, 253)),
        ("PEEL", "🧅", Color32::from_rgb(253, 186, 116)),
        ("SHADOW", "🌑", Color32::from_rgb(107, 114, 128)),
        ("ILLUM", "💡", Color32::from_rgb(250, 204, 21)),
        ("FLAGS", "🚩", Color32::from_rgb(239, 68, 68)),
        ("GAPS", "🕳", Color32::from_rgb(139, 92, 246)),
    ];
    
    let enabled = [
        esper_state.xray_enabled,
        esper_state.peel_enabled,
        esper_state.shadow_enabled,
        esper_state.illuminate_enabled,
        esper_state.red_flag_scan_enabled,
        esper_state.black_hole_enabled,
    ];
    
    for (i, ((label, icon, color), is_enabled)) in 
        active_modes.iter().zip(enabled.iter()).enumerate() 
    {
        if !is_enabled {
            continue;
        }
        
        // Badge background
        let badge_width = 50.0;
        let badge_rect = Rect::from_min_size(
            Pos2::new(x, y - badge_height / 2.0),
            Vec2::new(badge_width, badge_height),
        );
        
        painter.rect_filled(badge_rect, 4.0, color.linear_multiply(0.3));
        painter.rect_stroke(badge_rect, 4.0, Stroke::new(1.0, *color));
        
        // Icon + label
        painter.text(
            badge_rect.center(),
            egui::Align2::CENTER_CENTER,
            format!("{} {}", icon, label),
            egui::FontId::proportional(9.0),
            *color,
        );
        
        x += badge_width + 4.0;
    }
}
```

### 4.2 Add to Viewport HUD

```rust
// In render_viewport_hud():
// After breadcrumbs, before enhance indicator:
let esper_rect = Rect::from_min_size(
    Pos2::new(screen_rect.min.x + 200.0, screen_rect.min.y + 8.0),
    Vec2::new(320.0, 28.0),
);
render_esper_mode_badges(ui, esper_state, esper_rect);
```

---

## Testing Checklist

### Phase 1: State
- [ ] `AppState` has `esper_state: EsperRenderState` field
- [ ] State resets on CBU change

### Phase 2: DSL Handlers
- [ ] `view.xray` toggles `xray_enabled`
- [ ] `view.peel` increments `peel_depth`
- [ ] `view.shadow` toggles `shadow_enabled`
- [ ] `view.illuminate ownership` sets `illuminate_aspect`
- [ ] `view.red-flag` enables `red_flag_scan_enabled`
- [ ] `view.black-holes` enables `black_hole_enabled`
- [ ] Session watch returns ESPER state
- [ ] UI syncs ESPER state from session watch

### Phase 3: Rendering
- [ ] X-ray mode: non-focused nodes are semi-transparent
- [ ] Peel mode: depth > peel_depth nodes are hidden
- [ ] Shadow mode: non-focused nodes are dimmed
- [ ] Illuminate mode: relevant nodes have glow
- [ ] Red-flag mode: flagged nodes have pulsing red ring
- [ ] Black-holes mode: nodes with gaps have purple ring
- [ ] Edges respect alpha from connected nodes

### Phase 4: HUD
- [ ] Active modes show as badges in viewport
- [ ] Badges disappear when modes disabled

---

## Files Summary

### Modified Files
```
rust/crates/ob-poc-ui/src/state.rs              - Add esper_state field
rust/crates/ob-poc-ui/src/app.rs                - Pass esper_state to renderer, sync from watch
rust/crates/ob-poc-graph/src/graph/render.rs    - Apply ESPER effects to nodes/edges
rust/crates/ob-poc-graph/src/graph/viewport.rs  - Add ESPER mode badges
rust/src/api/session.rs                          - Add esper_state to session + watch response
rust/src/dsl_v2/custom_ops/view_ops.rs          - Update handlers to modify session state
```

---

## Total Effort

| Phase | Time | Priority |
|-------|------|----------|
| Phase 1: State | 30min | P0 |
| Phase 2: DSL Handlers | 1.5h | P0 |
| Phase 3: Rendering | 1.5h | P0 |
| Phase 4: HUD Badges | 30min | P1 |
| **Total** | **~4h** | |

---

## DSL Usage Examples

After implementation:

```lisp
;; Toggle X-ray mode (dim non-focused)
(view.xray)

;; X-ray with custom alpha
(view.xray :alpha 0.2)

;; Peel one layer (hide outermost entities)
(view.peel)

;; Peel to specific depth
(view.peel :depth 2)

;; Shadow mode (dim everything except focus)
(view.shadow)

;; Illuminate ownership relationships
(view.illuminate :aspect ownership)

;; Illuminate control chains
(view.illuminate :aspect control)

;; Highlight high-risk entities
(view.red-flag :category high-risk)

;; Highlight all risk indicators
(view.red-flag :category all)

;; Show entities with missing documents
(view.black-holes :type documents)

;; Show all data gaps
(view.black-holes :type all)

;; Reset all visual modes
(view.clear)
```
