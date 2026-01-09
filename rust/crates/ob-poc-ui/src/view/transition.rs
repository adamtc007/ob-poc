//! Layout transition and interpolation utilities
//!
//! Provides smooth transitions between graph layouts when:
//! - Changing view modes (KYC_UBO → SERVICE_DELIVERY)
//! - Drilling into/out of clusters
//! - Adding/removing nodes dynamically
//!
//! Uses easing functions for non-spring animations where appropriate.
//! For physics-based animation, use the spring system in ob-poc-graph.

use egui::{Pos2, Vec2};
use std::collections::HashMap;

// Re-export for convenience - these are the physics-based springs
// Use transition.rs for deterministic easing, springs for natural motion
pub use ob_poc_graph::graph::animation::{SpringConfig, SpringF32, SpringVec2};

// =============================================================================
// EASING FUNCTIONS
// =============================================================================

/// Cubic ease-out: fast start, slow finish (feels responsive)
/// t should be in [0, 1]
#[inline]
pub fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Cubic ease-in-out: slow start, fast middle, slow finish (feels polished)
/// t should be in [0, 1]
#[inline]
pub fn ease_in_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

/// Quadratic ease-out: slightly faster than cubic
#[inline]
pub fn ease_out_quad(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t) * (1.0 - t)
}

/// Exponential ease-out: very fast start, long tail
#[inline]
pub fn ease_out_expo(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t >= 1.0 {
        1.0
    } else {
        1.0 - 2.0_f32.powf(-10.0 * t)
    }
}

/// Linear interpolation (no easing)
#[inline]
pub fn linear(t: f32) -> f32 {
    t.clamp(0.0, 1.0)
}

// =============================================================================
// NODE POSITION SNAPSHOT
// =============================================================================

/// Snapshot of a node's position and size for interpolation
#[derive(Debug, Clone, Copy)]
pub struct NodeSnapshot {
    pub position: Pos2,
    pub size: Vec2,
    /// Opacity for fade in/out (0.0 = invisible, 1.0 = fully visible)
    pub opacity: f32,
}

impl Default for NodeSnapshot {
    fn default() -> Self {
        Self {
            position: Pos2::ZERO,
            size: Vec2::new(120.0, 60.0),
            opacity: 1.0,
        }
    }
}

impl NodeSnapshot {
    pub fn new(position: Pos2, size: Vec2) -> Self {
        Self {
            position,
            size,
            opacity: 1.0,
        }
    }

    /// Create a snapshot for a node that's fading in (starts invisible)
    pub fn fading_in(position: Pos2, size: Vec2) -> Self {
        Self {
            position,
            size,
            opacity: 0.0,
        }
    }

    /// Create a snapshot for a node that's fading out (ends invisible)
    pub fn fading_out(position: Pos2, size: Vec2) -> Self {
        Self {
            position,
            size,
            opacity: 1.0,
        }
    }
}

// =============================================================================
// LAYOUT SNAPSHOT
// =============================================================================

/// Complete snapshot of a layout for interpolation
#[derive(Debug, Clone, Default)]
pub struct LayoutSnapshot {
    /// Node positions and sizes by node ID
    pub nodes: HashMap<String, NodeSnapshot>,
    /// Bounding box center (for camera)
    pub center: Pos2,
    /// Suggested zoom level
    pub zoom: f32,
}

impl LayoutSnapshot {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create from a LayoutGraph (from ob-poc-graph)
    /// This allows capturing the current state before a transition
    pub fn from_layout_graph(graph: &ob_poc_graph::graph::types::LayoutGraph) -> Self {
        let mut nodes = HashMap::new();

        for (id, node) in &graph.nodes {
            nodes.insert(id.clone(), NodeSnapshot::new(node.position, node.size));
        }

        let center = if graph.bounds.is_positive() {
            graph.bounds.center()
        } else {
            Pos2::ZERO
        };

        Self {
            nodes,
            center,
            zoom: 1.0,
        }
    }

    /// Add a node snapshot
    pub fn add_node(&mut self, id: impl Into<String>, snapshot: NodeSnapshot) {
        self.nodes.insert(id.into(), snapshot);
    }

    /// Get a node snapshot if it exists
    pub fn get_node(&self, id: &str) -> Option<&NodeSnapshot> {
        self.nodes.get(id)
    }
}

// =============================================================================
// INTERPOLATED NODE
// =============================================================================

/// Result of interpolating a node between two snapshots
#[derive(Debug, Clone, Copy)]
pub struct InterpolatedNode {
    pub position: Pos2,
    pub size: Vec2,
    pub opacity: f32,
}

// =============================================================================
// LAYOUT TRANSITION
// =============================================================================

/// Manages smooth transition between two layouts
///
/// # Usage
/// ```ignore
/// // Capture current layout before change
/// let from = LayoutSnapshot::from_layout_graph(&current_graph);
///
/// // Apply new layout (changes the graph)
/// apply_new_layout(&mut current_graph);
///
/// // Capture target layout
/// let to = LayoutSnapshot::from_layout_graph(&current_graph);
///
/// // Start transition
/// let mut transition = LayoutTransition::new(from, to, 0.4);
///
/// // Each frame:
/// if transition.tick(dt) {
///     let interp = transition.interpolate_node("node-123");
///     // Use interp.position, interp.size, interp.opacity for rendering
/// }
/// ```
#[derive(Debug, Clone)]
pub struct LayoutTransition {
    from: LayoutSnapshot,
    to: LayoutSnapshot,
    /// Progress from 0.0 to 1.0
    progress: f32,
    /// Total duration in seconds
    duration: f32,
    /// Current elapsed time
    elapsed: f32,
    /// Easing function to use
    easing: EasingFn,
    /// Whether the transition is complete
    complete: bool,
}

/// Easing function type
#[derive(Debug, Clone, Copy, Default)]
pub enum EasingFn {
    Linear,
    #[default]
    EaseOutCubic,
    EaseInOutCubic,
    EaseOutQuad,
    EaseOutExpo,
}

impl EasingFn {
    fn apply(&self, t: f32) -> f32 {
        match self {
            EasingFn::Linear => linear(t),
            EasingFn::EaseOutCubic => ease_out_cubic(t),
            EasingFn::EaseInOutCubic => ease_in_out_cubic(t),
            EasingFn::EaseOutQuad => ease_out_quad(t),
            EasingFn::EaseOutExpo => ease_out_expo(t),
        }
    }
}

impl LayoutTransition {
    /// Create a new transition between layouts
    ///
    /// # Arguments
    /// * `from` - Starting layout snapshot
    /// * `to` - Target layout snapshot
    /// * `duration` - Transition duration in seconds (0.3-0.5 typical)
    pub fn new(from: LayoutSnapshot, to: LayoutSnapshot, duration: f32) -> Self {
        Self {
            from,
            to,
            progress: 0.0,
            duration: duration.max(0.01),
            elapsed: 0.0,
            easing: EasingFn::default(),
            complete: false,
        }
    }

    /// Set the easing function
    pub fn with_easing(mut self, easing: EasingFn) -> Self {
        self.easing = easing;
        self
    }

    /// Update transition progress. Returns true if still animating.
    pub fn tick(&mut self, dt: f32) -> bool {
        if self.complete {
            return false;
        }

        self.elapsed += dt;
        let raw_progress = (self.elapsed / self.duration).min(1.0);
        self.progress = self.easing.apply(raw_progress);

        if raw_progress >= 1.0 {
            self.progress = 1.0;
            self.complete = true;
        }

        !self.complete
    }

    /// Get interpolated position/size/opacity for a node
    ///
    /// Handles three cases:
    /// - Node exists in both: interpolate position/size
    /// - Node only in `from`: fade out (opacity decreases)
    /// - Node only in `to`: fade in (opacity increases)
    pub fn interpolate_node(&self, node_id: &str) -> Option<InterpolatedNode> {
        let t = self.progress;

        match (self.from.get_node(node_id), self.to.get_node(node_id)) {
            // Node in both: interpolate everything
            (Some(from), Some(to)) => Some(InterpolatedNode {
                position: lerp_pos2(from.position, to.position, t),
                size: lerp_vec2(from.size, to.size, t),
                opacity: lerp_f32(from.opacity, to.opacity, t),
            }),

            // Node only in from: fading out
            (Some(from), None) => Some(InterpolatedNode {
                position: from.position,
                size: from.size,
                opacity: lerp_f32(from.opacity, 0.0, t),
            }),

            // Node only in to: fading in
            (None, Some(to)) => Some(InterpolatedNode {
                position: to.position,
                size: to.size,
                opacity: lerp_f32(0.0, to.opacity, t),
            }),

            // Node in neither: shouldn't happen but handle gracefully
            (None, None) => None,
        }
    }

    /// Get all node IDs that are part of this transition
    pub fn all_node_ids(&self) -> impl Iterator<Item = &String> {
        self.from
            .nodes
            .keys()
            .chain(self.to.nodes.keys())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
    }

    /// Get interpolated camera center
    pub fn interpolate_center(&self) -> Pos2 {
        lerp_pos2(self.from.center, self.to.center, self.progress)
    }

    /// Get interpolated zoom level
    pub fn interpolate_zoom(&self) -> f32 {
        lerp_f32(self.from.zoom, self.to.zoom, self.progress)
    }

    /// Check if transition is complete
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    /// Get current progress (0.0 to 1.0, after easing)
    pub fn progress(&self) -> f32 {
        self.progress
    }

    /// Skip to end of transition
    pub fn complete_immediately(&mut self) {
        self.elapsed = self.duration;
        self.progress = 1.0;
        self.complete = true;
    }
}

// =============================================================================
// INTERPOLATION HELPERS
// =============================================================================

/// Linear interpolation for f32
#[inline]
pub fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Linear interpolation for Pos2
#[inline]
pub fn lerp_pos2(a: Pos2, b: Pos2, t: f32) -> Pos2 {
    Pos2::new(lerp_f32(a.x, b.x, t), lerp_f32(a.y, b.y, t))
}

/// Linear interpolation for Vec2
#[inline]
pub fn lerp_vec2(a: Vec2, b: Vec2, t: f32) -> Vec2 {
    Vec2::new(lerp_f32(a.x, b.x, t), lerp_f32(a.y, b.y, t))
}

/// Interpolate between two layouts at progress t
///
/// This is a convenience function when you don't need the full
/// LayoutTransition state machine.
pub fn interpolate_layouts(
    from: &LayoutSnapshot,
    to: &LayoutSnapshot,
    t: f32,
) -> HashMap<String, InterpolatedNode> {
    let t = t.clamp(0.0, 1.0);
    let mut result = HashMap::new();

    // Collect all node IDs
    let all_ids: std::collections::HashSet<_> = from.nodes.keys().chain(to.nodes.keys()).collect();

    for id in all_ids {
        let interp = match (from.nodes.get(id), to.nodes.get(id)) {
            (Some(f), Some(t_node)) => InterpolatedNode {
                position: lerp_pos2(f.position, t_node.position, t),
                size: lerp_vec2(f.size, t_node.size, t),
                opacity: lerp_f32(f.opacity, t_node.opacity, t),
            },
            (Some(f), None) => InterpolatedNode {
                position: f.position,
                size: f.size,
                opacity: lerp_f32(f.opacity, 0.0, t),
            },
            (None, Some(t_node)) => InterpolatedNode {
                position: t_node.position,
                size: t_node.size,
                opacity: lerp_f32(0.0, t_node.opacity, t),
            },
            (None, None) => continue,
        };
        result.insert(id.clone(), interp);
    }

    result
}

// =============================================================================
// VIEW MODE TRANSITION HELPERS
// =============================================================================

/// Suggested transition parameters for different view mode changes
#[derive(Debug, Clone, Copy)]
pub struct TransitionParams {
    pub duration: f32,
    pub easing: EasingFn,
}

impl TransitionParams {
    /// Fast transition for minor view adjustments
    pub const QUICK: Self = Self {
        duration: 0.2,
        easing: EasingFn::EaseOutQuad,
    };

    /// Standard transition for view mode changes
    pub const STANDARD: Self = Self {
        duration: 0.35,
        easing: EasingFn::EaseOutCubic,
    };

    /// Slow transition for dramatic changes (drill-down, zoom out)
    pub const DRAMATIC: Self = Self {
        duration: 0.5,
        easing: EasingFn::EaseInOutCubic,
    };

    /// Very fast for user-initiated actions (feels responsive)
    pub const RESPONSIVE: Self = Self {
        duration: 0.15,
        easing: EasingFn::EaseOutExpo,
    };
}

/// Get recommended transition params based on the type of layout change
pub fn suggest_transition_params(
    nodes_added: usize,
    nodes_removed: usize,
    is_drill_down: bool,
    is_zoom_out: bool,
) -> TransitionParams {
    let total_change = nodes_added + nodes_removed;

    if is_drill_down || is_zoom_out {
        TransitionParams::DRAMATIC
    } else if total_change > 10 {
        TransitionParams::STANDARD
    } else if total_change > 0 {
        TransitionParams::QUICK
    } else {
        TransitionParams::RESPONSIVE
    }
}

// =============================================================================
// ESPER CLICK-STEP TRANSITION
// =============================================================================

/// Esper-style stepped transition for enhance level changes
///
/// NOT smooth interpolation - discrete steps with brief hold and scale pulse.
/// Feels like Blade Runner's Esper machine: deliberate, mechanical, precision.
///
/// # Usage
/// ```ignore
/// let mut esper = EsperTransition::new(0, 3); // L0 → L3
///
/// // Each frame:
/// match esper.update(Duration::from_millis(16)) {
///     EsperTransitionState::Stepping { level, scale, .. } => {
///         render_at_enhance_level(level, scale);
///     }
///     EsperTransitionState::Complete { level } => {
///         render_at_enhance_level(level, 1.0);
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct EsperTransition {
    /// Discrete enhance levels to step through [L0, L1, L2, ...]
    steps: Vec<u8>,
    /// Current step index
    current_step: usize,
    /// Time spent at current step (in seconds)
    hold_timer: f32,
    /// How long to hold each step before advancing (seconds)
    hold_duration: f32,
    /// Scale pulse for "click" effect (1.0 = settled, 1.03 = peak)
    scale_pulse: f32,
    /// Whether transition is complete
    complete: bool,
}

impl EsperTransition {
    /// Default hold duration between steps (100ms)
    pub const DEFAULT_HOLD_MS: u64 = 100;

    /// Scale pulse peak (3% bump)
    pub const SCALE_PULSE_PEAK: f32 = 1.03;

    /// Scale settle factor (0.3 = fast settle, ~3-4 frames at 60fps)
    pub const SCALE_SETTLE_FACTOR: f32 = 0.3;

    /// Create transition from one enhance level to another
    ///
    /// Steps through each intermediate level - never skips.
    /// E.g., L0 → L3 steps through: L0 → L1 → L2 → L3
    pub fn new(from_level: u8, to_level: u8) -> Self {
        let steps: Vec<u8> = if from_level <= to_level {
            (from_level..=to_level).collect()
        } else {
            (to_level..=from_level).rev().collect()
        };

        Self {
            steps,
            current_step: 0,
            hold_timer: 0.0,
            hold_duration: Self::DEFAULT_HOLD_MS as f32 / 1000.0,
            scale_pulse: 1.0,
            complete: false,
        }
    }

    /// Create with custom hold duration in milliseconds
    pub fn with_hold_ms(mut self, ms: u64) -> Self {
        self.hold_duration = ms as f32 / 1000.0;
        self
    }

    /// Create with custom hold duration in seconds
    pub fn with_hold_duration(mut self, seconds: f32) -> Self {
        self.hold_duration = seconds;
        self
    }

    /// Update transition state. Call each frame with delta time in seconds.
    /// Returns current state for rendering.
    pub fn update(&mut self, dt: f32) -> EsperTransitionState {
        if self.complete {
            return EsperTransitionState::Complete {
                level: *self.steps.last().unwrap_or(&0),
            };
        }

        self.hold_timer += dt;

        // Time to advance to next step?
        if self.hold_timer >= self.hold_duration && self.current_step < self.steps.len() - 1 {
            self.current_step += 1;
            self.hold_timer = 0.0;
            self.scale_pulse = Self::SCALE_PULSE_PEAK; // The "click" - quick scale bump
        }

        // Check if we've completed all steps
        if self.current_step >= self.steps.len() - 1 && self.hold_timer >= self.hold_duration {
            self.complete = true;
            return EsperTransitionState::Complete {
                level: self.steps[self.current_step],
            };
        }

        // Settle the pulse (ease-out toward 1.0)
        self.scale_pulse = lerp_f32(self.scale_pulse, 1.0, Self::SCALE_SETTLE_FACTOR);

        EsperTransitionState::Stepping {
            level: self.steps[self.current_step],
            scale: self.scale_pulse,
            progress: if self.steps.len() <= 1 {
                1.0
            } else {
                self.current_step as f32 / (self.steps.len() - 1) as f32
            },
        }
    }

    /// Check if transition is complete
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    /// Get current enhance level
    pub fn current_level(&self) -> u8 {
        self.steps.get(self.current_step).copied().unwrap_or(0)
    }

    /// Get target (final) enhance level
    pub fn target_level(&self) -> u8 {
        *self.steps.last().unwrap_or(&0)
    }

    /// Get starting enhance level
    pub fn start_level(&self) -> u8 {
        *self.steps.first().unwrap_or(&0)
    }

    /// Get all steps in this transition
    pub fn steps(&self) -> &[u8] {
        &self.steps
    }

    /// Get number of steps remaining (including current)
    pub fn steps_remaining(&self) -> usize {
        self.steps.len().saturating_sub(self.current_step)
    }

    /// Skip to end immediately
    pub fn complete_immediately(&mut self) {
        self.current_step = self.steps.len().saturating_sub(1);
        self.scale_pulse = 1.0;
        self.complete = true;
    }
}

/// State returned by EsperTransition::update()
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EsperTransitionState {
    /// Currently stepping through levels
    Stepping {
        /// Current enhance level to render
        level: u8,
        /// Scale factor for "click" pulse (1.0 = normal, 1.03 = pulse peak)
        scale: f32,
        /// Overall progress 0.0 to 1.0
        progress: f32,
    },
    /// Transition complete
    Complete {
        /// Final enhance level
        level: u8,
    },
}

impl EsperTransitionState {
    /// Get the current level regardless of state
    pub fn level(&self) -> u8 {
        match self {
            EsperTransitionState::Stepping { level, .. } => *level,
            EsperTransitionState::Complete { level } => *level,
        }
    }

    /// Get the scale factor (1.0 if complete)
    pub fn scale(&self) -> f32 {
        match self {
            EsperTransitionState::Stepping { scale, .. } => *scale,
            EsperTransitionState::Complete { .. } => 1.0,
        }
    }

    /// Check if transition is complete
    pub fn is_complete(&self) -> bool {
        matches!(self, EsperTransitionState::Complete { .. })
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ease_out_cubic() {
        assert!((ease_out_cubic(0.0) - 0.0).abs() < 0.001);
        assert!((ease_out_cubic(1.0) - 1.0).abs() < 0.001);
        // Midpoint should be > 0.5 (fast start)
        assert!(ease_out_cubic(0.5) > 0.5);
    }

    #[test]
    fn test_ease_in_out_cubic() {
        assert!((ease_in_out_cubic(0.0) - 0.0).abs() < 0.001);
        assert!((ease_in_out_cubic(1.0) - 1.0).abs() < 0.001);
        // Midpoint should be exactly 0.5
        assert!((ease_in_out_cubic(0.5) - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_lerp_f32() {
        assert!((lerp_f32(0.0, 100.0, 0.0) - 0.0).abs() < 0.001);
        assert!((lerp_f32(0.0, 100.0, 1.0) - 100.0).abs() < 0.001);
        assert!((lerp_f32(0.0, 100.0, 0.5) - 50.0).abs() < 0.001);
    }

    #[test]
    fn test_layout_transition_basic() {
        let mut from = LayoutSnapshot::new();
        from.add_node(
            "a",
            NodeSnapshot::new(Pos2::new(0.0, 0.0), Vec2::new(100.0, 50.0)),
        );

        let mut to = LayoutSnapshot::new();
        to.add_node(
            "a",
            NodeSnapshot::new(Pos2::new(100.0, 100.0), Vec2::new(100.0, 50.0)),
        );

        let mut transition = LayoutTransition::new(from, to, 1.0);

        // Initial state
        let node = transition.interpolate_node("a").unwrap();
        assert!((node.position.x - 0.0).abs() < 0.001);

        // Advance halfway (with easing, won't be exactly 50%)
        transition.tick(0.5);
        let node = transition.interpolate_node("a").unwrap();
        assert!(node.position.x > 0.0 && node.position.x < 100.0);

        // Complete
        transition.tick(0.5);
        assert!(transition.is_complete());
        let node = transition.interpolate_node("a").unwrap();
        assert!((node.position.x - 100.0).abs() < 0.001);
    }

    #[test]
    fn test_fade_in_out() {
        let mut from = LayoutSnapshot::new();
        from.add_node("leaving", NodeSnapshot::new(Pos2::ZERO, Vec2::splat(50.0)));

        let mut to = LayoutSnapshot::new();
        to.add_node("entering", NodeSnapshot::new(Pos2::ZERO, Vec2::splat(50.0)));

        let transition = LayoutTransition::new(from, to, 1.0).with_easing(EasingFn::Linear);

        // At t=0: leaving should be visible, entering invisible
        let leaving = transition.interpolate_node("leaving").unwrap();
        let entering = transition.interpolate_node("entering").unwrap();
        assert!((leaving.opacity - 1.0).abs() < 0.001);
        assert!((entering.opacity - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_interpolate_layouts_function() {
        let mut from = LayoutSnapshot::new();
        from.add_node("a", NodeSnapshot::new(Pos2::ZERO, Vec2::splat(100.0)));

        let mut to = LayoutSnapshot::new();
        to.add_node(
            "a",
            NodeSnapshot::new(Pos2::new(200.0, 200.0), Vec2::splat(100.0)),
        );

        let result = interpolate_layouts(&from, &to, 0.5);
        let node = result.get("a").unwrap();

        assert!((node.position.x - 100.0).abs() < 0.001);
        assert!((node.position.y - 100.0).abs() < 0.001);
    }

    // =========================================================================
    // ESPER TRANSITION TESTS
    // =========================================================================

    #[test]
    fn test_esper_single_step() {
        let mut t = EsperTransition::new(0, 1);
        assert_eq!(t.current_level(), 0);
        assert_eq!(t.steps(), &[0, 1]);

        // Before hold duration, should still be at step 0
        let state = t.update(0.05); // 50ms
        assert_eq!(state.level(), 0);
        assert!(!t.is_complete());

        // After hold duration, should advance to step 1
        let state = t.update(0.06); // 60ms more = 110ms total
        assert_eq!(state.level(), 1);

        // After another hold, should be complete
        let state = t.update(0.1);
        assert!(state.is_complete());
        assert_eq!(state.level(), 1);
    }

    #[test]
    fn test_esper_multi_step() {
        let t = EsperTransition::new(0, 3);
        assert_eq!(t.steps(), &[0, 1, 2, 3]);
        assert_eq!(t.start_level(), 0);
        assert_eq!(t.target_level(), 3);
        assert_eq!(t.steps_remaining(), 4);
    }

    #[test]
    fn test_esper_multi_step_progression() {
        let mut t = EsperTransition::new(0, 3);

        // Step through all levels
        for expected_level in 0..=3 {
            assert_eq!(t.current_level(), expected_level);
            // Advance past hold duration
            t.update(0.11); // 110ms > 100ms hold
        }
        assert!(t.is_complete());
    }

    #[test]
    fn test_esper_scale_pulse() {
        let mut t = EsperTransition::new(0, 1);

        // First update at step 0, no pulse yet
        let state = t.update(0.05);
        if let EsperTransitionState::Stepping { scale, .. } = state {
            assert!((scale - 1.0).abs() < 0.01); // Should be settled
        }

        // Advance to trigger click (step to level 1)
        let state = t.update(0.06); // Now at 110ms, should advance

        if let EsperTransitionState::Stepping { scale, level, .. } = state {
            assert_eq!(level, 1);
            // Scale should be > 1.0 right after click
            assert!(scale > 1.0, "Scale should pulse up after click: {}", scale);
        } else {
            panic!("Expected Stepping state");
        }
    }

    #[test]
    fn test_esper_reverse_direction() {
        let t = EsperTransition::new(3, 1);
        assert_eq!(t.steps(), &[3, 2, 1]);
        assert_eq!(t.start_level(), 3);
        assert_eq!(t.target_level(), 1);
    }

    #[test]
    fn test_esper_same_level() {
        let mut t = EsperTransition::new(2, 2);
        assert_eq!(t.steps(), &[2]);
        assert_eq!(t.steps_remaining(), 1);

        // Should complete after one hold
        let state = t.update(0.11);
        assert!(state.is_complete());
        assert_eq!(state.level(), 2);
    }

    #[test]
    fn test_esper_complete_immediately() {
        let mut t = EsperTransition::new(0, 5);
        assert!(!t.is_complete());
        assert_eq!(t.current_level(), 0);

        t.complete_immediately();

        assert!(t.is_complete());
        assert_eq!(t.current_level(), 5);
        assert!((t.scale_pulse - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_esper_custom_hold_duration() {
        let mut t = EsperTransition::new(0, 1).with_hold_ms(200);

        // At 150ms, should still be at level 0
        let state = t.update(0.15);
        assert_eq!(state.level(), 0);

        // At 250ms total, should advance to level 1
        let state = t.update(0.1);
        assert_eq!(state.level(), 1);
    }

    #[test]
    fn test_esper_state_accessors() {
        let stepping = EsperTransitionState::Stepping {
            level: 2,
            scale: 1.02,
            progress: 0.5,
        };
        assert_eq!(stepping.level(), 2);
        assert!((stepping.scale() - 1.02).abs() < 0.001);
        assert!(!stepping.is_complete());

        let complete = EsperTransitionState::Complete { level: 3 };
        assert_eq!(complete.level(), 3);
        assert!((complete.scale() - 1.0).abs() < 0.001);
        assert!(complete.is_complete());
    }

    #[test]
    fn test_esper_progress_tracking() {
        let mut t = EsperTransition::new(0, 2); // 3 steps: 0, 1, 2

        // At step 0
        let state = t.update(0.01);
        if let EsperTransitionState::Stepping { progress, .. } = state {
            assert!((progress - 0.0).abs() < 0.01);
        }

        // Advance to step 1
        t.update(0.1);
        let state = t.update(0.01);
        if let EsperTransitionState::Stepping { progress, .. } = state {
            assert!((progress - 0.5).abs() < 0.01);
        }

        // Advance to step 2
        t.update(0.1);
        let state = t.update(0.01);
        if let EsperTransitionState::Stepping { progress, .. } = state {
            assert!((progress - 1.0).abs() < 0.01);
        }
    }
}
