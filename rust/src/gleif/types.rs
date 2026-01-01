//! GLEIF API response types
//! Complete mapping of Level 1 and Level 2 data structures
//!
//! Reference: https://api.gleif.org/api/v1/lei-records

use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RelationshipAttributes {
    pub relationship: RelationshipDetail,
    pub registration: RelationshipRegistration,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RelationshipDetail {
    #[serde(rename = "startNode")]
    pub start_node: RelationshipNode,
    #[serde(rename = "endNode")]
    pub end_node: RelationshipNode,
    #[serde(rename = "relationshipType")]
    pub relationship_type: String,
    #[serde(rename = "relationshipPeriods", default)]
    pub relationship_periods: Vec<RelationshipPeriod>,
    #[serde(rename = "relationshipStatus")]
    pub relationship_status: Option<String>,
    #[serde(rename = "relationshipQualifiers", default)]
    pub relationship_qualifiers: Vec<RelationshipQualifier>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RelationshipNode {
    #[serde(rename = "nodeID")]
    pub node_id: String,
    #[serde(rename = "nodeIDType")]
    pub node_id_type: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RelationshipPeriod {
    #[serde(rename = "startDate")]
    pub start_date: Option<String>,
    #[serde(rename = "endDate")]
    pub end_date: Option<String>,
    #[serde(rename = "periodType")]
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
    #[serde(rename = "initialRegistrationDate")]
    pub initial_registration_date: Option<String>,
    #[serde(rename = "lastUpdateDate")]
    pub last_update_date: Option<String>,
    pub status: Option<String>,
    #[serde(rename = "validationSources")]
    pub validation_sources: Option<String>,
    #[serde(rename = "validationDocuments")]
    pub validation_documents: Option<String>,
    #[serde(rename = "validationReference")]
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
}

impl ReportingException {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "NO_KNOWN_PERSON" => Some(Self::NoKnownPerson),
            "NATURAL_PERSONS" => Some(Self::NaturalPersons),
            "NON_CONSOLIDATING" => Some(Self::NonConsolidating),
            "NO_LEI" => Some(Self::NoLei),
            "BINDING_LEGAL_RESTRICTIONS" => Some(Self::BindingLegalRestrictions),
            "DETRIMENT_NOT_EXCLUDED" => Some(Self::DetrimentNotExcluded),
            "DISCLOSURE_DETRIMENTAL" => Some(Self::DisclosureDetrimental),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NoKnownPerson => "NO_KNOWN_PERSON",
            Self::NaturalPersons => "NATURAL_PERSONS",
            Self::NonConsolidating => "NON_CONSOLIDATING",
            Self::NoLei => "NO_LEI",
            Self::BindingLegalRestrictions => "BINDING_LEGAL_RESTRICTIONS",
            Self::DetrimentNotExcluded => "DETRIMENT_NOT_EXCLUDED",
            Self::DisclosureDetrimental => "DISCLOSURE_DETRIMENTAL",
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
