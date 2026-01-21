//! Primary Governance Controller Model
//!
//! Types for managing CBU groups anchored to governance controllers.
//!
//! ## Key Concepts
//!
//! - **Primary Governance Controller**: Entity that controls a CBU via board appointment rights
//! - **CBU Group**: Collection of CBUs under the same governance controller (e.g., "Allianz Lux Book")
//! - **Holding Control Link**: A shareholding that confers control (≥ threshold)
//!
//! ## Signal Priority (Deterministic)
//!
//! 1. Board appointment rights via control share class (primary)
//! 2. MANAGEMENT_COMPANY role assignment (fallback)
//! 3. GLEIF IS_FUND_MANAGED_BY (fallback)
//!
//! ## Usage
//!
//! ```rust,ignore
//! use ob_poc_types::manco_group::{CbuGroup, GroupMembership, HoldingControlLink, PrimaryGovernanceController};
//!
//! // Find all CBUs under a governance controller
//! let group = CbuGroup::find_by_controller(&pool, controller_entity_id).await?;
//! let cbus = group.members(&pool).await?;
//!
//! // Get primary governance controller for an issuer
//! let controller = PrimaryGovernanceController::for_issuer(&pool, issuer_entity_id).await?;
//!
//! // Trace control chain to ultimate parent
//! let chain = HoldingControlLink::control_chain(&pool, controller_entity_id, 5).await?;
//! ```

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// CBU Group Types
// ============================================================================

/// Type of CBU group
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GroupType {
    /// Computed from board appointment / control signals (primary)
    #[default]
    GovernanceBook,
    /// Standard ManCo management group (fallback)
    MancoBook,
    /// Corporate entity group (non-fund)
    CorporateGroup,
    /// Grouped by Investment Manager rather than ManCo
    InvestmentManager,
    /// Sub-funds of a SICAV umbrella
    UmbrellaSicav,
    /// Manual grouping
    Custom,
}

impl std::fmt::Display for GroupType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GovernanceBook => write!(f, "GOVERNANCE_BOOK"),
            Self::MancoBook => write!(f, "MANCO_BOOK"),
            Self::CorporateGroup => write!(f, "CORPORATE_GROUP"),
            Self::InvestmentManager => write!(f, "INVESTMENT_MANAGER"),
            Self::UmbrellaSicav => write!(f, "UMBRELLA_SICAV"),
            Self::Custom => write!(f, "CUSTOM"),
        }
    }
}

/// A CBU Group - collection of CBUs under the same governance controller
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuGroup {
    pub group_id: Uuid,

    /// The governance controller entity that anchors this group
    pub manco_entity_id: Uuid,

    /// Human-readable group name (e.g., "AllianzGI GmbH Book")
    pub group_name: String,

    /// Short code (e.g., "ALLIANZGI_LUX")
    pub group_code: Option<String>,

    /// Type of group
    pub group_type: GroupType,

    /// Jurisdiction scope (optional - controller might have multiple books)
    pub jurisdiction: Option<String>,

    /// Ultimate parent entity (e.g., Allianz SE)
    pub ultimate_parent_entity_id: Option<Uuid>,

    /// Description
    pub description: Option<String>,

    /// Whether this group was auto-derived from governance controller
    pub is_auto_derived: bool,

    /// When this group became effective
    pub effective_from: NaiveDate,

    /// When this group ceased to be effective (None = current)
    pub effective_to: Option<NaiveDate>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl CbuGroup {
    /// Check if this group is currently active
    pub fn is_active(&self) -> bool {
        self.effective_to.is_none()
    }
}

/// Source of group membership determination
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MembershipSource {
    /// Computed from board appointment / control signals (primary)
    #[default]
    GovernanceController,
    /// From cbu_entity_roles MANAGEMENT_COMPANY (fallback)
    MancoRole,
    /// From gleif_relationships IS_FUND_MANAGED_BY
    GleifManaged,
    /// From controlling shareholding
    Shareholding,
    /// Manually assigned
    Manual,
}

impl std::fmt::Display for MembershipSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GovernanceController => write!(f, "GOVERNANCE_CONTROLLER"),
            Self::MancoRole => write!(f, "MANCO_ROLE"),
            Self::GleifManaged => write!(f, "GLEIF_MANAGED"),
            Self::Shareholding => write!(f, "SHAREHOLDING"),
            Self::Manual => write!(f, "MANUAL"),
        }
    }
}

/// CBU membership in a group
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMembership {
    pub membership_id: Uuid,
    pub group_id: Uuid,
    pub cbu_id: Uuid,

    /// How was this membership determined?
    pub source: MembershipSource,

    /// Display order within group
    pub display_order: i32,

    pub effective_from: NaiveDate,
    pub effective_to: Option<NaiveDate>,

    pub created_at: DateTime<Utc>,
}

impl GroupMembership {
    pub fn is_active(&self) -> bool {
        self.effective_to.is_none()
    }
}

// ============================================================================
// Governance Controller Types
// ============================================================================

/// Basis for governance controller determination
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ControllerBasis {
    /// Control via board appointment rights (control share class)
    BoardAppointment,
    /// Control via voting majority (≥50%)
    VotingControl,
    /// Significant influence (≥25%)
    SignificantInfluence,
    /// No qualifying control signal found
    #[default]
    None,
}

impl std::fmt::Display for ControllerBasis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BoardAppointment => write!(f, "BOARD_APPOINTMENT"),
            Self::VotingControl => write!(f, "VOTING_CONTROL"),
            Self::SignificantInfluence => write!(f, "SIGNIFICANT_INFLUENCE"),
            Self::None => write!(f, "NONE"),
        }
    }
}

/// Primary governance controller for an issuer (single deterministic winner)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrimaryGovernanceController {
    /// The issuer entity being controlled
    pub issuer_entity_id: Uuid,

    /// The winning holder entity (direct controller)
    pub primary_controller_entity_id: Uuid,

    /// The group container entity (if holder has a group container, else same as primary)
    pub governance_controller_entity_id: Uuid,

    /// Basis for control determination
    pub basis: ControllerBasis,

    /// Number of board seats held
    pub board_seats: i32,

    /// Voting percentage
    pub voting_pct: Option<Decimal>,

    /// Economic percentage
    pub economic_pct: Option<Decimal>,

    /// Has voting control (≥50%)
    pub has_control: bool,

    /// Has significant influence (≥25%)
    pub has_significant_influence: bool,
}

impl PrimaryGovernanceController {
    /// Check if this represents a valid controller (not NONE basis)
    pub fn is_valid(&self) -> bool {
        self.basis != ControllerBasis::None
    }

    /// Check if controller has board appointment rights
    pub fn has_board_rights(&self) -> bool {
        self.basis == ControllerBasis::BoardAppointment
    }
}

// ============================================================================
// Holding Control Link Types
// ============================================================================

/// Classification of control based on shareholding percentage
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ControlType {
    /// ≥ 50% (or issuer-specific control threshold)
    Controlling,
    /// ≥ 25% (or issuer-specific significant threshold)
    SignificantInfluence,
    /// ≥ 10% (or issuer-specific material threshold)
    Material,
    /// ≥ 5% (or issuer-specific disclosure threshold)
    Notifiable,
    /// < disclosure threshold but tracked
    Minority,
}

impl ControlType {
    /// Check if this represents a controlling or significant position
    pub fn is_control_position(&self) -> bool {
        matches!(self, Self::Controlling | Self::SignificantInfluence)
    }

    /// Get the minimum threshold percentage for this control type
    pub fn default_threshold(&self) -> Decimal {
        match self {
            Self::Controlling => Decimal::new(5000, 2), // 50.00%
            Self::SignificantInfluence => Decimal::new(2500, 2), // 25.00%
            Self::Material => Decimal::new(1000, 2),    // 10.00%
            Self::Notifiable => Decimal::new(500, 2),   // 5.00%
            Self::Minority => Decimal::ZERO,
        }
    }
}

impl std::fmt::Display for ControlType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Controlling => write!(f, "CONTROLLING"),
            Self::SignificantInfluence => write!(f, "SIGNIFICANT_INFLUENCE"),
            Self::Material => write!(f, "MATERIAL"),
            Self::Notifiable => write!(f, "NOTIFIABLE"),
            Self::Minority => write!(f, "MINORITY"),
        }
    }
}

/// A materialized control relationship derived from shareholdings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoldingControlLink {
    pub link_id: Uuid,

    /// The entity that holds shares (controller)
    pub holder_entity_id: Uuid,

    /// The entity that issued shares (controlled)
    pub issuer_entity_id: Uuid,

    /// Specific share class (None = aggregated across all classes)
    pub share_class_id: Option<Uuid>,

    /// Total units held
    pub total_units: Option<Decimal>,

    /// Voting percentage
    pub voting_pct: Option<Decimal>,

    /// Economic percentage
    pub economic_pct: Option<Decimal>,

    /// Control classification
    pub control_type: ControlType,

    /// Threshold used for classification (for audit)
    pub threshold_pct: Decimal,

    /// Is this a direct holding?
    pub is_direct: bool,

    /// Chain depth (1 = direct, 2+ = indirect)
    pub chain_depth: i32,

    /// Source holding IDs (for traceability)
    pub source_holding_ids: Vec<Uuid>,

    /// As-of date for this computation
    pub as_of_date: NaiveDate,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl HoldingControlLink {
    /// Get the effective control percentage (voting takes precedence)
    pub fn effective_pct(&self) -> Option<Decimal> {
        self.voting_pct.or(self.economic_pct)
    }

    /// Check if this link represents a control position
    pub fn is_control_position(&self) -> bool {
        self.control_type.is_control_position()
    }
}

// ============================================================================
// View/Query Types
// ============================================================================

/// Summary of a governance controller group for display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MancoGroupSummary {
    pub group_id: Uuid,
    pub group_name: String,
    pub group_code: Option<String>,
    pub group_type: GroupType,
    pub manco_entity_id: Uuid,
    pub manco_name: String,
    pub jurisdiction: Option<String>,
    pub ultimate_parent_entity_id: Option<Uuid>,
    pub ultimate_parent_name: Option<String>,
    pub cbu_count: i64,
    pub cbu_names: Vec<String>,
    pub effective_from: NaiveDate,
    pub is_auto_derived: bool,
}

/// CBU with governance controller context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuByManco {
    pub manco_entity_id: Uuid,
    pub manco_name: String,
    pub cbu_id: Uuid,
    pub cbu_name: String,
    pub cbu_category: Option<String>,
    pub jurisdiction: Option<String>,
    pub membership_source: MembershipSource,
    pub controlling_holder_id: Option<Uuid>,
    pub controlling_holder_name: Option<String>,
    pub controlling_voting_pct: Option<Decimal>,
    pub control_type: Option<ControlType>,
}

/// Node in the control chain (upward traversal to ultimate controller)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlChainNode {
    /// Depth in chain (1 = controller itself, 2+ = controllers of controller)
    pub depth: i32,

    pub entity_id: Uuid,
    pub entity_name: String,
    pub entity_type: Option<String>,

    /// Who controls this entity (None for root at depth 1)
    pub controlled_by_entity_id: Option<Uuid>,
    pub controlled_by_name: Option<String>,

    /// How is control established (shareholding classification)
    pub control_type: Option<ControlType>,
    pub voting_pct: Option<Decimal>,

    /// True if no one controls this entity (top of chain)
    pub is_ultimate_controller: bool,
}

impl ControlChainNode {
    /// Check if this is the root controller node
    pub fn is_root(&self) -> bool {
        self.depth == 1
    }
}

/// Result of deriving CBU groups
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeriveGroupsResult {
    pub groups_created: i32,
    pub memberships_created: i32,
}

/// Result of bridging ManCo roles to board rights
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeRolesResult {
    pub rights_created: i32,
    pub rights_updated: i32,
}

/// Result of bridging GLEIF fund managers to board rights
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeGleifResult {
    pub rights_created: i32,
    pub rights_updated: i32,
}

/// Result of bridging BODS ownership to holdings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeBodsResult {
    pub holdings_created: i32,
    pub holdings_updated: i32,
    pub entities_linked: i32,
}

/// Result of computing control links
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeControlLinksResult {
    pub links_created: i32,
}

/// Result of full governance refresh pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceRefreshResult {
    pub manco_bridge: BridgeRolesResult,
    pub gleif_bridge: BridgeGleifResult,
    pub bods_bridge: BridgeBodsResult,
    pub control_links: ComputeControlLinksResult,
    pub groups: DeriveGroupsResult,
}

/// Result of book summary query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookSummaryResult {
    pub group: Option<MancoGroupSummary>,
    pub cbus: Vec<CbuByManco>,
    pub control_chain: Vec<ControlChainNode>,
}

/// CBU in a governance controller group (simplified view)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupCbuEntry {
    pub cbu_id: Uuid,
    pub cbu_name: String,
    pub cbu_category: String,
    pub jurisdiction: Option<String>,
    pub fund_entity_id: Option<Uuid>,
    pub fund_entity_name: Option<String>,
    pub membership_source: String,
}

/// Result of looking up governance controller for a CBU
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuMancoResult {
    pub manco_entity_id: Uuid,
    pub manco_name: String,
    pub manco_lei: Option<String>,
    pub group_id: Uuid,
    pub group_name: String,
    pub group_type: String,
    pub source: String,
}

/// Empty/not found result for CBU manco lookup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuMancoNotFound {
    pub message: String,
}

// ============================================================================
// Builder for queries
// ============================================================================

/// Options for querying governance controller groups
#[derive(Debug, Clone, Default)]
pub struct MancoGroupQuery {
    /// Filter by controller entity ID
    pub manco_entity_id: Option<Uuid>,

    /// Filter by jurisdiction
    pub jurisdiction: Option<String>,

    /// Filter by group type
    pub group_type: Option<GroupType>,

    /// Include only active groups
    pub active_only: bool,

    /// Minimum CBU count
    pub min_cbu_count: Option<i32>,
}

impl MancoGroupQuery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn controller(mut self, entity_id: Uuid) -> Self {
        self.manco_entity_id = Some(entity_id);
        self
    }

    pub fn jurisdiction(mut self, jurisdiction: impl Into<String>) -> Self {
        self.jurisdiction = Some(jurisdiction.into());
        self
    }

    pub fn active_only(mut self) -> Self {
        self.active_only = true;
        self
    }

    pub fn group_type(mut self, group_type: GroupType) -> Self {
        self.group_type = Some(group_type);
        self
    }
}

/// Options for computing control links
#[derive(Debug, Clone)]
pub struct ComputeControlLinksOptions {
    /// Scope to specific issuer (None = all issuers)
    pub issuer_entity_id: Option<Uuid>,

    /// As-of date for computation
    pub as_of_date: NaiveDate,

    /// Include minority positions (< disclosure threshold)
    pub include_minority: bool,
}

impl Default for ComputeControlLinksOptions {
    fn default() -> Self {
        Self {
            issuer_entity_id: None,
            as_of_date: chrono::Utc::now().date_naive(),
            include_minority: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_type_ordering() {
        assert!(ControlType::Controlling > ControlType::SignificantInfluence);
        assert!(ControlType::SignificantInfluence > ControlType::Material);
        assert!(ControlType::Material > ControlType::Notifiable);
        assert!(ControlType::Notifiable > ControlType::Minority);
    }

    #[test]
    fn control_type_thresholds() {
        assert_eq!(
            ControlType::Controlling.default_threshold(),
            Decimal::new(5000, 2)
        );
        assert_eq!(
            ControlType::SignificantInfluence.default_threshold(),
            Decimal::new(2500, 2)
        );
    }

    #[test]
    fn group_type_display() {
        assert_eq!(GroupType::GovernanceBook.to_string(), "GOVERNANCE_BOOK");
        assert_eq!(GroupType::MancoBook.to_string(), "MANCO_BOOK");
        assert_eq!(GroupType::UmbrellaSicav.to_string(), "UMBRELLA_SICAV");
    }

    #[test]
    fn membership_source_display() {
        assert_eq!(
            MembershipSource::GovernanceController.to_string(),
            "GOVERNANCE_CONTROLLER"
        );
        assert_eq!(MembershipSource::MancoRole.to_string(), "MANCO_ROLE");
    }

    #[test]
    fn controller_basis_display() {
        assert_eq!(
            ControllerBasis::BoardAppointment.to_string(),
            "BOARD_APPOINTMENT"
        );
        assert_eq!(ControllerBasis::VotingControl.to_string(), "VOTING_CONTROL");
    }

    #[test]
    fn primary_governance_controller_validity() {
        let controller = PrimaryGovernanceController {
            issuer_entity_id: Uuid::new_v4(),
            primary_controller_entity_id: Uuid::new_v4(),
            governance_controller_entity_id: Uuid::new_v4(),
            basis: ControllerBasis::BoardAppointment,
            board_seats: 2,
            voting_pct: Some(Decimal::new(5500, 2)),
            economic_pct: Some(Decimal::new(5500, 2)),
            has_control: true,
            has_significant_influence: true,
        };
        assert!(controller.is_valid());
        assert!(controller.has_board_rights());

        let no_controller = PrimaryGovernanceController {
            basis: ControllerBasis::None,
            ..controller
        };
        assert!(!no_controller.is_valid());
    }
}
