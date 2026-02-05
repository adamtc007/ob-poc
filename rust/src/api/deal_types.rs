//! Deal Taxonomy Types
//!
//! Type definitions for the Deal Taxonomy Builder, mirroring the CBU taxonomy pattern.
//! These types represent the deal hierarchy: Deal → Products → Rate Cards → Lines,
//! along with linked entities like Participants, Contracts, and Onboarding Requests.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Core Deal Types
// ============================================================================

/// Summary view of a deal for graph/taxonomy display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DealSummary {
    pub deal_id: Uuid,
    pub deal_name: String,
    pub deal_reference: Option<String>,
    pub deal_status: String,
    pub primary_client_group_id: Uuid,
    /// Resolved client group name for display
    pub client_group_name: Option<String>,
    pub sales_owner: Option<String>,
    pub sales_team: Option<String>,
    pub estimated_revenue: Option<Decimal>,
    pub currency_code: Option<String>,
    pub opened_at: DateTime<Utc>,
    pub qualified_at: Option<DateTime<Utc>>,
    pub contracted_at: Option<DateTime<Utc>>,
    pub active_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
    /// Computed counts for UI display
    pub product_count: i32,
    pub rate_card_count: i32,
    pub participant_count: i32,
    pub contract_count: i32,
    pub onboarding_request_count: i32,
}

/// Full deal graph response for taxonomy visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DealGraphResponse {
    pub deal: DealSummary,
    pub products: Vec<DealProductSummary>,
    pub rate_cards: Vec<RateCardSummary>,
    pub participants: Vec<DealParticipantSummary>,
    pub contracts: Vec<DealContractSummary>,
    pub onboarding_requests: Vec<OnboardingRequestSummary>,
    /// View mode: COMMERCIAL, FINANCIAL, STATUS
    pub view_mode: String,
}

// ============================================================================
// Product Types
// ============================================================================

/// Product associated with a deal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DealProductSummary {
    pub deal_product_id: Uuid,
    pub deal_id: Uuid,
    pub product_id: Uuid,
    /// Resolved product name
    pub product_name: String,
    pub product_code: Option<String>,
    pub product_category: Option<String>,
    pub product_status: String,
    pub indicative_revenue: Option<Decimal>,
    pub currency_code: Option<String>,
    pub notes: Option<String>,
    pub added_at: Option<DateTime<Utc>>,
    pub agreed_at: Option<DateTime<Utc>>,
    /// Count of rate cards for this product
    pub rate_card_count: i32,
}

// ============================================================================
// Rate Card Types
// ============================================================================

/// Rate card summary for deal taxonomy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateCardSummary {
    pub rate_card_id: Uuid,
    pub deal_id: Uuid,
    pub contract_id: Uuid,
    pub product_id: Uuid,
    /// Resolved product name for display
    pub product_name: Option<String>,
    pub rate_card_name: Option<String>,
    pub effective_from: NaiveDate,
    pub effective_to: Option<NaiveDate>,
    pub status: Option<String>,
    pub negotiation_round: Option<i32>,
    pub superseded_by: Option<Uuid>,
    /// Count of lines in this rate card
    pub line_count: i32,
    /// Is this the active rate card (not superseded, within effective dates)?
    pub is_active: bool,
}

/// Individual rate card line item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateCardLineSummary {
    pub line_id: Uuid,
    pub rate_card_id: Uuid,
    pub fee_type: String,
    pub fee_subtype: String,
    pub pricing_model: String,
    pub rate_value: Option<Decimal>,
    pub minimum_fee: Option<Decimal>,
    pub maximum_fee: Option<Decimal>,
    pub currency_code: Option<String>,
    pub tier_brackets: Option<serde_json::Value>,
    pub fee_basis: Option<String>,
    pub description: Option<String>,
    pub sequence_order: Option<i32>,
}

/// Rate card with its lines (for detail view)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateCardDetail {
    pub rate_card: RateCardSummary,
    pub lines: Vec<RateCardLineSummary>,
    /// Supersession chain (previous versions)
    pub history: Vec<RateCardSummary>,
}

// ============================================================================
// Participant Types
// ============================================================================

/// Participant in a deal (regional LEI entities)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DealParticipantSummary {
    pub deal_participant_id: Uuid,
    pub deal_id: Uuid,
    pub entity_id: Uuid,
    /// Resolved entity name
    pub entity_name: String,
    pub participant_role: String,
    pub lei: Option<String>,
    pub is_primary: bool,
    pub created_at: Option<DateTime<Utc>>,
}

// ============================================================================
// Contract Types
// ============================================================================

/// Contract linked to a deal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DealContractSummary {
    pub contract_id: Uuid,
    pub deal_id: Uuid,
    pub contract_role: Option<String>,
    pub sequence_order: i32,
    /// Resolved contract details
    pub client_label: Option<String>,
    pub contract_reference: Option<String>,
    pub effective_date: Option<NaiveDate>,
    pub termination_date: Option<NaiveDate>,
    pub status: Option<String>,
}

// ============================================================================
// Onboarding Request Types
// ============================================================================

/// Onboarding request linked via deal products
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingRequestSummary {
    pub request_id: Uuid,
    pub cbu_id: Uuid,
    /// Resolved CBU name
    pub cbu_name: Option<String>,
    pub request_state: String,
    pub current_phase: Option<String>,
    pub created_by: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    /// Link back to deal via product
    pub deal_product_id: Option<Uuid>,
}

// ============================================================================
// Session Context Types
// ============================================================================

/// Deal context stored in session for navigation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDealContext {
    pub deal_id: Uuid,
    pub deal_name: String,
    pub deal_status: String,
    pub client_group_name: Option<String>,
}

// ============================================================================
// Query Filter Types
// ============================================================================

/// Filters for deal listing/search
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DealFilters {
    pub client_group_id: Option<Uuid>,
    pub deal_status: Option<String>,
    pub sales_owner: Option<String>,
    pub sales_team: Option<String>,
    pub opened_after: Option<DateTime<Utc>>,
    pub opened_before: Option<DateTime<Utc>>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

/// View mode for deal taxonomy display
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DealViewMode {
    /// Commercial view: products, participants, revenue
    #[default]
    Commercial,
    /// Financial view: rate cards, billing, fees
    Financial,
    /// Status view: onboarding progress, milestones
    Status,
}

impl std::fmt::Display for DealViewMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Commercial => write!(f, "COMMERCIAL"),
            Self::Financial => write!(f, "FINANCIAL"),
            Self::Status => write!(f, "STATUS"),
        }
    }
}

impl std::str::FromStr for DealViewMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "COMMERCIAL" => Ok(Self::Commercial),
            "FINANCIAL" => Ok(Self::Financial),
            "STATUS" => Ok(Self::Status),
            _ => Err(anyhow::anyhow!("Invalid view mode: {}", s)),
        }
    }
}

// ============================================================================
// API Request/Response Types
// ============================================================================

/// Request to load a deal into session context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadDealRequest {
    /// Deal ID to load (either this or deal_name required)
    pub deal_id: Option<Uuid>,
    /// Deal name to search for
    pub deal_name: Option<String>,
}

/// Response after loading a deal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadDealResponse {
    pub deal_id: Uuid,
    pub deal_name: String,
    pub deal_status: String,
    pub client_group_name: Option<String>,
    pub message: String,
}

/// Response for deal list queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DealListResponse {
    pub deals: Vec<DealSummary>,
    pub total_count: i64,
    pub offset: i32,
    pub limit: i32,
}
