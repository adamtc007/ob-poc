# CBU Entity Graph - Phase 2 Implementation TODO

## Context

Phase 1 complete:
- Template-based layout (FUND_MANDATE, CORPORATE_GROUP, FAMILY_TRUST)
- Role priority positioning (UBO=100, Shareholder=90, etc.)
- Camera with pan/zoom/fit
- Basic focus mode (blur non-connected)
- Layout toggle in toolbar

This phase adds the features needed for a solid testable UI.

## Reference

Full spec: `/docs/CBU_ENTITY_GRAPH_SPEC.md`

---

## 1. Bezier Edge Routing

**Location**: `crates/ob-poc-ui/src/graph/render.rs` (or new `edges.rs`)

### 1.1 Replace straight lines with quadratic bezier curves

```rust
pub struct EdgeCurve {
    pub from: Vec2,
    pub to: Vec2,
    pub control: Vec2,
}

impl EdgeCurve {
    pub fn new(from: Vec2, to: Vec2, curve_strength: f32) -> Self {
        let delta = to - from;
        let distance = delta.length();
        
        // Perpendicular offset for control point
        let perpendicular = Vec2::new(-delta.y, delta.x).normalize();
        let offset = perpendicular * distance * curve_strength;
        
        let control = (from + to) / 2.0 + offset;
        
        Self { from, to, control }
    }
    
    pub fn point_at(&self, t: f32) -> Vec2 {
        let t2 = t * t;
        let mt = 1.0 - t;
        let mt2 = mt * mt;
        self.from * mt2 + self.control * (2.0 * mt * t) + self.to * t2
    }
}
```

### 1.2 Curve strength by edge type

```rust
fn curve_strength(role: &PrimaryRole) -> f32 {
    match role {
        PrimaryRole::Shareholder | PrimaryRole::Ubo => 0.25,
        PrimaryRole::Director => 0.15,
        PrimaryRole::ServiceProvider => 0.35,
        _ => 0.20,
    }
}
```

### 1.3 Render bezier as line segments

```rust
fn render_bezier_edge(
    painter: &egui::Painter,
    curve: &EdgeCurve,
    stroke: egui::Stroke,
    to_screen: impl Fn(Vec2) -> egui::Pos2,
) {
    const SEGMENTS: usize = 20;
    let points: Vec<egui::Pos2> = (0..=SEGMENTS)
        .map(|i| {
            let t = i as f32 / SEGMENTS as f32;
            to_screen(curve.point_at(t))
        })
        .collect();
    
    painter.add(egui::Shape::line(points, stroke));
}
```

### 1.4 Edge intersection detection and hop rendering

See spec section 6.3-6.4. Lower priority edges "hop over" higher priority edges.

```rust
fn edge_priority(role: &PrimaryRole) -> u32 {
    match role {
        PrimaryRole::Ubo => 100,
        PrimaryRole::Shareholder => 90,
        PrimaryRole::Director => 70,
        PrimaryRole::ServiceProvider => 20,
        _ => 50,
    }
}
```

For hop rendering, draw arc at intersection point (spec 6.4 has full algorithm).

---

## 2. Level of Detail (LOD)

**Location**: `crates/ob-poc-ui/src/graph/render.rs`

### 2.1 LOD enum based on screen-space size

```rust
#[derive(Clone, Copy, PartialEq)]
pub enum DetailLevel {
    Micro,      // < 8px: colored dot only
    Icon,       // 8-20px: shape + status color
    Compact,    // 20-40px: shape + truncated name
    Standard,   // 40-80px: shape + full name + badge
    Expanded,   // 80px+: all details inline
    Focused,    // clicked: full card
}

impl DetailLevel {
    pub fn from_screen_radius(radius: f32, is_focused: bool) -> Self {
        if is_focused {
            return DetailLevel::Focused;
        }
        match radius {
            r if r < 8.0 => DetailLevel::Micro,
            r if r < 20.0 => DetailLevel::Icon,
            r if r < 40.0 => DetailLevel::Compact,
            r if r < 80.0 => DetailLevel::Standard,
            _ => DetailLevel::Expanded,
        }
    }
}
```

### 2.2 Render node based on LOD

```rust
fn render_node_at_lod(
    painter: &egui::Painter,
    node: &LayoutNode,
    screen_pos: egui::Pos2,
    screen_radius: f32,
    lod: DetailLevel,
    colors: &NodeColors,
) {
    match lod {
        DetailLevel::Micro => {
            painter.circle_filled(screen_pos, 3.0, colors.fill);
        }
        DetailLevel::Icon => {
            render_entity_shape(painter, screen_pos, node.entity_type, screen_radius, colors);
            if node.is_ubo {
                render_ubo_badge_small(painter, screen_pos, screen_radius);
            }
        }
        DetailLevel::Compact => {
            render_entity_shape(painter, screen_pos, node.entity_type, screen_radius, colors);
            let name = truncate_name(&node.name, 12);
            painter.text(
                screen_pos + egui::vec2(0.0, screen_radius + 8.0),
                egui::Align2::CENTER_TOP,
                name,
                egui::FontId::proportional(10.0),
                egui::Color32::from_rgb(66, 66, 66),
            );
        }
        DetailLevel::Standard => {
            render_entity_shape(painter, screen_pos, node.entity_type, screen_radius, colors);
            render_role_badge(painter, screen_pos, screen_radius, &node.primary_role);
            painter.text(
                screen_pos + egui::vec2(0.0, screen_radius + 8.0),
                egui::Align2::CENTER_TOP,
                &node.name,
                egui::FontId::proportional(11.0),
                egui::Color32::from_rgb(33, 33, 33),
            );
        }
        DetailLevel::Expanded => {
            render_node_expanded(painter, node, screen_pos, screen_radius, colors);
        }
        DetailLevel::Focused => {
            // Handled separately via focus card
            render_node_at_lod(painter, node, screen_pos, screen_radius, DetailLevel::Standard, colors);
        }
    }
}

fn truncate_name(name: &str, max_chars: usize) -> String {
    if name.len() <= max_chars {
        name.to_string()
    } else {
        format!("{}...", &name[..max_chars - 3])
    }
}
```

### 2.3 Calculate screen radius for LOD

```rust
fn screen_radius(world_radius: f32, zoom: f32) -> f32 {
    world_radius * zoom
}
```

---

## 3. Focus Card Panel

**Location**: `crates/ob-poc-ui/src/graph/focus_card.rs` (new file)

### 3.1 Focus card data structure

```rust
pub struct FocusCardData {
    pub entity_id: Uuid,
    pub name: String,
    pub entity_type: EntityType,
    pub jurisdiction: Option<String>,
    pub roles: Vec<RoleInfo>,
    pub kyc_status: Option<KycStatus>,
    pub risk_rating: Option<RiskRating>,
    pub connected_entities: Vec<ConnectedEntity>,
}

pub struct RoleInfo {
    pub role: PrimaryRole,
    pub target_entity: Option<String>,
    pub ownership_pct: Option<f32>,
}

pub struct ConnectedEntity {
    pub entity_id: Uuid,
    pub name: String,
    pub relationship: String,
}
```

### 3.2 Render focus card as egui::Window

```rust
pub fn render_focus_card(
    ctx: &egui::Context,
    data: &FocusCardData,
    on_close: impl FnOnce(),
    on_navigate: impl FnOnce(Uuid),
) {
    egui::Window::new("Entity Details")
        .id(egui::Id::new("focus_card"))
        .fixed_size([300.0, 400.0])
        .anchor(egui::Align2::RIGHT_CENTER, [-20.0, 0.0])
        .collapsible(false)
        .show(ctx, |ui| {
            // Header
            ui.horizontal(|ui| {
                ui.heading(&data.name);
            });
            ui.label(format!("{:?}", data.entity_type));
            if let Some(j) = &data.jurisdiction {
                ui.label(format!("Jurisdiction: {}", j));
            }
            
            ui.separator();
            
            // Roles
            ui.collapsing("Roles", |ui| {
                for role in &data.roles {
                    ui.horizontal(|ui| {
                        ui.label(format!("• {:?}", role.role));
                        if let Some(target) = &role.target_entity {
                            ui.label(format!("→ {}", target));
                        }
                        if let Some(pct) = role.ownership_pct {
                            ui.label(format!("({}%)", pct));
                        }
                    });
                }
            });
            
            // KYC Status (if available)
            if let Some(status) = &data.kyc_status {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("KYC Status:");
                    ui.colored_label(kyc_status_color(status), format!("{:?}", status));
                });
            }
            
            if let Some(risk) = &data.risk_rating {
                ui.horizontal(|ui| {
                    ui.label("Risk Rating:");
                    ui.colored_label(risk_color(risk), format!("{:?}", risk));
                });
            }
            
            ui.separator();
            
            // Connected entities
            ui.collapsing("Connections", |ui| {
                for conn in &data.connected_entities {
                    if ui.link(&conn.name).clicked() {
                        on_navigate(conn.entity_id);
                    }
                    ui.label(format!("  ({})", conn.relationship));
                }
            });
            
            ui.separator();
            
            // Actions
            ui.horizontal(|ui| {
                if ui.button("Close").clicked() {
                    on_close();
                }
                if ui.button("View Details").clicked() {
                    // Navigate to entity detail page
                }
            });
        });
}
```

### 3.3 Wire up to focus state

In `CbuGraphWidget::ui()`, when `focus_state.focused_entity.is_some()`:

```rust
if let Some(focused_id) = &self.focus_state.focused_entity {
    if let Some(node) = self.graph.nodes.iter().find(|n| &n.entity_id == focused_id) {
        let card_data = build_focus_card_data(node, &self.graph);
        render_focus_card(
            ui.ctx(),
            &card_data,
            || self.focus_state.clear_focus(),
            |entity_id| self.focus_state.set_focus(entity_id, &self.graph),
        );
    }
}
```

---

## 4. Investor Group Aggregation

**Location**: `crates/ob-poc-ui/src/graph/types.rs` + `render.rs`

### 4.1 Investor group struct

```rust
pub struct InvestorGroup {
    pub group_id: usize,
    pub share_class_id: Option<Uuid>,
    pub share_class_name: String,
    pub count: usize,
    pub total_ownership_pct: f32,
    pub position: Vec2,
    pub expanded: bool,
    pub members: Option<Vec<InvestorMember>>,
}

pub struct InvestorMember {
    pub entity_id: Uuid,
    pub name: String,
    pub ownership_pct: f32,
}

const INVESTOR_COLLAPSE_THRESHOLD: usize = 5;
```

### 4.2 Aggregate investors during layout

In layout phase, group entities with `PrimaryRole::Investor`:

```rust
fn aggregate_investors(nodes: &[LayoutNode]) -> Vec<InvestorGroup> {
    let investors: Vec<_> = nodes.iter()
        .filter(|n| n.primary_role == PrimaryRole::Investor)
        .collect();
    
    // Group by parent entity (share class or fund)
    let mut by_parent: HashMap<Uuid, Vec<&LayoutNode>> = HashMap::new();
    for inv in investors {
        // Find parent from edges
        if let Some(parent_id) = find_investor_parent(inv) {
            by_parent.entry(parent_id).or_default().push(inv);
        }
    }
    
    by_parent.into_iter().map(|(parent_id, members)| {
        InvestorGroup {
            group_id: 0, // assign later
            share_class_id: Some(parent_id),
            share_class_name: "Investors".to_string(), // lookup name
            count: members.len(),
            total_ownership_pct: members.iter()
                .filter_map(|m| m.ownership_pct)
                .sum(),
            position: Vec2::ZERO, // calculate from parent
            expanded: members.len() <= INVESTOR_COLLAPSE_THRESHOLD,
            members: if members.len() <= INVESTOR_COLLAPSE_THRESHOLD {
                Some(members.iter().map(|m| InvestorMember {
                    entity_id: m.entity_id,
                    name: m.name.clone(),
                    ownership_pct: m.ownership_pct.unwrap_or(0.0),
                }).collect())
            } else {
                None
            },
        }
    }).collect()
}
```

### 4.3 Render collapsed investor group

```rust
fn render_investor_group_collapsed(
    painter: &egui::Painter,
    group: &InvestorGroup,
    screen_pos: egui::Pos2,
    scale: f32,
) {
    let size = egui::vec2(100.0 * scale, 60.0 * scale);
    let rect = egui::Rect::from_center_size(screen_pos, size);
    
    // Background
    painter.rect_filled(rect, 6.0, egui::Color32::from_rgb(245, 245, 245));
    painter.rect_stroke(rect, 6.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(189, 189, 189)));
    
    // Icon + count
    painter.text(
        rect.center_top() + egui::vec2(0.0, 12.0 * scale),
        egui::Align2::CENTER_CENTER,
        format!("👥 {}", group.count),
        egui::FontId::proportional(12.0 * scale),
        egui::Color32::from_rgb(66, 66, 66),
    );
    
    // Ownership total
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        format!("{:.1}%", group.total_ownership_pct),
        egui::FontId::proportional(10.0 * scale),
        egui::Color32::from_rgb(97, 97, 97),
    );
    
    // Click hint
    if scale > 0.7 {
        painter.text(
            rect.center_bottom() - egui::vec2(0.0, 8.0 * scale),
            egui::Align2::CENTER_CENTER,
            "[expand]",
            egui::FontId::proportional(8.0 * scale),
            egui::Color32::from_rgb(150, 150, 150),
        );
    }
}
```

### 4.4 Handle expand/collapse on double-click

In input handler:

```rust
if response.double_clicked() {
    if let Some(group_idx) = self.find_investor_group_at(mouse_world) {
        self.investor_groups[group_idx].expanded = !self.investor_groups[group_idx].expanded;
    }
}
```

---

## 5. Edge Labels (Ownership %)

**Location**: `crates/ob-poc-ui/src/graph/render.rs`

### 5.1 Render percentage label at edge midpoint

```rust
fn render_edge_label(
    painter: &egui::Painter,
    curve: &EdgeCurve,
    label: &str,
    to_screen: impl Fn(Vec2) -> egui::Pos2,
) {
    let mid = curve.point_at(0.5);
    let screen_mid = to_screen(mid);
    
    // Background pill
    let text_size = egui::vec2(30.0, 14.0);
    let pill_rect = egui::Rect::from_center_size(screen_mid, text_size);
    painter.rect_filled(pill_rect, 7.0, egui::Color32::WHITE);
    painter.rect_stroke(pill_rect, 7.0, egui::Stroke::new(0.5, egui::Color32::from_rgb(200, 200, 200)));
    
    // Text
    painter.text(
        screen_mid,
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(9.0),
        egui::Color32::from_rgb(66, 66, 66),
    );
}
```

### 5.2 Show label only for ownership edges with percentage

```rust
fn should_show_edge_label(edge: &LayoutEdge, zoom: f32) -> bool {
    edge.ownership_pct.is_some() && zoom > 0.8
}
```

---

## 6. Edge Arrow Heads

**Location**: `crates/ob-poc-ui/src/graph/render.rs`

### 6.1 Draw arrow at edge end

```rust
fn render_arrow_head(
    painter: &egui::Painter,
    tip: egui::Pos2,
    direction: egui::Vec2,
    size: f32,
    color: egui::Color32,
) {
    let dir = direction.normalized();
    let perp = egui::vec2(-dir.y, dir.x);
    
    let p1 = tip;
    let p2 = tip - dir * size + perp * size * 0.5;
    let p3 = tip - dir * size - perp * size * 0.5;
    
    painter.add(egui::Shape::convex_polygon(
        vec![p1, p2, p3],
        color,
        egui::Stroke::NONE,
    ));
}
```

### 6.2 Calculate direction from bezier end

```rust
fn edge_end_direction(curve: &EdgeCurve) -> Vec2 {
    // Tangent at t=1
    (curve.to - curve.control).normalize()
}
```

---

## 7. Color Refinements

**Location**: `crates/ob-poc-ui/src/graph/render.rs` or new `colors.rs`

### 7.1 Risk rating colors

```rust
pub fn risk_color(rating: &RiskRating) -> egui::Color32 {
    match rating {
        RiskRating::Unrated => egui::Color32::from_rgb(158, 158, 158),
        RiskRating::Standard => egui::Color32::from_rgb(76, 175, 80),
        RiskRating::Low => egui::Color32::from_rgb(139, 195, 74),
        RiskRating::Medium => egui::Color32::from_rgb(255, 193, 7),
        RiskRating::High => egui::Color32::from_rgb(255, 87, 34),
        RiskRating::Prohibited => egui::Color32::from_rgb(33, 33, 33),
    }
}
```

### 7.2 KYC status colors

```rust
pub fn kyc_status_color(status: &KycStatus) -> egui::Color32 {
    match status {
        KycStatus::Verified => egui::Color32::from_rgb(76, 175, 80),
        KycStatus::InProgress => egui::Color32::from_rgb(33, 150, 243),
        KycStatus::Pending => egui::Color32::from_rgb(255, 193, 7),
        KycStatus::NotStarted => egui::Color32::from_rgb(158, 158, 158),
        KycStatus::Rejected => egui::Color32::from_rgb(244, 67, 54),
    }
}
```

### 7.3 Entity type colors (neutral for non-KYC view)

```rust
pub fn entity_type_color(entity_type: &EntityType) -> egui::Color32 {
    match entity_type {
        EntityType::NaturalPerson => egui::Color32::from_rgb(100, 181, 246),
        EntityType::LimitedCompany => egui::Color32::from_rgb(144, 164, 174),
        EntityType::Fund => egui::Color32::from_rgb(178, 223, 219),
        EntityType::Trust => egui::Color32::from_rgb(206, 147, 216),
        _ => egui::Color32::from_rgb(176, 190, 197),
    }
}
```

---

## 8. Keyboard Shortcuts Enhancement

**Location**: `crates/ob-poc-ui/src/graph/input.rs`

### 8.1 Add more shortcuts

```rust
pub fn handle_keyboard(
    ctx: &egui::Context,
    camera: &mut Camera2D,
    focus: &mut FocusState,
    graph: &LayoutGraph,
) {
    ctx.input(|i| {
        // Escape: clear focus
        if i.key_pressed(egui::Key::Escape) {
            focus.clear_focus();
        }
        
        // R or F: fit to view
        if i.key_pressed(egui::Key::R) || i.key_pressed(egui::Key::F) {
            camera.fit_to_bounds(graph.bounds(), 0.1);
        }
        
        // 0: reset zoom to 1.0
        if i.key_pressed(egui::Key::Num0) {
            camera.target_zoom = 1.0;
        }
        
        // +/=: zoom in
        if i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals) {
            camera.target_zoom = (camera.target_zoom * 1.2).min(10.0);
        }
        
        // -: zoom out
        if i.key_pressed(egui::Key::Minus) {
            camera.target_zoom = (camera.target_zoom / 1.2).max(0.1);
        }
        
        // Tab: cycle through entities
        if i.key_pressed(egui::Key::Tab) {
            cycle_focus(focus, graph, i.modifiers.shift);
        }
        
        // Arrow keys: pan
        let pan_speed = 50.0 / camera.zoom;
        if i.key_down(egui::Key::ArrowLeft) {
            camera.target_center.x -= pan_speed;
        }
        if i.key_down(egui::Key::ArrowRight) {
            camera.target_center.x += pan_speed;
        }
        if i.key_down(egui::Key::ArrowUp) {
            camera.target_center.y -= pan_speed;
        }
        if i.key_down(egui::Key::ArrowDown) {
            camera.target_center.y += pan_speed;
        }
    });
}

fn cycle_focus(focus: &mut FocusState, graph: &LayoutGraph, reverse: bool) {
    let nodes = &graph.nodes;
    if nodes.is_empty() {
        return;
    }
    
    let current_idx = focus.focused_entity
        .and_then(|id| nodes.iter().position(|n| n.entity_id == id));
    
    let next_idx = match current_idx {
        Some(i) => {
            if reverse {
                if i == 0 { nodes.len() - 1 } else { i - 1 }
            } else {
                (i + 1) % nodes.len()
            }
        }
        None => 0,
    };
    
    focus.set_focus(nodes[next_idx].entity_id, graph);
}
```

---

## File Structure After Phase 2

```
crates/ob-poc-ui/src/graph/
├── mod.rs              # CbuGraphWidget
├── types.rs            # Core types + InvestorGroup
├── camera.rs           # Camera2D (existing)
├── layout.rs           # Template layout (existing)
├── render.rs           # Node/edge rendering + LOD
├── edges.rs            # NEW: Bezier curves, arrows, labels
├── input.rs            # Input handling + keyboard
├── focus_card.rs       # NEW: Focus card panel
└── colors.rs           # NEW: Color palettes
```

---

## Testing Checklist

After implementation, test:

- [ ] Bezier edges render smoothly
- [ ] Edge labels show ownership %
- [ ] Arrows point correct direction
- [ ] LOD transitions as zoom changes
- [ ] Focus card appears on click
- [ ] Focus card "Connections" list is accurate
- [ ] Investor groups collapse when > 5
- [ ] Double-click expands investor group
- [ ] Tab cycles through entities
- [ ] Arrow keys pan smoothly
- [ ] Colors match spec (risk, KYC, entity type)

---

## Priority Order

1. **Bezier edges** (biggest visual impact)
2. **LOD system** (handles complex graphs)
3. **Edge labels + arrows** (completes edge rendering)
4. **Focus card** (gives detail on interaction)
5. **Colors** (polish)
6. **Keyboard shortcuts** (power user feature)
7. **Investor aggregation** (needed for real data)
