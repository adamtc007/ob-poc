//! Spring-based animation system for smooth 60fps transitions
//!
//! Uses critically-damped spring physics for natural motion.
//! All animations converge smoothly without overshoot (unless configured for bounce).
//!
//! # EGUI-RULES Compliance
//! - Animation state is UI-only (not server data)
//! - No callbacks - values are polled each frame via `get()`
//! - Call `tick(dt)` at start of update(), then render with `get()` values

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
        Self::MEDIUM
    }
}

impl SpringConfig {
    /// Fast, snappy animation (UI button responses, quick zooms)
    pub const FAST: Self = Self {
        stiffness: 300.0,
        damping: 1.0,
    };

    /// Medium animation (camera moves, node transitions)
    pub const MEDIUM: Self = Self {
        stiffness: 150.0,
        damping: 1.0,
    };

    /// Slow, cinematic animation (view transitions, drill-down)
    pub const SLOW: Self = Self {
        stiffness: 80.0,
        damping: 1.0,
    };

    /// Bouncy animation (attention-grabbing, playful)
    pub const BOUNCY: Self = Self {
        stiffness: 200.0,
        damping: 0.6,
    };

    /// Very fast for immediate feedback
    pub const INSTANT: Self = Self {
        stiffness: 500.0,
        damping: 1.0,
    };

    // =========================================================================
    // GALAXY NAVIGATION SPRING CONFIGS (Phase 8 Polish)
    // =========================================================================

    /// Snappy response for UI elements and node interactions
    /// High stiffness, higher damping to prevent overshoot
    pub const SNAPPY: Self = Self {
        stiffness: 300.0,
        damping: 1.25, // Slightly overdamped for crisp feel
    };

    /// Organic, natural-feeling motion for node expansions
    /// Lower damping allows subtle overshoot for lifelike feel
    pub const ORGANIC: Self = Self {
        stiffness: 180.0,
        damping: 0.85,
    };

    /// Gentle transitions for level changes and deep navigation
    /// Slower, more contemplative motion
    pub const GENTLE: Self = Self {
        stiffness: 120.0,
        damping: 1.0,
    };

    /// Camera-specific spring for smooth fly-to animations
    /// Balanced for smooth tracking without lag
    pub const CAMERA: Self = Self {
        stiffness: 150.0,
        damping: 1.1, // Slightly overdamped to avoid camera jitter
    };

    /// Agent speech and UI overlay animations
    /// Quick fade-in, holds, gentle fade-out
    pub const AGENT_UI: Self = Self {
        stiffness: 200.0,
        damping: 1.0,
    };

    /// Autopilot camera following
    /// Slightly looser to allow for anticipatory camera lead
    pub const AUTOPILOT: Self = Self {
        stiffness: 120.0,
        damping: 0.95,
    };

    /// Anomaly pulse animation (continuous, subtle)
    /// Low stiffness for slow, organic pulse
    pub const PULSE: Self = Self {
        stiffness: 60.0,
        damping: 0.8,
    };
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
    /// Create with initial value and default (MEDIUM) spring config
    pub fn new(initial: f32) -> Self {
        Self::with_config(initial, SpringConfig::MEDIUM)
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
        let mut spring = SpringF32::with_config(0.0, SpringConfig::BOUNCY);
        spring.set_target(1.0);

        let mut max_value = 0.0f32;

        // Bouncy springs should overshoot
        for _ in 0..60 {
            spring.tick(1.0 / 60.0);
            max_value = max_value.max(spring.get());
        }

        assert!(max_value > 1.0, "Bouncy spring should overshoot target");
    }
}
