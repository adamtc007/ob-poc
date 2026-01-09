//! GLEIF API response types
//! Complete mapping of Level 1 and Level 2 data structures
//!
//! # External API Resilience
//!
//! All enums that map to GLEIF API string values use the Unknown(String) pattern.
//! GLEIF may add new codes at any time - we never fail on unknown values.
//! See `rust/src/gleif/mod.rs` for the full resilience pattern documentation.
//!
//! Reference: https://api.gleif.org/api/v1/lei-records

use serde::{Deserialize, Serialize};

// =============================================================================
// Hardened Enum Types - Unknown variants capture unrecognized API values
// =============================================================================

/// GLEIF Entity Category (FUND, GENERAL, BRANCH, etc.)
///
/// Uses resilience pattern - unknown values captured verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityCategory {
    Fund,
    General,
    Branch,
    SoleProprietor,
    /// Unknown category from GLEIF - captured verbatim
    Unknown(String),
}

impl EntityCategory {
    pub fn parse(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "FUND" => Self::Fund,
            "GENERAL" => Self::General,
            "BRANCH" => Self::Branch,
            "SOLE_PROPRIETOR" => Self::SoleProprietor,
            other => {
                tracing::debug!(code = other, "Unknown GLEIF entity category");
                Self::Unknown(s.to_string())
            }
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Fund => "FUND",
            Self::General => "GENERAL",
            Self::Branch => "BRANCH",
            Self::SoleProprietor => "SOLE_PROPRIETOR",
            Self::Unknown(s) => s.as_str(),
        }
    }

    pub fn is_fund(&self) -> bool {
        matches!(self, Self::Fund)
    }

    pub fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown(_))
    }
}

impl Default for EntityCategory {
    fn default() -> Self {
        Self::Unknown("UNSPECIFIED".to_string())
    }
}

/// Fund structure type - legal wrapper/vehicle type
///
/// Uses resilience pattern - unknown values captured verbatim.
/// GLEIF and other sources may use different terminology.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FundStructureType {
    /// SICAV - Société d'Investissement à Capital Variable
    Sicav,
    /// ICAV - Irish Collective Asset-management Vehicle
    Icav,
    /// OEIC - Open-Ended Investment Company (UK)
    Oeic,
    /// VCC - Variable Capital Company (Singapore)
    Vcc,
    /// Unit Trust
    UnitTrust,
    /// FCP - Fonds Commun de Placement
    Fcp,
    /// Limited Partnership
    LimitedPartnership,
    /// LLC - Limited Liability Company
    Llc,
    /// Corporate (standard company structure)
    Corporate,
    /// Unknown structure from external source - captured verbatim
    Unknown(String),
}

impl FundStructureType {
    pub fn parse(s: &str) -> Self {
        match s.to_uppercase().replace(['-', ' '], "_").as_str() {
            "SICAV" => Self::Sicav,
            "ICAV" => Self::Icav,
            "OEIC" => Self::Oeic,
            "VCC" => Self::Vcc,
            "UNIT_TRUST" | "UNITTRUST" => Self::UnitTrust,
            "FCP" => Self::Fcp,
            "LIMITED_PARTNERSHIP" | "LP" => Self::LimitedPartnership,
            "LLC" => Self::Llc,
            "CORPORATE" | "CORP" => Self::Corporate,
            other => {
                tracing::debug!(code = other, "Unknown fund structure type");
                Self::Unknown(s.to_string())
            }
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Sicav => "SICAV",
            Self::Icav => "ICAV",
            Self::Oeic => "OEIC",
            Self::Vcc => "VCC",
            Self::UnitTrust => "UNIT_TRUST",
            Self::Fcp => "FCP",
            Self::LimitedPartnership => "LIMITED_PARTNERSHIP",
            Self::Llc => "LLC",
            Self::Corporate => "CORPORATE",
            Self::Unknown(s) => s.as_str(),
        }
    }

    pub fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown(_))
    }
}

impl Default for FundStructureType {
    fn default() -> Self {
        Self::Unknown("UNSPECIFIED".to_string())
    }
}

/// Fund type - regulatory/strategy classification
///
/// Uses resilience pattern - unknown values captured verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FundType {
    /// UCITS - EU retail fund
    Ucits,
    /// AIF - Alternative Investment Fund
    Aif,
    /// Hedge Fund
    HedgeFund,
    /// Private Equity
    PrivateEquity,
    /// Venture Capital
    VentureCapital,
    /// Real Estate / REIT
    RealEstate,
    /// Infrastructure
    Infrastructure,
    /// Fund of Funds
    FundOfFunds,
    /// ETF - Exchange Traded Fund
    Etf,
    /// Money Market Fund
    MoneyMarket,
    /// Pension Fund
    PensionFund,
    /// Sovereign Wealth Fund
    SovereignWealth,
    /// ELTIF - European Long-Term Investment Fund
    Eltif,
    /// RAIF - Reserved Alternative Investment Fund
    Raif,
    /// Unknown type from external source - captured verbatim
    Unknown(String),
}

impl FundType {
    pub fn parse(s: &str) -> Self {
        match s.to_uppercase().replace(['-', ' '], "_").as_str() {
            "UCITS" => Self::Ucits,
            "AIF" | "AIFMD" => Self::Aif,
            "HEDGE_FUND" | "HEDGEFUND" => Self::HedgeFund,
            "PRIVATE_EQUITY" | "PE" => Self::PrivateEquity,
            "VENTURE_CAPITAL" | "VC" => Self::VentureCapital,
            "REAL_ESTATE" | "REIT" => Self::RealEstate,
            "INFRASTRUCTURE" | "INFRA" => Self::Infrastructure,
            "FUND_OF_FUNDS" | "FOF" => Self::FundOfFunds,
            "ETF" => Self::Etf,
            "MONEY_MARKET" | "MMF" => Self::MoneyMarket,
            "PENSION_FUND" | "PENSION" => Self::PensionFund,
            "SOVEREIGN_WEALTH" | "SWF" => Self::SovereignWealth,
            "ELTIF" => Self::Eltif,
            "RAIF" => Self::Raif,
            other => {
                tracing::debug!(code = other, "Unknown fund type");
                Self::Unknown(s.to_string())
            }
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Ucits => "UCITS",
            Self::Aif => "AIF",
            Self::HedgeFund => "HEDGE_FUND",
            Self::PrivateEquity => "PRIVATE_EQUITY",
            Self::VentureCapital => "VENTURE_CAPITAL",
            Self::RealEstate => "REAL_ESTATE",
            Self::Infrastructure => "INFRASTRUCTURE",
            Self::FundOfFunds => "FUND_OF_FUNDS",
            Self::Etf => "ETF",
            Self::MoneyMarket => "MONEY_MARKET",
            Self::PensionFund => "PENSION_FUND",
            Self::SovereignWealth => "SOVEREIGN_WEALTH",
            Self::Eltif => "ELTIF",
            Self::Raif => "RAIF",
            Self::Unknown(s) => s.as_str(),
        }
    }

    pub fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown(_))
    }

    /// Is this a retail-eligible fund type?
    pub fn is_retail_eligible(&self) -> bool {
        matches!(self, Self::Ucits | Self::Etf | Self::MoneyMarket)
    }

    /// Is this an alternative/professional fund type?
    pub fn is_alternative(&self) -> bool {
        matches!(
            self,
            Self::Aif
                | Self::HedgeFund
                | Self::PrivateEquity
                | Self::VentureCapital
                | Self::RealEstate
                | Self::Infrastructure
                | Self::Eltif
                | Self::Raif
        )
    }
}

impl Default for FundType {
    fn default() -> Self {
        Self::Unknown("UNSPECIFIED".to_string())
    }
}

/// GLEIF Entity Status (ACTIVE, INACTIVE, etc.)
///
/// Uses resilience pattern - unknown values captured verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityStatus {
    Active,
    Inactive,
    /// Unknown status from GLEIF - captured verbatim
    Unknown(String),
}

impl EntityStatus {
    pub fn parse(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "ACTIVE" => Self::Active,
            "INACTIVE" => Self::Inactive,
            other => {
                tracing::debug!(code = other, "Unknown GLEIF entity status");
                Self::Unknown(s.to_string())
            }
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Active => "ACTIVE",
            Self::Inactive => "INACTIVE",
            Self::Unknown(s) => s.as_str(),
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }

    pub fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown(_))
    }
}

impl Default for EntityStatus {
    fn default() -> Self {
        Self::Unknown("UNSPECIFIED".to_string())
    }
}

/// GLEIF Registration Status (ISSUED, LAPSED, MERGED, etc.)
///
/// Uses resilience pattern - unknown values captured verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegistrationStatus {
    Issued,
    Lapsed,
    Merged,
    Retired,
    Annulled,
    Cancelled,
    Transferred,
    PendingTransfer,
    PendingArchival,
    Duplicate,
    /// Unknown status from GLEIF - captured verbatim
    Unknown(String),
}

impl RegistrationStatus {
    pub fn parse(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "ISSUED" => Self::Issued,
            "LAPSED" => Self::Lapsed,
            "MERGED" => Self::Merged,
            "RETIRED" => Self::Retired,
            "ANNULLED" => Self::Annulled,
            "CANCELLED" => Self::Cancelled,
            "TRANSFERRED" => Self::Transferred,
            "PENDING_TRANSFER" => Self::PendingTransfer,
            "PENDING_ARCHIVAL" => Self::PendingArchival,
            "DUPLICATE" => Self::Duplicate,
            other => {
                tracing::debug!(code = other, "Unknown GLEIF registration status");
                Self::Unknown(s.to_string())
            }
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Issued => "ISSUED",
            Self::Lapsed => "LAPSED",
            Self::Merged => "MERGED",
            Self::Retired => "RETIRED",
            Self::Annulled => "ANNULLED",
            Self::Cancelled => "CANCELLED",
            Self::Transferred => "TRANSFERRED",
            Self::PendingTransfer => "PENDING_TRANSFER",
            Self::PendingArchival => "PENDING_ARCHIVAL",
            Self::Duplicate => "DUPLICATE",
            Self::Unknown(s) => s.as_str(),
        }
    }

    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Issued)
    }

    pub fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown(_))
    }
}

impl Default for RegistrationStatus {
    fn default() -> Self {
        Self::Unknown("UNSPECIFIED".to_string())
    }
}

/// GLEIF Corroboration Level (FULLY_CORROBORATED, PARTIALLY_CORROBORATED, etc.)
///
/// Uses resilience pattern - unknown values captured verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CorroborationLevel {
    FullyCorroborated,
    PartiallyCorroborated,
    NotCorroborated,
    /// Unknown level from GLEIF - captured verbatim
    Unknown(String),
}

impl CorroborationLevel {
    pub fn parse(s: &str) -> Self {
        match s.to_uppercase().replace('-', "_").as_str() {
            "FULLY_CORROBORATED" => Self::FullyCorroborated,
            "PARTIALLY_CORROBORATED" => Self::PartiallyCorroborated,
            "NOT_CORROBORATED" => Self::NotCorroborated,
            other => {
                tracing::debug!(code = other, "Unknown GLEIF corroboration level");
                Self::Unknown(s.to_string())
            }
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::FullyCorroborated => "FULLY_CORROBORATED",
            Self::PartiallyCorroborated => "PARTIALLY_CORROBORATED",
            Self::NotCorroborated => "NOT_CORROBORATED",
            Self::Unknown(s) => s.as_str(),
        }
    }

    pub fn is_fully_corroborated(&self) -> bool {
        matches!(self, Self::FullyCorroborated)
    }

    pub fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown(_))
    }
}

impl Default for CorroborationLevel {
    fn default() -> Self {
        Self::Unknown("UNSPECIFIED".to_string())
    }
}

/// GLEIF Relationship Type (IS_DIRECTLY_CONSOLIDATED_BY, IS_FUND_MANAGED_BY, etc.)
///
/// Uses resilience pattern - unknown values captured verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationshipType {
    IsDirectlyConsolidatedBy,
    IsUltimatelyConsolidatedBy,
    IsFundManagedBy,
    IsSubfundOf,
    IsFeederTo,
    /// Unknown relationship type from GLEIF - captured verbatim
    Unknown(String),
}

impl RelationshipType {
    pub fn parse(s: &str) -> Self {
        match s.to_uppercase().replace('-', "_").as_str() {
            "IS_DIRECTLY_CONSOLIDATED_BY" => Self::IsDirectlyConsolidatedBy,
            "IS_ULTIMATELY_CONSOLIDATED_BY" => Self::IsUltimatelyConsolidatedBy,
            "IS_FUND_MANAGED_BY" | "IS_FUND-MANAGED_BY" => Self::IsFundManagedBy,
            "IS_SUBFUND_OF" | "IS_SUB_FUND_OF" => Self::IsSubfundOf,
            "IS_FEEDER_TO" => Self::IsFeederTo,
            other => {
                tracing::debug!(code = other, "Unknown GLEIF relationship type");
                Self::Unknown(s.to_string())
            }
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::IsDirectlyConsolidatedBy => "IS_DIRECTLY_CONSOLIDATED_BY",
            Self::IsUltimatelyConsolidatedBy => "IS_ULTIMATELY_CONSOLIDATED_BY",
            Self::IsFundManagedBy => "IS_FUND-MANAGED_BY",
            Self::IsSubfundOf => "IS_SUBFUND_OF",
            Self::IsFeederTo => "IS_FEEDER_TO",
            Self::Unknown(s) => s.as_str(),
        }
    }

    pub fn is_parent_relationship(&self) -> bool {
        matches!(
            self,
            Self::IsDirectlyConsolidatedBy | Self::IsUltimatelyConsolidatedBy
        )
    }

    pub fn is_fund_relationship(&self) -> bool {
        matches!(
            self,
            Self::IsFundManagedBy | Self::IsSubfundOf | Self::IsFeederTo
        )
    }

    pub fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown(_))
    }
}

impl Default for RelationshipType {
    fn default() -> Self {
        Self::Unknown("UNSPECIFIED".to_string())
    }
}

/// Top-level API response wrapper
#[derive(Debug, Clone, Deserialize)]
pub struct GleifResponse<T> {
    pub data: T,
    #[serde(default)]
    pub links: Option<PaginationLinks>,
    #[serde(default)]
    pub meta: Option<ResponseMeta>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PaginationLinks {
    pub first: Option<String>,
    pub prev: Option<String>,
    pub next: Option<String>,
    pub last: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponseMeta {
    #[serde(rename = "goldenCopy")]
    pub golden_copy: Option<GoldenCopyInfo>,
    pub pagination: Option<PaginationInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GoldenCopyInfo {
    #[serde(rename = "publishDate")]
    pub publish_date: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PaginationInfo {
    #[serde(rename = "currentPage")]
    pub current_page: i32,
    #[serde(rename = "perPage")]
    pub per_page: i32,
    /// May be null for empty results
    #[serde(default)]
    pub from: Option<i32>,
    /// May be null for empty results
    #[serde(default)]
    pub to: Option<i32>,
    pub total: i32,
    #[serde(rename = "lastPage")]
    pub last_page: i32,
}

/// LEI Record (Level 1 data)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LeiRecord {
    pub id: String, // The LEI
    #[serde(rename = "type")]
    pub record_type: String,
    pub attributes: LeiAttributes,
    #[serde(default)]
    pub relationships: Option<LeiRelationships>,
    /// Record-specific links (present in search results)
    #[serde(default)]
    pub links: Option<serde_json::Value>,
}

impl LeiRecord {
    /// Get the LEI, preferring attributes.lei but falling back to id
    pub fn lei(&self) -> &str {
        self.attributes.lei.as_deref().unwrap_or_else(|| &self.id)
    }

    /// Get entity category as typed enum (never fails - unknown values captured)
    pub fn category(&self) -> EntityCategory {
        self.attributes
            .entity
            .category
            .as_deref()
            .map(EntityCategory::parse)
            .unwrap_or_default()
    }

    /// Get entity status as typed enum (never fails - unknown values captured)
    pub fn entity_status(&self) -> EntityStatus {
        self.attributes
            .entity
            .status
            .as_deref()
            .map(EntityStatus::parse)
            .unwrap_or_default()
    }

    /// Get registration status as typed enum (never fails - unknown values captured)
    pub fn registration_status(&self) -> RegistrationStatus {
        self.attributes
            .registration
            .status
            .as_deref()
            .map(RegistrationStatus::parse)
            .unwrap_or_default()
    }

    /// Get corroboration level as typed enum (never fails - unknown values captured)
    pub fn corroboration_level(&self) -> CorroborationLevel {
        self.attributes
            .registration
            .corroboration_level
            .as_deref()
            .map(CorroborationLevel::parse)
            .unwrap_or_default()
    }

    /// Check if this is a fund entity
    pub fn is_fund(&self) -> bool {
        self.category().is_fund()
    }

    /// Check if this entity has an active status
    pub fn is_active(&self) -> bool {
        self.entity_status().is_active()
    }

    /// Check if this LEI registration is valid/issued
    pub fn is_registration_valid(&self) -> bool {
        self.registration_status().is_valid()
    }

    /// Get legal name safely (never panics)
    pub fn legal_name(&self) -> &str {
        &self.attributes.entity.legal_name.name
    }

    /// Get jurisdiction if present
    pub fn jurisdiction(&self) -> Option<&str> {
        self.attributes.entity.jurisdiction.as_deref()
    }

    /// Get legal form ID if present
    pub fn legal_form_id(&self) -> Option<&str> {
        self.attributes
            .entity
            .legal_form
            .as_ref()
            .and_then(|lf| lf.id.as_deref())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LeiAttributes {
    /// The LEI - may be missing in some older records (use parent id instead)
    #[serde(default)]
    pub lei: Option<String>,
    pub entity: EntityInfo,
    pub registration: RegistrationInfo,
    #[serde(rename = "conformityFlag")]
    pub conformity_flag: Option<String>,
    // Additional optional fields that may be present in API responses
    // These fields have polymorphic types (can be null, string, or array)
    // so we use serde_json::Value to handle them generically
    #[serde(default)]
    pub bic: Option<serde_json::Value>,
    #[serde(default)]
    pub mic: Option<serde_json::Value>,
    #[serde(default)]
    pub ocid: Option<serde_json::Value>,
    #[serde(default)]
    pub qcc: Option<serde_json::Value>,
    #[serde(default)]
    pub spglobal: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EntityInfo {
    #[serde(rename = "legalName")]
    pub legal_name: NameValue,

    #[serde(rename = "otherNames", default)]
    pub other_names: Vec<OtherName>,

    #[serde(rename = "transliteratedOtherNames", default)]
    pub transliterated_other_names: Vec<OtherName>,

    #[serde(rename = "legalAddress")]
    pub legal_address: Address,

    #[serde(rename = "headquartersAddress")]
    pub headquarters_address: Option<Address>,

    #[serde(rename = "otherAddresses", default)]
    pub other_addresses: Vec<TypedAddress>,

    #[serde(rename = "registeredAt")]
    pub registered_at: Option<RegistrationAuthority>,

    #[serde(rename = "registeredAs")]
    pub registered_as: Option<String>,

    pub jurisdiction: Option<String>,
    pub category: Option<String>,

    #[serde(rename = "subCategory")]
    pub sub_category: Option<String>,

    #[serde(rename = "legalForm")]
    pub legal_form: Option<LegalForm>,

    pub status: Option<String>,

    #[serde(rename = "creationDate")]
    pub creation_date: Option<String>,

    #[serde(rename = "expirationDate")]
    pub expiration_date: Option<String>,

    #[serde(rename = "expirationReason")]
    pub expiration_reason: Option<String>,

    #[serde(rename = "successorEntities", default)]
    pub successor_entities: Vec<SuccessorEntity>,

    #[serde(rename = "eventGroups", default)]
    pub event_groups: Vec<EventGroup>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NameValue {
    pub name: String,
    #[serde(default)]
    pub language: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OtherName {
    pub name: String,
    #[serde(rename = "type")]
    pub name_type: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Address {
    #[serde(default)]
    pub language: Option<String>,
    #[serde(rename = "addressLines", default)]
    pub address_lines: Vec<String>,
    #[serde(rename = "addressNumber")]
    pub address_number: Option<String>,
    #[serde(rename = "addressNumberWithinBuilding")]
    pub address_number_within_building: Option<String>,
    #[serde(rename = "mailRouting")]
    pub mail_routing: Option<String>,
    pub city: Option<String>,
    pub region: Option<String>,
    pub country: Option<String>,
    #[serde(rename = "postalCode")]
    pub postal_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TypedAddress {
    #[serde(rename = "type")]
    pub address_type: String,
    #[serde(flatten)]
    pub address: Address,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegistrationAuthority {
    pub id: Option<String>,
    pub other: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LegalForm {
    pub id: Option<String>,
    pub other: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SuccessorEntity {
    pub lei: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EventGroup {
    #[serde(rename = "groupType")]
    pub group_type: String,
    #[serde(default)]
    pub events: Vec<EntityEvent>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EntityEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub status: Option<String>,
    #[serde(rename = "effectiveDate")]
    pub effective_date: Option<String>,
    #[serde(rename = "recordedDate")]
    pub recorded_date: Option<String>,
    #[serde(rename = "validationDocuments")]
    pub validation_documents: Option<String>,
    #[serde(rename = "validationReference")]
    pub validation_reference: Option<String>,
    #[serde(rename = "affectedFields", default)]
    pub affected_fields: Vec<AffectedField>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AffectedField {
    pub xpath: Option<String>,
    pub value: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegistrationInfo {
    #[serde(rename = "initialRegistrationDate")]
    pub initial_registration_date: Option<String>,
    #[serde(rename = "lastUpdateDate")]
    pub last_update_date: Option<String>,
    pub status: Option<String>,
    #[serde(rename = "nextRenewalDate")]
    pub next_renewal_date: Option<String>,
    #[serde(rename = "managingLou")]
    pub managing_lou: Option<String>,
    #[serde(rename = "corroborationLevel")]
    pub corroboration_level: Option<String>,
    #[serde(rename = "validatedAt")]
    pub validated_at: Option<RegistrationAuthority>,
    #[serde(rename = "validatedAs")]
    pub validated_as: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LeiRelationships {
    #[serde(rename = "managing-lou", default)]
    pub managing_lou: Option<RelationshipLink>,
    #[serde(rename = "lei-issuer", default)]
    pub lei_issuer: Option<RelationshipLink>,
    #[serde(rename = "direct-parent", default)]
    pub direct_parent: Option<RelationshipLink>,
    #[serde(rename = "ultimate-parent", default)]
    pub ultimate_parent: Option<RelationshipLink>,
    #[serde(rename = "direct-children", default)]
    pub direct_children: Option<RelationshipLink>,
    #[serde(rename = "ultimate-children", default)]
    pub ultimate_children: Option<RelationshipLink>,
    #[serde(rename = "fund-manager", default)]
    pub fund_manager: Option<RelationshipLink>,
    #[serde(rename = "managed-funds", default)]
    pub managed_funds: Option<RelationshipLink>,
    #[serde(rename = "umbrella-fund", default)]
    pub umbrella_fund: Option<RelationshipLink>,
    #[serde(rename = "sub-funds", default)]
    pub sub_funds: Option<RelationshipLink>,
    #[serde(rename = "master-fund", default)]
    pub master_fund: Option<RelationshipLink>,
    #[serde(rename = "feeder-funds", default)]
    pub feeder_funds: Option<RelationshipLink>,
    #[serde(rename = "successor-entities", default)]
    pub successor_entities: Option<RelationshipLink>,
    #[serde(rename = "field-modifications", default)]
    pub field_modifications: Option<RelationshipLink>,
    #[serde(default)]
    pub isins: Option<RelationshipLink>,
    #[serde(default)]
    pub bics: Option<RelationshipLink>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RelationshipLink {
    pub links: RelationshipLinkData,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RelationshipLinkData {
    #[serde(default)]
    pub related: Option<String>,
    #[serde(rename = "relationship-record", default)]
    pub relationship_record: Option<String>,
    #[serde(rename = "relationship-records", default)]
    pub relationship_records: Option<String>,
    #[serde(rename = "reporting-exception", default)]
    pub reporting_exception: Option<String>,
    #[serde(rename = "lei-record", default)]
    pub lei_record: Option<String>,
}

// =============================================================================
// Level 2 Relationship Records
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RelationshipRecord {
    pub id: String,
    #[serde(rename = "type")]
    pub record_type: String,
    pub attributes: RelationshipAttributes,
}

impl RelationshipRecord {
    /// Get relationship type as typed enum (never fails - unknown values captured)
    pub fn relationship_type(&self) -> RelationshipType {
        RelationshipType::parse(&self.attributes.relationship.relationship_type)
    }

    /// Get start node LEI
    pub fn start_lei(&self) -> &str {
        &self.attributes.relationship.start_node.id
    }

    /// Get end node LEI
    pub fn end_lei(&self) -> &str {
        &self.attributes.relationship.end_node.id
    }

    /// Get corroboration level as typed enum
    pub fn corroboration_level(&self) -> CorroborationLevel {
        self.attributes
            .registration
            .corroboration_level
            .as_deref()
            .map(CorroborationLevel::parse)
            .unwrap_or_default()
    }

    /// Check if this is a parent relationship (consolidation)
    pub fn is_parent_relationship(&self) -> bool {
        self.relationship_type().is_parent_relationship()
    }

    /// Check if this is a fund-related relationship
    pub fn is_fund_relationship(&self) -> bool {
        self.relationship_type().is_fund_relationship()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RelationshipAttributes {
    pub relationship: RelationshipDetail,
    pub registration: RelationshipRegistration,
    /// Valid from date
    #[serde(rename = "validFrom", default)]
    pub valid_from: Option<String>,
    /// Valid to date (null if current)
    #[serde(rename = "validTo", default)]
    pub valid_to: Option<String>,
    /// Extension data
    #[serde(default)]
    pub extension: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RelationshipDetail {
    #[serde(rename = "startNode")]
    pub start_node: RelationshipNode,
    #[serde(rename = "endNode")]
    pub end_node: RelationshipNode,
    /// Relationship type - API returns as "type" but we also accept "relationshipType"
    #[serde(alias = "relationshipType", rename = "type")]
    pub relationship_type: String,
    /// Relationship status - API returns as "status" but we also accept "relationshipStatus"
    #[serde(alias = "relationshipStatus", default)]
    pub status: Option<String>,
    /// Periods can be "periods" or "relationshipPeriods"
    #[serde(alias = "relationshipPeriods", default)]
    pub periods: Vec<RelationshipPeriod>,
    #[serde(rename = "relationshipQualifiers", default)]
    pub relationship_qualifiers: Vec<RelationshipQualifier>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RelationshipNode {
    /// Node ID - API returns as "id" but we also accept "nodeID"
    #[serde(alias = "nodeID")]
    pub id: String,
    /// Node type - API returns as "type" but we also accept "nodeIDType"
    #[serde(alias = "nodeIDType", rename = "type")]
    pub node_type: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RelationshipPeriod {
    #[serde(rename = "startDate")]
    pub start_date: Option<String>,
    #[serde(rename = "endDate", default)]
    pub end_date: Option<String>,
    /// Period type - API returns as "type" but we also accept "periodType"
    #[serde(alias = "periodType", rename = "type")]
    pub period_type: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RelationshipQualifier {
    #[serde(rename = "qualifierDimension")]
    pub qualifier_dimension: String,
    #[serde(rename = "qualifierCategory")]
    pub qualifier_category: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RelationshipRegistration {
    #[serde(rename = "initialRegistrationDate", default)]
    pub initial_registration_date: Option<String>,
    #[serde(rename = "lastUpdateDate", default)]
    pub last_update_date: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(rename = "nextRenewalDate", default)]
    pub next_renewal_date: Option<String>,
    #[serde(rename = "managingLou", default)]
    pub managing_lou: Option<String>,
    #[serde(rename = "corroborationLevel", default)]
    pub corroboration_level: Option<String>,
    #[serde(rename = "corroborationDocuments", default)]
    pub corroboration_documents: Option<String>,
    #[serde(rename = "corroborationReference", default)]
    pub corroboration_reference: Option<String>,
    /// Legacy field name aliases
    #[serde(rename = "validationSources", default)]
    pub validation_sources: Option<String>,
    #[serde(rename = "validationDocuments", default)]
    pub validation_documents: Option<String>,
    #[serde(rename = "validationReference", default)]
    pub validation_reference: Option<String>,
}

// =============================================================================
// BIC Mapping
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BicMapping {
    pub id: String,
    #[serde(rename = "type")]
    pub record_type: String,
    pub attributes: BicMappingAttributes,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BicMappingAttributes {
    pub bic: String,
    pub lei: String,
}

// =============================================================================
// Reporting Exceptions (Level 2)
// =============================================================================

/// GLEIF Level 2 Reporting Exception codes
///
/// Uses the resilience pattern: known variants are typed, unknown are captured raw.
/// GLEIF may add new exception codes at any time - we never fail on unknown values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReportingException {
    /// Widely held / publicly traded - no single UBO
    NoKnownPerson,
    /// Human owners without LEI - need BODS lookup
    NaturalPersons,
    /// Parent exists but doesn't consolidate
    NonConsolidating,
    /// Parent exists but has no LEI
    NoLei,
    /// Legal prohibition on disclosure
    BindingLegalRestrictions,
    /// Disclosure would cause harm
    DetrimentNotExcluded,
    /// Commercial sensitivity
    DisclosureDetrimental,
    /// Unknown exception code from GLEIF - captured verbatim for logging/debugging
    /// Never let unknown codes crash the pipeline
    Unknown(String),
}

impl ReportingException {
    /// Parse from GLEIF API string. Never returns None - unknown codes become Unknown(code).
    pub fn parse(s: &str) -> Self {
        match s {
            "NO_KNOWN_PERSON" => Self::NoKnownPerson,
            "NATURAL_PERSONS" => Self::NaturalPersons,
            "NON_CONSOLIDATING" => Self::NonConsolidating,
            "NO_LEI" => Self::NoLei,
            "BINDING_LEGAL_RESTRICTIONS" => Self::BindingLegalRestrictions,
            "DETRIMENT_NOT_EXCLUDED" => Self::DetrimentNotExcluded,
            "DISCLOSURE_DETRIMENTAL" => Self::DisclosureDetrimental,
            other => {
                tracing::warn!(
                    code = other,
                    "Unknown GLEIF reporting exception code - capturing verbatim"
                );
                Self::Unknown(other.to_string())
            }
        }
    }

    /// Convert to string for storage/display
    pub fn as_str(&self) -> &str {
        match self {
            Self::NoKnownPerson => "NO_KNOWN_PERSON",
            Self::NaturalPersons => "NATURAL_PERSONS",
            Self::NonConsolidating => "NON_CONSOLIDATING",
            Self::NoLei => "NO_LEI",
            Self::BindingLegalRestrictions => "BINDING_LEGAL_RESTRICTIONS",
            Self::DetrimentNotExcluded => "DETRIMENT_NOT_EXCLUDED",
            Self::DisclosureDetrimental => "DISCLOSURE_DETRIMENTAL",
            Self::Unknown(code) => code.as_str(),
        }
    }

    /// Returns true if this exception means we should query BODS for UBOs
    pub fn requires_bods_lookup(&self) -> bool {
        matches!(self, Self::NaturalPersons)
    }

    /// Returns true if this exception means the entity is widely held (no single UBO)
    pub fn is_public_float(&self) -> bool {
        matches!(self, Self::NoKnownPerson)
    }

    /// Returns true if this is an unknown/unrecognized exception code
    pub fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown(_))
    }
}

// =============================================================================
// Enrichment Result Types
// =============================================================================

/// Result of enriching an entity from GLEIF
#[derive(Debug, Clone)]
pub struct EnrichmentResult {
    pub entity_id: uuid::Uuid,
    pub lei: String,
    pub names_added: i32,
    pub addresses_added: i32,
    pub identifiers_added: i32,
    pub parent_relationships_added: i32,
    /// Fund relationships: fund_manager, umbrella_fund, master_fund
    pub fund_relationships_added: i32,
    pub events_added: i32,
    pub direct_parent_exception: Option<ReportingException>,
    pub ultimate_parent_exception: Option<ReportingException>,
}

/// Result of importing a corporate tree
#[derive(Debug, Clone)]
pub struct TreeImportResult {
    pub root_lei: String,
    pub entities_created: i32,
    pub entities_updated: i32,
    pub relationships_created: i32,
    pub terminal_entities: Vec<TerminalEntity>,
}

/// An entity at the end of an ownership chain
#[derive(Debug, Clone)]
pub struct TerminalEntity {
    pub lei: String,
    pub name: String,
    pub exception: Option<ReportingException>,
}

// =============================================================================
// UBO and Ownership Chain Types
// =============================================================================

/// UBO terminus status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum UboStatus {
    /// Publicly traded - no single UBO
    PublicFloat,
    /// Government/state owned
    StateOwned,
    /// Natural person at terminus
    NaturalPerson { name: String },
    /// Regulated entity with no public UBO
    RegulatedEntity,
    /// Unknown/not determined
    Unknown,
}

impl UboStatus {
    pub fn from_exception(exception: Option<&str>) -> Self {
        match exception {
            Some("NO_KNOWN_PERSON") => Self::PublicFloat,
            Some("NON_CONSOLIDATING") => Self::RegulatedEntity,
            Some("NATURAL_PERSONS") => Self::NaturalPerson {
                name: "Unknown".to_string(),
            },
            _ => Self::Unknown,
        }
    }
}

/// Discovered entity from GLEIF
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredEntity {
    pub lei: String,
    pub name: String,
    pub jurisdiction: Option<String>,
    pub category: Option<String>,
    pub status: Option<String>,
    pub direct_parent_lei: Option<String>,
    pub ultimate_parent_lei: Option<String>,
    pub legal_form_id: Option<String>,
}

impl DiscoveredEntity {
    pub fn from_lei_record(record: &LeiRecord) -> Self {
        Self {
            lei: record.lei().to_string(),
            name: record.attributes.entity.legal_name.name.clone(),
            jurisdiction: record.attributes.entity.jurisdiction.clone(),
            category: record.attributes.entity.category.clone(),
            status: record.attributes.entity.status.clone(),
            direct_parent_lei: None,
            ultimate_parent_lei: None,
            legal_form_id: record
                .attributes
                .entity
                .legal_form
                .as_ref()
                .and_then(|lf| lf.id.clone()),
        }
    }
}

/// Ownership chain trace result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnershipChain {
    pub start_lei: String,
    pub start_name: String,
    pub chain: Vec<ChainLink>,
    pub terminus: UboStatus,
    pub total_depth: usize,
}

/// Single link in ownership chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainLink {
    pub lei: String,
    pub name: String,
    pub jurisdiction: Option<String>,
    pub relationship_type: String,
    pub corroboration_level: Option<String>,
}

/// Result of managed funds query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundListResult {
    pub manager_lei: String,
    pub manager_name: Option<String>,
    pub funds: Vec<DiscoveredEntity>,
    pub fund_umbrellas: std::collections::HashMap<String, DiscoveredEntity>,
    pub total_count: usize,
}

/// Result of successor resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessorResult {
    pub original_lei: String,
    pub current_lei: String,
    pub chain: Vec<String>,
    pub current_entity: DiscoveredEntity,
    pub was_merged: bool,
}

// =============================================================================
// Fund Structure Relationship Results
// =============================================================================

/// Result of umbrella fund lookup (IS_SUBFUND_OF)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UmbrellaResult {
    pub subfund_lei: String,
    pub subfund_name: String,
    pub umbrella: Option<UmbrellaEntity>,
}

/// Umbrella fund entity info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UmbrellaEntity {
    pub lei: String,
    pub name: String,
    pub jurisdiction: Option<String>,
}

/// Result of fund manager lookup (IS_FUND-MANAGED_BY)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagerResult {
    pub fund_lei: String,
    pub fund_name: String,
    pub manager: Option<ManagerEntity>,
}

/// Fund manager entity info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagerEntity {
    pub lei: String,
    pub name: String,
    pub jurisdiction: Option<String>,
    pub role: String,
}

/// Result of master fund lookup (IS_FEEDER_TO)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterFundResult {
    pub feeder_lei: String,
    pub feeder_name: String,
    pub master: Option<MasterEntity>,
}

/// Master fund entity info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterEntity {
    pub lei: String,
    pub name: String,
    pub jurisdiction: Option<String>,
}

/// Result of ISIN to LEI lookup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsinLookupResult {
    pub isin: String,
    pub lei: String,
    pub name: String,
    pub jurisdiction: Option<String>,
}

// =============================================================================
// Tests for Hardened Enum Types
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_category_known_values() {
        assert_eq!(EntityCategory::parse("FUND"), EntityCategory::Fund);
        assert_eq!(EntityCategory::parse("GENERAL"), EntityCategory::General);
        assert_eq!(EntityCategory::parse("BRANCH"), EntityCategory::Branch);
        assert_eq!(
            EntityCategory::parse("SOLE_PROPRIETOR"),
            EntityCategory::SoleProprietor
        );

        // Case insensitive
        assert_eq!(EntityCategory::parse("fund"), EntityCategory::Fund);
        assert_eq!(EntityCategory::parse("Fund"), EntityCategory::Fund);
    }

    #[test]
    fn test_entity_category_unknown_captured() {
        let unknown = EntityCategory::parse("NEW_CATEGORY_2025");
        assert!(unknown.is_unknown());
        assert_eq!(unknown.as_str(), "NEW_CATEGORY_2025");

        // Verify it doesn't crash
        assert!(!unknown.is_fund());
    }

    #[test]
    fn test_entity_status_known_values() {
        assert_eq!(EntityStatus::parse("ACTIVE"), EntityStatus::Active);
        assert_eq!(EntityStatus::parse("INACTIVE"), EntityStatus::Inactive);
        assert!(EntityStatus::parse("ACTIVE").is_active());
        assert!(!EntityStatus::parse("INACTIVE").is_active());
    }

    #[test]
    fn test_entity_status_unknown_captured() {
        let unknown = EntityStatus::parse("PENDING_REVIEW");
        assert!(unknown.is_unknown());
        assert_eq!(unknown.as_str(), "PENDING_REVIEW");
        assert!(!unknown.is_active());
    }

    #[test]
    fn test_registration_status_known_values() {
        assert_eq!(
            RegistrationStatus::parse("ISSUED"),
            RegistrationStatus::Issued
        );
        assert_eq!(
            RegistrationStatus::parse("LAPSED"),
            RegistrationStatus::Lapsed
        );
        assert_eq!(
            RegistrationStatus::parse("MERGED"),
            RegistrationStatus::Merged
        );
        assert!(RegistrationStatus::parse("ISSUED").is_valid());
        assert!(!RegistrationStatus::parse("LAPSED").is_valid());
    }

    #[test]
    fn test_registration_status_unknown_captured() {
        let unknown = RegistrationStatus::parse("SUSPENDED_2025");
        assert!(unknown.is_unknown());
        assert_eq!(unknown.as_str(), "SUSPENDED_2025");
        assert!(!unknown.is_valid());
    }

    #[test]
    fn test_corroboration_level_known_values() {
        assert_eq!(
            CorroborationLevel::parse("FULLY_CORROBORATED"),
            CorroborationLevel::FullyCorroborated
        );
        assert_eq!(
            CorroborationLevel::parse("PARTIALLY_CORROBORATED"),
            CorroborationLevel::PartiallyCorroborated
        );
        assert!(CorroborationLevel::parse("FULLY_CORROBORATED").is_fully_corroborated());
    }

    #[test]
    fn test_corroboration_level_unknown_captured() {
        let unknown = CorroborationLevel::parse("SELF_ATTESTED");
        assert!(unknown.is_unknown());
        assert_eq!(unknown.as_str(), "SELF_ATTESTED");
        assert!(!unknown.is_fully_corroborated());
    }

    #[test]
    fn test_relationship_type_known_values() {
        assert_eq!(
            RelationshipType::parse("IS_DIRECTLY_CONSOLIDATED_BY"),
            RelationshipType::IsDirectlyConsolidatedBy
        );
        assert_eq!(
            RelationshipType::parse("IS_FUND-MANAGED_BY"),
            RelationshipType::IsFundManagedBy
        );
        assert_eq!(
            RelationshipType::parse("IS_SUBFUND_OF"),
            RelationshipType::IsSubfundOf
        );

        assert!(RelationshipType::parse("IS_DIRECTLY_CONSOLIDATED_BY").is_parent_relationship());
        assert!(RelationshipType::parse("IS_FUND-MANAGED_BY").is_fund_relationship());
    }

    #[test]
    fn test_relationship_type_unknown_captured() {
        let unknown = RelationshipType::parse("IS_SPONSORED_BY");
        assert!(unknown.is_unknown());
        assert_eq!(unknown.as_str(), "IS_SPONSORED_BY");
        assert!(!unknown.is_parent_relationship());
        assert!(!unknown.is_fund_relationship());
    }

    #[test]
    fn test_reporting_exception_known_values() {
        assert_eq!(
            ReportingException::parse("NO_KNOWN_PERSON"),
            ReportingException::NoKnownPerson
        );
        assert_eq!(
            ReportingException::parse("NATURAL_PERSONS"),
            ReportingException::NaturalPersons
        );
        assert!(ReportingException::parse("NO_KNOWN_PERSON").is_public_float());
        assert!(ReportingException::parse("NATURAL_PERSONS").requires_bods_lookup());
    }

    #[test]
    fn test_reporting_exception_unknown_captured() {
        let unknown = ReportingException::parse("REGULATORY_RESTRICTION_2025");
        assert!(unknown.is_unknown());
        assert_eq!(unknown.as_str(), "REGULATORY_RESTRICTION_2025");
        assert!(!unknown.is_public_float());
        assert!(!unknown.requires_bods_lookup());
    }

    #[test]
    fn test_defaults_are_unknown() {
        // Verify defaults don't crash and are marked as unknown
        assert!(EntityCategory::default().is_unknown());
        assert!(EntityStatus::default().is_unknown());
        assert!(RegistrationStatus::default().is_unknown());
        assert!(CorroborationLevel::default().is_unknown());
        assert!(RelationshipType::default().is_unknown());
    }
}
