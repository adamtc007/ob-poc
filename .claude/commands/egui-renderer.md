# egui Graph Renderer Implementation

Implement the egui renderer that draws PositionedGraph to the viewport.

## Files to Create

1. `rust/crates/ob-poc-ui/src/egui/graph_renderer.rs`
2. `rust/crates/ob-poc-ui/src/egui/overlays.rs`

## Graph Renderer

### graph_renderer.rs
```rust
use egui::{
    Ui, Painter, Pos2, Vec2, Rect, Color32, Stroke, Shape,
    FontId, Align2, Response, Sense, CursorIcon,
};

/// Interaction state for graph rendering
#[derive(Debug, Default)]
pub struct GraphInteractionState {
    pub hovered_node: Option<EntityId>,
    pub selected_nodes: Vec<EntityId>,
    pub dragging_node: Option<EntityId>,
    pub last_click_pos: Option<Pos2>,
    pub double_click_node: Option<EntityId>,
}

/// Main rendering function
pub fn render_positioned_graph(
    ui: &mut Ui,
    graph: &PositionedGraph,
    interaction: &mut GraphInteractionState,
    zoom: f32,
    pan: Vec2,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(
        ui.available_size(),
        Sense::click_and_drag(),
    );
    
    if !ui.is_rect_visible(rect) {
        return response;
    }
    
    let painter = ui.painter_at(rect);
    let transform = GraphTransform::new(rect, zoom, pan);
    
    // 1. Draw floating zone background (if present)
    if let Some(ref fz) = graph.floating_zone {
        draw_floating_zone(&painter, fz, &transform);
    }
    
    // 2. Draw edges (behind nodes)
    for edge in &graph.edges {
        draw_edge(&painter, edge, &transform, graph);
    }
    
    // 3. Draw nodes
    interaction.hovered_node = None;
    interaction.double_click_node = None;
    
    for node in &graph.nodes {
        let node_response = draw_node(
            ui,
            &painter,
            node,
            &transform,
            interaction,
        );
        
        // Track interactions
        if node_response.hovered() {
            interaction.hovered_node = Some(node.id);
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        
        if node_response.clicked() {
            if interaction.selected_nodes.contains(&node.id) {
                interaction.selected_nodes.retain(|&id| id != node.id);
            } else {
                if !ui.input(|i| i.modifiers.shift) {
                    interaction.selected_nodes.clear();
                }
                interaction.selected_nodes.push(node.id);
            }
        }
        
        if node_response.double_clicked() {
            interaction.double_click_node = Some(node.id);
        }
    }
    
    // 4. Draw selection highlights
    for &selected_id in &interaction.selected_nodes {
        if let Some(node) = graph.nodes.iter().find(|n| n.id == selected_id) {
            draw_selection_highlight(&painter, node, &transform);
        }
    }
    
    // 5. Draw matrix headers (if present)
    if let Some(ref col_headers) = graph.col_headers {
        draw_column_headers(&painter, col_headers, graph, &transform);
    }
    if let Some(ref row_headers) = graph.row_headers {
        draw_row_headers(&painter, row_headers, graph, &transform);
    }
    
    response
}

/// Transform between world and screen coordinates
struct GraphTransform {
    rect: Rect,
    zoom: f32,
    pan: Vec2,
}

impl GraphTransform {
    fn new(rect: Rect, zoom: f32, pan: Vec2) -> Self {
        Self { rect, zoom, pan }
    }
    
    fn world_to_screen(&self, pos: Pos2) -> Pos2 {
        let centered = pos - self.rect.center().to_vec2();
        let zoomed = centered * self.zoom;
        let panned = zoomed + self.pan;
        self.rect.center() + panned.to_vec2()
    }
    
    fn world_to_screen_size(&self, size: Vec2) -> Vec2 {
        size * self.zoom
    }
    
    fn screen_to_world(&self, pos: Pos2) -> Pos2 {
        let from_center = pos - self.rect.center();
        let unpanned = from_center - self.pan;
        let unzoomed = unpanned / self.zoom;
        self.rect.center() + unzoomed.to_vec2()
    }
}

/// Draw a single edge
fn draw_edge(
    painter: &Painter,
    edge: &PositionedEdge,
    transform: &GraphTransform,
    graph: &PositionedGraph,
) {
    if edge.path.len() < 2 {
        return;
    }
    
    let stroke = Stroke::new(
        edge.style.width * transform.zoom.sqrt(),
        edge.style.color,
    );
    
    let screen_path: Vec<Pos2> = edge.path.iter()
        .map(|&p| transform.world_to_screen(p))
        .collect();
    
    if edge.path.len() == 2 {
        // Simple line
        if edge.style.dashed {
            draw_dashed_line(painter, screen_path[0], screen_path[1], stroke);
        } else {
            painter.line_segment([screen_path[0], screen_path[1]], stroke);
        }
    } else if edge.path.len() == 3 {
        // Quadratic bezier
        let shape = Shape::QuadraticBezier(egui::epaint::QuadraticBezierShape {
            points: [screen_path[0], screen_path[1], screen_path[2]],
            closed: false,
            fill: Color32::TRANSPARENT,
            stroke: stroke.into(),
        });
        painter.add(shape);
    } else {
        // Polyline for more complex paths
        painter.add(Shape::line(screen_path.clone(), stroke));
    }
    
    // Draw arrowhead
    if edge.style.arrow && screen_path.len() >= 2 {
        let end = screen_path[screen_path.len() - 1];
        let prev = screen_path[screen_path.len() - 2];
        draw_arrowhead(painter, prev, end, stroke);
    }
}

fn draw_dashed_line(painter: &Painter, from: Pos2, to: Pos2, stroke: Stroke) {
    let dir = (to - from).normalized();
    let len = (to - from).length();
    let dash_len = 8.0;
    let gap_len = 4.0;
    
    let mut pos = 0.0;
    while pos < len {
        let start = from + dir * pos;
        let end_pos = (pos + dash_len).min(len);
        let end = from + dir * end_pos;
        painter.line_segment([start, end], stroke);
        pos += dash_len + gap_len;
    }
}

fn draw_arrowhead(painter: &Painter, from: Pos2, to: Pos2, stroke: Stroke) {
    let dir = (to - from).normalized();
    let perp = Vec2::new(-dir.y, dir.x);
    
    let arrow_size = 10.0;
    let arrow_angle = 0.4;  // radians
    
    let tip = to;
    let left = tip - dir * arrow_size + perp * arrow_size * arrow_angle;
    let right = tip - dir * arrow_size - perp * arrow_size * arrow_angle;
    
    painter.add(Shape::convex_polygon(
        vec![tip, left, right],
        stroke.color,
        Stroke::NONE,
    ));
}

/// Draw a single node
fn draw_node(
    ui: &mut Ui,
    painter: &Painter,
    node: &PositionedNode,
    transform: &GraphTransform,
    interaction: &GraphInteractionState,
) -> Response {
    let screen_pos = transform.world_to_screen(node.position);
    let screen_size = transform.world_to_screen_size(node.size);
    let node_rect = Rect::from_center_size(screen_pos, screen_size);
    
    // Apply alpha for transitions
    let alpha = node.alpha.unwrap_or(1.0);
    let fill_color = apply_alpha(node.style.fill_color, alpha);
    let stroke_color = apply_alpha(node.style.stroke_color, alpha);
    
    // Check if hovered
    let is_hovered = interaction.hovered_node == Some(node.id);
    let is_selected = interaction.selected_nodes.contains(&node.id);
    
    // Draw shape
    match node.style.shape {
        NodeShape::Rectangle => {
            let rounding = 4.0;
            painter.rect(
                node_rect,
                rounding,
                fill_color,
                Stroke::new(
                    if is_hovered || is_selected { 3.0 } else { node.style.stroke_width },
                    if is_selected { Color32::from_rgb(241, 196, 15) } else { stroke_color },
                ),
            );
        }
        NodeShape::Circle => {
            let radius = screen_size.x.min(screen_size.y) / 2.0;
            painter.circle(
                screen_pos,
                radius,
                fill_color,
                Stroke::new(
                    if is_hovered || is_selected { 3.0 } else { node.style.stroke_width },
                    if is_selected { Color32::from_rgb(241, 196, 15) } else { stroke_color },
                ),
            );
        }
        NodeShape::Diamond => {
            let half_w = screen_size.x / 2.0;
            let half_h = screen_size.y / 2.0;
            let points = vec![
                screen_pos + Vec2::new(0.0, -half_h),  // top
                screen_pos + Vec2::new(half_w, 0.0),   // right
                screen_pos + Vec2::new(0.0, half_h),   // bottom
                screen_pos + Vec2::new(-half_w, 0.0),  // left
            ];
            painter.add(Shape::convex_polygon(
                points,
                fill_color,
                Stroke::new(node.style.stroke_width, stroke_color),
            ));
        }
        NodeShape::Hexagon => {
            let radius = screen_size.x.min(screen_size.y) / 2.0;
            let points: Vec<Pos2> = (0..6)
                .map(|i| {
                    let angle = std::f32::consts::TAU * i as f32 / 6.0 - std::f32::consts::FRAC_PI_2;
                    screen_pos + Vec2::angled(angle) * radius
                })
                .collect();
            painter.add(Shape::convex_polygon(
                points,
                fill_color,
                Stroke::new(node.style.stroke_width, stroke_color),
            ));
        }
    }
    
    // Draw label
    if transform.zoom > 0.5 {  // Only show labels when zoomed in enough
        let text_color = if is_text_dark(fill_color) {
            Color32::WHITE
        } else {
            Color32::BLACK
        };
        
        let font_size = (node.style.font_size * transform.zoom).clamp(8.0, 16.0);
        
        painter.text(
            screen_pos,
            Align2::CENTER_CENTER,
            &node.name,
            FontId::proportional(font_size),
            apply_alpha(text_color, alpha),
        );
    }
    
    // Return response for interaction handling
    ui.interact(node_rect, ui.id().with(node.id), Sense::click())
}

fn apply_alpha(color: Color32, alpha: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(
        color.r(),
        color.g(),
        color.b(),
        (color.a() as f32 * alpha) as u8,
    )
}

fn is_text_dark(bg: Color32) -> bool {
    // Simple luminance check
    let lum = 0.299 * bg.r() as f32 + 0.587 * bg.g() as f32 + 0.114 * bg.b() as f32;
    lum < 128.0
}

fn draw_selection_highlight(painter: &Painter, node: &PositionedNode, transform: &GraphTransform) {
    let screen_pos = transform.world_to_screen(node.position);
    let screen_size = transform.world_to_screen_size(node.size) + Vec2::splat(10.0);
    let rect = Rect::from_center_size(screen_pos, screen_size);
    
    // Glowing outline effect
    for i in 0..3 {
        let offset = (i + 1) as f32 * 2.0;
        let alpha = 100 - i * 30;
        painter.rect_stroke(
            rect.expand(offset),
            6.0,
            Stroke::new(2.0, Color32::from_rgba_unmultiplied(241, 196, 15, alpha as u8)),
        );
    }
}

fn draw_floating_zone(painter: &Painter, zone: &FloatingZoneLayout, transform: &GraphTransform) {
    let screen_rect = Rect::from_min_max(
        transform.world_to_screen(zone.bounds.min),
        transform.world_to_screen(zone.bounds.max),
    );
    
    // Background
    painter.rect_filled(
        screen_rect,
        4.0,
        Color32::from_rgba_unmultiplied(128, 128, 128, 20),
    );
    
    // Border
    painter.rect_stroke(
        screen_rect,
        4.0,
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(128, 128, 128, 80)),
    );
    
    // Label
    if let Some(ref label) = zone.label {
        painter.text(
            screen_rect.left_top() + Vec2::new(8.0, 4.0),
            Align2::LEFT_TOP,
            label,
            FontId::proportional(11.0),
            Color32::GRAY,
        );
    }
}

fn draw_column_headers(
    painter: &Painter,
    headers: &[String],
    graph: &PositionedGraph,
    transform: &GraphTransform,
) {
    let header_height = graph.header_height.unwrap_or(40.0);
    let cell_width = graph.cell_width.unwrap_or(100.0);
    let row_header_width = graph.row_header_width.unwrap_or(120.0);
    
    for (i, header) in headers.iter().enumerate() {
        let x = row_header_width + i as f32 * (cell_width + 10.0) + cell_width / 2.0;
        let pos = transform.world_to_screen(Pos2::new(x, header_height / 2.0));
        
        painter.text(
            pos,
            Align2::CENTER_CENTER,
            header,
            FontId::proportional(12.0),
            Color32::WHITE,
        );
    }
}

fn draw_row_headers(
    painter: &Painter,
    headers: &[String],
    graph: &PositionedGraph,
    transform: &GraphTransform,
) {
    let header_height = graph.header_height.unwrap_or(40.0);
    let cell_height = graph.cell_height.unwrap_or(60.0);
    let row_header_width = graph.row_header_width.unwrap_or(120.0);
    
    for (i, header) in headers.iter().enumerate() {
        let y = header_height + i as f32 * (cell_height + 10.0) + cell_height / 2.0;
        let pos = transform.world_to_screen(Pos2::new(row_header_width / 2.0, y));
        
        painter.text(
            pos,
            Align2::CENTER_CENTER,
            header,
            FontId::proportional(12.0),
            Color32::WHITE,
        );
    }
}
```

## Overlays

### overlays.rs
```rust
use egui::{Ui, Area, Color32, RichText, Vec2};

/// Render the scope indicator overlay
pub fn render_scope_indicator(ui: &mut Ui, session: &SessionContext) {
    Area::new("scope_indicator")
        .fixed_pos(egui::pos2(10.0, 10.0))
        .show(ui.ctx(), |ui| {
            egui::Frame::none()
                .fill(Color32::from_rgba_unmultiplied(30, 30, 30, 220))
                .inner_margin(8.0)
                .rounding(4.0)
                .show(ui, |ui| {
                    // Breadcrumb
                    ui.horizontal(|ui| {
                        for (i, segment) in session.scope.segments.iter().enumerate() {
                            if i > 0 {
                                ui.label(RichText::new("›").color(Color32::GRAY));
                            }
                            
                            let is_last = i == session.scope.segments.len() - 1;
                            
                            if is_last {
                                ui.label(RichText::new(&segment.name).strong().color(Color32::WHITE));
                            } else {
                                if ui.link(&segment.name).clicked() {
                                    // TODO: Navigate up to this level
                                }
                            }
                            
                            ui.label(
                                RichText::new(format!("({})", segment.mass))
                                    .small()
                                    .color(Color32::GRAY)
                            );
                        }
                    });
                    
                    ui.add_space(4.0);
                    
                    // Stats line
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("Mass: {}", session.mass.total))
                                .color(Color32::from_rgb(241, 196, 15))
                        );
                        ui.separator();
                        ui.label(
                            RichText::new(format!("{:?}", session.view_mode))
                                .color(Color32::from_rgb(52, 152, 219))
                        );
                        ui.separator();
                        ui.label(
                            RichText::new(format!(
                                "{} CBUs | {} persons | {} floating",
                                session.mass.breakdown.cbus,
                                session.mass.breakdown.persons,
                                session.mass.breakdown.floating,
                            ))
                            .small()
                            .color(Color32::LIGHT_GRAY)
                        );
                    });
                });
        });
}

/// Render blast radius visualization when operation pending
pub fn render_blast_radius_overlay(
    ui: &mut Ui,
    graph: &PositionedGraph,
    affected_ids: &[EntityId],
    operation: &str,
) {
    // Draw glow around affected nodes
    let painter = ui.painter();
    
    for node in &graph.nodes {
        if affected_ids.contains(&node.id) {
            // Pulse animation
            let time = ui.input(|i| i.time);
            let pulse = ((time * 3.0).sin() * 0.5 + 0.5) as f32;
            let alpha = (100.0 + pulse * 80.0) as u8;
            
            let rect = Rect::from_center_size(node.position, node.size + Vec2::splat(20.0));
            
            painter.rect_stroke(
                rect,
                8.0,
                Stroke::new(3.0, Color32::from_rgba_unmultiplied(231, 76, 60, alpha)),
            );
        }
    }
    
    // Count badge
    Area::new("blast_radius_badge")
        .fixed_pos(egui::pos2(10.0, ui.available_height() - 60.0))
        .show(ui.ctx(), |ui| {
            egui::Frame::none()
                .fill(Color32::from_rgba_unmultiplied(231, 76, 60, 220))
                .inner_margin(12.0)
                .rounding(4.0)
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(format!(
                            "⚠️ {} will affect {} entities",
                            operation,
                            affected_ids.len()
                        ))
                        .color(Color32::WHITE)
                        .strong()
                    );
                });
        });
}
```

## Acceptance Criteria

- [ ] Edges render behind nodes
- [ ] Bezier curves for multi-segment paths
- [ ] Arrowheads on directed edges
- [ ] Dashed lines for non-structural edges
- [ ] Node shapes: rectangle, circle, diamond, hexagon
- [ ] Selection highlight with glow effect
- [ ] Floating zone background and label
- [ ] Labels fade at low zoom levels
- [ ] Scope indicator shows breadcrumb and stats
- [ ] Blast radius overlay pulses affected nodes
