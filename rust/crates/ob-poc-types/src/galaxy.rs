//! Galaxy Navigation Types
//!
//! Shared types for galaxy navigation between server and client.
//! These are DATA CONTRACTS only - no behavior, no state, no egui.
//!
//! ## Design Principles
//!
//! 1. **Pos2 not Vec3** - Camera2D uses egui::Pos2, we use (f32, f32) for transport
//! 2. **String IDs** - UUIDs as strings for JSON compatibility
//! 3. **No egui dependency** - This crate is pure data, usable by server
//! 4. **Derive-heavy** - Serialize, Deserialize, Clone, Debug for all types

use serde::{Deserialize, Serialize};

// ============================================================================
// NAVIGATION SCOPE (client-side equivalent of server's GraphScope)
// ============================================================================

/// What scope is currently being viewed
/// This is the client-side equivalent of server's GraphScope
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NavigationScope {
    /// Full universe - all CBUs clustered
    #[default]
    Universe,
    /// A specific book (commercial client's CBUs)
    Book {
        apex_entity_id: String,
        apex_name: String,
    },
    /// A cluster within the universe (e.g., jurisdiction, client type)
    Cluster {
        cluster_id: String,
        cluster_type: ClusterType,
    },
    /// Single CBU with entities
    Cbu { cbu_id: String, cbu_name: String },
    /// Single entity focused
    Entity {
        entity_id: String,
        entity_name: String,
        cbu_id: String,
    },
    /// Deep dive - ownership chains, derived data
    Deep {
        entity_id: String,
        depth_type: DepthType,
    },
}

// ============================================================================
// VIEW LEVEL (astronomical metaphor)
// ============================================================================

/// Discrete navigation levels (astronomical metaphor)
/// Each level has different data density and rendering style
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ViewLevel {
    /// Full universe - clusters of CBUs
    #[default]
    Universe,
    /// Galaxy/Cluster - expanded cluster showing CBU nodes
    Cluster,
    /// Solar system - single CBU with entity satellites
    System,
    /// Planet - single entity with relationships
    Planet,
    /// Surface - entity details and attributes
    Surface,
    /// Core - derived data, indirect ownership, deep analysis
    Core,
}

impl ViewLevel {
    /// Get zoom range for this level (min, max)
    pub fn zoom_range(&self) -> (f32, f32) {
        match self {
            ViewLevel::Universe => (0.05, 0.3),
            ViewLevel::Cluster => (0.2, 0.6),
            ViewLevel::System => (0.5, 1.5),
            ViewLevel::Planet => (1.0, 3.0),
            ViewLevel::Surface => (2.0, 5.0),
            ViewLevel::Core => (3.0, 10.0),
        }
    }

    /// Get the parent level (zoom out)
    pub fn parent(&self) -> Option<ViewLevel> {
        match self {
            ViewLevel::Universe => None,
            ViewLevel::Cluster => Some(ViewLevel::Universe),
            ViewLevel::System => Some(ViewLevel::Cluster),
            ViewLevel::Planet => Some(ViewLevel::System),
            ViewLevel::Surface => Some(ViewLevel::Planet),
            ViewLevel::Core => Some(ViewLevel::Surface),
        }
    }

    /// Get the child level (zoom in)
    pub fn child(&self) -> Option<ViewLevel> {
        match self {
            ViewLevel::Universe => Some(ViewLevel::Cluster),
            ViewLevel::Cluster => Some(ViewLevel::System),
            ViewLevel::System => Some(ViewLevel::Planet),
            ViewLevel::Planet => Some(ViewLevel::Surface),
            ViewLevel::Surface => Some(ViewLevel::Core),
            ViewLevel::Core => None,
        }
    }
}

// ============================================================================
// CLUSTER TYPES
// ============================================================================

/// How CBUs are clustered in universe view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ClusterType {
    /// By jurisdiction (LU, IE, US, etc.)
    #[default]
    Jurisdiction,
    /// By commercial client (Allianz, BlackRock, etc.)
    Client,
    /// By risk rating (HIGH, MEDIUM, LOW)
    Risk,
    /// By product type (FUND, CORPORATE, etc.)
    Product,
}

/// Type of deep dive
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DepthType {
    /// Ownership chain to natural persons
    #[default]
    Ownership,
    /// Control chain (board, voting rights)
    Control,
    /// Service/product dependencies
    Services,
    /// Document/evidence trail
    Documents,
}

// ============================================================================
// UNIVERSE GRAPH (server response for universe view)
// ============================================================================

/// Full universe graph - clusters of CBUs
/// Returned by GET /api/universe
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniverseGraph {
    /// All clusters in the universe
    pub clusters: Vec<ClusterNode>,
    /// Edges between clusters (shared entities, ManCos, etc.)
    pub cluster_edges: Vec<ClusterEdge>,
    /// Summary statistics
    pub stats: UniverseStats,
    /// Current cluster type used for grouping
    pub cluster_type: ClusterType,
}

/// A cluster of CBUs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterNode {
    /// Unique cluster ID (e.g., "jurisdiction:LU", "client:allianz")
    pub id: String,
    /// Display label
    pub label: String,
    /// Cluster type
    pub cluster_type: ClusterType,
    /// Number of CBUs in this cluster
    pub cbu_count: i32,
    /// Number of entities across all CBUs
    pub entity_count: i32,
    /// Aggregate risk summary
    pub risk_summary: RiskSummary,
    /// Position hint from server (x, y) - client may adjust
    #[serde(default)]
    pub position: Option<(f32, f32)>,
    /// Visual radius hint
    #[serde(default)]
    pub radius: Option<f32>,
    /// Anomalies/issues in this cluster
    #[serde(default)]
    pub anomalies: Vec<Anomaly>,
    /// Whether cluster is expanded in UI
    #[serde(default)]
    pub is_expanded: bool,
}

/// Edge between clusters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterEdge {
    /// Source cluster ID
    pub source: String,
    /// Target cluster ID
    pub target: String,
    /// Connection type
    pub edge_type: ClusterEdgeType,
    /// Weight (number of shared entities, etc.)
    pub weight: f32,
    /// Optional label
    #[serde(default)]
    pub label: Option<String>,
}

/// Types of cluster connections
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterEdgeType {
    /// Shared ManCo across clusters
    SharedManco,
    /// Shared Investment Manager
    SharedIm,
    /// Cross-jurisdiction ownership
    CrossOwnership,
    /// Service provider relationship
    ServiceProvider,
}

/// Risk summary for a cluster or CBU
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RiskSummary {
    /// Number of high-risk items
    pub high: i32,
    /// Number of medium-risk items
    pub medium: i32,
    /// Number of low-risk items
    pub low: i32,
    /// Number of unrated items
    pub unrated: i32,
}

impl RiskSummary {
    pub fn total(&self) -> i32 {
        self.high + self.medium + self.low + self.unrated
    }

    pub fn dominant_rating(&self) -> RiskRating {
        if self.high > 0 {
            RiskRating::High
        } else if self.medium > 0 {
            RiskRating::Medium
        } else if self.low > 0 {
            RiskRating::Low
        } else {
            RiskRating::Unrated
        }
    }
}

/// Universe statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UniverseStats {
    pub total_cbus: i32,
    pub total_entities: i32,
    pub total_clusters: i32,
    pub high_risk_count: i32,
    pub pending_kyc_count: i32,
    pub anomaly_count: i32,
}

// ============================================================================
// CLUSTER DETAIL (expanded cluster showing CBUs)
// ============================================================================

/// Expanded cluster with CBU nodes
/// Returned when drilling into a cluster
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterDetailGraph {
    /// The cluster being viewed
    pub cluster: ClusterNode,
    /// CBU nodes within this cluster
    pub cbus: Vec<CbuNode>,
    /// Edges between CBUs (shared entities)
    pub cbu_edges: Vec<CbuEdge>,
    /// Shared entities visible at this level
    #[serde(default)]
    pub shared_entities: Vec<SharedEntityNode>,
}

/// A CBU node within a cluster
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuNode {
    /// CBU UUID
    pub id: String,
    /// Display name
    pub name: String,
    /// Jurisdiction code
    #[serde(default)]
    pub jurisdiction: Option<String>,
    /// Client type
    #[serde(default)]
    pub client_type: Option<String>,
    /// Entity count
    pub entity_count: i32,
    /// Risk rating
    pub risk_rating: RiskRating,
    /// KYC status
    #[serde(default)]
    pub kyc_status: Option<String>,
    /// Position hint (x, y)
    #[serde(default)]
    pub position: Option<(f32, f32)>,
    /// Anomalies on this CBU
    #[serde(default)]
    pub anomalies: Vec<Anomaly>,
    /// Parent cluster ID (for back-navigation)
    #[serde(default)]
    pub parent_cluster_id: Option<String>,
}

/// Edge between CBUs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuEdge {
    /// Source CBU ID
    pub source: String,
    /// Target CBU ID
    pub target: String,
    /// Edge type
    pub edge_type: String,
    /// Shared entity IDs
    #[serde(default)]
    pub shared_entity_ids: Vec<String>,
}

/// Shared entity visible at cluster level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedEntityNode {
    /// Entity UUID
    pub id: String,
    /// Display name
    pub name: String,
    /// Entity type
    pub entity_type: String,
    /// CBU IDs this entity appears in
    pub cbu_ids: Vec<String>,
    /// Role in each CBU (simplified)
    #[serde(default)]
    pub roles: Vec<String>,
}

// ============================================================================
// ENUMS
// ============================================================================

/// Risk rating levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RiskRating {
    High,
    Medium,
    Low,
    #[default]
    Unrated,
}

/// Severity of an anomaly
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AnomalySeverity {
    Critical,
    High,
    #[default]
    Medium,
    Low,
    Info,
}

// ============================================================================
// ANOMALIES / BADGES
// ============================================================================

/// An anomaly or issue requiring attention
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anomaly {
    /// Unique ID
    pub id: String,
    /// Anomaly type code
    pub anomaly_type: String,
    /// Severity
    pub severity: AnomalySeverity,
    /// Short description
    pub message: String,
    /// Affected entity ID (if applicable)
    #[serde(default)]
    pub entity_id: Option<String>,
    /// Suggested action
    #[serde(default)]
    pub suggested_action: Option<String>,
}

/// Badge for quick visual indicators
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Badge {
    /// Badge type code
    pub badge_type: String,
    /// Display label
    pub label: String,
    /// Color hint (CSS color or named color)
    #[serde(default)]
    pub color: Option<String>,
    /// Tooltip
    #[serde(default)]
    pub tooltip: Option<String>,
}

// ============================================================================
// NAVIGATION ACTIONS (returned from UI, processed by app)
// ============================================================================

/// Actions returned from galaxy UI widgets
/// These are processed in app.rs update(), NOT in the widget
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum NavigationAction {
    /// Fly camera to position
    FlyTo { x: f32, y: f32 },
    /// Zoom to level
    ZoomTo { level: f32 },
    /// Zoom in by factor
    ZoomIn { factor: Option<f32> },
    /// Zoom out by factor
    ZoomOut { factor: Option<f32> },
    /// Zoom to fit bounds
    ZoomFit,
    /// Pan by delta
    Pan { dx: f32, dy: f32 },
    /// Center view
    Center,
    /// Drill into a cluster
    DrillIntoCluster { cluster_id: String },
    /// Drill into a CBU
    DrillIntoCbu { cbu_id: String },
    /// Drill into an entity
    DrillIntoEntity { entity_id: String },
    /// Go up one level
    DrillUp,
    /// Go to universe view
    GoToUniverse,
    /// Go to specific breadcrumb index
    GoToBreadcrumb { index: usize },
    /// Select a node (for details panel)
    Select { node_id: String, node_type: String },
    /// Clear selection
    Deselect,
    /// Hover start (for preview)
    Hover { node_id: String, node_type: String },
    /// Hover end
    ClearHover,
    /// Request data fetch
    FetchData { scope: NavigationScope },
    /// Request prefetch for a scope
    Prefetch { scope_id: String },
    /// Change cluster type
    SetClusterType { cluster_type: ClusterType },
}

// ============================================================================
// AGENT SUGGESTIONS
// ============================================================================

/// Agent suggestion for navigation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSuggestion {
    /// Suggestion type
    pub suggestion_type: SuggestionType,
    /// Human-readable label
    pub label: String,
    /// Detailed description
    #[serde(default)]
    pub description: Option<String>,
    /// Action to take if accepted
    pub action: NavigationAction,
    /// Confidence score (0.0 - 1.0)
    #[serde(default)]
    pub confidence: Option<f32>,
}

/// Types of agent suggestions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionType {
    /// Suggested navigation path
    Path,
    /// Suggested filter
    Filter,
    /// Suggested expansion
    Expand,
    /// Anomaly to investigate
    Investigate,
    /// Comparison suggestion
    Compare,
}

// ============================================================================
// PREFETCH
// ============================================================================

/// Status of prefetched data
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PrefetchStatus {
    #[default]
    NotStarted,
    /// Queued for prefetch
    Queued,
    Loading,
    Ready,
    Failed,
}

// ============================================================================
// VIEW TRANSITION (for animated level changes)
// ============================================================================

/// A view transition captures the state of an animated navigation
///
/// This is used for smooth fly-through between levels. The camera leads
/// (arrives before content loads), and depth encoding shifts during transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewTransition {
    /// Where we're coming from
    pub from_level: ViewLevel,
    pub from_scope: NavigationScope,
    /// Where we're going to
    pub to_level: ViewLevel,
    pub to_scope: NavigationScope,
    /// Animation progress 0.0 (start) to 1.0 (complete)
    pub progress: f32,
    /// Duration in seconds
    pub duration: f32,
    /// Elapsed time
    pub elapsed: f32,
    /// Camera path control points (for Bezier curves)
    pub camera_path: CameraPath,
    /// Whether camera has arrived (leads content by ~30%)
    pub camera_arrived: bool,
    /// Whether content is loaded and ready
    pub content_ready: bool,
}

impl Default for ViewTransition {
    fn default() -> Self {
        Self {
            from_level: ViewLevel::Universe,
            from_scope: NavigationScope::Universe,
            to_level: ViewLevel::Universe,
            to_scope: NavigationScope::Universe,
            progress: 1.0, // Complete by default (no transition)
            duration: 0.6,
            elapsed: 0.0,
            camera_path: CameraPath::Direct {
                from: (0.0, 0.0),
                to: (0.0, 0.0),
            },
            camera_arrived: true,
            content_ready: true,
        }
    }
}

impl ViewTransition {
    /// Create a new transition between levels
    pub fn new(
        from_level: ViewLevel,
        from_scope: NavigationScope,
        to_level: ViewLevel,
        to_scope: NavigationScope,
        from_pos: (f32, f32),
        to_pos: (f32, f32),
    ) -> Self {
        // Calculate duration based on level change magnitude
        let level_diff = Self::level_distance(&from_level, &to_level);
        let duration = 0.4 + (level_diff as f32 * 0.15); // Base 0.4s + 0.15s per level

        Self {
            from_level,
            from_scope,
            to_level,
            to_scope,
            progress: 0.0,
            duration,
            elapsed: 0.0,
            camera_path: CameraPath::Direct {
                from: from_pos,
                to: to_pos,
            },
            camera_arrived: false,
            content_ready: false,
        }
    }

    /// Check if transition is complete
    pub fn is_complete(&self) -> bool {
        self.progress >= 1.0
    }

    /// Check if in progress
    pub fn is_active(&self) -> bool {
        self.progress > 0.0 && self.progress < 1.0
    }

    /// Get interpolated camera position
    pub fn camera_position(&self) -> (f32, f32) {
        self.camera_path.position_at(self.camera_progress())
    }

    /// Camera progress leads content progress by 30%
    pub fn camera_progress(&self) -> f32 {
        // Camera is 30% ahead of content, clamped to 1.0
        (self.progress * 1.3).min(1.0)
    }

    /// Get interpolated depth factor for background color
    pub fn depth_factor(&self) -> f32 {
        let from_depth = Self::level_to_depth(&self.from_level);
        let to_depth = Self::level_to_depth(&self.to_level);
        let t = Self::ease_out_cubic(self.progress);
        from_depth + (to_depth - from_depth) * t
    }

    /// Convert view level to depth value (0.0 = Universe, 1.0 = Core)
    fn level_to_depth(level: &ViewLevel) -> f32 {
        match level {
            ViewLevel::Universe => 0.0,
            ViewLevel::Cluster => 0.2,
            ViewLevel::System => 0.4,
            ViewLevel::Planet => 0.6,
            ViewLevel::Surface => 0.8,
            ViewLevel::Core => 1.0,
        }
    }

    /// Calculate distance between levels (for duration)
    fn level_distance(from: &ViewLevel, to: &ViewLevel) -> u8 {
        let from_idx = Self::level_index(from);
        let to_idx = Self::level_index(to);
        from_idx.abs_diff(to_idx)
    }

    fn level_index(level: &ViewLevel) -> u8 {
        match level {
            ViewLevel::Universe => 0,
            ViewLevel::Cluster => 1,
            ViewLevel::System => 2,
            ViewLevel::Planet => 3,
            ViewLevel::Surface => 4,
            ViewLevel::Core => 5,
        }
    }

    /// Ease-out cubic for smooth deceleration
    fn ease_out_cubic(t: f32) -> f32 {
        1.0 - (1.0 - t).powi(3)
    }
}

/// Camera path for transitions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CameraPath {
    /// Direct line from A to B
    Direct { from: (f32, f32), to: (f32, f32) },
    /// Bezier curve with control points
    Bezier {
        from: (f32, f32),
        control1: (f32, f32),
        control2: (f32, f32),
        to: (f32, f32),
    },
    /// Multi-point path (for autopilot)
    Waypoints { points: Vec<(f32, f32)> },
}

impl CameraPath {
    /// Get position at progress t (0.0 - 1.0)
    pub fn position_at(&self, t: f32) -> (f32, f32) {
        let t = t.clamp(0.0, 1.0);
        match self {
            CameraPath::Direct { from, to } => {
                let x = from.0 + (to.0 - from.0) * t;
                let y = from.1 + (to.1 - from.1) * t;
                (x, y)
            }
            CameraPath::Bezier {
                from,
                control1,
                control2,
                to,
            } => {
                // Cubic Bezier: B(t) = (1-t)³P0 + 3(1-t)²tP1 + 3(1-t)t²P2 + t³P3
                let u = 1.0 - t;
                let tt = t * t;
                let uu = u * u;
                let uuu = uu * u;
                let ttt = tt * t;

                let x = uuu * from.0
                    + 3.0 * uu * t * control1.0
                    + 3.0 * u * tt * control2.0
                    + ttt * to.0;
                let y = uuu * from.1
                    + 3.0 * uu * t * control1.1
                    + 3.0 * u * tt * control2.1
                    + ttt * to.1;
                (x, y)
            }
            CameraPath::Waypoints { points } => {
                if points.is_empty() {
                    return (0.0, 0.0);
                }
                if points.len() == 1 {
                    return points[0];
                }

                // Find which segment we're in
                let segment_count = points.len() - 1;
                let segment_f = t * segment_count as f32;
                let segment_idx = (segment_f.floor() as usize).min(segment_count - 1);
                let segment_t = segment_f - segment_idx as f32;

                let from = points[segment_idx];
                let to = points[segment_idx + 1];
                let x = from.0 + (to.0 - from.0) * segment_t;
                let y = from.1 + (to.1 - from.1) * segment_t;
                (x, y)
            }
        }
    }
}

// ============================================================================
// DEPTH ENCODING COLORS (for background shifts)
// ============================================================================

/// Background colors for each depth level (RGB values 0-255)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DepthColors {
    /// Universe view - deep space (darkest)
    pub universe: (u8, u8, u8),
    /// Cluster view
    pub cluster: (u8, u8, u8),
    /// System/CBU view
    pub system: (u8, u8, u8),
    /// Planet/Entity view
    pub planet: (u8, u8, u8),
    /// Surface view
    pub surface: (u8, u8, u8),
    /// Core view - intimate (lightest dark)
    pub core: (u8, u8, u8),
}

impl Default for DepthColors {
    fn default() -> Self {
        Self {
            universe: (15, 23, 42), // Deep space blue-black
            cluster: (20, 30, 50),  // Slightly lighter
            system: (25, 35, 55),   // CBU level
            planet: (30, 40, 60),   // Entity level
            surface: (35, 45, 65),  // Detail level
            core: (40, 50, 70),     // Deepest dive
        }
    }
}

impl DepthColors {
    /// Get interpolated color for a depth factor (0.0 = Universe, 1.0 = Core)
    pub fn color_at(&self, depth: f32) -> (u8, u8, u8) {
        let depth = depth.clamp(0.0, 1.0);

        // Map depth to color stops
        let (from_color, to_color, t) = if depth < 0.2 {
            (self.universe, self.cluster, depth / 0.2)
        } else if depth < 0.4 {
            (self.cluster, self.system, (depth - 0.2) / 0.2)
        } else if depth < 0.6 {
            (self.system, self.planet, (depth - 0.4) / 0.2)
        } else if depth < 0.8 {
            (self.planet, self.surface, (depth - 0.6) / 0.2)
        } else {
            (self.surface, self.core, (depth - 0.8) / 0.2)
        };

        let r = (from_color.0 as f32 + (to_color.0 as f32 - from_color.0 as f32) * t) as u8;
        let g = (from_color.1 as f32 + (to_color.1 as f32 - from_color.1 as f32) * t) as u8;
        let b = (from_color.2 as f32 + (to_color.2 as f32 - from_color.2 as f32) * t) as u8;

        (r, g, b)
    }
}

// ============================================================================
// PREVIEW DATA (for fork presentation - Phase 3)
// ============================================================================

/// Preview data returned when hovering at decision points
///
/// This enables "branches present themselves" - when user loiters at a fork,
/// we show lightweight previews of what's down each branch.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PreviewData {
    /// Node ID this preview is for
    pub node_id: String,
    /// Preview items (branches or children)
    pub items: Vec<PreviewItem>,
    /// Whether the preview data is complete
    pub complete: bool,
    /// Error message if preview failed to load
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// A single preview item (one potential navigation branch)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewItem {
    /// Unique ID for this branch/child
    pub id: String,
    /// Display label
    pub label: String,
    /// Type of preview
    pub preview_type: PreviewType,
    /// Optional count (e.g., "12 entities")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    /// Optional risk indicator
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk: Option<RiskRating>,
    /// Brief description or status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Visual hint for rendering (icon key, color hex, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visual_hint: Option<String>,
    /// Navigation action if selected
    pub action: NavigationAction,
}

/// Type of preview item
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewType {
    /// A cluster of CBUs
    Cluster,
    /// A single CBU
    Cbu,
    /// An entity within a CBU
    Entity,
    /// A document
    Document,
    /// A KYC case or workstream
    Workflow,
    /// A product or service
    Product,
    /// An anomaly or issue
    Anomaly,
}

/// Loiter state for hover-at-decision-point behavior
///
/// When user pauses at a fork (multiple children), we:
/// 1. Start loading previews after loiter_threshold
/// 2. Fan out branches visually
/// 3. Highlight branch based on mouse direction
#[derive(Debug, Clone, Default)]
pub struct LoiterState {
    /// Node ID we're loitering at
    pub node_id: Option<String>,
    /// Time spent at this node (seconds)
    pub duration: f32,
    /// Threshold before showing branches (seconds)
    pub threshold: f32,
    /// Whether branches are fanned out
    pub branches_visible: bool,
    /// Currently highlighted branch (by index)
    pub highlighted_branch: Option<usize>,
    /// Preview data if loaded
    pub preview: Option<PreviewData>,
    /// Whether preview is loading
    pub loading: bool,
}

impl LoiterState {
    /// Default loiter threshold (300ms)
    pub const DEFAULT_THRESHOLD: f32 = 0.3;

    /// Create new loiter state for a node
    pub fn new(node_id: String) -> Self {
        Self {
            node_id: Some(node_id),
            duration: 0.0,
            threshold: Self::DEFAULT_THRESHOLD,
            branches_visible: false,
            highlighted_branch: None,
            preview: None,
            loading: false,
        }
    }

    /// Update duration and check if threshold reached
    pub fn update(&mut self, dt: f32) -> bool {
        self.duration += dt;
        let should_show = self.duration >= self.threshold;
        if should_show && !self.branches_visible {
            self.branches_visible = true;
            return true; // Threshold just crossed
        }
        false
    }

    /// Reset loiter state
    pub fn reset(&mut self) {
        self.node_id = None;
        self.duration = 0.0;
        self.branches_visible = false;
        self.highlighted_branch = None;
        self.preview = None;
        self.loading = false;
    }

    /// Set highlighted branch based on mouse angle
    pub fn highlight_from_angle(&mut self, angle: f32, branch_count: usize) {
        if branch_count == 0 {
            self.highlighted_branch = None;
            return;
        }
        // Divide circle into sectors
        let sector_size = std::f32::consts::TAU / branch_count as f32;
        // Offset so first branch is at top
        let adjusted_angle =
            (angle + std::f32::consts::FRAC_PI_2).rem_euclid(std::f32::consts::TAU);
        let index = (adjusted_angle / sector_size) as usize % branch_count;
        self.highlighted_branch = Some(index);
    }
}

// ============================================================================
// FOCUS AND EXPANSION (Phase 4)
// ============================================================================

/// A frame in the focus stack - represents soft focus on an entity within the current level
///
/// Unlike navigation (which changes levels), focus keeps you at the same level
/// but highlights a specific entity and potentially shows expanded details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusFrame {
    /// The focused node ID
    pub node_id: String,
    /// Node type (entity, cluster, cbu)
    pub node_type: String,
    /// Display label for breadcrumb
    pub label: String,
    /// What expansion is active (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expansion: Option<ExpansionType>,
    /// Timestamp when focus was set (for ordering)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused_at: Option<f64>,
}

impl FocusFrame {
    /// Create a new focus frame
    pub fn new(node_id: String, node_type: String, label: String) -> Self {
        Self {
            node_id,
            node_type,
            label,
            expansion: None,
            focused_at: None,
        }
    }

    /// Create focus frame with expansion
    pub fn with_expansion(mut self, expansion: ExpansionType) -> Self {
        self.expansion = Some(expansion);
        self
    }
}

/// Type of expansion to show for a focused node
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpansionType {
    /// Show ownership chain (UBO tracing upward)
    Ownership,
    /// Show control relationships
    Control,
    /// Show related documents
    Documents,
    /// Show KYC/workflow status
    Workflow,
    /// Show roles and relationships
    Roles,
    /// Show all children
    Children,
}

/// State of a node's expansion animation
///
/// Tracks whether a node is expanded inline to show children/details,
/// and manages the animation phase for smooth transitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpansionState {
    /// Current animation phase (0.0 = collapsed, 1.0 = expanded)
    pub progress: f32,
    /// Target state (true = expanding, false = collapsing)
    pub target_expanded: bool,
    /// Type of expansion
    pub expansion_type: ExpansionType,
    /// Child node IDs that are being shown
    pub children: Vec<String>,
    /// Animation phase name (for debugging/logging)
    pub phase: AnimationPhase,
}

impl Default for ExpansionState {
    fn default() -> Self {
        Self {
            progress: 0.0,
            target_expanded: false,
            expansion_type: ExpansionType::Children,
            children: Vec::new(),
            phase: AnimationPhase::Hidden,
        }
    }
}

impl ExpansionState {
    /// Create a new expansion state targeting expanded
    pub fn expanding(expansion_type: ExpansionType, children: Vec<String>) -> Self {
        Self {
            progress: 0.0,
            target_expanded: true,
            expansion_type,
            children,
            phase: AnimationPhase::Budding,
        }
    }

    /// Check if expansion is complete
    pub fn is_expanded(&self) -> bool {
        self.progress >= 0.99 && self.target_expanded
    }

    /// Check if collapse is complete
    pub fn is_collapsed(&self) -> bool {
        self.progress <= 0.01 && !self.target_expanded
    }

    /// Check if animating
    pub fn is_animating(&self) -> bool {
        !self.is_expanded() && !self.is_collapsed()
    }

    /// Start collapsing
    pub fn collapse(&mut self) {
        self.target_expanded = false;
        self.phase = AnimationPhase::Collapsing;
    }

    /// Update animation phase based on progress
    pub fn update_phase(&mut self) {
        self.phase = if !self.target_expanded {
            if self.progress <= 0.01 {
                AnimationPhase::Hidden
            } else {
                AnimationPhase::Collapsing
            }
        } else {
            match self.progress {
                p if p < 0.01 => AnimationPhase::Hidden,
                p if p < 0.2 => AnimationPhase::Budding,
                p if p < 0.5 => AnimationPhase::Sprouting,
                p if p < 0.8 => AnimationPhase::Unfurling,
                p if p < 0.99 => AnimationPhase::Settling,
                _ => AnimationPhase::Visible,
            }
        };
    }
}

/// Animation phase for organic growth/collapse
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AnimationPhase {
    /// Not visible
    #[default]
    Hidden,
    /// 0-20%: Dot appears
    Budding,
    /// 20-50%: Growing
    Sprouting,
    /// 50-80%: Reaching full size
    Unfurling,
    /// 80-100%: Micro-adjustments
    Settling,
    /// Stable and visible
    Visible,
    /// Shrinking back
    Collapsing,
}

/// Focus stack for managing soft focus within a level
///
/// This allows focusing on entities without navigating away.
/// "show ownership" pushes a focus frame, "pull back" pops it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FocusStack {
    /// Stack of focus frames (most recent at end)
    pub frames: Vec<FocusFrame>,
    /// Maximum depth (prevents infinite expansion)
    pub max_depth: usize,
}

impl FocusStack {
    /// Default maximum focus depth
    pub const DEFAULT_MAX_DEPTH: usize = 5;

    /// Create a new empty focus stack
    pub fn new() -> Self {
        Self {
            frames: Vec::new(),
            max_depth: Self::DEFAULT_MAX_DEPTH,
        }
    }

    /// Push a new focus frame
    ///
    /// Returns false if at max depth
    pub fn push(&mut self, frame: FocusFrame) -> bool {
        if self.frames.len() >= self.max_depth {
            return false;
        }
        self.frames.push(frame);
        true
    }

    /// Pop the most recent focus frame
    pub fn pop(&mut self) -> Option<FocusFrame> {
        self.frames.pop()
    }

    /// Get the current (topmost) focus
    pub fn current(&self) -> Option<&FocusFrame> {
        self.frames.last()
    }

    /// Check if a node is in the focus stack
    pub fn contains(&self, node_id: &str) -> bool {
        self.frames.iter().any(|f| f.node_id == node_id)
    }

    /// Get depth (number of frames)
    pub fn depth(&self) -> usize {
        self.frames.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Clear all focus
    pub fn clear(&mut self) {
        self.frames.clear();
    }

    /// Get breadcrumb labels
    pub fn breadcrumbs(&self) -> Vec<&str> {
        self.frames.iter().map(|f| f.label.as_str()).collect()
    }
}

// ============================================================================
// PHASE 5: Agent Intelligence Types (additions to existing types)
// ============================================================================

/// Agent state for intelligent assistance
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentState {
    /// Current agent mode
    pub mode: AgentMode,
    /// Pending suggestions for user
    pub suggestions: Vec<AgentSuggestion>,
    /// Current speech bubble (if any)
    pub speech: Option<AgentSpeech>,
    /// Anomalies detected in current scope
    pub anomalies: Vec<Anomaly>,
    /// Pre-fetched data cache status
    #[serde(default)]
    pub prefetch_cache: std::collections::HashMap<String, PrefetchStatus>,
}

/// Agent operating mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentMode {
    /// Only responds to explicit commands
    #[default]
    Passive,
    /// Offers suggestions proactively
    Suggestive,
    /// Actively guides navigation
    Guiding,
    /// Executing an autopilot mission
    Autopilot,
    /// Scanning for anomalies or issues (red flag scan, black hole detection)
    Scanning,
}

/// Agent speech bubble for contextual guidance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpeech {
    /// Speech text
    pub text: String,
    /// Duration to show (seconds, 0 = until dismissed)
    pub duration_secs: f32,
    /// Position hint (node ID to anchor near)
    #[serde(default)]
    pub anchor_node_id: Option<String>,
    /// Speech urgency
    pub urgency: SpeechUrgency,
    /// When speech started (for fade timing)
    #[serde(default)]
    pub started_at: f64,
}

/// Urgency level for agent speech
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeechUrgency {
    /// Casual information
    #[default]
    Info,
    /// Gentle suggestion
    Suggestion,
    /// Important notice
    Important,
    /// Urgent warning
    Warning,
}

/// Agent insight about the current view
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInsight {
    /// Insight type
    pub insight_type: InsightType,
    /// Human-readable insight
    pub message: String,
    /// Affected node IDs
    #[serde(default)]
    pub node_ids: Vec<String>,
    /// Confidence (0.0 - 1.0)
    pub confidence: f32,
    /// Suggested action (if any)
    #[serde(default)]
    pub suggested_action: Option<NavigationAction>,
}

/// Type of agent insight
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InsightType {
    /// Pattern detected (e.g., circular ownership)
    Pattern,
    /// Anomaly detected
    Anomaly,
    /// Missing data or incomplete records
    Incomplete,
    /// Expiring or stale information
    Expiring,
    /// Relationship observation
    Relationship,
    /// Risk observation
    Risk,
}

/// Hint for speculative prefetching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefetchHint {
    /// Endpoint to prefetch
    pub endpoint: String,
    /// Priority level
    pub priority: PrefetchPriority,
    /// Human-readable reason
    pub reason: String,
    /// Cache key
    #[serde(default)]
    pub cache_key: Option<String>,
}

/// Priority for prefetch operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrefetchPriority {
    /// Load when idle
    Low,
    /// Load soon
    Medium,
    /// Load immediately
    High,
}

/// Enriched response wrapper with agent intelligence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichedResponse<T> {
    /// The actual data
    pub data: T,
    /// Detected anomalies
    #[serde(default)]
    pub anomalies: Vec<Anomaly>,
    /// Agent insights
    #[serde(default)]
    pub insights: Vec<AgentInsight>,
    /// Suggestions for user
    #[serde(default)]
    pub suggestions: Vec<AgentSuggestion>,
    /// Prefetch hints for next likely requests
    #[serde(default)]
    pub prefetch_hints: Vec<PrefetchHint>,
}

impl<T> EnrichedResponse<T> {
    /// Create a simple response with just data
    pub fn simple(data: T) -> Self {
        Self {
            data,
            anomalies: Vec::new(),
            insights: Vec::new(),
            suggestions: Vec::new(),
            prefetch_hints: Vec::new(),
        }
    }

    /// Create response with anomalies
    pub fn with_anomalies(data: T, anomalies: Vec<Anomaly>) -> Self {
        Self {
            data,
            anomalies,
            insights: Vec::new(),
            suggestions: Vec::new(),
            prefetch_hints: Vec::new(),
        }
    }
}

// ============================================================================
// PHASE 6: Autopilot Route Types
// ============================================================================

/// Response with calculated route
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteResponse {
    /// The calculated route
    pub route: Route,
    /// Estimated flight duration in seconds
    pub estimated_duration_secs: f32,
    /// Alternative routes (if any)
    #[serde(default)]
    pub alternatives: Vec<Route>,
}

/// A navigation route through the galaxy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    /// Unique route ID
    pub route_id: String,
    /// Ordered waypoints from start to destination
    pub waypoints: Vec<RouteWaypoint>,
    /// Total distance (arbitrary units for animation timing)
    pub total_distance: f32,
    /// Number of level transitions required
    pub level_transitions: usize,
    /// Brief description of the route
    pub description: String,
}

impl Route {
    /// Get the camera path for this route
    pub fn to_camera_path(&self) -> CameraPath {
        let points: Vec<(f32, f32)> = self
            .waypoints
            .iter()
            .map(|w| (w.position.0, w.position.1))
            .collect();
        CameraPath::Waypoints { points }
    }
}

/// A waypoint along a route
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteWaypoint {
    /// Node ID at this waypoint
    pub node_id: String,
    /// Node type
    pub node_type: NodeType,
    /// Display label
    pub label: String,
    /// Position in world space (x, y)
    pub position: (f32, f32),
    /// View level at this waypoint
    pub view_level: ViewLevel,
    /// Whether this is a decision point (fork)
    pub is_fork: bool,
    /// Context hint for agent speech at this waypoint
    #[serde(default)]
    pub context_hint: Option<String>,
}

/// Type of node in the navigation graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    /// Universe root
    #[default]
    Universe,
    /// Cluster of CBUs
    Cluster,
    /// Client Business Unit
    Cbu,
    /// Entity (person, company, trust, etc.)
    Entity,
    /// Document
    Document,
    /// KYC Case
    KycCase,
}

/// Autopilot mission state for client-side execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutopilotMission {
    /// The route being followed
    pub route: Route,
    /// Current waypoint index
    pub current_waypoint: usize,
    /// Mission status
    pub status: AutopilotStatus,
    /// Speed multiplier (1.0 = normal)
    pub speed: f32,
    /// Whether to pause at forks
    pub pause_at_forks: bool,
    /// Progress within current leg (0.0 - 1.0)
    pub leg_progress: f32,
}

impl AutopilotMission {
    /// Create a new mission from a route
    pub fn new(route: Route) -> Self {
        Self {
            route,
            current_waypoint: 0,
            status: AutopilotStatus::Flying,
            speed: 1.0,
            pause_at_forks: true,
            leg_progress: 0.0,
        }
    }

    /// Get the current waypoint
    pub fn current(&self) -> Option<&RouteWaypoint> {
        self.route.waypoints.get(self.current_waypoint)
    }

    /// Get the next waypoint
    pub fn next(&self) -> Option<&RouteWaypoint> {
        self.route.waypoints.get(self.current_waypoint + 1)
    }

    /// Check if at final destination
    pub fn is_at_destination(&self) -> bool {
        self.current_waypoint >= self.route.waypoints.len().saturating_sub(1)
    }

    /// Advance to next waypoint
    pub fn advance(&mut self) -> bool {
        if self.current_waypoint < self.route.waypoints.len() - 1 {
            self.current_waypoint += 1;
            self.leg_progress = 0.0;
            true
        } else {
            self.status = AutopilotStatus::Arrived;
            false
        }
    }

    /// Abort the mission
    pub fn abort(&mut self) {
        self.status = AutopilotStatus::Aborted;
    }

    /// Pause at current position
    pub fn pause(&mut self) {
        if self.status == AutopilotStatus::Flying {
            self.status = AutopilotStatus::Paused;
        }
    }

    /// Resume from pause
    pub fn resume(&mut self) {
        if self.status == AutopilotStatus::Paused {
            self.status = AutopilotStatus::Flying;
        }
    }
}

/// Status of an autopilot mission
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutopilotStatus {
    /// Actively flying toward destination
    Flying,
    /// Paused at a fork, waiting for user decision
    PausedAtFork,
    /// Paused by user
    Paused,
    /// Arrived at destination
    Arrived,
    /// Aborted by user input
    Aborted,
}

// ============================================================================
// SOLAR NAVIGATION TYPES (038 Design Spec)
// ============================================================================

/// Canonical orbit position for System-level navigation.
/// This is the authoritative "cursor" when in System view.
/// `focus_cbu_id` is DERIVED from this via layout, not the other way around.
///
/// # Design Decision (038 v2)
/// At System level, orbit_pos is the source of truth. When navigating
/// clockwise/counter-clockwise/inner/outer, we mutate orbit_pos and
/// derive the focused CBU from the layout engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrbitPos {
    /// Ring index (0 = innermost orbit)
    pub ring: usize,
    /// Position within ring (0 = first CBU, clockwise)
    pub index: usize,
}

impl OrbitPos {
    /// Create a new orbit position
    pub fn new(ring: usize, index: usize) -> Self {
        Self { ring, index }
    }

    /// Default starting position (first planet in innermost ring)
    pub fn origin() -> Self {
        Self { ring: 0, index: 0 }
    }
}

/// Complete semantic view state for navigation.
///
/// This captures WHERE the user is looking, not HOW it's rendered.
/// Layout and camera are derived from this state.
///
/// # Design Decisions (038 v2-v4)
/// - `timestamp` is metadata only, excluded from identity checks
/// - At System level, `orbit_pos` is canonical; `focus_cbu_id` is derived
/// - Constructors prevent invalid state combinations
/// - `validate()` catches bugs in debug builds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewState {
    /// Current discrete view level
    pub level: ViewLevel,

    /// Focus for Cluster/System views (ManCo or cluster being viewed)
    /// At System level, this is the ManCo whose CBUs are in orbit
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focus_manco_id: Option<String>,

    /// Focus for Planet/Surface views (the CBU being examined)
    /// At System level, this is DERIVED from orbit_pos via layout
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focus_cbu_id: Option<String>,

    /// System-level position cursor (canonical for orbital navigation)
    /// Only meaningful at System level
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orbit_pos: Option<OrbitPos>,

    /// Timestamp when this state was created (metadata only)
    /// Excluded from identity comparisons via ViewStateKey
    pub timestamp: i64,
}

impl ViewState {
    /// Create Universe view state
    pub fn universe(timestamp: i64) -> Self {
        Self {
            level: ViewLevel::Universe,
            focus_manco_id: None,
            focus_cbu_id: None,
            orbit_pos: None,
            timestamp,
        }
    }

    /// Create Cluster view state (viewing a cluster of CBUs)
    pub fn cluster(cluster_id: String, timestamp: i64) -> Self {
        Self {
            level: ViewLevel::Cluster,
            focus_manco_id: Some(cluster_id),
            focus_cbu_id: None,
            orbit_pos: None,
            timestamp,
        }
    }

    /// Create System view state (ManCo + orbiting CBUs)
    pub fn system(manco_id: String, timestamp: i64) -> Self {
        Self {
            level: ViewLevel::System,
            focus_manco_id: Some(manco_id),
            focus_cbu_id: None,
            orbit_pos: None,
            timestamp,
        }
    }

    /// Create System view with explicit orbit position
    pub fn system_at(manco_id: String, orbit_pos: OrbitPos, timestamp: i64) -> Self {
        Self {
            level: ViewLevel::System,
            focus_manco_id: Some(manco_id),
            focus_cbu_id: None, // Derived from orbit_pos via layout
            orbit_pos: Some(orbit_pos),
            timestamp,
        }
    }

    /// Create Planet view state (single CBU focused)
    /// `parent_manco_id` enables clean zoom_out back to System view
    pub fn planet(cbu_id: String, parent_manco_id: Option<String>, timestamp: i64) -> Self {
        Self {
            level: ViewLevel::Planet,
            focus_manco_id: parent_manco_id,
            focus_cbu_id: Some(cbu_id),
            orbit_pos: None,
            timestamp,
        }
    }

    /// Create Planet view without parent ManCo context
    pub fn planet_simple(cbu_id: String, timestamp: i64) -> Self {
        Self::planet(cbu_id, None, timestamp)
    }

    /// Create Surface view state (entities within CBU)
    pub fn surface(cbu_id: String, timestamp: i64) -> Self {
        Self {
            level: ViewLevel::Surface,
            focus_manco_id: None,
            focus_cbu_id: Some(cbu_id),
            orbit_pos: None,
            timestamp,
        }
    }

    /// Create Core view state (deep analysis)
    pub fn core(cbu_id: String, timestamp: i64) -> Self {
        Self {
            level: ViewLevel::Core,
            focus_manco_id: None,
            focus_cbu_id: Some(cbu_id),
            orbit_pos: None,
            timestamp,
        }
    }

    /// Get stable identity key (excludes timestamp for dedupe)
    pub fn key(&self) -> ViewStateKey {
        ViewStateKey {
            level: self.level,
            focus_manco_id: self.focus_manco_id.clone(),
            focus_cbu_id: self.focus_cbu_id.clone(),
            orbit_pos: self.orbit_pos,
        }
    }

    /// Validate state invariants (call in debug builds)
    ///
    /// # Returns
    /// - `Ok(())` if state is valid
    /// - `Err(message)` describing the invariant violation
    pub fn validate(&self) -> Result<(), String> {
        match self.level {
            ViewLevel::Universe => {
                // Universe has no requirements
                Ok(())
            }
            ViewLevel::Cluster => {
                // Cluster may have focus_manco_id but no orbit
                if self.orbit_pos.is_some() {
                    return Err("Cluster view must not have orbit_pos".into());
                }
                Ok(())
            }
            ViewLevel::System => {
                if self.focus_manco_id.is_none() {
                    return Err("System view requires focus_manco_id".into());
                }
                // orbit_pos is optional (allow "entered but not selected")
                Ok(())
            }
            ViewLevel::Planet => {
                if self.focus_cbu_id.is_none() {
                    return Err("Planet view requires focus_cbu_id".into());
                }
                Ok(())
            }
            ViewLevel::Surface => {
                if self.focus_cbu_id.is_none() {
                    return Err("Surface view requires focus_cbu_id".into());
                }
                Ok(())
            }
            ViewLevel::Core => {
                if self.focus_cbu_id.is_none() {
                    return Err("Core view requires focus_cbu_id".into());
                }
                Ok(())
            }
        }
    }

    /// Compact string encoding for URLs/sharing
    /// Format: "U" | "C:id" | "S:id" | "P:id" | "R:id" | "X:id"
    pub fn to_compact_string(&self) -> String {
        match self.level {
            ViewLevel::Universe => "U".into(),
            ViewLevel::Cluster => format!("C:{}", self.focus_manco_id.as_deref().unwrap_or("")),
            ViewLevel::System => format!("S:{}", self.focus_manco_id.as_deref().unwrap_or("")),
            ViewLevel::Planet => format!("P:{}", self.focus_cbu_id.as_deref().unwrap_or("")),
            ViewLevel::Surface => format!("R:{}", self.focus_cbu_id.as_deref().unwrap_or("")),
            ViewLevel::Core => format!("X:{}", self.focus_cbu_id.as_deref().unwrap_or("")),
        }
    }

    /// Parse compact string back to ViewState
    pub fn from_compact_string(s: &str, timestamp: i64) -> Option<Self> {
        if s == "U" {
            return Some(Self::universe(timestamp));
        }
        if let Some(id) = s.strip_prefix("C:") {
            return Some(Self::cluster(id.to_string(), timestamp));
        }
        if let Some(id) = s.strip_prefix("S:") {
            return Some(Self::system(id.to_string(), timestamp));
        }
        if let Some(id) = s.strip_prefix("P:") {
            return Some(Self::planet_simple(id.to_string(), timestamp));
        }
        if let Some(id) = s.strip_prefix("R:") {
            return Some(Self::surface(id.to_string(), timestamp));
        }
        if let Some(id) = s.strip_prefix("X:") {
            return Some(Self::core(id.to_string(), timestamp));
        }
        None
    }
}

impl Default for ViewState {
    fn default() -> Self {
        Self::universe(0)
    }
}

/// Identity key for ViewState (excludes timestamp for dedupe).
///
/// Two ViewStates with the same key are semantically identical,
/// even if their timestamps differ. Used by NavigationHistory
/// to prevent duplicate entries.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ViewStateKey {
    pub level: ViewLevel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focus_manco_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focus_cbu_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orbit_pos: Option<OrbitPos>,
}

// ============================================================================
// NAVIGATION HISTORY (038 Design Spec - Part 2.3)
// ============================================================================

/// Navigation history with browser-style back/forward/rewind.
///
/// # Design Decisions (038 v2)
/// - Uses VecDeque for efficient front removal when max_size exceeded
/// - Cursor is Option<usize> to handle empty state safely
/// - `push_if_changed()` dedupes by ViewStateKey (ignores timestamp)
/// - Forward history is truncated when pushing after back()
///
/// # Critical Rule (038 v3)
/// History navigation verbs (back/forward/rewind) NEVER call push_if_changed().
/// They only move the cursor. Otherwise "back" creates new history entries
/// and becomes impossible to use correctly.
#[derive(Debug, Clone, Default)]
pub struct NavigationHistory {
    /// Snapshots stored in order (oldest first)
    snapshots: std::collections::VecDeque<ViewState>,
    /// Current position in history (None if empty)
    cursor: Option<usize>,
    /// Maximum history size (oldest dropped when exceeded)
    max_size: usize,
}

impl NavigationHistory {
    /// Create new history with specified max size
    pub fn new(max_size: usize) -> Self {
        Self {
            snapshots: std::collections::VecDeque::new(),
            cursor: None,
            max_size: max_size.max(1),
        }
    }

    /// Create with default max size (50)
    pub fn with_default_size() -> Self {
        Self::new(50)
    }

    /// Check if history is empty
    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    /// Get number of entries
    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    /// Get current cursor position
    pub fn cursor(&self) -> Option<usize> {
        self.cursor
    }

    /// Get current view state (at cursor)
    pub fn current(&self) -> Option<&ViewState> {
        self.cursor.and_then(|i| self.snapshots.get(i))
    }

    /// Push a state ONLY if it meaningfully changes the semantic view.
    ///
    /// - Dedupes by ViewStateKey (timestamp-only changes are ignored)
    /// - Truncates forward history if cursor is not at end
    /// - Enforces max_size by dropping oldest entries
    /// - Cursor always ends pointing at the new state
    ///
    /// # Returns
    /// `true` if state was pushed, `false` if dedupe prevented it
    pub fn push_if_changed(&mut self, state: ViewState) -> bool {
        // Validate in debug builds
        debug_assert!(
            state.validate().is_ok(),
            "Invalid ViewState pushed to history"
        );

        // Dedupe: if same semantic state, do nothing
        if let Some(cur) = self.current() {
            if cur.key() == state.key() {
                return false;
            }
        }

        // If cursor is not at end, drop forward history (browser semantics)
        if let Some(c) = self.cursor {
            let keep_len = c + 1;
            while self.snapshots.len() > keep_len {
                self.snapshots.pop_back();
            }
        }

        // Push new state
        self.snapshots.push_back(state);
        self.cursor = Some(self.snapshots.len() - 1);

        // Enforce max size by dropping from front
        while self.snapshots.len() > self.max_size {
            self.snapshots.pop_front();
            // Adjust cursor (it shifted left)
            if let Some(c) = self.cursor {
                self.cursor = Some(c.saturating_sub(1));
            }
        }

        true
    }

    /// Move back one step in history.
    ///
    /// # Critical Rule
    /// This does NOT push to history. It only moves the cursor.
    ///
    /// # Returns
    /// The previous state if available, None if already at oldest
    pub fn back(&mut self) -> Option<&ViewState> {
        let c = self.cursor?;
        if c == 0 {
            return None;
        }
        self.cursor = Some(c - 1);
        self.current()
    }

    /// Move forward one step in history.
    ///
    /// # Critical Rule
    /// This does NOT push to history. It only moves the cursor.
    ///
    /// # Returns
    /// The next state if available, None if already at newest
    pub fn forward(&mut self) -> Option<&ViewState> {
        let c = self.cursor?;
        if c + 1 >= self.snapshots.len() {
            return None;
        }
        self.cursor = Some(c + 1);
        self.current()
    }

    /// Jump to the first (oldest) entry.
    ///
    /// # Critical Rule
    /// This does NOT push to history. It only moves the cursor.
    pub fn rewind(&mut self) -> Option<&ViewState> {
        if self.snapshots.is_empty() {
            self.cursor = None;
            return None;
        }
        self.cursor = Some(0);
        self.current()
    }

    /// Jump to a specific index.
    ///
    /// # Critical Rule
    /// This does NOT push to history. It only moves the cursor.
    pub fn jump_to(&mut self, index: usize) -> Option<&ViewState> {
        if index >= self.snapshots.len() {
            return None;
        }
        self.cursor = Some(index);
        self.current()
    }

    /// Get breadcrumbs for UI display (index + key)
    pub fn breadcrumbs(&self) -> Vec<(usize, ViewStateKey)> {
        self.snapshots
            .iter()
            .enumerate()
            .map(|(i, s)| (i, s.key()))
            .collect()
    }

    /// Check if we can go back
    pub fn can_go_back(&self) -> bool {
        self.cursor.map(|c| c > 0).unwrap_or(false)
    }

    /// Check if we can go forward
    pub fn can_go_forward(&self) -> bool {
        self.cursor
            .map(|c| c + 1 < self.snapshots.len())
            .unwrap_or(false)
    }

    /// Clear all history
    pub fn clear(&mut self) {
        self.snapshots.clear();
        self.cursor = None;
    }
}

// ============================================================================
// NAVIGATION RESULT (038 Design Spec - Part 4c)
// ============================================================================

/// Result of a navigation operation.
///
/// Three-state result for clean error handling:
/// - `Ok`: Operation succeeded, state changed
/// - `NoOp`: Operation valid but no change needed (e.g., already at destination)
/// - `Err`: Operation failed (invalid transition, missing data)
///
/// # Design Decision (038 v3)
/// NoOp is distinct from Err because:
/// - NoOp: "clockwise" when already at same position = valid, no change
/// - Err: "land on" when not at System level = invalid transition
///
/// Both NoOp and Err should NOT push to history.
///
/// # Structured Errors (UX/Telemetry Enhancement)
/// Use `noop()` and `err()` constructors to include reason codes for
/// telemetry aggregation and agent decision-making. The old string-only
/// variants still work for backwards compatibility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavResult {
    /// Operation succeeded, message describes what happened
    Ok(&'static str),
    /// Operation valid but no state change (dedupe, already there, etc.)
    NoOp(&'static str),
    /// Operation failed with error message
    Err(String),
}

impl NavResult {
    // === Constructors with reason codes ===

    /// Create a NoOp with a standard reason code.
    /// Message is derived from the reason.
    pub fn noop(reason: NavReason) -> Self {
        NavResult::NoOp(reason.message())
    }

    /// Create an Err with a standard reason code.
    /// Message is derived from the reason.
    pub fn err(reason: NavReason) -> Self {
        NavResult::Err(reason.message().to_string())
    }

    /// Create an Err with a formatted message (for dynamic content like names).
    /// Use when the standard message needs interpolation.
    pub fn err_fmt(reason: NavReason, name: &str) -> Self {
        let msg = match reason {
            NavReason::UnknownAlias => format!("Unknown ManCo: \"{}\".", name),
            NavReason::NotInScope => format!("{} not available in current scope.", name),
            _ => reason.message().to_string(),
        };
        NavResult::Err(msg)
    }

    // === Predicates ===

    /// Check if operation was successful
    pub fn is_ok(&self) -> bool {
        matches!(self, NavResult::Ok(_))
    }

    /// Check if operation was a no-op
    pub fn is_noop(&self) -> bool {
        matches!(self, NavResult::NoOp(_))
    }

    /// Check if operation failed
    pub fn is_err(&self) -> bool {
        matches!(self, NavResult::Err(_))
    }

    // === Accessors ===

    /// Get the message regardless of variant
    pub fn message(&self) -> &str {
        match self {
            NavResult::Ok(msg) => msg,
            NavResult::NoOp(msg) => msg,
            NavResult::Err(msg) => msg,
        }
    }

    /// Convert to Result for ? operator compatibility
    pub fn to_result(self) -> Result<&'static str, String> {
        match self {
            NavResult::Ok(msg) => Ok(msg),
            NavResult::NoOp(msg) => Ok(msg), // NoOp is not an error
            NavResult::Err(msg) => Err(msg),
        }
    }

    /// Get suggestion for this result (if it's a NoOp/Err with known reason).
    /// Returns None for Ok results or unknown error messages.
    pub fn suggestion(&self) -> Option<NavSuggestion> {
        let msg = match self {
            NavResult::Ok(_) => return None,
            NavResult::NoOp(msg) => *msg,
            NavResult::Err(msg) => msg.as_str(),
        };
        Self::reason_from_message(msg).and_then(NavSuggestion::for_reason)
    }

    /// Try to extract reason code from message (for telemetry).
    /// This is a reverse lookup - prefer using noop()/err() constructors.
    fn reason_from_message(msg: &str) -> Option<NavReason> {
        // Match against known standard messages
        match msg {
            "Not available at this zoom level." => Some(NavReason::LevelBlocked),
            "Orbit navigation only works in System view." => Some(NavReason::OrbitOnlyInSystem),
            "ManCo selection works in Galaxy view." => Some(NavReason::SelectionOnlyInGalaxy),
            "Planet selection works in System view." => Some(NavReason::SelectionOnlyInSystem),
            "Select a ManCo first." => Some(NavReason::MissingFocusManco),
            "Select a planet first." => Some(NavReason::MissingOrbitPos),
            "No planet is focused." => Some(NavReason::MissingFocusCbu),
            "Already at deepest level." => Some(NavReason::AlreadyDeepest),
            "Already at galaxy level." => Some(NavReason::AlreadyShallowest),
            "Already at oldest state." => Some(NavReason::AtOldest),
            "Already at newest state." => Some(NavReason::AtNewest),
            "Already selected." => Some(NavReason::SelectionUnchanged),
            "Selected planet no longer exists." => Some(NavReason::SelectionOutOfRange),
            "History index out of range." => Some(NavReason::IndexOutOfRange),
            "History is empty." => Some(NavReason::EmptyHistory),
            "No planets in this system." => Some(NavReason::NoPlanets),
            "No more rings in that direction." => Some(NavReason::NoMoreRings),
            "Layout not ready yet." => Some(NavReason::LayoutNotReady),
            "Cannot determine parent system." => Some(NavReason::CannotDeriveParent),
            "Navigation state is invalid. Try returning to Galaxy." => {
                Some(NavReason::InvalidState)
            }
            _ => None,
        }
    }
}

/// Error type for navigation failures (used with NavResult::Err)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavError {
    /// Invalid level transition
    InvalidTransition {
        from: ViewLevel,
        to: ViewLevel,
        hint: String,
    },
    /// Entity not in current scope
    NotInScope {
        entity_type: &'static str,
        id: String,
    },
    /// Missing required state
    MissingState(&'static str),
    /// No orbit position set
    NoOrbitPosition,
    /// Layout error
    LayoutError(String),
}

impl std::fmt::Display for NavError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NavError::InvalidTransition { from, to, hint } => {
                write!(f, "Cannot transition from {:?} to {:?}. {}", from, to, hint)
            }
            NavError::NotInScope { entity_type, id } => {
                write!(f, "{} '{}' is not in current scope", entity_type, id)
            }
            NavError::MissingState(field) => {
                write!(f, "Missing required state: {}", field)
            }
            NavError::NoOrbitPosition => {
                write!(f, "No orbit position set. Select a planet first.")
            }
            NavError::LayoutError(msg) => {
                write!(f, "Layout error: {}", msg)
            }
        }
    }
}

impl From<NavError> for NavResult {
    fn from(err: NavError) -> Self {
        NavResult::Err(err.to_string())
    }
}

// ============================================================================
// NAVIGATION REASON CODES & SUGGESTIONS (UX/Telemetry)
// ============================================================================

/// Stable reason codes for navigation outcomes.
///
/// Used for:
/// - Telemetry aggregation (same condition = same code)
/// - Agent decision-making (can retry vs needs user action)
/// - Structured error handling
///
/// # Design Rules
/// - Codes are stable strings, not changing messages
/// - Keep enumerable for clean analytics
/// - Use `level_blocked` not `wrong_level` (describe condition, not blame)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NavReason {
    // === Level/Context errors ===
    /// Verb not valid at current zoom level
    LevelBlocked,
    /// Need to be at System level for orbit operations
    OrbitOnlyInSystem,
    /// Need to be at Galaxy level for ManCo selection
    SelectionOnlyInGalaxy,
    /// Need to be at System level for planet selection
    SelectionOnlyInSystem,

    // === Missing focus/selection ===
    /// No ManCo selected (required for zoom in from Galaxy)
    MissingFocusManco,
    /// No orbit position selected (required for zoom in from System)
    MissingOrbitPos,
    /// No CBU focused (required for deeper zoom)
    MissingFocusCbu,

    // === Boundary conditions ===
    /// Already at deepest level
    AlreadyDeepest,
    /// Already at shallowest level
    AlreadyShallowest,
    /// Already at oldest history entry
    AtOldest,
    /// Already at newest history entry
    AtNewest,
    /// Selection unchanged (same item re-selected)
    SelectionUnchanged,

    // === Invalid input ===
    /// Orbit ring/index out of range
    SelectionOutOfRange,
    /// History index out of range
    IndexOutOfRange,
    /// Alias/name cannot be resolved
    UnknownAlias,
    /// Entity exists but not in current scope
    NotInScope,

    // === State issues ===
    /// History is empty
    EmptyHistory,
    /// No planets in current system
    NoPlanets,
    /// No more rings in requested direction
    NoMoreRings,
    /// Layout not computed yet
    LayoutNotReady,
    /// Cannot determine parent (orphan node)
    CannotDeriveParent,
    /// Internal state inconsistency
    InvalidState,
}

impl NavReason {
    /// Get the standard user-facing message for this reason.
    ///
    /// These messages follow UX copy guidelines:
    /// - Short, actionable phrases
    /// - No internal IDs exposed
    /// - Consistent across all navigation verbs
    pub fn message(&self) -> &'static str {
        match self {
            // Level/Context
            NavReason::LevelBlocked => "Not available at this zoom level.",
            NavReason::OrbitOnlyInSystem => "Orbit navigation only works in System view.",
            NavReason::SelectionOnlyInGalaxy => "ManCo selection works in Galaxy view.",
            NavReason::SelectionOnlyInSystem => "Planet selection works in System view.",

            // Missing focus
            NavReason::MissingFocusManco => "Select a ManCo first.",
            NavReason::MissingOrbitPos => "Select a planet first.",
            NavReason::MissingFocusCbu => "No planet is focused.",

            // Boundaries
            NavReason::AlreadyDeepest => "Already at deepest level.",
            NavReason::AlreadyShallowest => "Already at galaxy level.",
            NavReason::AtOldest => "Already at oldest state.",
            NavReason::AtNewest => "Already at newest state.",
            NavReason::SelectionUnchanged => "Already selected.",

            // Invalid input
            NavReason::SelectionOutOfRange => "Selected planet no longer exists.",
            NavReason::IndexOutOfRange => "History index out of range.",
            NavReason::UnknownAlias => "Unknown name.", // Caller should format with actual name
            NavReason::NotInScope => "Not available in current scope.",

            // State issues
            NavReason::EmptyHistory => "History is empty.",
            NavReason::NoPlanets => "No planets in this system.",
            NavReason::NoMoreRings => "No more rings in that direction.",
            NavReason::LayoutNotReady => "Layout not ready yet.",
            NavReason::CannotDeriveParent => "Cannot determine parent system.",
            NavReason::InvalidState => "Navigation state is invalid. Try returning to Galaxy.",
        }
    }

    /// Is this a retryable condition (agent can try alternative)?
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            NavReason::MissingFocusManco
                | NavReason::MissingOrbitPos
                | NavReason::LayoutNotReady
                | NavReason::NoPlanets
        )
    }

    /// Does this require user action to fix?
    pub fn needs_user_action(&self) -> bool {
        matches!(
            self,
            NavReason::UnknownAlias | NavReason::NotInScope | NavReason::InvalidState
        )
    }
}

impl std::fmt::Display for NavReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message())
    }
}

/// Suggested follow-up actions when navigation fails or no-ops.
///
/// Helps the agent (and UI) offer constructive next steps without
/// hardcoding natural language in multiple places.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavSuggestion {
    /// Human-readable hint (e.g., "Select a ManCo to zoom into")
    pub hint: String,
    /// Suggested verbs/actions the user or agent could take
    pub actions: Vec<String>,
}

impl NavSuggestion {
    /// Create a new suggestion
    pub fn new(hint: &'static str, actions: &[&'static str]) -> Self {
        Self {
            hint: hint.to_string(),
            actions: actions.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// No ManCo selected - suggest selection or direct entry
    pub fn select_manco() -> Self {
        Self::new(
            "Select a ManCo to zoom into",
            &["nav.select_manco", "nav.enter_system"],
        )
    }

    /// No planet selected - suggest orbit navigation
    pub fn select_planet() -> Self {
        Self::new(
            "Select a planet to land on",
            &["nav.orbit_select", "nav.orbit_next"],
        )
    }

    /// At deepest level - suggest zoom out
    pub fn zoom_out() -> Self {
        Self::new("Use zoom out to go back", &["nav.zoom_out", "nav.back"])
    }

    /// Wrong level for operation - suggest correct level
    pub fn go_to_system() -> Self {
        Self::new("Enter a system first", &["nav.enter_system", "nav.zoom_in"])
    }

    /// Wrong level for operation - suggest correct level
    pub fn go_to_galaxy() -> Self {
        Self::new(
            "Return to galaxy view first",
            &["nav.zoom_out", "nav.rewind"],
        )
    }

    /// History navigation failed - suggest content navigation
    pub fn use_content_nav() -> Self {
        Self::new(
            "Navigate to content instead",
            &["nav.enter_system", "nav.enter_planet"],
        )
    }

    /// Invalid state - suggest reset
    pub fn reset() -> Self {
        Self::new(
            "Reset navigation to recover",
            &["nav.rewind", "view.universe"],
        )
    }

    /// Get suggestion for a given reason code
    pub fn for_reason(reason: NavReason) -> Option<Self> {
        match reason {
            NavReason::MissingFocusManco => Some(Self::select_manco()),
            NavReason::MissingOrbitPos => Some(Self::select_planet()),
            NavReason::AlreadyDeepest => Some(Self::zoom_out()),
            NavReason::OrbitOnlyInSystem => Some(Self::go_to_system()),
            NavReason::SelectionOnlyInGalaxy => Some(Self::go_to_galaxy()),
            NavReason::EmptyHistory | NavReason::AtOldest | NavReason::AtNewest => {
                Some(Self::use_content_nav())
            }
            NavReason::InvalidState => Some(Self::reset()),
            _ => None,
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_scope_tagged() {
        let scope = NavigationScope::Cbu {
            cbu_id: "abc".into(),
            cbu_name: "Test".into(),
        };
        let json = serde_json::to_string(&scope).unwrap();
        assert!(json.contains(r#""type":"cbu""#));
    }

    #[test]
    fn view_level_zoom_ranges() {
        assert_eq!(ViewLevel::Universe.zoom_range(), (0.05, 0.3));
        assert_eq!(ViewLevel::System.zoom_range(), (0.5, 1.5));
    }

    #[test]
    fn view_level_hierarchy() {
        assert_eq!(ViewLevel::Universe.parent(), None);
        assert_eq!(ViewLevel::Universe.child(), Some(ViewLevel::Cluster));
        assert_eq!(ViewLevel::Core.child(), None);
        assert_eq!(ViewLevel::Core.parent(), Some(ViewLevel::Surface));
    }

    #[test]
    fn risk_summary_dominant() {
        let summary = RiskSummary {
            high: 1,
            medium: 5,
            low: 10,
            unrated: 2,
        };
        assert_eq!(summary.dominant_rating(), RiskRating::High);

        let summary2 = RiskSummary {
            high: 0,
            medium: 0,
            low: 5,
            unrated: 0,
        };
        assert_eq!(summary2.dominant_rating(), RiskRating::Low);
    }

    #[test]
    fn navigation_action_tagged() {
        let action = NavigationAction::DrillIntoCbu {
            cbu_id: "123".into(),
        };
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains(r#""action":"drill_into_cbu""#));
    }

    // ========================================================================
    // SOLAR NAVIGATION TESTS (038 Design Spec)
    // ========================================================================

    #[test]
    fn orbit_pos_creation() {
        let pos = OrbitPos::new(1, 5);
        assert_eq!(pos.ring, 1);
        assert_eq!(pos.index, 5);

        let origin = OrbitPos::origin();
        assert_eq!(origin.ring, 0);
        assert_eq!(origin.index, 0);
    }

    #[test]
    fn view_state_constructors() {
        let ts = 1234567890;

        let universe = ViewState::universe(ts);
        assert_eq!(universe.level, ViewLevel::Universe);
        assert!(universe.validate().is_ok());

        let cluster = ViewState::cluster("cluster-1".into(), ts);
        assert_eq!(cluster.level, ViewLevel::Cluster);
        assert_eq!(cluster.focus_manco_id, Some("cluster-1".into()));
        assert!(cluster.validate().is_ok());

        let system = ViewState::system("manco-1".into(), ts);
        assert_eq!(system.level, ViewLevel::System);
        assert_eq!(system.focus_manco_id, Some("manco-1".into()));
        assert!(system.validate().is_ok());

        let system_at = ViewState::system_at("manco-1".into(), OrbitPos::new(0, 3), ts);
        assert_eq!(system_at.orbit_pos, Some(OrbitPos::new(0, 3)));
        assert!(system_at.validate().is_ok());

        let planet = ViewState::planet("cbu-1".into(), Some("manco-1".into()), ts);
        assert_eq!(planet.level, ViewLevel::Planet);
        assert_eq!(planet.focus_cbu_id, Some("cbu-1".into()));
        assert_eq!(planet.focus_manco_id, Some("manco-1".into())); // For zoom_out
        assert!(planet.validate().is_ok());

        let surface = ViewState::surface("cbu-1".into(), ts);
        assert_eq!(surface.level, ViewLevel::Surface);
        assert!(surface.validate().is_ok());

        let core = ViewState::core("cbu-1".into(), ts);
        assert_eq!(core.level, ViewLevel::Core);
        assert!(core.validate().is_ok());
    }

    #[test]
    fn view_state_validation_failures() {
        // System without focus_manco_id
        let invalid_system = ViewState {
            level: ViewLevel::System,
            focus_manco_id: None,
            focus_cbu_id: None,
            orbit_pos: None,
            timestamp: 0,
        };
        assert!(invalid_system.validate().is_err());

        // Planet without focus_cbu_id
        let invalid_planet = ViewState {
            level: ViewLevel::Planet,
            focus_manco_id: None,
            focus_cbu_id: None,
            orbit_pos: None,
            timestamp: 0,
        };
        assert!(invalid_planet.validate().is_err());

        // Cluster with orbit_pos (invalid)
        let invalid_cluster = ViewState {
            level: ViewLevel::Cluster,
            focus_manco_id: Some("c".into()),
            focus_cbu_id: None,
            orbit_pos: Some(OrbitPos::origin()),
            timestamp: 0,
        };
        assert!(invalid_cluster.validate().is_err());
    }

    #[test]
    fn view_state_key_excludes_timestamp() {
        let state1 = ViewState::system("manco-1".into(), 1000);
        let state2 = ViewState::system("manco-1".into(), 2000);

        // Same semantic state, different timestamps
        assert_eq!(state1.key(), state2.key());

        // Different focus = different key
        let state3 = ViewState::system("manco-2".into(), 1000);
        assert_ne!(state1.key(), state3.key());
    }

    #[test]
    fn view_state_compact_string_roundtrip() {
        let ts = 12345;
        let states = vec![
            ViewState::universe(ts),
            ViewState::cluster("cluster-abc".into(), ts),
            ViewState::system("manco-xyz".into(), ts),
            ViewState::planet_simple("cbu-123".into(), ts),
            ViewState::surface("cbu-456".into(), ts),
            ViewState::core("cbu-789".into(), ts),
        ];

        for state in states {
            let compact = state.to_compact_string();
            let parsed = ViewState::from_compact_string(&compact, ts).unwrap();
            assert_eq!(state.key(), parsed.key());
        }
    }

    #[test]
    fn navigation_history_push_and_dedupe() {
        let mut history = NavigationHistory::new(10);

        // Push first state
        let state1 = ViewState::universe(1000);
        assert!(history.push_if_changed(state1.clone()));
        assert_eq!(history.len(), 1);
        assert_eq!(history.cursor(), Some(0));

        // Push same state again (different timestamp) - should be deduped
        let state1_again = ViewState::universe(2000);
        assert!(!history.push_if_changed(state1_again));
        assert_eq!(history.len(), 1); // No change

        // Push different state
        let state2 = ViewState::cluster("c1".into(), 3000);
        assert!(history.push_if_changed(state2));
        assert_eq!(history.len(), 2);
        assert_eq!(history.cursor(), Some(1));
    }

    #[test]
    fn navigation_history_back_forward() {
        let mut history = NavigationHistory::new(10);

        history.push_if_changed(ViewState::universe(1));
        history.push_if_changed(ViewState::cluster("c1".into(), 2));
        history.push_if_changed(ViewState::system("m1".into(), 3));

        assert_eq!(history.cursor(), Some(2));
        assert!(history.can_go_back());
        assert!(!history.can_go_forward());

        // Go back
        let prev = history.back().unwrap();
        assert_eq!(prev.level, ViewLevel::Cluster);
        assert_eq!(history.cursor(), Some(1));
        assert!(history.can_go_forward());

        // Go forward
        let next = history.forward().unwrap();
        assert_eq!(next.level, ViewLevel::System);
        assert_eq!(history.cursor(), Some(2));

        // Can't go forward from end
        assert!(history.forward().is_none());
    }

    #[test]
    fn navigation_history_back_then_push_truncates_forward() {
        let mut history = NavigationHistory::new(10);

        history.push_if_changed(ViewState::universe(1));
        history.push_if_changed(ViewState::cluster("c1".into(), 2));
        history.push_if_changed(ViewState::system("m1".into(), 3));

        // Go back twice
        history.back();
        history.back();
        assert_eq!(history.cursor(), Some(0));

        // Push new state - should truncate forward history
        history.push_if_changed(ViewState::planet_simple("cbu-1".into(), 4));
        assert_eq!(history.len(), 2); // Universe + Planet (Cluster and System truncated)
        assert!(!history.can_go_forward());
    }

    #[test]
    fn navigation_history_rewind() {
        let mut history = NavigationHistory::new(10);

        history.push_if_changed(ViewState::universe(1));
        history.push_if_changed(ViewState::cluster("c1".into(), 2));
        history.push_if_changed(ViewState::system("m1".into(), 3));

        let first = history.rewind().unwrap();
        assert_eq!(first.level, ViewLevel::Universe);
        assert_eq!(history.cursor(), Some(0));
    }

    #[test]
    fn navigation_history_max_size() {
        let mut history = NavigationHistory::new(3);

        history.push_if_changed(ViewState::universe(1));
        history.push_if_changed(ViewState::cluster("c1".into(), 2));
        history.push_if_changed(ViewState::system("m1".into(), 3));
        assert_eq!(history.len(), 3);

        // Push 4th - oldest should be dropped
        history.push_if_changed(ViewState::planet_simple("p1".into(), 4));
        assert_eq!(history.len(), 3);

        // Cursor should still be valid (adjusted after drop)
        assert_eq!(history.cursor(), Some(2));

        // First entry is now Cluster (Universe was dropped)
        let first = history.rewind().unwrap();
        assert_eq!(first.level, ViewLevel::Cluster);
    }

    #[test]
    fn navigation_history_jump_to() {
        let mut history = NavigationHistory::new(10);

        history.push_if_changed(ViewState::universe(1));
        history.push_if_changed(ViewState::cluster("c1".into(), 2));
        history.push_if_changed(ViewState::system("m1".into(), 3));

        let jumped = history.jump_to(1).unwrap();
        assert_eq!(jumped.level, ViewLevel::Cluster);
        assert_eq!(history.cursor(), Some(1));

        // Invalid index
        assert!(history.jump_to(10).is_none());
    }

    #[test]
    fn nav_result_variants() {
        let ok = NavResult::Ok("Success");
        assert!(ok.is_ok());
        assert!(!ok.is_noop());
        assert!(!ok.is_err());
        assert_eq!(ok.message(), "Success");

        let noop = NavResult::NoOp("Already there");
        assert!(!noop.is_ok());
        assert!(noop.is_noop());
        assert!(!noop.is_err());

        let err = NavResult::Err("Failed".into());
        assert!(!err.is_ok());
        assert!(!err.is_noop());
        assert!(err.is_err());
    }

    #[test]
    fn nav_error_display() {
        let err = NavError::InvalidTransition {
            from: ViewLevel::Cluster,
            to: ViewLevel::Surface,
            hint: "Enter a system first".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("Cluster"));
        assert!(msg.contains("Surface"));
        assert!(msg.contains("Enter a system first"));

        let result: NavResult = err.into();
        assert!(result.is_err());
    }

    #[test]
    fn nav_reason_messages() {
        // Each reason has a standard message
        assert_eq!(
            NavReason::MissingFocusManco.message(),
            "Select a ManCo first."
        );
        assert_eq!(
            NavReason::OrbitOnlyInSystem.message(),
            "Orbit navigation only works in System view."
        );
        assert_eq!(NavReason::EmptyHistory.message(), "History is empty.");
    }

    #[test]
    fn nav_reason_retryable() {
        // These are retryable (agent can try alternative)
        assert!(NavReason::MissingFocusManco.is_retryable());
        assert!(NavReason::LayoutNotReady.is_retryable());

        // These are not (boundary or invalid input)
        assert!(!NavReason::AlreadyDeepest.is_retryable());
        assert!(!NavReason::IndexOutOfRange.is_retryable());
    }

    #[test]
    fn nav_result_with_reason() {
        // noop() constructor uses reason's message
        let noop = NavResult::noop(NavReason::AtOldest);
        assert!(noop.is_noop());
        assert_eq!(noop.message(), "Already at oldest state.");

        // err() constructor uses reason's message
        let err = NavResult::err(NavReason::InvalidState);
        assert!(err.is_err());
        assert_eq!(
            err.message(),
            "Navigation state is invalid. Try returning to Galaxy."
        );

        // err_fmt() allows dynamic content
        let err_fmt = NavResult::err_fmt(NavReason::UnknownAlias, "Allianz");
        assert!(err_fmt.is_err());
        assert_eq!(err_fmt.message(), "Unknown ManCo: \"Allianz\".");
    }

    #[test]
    fn nav_result_suggestion() {
        // NoOp with known reason gets suggestion
        let noop = NavResult::noop(NavReason::MissingFocusManco);
        let suggestion = noop.suggestion();
        assert!(suggestion.is_some());
        let s = suggestion.unwrap();
        assert_eq!(s.hint, "Select a ManCo to zoom into");
        assert!(s.actions.contains(&"nav.select_manco".to_string()));

        // Ok results have no suggestion
        let ok = NavResult::Ok("Success");
        assert!(ok.suggestion().is_none());

        // Unknown error message has no suggestion
        let err = NavResult::Err("Something weird happened".into());
        assert!(err.suggestion().is_none());
    }

    #[test]
    fn nav_suggestion_for_reason() {
        // Each retryable reason should have a suggestion
        assert!(NavSuggestion::for_reason(NavReason::MissingFocusManco).is_some());
        assert!(NavSuggestion::for_reason(NavReason::MissingOrbitPos).is_some());
        assert!(NavSuggestion::for_reason(NavReason::EmptyHistory).is_some());
        assert!(NavSuggestion::for_reason(NavReason::InvalidState).is_some());

        // Boundary conditions may not have suggestions
        assert!(NavSuggestion::for_reason(NavReason::SelectionOutOfRange).is_none());
    }
}
