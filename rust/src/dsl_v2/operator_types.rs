//! Operator Types - Business vocabulary for DSL arguments
//!
//! Replaces raw `entity_ref kinds:[x]` with typed operator references.
//! Operators never see implementation details (CBU, entity_ref, trading-profile).
//!
//! # Example
//!
//! ```yaml
//! # WRONG (leaks implementation)
//! args:
//!   target:
//!     type: entity_ref
//!     kinds: [cbu]
//!
//! # RIGHT (operator vocabulary)
//! args:
//!   target:
//!     type: structure_ref
//! ```
//!
//! The `OperatorType` maps to internal entity kinds for resolution,
//! but the UI only ever shows the operator-facing labels.

use serde::{Deserialize, Serialize};

/// Operator-facing reference types
///
/// These replace raw `entity_ref kinds:[x]` in verb schemas.
/// Each type maps to one or more internal entity kinds.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum OperatorType {
    /// Reference to a structure (fund, mandate vehicle)
    /// Internal: cbu
    StructureRef,

    /// Reference to a party (person, company, trust)
    /// Internal: person, company, trust
    PartyRef,

    /// Reference to a natural person specifically
    /// Internal: person
    PersonRef,

    /// Reference to a corporate entity specifically
    /// Internal: company
    CompanyRef,

    /// Reference to a client (commercial relationship)
    /// Internal: client
    ClientRef,

    /// Reference to a KYC case
    /// Internal: kyc-case
    CaseRef,

    /// Reference to an investment mandate
    /// Internal: trading-profile
    MandateRef,

    /// Reference to a document
    /// Internal: document
    DocumentRef,

    /// Reference to a role assignment
    /// Internal: cbu-role
    RoleRef,
}

impl OperatorType {
    /// Get internal entity kinds for resolution
    ///
    /// These are the actual entity_ref kinds used in database queries.
    /// The operator never sees these.
    pub fn internal_kinds(&self) -> &'static [&'static str] {
        match self {
            Self::StructureRef => &["cbu"],
            Self::PartyRef => &["person", "company", "trust"],
            Self::PersonRef => &["person"],
            Self::CompanyRef => &["company"],
            Self::ClientRef => &["client"],
            Self::CaseRef => &["kyc-case"],
            Self::MandateRef => &["trading-profile"],
            Self::DocumentRef => &["document"],
            Self::RoleRef => &["cbu-role"],
        }
    }

    /// Get UI display label (what operator sees)
    ///
    /// Never returns internal names like "cbu" or "trading-profile".
    pub fn display_label(&self) -> &'static str {
        match self {
            Self::StructureRef => "Structure",
            Self::PartyRef => "Party",
            Self::PersonRef => "Person",
            Self::CompanyRef => "Company",
            Self::ClientRef => "Client",
            Self::CaseRef => "Case",
            Self::MandateRef => "Mandate",
            Self::DocumentRef => "Document",
            Self::RoleRef => "Role",
        }
    }

    /// Get the placeholder text for search/picker UI
    pub fn placeholder(&self) -> &'static str {
        match self {
            Self::StructureRef => "Search structures...",
            Self::PartyRef => "Search parties...",
            Self::PersonRef => "Search people...",
            Self::CompanyRef => "Search companies...",
            Self::ClientRef => "Search clients...",
            Self::CaseRef => "Search cases...",
            Self::MandateRef => "Search mandates...",
            Self::DocumentRef => "Search documents...",
            Self::RoleRef => "Search roles...",
        }
    }

    /// Parse from string (used in YAML schema parsing)
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "structure_ref" | "StructureRef" => Some(Self::StructureRef),
            "party_ref" | "PartyRef" => Some(Self::PartyRef),
            "person_ref" | "PersonRef" => Some(Self::PersonRef),
            "company_ref" | "CompanyRef" => Some(Self::CompanyRef),
            "client_ref" | "ClientRef" => Some(Self::ClientRef),
            "case_ref" | "CaseRef" => Some(Self::CaseRef),
            "mandate_ref" | "MandateRef" => Some(Self::MandateRef),
            "document_ref" | "DocumentRef" => Some(Self::DocumentRef),
            "role_ref" | "RoleRef" => Some(Self::RoleRef),
            _ => None,
        }
    }

    /// Get schema type name (for YAML serialization)
    pub fn schema_name(&self) -> &'static str {
        match self {
            Self::StructureRef => "structure_ref",
            Self::PartyRef => "party_ref",
            Self::PersonRef => "person_ref",
            Self::CompanyRef => "company_ref",
            Self::ClientRef => "client_ref",
            Self::CaseRef => "case_ref",
            Self::MandateRef => "mandate_ref",
            Self::DocumentRef => "document_ref",
            Self::RoleRef => "role_ref",
        }
    }

    /// Check if this type is a subset of another
    ///
    /// e.g., PersonRef is a subset of PartyRef
    pub fn is_subset_of(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::PersonRef, Self::PartyRef) => true,
            (Self::CompanyRef, Self::PartyRef) => true,
            _ => self == other,
        }
    }
}

impl std::fmt::Display for OperatorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_label())
    }
}

/// Role types in the operator vocabulary
///
/// Maps operator-facing role names to internal role codes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum OperatorRole {
    /// General Partner (PE/Hedge only)
    Gp,
    /// Limited Partner
    Lp,
    /// Investment Manager
    Im,
    /// Management Company
    Manco,
    /// Custodian
    Custodian,
    /// Administrator
    Admin,
    /// Director
    Director,
    /// Beneficial Owner
    Ubo,
    /// Authorized Signatory
    Signatory,
}

impl OperatorRole {
    /// Get internal role code
    pub fn internal_code(&self) -> &'static str {
        match self {
            Self::Gp => "general-partner",
            Self::Lp => "limited-partner",
            Self::Im => "investment-manager",
            Self::Manco => "management-company",
            Self::Custodian => "custodian",
            Self::Admin => "administrator",
            Self::Director => "director",
            Self::Ubo => "beneficial-owner",
            Self::Signatory => "authorized-signatory",
        }
    }

    /// Get display label
    pub fn display_label(&self) -> &'static str {
        match self {
            Self::Gp => "General Partner",
            Self::Lp => "Limited Partner",
            Self::Im => "Investment Manager",
            Self::Manco => "Management Company",
            Self::Custodian => "Custodian",
            Self::Admin => "Administrator",
            Self::Director => "Director",
            Self::Ubo => "Beneficial Owner",
            Self::Signatory => "Authorized Signatory",
        }
    }

    /// Get short label (for compact UI)
    pub fn short_label(&self) -> &'static str {
        match self {
            Self::Gp => "GP",
            Self::Lp => "LP",
            Self::Im => "IM",
            Self::Manco => "ManCo",
            Self::Custodian => "Custodian",
            Self::Admin => "Admin",
            Self::Director => "Director",
            Self::Ubo => "UBO",
            Self::Signatory => "Signatory",
        }
    }

    /// Parse from operator key (what user types)
    pub fn from_operator_key(key: &str) -> Option<Self> {
        match key.to_lowercase().as_str() {
            "gp" | "general-partner" | "general_partner" => Some(Self::Gp),
            "lp" | "limited-partner" | "limited_partner" => Some(Self::Lp),
            "im" | "investment-manager" | "investment_manager" => Some(Self::Im),
            "manco" | "management-company" | "management_company" => Some(Self::Manco),
            "custodian" => Some(Self::Custodian),
            "admin" | "administrator" => Some(Self::Admin),
            "director" => Some(Self::Director),
            "ubo" | "beneficial-owner" | "beneficial_owner" => Some(Self::Ubo),
            "signatory" | "authorized-signatory" | "authorized_signatory" => Some(Self::Signatory),
            _ => None,
        }
    }

    /// Check if this role is valid for a given structure type
    pub fn valid_for_structure(
        &self,
        structure_type: &crate::session::unified::StructureType,
    ) -> bool {
        use crate::session::unified::StructureType;
        match self {
            // GP only valid for PE and Hedge
            Self::Gp => matches!(structure_type, StructureType::Pe | StructureType::Hedge),
            // LP only valid for PE and Hedge
            Self::Lp => matches!(structure_type, StructureType::Pe | StructureType::Hedge),
            // Other roles valid for all structure types
            _ => true,
        }
    }
}

impl std::fmt::Display for OperatorRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operator_type_internal_kinds() {
        assert_eq!(OperatorType::StructureRef.internal_kinds(), &["cbu"]);
        assert_eq!(
            OperatorType::PartyRef.internal_kinds(),
            &["person", "company", "trust"]
        );
        assert_eq!(
            OperatorType::MandateRef.internal_kinds(),
            &["trading-profile"]
        );
    }

    #[test]
    fn test_operator_type_display() {
        assert_eq!(OperatorType::StructureRef.display_label(), "Structure");
        assert_eq!(OperatorType::MandateRef.display_label(), "Mandate");
        assert_eq!(OperatorType::CaseRef.display_label(), "Case");

        // Never shows internal names
        for typ in [
            OperatorType::StructureRef,
            OperatorType::MandateRef,
            OperatorType::CaseRef,
        ] {
            let label = typ.display_label();
            assert!(!label.contains("cbu"), "Label should not contain 'cbu'");
            assert!(
                !label.contains("trading-profile"),
                "Label should not contain 'trading-profile'"
            );
            assert!(
                !label.contains("kyc-case"),
                "Label should not contain 'kyc-case'"
            );
        }
    }

    #[test]
    fn test_operator_type_parse() {
        assert_eq!(
            OperatorType::parse("structure_ref"),
            Some(OperatorType::StructureRef)
        );
        assert_eq!(
            OperatorType::parse("StructureRef"),
            Some(OperatorType::StructureRef)
        );
        assert_eq!(
            OperatorType::parse("mandate_ref"),
            Some(OperatorType::MandateRef)
        );
        assert_eq!(OperatorType::parse("unknown"), None);
    }

    #[test]
    fn test_operator_type_subset() {
        assert!(OperatorType::PersonRef.is_subset_of(&OperatorType::PartyRef));
        assert!(OperatorType::CompanyRef.is_subset_of(&OperatorType::PartyRef));
        assert!(!OperatorType::PartyRef.is_subset_of(&OperatorType::PersonRef));
        assert!(OperatorType::StructureRef.is_subset_of(&OperatorType::StructureRef));
    }

    #[test]
    fn test_operator_role_internal_code() {
        assert_eq!(OperatorRole::Gp.internal_code(), "general-partner");
        assert_eq!(OperatorRole::Im.internal_code(), "investment-manager");
        assert_eq!(OperatorRole::Manco.internal_code(), "management-company");
    }

    #[test]
    fn test_operator_role_from_key() {
        assert_eq!(
            OperatorRole::from_operator_key("gp"),
            Some(OperatorRole::Gp)
        );
        assert_eq!(
            OperatorRole::from_operator_key("GP"),
            Some(OperatorRole::Gp)
        );
        assert_eq!(
            OperatorRole::from_operator_key("general-partner"),
            Some(OperatorRole::Gp)
        );
        assert_eq!(
            OperatorRole::from_operator_key("im"),
            Some(OperatorRole::Im)
        );
        assert_eq!(OperatorRole::from_operator_key("unknown"), None);
    }

    #[test]
    fn test_operator_role_structure_validation() {
        use crate::session::unified::StructureType;

        // GP only valid for PE and Hedge
        assert!(OperatorRole::Gp.valid_for_structure(&StructureType::Pe));
        assert!(OperatorRole::Gp.valid_for_structure(&StructureType::Hedge));
        assert!(!OperatorRole::Gp.valid_for_structure(&StructureType::Sicav));
        assert!(!OperatorRole::Gp.valid_for_structure(&StructureType::Etf));

        // IM valid for all
        assert!(OperatorRole::Im.valid_for_structure(&StructureType::Pe));
        assert!(OperatorRole::Im.valid_for_structure(&StructureType::Sicav));
        assert!(OperatorRole::Im.valid_for_structure(&StructureType::Etf));
    }
}
