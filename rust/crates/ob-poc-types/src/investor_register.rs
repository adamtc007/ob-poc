//! Investor Register Visualization API Types
//!
//! Types for investor register visualization crossing the HTTP boundary.
//! Uses `f64` for decimals (JSON-friendly) instead of `rust_decimal::Decimal`.
//!
//! # API Endpoints
//!
//! - `GET /api/capital/:issuer_id/investors` → `InvestorRegisterView`
//! - `GET /api/capital/:issuer_id/investors/list` → `InvestorListResponse`

use serde::{Deserialize, Serialize};

// =============================================================================
// INVESTOR REGISTER VIEW (Main Response)
// =============================================================================

/// Complete response for investor register visualization
///
/// Returns two components:
/// 1. Control holders - individual nodes for holders above threshold
/// 2. Aggregate - collapsed node for remaining investors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvestorRegisterView {
    /// The issuer entity (fund, company)
    pub issuer: IssuerSummary,

    /// Share class being viewed (if specific), or None for all classes
    #[serde(default)]
    pub share_class_filter: Option<String>,

    /// As-of date for snapshot (ISO format YYYY-MM-DD)
    pub as_of_date: String,

    /// Thresholds used for this view
    pub thresholds: ThresholdConfig,

    /// Holders above threshold or with special rights
    /// These become individual nodes in the taxonomy graph
    pub control_holders: Vec<ControlHolderNode>,

    /// Summary of all holders below threshold
    /// Becomes single "N other investors" node in taxonomy
    #[serde(default)]
    pub aggregate: Option<AggregateInvestorsNode>,

    /// Total investor count (for UI display)
    pub total_investor_count: i32,

    /// Total issued units (denominator)
    pub total_issued_units: f64,

    /// Whether dilution data is available
    #[serde(default)]
    pub has_dilution_data: bool,
}

/// Summary of the issuer entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssuerSummary {
    pub entity_id: String,
    pub name: String,
    pub entity_type: String,
    #[serde(default)]
    pub jurisdiction: Option<String>,
    #[serde(default)]
    pub lei: Option<String>,
}

/// Threshold configuration for control determination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdConfig {
    /// Disclosure threshold (default 5%)
    pub disclosure_pct: f64,
    /// Material threshold (default 10%)
    pub material_pct: f64,
    /// Significant influence threshold (default 25%)
    pub significant_pct: f64,
    /// Control threshold (default 50%)
    pub control_pct: f64,
    /// Basis for control computation: VOTES or ECONOMIC
    pub control_basis: String,
}

impl Default for ThresholdConfig {
    fn default() -> Self {
        Self {
            disclosure_pct: 5.0,
            material_pct: 10.0,
            significant_pct: 25.0,
            control_pct: 50.0,
            control_basis: "VOTES".to_string(),
        }
    }
}

// =============================================================================
// CONTROL HOLDER NODE (Individual)
// =============================================================================

/// Individual holder displayed as taxonomy node
///
/// Only for holders above threshold or with special rights.
/// Rendered as individual nodes in the graph visualization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlHolderNode {
    /// Entity ID of the holder
    pub entity_id: String,

    /// Display name
    pub name: String,

    /// Entity type: PROPER_PERSON, LIMITED_COMPANY, etc.
    pub entity_type: String,

    /// Investor classification: INSTITUTIONAL, PROFESSIONAL, RETAIL, etc.
    #[serde(default)]
    pub investor_type: Option<String>,

    /// Total units held
    pub units: f64,

    /// Voting percentage
    pub voting_pct: f64,

    /// Economic percentage
    pub economic_pct: f64,

    /// Has voting control (>control_threshold)
    #[serde(default)]
    pub has_control: bool,

    /// Has significant influence (>significant_threshold)
    #[serde(default)]
    pub has_significant_influence: bool,

    /// Above disclosure threshold
    #[serde(default)]
    pub above_disclosure: bool,

    /// Board appointment rights (number of seats)
    #[serde(default)]
    pub board_seats: i32,

    /// Veto rights held (e.g., ["VETO_MA", "VETO_FUNDRAISE"])
    #[serde(default)]
    pub veto_rights: Vec<String>,

    /// Other special rights
    #[serde(default)]
    pub other_rights: Vec<String>,

    /// Why this holder is shown individually (for tooltip)
    pub inclusion_reason: String,

    /// KYC status for badge: APPROVED, PENDING, REJECTED, EXPIRED
    pub kyc_status: String,

    /// Position in hierarchy (for layout, 0 = direct holder)
    #[serde(default)]
    pub hierarchy_depth: i32,
}

// =============================================================================
// AGGREGATE INVESTORS NODE (Collapsed)
// =============================================================================

/// Collapsed node representing all holders below threshold
///
/// Rendered as a single clickable node that expands to show breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateInvestorsNode {
    /// Number of investors in aggregate
    pub investor_count: i32,

    /// Total units held by aggregate
    pub total_units: f64,

    /// Voting percentage of aggregate
    pub voting_pct: f64,

    /// Economic percentage of aggregate
    pub economic_pct: f64,

    /// Breakdown by investor type
    #[serde(default)]
    pub by_type: Vec<AggregateBreakdown>,

    /// Breakdown by KYC status
    #[serde(default)]
    pub by_kyc_status: Vec<AggregateBreakdown>,

    /// Breakdown by jurisdiction (top 10)
    #[serde(default)]
    pub by_jurisdiction: Vec<AggregateBreakdown>,

    /// Whether drill-down is available (false if count > MAX_DRILLDOWN)
    #[serde(default)]
    pub can_drill_down: bool,

    /// Maximum page size for drill-down
    #[serde(default)]
    pub page_size: i32,

    /// Label for collapsed node, e.g., "4,847 other investors (22.0%)"
    pub display_label: String,
}

/// Breakdown category for aggregate summaries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateBreakdown {
    /// Category key (e.g., "RETAIL", "APPROVED", "LU")
    pub key: String,

    /// Display label
    pub label: String,

    /// Count of investors in category
    pub count: i32,

    /// Total units
    pub units: f64,

    /// Percentage of total
    pub pct: f64,
}

// =============================================================================
// PAGINATED INVESTOR LIST (For Drill-Down)
// =============================================================================

/// Paginated list of investors for drill-down view
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvestorListResponse {
    /// Current page items
    pub items: Vec<InvestorListItem>,

    /// Pagination info
    pub pagination: PaginationInfo,

    /// Applied filters (echoed back for UI state)
    pub filters: InvestorFilters,
}

/// Single investor in the list view
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvestorListItem {
    pub entity_id: String,
    pub name: String,
    pub entity_type: String,
    #[serde(default)]
    pub investor_type: Option<String>,
    pub units: f64,
    pub economic_pct: f64,
    pub voting_pct: f64,
    pub kyc_status: String,
    #[serde(default)]
    pub jurisdiction: Option<String>,
    #[serde(default)]
    pub acquisition_date: Option<String>,
}

/// Pagination metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationInfo {
    /// Current page (1-indexed)
    pub page: i32,
    /// Items per page
    pub page_size: i32,
    /// Total items matching filters
    pub total_items: i32,
    /// Total pages
    pub total_pages: i32,
}

/// Active filters for investor list (echoed in response)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InvestorFilters {
    #[serde(default)]
    pub investor_type: Option<String>,
    #[serde(default)]
    pub kyc_status: Option<String>,
    #[serde(default)]
    pub jurisdiction: Option<String>,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub min_units: Option<f64>,
}

// =============================================================================
// UI RENDERING HELPERS
// =============================================================================

impl ControlHolderNode {
    /// Get color hint based on control status
    pub fn control_tier(&self) -> ControlTier {
        if self.has_control {
            ControlTier::Control
        } else if self.has_significant_influence {
            ControlTier::Significant
        } else if self.above_disclosure {
            ControlTier::Disclosure
        } else {
            ControlTier::SpecialRights
        }
    }
}

/// Control tier for color-coding nodes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlTier {
    /// >50% - has control
    Control,
    /// >25% - significant influence
    Significant,
    /// >5% - above disclosure threshold
    Disclosure,
    /// Special rights only (board seats, veto, etc.)
    SpecialRights,
}

impl AggregateInvestorsNode {
    /// Get the breakdown for a specific dimension
    pub fn get_breakdown(&self, dimension: BreakdownDimension) -> &[AggregateBreakdown] {
        match dimension {
            BreakdownDimension::InvestorType => &self.by_type,
            BreakdownDimension::KycStatus => &self.by_kyc_status,
            BreakdownDimension::Jurisdiction => &self.by_jurisdiction,
        }
    }
}

/// Dimensions for aggregate breakdown
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakdownDimension {
    InvestorType,
    KycStatus,
    Jurisdiction,
}
