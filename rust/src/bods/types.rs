//! Beneficial Ownership Data Standard (BODS) types
//! Based on BODS v0.4 schema

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ============================================================================
// BODS API Response Types
// ============================================================================

/// BODS Entity Statement
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EntityStatement {
    #[serde(rename = "statementId")]
    pub statement_id: String,

    #[serde(rename = "statementType")]
    pub statement_type: String, // "entityStatement"

    #[serde(rename = "entityType")]
    pub entity_type: EntityType,

    pub name: Option<String>,

    #[serde(default)]
    pub identifiers: Vec<Identifier>,

    #[serde(rename = "incorporatedInJurisdiction")]
    pub jurisdiction: Option<Jurisdiction>,

    #[serde(default)]
    pub addresses: Vec<Address>,

    #[serde(rename = "statementDate")]
    pub statement_date: Option<String>,

    pub source: Option<Source>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EntityType {
    RegisteredEntity,
    LegalEntity,
    Arrangement,
    AnonymousEntity,
    UnknownEntity,
    State,
    StateBody,
}

impl EntityType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EntityType::RegisteredEntity => "registeredEntity",
            EntityType::LegalEntity => "legalEntity",
            EntityType::Arrangement => "arrangement",
            EntityType::AnonymousEntity => "anonymousEntity",
            EntityType::UnknownEntity => "unknownEntity",
            EntityType::State => "state",
            EntityType::StateBody => "stateBody",
        }
    }
}

/// BODS Person Statement
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PersonStatement {
    #[serde(rename = "statementId")]
    pub statement_id: String,

    #[serde(rename = "statementType")]
    pub statement_type: String, // "personStatement"

    #[serde(rename = "personType")]
    pub person_type: PersonType,

    #[serde(default)]
    pub names: Vec<Name>,

    #[serde(default)]
    pub identifiers: Vec<Identifier>,

    #[serde(default)]
    pub nationalities: Vec<Jurisdiction>,

    #[serde(rename = "birthDate")]
    pub birth_date: Option<String>,

    #[serde(default)]
    pub addresses: Vec<Address>,

    #[serde(rename = "statementDate")]
    pub statement_date: Option<String>,

    pub source: Option<Source>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PersonType {
    KnownPerson,
    AnonymousPerson,
    UnknownPerson,
}

impl PersonType {
    pub fn as_str(&self) -> &'static str {
        match self {
            PersonType::KnownPerson => "knownPerson",
            PersonType::AnonymousPerson => "anonymousPerson",
            PersonType::UnknownPerson => "unknownPerson",
        }
    }
}

/// BODS Ownership or Control Statement
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OwnershipStatement {
    #[serde(rename = "statementId")]
    pub statement_id: String,

    #[serde(rename = "statementType")]
    pub statement_type: String, // "ownershipOrControlStatement"

    pub subject: Subject,

    #[serde(rename = "interestedParty")]
    pub interested_party: InterestedParty,

    #[serde(default)]
    pub interests: Vec<Interest>,

    #[serde(rename = "statementDate")]
    pub statement_date: Option<String>,

    pub source: Option<Source>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Subject {
    #[serde(rename = "describedByEntityStatement")]
    pub entity_statement_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InterestedParty {
    #[serde(rename = "describedByEntityStatement")]
    pub entity_statement_id: Option<String>,

    #[serde(rename = "describedByPersonStatement")]
    pub person_statement_id: Option<String>,

    pub unspecified: Option<UnspecifiedParty>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UnspecifiedParty {
    pub reason: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Interest {
    #[serde(rename = "type")]
    pub interest_type: Option<String>,

    #[serde(rename = "interestLevel")]
    pub interest_level: Option<String>, // "direct", "indirect", "unknown"

    pub share: Option<Share>,

    #[serde(rename = "startDate")]
    pub start_date: Option<String>,

    #[serde(rename = "endDate")]
    pub end_date: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Share {
    pub exact: Option<f64>,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    #[serde(rename = "exclusiveMinimum")]
    pub exclusive_minimum: Option<bool>,
    #[serde(rename = "exclusiveMaximum")]
    pub exclusive_maximum: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Identifier {
    pub scheme: Option<String>,
    pub id: String,
    pub uri: Option<String>,

    #[serde(rename = "schemeName")]
    pub scheme_name: Option<String>,
}

impl Identifier {
    /// Extract LEI from identifier if present
    pub fn as_lei(&self) -> Option<&str> {
        match self.scheme.as_deref() {
            Some("XI-LEI") | Some("LEI") => Some(&self.id),
            _ => None,
        }
    }

    /// Extract company registration number
    pub fn as_company_number(&self) -> Option<&str> {
        match self.scheme.as_deref() {
            Some(s) if s.starts_with("GB-COH") => Some(&self.id), // UK Companies House
            Some(s) if s.starts_with("DK-CVR") => Some(&self.id), // Denmark
            Some(s) if s.starts_with("SK-") => Some(&self.id),    // Slovakia
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Jurisdiction {
    pub code: Option<String>, // ISO 3166-1/2
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Name {
    #[serde(rename = "type")]
    pub name_type: Option<String>, // "individual", "transliteration", etc.

    #[serde(rename = "fullName")]
    pub full_name: Option<String>,

    #[serde(rename = "givenName")]
    pub given_name: Option<String>,

    #[serde(rename = "familyName")]
    pub family_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Address {
    #[serde(rename = "type")]
    pub address_type: Option<String>,
    pub address: Option<String>,
    pub country: Option<String>,
    #[serde(rename = "postCode")]
    pub post_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Source {
    #[serde(rename = "type")]
    pub source_type: Option<Vec<String>>,
    pub description: Option<String>,
    pub url: Option<String>,
    #[serde(rename = "retrievedAt")]
    pub retrieved_at: Option<String>,
}

// ============================================================================
// Database Row Types
// ============================================================================

/// Database row for bods_entity_statements
#[derive(Debug, Clone, FromRow)]
pub struct BodsEntityRow {
    pub statement_id: String,
    pub entity_type: Option<String>,
    pub name: Option<String>,
    pub jurisdiction: Option<String>,
    pub lei: Option<String>,
    pub company_number: Option<String>,
    pub opencorporates_id: Option<String>,
    pub identifiers: Option<serde_json::Value>,
    pub source_register: Option<String>,
    pub statement_date: Option<NaiveDate>,
    pub source_url: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Database row for bods_person_statements
#[derive(Debug, Clone, FromRow)]
pub struct BodsPersonRow {
    pub statement_id: String,
    pub person_type: Option<String>,
    pub full_name: Option<String>,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub names: Option<serde_json::Value>,
    pub birth_date: Option<NaiveDate>,
    pub birth_date_precision: Option<String>,
    pub death_date: Option<NaiveDate>,
    pub nationalities: Option<Vec<String>>,
    pub country_of_residence: Option<String>,
    pub addresses: Option<serde_json::Value>,
    pub tax_residencies: Option<Vec<String>>,
    pub source_register: Option<String>,
    pub statement_date: Option<NaiveDate>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Database row for bods_ownership_statements
#[derive(Debug, Clone, FromRow)]
pub struct BodsOwnershipRow {
    pub statement_id: String,
    pub subject_entity_statement_id: Option<String>,
    pub subject_lei: Option<String>,
    pub subject_name: Option<String>,
    pub interested_party_type: Option<String>,
    pub interested_party_statement_id: Option<String>,
    pub interested_party_name: Option<String>,
    pub ownership_type: Option<String>,
    pub share_min: Option<rust_decimal::Decimal>,
    pub share_max: Option<rust_decimal::Decimal>,
    pub share_exact: Option<rust_decimal::Decimal>,
    pub is_direct: Option<bool>,
    pub control_types: Option<Vec<String>>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub source_register: Option<String>,
    pub statement_date: Option<NaiveDate>,
    pub source_description: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Database row for entity_bods_links
#[derive(Debug, Clone, FromRow)]
pub struct EntityBodsLinkRow {
    pub link_id: Uuid,
    pub entity_id: Uuid,
    pub bods_entity_statement_id: Option<String>,
    pub match_method: Option<String>,
    pub match_confidence: Option<rust_decimal::Decimal>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Database row for entity_ubos
#[derive(Debug, Clone, FromRow)]
pub struct EntityUboRow {
    pub ubo_id: Uuid,
    pub entity_id: Uuid,
    pub person_statement_id: Option<String>,
    pub person_name: Option<String>,
    pub nationalities: Option<Vec<String>>,
    pub country_of_residence: Option<String>,
    pub ownership_chain: Option<serde_json::Value>,
    pub chain_depth: Option<i32>,
    pub ownership_min: Option<rust_decimal::Decimal>,
    pub ownership_max: Option<rust_decimal::Decimal>,
    pub ownership_exact: Option<rust_decimal::Decimal>,
    pub control_types: Option<Vec<String>>,
    pub is_direct: Option<bool>,
    pub ubo_type: Option<String>,
    pub confidence_level: Option<String>,
    pub source: Option<String>,
    pub source_register: Option<String>,
    pub discovered_at: Option<chrono::DateTime<chrono::Utc>>,
    pub verified_at: Option<chrono::DateTime<chrono::Utc>>,
    pub verified_by: Option<String>,
}

// ============================================================================
// UBO Discovery Result Types
// ============================================================================

/// Result of UBO discovery for an entity
#[derive(Debug, Clone, Serialize)]
pub struct UboDiscoveryResult {
    /// The entity we discovered UBOs for
    pub entity_id: Uuid,
    pub entity_lei: Option<String>,

    /// Discovered UBOs
    pub ubos: Vec<DiscoveredUbo>,

    /// Whether discovery is complete
    pub is_complete: bool,

    /// Any gaps or issues
    pub gaps: Vec<String>,

    /// Source registers queried
    pub sources_queried: Vec<String>,
}

/// A discovered UBO
#[derive(Debug, Clone, Serialize)]
pub struct DiscoveredUbo {
    /// BODS person statement ID if from BODS
    pub person_statement_id: Option<String>,

    /// Person details
    pub name: String,
    pub nationalities: Vec<String>,
    pub country_of_residence: Option<String>,
    pub birth_date: Option<NaiveDate>,

    /// Ownership details
    pub ownership_percentage: Option<f64>,
    pub ownership_min: Option<f64>,
    pub ownership_max: Option<f64>,
    pub is_direct: bool,

    /// Control types if control-based UBO
    pub control_types: Vec<String>,

    /// Chain of ownership from entity to this UBO
    pub ownership_chain: Vec<ChainLink>,

    /// UBO type classification
    pub ubo_type: UboType,

    /// Confidence in this discovery
    pub confidence: f64,

    /// Source of discovery
    pub source: String,
}

/// A link in the ownership chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainLink {
    pub entity_name: String,
    pub entity_lei: Option<String>,
    pub ownership_percentage: Option<f64>,
    pub relationship_type: String,
}

/// UBO type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UboType {
    NaturalPerson,
    PublicFloat,
    StateOwned,
    WidelyHeld,
    Unknown,
    Exempt,
}

impl UboType {
    pub fn as_str(&self) -> &'static str {
        match self {
            UboType::NaturalPerson => "NATURAL_PERSON",
            UboType::PublicFloat => "PUBLIC_FLOAT",
            UboType::StateOwned => "STATE_OWNED",
            UboType::WidelyHeld => "WIDELY_HELD",
            UboType::Unknown => "UNKNOWN",
            UboType::Exempt => "EXEMPT",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "NATURAL_PERSON" => UboType::NaturalPerson,
            "PUBLIC_FLOAT" => UboType::PublicFloat,
            "STATE_OWNED" => UboType::StateOwned,
            "WIDELY_HELD" => UboType::WidelyHeld,
            "EXEMPT" => UboType::Exempt,
            _ => UboType::Unknown,
        }
    }
}
