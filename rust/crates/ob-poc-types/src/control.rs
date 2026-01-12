//! Control/Ownership Types
//!
//! Types for ownership, voting rights, and board control relationships.
//! Aligned to BODS, GLEIF RR, and UK PSC standards.
//!
//! Key concepts:
//! - ControlEdge: Ownership/voting/board control edge with standard xrefs
//! - BoardControllerEdge: Derived edge (computed by rules engine, not hand-authored)
//! - ControlAnchor: Portal entity linking CBU to ownership/control graph

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// CONTROL EDGE TYPES (aligned to BODS/GLEIF/PSC)
// ============================================================================

/// Interest types aligned to BODS standard
/// https://standard.openownership.org/en/0.3.0/schema/reference.html#interest
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ControlEdgeType {
    // Ownership/Voting (BODS-aligned)
    HoldsShares,
    HoldsVotingRights,

    // Board control (BODS-aligned)
    AppointsBoard,
    ExercisesInfluence,
    IsSeniorManager,

    // Trust arrangements (BODS-aligned)
    IsSettlor,
    IsTrustee,
    IsProtector,
    IsBeneficiary,

    // Economic rights (BODS-aligned)
    HasDissolutionRights,
    HasProfitRights,

    // GLEIF hierarchy
    ConsolidatedBy,
    UltimatelyConsolidatedBy,
    ManagedBy,
    SubfundOf,
    FeedsInto,
}

impl ControlEdgeType {
    /// Map to BODS interest type string
    pub fn to_bods_interest(&self) -> Option<&'static str> {
        match self {
            Self::HoldsShares => Some("shareholding"),
            Self::HoldsVotingRights => Some("voting-rights"),
            Self::AppointsBoard => Some("appointment-of-board"),
            Self::ExercisesInfluence => Some("other-influence-or-control"),
            Self::IsSeniorManager => Some("senior-managing-official"),
            Self::IsSettlor => Some("settlor-of-trust"),
            Self::IsTrustee => Some("trustee-of-trust"),
            Self::IsProtector => Some("protector-of-trust"),
            Self::IsBeneficiary => Some("beneficiary-of-trust"),
            Self::HasDissolutionRights => Some("rights-to-surplus-assets-on-dissolution"),
            Self::HasProfitRights => Some("rights-to-profit-or-income"),
            // GLEIF types don't map to BODS
            _ => None,
        }
    }

    /// Map to GLEIF relationship type
    pub fn to_gleif_relationship(&self) -> Option<&'static str> {
        match self {
            Self::ConsolidatedBy => Some("IS_DIRECTLY_CONSOLIDATED_BY"),
            Self::UltimatelyConsolidatedBy => Some("IS_ULTIMATELY_CONSOLIDATED_BY"),
            Self::ManagedBy => Some("IS_FUND_MANAGED_BY"),
            Self::SubfundOf => Some("IS_SUBFUND_OF"),
            Self::FeedsInto => Some("IS_FEEDER_TO"),
            _ => None,
        }
    }

    /// Parse from database string
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "HOLDS_SHARES" => Some(Self::HoldsShares),
            "HOLDS_VOTING_RIGHTS" => Some(Self::HoldsVotingRights),
            "APPOINTS_BOARD" => Some(Self::AppointsBoard),
            "EXERCISES_INFLUENCE" => Some(Self::ExercisesInfluence),
            "IS_SENIOR_MANAGER" => Some(Self::IsSeniorManager),
            "IS_SETTLOR" => Some(Self::IsSettlor),
            "IS_TRUSTEE" => Some(Self::IsTrustee),
            "IS_PROTECTOR" => Some(Self::IsProtector),
            "IS_BENEFICIARY" => Some(Self::IsBeneficiary),
            "HAS_DISSOLUTION_RIGHTS" => Some(Self::HasDissolutionRights),
            "HAS_PROFIT_RIGHTS" => Some(Self::HasProfitRights),
            "CONSOLIDATED_BY" => Some(Self::ConsolidatedBy),
            "ULTIMATELY_CONSOLIDATED_BY" => Some(Self::UltimatelyConsolidatedBy),
            "MANAGED_BY" => Some(Self::ManagedBy),
            "SUBFUND_OF" => Some(Self::SubfundOf),
            "FEEDS_INTO" => Some(Self::FeedsInto),
            _ => None,
        }
    }

    /// Convert to database string
    pub fn to_db_str(&self) -> &'static str {
        match self {
            Self::HoldsShares => "HOLDS_SHARES",
            Self::HoldsVotingRights => "HOLDS_VOTING_RIGHTS",
            Self::AppointsBoard => "APPOINTS_BOARD",
            Self::ExercisesInfluence => "EXERCISES_INFLUENCE",
            Self::IsSeniorManager => "IS_SENIOR_MANAGER",
            Self::IsSettlor => "IS_SETTLOR",
            Self::IsTrustee => "IS_TRUSTEE",
            Self::IsProtector => "IS_PROTECTOR",
            Self::IsBeneficiary => "IS_BENEFICIARY",
            Self::HasDissolutionRights => "HAS_DISSOLUTION_RIGHTS",
            Self::HasProfitRights => "HAS_PROFIT_RIGHTS",
            Self::ConsolidatedBy => "CONSOLIDATED_BY",
            Self::UltimatelyConsolidatedBy => "ULTIMATELY_CONSOLIDATED_BY",
            Self::ManagedBy => "MANAGED_BY",
            Self::SubfundOf => "SUBFUND_OF",
            Self::FeedsInto => "FEEDS_INTO",
        }
    }
}

/// UK PSC threshold categories
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum PscCategory {
    OwnershipOfShares25To50,
    OwnershipOfShares50To75,
    OwnershipOfShares75To100,
    VotingRights25To50,
    VotingRights50To75,
    VotingRights75To100,
    AppointsMajorityOfBoard,
    SignificantInfluenceOrControl,
}

impl PscCategory {
    /// Derive PSC category from edge type and percentage
    pub fn from_edge(edge_type: ControlEdgeType, pct: Option<f32>) -> Option<Self> {
        let pct = pct?;
        match edge_type {
            ControlEdgeType::HoldsShares => match pct as u8 {
                76..=100 => Some(Self::OwnershipOfShares75To100),
                51..=75 => Some(Self::OwnershipOfShares50To75),
                25..=50 => Some(Self::OwnershipOfShares25To50),
                _ => None,
            },
            ControlEdgeType::HoldsVotingRights => match pct as u8 {
                76..=100 => Some(Self::VotingRights75To100),
                51..=75 => Some(Self::VotingRights50To75),
                25..=50 => Some(Self::VotingRights25To50),
                _ => None,
            },
            ControlEdgeType::AppointsBoard if pct > 50.0 => Some(Self::AppointsMajorityOfBoard),
            ControlEdgeType::ExercisesInfluence => Some(Self::SignificantInfluenceOrControl),
            _ => None,
        }
    }

    /// Convert to database string
    pub fn to_db_str(&self) -> &'static str {
        match self {
            Self::OwnershipOfShares25To50 => "ownership-of-shares-25-to-50",
            Self::OwnershipOfShares50To75 => "ownership-of-shares-50-to-75",
            Self::OwnershipOfShares75To100 => "ownership-of-shares-75-to-100",
            Self::VotingRights25To50 => "voting-rights-25-to-50",
            Self::VotingRights50To75 => "voting-rights-50-to-75",
            Self::VotingRights75To100 => "voting-rights-75-to-100",
            Self::AppointsMajorityOfBoard => "appoints-majority-of-board",
            Self::SignificantInfluenceOrControl => "significant-influence-or-control",
        }
    }
}

/// A control/ownership edge with standard xrefs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlEdge {
    pub id: Uuid,
    pub from_entity_id: Uuid,
    pub to_entity_id: Uuid,
    pub edge_type: ControlEdgeType,

    // Quantitative
    pub percentage: Option<f32>,
    pub is_direct: bool,

    // Standard xrefs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bods_interest_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gleif_relationship_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub psc_category: Option<String>,

    // Provenance
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_register: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_date: Option<NaiveDate>,
}

// ============================================================================
// BOARD CONTROLLER DERIVATION (Rules Engine Output)
// ============================================================================

/// Derivation method: which rule fired to identify the board controller
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BoardControlMethod {
    /// Rule A: Can appoint/remove majority of directors
    BoardAppointmentRights,
    /// Rule B: >50% voting power → controls board
    VotingRightsMajority,
    /// Rule C: Golden share, GP authority, trustee powers
    SpecialInstrument,
    /// Multiple rules contributed
    Mixed,
    /// Rule D: No entity meets threshold
    NoSingleController,
}

impl BoardControlMethod {
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "board_appointment_rights" => Some(Self::BoardAppointmentRights),
            "voting_rights_majority" => Some(Self::VotingRightsMajority),
            "special_instrument" => Some(Self::SpecialInstrument),
            "mixed" => Some(Self::Mixed),
            "no_single_controller" => Some(Self::NoSingleController),
            _ => None,
        }
    }

    pub fn to_db_str(&self) -> &'static str {
        match self {
            Self::BoardAppointmentRights => "board_appointment_rights",
            Self::VotingRightsMajority => "voting_rights_majority",
            Self::SpecialInstrument => "special_instrument",
            Self::Mixed => "mixed",
            Self::NoSingleController => "no_single_controller",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::BoardAppointmentRights => "Board Appointment Rights (Rule A)",
            Self::VotingRightsMajority => "Voting Rights Majority (Rule B)",
            Self::SpecialInstrument => "Special Instrument (Rule C)",
            Self::Mixed => "Mixed (Multiple Rules)",
            Self::NoSingleController => "No Single Controller (Rule D)",
        }
    }
}

/// Confidence level in the board control derivation
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ControlConfidence {
    Low,
    Medium,
    High,
}

impl ControlConfidence {
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            _ => None,
        }
    }

    pub fn to_db_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

/// Scoring breakdown for board control computation
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ControlScore {
    /// From appointment rights coverage (Rule A)
    pub s_appoint: f32,
    /// From voting power (Rule B)
    pub s_vote: f32,
    /// From board member affiliations (weak signal)
    pub s_affiliation: f32,
    /// From special instruments (Rule C)
    pub s_override: f32,
    /// Data completeness (0.0-1.0)
    pub data_coverage: f32,
}

impl ControlScore {
    /// Compute weighted total score
    pub fn total(&self) -> f32 {
        // Override trumps everything
        if self.s_override > 0.0 {
            return self.s_override;
        }
        // Weighted combination, penalized by data coverage
        let raw = 0.70 * self.s_appoint + 0.25 * self.s_vote + 0.05 * self.s_affiliation;
        raw * self.data_coverage
    }

    /// Derive confidence from score and data coverage
    pub fn confidence(&self) -> ControlConfidence {
        match (self.data_coverage, self.total()) {
            (c, s) if c > 0.8 && s > 0.7 => ControlConfidence::High,
            (c, s) if c > 0.5 && s > 0.5 => ControlConfidence::Medium,
            _ => ControlConfidence::Low,
        }
    }
}

/// Evidence source type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSource {
    GleifRr,
    BodsStatement,
    InvestorRegister,
    GovernanceDoc,
    SpecialInstrument,
    ManualEntry,
}

impl EvidenceSource {
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "gleif_rr" => Some(Self::GleifRr),
            "bods_statement" => Some(Self::BodsStatement),
            "investor_register" => Some(Self::InvestorRegister),
            "governance_doc" => Some(Self::GovernanceDoc),
            "special_instrument" => Some(Self::SpecialInstrument),
            "manual_entry" => Some(Self::ManualEntry),
            _ => None,
        }
    }

    pub fn to_db_str(&self) -> &'static str {
        match self {
            Self::GleifRr => "gleif_rr",
            Self::BodsStatement => "bods_statement",
            Self::InvestorRegister => "investor_register",
            Self::GovernanceDoc => "governance_doc",
            Self::SpecialInstrument => "special_instrument",
            Self::ManualEntry => "manual_entry",
        }
    }
}

/// Reference to evidence used in derivation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceRef {
    pub source_type: EvidenceSource,
    pub source_id: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub as_of: Option<NaiveDate>,
}

/// A candidate for board control with scoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlCandidate {
    pub entity_id: Uuid,
    pub entity_name: String,
    pub score: ControlScore,
    pub total_score: f32,
    /// Human-readable reasons
    pub why: Vec<String>,
}

/// Full explanation of board control derivation (stored as JSON)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BoardControlExplanation {
    pub as_of: Option<NaiveDate>,
    pub rule_fired: Option<BoardControlMethod>,
    pub candidates: Vec<ControlCandidate>,
    pub evidence_refs: Vec<EvidenceRef>,
    /// What data is missing
    pub data_gaps: Vec<String>,
}

/// Derived board controller edge (materialized, computed by rules engine)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardControllerEdge {
    pub id: Uuid,
    pub cbu_id: Uuid,
    pub controller_entity_id: Option<Uuid>,
    pub controller_name: Option<String>,
    pub method: BoardControlMethod,
    pub confidence: ControlConfidence,
    pub score: f32,
    pub as_of: NaiveDate,
    pub explanation: BoardControlExplanation,
}

/// Summary for UI display (lighter than full BoardControllerEdge)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardControllerSummary {
    pub controller_entity_id: Option<Uuid>,
    pub controller_name: Option<String>,
    pub method: BoardControlMethod,
    pub confidence: ControlConfidence,
    pub score: f32,
    pub as_of: NaiveDate,
    /// Key evidence points for tooltip
    pub evidence_summary: Vec<String>,
    /// What's missing
    pub data_gaps: Vec<String>,
}

// ============================================================================
// CONTROL ANCHORS (Portal Entities)
// ============================================================================

/// Role of a control anchor in bridging CBU to ownership graph
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnchorRole {
    /// ManCo, board oversight - who controls the board
    Governance,
    /// Parent group, ultimate controller
    Sponsor,
    /// Fund legal entity itself
    Issuer,
}

impl AnchorRole {
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "governance" => Some(Self::Governance),
            "sponsor" => Some(Self::Sponsor),
            "issuer" => Some(Self::Issuer),
            _ => None,
        }
    }

    pub fn to_db_str(&self) -> &'static str {
        match self {
            Self::Governance => "governance",
            Self::Sponsor => "sponsor",
            Self::Issuer => "issuer",
        }
    }
}

/// A control anchor linking CBU to ownership/control graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlAnchor {
    pub id: Uuid,
    pub cbu_id: Uuid,
    pub entity_id: Uuid,
    pub entity_name: Option<String>,
    pub anchor_role: AnchorRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jurisdiction: Option<String>,
}

// ============================================================================
// CONTROL SPHERE (Query Result)
// ============================================================================

/// Entity reference for control graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlEntityRef {
    pub id: Uuid,
    pub name: String,
    pub entity_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jurisdiction: Option<String>,
    #[serde(default)]
    pub is_ubo: bool,
}

/// Control sphere query result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlSphere {
    pub anchor_entity: ControlEntityRef,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ultimate_controller: Option<ControlEntityRef>,
    pub nodes: Vec<ControlEntityRef>,
    pub edges: Vec<ControlEdge>,
    pub board_control_summary: Vec<BoardController>,
}

/// Board controller with path from control sphere query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardController {
    pub entity_id: Uuid,
    pub entity_name: String,
    pub entity_type: String,
    pub total_control_pct: f32,
    pub has_board_majority: bool,
    pub psc_categories: Vec<PscCategory>,
    pub control_path: Vec<ControlPathStep>,
}

/// Step in a control path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPathStep {
    pub entity_id: Uuid,
    pub entity_name: String,
    pub edge_type: ControlEdgeType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percentage: Option<f32>,
}

// ============================================================================
// API REQUEST/RESPONSE TYPES
// ============================================================================

/// Response from GET /api/cbu/{id}/board-controller
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetBoardControllerResponse {
    pub cbu_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub board_controller: Option<BoardControllerEdge>,
}

/// Response from GET /api/cbu/{id}/control-anchors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetControlAnchorsResponse {
    pub cbu_id: Uuid,
    pub anchors: Vec<ControlAnchor>,
}

/// Request for POST /api/cbu/{id}/control-anchors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetControlAnchorsRequest {
    pub anchors: Vec<SetControlAnchorItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetControlAnchorItem {
    pub entity_id: Uuid,
    pub anchor_role: AnchorRole,
}

/// Response from GET /api/control-sphere/{entity_id}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetControlSphereResponse {
    pub sphere: ControlSphere,
    pub depth: u8,
}
