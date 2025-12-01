//! Graph visualization view with pan/zoom

use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2};
use serde::Deserialize;
use std::collections::HashMap;

/// Graph data from the API
#[derive(Default, Clone, Deserialize)]
pub struct CbuGraph {
    pub cbu_id: uuid::Uuid,
    pub label: String,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub layers: Vec<LayerInfo>,
}

#[derive(Clone, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub node_type: String,
    pub layer: String,
    pub label: String,
    pub sublabel: Option<String>,
    pub status: String,
    pub data: serde_json::Value,
    /// Parent node ID for hierarchical grouping (e.g., market groups custody items)
    #[serde(default)]
    pub parent_id: Option<String>,
}

#[derive(Clone, Deserialize)]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub edge_type: String,
    pub label: Option<String>,
}

#[derive(Clone, Deserialize)]
pub struct LayerInfo {
    pub layer_type: String,
    pub label: String,
    pub color: String,
    pub node_count: usize,
    pub visible: bool,
}

/// Graph visualization widget
pub struct GraphView {
    pub graph: Option<CbuGraph>,
    pub show_custody: bool,
    pub show_kyc: bool,
    pub show_ubo: bool,
    pub show_services: bool,

    // Pan/zoom state
    offset: Vec2,
    zoom: f32,

    // Layout cache
    node_positions: HashMap<String, Pos2>,
    selected_node: Option<String>,
}

impl Default for GraphView {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphView {
    pub fn new() -> Self {
        Self {
            graph: None,
            show_custody: true,
            show_kyc: false,
            show_ubo: false,
            show_services: false,
            offset: Vec2::ZERO,
            zoom: 1.0,
            node_positions: HashMap::new(),
            selected_node: None,
        }
    }

    pub fn set_graph(&mut self, graph: CbuGraph) {
        self.graph = Some(graph);
        self.compute_layout();
    }

    fn compute_layout(&mut self) {
        let Some(ref graph) = self.graph else {
            return;
        };

        self.node_positions.clear();

        use std::f32::consts::PI;

        // Radial/Orbital layout: CBU at center, layers as rings outward
        // Markets are grouping nodes - they go on the custody ring, children fan out from them

        let ring_radius = |layer: &str, node_type: &str| -> f32 {
            match (layer, node_type) {
                ("core", "cbu") => 0.0,         // CBU at center
                ("core", _) => 180.0,           // Entities - first ring
                ("custody", "market") => 320.0, // Markets - inner custody ring
                ("custody", "isda") => 350.0,   // ISDA on custody ring (not grouped by market)
                ("custody", "csa") => 420.0,    // CSA outer from ISDA
                ("custody", _) => 420.0, // Other custody items - outer ring (will be repositioned if parented)
                ("kyc", _) => 520.0,     // KYC - third ring
                ("ubo", _) => 520.0,     // UBO - same ring as KYC
                ("services", _) => 650.0, // Services - outer ring
                _ => 350.0,
            }
        };

        // Group nodes by ring (for non-parented nodes)
        let node_ring = |layer: &str, node_type: &str| -> String {
            match (layer, node_type) {
                ("core", "cbu") => "center".to_string(),
                ("core", _) => "ring1".to_string(),
                ("custody", "market") => "ring2_market".to_string(),
                ("custody", "isda") => "ring2_isda".to_string(),
                ("custody", "csa") => "ring2_csa".to_string(),
                ("custody", _) => "ring2_custody".to_string(),
                ("kyc", _) => "ring3".to_string(),
                ("ubo", _) => "ring3".to_string(),
                ("services", _) => "ring4".to_string(),
                _ => "ring2_custody".to_string(),
            }
        };

        // First pass: identify market nodes and their children
        let mut market_children: HashMap<String, Vec<String>> = HashMap::new();
        let mut parented_nodes: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        for node in &graph.nodes {
            if let Some(ref parent_id) = node.parent_id {
                market_children
                    .entry(parent_id.clone())
                    .or_default()
                    .push(node.id.clone());
                parented_nodes.insert(node.id.clone());
            }
        }

        // Count nodes per ring (excluding parented nodes)
        let mut ring_counts: HashMap<String, usize> = HashMap::new();
        let mut ring_positions: HashMap<String, usize> = HashMap::new();

        for node in &graph.nodes {
            if parented_nodes.contains(&node.id) {
                continue; // Skip parented nodes in ring count
            }
            let ring = node_ring(&node.layer, &node.node_type);
            *ring_counts.entry(ring).or_insert(0) += 1;
        }

        // Position non-parented nodes radially
        for node in &graph.nodes {
            if parented_nodes.contains(&node.id) {
                continue; // Will position later based on parent
            }

            let ring = node_ring(&node.layer, &node.node_type);
            let radius = ring_radius(&node.layer, &node.node_type);
            let count = ring_counts.get(&ring).copied().unwrap_or(1);
            let pos = ring_positions.entry(ring).or_insert(0);

            let (x, y) = if radius == 0.0 {
                // Center node (CBU)
                (0.0, 0.0)
            } else {
                // Distribute evenly around the ring, starting from top
                let angle = -PI / 2.0 + (*pos as f32) * 2.0 * PI / (count as f32);
                (radius * angle.cos(), radius * angle.sin())
            };

            self.node_positions.insert(node.id.clone(), Pos2::new(x, y));
            *pos += 1;
        }

        // Second pass: position children around their parent markets
        let child_offset_radius = 110.0; // Distance from parent
        let child_arc_span = PI / 3.0; // 60 degree arc for children

        for (parent_id, children) in &market_children {
            let Some(parent_pos) = self.node_positions.get(parent_id).copied() else {
                continue;
            };

            // Calculate angle from center to parent
            let parent_angle = parent_pos.y.atan2(parent_pos.x);

            // Distribute children in an arc centered on parent's angle
            let child_count = children.len();
            for (i, child_id) in children.iter().enumerate() {
                let child_angle = if child_count == 1 {
                    parent_angle // Single child: same angle as parent, just further out
                } else {
                    // Fan out children in an arc
                    let arc_start = parent_angle - child_arc_span / 2.0;
                    arc_start + (i as f32) * child_arc_span / ((child_count - 1) as f32)
                };

                // Position child at offset from parent
                let child_x = parent_pos.x + child_offset_radius * child_angle.cos();
                let child_y = parent_pos.y + child_offset_radius * child_angle.sin();

                self.node_positions
                    .insert(child_id.clone(), Pos2::new(child_x, child_y));
            }
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let Some(ref graph) = self.graph else {
            ui.centered_and_justified(|ui| {
                ui.label("Select a CBU to visualize");
            });
            return;
        };

        // Get available rect and create painter
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());

        let rect = response.rect;
        let center = rect.center();

        // Handle pan
        if response.dragged() {
            self.offset += response.drag_delta();
        }

        // Handle zoom (scroll)
        let scroll = ui.input(|i| i.raw_scroll_delta.y);
        if scroll != 0.0 {
            let zoom_factor = 1.0 + scroll * 0.001;
            self.zoom = (self.zoom * zoom_factor).clamp(0.2, 3.0);
        }

        // Transform helper
        let transform = |pos: Pos2| -> Pos2 { center + (pos.to_vec2() + self.offset) * self.zoom };
        let transform_center = center + self.offset * self.zoom;

        // Draw faint ring circles (behind everything)
        let ring_color = Color32::from_rgba_unmultiplied(100, 100, 120, 40);
        let ring_radii = [180.0, 350.0, 500.0, 650.0]; // Matches layout radii
        let ring_labels = ["Entities", "Custody", "KYC/UBO", "Services"];

        for (i, &radius) in ring_radii.iter().enumerate() {
            let scaled_radius = radius * self.zoom;
            painter.circle_stroke(
                transform_center,
                scaled_radius,
                Stroke::new(1.0, ring_color),
            );

            // Ring label at top of each ring
            let label_pos = transform_center + Vec2::new(0.0, -scaled_radius - 12.0 * self.zoom);
            painter.text(
                label_pos,
                egui::Align2::CENTER_BOTTOM,
                ring_labels[i],
                egui::FontId::proportional(10.0 * self.zoom),
                Color32::from_rgba_unmultiplied(150, 150, 170, 80),
            );
        }

        // Draw edges first (below nodes)
        for edge in &graph.edges {
            let Some(source_pos) = self.node_positions.get(&edge.source) else {
                continue;
            };
            let Some(target_pos) = self.node_positions.get(&edge.target) else {
                continue;
            };

            let from = transform(*source_pos);
            let to = transform(*target_pos);

            let color = Color32::from_rgb(100, 100, 100);
            painter.line_segment([from, to], Stroke::new(1.5 * self.zoom, color));

            // Draw edge label
            if let Some(ref label) = edge.label {
                let mid = Pos2::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0);
                painter.text(
                    mid,
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(10.0 * self.zoom),
                    Color32::GRAY,
                );
            }
        }

        // Draw nodes
        for node in &graph.nodes {
            // Filter by layer visibility
            let visible = match node.layer.as_str() {
                "core" => true,
                "custody" => self.show_custody,
                "kyc" => self.show_kyc,
                "ubo" => self.show_ubo,
                "services" => self.show_services,
                _ => true,
            };

            if !visible {
                continue;
            }

            let Some(pos) = self.node_positions.get(&node.id) else {
                continue;
            };
            let screen_pos = transform(*pos);

            // Node styling based on type
            let (bg_color, border_color) = node_colors(&node.node_type, &node.status);

            let node_size = Vec2::new(140.0, 50.0) * self.zoom;
            let node_rect = Rect::from_center_size(screen_pos, node_size);

            // Draw node background
            painter.rect_filled(node_rect, 8.0 * self.zoom, bg_color);
            painter.rect_stroke(
                node_rect,
                8.0 * self.zoom,
                Stroke::new(2.0 * self.zoom, border_color),
            );

            // Draw label
            painter.text(
                screen_pos - Vec2::new(0.0, 8.0 * self.zoom),
                egui::Align2::CENTER_CENTER,
                &node.label,
                egui::FontId::proportional(12.0 * self.zoom),
                Color32::WHITE,
            );

            // Draw sublabel
            if let Some(ref sublabel) = node.sublabel {
                painter.text(
                    screen_pos + Vec2::new(0.0, 10.0 * self.zoom),
                    egui::Align2::CENTER_CENTER,
                    sublabel,
                    egui::FontId::proportional(10.0 * self.zoom),
                    Color32::from_rgb(180, 180, 180),
                );
            }

            // Handle click
            if response.clicked() {
                if let Some(pointer_pos) = response.interact_pointer_pos() {
                    if node_rect.contains(pointer_pos) {
                        self.selected_node = Some(node.id.clone());
                    }
                }
            }
        }

        // Draw zoom/pan info
        painter.text(
            rect.left_bottom() + Vec2::new(10.0, -10.0),
            egui::Align2::LEFT_BOTTOM,
            format!(
                "Zoom: {:.0}%  |  Drag to pan, scroll to zoom",
                self.zoom * 100.0
            ),
            egui::FontId::proportional(11.0),
            Color32::GRAY,
        );
    }
}

fn node_colors(node_type: &str, status: &str) -> (Color32, Color32) {
    let base = match node_type {
        "cbu" => Color32::from_rgb(75, 85, 99),            // Gray
        "market" => Color32::from_rgb(14, 165, 233),       // Sky blue - grouping node
        "universe" => Color32::from_rgb(59, 130, 246),     // Blue
        "ssi" => Color32::from_rgb(16, 185, 129),          // Green
        "booking_rule" => Color32::from_rgb(245, 158, 11), // Amber
        "isda" => Color32::from_rgb(139, 92, 246),         // Purple
        "csa" => Color32::from_rgb(236, 72, 153),          // Pink
        "entity" => Color32::from_rgb(34, 197, 94),        // Emerald
        "document" => Color32::from_rgb(99, 102, 241),     // Indigo
        "resource" => Color32::from_rgb(249, 115, 22),     // Orange
        _ => Color32::from_rgb(107, 114, 128),             // Gray
    };

    let border = match status {
        "active" => Color32::from_rgb(34, 197, 94),    // Green
        "pending" => Color32::from_rgb(251, 191, 36),  // Yellow
        "suspended" => Color32::from_rgb(239, 68, 68), // Red
        "expired" => Color32::from_rgb(107, 114, 128), // Gray
        "draft" => Color32::from_rgb(156, 163, 175),   // Light gray
        _ => Color32::WHITE,
    };

    (base, border)
}
