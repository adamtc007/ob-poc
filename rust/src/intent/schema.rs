//! Schema types for KYC intents
//!
//! These types represent the domain entities referenced in intents.
//! They use KYC/financial services terminology.

use serde::{Deserialize, Serialize};

// ============================================================================
// Client Types
// ============================================================================

/// Individual client (natural person)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndividualClient {
    pub name: String,
    
    #[serde(default)]
    pub jurisdiction: Option<String>,
    
    #[serde(default)]
    pub nationality: Option<String>,
    
    #[serde(default)]
    pub date_of_birth: Option<String>,
    
    #[serde(default)]
    pub tax_residency: Option<String>,
    
    #[serde(default)]
    pub occupation: Option<String>,
    
    #[serde(default)]
    pub source_of_wealth: Option<String>,
}

/// Corporate client (legal entity)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CorporateClient {
    pub name: String,
    
    #[serde(default)]
    pub jurisdiction: Option<String>,
    
    #[serde(default)]
    pub registration_number: Option<String>,
    
    #[serde(default)]
    pub entity_type: Option<CorporateEntityType>,
    
    #[serde(default)]
    pub incorporation_date: Option<String>,
    
    #[serde(default)]
    pub registered_address: Option<AddressSpec>,
    
    #[serde(default)]
    pub trading_address: Option<AddressSpec>,
    
    #[serde(default)]
    pub industry_sector: Option<String>,
    
    #[serde(default)]
    pub lei_code: Option<String>,  // Legal Entity Identifier
}

/// Fund/investment vehicle
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FundClient {
    pub name: String,
    
    #[serde(default)]
    pub jurisdiction: Option<String>,
    
    #[serde(default)]
    pub fund_type: Option<FundType>,
    
    #[serde(default)]
    pub registration_number: Option<String>,
    
    #[serde(default)]
    pub isin: Option<String>,  // International Securities ID
    
    #[serde(default)]
    pub nav_currency: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CorporateEntityType {
    LimitedCompany,
    PublicLimitedCompany,
    Partnership,
    LimitedPartnership,
    LimitedLiabilityPartnership,
    Trust,
    Foundation,
    Charity,
    GovernmentEntity,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FundType {
    Ucits,
    Aif,
    Etf,
    HedgeFund,
    PrivateEquity,
    VentureCapital,
    RealEstate,
    MoneyMarket,
    Other,
}

// ============================================================================
// Document Specifications
// ============================================================================

/// Document to be uploaded/cataloged
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocumentSpec {
    /// Document type code (e.g., PASSPORT_GBR, CERT_OF_INCORPORATION)
    pub document_type: String,
    
    /// Optional title/description
    #[serde(default)]
    pub title: Option<String>,
    
    /// Optional file reference (S3 key, URL, etc.)
    #[serde(default)]
    pub file_reference: Option<String>,
    
    /// Whether to extract attributes automatically
    #[serde(default = "default_true")]
    pub extract_attributes: bool,
    
    /// Document issue date
    #[serde(default)]
    pub issue_date: Option<String>,
    
    /// Document expiry date
    #[serde(default)]
    pub expiry_date: Option<String>,
    
    /// Issuing authority/country
    #[serde(default)]
    pub issuing_authority: Option<String>,
}

fn default_true() -> bool {
    true
}

// ============================================================================
// Entity/Role Specifications
// ============================================================================

/// Beneficial owner specification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BeneficialOwnerSpec {
    pub name: String,
    
    /// Ownership percentage (0.0 - 100.0)
    pub ownership_percentage: f64,
    
    #[serde(default)]
    pub nationality: Option<String>,
    
    #[serde(default)]
    pub date_of_birth: Option<String>,
    
    /// Is this a direct or indirect ownership?
    #[serde(default = "default_true")]
    pub is_direct: bool,
    
    /// Through which entity (for indirect ownership)
    #[serde(default)]
    pub via_entity: Option<String>,
    
    /// Nature of control if not ownership-based
    #[serde(default)]
    pub control_type: Option<ControlType>,
}

/// Director specification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DirectorSpec {
    pub name: String,
    
    #[serde(default)]
    pub role: Option<DirectorRole>,
    
    #[serde(default)]
    pub nationality: Option<String>,
    
    #[serde(default)]
    pub date_of_birth: Option<String>,
    
    #[serde(default)]
    pub appointment_date: Option<String>,
    
    #[serde(default)]
    pub is_executive: Option<bool>,
}

/// Authorized signatory specification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SignatorySpec {
    pub name: String,
    
    #[serde(default)]
    pub signing_authority: Option<SigningAuthority>,
    
    #[serde(default)]
    pub nationality: Option<String>,
    
    /// Maximum signing limit (optional)
    #[serde(default)]
    pub signing_limit: Option<f64>,
    
    #[serde(default)]
    pub signing_limit_currency: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ControlType {
    Voting,
    AppointmentRights,
    VetoRights,
    SignificantInfluence,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DirectorRole {
    Chairman,
    ExecutiveDirector,
    NonExecutiveDirector,
    IndependentDirector,
    AlternateDirector,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SigningAuthority {
    /// Can sign alone
    Sole,
    /// Must sign with another
    Joint,
    /// Can sign up to a limit alone
    LimitedSole,
    /// Any two must sign together
    AnyTwo,
}

// ============================================================================
// Reference Types (for existing entities)
// ============================================================================

/// Reference to an existing CBU
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum CbuReference {
    /// Reference by UUID
    ById {
        id: String,
    },
    /// Reference by business code
    ByCode {
        code: String,
    },
    /// Reference by binding in current session
    ByBinding {
        binding: String,
    },
}

/// Reference to an existing document
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum DocumentReference {
    ById {
        id: String,
    },
    ByCode {
        code: String,
    },
    ByBinding {
        binding: String,
    },
}

/// Reference to an existing entity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum EntityReference {
    ById {
        id: String,
    },
    ByCode {
        code: String,
    },
    ByBinding {
        binding: String,
    },
    /// Create new if not exists
    CreateOrLookup {
        name: String,
        entity_type: Option<String>,
    },
}

// ============================================================================
// Address and Contact
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AddressSpec {
    #[serde(default)]
    pub line1: Option<String>,
    #[serde(default)]
    pub line2: Option<String>,
    #[serde(default)]
    pub city: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub postal_code: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContactInfo {
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub address: Option<AddressSpec>,
}

// ============================================================================
// KYC/Verification Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum KycCheckType {
    SanctionsScreening,
    PepScreening,
    AdverseMedia,
    IdVerification,
    AddressVerification,
    DocumentAuthenticity,
    UboVerification,
    CompanyRegistryCheck,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RiskRating {
    Low,
    Medium,
    High,
    Prohibited,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CorporateRelationshipType {
    ParentSubsidiary,
    Associate,
    JointVenture,
    ControlledEntity,
    BranchOffice,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_spec_defaults() {
        let json = r#"{ "document_type": "PASSPORT_GBR" }"#;
        let doc: DocumentSpec = serde_json::from_str(json).unwrap();
        
        assert_eq!(doc.document_type, "PASSPORT_GBR");
        assert!(doc.extract_attributes); // default true
        assert!(doc.title.is_none());
    }

    #[test]
    fn test_beneficial_owner_with_control() {
        let json = r#"{
            "name": "Complex Owner",
            "ownership_percentage": 15.0,
            "is_direct": false,
            "via_entity": "Holding Company Ltd",
            "control_type": "voting"
        }"#;
        
        let owner: BeneficialOwnerSpec = serde_json::from_str(json).unwrap();
        assert_eq!(owner.ownership_percentage, 15.0);
        assert!(!owner.is_direct);
        assert_eq!(owner.control_type, Some(ControlType::Voting));
    }

    #[test]
    fn test_cbu_reference_variants() {
        // By ID
        let by_id: CbuReference = serde_json::from_str(
            r#"{ "id": "550e8400-e29b-41d4-a716-446655440000" }"#
        ).unwrap();
        assert!(matches!(by_id, CbuReference::ById { .. }));

        // By code
        let by_code: CbuReference = serde_json::from_str(
            r#"{ "code": "CBU-2024-001" }"#
        ).unwrap();
        assert!(matches!(by_code, CbuReference::ByCode { .. }));

        // By binding
        let by_binding: CbuReference = serde_json::from_str(
            r#"{ "binding": "@cbu" }"#
        ).unwrap();
        assert!(matches!(by_binding, CbuReference::ByBinding { .. }));
    }
}
