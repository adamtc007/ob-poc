//! Input handling - mouse/touch interaction with the graph
//!
//! Handles clicks, drags, and scroll for node selection, panning, and zooming.

use super::camera::Camera2D;
use super::types::*;
use egui::{Pos2, Rect, Response, Vec2};

// =============================================================================
// INPUT STATE
// =============================================================================

/// Tracks input state for the graph view
#[derive(Debug, Clone, Default)]
pub struct InputState {
    /// Currently hovered node ID
    pub hovered_node: Option<String>,
    /// Currently selected/focused node ID
    pub focused_node: Option<String>,
    /// Is the user currently dragging to pan?
    pub is_panning: bool,
    /// Last pointer position for drag tracking
    #[allow(dead_code)]
    last_pointer_pos: Option<Pos2>,
}

impl InputState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear focus
    pub fn clear_focus(&mut self) {
        self.focused_node = None;
    }

    /// Set focus to a node
    pub fn set_focus(&mut self, node_id: &str) {
        self.focused_node = Some(node_id.to_string());
    }

    /// Toggle focus on a node (click to focus, click again to unfocus)
    pub fn toggle_focus(&mut self, node_id: &str) {
        if self.focused_node.as_deref() == Some(node_id) {
            self.focused_node = None;
        } else {
            self.focused_node = Some(node_id.to_string());
        }
    }
}

// =============================================================================
// INPUT HANDLER
// =============================================================================

/// Handles all input for the graph view
pub struct InputHandler;

impl InputHandler {
    /// Process input and update camera/state
    /// Returns true if the graph needs to be repainted
    pub fn handle_input(
        response: &Response,
        camera: &mut Camera2D,
        state: &mut InputState,
        graph: &LayoutGraph,
        screen_rect: Rect,
    ) -> bool {
        let mut needs_repaint = false;

        // Get pointer position
        let pointer_pos = response.hover_pos();

        // Handle hover detection
        state.hovered_node = None;
        if let Some(pos) = pointer_pos {
            if let Some(node_id) = Self::hit_test_node(pos, graph, camera, screen_rect) {
                state.hovered_node = Some(node_id);
            }
        }

        // Handle click for node selection
        if response.clicked() {
            if let Some(pos) = pointer_pos {
                if let Some(node_id) = Self::hit_test_node(pos, graph, camera, screen_rect) {
                    state.toggle_focus(&node_id);
                    needs_repaint = true;
                } else {
                    // Clicked on empty space - clear focus
                    state.clear_focus();
                    needs_repaint = true;
                }
            }
        }

        // Handle double-click to fit focused node or toggle investor group
        if response.double_clicked() {
            if let Some(pos) = pointer_pos {
                // Check if double-clicked on investor group
                if let Some(_group_idx) =
                    Self::hit_test_investor_group(pos, graph, camera, screen_rect)
                {
                    // Note: Investor group toggling would need mutable graph access
                    // For now, just pan to the group
                    needs_repaint = true;
                } else if let Some(ref focus_id) = state.focused_node {
                    if let Some(node) = graph.get_node(focus_id) {
                        camera.pan_to(node.position);
                        needs_repaint = true;
                    }
                } else {
                    // Double-click on empty space - fit to all nodes
                    camera.fit_to_bounds(graph.bounds, screen_rect, 50.0);
                    needs_repaint = true;
                }
            }
        }

        // Handle drag for panning
        if response.dragged() {
            let delta = response.drag_delta();
            if delta.length() > 0.0 {
                camera.pan(delta);
                state.is_panning = true;
                needs_repaint = true;
            }
        } else {
            state.is_panning = false;
        }

        // Handle scroll for zooming
        let scroll_delta = response.ctx.input(|i| i.raw_scroll_delta);
        if scroll_delta.y != 0.0 {
            let zoom_factor = 1.0 + scroll_delta.y * 0.001;
            if let Some(pos) = pointer_pos {
                camera.zoom_at(zoom_factor, pos, screen_rect);
                needs_repaint = true;
            }
        }

        // Handle keyboard shortcuts
        needs_repaint |= Self::handle_keyboard(response, camera, state, graph, screen_rect);

        needs_repaint
    }

    /// Handle keyboard shortcuts
    fn handle_keyboard(
        response: &Response,
        camera: &mut Camera2D,
        state: &mut InputState,
        graph: &LayoutGraph,
        screen_rect: Rect,
    ) -> bool {
        let mut needs_repaint = false;

        response.ctx.input(|i| {
            // 'R' or 'F' to fit view
            if i.key_pressed(egui::Key::R) || i.key_pressed(egui::Key::F) {
                camera.fit_to_bounds(graph.bounds, screen_rect, 50.0);
                needs_repaint = true;
            }

            // Escape to clear focus
            if i.key_pressed(egui::Key::Escape) {
                state.clear_focus();
                needs_repaint = true;
            }

            // '+' or '=' to zoom in
            if i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals) {
                camera.target_zoom = (camera.target_zoom * 1.2).min(10.0);
                needs_repaint = true;
            }

            // '-' to zoom out
            if i.key_pressed(egui::Key::Minus) {
                camera.target_zoom = (camera.target_zoom / 1.2).max(0.1);
                needs_repaint = true;
            }

            // '0' to reset zoom to 100%
            if i.key_pressed(egui::Key::Num0) {
                camera.target_zoom = 1.0;
                needs_repaint = true;
            }

            // Tab to cycle through entities
            if i.key_pressed(egui::Key::Tab) {
                Self::cycle_focus(state, graph, i.modifiers.shift);
                // Pan to newly focused node
                if let Some(ref focus_id) = state.focused_node {
                    if let Some(node) = graph.get_node(focus_id) {
                        camera.pan_to(node.position);
                    }
                }
                needs_repaint = true;
            }

            // Arrow keys for panning
            let pan_speed = 50.0 / camera.zoom;
            if i.key_down(egui::Key::ArrowLeft) {
                camera.target_center.x -= pan_speed;
                needs_repaint = true;
            }
            if i.key_down(egui::Key::ArrowRight) {
                camera.target_center.x += pan_speed;
                needs_repaint = true;
            }
            if i.key_down(egui::Key::ArrowUp) {
                camera.target_center.y -= pan_speed;
                needs_repaint = true;
            }
            if i.key_down(egui::Key::ArrowDown) {
                camera.target_center.y += pan_speed;
                needs_repaint = true;
            }

            // Home key to center on CBU root
            if i.key_pressed(egui::Key::Home) {
                // Find CBU root node
                if let Some(root) = graph.nodes.values().find(|n| n.is_cbu_root) {
                    camera.pan_to(root.position);
                    state.set_focus(&root.id);
                    needs_repaint = true;
                }
            }
        });

        needs_repaint
    }

    /// Cycle focus through entities (Tab / Shift+Tab)
    fn cycle_focus(state: &mut InputState, graph: &LayoutGraph, reverse: bool) {
        let node_ids: Vec<&String> = graph.nodes.keys().collect();
        if node_ids.is_empty() {
            return;
        }

        let current_idx = state
            .focused_node
            .as_ref()
            .and_then(|id| node_ids.iter().position(|n| *n == id));

        let next_idx = match current_idx {
            Some(i) => {
                if reverse {
                    if i == 0 {
                        node_ids.len() - 1
                    } else {
                        i - 1
                    }
                } else {
                    (i + 1) % node_ids.len()
                }
            }
            None => 0,
        };

        state.set_focus(node_ids[next_idx]);
    }

    /// Hit test to find which node (if any) is under the given screen position
    fn hit_test_node(
        screen_pos: Pos2,
        graph: &LayoutGraph,
        camera: &Camera2D,
        screen_rect: Rect,
    ) -> Option<String> {
        // Convert screen position to world coordinates
        let world_pos = camera.screen_to_world(screen_pos, screen_rect);

        // Check each node
        for node in graph.nodes.values() {
            let _half_size = node.size / 2.0;
            let node_rect = Rect::from_center_size(node.position, node.size);

            if node_rect.contains(world_pos) {
                return Some(node.id.clone());
            }
        }

        None
    }

    /// Hit test for investor groups (returns group index)
    pub fn hit_test_investor_group(
        screen_pos: Pos2,
        graph: &LayoutGraph,
        camera: &Camera2D,
        screen_rect: Rect,
    ) -> Option<usize> {
        let world_pos = camera.screen_to_world(screen_pos, screen_rect);

        for (i, group) in graph.investor_groups.iter().enumerate() {
            let group_size = Vec2::new(140.0, 50.0);
            let group_rect = Rect::from_center_size(group.position, group_size);

            if group_rect.contains(world_pos) {
                return Some(i);
            }
        }

        None
    }
}

// =============================================================================
// CURSOR HELPER
// =============================================================================

/// Get cursor icon based on current state
pub fn cursor_for_state(state: &InputState) -> egui::CursorIcon {
    if state.is_panning {
        egui::CursorIcon::Grabbing
    } else if state.hovered_node.is_some() {
        egui::CursorIcon::PointingHand
    } else {
        egui::CursorIcon::Grab
    }
}
