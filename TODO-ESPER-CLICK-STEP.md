# TODO: Implement Esper Click-Step Transitions

> **Priority:** HIGH - Core UX identity
> **Location:** `rust/crates/ob-poc-ui/src/view/transition.rs`
> **Status:** ✅ COMPLETE - Implemented 2025-01-08

## Problem

Current `LayoutTransition` uses smooth easing (ease_out_cubic, etc.) for all transitions. This feels "floaty" and generic.

We need **Esper-style click-step transitions** for enhance level changes - discrete, mechanical, like a slide carousel clicking into place.

## What to Implement

### 1. Add EsperTransition struct

```rust
use std::time::Duration;

/// Esper-style stepped transition for enhance level changes
/// 
/// NOT smooth interpolation - discrete steps with brief hold and scale pulse.
/// Feels like Blade Runner's Esper machine: deliberate, mechanical, precision.
pub struct EsperTransition {
    /// Discrete enhance levels to step through [L0, L1, L2]
    steps: Vec<u8>,
    /// Current step index
    current_step: usize,
    /// Time spent at current step
    hold_timer: Duration,
    /// How long to hold each step before advancing (100ms typical)
    hold_duration: Duration,
    /// Scale pulse for "click" effect (1.0 = settled, 1.03 = peak)
    scale_pulse: f32,
    /// Whether transition is complete
    complete: bool,
}

impl EsperTransition {
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
            hold_timer: Duration::ZERO,
            hold_duration: Duration::from_millis(100),
            scale_pulse: 1.0,
            complete: false,
        }
    }
    
    /// Create with custom hold duration
    pub fn with_hold_duration(mut self, duration: Duration) -> Self {
        self.hold_duration = duration;
        self
    }
    
    /// Update transition state. Call each frame with delta time.
    /// Returns current state for rendering.
    pub fn update(&mut self, dt: Duration) -> EsperTransitionState {
        if self.complete {
            return EsperTransitionState::Complete {
                level: *self.steps.last().unwrap_or(&0),
            };
        }
        
        self.hold_timer += dt;
        
        // Time to advance to next step?
        if self.hold_timer >= self.hold_duration && self.current_step < self.steps.len() - 1 {
            self.current_step += 1;
            self.hold_timer = Duration::ZERO;
            self.scale_pulse = 1.03; // The "click" - quick scale bump
        }
        
        // Check if we've completed all steps
        if self.current_step >= self.steps.len() - 1 && self.hold_timer >= self.hold_duration {
            self.complete = true;
        }
        
        // Settle the pulse (ease-out toward 1.0)
        // 0.3 factor = fast settle (~3-4 frames at 60fps)
        self.scale_pulse = lerp_f32(self.scale_pulse, 1.0, 0.3);
        
        EsperTransitionState::Stepping {
            level: self.steps[self.current_step],
            scale: self.scale_pulse,
            progress: self.current_step as f32 / (self.steps.len() - 1).max(1) as f32,
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
    
    /// Skip to end immediately
    pub fn complete_immediately(&mut self) {
        self.current_step = self.steps.len().saturating_sub(1);
        self.scale_pulse = 1.0;
        self.complete = true;
    }
}

/// State returned by EsperTransition::update()
#[derive(Debug, Clone, Copy)]
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
```

### 2. Update Transition Usage

**Where to use EsperTransition (discrete steps):**
- `VIEWPORT.enhance(+)` / `VIEWPORT.enhance(-)`
- `VIEWPORT.enhance(n)` / `VIEWPORT.enhance(max)` / `VIEWPORT.enhance(reset)`
- `VIEWPORT.ascend()` / `VIEWPORT.descend()`

**Where to keep smooth LayoutTransition:**
- Camera pan/zoom (200ms lerp)
- View switch (snap, no transition needed)
- `VIEWPORT.fit()` (camera reset)

### 3. Integration Point

In the render loop or wherever enhance level changes are applied:

```rust
// When VIEWPORT.enhance(+) is executed:
if let Some(ref mut esper) = self.active_esper_transition {
    let state = esper.update(dt);
    match state {
        EsperTransitionState::Stepping { level, scale, .. } => {
            // Render at this enhance level with scale pulse
            self.render_at_enhance_level(level, scale);
        }
        EsperTransitionState::Complete { level } => {
            self.render_at_enhance_level(level, 1.0);
            self.active_esper_transition = None;
        }
    }
} else {
    // No transition active - render current state
    self.render_at_enhance_level(self.current_enhance_level, 1.0);
}
```

### 4. What This Looks Like to the User

```
User: VIEWPORT.enhance(+) from L0 to L2

Frame 0-5:    L0 visible (100ms hold)
Frame 6:      [CLICK] L1 appears, scale bumps to 1.03x
Frame 7-11:   L1 visible, scale settles to 1.0
Frame 12:     [CLICK] L2 appears, scale bumps to 1.03x
Frame 13-17:  L2 visible, scale settles to 1.0
Frame 18:     Complete

Total: ~300ms for 2-level enhance
User perceives: Deliberate, mechanical, precision zoom
```

### 5. Tests to Add

```rust
#[cfg(test)]
mod esper_tests {
    use super::*;
    
    #[test]
    fn test_single_step() {
        let mut t = EsperTransition::new(0, 1);
        assert_eq!(t.current_level(), 0);
        
        // After hold duration, should advance
        let state = t.update(Duration::from_millis(100));
        assert!(matches!(state, EsperTransitionState::Stepping { level: 1, .. }));
    }
    
    #[test]
    fn test_multi_step() {
        let mut t = EsperTransition::new(0, 3);
        assert_eq!(t.steps, vec![0, 1, 2, 3]);
        
        // Step through all levels
        for expected_level in 0..=3 {
            assert_eq!(t.current_level(), expected_level);
            t.update(Duration::from_millis(100));
        }
        assert!(t.is_complete());
    }
    
    #[test]
    fn test_scale_pulse() {
        let mut t = EsperTransition::new(0, 1);
        
        // Advance to trigger click
        let state = t.update(Duration::from_millis(100));
        
        if let EsperTransitionState::Stepping { scale, .. } = state {
            // Scale should be > 1.0 right after click
            assert!(scale > 1.0);
        }
    }
    
    #[test]
    fn test_reverse_direction() {
        let t = EsperTransition::new(3, 1);
        assert_eq!(t.steps, vec![3, 2, 1]);
    }
}
```

## DO NOT Change

- Keep `LayoutTransition` for camera/layout interpolation
- Keep easing functions (may be useful elsewhere)
- Keep `TransitionParams` presets

## Files to Modify

1. `rust/crates/ob-poc-ui/src/view/transition.rs` - Add EsperTransition
2. `rust/crates/ob-poc-ui/src/view/mod.rs` - Export EsperTransition
3. Wherever enhance level changes trigger rendering - integrate EsperTransition

## Acceptance Criteria

- [x] `EsperTransition` struct implemented as above
- [x] Tests pass for single-step, multi-step, reverse direction
- [x] Scale pulse visible on enhance level change (1.03x → 1.0 settle)
- [x] 100ms hold between steps feels deliberate, not rushed
- [x] Camera pan/zoom still uses smooth interpolation (unchanged)

## Implementation Notes

Completed in `rust/crates/ob-poc-ui/src/view/transition.rs`:
- `EsperTransition` struct with f32-based timing (dt in seconds)
- `EsperTransitionState` enum with `Stepping` and `Complete` variants
- 10 comprehensive tests all passing
- Exported from `view/mod.rs`
