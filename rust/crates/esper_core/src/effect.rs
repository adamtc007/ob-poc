//! Effect flags - the output protocol from verb execution.
//!
//! When a verb is executed, it returns an EffectSet indicating what changed.
//! The caller (renderer, UI) then handles each effect appropriately.

use bitflags::bitflags;

bitflags! {
    /// Set of effects produced by verb execution.
    ///
    /// Effects are additive - a single verb can produce multiple effects.
    /// The caller checks which effects are set and handles each one.
    ///
    /// # Example
    ///
    /// ```
    /// use esper_core::EffectSet;
    ///
    /// let effects = EffectSet::CAMERA_CHANGED | EffectSet::PHASE_RESET;
    ///
    /// if effects.contains(EffectSet::CAMERA_CHANGED) {
    ///     // Start camera animation
    /// }
    /// if effects.contains(EffectSet::PHASE_RESET) {
    ///     // Reset navigation phase to Moving
    /// }
    /// ```
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct EffectSet: u16 {
        /// No effects.
        const NONE = 0;

        // =====================================================================
        // CAMERA EFFECTS
        // =====================================================================

        /// Camera target position or zoom changed.
        /// Renderer should start camera animation.
        const CAMERA_CHANGED = 1 << 0;

        /// Camera should snap instantly (no animation).
        const SNAP_TRANSITION = 1 << 1;

        // =====================================================================
        // LOD EFFECTS
        // =====================================================================

        /// LOD mode was reset (e.g., after chamber change).
        const LOD_MODE_RESET = 1 << 2;

        // =====================================================================
        // CHAMBER EFFECTS
        // =====================================================================

        /// Active chamber changed.
        /// Renderer needs to load new chamber data.
        const CHAMBER_CHANGED = 1 << 3;

        /// Context was pushed onto stack.
        const CONTEXT_PUSHED = 1 << 4;

        /// Context was popped from stack.
        const CONTEXT_POPPED = 1 << 5;

        // =====================================================================
        // MODE EFFECTS
        // =====================================================================

        /// Navigation mode changed (Spatial ↔ Structural).
        const MODE_CHANGED = 1 << 6;

        // =====================================================================
        // TAXONOMY EFFECTS
        // =====================================================================

        /// Taxonomy selection or focus changed.
        const TAXONOMY_CHANGED = 1 << 7;

        /// Scroll position should be adjusted.
        const SCROLL_ADJUST = 1 << 8;

        // =====================================================================
        // PHASE EFFECTS
        // =====================================================================

        /// Navigation phase should reset to Moving.
        const PHASE_RESET = 1 << 9;

        // =====================================================================
        // PREVIEW EFFECTS
        // =====================================================================

        /// Preview target was set.
        const PREVIEW_SET = 1 << 10;

        /// Preview target was cleared.
        const PREVIEW_CLEAR = 1 << 11;

        // =====================================================================
        // DATA EFFECTS
        // =====================================================================

        /// Details should be prefetched for focused entity.
        const PREFETCH_DETAILS = 1 << 12;
    }
}

impl Default for EffectSet {
    fn default() -> Self {
        EffectSet::NONE
    }
}

impl EffectSet {
    /// Check if any camera-related effects are set.
    pub fn has_camera_effects(&self) -> bool {
        self.intersects(EffectSet::CAMERA_CHANGED | EffectSet::SNAP_TRANSITION)
    }

    /// Check if any taxonomy-related effects are set.
    pub fn has_taxonomy_effects(&self) -> bool {
        self.intersects(EffectSet::TAXONOMY_CHANGED | EffectSet::SCROLL_ADJUST)
    }

    /// Check if the chamber changed.
    pub fn chamber_changed(&self) -> bool {
        self.contains(EffectSet::CHAMBER_CHANGED)
    }

    /// Check if animation should be skipped (snap transition).
    pub fn should_snap(&self) -> bool {
        self.contains(EffectSet::SNAP_TRANSITION)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_set_default() {
        assert_eq!(EffectSet::default(), EffectSet::NONE);
    }

    #[test]
    fn effect_set_combine() {
        let effects = EffectSet::CAMERA_CHANGED | EffectSet::PHASE_RESET;

        assert!(effects.contains(EffectSet::CAMERA_CHANGED));
        assert!(effects.contains(EffectSet::PHASE_RESET));
        assert!(!effects.contains(EffectSet::CHAMBER_CHANGED));
    }

    #[test]
    fn effect_set_helpers() {
        let camera_effects = EffectSet::CAMERA_CHANGED | EffectSet::SNAP_TRANSITION;
        assert!(camera_effects.has_camera_effects());
        assert!(!camera_effects.has_taxonomy_effects());

        let taxonomy_effects = EffectSet::TAXONOMY_CHANGED | EffectSet::SCROLL_ADJUST;
        assert!(taxonomy_effects.has_taxonomy_effects());
        assert!(!taxonomy_effects.has_camera_effects());
    }

    #[test]
    fn effect_set_snap() {
        let snap = EffectSet::CAMERA_CHANGED | EffectSet::SNAP_TRANSITION;
        assert!(snap.should_snap());

        let no_snap = EffectSet::CAMERA_CHANGED;
        assert!(!no_snap.should_snap());
    }
}
