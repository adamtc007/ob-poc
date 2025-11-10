//! AST Viewer Panel
//!
//! This module implements the AST (Abstract Syntax Tree) viewer panel for the DSL visualizer.
//! It provides interactive tree/graph visualization of parsed DSL structures with multiple
//! layout modes, zooming, panning, and node inspection capabilities.

use super::{
    constants::*,
    models::{ASTNode, ASTNodeType},
    VisualizerResult,
};
use eframe::egui;
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// AST Viewer Panel state and functionality
pub struct ASTViewerPanel {
    /// Current AST root node
    ast_root: Option<ASTNode>,

    /// Currently selected node ID
    selected_node_id: Option<String>,

    /// Layout mode for visualization
    layout_mode: LayoutMode,

    /// Node positions for graph layout (node_id -> (x, y))
    node_positions: HashMap<String, (f32, f32)>,

    /// Zoom level
    zoom_level: f32,

    /// Pan offset
    pan_offset: (f32, f32),

    /// Show node details panel
    show_details: bool,

    /// Show node IDs on nodes
    show_node_ids: bool,

    /// Show node types
    show_node_types: bool,

    /// Expand all nodes by default
    expand_all: bool,

    /// Collapsed nodes (for tree view)
    collapsed_nodes: std::collections::HashSet<String>,

    /// UI state
    mouse_dragging: bool,
    last_mouse_pos: Option<egui::Pos2>,

    /// Performance metrics
    render_time_ms: f32,
    node_count: usize,
}

/// Available layout modes for AST visualization
#[derive(Debug, Clone, PartialEq)]
pub enum LayoutMode {
    /// Traditional tree layout (top-down)
    Tree,
    /// Force-directed graph layout
    Graph,
    /// Compact horizontal layout
    Compact,
}

impl ASTViewerPanel {
    /// Create a new AST viewer panel
    pub fn new() -> Self {
        Self {
            ast_root: None,
            selected_node_id: None,
            layout_mode: LayoutMode::Tree,
            node_positions: HashMap::new(),
            zoom_level: 1.0,
            pan_offset: (0.0, 0.0),
            show_details: true,
            show_node_ids: false,
            show_node_types: true,
            expand_all: true,
            collapsed_nodes: std::collections::HashSet::new(),
            mouse_dragging: false,
            last_mouse_pos: None,
            render_time_ms: 0.0,
            node_count: 0,
        }
    }

    /// Update the AST to display
    pub fn update_ast(&mut self, ast: ASTNode) {
        info!("Updating AST visualization");
        self.ast_root = Some(ast);
        self.selected_node_id = None;
        self.calculate_layout();
    }

    /// Check if AST is loaded
    pub fn has_ast(&self) -> bool {
        self.ast_root.is_some()
    }

    /// Get current layout mode
    pub fn get_layout_mode(&self) -> &LayoutMode {
        &self.layout_mode
    }

    /// Set layout mode and recalculate positions
    pub fn set_layout_mode(&mut self, mode: LayoutMode) {
        if self.layout_mode != mode {
            self.layout_mode = mode;
            self.calculate_layout();
        }
    }

    /// Calculate node positions based on current layout mode
    fn calculate_layout(&mut self) {
        if let Some(root) = self.ast_root.clone() {
            self.node_positions.clear();
            self.node_count = self.count_nodes(&root);

            match self.layout_mode {
                LayoutMode::Tree => self.calculate_tree_layout(&root, 0.0, 0.0, 0),
                LayoutMode::Graph => self.calculate_graph_layout(&root),
                LayoutMode::Compact => self.calculate_compact_layout(&root),
            }

            debug!(
                "Calculated layout for {} nodes in {:?} mode",
                self.node_positions.len(),
                self.layout_mode
            );
        }
    }

    /// Calculate tree layout positions
    fn calculate_tree_layout(&mut self, node: &ASTNode, x: f32, y: f32, level: usize) {
        let node_spacing_x = 120.0 * self.zoom_level;
        let level_spacing_y = 80.0 * self.zoom_level;

        // Position current node
        self.node_positions.insert(node.id.clone(), (x, y));

        // Position children
        if !node.children.is_empty() && !self.collapsed_nodes.contains(&node.id) {
            let child_count = node.children.len();
            let total_width = (child_count as f32 - 1.0) * node_spacing_x;
            let start_x = x - total_width / 2.0;

            for (i, child) in node.children.iter().enumerate() {
                let child_x = start_x + (i as f32 * node_spacing_x);
                let child_y = y + level_spacing_y;
                self.calculate_tree_layout(child, child_x, child_y, level + 1);
            }
        }
    }

    /// Calculate force-directed graph layout
    fn calculate_graph_layout(&mut self, root: &ASTNode) {
        // Simple spring-based layout algorithm
        let mut nodes = Vec::new();
        self.collect_all_nodes(root, &mut nodes);

        // Initialize random positions if not set
        for node in &nodes {
            if !self.node_positions.contains_key(&node.id) {
                let angle = (nodes.len() as f32 * std::f32::consts::PI * 2.0) / nodes.len() as f32;
                let radius = 100.0 * self.zoom_level;
                let x = angle.cos() * radius;
                let y = angle.sin() * radius;
                self.node_positions.insert(node.id.clone(), (x, y));
            }
        }

        // Simple force-directed layout (simplified)
        for _iteration in 0..10 {
            let mut forces = HashMap::new();

            // Calculate repulsive forces between all nodes
            for node1 in &nodes {
                let mut total_force = (0.0, 0.0);
                if let Some(&(x1, y1)) = self.node_positions.get(&node1.id) {
                    for node2 in &nodes {
                        if node1.id != node2.id {
                            if let Some(&(x2, y2)) = self.node_positions.get(&node2.id) {
                                let dx = x1 - x2;
                                let dy = y1 - y2;
                                let distance = (dx * dx + dy * dy).sqrt().max(1.0);
                                let force_magnitude = 1000.0 / (distance * distance);
                                total_force.0 += (dx / distance) * force_magnitude;
                                total_force.1 += (dy / distance) * force_magnitude;
                            }
                        }
                    }

                    // Add attractive forces to parent
                    for child in &node1.children {
                        if let Some(&(cx, cy)) = self.node_positions.get(&child.id) {
                            let dx = cx - x1;
                            let dy = cy - y1;
                            let distance = (dx * dx + dy * dy).sqrt().max(1.0);
                            let force_magnitude = distance * 0.01;
                            total_force.0 += (dx / distance) * force_magnitude;
                            total_force.1 += (dy / distance) * force_magnitude;
                        }
                    }
                }

                forces.insert(node1.id.clone(), total_force);
            }

            // Apply forces
            for node in &nodes {
                if let (Some(&(x, y)), Some(&(fx, fy))) =
                    (self.node_positions.get(&node.id), forces.get(&node.id))
                {
                    let damping = 0.1;
                    let new_x = x + fx * damping;
                    let new_y = y + fy * damping;
                    self.node_positions.insert(node.id.clone(), (new_x, new_y));
                }
            }
        }
    }

    /// Calculate compact horizontal layout
    fn calculate_compact_layout(&mut self, root: &ASTNode) {
        let mut y_offset = 0.0;
        let line_height = 30.0 * self.zoom_level;
        self.calculate_compact_layout_recursive(root, 0.0, &mut y_offset, line_height, 0);
    }

    fn calculate_compact_layout_recursive(
        &mut self,
        node: &ASTNode,
        x: f32,
        y_offset: &mut f32,
        line_height: f32,
        level: usize,
    ) {
        let indent = (level as f32 * 20.0) * self.zoom_level;
        self.node_positions
            .insert(node.id.clone(), (x + indent, *y_offset));
        *y_offset += line_height;

        if !self.collapsed_nodes.contains(&node.id) {
            for child in &node.children {
                self.calculate_compact_layout_recursive(child, x, y_offset, line_height, level + 1);
            }
        }
    }

    /// Collect all nodes in the AST
    fn collect_all_nodes(&self, node: &ASTNode, nodes: &mut Vec<ASTNode>) {
        nodes.push(node.clone());
        for child in &node.children {
            self.collect_all_nodes(child, nodes);
        }
    }

    /// Count total nodes in AST
    fn count_nodes(&self, node: &ASTNode) -> usize {
        1 + node
            .children
            .iter()
            .map(|c| self.count_nodes(c))
            .sum::<usize>()
    }

    /// Handle mouse interaction with the AST view
    fn handle_mouse_interaction(&mut self, ui: &mut egui::Ui, response: &egui::Response) {
        // Handle panning
        if response.dragged_by(egui::PointerButton::Middle) {
            if let Some(last_pos) = self.last_mouse_pos {
                let delta = response.interact_pointer_pos().unwrap_or_default() - last_pos;
                self.pan_offset.0 += delta.x;
                self.pan_offset.1 += delta.y;
            }
        }

        // Handle zooming
        if response.hovered() {
            let scroll_delta = ui.input(|i| i.raw_scroll_delta);
            if scroll_delta.y != 0.0 {
                let zoom_factor = 1.0 + scroll_delta.y * 0.001;
                self.zoom_level = (self.zoom_level * zoom_factor).clamp(0.1, 5.0);
                self.calculate_layout(); // Recalculate positions with new zoom
            }
        }

        // Handle node selection
        if response.clicked_by(egui::PointerButton::Primary) {
            if let Some(mouse_pos) = response.interact_pointer_pos() {
                let adjusted_pos = egui::pos2(
                    mouse_pos.x - self.pan_offset.0,
                    mouse_pos.y - self.pan_offset.1,
                );
                self.selected_node_id = self.find_node_at_position(adjusted_pos);
            }
        }

        self.last_mouse_pos = response.interact_pointer_pos();
    }

    /// Find which node is at the given position
    fn find_node_at_position(&self, pos: egui::Pos2) -> Option<String> {
        let node_size = 60.0 * self.zoom_level;

        for (node_id, &(node_x, node_y)) in &self.node_positions {
            let node_rect = egui::Rect::from_center_size(
                egui::pos2(node_x, node_y),
                egui::vec2(node_size, node_size * 0.6),
            );

            if node_rect.contains(pos) {
                return Some(node_id.clone());
            }
        }
        None
    }

    /// Render a single AST node
    fn render_node_direct(&self, painter: &egui::Painter, node: &ASTNode, pos: egui::Pos2) {
        let is_selected = self.selected_node_id.as_ref() == Some(&node.id);
        let node_size = egui::vec2(80.0 * self.zoom_level, 40.0 * self.zoom_level);

        // Node background
        let node_rect = egui::Rect::from_center_size(pos, node_size);
        let node_color = match node.node_type {
            ASTNodeType::Verb => egui::Color32::from_rgb(100, 150, 255),
            ASTNodeType::Attribute => egui::Color32::from_rgb(255, 150, 100),
            ASTNodeType::Value => egui::Color32::from_rgb(150, 255, 100),
            ASTNodeType::List => egui::Color32::from_rgb(255, 255, 100),
            ASTNodeType::Root => egui::Color32::from_rgb(200, 200, 200),
        };

        let fill_color = if is_selected {
            node_color.gamma_multiply(1.3)
        } else {
            node_color
        };

        painter.rect_filled(node_rect, 4.0, fill_color);
        painter.rect_stroke(
            node_rect,
            4.0,
            egui::Stroke::new(
                if is_selected { 2.0 } else { 1.0 },
                if is_selected {
                    egui::Color32::WHITE
                } else {
                    egui::Color32::GRAY
                },
            ),
        );

        // Node text
        let font_size = (12.0 * self.zoom_level).max(8.0);
        let font_id = egui::FontId::proportional(font_size);

        // Main label
        let label = if self.show_node_types {
            format!("{}\n{:?}", node.label, node.node_type)
        } else {
            node.label.clone()
        };

        painter.text(
            pos,
            egui::Align2::CENTER_CENTER,
            &label,
            font_id.clone(),
            egui::Color32::BLACK,
        );

        // Node ID (if enabled)
        if self.show_node_ids && self.zoom_level > 0.5 {
            let id_pos = pos + egui::vec2(0.0, node_size.y / 2.0 + 8.0);
            painter.text(
                id_pos,
                egui::Align2::CENTER_TOP,
                &node.id,
                egui::FontId::monospace(8.0 * self.zoom_level),
                egui::Color32::GRAY,
            );
        }
    }

    /// Render connections between nodes
    fn render_connections(&self, painter: &egui::Painter, node: &ASTNode) {
        if let Some(&(parent_x, parent_y)) = self.node_positions.get(&node.id) {
            let parent_pos = egui::pos2(parent_x + self.pan_offset.0, parent_y + self.pan_offset.1);

            for child in &node.children {
                if let Some(&(child_x, child_y)) = self.node_positions.get(&child.id) {
                    let child_pos =
                        egui::pos2(child_x + self.pan_offset.0, child_y + self.pan_offset.1);

                    // Draw connection line
                    painter.line_segment(
                        [parent_pos, child_pos],
                        egui::Stroke::new(1.0 * self.zoom_level, egui::Color32::GRAY),
                    );

                    // Draw arrow head
                    if self.zoom_level > 0.3 {
                        let direction = (child_pos - parent_pos).normalized();
                        let arrow_size = 8.0 * self.zoom_level;
                        let arrow_base = child_pos - direction * (20.0 * self.zoom_level);
                        let arrow_left = arrow_base + direction.rot90() * arrow_size;
                        let arrow_right = arrow_base - direction.rot90() * arrow_size;

                        painter.add(egui::Shape::convex_polygon(
                            vec![child_pos, arrow_left, arrow_right],
                            egui::Color32::GRAY,
                            egui::Stroke::NONE,
                        ));
                    }
                }

                // Recursively render child connections
                self.render_connections(painter, child);
            }
        }
    }

    /// Render node details panel
    fn render_details_panel(&mut self, ui: &mut egui::Ui) {
        if !self.show_details {
            return;
        }

        let selected_node =
            if let (Some(ref node_id), Some(ref root)) = (&self.selected_node_id, &self.ast_root) {
                self.find_node_by_id(root, node_id)
            } else {
                None
            };

        egui::SidePanel::right("node_details")
            .default_width(250.0)
            .resizable(true)
            .show_inside(ui, |ui| {
                ui.heading("Node Details");
                ui.separator();

                if let Some(node) = selected_node {
                    ui.label(format!("ID: {}", node.id));
                    ui.label(format!("Type: {:?}", node.node_type));
                    ui.label(format!("Label: {}", node.label));
                    ui.label(format!("Children: {}", node.children.len()));

                    if !node.properties.is_empty() {
                        ui.separator();
                        ui.label("Properties:");
                        for (key, value) in &node.properties {
                            ui.label(format!("  {}: {}", key, value));
                        }
                    }

                    if !node.children.is_empty() {
                        ui.separator();
                        ui.label("Children:");
                        for child in &node.children {
                            ui.label(format!("  • {} ({})", child.label, child.id));
                        }
                    }
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.label("Select a node to view details");
                    });
                }
            });
    }

    /// Find node by ID in the AST
    fn find_node_by_id<'a>(&self, node: &'a ASTNode, target_id: &str) -> Option<&'a ASTNode> {
        if node.id == target_id {
            return Some(node);
        }

        for child in &node.children {
            if let Some(found) = self.find_node_by_id(child, target_id) {
                return Some(found);
            }
        }

        None
    }

    /// Render control panel
    fn render_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Layout mode selector
            egui::ComboBox::from_label("Layout")
                .selected_text(format!("{:?}", self.layout_mode))
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_value(&mut self.layout_mode, LayoutMode::Tree, "Tree")
                        .clicked()
                    {
                        self.calculate_layout();
                    }
                    if ui
                        .selectable_value(&mut self.layout_mode, LayoutMode::Graph, "Graph")
                        .clicked()
                    {
                        self.calculate_layout();
                    }
                    if ui
                        .selectable_value(&mut self.layout_mode, LayoutMode::Compact, "Compact")
                        .clicked()
                    {
                        self.calculate_layout();
                    }
                });

            ui.separator();

            // Zoom controls
            if ui.button("🔍+").clicked() {
                self.zoom_level = (self.zoom_level * 1.2).min(5.0);
                self.calculate_layout();
            }
            if ui.button("🔍-").clicked() {
                self.zoom_level = (self.zoom_level / 1.2).max(0.1);
                self.calculate_layout();
            }
            if ui.button("🎯").clicked() {
                self.zoom_level = 1.0;
                self.pan_offset = (0.0, 0.0);
                self.calculate_layout();
            }

            ui.separator();

            // Display options
            ui.checkbox(&mut self.show_node_ids, "IDs");
            ui.checkbox(&mut self.show_node_types, "Types");
            ui.checkbox(&mut self.show_details, "Details");
        });

        // Status information
        ui.horizontal(|ui| {
            ui.label(format!("Zoom: {:.1}x", self.zoom_level));
            ui.label(format!("Nodes: {}", self.node_count));
            if self.render_time_ms > 0.0 {
                ui.label(format!("Render: {:.1}ms", self.render_time_ms));
            }
        });
    }

    /// Main render function for the AST viewer panel
    pub fn render(&mut self, ui: &mut egui::Ui) {
        let start_time = std::time::Instant::now();

        if self.ast_root.is_none() {
            ui.centered_and_justified(|ui| {
                ui.label(
                    "No AST to display.\nSelect a DSL entry from the browser to visualize its AST.",
                );
            });
            return;
        }

        ui.vertical(|ui| {
            // Control panel
            self.render_controls(ui);
            ui.separator();

            // Main visualization area
            let available_rect = ui.available_rect_before_wrap();
            let (response, painter) =
                ui.allocate_painter(available_rect.size(), egui::Sense::click_and_drag());

            // Handle mouse interaction
            self.handle_mouse_interaction(ui, &response);

            // Render the AST
            if let Some(ref root) = self.ast_root {
                // First pass: render connections
                self.render_connections(&painter, root);

                // Second pass: render nodes
                self.render_nodes_recursive(&painter, root);
            }
        });

        // Render details panel
        self.render_details_panel(ui);

        // Update performance metrics
        self.render_time_ms = start_time.elapsed().as_millis() as f32;
    }

    /// Recursively render all nodes
    fn render_nodes_recursive(&self, painter: &egui::Painter, node: &ASTNode) {
        if let Some(&(x, y)) = self.node_positions.get(&node.id) {
            let screen_pos = egui::pos2(x + self.pan_offset.0, y + self.pan_offset.1);

            // Only render if visible (simple culling)
            let node_size = egui::vec2(80.0 * self.zoom_level, 40.0 * self.zoom_level);
            let node_rect = egui::Rect::from_center_size(screen_pos, node_size);

            // Simple visibility check (can be improved)
            if node_rect.intersects(painter.clip_rect()) {
                self.render_node_direct(painter, node, screen_pos);
            }
        }

        // Render children
        if !self.collapsed_nodes.contains(&node.id) {
            for child in &node.children {
                self.render_nodes_recursive(painter, child);
            }
        }
    }
}

impl Default for ASTViewerPanel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_node(id: &str, label: &str, node_type: ASTNodeType) -> ASTNode {
        ASTNode {
            id: id.to_string(),
            label: label.to_string(),
            node_type,
            children: Vec::new(),
            properties: HashMap::new(),
            position: None,
        }
    }

    #[test]
    fn test_ast_viewer_creation() {
        let viewer = ASTViewerPanel::new();
        assert!(!viewer.has_ast());
        assert_eq!(viewer.get_layout_mode(), &LayoutMode::Tree);
        assert_eq!(viewer.zoom_level, 1.0);
    }

    #[test]
    fn test_ast_update() {
        let mut viewer = ASTViewerPanel::new();
        let ast = create_test_node("root", "root", ASTNodeType::Root);

        viewer.update_ast(ast);
        assert!(viewer.has_ast());
        assert_eq!(viewer.node_count, 1);
    }

    #[test]
    fn test_layout_mode_change() {
        let mut viewer = ASTViewerPanel::new();
        let ast = create_test_node("root", "root", ASTNodeType::Root);
        viewer.update_ast(ast);

        viewer.set_layout_mode(LayoutMode::Graph);
        assert_eq!(viewer.get_layout_mode(), &LayoutMode::Graph);

        viewer.set_layout_mode(LayoutMode::Compact);
        assert_eq!(viewer.get_layout_mode(), &LayoutMode::Compact);
    }

    #[test]
    fn test_node_counting() {
        let viewer = ASTViewerPanel::new();
        let mut root = create_test_node("root", "root", ASTNodeType::Root);
        root.children
            .push(create_test_node("child1", "child1", ASTNodeType::Verb));
        root.children
            .push(create_test_node("child2", "child2", ASTNodeType::Attribute));

        assert_eq!(viewer.count_nodes(&root), 3);
    }

    #[test]
    fn test_find_node_by_id() {
        let viewer = ASTViewerPanel::new();
        let mut root = create_test_node("root", "root", ASTNodeType::Root);
        let child = create_test_node("child1", "child1", ASTNodeType::Verb);
        root.children.push(child);

        assert!(viewer.find_node_by_id(&root, "root").is_some());
        assert!(viewer.find_node_by_id(&root, "child1").is_some());
        assert!(viewer.find_node_by_id(&root, "nonexistent").is_none());
    }
}
