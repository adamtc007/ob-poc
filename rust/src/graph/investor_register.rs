//! Investor Register Visualization Data
//!
//! Provides response structures for the investor register visualization.
//! Server computes thresholds and returns two components:
//! 1. Control holders (individual taxonomy nodes)
//! 2. Aggregate investors (collapsed node + breakdown data)
//!
//! # Design Principles
//!
//! - **Server owns thresholds and aggregation logic** - Client just renders
//! - **Parameter structs for complex queries** - Avoids too_many_arguments
//! - **Response structs are DTOs** - Serialized directly to JSON
//!
//! # Pattern: Parameter Structs
//!
//! When querying investor data, use `InvestorRegisterQuery` or `InvestorListQuery`
//! instead of passing 6+ individual arguments. This enables:
//! - `Option<T>` for optional filters
//! - Self-documenting field names
//! - Easy extension without breaking signatures

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// =============================================================================
// QUERY PARAMETER STRUCTS
// =============================================================================

/// Parameters for fetching investor register view
///
/// Use this struct instead of multiple function arguments.
/// All fields except `issuer_entity_id` are optional.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InvestorRegisterQuery {
    /// The issuer entity (fund, company) - REQUIRED
    pub issuer_entity_id: Uuid,

    /// Filter to specific share class (None = all classes)
    pub share_class_id: Option<Uuid>,

    /// As-of date for snapshot (None = today)
    pub as_of_date: Option<NaiveDate>,

    /// Include dilution instruments in fully-diluted view
    pub include_dilution: Option<bool>,

    /// Basis for control computation: VOTES, ECONOMIC, UNITS
    pub control_basis: Option<String>,
}

/// Parameters for fetching paginated investor list (drill-down)
///
/// Use this struct for the investor list endpoint.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InvestorListQuery {
    /// The issuer entity - REQUIRED
    pub issuer_entity_id: Uuid,

    /// Filter to specific share class
    pub share_class_id: Option<Uuid>,

    /// As-of date
    pub as_of_date: Option<NaiveDate>,

    /// Page number (1-indexed, default 1)
    pub page: Option<i32>,

    /// Page size (default 50, max 200)
    pub page_size: Option<i32>,

    /// Filter by investor type: INSTITUTIONAL, PROFESSIONAL, RETAIL, NOMINEE
    pub investor_type: Option<String>,

    /// Filter by KYC status: APPROVED, PENDING, REJECTED, EXPIRED
    pub kyc_status: Option<String>,

    /// Filter by jurisdiction (ISO country code)
    pub jurisdiction: Option<String>,

    /// Search by investor name (fuzzy match)
    pub search: Option<String>,

    /// Minimum units held
    pub min_units: Option<Decimal>,

    /// Sort field: name, units, pct, kyc_status, acquisition_date
    pub sort_by: Option<String>,

    /// Sort direction: asc, desc (default asc)
    pub sort_dir: Option<String>,
}

// =============================================================================
// RESPONSE STRUCTS (DTOs)
// =============================================================================

/// Complete response for investor register visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvestorRegisterView {
    /// The issuer entity (fund, company)
    pub issuer: IssuerSummary,

    /// Share class being viewed (if specific), or None for all classes
    pub share_class_filter: Option<Uuid>,

    /// As-of date for snapshot
    pub as_of_date: NaiveDate,

    /// Thresholds used for this view (from issuer_control_config)
    pub thresholds: ThresholdConfig,

    // =========================================================================
    // CONTROL HOLDERS (Individual Taxonomy Nodes)
    // =========================================================================
    /// Holders above threshold or with special rights
    /// These become individual nodes in the taxonomy graph
    pub control_holders: Vec<ControlHolderNode>,

    // =========================================================================
    // AGGREGATE INVESTORS (Collapsed Node)
    // =========================================================================
    /// Summary of all holders below threshold
    /// Becomes single "N other investors" node in taxonomy
    pub aggregate: Option<AggregateInvestorsNode>,

    // =========================================================================
    // VISUALIZATION HINTS
    // =========================================================================
    /// Total investor count (for UI display)
    pub total_investor_count: i32,

    /// Total issued units (denominator)
    pub total_issued_units: Decimal,

    /// Whether dilution data is available
    pub has_dilution_data: bool,
}

/// Summary of the issuer entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssuerSummary {
    pub entity_id: Uuid,
    pub name: String,
    pub entity_type: String,
    pub jurisdiction: Option<String>,
    pub lei: Option<String>,
}

/// Threshold configuration for control determination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdConfig {
    /// Disclosure threshold (default 5%)
    pub disclosure_pct: Decimal,
    /// Material threshold (default 10%)
    pub material_pct: Decimal,
    /// Significant influence threshold (default 25%)
    pub significant_pct: Decimal,
    /// Control threshold (default 50%)
    pub control_pct: Decimal,
    /// Basis for control computation: VOTES or ECONOMIC
    pub control_basis: String,
}

impl Default for ThresholdConfig {
    fn default() -> Self {
        Self {
            disclosure_pct: Decimal::new(5, 0),
            material_pct: Decimal::new(10, 0),
            significant_pct: Decimal::new(25, 0),
            control_pct: Decimal::new(50, 0),
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
    pub entity_id: Uuid,

    /// Display name
    pub name: String,

    /// Entity type: PROPER_PERSON, LIMITED_COMPANY, etc.
    pub entity_type: String,

    /// Investor classification: INSTITUTIONAL, PROFESSIONAL, RETAIL, etc.
    pub investor_type: Option<String>,

    // =========================================================================
    // OWNERSHIP DATA
    // =========================================================================
    /// Total units held (across all share classes if not filtered)
    pub units: Decimal,

    /// Voting percentage (may differ from economic)
    pub voting_pct: Decimal,

    /// Economic percentage
    pub economic_pct: Decimal,

    // =========================================================================
    // CONTROL FLAGS (Visualization Hints)
    // =========================================================================
    /// Has voting control (>control_threshold)
    pub has_control: bool,

    /// Has significant influence (>significant_threshold)
    pub has_significant_influence: bool,

    /// Above disclosure threshold
    pub above_disclosure: bool,

    // =========================================================================
    // SPECIAL RIGHTS
    // =========================================================================
    /// Board appointment rights (number of seats)
    pub board_seats: i32,

    /// Veto rights held (e.g., ["VETO_MA", "VETO_FUNDRAISE"])
    pub veto_rights: Vec<String>,

    /// Other special rights
    pub other_rights: Vec<String>,

    // =========================================================================
    // RENDERING HINTS
    // =========================================================================
    /// Why this holder is shown individually (for tooltip)
    /// e.g., ">5% voting", "Board rights", "Significant influence"
    pub inclusion_reason: String,

    /// KYC status for badge: APPROVED, PENDING, REJECTED, EXPIRED
    pub kyc_status: String,

    /// Position in hierarchy (for layout, 0 = direct holder)
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
    pub total_units: Decimal,

    /// Voting percentage of aggregate
    pub voting_pct: Decimal,

    /// Economic percentage of aggregate
    pub economic_pct: Decimal,

    // =========================================================================
    // BREAKDOWN DATA (For Drill-Down)
    // =========================================================================
    /// Breakdown by investor type
    pub by_type: Vec<AggregateBreakdown>,

    /// Breakdown by KYC status
    pub by_kyc_status: Vec<AggregateBreakdown>,

    /// Breakdown by jurisdiction (top 10)
    pub by_jurisdiction: Vec<AggregateBreakdown>,

    // =========================================================================
    // VISUALIZATION HINTS
    // =========================================================================
    /// Whether drill-down is available (false if count > MAX_DRILLDOWN)
    pub can_drill_down: bool,

    /// Maximum page size for drill-down
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
    pub units: Decimal,

    /// Percentage of total
    pub pct: Decimal,
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
    pub entity_id: Uuid,
    pub name: String,
    pub entity_type: String,
    pub investor_type: Option<String>,
    pub units: Decimal,
    pub economic_pct: Decimal,
    pub voting_pct: Decimal,
    pub kyc_status: String,
    pub jurisdiction: Option<String>,
    pub acquisition_date: Option<NaiveDate>,
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
    pub investor_type: Option<String>,
    pub kyc_status: Option<String>,
    pub jurisdiction: Option<String>,
    pub search: Option<String>,
    pub min_units: Option<Decimal>,
}

// =============================================================================
// BUILDER HELPERS
// =============================================================================

impl InvestorRegisterQuery {
    /// Create a new query for an issuer
    pub fn new(issuer_entity_id: Uuid) -> Self {
        Self {
            issuer_entity_id,
            ..Default::default()
        }
    }

    /// Set share class filter
    pub fn with_share_class(mut self, share_class_id: Uuid) -> Self {
        self.share_class_id = Some(share_class_id);
        self
    }

    /// Set as-of date
    pub fn with_as_of(mut self, date: NaiveDate) -> Self {
        self.as_of_date = Some(date);
        self
    }

    /// Include dilution instruments
    pub fn with_dilution(mut self) -> Self {
        self.include_dilution = Some(true);
        self
    }

    /// Set control basis
    pub fn with_basis(mut self, basis: &str) -> Self {
        self.control_basis = Some(basis.to_string());
        self
    }
}

impl InvestorListQuery {
    /// Create a new query for an issuer
    pub fn new(issuer_entity_id: Uuid) -> Self {
        Self {
            issuer_entity_id,
            page: Some(1),
            page_size: Some(50),
            ..Default::default()
        }
    }

    /// Set page
    pub fn with_page(mut self, page: i32) -> Self {
        self.page = Some(page);
        self
    }

    /// Set page size
    pub fn with_page_size(mut self, size: i32) -> Self {
        self.page_size = Some(size.min(200)); // Cap at 200
        self
    }

    /// Filter by investor type
    pub fn with_investor_type(mut self, investor_type: &str) -> Self {
        self.investor_type = Some(investor_type.to_string());
        self
    }

    /// Filter by KYC status
    pub fn with_kyc_status(mut self, status: &str) -> Self {
        self.kyc_status = Some(status.to_string());
        self
    }

    /// Search by name
    pub fn with_search(mut self, search: &str) -> Self {
        self.search = Some(search.to_string());
        self
    }

    /// Sort by field
    pub fn with_sort(mut self, field: &str, ascending: bool) -> Self {
        self.sort_by = Some(field.to_string());
        self.sort_dir = Some(if ascending { "asc" } else { "desc" }.to_string());
        self
    }
}

// =============================================================================
// CONVENIENCE CONSTRUCTORS
// =============================================================================

impl AggregateInvestorsNode {
    /// Create display label from count and percentage
    pub fn make_display_label(count: i32, economic_pct: Decimal) -> String {
        format!("{} other investors ({:.1}%)", count, economic_pct)
    }
}

impl ControlHolderNode {
    /// Determine inclusion reason based on flags
    pub fn compute_inclusion_reason(&self, thresholds: &ThresholdConfig) -> String {
        let mut reasons = Vec::new();

        if self.has_control {
            reasons.push(format!(">{}% control", thresholds.control_pct));
        } else if self.has_significant_influence {
            reasons.push(format!(">{}% significant", thresholds.significant_pct));
        } else if self.above_disclosure {
            reasons.push(format!(
                ">{}% {}",
                thresholds.disclosure_pct, thresholds.control_basis
            ));
        }

        if self.board_seats > 0 {
            reasons.push(format!("{} board seat(s)", self.board_seats));
        }

        if !self.veto_rights.is_empty() {
            reasons.push("Veto rights".to_string());
        }

        if reasons.is_empty() {
            "Special rights".to_string()
        } else {
            reasons.join(", ")
        }
    }
}
