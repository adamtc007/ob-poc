//! Viewport Focus State Machine Types
//!
//! Core types for the CBU viewport navigation system following Esper-inspired patterns:
//! - ENHANCE - Polymorphic detail increase based on focus context
//! - TRACK - Lock and follow entity
//! - NAVIGATE - Spatial movement without changing focus
//! - ASCEND/DESCEND - Hierarchical focus stack navigation

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// ============================================================================
// REFERENCE TYPES
// ============================================================================

/// Reference to a CBU container
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CbuRef(pub Uuid);

impl CbuRef {
    pub fn new(id: Uuid) -> Self {
        Self(id)
    }
}

/// Reference to a concrete entity (Company, Partnership, Trust, Person)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConcreteEntityRef {
    pub id: Uuid,
    pub entity_type: ConcreteEntityType,
}

/// Types of concrete entities within a CBU
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConcreteEntityType {
    Company,
    Partnership,
    Trust,
    Person,
}

/// Reference to a Product or Service link
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProductServiceRef {
    Product { id: Uuid },
    Service { id: Uuid },
    ServiceResource { id: Uuid },
}

/// Reference to an Instrument Matrix
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InstrumentMatrixRef(pub Uuid);

/// Instrument type within a matrix
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InstrumentType {
    Equity,
    FixedIncome,
    Derivative,
    Fund,
    Cash,
    Commodity,
    Fx,
    StructuredProduct,
}

/// Reference to a config node (MIC, BIC, Pricing, etc.)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConfigNodeRef {
    Mic { code: String },
    Bic { code: String },
    Pricing { id: Uuid },
    Restrictions { id: Uuid },
}

// ============================================================================
// CONFIDENCE ZONE
// ============================================================================

/// Confidence zone for soft-edged membership rendering
///
/// Determines visual treatment:
/// - Core: Solid rendering, high certainty
/// - Shell: Normal rendering, good confidence
/// - Penumbra: Dashed/faded rendering, needs verification
/// - Speculative: Ghost rendering, low confidence
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfidenceZone {
    /// >= 0.95 confidence - solid rendering
    Core,
    /// >= 0.70 confidence - normal rendering
    Shell,
    /// >= 0.40 confidence - dashed/faded rendering
    Penumbra,
    /// < 0.40 confidence - ghost/speculative rendering
    Speculative,
}

impl ConfidenceZone {
    /// Compute zone from a confidence score (0.0 - 1.0)
    pub fn from_score(score: f32) -> Self {
        match score {
            x if x >= 0.95 => Self::Core,
            x if x >= 0.70 => Self::Shell,
            x if x >= 0.40 => Self::Penumbra,
            _ => Self::Speculative,
        }
    }

    /// Get the minimum confidence score for this zone
    pub fn min_score(&self) -> f32 {
        match self {
            Self::Core => 0.95,
            Self::Shell => 0.70,
            Self::Penumbra => 0.40,
            Self::Speculative => 0.0,
        }
    }

    /// Get opacity multiplier for rendering
    pub fn opacity(&self) -> f32 {
        match self {
            Self::Core => 1.0,
            Self::Shell => 0.85,
            Self::Penumbra => 0.6,
            Self::Speculative => 0.35,
        }
    }

    /// Whether to use dashed stroke
    pub fn is_dashed(&self) -> bool {
        matches!(self, Self::Penumbra | Self::Speculative)
    }
}

// ============================================================================
// FOCUS MODE
// ============================================================================

/// How focus behaves during navigation
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FocusMode {
    /// Focus stays on entity when panning
    #[default]
    Sticky,
    /// Focus transfers to nearest entity within radius
    Proximity { radius: f32 },
    /// Focus clears when entity leaves center region
    CenterLock { region_pct: f32 },
    /// Explicit focus changes only
    Manual,
}

// ============================================================================
// VIEWPORT FOCUS STATE
// ============================================================================

/// Hierarchical focus state machine for CBU viewport navigation
///
/// The focus state represents what the user is currently viewing/inspecting.
/// Each level maintains enhance levels for progressive disclosure.
///
/// Hierarchy:
/// ```text
/// None
///   └── CbuContainer (L0-L2)
///         ├── CbuEntity (L0-L4)
///         ├── CbuProductService (L0-L3)
///         └── InstrumentMatrix (L0-L2)
///               └── InstrumentType (L0-L3)
///                     └── ConfigNode (L0-L2)
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "focus_type", rename_all = "snake_case")]
pub enum ViewportFocusState {
    /// No focus - overview mode
    #[default]
    None,

    /// CBU container level focus
    CbuContainer {
        cbu: CbuRef,
        /// Enhance level 0-2:
        /// - L0: Collapsed badge + jurisdiction flag
        /// - L1: Category counts visible
        /// - L2: Entity nodes visible
        enhance_level: u8,
    },

    /// Entity within CBU focus (Company, Partnership, Trust, Person)
    CbuEntity {
        cbu: CbuRef,
        entity: ConcreteEntityRef,
        /// Entity enhance level 0-4:
        /// - L0: Name + type badge
        /// - L1: Jurisdiction, status
        /// - L2: 1-hop relationships
        /// - L3: Key attributes
        /// - L4: Full attributes + evidence
        entity_enhance: u8,
        /// Container enhance level (CBU stays visible)
        container_enhance: u8,
    },

    /// Product/Service within CBU focus
    CbuProductService {
        cbu: CbuRef,
        target: ProductServiceRef,
        /// Target enhance level 0-3
        target_enhance: u8,
        /// Container enhance level
        container_enhance: u8,
    },

    /// Instrument Matrix focus - first level into nested taxonomy
    InstrumentMatrix {
        cbu: CbuRef,
        matrix: InstrumentMatrixRef,
        /// Matrix enhance level 0-2:
        /// - L0: Collapsed badge
        /// - L1: Type node grid
        /// - L2: Type counts + status
        matrix_enhance: u8,
        /// Container enhance level
        container_enhance: u8,
    },

    /// Instrument Type Node within matrix
    InstrumentType {
        cbu: CbuRef,
        matrix: InstrumentMatrixRef,
        instrument_type: InstrumentType,
        /// Type enhance level 0-3:
        /// - L0: Type badge
        /// - L1: MIC/BIC/Pricing panels collapsed
        /// - L2: Panels expanded
        /// - L3: Full config details
        type_enhance: u8,
        /// Matrix enhance level
        matrix_enhance: u8,
        /// Container enhance level
        container_enhance: u8,
    },

    /// Deep config node focus (MIC, BIC, Pricing)
    ConfigNode {
        cbu: CbuRef,
        matrix: InstrumentMatrixRef,
        instrument_type: InstrumentType,
        config_node: ConfigNodeRef,
        /// Node enhance level 0-2:
        /// - L0: Summary line
        /// - L1: Full detail
        /// - L2: Full detail + evidence
        node_enhance: u8,
        /// Type enhance level
        type_enhance: u8,
        /// Matrix enhance level
        matrix_enhance: u8,
        /// Container enhance level
        container_enhance: u8,
    },
}

impl ViewportFocusState {
    /// Get the CBU reference if focused on anything within a CBU
    pub fn cbu(&self) -> Option<&CbuRef> {
        match self {
            Self::None => None,
            Self::CbuContainer { cbu, .. } => Some(cbu),
            Self::CbuEntity { cbu, .. } => Some(cbu),
            Self::CbuProductService { cbu, .. } => Some(cbu),
            Self::InstrumentMatrix { cbu, .. } => Some(cbu),
            Self::InstrumentType { cbu, .. } => Some(cbu),
            Self::ConfigNode { cbu, .. } => Some(cbu),
        }
    }

    /// Get the current enhance level for the primary focus target
    pub fn primary_enhance_level(&self) -> u8 {
        match self {
            Self::None => 0,
            Self::CbuContainer { enhance_level, .. } => *enhance_level,
            Self::CbuEntity { entity_enhance, .. } => *entity_enhance,
            Self::CbuProductService { target_enhance, .. } => *target_enhance,
            Self::InstrumentMatrix { matrix_enhance, .. } => *matrix_enhance,
            Self::InstrumentType { type_enhance, .. } => *type_enhance,
            Self::ConfigNode { node_enhance, .. } => *node_enhance,
        }
    }

    /// Get the maximum enhance level for the current focus type
    pub fn max_enhance_level(&self) -> u8 {
        match self {
            Self::None => 0,
            Self::CbuContainer { .. } => 2,
            Self::CbuEntity { .. } => 4,
            Self::CbuProductService { .. } => 3,
            Self::InstrumentMatrix { .. } => 2,
            Self::InstrumentType { .. } => 3,
            Self::ConfigNode { .. } => 2,
        }
    }

    /// Check if we can enhance further
    pub fn can_enhance(&self) -> bool {
        self.primary_enhance_level() < self.max_enhance_level()
    }

    /// Check if we can reduce enhance level
    pub fn can_reduce(&self) -> bool {
        self.primary_enhance_level() > 0
    }

    /// Get hierarchy depth (0 = None, 1 = CbuContainer, etc.)
    pub fn depth(&self) -> u8 {
        match self {
            Self::None => 0,
            Self::CbuContainer { .. } => 1,
            Self::CbuEntity { .. } | Self::CbuProductService { .. } => 2,
            Self::InstrumentMatrix { .. } => 2,
            Self::InstrumentType { .. } => 3,
            Self::ConfigNode { .. } => 4,
        }
    }
}

// ============================================================================
// ENHANCE OPERATIONS
// ============================================================================

/// Operations that can be performed when enhancing focus
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum EnhanceOp {
    /// Show specified attributes
    ShowAttributes { keys: Vec<String> },
    /// Expand relationship edges
    ExpandRelationships {
        depth: u8,
        #[serde(default)]
        rel_types: Option<Vec<String>>,
    },
    /// Show confidence scores on edges/nodes
    ShowConfidenceScores,
    /// Show temporal history panel
    ShowTemporalHistory,
    /// Show evidence/document panel
    ShowEvidencePanel,
    /// Expand a collapsed cluster
    ExpandCluster,
    /// Increase label density (semantic zoom)
    SemanticZoom { label_density: f32 },
    /// Geometric zoom factor
    GeometricZoom { factor: f32 },
    /// Show MIC preferences panel
    ShowMicPreferences,
    /// Show BIC routing panel
    ShowBicRouting,
    /// Show pricing configuration
    ShowPricingConfig,
    /// Show restrictions panel
    ShowRestrictions,
}

/// Enhance argument for DSL verbs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnhanceArg {
    /// Increase by 1
    Increment,
    /// Decrease by 1
    Decrement,
    /// Set to specific level
    Level(u8),
    /// Set to maximum
    Max,
    /// Reset to 0
    Reset,
}

impl EnhanceArg {
    /// Compute new enhance level given current and max
    pub fn apply(&self, current: u8, max: u8) -> u8 {
        match self {
            Self::Increment => (current + 1).min(max),
            Self::Decrement => current.saturating_sub(1),
            Self::Level(n) => (*n).min(max),
            Self::Max => max,
            Self::Reset => 0,
        }
    }
}

// ============================================================================
// ENHANCEABLE TRAIT
// ============================================================================

/// Trait for entities that support progressive disclosure via enhance levels
///
/// Each entity type has a different max enhance level and different operations
/// available at each level. This trait enables polymorphic enhance behavior.
///
/// ## Enhance Levels by Entity Type
///
/// | Entity Type      | Max | L0         | L1              | L2              | L3          | L4          |
/// |------------------|-----|------------|-----------------|-----------------|-------------|-------------|
/// | CBU Container    | 2   | Badge+flag | Category counts | Entity nodes    | -           | -           |
/// | ConcreteEntity   | 4   | Name+type  | Jurisdiction    | 1-hop rels      | Key attrs   | Full+evidence|
/// | ProductService   | 3   | Icon       | Name+status     | Config preview  | Full config | -           |
/// | InstrumentMatrix | 2   | Badge      | Type node grid  | Type counts     | -           | -           |
/// | InstrumentType   | 3   | Type badge | Panels collapsed| Panels expanded | Full config | -           |
/// | ConfigNode       | 2   | Summary    | Full detail     | Detail+evidence | -           | -           |
pub trait Enhanceable {
    /// Get the current enhance level
    fn enhance_level(&self) -> u8;

    /// Get the maximum enhance level for this entity type
    fn max_enhance_level(&self) -> u8;

    /// Check if we can enhance further
    fn can_enhance(&self) -> bool {
        self.enhance_level() < self.max_enhance_level()
    }

    /// Check if we can reduce enhance level
    fn can_reduce(&self) -> bool {
        self.enhance_level() > 0
    }

    /// Get operations available at current enhance level
    fn available_ops(&self) -> Vec<EnhanceOp>;

    /// Get operations that would be added at next enhance level
    fn next_level_ops(&self) -> Vec<EnhanceOp>;

    /// Get a human-readable description of the current level
    fn level_description(&self) -> &'static str;
}

/// Enhance level info for a given entity type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnhanceLevelInfo {
    /// Current level
    pub level: u8,
    /// Maximum level for this type
    pub max_level: u8,
    /// Description of current level
    pub description: String,
    /// Operations available at this level
    pub available_ops: Vec<EnhanceOp>,
    /// Whether we can enhance further
    pub can_enhance: bool,
    /// Whether we can reduce
    pub can_reduce: bool,
}

impl EnhanceLevelInfo {
    /// Create info from any Enhanceable
    pub fn from_enhanceable(e: &dyn Enhanceable) -> Self {
        Self {
            level: e.enhance_level(),
            max_level: e.max_enhance_level(),
            description: e.level_description().to_string(),
            available_ops: e.available_ops(),
            can_enhance: e.can_enhance(),
            can_reduce: e.can_reduce(),
        }
    }
}

// ============================================================================
// VIEW MEMORY
// ============================================================================

/// Camera state for view persistence
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameraState {
    /// Camera position (center point)
    pub x: f32,
    pub y: f32,
    /// Zoom level (1.0 = 100%)
    pub zoom: f32,
}

impl Default for CameraState {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        }
    }
}

/// View type for CBU visualization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CbuViewType {
    /// Ownership and control structure
    #[default]
    Structure,
    /// Beneficial ownership chains
    Ownership,
    /// Account and service relationships
    Accounts,
    /// Compliance status and KYC
    Compliance,
    /// Geographic distribution
    Geographic,
    /// Temporal/historical view
    Temporal,
    /// Instrument matrix view
    Instruments,
}

/// Per-CBU view memory for persistence across navigation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CbuViewMemory {
    /// Last active view type
    pub last_view: CbuViewType,
    /// Last enhance level at container level
    #[serde(default)]
    pub last_enhance: u8,
    /// Focus path for restore
    #[serde(default)]
    pub last_focus_path: Vec<ViewportFocusState>,
    /// Camera state
    pub camera: CameraState,
}

// ============================================================================
// FOCUS MANAGER
// ============================================================================

/// Manages focus state with stack for ascend/descend navigation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FocusManager {
    /// Current focus state
    pub state: ViewportFocusState,
    /// Stack for ascend() navigation
    pub focus_stack: Vec<ViewportFocusState>,
    /// Focus behavior mode
    pub focus_mode: FocusMode,
    /// Per-CBU view memory
    pub view_memory: HashMap<Uuid, CbuViewMemory>,
}

impl Default for FocusManager {
    fn default() -> Self {
        Self {
            state: ViewportFocusState::None,
            focus_stack: Vec::new(),
            focus_mode: FocusMode::default(),
            view_memory: HashMap::new(),
        }
    }
}

impl FocusManager {
    /// Create a new focus manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Get current focus state
    pub fn current(&self) -> &ViewportFocusState {
        &self.state
    }

    /// Get mutable reference to current focus state
    pub fn current_mut(&mut self) -> &mut ViewportFocusState {
        &mut self.state
    }

    /// Set focus to a new state, pushing current to stack
    pub fn set_focus(&mut self, new_state: ViewportFocusState) {
        if self.state != ViewportFocusState::None {
            self.focus_stack.push(self.state.clone());
        }
        self.state = new_state;
    }

    /// Descend into a new focus level
    pub fn descend(&mut self, new_state: ViewportFocusState) {
        self.focus_stack.push(self.state.clone());
        self.state = new_state;
    }

    /// Ascend to previous focus level
    pub fn ascend(&mut self) -> Option<ViewportFocusState> {
        self.focus_stack
            .pop()
            .map(|prev| std::mem::replace(&mut self.state, prev))
    }

    /// Ascend all the way to root (None)
    pub fn ascend_to_root(&mut self) {
        self.focus_stack.clear();
        self.state = ViewportFocusState::None;
    }

    /// Check if we can ascend
    pub fn can_ascend(&self) -> bool {
        !self.focus_stack.is_empty() || self.state != ViewportFocusState::None
    }

    /// Get the focus stack depth
    pub fn stack_depth(&self) -> usize {
        self.focus_stack.len()
    }

    /// Get or create view memory for a CBU
    pub fn get_or_create_memory(&mut self, cbu_id: Uuid) -> &mut CbuViewMemory {
        self.view_memory.entry(cbu_id).or_default()
    }

    /// Save current state to CBU memory
    pub fn save_to_memory(&mut self) {
        if let Some(cbu) = self.state.cbu() {
            let cbu_id = cbu.0;
            let memory = self.view_memory.entry(cbu_id).or_default();
            memory.last_enhance = self.state.primary_enhance_level();
            // Clone the focus stack plus current state as the path
            memory.last_focus_path = self.focus_stack.clone();
            memory.last_focus_path.push(self.state.clone());
        }
    }

    /// Restore from CBU memory
    pub fn restore_from_memory(&mut self, cbu_id: Uuid) -> bool {
        if let Some(memory) = self.view_memory.get(&cbu_id).cloned() {
            if let Some(last_state) = memory.last_focus_path.last().cloned() {
                // Restore the focus path
                self.focus_stack = memory
                    .last_focus_path
                    .iter()
                    .take(memory.last_focus_path.len().saturating_sub(1))
                    .cloned()
                    .collect();
                self.state = last_state;
                return true;
            }
        }
        false
    }
}

// ============================================================================
// VIEWPORT STATE (Complete State Container)
// ============================================================================

/// Complete viewport state - source of truth for rendering
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewportState {
    /// Focus manager with current state and stack
    pub focus: FocusManager,
    /// Current view type
    pub view_type: CbuViewType,
    /// Camera state
    pub camera: CameraState,
    /// Confidence threshold for entity visibility
    pub confidence_threshold: f32,
    /// Active filters
    pub filters: ViewportFilters,
}

impl Default for ViewportState {
    fn default() -> Self {
        Self {
            focus: FocusManager::default(),
            view_type: CbuViewType::default(),
            camera: CameraState::default(),
            confidence_threshold: 0.0, // Show all by default
            filters: ViewportFilters::default(),
        }
    }
}

/// Active viewport filters
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ViewportFilters {
    /// Filter by entity types
    #[serde(default)]
    pub entity_types: Option<Vec<ConcreteEntityType>>,
    /// Filter by confidence zone
    #[serde(default)]
    pub confidence_zone: Option<ConfidenceZone>,
    /// Filter by instrument types
    #[serde(default)]
    pub instrument_types: Option<Vec<InstrumentType>>,
    /// Text search filter
    #[serde(default)]
    pub search_text: Option<String>,
}

// ============================================================================
// RESOLVED TYPES (For ViewportResolutionService)
// ============================================================================

/// Resolved CBU data for viewport rendering
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedCbu {
    /// CBU identifier
    pub id: Uuid,
    /// CBU name
    pub name: String,
    /// Jurisdiction code (e.g., "LU", "US")
    pub jurisdiction: Option<String>,
    /// Client type (e.g., "FUND", "CORPORATE")
    pub client_type: Option<String>,
}

/// Entity member within a CBU with role information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CbuEntityMember {
    /// Entity identifier
    pub entity_id: Uuid,
    /// Entity name
    pub name: String,
    /// Entity type (e.g., "LIMITED_COMPANY", "PROPER_PERSON")
    pub entity_type: String,
    /// Entity category (e.g., "SHELL", "PERSON")
    pub entity_category: Option<String>,
    /// Jurisdiction code
    pub jurisdiction: Option<String>,
    /// Roles assigned within this CBU
    pub roles: Vec<String>,
    /// Primary role (highest priority)
    pub primary_role: Option<String>,
    /// Role category for layout
    pub role_category: Option<String>,
    /// Confidence score (0.0 - 1.0)
    pub confidence_score: f32,
    /// Computed confidence zone
    pub confidence_zone: ConfidenceZone,
}

/// Resolved Instrument Matrix (trading profile)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedInstrumentMatrix {
    /// Profile identifier
    pub profile_id: Uuid,
    /// Version number
    pub version: i32,
    /// Profile status
    pub status: String,
    /// Instrument types available
    pub instrument_types: Vec<ResolvedInstrumentType>,
}

/// Resolved instrument type configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedInstrumentType {
    /// Instrument type
    pub instrument_type: InstrumentType,
    /// Instrument class code (e.g., "EQUITY", "GOVT_BOND")
    pub class_code: String,
    /// Class name
    pub class_name: String,
    /// Markets where this instrument type is traded
    pub markets: Vec<ResolvedMarket>,
    /// Whether OTC (requires ISDA)
    pub is_otc: bool,
    /// Allowed currencies
    pub currencies: Vec<String>,
}

/// Resolved market (MIC) configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedMarket {
    /// MIC code
    pub mic: String,
    /// Market name
    pub market_name: Option<String>,
    /// Currencies for this market
    pub currencies: Vec<String>,
    /// Settlement types
    pub settlement_types: Vec<String>,
}

/// Resolved SSI configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedSsi {
    /// SSI identifier
    pub ssi_id: Uuid,
    /// SSI name
    pub name: String,
    /// SSI type (SECURITIES, CASH, COLLATERAL)
    pub ssi_type: String,
    /// Status
    pub status: Option<String>,
    /// Associated MIC (if market-specific)
    pub mic: Option<String>,
    /// Cash currency
    pub currency: Option<String>,
    /// Safekeeping account
    pub safekeeping_account: Option<String>,
    /// Safekeeping BIC
    pub safekeeping_bic: Option<String>,
}

/// Resolved ISDA agreement
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedIsda {
    /// ISDA identifier
    pub isda_id: Uuid,
    /// Counterparty entity ID
    pub counterparty_id: Uuid,
    /// Counterparty name
    pub counterparty_name: Option<String>,
    /// Governing law
    pub governing_law: Option<String>,
    /// Has CSA
    pub has_csa: bool,
    /// CSA type if present
    pub csa_type: Option<String>,
}

/// Resolution error types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "error_type", rename_all = "snake_case")]
pub enum ResolutionError {
    /// CBU not found
    CbuNotFound { cbu_id: Uuid },
    /// Entity not found
    EntityNotFound { entity_id: Uuid },
    /// No trading profile for CBU
    NoTradingProfile { cbu_id: Uuid },
    /// Database error
    DatabaseError { message: String },
}

impl std::fmt::Display for ResolutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CbuNotFound { cbu_id } => write!(f, "CBU not found: {}", cbu_id),
            Self::EntityNotFound { entity_id } => write!(f, "Entity not found: {}", entity_id),
            Self::NoTradingProfile { cbu_id } => {
                write!(f, "No trading profile for CBU: {}", cbu_id)
            }
            Self::DatabaseError { message } => write!(f, "Database error: {}", message),
        }
    }
}

impl std::error::Error for ResolutionError {}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confidence_zone_from_score() {
        assert_eq!(ConfidenceZone::from_score(0.99), ConfidenceZone::Core);
        assert_eq!(ConfidenceZone::from_score(0.95), ConfidenceZone::Core);
        assert_eq!(ConfidenceZone::from_score(0.80), ConfidenceZone::Shell);
        assert_eq!(ConfidenceZone::from_score(0.70), ConfidenceZone::Shell);
        assert_eq!(ConfidenceZone::from_score(0.50), ConfidenceZone::Penumbra);
        assert_eq!(ConfidenceZone::from_score(0.40), ConfidenceZone::Penumbra);
        assert_eq!(
            ConfidenceZone::from_score(0.30),
            ConfidenceZone::Speculative
        );
        assert_eq!(ConfidenceZone::from_score(0.0), ConfidenceZone::Speculative);
    }

    #[test]
    fn enhance_arg_apply() {
        assert_eq!(EnhanceArg::Increment.apply(0, 4), 1);
        assert_eq!(EnhanceArg::Increment.apply(4, 4), 4); // Capped at max
        assert_eq!(EnhanceArg::Decrement.apply(2, 4), 1);
        assert_eq!(EnhanceArg::Decrement.apply(0, 4), 0); // No negative
        assert_eq!(EnhanceArg::Level(3).apply(0, 4), 3);
        assert_eq!(EnhanceArg::Level(10).apply(0, 4), 4); // Capped at max
        assert_eq!(EnhanceArg::Max.apply(0, 4), 4);
        assert_eq!(EnhanceArg::Reset.apply(3, 4), 0);
    }

    #[test]
    fn focus_manager_descend_ascend() {
        let mut fm = FocusManager::new();
        let cbu = CbuRef::new(Uuid::new_v4());

        // Start at None
        assert_eq!(fm.stack_depth(), 0);
        assert!(!fm.can_ascend());

        // Focus on CBU
        fm.set_focus(ViewportFocusState::CbuContainer {
            cbu: cbu.clone(),
            enhance_level: 0,
        });
        assert!(fm.can_ascend());
        assert_eq!(fm.stack_depth(), 0); // set_focus from None doesn't push

        // Descend to entity
        let entity = ConcreteEntityRef {
            id: Uuid::new_v4(),
            entity_type: ConcreteEntityType::Company,
        };
        fm.descend(ViewportFocusState::CbuEntity {
            cbu: cbu.clone(),
            entity: entity.clone(),
            entity_enhance: 0,
            container_enhance: 1,
        });
        assert_eq!(fm.stack_depth(), 1);

        // Ascend back to CBU
        let prev = fm.ascend();
        assert!(prev.is_some());
        assert!(matches!(
            fm.current(),
            ViewportFocusState::CbuContainer { .. }
        ));
        assert_eq!(fm.stack_depth(), 0);
    }

    #[test]
    fn viewport_focus_state_max_enhance() {
        let cbu = CbuRef::new(Uuid::new_v4());

        let container = ViewportFocusState::CbuContainer {
            cbu: cbu.clone(),
            enhance_level: 0,
        };
        assert_eq!(container.max_enhance_level(), 2);

        let entity = ViewportFocusState::CbuEntity {
            cbu: cbu.clone(),
            entity: ConcreteEntityRef {
                id: Uuid::new_v4(),
                entity_type: ConcreteEntityType::Person,
            },
            entity_enhance: 0,
            container_enhance: 1,
        };
        assert_eq!(entity.max_enhance_level(), 4);

        let matrix = ViewportFocusState::InstrumentMatrix {
            cbu: cbu.clone(),
            matrix: InstrumentMatrixRef(Uuid::new_v4()),
            matrix_enhance: 0,
            container_enhance: 1,
        };
        assert_eq!(matrix.max_enhance_level(), 2);
    }

    #[test]
    fn serialization_roundtrip() {
        let state = ViewportFocusState::CbuContainer {
            cbu: CbuRef::new(Uuid::new_v4()),
            enhance_level: 1,
        };

        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains(r#""focus_type":"cbu_container""#));

        let parsed: ViewportFocusState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, parsed);
    }
}
