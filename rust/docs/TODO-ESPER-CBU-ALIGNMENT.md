# TODO: ESPER Navigation + CBU Struct Alignment

> **Status:** READY FOR EXECUTION  
> **Priority:** HIGH (ESPER is 95% done, just needs UI wiring)  
> **Related:** TODO-ISDA-MATRIX-CONSOLIDATION.md

---

## Issue 1: ESPER UI "Last Mile" Problem

### Current State
```
DSL Command → Rust Handler → ViewState → Session → API → ✅ Works
                                                    ↓
                                              egui Panel → ❌ Not reading ViewState
```

### Gap Analysis

| ViewState Field | Set By | Read By egui? | Visual Effect Missing |
|-----------------|--------|---------------|----------------------|
| `trace_mode` | `view.trace` | ❌ No | Path highlighting in graph |
| `xray_layers` | `view.xray` | ❌ No | Layer transparency toggles |
| `drill_depth` | `view.drill` | ❌ No | Depth-limited node expansion |
| `surface_level` | `view.surface` | ❌ No | Aggregation level display |
| `focus_entity_id` | `view.focus` | ❌ No | Centered/highlighted node |
| `taxonomy_stack` | `zoom-in/out` | ⚠️ Partial | Breadcrumb navigation |

### Fix: Wire ViewState to egui Panels

**File:** `crates/ob-poc-ui/src/panels/graph_panel.rs`

#### Step 1.1: Add ViewState to panel render context

```rust
pub fn render_graph_panel(ui: &mut egui::Ui, state: &AppState) {
    // Get current ViewState from session
    let view_state = state.session_context
        .as_ref()
        .and_then(|ctx| ctx.view_state.as_ref());
    
    // Apply view state to rendering
    if let Some(vs) = view_state {
        apply_view_state_effects(ui, vs, &mut state.graph_data);
    }
    
    // ... existing graph rendering ...
}

fn apply_view_state_effects(
    ui: &mut egui::Ui, 
    vs: &ViewState, 
    graph: &mut GraphData
) {
    // 1. Trace mode - highlight path
    if let Some(ref trace) = vs.trace_mode {
        for node_id in &trace.path_node_ids {
            graph.set_node_highlight(*node_id, TraceHighlight {
                color: egui::Color32::from_rgb(255, 200, 0), // Blade Runner amber
                pulse: true,
            });
        }
    }
    
    // 2. X-ray layers - set transparency
    for (layer_name, opacity) in &vs.xray_layers {
        graph.set_layer_opacity(layer_name, *opacity);
    }
    
    // 3. Drill depth - collapse nodes beyond depth
    if let Some(depth) = vs.drill_depth {
        graph.set_max_visible_depth(depth);
    }
    
    // 4. Focus entity - center and highlight
    if let Some(focus_id) = vs.focus_entity_id {
        graph.center_on_node(focus_id);
        graph.set_node_highlight(focus_id, FocusHighlight::Primary);
    }
}
```

#### Step 1.2: Add breadcrumb UI for drill navigation

**File:** `crates/ob-poc-ui/src/panels/breadcrumb_bar.rs` (NEW)

```rust
pub fn render_breadcrumb_bar(ui: &mut egui::Ui, state: &mut AppState) -> Vec<ViewAction> {
    let mut actions = Vec::new();
    
    let taxonomy_stack = state.session_context
        .as_ref()
        .map(|ctx| &ctx.taxonomy_stack)
        .unwrap_or(&TaxonomyStack::default());
    
    ui.horizontal(|ui| {
        // Root
        if ui.selectable_label(taxonomy_stack.is_empty(), "🏠 Root").clicked() {
            actions.push(ViewAction::ZoomBackTo { level: 0 });
        }
        
        // Breadcrumb segments
        for (idx, segment) in taxonomy_stack.segments().iter().enumerate() {
            ui.label("›");
            let is_current = idx == taxonomy_stack.len() - 1;
            if ui.selectable_label(is_current, &segment.display_name).clicked() {
                actions.push(ViewAction::ZoomBackTo { level: idx + 1 });
            }
        }
    });
    
    actions
}
```

#### Step 1.3: Add trace path overlay to graph

**File:** `crates/ob-poc-ui/src/panels/graph_panel.rs`

```rust
fn render_trace_overlay(painter: &egui::Painter, vs: &ViewState, layout: &GraphLayout) {
    if let Some(ref trace) = vs.trace_mode {
        // Draw connecting lines between trace path nodes
        let points: Vec<egui::Pos2> = trace.path_node_ids
            .iter()
            .filter_map(|id| layout.node_positions.get(id))
            .cloned()
            .collect();
        
        if points.len() >= 2 {
            // Blade Runner style: amber glow with pulse
            let stroke = egui::Stroke::new(3.0, egui::Color32::from_rgb(255, 176, 0));
            painter.add(egui::Shape::line(points, stroke));
            
            // Add direction arrows
            for window in points.windows(2) {
                let (from, to) = (window[0], window[1]);
                draw_arrow(painter, from, to, stroke.color);
            }
        }
    }
}
```

#### Step 1.4: Add xray layer controls

**File:** `crates/ob-poc-ui/src/panels/layer_control_panel.rs` (NEW)

```rust
pub fn render_layer_controls(ui: &mut egui::Ui, state: &mut AppState) -> Vec<ViewAction> {
    let mut actions = Vec::new();
    
    let layers = ["entities", "ownership", "roles", "documents", "kyc", "custody"];
    
    ui.heading("X-Ray Layers");
    
    for layer in layers {
        let current_opacity = state.view_state
            .as_ref()
            .and_then(|vs| vs.xray_layers.get(layer))
            .copied()
            .unwrap_or(1.0);
        
        ui.horizontal(|ui| {
            ui.label(layer);
            let mut opacity = current_opacity;
            if ui.add(egui::Slider::new(&mut opacity, 0.0..=1.0)).changed() {
                actions.push(ViewAction::SetLayerOpacity { 
                    layer: layer.to_string(), 
                    opacity 
                });
            }
        });
    }
    
    actions
}
```

---

## Issue 2: CBU Struct Misalignment

### Current State

```
Database (cbus table)
├── cbu_category: VARCHAR(50) ✅ EXISTS
│
├── CbuRow (Rust struct)
│   └── cbu_category: ❌ MISSING
│
├── CbuSummary (API response)  
│   └── cbu_category: ❌ MISSING
│
├── CbuGraphResponse (Graph API)
│   └── cbu_category: ✅ EXISTS
│
└── egui Context Panel
    └── cbu_category display: ❌ MISSING
```

### Fix: Propagate cbu_category Through All Layers

#### Step 2.1: Add to CbuRow

**File:** `src/services/cbu_service.rs`

```rust
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CbuRow {
    pub cbu_id: Uuid,
    pub name: String,
    pub jurisdiction: Option<String>,
    pub status: String,
    pub client_type: Option<String>,
    pub cbu_category: Option<String>,  // ADD THIS
    pub description: Option<String>,   // ADD THIS
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

Update the query:

```rust
pub async fn get_cbu_by_id(pool: &PgPool, cbu_id: Uuid) -> Result<Option<CbuRow>> {
    sqlx::query_as!(
        CbuRow,
        r#"
        SELECT 
            cbu_id, name, jurisdiction, status, client_type,
            cbu_category,  -- ADD
            description,   -- ADD
            created_at, updated_at
        FROM "ob-poc".cbus
        WHERE cbu_id = $1
        "#,
        cbu_id
    )
    .fetch_optional(pool)
    .await
    .map_err(Into::into)
}
```

#### Step 2.2: Add to CbuSummary API type

**File:** `src/api/types.rs`

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct CbuSummary {
    pub cbu_id: Uuid,
    pub name: String,
    pub jurisdiction: Option<String>,
    pub status: String,
    pub client_type: Option<String>,
    pub cbu_category: Option<String>,  // ADD THIS
    pub created_at: DateTime<Utc>,
}
```

#### Step 2.3: Update list endpoint

**File:** `src/api/cbu_routes.rs`

```rust
pub async fn list_cbus(
    State(state): State<AppState>,
) -> Result<Json<Vec<CbuSummary>>, ApiError> {
    let cbus = sqlx::query_as!(
        CbuSummary,
        r#"
        SELECT 
            cbu_id, name, jurisdiction, status, client_type,
            cbu_category,  -- ADD
            created_at
        FROM "ob-poc".cbus
        ORDER BY name
        "#
    )
    .fetch_all(&state.pool)
    .await?;
    
    Ok(Json(cbus))
}
```

#### Step 2.4: Add to egui context panel

**File:** `crates/ob-poc-ui/src/panels/context_panel.rs`

```rust
fn render_cbu_details(ui: &mut egui::Ui, cbu: &CbuSummary) {
    egui::Grid::new("cbu_details").show(ui, |ui| {
        ui.label("Name:");
        ui.label(&cbu.name);
        ui.end_row();
        
        ui.label("Status:");
        ui.label(&cbu.status);
        ui.end_row();
        
        ui.label("Jurisdiction:");
        ui.label(cbu.jurisdiction.as_deref().unwrap_or("-"));
        ui.end_row();
        
        // ADD: Category with template indicator
        ui.label("Category:");
        if let Some(ref category) = cbu.cbu_category {
            let template_icon = match category.as_str() {
                "FUND" => "📊",
                "CORPORATE" => "🏢",
                "FAMILY_OFFICE" => "👨‍👩‍👧‍👦",
                "BANK" => "🏦",
                "INSURANCE" => "🛡️",
                _ => "📁",
            };
            ui.label(format!("{} {}", template_icon, category));
        } else {
            ui.label("-");
        }
        ui.end_row();
    });
}
```

---

## Issue 3: Trading Matrix - No Changes Needed ✅

The trading matrix architecture is confirmed solid:
- Single source of truth in `ob-poc-types/src/trading_matrix.rs`
- Path-based node IDs for efficient navigation
- Clear separation between UI AST and config documents
- egui compliance verified

**Action:** Continue using existing pattern. Focus on ESPER UI wiring.

---

## Execution Order

```
1. ESPER UI (Issue 1) - High impact, 95% already done
   ├── 1.1 Wire ViewState to graph_panel
   ├── 1.2 Add breadcrumb bar
   ├── 1.3 Add trace overlay
   └── 1.4 Add layer controls

2. CBU Struct (Issue 2) - Medium impact, quick fix
   ├── 2.1 Add to CbuRow
   ├── 2.2 Add to CbuSummary
   ├── 2.3 Update list endpoint
   └── 2.4 Add to context panel

3. Trading Matrix - No action needed ✅
```

---

## Verification

```bash
# After ESPER fixes
cargo build -p ob-poc-ui
# Test view commands in DSL REPL:
# > view.trace entity: $acme
# > view.xray layers: ["ownership", "kyc"] opacity: 0.5
# > view.drill entity: $acme depth: 2

# After CBU fixes
cargo test cbu_category
# Verify API returns category:
# curl localhost:8080/api/cbu | jq '.[0].cbu_category'
```

---

## Summary

| Issue | Effort | Impact | Status |
|-------|--------|--------|--------|
| ESPER UI wiring | 4-6 hours | HIGH - Completes Blade Runner vision | ⬜ TODO |
| CBU struct alignment | 1-2 hours | MEDIUM - Enables template discrimination | ⬜ TODO |
| Trading Matrix | 0 | N/A - Already solid | ✅ DONE |
