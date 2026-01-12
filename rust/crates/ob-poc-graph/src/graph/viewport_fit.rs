//! Viewport auto-fit logic
//!
//! Computes optimal zoom level to fit content in the viewport.
//! User can override; reset returns to auto.
//!
//! Also provides ViewLevel for progressive disclosure based on zoom + content density.

use egui::{Pos2, Rect, Vec2};

use super::types::LayoutGraph;

// =============================================================================
// CONSTANTS
// =============================================================================

/// Margin around content (10% on each side)
const FIT_MARGIN: f32 = 0.9;

/// Minimum zoom level for auto-fit
const MIN_AUTO_ZOOM: f32 = 0.1;

/// Maximum zoom level for auto-fit
const MAX_AUTO_ZOOM: f32 = 2.0;

/// Maximum nodes to render before aggregation kicks in
pub const MAX_VISIBLE_NODES: usize = 200;

/// Maximum clusters to show before collapsing to galaxies
pub const MAX_VISIBLE_CLUSTERS: usize = 50;

// =============================================================================
// VIEW LEVEL
// =============================================================================

/// Progressive disclosure level based on zoom and content density
///
/// Visual progression:
/// - Galaxy: Jurisdiction bubbles (LU(47), DE(23), US(89))
/// - Region: ManCo clusters within jurisdiction
/// - Cluster: Individual CBUs
/// - Solar: CBU internals (fund + services + participants)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewLevel {
    /// Jurisdiction-level aggregation: LU(47), DE(23), US(89)
    /// Shown when: many CBUs + zoomed out
    Galaxy,

    /// ManCo/segment clusters within a jurisdiction
    /// Shown when: moderate zoom, within a jurisdiction
    Region,

    /// Individual CBUs visible
    /// Shown when: moderate zoom
    #[default]
    Cluster,

    /// CBU internals (fund + services + participants)
    /// Shown when: zoomed in on single CBU
    Solar,
}

impl ViewLevel {
    /// Determine view level from CBU count and zoom level
    ///
    /// Rules:
    /// - Many CBUs (>100) + zoomed out (<0.3) → Galaxy
    /// - Moderate CBUs (>30) + zoomed out (<0.5) → Region
    /// - Zoomed out (<0.8) → Cluster
    /// - Zoomed in → Solar
    pub fn determine(total_cbus: usize, zoom: f32) -> Self {
        match (total_cbus, zoom) {
            (n, z) if n > 100 && z < 0.3 => ViewLevel::Galaxy,
            (n, z) if n > 30 && z < 0.5 => ViewLevel::Region,
            (_, z) if z < 0.8 => ViewLevel::Cluster,
            _ => ViewLevel::Solar,
        }
    }

    /// Check if this level should show aggregated nodes
    pub fn is_aggregated(&self) -> bool {
        matches!(self, ViewLevel::Galaxy | ViewLevel::Region)
    }

    /// Get max nodes that should be visible at this level
    pub fn max_visible_nodes(&self) -> usize {
        match self {
            ViewLevel::Galaxy => 20,
            ViewLevel::Region => 50,
            ViewLevel::Cluster => MAX_VISIBLE_NODES,
            ViewLevel::Solar => MAX_VISIBLE_NODES,
        }
    }
}

// =============================================================================
// VIEWPORT FIT
// =============================================================================

/// Auto-fit state for the viewport
#[derive(Debug, Clone)]
pub struct ViewportFit {
    /// Whether auto-fit is active (user hasn't manually zoomed)
    pub auto_enabled: bool,

    /// Bounding box of all content in world coordinates
    pub content_bounds: Rect,

    /// Computed optimal zoom to fit content
    pub optimal_zoom: f32,

    /// Computed center to fit content
    pub optimal_center: Pos2,

    /// Current view level (Galaxy/Region/Cluster/Solar)
    pub view_level: ViewLevel,

    /// Total CBU count (for view level calculation)
    pub total_cbus: usize,
}

impl Default for ViewportFit {
    fn default() -> Self {
        Self {
            auto_enabled: true,
            content_bounds: Rect::NOTHING,
            optimal_zoom: 1.0,
            optimal_center: Pos2::ZERO,
            view_level: ViewLevel::default(),
            total_cbus: 0,
        }
    }
}

impl ViewportFit {
    /// Create a new ViewportFit in auto mode
    pub fn new() -> Self {
        Self::default()
    }

    /// Compute auto-fit parameters from graph content
    pub fn compute_from_graph(viewport_size: Vec2, graph: &LayoutGraph) -> Self {
        let bounds = Self::graph_bounds(graph);
        Self::compute_from_bounds(viewport_size, bounds)
    }

    /// Compute auto-fit parameters from explicit bounds
    pub fn compute_from_bounds(viewport_size: Vec2, bounds: Rect) -> Self {
        if bounds.is_negative() || bounds.width() < 1.0 || bounds.height() < 1.0 {
            return Self::default();
        }

        // Calculate zoom to fit with margin
        let zoom_x = (viewport_size.x * FIT_MARGIN) / bounds.width();
        let zoom_y = (viewport_size.y * FIT_MARGIN) / bounds.height();
        let optimal_zoom = zoom_x.min(zoom_y).clamp(MIN_AUTO_ZOOM, MAX_AUTO_ZOOM);

        Self {
            auto_enabled: true,
            content_bounds: bounds,
            optimal_zoom,
            optimal_center: bounds.center(),
            view_level: ViewLevel::default(),
            total_cbus: 0,
        }
    }

    /// Compute bounding box of all nodes in graph
    pub fn graph_bounds(graph: &LayoutGraph) -> Rect {
        if graph.nodes.is_empty() {
            return Rect::from_center_size(Pos2::ZERO, Vec2::new(100.0, 100.0));
        }

        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;

        for node in graph.nodes.values() {
            let half_size = node.size / 2.0;
            min_x = min_x.min(node.position.x - half_size.x);
            min_y = min_y.min(node.position.y - half_size.y);
            max_x = max_x.max(node.position.x + half_size.x);
            max_y = max_y.max(node.position.y + half_size.y);
        }

        // Add padding
        let padding = 50.0;
        Rect::from_min_max(
            Pos2::new(min_x - padding, min_y - padding),
            Pos2::new(max_x + padding, max_y + padding),
        )
    }

    /// Update when viewport is resized
    pub fn on_resize(&mut self, viewport_size: Vec2) {
        if self.content_bounds.is_negative() {
            return;
        }

        let zoom_x = (viewport_size.x * FIT_MARGIN) / self.content_bounds.width();
        let zoom_y = (viewport_size.y * FIT_MARGIN) / self.content_bounds.height();
        self.optimal_zoom = zoom_x.min(zoom_y).clamp(MIN_AUTO_ZOOM, MAX_AUTO_ZOOM);
    }

    /// Update when content changes
    pub fn on_content_change(&mut self, graph: &LayoutGraph, viewport_size: Vec2) {
        self.content_bounds = Self::graph_bounds(graph);
        self.optimal_center = self.content_bounds.center();
        self.on_resize(viewport_size);
    }

    /// Disable auto-fit (user has taken manual control)
    pub fn disable_auto(&mut self) {
        self.auto_enabled = false;
    }

    /// Re-enable auto-fit (reset command)
    pub fn enable_auto(&mut self) {
        self.auto_enabled = true;
    }

    /// Check if auto-fit should apply camera changes
    pub fn should_apply(&self) -> bool {
        self.auto_enabled && !self.content_bounds.is_negative()
    }

    /// Update view level based on current zoom
    /// Returns true if view level changed (triggers re-render)
    pub fn update_view_level(&mut self, zoom: f32) -> bool {
        let new_level = ViewLevel::determine(self.total_cbus, zoom);
        if new_level != self.view_level {
            self.view_level = new_level;
            true
        } else {
            false
        }
    }

    /// Set total CBU count (for view level calculation)
    pub fn set_cbu_count(&mut self, count: usize) {
        self.total_cbus = count;
    }

    /// Check if aggregation is needed at current view level
    pub fn needs_aggregation(&self) -> bool {
        self.view_level.is_aggregated()
    }

    /// Get current view level
    pub fn current_view_level(&self) -> ViewLevel {
        self.view_level
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_from_bounds() {
        let viewport = Vec2::new(800.0, 600.0);
        let bounds = Rect::from_min_max(Pos2::new(-100.0, -100.0), Pos2::new(100.0, 100.0));

        let fit = ViewportFit::compute_from_bounds(viewport, bounds);

        assert!(fit.auto_enabled);
        assert!(fit.optimal_zoom > 0.0);
        assert!(fit.optimal_zoom < 10.0);
    }

    #[test]
    fn test_empty_bounds() {
        let viewport = Vec2::new(800.0, 600.0);
        let bounds = Rect::NOTHING;

        let fit = ViewportFit::compute_from_bounds(viewport, bounds);

        assert!(fit.auto_enabled);
        assert_eq!(fit.optimal_zoom, 1.0);
    }
}
