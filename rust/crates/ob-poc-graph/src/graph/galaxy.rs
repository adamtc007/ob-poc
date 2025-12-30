//! Galaxy View - Cluster-based visualization for large CBU portfolios
//!
//! Renders clusters (jurisdictions, ManCos, etc.) as glowing orbs with:
//! - Force-directed positioning (repulsion between clusters)
//! - Zoom-responsive compression (clusters collapse at low zoom)
//! - Click to drill into solar system view
//!
//! # EGUI-RULES Compliance
//! - Cluster metadata comes from server, positions are UI-local
//! - Actions return values (GalaxyAction), no callbacks
//! - No server round-trips for animation/position state

use super::animation::SpringF32;
use super::astronomy::astronomy_colors;
use super::camera::Camera2D;
use super::force_sim::{ClusterNode, ForceConfig, ForceSimulation};
use egui::{Color32, Painter, Pos2, Rect, Stroke, Vec2};
use std::collections::HashMap;
use uuid::Uuid;

// =============================================================================
// GALAXY DATA (from server)
// =============================================================================

/// Cluster data from server (read-only, positions computed client-side)
#[derive(Debug, Clone)]
pub struct ClusterData {
    /// Cluster identifier (e.g., jurisdiction code or ManCo ID)
    pub id: String,

    /// Display label
    pub label: String,

    /// Short label for compressed view
    pub short_label: String,

    /// Number of CBUs in this cluster
    pub cbu_count: usize,

    /// CBU IDs in this cluster (for drill-down)
    pub cbu_ids: Vec<Uuid>,

    /// Cluster type for styling
    pub cluster_type: ClusterType,

    /// Optional: aggregate risk distribution
    pub risk_summary: Option<RiskSummary>,
}

/// Type of cluster (affects rendering style)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClusterType {
    #[default]
    Jurisdiction,
    ManCo,
    ProductType,
    RiskBand,
    Custom,
}

/// Aggregate risk distribution for a cluster
#[derive(Debug, Clone, Default)]
pub struct RiskSummary {
    pub low: usize,
    pub medium: usize,
    pub high: usize,
    pub unrated: usize,
}

impl RiskSummary {
    /// Get dominant risk level
    pub fn dominant(&self) -> &'static str {
        let max = self.low.max(self.medium).max(self.high).max(self.unrated);
        if max == self.high {
            "HIGH"
        } else if max == self.medium {
            "MEDIUM"
        } else if max == self.low {
            "LOW"
        } else {
            "UNRATED"
        }
    }

    /// Get color based on dominant risk
    pub fn color(&self) -> Color32 {
        astronomy_colors::risk_color(self.dominant())
    }
}

// =============================================================================
// GALAXY VIEW
// =============================================================================

/// Galaxy view widget - renders clusters with force simulation
pub struct GalaxyView {
    /// Force simulation for cluster positioning
    simulation: ForceSimulation,

    /// Cluster metadata (from server)
    clusters: HashMap<String, ClusterData>,

    /// Hovered cluster ID
    hovered: Option<String>,

    /// Glow animation per cluster (for hover effects)
    glow_springs: HashMap<String, SpringF32>,

    /// Is data loaded?
    has_data: bool,
}

impl Default for GalaxyView {
    fn default() -> Self {
        Self::new()
    }
}

/// Actions returned from galaxy view (caller handles these)
#[derive(Debug, Clone)]
pub enum GalaxyAction {
    /// No action
    None,
    /// Cluster was clicked - drill down to solar system
    DrillDown {
        cluster_id: String,
        cluster_label: String,
        cbu_ids: Vec<Uuid>,
    },
    /// Hover changed
    HoverChanged { cluster_id: Option<String> },
}

impl GalaxyView {
    pub fn new() -> Self {
        Self {
            simulation: ForceSimulation::with_config(ForceConfig::galaxy()),
            clusters: HashMap::new(),
            hovered: None,
            glow_springs: HashMap::new(),
            has_data: false,
        }
    }

    // =========================================================================
    // DATA LOADING
    // =========================================================================

    /// Load cluster data from server
    pub fn set_clusters(&mut self, clusters: Vec<ClusterData>) {
        self.simulation.clear();
        self.clusters.clear();
        self.glow_springs.clear();

        for cluster in clusters {
            // Create simulation node
            let node = ClusterNode::new(&cluster.id, &cluster.label, cluster.cbu_count)
                .with_color(self.color_for_cluster(&cluster));

            self.simulation.add_node(node);
            self.glow_springs
                .insert(cluster.id.clone(), SpringF32::new(0.0));
            self.clusters.insert(cluster.id.clone(), cluster);
        }

        self.has_data = !self.clusters.is_empty();

        // Give initial kick to spread out
        if self.has_data {
            self.simulation.kick();
        }
    }

    /// Generate mock data for testing
    pub fn load_mock_data(&mut self) {
        let mock_clusters = vec![
            ClusterData {
                id: "LU".into(),
                label: "Luxembourg".into(),
                short_label: "LU".into(),
                cbu_count: 177,
                cbu_ids: vec![],
                cluster_type: ClusterType::Jurisdiction,
                risk_summary: Some(RiskSummary {
                    low: 150,
                    medium: 20,
                    high: 5,
                    unrated: 2,
                }),
            },
            ClusterData {
                id: "IE".into(),
                label: "Ireland".into(),
                short_label: "IE".into(),
                cbu_count: 150,
                cbu_ids: vec![],
                cluster_type: ClusterType::Jurisdiction,
                risk_summary: Some(RiskSummary {
                    low: 120,
                    medium: 25,
                    high: 3,
                    unrated: 2,
                }),
            },
            ClusterData {
                id: "DE".into(),
                label: "Germany".into(),
                short_label: "DE".into(),
                cbu_count: 200,
                cbu_ids: vec![],
                cluster_type: ClusterType::Jurisdiction,
                risk_summary: Some(RiskSummary {
                    low: 180,
                    medium: 15,
                    high: 3,
                    unrated: 2,
                }),
            },
            ClusterData {
                id: "FR".into(),
                label: "France".into(),
                short_label: "FR".into(),
                cbu_count: 80,
                cbu_ids: vec![],
                cluster_type: ClusterType::Jurisdiction,
                risk_summary: Some(RiskSummary {
                    low: 70,
                    medium: 8,
                    high: 1,
                    unrated: 1,
                }),
            },
            ClusterData {
                id: "UK".into(),
                label: "United Kingdom".into(),
                short_label: "UK".into(),
                cbu_count: 45,
                cbu_ids: vec![],
                cluster_type: ClusterType::Jurisdiction,
                risk_summary: Some(RiskSummary {
                    low: 35,
                    medium: 8,
                    high: 1,
                    unrated: 1,
                }),
            },
            ClusterData {
                id: "CH".into(),
                label: "Switzerland".into(),
                short_label: "CH".into(),
                cbu_count: 19,
                cbu_ids: vec![],
                cluster_type: ClusterType::Jurisdiction,
                risk_summary: Some(RiskSummary {
                    low: 15,
                    medium: 3,
                    high: 1,
                    unrated: 0,
                }),
            },
        ];

        self.set_clusters(mock_clusters);
    }

    /// Check if data is loaded
    pub fn has_data(&self) -> bool {
        self.has_data
    }

    /// Clear all data
    pub fn clear(&mut self) {
        self.simulation.clear();
        self.clusters.clear();
        self.glow_springs.clear();
        self.hovered = None;
        self.has_data = false;
    }

    // =========================================================================
    // STYLING
    // =========================================================================

    fn color_for_cluster(&self, cluster: &ClusterData) -> Color32 {
        if let Some(ref risk) = cluster.risk_summary {
            risk.color()
        } else {
            match cluster.cluster_type {
                ClusterType::Jurisdiction => Color32::from_rgb(100, 149, 237), // Cornflower
                ClusterType::ManCo => Color32::from_rgb(147, 112, 219),        // Medium purple
                ClusterType::ProductType => Color32::from_rgb(255, 167, 38),   // Orange
                ClusterType::RiskBand => Color32::from_rgb(76, 175, 80),       // Green
                ClusterType::Custom => Color32::from_rgb(158, 158, 158),       // Grey
            }
        }
    }

    // =========================================================================
    // UPDATE & RENDER
    // =========================================================================

    /// Update simulation and render
    ///
    /// Returns action if user interacted
    pub fn ui(
        &mut self,
        painter: &Painter,
        camera: &Camera2D,
        screen_rect: Rect,
        dt: f32,
    ) -> GalaxyAction {
        // Update simulation
        self.simulation.set_zoom(camera.zoom());
        self.simulation.tick(dt);

        // Update glow springs
        for (id, spring) in self.glow_springs.iter_mut() {
            let target = if Some(id) == self.hovered.as_ref() {
                1.0
            } else {
                0.0
            };
            spring.set_target(target);
            spring.tick(dt);
        }

        // Render clusters
        let compression = self.simulation.compression();

        for node in self.simulation.nodes() {
            let screen_pos = camera.world_to_screen(node.position, screen_rect);
            let radius = node.display_radius(compression) * camera.zoom();
            let glow = self
                .glow_springs
                .get(&node.id)
                .map(|s| s.get())
                .unwrap_or(0.0);

            self.render_cluster(painter, screen_pos, radius, node, glow, compression);
        }

        // Render title
        self.render_title(painter, screen_rect);

        GalaxyAction::None
    }

    /// Handle input (call before ui)
    pub fn handle_input(
        &mut self,
        response: &egui::Response,
        camera: &Camera2D,
        screen_rect: Rect,
    ) -> GalaxyAction {
        let mut action = GalaxyAction::None;

        // Hit test for hover
        if let Some(pointer_pos) = response.hover_pos() {
            let world_pos = camera.screen_to_world(pointer_pos, screen_rect);
            let new_hover = self.simulation.node_id_at(world_pos).map(|s| s.to_string());

            if new_hover != self.hovered {
                self.hovered = new_hover.clone();
                action = GalaxyAction::HoverChanged {
                    cluster_id: new_hover,
                };
            }
        } else if self.hovered.is_some() {
            self.hovered = None;
            action = GalaxyAction::HoverChanged { cluster_id: None };
        }

        // Click to drill down
        if response.clicked() {
            if let Some(ref hovered_id) = self.hovered {
                if let Some(cluster) = self.clusters.get(hovered_id) {
                    action = GalaxyAction::DrillDown {
                        cluster_id: cluster.id.clone(),
                        cluster_label: cluster.label.clone(),
                        cbu_ids: cluster.cbu_ids.clone(),
                    };
                }
            }
        }

        // Drag to move cluster
        if response.dragged() {
            if let Some(ref hovered_id) = self.hovered {
                self.simulation.pin(hovered_id);
                if let Some(pointer_pos) = response.hover_pos() {
                    let world_pos = camera.screen_to_world(pointer_pos, screen_rect);
                    self.simulation.move_node(hovered_id, world_pos);
                }
            }
        }

        if response.drag_stopped() {
            if let Some(ref hovered_id) = self.hovered {
                self.simulation.unpin(hovered_id);
            }
        }

        action
    }

    // =========================================================================
    // RENDERING
    // =========================================================================

    fn render_cluster(
        &self,
        painter: &Painter,
        pos: Pos2,
        radius: f32,
        node: &ClusterNode,
        glow: f32,
        compression: f32,
    ) {
        let is_hovered = Some(&node.id) == self.hovered.as_ref();

        // Outer glow (always visible, stronger on hover)
        let glow_radius = radius * (1.3 + glow * 0.3);
        let glow_alpha = (0.15 + glow * 0.2) as u8;
        let glow_color = Color32::from_rgba_unmultiplied(
            node.color.r(),
            node.color.g(),
            node.color.b(),
            (glow_alpha as f32 * 255.0) as u8,
        );
        painter.circle_filled(pos, glow_radius, glow_color);

        // Middle glow layer
        let mid_glow_radius = radius * 1.15;
        let mid_glow_color = Color32::from_rgba_unmultiplied(
            node.color.r(),
            node.color.g(),
            node.color.b(),
            60 + (glow * 40.0) as u8,
        );
        painter.circle_filled(pos, mid_glow_radius, mid_glow_color);

        // Core circle
        let core_color = if is_hovered {
            astronomy_colors::brighten(node.color, 1.2)
        } else {
            node.color
        };
        painter.circle_filled(pos, radius, core_color);

        // Border
        let border_color = if is_hovered {
            Color32::WHITE
        } else {
            Color32::from_rgba_unmultiplied(255, 255, 255, 100)
        };
        painter.circle_stroke(pos, radius, Stroke::new(1.5, border_color));

        // Label (switch between short and full based on compression/zoom)
        let label = if compression > 0.5 || radius < 30.0 {
            &node.short_label
        } else {
            &node.label
        };

        let font_size = (12.0 + radius * 0.15).min(18.0);
        let text_color = if is_hovered {
            Color32::WHITE
        } else {
            Color32::from_rgb(220, 220, 220)
        };

        painter.text(
            pos,
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(font_size),
            text_color,
        );

        // Count badge (below label, if not too compressed)
        if compression < 0.7 && radius > 25.0 {
            if let Some(cluster) = self.clusters.get(&node.id) {
                let count_text = format!("{}", cluster.cbu_count);
                painter.text(
                    pos + Vec2::new(0.0, font_size * 0.8),
                    egui::Align2::CENTER_CENTER,
                    count_text,
                    egui::FontId::proportional(10.0),
                    Color32::from_rgb(180, 180, 180),
                );
            }
        }
    }

    fn render_title(&self, painter: &Painter, screen_rect: Rect) {
        let total_cbus: usize = self.clusters.values().map(|c| c.cbu_count).sum();
        let title = format!("Client Universe ({} CBUs)", total_cbus);

        painter.text(
            screen_rect.center_top() + Vec2::new(0.0, 30.0),
            egui::Align2::CENTER_TOP,
            title,
            egui::FontId::proportional(20.0),
            Color32::from_rgb(200, 200, 200),
        );
    }

    // =========================================================================
    // QUERIES
    // =========================================================================

    /// Check if simulation needs repaint
    pub fn needs_repaint(&self) -> bool {
        !self.simulation.is_stable() || self.glow_springs.values().any(|s| s.is_animating())
    }

    /// Get cluster count
    pub fn cluster_count(&self) -> usize {
        self.clusters.len()
    }

    /// Get total CBU count
    pub fn total_cbu_count(&self) -> usize {
        self.clusters.values().map(|c| c.cbu_count).sum()
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_mock_data() {
        let mut view = GalaxyView::new();
        view.load_mock_data();

        assert!(view.has_data());
        assert_eq!(view.cluster_count(), 6);
        assert!(view.total_cbu_count() > 600);
    }

    #[test]
    fn test_risk_summary_dominant() {
        let summary = RiskSummary {
            low: 100,
            medium: 20,
            high: 5,
            unrated: 0,
        };
        assert_eq!(summary.dominant(), "LOW");

        let summary = RiskSummary {
            low: 10,
            medium: 50,
            high: 5,
            unrated: 0,
        };
        assert_eq!(summary.dominant(), "MEDIUM");
    }

    #[test]
    fn test_clear() {
        let mut view = GalaxyView::new();
        view.load_mock_data();
        assert!(view.has_data());

        view.clear();
        assert!(!view.has_data());
        assert_eq!(view.cluster_count(), 0);
    }
}
