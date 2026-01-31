//! Navigation verbs - the complete command vocabulary.
//!
//! Verbs are the atomic units of navigation. All input (mouse, keyboard, voice,
//! agent) is translated into verbs before execution.

use serde::{Deserialize, Serialize};

/// Type alias for entity IDs (matches snapshot).
pub type EntityId = u64;

/// Type alias for door IDs.
pub type DoorId = u32;

/// Type alias for chamber IDs.
pub type ChamberId = u32;

/// Type alias for node indices within a chamber.
pub type NodeIdx = u32;

/// Navigation verb - a single atomic navigation command.
///
/// Verbs are organized into categories:
/// - **Spatial**: Camera and viewport manipulation
/// - **Structural**: Tree/hierarchy navigation
/// - **Cross-chamber**: Door-based navigation between chambers
/// - **Selection**: Entity selection and preview
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum Verb {
    // =========================================================================
    // SPATIAL NAVIGATION (Camera/Viewport)
    // =========================================================================
    /// Pan camera by delta.
    PanBy { dx: f32, dy: f32 },

    /// Pan camera to absolute position.
    PanTo { x: f32, y: f32 },

    /// Zoom by factor (>1 = zoom in, <1 = zoom out).
    Zoom(f32),

    /// Zoom to fit all content.
    ZoomFit,

    /// Zoom to specific level.
    ZoomTo(f32),

    /// Center view on current selection.
    Center,

    /// Stop all camera animation.
    Stop,

    /// Increase detail level (semantic zoom in).
    Enhance,

    /// Decrease detail level (semantic zoom out).
    Reduce,

    // =========================================================================
    // CROSS-CHAMBER NAVIGATION (Doors)
    // =========================================================================
    /// Enter through a door to another chamber.
    DiveInto(DoorId),

    /// Exit current chamber (pop context stack).
    PullBack,

    /// Return to root chamber.
    Surface,

    // =========================================================================
    // STRUCTURAL NAVIGATION (Tree/Hierarchy)
    // =========================================================================
    /// Move to parent node.
    Ascend,

    /// Move to first child of current node.
    Descend,

    /// Move to specific child node.
    DescendTo(NodeIdx),

    /// Move to next sibling.
    Next,

    /// Move to previous sibling.
    Prev,

    /// Move to first sibling.
    First,

    /// Move to last sibling.
    Last,

    /// Expand current node (show children).
    Expand,

    /// Collapse current node (hide children).
    Collapse,

    /// Return to root of taxonomy.
    Root,

    // =========================================================================
    // SELECTION & PREVIEW
    // =========================================================================
    /// Select a node (confirm selection).
    Select(NodeIdx),

    /// Focus on an entity (center camera, highlight).
    Focus(EntityId),

    /// Track an entity (keep centered during animation).
    Track(EntityId),

    /// Preview a node (hover state, temporary focus).
    Preview(NodeIdx),

    /// Clear preview state.
    ClearPreview,

    // =========================================================================
    // MODE SWITCHING
    // =========================================================================
    /// Switch to spatial navigation mode.
    ModeSpatial,

    /// Switch to structural navigation mode.
    ModeStructural,

    /// Toggle between spatial and structural modes.
    ModeToggle,

    // =========================================================================
    // SPECIAL
    // =========================================================================
    /// No operation (useful for input mapping that produces no action).
    #[default]
    Noop,
}

impl Verb {
    /// Check if this verb affects the camera.
    pub fn affects_camera(&self) -> bool {
        matches!(
            self,
            Verb::PanBy { .. }
                | Verb::PanTo { .. }
                | Verb::Zoom(_)
                | Verb::ZoomFit
                | Verb::ZoomTo(_)
                | Verb::Center
                | Verb::Focus(_)
                | Verb::Track(_)
        )
    }

    /// Check if this verb affects the taxonomy state.
    pub fn affects_taxonomy(&self) -> bool {
        matches!(
            self,
            Verb::Ascend
                | Verb::Descend
                | Verb::DescendTo(_)
                | Verb::Next
                | Verb::Prev
                | Verb::First
                | Verb::Last
                | Verb::Expand
                | Verb::Collapse
                | Verb::Root
                | Verb::Select(_)
                | Verb::Preview(_)
                | Verb::ClearPreview
        )
    }

    /// Check if this verb changes chambers.
    pub fn changes_chamber(&self) -> bool {
        matches!(self, Verb::DiveInto(_) | Verb::PullBack | Verb::Surface)
    }

    /// Convert to DSL-style string representation.
    pub fn to_dsl(&self) -> String {
        match self {
            Verb::PanBy { dx, dy } => format!("(pan-by :dx {} :dy {})", dx, dy),
            Verb::PanTo { x, y } => format!("(pan-to :x {} :y {})", x, y),
            Verb::Zoom(f) => format!("(zoom {})", f),
            Verb::ZoomFit => "(zoom-fit)".to_string(),
            Verb::ZoomTo(level) => format!("(zoom-to {})", level),
            Verb::Center => "(center)".to_string(),
            Verb::Stop => "(stop)".to_string(),
            Verb::Enhance => "(enhance)".to_string(),
            Verb::Reduce => "(reduce)".to_string(),
            Verb::DiveInto(door) => format!("(dive-into :door {})", door),
            Verb::PullBack => "(pull-back)".to_string(),
            Verb::Surface => "(surface)".to_string(),
            Verb::Ascend => "(ascend)".to_string(),
            Verb::Descend => "(descend)".to_string(),
            Verb::DescendTo(node) => format!("(descend-to :node {})", node),
            Verb::Next => "(next)".to_string(),
            Verb::Prev => "(prev)".to_string(),
            Verb::First => "(first)".to_string(),
            Verb::Last => "(last)".to_string(),
            Verb::Expand => "(expand)".to_string(),
            Verb::Collapse => "(collapse)".to_string(),
            Verb::Root => "(root)".to_string(),
            Verb::Select(node) => format!("(select :node {})", node),
            Verb::Focus(entity) => format!("(focus :entity {})", entity),
            Verb::Track(entity) => format!("(track :entity {})", entity),
            Verb::Preview(node) => format!("(preview :node {})", node),
            Verb::ClearPreview => "(clear-preview)".to_string(),
            Verb::ModeSpatial => "(mode-spatial)".to_string(),
            Verb::ModeStructural => "(mode-structural)".to_string(),
            Verb::ModeToggle => "(mode-toggle)".to_string(),
            Verb::Noop => "(noop)".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verb_affects_camera() {
        assert!(Verb::PanBy { dx: 1.0, dy: 0.0 }.affects_camera());
        assert!(Verb::Zoom(1.5).affects_camera());
        assert!(Verb::Focus(123).affects_camera());
        assert!(!Verb::Next.affects_camera());
        assert!(!Verb::Expand.affects_camera());
    }

    #[test]
    fn verb_affects_taxonomy() {
        assert!(Verb::Next.affects_taxonomy());
        assert!(Verb::Ascend.affects_taxonomy());
        assert!(Verb::Select(0).affects_taxonomy());
        assert!(!Verb::Zoom(1.0).affects_taxonomy());
        assert!(!Verb::PanBy { dx: 0.0, dy: 0.0 }.affects_taxonomy());
    }

    #[test]
    fn verb_changes_chamber() {
        assert!(Verb::DiveInto(1).changes_chamber());
        assert!(Verb::PullBack.changes_chamber());
        assert!(Verb::Surface.changes_chamber());
        assert!(!Verb::Next.changes_chamber());
        assert!(!Verb::Zoom(1.0).changes_chamber());
    }

    #[test]
    fn verb_to_dsl() {
        assert_eq!(Verb::Next.to_dsl(), "(next)");
        assert_eq!(Verb::Zoom(1.5).to_dsl(), "(zoom 1.5)");
        assert_eq!(
            Verb::PanBy { dx: 10.0, dy: 20.0 }.to_dsl(),
            "(pan-by :dx 10 :dy 20)"
        );
    }

    #[test]
    fn verb_default() {
        assert_eq!(Verb::default(), Verb::Noop);
    }
}
