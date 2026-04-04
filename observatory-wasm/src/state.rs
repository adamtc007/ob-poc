//! ObservatoryState — root application state.
//!
//! Strict three-layer separation:
//! - Semantic input: server-authored, read-only during render
//! - Observation frame: client-owned camera/view state, no semantic meaning
//! - Render cache: derived from semantic input + observation frame, invalidated on change

use ob_poc_types::galaxy::ViewLevel;
use ob_poc_types::graph_scene::GraphSceneModel;

use crate::actions::Tab;

// ── Semantic Input (server-authored) ─────────────────────────

/// Orientation contract — the canonical answer to "where am I?"
/// Deserialized from server JSON. Never modified by egui.
pub type OrientationContract = serde_json::Value;

/// ShowPacket with viewports. Deserialized from server JSON.
pub type ShowPacket = serde_json::Value;

/// Health metrics for Mission Control.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct HealthMetrics {
    pub pending_changesets: i64,
    pub stale_dryruns: i64,
    pub active_snapshots: i64,
    pub archived_changesets: i64,
    pub embedding_freshness_hours: Option<f64>,
    pub outbox_depth: Option<i64>,
}

// ── Async Fetch State ────────────────────────────────────────

/// Generic async slot: tracks fetch lifecycle.
#[derive(Debug, Clone)]
pub enum AsyncSlot<T> {
    Empty,
    Pending,
    Ready(T),
    Error(String),
}

impl<T> Default for AsyncSlot<T> {
    fn default() -> Self {
        Self::Empty
    }
}

impl<T> AsyncSlot<T> {
    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending)
    }

    pub fn as_ready(&self) -> Option<&T> {
        match self {
            Self::Ready(v) => Some(v),
            _ => None,
        }
    }
}

/// All in-flight or completed fetches.
#[derive(Default)]
pub struct FetchState {
    pub orientation: AsyncSlot<OrientationContract>,
    pub show_packet: AsyncSlot<ShowPacket>,
    pub graph_scene: AsyncSlot<GraphSceneModel>,
    pub health: AsyncSlot<HealthMetrics>,
}

// ── Observation Frame (client-owned) ─────────────────────────

/// Client-owned camera state. NO semantic meaning.
/// Zoom, pan, anchor do NOT change the semantic struct.
/// Uses spring interpolation: current values lerp toward target values.
#[derive(Debug, Clone)]
pub struct ObservationFrame {
    pub zoom: f32,
    pub pan_x: f32,
    pub pan_y: f32,
    pub target_zoom: f32,
    pub target_pan_x: f32,
    pub target_pan_y: f32,
    pub anchor_node_id: Option<String>,
    pub focus_lock_node_id: Option<String>,
}

impl Default for ObservationFrame {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            target_zoom: 1.0,
            target_pan_x: 0.0,
            target_pan_y: 0.0,
            anchor_node_id: None,
            focus_lock_node_id: None,
        }
    }
}

impl ObservationFrame {
    /// Whether the camera is still interpolating toward its target.
    pub fn is_animating(&self) -> bool {
        let eps = 0.01;
        (self.zoom - self.target_zoom).abs() > eps
            || (self.pan_x - self.target_pan_x).abs() > eps
            || (self.pan_y - self.target_pan_y).abs() > eps
    }
}

// ── Interaction State (ephemeral) ────────────────────────────

/// Per-frame interaction state. NOT semantic focus.
#[derive(Debug, Clone, Default)]
pub struct InteractionState {
    /// Hovered node (visual only).
    pub hovered_node: Option<String>,
    /// Selected node (visual only — NOT semantic focus).
    pub selected_node: Option<String>,
}

// ── Render Cache ─────────────────────────────────────────────

/// Cached layout data. Invalidated when scene generation changes.
#[derive(Debug, Clone, Default)]
pub struct SceneCache {
    /// Generation counter of the GraphSceneModel this cache was built from.
    pub source_generation: u64,
    /// Computed node positions (node_id → (x, y)).
    pub node_positions: Vec<(String, f32, f32)>,
    /// Whether the cache needs rebuilding.
    pub dirty: bool,
}

impl SceneCache {
    /// Check if cache is valid for the given scene generation.
    pub fn is_valid_for(&self, generation: u64) -> bool {
        !self.dirty && self.source_generation == generation
    }

    /// Mark cache as needing rebuild.
    pub fn invalidate(&mut self) {
        self.dirty = true;
    }
}

// ── Root State ───────────────────────────────────────────────

/// Root application state. Owned by the eframe::App.
/// Three-layer separation enforced by field grouping.
#[derive(Default)]
pub struct ObservatoryState {
    // ── Semantic input (server-authored) ──
    pub fetch: FetchState,
    pub navigation_history: Vec<OrientationContract>,

    // ── Async fetch mailbox (ehttp callbacks write here) ──
    pub mailbox: crate::fetch::FetchMailbox,

    // ── Observation frame (client-owned) ──
    pub camera: ObservationFrame,

    // ── Interaction (ephemeral) ──
    pub interaction: InteractionState,

    // ── Render cache ──
    pub scene_cache: SceneCache,

    // ── UI mode ──
    pub active_tab: Tab,

    // ── Session identity ──
    pub session_id: String,
    pub base_url: String,

    // ── View level (derived from orientation, cached for quick access) ──
    pub current_level: ViewLevel,
}
