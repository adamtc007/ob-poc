//! Cluster View - ManCo center with CBU icons in orbital rings
//!
//! Renders a ManCo/governance controller at center with controlled CBUs
//! arranged in concentric orbital rings. Used when session loads a "book"
//! (e.g., "show Allianz Lux book").
//!
//! # Layout
//! - ManCo icon at center (larger, distinct style)
//! - CBUs arranged in rings based on count:
//!   - 1-8 CBUs: single ring
//!   - 9-24 CBUs: two rings (inner 8, outer rest)
//!   - 25+ CBUs: three rings distributed proportionally
//!
//! # EGUI-RULES Compliance
//! - Positions are UI-local (computed from CBU count)
//! - Actions return NavigationAction values, no callbacks
//! - Ring navigation via ESPER commands (clockwise, counterclockwise, ring_out, ring_in)

use super::animation::SpringF32;
use super::astronomy::astronomy_colors;
use super::camera::Camera2D;
use egui::{Color32, FontId, Painter, Pos2, Rect, Stroke};
use ob_poc_types::galaxy::{NavigationAction, OrbitPos, RiskRating};
use std::collections::HashMap;
use uuid::Uuid;

// =============================================================================
// CLUSTER DATA
// =============================================================================

/// Data for a CBU in the cluster view
#[derive(Debug, Clone)]
pub struct ClusterCbuData {
    /// CBU UUID
    pub cbu_id: Uuid,
    /// Display name
    pub name: String,
    /// Short name for icon label (first 3 chars or abbreviation)
    pub short_name: String,
    /// Jurisdiction code
    pub jurisdiction: Option<String>,
    /// Risk rating for color coding
    pub risk_rating: RiskRating,
    /// Entity count (shown on hover)
    pub entity_count: i32,
}

/// ManCo (governance controller) data
#[derive(Debug, Clone)]
pub struct ManCoData {
    /// Entity UUID
    pub entity_id: Uuid,
    /// Display name
    pub name: String,
    /// Short name for icon
    pub short_name: String,
    /// Jurisdiction
    pub jurisdiction: Option<String>,
}

// =============================================================================
// RING LAYOUT
// =============================================================================

/// Computed ring layout for CBUs
#[derive(Debug, Clone)]
struct RingLayout {
    /// CBUs per ring (inner to outer)
    rings: Vec<Vec<usize>>, // indices into cbus array
    /// Radius for each ring
    radii: Vec<f32>,
}

impl RingLayout {
    /// Compute ring layout based on CBU count
    fn compute(cbu_count: usize, center_radius: f32, available_space: f32) -> Self {
        if cbu_count == 0 {
            return Self {
                rings: vec![],
                radii: vec![],
            };
        }

        // Determine ring distribution
        let (ring_counts, radii) = if cbu_count <= 8 {
            // Single ring
            let radius = center_radius + available_space * 0.4;
            (vec![cbu_count], vec![radius])
        } else if cbu_count <= 24 {
            // Two rings: inner gets 8, outer gets rest
            let inner = 8.min(cbu_count);
            let outer = cbu_count - inner;
            let r1 = center_radius + available_space * 0.25;
            let r2 = center_radius + available_space * 0.55;
            (vec![inner, outer], vec![r1, r2])
        } else if cbu_count <= 60 {
            // Three rings: 8, 16, rest
            let inner = 8;
            let middle = 16.min(cbu_count - inner);
            let outer = cbu_count - inner - middle;
            let r1 = center_radius + available_space * 0.2;
            let r2 = center_radius + available_space * 0.4;
            let r3 = center_radius + available_space * 0.65;
            (vec![inner, middle, outer], vec![r1, r2, r3])
        } else {
            // Four rings for very large clusters
            let r1_count = 8;
            let r2_count = 16;
            let r3_count = 24;
            let r4_count = cbu_count - r1_count - r2_count - r3_count;
            let r1 = center_radius + available_space * 0.15;
            let r2 = center_radius + available_space * 0.3;
            let r3 = center_radius + available_space * 0.5;
            let r4 = center_radius + available_space * 0.7;
            (
                vec![r1_count, r2_count, r3_count, r4_count],
                vec![r1, r2, r3, r4],
            )
        };

        // Assign CBU indices to rings
        let mut rings = Vec::new();
        let mut idx = 0;
        for count in ring_counts {
            let mut ring = Vec::new();
            for _ in 0..count {
                if idx < cbu_count {
                    ring.push(idx);
                    idx += 1;
                }
            }
            rings.push(ring);
        }

        Self { rings, radii }
    }

    /// Get position for a CBU at given ring and index
    fn position_at(&self, ring: usize, index: usize, center: Pos2) -> Option<Pos2> {
        let radius = self.radii.get(ring)?;
        let ring_size = self.rings.get(ring)?.len();
        if ring_size == 0 || index >= ring_size {
            return None;
        }

        let angle =
            (index as f32 / ring_size as f32) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2; // Start at top
        let x = center.x + radius * angle.cos();
        let y = center.y + radius * angle.sin();
        Some(Pos2::new(x, y))
    }

    /// Convert orbit position to CBU index
    fn orbit_to_index(&self, orbit: &OrbitPos) -> Option<usize> {
        let ring = self.rings.get(orbit.ring)?;
        ring.get(orbit.index).copied()
    }

    /// Convert CBU index to orbit position
    fn index_to_orbit(&self, cbu_index: usize) -> Option<OrbitPos> {
        let mut idx = 0;
        for (ring_idx, ring) in self.rings.iter().enumerate() {
            for (pos_idx, &stored_idx) in ring.iter().enumerate() {
                if stored_idx == cbu_index {
                    return Some(OrbitPos::new(ring_idx, pos_idx));
                }
                idx += 1;
            }
        }
        let _ = idx; // suppress warning
        None
    }

    /// Get ring count
    fn ring_count(&self) -> usize {
        self.rings.len()
    }

    /// Get size of a ring
    fn ring_size(&self, ring: usize) -> usize {
        self.rings.get(ring).map(|r| r.len()).unwrap_or(0)
    }
}

// =============================================================================
// CLUSTER VIEW
// =============================================================================

/// Cluster view widget - ManCo at center with CBU orbital rings
pub struct ClusterView {
    /// ManCo data (center node)
    manco: Option<ManCoData>,

    /// CBU data (orbital nodes)
    cbus: Vec<ClusterCbuData>,

    /// Computed ring layout
    layout: RingLayout,

    /// Current orbit position (for ring navigation)
    orbit_pos: OrbitPos,

    /// Hovered CBU index
    hovered: Option<usize>,

    /// Hovered ManCo (center)
    hovered_manco: bool,

    /// Glow animations for CBUs
    glow_springs: HashMap<usize, SpringF32>,

    /// Center glow animation
    center_glow: SpringF32,

    /// Has data been loaded?
    has_data: bool,

    /// Selection highlight animation
    selection_pulse: f32,
}

impl Default for ClusterView {
    fn default() -> Self {
        Self::new()
    }
}

impl ClusterView {
    /// Create a new empty cluster view
    pub fn new() -> Self {
        Self {
            manco: None,
            cbus: vec![],
            layout: RingLayout {
                rings: vec![],
                radii: vec![],
            },
            orbit_pos: OrbitPos::origin(),
            hovered: None,
            hovered_manco: false,
            glow_springs: HashMap::new(),
            center_glow: SpringF32::new(0.0),
            has_data: false,
            selection_pulse: 0.0,
        }
    }

    /// Load cluster data
    pub fn load_data(&mut self, manco: ManCoData, cbus: Vec<ClusterCbuData>) {
        self.manco = Some(manco);
        self.cbus = cbus;
        self.layout = RingLayout::compute(self.cbus.len(), 60.0, 300.0);
        self.orbit_pos = OrbitPos::origin();
        self.hovered = None;
        self.hovered_manco = false;
        self.glow_springs.clear();
        self.has_data = true;
    }

    /// Clear data
    pub fn clear(&mut self) {
        self.manco = None;
        self.cbus.clear();
        self.layout = RingLayout {
            rings: vec![],
            radii: vec![],
        };
        self.has_data = false;
    }

    /// Check if data is loaded
    pub fn has_data(&self) -> bool {
        self.has_data
    }

    /// Get current orbit position
    pub fn orbit_pos(&self) -> OrbitPos {
        self.orbit_pos
    }

    /// Get CBU at current orbit position
    pub fn current_cbu(&self) -> Option<&ClusterCbuData> {
        let idx = self.layout.orbit_to_index(&self.orbit_pos)?;
        self.cbus.get(idx)
    }

    // =========================================================================
    // RING NAVIGATION (ESPER commands)
    // =========================================================================

    /// Move clockwise within current ring
    pub fn clockwise(&mut self, steps: u32) -> bool {
        let ring_size = self.layout.ring_size(self.orbit_pos.ring);
        if ring_size == 0 {
            return false;
        }
        self.orbit_pos.index = (self.orbit_pos.index + steps as usize) % ring_size;
        true
    }

    /// Move counterclockwise within current ring
    pub fn counterclockwise(&mut self, steps: u32) -> bool {
        let ring_size = self.layout.ring_size(self.orbit_pos.ring);
        if ring_size == 0 {
            return false;
        }
        self.orbit_pos.index =
            (self.orbit_pos.index + ring_size - (steps as usize % ring_size)) % ring_size;
        true
    }

    /// Move to outer ring
    pub fn ring_out(&mut self) -> bool {
        if self.orbit_pos.ring + 1 < self.layout.ring_count() {
            self.orbit_pos.ring += 1;
            // Adjust index to stay within new ring
            let new_ring_size = self.layout.ring_size(self.orbit_pos.ring);
            if new_ring_size > 0 {
                self.orbit_pos.index = self.orbit_pos.index.min(new_ring_size - 1);
            }
            true
        } else {
            false
        }
    }

    /// Move to inner ring
    pub fn ring_in(&mut self) -> bool {
        if self.orbit_pos.ring > 0 {
            self.orbit_pos.ring -= 1;
            // Adjust index to stay within new ring
            let new_ring_size = self.layout.ring_size(self.orbit_pos.ring);
            if new_ring_size > 0 {
                self.orbit_pos.index = self.orbit_pos.index.min(new_ring_size - 1);
            }
            true
        } else {
            false
        }
    }

    /// Snap to a specific CBU by name (partial match)
    pub fn snap_to(&mut self, target: &str) -> bool {
        let target_lower = target.to_lowercase();
        for (idx, cbu) in self.cbus.iter().enumerate() {
            if cbu.name.to_lowercase().contains(&target_lower)
                || cbu.short_name.to_lowercase().contains(&target_lower)
            {
                if let Some(orbit) = self.layout.index_to_orbit(idx) {
                    self.orbit_pos = orbit;
                    return true;
                }
            }
        }
        false
    }

    // =========================================================================
    // ANIMATION
    // =========================================================================

    /// Tick animations (call before render)
    pub fn tick(&mut self, dt: f32) {
        // Update selection pulse
        self.selection_pulse += dt * 2.0;
        if self.selection_pulse > std::f32::consts::TAU {
            self.selection_pulse -= std::f32::consts::TAU;
        }

        // Update glow springs
        for spring in self.glow_springs.values_mut() {
            spring.tick(dt);
        }
        self.center_glow.tick(dt);
    }

    // =========================================================================
    // RENDERING
    // =========================================================================

    /// Render the cluster view
    pub fn render(&self, painter: &Painter, _camera: &Camera2D, screen_rect: Rect) {
        let center = screen_rect.center();
        let available_radius = screen_rect.width().min(screen_rect.height()) * 0.45;

        // Recompute layout with actual available space
        let layout = RingLayout::compute(self.cbus.len(), 50.0, available_radius);

        // Draw orbit rings (faint circles)
        for radius in &layout.radii {
            painter.circle_stroke(
                center,
                *radius,
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 20)),
            );
        }

        // Draw CBU icons
        for (ring_idx, ring) in layout.rings.iter().enumerate() {
            for (pos_idx, &cbu_idx) in ring.iter().enumerate() {
                if let Some(cbu) = self.cbus.get(cbu_idx) {
                    if let Some(pos) = layout.position_at(ring_idx, pos_idx, center) {
                        let is_selected =
                            self.orbit_pos.ring == ring_idx && self.orbit_pos.index == pos_idx;
                        let is_hovered = self.hovered == Some(cbu_idx);
                        self.draw_cbu_icon(painter, cbu, pos, is_selected, is_hovered);
                    }
                }
            }
        }

        // Draw ManCo at center
        if let Some(manco) = &self.manco {
            self.draw_manco_icon(painter, manco, center, self.hovered_manco);
        }
    }

    /// Draw a CBU icon
    fn draw_cbu_icon(
        &self,
        painter: &Painter,
        cbu: &ClusterCbuData,
        pos: Pos2,
        is_selected: bool,
        is_hovered: bool,
    ) {
        let base_radius = 20.0;
        let radius = if is_selected || is_hovered {
            base_radius * 1.2
        } else {
            base_radius
        };

        // Color based on risk rating
        let base_color = match cbu.risk_rating {
            RiskRating::High => astronomy_colors::RISK_HIGH,
            RiskRating::Medium => astronomy_colors::RISK_MEDIUM,
            RiskRating::Low => astronomy_colors::RISK_LOW,
            RiskRating::Unrated => astronomy_colors::RISK_UNRATED,
        };

        // Selection pulse effect
        let pulse_alpha = if is_selected {
            let pulse = (self.selection_pulse.sin() + 1.0) * 0.5; // 0.0 to 1.0
            (50.0 + pulse * 80.0) as u8
        } else {
            0
        };

        // Draw selection glow
        if is_selected {
            painter.circle_filled(
                pos,
                radius + 8.0,
                Color32::from_rgba_unmultiplied(255, 215, 0, pulse_alpha),
            );
        }

        // Draw main circle
        let fill_color = if is_hovered {
            astronomy_colors::brighten(base_color, 1.3)
        } else {
            base_color
        };
        painter.circle_filled(pos, radius, fill_color);

        // Draw border
        let border_color = if is_selected {
            Color32::from_rgb(255, 215, 0) // Gold
        } else {
            Color32::from_rgba_unmultiplied(255, 255, 255, 100)
        };
        painter.circle_stroke(pos, radius, Stroke::new(2.0, border_color));

        // Draw short name
        let font = FontId::proportional(10.0);
        painter.text(
            pos,
            egui::Align2::CENTER_CENTER,
            &cbu.short_name,
            font,
            Color32::WHITE,
        );

        // Draw full name on hover
        if is_hovered || is_selected {
            let name_pos = Pos2::new(pos.x, pos.y + radius + 12.0);
            painter.text(
                name_pos,
                egui::Align2::CENTER_TOP,
                &cbu.name,
                FontId::proportional(11.0),
                Color32::WHITE,
            );
        }
    }

    /// Draw the ManCo icon at center
    fn draw_manco_icon(&self, painter: &Painter, manco: &ManCoData, pos: Pos2, is_hovered: bool) {
        let radius = if is_hovered { 45.0 } else { 40.0 };

        // ManCo uses a distinct color (gold/sun)
        let fill_color = if is_hovered {
            astronomy_colors::brighten(astronomy_colors::SUN_CORE, 1.2)
        } else {
            astronomy_colors::SUN_CORE
        };

        // Draw glow
        let glow_radius = radius + 15.0;
        painter.circle_filled(
            pos,
            glow_radius,
            Color32::from_rgba_unmultiplied(255, 200, 50, 30),
        );

        // Draw main circle
        painter.circle_filled(pos, radius, fill_color);

        // Draw border
        painter.circle_stroke(
            pos,
            radius,
            Stroke::new(3.0, Color32::from_rgb(255, 235, 100)),
        );

        // Draw ManCo label
        painter.text(
            pos,
            egui::Align2::CENTER_CENTER,
            &manco.short_name,
            FontId::proportional(14.0),
            Color32::from_rgb(50, 30, 0),
        );

        // Draw full name below
        let name_pos = Pos2::new(pos.x, pos.y + radius + 10.0);
        painter.text(
            name_pos,
            egui::Align2::CENTER_TOP,
            &manco.name,
            FontId::proportional(12.0),
            Color32::WHITE,
        );
    }

    // =========================================================================
    // INPUT HANDLING
    // =========================================================================

    /// Handle input and return any navigation action
    pub fn handle_input(
        &mut self,
        response: &egui::Response,
        screen_rect: Rect,
    ) -> Option<NavigationAction> {
        let center = screen_rect.center();
        let available_radius = screen_rect.width().min(screen_rect.height()) * 0.45;
        let layout = RingLayout::compute(self.cbus.len(), 50.0, available_radius);

        // Reset hover state
        self.hovered = None;
        self.hovered_manco = false;

        // Check for hover
        if let Some(hover_pos) = response.hover_pos() {
            // Check ManCo hover
            let manco_radius = 40.0;
            if (hover_pos - center).length() < manco_radius {
                self.hovered_manco = true;
            } else {
                // Check CBU hovers
                for (ring_idx, ring) in layout.rings.iter().enumerate() {
                    for (pos_idx, &cbu_idx) in ring.iter().enumerate() {
                        if let Some(cbu_pos) = layout.position_at(ring_idx, pos_idx, center) {
                            if (hover_pos - cbu_pos).length() < 25.0 {
                                self.hovered = Some(cbu_idx);
                                break;
                            }
                        }
                    }
                }
            }
        }

        // Check for clicks
        if response.clicked() {
            if self.hovered_manco {
                // Clicked on ManCo - could show ManCo details
                // For now, just log
                return None;
            }

            if let Some(cbu_idx) = self.hovered {
                // Update orbit position to clicked CBU
                if let Some(orbit) = layout.index_to_orbit(cbu_idx) {
                    self.orbit_pos = orbit;
                }

                // Return drill-in action
                if let Some(cbu) = self.cbus.get(cbu_idx) {
                    return Some(NavigationAction::DrillIntoCbu {
                        cbu_id: cbu.cbu_id.to_string(),
                    });
                }
            }
        }

        // Double-click to drill into selected CBU
        if response.double_clicked() {
            if let Some(cbu) = self.current_cbu() {
                return Some(NavigationAction::DrillIntoCbu {
                    cbu_id: cbu.cbu_id.to_string(),
                });
            }
        }

        None
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_layout_small() {
        let layout = RingLayout::compute(5, 50.0, 200.0);
        assert_eq!(layout.rings.len(), 1);
        assert_eq!(layout.rings[0].len(), 5);
    }

    #[test]
    fn test_ring_layout_medium() {
        let layout = RingLayout::compute(15, 50.0, 200.0);
        assert_eq!(layout.rings.len(), 2);
        assert_eq!(layout.rings[0].len(), 8);
        assert_eq!(layout.rings[1].len(), 7);
    }

    #[test]
    fn test_ring_layout_large() {
        let layout = RingLayout::compute(50, 50.0, 200.0);
        assert_eq!(layout.rings.len(), 3);
    }

    #[test]
    fn test_navigation_clockwise() {
        let mut view = ClusterView::new();
        view.layout = RingLayout::compute(8, 50.0, 200.0);
        view.orbit_pos = OrbitPos::new(0, 0);

        view.clockwise(1);
        assert_eq!(view.orbit_pos.index, 1);

        view.clockwise(7);
        assert_eq!(view.orbit_pos.index, 0); // Wrapped around
    }

    #[test]
    fn test_navigation_ring() {
        let mut view = ClusterView::new();
        view.layout = RingLayout::compute(20, 50.0, 200.0);
        view.orbit_pos = OrbitPos::new(0, 0);

        assert!(view.ring_out());
        assert_eq!(view.orbit_pos.ring, 1);

        assert!(view.ring_in());
        assert_eq!(view.orbit_pos.ring, 0);

        assert!(!view.ring_in()); // Can't go below 0
    }
}
