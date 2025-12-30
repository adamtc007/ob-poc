//! Viewport Context for Graph Visualization
//!
//! Tracks zoom, pan, and visibility state for the graph viewport.
//! This is UI state separate from the graph data itself.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

use super::EntityGraph;

/// Viewport state for graph visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportContext {
    /// Zoom level (0.1 = zoomed out, 2.0 = zoomed in)
    pub zoom: f32,

    /// Zoom level name for agent context
    pub zoom_name: ZoomName,

    /// Pan offset from center (pixels)
    pub pan_offset: (f32, f32),

    /// Canvas dimensions
    pub canvas_size: (f32, f32),

    /// Entity IDs currently visible in viewport
    pub visible_entities: HashSet<Uuid>,

    /// Summary of what's off-screen by direction
    pub off_screen: OffScreenSummary,

    /// Whether viewport has been explicitly set
    pub is_default: bool,
}

/// Named zoom levels for agent context
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ZoomName {
    /// 0.1 - 0.3: See entire structure
    Overview,
    /// 0.3 - 0.7: Normal working view
    #[default]
    Standard,
    /// 0.7 - 2.0: Close-up with all labels
    Detail,
}

impl ZoomName {
    pub fn from_zoom(zoom: f32) -> Self {
        match zoom {
            z if z < 0.3 => ZoomName::Overview,
            z if z > 0.7 => ZoomName::Detail,
            _ => ZoomName::Standard,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ZoomName::Overview => "Overview",
            ZoomName::Standard => "Standard",
            ZoomName::Detail => "Detail",
        }
    }
}

/// Summary of entities off-screen by direction
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OffScreenSummary {
    /// Entities above viewport (owners/parents)
    pub above: usize,
    /// Entities below viewport (owned/children)
    pub below: usize,
    /// Entities to the left (siblings)
    pub left: usize,
    /// Entities to the right (siblings)
    pub right: usize,

    /// Hint about what's off-screen for agent context
    pub above_hint: Option<String>,
    /// Hint about what's below
    pub below_hint: Option<String>,
}

impl OffScreenSummary {
    /// Total entities off-screen
    pub fn total(&self) -> usize {
        self.above + self.below + self.left + self.right
    }

    /// Check if anything is off-screen
    pub fn has_any(&self) -> bool {
        self.total() > 0
    }

    /// Generate a natural language hint
    pub fn to_hint(&self) -> Option<String> {
        let mut parts = Vec::new();

        if self.above > 0 {
            parts.push(format!("{} above", self.above));
        }
        if self.below > 0 {
            parts.push(format!("{} below", self.below));
        }
        if self.left > 0 {
            parts.push(format!("{} left", self.left));
        }
        if self.right > 0 {
            parts.push(format!("{} right", self.right));
        }

        if parts.is_empty() {
            None
        } else {
            Some(format!(
                "{} entities off-screen: {}",
                self.total(),
                parts.join(", ")
            ))
        }
    }
}

/// Pan direction for viewport commands
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PanDirection {
    Up,
    Down,
    Left,
    Right,
}

impl Default for ViewportContext {
    fn default() -> Self {
        Self::new(1200.0, 800.0)
    }
}

impl ViewportContext {
    /// Create a new viewport with given canvas dimensions
    pub fn new(canvas_width: f32, canvas_height: f32) -> Self {
        Self {
            zoom: 0.5,
            zoom_name: ZoomName::Standard,
            pan_offset: (0.0, 0.0),
            canvas_size: (canvas_width, canvas_height),
            visible_entities: HashSet::new(),
            off_screen: OffScreenSummary::default(),
            is_default: true,
        }
    }

    /// Compute what's visible given current zoom/pan and node positions
    pub fn compute_visibility(&mut self, graph: &EntityGraph) {
        self.visible_entities.clear();
        self.off_screen = OffScreenSummary::default();

        let (vp_left, vp_top, vp_right, vp_bottom) = self.viewport_bounds();

        for (id, node) in &graph.nodes {
            if let (Some(x), Some(y)) = (node.x, node.y) {
                if x >= vp_left && x <= vp_right && y >= vp_top && y <= vp_bottom {
                    self.visible_entities.insert(*id);
                } else {
                    // Track off-screen direction
                    if y < vp_top {
                        self.off_screen.above += 1;
                    }
                    if y > vp_bottom {
                        self.off_screen.below += 1;
                    }
                    if x < vp_left {
                        self.off_screen.left += 1;
                    }
                    if x > vp_right {
                        self.off_screen.right += 1;
                    }
                }
            }
        }

        self.update_zoom_name();
        self.update_off_screen_hints(graph);
    }

    /// Get viewport bounds in world coordinates
    fn viewport_bounds(&self) -> (f32, f32, f32, f32) {
        let half_w = self.canvas_size.0 / 2.0 / self.zoom;
        let half_h = self.canvas_size.1 / 2.0 / self.zoom;
        let center_x = self.canvas_size.0 / 2.0 + self.pan_offset.0;
        let center_y = self.canvas_size.1 / 2.0 + self.pan_offset.1;
        (
            center_x - half_w,
            center_y - half_h,
            center_x + half_w,
            center_y + half_h,
        )
    }

    /// Update the zoom name based on current zoom level
    pub fn update_zoom_name(&mut self) {
        self.zoom_name = ZoomName::from_zoom(self.zoom);
    }

    fn update_off_screen_hints(&mut self, graph: &EntityGraph) {
        // Generate hints about what's above/below
        if self.off_screen.above > 0 {
            // Find entity types above
            let above_types = self.count_off_screen_types(graph, |y, vp_top, _| y < vp_top);
            self.off_screen.above_hint = Some(format!(
                "{} entities above ({})",
                self.off_screen.above, above_types
            ));
        }

        if self.off_screen.below > 0 {
            let below_types = self.count_off_screen_types(graph, |y, _, vp_bottom| y > vp_bottom);
            self.off_screen.below_hint = Some(format!(
                "{} entities below ({})",
                self.off_screen.below, below_types
            ));
        }
    }

    fn count_off_screen_types<F>(&self, graph: &EntityGraph, is_off_screen: F) -> String
    where
        F: Fn(f32, f32, f32) -> bool,
    {
        use std::collections::HashMap;

        let (_, vp_top, _, vp_bottom) = self.viewport_bounds();
        let mut type_counts: HashMap<String, usize> = HashMap::new();

        for node in graph.nodes.values() {
            if let Some(y) = node.y {
                if is_off_screen(y, vp_top, vp_bottom) {
                    let type_name = format!("{:?}", node.entity_type);
                    *type_counts.entry(type_name).or_insert(0) += 1;
                }
            }
        }

        // Return top 2 types
        let mut sorted: Vec<_> = type_counts.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));

        sorted
            .into_iter()
            .take(2)
            .map(|(t, c)| format!("{} {}", c, t))
            .collect::<Vec<_>>()
            .join(", ")
    }

    // =========================================================================
    // Viewport Commands
    // =========================================================================

    /// Pan the viewport in a direction
    pub fn pan(&mut self, direction: PanDirection, amount: f32) {
        match direction {
            PanDirection::Up => self.pan_offset.1 -= amount,
            PanDirection::Down => self.pan_offset.1 += amount,
            PanDirection::Left => self.pan_offset.0 -= amount,
            PanDirection::Right => self.pan_offset.0 += amount,
        }
        self.is_default = false;
    }

    /// Pan by a default amount (100 pixels)
    pub fn pan_default(&mut self, direction: PanDirection) {
        self.pan(direction, 100.0);
    }

    /// Zoom in by a factor
    pub fn zoom_in(&mut self) {
        self.zoom = (self.zoom * 1.25).min(2.0);
        self.update_zoom_name();
        self.is_default = false;
    }

    /// Zoom out by a factor
    pub fn zoom_out(&mut self) {
        self.zoom = (self.zoom / 1.25).max(0.1);
        self.update_zoom_name();
        self.is_default = false;
    }

    /// Set zoom to a specific level
    pub fn set_zoom(&mut self, level: f32) {
        self.zoom = level.clamp(0.1, 2.0);
        self.update_zoom_name();
        self.is_default = false;
    }

    /// Fit all entities in view (reset to overview)
    pub fn fit_all(&mut self) {
        self.zoom = 0.25;
        self.pan_offset = (0.0, 0.0);
        self.update_zoom_name();
        self.is_default = false;
    }

    /// Center on a specific point
    pub fn center_on(&mut self, x: f32, y: f32) {
        let center_x = self.canvas_size.0 / 2.0;
        let center_y = self.canvas_size.1 / 2.0;
        self.pan_offset = (center_x - x, center_y - y);
        self.is_default = false;
    }

    /// Center on an entity by ID
    pub fn center_on_entity(&mut self, entity_id: Uuid, graph: &EntityGraph) -> bool {
        if let Some(node) = graph.nodes.get(&entity_id) {
            if let (Some(x), Some(y)) = (node.x, node.y) {
                self.center_on(x, y);
                return true;
            }
        }
        false
    }

    /// Reset viewport to default
    pub fn reset(&mut self) {
        self.zoom = 0.5;
        self.zoom_name = ZoomName::Standard;
        self.pan_offset = (0.0, 0.0);
        self.is_default = true;
    }

    /// Check if an entity is visible
    pub fn is_visible(&self, entity_id: &Uuid) -> bool {
        self.visible_entities.contains(entity_id)
    }

    /// Get count of visible entities
    pub fn visible_count(&self) -> usize {
        self.visible_entities.len()
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_viewport_new() {
        let vp = ViewportContext::new(1200.0, 800.0);
        assert_eq!(vp.canvas_size, (1200.0, 800.0));
        assert_eq!(vp.zoom, 0.5);
        assert!(vp.is_default);
    }

    #[test]
    fn test_zoom_name_from_zoom() {
        assert_eq!(ZoomName::from_zoom(0.1), ZoomName::Overview);
        assert_eq!(ZoomName::from_zoom(0.5), ZoomName::Standard);
        assert_eq!(ZoomName::from_zoom(1.5), ZoomName::Detail);
    }

    #[test]
    fn test_pan_updates_offset() {
        let mut vp = ViewportContext::default();
        vp.pan(PanDirection::Down, 50.0);
        assert_eq!(vp.pan_offset.1, 50.0);
        assert!(!vp.is_default);
    }

    #[test]
    fn test_zoom_in_bounds() {
        let mut vp = ViewportContext::default();
        vp.zoom = 1.9;
        vp.zoom_in();
        assert!(vp.zoom <= 2.0);
    }

    #[test]
    fn test_zoom_out_bounds() {
        let mut vp = ViewportContext::default();
        vp.zoom = 0.15;
        vp.zoom_out();
        assert!(vp.zoom >= 0.1);
    }

    #[test]
    fn test_fit_all_resets() {
        let mut vp = ViewportContext::default();
        vp.pan(PanDirection::Right, 200.0);
        vp.zoom_in();
        vp.fit_all();
        assert_eq!(vp.pan_offset, (0.0, 0.0));
        assert_eq!(vp.zoom, 0.25);
    }

    #[test]
    fn test_off_screen_summary_total() {
        let summary = OffScreenSummary {
            above: 3,
            below: 5,
            left: 2,
            right: 1,
            above_hint: None,
            below_hint: None,
        };
        assert_eq!(summary.total(), 11);
        assert!(summary.has_any());
    }

    #[test]
    fn test_off_screen_hint_generation() {
        let summary = OffScreenSummary {
            above: 3,
            below: 5,
            left: 0,
            right: 0,
            above_hint: None,
            below_hint: None,
        };
        let hint = summary.to_hint().unwrap();
        assert!(hint.contains("8 entities off-screen"));
        assert!(hint.contains("3 above"));
        assert!(hint.contains("5 below"));
    }

    #[test]
    fn test_empty_off_screen_no_hint() {
        let summary = OffScreenSummary::default();
        assert!(summary.to_hint().is_none());
    }
}
