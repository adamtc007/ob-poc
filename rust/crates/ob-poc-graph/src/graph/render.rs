//! Rendering - draws nodes, edges, and overlays using egui::Painter
//!
//! All rendering uses world coordinates transformed by the camera.
//! Uses LOD system for performance and bezier curves for edges.

use super::camera::Camera2D;
use super::colors::edge_color;
use super::edges::{
    curve_strength_for_edge, parallel_edge_offset, render_arrow_head, render_bezier_edge,
    render_edge_label, should_show_edge_label, EdgeCurve,
};
use super::lod::{render_node_at_lod, DetailLevel};
use super::types::*;
use egui::{Color32, FontId, Pos2, Rect, Stroke, Vec2};
use std::collections::HashMap;

// =============================================================================
// RENDER CONSTANTS
// =============================================================================

const CORNER_RADIUS: f32 = 8.0;

// =============================================================================
// GRAPH RENDERER
// =============================================================================

pub struct GraphRenderer {
    /// Opacity for non-focused elements (0.0-1.0)
    pub blur_opacity: f32,
    /// Whether to use bezier curves for edges
    pub use_bezier_edges: bool,
    /// Whether to use LOD system for nodes
    pub use_lod: bool,
}

impl Default for GraphRenderer {
    fn default() -> Self {
        Self {
            blur_opacity: 0.25,
            use_bezier_edges: true,
            use_lod: true,
        }
    }
}

impl GraphRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Render the complete graph
    pub fn render(
        &self,
        painter: &egui::Painter,
        graph: &LayoutGraph,
        camera: &Camera2D,
        screen_rect: Rect,
        focused_node: Option<&str>,
    ) {
        // Build parallel edge index: for each (source, target) pair, track edge indices
        // This allows us to offset edges that share the same endpoints
        let parallel_info = self.compute_parallel_edge_info(&graph.edges);

        // Render edges first (below nodes)
        for (i, edge) in graph.edges.iter().enumerate() {
            let in_focus = self.is_edge_in_focus(edge, focused_node, graph);
            let (edge_index, total_parallel) = parallel_info.get(&i).copied().unwrap_or((0, 1));
            self.render_edge_with_offset(
                painter,
                edge,
                graph,
                camera,
                screen_rect,
                in_focus,
                edge_index,
                total_parallel,
            );
        }

        // Render nodes
        for node in graph.nodes.values() {
            let in_focus = self.is_node_in_focus(&node.id, focused_node, graph);
            let is_focused = focused_node == Some(node.id.as_str());
            self.render_node(painter, node, camera, screen_rect, in_focus, is_focused);
        }

        // Render investor groups (collapsed)
        for group in &graph.investor_groups {
            self.render_investor_group(painter, group, camera, screen_rect);
        }
    }

    /// Render a single node
    fn render_node(
        &self,
        painter: &egui::Painter,
        node: &LayoutNode,
        camera: &Camera2D,
        screen_rect: Rect,
        in_focus: bool,
        is_focused: bool,
    ) {
        // Transform to screen coordinates
        let screen_pos = camera.world_to_screen(node.position, screen_rect);
        let screen_size = node.size * camera.zoom;

        // Check if visible
        let node_rect = Rect::from_center_size(screen_pos, screen_size);
        if !screen_rect.intersects(node_rect) {
            return;
        }

        // Apply focus opacity
        let opacity = if in_focus { 1.0 } else { self.blur_opacity };

        if self.use_lod {
            // Use LOD system
            let lod = DetailLevel::from_screen_size(screen_size.x, is_focused);
            render_node_at_lod(painter, node, screen_pos, screen_size, lod, opacity);
        } else {
            // Legacy rendering
            self.render_node_legacy(painter, node, screen_pos, screen_size, opacity);
        }
    }

    /// Legacy node rendering (for reference/fallback)
    fn render_node_legacy(
        &self,
        painter: &egui::Painter,
        node: &LayoutNode,
        screen_pos: Pos2,
        screen_size: Vec2,
        opacity: f32,
    ) {
        let node_rect = Rect::from_center_size(screen_pos, screen_size);

        // Draw node background
        let fill = apply_opacity(node.style.fill_color, opacity);
        let border = apply_opacity(node.style.border_color, opacity);
        let text_color = apply_opacity(node.style.text_color, opacity);

        let corner_radius = CORNER_RADIUS * (screen_size.x / node.size.x);
        let border_width = node.style.border_width * (screen_size.x / node.size.x);

        painter.rect_filled(node_rect, corner_radius, fill);
        painter.rect_stroke(node_rect, corner_radius, Stroke::new(border_width, border));

        // Draw label
        let font_size = 12.0 * (screen_size.x / node.size.x);
        painter.text(
            screen_pos,
            egui::Align2::CENTER_CENTER,
            &node.label,
            FontId::proportional(font_size),
            text_color,
        );
    }

    /// Compute parallel edge info: maps edge index to (index_among_parallels, total_parallels)
    ///
    /// When multiple edges connect the same source/target pair, they need different
    /// curve offsets to be visually distinguishable.
    fn compute_parallel_edge_info(&self, edges: &[LayoutEdge]) -> HashMap<usize, (usize, usize)> {
        // Group edges by (source, target) pair - order matters, so we normalize
        let mut edge_groups: HashMap<(String, String), Vec<usize>> = HashMap::new();

        for (i, edge) in edges.iter().enumerate() {
            // Use ordered pair so A->B and B->A are treated as same pair
            let key = if edge.source_id <= edge.target_id {
                (edge.source_id.clone(), edge.target_id.clone())
            } else {
                (edge.target_id.clone(), edge.source_id.clone())
            };
            edge_groups.entry(key).or_default().push(i);
        }

        // Build result map: edge_index -> (index_in_group, group_size)
        let mut result = HashMap::new();
        for (_key, indices) in edge_groups {
            let total = indices.len();
            for (group_idx, edge_idx) in indices.into_iter().enumerate() {
                result.insert(edge_idx, (group_idx, total));
            }
        }

        result
    }

    /// Render a single edge with offset for parallel edges
    fn render_edge_with_offset(
        &self,
        painter: &egui::Painter,
        edge: &LayoutEdge,
        graph: &LayoutGraph,
        camera: &Camera2D,
        screen_rect: Rect,
        in_focus: bool,
        edge_index: usize,
        total_parallel: usize,
    ) {
        let Some(source_node) = graph.get_node(&edge.source_id) else {
            return;
        };
        let Some(target_node) = graph.get_node(&edge.target_id) else {
            return;
        };

        // Transform to screen coordinates
        let source_pos = camera.world_to_screen(source_node.position, screen_rect);
        let target_pos = camera.world_to_screen(target_node.position, screen_rect);

        // Calculate edge attachment points (bottom of source, top of target)
        let source_bottom = source_pos + Vec2::new(0.0, source_node.size.y * camera.zoom / 2.0);
        let target_top = target_pos - Vec2::new(0.0, target_node.size.y * camera.zoom / 2.0);

        // Apply focus opacity
        let opacity = if in_focus { 1.0 } else { self.blur_opacity };
        let base_color = edge_color(edge.edge_type);
        let color = apply_opacity(base_color, opacity);
        let width = edge.style.width * camera.zoom;

        if self.use_bezier_edges {
            // Use bezier curves with offset for parallel edges
            let base_strength = curve_strength_for_edge(edge.edge_type, None);
            let curve_strength = parallel_edge_offset(edge_index, total_parallel, base_strength);
            let curve = EdgeCurve::new(source_bottom, target_top, curve_strength);

            // Render curve
            render_bezier_edge(
                painter,
                &curve,
                Stroke::new(width, color),
                edge.style.dashed,
            );

            // Render arrow at end
            let direction = curve.end_direction();
            render_arrow_head(painter, target_top, direction, camera.zoom, color);

            // Render label if present
            if should_show_edge_label(edge.label.is_some(), camera.zoom) {
                if let Some(ref label) = edge.label {
                    render_edge_label(
                        painter,
                        curve.midpoint(),
                        label,
                        camera.zoom,
                        Color32::WHITE,
                        apply_opacity(Color32::from_rgb(80, 80, 80), opacity),
                    );
                }
            }
        } else {
            // Legacy straight line rendering
            if edge.style.dashed {
                self.draw_dashed_line(painter, source_bottom, target_top, color, width);
            } else {
                painter.line_segment([source_bottom, target_top], Stroke::new(width, color));
            }

            // Draw arrow at target
            let direction = (target_top - source_bottom).normalized();
            render_arrow_head(painter, target_top, direction, camera.zoom, color);

            // Draw label if present
            if let Some(ref label) = edge.label {
                let mid = Pos2::new(
                    (source_bottom.x + target_top.x) / 2.0,
                    (source_bottom.y + target_top.y) / 2.0,
                );
                let label_color = apply_opacity(Color32::from_rgb(180, 180, 180), opacity);
                painter.text(
                    mid + Vec2::new(8.0 * camera.zoom, 0.0),
                    egui::Align2::LEFT_CENTER,
                    label,
                    FontId::proportional(9.0 * camera.zoom),
                    label_color,
                );
            }
        }
    }

    /// Draw a dashed line (legacy)
    fn draw_dashed_line(
        &self,
        painter: &egui::Painter,
        from: Pos2,
        to: Pos2,
        color: Color32,
        width: f32,
    ) {
        let dir = (to - from).normalized();
        let length = (to - from).length();
        let dash_len = 8.0;
        let gap_len = 4.0;
        let segment_len = dash_len + gap_len;

        let mut dist = 0.0;
        while dist < length {
            let start = from + dir * dist;
            let end_dist = (dist + dash_len).min(length);
            let end = from + dir * end_dist;
            painter.line_segment([start, end], Stroke::new(width, color));
            dist += segment_len;
        }
    }

    /// Render collapsed investor group
    fn render_investor_group(
        &self,
        painter: &egui::Painter,
        group: &InvestorGroup,
        camera: &Camera2D,
        screen_rect: Rect,
    ) {
        let screen_pos = camera.world_to_screen(group.position, screen_rect);
        let size = Vec2::new(140.0, 50.0) * camera.zoom;
        let rect = Rect::from_center_size(screen_pos, size);

        if !screen_rect.intersects(rect) {
            return;
        }

        // Draw collapsed group box
        let fill = Color32::from_rgb(55, 48, 23);
        let border = Color32::from_rgb(161, 98, 7);
        let corner_radius = CORNER_RADIUS * camera.zoom;

        painter.rect_filled(rect, corner_radius, fill);
        painter.rect_stroke(rect, corner_radius, Stroke::new(2.0 * camera.zoom, border));

        // Draw investor count
        let text = format!("{} Investors", group.investor_count);
        painter.text(
            screen_pos,
            egui::Align2::CENTER_CENTER,
            text,
            FontId::proportional(11.0 * camera.zoom),
            Color32::WHITE,
        );

        // Draw expand hint if zoomed in enough
        if camera.zoom > 0.7 {
            let hint_pos = Pos2::new(rect.right() - 8.0 * camera.zoom, rect.center().y);
            painter.text(
                hint_pos,
                egui::Align2::RIGHT_CENTER,
                "+",
                FontId::proportional(14.0 * camera.zoom),
                Color32::from_rgb(200, 200, 200),
            );
        }
    }

    /// Check if node is in focus (connected to focused node or is the focused node)
    fn is_node_in_focus(
        &self,
        node_id: &str,
        focused_node: Option<&str>,
        graph: &LayoutGraph,
    ) -> bool {
        let Some(focus_id) = focused_node else {
            return true; // No focus = everything in focus
        };

        if node_id == focus_id {
            return true;
        }

        // Check if connected to focused node
        for edge in &graph.edges {
            if (edge.source_id == focus_id && edge.target_id == node_id)
                || (edge.target_id == focus_id && edge.source_id == node_id)
            {
                return true;
            }
        }

        false
    }

    /// Check if edge is in focus
    fn is_edge_in_focus(
        &self,
        edge: &LayoutEdge,
        focused_node: Option<&str>,
        _graph: &LayoutGraph,
    ) -> bool {
        let Some(focus_id) = focused_node else {
            return true;
        };

        edge.source_id == focus_id || edge.target_id == focus_id
    }
}

// =============================================================================
// HELPERS
// =============================================================================

fn apply_opacity(color: Color32, opacity: f32) -> Color32 {
    let [r, g, b, a] = color.to_array();
    Color32::from_rgba_unmultiplied(r, g, b, (a as f32 * opacity) as u8)
}
