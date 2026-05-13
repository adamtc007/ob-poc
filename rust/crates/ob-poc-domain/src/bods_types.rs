//! BODS 0.4 Types for OB-POC
//!
//! Structs matching the BODS integration tables from migration 010.
//! These support the LEI spine + GLEIF hierarchy + BODS interest types.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Entity identifier (LEI spine + other identifiers like BIC, ISIN, REG_NUM)
/// Maps to `ob-poc.entity_identifiers` table.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EntityIdentifier {
    pub identifier_id: Uuid,
    pub entity_id: Uuid,
    /// Type of identifier: LEI, BIC, ISIN, CIK, MIC, REG_NUM, FIGI, CUSIP, SEDOL
    pub identifier_type: String,
    /// The actual identifier value (e.g., the 20-char LEI code)
    pub identifier_value: String,
    pub issuing_authority: Option<String>,
    pub is_primary: Option<bool>,
    pub valid_from: Option<NaiveDate>,
    pub valid_until: Option<NaiveDate>,
    pub source: Option<String>,
    pub scheme_name: Option<String>,
    pub uri: Option<String>,
    pub is_validated: Option<bool>,
    pub validated_at: Option<DateTime<Utc>>,
    pub validation_source: Option<String>,
    pub validation_details: Option<serde_json::Value>,
    /// LEI-specific: ISSUED, LAPSED, RETIRED, etc.
    pub lei_status: Option<String>,
    pub lei_next_renewal: Option<NaiveDate>,
    pub lei_managing_lou: Option<String>,
    pub lei_initial_registration: Option<NaiveDate>,
    pub lei_last_update: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// GLEIF corporate hierarchy relationship (SEPARATE from UBO ownership)
/// Maps to `ob-poc.gleif_relationships` table.
///
/// GLEIF hierarchy = accounting consolidation (who owns whom for financial reporting)
/// This is NOT the same as beneficial ownership (KYC/AML).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct GleifRelationship {
    pub gleif_rel_id: Uuid,
    pub parent_entity_id: Uuid,
    pub parent_lei: String,
    pub child_entity_id: Uuid,
    pub child_lei: String,
    /// DIRECT_PARENT, ULTIMATE_PARENT, IS_DIRECTLY_CONSOLIDATED_BY, IS_ULTIMATELY_CONSOLIDATED_BY
    pub relationship_type: String,
    pub relationship_status: Option<String>,
    pub ownership_percentage: Option<rust_decimal::Decimal>,
    /// IFRS, US_GAAP, LOCAL_GAAP
    pub accounting_standard: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub gleif_record_id: Option<String>,
    pub fetched_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
}

/// BODS interest type codelist entry
/// Maps to `ob-poc.bods_interest_types` reference table.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BodsInterestType {
    pub type_code: String,
    pub display_name: String,
    /// ownership, control, trust, beneficial
    pub category: String,
    pub description: Option<String>,
    pub bods_standard: Option<bool>,
    pub requires_percentage: Option<bool>,
    pub display_order: Option<i32>,
}

/// BODS entity type codelist entry
/// Maps to `ob-poc.bods_entity_types` reference table.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BodsEntityType {
    pub type_code: String,
    pub display_name: String,
    pub description: Option<String>,
    pub bods_standard: Option<bool>,
    pub display_order: Option<i32>,
}

/// Person PEP status (BODS-compliant)
/// Maps to `ob-poc.person_pep_status` table.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PersonPepStatus {
    pub pep_status_id: Uuid,
    pub person_entity_id: Uuid,
    /// 'isPep', 'isNotPep', 'unknown'
    pub status: String,
    pub reason: Option<String>,
    pub jurisdiction: Option<String>,
    pub position_held: Option<String>,
    pub position_level: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub source_type: Option<String>,
    pub screening_id: Option<Uuid>,
    pub verified_at: Option<DateTime<Utc>>,
    pub verified_by: Option<String>,
    pub pep_risk_level: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

// =============================================================================
// Input types for creating new records
// =============================================================================

/// Fields for creating/attaching a new entity identifier
#[derive(Debug, Clone)]
pub struct NewEntityIdentifier {
    pub entity_id: Uuid,
    /// Type: LEI, BIC, ISIN, REG_NUM, etc.
    pub identifier_type: String,
    /// The actual identifier value
    pub identifier_value: String,
    pub issuing_authority: Option<String>,
    pub is_primary: Option<bool>,
    pub source: Option<String>,
    pub scheme_name: Option<String>,
    pub uri: Option<String>,
}

/// Fields for creating a GLEIF relationship
#[derive(Debug, Clone)]
pub struct NewGleifRelationship {
    pub parent_entity_id: Uuid,
    pub parent_lei: String,
    pub child_entity_id: Uuid,
    pub child_lei: String,
    pub relationship_type: String,
    pub ownership_percentage: Option<rust_decimal::Decimal>,
    pub accounting_standard: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub gleif_record_id: Option<String>,
}

/// Fields for creating a PEP status record
#[derive(Debug, Clone)]
pub struct NewPersonPepStatus {
    pub person_entity_id: Uuid,
    pub status: String,
    pub reason: Option<String>,
    pub jurisdiction: Option<String>,
    pub position_held: Option<String>,
    pub position_level: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub source_type: Option<String>,
    pub screening_id: Option<Uuid>,
}

// =============================================================================
// Lightweight view types
// =============================================================================

/// Entity with its LEI (from v_entities_with_lei view)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EntityWithLei {
    pub entity_id: Uuid,
    pub name: String,
    pub bods_entity_type: Option<String>,
    pub entity_type_code: Option<String>,
    pub lei: Option<String>,
    pub lei_status: Option<String>,
    pub lei_next_renewal: Option<NaiveDate>,
    pub lei_validated: Option<bool>,
}

/// UBO interest with BODS type info (from v_ubo_interests view)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UboInterest {
    pub relationship_id: Uuid,
    pub interested_party_id: Uuid,
    pub interested_party_name: String,
    pub subject_id: Uuid,
    pub subject_name: String,
    pub interest_type: Option<String>,
    pub interest_type_display: Option<String>,
    pub interest_category: Option<String>,
    pub direct_or_indirect: Option<String>,
    pub ownership_share: Option<rust_decimal::Decimal>,
    pub share_minimum: Option<rust_decimal::Decimal>,
    pub share_maximum: Option<rust_decimal::Decimal>,
    pub effective_from: Option<NaiveDate>,
    pub effective_to: Option<NaiveDate>,
    pub is_component: Option<bool>,
    pub component_of_relationship_id: Option<Uuid>,
}

/// GLEIF hierarchy entry (from v_gleif_hierarchy view)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct GleifHierarchyEntry {
    pub gleif_rel_id: Uuid,
    pub parent_entity_id: Uuid,
    pub parent_name: String,
    pub parent_lei: String,
    pub child_entity_id: Uuid,
    pub child_name: String,
    pub child_lei: String,
    pub relationship_type: String,
    pub relationship_status: Option<String>,
    pub ownership_percentage: Option<rust_decimal::Decimal>,
    pub accounting_standard: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
}
