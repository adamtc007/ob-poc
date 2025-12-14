//! Trading Profile Document Types
//!
//! These types represent the complete trading profile document structure.
//! The document is stored as JSONB in cbu_trading_profiles and materialized
//! to operational tables (cbu_ssi, ssi_booking_rules, etc.).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Status of a trading profile
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProfileStatus {
    #[default]
    Draft,
    PendingReview,
    Active,
    Superseded,
    Archived,
}

/// Entity reference pattern - used throughout for lookups
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityRef {
    #[serde(rename = "type")]
    pub ref_type: EntityRefType,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EntityRefType {
    Lei,
    Bic,
    Name,
    Uuid,
}

/// Complete Trading Profile Document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingProfileDocument {
    /// What the CBU can trade
    pub universe: Universe,

    /// Investment manager mandates
    #[serde(default)]
    pub investment_managers: Vec<InvestmentManagerMandate>,

    /// ISDA agreements for OTC
    #[serde(default)]
    pub isda_agreements: Vec<IsdaAgreementConfig>,

    /// Settlement configuration
    #[serde(default)]
    pub settlement_config: Option<SettlementConfig>,

    /// ALERT-style booking rules
    #[serde(default)]
    pub booking_rules: Vec<BookingRule>,

    /// Standing settlement instructions by category
    #[serde(default)]
    pub standing_instructions: HashMap<String, Vec<StandingInstruction>>,

    /// Pricing source hierarchy
    #[serde(default)]
    pub pricing_matrix: Vec<PricingRule>,

    /// Valuation configuration
    #[serde(default)]
    pub valuation_config: Option<ValuationConfig>,

    /// Trading constraints
    #[serde(default)]
    pub constraints: Option<TradingConstraints>,

    /// Metadata
    #[serde(default)]
    pub metadata: Option<ProfileMetadata>,
}

// =============================================================================
// UNIVERSE
// =============================================================================

/// What the CBU can trade
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Universe {
    pub base_currency: String,

    #[serde(default)]
    pub allowed_currencies: Vec<String>,

    #[serde(default)]
    pub allowed_markets: Vec<MarketConfig>,

    #[serde(default)]
    pub instrument_classes: Vec<InstrumentClassConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketConfig {
    pub mic: String,

    #[serde(default)]
    pub currencies: Vec<String>,

    #[serde(default)]
    pub settlement_types: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstrumentClassConfig {
    pub class_code: String,

    #[serde(default)]
    pub cfi_prefixes: Vec<String>,

    #[serde(default)]
    pub isda_asset_classes: Vec<String>,

    #[serde(default = "default_true")]
    pub is_held: bool,

    #[serde(default = "default_true")]
    pub is_traded: bool,
}

fn default_true() -> bool {
    true
}

// =============================================================================
// INVESTMENT MANAGERS
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvestmentManagerMandate {
    pub priority: i32,
    pub manager: EntityRef,
    pub role: String,
    pub scope: ManagerScope,

    #[serde(default)]
    pub instruction_method: Option<String>,

    #[serde(default = "default_true")]
    pub can_trade: bool,

    #[serde(default = "default_true")]
    pub can_settle: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagerScope {
    #[serde(default)]
    pub all: bool,

    /// Market Identifier Codes (ISO 10383), e.g., ["XNYS", "XLON"]
    #[serde(default, alias = "markets")]
    pub mics: Vec<String>,

    #[serde(default)]
    pub instrument_classes: Vec<String>,
}

// =============================================================================
// ISDA AGREEMENTS
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsdaAgreementConfig {
    pub counterparty: EntityRef,
    pub agreement_date: String,
    pub governing_law: String,

    #[serde(default)]
    pub effective_date: Option<String>,

    #[serde(default)]
    pub product_coverage: Vec<ProductCoverage>,

    #[serde(default)]
    pub csa: Option<CsaConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductCoverage {
    pub asset_class: String,

    #[serde(default)]
    pub base_products: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsaConfig {
    pub csa_type: String, // VM, VM_IM

    #[serde(default)]
    pub threshold_amount: Option<i64>,

    #[serde(default)]
    pub threshold_currency: Option<String>,

    #[serde(default)]
    pub minimum_transfer_amount: Option<i64>,

    #[serde(default)]
    pub rounding_amount: Option<i64>,

    #[serde(default)]
    pub eligible_collateral: Vec<EligibleCollateral>,

    #[serde(default)]
    pub initial_margin: Option<InitialMarginConfig>,

    /// Reference to SSI name in standing_instructions.OTC_COLLATERAL
    /// The SSI must exist - validated before materialization
    #[serde(default)]
    pub collateral_ssi_ref: Option<String>,

    /// Deprecated: inline SSI definition. Use collateral_ssi_ref instead.
    /// Kept for backward compatibility during migration.
    #[serde(default)]
    pub collateral_ssi: Option<CollateralSsi>,

    #[serde(default)]
    pub valuation_time: Option<String>,

    #[serde(default)]
    pub valuation_timezone: Option<String>,

    #[serde(default)]
    pub notification_time: Option<String>,

    #[serde(default)]
    pub settlement_days: Option<i32>,

    #[serde(default)]
    pub dispute_resolution: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EligibleCollateral {
    #[serde(rename = "type")]
    pub collateral_type: String,

    #[serde(default)]
    pub currencies: Vec<String>,

    #[serde(default)]
    pub issuers: Vec<String>,

    #[serde(default)]
    pub min_rating: Option<String>,

    #[serde(default)]
    pub haircut_pct: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitialMarginConfig {
    pub calculation_method: String,

    #[serde(default)]
    pub posting_frequency: Option<String>,

    #[serde(default)]
    pub segregation_required: bool,

    #[serde(default)]
    pub custodian: Option<EntityRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollateralSsi {
    pub name: String,

    #[serde(default)]
    pub custody_account: Option<String>,

    #[serde(default)]
    pub custody_bic: Option<String>,

    #[serde(default)]
    pub cash_account: Option<String>,

    #[serde(default)]
    pub cash_bic: Option<String>,
}

// =============================================================================
// SETTLEMENT CONFIG
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementConfig {
    #[serde(default)]
    pub matching_platforms: Vec<MatchingPlatform>,

    #[serde(default)]
    pub settlement_identities: Vec<SettlementIdentity>,

    #[serde(default)]
    pub subcustodian_network: Vec<SubcustodianEntry>,

    #[serde(default)]
    pub enrichment_chain: Vec<String>,

    #[serde(default)]
    pub instruction_preferences: Vec<InstructionPreference>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchingPlatform {
    pub platform: String, // CTM, ALERT
    pub participant_id: String,

    /// Market Identifier Codes where this platform is enabled (ISO 10383)
    #[serde(default, alias = "enabled_markets")]
    pub enabled_mics: Vec<String>,

    #[serde(default)]
    pub matching_rules: Option<MatchingRules>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchingRules {
    #[serde(default)]
    pub auto_match: bool,

    #[serde(default)]
    pub tolerance_price_pct: Option<f64>,

    #[serde(default)]
    pub tolerance_quantity: Option<i64>,

    #[serde(default)]
    pub auto_affirm_threshold_usd: Option<i64>,

    #[serde(default)]
    pub enrichment_sources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementIdentity {
    pub role: String,

    #[serde(default)]
    pub bic: Option<String>,

    #[serde(default)]
    pub lei: Option<String>,

    #[serde(default)]
    pub alert_participant_id: Option<String>,

    #[serde(default)]
    pub ctm_participant_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubcustodianEntry {
    /// Market Identifier Code (ISO 10383), e.g., "XNYS", "XLON"
    #[serde(alias = "market")]
    pub mic: String,
    pub currency: String,
    pub subcustodian: SubcustodianInfo,

    #[serde(default)]
    pub place_of_settlement: Option<String>,

    #[serde(default = "default_true")]
    pub is_primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubcustodianInfo {
    pub bic: String,

    #[serde(default)]
    pub name: Option<String>,

    #[serde(default)]
    pub local_agent_account: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstructionPreference {
    pub instruction_type: String,

    #[serde(default)]
    pub swift_msg: Option<String>,

    #[serde(default)]
    pub iso20022_msg: Option<String>,

    #[serde(default)]
    pub auto_release: bool,

    #[serde(default)]
    pub requires_approval: bool,
}

// =============================================================================
// BOOKING RULES
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookingRule {
    pub name: String,
    pub priority: i32,

    #[serde(rename = "match")]
    pub match_criteria: BookingMatch,

    pub ssi_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BookingMatch {
    #[serde(default)]
    pub counterparty: Option<EntityRef>,

    #[serde(default)]
    pub instrument_class: Option<String>,

    #[serde(default)]
    pub security_type: Option<String>,

    /// Market Identifier Code (ISO 10383), e.g., "XNYS", "XLON"
    #[serde(default, alias = "market")]
    pub mic: Option<String>,

    #[serde(default)]
    pub currency: Option<String>,

    #[serde(default)]
    pub settlement_type: Option<String>,
}

// =============================================================================
// STANDING INSTRUCTIONS
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandingInstruction {
    pub name: String,

    /// Market Identifier Code (ISO 10383), e.g., "XNYS", "XLON"
    #[serde(default, alias = "market")]
    pub mic: Option<String>,

    #[serde(default)]
    pub currency: Option<String>,

    #[serde(default)]
    pub custody_account: Option<String>,

    #[serde(default)]
    pub custody_bic: Option<String>,

    #[serde(default)]
    pub cash_account: Option<String>,

    #[serde(default)]
    pub cash_bic: Option<String>,

    #[serde(default)]
    pub settlement_model: Option<String>,

    #[serde(default)]
    pub cutoff: Option<CutoffConfig>,

    // For OTC collateral SSIs - counterparty reference
    /// Counterparty as EntityRef (preferred for new documents)
    #[serde(default)]
    pub counterparty: Option<EntityRef>,

    /// Deprecated: counterparty LEI as string. Use counterparty instead.
    #[serde(default)]
    pub counterparty_lei: Option<String>,

    // For fund accounting
    #[serde(default)]
    pub provider_ref: Option<String>,

    #[serde(default)]
    pub channel: Option<String>,

    #[serde(default)]
    pub reporting_frequency: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CutoffConfig {
    pub time: String,
    pub timezone: String,
}

// =============================================================================
// PRICING MATRIX
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingRule {
    pub priority: i32,
    pub scope: PricingScope,
    pub source: String,

    #[serde(default)]
    pub price_type: Option<String>,

    #[serde(default)]
    pub fallback_source: Option<String>,

    #[serde(default)]
    pub max_age_hours: Option<i32>,

    #[serde(default)]
    pub tolerance_pct: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingScope {
    #[serde(default)]
    pub instrument_classes: Vec<String>,

    /// Market Identifier Codes (ISO 10383), e.g., ["XNYS", "XLON"]
    #[serde(default, alias = "markets")]
    pub mics: Vec<String>,
}

// =============================================================================
// VALUATION CONFIG
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValuationConfig {
    #[serde(default)]
    pub frequency: Option<String>,

    #[serde(default)]
    pub cutoff_time: Option<String>,

    #[serde(default)]
    pub timezone: Option<String>,

    #[serde(default)]
    pub pricing_point: Option<String>,

    #[serde(default)]
    pub swing_pricing: bool,

    #[serde(default)]
    pub swing_threshold_pct: Option<f64>,

    #[serde(default)]
    pub holiday_calendars: Vec<String>,
}

// =============================================================================
// CONSTRAINTS
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingConstraints {
    #[serde(default)]
    pub short_selling: Option<String>,

    #[serde(default)]
    pub leverage: Option<LeverageConstraints>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeverageConstraints {
    #[serde(default)]
    pub max_gross: Option<f64>,

    #[serde(default)]
    pub max_net: Option<f64>,
}

// =============================================================================
// METADATA
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfileMetadata {
    #[serde(default)]
    pub source: Option<String>,

    #[serde(default)]
    pub source_ref: Option<String>,

    #[serde(default)]
    pub created_by: Option<String>,

    #[serde(default)]
    pub notes: Option<String>,

    #[serde(default)]
    pub regulatory_framework: Option<String>,
}

// =============================================================================
// DATABASE ROW TYPES
// =============================================================================

/// Row from cbu_trading_profiles table
#[derive(Debug, Clone)]
pub struct TradingProfileRow {
    pub profile_id: Uuid,
    pub cbu_id: Uuid,
    pub version: i32,
    pub status: ProfileStatus,
    pub document: TradingProfileDocument,
    pub document_hash: String,
    pub created_by: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub activated_at: Option<chrono::DateTime<chrono::Utc>>,
    pub activated_by: Option<String>,
    pub notes: Option<String>,
}

/// Materialization audit record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterializationResult {
    pub profile_id: Uuid,
    pub sections_materialized: Vec<String>,
    pub records_created: HashMap<String, i32>,
    pub records_updated: HashMap<String, i32>,
    pub records_deleted: HashMap<String, i32>,
    pub errors: Vec<String>,
    pub duration_ms: i64,
}

// =============================================================================
// IMPORT/EXPORT TYPES
// =============================================================================

/// Full profile import structure (matches YAML seed format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingProfileImport {
    #[serde(default)]
    pub profile_id: Option<String>,

    pub cbu_id: String,

    #[serde(default = "default_version")]
    pub version: i32,

    #[serde(default)]
    pub status: ProfileStatus,

    #[serde(default)]
    pub as_of: Option<String>,

    #[serde(default)]
    pub name: Option<String>,

    // Document sections
    pub universe: Universe,

    #[serde(default)]
    pub investment_managers: Vec<InvestmentManagerMandate>,

    #[serde(default)]
    pub isda_agreements: Vec<IsdaAgreementConfig>,

    #[serde(default)]
    pub settlement_config: Option<SettlementConfig>,

    #[serde(default)]
    pub booking_rules: Vec<BookingRule>,

    #[serde(default)]
    pub standing_instructions: HashMap<String, Vec<StandingInstruction>>,

    #[serde(default)]
    pub pricing_matrix: Vec<PricingRule>,

    #[serde(default)]
    pub valuation_config: Option<ValuationConfig>,

    #[serde(default)]
    pub constraints: Option<TradingConstraints>,

    #[serde(default)]
    pub metadata: Option<ProfileMetadata>,
}

fn default_version() -> i32 {
    1
}

impl TradingProfileImport {
    /// Convert import format to document format
    pub fn into_document(self) -> TradingProfileDocument {
        TradingProfileDocument {
            universe: self.universe,
            investment_managers: self.investment_managers,
            isda_agreements: self.isda_agreements,
            settlement_config: self.settlement_config,
            booking_rules: self.booking_rules,
            standing_instructions: self.standing_instructions,
            pricing_matrix: self.pricing_matrix,
            valuation_config: self.valuation_config,
            constraints: self.constraints,
            metadata: self.metadata,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_seed_file() {
        let yaml = include_str!("../../config/seed/trading_profiles/allianzgi_complete.yaml");
        let result: Result<TradingProfileImport, _> = serde_yaml::from_str(yaml);
        assert!(
            result.is_ok(),
            "Failed to parse seed file: {:?}",
            result.err()
        );

        let profile = result.unwrap();
        assert_eq!(profile.universe.base_currency, "EUR");
        assert!(!profile.universe.allowed_markets.is_empty());
        assert!(!profile.booking_rules.is_empty());
        assert!(!profile.isda_agreements.is_empty());
    }

    #[test]
    fn test_convert_to_document() {
        let yaml = include_str!("../../config/seed/trading_profiles/allianzgi_complete.yaml");
        let import: TradingProfileImport = serde_yaml::from_str(yaml).unwrap();
        let doc = import.into_document();

        assert_eq!(doc.universe.base_currency, "EUR");
        assert!(!doc.booking_rules.is_empty());
    }
}
