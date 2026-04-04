//! ObservatoryAction — all UI actions returned from the canvas.
//!
//! Rule: the canvas returns Option<ObservatoryAction>. Never callbacks.
//! The app loop processes actions — the canvas doesn't know what happens next.
//! All actions are serialized to JSON and forwarded to React via the on_action callback.

use ob_poc_types::galaxy::ViewLevel;
use serde::Serialize;

/// All possible canvas actions.
/// Semantic actions require React to trigger a server round-trip.
/// Observation actions are handled locally by the canvas app.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ObservatoryAction {
    // ── Semantic (forwarded to React for server round-trip) ────
    /// Drill into a node — opens deeper level.
    Drill {
        node_id: String,
        target_level: ViewLevel,
    },
    /// Semantic zoom out — go up one level.
    SemanticZoomOut,
    /// Navigate to a history entry.
    NavigateHistory { index: usize },
    /// Invoke a maintenance or navigation verb.
    InvokeVerb { verb_fqn: String },

    // ── Observation frame (local only, no server) ─────────────
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
}
