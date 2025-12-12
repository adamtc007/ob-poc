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
    /// Currently dragged node ID (if any)
    pub dragging_node: Option<String>,
    pub drag_start_offset: Option<egui::Vec2>,
    pub drag_start_world: Option<Pos2>,
    /// Currently resizing node ID (if any)
    pub resizing_node: Option<String>,
    pub resize_start_size: Option<egui::Vec2>,
    /// Layout dirty flag (set when position/size changes)
    pub layout_dirty: bool,
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
        graph: &mut LayoutGraph,
        screen_rect: Rect,
    ) -> bool {
        let mut needs_repaint = false;
        
        // Debug: log shift state and drag state
        #[cfg(target_arch = "wasm32")]
        {
            let shift = response.ctx.input(|i| i.modifiers.shift);
            if response.drag_started() {
                web_sys::console::log_1(&format!("Drag started! Shift held: {}", shift).into());
            }
        }

        // Get pointer position
        let pointer_pos = response.hover_pos();

        // Handle hover detection
        state.hovered_node = None;
        if let Some(pos) = pointer_pos {
            if let Some(node_id) = Self::hit_test_node(pos, graph, camera, screen_rect) {
                state.hovered_node = Some(node_id);
            }
        }

        // Handle drag start on node - we'll determine move vs resize by Shift during drag
        if response.drag_started() {
            if let Some(pos) = pointer_pos {
                if let Some(node_id) = Self::hit_test_node(pos, graph, camera, screen_rect) {
                    state.toggle_focus(&node_id);
                    let world = camera.screen_to_world(pos, screen_rect);
                    // Always set up for potential drag - shift check happens during drag
                    state.dragging_node = Some(node_id.clone());
                    state.drag_start_world = Some(world);
                    state.drag_start_offset = graph.get_node(&node_id).map(|n| n.offset);
                    state.resize_start_size = graph.get_node(&node_id).map(|n| n.size);
                    needs_repaint = true;
                } else {
                    // Clicked on empty space - clear focus
                    state.clear_focus();
                    state.dragging_node = None;
                    state.resizing_node = None;
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

                // Handle drag for moving/resizing nodes or panning
        if response.dragged() {
            let delta = response.drag_delta();
            if delta.length() > 0.0 {
                let is_shift = response.ctx.input(|i| i.modifiers.shift);
                if let Some(ref node_id) = state.dragging_node {
                    if is_shift {
                        // Shift held = resize mode
                        if let Some(node) = graph.get_node_mut(node_id) {
                            if let Some(current_size) = state.resize_start_size {
                                let delta_world = delta / camera.zoom;
                                let mut new_size = current_size + delta_world;
                                let min_size = egui::vec2(80.0, 50.0);
                                new_size.x = new_size.x.max(min_size.x);
                                new_size.y = new_size.y.max(min_size.y);
                                node.size_override = Some(new_size);
                                node.size = new_size;
                                state.resize_start_size = Some(new_size);
                                graph.recompute_bounds();
                                state.layout_dirty = true;
                                needs_repaint = true;
                            }
                        }
                    } else {
                        // No shift = move mode
                        if let Some(node) = graph.get_node_mut(node_id) {
                            if let (Some(start_world), Some(start_off), Some(pos)) = (
                                state.drag_start_world,
                                state.drag_start_offset,
                                pointer_pos,
                            ) {
                                let world_delta = camera.screen_to_world(pos, screen_rect) - start_world;
                                node.offset = start_off + world_delta;
                                node.position = node.base_position + node.offset;
                                graph.recompute_bounds();
                                state.layout_dirty = true;
                                needs_repaint = true;
                            }
                        }
                    }
                } else if let Some(ref node_id) = state.resizing_node {
                    if let Some(node) = graph.get_node_mut(node_id) {
                        if let Some(current_size) = state.resize_start_size {
                            let delta_world = delta / camera.zoom;
                            let mut new_size = current_size + delta_world;
                            let min_size = egui::vec2(80.0, 50.0);
                            new_size.x = new_size.x.max(min_size.x);
                            new_size.y = new_size.y.max(min_size.y);
                            node.size_override = Some(new_size);
                            node.size = new_size;
                            // Update start_size so next frame accumulates from current
                            state.resize_start_size = Some(new_size);
                            graph.recompute_bounds();
                            state.layout_dirty = true;
                            needs_repaint = true;
                        }
                    }
                } else {
                    camera.pan(delta);
                    state.is_panning = true;
                    needs_repaint = true;
                }
            }
        } else {
            state.is_panning = false;
        }

// Clear drag/resize on mouse up
        let primary_down = response.ctx.input(|i| i.pointer.primary_down());
        if !primary_down {
            state.dragging_node = None;
            state.resizing_node = None;
            state.drag_start_offset = None;
            state.drag_start_world = None;
            state.resize_start_size = None;
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
