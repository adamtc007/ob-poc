//! Spring-based animation system for smooth 60fps transitions
//!
//! Uses critically-damped spring physics for natural motion.
//! All animations converge smoothly without overshoot (unless configured for bounce).
//!
//! # EGUI-RULES Compliance
//! - Animation state is UI-only (not server data)
//! - No callbacks - values are polled each frame via `get()`
//! - Call `tick(dt)` at start of update(), then render with `get()` values
//!
//! # Config-Driven Spring Presets
//!
//! Spring presets are loaded from `config/graph_settings.yaml` via `global_config()`.
//! Use `SpringConfig::from_preset("camera")` to get config-driven values.
//!
//! Available presets: fast, medium, slow, bouncy, instant, snappy, organic,
//! gentle, camera, agent_ui, autopilot, pulse

use crate::config::{global_config, SpringConfigYaml};

/// Spring configuration parameters
#[derive(Debug, Clone, Copy)]
pub struct SpringConfig {
    /// Stiffness (higher = faster response). Typical: 80-300
    pub stiffness: f32,
    /// Damping ratio: 1.0 = critically damped (no overshoot)
    /// < 1.0 = underdamped (bouncy), > 1.0 = overdamped (sluggish)
    pub damping: f32,
}

impl Default for SpringConfig {
    fn default() -> Self {
        Self::from_preset("medium")
    }
}

impl From<SpringConfigYaml> for SpringConfig {
    fn from(yaml: SpringConfigYaml) -> Self {
        Self {
            stiffness: yaml.stiffness,
            damping: yaml.damping,
        }
    }
}

impl SpringConfig {
    /// Load spring config from YAML preset by name.
    ///
    /// Falls back to reasonable defaults if preset not found.
    ///
    /// # Available presets
    /// - `fast`: UI button responses, quick zooms
    /// - `medium`: Camera moves, node transitions
    /// - `slow`: View transitions, drill-down
    /// - `bouncy`: Attention-grabbing, playful
    /// - `instant`: Immediate feedback
    /// - `snappy`: UI elements, crisp feel
    /// - `organic`: Natural motion with subtle overshoot
    /// - `gentle`: Level changes, contemplative motion
    /// - `camera`: Smooth fly-to animations
    /// - `agent_ui`: Agent speech overlays
    /// - `autopilot`: Camera following
    /// - `pulse`: Anomaly pulse animation
    pub fn from_preset(name: &str) -> Self {
        global_config().animation.spring(name).into()
    }
}

// =============================================================================
// SPRING F32
// =============================================================================

/// Animated f32 value with spring physics
///
/// # Usage
/// ```ignore
/// let mut zoom = SpringF32::new(1.0);
/// zoom.set_target(2.0);  // Start animating toward 2.0
///
/// // Each frame:
/// zoom.tick(dt);
/// let current = zoom.get();  // Smoothly interpolates
/// ```
#[derive(Debug, Clone)]
pub struct SpringF32 {
    current: f32,
    target: f32,
    velocity: f32,
    config: SpringConfig,
}

impl SpringF32 {
    /// Create with initial value and default (medium) spring config
    pub fn new(initial: f32) -> Self {
        Self::with_config(initial, SpringConfig::from_preset("medium"))
    }

    /// Create with initial value and custom spring config
    pub fn with_config(initial: f32, config: SpringConfig) -> Self {
        Self {
            current: initial,
            target: initial,
            velocity: 0.0,
            config,
        }
    }

    /// Set new target value (animation begins)
    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    /// Get current target value
    pub fn target(&self) -> f32 {
        self.target
    }

    /// Jump immediately to value (no animation)
    pub fn set_immediate(&mut self, value: f32) {
        self.current = value;
        self.target = value;
        self.velocity = 0.0;
    }

    /// Update animation state (call each frame with delta time in seconds)
    ///
    /// Uses critically-damped spring physics:
    /// F = -k*x - c*v
    /// where k = stiffness, c = damping * 2 * sqrt(k), x = displacement, v = velocity
    pub fn tick(&mut self, dt: f32) {
        // Clamp dt to prevent instability with large time steps
        let dt = dt.min(0.1);

        let displacement = self.current - self.target;
        let spring_force = -self.config.stiffness * displacement;
        let damping_force =
            -self.config.damping * 2.0 * self.config.stiffness.sqrt() * self.velocity;
        let acceleration = spring_force + damping_force;

        self.velocity += acceleration * dt;
        self.current += self.velocity * dt;

        // Snap to target if close enough (prevents micro-oscillation)
        if (self.current - self.target).abs() < 0.0001 && self.velocity.abs() < 0.001 {
            self.current = self.target;
            self.velocity = 0.0;
        }
    }

    /// Get current animated value
    pub fn get(&self) -> f32 {
        self.current
    }

    /// Check if animation is still in progress
    pub fn is_animating(&self) -> bool {
        (self.current - self.target).abs() > 0.0001 || self.velocity.abs() > 0.001
    }

    /// Update spring configuration
    pub fn set_config(&mut self, config: SpringConfig) {
        self.config = config;
    }
}

// =============================================================================
// SPRING VEC2
// =============================================================================

/// Animated 2D vector with spring physics (for positions, offsets)
#[derive(Debug, Clone)]
pub struct SpringVec2 {
    pub x: SpringF32,
    pub y: SpringF32,
}

impl SpringVec2 {
    /// Create with initial position and default spring config
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            x: SpringF32::new(x),
            y: SpringF32::new(y),
        }
    }

    /// Create with initial position and custom spring config
    pub fn with_config(x: f32, y: f32, config: SpringConfig) -> Self {
        Self {
            x: SpringF32::with_config(x, config),
            y: SpringF32::with_config(y, config),
        }
    }

    /// Create from egui::Pos2
    pub fn from_pos2(pos: egui::Pos2) -> Self {
        Self::new(pos.x, pos.y)
    }

    /// Set new target position
    pub fn set_target(&mut self, x: f32, y: f32) {
        self.x.set_target(x);
        self.y.set_target(y);
    }

    /// Set target from egui::Pos2
    pub fn set_target_pos2(&mut self, pos: egui::Pos2) {
        self.set_target(pos.x, pos.y);
    }

    /// Get current target
    pub fn target(&self) -> (f32, f32) {
        (self.x.target(), self.y.target())
    }

    /// Jump immediately to position (no animation)
    pub fn set_immediate(&mut self, x: f32, y: f32) {
        self.x.set_immediate(x);
        self.y.set_immediate(y);
    }

    /// Update animation state
    pub fn tick(&mut self, dt: f32) {
        self.x.tick(dt);
        self.y.tick(dt);
    }

    /// Get current animated position as tuple
    pub fn get(&self) -> (f32, f32) {
        (self.x.get(), self.y.get())
    }

    /// Get current animated position as egui::Pos2
    pub fn get_pos2(&self) -> egui::Pos2 {
        egui::Pos2::new(self.x.get(), self.y.get())
    }

    /// Check if animation is still in progress
    pub fn is_animating(&self) -> bool {
        self.x.is_animating() || self.y.is_animating()
    }

    /// Update spring configuration for both axes
    pub fn set_config(&mut self, config: SpringConfig) {
        self.x.set_config(config);
        self.y.set_config(config);
    }
}

// =============================================================================
// TESTS
// =============================================================================

// =============================================================================
// ESPER TRANSITION (Discrete Stepped Enhance Level Changes)
// =============================================================================

/// Esper-style stepped transition for enhance level changes
///
/// NOT smooth interpolation - discrete steps with brief hold and scale pulse.
/// Feels like Blade Runner's Esper machine: deliberate, mechanical, precision.
///
/// # Config-Driven Parameters
///
/// Parameters are loaded from `config/graph_settings.yaml`:
/// - `animation.default_hold_ms`: Hold duration between steps
/// - `animation.scale_pulse_peak`: Scale bump on step change
/// - `animation.scale_settle_factor`: How fast the pulse settles
///
/// # EGUI-RULES Compliance
/// - Call `update(dt)` at start of frame (in update loop, not render)
/// - Read state via `current_level()`, `scale()` for rendering
/// - No callbacks - poll state each frame
///
/// # Usage
/// ```ignore
/// let mut esper = EsperTransition::new(0, 3); // L0 → L3
///
/// // Each frame in update():
/// match esper.update(dt) {
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
    /// Scale pulse for "click" effect (1.0 = settled, peak from config)
    scale_pulse: f32,
    /// Scale pulse peak value (from config)
    scale_pulse_peak: f32,
    /// Scale settle factor (from config)
    scale_settle_factor: f32,
    /// Whether transition is complete
    complete: bool,
}

impl EsperTransition {
    /// Default hold duration between steps (100ms) - deprecated, use config
    #[deprecated(
        since = "0.1.0",
        note = "use global_config().animation.default_hold_ms"
    )]
    pub const DEFAULT_HOLD_MS: u64 = 100;

    /// Scale pulse peak (3% bump) - deprecated, use config
    #[deprecated(
        since = "0.1.0",
        note = "use global_config().animation.scale_pulse_peak"
    )]
    pub const SCALE_PULSE_PEAK: f32 = 1.03;

    /// Scale settle factor - deprecated, use config
    #[deprecated(
        since = "0.1.0",
        note = "use global_config().animation.scale_settle_factor"
    )]
    pub const SCALE_SETTLE_FACTOR: f32 = 0.3;

    /// Get default hold duration from config (in milliseconds)
    pub fn default_hold_ms() -> u64 {
        global_config().animation.default_hold_ms
    }

    /// Get scale pulse peak from config
    pub fn scale_pulse_peak_config() -> f32 {
        global_config().animation.scale_pulse_peak
    }

    /// Get scale settle factor from config
    pub fn scale_settle_factor_config() -> f32 {
        global_config().animation.scale_settle_factor
    }

    /// Create transition from one enhance level to another
    ///
    /// Steps through each intermediate level - never skips.
    /// E.g., L0 → L3 steps through: L0 → L1 → L2 → L3
    ///
    /// Uses config-driven default hold duration from `graph_settings.yaml`.
    pub fn new(from_level: u8, to_level: u8) -> Self {
        let steps: Vec<u8> = if from_level <= to_level {
            (from_level..=to_level).collect()
        } else {
            (to_level..=from_level).rev().collect()
        };

        let anim_cfg = &global_config().animation;

        Self {
            steps,
            current_step: 0,
            hold_timer: 0.0,
            hold_duration: anim_cfg.default_hold_ms as f32 / 1000.0,
            scale_pulse: 1.0,
            scale_pulse_peak: anim_cfg.scale_pulse_peak,
            scale_settle_factor: anim_cfg.scale_settle_factor,
            complete: false,
        }
    }

    /// Create with custom hold duration in milliseconds
    pub fn with_hold_ms(mut self, ms: u64) -> Self {
        self.hold_duration = ms as f32 / 1000.0;
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
            self.scale_pulse = self.scale_pulse_peak; // The "click" - quick scale bump
        }

        // Check if we've completed all steps
        if self.current_step >= self.steps.len() - 1 && self.hold_timer >= self.hold_duration {
            self.complete = true;
            return EsperTransitionState::Complete {
                level: self.steps[self.current_step],
            };
        }

        // Settle the pulse (lerp toward 1.0)
        self.scale_pulse = self.scale_pulse + (1.0 - self.scale_pulse) * self.scale_settle_factor;

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

    /// Get current scale factor
    pub fn scale(&self) -> f32 {
        if self.complete {
            1.0
        } else {
            self.scale_pulse
        }
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
    fn test_spring_converges() {
        let mut spring = SpringF32::new(0.0);
        spring.set_target(1.0);

        // Simulate 2 seconds at 60fps
        for _ in 0..120 {
            spring.tick(1.0 / 60.0);
        }

        assert!(
            (spring.get() - 1.0).abs() < 0.01,
            "Spring should converge to target"
        );
        assert!(!spring.is_animating(), "Spring should stop animating");
    }

    #[test]
    fn test_spring_immediate() {
        let mut spring = SpringF32::new(0.0);
        spring.set_immediate(5.0);

        assert_eq!(spring.get(), 5.0);
        assert!(!spring.is_animating());
    }

    #[test]
    fn test_spring_vec2() {
        let mut pos = SpringVec2::new(0.0, 0.0);
        pos.set_target(100.0, 50.0);

        // Simulate
        for _ in 0..120 {
            pos.tick(1.0 / 60.0);
        }

        let (x, y) = pos.get();
        assert!((x - 100.0).abs() < 0.1);
        assert!((y - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_bouncy_spring() {
        let mut spring = SpringF32::with_config(0.0, SpringConfig::from_preset("bouncy"));
        spring.set_target(1.0);

        let mut max_value = 0.0f32;

        // Bouncy springs should overshoot
        for _ in 0..60 {
            spring.tick(1.0 / 60.0);
            max_value = max_value.max(spring.get());
        }

        assert!(max_value > 1.0, "Bouncy spring should overshoot target");
    }

    // =========================================================================
    // ESPER TRANSITION TESTS
    // =========================================================================

    #[test]
    fn test_esper_transition_steps_through_levels() {
        let mut esper = EsperTransition::new(0, 3);

        // Should start at level 0
        assert_eq!(esper.current_level(), 0);
        assert!(!esper.is_complete());

        // Step through with enough time to advance
        let dt = 0.15; // 150ms > 100ms hold

        let state = esper.update(dt);
        assert!(matches!(
            state,
            EsperTransitionState::Stepping { level: 1, .. }
        ));

        let state = esper.update(dt);
        assert!(matches!(
            state,
            EsperTransitionState::Stepping { level: 2, .. }
        ));

        let state = esper.update(dt);
        assert!(matches!(
            state,
            EsperTransitionState::Stepping { level: 3, .. }
        ));

        // One more should complete
        let state = esper.update(dt);
        assert!(matches!(state, EsperTransitionState::Complete { level: 3 }));
        assert!(esper.is_complete());
    }

    #[test]
    fn test_esper_transition_reverse() {
        let mut esper = EsperTransition::new(3, 0);

        // Should step down: 3 → 2 → 1 → 0
        assert_eq!(esper.current_level(), 3);

        let dt = 0.15;
        esper.update(dt);
        assert_eq!(esper.current_level(), 2);

        esper.update(dt);
        assert_eq!(esper.current_level(), 1);

        esper.update(dt);
        assert_eq!(esper.current_level(), 0);
    }

    #[test]
    fn test_esper_transition_scale_pulse() {
        let mut esper = EsperTransition::new(0, 2);

        // Initial scale should be 1.0
        assert!((esper.scale() - 1.0).abs() < 0.001);

        // After advancing, should get a scale pulse
        esper.update(0.15); // Advance to level 1
        let scale = esper.scale();

        // Scale should be between 1.0 and 1.03 (settling from pulse)
        assert!(scale >= 1.0 && scale <= 1.03);
    }

    #[test]
    fn test_esper_transition_complete_immediately() {
        let mut esper = EsperTransition::new(0, 4);

        assert!(!esper.is_complete());
        esper.complete_immediately();

        assert!(esper.is_complete());
        assert_eq!(esper.current_level(), 4);
        assert!((esper.scale() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_esper_transition_single_level() {
        let mut esper = EsperTransition::new(2, 2);

        // Same start and end = already complete after one update
        let state = esper.update(0.15);
        assert!(matches!(state, EsperTransitionState::Complete { level: 2 }));
    }

    #[test]
    fn test_esper_transition_state_accessors() {
        let stepping = EsperTransitionState::Stepping {
            level: 2,
            scale: 1.02,
            progress: 0.5,
        };
        assert_eq!(stepping.level(), 2);
        assert!((stepping.scale() - 1.02).abs() < 0.001);
        assert!(!stepping.is_complete());

        let complete = EsperTransitionState::Complete { level: 4 };
        assert_eq!(complete.level(), 4);
        assert!((complete.scale() - 1.0).abs() < 0.001);
        assert!(complete.is_complete());
    }
}
