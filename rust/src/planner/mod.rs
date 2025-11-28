//! Planner: Intent → DSL Source
//!
//! The planner converts semantic intents into DSL source code.
//! This is a PURE FUNCTION: same intent always produces same DSL.
//!
//! # Architecture
//!
//! ```text
//! Intent (JSON)     Planner (this module)     DSL Source
//! ─────────────     ─────────────────────     ──────────
//! OnboardIndividual → plan_individual() →     (cbu.create ...)
//!                                             (document.catalog ...)
//! ```
//!
//! # Design Principles
//!
//! 1. **Deterministic**: Same input = same output, always
//! 2. **Business Logic Lives Here**: Jurisdiction rules, workflow variations
//! 3. **DSL is Assembly**: Planner handles high-level, DSL handles low-level
//! 4. **Testable**: No DB, no side effects, pure functions

mod dsl_builder;

use crate::intent::*;
pub use dsl_builder::DslBuilder;

// ============================================================================
// Planner Errors
// ============================================================================

#[derive(Debug, Clone)]
pub enum PlanError {
    /// Missing required field
    MissingField { field: String },
    /// Invalid reference
    InvalidReference { reference: String, reason: String },
    /// Business rule violation
    BusinessRule { rule: String, message: String },
    /// Unsupported intent/configuration
    Unsupported { message: String },
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanError::MissingField { field } => write!(f, "Missing required field: {}", field),
            PlanError::InvalidReference { reference, reason } => {
                write!(f, "Invalid reference '{}': {}", reference, reason)
            }
            PlanError::BusinessRule { rule, message } => {
                write!(f, "Business rule '{}' violated: {}", rule, message)
            }
            PlanError::Unsupported { message } => write!(f, "Unsupported: {}", message),
        }
    }
}

impl std::error::Error for PlanError {}

// ============================================================================
// Planner Configuration
// ============================================================================

/// Configuration for the planner
#[derive(Debug, Clone, Default)]
pub struct PlannerConfig {
    /// Default jurisdiction if not specified
    pub default_jurisdiction: Option<String>,
    
    /// Whether to auto-extract attributes from documents
    pub auto_extract: bool,
    
    /// UBO threshold percentage (default 25%)
    pub ubo_threshold: f64,
}

impl PlannerConfig {
    pub fn new() -> Self {
        Self {
            default_jurisdiction: None,
            auto_extract: true,
            ubo_threshold: 25.0,
        }
    }
}

// ============================================================================
// Planner
// ============================================================================

/// The planner: converts intents to DSL source
pub struct Planner {
    config: PlannerConfig,
}

impl Planner {
    pub fn new() -> Self {
        Self {
            config: PlannerConfig::new(),
        }
    }

    pub fn with_config(config: PlannerConfig) -> Self {
        Self { config }
    }

    /// Plan an intent into DSL source.
    /// This is the main entry point.
    pub fn plan(&self, intent: &KycIntent) -> Result<String, PlanError> {
        match intent {
            KycIntent::OnboardIndividual { client, documents, contact } => {
                self.plan_onboard_individual(client, documents, contact.as_ref())
            }
            KycIntent::OnboardCorporate { 
                client, 
                documents, 
                beneficial_owners, 
                directors,
                authorized_signatories 
            } => {
                self.plan_onboard_corporate(
                    client, 
                    documents, 
                    beneficial_owners, 
                    directors,
                    authorized_signatories
                )
            }
            KycIntent::OnboardFund { fund, documents, management_company } => {
                self.plan_onboard_fund(fund, documents, management_company.as_ref())
            }
            KycIntent::AddDocument { cbu_id, document, extract_attributes } => {
                self.plan_add_document(cbu_id, document, *extract_attributes)
            }
            KycIntent::AddDocuments { cbu_id, documents, extract_attributes } => {
                self.plan_add_documents(cbu_id, documents, *extract_attributes)
            }
            KycIntent::ExtractDocument { document_id } => {
                self.plan_extract_document(document_id)
            }
            KycIntent::AddBeneficialOwner { cbu_id, owner, evidence_documents } => {
                self.plan_add_beneficial_owner(cbu_id, owner, evidence_documents)
            }
            KycIntent::AddDirector { cbu_id, director, evidence_documents } => {
                self.plan_add_director(cbu_id, director, evidence_documents)
            }
            KycIntent::AddSignatory { cbu_id, signatory, evidence_documents } => {
                self.plan_add_signatory(cbu_id, signatory, evidence_documents)
            }
            KycIntent::LinkDocumentToEntity { document_id, entity_id } => {
                self.plan_link_document_to_entity(document_id, entity_id)
            }
            KycIntent::RunKycChecks { cbu_id, check_types } => {
                self.plan_run_kyc_checks(cbu_id, check_types)
            }
            KycIntent::ValidateAttributes { cbu_id, attribute_codes } => {
                self.plan_validate_attributes(cbu_id, attribute_codes)
            }
            KycIntent::UpdateRiskRating { cbu_id, risk_rating, rationale } => {
                self.plan_update_risk_rating(cbu_id, risk_rating, rationale.as_deref())
            }
            KycIntent::GetCbuStatus { cbu_id } => {
                self.plan_get_cbu_status(cbu_id)
            }
            KycIntent::ListDocuments { cbu_id, document_type_filter } => {
                self.plan_list_documents(cbu_id, document_type_filter.as_deref())
            }
            KycIntent::ListBeneficialOwners { cbu_id } => {
                self.plan_list_beneficial_owners(cbu_id)
            }
            KycIntent::LinkCorporateStructure { 
                parent_entity, 
                subsidiary_entity, 
                ownership_percentage,
                relationship_type 
            } => {
                self.plan_link_corporate_structure(
                    parent_entity, 
                    subsidiary_entity, 
                    *ownership_percentage,
                    relationship_type
                )
            }
        }
    }

    // =========================================================================
    // Individual Onboarding
    // =========================================================================

    fn plan_onboard_individual(
        &self,
        client: &IndividualClient,
        documents: &[DocumentSpec],
        _contact: Option<&ContactInfo>,
    ) -> Result<String, PlanError> {
        let mut b = DslBuilder::new();

        // 1. Create CBU
        let jurisdiction = client.jurisdiction.as_deref()
            .or(self.config.default_jurisdiction.as_deref());
        
        b.cbu_create(&client.name, "individual", jurisdiction, "@cbu");

        // 2. Create natural person entity and link
        b.entity_create_natural_person(&client.name, "@person");
        b.cbu_assign_role("@cbu", "@person", "account_holder");

        // 3. Add documents
        for (i, doc) in documents.iter().enumerate() {
            let binding = format!("@doc{}", i);
            b.document_catalog(&doc.document_type, "@cbu", doc.title.as_deref(), &binding);
            
            if doc.extract_attributes && self.config.auto_extract {
                b.document_extract(&binding);
            }
            
            // Link document to person entity
            b.document_link_entity(&binding, "@person");
        }

        // 4. Add jurisdiction-specific requirements
        if let Some(j) = jurisdiction {
            self.add_jurisdiction_requirements(&mut b, j, "individual");
        }

        Ok(b.build())
    }

    // =========================================================================
    // Corporate Onboarding
    // =========================================================================

    fn plan_onboard_corporate(
        &self,
        client: &CorporateClient,
        documents: &[DocumentSpec],
        beneficial_owners: &[BeneficialOwnerSpec],
        directors: &[DirectorSpec],
        signatories: &[SignatorySpec],
    ) -> Result<String, PlanError> {
        let mut b = DslBuilder::new();

        // 1. Create CBU
        let jurisdiction = client.jurisdiction.as_deref()
            .or(self.config.default_jurisdiction.as_deref());
        
        b.cbu_create(&client.name, "corporate", jurisdiction, "@cbu");

        // 2. Create corporate entity and link
        let entity_type = match &client.entity_type {
            Some(CorporateEntityType::LimitedCompany) => "limited-company",
            Some(CorporateEntityType::PublicLimitedCompany) => "public-limited-company",
            Some(CorporateEntityType::Partnership) => "partnership",
            Some(CorporateEntityType::Trust) => "trust",
            _ => "limited-company",
        };
        
        b.entity_create_corporate(&client.name, entity_type, "@company");
        b.cbu_assign_role("@cbu", "@company", "account_holder");

        // Set registration number if provided
        if let Some(reg_num) = &client.registration_number {
            b.entity_set_attribute("@company", "registration_number", reg_num);
        }

        // 3. Add documents (cert of incorporation by default for corporate)
        let has_cert = documents.iter()
            .any(|d| d.document_type.contains("CERT_OF_INCORPORATION"));
        
        if !has_cert && documents.is_empty() {
            b.document_catalog("CERT_OF_INCORPORATION", "@cbu", None, "@doc_cert");
            b.document_extract("@doc_cert");
        }

        for (i, doc) in documents.iter().enumerate() {
            let binding = format!("@doc{}", i);
            b.document_catalog(&doc.document_type, "@cbu", doc.title.as_deref(), &binding);
            
            if doc.extract_attributes && self.config.auto_extract {
                b.document_extract(&binding);
            }
        }

        // 4. Add beneficial owners
        for (i, owner) in beneficial_owners.iter().enumerate() {
            let binding = format!("@ubo{}", i);
            b.entity_create_natural_person(&owner.name, &binding);
            
            // Link as UBO with ownership percentage
            b.cbu_assign_role_with_ownership(
                "@cbu", 
                &binding, 
                "beneficial_owner",
                owner.ownership_percentage
            );
            
            // Set additional attributes
            if let Some(nat) = &owner.nationality {
                b.entity_set_attribute(&binding, "nationality", nat);
            }
        }

        // 5. Add directors
        for (i, director) in directors.iter().enumerate() {
            let binding = format!("@director{}", i);
            b.entity_create_natural_person(&director.name, &binding);
            
            let role = match &director.role {
                Some(DirectorRole::Chairman) => "chairman",
                Some(DirectorRole::ExecutiveDirector) => "executive_director",
                Some(DirectorRole::NonExecutiveDirector) => "non_executive_director",
                _ => "director",
            };
            
            b.cbu_assign_role("@cbu", &binding, role);
        }

        // 6. Add signatories
        for (i, signatory) in signatories.iter().enumerate() {
            let binding = format!("@signatory{}", i);
            b.entity_create_natural_person(&signatory.name, &binding);
            b.cbu_assign_role("@cbu", &binding, "authorized_signatory");
        }

        // 7. Jurisdiction-specific requirements
        if let Some(j) = jurisdiction {
            self.add_jurisdiction_requirements(&mut b, j, "corporate");
        }

        Ok(b.build())
    }

    // =========================================================================
    // Fund Onboarding
    // =========================================================================

    fn plan_onboard_fund(
        &self,
        fund: &FundClient,
        documents: &[DocumentSpec],
        management_company: Option<&EntityReference>,
    ) -> Result<String, PlanError> {
        let mut b = DslBuilder::new();

        let jurisdiction = fund.jurisdiction.as_deref()
            .or(self.config.default_jurisdiction.as_deref());
        
        b.cbu_create(&fund.name, "fund", jurisdiction, "@cbu");

        // Create fund entity
        let fund_type = match &fund.fund_type {
            Some(FundType::Ucits) => "ucits-fund",
            Some(FundType::Aif) => "aif-fund",
            Some(FundType::Etf) => "etf-fund",
            Some(FundType::HedgeFund) => "hedge-fund",
            _ => "investment-fund",
        };
        
        b.entity_create_fund(&fund.name, fund_type, "@fund");
        b.cbu_assign_role("@cbu", "@fund", "account_holder");

        // Link management company if provided
        if let Some(mgmt) = management_company {
            let mgmt_ref = self.resolve_entity_reference(mgmt);
            b.cbu_assign_role("@cbu", &mgmt_ref, "management_company");
        }

        // Add documents
        for (i, doc) in documents.iter().enumerate() {
            let binding = format!("@doc{}", i);
            b.document_catalog(&doc.document_type, "@cbu", doc.title.as_deref(), &binding);
            
            if doc.extract_attributes {
                b.document_extract(&binding);
            }
        }

        Ok(b.build())
    }

    // =========================================================================
    // Document Operations
    // =========================================================================

    fn plan_add_document(
        &self,
        cbu_id: &CbuReference,
        document: &DocumentSpec,
        extract_attributes: bool,
    ) -> Result<String, PlanError> {
        let mut b = DslBuilder::new();

        let cbu_ref = self.resolve_cbu_reference(cbu_id);
        
        b.document_catalog(&document.document_type, &cbu_ref, document.title.as_deref(), "@doc");
        
        if extract_attributes && self.config.auto_extract {
            b.document_extract("@doc");
        }

        Ok(b.build())
    }

    fn plan_add_documents(
        &self,
        cbu_id: &CbuReference,
        documents: &[DocumentSpec],
        extract_attributes: bool,
    ) -> Result<String, PlanError> {
        let mut b = DslBuilder::new();

        let cbu_ref = self.resolve_cbu_reference(cbu_id);
        
        for (i, doc) in documents.iter().enumerate() {
            let binding = format!("@doc{}", i);
            b.document_catalog(&doc.document_type, &cbu_ref, doc.title.as_deref(), &binding);
            
            if extract_attributes && self.config.auto_extract {
                b.document_extract(&binding);
            }
        }

        Ok(b.build())
    }

    fn plan_extract_document(&self, document_id: &DocumentReference) -> Result<String, PlanError> {
        let mut b = DslBuilder::new();
        let doc_ref = self.resolve_document_reference(document_id);
        b.document_extract(&doc_ref);
        Ok(b.build())
    }

    fn plan_link_document_to_entity(
        &self,
        document_id: &DocumentReference,
        entity_id: &EntityReference,
    ) -> Result<String, PlanError> {
        let mut b = DslBuilder::new();
        
        let doc_ref = self.resolve_document_reference(document_id);
        let entity_ref = self.resolve_entity_reference(entity_id);
        
        b.document_link_entity(&doc_ref, &entity_ref);
        
        Ok(b.build())
    }

    // =========================================================================
    // Entity/Role Operations
    // =========================================================================

    fn plan_add_beneficial_owner(
        &self,
        cbu_id: &CbuReference,
        owner: &BeneficialOwnerSpec,
        evidence_documents: &[DocumentSpec],
    ) -> Result<String, PlanError> {
        let mut b = DslBuilder::new();

        let cbu_ref = self.resolve_cbu_reference(cbu_id);

        // Create entity
        b.entity_create_natural_person(&owner.name, "@ubo");
        
        // Link with ownership
        b.cbu_assign_role_with_ownership(&cbu_ref, "@ubo", "beneficial_owner", owner.ownership_percentage);
        
        // Set attributes
        if let Some(nat) = &owner.nationality {
            b.entity_set_attribute("@ubo", "nationality", nat);
        }
        if let Some(dob) = &owner.date_of_birth {
            b.entity_set_attribute("@ubo", "date_of_birth", dob);
        }

        // Add evidence documents
        for (i, doc) in evidence_documents.iter().enumerate() {
            let binding = format!("@ubo_doc{}", i);
            b.document_catalog(&doc.document_type, &cbu_ref, doc.title.as_deref(), &binding);
            b.document_link_entity(&binding, "@ubo");
            
            if doc.extract_attributes {
                b.document_extract(&binding);
            }
        }

        Ok(b.build())
    }

    fn plan_add_director(
        &self,
        cbu_id: &CbuReference,
        director: &DirectorSpec,
        evidence_documents: &[DocumentSpec],
    ) -> Result<String, PlanError> {
        let mut b = DslBuilder::new();

        let cbu_ref = self.resolve_cbu_reference(cbu_id);

        b.entity_create_natural_person(&director.name, "@director");
        
        let role = match &director.role {
            Some(DirectorRole::Chairman) => "chairman",
            Some(DirectorRole::ExecutiveDirector) => "executive_director",
            Some(DirectorRole::NonExecutiveDirector) => "non_executive_director",
            _ => "director",
        };
        
        b.cbu_assign_role(&cbu_ref, "@director", role);

        // Add evidence documents
        for (i, doc) in evidence_documents.iter().enumerate() {
            let binding = format!("@dir_doc{}", i);
            b.document_catalog(&doc.document_type, &cbu_ref, doc.title.as_deref(), &binding);
            b.document_link_entity(&binding, "@director");
        }

        Ok(b.build())
    }

    fn plan_add_signatory(
        &self,
        cbu_id: &CbuReference,
        signatory: &SignatorySpec,
        evidence_documents: &[DocumentSpec],
    ) -> Result<String, PlanError> {
        let mut b = DslBuilder::new();

        let cbu_ref = self.resolve_cbu_reference(cbu_id);

        b.entity_create_natural_person(&signatory.name, "@signatory");
        b.cbu_assign_role(&cbu_ref, "@signatory", "authorized_signatory");

        for (i, doc) in evidence_documents.iter().enumerate() {
            let binding = format!("@sig_doc{}", i);
            b.document_catalog(&doc.document_type, &cbu_ref, doc.title.as_deref(), &binding);
            b.document_link_entity(&binding, "@signatory");
        }

        Ok(b.build())
    }

    fn plan_link_corporate_structure(
        &self,
        parent_entity: &EntityReference,
        subsidiary_entity: &EntityReference,
        ownership_percentage: Option<f64>,
        relationship_type: &CorporateRelationshipType,
    ) -> Result<String, PlanError> {
        let mut b = DslBuilder::new();

        let parent_ref = self.resolve_entity_reference(parent_entity);
        let subsidiary_ref = self.resolve_entity_reference(subsidiary_entity);
        
        let rel_type = match relationship_type {
            CorporateRelationshipType::ParentSubsidiary => "parent_subsidiary",
            CorporateRelationshipType::Associate => "associate",
            CorporateRelationshipType::JointVenture => "joint_venture",
            CorporateRelationshipType::ControlledEntity => "controlled_entity",
            CorporateRelationshipType::BranchOffice => "branch_office",
        };

        b.entity_link_corporate_structure(&parent_ref, &subsidiary_ref, rel_type, ownership_percentage);

        Ok(b.build())
    }

    // =========================================================================
    // KYC Operations
    // =========================================================================

    fn plan_run_kyc_checks(
        &self,
        cbu_id: &CbuReference,
        check_types: &[KycCheckType],
    ) -> Result<String, PlanError> {
        let mut b = DslBuilder::new();

        let cbu_ref = self.resolve_cbu_reference(cbu_id);

        // If no check types specified, run all standard checks
        let checks = if check_types.is_empty() {
            vec![
                KycCheckType::SanctionsScreening,
                KycCheckType::PepScreening,
                KycCheckType::AdverseMedia,
            ]
        } else {
            check_types.to_vec()
        };

        for check in checks {
            let check_name = match check {
                KycCheckType::SanctionsScreening => "sanctions",
                KycCheckType::PepScreening => "pep",
                KycCheckType::AdverseMedia => "adverse_media",
                KycCheckType::IdVerification => "id_verification",
                KycCheckType::AddressVerification => "address_verification",
                KycCheckType::DocumentAuthenticity => "document_authenticity",
                KycCheckType::UboVerification => "ubo_verification",
                KycCheckType::CompanyRegistryCheck => "company_registry",
            };
            b.kyc_run_check(&cbu_ref, check_name);
        }

        Ok(b.build())
    }

    fn plan_validate_attributes(
        &self,
        cbu_id: &CbuReference,
        attribute_codes: &[String],
    ) -> Result<String, PlanError> {
        let mut b = DslBuilder::new();

        let cbu_ref = self.resolve_cbu_reference(cbu_id);

        if attribute_codes.is_empty() {
            // Validate all extracted attributes
            b.kyc_validate_all_attributes(&cbu_ref);
        } else {
            for attr in attribute_codes {
                b.kyc_validate_attribute(&cbu_ref, attr);
            }
        }

        Ok(b.build())
    }

    fn plan_update_risk_rating(
        &self,
        cbu_id: &CbuReference,
        risk_rating: &RiskRating,
        rationale: Option<&str>,
    ) -> Result<String, PlanError> {
        let mut b = DslBuilder::new();

        let cbu_ref = self.resolve_cbu_reference(cbu_id);
        
        let rating = match risk_rating {
            RiskRating::Low => "low",
            RiskRating::Medium => "medium",
            RiskRating::High => "high",
            RiskRating::Prohibited => "prohibited",
        };

        b.cbu_set_risk_rating(&cbu_ref, rating, rationale);

        Ok(b.build())
    }

    // =========================================================================
    // Query Operations
    // =========================================================================

    fn plan_get_cbu_status(&self, cbu_id: &CbuReference) -> Result<String, PlanError> {
        let mut b = DslBuilder::new();
        let cbu_ref = self.resolve_cbu_reference(cbu_id);
        b.cbu_get_status(&cbu_ref);
        Ok(b.build())
    }

    fn plan_list_documents(
        &self,
        cbu_id: &CbuReference,
        document_type_filter: Option<&str>,
    ) -> Result<String, PlanError> {
        let mut b = DslBuilder::new();
        let cbu_ref = self.resolve_cbu_reference(cbu_id);
        b.document_list(&cbu_ref, document_type_filter);
        Ok(b.build())
    }

    fn plan_list_beneficial_owners(&self, cbu_id: &CbuReference) -> Result<String, PlanError> {
        let mut b = DslBuilder::new();
        let cbu_ref = self.resolve_cbu_reference(cbu_id);
        b.cbu_list_ubos(&cbu_ref);
        Ok(b.build())
    }

    // =========================================================================
    // Helpers
    // =========================================================================

    fn resolve_cbu_reference(&self, cbu_ref: &CbuReference) -> String {
        match cbu_ref {
            CbuReference::ById { id } => format!("\"{}\"", id),
            CbuReference::ByCode { code } => format!("\"{}\"", code),
            CbuReference::ByBinding { binding } => binding.clone(),
        }
    }

    fn resolve_document_reference(&self, doc_ref: &DocumentReference) -> String {
        match doc_ref {
            DocumentReference::ById { id } => format!("\"{}\"", id),
            DocumentReference::ByCode { code } => format!("\"{}\"", code),
            DocumentReference::ByBinding { binding } => binding.clone(),
        }
    }

    fn resolve_entity_reference(&self, entity_ref: &EntityReference) -> String {
        match entity_ref {
            EntityReference::ById { id } => format!("\"{}\"", id),
            EntityReference::ByCode { code } => format!("\"{}\"", code),
            EntityReference::ByBinding { binding } => binding.clone(),
            EntityReference::CreateOrLookup { name, .. } => {
                // For CreateOrLookup, we generate inline lookup
                format!("(entity.lookup-or-create :name \"{}\")", name)
            }
        }
    }

    fn add_jurisdiction_requirements(&self, b: &mut DslBuilder, jurisdiction: &str, client_type: &str) {
        match (jurisdiction, client_type) {
            ("UK", "corporate") => {
                // UK requires Companies House verification
                b.comment("UK corporate: Companies House verification required");
                b.kyc_run_check("@cbu", "company_registry");
            }
            ("UK", "individual") => {
                // UK requires electoral roll check
                b.comment("UK individual: Electoral roll verification");
            }
            ("US", _) => {
                // US requires OFAC screening
                b.comment("US jurisdiction: Enhanced OFAC screening");
                b.kyc_run_check("@cbu", "ofac_screening");
            }
            ("CH", _) => {
                // Switzerland requires enhanced due diligence
                b.comment("Swiss jurisdiction: FINMA enhanced due diligence");
            }
            _ => {}
        }
    }
}

impl Default for Planner {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_individual_onboarding() {
        let planner = Planner::new();
        
        let intent = KycIntent::OnboardIndividual {
            client: IndividualClient {
                name: "John Smith".to_string(),
                jurisdiction: Some("UK".to_string()),
                nationality: Some("GBR".to_string()),
                date_of_birth: None,
                tax_residency: None,
                occupation: None,
                source_of_wealth: None,
            },
            documents: vec![
                DocumentSpec {
                    document_type: "PASSPORT_GBR".to_string(),
                    title: None,
                    file_reference: None,
                    extract_attributes: true,
                    issue_date: None,
                    expiry_date: None,
                    issuing_authority: None,
                },
            ],
            contact: None,
        };

        let dsl = planner.plan(&intent).unwrap();
        println!("Generated DSL:\n{}", dsl);
        
        assert!(dsl.contains("cbu.create"));
        assert!(dsl.contains("John Smith"));
        assert!(dsl.contains("PASSPORT_GBR"));
        assert!(dsl.contains("document.extract"));
    }

    #[test]
    fn test_plan_corporate_with_ubos() {
        let planner = Planner::new();
        
        let intent = KycIntent::OnboardCorporate {
            client: CorporateClient {
                name: "Acme Holdings Ltd".to_string(),
                jurisdiction: Some("UK".to_string()),
                registration_number: Some("12345678".to_string()),
                entity_type: Some(CorporateEntityType::LimitedCompany),
                incorporation_date: None,
                registered_address: None,
                trading_address: None,
                industry_sector: None,
                lei_code: None,
            },
            documents: vec![],
            beneficial_owners: vec![
                BeneficialOwnerSpec {
                    name: "Jane Owner".to_string(),
                    ownership_percentage: 51.0,
                    nationality: Some("GBR".to_string()),
                    date_of_birth: None,
                    is_direct: true,
                    via_entity: None,
                    control_type: None,
                },
            ],
            directors: vec![
                DirectorSpec {
                    name: "Bob Director".to_string(),
                    role: Some(DirectorRole::ExecutiveDirector),
                    nationality: None,
                    date_of_birth: None,
                    appointment_date: None,
                    is_executive: Some(true),
                },
            ],
            authorized_signatories: vec![],
        };

        let dsl = planner.plan(&intent).unwrap();
        println!("Corporate DSL:\n{}", dsl);
        
        assert!(dsl.contains("Acme Holdings"));
        assert!(dsl.contains("beneficial_owner"));
        assert!(dsl.contains("51.0"));  // ownership percentage
        assert!(dsl.contains("executive_director"));
    }

    #[test]
    fn test_deterministic_output() {
        let planner = Planner::new();
        
        let intent = KycIntent::OnboardIndividual {
            client: IndividualClient {
                name: "Test Client".to_string(),
                jurisdiction: Some("UK".to_string()),
                nationality: None,
                date_of_birth: None,
                tax_residency: None,
                occupation: None,
                source_of_wealth: None,
            },
            documents: vec![],
            contact: None,
        };

        let dsl1 = planner.plan(&intent).unwrap();
        let dsl2 = planner.plan(&intent).unwrap();
        
        assert_eq!(dsl1, dsl2, "Same intent must produce identical DSL");
    }
}
