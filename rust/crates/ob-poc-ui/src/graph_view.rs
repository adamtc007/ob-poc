//! Hierarchical tree visualization view with pan/zoom

#![allow(dead_code)]

use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2};
use serde::Deserialize;
use std::collections::HashMap;

/// View mode - two distinct views
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    /// KYC/UBO structure: Who is this client? Who owns/controls it?
    #[default]
    KycUbo,
    /// Service delivery map: What services does this client receive?
    ServiceDelivery,
}

/// Tree visualization from the /api/cbu/:id/tree endpoint
#[derive(Clone, Deserialize, Default)]
pub struct CbuTreeVisualization {
    pub cbu_id: uuid::Uuid,
    pub cbu_name: String,
    pub client_type: Option<String>,
    pub jurisdiction: Option<String>,
    pub view_mode: String,
    pub root: TreeNode,
    pub overlay_edges: Vec<TreeEdge>,
    pub stats: VisualizationStats,
}

#[derive(Clone, Deserialize, Default)]
pub struct TreeNode {
    pub id: uuid::Uuid,
    pub node_type: String,
    pub label: String,
    pub sublabel: Option<String>,
    pub jurisdiction: Option<String>,
    pub children: Vec<TreeNode>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Clone, Deserialize, Default)]
pub struct TreeEdge {
    pub from: uuid::Uuid,
    pub to: uuid::Uuid,
    pub edge_type: String,
    pub label: Option<String>,
    pub weight: Option<f32>,
}

#[derive(Clone, Deserialize, Default)]
pub struct VisualizationStats {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub max_depth: usize,
    pub entity_count: usize,
    pub person_count: usize,
    pub share_class_count: usize,
}

/// Graph visualization widget
pub struct GraphView {
    tree: Option<CbuTreeVisualization>,

    // Pan/zoom state
    offset: Vec2,
    zoom: f32,

    // Layout cache for hierarchical tree
    tree_positions: HashMap<uuid::Uuid, Pos2>,
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
            tree: None,
            offset: Vec2::ZERO,
            zoom: 1.0,
            tree_positions: HashMap::new(),
            selected_node: None,
        }
    }

    pub fn set_tree(&mut self, tree: CbuTreeVisualization) {
        self.tree = Some(tree);
        self.compute_tree_layout();
    }

    fn compute_tree_layout(&mut self) {
        let Some(ref tree) = self.tree else {
            return;
        };

        self.tree_positions.clear();

        // Hierarchical top-down tree layout
        let node_width = 160.0;
        let node_height = 60.0;
        let h_spacing = 40.0;
        let v_spacing = 100.0;

        // First pass: calculate subtree widths
        fn calc_subtree_width(node: &TreeNode, node_width: f32, h_spacing: f32) -> f32 {
            if node.children.is_empty() {
                node_width
            } else {
                let children_width: f32 = node
                    .children
                    .iter()
                    .map(|c| calc_subtree_width(c, node_width, h_spacing))
                    .sum();
                let gaps = (node.children.len().saturating_sub(1)) as f32 * h_spacing;
                (children_width + gaps).max(node_width)
            }
        }

        // Second pass: position nodes
        fn position_nodes(
            node: &TreeNode,
            x: f32,
            y: f32,
            available_width: f32,
            node_width: f32,
            node_height: f32,
            h_spacing: f32,
            v_spacing: f32,
            positions: &mut HashMap<uuid::Uuid, Pos2>,
        ) {
            let center_x = x + available_width / 2.0;
            positions.insert(node.id, Pos2::new(center_x, y));

            if node.children.is_empty() {
                return;
            }

            let child_widths: Vec<f32> = node
                .children
                .iter()
                .map(|c| calc_subtree_width(c, node_width, h_spacing))
                .collect();
            let total_children_width: f32 = child_widths.iter().sum();
            let gaps = (node.children.len().saturating_sub(1)) as f32 * h_spacing;
            let total_width = total_children_width + gaps;

            let mut child_x = center_x - total_width / 2.0;
            let child_y = y + node_height + v_spacing;

            for (i, child) in node.children.iter().enumerate() {
                let child_width = child_widths[i];
                position_nodes(
                    child,
                    child_x,
                    child_y,
                    child_width,
                    node_width,
                    node_height,
                    h_spacing,
                    v_spacing,
                    positions,
                );
                child_x += child_width + h_spacing;
            }
        }

        let total_width = calc_subtree_width(&tree.root, node_width, h_spacing);
        position_nodes(
            &tree.root,
            -total_width / 2.0,
            -200.0,
            total_width,
            node_width,
            node_height,
            h_spacing,
            v_spacing,
            &mut self.tree_positions,
        );
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let Some(ref tree) = self.tree else {
            ui.centered_and_justified(|ui| {
                ui.label("Select a CBU to visualize");
            });
            return;
        };

        // Clone data needed for drawing to avoid borrow issues
        let root = tree.root.clone();
        let overlay_edges = tree.overlay_edges.clone();
        let cbu_name = tree.cbu_name.clone();
        let stats = tree.stats.clone();

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

        // Copy values for transform closure
        let offset = self.offset;
        let zoom = self.zoom;

        // Transform helper
        let transform = |pos: Pos2| -> Pos2 { center + (pos.to_vec2() + offset) * zoom };

        // Draw tree edges (parent to children)
        self.draw_tree_edges(&root, &painter, &transform);

        // Draw overlay edges (ownership, control relationships)
        for edge in &overlay_edges {
            let Some(from_pos) = self.tree_positions.get(&edge.from) else {
                continue;
            };
            let Some(to_pos) = self.tree_positions.get(&edge.to) else {
                continue;
            };

            let from = transform(*from_pos);
            let to = transform(*to_pos);

            let color = match edge.edge_type.as_str() {
                "owns" => Color32::from_rgb(34, 197, 94),
                "controls" => Color32::from_rgb(251, 191, 36),
                "role" => Color32::from_rgb(99, 102, 241),
                _ => Color32::from_rgb(150, 150, 150),
            };

            // Draw curved line for overlay edges
            let mid = Pos2::new(
                (from.x + to.x) / 2.0,
                (from.y + to.y) / 2.0 - 30.0 * self.zoom,
            );
            painter.line_segment([from, mid], Stroke::new(2.0 * self.zoom, color));
            painter.line_segment([mid, to], Stroke::new(2.0 * self.zoom, color));

            if let Some(ref label) = edge.label {
                painter.text(
                    mid,
                    egui::Align2::CENTER_BOTTOM,
                    label,
                    egui::FontId::proportional(10.0 * self.zoom),
                    color,
                );
            }
        }

        // Draw tree nodes
        self.draw_tree_nodes(&root, &response, &painter, &transform);

        // Draw stats in corner
        let stats_text = format!(
            "{} | {} entities | {} persons",
            cbu_name, stats.entity_count, stats.person_count
        );
        painter.text(
            rect.left_top() + Vec2::new(10.0, 20.0),
            egui::Align2::LEFT_TOP,
            stats_text,
            egui::FontId::proportional(12.0),
            Color32::from_rgb(180, 180, 180),
        );

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

    fn draw_tree_edges<F>(&self, node: &TreeNode, painter: &egui::Painter, transform: &F)
    where
        F: Fn(Pos2) -> Pos2,
    {
        let Some(parent_pos) = self.tree_positions.get(&node.id) else {
            return;
        };

        let parent_screen = transform(*parent_pos);

        for child in &node.children {
            let Some(child_pos) = self.tree_positions.get(&child.id) else {
                continue;
            };

            let child_screen = transform(*child_pos);

            let color = Color32::from_rgb(100, 100, 120);
            painter.line_segment(
                [parent_screen, child_screen],
                Stroke::new(1.5 * self.zoom, color),
            );

            self.draw_tree_edges(child, painter, transform);
        }
    }

    fn draw_tree_nodes<F>(
        &mut self,
        node: &TreeNode,
        response: &egui::Response,
        painter: &egui::Painter,
        transform: &F,
    ) where
        F: Fn(Pos2) -> Pos2,
    {
        let Some(pos) = self.tree_positions.get(&node.id) else {
            return;
        };

        let screen_pos = transform(*pos);
        let (bg_color, border_color) = tree_node_colors(&node.node_type);

        let node_size = Vec2::new(150.0, 55.0) * self.zoom;
        let node_rect = Rect::from_center_size(screen_pos, node_size);

        painter.rect_filled(node_rect, 8.0 * self.zoom, bg_color);
        painter.rect_stroke(
            node_rect,
            8.0 * self.zoom,
            Stroke::new(2.0 * self.zoom, border_color),
        );

        painter.text(
            screen_pos - Vec2::new(0.0, 10.0 * self.zoom),
            egui::Align2::CENTER_CENTER,
            &node.label,
            egui::FontId::proportional(11.0 * self.zoom),
            Color32::WHITE,
        );

        if let Some(ref sublabel) = node.sublabel {
            painter.text(
                screen_pos + Vec2::new(0.0, 8.0 * self.zoom),
                egui::Align2::CENTER_CENTER,
                sublabel,
                egui::FontId::proportional(9.0 * self.zoom),
                Color32::from_rgb(180, 180, 180),
            );
        }

        let type_label = match node.node_type.as_str() {
            "cbu" => "CBU",
            "commercial_client" => "Client",
            "man_co" => "ManCo",
            "fund_entity" => "Fund",
            "trust_entity" => "Trust",
            "person" => "Person",
            "share_class" => "Share",
            _ => "",
        };
        if !type_label.is_empty() {
            painter.text(
                Pos2::new(
                    node_rect.right() - 5.0 * self.zoom,
                    node_rect.top() + 5.0 * self.zoom,
                ),
                egui::Align2::RIGHT_TOP,
                type_label,
                egui::FontId::proportional(8.0 * self.zoom),
                Color32::from_rgb(150, 150, 150),
            );
        }

        if response.clicked() {
            if let Some(pointer_pos) = response.interact_pointer_pos() {
                if node_rect.contains(pointer_pos) {
                    self.selected_node = Some(node.id.to_string());
                }
            }
        }

        for child in &node.children {
            self.draw_tree_nodes(child, response, painter, transform);
        }
    }
}

fn tree_node_colors(node_type: &str) -> (Color32, Color32) {
    match node_type {
        "cbu" => (
            Color32::from_rgb(75, 85, 99),
            Color32::from_rgb(156, 163, 175),
        ),
        "commercial_client" => (
            Color32::from_rgb(30, 64, 175),
            Color32::from_rgb(96, 165, 250),
        ),
        "man_co" => (
            Color32::from_rgb(124, 45, 18),
            Color32::from_rgb(251, 146, 60),
        ),
        "fund_entity" => (
            Color32::from_rgb(21, 128, 61),
            Color32::from_rgb(74, 222, 128),
        ),
        "trust_entity" => (
            Color32::from_rgb(88, 28, 135),
            Color32::from_rgb(192, 132, 252),
        ),
        "person" => (
            Color32::from_rgb(14, 116, 144),
            Color32::from_rgb(34, 211, 238),
        ),
        "share_class" => (
            Color32::from_rgb(161, 98, 7),
            Color32::from_rgb(250, 204, 21),
        ),
        _ => (
            Color32::from_rgb(55, 65, 81),
            Color32::from_rgb(156, 163, 175),
        ),
    }
}
