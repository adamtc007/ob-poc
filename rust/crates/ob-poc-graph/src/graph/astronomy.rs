//! Astronomy View - CBU Universe & Solar System Visualization
//!
//! Treats CBU data as a spatial universe:
//! - Universe View: All CBUs as stars, grouped by jurisdiction clusters
//! - Solar System View: Single CBU with orbiting entities
//!
//! # EGUI-RULES Compliance
//! - Transition state is UI-only (not server data)
//! - Actions return values, no callbacks
//! - Animation uses spring physics from animation module

use super::animation::{SpringConfig, SpringF32};
use super::camera::Camera2D;
use egui::Pos2;
use std::collections::HashMap;
use uuid::Uuid;

// =============================================================================
// VIEW MODE (Astronomy)
// =============================================================================

/// The astronomy view mode - universe or solar system
#[derive(Debug, Clone, PartialEq, Default)]
pub enum AstronomyView {
    /// Universe view - all CBUs visible as stars
    #[default]
    Universe,
    /// Solar system view - focused on a single CBU with orbiting entities
    SolarSystem { cbu_id: Uuid, cbu_name: String },
    /// Transitioning between views
    Transitioning {
        from: Box<AstronomyView>,
        to: Box<AstronomyView>,
        progress: f32, // 0.0 to 1.0
    },
}

// =============================================================================
// VIEW TRANSITION
// =============================================================================

/// Manages transitions between Universe and Solar System views
///
/// Following EGUI-RULES: This is UI-only state for managing animations.
/// Server data (CBU details) is requested separately and arrives via async.
#[derive(Debug, Clone)]
pub struct ViewTransition {
    /// Current view state
    current_view: AstronomyView,

    /// Fade opacity per CBU (for smooth fade out/in)
    /// Key: CBU ID as string, Value: opacity (0.0-1.0)
    cbu_opacity: HashMap<String, SpringF32>,

    /// Navigation history (breadcrumb)
    navigation_stack: Vec<NavigationEntry>,

    /// Is a transition currently in progress?
    is_transitioning: bool,

    /// Transition progress spring
    transition_progress: SpringF32,

    /// Target CBU for solar system view (set during transition)
    pending_cbu_id: Option<Uuid>,
}

/// Entry in the navigation breadcrumb
#[derive(Debug, Clone)]
pub struct NavigationEntry {
    pub view: AstronomyView,
    pub label: String,
    pub timestamp: f64, // egui time
}

impl Default for ViewTransition {
    fn default() -> Self {
        Self {
            current_view: AstronomyView::Universe,
            cbu_opacity: HashMap::new(),
            navigation_stack: vec![NavigationEntry {
                view: AstronomyView::Universe,
                label: "Universe".to_string(),
                timestamp: 0.0,
            }],
            is_transitioning: false,
            transition_progress: SpringF32::with_config(0.0, SpringConfig::from_preset("slow")),
            pending_cbu_id: None,
        }
    }
}

/// Actions that can be returned from transition logic
#[derive(Debug, Clone)]
pub enum TransitionAction {
    /// No action
    None,
    /// Request to load CBU detail data for solar system view
    LoadCbuDetail { cbu_id: Uuid },
    /// Transition completed
    TransitionComplete,
}

impl ViewTransition {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get current view
    pub fn current_view(&self) -> &AstronomyView {
        &self.current_view
    }

    /// Is currently transitioning?
    pub fn is_transitioning(&self) -> bool {
        self.is_transitioning
    }

    /// Get transition progress (0.0 to 1.0)
    pub fn transition_progress(&self) -> f32 {
        self.transition_progress.get()
    }

    /// Get opacity for a specific CBU
    pub fn cbu_opacity(&self, cbu_id: &str) -> f32 {
        self.cbu_opacity.get(cbu_id).map(|s| s.get()).unwrap_or(1.0)
    }

    /// Get navigation breadcrumb
    pub fn breadcrumb(&self) -> &[NavigationEntry] {
        &self.navigation_stack
    }

    /// Get pending CBU ID (for async data loading)
    pub fn take_pending_cbu(&mut self) -> Option<Uuid> {
        self.pending_cbu_id.take()
    }

    // =========================================================================
    // TRANSITION ACTIONS
    // =========================================================================

    /// Zoom into a specific CBU (Universe → Solar System)
    ///
    /// Returns an action indicating what needs to happen (e.g., load data)
    pub fn zoom_into_cbu(
        &mut self,
        cbu_id: Uuid,
        cbu_name: &str,
        cbu_position: Pos2,
        all_cbu_ids: &[String],
        camera: &mut Camera2D,
        current_time: f64,
    ) -> TransitionAction {
        if self.is_transitioning {
            return TransitionAction::None;
        }

        // Start transition
        self.is_transitioning = true;
        self.transition_progress.set_immediate(0.0);
        self.transition_progress.set_target(1.0);

        // Fly camera to CBU position and zoom in
        camera.fly_to_slow(cbu_position);
        camera.zoom_to_with_config(3.0, SpringConfig::from_preset("slow"));

        // Fade out other CBUs
        let target_id = cbu_id.to_string();
        for cbu_id_str in all_cbu_ids {
            let opacity = self
                .cbu_opacity
                .entry(cbu_id_str.clone())
                .or_insert_with(|| {
                    SpringF32::with_config(1.0, SpringConfig::from_preset("medium"))
                });

            if cbu_id_str == &target_id {
                // Keep target fully visible
                opacity.set_target(1.0);
            } else {
                // Fade out others
                opacity.set_target(0.0);
            }
        }

        // Update view state
        self.current_view = AstronomyView::Transitioning {
            from: Box::new(AstronomyView::Universe),
            to: Box::new(AstronomyView::SolarSystem {
                cbu_id,
                cbu_name: cbu_name.to_string(),
            }),
            progress: 0.0,
        };

        // Add to navigation stack
        self.navigation_stack.push(NavigationEntry {
            view: AstronomyView::SolarSystem {
                cbu_id,
                cbu_name: cbu_name.to_string(),
            },
            label: cbu_name.to_string(),
            timestamp: current_time,
        });

        // Request CBU detail data
        self.pending_cbu_id = Some(cbu_id);
        TransitionAction::LoadCbuDetail { cbu_id }
    }

    /// Zoom out to universe (Solar System → Universe)
    pub fn zoom_out_to_universe(
        &mut self,
        all_cbu_ids: &[String],
        camera: &mut Camera2D,
    ) -> TransitionAction {
        if self.is_transitioning {
            return TransitionAction::None;
        }

        // Start transition
        self.is_transitioning = true;
        self.transition_progress.set_immediate(1.0);
        self.transition_progress.set_target(0.0);

        // Pull camera back to universe center and zoom out
        camera.fly_to_slow(Pos2::ZERO);
        camera.zoom_to_with_config(0.5, SpringConfig::from_preset("slow"));

        // Fade in all CBUs
        for cbu_id_str in all_cbu_ids {
            let opacity = self
                .cbu_opacity
                .entry(cbu_id_str.clone())
                .or_insert_with(|| {
                    SpringF32::with_config(0.0, SpringConfig::from_preset("medium"))
                });
            opacity.set_target(1.0);
        }

        // Get current solar system CBU for transition state
        let from_view = match &self.current_view {
            AstronomyView::SolarSystem { cbu_id, cbu_name } => AstronomyView::SolarSystem {
                cbu_id: *cbu_id,
                cbu_name: cbu_name.clone(),
            },
            _ => AstronomyView::Universe,
        };

        // Update view state
        self.current_view = AstronomyView::Transitioning {
            from: Box::new(from_view),
            to: Box::new(AstronomyView::Universe),
            progress: 1.0,
        };

        // Pop from navigation stack (back to universe)
        if self.navigation_stack.len() > 1 {
            self.navigation_stack.pop();
        }

        TransitionAction::None
    }

    /// Navigate back (via breadcrumb)
    pub fn navigate_back(
        &mut self,
        all_cbu_ids: &[String],
        camera: &mut Camera2D,
    ) -> TransitionAction {
        if self.navigation_stack.len() <= 1 {
            return TransitionAction::None;
        }

        // For now, just zoom out to universe
        // In future, could support multi-level navigation
        self.zoom_out_to_universe(all_cbu_ids, camera)
    }

    // =========================================================================
    // ANIMATION UPDATE
    // =========================================================================

    /// Update transition animations (call each frame)
    ///
    /// Returns an action if transition completes
    pub fn tick(&mut self, dt: f32) -> TransitionAction {
        // Update transition progress
        self.transition_progress.tick(dt);

        // Update all CBU opacities
        for opacity in self.cbu_opacity.values_mut() {
            opacity.tick(dt);
        }

        // Check if transition completed
        if self.is_transitioning && !self.transition_progress.is_animating() {
            self.is_transitioning = false;

            // Finalize view state
            if let AstronomyView::Transitioning { to, .. } = &self.current_view {
                self.current_view = (**to).clone();
            }

            return TransitionAction::TransitionComplete;
        }

        // Update transitioning progress in view state
        if let AstronomyView::Transitioning { progress, .. } = &mut self.current_view {
            *progress = self.transition_progress.get();
        }

        TransitionAction::None
    }

    /// Check if any animations are in progress
    pub fn is_animating(&self) -> bool {
        self.is_transitioning
            || self.transition_progress.is_animating()
            || self.cbu_opacity.values().any(|o| o.is_animating())
    }

    // =========================================================================
    // RESET
    // =========================================================================

    /// Reset to universe view (e.g., when loading new data)
    pub fn reset(&mut self) {
        self.current_view = AstronomyView::Universe;
        self.cbu_opacity.clear();
        self.navigation_stack = vec![NavigationEntry {
            view: AstronomyView::Universe,
            label: "Universe".to_string(),
            timestamp: 0.0,
        }];
        self.is_transitioning = false;
        self.transition_progress.set_immediate(0.0);
        self.pending_cbu_id = None;
    }
}

// =============================================================================
// COLORS (Astronomy Theme)
// =============================================================================

/// Color palette for astronomy visualization
pub mod astronomy_colors {
    use egui::Color32;

    // Risk rating (universe view - star colors)
    pub const RISK_STANDARD: Color32 = Color32::from_rgb(76, 175, 80); // Green
    pub const RISK_LOW: Color32 = Color32::from_rgb(139, 195, 74); // Light green
    pub const RISK_MEDIUM: Color32 = Color32::from_rgb(255, 193, 7); // Amber
    pub const RISK_HIGH: Color32 = Color32::from_rgb(255, 87, 34); // Deep orange
    pub const RISK_PROHIBITED: Color32 = Color32::from_rgb(33, 33, 33); // Near black
    pub const RISK_UNRATED: Color32 = Color32::from_rgb(158, 158, 158); // Grey

    // KYC completion (solar system view)
    pub const KYC_COMPLETE: Color32 = Color32::from_rgb(76, 175, 80); // Green
    pub const KYC_PARTIAL: Color32 = Color32::from_rgb(255, 193, 7); // Amber
    pub const KYC_DRAFT: Color32 = Color32::from_rgb(158, 158, 158); // Grey
    pub const KYC_PENDING: Color32 = Color32::from_rgb(66, 165, 245); // Blue
    pub const KYC_OVERDUE: Color32 = Color32::from_rgb(244, 67, 54); // Red

    // Entity category (rings/markers)
    pub const ENTITY_SHELL: Color32 = Color32::from_rgb(66, 165, 245); // Blue
    pub const ENTITY_PERSON: Color32 = Color32::from_rgb(102, 187, 106); // Green
    pub const ENTITY_PRODUCT: Color32 = Color32::from_rgb(255, 167, 38); // Orange
    pub const ENTITY_SERVICE: Color32 = Color32::from_rgb(171, 71, 188); // Purple

    // Sun/star glow
    pub const SUN_CORE: Color32 = Color32::from_rgb(255, 215, 0); // Gold
    pub const SUN_GLOW: Color32 = Color32::from_rgb(255, 200, 50); // Warm yellow

    // Orbit lines (can't be const due to from_rgba_unmultiplied not being const fn)
    pub fn orbit_line() -> Color32 {
        Color32::from_rgba_unmultiplied(255, 255, 255, 30)
    }

    /// Get risk color from rating string
    pub fn risk_color(rating: &str) -> Color32 {
        match rating {
            "STANDARD" => RISK_STANDARD,
            "LOW" => RISK_LOW,
            "MEDIUM" => RISK_MEDIUM,
            "HIGH" => RISK_HIGH,
            "PROHIBITED" => RISK_PROHIBITED,
            _ => RISK_UNRATED,
        }
    }

    /// Get KYC color from completion status
    pub fn kyc_color(status: &str) -> Color32 {
        match status {
            "COMPLETE" | "APPROVED" => KYC_COMPLETE,
            "PARTIAL" | "IN_PROGRESS" => KYC_PARTIAL,
            "DRAFT" | "NOT_STARTED" => KYC_DRAFT,
            "PENDING" | "PENDING_REVIEW" => KYC_PENDING,
            "OVERDUE" | "EXPIRED" | "REJECTED" => KYC_OVERDUE,
            _ => KYC_DRAFT,
        }
    }

    /// Apply brightness adjustment to a color
    pub fn brighten(color: Color32, factor: f32) -> Color32 {
        let [r, g, b, a] = color.to_array();
        let brighten = |c: u8| ((c as f32 * factor).min(255.0)) as u8;
        Color32::from_rgba_unmultiplied(brighten(r), brighten(g), brighten(b), a)
    }

    /// Apply opacity to a color
    pub fn with_opacity(color: Color32, opacity: f32) -> Color32 {
        let [r, g, b, _] = color.to_array();
        Color32::from_rgba_unmultiplied(r, g, b, (opacity * 255.0) as u8)
    }
}

// =============================================================================
// BREADCRUMB RENDERING
// =============================================================================

/// Render navigation breadcrumb
///
/// Returns Some(index) if a breadcrumb item was clicked
pub fn render_breadcrumb(ui: &mut egui::Ui, entries: &[NavigationEntry]) -> Option<usize> {
    let mut clicked_index = None;

    ui.horizontal(|ui| {
        for (i, entry) in entries.iter().enumerate() {
            let is_last = i == entries.len() - 1;

            if i > 0 {
                ui.label(" › ");
            }

            if is_last {
                // Current location - just text
                ui.strong(&entry.label);
            } else {
                // Clickable link
                if ui.link(&entry.label).clicked() {
                    clicked_index = Some(i);
                }
            }
        }
    });

    clicked_index
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_transition_zoom_in() {
        let mut transition = ViewTransition::new();
        let mut camera = Camera2D::new();
        let cbu_id = Uuid::now_v7();
        let all_cbus = vec![cbu_id.to_string(), Uuid::now_v7().to_string()];

        let action = transition.zoom_into_cbu(
            cbu_id,
            "Test CBU",
            Pos2::new(100.0, 100.0),
            &all_cbus,
            &mut camera,
            0.0,
        );

        assert!(matches!(action, TransitionAction::LoadCbuDetail { .. }));
        assert!(transition.is_transitioning());
        assert_eq!(transition.breadcrumb().len(), 2);
    }

    #[test]
    fn test_view_transition_zoom_out() {
        let mut transition = ViewTransition::new();
        let mut camera = Camera2D::new();
        let cbu_id = Uuid::now_v7();
        let all_cbus = vec![cbu_id.to_string()];

        // First zoom in
        transition.zoom_into_cbu(
            cbu_id,
            "Test CBU",
            Pos2::new(100.0, 100.0),
            &all_cbus,
            &mut camera,
            0.0,
        );

        // Complete transition
        for _ in 0..120 {
            transition.tick(1.0 / 60.0);
        }

        // Then zoom out
        transition.zoom_out_to_universe(&all_cbus, &mut camera);

        assert!(transition.is_transitioning());
    }

    #[test]
    fn test_cbu_opacity() {
        let mut transition = ViewTransition::new();
        let mut camera = Camera2D::new();
        let target_id = Uuid::now_v7();
        let other_id = Uuid::now_v7();
        let all_cbus = vec![target_id.to_string(), other_id.to_string()];

        transition.zoom_into_cbu(
            target_id,
            "Target",
            Pos2::new(0.0, 0.0),
            &all_cbus,
            &mut camera,
            0.0,
        );

        // Simulate animation
        for _ in 0..120 {
            transition.tick(1.0 / 60.0);
        }

        // Target should be visible, other should be faded
        assert!(transition.cbu_opacity(&target_id.to_string()) > 0.9);
        assert!(transition.cbu_opacity(&other_id.to_string()) < 0.1);
    }
}
