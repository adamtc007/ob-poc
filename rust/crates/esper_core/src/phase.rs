//! Navigation phase - fly-over state for label rendering.
//!
//! During navigation, we skip expensive label rendering to maintain frame rate.
//! The phase tracks how long since the last navigation to determine what to render.

use crate::{DWELL_TICKS, SETTLE_TICKS};
use serde::{Deserialize, Serialize};

/// Navigation phase for render optimization.
///
/// The phase determines the level of detail to render:
/// - **Moving**: User actively navigating. Skip labels, render icons only.
/// - **Settling**: Brief pause after navigation. Start fading in labels.
/// - **Focused**: User has stopped. Render full detail.
///
/// # State Transitions
///
/// ```text
/// ┌─────────────────────────────────────────────────┐
/// │                                                 │
/// │  ┌────────┐    dwell    ┌──────────┐   settle  │
/// │  │ Moving │ ──────────► │ Settling │ ────────► │
/// │  └────────┘             └──────────┘           │
/// │       ▲                      │                 │
/// │       │                      │                 │
/// │       └──────────────────────┘                 │
/// │              any nav verb                      │
/// │                                                │
/// │  ┌─────────┐                                   │
/// │  │ Focused │ ◄─────────────────────────────────┘
/// │  └─────────┘
/// │       │
/// │       │ any nav verb
/// │       ▼
/// │  ┌────────┐
/// │  │ Moving │
/// │  └────────┘
/// └─────────────────────────────────────────────────┘
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum NavigationPhase {
    /// User is actively navigating. Skip expensive rendering.
    #[default]
    Moving,

    /// Brief pause after navigation. Transitioning to full detail.
    Settling,

    /// User has stopped. Render full detail.
    Focused,
}

impl NavigationPhase {
    /// Update phase based on elapsed ticks since last navigation.
    ///
    /// # Arguments
    ///
    /// * `current_tick` - Current frame tick
    /// * `last_nav_tick` - Tick of last navigation verb
    ///
    /// # Returns
    ///
    /// The new phase based on elapsed time.
    pub fn update(current_tick: u64, last_nav_tick: u64) -> Self {
        let elapsed = current_tick.saturating_sub(last_nav_tick);

        if elapsed < DWELL_TICKS {
            NavigationPhase::Moving
        } else if elapsed < DWELL_TICKS + SETTLE_TICKS {
            NavigationPhase::Settling
        } else {
            NavigationPhase::Focused
        }
    }

    /// Check if in a phase where labels should be rendered.
    pub fn should_render_labels(&self) -> bool {
        !matches!(self, NavigationPhase::Moving)
    }

    /// Check if in a phase where full detail should be rendered.
    pub fn should_render_full(&self) -> bool {
        matches!(self, NavigationPhase::Focused)
    }

    /// Get opacity factor for transitional rendering.
    ///
    /// Returns 0.0 during Moving, 0.0-1.0 during Settling, 1.0 when Focused.
    pub fn label_opacity(&self, current_tick: u64, last_nav_tick: u64) -> f32 {
        let elapsed = current_tick.saturating_sub(last_nav_tick);

        if elapsed < DWELL_TICKS {
            0.0
        } else if elapsed < DWELL_TICKS + SETTLE_TICKS {
            let settling_progress = (elapsed - DWELL_TICKS) as f32 / SETTLE_TICKS as f32;
            settling_progress.clamp(0.0, 1.0)
        } else {
            1.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_update_moving() {
        let phase = NavigationPhase::update(10, 5); // 5 ticks elapsed
        assert_eq!(phase, NavigationPhase::Moving);
    }

    #[test]
    fn phase_update_settling() {
        let phase = NavigationPhase::update(DWELL_TICKS + 5, 0);
        assert_eq!(phase, NavigationPhase::Settling);
    }

    #[test]
    fn phase_update_focused() {
        let phase = NavigationPhase::update(DWELL_TICKS + SETTLE_TICKS + 10, 0);
        assert_eq!(phase, NavigationPhase::Focused);
    }

    #[test]
    fn phase_should_render_labels() {
        assert!(!NavigationPhase::Moving.should_render_labels());
        assert!(NavigationPhase::Settling.should_render_labels());
        assert!(NavigationPhase::Focused.should_render_labels());
    }

    #[test]
    fn phase_should_render_full() {
        assert!(!NavigationPhase::Moving.should_render_full());
        assert!(!NavigationPhase::Settling.should_render_full());
        assert!(NavigationPhase::Focused.should_render_full());
    }

    #[test]
    fn phase_label_opacity() {
        let moving = NavigationPhase::Moving.label_opacity(10, 5);
        assert_eq!(moving, 0.0);

        let settling_mid =
            NavigationPhase::Settling.label_opacity(DWELL_TICKS + SETTLE_TICKS / 2, 0);
        assert!(settling_mid > 0.0 && settling_mid < 1.0);

        let focused = NavigationPhase::Focused.label_opacity(DWELL_TICKS + SETTLE_TICKS + 100, 0);
        assert_eq!(focused, 1.0);
    }

    #[test]
    fn phase_default() {
        assert_eq!(NavigationPhase::default(), NavigationPhase::Moving);
    }
}
