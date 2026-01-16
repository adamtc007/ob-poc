//! Rendering - draws nodes, edges, and overlays using egui::Painter
//!
//! All rendering uses world coordinates transformed by the camera.
//! Uses LOD system for performance and bezier curves for edges.

use super::camera::Camera2D;
use super::colors::{edge_color, edge_width_for_weight, verification_edge_style};
use super::edges::{
    curve_strength_for_edge, parallel_edge_offset, render_arrow_head, render_bezier_edge,
    render_edge_label, should_show_edge_label, EdgeCurve,
};
use super::lod::{render_node_at_lod, DetailLevel};
use super::types::*;
use super::viewport::EsperRenderState;
use egui::{Color32, FontId, Pos2, Rect, Stroke, Vec2};
use std::collections::HashMap;

// =============================================================================
// RENDER CONSTANTS
// =============================================================================

const CORNER_RADIUS: f32 = 8.0;
const CONTAINER_PADDING: f32 = 40.0;
const CONTAINER_HEADER_HEIGHT: f32 = 36.0; // Taller header for larger CBU title font

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

/// Options controlling how the graph is rendered
///
/// Groups optional rendering parameters to reduce function argument count.
#[derive(Debug, Clone, Default)]
pub struct RenderOptions<'a> {
    /// Currently focused node (highlighted with connected edges)
    pub focused_node: Option<&'a str>,
    /// If set, only nodes matching this type are fully visible
    pub type_filter: Option<&'a str>,
    /// If set, nodes of this type get a highlight effect
    pub highlighted_type: Option<&'a str>,
    /// Esper render modes (xray, peel, shadow, etc.)
    pub esper_state: Option<&'a EsperRenderState>,
    /// Matrix focus path - when set, highlights trading-related nodes
    /// Path segments like ["Equities", "Listed"] trigger trading entity highlighting
    pub matrix_focus_path: Option<&'a [String]>,
}

impl GraphRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Render the complete graph
    ///
    /// # Arguments
    /// * `painter` - egui painter for drawing
    /// * `graph` - the layout graph to render
    /// * `camera` - camera for world-to-screen transformation
    /// * `screen_rect` - visible screen area
    /// * `opts` - optional render settings (focus, filters, esper state)
    pub fn render(
        &self,
        painter: &egui::Painter,
        graph: &LayoutGraph,
        camera: &Camera2D,
        screen_rect: Rect,
        opts: &RenderOptions<'_>,
    ) {
        // Extract options for use in rendering
        let focused_node = opts.focused_node;
        let type_filter = opts.type_filter;
        let highlighted_type = opts.highlighted_type;
        let esper_state = opts.esper_state;
        let matrix_focus_path = opts.matrix_focus_path;

        // Render container backgrounds first (below everything)
        self.render_containers(painter, graph, camera, screen_rect);

        // Build parallel edge index: for each (source, target) pair, track edge indices
        // This allows us to offset edges that share the same endpoints
        let parallel_info = self.compute_parallel_edge_info(&graph.edges);

        // Build map of node -> container for edge filtering
        let node_containers: std::collections::HashMap<&str, Option<&str>> = graph
            .nodes
            .iter()
            .map(|(id, n)| (id.as_str(), n.container_parent_id.as_deref()))
            .collect();

        // Render edges first (below nodes)
        // Skip edges where both endpoints are in the same container (redundant in container view)
        for (i, edge) in graph.edges.iter().enumerate() {
            // Check if both nodes are in the same container
            let source_container = node_containers
                .get(edge.source_id.as_str())
                .copied()
                .flatten();
            let target_container = node_containers
                .get(edge.target_id.as_str())
                .copied()
                .flatten();

            // Skip edge if both are in same container (or if edge goes to container itself)
            if let (Some(src_c), Some(tgt_c)) = (source_container, target_container) {
                if src_c == tgt_c {
                    continue; // Both in same container, skip this edge
                }
            }
            // Also skip edges from container to its children
            if source_container.is_some() && target_container.is_none() {
                if let Some(target_node) = graph.get_node(&edge.target_id) {
                    if target_node.is_cbu_root {
                        continue;
                    }
                }
            }
            if target_container.is_some() && source_container.is_none() {
                if let Some(source_node) = graph.get_node(&edge.source_id) {
                    if source_node.is_cbu_root {
                        continue;
                    }
                }
            }

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

        // Render attachment edges (CBU container to external taxonomies)
        self.render_attachment_edges(painter, graph, camera, screen_rect);

        // Render nodes (skip nodes that are rendered as containers)
        let container_ids: std::collections::HashSet<&str> = graph
            .nodes
            .values()
            .filter_map(|n| n.container_parent_id.as_deref())
            .collect();

        for node in graph.nodes.values() {
            // Skip CBU nodes that are being used as containers (they're drawn as backgrounds)
            if node.is_cbu_root && container_ids.contains(node.id.as_str()) {
                continue;
            }

            // Determine focus based on node focus AND type filter
            let node_in_focus = self.is_node_in_focus(&node.id, focused_node, graph);
            let is_focused = focused_node == Some(node.id.as_str());

            // Check type filter - if set, nodes not matching get reduced opacity
            let matches_type_filter = type_filter
                .map(|filter| self.node_matches_type(node, filter))
                .unwrap_or(true);

            // Check highlighted type - nodes matching get a highlight effect
            let is_highlighted = highlighted_type
                .map(|hl| self.node_matches_type(node, hl))
                .unwrap_or(false);

            // Check matrix focus - when matrix node is selected, highlight trading entities
            let is_matrix_highlighted = matrix_focus_path.is_some() && self.is_trading_entity(node);

            // Combine focus states: in_focus if (node focus AND type filter match)
            // If type filter is set but node doesn't match, treat as not in focus
            let in_focus = node_in_focus && matches_type_filter;

            // Combine highlight states
            let combined_highlight = is_highlighted || is_matrix_highlighted;

            self.render_node(
                painter,
                node,
                camera,
                screen_rect,
                in_focus,
                is_focused,
                combined_highlight,
                esper_state,
            );
        }

        // Render investor groups (collapsed)
        for group in &graph.investor_groups {
            self.render_investor_group(painter, group, camera, screen_rect);
        }
    }

    /// Render container backgrounds for nodes that have container_parent_id set
    fn render_containers(
        &self,
        painter: &egui::Painter,
        graph: &LayoutGraph,
        camera: &Camera2D,
        screen_rect: Rect,
    ) {
        // Group nodes by their container_parent_id
        let mut containers: HashMap<String, Vec<&LayoutNode>> = HashMap::new();

        for node in graph.nodes.values() {
            if let Some(ref parent_id) = node.container_parent_id {
                containers.entry(parent_id.clone()).or_default().push(node);
            }
        }

        // Render each container
        for (container_id, child_nodes) in containers {
            // Get the container node (CBU) for label and status
            let container_node = graph.get_node(&container_id);
            let container_label = container_node
                .map(|n| n.label.as_str())
                .unwrap_or("Container");
            let container_status = container_node.and_then(|n| n.status.as_deref());

            // Calculate bounding box of all child nodes
            if let Some(bounds) = self.compute_container_bounds(&child_nodes, camera, screen_rect) {
                self.render_container_background(
                    painter,
                    bounds,
                    container_label,
                    container_status,
                    camera,
                    screen_rect,
                );
            }
        }
    }

    /// Render attachment edges from CBU container to external taxonomy nodes
    ///
    /// These edges visually connect the container boundary to external nodes
    /// (TradingProfile, InstrumentMatrix above; Products below)
    fn render_attachment_edges(
        &self,
        painter: &egui::Painter,
        graph: &LayoutGraph,
        camera: &Camera2D,
        screen_rect: Rect,
    ) {
        use super::types::EntityType;

        // Find CBU container and its bounds
        let cbu_node = graph.nodes.values().find(|n| n.is_cbu_root);
        let Some(cbu_node) = cbu_node else {
            return;
        };
        let cbu_id = &cbu_node.id;

        // Get contained nodes to compute container bounds
        let contained_nodes: Vec<_> = graph
            .nodes
            .values()
            .filter(|n| n.container_parent_id.as_deref() == Some(cbu_id))
            .collect();

        let Some(container_bounds) =
            self.compute_container_bounds(&contained_nodes, camera, screen_rect)
        else {
            return;
        };

        // Find external taxonomy nodes to connect to
        let mut trading_profile: Option<&super::types::LayoutNode> = None;
        let mut instrument_matrix: Option<&super::types::LayoutNode> = None;
        let mut first_product: Option<&super::types::LayoutNode> = None;

        for node in graph.nodes.values() {
            // Skip nodes inside the container
            if node.container_parent_id.is_some() || node.is_cbu_root {
                continue;
            }

            match node.entity_type {
                EntityType::TradingProfile => trading_profile = Some(node),
                EntityType::InstrumentMatrix => instrument_matrix = Some(node),
                EntityType::Product => {
                    if first_product.is_none() {
                        first_product = Some(node);
                    }
                }
                _ => {}
            }
        }

        // Style for attachment edges
        let edge_color = Color32::from_rgb(100, 116, 139); // slate-500
        let edge_width = 1.5 * camera.zoom();
        let connector_radius = 4.0 * camera.zoom();

        // Draw attachment edge to TradingProfile (below container)
        if let Some(profile) = trading_profile {
            let profile_screen_pos = camera.world_to_screen(profile.position, screen_rect);
            let profile_size = profile.size * camera.zoom();

            // From container bottom center to profile top
            let start = Pos2::new(container_bounds.center().x, container_bounds.max.y);
            let end = Pos2::new(
                profile_screen_pos.x,
                profile_screen_pos.y - profile_size.y / 2.0,
            );

            self.render_attachment_edge_line(
                painter,
                start,
                end,
                edge_color,
                edge_width,
                connector_radius,
            );
        }
        // If no trading profile, connect directly to instrument matrix
        else if let Some(matrix) = instrument_matrix {
            let matrix_screen_pos = camera.world_to_screen(matrix.position, screen_rect);
            let matrix_size = matrix.size * camera.zoom();

            let start = Pos2::new(container_bounds.center().x, container_bounds.max.y);
            let end = Pos2::new(
                matrix_screen_pos.x,
                matrix_screen_pos.y - matrix_size.y / 2.0,
            );

            self.render_attachment_edge_line(
                painter,
                start,
                end,
                edge_color,
                edge_width,
                connector_radius,
            );
        }

        // Draw attachment edge to first Product (below container)
        if let Some(product) = first_product {
            let product_screen_pos = camera.world_to_screen(product.position, screen_rect);
            let product_size = product.size * camera.zoom();

            // From container bottom center to product top
            let start = Pos2::new(container_bounds.center().x, container_bounds.max.y);
            let end = Pos2::new(
                product_screen_pos.x,
                product_screen_pos.y - product_size.y / 2.0,
            );

            self.render_attachment_edge_line(
                painter,
                start,
                end,
                edge_color,
                edge_width,
                connector_radius,
            );
        }
    }

    /// Render a single attachment edge with connector circles at endpoints
    fn render_attachment_edge_line(
        &self,
        painter: &egui::Painter,
        start: Pos2,
        end: Pos2,
        color: Color32,
        width: f32,
        connector_radius: f32,
    ) {
        // Draw the line
        painter.line_segment([start, end], Stroke::new(width, color));

        // Draw connector circles at endpoints
        painter.circle_filled(start, connector_radius, color);
        painter.circle_filled(end, connector_radius, color);
    }

    /// Compute the bounding box for a container's child nodes (in screen coordinates)
    fn compute_container_bounds(
        &self,
        nodes: &[&LayoutNode],
        camera: &Camera2D,
        screen_rect: Rect,
    ) -> Option<Rect> {
        if nodes.is_empty() {
            return None;
        }

        let padding = CONTAINER_PADDING * camera.zoom();
        let header_height = CONTAINER_HEADER_HEIGHT * camera.zoom();

        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;

        for node in nodes {
            let screen_pos = camera.world_to_screen(node.position, screen_rect);
            let screen_size = node.size * camera.zoom();

            let node_left = screen_pos.x - screen_size.x / 2.0;
            let node_right = screen_pos.x + screen_size.x / 2.0;
            let node_top = screen_pos.y - screen_size.y / 2.0;
            let node_bottom = screen_pos.y + screen_size.y / 2.0;

            min_x = min_x.min(node_left);
            max_x = max_x.max(node_right);
            min_y = min_y.min(node_top);
            max_y = max_y.max(node_bottom);
        }

        // Add padding and header space
        Some(Rect::from_min_max(
            Pos2::new(min_x - padding, min_y - padding - header_height),
            Pos2::new(max_x + padding, max_y + padding),
        ))
    }

    /// Render a container background with label and status badge
    fn render_container_background(
        &self,
        painter: &egui::Painter,
        bounds: Rect,
        label: &str,
        status: Option<&str>,
        camera: &Camera2D,
        _screen_rect: Rect,
    ) {
        let corner_radius = CORNER_RADIUS * camera.zoom();

        // Container background - subtle dark fill
        let fill_color = Color32::from_rgba_unmultiplied(30, 35, 45, 180);
        let border_color = Color32::from_rgb(70, 80, 100);
        let border_width = 2.0 * camera.zoom();

        painter.rect_filled(bounds, corner_radius, fill_color);
        painter.rect_stroke(
            bounds,
            corner_radius,
            Stroke::new(border_width, border_color),
        );

        // Container label in top-left
        let header_height = CONTAINER_HEADER_HEIGHT * camera.zoom();
        let label_pos = Pos2::new(
            bounds.left() + 12.0 * camera.zoom(),
            bounds.top() + header_height / 2.0,
        );

        let font_size = 20.0 * camera.zoom(); // Larger font for CBU container title
        let label_color = Color32::from_rgb(220, 230, 245); // Brighter for better visibility

        painter.text(
            label_pos,
            egui::Align2::LEFT_CENTER,
            label,
            FontId::proportional(font_size),
            label_color,
        );

        // Status badge on the right side of header
        if let Some(status_str) = status {
            let status_lower = status_str.to_lowercase();
            let (status_color, status_text) = match status_lower.as_str() {
                "active" => (Color32::from_rgb(34, 197, 94), "Active"), // green-500
                "pending" => (Color32::from_rgb(250, 204, 21), "Pending"), // yellow-400
                "blocked" => (Color32::from_rgb(239, 68, 68), "Blocked"), // red-500
                "draft" => (Color32::from_rgb(148, 163, 184), "Draft"), // slate-400
                "approved" => (Color32::from_rgb(34, 197, 94), "Approved"), // green-500
                "rejected" => (Color32::from_rgb(239, 68, 68), "Rejected"), // red-500
                _ => (Color32::from_rgb(148, 163, 184), status_str),    // slate-400 for unknown
            };

            // Draw status indicator circle
            let badge_x = bounds.right() - 20.0 * camera.zoom();
            let badge_y = bounds.top() + header_height / 2.0;
            let badge_radius = 5.0 * camera.zoom();

            painter.circle_filled(Pos2::new(badge_x, badge_y), badge_radius, status_color);

            // Draw status text next to the circle (if zoomed in enough)
            if camera.zoom() > 0.6 {
                let text_pos = Pos2::new(badge_x - 8.0 * camera.zoom(), badge_y);
                let text_size = 10.0 * camera.zoom();
                painter.text(
                    text_pos,
                    egui::Align2::RIGHT_CENTER,
                    status_text,
                    FontId::proportional(text_size),
                    status_color,
                );
            }
        }

        // Optional: render a subtle header divider line
        let divider_y = bounds.top() + header_height;
        if divider_y < bounds.bottom() {
            let divider_start = Pos2::new(bounds.left() + corner_radius, divider_y);
            let divider_end = Pos2::new(bounds.right() - corner_radius, divider_y);
            painter.line_segment(
                [divider_start, divider_end],
                Stroke::new(
                    1.0 * camera.zoom(),
                    Color32::from_rgba_unmultiplied(70, 80, 100, 100),
                ),
            );
        }
    }

    /// Render a single node
    #[allow(clippy::too_many_arguments)]
    fn render_node(
        &self,
        painter: &egui::Painter,
        node: &LayoutNode,
        camera: &Camera2D,
        screen_rect: Rect,
        in_focus: bool,
        is_focused: bool,
        is_highlighted: bool,
        esper_state: Option<&EsperRenderState>,
    ) {
        // Transform to screen coordinates
        let screen_pos = camera.world_to_screen(node.position, screen_rect);
        let screen_size = node.size * camera.zoom();

        // Check if visible
        let node_rect = Rect::from_center_size(screen_pos, screen_size);
        if !screen_rect.intersects(node_rect) {
            return;
        }

        // Calculate opacity and highlight state
        // First, apply Esper render modes if active
        let (esper_alpha, esper_highlight) = if let Some(esper) = esper_state {
            if esper.any_mode_active() {
                // Use Esper state to compute alpha and highlight
                // depth: use hierarchy_depth from node (clamped to u8)
                // has_red_flag: use needs_attention from node
                // has_gap: for now, treat incomplete KYC as a gap
                let depth = node.hierarchy_depth as u8;
                let has_red_flag = node.needs_attention;
                let has_gap = node.kyc_completion.map(|c| c < 100).unwrap_or(false);

                esper.get_node_alpha(is_focused, depth, has_red_flag, has_gap)
            } else {
                (1.0, false)
            }
        } else {
            (1.0, false)
        };

        // If Esper computed alpha is 0, skip rendering entirely (peel mode hides nodes)
        if esper_alpha <= 0.0 {
            return;
        }

        // Combine focus opacity with Esper alpha
        let base_opacity = if in_focus || is_highlighted || esper_highlight {
            1.0
        } else {
            self.blur_opacity
        };
        let opacity = base_opacity * esper_alpha;

        if self.use_lod {
            // Use LOD system
            let lod = DetailLevel::from_screen_size(screen_size.x, is_focused);
            #[cfg(target_arch = "wasm32")]
            {
                static LOGGED: std::sync::atomic::AtomicBool =
                    std::sync::atomic::AtomicBool::new(false);
                if !LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    web_sys::console::log_1(
                        &format!(
                            "LOD: screen_size.x={}, lod={:?}, use_lod={}",
                            screen_size.x, lod, self.use_lod
                        )
                        .into(),
                    );
                }
            }

            // If esper highlight is active, render with highlight effect
            if esper_highlight {
                self.render_node_with_esper_highlight(
                    painter,
                    node,
                    screen_pos,
                    screen_size,
                    camera.zoom(),
                    opacity,
                    esper_state,
                );
            } else {
                render_node_at_lod(painter, node, screen_pos, screen_size, lod, opacity);
            }
        } else {
            // Legacy rendering
            self.render_node_legacy(painter, node, screen_pos, screen_size, opacity);
        }
    }

    /// Render a node with Esper highlight effect (glow, pulsing border, etc.)
    #[allow(clippy::too_many_arguments)]
    fn render_node_with_esper_highlight(
        &self,
        painter: &egui::Painter,
        node: &LayoutNode,
        screen_pos: Pos2,
        screen_size: Vec2,
        zoom: f32,
        opacity: f32,
        esper_state: Option<&EsperRenderState>,
    ) {
        let node_rect = Rect::from_center_size(screen_pos, screen_size);
        let corner_radius = CORNER_RADIUS * zoom;

        // Determine highlight color based on active Esper mode
        let highlight_color = if let Some(esper) = esper_state {
            if esper.red_flag_scan_enabled {
                // Red/orange glow for red flags
                Color32::from_rgb(255, 100, 50)
            } else if esper.black_hole_enabled {
                // Purple/magenta for gaps/black holes
                Color32::from_rgb(180, 50, 255)
            } else if esper.illuminate_enabled {
                // Gold/yellow for illuminate
                Color32::from_rgb(255, 200, 50)
            } else {
                // Default highlight
                Color32::from_rgb(100, 200, 255)
            }
        } else {
            Color32::from_rgb(100, 200, 255)
        };

        // Draw outer glow (larger rect with fade)
        let glow_expand = 6.0 * zoom;
        let glow_rect = node_rect.expand(glow_expand);
        let glow_color = Color32::from_rgba_unmultiplied(
            highlight_color.r(),
            highlight_color.g(),
            highlight_color.b(),
            (80.0 * opacity) as u8,
        );
        painter.rect_filled(glow_rect, corner_radius + glow_expand / 2.0, glow_color);

        // Draw node background
        let fill = apply_opacity(node.style.fill_color, opacity);
        let text_color = apply_opacity(node.style.text_color, opacity);

        painter.rect_filled(node_rect, corner_radius, fill);

        // Draw highlighted border
        let border_width = 3.0 * zoom;
        let border_color = apply_opacity(highlight_color, opacity);
        painter.rect_stroke(
            node_rect,
            corner_radius,
            Stroke::new(border_width, border_color),
        );

        // Draw label
        let font_size = 12.0 * zoom;
        painter.text(
            screen_pos,
            egui::Align2::CENTER_CENTER,
            &node.label,
            FontId::proportional(font_size),
            text_color,
        );
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
    #[allow(clippy::too_many_arguments)]
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
        let source_bottom = source_pos + Vec2::new(0.0, source_node.size.y * camera.zoom() / 2.0);
        let target_top = target_pos - Vec2::new(0.0, target_node.size.y * camera.zoom() / 2.0);

        // Apply focus opacity
        let opacity = if in_focus { 1.0 } else { self.blur_opacity };

        // Determine edge styling based on verification status (if present) or edge type
        let (base_color, is_dashed) =
            if let Some(ver_style) = verification_edge_style(edge.verification_status.as_deref()) {
                (ver_style.color, ver_style.dashed)
            } else {
                (edge_color(edge.edge_type), edge.style.dashed)
            };

        let color = apply_opacity(base_color, opacity);

        // Apply weight-based width multiplier (for ownership percentage)
        let weight_multiplier = edge_width_for_weight(edge.weight);
        let width = edge.style.width * camera.zoom() * weight_multiplier;

        if self.use_bezier_edges {
            // Use bezier curves with offset for parallel edges
            let base_strength = curve_strength_for_edge(edge.edge_type, None);
            let curve_strength = parallel_edge_offset(edge_index, total_parallel, base_strength);
            let curve = EdgeCurve::new(source_bottom, target_top, curve_strength);

            // Render curve
            render_bezier_edge(painter, &curve, Stroke::new(width, color), is_dashed);

            // Render arrow at end
            let direction = curve.end_direction();
            render_arrow_head(painter, target_top, direction, camera.zoom(), color);

            // Render label if present
            if should_show_edge_label(edge.label.is_some(), camera.zoom()) {
                if let Some(ref label) = edge.label {
                    render_edge_label(
                        painter,
                        curve.midpoint(),
                        label,
                        camera.zoom(),
                        Color32::WHITE,
                        apply_opacity(Color32::from_rgb(80, 80, 80), opacity),
                    );
                }
            }
        } else {
            // Legacy straight line rendering
            if is_dashed {
                self.draw_dashed_line(painter, source_bottom, target_top, color, width);
            } else {
                painter.line_segment([source_bottom, target_top], Stroke::new(width, color));
            }

            // Draw arrow at target
            let direction = (target_top - source_bottom).normalized();
            render_arrow_head(painter, target_top, direction, camera.zoom(), color);

            // Draw label if present
            if let Some(ref label) = edge.label {
                let mid = Pos2::new(
                    (source_bottom.x + target_top.x) / 2.0,
                    (source_bottom.y + target_top.y) / 2.0,
                );
                let label_color = apply_opacity(Color32::from_rgb(180, 180, 180), opacity);
                painter.text(
                    mid + Vec2::new(8.0 * camera.zoom(), 0.0),
                    egui::Align2::LEFT_CENTER,
                    label,
                    FontId::proportional(9.0 * camera.zoom()),
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
        let size = Vec2::new(140.0, 50.0) * camera.zoom();
        let rect = Rect::from_center_size(screen_pos, size);

        if !screen_rect.intersects(rect) {
            return;
        }

        // Draw collapsed group box
        let fill = Color32::from_rgb(55, 48, 23);
        let border = Color32::from_rgb(161, 98, 7);
        let corner_radius = CORNER_RADIUS * camera.zoom();

        painter.rect_filled(rect, corner_radius, fill);
        painter.rect_stroke(
            rect,
            corner_radius,
            Stroke::new(2.0 * camera.zoom(), border),
        );

        // Draw investor count
        let text = format!("{} Investors", group.investor_count);
        painter.text(
            screen_pos,
            egui::Align2::CENTER_CENTER,
            text,
            FontId::proportional(11.0 * camera.zoom()),
            Color32::WHITE,
        );

        // Draw expand hint if zoomed in enough
        if camera.zoom() > 0.7 {
            let hint_pos = Pos2::new(rect.right() - 8.0 * camera.zoom(), rect.center().y);
            painter.text(
                hint_pos,
                egui::Align2::RIGHT_CENTER,
                "+",
                FontId::proportional(14.0 * camera.zoom()),
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

    /// Check if a node matches a type code from the ontology
    ///
    /// Matches if the node's entity type or category matches the filter.
    /// Also matches parent types (e.g., "SHELL" matches "LIMITED_COMPANY")
    fn node_matches_type(&self, node: &LayoutNode, type_code: &str) -> bool {
        // Direct match on entity_type enum
        let node_type_str = match node.entity_type {
            EntityType::ProperPerson => "PROPER_PERSON",
            EntityType::LimitedCompany => "LIMITED_COMPANY",
            EntityType::Partnership => "PARTNERSHIP",
            EntityType::Trust => "TRUST",
            EntityType::Fund => "FUND",
            EntityType::Product => "PRODUCT",
            EntityType::Service => "SERVICE",
            EntityType::Resource => "RESOURCE",
            // Trading layer types
            EntityType::TradingProfile => "TRADING_PROFILE",
            EntityType::InstrumentMatrix => "INSTRUMENT_MATRIX",
            EntityType::InstrumentClass => "INSTRUMENT_CLASS",
            EntityType::Market => "MARKET",
            EntityType::Counterparty => "COUNTERPARTY",
            EntityType::IsdaAgreement => "ISDA_AGREEMENT",
            EntityType::CsaAgreement => "CSA_AGREEMENT",
            EntityType::ControlPortal => "CONTROL_PORTAL",
            EntityType::Unknown => "ENTITY",
        };

        if node_type_str == type_code {
            return true;
        }

        // Check entity_category for parent type matching
        if let Some(ref category) = node.entity_category {
            let cat_upper = category.to_uppercase();
            if cat_upper == type_code {
                return true;
            }

            // Parent type matching: SHELL matches all shell subtypes
            match type_code {
                "SHELL" => {
                    if cat_upper == "SHELL"
                        || matches!(
                            node.entity_type,
                            EntityType::LimitedCompany
                                | EntityType::Fund
                                | EntityType::Trust
                                | EntityType::Partnership
                        )
                    {
                        return true;
                    }
                }
                "PERSON" => {
                    if cat_upper == "PERSON" || matches!(node.entity_type, EntityType::ProperPerson)
                    {
                        return true;
                    }
                }
                "SERVICE_LAYER" => {
                    if matches!(
                        node.entity_type,
                        EntityType::Product | EntityType::Service | EntityType::Resource
                    ) {
                        return true;
                    }
                }
                "TRADING_LAYER" => {
                    if matches!(
                        node.entity_type,
                        EntityType::TradingProfile
                            | EntityType::InstrumentMatrix
                            | EntityType::InstrumentClass
                            | EntityType::Market
                            | EntityType::Counterparty
                            | EntityType::IsdaAgreement
                            | EntityType::CsaAgreement
                    ) {
                        return true;
                    }
                }
                "ENTITY" => {
                    // Root type matches everything
                    return true;
                }
                _ => {}
            }
        }

        false
    }

    /// Check if a node is a trading layer entity (for matrix highlighting)
    fn is_trading_entity(&self, node: &LayoutNode) -> bool {
        matches!(
            node.entity_type,
            EntityType::TradingProfile
                | EntityType::InstrumentMatrix
                | EntityType::InstrumentClass
                | EntityType::Market
                | EntityType::Counterparty
                | EntityType::IsdaAgreement
                | EntityType::CsaAgreement
        )
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
