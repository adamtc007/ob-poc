//! Intent Schema for KYC/Onboarding Domain
//!
//! This module defines the semantic intents that an agent can produce.
//! Intents are JSON-serializable and represent WHAT the user wants to achieve,
//! not HOW to achieve it (that's the Planner's job).
//!
//! # Architecture
//!
//! ```text
//! Agent Output (JSON) → Intent (validated) → Planner → DSL Source
//!                            ↓
//!                    This module defines the schema
//! ```
//!
//! # Design Principles
//!
//! 1. **Closed Enum**: Agent can only produce known intent types
//! 2. **Validated Structure**: serde handles JSON → struct validation
//! 3. **Domain Language**: Uses KYC terms (CBU, UBO, jurisdiction)
//! 4. **No DSL Knowledge**: Intent has no idea about :as bindings or syntax

mod schema;
mod validation;

pub use schema::*;
pub use validation::*;

use serde::{Deserialize, Serialize};

// ============================================================================
// Top-Level Intent Enum
// ============================================================================

/// All possible intents an agent can produce.
///
/// This is a CLOSED set - agents can only classify into these categories.
/// Adding new capabilities means adding new variants here.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "intent", rename_all = "snake_case")]
pub enum KycIntent {
    // -------------------------------------------------------------------------
    // CBU Lifecycle
    // -------------------------------------------------------------------------
    
    /// Create a new individual client (natural person)
    OnboardIndividual {
        client: IndividualClient,
        #[serde(default)]
        documents: Vec<DocumentSpec>,
        #[serde(default)]
        contact: Option<ContactInfo>,
    },

    /// Create a new corporate client (legal entity)
    OnboardCorporate {
        client: CorporateClient,
        #[serde(default)]
        documents: Vec<DocumentSpec>,
        #[serde(default)]
        beneficial_owners: Vec<BeneficialOwnerSpec>,
        #[serde(default)]
        directors: Vec<DirectorSpec>,
        #[serde(default)]
        authorized_signatories: Vec<SignatorySpec>,
    },

    /// Create a fund/investment vehicle CBU
    OnboardFund {
        fund: FundClient,
        #[serde(default)]
        documents: Vec<DocumentSpec>,
        #[serde(default)]
        management_company: Option<EntityReference>,
    },

    // -------------------------------------------------------------------------
    // Document Operations
    // -------------------------------------------------------------------------

    /// Add a document to an existing CBU
    AddDocument {
        cbu_id: CbuReference,
        document: DocumentSpec,
        #[serde(default = "default_true")]
        extract_attributes: bool,
    },

    /// Add multiple documents to a CBU
    AddDocuments {
        cbu_id: CbuReference,
        documents: Vec<DocumentSpec>,
        #[serde(default = "default_true")]
        extract_attributes: bool,
    },

    /// Extract/re-extract attributes from a document
    ExtractDocument {
        document_id: DocumentReference,
    },

    /// Link a document to an entity (e.g., passport to beneficial owner)
    LinkDocumentToEntity {
        document_id: DocumentReference,
        entity_id: EntityReference,
    },

    // -------------------------------------------------------------------------
    // Entity/Role Operations
    // -------------------------------------------------------------------------

    /// Add a beneficial owner to a corporate CBU
    AddBeneficialOwner {
        cbu_id: CbuReference,
        owner: BeneficialOwnerSpec,
        #[serde(default)]
        evidence_documents: Vec<DocumentSpec>,
    },

    /// Add a director to a corporate CBU
    AddDirector {
        cbu_id: CbuReference,
        director: DirectorSpec,
        #[serde(default)]
        evidence_documents: Vec<DocumentSpec>,
    },

    /// Add an authorized signatory
    AddSignatory {
        cbu_id: CbuReference,
        signatory: SignatorySpec,
        #[serde(default)]
        evidence_documents: Vec<DocumentSpec>,
    },

    /// Create a parent-subsidiary relationship
    LinkCorporateStructure {
        parent_entity: EntityReference,
        subsidiary_entity: EntityReference,
        ownership_percentage: Option<f64>,
        relationship_type: CorporateRelationshipType,
    },

    // -------------------------------------------------------------------------
    // KYC/Verification Operations
    // -------------------------------------------------------------------------

    /// Run KYC checks on a CBU
    RunKycChecks {
        cbu_id: CbuReference,
        #[serde(default)]
        check_types: Vec<KycCheckType>,
    },

    /// Validate attributes across documents (cross-doc validation)
    ValidateAttributes {
        cbu_id: CbuReference,
        #[serde(default)]
        attribute_codes: Vec<String>,
    },

    /// Update CBU risk rating
    UpdateRiskRating {
        cbu_id: CbuReference,
        risk_rating: RiskRating,
        rationale: Option<String>,
    },

    // -------------------------------------------------------------------------
    // Query Operations (read-only)
    // -------------------------------------------------------------------------

    /// Get CBU status and summary
    GetCbuStatus {
        cbu_id: CbuReference,
    },

    /// List documents for a CBU
    ListDocuments {
        cbu_id: CbuReference,
        #[serde(default)]
        document_type_filter: Option<String>,
    },

    /// List beneficial owners for a CBU
    ListBeneficialOwners {
        cbu_id: CbuReference,
    },
}

fn default_true() -> bool {
    true
}

impl KycIntent {
    /// Get the intent name for logging/debugging
    pub fn intent_name(&self) -> &'static str {
        match self {
            KycIntent::OnboardIndividual { .. } => "onboard_individual",
            KycIntent::OnboardCorporate { .. } => "onboard_corporate",
            KycIntent::OnboardFund { .. } => "onboard_fund",
            KycIntent::AddDocument { .. } => "add_document",
            KycIntent::AddDocuments { .. } => "add_documents",
            KycIntent::ExtractDocument { .. } => "extract_document",
            KycIntent::LinkDocumentToEntity { .. } => "link_document_to_entity",
            KycIntent::AddBeneficialOwner { .. } => "add_beneficial_owner",
            KycIntent::AddDirector { .. } => "add_director",
            KycIntent::AddSignatory { .. } => "add_signatory",
            KycIntent::LinkCorporateStructure { .. } => "link_corporate_structure",
            KycIntent::RunKycChecks { .. } => "run_kyc_checks",
            KycIntent::ValidateAttributes { .. } => "validate_attributes",
            KycIntent::UpdateRiskRating { .. } => "update_risk_rating",
            KycIntent::GetCbuStatus { .. } => "get_cbu_status",
            KycIntent::ListDocuments { .. } => "list_documents",
            KycIntent::ListBeneficialOwners { .. } => "list_beneficial_owners",
        }
    }

    /// Check if this intent mutates state
    pub fn is_mutating(&self) -> bool {
        match self {
            KycIntent::GetCbuStatus { .. }
            | KycIntent::ListDocuments { .. }
            | KycIntent::ListBeneficialOwners { .. } => false,
            _ => true,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_onboard_individual() {
        let json = r#"{
            "intent": "onboard_individual",
            "client": {
                "name": "John Smith",
                "jurisdiction": "UK",
                "nationality": "GBR"
            },
            "documents": [
                { "document_type": "PASSPORT_GBR" }
            ]
        }"#;

        let intent: KycIntent = serde_json::from_str(json).unwrap();
        
        match intent {
            KycIntent::OnboardIndividual { client, documents, .. } => {
                assert_eq!(client.name, "John Smith");
                assert_eq!(client.jurisdiction, Some("UK".to_string()));
                assert_eq!(documents.len(), 1);
                assert_eq!(documents[0].document_type, "PASSPORT_GBR");
            }
            _ => panic!("Wrong intent type"),
        }
    }

    #[test]
    fn test_deserialize_onboard_corporate_with_ubos() {
        let json = r#"{
            "intent": "onboard_corporate",
            "client": {
                "name": "Acme Holdings Ltd",
                "jurisdiction": "UK",
                "registration_number": "12345678",
                "entity_type": "limited_company"
            },
            "documents": [
                { "document_type": "CERT_OF_INCORPORATION" },
                { "document_type": "UTILITY_BILL" }
            ],
            "beneficial_owners": [
                {
                    "name": "Jane Owner",
                    "ownership_percentage": 51.0,
                    "nationality": "GBR",
                    "is_direct": true
                }
            ],
            "directors": [
                {
                    "name": "Bob Director",
                    "role": "executive_director",
                    "appointment_date": "2020-01-15"
                }
            ]
        }"#;

        let intent: KycIntent = serde_json::from_str(json).unwrap();
        assert_eq!(intent.intent_name(), "onboard_corporate");
        
        match intent {
            KycIntent::OnboardCorporate { client, beneficial_owners, directors, .. } => {
                assert_eq!(client.name, "Acme Holdings Ltd");
                assert_eq!(beneficial_owners.len(), 1);
                assert_eq!(beneficial_owners[0].ownership_percentage, 51.0);
                assert_eq!(directors.len(), 1);
                assert_eq!(directors[0].name, "Bob Director");
            }
            _ => panic!("Wrong intent type"),
        }
    }

    #[test]
    fn test_deserialize_add_document() {
        let json = r#"{
            "intent": "add_document",
            "cbu_id": { "id": "550e8400-e29b-41d4-a716-446655440000" },
            "document": { "document_type": "PASSPORT_USA" },
            "extract_attributes": true
        }"#;

        let intent: KycIntent = serde_json::from_str(json).unwrap();
        
        match intent {
            KycIntent::AddDocument { cbu_id, document, extract_attributes } => {
                assert!(matches!(cbu_id, CbuReference::ById { .. }));
                assert_eq!(document.document_type, "PASSPORT_USA");
                assert!(extract_attributes);
            }
            _ => panic!("Wrong intent type"),
        }
    }

    #[test]
    fn test_cbu_reference_by_code() {
        let json = r#"{
            "intent": "add_document",
            "cbu_id": { "code": "CBU-2024-001" },
            "document": { "document_type": "BANK_STATEMENT" }
        }"#;

        let intent: KycIntent = serde_json::from_str(json).unwrap();
        
        match intent {
            KycIntent::AddDocument { cbu_id, .. } => {
                match cbu_id {
                    CbuReference::ByCode { code } => assert_eq!(code, "CBU-2024-001"),
                    _ => panic!("Expected ByCode reference"),
                }
            }
            _ => panic!("Wrong intent type"),
        }
    }

    #[test]
    fn test_invalid_intent_fails() {
        let json = r#"{
            "intent": "unknown_intent",
            "foo": "bar"
        }"#;

        let result: Result<KycIntent, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }
}
