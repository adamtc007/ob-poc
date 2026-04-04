//! ObservatoryAction — all UI actions returned from panels/canvas.
//!
//! Rule: panels return Option<ObservatoryAction>. Never callbacks.
//! The app loop processes actions — panels don't know what happens next.

use ob_poc_types::galaxy::ViewLevel;

/// All possible UI actions.
/// Semantic actions require server round-trip.
/// Observation actions are local-only.
#[derive(Debug, Clone)]
pub enum ObservatoryAction {
    // ── Semantic (server round-trip required) ─────────────────
    /// Drill into a node — opens deeper level. Server returns new orientation + scene.
    Drill { node_id: String, target_level: ViewLevel },
    /// Semantic zoom out — go up one level. NOT visual zoom.
    SemanticZoomOut,
    /// Navigate to a history entry (server-authored OrientationContract).
    NavigateHistory { index: usize },
    /// Invoke a maintenance or navigation verb.
    InvokeVerb { verb_fqn: String },
    /// Change lens (overlay, depth probe, cluster mode).
    SetLens { overlay_draft: Option<String> },
    /// Refresh all data from server.
    RefreshData,

    // ── Observation frame (local only, no server) ────────────
    /// Visual zoom — camera scale only. NOT semantic.
    VisualZoom { delta: f32 },
    /// Pan camera.
    Pan { dx: f32, dy: f32 },
    /// Select a node (visual indication only — NOT semantic focus).
    SelectNode { node_id: String },
    /// Clear selection.
    DeselectNode,
    /// Anchor camera to a node.
    AnchorNode { node_id: String },
    /// Clear anchor.
    ClearAnchor,
    /// Reset camera to default position.
    ResetView,

    // ── UI mode ──────────────────────────────────────────────
    /// Switch tab (Observe / Mission Control).
    SwitchTab { tab: Tab },
}

/// Active tab in the observatory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Observe,
    MissionControl,
}
