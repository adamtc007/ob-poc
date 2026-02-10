//! Agent command types for UI communication
//!
//! This module contains the `AgentCommand` enum and related types that define
//! the canonical vocabulary for agent → UI communication.

use serde::{Deserialize, Serialize};

/// Commands the agent can issue to the UI
/// This is the canonical vocabulary for agent → UI communication.
/// The LLM maps natural language ("run it", "undo that") to these commands.
///
/// Design: Blade Runner Esper-style - natural language voice commands for
/// graph navigation: "enhance", "track 45 right", "stop", "give me a hard copy"
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AgentCommand {
    // =========================================================================
    // REPL Commands
    // =========================================================================
    /// Execute accumulated DSL ("execute", "run it", "do it", "go")
    Execute,
    /// Undo last DSL block ("undo", "take that back", "never mind")
    Undo,
    /// Clear all accumulated DSL ("clear", "start over", "reset")
    Clear,
    /// Delete specific statement by index ("delete the second one", "remove that")
    Delete { index: u32 },
    /// Delete the last statement ("delete that", "remove the last one")
    DeleteLast,

    // =========================================================================
    // CBU & Entity Navigation
    // =========================================================================
    /// Show a specific CBU in the graph ("show me X fund", "load allianz")
    ShowCbu { cbu_id: String },
    /// Open CBU search popup with query pre-filled (for typos/no results)
    SearchCbu { query: String },
    /// Highlight an entity in the graph
    HighlightEntity { entity_id: String },
    /// Navigate to a line in the DSL panel
    NavigateDsl { line: u32 },
    /// Focus an AST node
    FocusAst { node_id: String },
    /// Focus on a specific entity ("center on john smith", "zoom to that director")
    FocusEntity {
        /// Entity ID or search term
        entity_id: String,
    },
    // =========================================================================
    // View Mode Commands
    // =========================================================================
    /// Set the view mode ("show kyc view", "switch to custody", "trading matrix")
    SetViewMode {
        /// View mode: "KYC_UBO", "SERVICE_DELIVERY", "CUSTODY", "TRADING_MATRIX"
        view_mode: String,
    },

    // =========================================================================
    // Zoom Commands (Esper-style)
    // =========================================================================
    /// Zoom in ("enhance", "zoom in", "closer", "magnify")
    ZoomIn {
        /// Optional zoom factor (1.0 = 100% = no change, 2.0 = 2x zoom)
        #[serde(default)]
        factor: Option<f32>,
    },
    /// Zoom out ("pull back", "zoom out", "wider")
    ZoomOut {
        /// Optional zoom factor
        #[serde(default)]
        factor: Option<f32>,
    },
    /// Fit entire graph to screen ("fit to screen", "show all", "bird's eye")
    ZoomFit,
    /// Zoom to specific level ("zoom to 50%", "set zoom 200%")
    ZoomTo {
        /// Zoom level as percentage (100 = 100%)
        level: f32,
    },

    // =========================================================================
    // Pan Commands (Esper-style)
    // =========================================================================
    /// Pan in a direction ("track left", "pan right", "move up")
    Pan {
        direction: PanDirection,
        /// Optional amount in pixels or relative units
        #[serde(default)]
        amount: Option<f32>,
    },
    /// Center view on current selection or graph center ("center", "home")
    Center,
    /// Stop all animation/movement ("stop", "hold", "freeze", "that's good")
    Stop,

    // =========================================================================
    // Hierarchy Navigation
    // =========================================================================
    /// Expand specific node ("expand allianz", "open that")
    ExpandNode { node_key: String },
    /// Collapse specific node ("collapse that", "close allianz")
    CollapseNode { node_key: String },

    // =========================================================================
    // Graph Filtering Commands
    // =========================================================================
    /// Filter graph to show only specific entity types ("show only shells", "filter to people")
    FilterByType {
        /// Entity type codes to show (e.g., ["SHELL", "PERSON"])
        type_codes: Vec<String>,
    },
    /// Highlight entities of a specific type without filtering others
    HighlightType {
        /// Entity type code to highlight (e.g., "SHELL")
        type_code: String,
    },
    /// Clear all graph filters and highlights
    ClearFilter,
    // =========================================================================
    // Export Commands ("Give me a hard copy")
    // =========================================================================
    /// Export current view ("give me a hard copy", "print", "screenshot")
    Export {
        /// Format: "png", "svg", "pdf"
        #[serde(default)]
        format: Option<String>,
    },

    // =========================================================================
    // Layout Commands
    // =========================================================================
    /// Reset layout to default ("reset layout", "auto arrange")
    ResetLayout,
    /// Toggle layout orientation ("flip", "rotate layout")
    ToggleOrientation,

    // =========================================================================
    // Search Commands
    // =========================================================================
    /// Search within graph ("find john", "search for director")
    Search { query: String },

    // =========================================================================
    // Scale Navigation (Astronomical metaphor for client book depth)
    // =========================================================================
    /// Show full universe/client book ("show universe", "full book", "god view")
    ScaleUniverse,
    /// Show galaxy/segment view ("hedge fund galaxy", "segment view")
    ScaleGalaxy {
        /// Optional segment filter (e.g., "hedge_fund", "pension")
        #[serde(default)]
        segment: Option<String>,
    },
    /// Show solar system/CBU view ("enter system", "cbu with satellites")
    ScaleSystem {
        /// Optional CBU ID to focus on
        #[serde(default)]
        cbu_id: Option<String>,
    },
    /// Land on planet/single entity ("land on", "focus entity")
    ScalePlanet {
        /// Entity ID to focus on
        #[serde(default)]
        entity_id: Option<String>,
    },
    /// Surface scan/entity details ("surface scan", "show attributes")
    ScaleSurface,
    /// Core sample/derived data ("core sample", "show hidden", "indirect ownership")
    ScaleCore,

    // =========================================================================
    // Depth Navigation (Z-axis through entity structures)
    // =========================================================================
    /// Drill all the way through to natural persons ("drill through", "find the humans")
    DrillThrough,
    /// Return to surface/top level ("surface", "come up", "back to top")
    SurfaceReturn,
    /// X-ray/transparent view showing all layers ("x-ray", "skeleton view")
    XRay,
    /// Peel one layer at a time ("peel", "next layer")
    Peel,
    /// Horizontal slice at current depth ("cross section", "peers at this level")
    CrossSection,
    /// Show depth indicator ("how deep", "what level")
    DepthIndicator,

    // =========================================================================
    // Orbital Navigation (Rotating around entities)
    // =========================================================================
    /// Orbit around entity showing all connections ("orbit", "360 view")
    Orbit {
        /// Entity ID to orbit around
        #[serde(default)]
        entity_id: Option<String>,
    },
    /// Rotate to different relationship layer ("rotate to ownership", "flip to control")
    RotateLayer {
        /// Layer to rotate to (e.g., "ownership", "control", "services")
        layer: String,
    },
    /// Flip view direction ("flip", "upstream vs downstream")
    Flip,
    /// Tilt view towards dimension ("tilt to time", "angle to services")
    Tilt {
        /// Dimension to tilt towards
        dimension: String,
    },

    // =========================================================================
    // Temporal Navigation (4th dimension - time)
    // =========================================================================
    /// Rewind to historical state ("rewind to", "as of date", "before restructure")
    TimeRewind {
        /// Target date (ISO format or relative like "last_quarter")
        #[serde(default)]
        target_date: Option<String>,
    },
    /// Play forward through time ("play", "animate changes", "show evolution")
    TimePlay {
        /// Start date
        #[serde(default)]
        from_date: Option<String>,
        /// End date
        #[serde(default)]
        to_date: Option<String>,
    },
    /// Freeze at current time ("freeze time", "lock date")
    TimeFreeze,
    /// Compare two time points ("time slice", "before after", "what changed")
    TimeSlice {
        /// First time point
        #[serde(default)]
        date1: Option<String>,
        /// Second time point
        #[serde(default)]
        date2: Option<String>,
    },
    /// Show full timeline/audit trail ("show trail", "history", "chronology")
    TimeTrail {
        /// Entity ID for entity-specific trail
        #[serde(default)]
        entity_id: Option<String>,
    },

    // =========================================================================
    // Investigation Patterns (Compound navigation intentions)
    // =========================================================================
    /// Trace money/ownership flow ("follow the money", "trace ownership")
    FollowTheMoney {
        /// Starting entity ID
        #[serde(default)]
        from_entity: Option<String>,
    },
    /// Trace control chain ("who controls", "find puppet master")
    WhoControls {
        /// Target entity ID
        #[serde(default)]
        entity_id: Option<String>,
    },
    /// Illuminate/highlight specific aspect ("illuminate ownership", "highlight risk")
    Illuminate {
        /// Aspect to illuminate (e.g., "ownership", "control", "risk", "changes")
        aspect: String,
    },
    /// Show shadow/indirect relationships ("show shadow", "indirect ownership")
    Shadow,
    /// Scan for red flags ("red flag scan", "show problems", "anomaly scan")
    RedFlagScan,
    /// Show black holes/data gaps ("black hole", "what's missing", "where does it go dark")
    BlackHole,

    // =========================================================================
    // Context Intentions (User declares purpose)
    // =========================================================================
    /// Set context to board/committee review mode
    ContextReview,
    /// Set context to investigation/forensic mode
    ContextInvestigation,
    /// Set context to onboarding/intake mode
    ContextOnboarding,
    /// Set context to monitoring/pkyc mode
    ContextMonitoring,
    /// Set context to remediation/gap-filling mode
    ContextRemediation,

    // =========================================================================
    // Taxonomy Navigation (Entity type hierarchy browsing)
    // =========================================================================
    /// Show current taxonomy position ("show taxonomy", "where am I")
    TaxonomyShow,
    /// Drill into a taxonomy node ("drill into shells", "explore funds")
    TaxonomyDrillIn {
        /// Node label or type to drill into (e.g., "SHELL", "FUND", "LIMITED_COMPANY")
        node_label: String,
    },
    /// Zoom out one level in taxonomy ("zoom out", "go back", "up one level")
    TaxonomyZoomOut,
    /// Reset taxonomy to root level ("reset taxonomy", "taxonomy home")
    TaxonomyReset,
    /// Filter taxonomy view by criteria ("filter to active", "show only funds")
    TaxonomyFilter {
        /// Filter expression
        filter: String,
    },
    /// Clear taxonomy filters ("clear taxonomy filter")
    TaxonomyClearFilter,

    // =========================================================================
    // Ring Navigation (Cluster view - CBUs orbiting ManCo)
    // =========================================================================
    /// Move to outer ring in cluster view ("ring out", "outer ring")
    RingOut,
    /// Move to inner ring in cluster view ("ring in", "inner ring")
    RingIn,
    /// Rotate clockwise within current ring ("clockwise", "cw", "next")
    Clockwise {
        /// Number of steps to rotate (default 1)
        #[serde(default)]
        steps: Option<u32>,
    },
    /// Rotate counterclockwise within current ring ("counterclockwise", "ccw", "prev")
    Counterclockwise {
        /// Number of steps to rotate (default 1)
        #[serde(default)]
        steps: Option<u32>,
    },
    /// Jump directly to a specific CBU ("snap to", "go to", "select")
    SnapTo {
        /// Target CBU name or ID
        target: String,
    },

    // =========================================================================
    // Help
    // =========================================================================
    /// Show help for navigation ("help", "what can I say")
    ShowHelp {
        #[serde(default)]
        topic: Option<String>,
    },

    // =========================================================================
    // Resolution Commands (Entity disambiguation)
    // =========================================================================
    /// Start entity resolution sub-session for ambiguous refs
    StartResolution {
        /// Sub-session ID
        subsession_id: String,
        /// Number of entities to resolve
        total_refs: usize,
    },
    /// Select a match in resolution (by index)
    ResolutionSelect {
        /// Selection index (0-based)
        selection: usize,
    },
    /// Skip current entity in resolution
    ResolutionSkip,
    /// Complete resolution and apply to parent session
    ResolutionComplete {
        /// Whether to apply resolutions (false to discard)
        #[serde(default = "default_true")]
        apply: bool,
    },
    /// Cancel resolution and return to parent session
    ResolutionCancel,
}

pub(crate) fn default_true() -> bool {
    true
}

/// Direction for pan commands
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PanDirection {
    Left,
    Right,
    Up,
    Down,
}
