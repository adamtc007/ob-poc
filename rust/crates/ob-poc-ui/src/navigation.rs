//! NavigationService - Single service orchestrating all galaxy navigation state
//!
//! # EGUI-RULES Compliance
//! - NavigationService owns ALL navigation state (camera, scope, springs, cache)
//! - No state in widgets - widgets receive &NavigationState, return Option<NavigationAction>
//! - No callbacks - actions are processed in app.rs update() loop
//! - Animation in tick() BEFORE ui() - widgets just read interpolated values
//! - Service mutates, widget renders
//!
//! # Usage in app.rs
//! ```ignore
//! // In update() - BEFORE any ui() calls:
//! self.navigation_service.tick(dt);
//!
//! // In ui() - widgets READ state, RETURN actions:
//! let action = galaxy_view.ui(ui, self.navigation_service.state());
//! if let Some(action) = action {
//!     self.pending_navigation_action = Some(action);
//! }
//!
//! // After ui() - process pending actions:
//! if let Some(action) = self.pending_navigation_action.take() {
//!     self.navigation_service.handle_action(action);
//! }
//! ```

use ob_poc_graph::graph::animation::{SpringConfig, SpringF32};
use ob_poc_graph::graph::camera::Camera2D;
use ob_poc_types::galaxy::{
    ClusterDetailGraph, ClusterType, DepthColors, NavigationAction, NavigationScope,
    PrefetchStatus, UniverseGraph, ViewLevel, ViewTransition,
};

use egui::Pos2;
use std::collections::HashMap;

// =============================================================================
// NAVIGATION STATE (read-only view for widgets)
// =============================================================================

/// Read-only snapshot of navigation state for widgets
///
/// Widgets receive this via `navigation_service.state()` and use it for rendering.
/// They NEVER mutate it - mutations happen via NavigationAction return values.
#[derive(Debug)]
pub struct NavigationState<'a> {
    /// Current navigation scope (what are we looking at?)
    pub scope: &'a NavigationScope,
    /// Current discrete view level
    pub view_level: ViewLevel,
    /// Camera state (position, zoom - already interpolated)
    pub camera: &'a Camera2D,
    /// Current zoom as 0.0-1.0 normalized value
    pub zoom_normalized: f32,
    /// Is any animation in progress?
    pub is_animating: bool,
    /// Universe graph data (if loaded)
    pub universe_graph: Option<&'a UniverseGraph>,
    /// Current cluster detail (if drilled into a cluster)
    pub cluster_detail: Option<&'a ClusterDetailGraph>,
    /// Selected node ID (if any)
    pub selected_node_id: Option<&'a str>,
    /// Hovered node ID (if any)
    pub hovered_node_id: Option<&'a str>,
    /// Prefetch status for nearby scopes
    pub prefetch_status: &'a HashMap<String, PrefetchStatus>,
    /// Breadcrumb trail for navigation history
    pub breadcrumbs: &'a [BreadcrumbEntry],
    /// Active view transition (if navigating between levels)
    pub active_transition: Option<&'a ViewTransition>,
    /// Current background color based on depth (RGB)
    pub background_color: (u8, u8, u8),
    /// Current depth factor (0.0 = Universe, 1.0 = Core)
    pub depth_factor: f32,
}

/// Entry in the navigation breadcrumb trail
#[derive(Debug, Clone)]
pub struct BreadcrumbEntry {
    pub label: String,
    pub scope: NavigationScope,
    pub icon: Option<String>,
}

// =============================================================================
// NAVIGATION SERVICE
// =============================================================================

/// Central service orchestrating all galaxy navigation
///
/// Owns:
/// - Camera state (position, zoom with spring physics)
/// - Navigation scope (what we're looking at)
/// - View level (discrete astronomical level)
/// - Data cache (universe graph, cluster details)
/// - Animation state (transition springs)
/// - Selection state (selected/hovered nodes)
/// - Breadcrumb history
/// - Prefetch cache
pub struct NavigationService {
    // =========================================================================
    // CAMERA & VIEW STATE
    // =========================================================================
    /// Camera with spring-based pan/zoom
    camera: Camera2D,

    /// Current navigation scope
    scope: NavigationScope,

    /// Current discrete view level
    view_level: ViewLevel,

    /// Active view transition (for animated level changes)
    /// None means no transition in progress
    active_transition: Option<ViewTransition>,

    /// Spring for smooth transition progress animation
    transition_spring: SpringF32,

    /// Depth colors for background encoding
    depth_colors: DepthColors,

    /// Current depth factor (0.0 = Universe, 1.0 = Core)
    /// This interpolates during transitions
    current_depth: f32,

    // =========================================================================
    // DATA CACHE
    // =========================================================================
    /// Universe graph (clusters overview)
    universe_graph: Option<UniverseGraph>,

    /// Cluster detail graphs keyed by cluster_id
    cluster_details: HashMap<String, ClusterDetailGraph>,

    /// Currently displayed cluster detail
    current_cluster_id: Option<String>,

    // =========================================================================
    // SELECTION STATE
    // =========================================================================
    /// Currently selected node ID
    selected_node_id: Option<String>,

    /// Currently hovered node ID
    hovered_node_id: Option<String>,

    // =========================================================================
    // NAVIGATION HISTORY
    // =========================================================================
    /// Breadcrumb trail for back navigation
    breadcrumbs: Vec<BreadcrumbEntry>,

    // =========================================================================
    // PREFETCH CACHE
    // =========================================================================
    /// Prefetch status for adjacent scopes
    prefetch_status: HashMap<String, PrefetchStatus>,

    // =========================================================================
    // PENDING FETCHES (flags for app.rs to process)
    // =========================================================================
    /// Universe graph needs to be fetched
    pub pending_fetch_universe: bool,

    /// Cluster details need to be fetched (cluster_id)
    pub pending_fetch_cluster: Option<String>,

    /// CBU details need to be fetched (cbu_id)
    pub pending_fetch_cbu: Option<String>,
}

impl Default for NavigationService {
    fn default() -> Self {
        Self::new()
    }
}

impl NavigationService {
    /// Create a new navigation service starting at Universe view
    pub fn new() -> Self {
        let mut camera = Camera2D::new();
        // Universe view starts zoomed out
        camera.set_zoom(0.2);
        camera.min_zoom = 0.05;
        camera.max_zoom = 10.0;

        Self {
            camera,
            scope: NavigationScope::Universe,
            view_level: ViewLevel::Universe,
            active_transition: None,
            transition_spring: SpringF32::with_config(1.0, SpringConfig::SLOW),
            depth_colors: DepthColors::default(),
            current_depth: 0.0, // Start at Universe depth
            universe_graph: None,
            cluster_details: HashMap::new(),
            current_cluster_id: None,
            selected_node_id: None,
            hovered_node_id: None,
            breadcrumbs: vec![BreadcrumbEntry {
                label: "Universe".to_string(),
                scope: NavigationScope::Universe,
                icon: Some("🌌".to_string()),
            }],
            prefetch_status: HashMap::new(),
            pending_fetch_universe: true, // Start by fetching universe
            pending_fetch_cluster: None,
            pending_fetch_cbu: None,
        }
    }

    // =========================================================================
    // TICK - Called each frame BEFORE ui()
    // =========================================================================

    /// Update all animations (call at start of update(), before any ui())
    ///
    /// This is where all spring physics run. After tick(), widgets can read
    /// the interpolated values via `state()`.
    pub fn tick(&mut self, dt: f32) {
        // Update camera springs
        self.camera.update(dt);

        // Update transition animation
        self.transition_spring.tick(dt);

        // Update active ViewTransition if present
        if let Some(ref mut transition) = self.active_transition {
            // Sync progress from spring
            transition.progress = self.transition_spring.get();
            transition.elapsed += dt;

            // Update camera arrival status (camera leads content by 30%)
            if transition.camera_progress() >= 1.0 && !transition.camera_arrived {
                transition.camera_arrived = true;
            }

            // Update depth during transition for smooth background color
            self.current_depth = transition.depth_factor();

            // Check if transition is complete
            if transition.is_complete() {
                // Finalize transition
                self.view_level = transition.to_level;
                self.scope = transition.to_scope.clone();
                self.active_transition = None;
            }
        }
    }

    /// Check if any animation is in progress (for repaint scheduling)
    pub fn is_animating(&self) -> bool {
        self.camera.is_animating() || self.active_transition.is_some()
    }

    // =========================================================================
    // STATE ACCESS (for widgets)
    // =========================================================================

    /// Get read-only state snapshot for widgets
    ///
    /// Widgets call this to get current state for rendering.
    /// They NEVER mutate - they return NavigationAction instead.
    pub fn state(&self) -> NavigationState<'_> {
        // Calculate normalized zoom (0.0 = min, 1.0 = max)
        let zoom_range = self.camera.max_zoom - self.camera.min_zoom;
        let zoom_normalized = if zoom_range > 0.0 {
            (self.camera.zoom() - self.camera.min_zoom) / zoom_range
        } else {
            0.5
        };

        // Calculate background color based on current depth
        let background_color = self.depth_colors.color_at(self.current_depth);

        NavigationState {
            scope: &self.scope,
            view_level: self.view_level,
            camera: &self.camera,
            zoom_normalized,
            is_animating: self.is_animating(),
            universe_graph: self.universe_graph.as_ref(),
            cluster_detail: self
                .current_cluster_id
                .as_ref()
                .and_then(|id| self.cluster_details.get(id)),
            selected_node_id: self.selected_node_id.as_deref(),
            hovered_node_id: self.hovered_node_id.as_deref(),
            prefetch_status: &self.prefetch_status,
            breadcrumbs: &self.breadcrumbs,
            active_transition: self.active_transition.as_ref(),
            background_color,
            depth_factor: self.current_depth,
        }
    }

    /// Get camera reference (for coordinate transforms in rendering)
    pub fn camera(&self) -> &Camera2D {
        &self.camera
    }

    /// Get current scope
    pub fn scope(&self) -> &NavigationScope {
        &self.scope
    }

    /// Get current view level
    pub fn view_level(&self) -> ViewLevel {
        self.view_level
    }

    /// Get transition progress (0.0 to 1.0)
    pub fn transition_progress(&self) -> f32 {
        self.transition_spring.get()
    }

    /// Get active transition (if any)
    pub fn active_transition(&self) -> Option<&ViewTransition> {
        self.active_transition.as_ref()
    }

    /// Get current depth factor (0.0 = Universe, 1.0 = Core)
    pub fn current_depth(&self) -> f32 {
        self.current_depth
    }

    /// Get current background color based on depth
    pub fn background_color(&self) -> (u8, u8, u8) {
        self.depth_colors.color_at(self.current_depth)
    }

    /// Check if camera has arrived at destination during transition
    /// (Camera leads content by 30%)
    pub fn camera_arrived(&self) -> bool {
        self.active_transition
            .as_ref()
            .map(|t| t.camera_arrived)
            .unwrap_or(true) // No transition = camera is "arrived"
    }

    /// Mark content as ready (called when data fetch completes)
    pub fn mark_content_ready(&mut self) {
        if let Some(ref mut transition) = self.active_transition {
            transition.content_ready = true;
        }
    }

    // =========================================================================
    // ACTION HANDLING (called from app.rs after ui())
    // =========================================================================

    /// Handle a navigation action returned from a widget
    ///
    /// Called in app.rs update() loop after collecting actions from widgets.
    pub fn handle_action(&mut self, action: NavigationAction) {
        match action {
            NavigationAction::FlyTo { x, y } => {
                self.camera.fly_to(Pos2::new(x, y));
            }

            NavigationAction::ZoomTo { level } => {
                self.camera.zoom_to(level);
            }

            NavigationAction::DrillIntoCluster { cluster_id } => {
                self.drill_into_cluster(&cluster_id);
            }

            NavigationAction::DrillIntoCbu { cbu_id } => {
                self.drill_into_cbu(&cbu_id);
            }

            NavigationAction::DrillIntoEntity { entity_id } => {
                self.drill_into_entity(&entity_id);
            }

            NavigationAction::DrillUp => {
                self.drill_up();
            }

            NavigationAction::GoToUniverse => {
                self.go_to_universe();
            }

            NavigationAction::Select { node_id, .. } => {
                self.selected_node_id = Some(node_id);
            }

            NavigationAction::Deselect => {
                self.selected_node_id = None;
            }

            NavigationAction::Hover { node_id, .. } => {
                self.hovered_node_id = Some(node_id);
            }

            NavigationAction::ClearHover => {
                self.hovered_node_id = None;
            }

            NavigationAction::GoToBreadcrumb { index } => {
                self.go_to_breadcrumb(index);
            }

            NavigationAction::ZoomIn { factor } => {
                let new_zoom = self.camera.zoom() * factor.unwrap_or(1.5);
                self.camera.zoom_to(new_zoom);
            }

            NavigationAction::ZoomOut { factor } => {
                let new_zoom = self.camera.zoom() / factor.unwrap_or(1.5);
                self.camera.zoom_to(new_zoom);
            }

            NavigationAction::ZoomFit => {
                // Will need bounds from current view - handled by caller
            }

            NavigationAction::Pan { dx, dy } => {
                self.camera.pan(egui::Vec2::new(dx, dy));
            }

            NavigationAction::Center => {
                self.camera.fly_to(Pos2::ZERO);
            }

            NavigationAction::Prefetch { scope_id } => {
                self.request_prefetch(&scope_id);
            }

            NavigationAction::FetchData { scope } => {
                // Set pending flags based on scope - app.rs will handle the async fetch
                match &scope {
                    NavigationScope::Universe => {
                        self.pending_fetch_universe = true;
                    }
                    NavigationScope::Cluster { cluster_id, .. } => {
                        self.pending_fetch_cluster = Some(cluster_id.clone());
                    }
                    NavigationScope::Cbu { cbu_id, .. } => {
                        self.pending_fetch_cbu = Some(cbu_id.clone());
                    }
                    _ => {
                        // Book, Entity, Deep - handled differently
                    }
                }
            }

            NavigationAction::SetClusterType { cluster_type } => {
                // Update cluster type for current scope if applicable
                if let NavigationScope::Cluster { cluster_id, .. } = &self.scope {
                    self.scope = NavigationScope::Cluster {
                        cluster_id: cluster_id.clone(),
                        cluster_type,
                    };
                }
            }
        }
    }

    // =========================================================================
    // NAVIGATION COMMANDS
    // =========================================================================

    /// Drill into a cluster from universe view
    fn drill_into_cluster(&mut self, cluster_id: &str) {
        // Build target scope
        let cluster_type = self.get_cluster_type(cluster_id);
        let to_scope = NavigationScope::Cluster {
            cluster_id: cluster_id.to_string(),
            cluster_type,
        };
        let to_level = ViewLevel::Cluster;

        // Get target position (fall back to origin if unknown)
        let to_pos = self.get_cluster_position(cluster_id).unwrap_or(Pos2::ZERO);

        // Start the full ViewTransition
        self.start_transition_to(to_level, to_scope.clone(), to_pos);

        // Update scope immediately (transition will animate visually)
        self.scope = to_scope.clone();
        self.view_level = to_level;

        // Update breadcrumbs
        self.breadcrumbs.push(BreadcrumbEntry {
            label: self.get_cluster_label(cluster_id),
            scope: to_scope,
            icon: Some(self.get_cluster_icon(&cluster_type)),
        });

        // Request cluster data if not cached
        if !self.cluster_details.contains_key(cluster_id) {
            self.pending_fetch_cluster = Some(cluster_id.to_string());
        }
        self.current_cluster_id = Some(cluster_id.to_string());

        // Animate camera (spring physics handles smooth movement)
        self.camera.fly_to_slow(to_pos);
        self.camera.zoom_to(1.0);
    }

    /// Drill into a CBU from cluster view
    fn drill_into_cbu(&mut self, cbu_id: &str) {
        // Get CBU name from cache if available
        let cbu_name = self.get_cbu_name(cbu_id);

        // Build target scope
        let to_scope = NavigationScope::Cbu {
            cbu_id: cbu_id.to_string(),
            cbu_name: cbu_name.clone(),
        };
        let to_level = ViewLevel::System;

        // Get target position (fall back to origin if unknown)
        let to_pos = self.get_cbu_position(cbu_id).unwrap_or(Pos2::ZERO);

        // Start the full ViewTransition
        self.start_transition_to(to_level, to_scope.clone(), to_pos);

        // Update scope immediately
        self.scope = to_scope.clone();
        self.view_level = to_level;

        self.breadcrumbs.push(BreadcrumbEntry {
            label: cbu_name,
            scope: to_scope,
            icon: Some("🏛️".to_string()),
        });

        self.pending_fetch_cbu = Some(cbu_id.to_string());

        // Animate camera (spring physics handles smooth movement)
        self.camera.fly_to_slow(to_pos);
        self.camera.zoom_to(2.0);
    }

    /// Drill into an entity from CBU view
    fn drill_into_entity(&mut self, entity_id: &str) {
        let entity_name = self.get_entity_name(entity_id);
        let cbu_id = self.get_current_cbu_id().unwrap_or_default();

        // Build target scope
        let to_scope = NavigationScope::Entity {
            entity_id: entity_id.to_string(),
            entity_name: entity_name.clone(),
            cbu_id,
        };
        let to_level = ViewLevel::Planet;

        // Get target position (fall back to origin if unknown)
        let to_pos = self.get_entity_position(entity_id).unwrap_or(Pos2::ZERO);

        // Start the full ViewTransition
        self.start_transition_to(to_level, to_scope.clone(), to_pos);

        // Update scope immediately
        self.scope = to_scope.clone();
        self.view_level = to_level;

        self.breadcrumbs.push(BreadcrumbEntry {
            label: entity_name,
            scope: to_scope,
            icon: Some("👤".to_string()),
        });

        // Animate camera to focus on entity
        self.camera.fly_to(to_pos);
        self.camera.zoom_to(4.0);
    }

    /// Drill up one level
    fn drill_up(&mut self) {
        if self.breadcrumbs.len() > 1 {
            self.breadcrumbs.pop();
            // Clone the scope BEFORE calling mutable methods to avoid borrow conflict
            let parent_scope = self.breadcrumbs.last().map(|p| p.scope.clone());
            if let Some(to_scope) = parent_scope {
                let to_level = self.scope_to_view_level(&to_scope);

                // Zoom out target
                let target_zoom = match to_level {
                    ViewLevel::Universe => 0.2,
                    ViewLevel::Cluster => 1.0,
                    ViewLevel::System => 2.0,
                    ViewLevel::Planet => 4.0,
                    ViewLevel::Surface => 6.0,
                    ViewLevel::Core => 8.0,
                };

                // For drill up, we zoom out - use current position as target
                // (the parent view will be centered differently)
                let to_pos = self.camera.center();

                // Start the full ViewTransition
                self.start_transition_to(to_level, to_scope.clone(), to_pos);

                // Update scope immediately
                self.scope = to_scope;
                self.view_level = to_level;

                self.camera.zoom_to(target_zoom);
            }
        }
    }

    /// Go directly to universe view
    fn go_to_universe(&mut self) {
        let to_scope = NavigationScope::Universe;
        let to_level = ViewLevel::Universe;
        let to_pos = Pos2::ZERO;

        // Start the full ViewTransition
        self.start_transition_to(to_level, to_scope.clone(), to_pos);

        // Update state immediately
        self.scope = to_scope;
        self.view_level = to_level;
        self.breadcrumbs = vec![BreadcrumbEntry {
            label: "Universe".to_string(),
            scope: NavigationScope::Universe,
            icon: Some("🌌".to_string()),
        }];
        self.current_cluster_id = None;

        // Animate camera
        self.camera.fly_to(to_pos);
        self.camera.zoom_to(0.2);
    }

    /// Navigate to a specific breadcrumb
    fn go_to_breadcrumb(&mut self, index: usize) {
        if index >= self.breadcrumbs.len() {
            return;
        }

        // Clone target scope before modifying breadcrumbs
        let target_scope = self.breadcrumbs[index].scope.clone();
        let to_level = self.scope_to_view_level(&target_scope);

        // Calculate target position based on scope type
        let to_pos = match &target_scope {
            NavigationScope::Universe => Pos2::ZERO,
            NavigationScope::Cluster { cluster_id, .. } => {
                self.get_cluster_position(cluster_id).unwrap_or(Pos2::ZERO)
            }
            NavigationScope::Cbu { cbu_id, .. } => {
                // CBUs might have position in cluster detail
                self.get_cbu_position(cbu_id).unwrap_or(Pos2::ZERO)
            }
            _ => Pos2::ZERO,
        };

        // Start the full ViewTransition (animates camera, depth, etc.)
        self.start_transition_to(to_level, target_scope.clone(), to_pos);

        // Truncate breadcrumbs to selected index + 1
        self.breadcrumbs.truncate(index + 1);

        // Update scope and level
        self.scope = target_scope;
        self.view_level = to_level;
    }

    /// Request prefetch for a scope
    fn request_prefetch(&mut self, scope_id: &str) {
        if !self.prefetch_status.contains_key(scope_id) {
            self.prefetch_status
                .insert(scope_id.to_string(), PrefetchStatus::Queued);
        }
    }

    // =========================================================================
    // DATA LOADING (called from app.rs after async fetch completes)
    // =========================================================================

    /// Set universe graph data (from server response)
    pub fn set_universe_graph(&mut self, graph: UniverseGraph) {
        self.universe_graph = Some(graph);
    }

    /// Set cluster detail data (from server response)
    pub fn set_cluster_detail(&mut self, cluster_id: String, detail: ClusterDetailGraph) {
        self.cluster_details.insert(cluster_id, detail);
    }

    /// Update prefetch status
    pub fn set_prefetch_status(&mut self, scope_id: String, status: PrefetchStatus) {
        self.prefetch_status.insert(scope_id, status);
    }

    /// Take pending fetch requests (for app.rs to process)
    pub fn take_pending_fetch_universe(&mut self) -> bool {
        std::mem::take(&mut self.pending_fetch_universe)
    }

    pub fn take_pending_fetch_cluster(&mut self) -> Option<String> {
        self.pending_fetch_cluster.take()
    }

    pub fn take_pending_fetch_cbu(&mut self) -> Option<String> {
        self.pending_fetch_cbu.take()
    }

    // =========================================================================
    // HELPER METHODS
    // =========================================================================

    /// Start a view transition to a new level/scope
    fn start_transition_to(
        &mut self,
        to_level: ViewLevel,
        to_scope: NavigationScope,
        to_pos: Pos2,
    ) {
        let from_pos = self.camera.center();

        // Create the ViewTransition
        let transition = ViewTransition::new(
            self.view_level,
            self.scope.clone(),
            to_level,
            to_scope,
            (from_pos.x, from_pos.y),
            (to_pos.x, to_pos.y),
        );

        // Configure spring for transition duration
        // Use MEDIUM config for smooth transitions
        self.transition_spring.set_immediate(0.0);
        self.transition_spring.set_target(1.0);

        self.active_transition = Some(transition);
    }

    fn scope_to_view_level(&self, scope: &NavigationScope) -> ViewLevel {
        match scope {
            NavigationScope::Universe => ViewLevel::Universe,
            NavigationScope::Book { .. } => ViewLevel::Cluster,
            NavigationScope::Cluster { .. } => ViewLevel::Cluster,
            NavigationScope::Cbu { .. } => ViewLevel::System,
            NavigationScope::Entity { .. } => ViewLevel::Planet,
            NavigationScope::Deep { .. } => ViewLevel::Core,
        }
    }

    fn get_cluster_type(&self, cluster_id: &str) -> ClusterType {
        self.universe_graph
            .as_ref()
            .and_then(|g| g.clusters.iter().find(|c| c.id == cluster_id))
            .map(|c| c.cluster_type.clone())
            .unwrap_or(ClusterType::Client)
    }

    fn get_cluster_label(&self, cluster_id: &str) -> String {
        self.universe_graph
            .as_ref()
            .and_then(|g| g.clusters.iter().find(|c| c.id == cluster_id))
            .map(|c| c.label.clone())
            .unwrap_or_else(|| cluster_id.to_string())
    }

    fn get_cluster_icon(&self, cluster_type: &ClusterType) -> String {
        match cluster_type {
            ClusterType::Jurisdiction => "🌍".to_string(),
            ClusterType::Client => "🏢".to_string(),
            ClusterType::Risk => "⚠️".to_string(),
            ClusterType::Product => "📦".to_string(),
        }
    }

    fn get_cluster_position(&self, cluster_id: &str) -> Option<Pos2> {
        self.universe_graph
            .as_ref()
            .and_then(|g| g.clusters.iter().find(|c| c.id == cluster_id))
            .and_then(|c| c.position)
            .map(|(x, y)| Pos2::new(x, y))
    }

    fn get_cbu_name(&self, cbu_id: &str) -> String {
        // Try to find in current cluster detail
        self.current_cluster_id
            .as_ref()
            .and_then(|id| self.cluster_details.get(id))
            .and_then(|detail| detail.cbus.iter().find(|c| c.id == cbu_id))
            .map(|c| c.name.clone())
            .unwrap_or_else(|| format!("CBU {}", &cbu_id[..8.min(cbu_id.len())]))
    }

    fn get_cbu_position(&self, cbu_id: &str) -> Option<Pos2> {
        self.current_cluster_id
            .as_ref()
            .and_then(|id| self.cluster_details.get(id))
            .and_then(|detail| detail.cbus.iter().find(|c| c.id == cbu_id))
            .and_then(|c| c.position)
            .map(|(x, y)| Pos2::new(x, y))
    }

    fn get_entity_name(&self, entity_id: &str) -> String {
        // Would be looked up from CBU detail cache
        format!("Entity {}", &entity_id[..8.min(entity_id.len())])
    }

    fn get_entity_position(&self, _entity_id: &str) -> Option<Pos2> {
        // Would be looked up from CBU detail cache
        None
    }

    fn get_current_cbu_id(&self) -> Option<String> {
        match &self.scope {
            NavigationScope::Cbu { cbu_id, .. } => Some(cbu_id.clone()),
            NavigationScope::Entity { cbu_id, .. } => Some(cbu_id.clone()),
            _ => None,
        }
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_navigation_service_new() {
        let service = NavigationService::new();
        assert_eq!(service.view_level(), ViewLevel::Universe);
        assert!(matches!(service.scope(), NavigationScope::Universe));
        assert!(service.pending_fetch_universe);
    }

    #[test]
    fn test_tick_updates_camera() {
        let mut service = NavigationService::new();
        service.camera.fly_to(Pos2::new(100.0, 100.0));

        assert!(service.is_animating());

        // Simulate several frames
        for _ in 0..120 {
            service.tick(1.0 / 60.0);
        }

        // Camera should have converged
        let center = service.camera.center();
        assert!((center.x - 100.0).abs() < 1.0);
        assert!((center.y - 100.0).abs() < 1.0);
    }

    #[test]
    fn test_handle_fly_to() {
        let mut service = NavigationService::new();
        service.handle_action(NavigationAction::FlyTo { x: 50.0, y: 75.0 });

        let target = service.camera.target_center();
        assert_eq!(target.x, 50.0);
        assert_eq!(target.y, 75.0);
    }

    #[test]
    fn test_drill_up_pops_breadcrumb() {
        let mut service = NavigationService::new();
        assert_eq!(service.breadcrumbs.len(), 1);

        service.drill_into_cluster("cluster-1");
        assert_eq!(service.breadcrumbs.len(), 2);

        service.drill_up();
        assert_eq!(service.breadcrumbs.len(), 1);
        assert!(matches!(service.scope(), NavigationScope::Universe));
    }

    #[test]
    fn test_go_to_universe_resets() {
        let mut service = NavigationService::new();
        service.drill_into_cluster("cluster-1");
        service.drill_into_cbu("cbu-1");

        service.go_to_universe();

        assert_eq!(service.breadcrumbs.len(), 1);
        assert!(matches!(service.scope(), NavigationScope::Universe));
        assert_eq!(service.view_level(), ViewLevel::Universe);
    }
}
