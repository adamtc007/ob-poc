//! Companies House API response types
//!
//! Serde structs matching the Companies House API responses.
//!
//! # Resilience Pattern
//!
//! This module follows the "store raw, map lazily" pattern from CLAUDE.md:
//! - All enum-like fields stored as String
//! - Helper methods map to typed enums with Unknown variants
//! - Liberal use of `#[serde(default)]` for optional fields
//! - Unknown values logged at WARN level for future mapping

use crate::research::sources::normalized::{EntityStatus, EntityType, HolderType, OfficerRole};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

// =============================================================================
// Company Profile
// =============================================================================

/// Company profile from GET /company/{number}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChCompanyProfile {
    pub company_number: String,
    pub company_name: String,
    /// Raw status - use status() for typed enum
    #[serde(default)]
    pub company_status: String,
    /// Raw type - use company_type() for typed enum
    #[serde(rename = "type", default)]
    pub company_type_raw: String,
    #[serde(default)]
    pub jurisdiction: Option<String>,
    #[serde(default)]
    pub date_of_creation: Option<String>,
    #[serde(default)]
    pub date_of_cessation: Option<String>,
    #[serde(default)]
    pub registered_office_address: Option<ChAddress>,
    #[serde(default)]
    pub sic_codes: Vec<String>,
    #[serde(default)]
    pub has_been_liquidated: bool,
    #[serde(default)]
    pub has_charges: bool,
    #[serde(default)]
    pub has_insolvency_history: bool,
    #[serde(default)]
    pub registered_office_is_in_dispute: bool,
    #[serde(default)]
    pub undeliverable_registered_office_address: bool,
    /// Capture unknown fields for debugging
    #[serde(flatten)]
    pub extra: Option<std::collections::HashMap<String, serde_json::Value>>,
}

impl ChCompanyProfile {
    /// Map raw status to EntityStatus (resilient - never fails)
    pub fn status(&self) -> EntityStatus {
        match self.company_status.to_lowercase().as_str() {
            "active" => EntityStatus::Active,
            "dissolved" => EntityStatus::Dissolved,
            "liquidation"
            | "voluntary-arrangement"
            | "insolvency-proceedings"
            | "administration"
            | "receivership" => EntityStatus::Liquidation,
            "converted-closed" | "closed" => EntityStatus::Dissolved,
            "" => EntityStatus::Unknown("UNSPECIFIED".to_string()),
            other => {
                tracing::warn!(
                    source = "companies-house",
                    field = "company_status",
                    value = other,
                    "Unknown company status"
                );
                EntityStatus::Unknown(self.company_status.clone())
            }
        }
    }

    /// Map raw type to EntityType (resilient - never fails)
    pub fn company_type(&self) -> EntityType {
        match self.company_type_raw.to_lowercase().as_str() {
            "ltd"
            | "private-limited-guarant-nsc-limited-exemption"
            | "private-limited-guarant-nsc"
            | "private-limited-shares-section-30-exemption" => EntityType::LimitedCompany,
            "plc" | "public-limited-company" => EntityType::PublicCompany,
            "llp" | "limited-liability-partnership" => EntityType::Llp,
            "limited-partnership" | "scottish-partnership" => EntityType::Partnership,
            "uk-establishment" | "overseas-company" => EntityType::Branch,
            "" => EntityType::Unknown("UNSPECIFIED".to_string()),
            other => {
                tracing::warn!(
                    source = "companies-house",
                    field = "company_type",
                    value = other,
                    "Unknown company type"
                );
                EntityType::Unknown(self.company_type_raw.clone())
            }
        }
    }

    /// Check if company is active
    pub fn is_active(&self) -> bool {
        matches!(self.status(), EntityStatus::Active)
    }
}

// =============================================================================
// Address
// =============================================================================

/// Address structure - all fields optional for resilience
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ChAddress {
    #[serde(default)]
    pub address_line_1: Option<String>,
    #[serde(default)]
    pub address_line_2: Option<String>,
    #[serde(default)]
    pub locality: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub postal_code: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub premises: Option<String>,
    #[serde(default)]
    pub care_of: Option<String>,
    #[serde(default)]
    pub po_box: Option<String>,
}

// =============================================================================
// PSC (Persons with Significant Control)
// =============================================================================

/// PSC list from GET /company/{number}/persons-with-significant-control
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChPscList {
    #[serde(default)]
    pub items: Vec<ChPscRecord>,
    #[serde(default)]
    pub active_count: i32,
    #[serde(default)]
    pub ceased_count: i32,
    #[serde(default)]
    pub total_results: i32,
    #[serde(default)]
    pub items_per_page: i32,
    #[serde(default)]
    pub start_index: i32,
}

/// Individual PSC record
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChPscRecord {
    #[serde(default)]
    pub name: String,
    /// Raw kind - use holder_type() for typed enum
    #[serde(default)]
    pub kind: String,
    /// Raw natures - use ownership_range() for extraction
    #[serde(default)]
    pub natures_of_control: Vec<String>,

    // Individual PSC fields
    #[serde(default)]
    pub nationality: Option<String>,
    #[serde(default)]
    pub country_of_residence: Option<String>,
    #[serde(default)]
    pub date_of_birth: Option<ChPartialDate>,

    // Corporate PSC fields
    #[serde(default)]
    pub identification: Option<ChIdentification>,

    #[serde(default)]
    pub address: Option<ChAddress>,
    #[serde(default)]
    pub notified_on: Option<String>,
    #[serde(default)]
    pub ceased_on: Option<String>,

    // Links
    #[serde(default)]
    pub links: Option<ChLinks>,
}

impl ChPscRecord {
    /// Map PSC kind to HolderType (resilient - never fails)
    pub fn holder_type(&self) -> HolderType {
        match self.kind.as_str() {
            "individual-person-with-significant-control" => HolderType::Individual,
            "corporate-entity-person-with-significant-control" => HolderType::Corporate,
            "legal-person-person-with-significant-control" => HolderType::Corporate,
            "super-secure-person-with-significant-control" => HolderType::Individual,
            "" => HolderType::Unknown,
            other => {
                tracing::warn!(
                    source = "companies-house",
                    field = "psc.kind",
                    value = other,
                    "Unknown PSC kind"
                );
                HolderType::Unknown
            }
        }
    }

    /// Extract ownership percentage range from natures_of_control (resilient)
    pub fn ownership_range(&self) -> (Option<Decimal>, Option<Decimal>) {
        for nature in &self.natures_of_control {
            let n = nature.to_lowercase();
            // Ownership percentages
            if n.contains("25-to-50-percent") && n.contains("ownership") {
                return (Some(Decimal::from(25)), Some(Decimal::from(50)));
            }
            if n.contains("50-to-75-percent") && n.contains("ownership") {
                return (Some(Decimal::from(50)), Some(Decimal::from(75)));
            }
            if n.contains("75-to-100-percent") && n.contains("ownership") {
                return (Some(Decimal::from(75)), Some(Decimal::from(100)));
            }
            if n.contains("more-than-25-percent") && n.contains("ownership") {
                return (Some(Decimal::from(25)), None);
            }
        }
        // No ownership found - may be voting/control only
        (None, None)
    }

    /// Extract voting percentage from natures_of_control
    pub fn voting_range(&self) -> (Option<Decimal>, Option<Decimal>) {
        for nature in &self.natures_of_control {
            let n = nature.to_lowercase();
            if n.contains("25-to-50-percent") && n.contains("voting") {
                return (Some(Decimal::from(25)), Some(Decimal::from(50)));
            }
            if n.contains("50-to-75-percent") && n.contains("voting") {
                return (Some(Decimal::from(50)), Some(Decimal::from(75)));
            }
            if n.contains("75-to-100-percent") && n.contains("voting") {
                return (Some(Decimal::from(75)), Some(Decimal::from(100)));
            }
            if n.contains("more-than-25-percent") && n.contains("voting") {
                return (Some(Decimal::from(25)), None);
            }
        }
        (None, None)
    }

    /// Check if this PSC has voting rights
    pub fn has_voting_rights(&self) -> bool {
        self.natures_of_control
            .iter()
            .any(|n| n.to_lowercase().contains("voting"))
    }

    /// Check if this PSC has appointment/removal rights
    pub fn has_appointment_rights(&self) -> bool {
        self.natures_of_control.iter().any(|n| {
            let lower = n.to_lowercase();
            lower.contains("appoint") || lower.contains("remove")
        })
    }

    /// Check if this PSC has significant influence
    pub fn has_significant_influence(&self) -> bool {
        self.natures_of_control
            .iter()
            .any(|n| n.to_lowercase().contains("significant-influence"))
    }

    /// Check if this PSC is still active
    pub fn is_active(&self) -> bool {
        self.ceased_on.is_none()
    }
}

/// Corporate PSC identification - all fields optional
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ChIdentification {
    #[serde(default)]
    pub legal_form: Option<String>,
    #[serde(default)]
    pub legal_authority: Option<String>,
    #[serde(default)]
    pub place_registered: Option<String>,
    #[serde(default)]
    pub registration_number: Option<String>,
    #[serde(default)]
    pub country_registered: Option<String>,
}

/// Partial date (month and year, or just year)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChPartialDate {
    #[serde(default)]
    pub month: Option<i32>,
    #[serde(default)]
    pub year: i32,
}

impl ChPartialDate {
    /// Format as "YYYY-MM" or "YYYY"
    pub fn to_string_partial(&self) -> String {
        match self.month {
            Some(m) => format!("{}-{:02}", self.year, m),
            None => format!("{}", self.year),
        }
    }
}

/// Links in responses
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ChLinks {
    #[serde(rename = "self", default)]
    pub self_link: Option<String>,
    #[serde(default)]
    pub statement: Option<String>,
}

// =============================================================================
// Officers
// =============================================================================

/// Officers list from GET /company/{number}/officers
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChOfficerList {
    #[serde(default)]
    pub items: Vec<ChOfficer>,
    #[serde(default)]
    pub active_count: i32,
    #[serde(default)]
    pub resigned_count: i32,
    #[serde(default)]
    pub total_results: i32,
    #[serde(default)]
    pub items_per_page: i32,
    #[serde(default)]
    pub start_index: i32,
}

/// Individual officer record
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChOfficer {
    #[serde(default)]
    pub name: String,
    /// Raw role - use role() for typed enum
    #[serde(default)]
    pub officer_role: String,
    #[serde(default)]
    pub appointed_on: Option<String>,
    #[serde(default)]
    pub resigned_on: Option<String>,
    #[serde(default)]
    pub nationality: Option<String>,
    #[serde(default)]
    pub country_of_residence: Option<String>,
    #[serde(default)]
    pub date_of_birth: Option<ChPartialDate>,
    #[serde(default)]
    pub occupation: Option<String>,
    #[serde(default)]
    pub address: Option<ChAddress>,
    #[serde(default)]
    pub links: Option<ChOfficerLinks>,
}

impl ChOfficer {
    /// Map raw role to OfficerRole (resilient - never fails)
    pub fn role(&self) -> OfficerRole {
        match self.officer_role.to_lowercase().as_str() {
            "director" => OfficerRole::Director,
            "secretary" | "corporate-secretary" => OfficerRole::Secretary,
            "llp-member" | "llp-designated-member" => OfficerRole::Partner,
            "corporate-director" => OfficerRole::Director,
            "corporate-llp-member" | "corporate-llp-designated-member" => OfficerRole::Partner,
            "cic-manager" | "corporate-managing-officer" | "managing-officer" => {
                OfficerRole::Manager
            }
            "" => OfficerRole::Other("UNSPECIFIED".to_string()),
            other => {
                tracing::warn!(
                    source = "companies-house",
                    field = "officer_role",
                    value = other,
                    "Unknown officer role"
                );
                OfficerRole::Other(self.officer_role.clone())
            }
        }
    }

    /// Check if officer is still active
    pub fn is_active(&self) -> bool {
        self.resigned_on.is_none()
    }
}

/// Officer-specific links
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ChOfficerLinks {
    #[serde(default)]
    pub officer: Option<ChOfficerAppointments>,
    #[serde(rename = "self", default)]
    pub self_link: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ChOfficerAppointments {
    #[serde(default)]
    pub appointments: Option<String>,
}

// =============================================================================
// Search
// =============================================================================

/// Company search results from GET /search/companies
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChSearchResult {
    #[serde(default)]
    pub items: Vec<ChSearchItem>,
    #[serde(default)]
    pub total_results: i32,
    #[serde(default)]
    pub items_per_page: i32,
    #[serde(default)]
    pub start_index: i32,
    #[serde(default)]
    pub kind: Option<String>,
}

/// Individual search result item
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChSearchItem {
    #[serde(default)]
    pub company_number: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub company_status: String,
    #[serde(default)]
    pub company_type: String,
    #[serde(default)]
    pub address_snippet: Option<String>,
    #[serde(default)]
    pub date_of_creation: Option<String>,
    #[serde(default)]
    pub date_of_cessation: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub description_identifier: Option<Vec<String>>,
    #[serde(default)]
    pub matches: Option<ChMatches>,
}

impl ChSearchItem {
    /// Check if company is active
    pub fn is_active(&self) -> bool {
        self.company_status.to_lowercase() == "active"
    }
}

/// Match highlighting info
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ChMatches {
    #[serde(default)]
    pub title: Option<Vec<i32>>,
    #[serde(default)]
    pub snippet: Option<Vec<i32>>,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_partial_date_to_string() {
        let with_month = ChPartialDate {
            year: 1980,
            month: Some(6),
        };
        assert_eq!(with_month.to_string_partial(), "1980-06");

        let without_month = ChPartialDate {
            year: 1980,
            month: None,
        };
        assert_eq!(without_month.to_string_partial(), "1980");
    }

    #[test]
    fn test_company_status_mapping() {
        let mut profile = ChCompanyProfile {
            company_number: "12345678".into(),
            company_name: "Test Ltd".into(),
            company_status: "active".into(),
            company_type_raw: "ltd".into(),
            jurisdiction: None,
            date_of_creation: None,
            date_of_cessation: None,
            registered_office_address: None,
            sic_codes: vec![],
            has_been_liquidated: false,
            has_charges: false,
            has_insolvency_history: false,
            registered_office_is_in_dispute: false,
            undeliverable_registered_office_address: false,
            extra: None,
        };

        assert!(matches!(profile.status(), EntityStatus::Active));

        profile.company_status = "dissolved".into();
        assert!(matches!(profile.status(), EntityStatus::Dissolved));

        profile.company_status = "liquidation".into();
        assert!(matches!(profile.status(), EntityStatus::Liquidation));

        profile.company_status = "unknown-status-2025".into();
        assert!(matches!(profile.status(), EntityStatus::Unknown(_)));
    }

    #[test]
    fn test_psc_ownership_range() {
        let psc = ChPscRecord {
            name: "Test Person".into(),
            kind: "individual-person-with-significant-control".into(),
            natures_of_control: vec!["ownership-of-shares-25-to-50-percent".into()],
            nationality: None,
            country_of_residence: None,
            date_of_birth: None,
            identification: None,
            address: None,
            notified_on: None,
            ceased_on: None,
            links: None,
        };

        let (low, high) = psc.ownership_range();
        assert_eq!(low, Some(Decimal::from(25)));
        assert_eq!(high, Some(Decimal::from(50)));
    }

    #[test]
    fn test_psc_holder_type() {
        let mut psc = ChPscRecord {
            name: "Test".into(),
            kind: "individual-person-with-significant-control".into(),
            natures_of_control: vec![],
            nationality: None,
            country_of_residence: None,
            date_of_birth: None,
            identification: None,
            address: None,
            notified_on: None,
            ceased_on: None,
            links: None,
        };

        assert!(matches!(psc.holder_type(), HolderType::Individual));

        psc.kind = "corporate-entity-person-with-significant-control".into();
        assert!(matches!(psc.holder_type(), HolderType::Corporate));

        psc.kind = "unknown-psc-type".into();
        assert!(matches!(psc.holder_type(), HolderType::Unknown));
    }

    #[test]
    fn test_officer_role_mapping() {
        let mut officer = ChOfficer {
            name: "Test Person".into(),
            officer_role: "director".into(),
            appointed_on: None,
            resigned_on: None,
            nationality: None,
            country_of_residence: None,
            date_of_birth: None,
            occupation: None,
            address: None,
            links: None,
        };

        assert!(matches!(officer.role(), OfficerRole::Director));

        officer.officer_role = "secretary".into();
        assert!(matches!(officer.role(), OfficerRole::Secretary));

        officer.officer_role = "unknown-role-2025".into();
        assert!(matches!(officer.role(), OfficerRole::Other(_)));
    }

    #[test]
    fn test_deserialize_with_unknown_fields() {
        // Should not fail on unknown fields
        let json = r#"{
            "company_number": "12345678",
            "company_name": "TEST COMPANY LTD",
            "company_status": "active",
            "type": "ltd",
            "jurisdiction": "england-wales",
            "date_of_creation": "2020-01-15",
            "unknown_field_2025": "some value",
            "another_new_field": 42
        }"#;

        let profile: ChCompanyProfile = serde_json::from_str(json).unwrap();
        assert_eq!(profile.company_number, "12345678");
        assert!(profile.extra.is_some());
    }

    #[test]
    fn test_deserialize_minimal_response() {
        // Should handle minimal response with just required fields
        let json = r#"{
            "company_number": "12345678",
            "company_name": "TEST COMPANY LTD"
        }"#;

        let profile: ChCompanyProfile = serde_json::from_str(json).unwrap();
        assert_eq!(profile.company_number, "12345678");
        assert_eq!(profile.company_status, ""); // Empty default
    }
}
