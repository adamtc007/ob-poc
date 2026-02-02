//! ViewState - The "it" that session is looking at
//!
//! This module implements the visual state that:
//! - IS what the user sees
//! - IS what operations target
//! - IS what agent knows about
//!
//! Key insight: Session = Intent Scope = Visual State = Operation Target
//!
//! # Fractal Navigation
//!
//! ViewState now uses TaxonomyStack for fractal zoom navigation:
//! - `zoom_in(node_id)` - Push child taxonomy onto stack
//! - `zoom_out()` - Pop back to parent taxonomy
//! - `back_to(index)` - Jump to specific breadcrumb level

use anyhow::{anyhow, Result};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

use crate::taxonomy::{Filter, TaxonomyContext, TaxonomyNode, TaxonomyStack};

// =============================================================================
// VIEW STATE - The complete visual state
// =============================================================================

/// View state - the "it" that session is looking at
/// This IS what the user sees, what operations target, what agent knows about
///
/// # Fractal Navigation with TaxonomyStack
///
/// The ViewState now includes a `TaxonomyStack` for fractal zoom navigation:
/// - Each frame on the stack represents a "zoom level"
/// - `zoom_in(node_id)` expands a node into its child taxonomy (pushes frame)
/// - `zoom_out()` returns to the parent taxonomy (pops frame)
/// - `back_to(index)` jumps to a specific breadcrumb level
///
/// The `taxonomy` field always reflects the CURRENT frame's tree.
/// The stack provides the navigation history.
///
/// # Esper Navigation
///
/// Blade Runner-inspired navigation vocabulary with dedicated state fields:
/// - `scale_level`: Universe → Galaxy → System → Planet → Surface → Core
/// - `drill_*`: Hierarchical navigation (subsidiaries/parents)
/// - `trace_*`: Follow money, control, risk threads
/// - `temporal_*`: Historical point-in-time views
/// - `highlight_*`: Visual emphasis modes (xray, illuminate, shadow)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewState {
    // =========================================================================
    // CORE TAXONOMY/FRACTAL NAVIGATION
    // =========================================================================
    /// Navigation stack for fractal zoom (current view is top of stack)
    #[serde(skip)]
    pub stack: TaxonomyStack,

    /// The taxonomy tree (rebuilt on context change)
    /// NOTE: This is a convenience accessor that mirrors stack.current().tree
    pub taxonomy: TaxonomyNode,

    /// Current context (what built this taxonomy)
    pub context: TaxonomyContext,

    /// Active refinements ("except...", "plus...")
    pub refinements: Vec<Refinement>,

    /// Computed selection (the actual "those" after refinements)
    /// This is what operations target
    pub selection: Vec<Uuid>,

    /// Staged operation awaiting confirmation
    pub pending: Option<PendingOperation>,

    /// Layout result (computed positions for rendering)
    pub layout: Option<LayoutResult>,

    /// When this view was computed
    pub computed_at: DateTime<Utc>,

    // =========================================================================
    // ESPER NAVIGATION - Scale & Focus
    // =========================================================================
    /// Current scale level (Universe → Galaxy → System → Planet → Surface → Core)
    #[serde(default)]
    pub scale_level: ScaleLevel,

    /// Currently focused CBU (if any)
    pub focus_cbu_id: Option<Uuid>,

    /// Currently focused entity (if any)
    pub focus_entity_id: Option<Uuid>,

    /// How many relationship hops to show (default: 2)
    #[serde(default = "default_visible_depth")]
    pub visible_depth: u8,

    // =========================================================================
    // ESPER NAVIGATION - Drill Navigation
    // =========================================================================
    /// Esper nav stack for drill/surface operations
    #[serde(default)]
    pub nav_stack: Vec<NavStackEntry>,

    /// Current drill root entity
    pub drill_root: Option<Uuid>,

    /// Drill direction (down = subsidiaries, up = parents)
    #[serde(default)]
    pub drill_direction: DrillDirection,

    /// Current drill depth
    #[serde(default)]
    pub drill_depth: u8,

    // =========================================================================
    // ESPER NAVIGATION - Detail Level
    // =========================================================================
    /// Current detail level (Graph → Attributes → Raw)
    #[serde(default)]
    pub detail_level: DetailLevel,

    // =========================================================================
    // ESPER NAVIGATION - Layer Visibility (X-ray, Peel)
    // =========================================================================
    /// X-ray transparency mode active
    #[serde(default)]
    pub xray_mode: bool,

    /// Which layers are transparent in x-ray mode
    #[serde(default)]
    pub xray_layers: Vec<String>,

    /// Number of peeled layers
    #[serde(default)]
    pub peel_depth: u8,

    /// Which layers have been peeled
    #[serde(default)]
    pub peeled_layers: Vec<String>,

    // =========================================================================
    // ESPER NAVIGATION - Trace/Highlight Modes
    // =========================================================================
    /// Active trace mode (follow money, control, risk, etc.)
    pub trace_mode: Option<TraceMode>,

    /// Entity to trace from
    pub trace_from_entity: Option<Uuid>,

    /// Trace depth (how many hops)
    #[serde(default = "default_trace_depth")]
    pub trace_depth: u8,

    /// Highlight mode for control/ownership
    pub highlight_mode: Option<HighlightMode>,

    /// Illuminate aspect (risks, documents, gaps, etc.)
    pub illuminate_aspect: Option<IlluminateAspect>,

    // =========================================================================
    // ESPER NAVIGATION - Risk/Flag Highlighting
    // =========================================================================
    /// Shadow mode (dim non-risk items)
    #[serde(default)]
    pub shadow_mode: bool,

    /// Shadow threshold level
    #[serde(default)]
    pub shadow_threshold: RiskThreshold,

    /// Red flag scan active
    #[serde(default)]
    pub red_flag_scan_active: bool,

    /// Red flag category filter
    pub red_flag_category: Option<RedFlagCategory>,

    /// Black hole mode (show data gaps)
    #[serde(default)]
    pub black_hole_mode: bool,

    /// Black hole type filter
    pub black_hole_type: Option<GapType>,

    // =========================================================================
    // ESPER NAVIGATION - Temporal
    // =========================================================================
    /// Temporal mode (current, historical, comparison, etc.)
    #[serde(default)]
    pub temporal_mode: TemporalMode,

    /// Historical view date
    pub time_view_date: Option<NaiveDate>,

    /// Time slice comparison dates
    pub time_slice_dates: Option<(NaiveDate, NaiveDate)>,

    /// Entity for time trail
    pub time_trail_entity: Option<Uuid>,

    /// Time play animation active
    #[serde(default)]
    pub time_playing: bool,

    /// Time play range
    pub time_play_range: Option<(NaiveDate, NaiveDate)>,

    // =========================================================================
    // ESPER NAVIGATION - Context Mode
    // =========================================================================
    /// UI context mode (onboarding, review, investigation, etc.)
    #[serde(default)]
    pub context_mode: ContextMode,

    // =========================================================================
    // ESPER NAVIGATION - Orbit/Camera
    // =========================================================================
    /// Orbital navigation active
    #[serde(default)]
    pub orbit_active: bool,

    /// Orbit center entity
    pub orbit_center: Option<Uuid>,

    /// Orbit speed multiplier
    #[serde(default = "default_orbit_speed")]
    pub orbit_speed: f32,

    /// Camera target entity
    pub camera_target: Option<Uuid>,

    /// Tilt dimension emphasis
    pub tilt_dimension: Option<String>,

    /// Tilt amount (0.0 - 1.0)
    #[serde(default)]
    pub tilt_amount: f32,

    /// Perspective flipped
    #[serde(default)]
    pub flipped: bool,

    // =========================================================================
    // ESPER NAVIGATION - Cross Section
    // =========================================================================
    /// Cross section mode active
    #[serde(default)]
    pub cross_section_active: bool,

    /// Cross section axis
    #[serde(default)]
    pub cross_section_axis: CrossSectionAxis,

    /// Cross section position (0.0 - 1.0)
    #[serde(default = "default_cross_section_position")]
    pub cross_section_position: f32,

    /// Show depth indicator overlay
    #[serde(default)]
    pub show_depth_indicator: bool,
}

// =============================================================================
// DEFAULT VALUE HELPERS
// =============================================================================

fn default_visible_depth() -> u8 {
    2
}

fn default_trace_depth() -> u8 {
    5
}

fn default_orbit_speed() -> f32 {
    1.0
}

fn default_cross_section_position() -> f32 {
    0.5
}

// =============================================================================
// ESPER NAVIGATION TYPES
// =============================================================================

/// Scale level for Esper navigation (Universe → Core)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum ScaleLevel {
    /// All CBUs - highest level
    #[default]
    Universe,
    /// Segment/cluster of CBUs
    Galaxy,
    /// Single CBU
    System,
    /// Single Entity within CBU
    Planet,
    /// Entity attributes/details
    Surface,
    /// Raw data/JSON view
    Core,
}

/// Navigation stack entry for drill/surface operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavStackEntry {
    pub scale_level: ScaleLevel,
    pub focus_entity_id: Option<Uuid>,
    pub focus_cbu_id: Option<Uuid>,
    pub timestamp: DateTime<Utc>,
}

/// Drill direction for hierarchical navigation
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum DrillDirection {
    /// Drill into subsidiaries/children
    #[default]
    Down,
    /// Drill into parents/owners
    Up,
}

/// Detail level for entity views
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum DetailLevel {
    /// Normal graph view
    #[default]
    Graph,
    /// Expanded attribute cards
    Attributes,
    /// JSON/raw data view
    Raw,
}

/// Trace mode for investigation threads
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TraceMode {
    /// Follow money/financial flows
    Money,
    /// Follow control relationships
    Control,
    /// Follow risk indicators
    Risk,
    /// Follow document chains
    Documents,
    /// Follow alert threads
    Alerts,
}

/// Highlight mode for visual emphasis
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum HighlightMode {
    /// Highlight control relationships
    Control,
    /// Highlight ownership chain
    Ownership,
    /// Highlight risk indicators
    Risk,
}

/// Illuminate aspect for entity highlighting
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum IlluminateAspect {
    /// Highlight risks
    Risks,
    /// Highlight documents
    Documents,
    /// Highlight screenings
    Screenings,
    /// Highlight data gaps
    Gaps,
    /// Highlight pending items
    Pending,
}

/// Risk threshold for shadow mode
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum RiskThreshold {
    /// High risk only
    High,
    /// Medium and high risk
    Medium,
    /// Any risk level
    #[default]
    Any,
}

/// Red flag category filter
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RedFlagCategory {
    /// PEP flags
    Pep,
    /// Sanctions flags
    Sanctions,
    /// Adverse media flags
    AdverseMedia,
    /// All flag types
    All,
}

/// Gap type for black hole mode
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum GapType {
    /// Missing ownership data
    Ownership,
    /// Missing documents
    Documents,
    /// Missing screening
    Screening,
    /// All gap types
    All,
}

/// Temporal view mode
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum TemporalMode {
    /// Current/live view
    #[default]
    Current,
    /// Historical point-in-time
    Historical,
    /// Side-by-side comparison
    Comparison,
    /// Entity change trail
    Trail,
    /// Animated playback
    Playing,
}

/// Context mode for UI workflow adaptation
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum ContextMode {
    /// New client onboarding
    #[default]
    Onboarding,
    /// Periodic review
    Review,
    /// Investigation/deep dive
    Investigation,
    /// Ongoing monitoring
    Monitoring,
    /// Remediation workflow
    Remediation,
}

/// Cross section axis for slice views
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum CrossSectionAxis {
    /// Horizontal cut
    #[default]
    Horizontal,
    /// Vertical cut
    Vertical,
    /// Ownership dimension
    Ownership,
    /// Control dimension
    Control,
}

// =============================================================================
// REFINEMENTS - How selection is narrowed/expanded
// =============================================================================

/// Refinement operations that modify the selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Refinement {
    /// Add filter: "only the Luxembourg ones"
    Include { filter: Filter },

    /// Remove filter: "except under 100M"
    Exclude { filter: Filter },

    /// Add specific entities: "and also ABC Fund"
    Add { ids: Vec<Uuid> },

    /// Remove specific entities: "but not that one"
    Remove { ids: Vec<Uuid> },
}

// =============================================================================
// PENDING OPERATIONS - Staged operations awaiting confirmation
// =============================================================================

/// A staged operation awaiting user confirmation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingOperation {
    /// What operation
    pub operation: BatchOperation,

    /// Target IDs (from selection)
    pub targets: Vec<Uuid>,

    /// Generated DSL verbs
    pub verbs: String,

    /// Preview of what will happen
    pub preview: OperationPreview,

    /// When staged
    pub staged_at: DateTime<Utc>,
}

/// Batch operations that can be applied to selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BatchOperation {
    /// Subscribe selection to a product
    Subscribe { product: String },

    /// Unsubscribe selection from a product
    Unsubscribe { product: String },

    /// Set status on selection
    SetStatus { status: String },

    /// Assign role to entity across selection
    AssignRole { entity_id: Uuid, role: String },

    /// Create entities from research findings
    CreateFromResearch,

    /// Enrich existing entities from research
    EnrichFromResearch,

    /// Custom verb with arguments
    Custom {
        verb: String,
        args: HashMap<String, serde_json::Value>,
    },
}

/// Preview of what an operation will do
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationPreview {
    /// Summary text: "Add CUSTODY to 12 CBUs"
    pub summary: String,

    /// How many items affected
    pub affected_count: usize,

    /// How many already have it: "3 already have it"
    pub already_done_count: usize,

    /// How many would fail: "2 missing prerequisites"
    pub would_fail_count: usize,

    /// Estimated duration
    #[serde(
        serialize_with = "serialize_duration_opt",
        deserialize_with = "deserialize_duration_opt"
    )]
    pub estimated_duration: Option<Duration>,
}

// =============================================================================
// LAYOUT RESULT - Computed positions for rendering
// =============================================================================

/// Layout result from positioning algorithm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutResult {
    /// Node positions keyed by node ID
    pub positions: HashMap<Uuid, NodePosition>,

    /// Bounds of the layout
    pub bounds: LayoutBounds,

    /// Layout algorithm used
    pub algorithm: String,

    /// When computed
    pub computed_at: DateTime<Utc>,
}

/// Position of a single node
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct NodePosition {
    pub x: f32,
    pub y: f32,
    pub z: f32, // For 3D/layered views
    pub radius: f32,
}

/// Bounding box of layout
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LayoutBounds {
    pub min_x: f32,
    pub max_x: f32,
    pub min_y: f32,
    pub max_y: f32,
    pub min_z: f32,
    pub max_z: f32,
}

// =============================================================================
// VIEW STATE IMPLEMENTATION
// =============================================================================

impl ViewState {
    /// Create empty view state (no taxonomy)
    pub fn empty() -> Self {
        let taxonomy = TaxonomyNode::empty_root();
        Self {
            // Core taxonomy/fractal navigation
            stack: TaxonomyStack::with_root(taxonomy.clone()),
            taxonomy,
            context: TaxonomyContext::Universe,
            refinements: Vec::new(),
            selection: Vec::new(),
            pending: None,
            layout: None,
            computed_at: Utc::now(),

            // Esper navigation - Scale & Focus
            scale_level: ScaleLevel::default(),
            focus_cbu_id: None,
            focus_entity_id: None,
            visible_depth: default_visible_depth(),

            // Esper navigation - Drill
            nav_stack: Vec::new(),
            drill_root: None,
            drill_direction: DrillDirection::default(),
            drill_depth: 0,

            // Esper navigation - Detail Level
            detail_level: DetailLevel::default(),

            // Esper navigation - Layer Visibility
            xray_mode: false,
            xray_layers: Vec::new(),
            peel_depth: 0,
            peeled_layers: Vec::new(),

            // Esper navigation - Trace/Highlight
            trace_mode: None,
            trace_from_entity: None,
            trace_depth: default_trace_depth(),
            highlight_mode: None,
            illuminate_aspect: None,

            // Esper navigation - Risk/Flag
            shadow_mode: false,
            shadow_threshold: RiskThreshold::default(),
            red_flag_scan_active: false,
            red_flag_category: None,
            black_hole_mode: false,
            black_hole_type: None,

            // Esper navigation - Temporal
            temporal_mode: TemporalMode::default(),
            time_view_date: None,
            time_slice_dates: None,
            time_trail_entity: None,
            time_playing: false,
            time_play_range: None,

            // Esper navigation - Context Mode
            context_mode: ContextMode::default(),

            // Esper navigation - Orbit/Camera
            orbit_active: false,
            orbit_center: None,
            orbit_speed: default_orbit_speed(),
            camera_target: None,
            tilt_dimension: None,
            tilt_amount: 0.0,
            flipped: false,

            // Esper navigation - Cross Section
            cross_section_active: false,
            cross_section_axis: CrossSectionAxis::default(),
            cross_section_position: default_cross_section_position(),
            show_depth_indicator: false,
        }
    }

    /// Create from taxonomy and context
    pub fn from_taxonomy(taxonomy: TaxonomyNode, context: TaxonomyContext) -> Self {
        // Initial selection is all IDs in taxonomy
        let selection = taxonomy.all_ids();

        Self {
            // Core taxonomy/fractal navigation
            stack: TaxonomyStack::with_root(taxonomy.clone()),
            taxonomy,
            context,
            refinements: Vec::new(),
            selection,
            pending: None,
            layout: None,
            computed_at: Utc::now(),

            // Esper navigation - Scale & Focus
            scale_level: ScaleLevel::default(),
            focus_cbu_id: None,
            focus_entity_id: None,
            visible_depth: default_visible_depth(),

            // Esper navigation - Drill
            nav_stack: Vec::new(),
            drill_root: None,
            drill_direction: DrillDirection::default(),
            drill_depth: 0,

            // Esper navigation - Detail Level
            detail_level: DetailLevel::default(),

            // Esper navigation - Layer Visibility
            xray_mode: false,
            xray_layers: Vec::new(),
            peel_depth: 0,
            peeled_layers: Vec::new(),

            // Esper navigation - Trace/Highlight
            trace_mode: None,
            trace_from_entity: None,
            trace_depth: default_trace_depth(),
            highlight_mode: None,
            illuminate_aspect: None,

            // Esper navigation - Risk/Flag
            shadow_mode: false,
            shadow_threshold: RiskThreshold::default(),
            red_flag_scan_active: false,
            red_flag_category: None,
            black_hole_mode: false,
            black_hole_type: None,

            // Esper navigation - Temporal
            temporal_mode: TemporalMode::default(),
            time_view_date: None,
            time_slice_dates: None,
            time_trail_entity: None,
            time_playing: false,
            time_play_range: None,

            // Esper navigation - Context Mode
            context_mode: ContextMode::default(),

            // Esper navigation - Orbit/Camera
            orbit_active: false,
            orbit_center: None,
            orbit_speed: default_orbit_speed(),
            camera_target: None,
            tilt_dimension: None,
            tilt_amount: 0.0,
            flipped: false,

            // Esper navigation - Cross Section
            cross_section_active: false,
            cross_section_axis: CrossSectionAxis::default(),
            cross_section_position: default_cross_section_position(),
            show_depth_indicator: false,
        }
    }

    /// Apply a refinement, recomputing selection
    pub fn refine(&mut self, refinement: Refinement) {
        // Store the refinement
        self.refinements.push(refinement);

        // Recompute selection from scratch
        self.recompute_selection();
    }

    /// Clear all refinements, restore full selection
    pub fn clear_refinements(&mut self) {
        self.refinements.clear();
        self.selection = self.taxonomy.all_ids();
    }

    /// Recompute selection based on current refinements
    fn recompute_selection(&mut self) {
        // Start with all IDs
        let mut selection: Vec<Uuid> = self.taxonomy.all_ids();

        // Apply each refinement in order
        for refinement in &self.refinements {
            match refinement {
                Refinement::Include { filter } => {
                    // Keep only items matching filter
                    selection.retain(|id| {
                        self.taxonomy
                            .find(*id)
                            .is_some_and(|node| filter.matches(&node.dimensions))
                    });
                }
                Refinement::Exclude { filter } => {
                    // Remove items matching filter
                    selection.retain(|id| {
                        self.taxonomy
                            .find(*id)
                            .is_none_or(|node| !filter.matches(&node.dimensions))
                    });
                }
                Refinement::Add { ids } => {
                    // Add specific IDs (if they exist in taxonomy)
                    for id in ids {
                        if self.taxonomy.find(*id).is_some() && !selection.contains(id) {
                            selection.push(*id);
                        }
                    }
                }
                Refinement::Remove { ids } => {
                    // Remove specific IDs
                    selection.retain(|id| !ids.contains(id));
                }
            }
        }

        self.selection = selection;
    }

    /// Stage an operation for confirmation
    pub fn stage_operation(&mut self, operation: BatchOperation) -> Result<()> {
        if self.selection.is_empty() {
            return Err(anyhow!("No selection to operate on"));
        }

        let targets = self.selection.clone();
        let verbs = self.generate_verbs(&operation, &targets);
        let preview = self.compute_preview(&operation, &targets);

        self.pending = Some(PendingOperation {
            operation,
            targets,
            verbs,
            preview,
            staged_at: Utc::now(),
        });

        Ok(())
    }

    /// Clear pending operation
    pub fn clear_pending(&mut self) {
        self.pending = None;
    }

    /// Get selection count
    pub fn selection_count(&self) -> usize {
        self.selection.len()
    }

    /// Check if has pending operation
    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Generate DSL verbs for an operation
    fn generate_verbs(&self, operation: &BatchOperation, targets: &[Uuid]) -> String {
        let mut lines = Vec::new();

        for target in targets {
            let verb = match operation {
                BatchOperation::Subscribe { product } => {
                    format!(
                        "(cbu.add-product :cbu-id \"{}\" :product \"{}\")",
                        target, product
                    )
                }
                BatchOperation::Unsubscribe { product } => {
                    format!(
                        "(cbu.remove-product :cbu-id \"{}\" :product \"{}\")",
                        target, product
                    )
                }
                BatchOperation::SetStatus { status } => {
                    format!("(cbu.update :cbu-id \"{}\" :status \"{}\")", target, status)
                }
                BatchOperation::AssignRole { entity_id, role } => {
                    format!(
                        "(cbu.assign-role :cbu-id \"{}\" :entity-id \"{}\" :role \"{}\")",
                        target, entity_id, role
                    )
                }
                BatchOperation::CreateFromResearch => {
                    format!("(research.execute-create :finding-id \"{}\")", target)
                }
                BatchOperation::EnrichFromResearch => {
                    format!("(research.execute-enrich :finding-id \"{}\")", target)
                }
                BatchOperation::Custom { verb, args } => {
                    let args_str: Vec<String> =
                        args.iter().map(|(k, v)| format!(":{} {}", k, v)).collect();
                    format!(
                        "({} :target-id \"{}\" {})",
                        verb,
                        target,
                        args_str.join(" ")
                    )
                }
            };
            lines.push(verb);
        }

        lines.join("\n")
    }

    /// Compute preview for an operation
    fn compute_preview(&self, operation: &BatchOperation, targets: &[Uuid]) -> OperationPreview {
        let summary = match operation {
            BatchOperation::Subscribe { product } => {
                format!("Add {} to {} CBUs", product, targets.len())
            }
            BatchOperation::Unsubscribe { product } => {
                format!("Remove {} from {} CBUs", product, targets.len())
            }
            BatchOperation::SetStatus { status } => {
                format!("Set status to {} for {} items", status, targets.len())
            }
            BatchOperation::AssignRole { role, .. } => {
                format!("Assign {} role to {} CBUs", role, targets.len())
            }
            BatchOperation::CreateFromResearch => {
                format!("Create {} entities from research", targets.len())
            }
            BatchOperation::EnrichFromResearch => {
                format!("Enrich {} entities from research", targets.len())
            }
            BatchOperation::Custom { verb, .. } => {
                format!("Execute {} on {} items", verb, targets.len())
            }
        };

        // TODO: These would be computed from actual database state
        OperationPreview {
            summary,
            affected_count: targets.len(),
            already_done_count: 0,
            would_fail_count: 0,
            estimated_duration: Some(Duration::from_millis(targets.len() as u64 * 50)),
        }
    }

    /// Get taxonomy metaphor
    pub fn metaphor(&self) -> crate::taxonomy::Metaphor {
        self.taxonomy.metaphor()
    }

    /// Get taxonomy astro level
    pub fn astro_level(&self) -> crate::taxonomy::AstroLevel {
        self.taxonomy.astro_level()
    }

    // =========================================================================
    // ESPER NAVIGATION METHODS
    // =========================================================================

    /// Push current Esper state onto navigation stack before drilling
    pub fn push_nav_stack(&mut self) {
        self.nav_stack.push(NavStackEntry {
            scale_level: self.scale_level,
            focus_entity_id: self.focus_entity_id,
            focus_cbu_id: self.focus_cbu_id,
            timestamp: Utc::now(),
        });
    }

    /// Pop Esper navigation stack (for surface/back operations)
    pub fn pop_nav_stack(&mut self) -> Option<NavStackEntry> {
        self.nav_stack.pop()
    }

    /// Drill into an entity (push current state, focus new entity)
    pub fn drill_into(&mut self, entity_id: Uuid, direction: DrillDirection) {
        // Push current state before drilling
        self.push_nav_stack();

        // Update drill state
        self.drill_root = Some(entity_id);
        self.drill_direction = direction;
        self.drill_depth += 1;
        self.focus_entity_id = Some(entity_id);

        // Advance scale level
        self.scale_level = match self.scale_level {
            ScaleLevel::Universe => ScaleLevel::Galaxy,
            ScaleLevel::Galaxy => ScaleLevel::System,
            ScaleLevel::System => ScaleLevel::Planet,
            ScaleLevel::Planet => ScaleLevel::Surface,
            ScaleLevel::Surface => ScaleLevel::Core,
            ScaleLevel::Core => ScaleLevel::Core, // Can't go deeper
        };
    }

    /// Surface up one level (pop navigation stack)
    pub fn surface_up(&mut self) -> bool {
        if let Some(entry) = self.pop_nav_stack() {
            self.scale_level = entry.scale_level;
            self.focus_entity_id = entry.focus_entity_id;
            self.focus_cbu_id = entry.focus_cbu_id;
            self.drill_depth = self.drill_depth.saturating_sub(1);
            if self.drill_depth == 0 {
                self.drill_root = None;
            }
            true
        } else {
            false
        }
    }

    /// Clear Esper navigation stack and return to universe view
    pub fn surface_to_universe(&mut self) {
        self.nav_stack.clear();
        self.drill_root = None;
        self.drill_depth = 0;
        self.scale_level = ScaleLevel::Universe;
        self.focus_entity_id = None;
        self.focus_cbu_id = None;
        self.detail_level = DetailLevel::Graph;
    }

    /// Clear all trace/highlight modes
    pub fn clear_traces(&mut self) {
        self.trace_mode = None;
        self.trace_from_entity = None;
        self.highlight_mode = None;
        self.illuminate_aspect = None;
        self.shadow_mode = false;
        self.red_flag_scan_active = false;
        self.black_hole_mode = false;
    }

    /// Start a trace from the current focus entity
    pub fn start_trace(&mut self, mode: TraceMode, from_entity: Option<Uuid>) {
        self.trace_mode = Some(mode);
        self.trace_from_entity = from_entity.or(self.focus_entity_id);
    }

    /// Enable x-ray mode for specified layers
    pub fn enable_xray(&mut self, layers: Vec<String>) {
        self.xray_mode = true;
        self.xray_layers = layers;
    }

    /// Disable x-ray mode
    pub fn disable_xray(&mut self) {
        self.xray_mode = false;
        self.xray_layers.clear();
    }

    /// Peel a layer (increment peel depth)
    pub fn peel_layer(&mut self, layer: String) {
        if !self.peeled_layers.contains(&layer) {
            self.peeled_layers.push(layer);
            self.peel_depth += 1;
        }
    }

    /// Unpeel all layers
    pub fn unpeel_all(&mut self) {
        self.peeled_layers.clear();
        self.peel_depth = 0;
    }

    /// Enable shadow mode (dim non-risk items)
    pub fn enable_shadow(&mut self, threshold: RiskThreshold) {
        self.shadow_mode = true;
        self.shadow_threshold = threshold;
    }

    /// Disable shadow mode
    pub fn disable_shadow(&mut self) {
        self.shadow_mode = false;
    }

    /// Start red flag scan
    pub fn start_red_flag_scan(&mut self, category: Option<RedFlagCategory>) {
        self.red_flag_scan_active = true;
        self.red_flag_category = category;
    }

    /// Stop red flag scan
    pub fn stop_red_flag_scan(&mut self) {
        self.red_flag_scan_active = false;
        self.red_flag_category = None;
    }

    /// Enable black hole mode (show data gaps)
    pub fn enable_black_holes(&mut self, gap_type: Option<GapType>) {
        self.black_hole_mode = true;
        self.black_hole_type = gap_type;
    }

    /// Disable black hole mode
    pub fn disable_black_holes(&mut self) {
        self.black_hole_mode = false;
        self.black_hole_type = None;
    }

    /// Set temporal mode to historical point-in-time
    pub fn set_historical_view(&mut self, date: NaiveDate) {
        self.temporal_mode = TemporalMode::Historical;
        self.time_view_date = Some(date);
    }

    /// Set temporal mode to comparison
    pub fn set_comparison_view(&mut self, date1: NaiveDate, date2: NaiveDate) {
        self.temporal_mode = TemporalMode::Comparison;
        self.time_slice_dates = Some((date1, date2));
    }

    /// Start time trail for an entity
    pub fn start_time_trail(&mut self, entity_id: Uuid) {
        self.temporal_mode = TemporalMode::Trail;
        self.time_trail_entity = Some(entity_id);
    }

    /// Reset to current/live view
    pub fn reset_temporal(&mut self) {
        self.temporal_mode = TemporalMode::Current;
        self.time_view_date = None;
        self.time_slice_dates = None;
        self.time_trail_entity = None;
        self.time_playing = false;
        self.time_play_range = None;
    }

    /// Start orbit around an entity
    pub fn start_orbit(&mut self, center: Uuid, speed: f32) {
        self.orbit_active = true;
        self.orbit_center = Some(center);
        self.orbit_speed = speed;
    }

    /// Stop orbit
    pub fn stop_orbit(&mut self) {
        self.orbit_active = false;
    }

    /// Fly camera to target entity
    pub fn fly_to(&mut self, target: Uuid) {
        self.camera_target = Some(target);
    }

    /// Flip perspective
    pub fn flip_perspective(&mut self) {
        self.flipped = !self.flipped;
    }

    /// Enable cross section view
    pub fn enable_cross_section(&mut self, axis: CrossSectionAxis, position: f32) {
        self.cross_section_active = true;
        self.cross_section_axis = axis;
        self.cross_section_position = position.clamp(0.0, 1.0);
    }

    /// Disable cross section view
    pub fn disable_cross_section(&mut self) {
        self.cross_section_active = false;
    }

    /// Set context mode (changes UI workflow adaptation)
    pub fn set_context_mode(&mut self, mode: ContextMode) {
        self.context_mode = mode;
    }

    /// Set detail level
    pub fn set_detail_level(&mut self, level: DetailLevel) {
        self.detail_level = level;
    }

    /// Illuminate a specific aspect
    pub fn illuminate(&mut self, aspect: IlluminateAspect) {
        self.illuminate_aspect = Some(aspect);
    }

    /// Clear illumination
    pub fn clear_illumination(&mut self) {
        self.illuminate_aspect = None;
    }

    // =========================================================================
    // FRACTAL NAVIGATION - Zoom in/out via TaxonomyStack
    // =========================================================================

    /// Zoom into a node, expanding it into its child taxonomy.
    ///
    /// If the node has an ExpansionRule::Parser, the parser is invoked
    /// to build the child taxonomy and push it onto the stack.
    ///
    /// Returns Ok(true) if zoom succeeded, Ok(false) if node not expandable.
    pub async fn zoom_in(&mut self, node_id: Uuid) -> Result<bool> {
        use crate::taxonomy::{ExpansionRule, TaxonomyFrame};

        // Find the node in current taxonomy
        let node = self
            .taxonomy
            .find(node_id)
            .ok_or_else(|| anyhow!("Node {} not found in current taxonomy", node_id))?;

        // Check expansion rule
        match &node.expansion {
            ExpansionRule::Parser(parser) => {
                // Parse child taxonomy
                let child_tree = parser.parse_for(node_id).await.map_err(|e| {
                    anyhow!("Failed to parse child taxonomy for {}: {}", node_id, e)
                })?;

                // Create new frame using from_zoom
                let frame = TaxonomyFrame::from_zoom(
                    node_id,
                    &node.label,
                    child_tree.clone(),
                    Some(parser.clone_arc()),
                );

                // Push onto stack (ignore max depth error, just return false)
                if self.stack.push(frame).is_err() {
                    return Ok(false);
                }

                // Update convenience accessor
                self.taxonomy = child_tree.clone();
                self.selection = child_tree.all_ids();
                self.refinements.clear();
                self.layout = None;
                self.computed_at = Utc::now();

                Ok(true)
            }
            ExpansionRule::Context(ctx) => {
                // Context-based expansion - would need a builder
                // For now, return false (not directly expandable)
                tracing::debug!(
                    "Node {} has Context expansion, not directly expandable: {:?}",
                    node_id,
                    ctx
                );
                Ok(false)
            }
            ExpansionRule::Complete | ExpansionRule::Terminal => {
                // Not expandable
                Ok(false)
            }
        }
    }

    /// Zoom out to the parent taxonomy.
    ///
    /// Pops the current frame from the stack and restores the parent view.
    /// Returns Ok(true) if zoom out succeeded, Ok(false) if already at root.
    pub fn zoom_out(&mut self) -> Result<bool> {
        if self.stack.depth() <= 1 {
            // Already at root, can't zoom out further
            return Ok(false);
        }

        // Pop current frame
        self.stack.pop();

        // Update from new current frame
        if let Some(frame) = self.stack.current() {
            self.taxonomy = frame.tree.clone();
            self.selection = frame.selection.clone();
            self.refinements.clear();
            self.layout = None;
            self.computed_at = Utc::now();
        }

        Ok(true)
    }

    /// Jump back to a specific breadcrumb level.
    ///
    /// `depth` is 0-indexed: 0 = root, 1 = first zoom, etc.
    /// Returns Ok(true) if jump succeeded, Ok(false) if invalid depth.
    pub fn back_to(&mut self, depth: usize) -> Result<bool> {
        if depth >= self.stack.depth() {
            return Ok(false);
        }

        // Pop down to target depth
        self.stack.pop_to_depth(depth + 1); // +1 because depth is 0-indexed but we want to keep that frame

        // Update from new current frame
        if let Some(frame) = self.stack.current() {
            self.taxonomy = frame.tree.clone();
            self.selection = frame.selection.clone();
            self.refinements.clear();
            self.layout = None;
            self.computed_at = Utc::now();
        }

        Ok(true)
    }

    /// Get breadcrumbs for navigation display.
    ///
    /// Returns a list of labels from root to current.
    pub fn breadcrumbs(&self) -> Vec<String> {
        self.stack.breadcrumbs()
    }

    /// Get breadcrumbs with frame IDs for navigation.
    ///
    /// Returns a list of (label, frame_id) pairs from root to current.
    pub fn breadcrumbs_with_ids(&self) -> Vec<(String, Uuid)> {
        self.stack
            .frames()
            .iter()
            .map(|f| (f.label.clone(), f.frame_id))
            .collect()
    }

    /// Get current zoom depth (0 = root level).
    pub fn zoom_depth(&self) -> usize {
        self.stack.depth().saturating_sub(1)
    }

    /// Check if we can zoom out (not at root).
    pub fn can_zoom_out(&self) -> bool {
        self.stack.depth() > 1
    }

    /// Check if a node can be zoomed into.
    pub fn can_zoom_in(&self, node_id: Uuid) -> bool {
        use crate::taxonomy::ExpansionRule;

        self.taxonomy
            .find(node_id)
            .is_some_and(|node| matches!(node.expansion, ExpansionRule::Parser(_)))
    }
}

impl LayoutBounds {
    /// Create default bounds
    pub fn default_bounds() -> Self {
        Self {
            min_x: -1000.0,
            max_x: 1000.0,
            min_y: -1000.0,
            max_y: 1000.0,
            min_z: 0.0,
            max_z: 100.0,
        }
    }

    /// Width of bounds
    pub fn width(&self) -> f32 {
        self.max_x - self.min_x
    }

    /// Height of bounds
    pub fn height(&self) -> f32 {
        self.max_y - self.min_y
    }

    /// Depth of bounds
    pub fn depth(&self) -> f32 {
        self.max_z - self.min_z
    }
}

// =============================================================================
// SERDE HELPERS FOR DURATION
// =============================================================================

fn serialize_duration_opt<S>(duration: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match duration {
        Some(d) => serializer.serialize_some(&d.as_millis()),
        None => serializer.serialize_none(),
    }
}

fn deserialize_duration_opt<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt: Option<u64> = Option::deserialize(deserializer)?;
    Ok(opt.map(Duration::from_millis))
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::taxonomy::{DimensionValues, NodeType, Status};

    fn make_test_taxonomy() -> TaxonomyNode {
        // Create a small test taxonomy
        let mut root = TaxonomyNode::new(
            Uuid::now_v7(),
            NodeType::Root,
            "Universe".to_string(),
            DimensionValues::default(),
        );

        // Add some children with different dimensions
        for i in 0..5 {
            let jurisdiction = if i % 2 == 0 { "LU" } else { "IE" };
            let status = if i % 3 == 0 {
                Status::Green
            } else {
                Status::Amber
            };

            let dims = DimensionValues {
                jurisdiction: Some(jurisdiction.to_string()),
                status: Some(status),
                ..Default::default()
            };

            let child =
                TaxonomyNode::new(Uuid::now_v7(), NodeType::Cbu, format!("CBU {}", i), dims);
            root.children.push(child);
        }

        root.compute_metrics();
        root
    }

    #[test]
    fn test_view_state_creation() {
        let taxonomy = make_test_taxonomy();
        let view = ViewState::from_taxonomy(taxonomy, TaxonomyContext::Universe);

        // Should have all IDs in selection (root + 5 children = 6)
        assert_eq!(view.selection.len(), 6);
        assert!(view.refinements.is_empty());
        assert!(view.pending.is_none());
    }

    #[test]
    fn test_refinement_include() {
        let taxonomy = make_test_taxonomy();
        let mut view = ViewState::from_taxonomy(taxonomy, TaxonomyContext::Universe);

        let original_count = view.selection.len();

        // Include only Luxembourg
        view.refine(Refinement::Include {
            filter: Filter::Jurisdiction(vec!["LU".to_string()]),
        });

        // Should have fewer items
        assert!(view.selection.len() < original_count);
    }

    #[test]
    fn test_refinement_exclude() {
        let taxonomy = make_test_taxonomy();
        let mut view = ViewState::from_taxonomy(taxonomy, TaxonomyContext::Universe);

        let original_count = view.selection.len();

        // Exclude green status
        view.refine(Refinement::Exclude {
            filter: Filter::Status(vec![Status::Green]),
        });

        // Should have fewer items
        assert!(view.selection.len() < original_count);
    }

    #[test]
    fn test_clear_refinements() {
        let taxonomy = make_test_taxonomy();
        let mut view = ViewState::from_taxonomy(taxonomy, TaxonomyContext::Universe);

        let original_count = view.selection.len();

        // Add some refinements
        view.refine(Refinement::Include {
            filter: Filter::Jurisdiction(vec!["LU".to_string()]),
        });
        assert!(view.selection.len() < original_count);

        // Clear
        view.clear_refinements();
        assert_eq!(view.selection.len(), original_count);
    }

    #[test]
    fn test_stage_operation() {
        let taxonomy = make_test_taxonomy();
        let mut view = ViewState::from_taxonomy(taxonomy, TaxonomyContext::Universe);

        // Stage an operation
        view.stage_operation(BatchOperation::Subscribe {
            product: "CUSTODY".to_string(),
        })
        .unwrap();

        assert!(view.has_pending());
        let pending = view.pending.as_ref().unwrap();
        assert_eq!(pending.targets.len(), view.selection.len());
        assert!(pending.verbs.contains("cbu.add-product"));
    }

    #[test]
    fn test_stage_operation_empty_selection() {
        let taxonomy = make_test_taxonomy();
        let mut view = ViewState::from_taxonomy(taxonomy, TaxonomyContext::Universe);

        // Clear selection
        view.selection.clear();

        // Should fail
        let result = view.stage_operation(BatchOperation::Subscribe {
            product: "CUSTODY".to_string(),
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_add_remove_refinements() {
        let taxonomy = make_test_taxonomy();
        let mut view = ViewState::from_taxonomy(taxonomy, TaxonomyContext::Universe);

        let new_id = Uuid::now_v7();

        // Remove an existing ID
        let first_id = view.selection[0];
        view.refine(Refinement::Remove {
            ids: vec![first_id],
        });
        assert!(!view.selection.contains(&first_id));

        // Try to add a non-existent ID (should not be added)
        let before_count = view.selection.len();
        view.refine(Refinement::Add { ids: vec![new_id] });
        assert_eq!(view.selection.len(), before_count); // unchanged
    }
}
