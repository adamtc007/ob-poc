//! Normalized structures for source-agnostic data representation
//!
//! These structures represent the common denominator across all external sources
//! and map directly to our database schema.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Normalized entity from any source
///
/// Maps to: `"ob-poc".entities`, `entity_limited_companies`, `entity_natural_persons`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedEntity {
    // === Source identification ===
    /// Source-specific key (LEI, company number, CIK, etc.)
    pub source_key: String,
    /// Source name (GLEIF, Companies House, SEC EDGAR, etc.)
    pub source_name: String,

    // === Required fields ===
    /// Primary entity name
    pub name: String,

    // === Optional identifiers ===
    /// Legal Entity Identifier (20 alphanumeric)
    pub lei: Option<String>,
    /// Company registration number
    pub registration_number: Option<String>,
    /// Tax identification number
    pub tax_id: Option<String>,

    // === Classification ===
    /// Entity type (company, natural person, etc.)
    pub entity_type: Option<EntityType>,
    /// Jurisdiction (ISO 3166-1 alpha-2)
    pub jurisdiction: Option<String>,
    /// Entity status
    pub status: Option<EntityStatus>,

    // === Dates ===
    /// Date of incorporation/formation
    pub incorporated_date: Option<NaiveDate>,
    /// Date of dissolution (if dissolved)
    pub dissolved_date: Option<NaiveDate>,

    // === Addresses ===
    /// Registered/legal address
    pub registered_address: Option<NormalizedAddress>,
    /// Business/headquarters address
    pub business_address: Option<NormalizedAddress>,

    // === Audit ===
    /// Raw API response for audit trail
    pub raw_response: Option<serde_json::Value>,
}

impl NormalizedEntity {
    /// Create a minimal entity with just the required fields
    pub fn new(source_key: String, source_name: String, name: String) -> Self {
        Self {
            source_key,
            source_name,
            name,
            lei: None,
            registration_number: None,
            tax_id: None,
            entity_type: None,
            jurisdiction: None,
            status: None,
            incorporated_date: None,
            dissolved_date: None,
            registered_address: None,
            business_address: None,
            raw_response: None,
        }
    }

    /// Check if this entity is active
    pub fn is_active(&self) -> bool {
        self.status
            .as_ref()
            .is_none_or(|s| matches!(s, EntityStatus::Active))
    }
}

/// Entity type classification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityType {
    /// Limited company / corporation
    LimitedCompany,
    /// Public limited company
    PublicCompany,
    /// Limited liability partnership
    Llp,
    /// General partnership
    Partnership,
    /// Sole proprietor
    SoleProprietor,
    /// Trust
    Trust,
    /// Fund / investment vehicle
    Fund,
    /// Natural person
    NaturalPerson,
    /// Government entity
    Government,
    /// Branch of foreign company
    Branch,
    /// Unknown - captured verbatim from source
    Unknown(String),
}

impl std::fmt::Display for EntityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LimitedCompany => write!(f, "limited_company"),
            Self::PublicCompany => write!(f, "public_company"),
            Self::Llp => write!(f, "llp"),
            Self::Partnership => write!(f, "partnership"),
            Self::SoleProprietor => write!(f, "sole_proprietor"),
            Self::Trust => write!(f, "trust"),
            Self::Fund => write!(f, "fund"),
            Self::NaturalPerson => write!(f, "natural_person"),
            Self::Government => write!(f, "government"),
            Self::Branch => write!(f, "branch"),
            Self::Unknown(s) => write!(f, "{}", s),
        }
    }
}

/// Entity status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityStatus {
    Active,
    Inactive,
    Dissolved,
    Liquidation,
    Merged,
    /// Unknown - captured verbatim from source
    Unknown(String),
}

impl std::fmt::Display for EntityStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Inactive => write!(f, "inactive"),
            Self::Dissolved => write!(f, "dissolved"),
            Self::Liquidation => write!(f, "liquidation"),
            Self::Merged => write!(f, "merged"),
            Self::Unknown(s) => write!(f, "{}", s),
        }
    }
}

/// Normalized address
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedAddress {
    /// Address lines (street, building, etc.)
    pub lines: Vec<String>,
    /// City/locality
    pub city: Option<String>,
    /// Region/state/province
    pub region: Option<String>,
    /// Postal code
    pub postal_code: Option<String>,
    /// Country (ISO 3166-1 alpha-2)
    pub country: Option<String>,
}

impl NormalizedAddress {
    /// Format as a single-line address
    pub fn to_single_line(&self) -> String {
        let mut parts = self.lines.clone();
        if let Some(ref city) = self.city {
            parts.push(city.clone());
        }
        if let Some(ref region) = self.region {
            parts.push(region.clone());
        }
        if let Some(ref postal) = self.postal_code {
            parts.push(postal.clone());
        }
        if let Some(ref country) = self.country {
            parts.push(country.clone());
        }
        parts.join(", ")
    }
}

/// Normalized control holder (PSC, 13D/G filer, shareholder, etc.)
///
/// Maps to: `kyc.control_relationships`, `kyc.holdings`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedControlHolder {
    // === Identity ===
    /// Holder name (individual or corporate)
    pub holder_name: String,
    /// Holder type
    pub holder_type: HolderType,

    // === Corporate holder fields ===
    /// Registration number (for corporate holders)
    pub registration_number: Option<String>,
    /// Jurisdiction (for corporate holders)
    pub jurisdiction: Option<String>,
    /// LEI (for corporate holders)
    pub lei: Option<String>,

    // === Individual holder fields ===
    /// Nationality (for individuals)
    pub nationality: Option<String>,
    /// Country of residence (for individuals)
    pub country_of_residence: Option<String>,
    /// Partial date of birth ("YYYY-MM" or "YYYY")
    pub date_of_birth_partial: Option<String>,

    // === Control details ===
    /// Lower bound of ownership percentage (if range)
    pub ownership_pct_low: Option<Decimal>,
    /// Upper bound of ownership percentage (if range)
    pub ownership_pct_high: Option<Decimal>,
    /// Exact ownership percentage (if known)
    pub ownership_pct_exact: Option<Decimal>,
    /// Voting percentage
    pub voting_pct: Option<Decimal>,

    // === Control rights ===
    /// Has voting rights
    pub has_voting_rights: bool,
    /// Has rights to appoint/remove directors
    pub has_appointment_rights: bool,
    /// Has veto rights
    pub has_veto_rights: bool,
    /// Nature of control descriptions from source
    pub natures_of_control: Vec<String>,

    // === Timing ===
    /// Date control was notified/registered
    pub notified_on: Option<NaiveDate>,
    /// Date control ceased (None if still active)
    pub ceased_on: Option<NaiveDate>,

    // === Source ===
    /// Source document reference (filing number, etc.)
    pub source_document: Option<String>,
}

impl NormalizedControlHolder {
    /// Check if this control relationship is still active
    pub fn is_active(&self) -> bool {
        self.ceased_on.is_none()
    }

    /// Get the best ownership percentage estimate
    pub fn ownership_pct_best(&self) -> Option<Decimal> {
        self.ownership_pct_exact.or_else(|| {
            // Use midpoint of range if available
            match (self.ownership_pct_low, self.ownership_pct_high) {
                (Some(low), Some(high)) => Some((low + high) / Decimal::from(2)),
                (Some(low), None) => Some(low),
                (None, Some(high)) => Some(high),
                (None, None) => None,
            }
        })
    }
}

/// Control holder type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HolderType {
    /// Natural person
    Individual,
    /// Corporate entity
    Corporate,
    /// Trust
    Trust,
    /// Partnership
    Partnership,
    /// Government entity
    Government,
    /// Nominee/custodian
    Nominee,
    /// Unknown
    Unknown,
}

impl std::fmt::Display for HolderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Individual => write!(f, "individual"),
            Self::Corporate => write!(f, "corporate"),
            Self::Trust => write!(f, "trust"),
            Self::Partnership => write!(f, "partnership"),
            Self::Government => write!(f, "government"),
            Self::Nominee => write!(f, "nominee"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Normalized officer/director
///
/// Maps to: `kyc.officers`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedOfficer {
    /// Officer name
    pub name: String,
    /// Role
    pub role: OfficerRole,
    /// Date appointed
    pub appointed_date: Option<NaiveDate>,
    /// Date resigned (None if still active)
    pub resigned_date: Option<NaiveDate>,
    /// Nationality
    pub nationality: Option<String>,
    /// Country of residence
    pub country_of_residence: Option<String>,
    /// Partial date of birth ("YYYY-MM" or "YYYY")
    pub date_of_birth_partial: Option<String>,
    /// Occupation
    pub occupation: Option<String>,
}

impl NormalizedOfficer {
    /// Check if this officer is still active
    pub fn is_active(&self) -> bool {
        self.resigned_date.is_none()
    }
}

/// Officer role
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OfficerRole {
    Director,
    Secretary,
    Chairman,
    Ceo,
    Cfo,
    Coo,
    NonExecutiveDirector,
    AlternateDirector,
    Manager,
    Partner,
    Other(String),
}

impl std::fmt::Display for OfficerRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Director => write!(f, "director"),
            Self::Secretary => write!(f, "secretary"),
            Self::Chairman => write!(f, "chairman"),
            Self::Ceo => write!(f, "ceo"),
            Self::Cfo => write!(f, "cfo"),
            Self::Coo => write!(f, "coo"),
            Self::NonExecutiveDirector => write!(f, "non_executive_director"),
            Self::AlternateDirector => write!(f, "alternate_director"),
            Self::Manager => write!(f, "manager"),
            Self::Partner => write!(f, "partner"),
            Self::Other(s) => write!(f, "{}", s),
        }
    }
}

impl OfficerRole {
    /// Parse role from string (case-insensitive)
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "director" => Self::Director,
            "secretary" | "company secretary" => Self::Secretary,
            "chairman" | "chair" => Self::Chairman,
            "ceo" | "chief executive officer" => Self::Ceo,
            "cfo" | "chief financial officer" => Self::Cfo,
            "coo" | "chief operating officer" => Self::Coo,
            "non-executive director" | "ned" => Self::NonExecutiveDirector,
            "alternate director" => Self::AlternateDirector,
            "manager" | "llp designated member" => Self::Manager,
            "partner" | "general partner" | "limited partner" => Self::Partner,
            other => Self::Other(other.to_string()),
        }
    }
}

/// Normalized parent/subsidiary relationship
///
/// Maps to: `kyc.ownership_edges`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedRelationship {
    /// Parent entity key (source-specific)
    pub parent_key: String,
    /// Parent entity name
    pub parent_name: String,
    /// Child entity key (source-specific)
    pub child_key: String,
    /// Child entity name
    pub child_name: String,
    /// Type of relationship
    pub relationship_type: RelationshipType,
    /// Ownership percentage (if known)
    pub ownership_pct: Option<Decimal>,
    /// Whether this is a direct relationship
    pub is_direct: bool,
}

/// Relationship type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationshipType {
    /// Direct parent (immediate owner)
    DirectParent,
    /// Ultimate parent (top of chain)
    UltimateParent,
    /// Subsidiary
    Subsidiary,
    /// Branch of parent
    BranchOf,
    /// Fund managed by
    FundManagedBy,
    /// Sub-fund of umbrella
    SubfundOf,
}

impl std::fmt::Display for RelationshipType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DirectParent => write!(f, "direct_parent"),
            Self::UltimateParent => write!(f, "ultimate_parent"),
            Self::Subsidiary => write!(f, "subsidiary"),
            Self::BranchOf => write!(f, "branch_of"),
            Self::FundManagedBy => write!(f, "fund_managed_by"),
            Self::SubfundOf => write!(f, "subfund_of"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalized_entity_is_active() {
        let mut entity =
            NormalizedEntity::new("12345678".into(), "test".into(), "Test Company".into());

        // No status = active
        assert!(entity.is_active());

        // Active status
        entity.status = Some(EntityStatus::Active);
        assert!(entity.is_active());

        // Dissolved = not active
        entity.status = Some(EntityStatus::Dissolved);
        assert!(!entity.is_active());
    }

    #[test]
    fn test_control_holder_ownership_pct_best() {
        let mut holder = NormalizedControlHolder {
            holder_name: "Test".into(),
            holder_type: HolderType::Corporate,
            registration_number: None,
            jurisdiction: None,
            lei: None,
            nationality: None,
            country_of_residence: None,
            date_of_birth_partial: None,
            ownership_pct_low: None,
            ownership_pct_high: None,
            ownership_pct_exact: None,
            voting_pct: None,
            has_voting_rights: false,
            has_appointment_rights: false,
            has_veto_rights: false,
            natures_of_control: vec![],
            notified_on: None,
            ceased_on: None,
            source_document: None,
        };

        // No percentages
        assert_eq!(holder.ownership_pct_best(), None);

        // Exact takes precedence
        holder.ownership_pct_exact = Some(Decimal::from(50));
        holder.ownership_pct_low = Some(Decimal::from(25));
        holder.ownership_pct_high = Some(Decimal::from(75));
        assert_eq!(holder.ownership_pct_best(), Some(Decimal::from(50)));

        // Range midpoint
        holder.ownership_pct_exact = None;
        assert_eq!(holder.ownership_pct_best(), Some(Decimal::from(50)));

        // Only low bound
        holder.ownership_pct_high = None;
        assert_eq!(holder.ownership_pct_best(), Some(Decimal::from(25)));
    }

    #[test]
    fn test_officer_role_parse() {
        assert_eq!(OfficerRole::parse("director"), OfficerRole::Director);
        assert_eq!(OfficerRole::parse("DIRECTOR"), OfficerRole::Director);
        assert_eq!(OfficerRole::parse("Secretary"), OfficerRole::Secretary);
        assert_eq!(OfficerRole::parse("CEO"), OfficerRole::Ceo);
        assert_eq!(
            OfficerRole::parse("Chief Executive Officer"),
            OfficerRole::Ceo
        );
        assert_eq!(
            OfficerRole::parse("custom role"),
            OfficerRole::Other("custom role".into())
        );
    }
}
