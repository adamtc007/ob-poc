package vocabulary

import (
	"context"
	"fmt"
	"time"
)

// MigrationServiceImpl implements the MigrationService interface
type MigrationServiceImpl struct {
	repo    Repository
	service VocabularyService
}

// NewMigrationService creates a new migration service
func NewMigrationService(repo Repository, service VocabularyService) MigrationService {
	return &MigrationServiceImpl{
		repo:    repo,
		service: service,
	}
}

// =============================================================================
// Hardcoded Vocabulary Extraction
// =============================================================================

// extractOnboardingVocabulary extracts verbs from the hardcoded onboarding vocabulary
func (m *MigrationServiceImpl) extractOnboardingVocabulary() []DomainVocabulary {
	var vocabs []DomainVocabulary

	// Case Management Verbs
	vocabs = append(vocabs, []DomainVocabulary{
		{
			Verb:        "case.create",
			Category:    stringPtr("case_management"),
			Description: stringPtr("Create a new onboarding case"),
			Parameters: map[string]interface{}{
				"cbu_id":         VerbParameter{Name: "cbu_id", Type: "string", Required: true, Description: "Client Business Unit identifier"},
				"nature_purpose": VerbParameter{Name: "nature_purpose", Type: "string", Required: true, Description: "Nature and purpose of the business relationship"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Create UCITS fund case", Usage: `(case.create (cbu.id "CBU-1234") (nature-purpose "UCITS equity fund domiciled in LU"))`, Context: "Fund onboarding"},
			},
			Active:  true,
			Version: "1.0.0",
		},
		{
			Verb:        "case.update",
			Category:    stringPtr("case_management"),
			Description: stringPtr("Update an existing onboarding case"),
			Parameters: map[string]interface{}{
				"cbu_id": VerbParameter{Name: "cbu_id", Type: "string", Required: true, Description: "Client Business Unit identifier"},
				"status": VerbParameter{Name: "status", Type: "string", Required: true, Description: "New status for the case"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Update case status", Usage: `(case.update (cbu.id "CBU-1234") (status "IN_PROGRESS"))`, Context: "Status management"},
			},
			Active:  true,
			Version: "1.0.0",
		},
		{
			Verb:        "case.validate",
			Category:    stringPtr("case_management"),
			Description: stringPtr("Validate case requirements"),
			Parameters: map[string]interface{}{
				"requirements": VerbParameter{Name: "requirements", Type: "string", Required: true, Description: "Requirements to validate"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Validate case requirements", Usage: `(case.validate (requirements "KYC_COMPLETE"))`, Context: "Validation workflow"},
			},
			Active:  true,
			Version: "1.0.0",
		},
		{
			Verb:        "case.approve",
			Category:    stringPtr("case_management"),
			Description: stringPtr("Approve an onboarding case"),
			Parameters: map[string]interface{}{
				"approver_id": VerbParameter{Name: "approver_id", Type: "string", Required: true, Description: "ID of the approving user"},
				"timestamp":   VerbParameter{Name: "timestamp", Type: "string", Required: true, Description: "Approval timestamp"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Approve case", Usage: `(case.approve (approver.id "USER-123") (timestamp "2024-01-15T10:30:00Z"))`, Context: "Case approval"},
			},
			Active:  true,
			Version: "1.0.0",
		},
		{
			Verb:        "case.close",
			Category:    stringPtr("case_management"),
			Description: stringPtr("Close an onboarding case"),
			Parameters: map[string]interface{}{
				"reason":      VerbParameter{Name: "reason", Type: "string", Required: true, Description: "Reason for closing"},
				"final_state": VerbParameter{Name: "final_state", Type: "string", Required: true, Description: "Final state of the case"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Close case", Usage: `(case.close (reason "Completed successfully") (final-state "COMPLETED"))`, Context: "Case closure"},
			},
			Active:  true,
			Version: "1.0.0",
		},
	}...)

	// Entity Identity Verbs
	vocabs = append(vocabs, []DomainVocabulary{
		{
			Verb:        "entity.register",
			Category:    stringPtr("entity_identity"),
			Description: stringPtr("Register a new entity"),
			Parameters: map[string]interface{}{
				"type":         VerbParameter{Name: "type", Type: "enum", Required: true, Description: "Entity type", EnumValues: []string{"PROPER_PERSON", "CORPORATE", "TRUST", "PARTNERSHIP"}},
				"jurisdiction": VerbParameter{Name: "jurisdiction", Type: "string", Required: true, Description: "Legal jurisdiction"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Register corporate entity", Usage: `(entity.register (type "CORPORATE") (jurisdiction "LU"))`, Context: "Entity registration"},
			},
			Active:  true,
			Version: "1.0.0",
		},
		{
			Verb:        "entity.classify",
			Category:    stringPtr("entity_identity"),
			Description: stringPtr("Classify entity by category and risk"),
			Parameters: map[string]interface{}{
				"category":   VerbParameter{Name: "category", Type: "string", Required: true, Description: "Entity category"},
				"risk_level": VerbParameter{Name: "risk_level", Type: "enum", Required: true, Description: "Risk assessment", EnumValues: []string{"LOW", "MEDIUM", "HIGH", "PROHIBITED"}},
			},
			Examples: []interface{}{
				VerbExample{Description: "Classify entity", Usage: `(entity.classify (category "FUND_MANAGER") (risk-level "MEDIUM"))`, Context: "Risk classification"},
			},
			Active:  true,
			Version: "1.0.0",
		},
		{
			Verb:        "entity.link",
			Category:    stringPtr("entity_identity"),
			Description: stringPtr("Link entities with relationships"),
			Parameters: map[string]interface{}{
				"parent_id":    VerbParameter{Name: "parent_id", Type: "string", Required: true, Description: "Parent entity identifier"},
				"relationship": VerbParameter{Name: "relationship", Type: "string", Required: true, Description: "Type of relationship"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Link subsidiary", Usage: `(entity.link (parent.id "ENTITY-123") (relationship "SUBSIDIARY"))`, Context: "Entity relationships"},
			},
			Active:  true,
			Version: "1.0.0",
		},
		{
			Verb:        "identity.verify",
			Category:    stringPtr("entity_identity"),
			Description: stringPtr("Verify entity identity"),
			Parameters: map[string]interface{}{
				"document_id": VerbParameter{Name: "document_id", Type: "string", Required: true, Description: "Identity document identifier"},
				"status":      VerbParameter{Name: "status", Type: "enum", Required: true, Description: "Verification status", EnumValues: []string{"PENDING", "VERIFIED", "REJECTED"}},
			},
			Examples: []interface{}{
				VerbExample{Description: "Verify identity", Usage: `(identity.verify (document.id "DOC-123") (status "VERIFIED"))`, Context: "Identity verification"},
			},
			Active:  true,
			Version: "1.0.0",
		},
		{
			Verb:        "identity.attest",
			Category:    stringPtr("entity_identity"),
			Description: stringPtr("Attest identity by authorized signatory"),
			Parameters: map[string]interface{}{
				"signatory_id": VerbParameter{Name: "signatory_id", Type: "string", Required: true, Description: "Authorized signatory identifier"},
				"capacity":     VerbParameter{Name: "capacity", Type: "string", Required: true, Description: "Signatory capacity/role"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Attest identity", Usage: `(identity.attest (signatory.id "SIGN-123") (capacity "DIRECTOR"))`, Context: "Identity attestation"},
			},
			Active:  true,
			Version: "1.0.0",
		},
	}...)

	// Product Service Verbs
	vocabs = append(vocabs, []DomainVocabulary{
		{
			Verb:        "products.add",
			Category:    stringPtr("product_service"),
			Description: stringPtr("Add products to onboarding case"),
			Parameters: map[string]interface{}{
				"products": VerbParameter{Name: "products", Type: "array", Required: true, Description: "List of product identifiers"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Add multiple products", Usage: `(products.add "CUSTODY" "FUND_ACCOUNTING")`, Context: "Product selection"},
			},
			Active:  true,
			Version: "1.0.0",
		},
		{
			Verb:        "products.configure",
			Category:    stringPtr("product_service"),
			Description: stringPtr("Configure product settings"),
			Parameters: map[string]interface{}{
				"product":     VerbParameter{Name: "product", Type: "string", Required: true, Description: "Product identifier"},
				"settings_id": VerbParameter{Name: "settings_id", Type: "string", Required: true, Description: "Configuration settings identifier"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Configure product", Usage: `(products.configure (product "CUSTODY") (settings.id "SETTINGS-123"))`, Context: "Product configuration"},
			},
			Active:  true,
			Version: "1.0.0",
		},
		{
			Verb:        "services.discover",
			Category:    stringPtr("product_service"),
			Description: stringPtr("Discover available services for products"),
			Parameters: map[string]interface{}{
				"for_product": VerbParameter{Name: "for_product", Type: "string", Required: true, Description: "Product to discover services for"},
				"services":    VerbParameter{Name: "services", Type: "array", Required: true, Description: "List of discovered services"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Discover services", Usage: `(services.discover (for.product "CUSTODY" (service "SETTLEMENT") (service "SAFEKEEPING")))`, Context: "Service discovery"},
			},
			Active:  true,
			Version: "1.0.0",
		},
		{
			Verb:        "services.provision",
			Category:    stringPtr("product_service"),
			Description: stringPtr("Provision a service"),
			Parameters: map[string]interface{}{
				"service_id": VerbParameter{Name: "service_id", Type: "string", Required: true, Description: "Service identifier"},
				"config_id":  VerbParameter{Name: "config_id", Type: "string", Required: true, Description: "Service configuration identifier"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Provision service", Usage: `(services.provision (service.id "SERVICE-123") (config.id "CONFIG-456"))`, Context: "Service provisioning"},
			},
			Active:  true,
			Version: "1.0.0",
		},
		{
			Verb:        "services.activate",
			Category:    stringPtr("product_service"),
			Description: stringPtr("Activate a provisioned service"),
			Parameters: map[string]interface{}{
				"service_id":     VerbParameter{Name: "service_id", Type: "string", Required: true, Description: "Service identifier"},
				"effective_date": VerbParameter{Name: "effective_date", Type: "date", Required: true, Description: "Service activation date"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Activate service", Usage: `(services.activate (service.id "SERVICE-123") (effective-date "2024-02-01"))`, Context: "Service activation"},
			},
			Active:  true,
			Version: "1.0.0",
		},
	}...)

	// KYC Compliance Verbs
	vocabs = append(vocabs, []DomainVocabulary{
		{
			Verb:        "kyc.start",
			Category:    stringPtr("kyc_compliance"),
			Description: stringPtr("Start KYC process"),
			Parameters: map[string]interface{}{
				"documents":     VerbParameter{Name: "documents", Type: "array", Required: false, Description: "Required documents"},
				"jurisdictions": VerbParameter{Name: "jurisdictions", Type: "array", Required: false, Description: "Relevant jurisdictions"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Start KYC with documents", Usage: `(kyc.start (documents (document "CertificateOfIncorporation")) (jurisdictions (jurisdiction "LU")))`, Context: "KYC initiation"},
			},
			Active:  true,
			Version: "1.0.0",
		},
		{
			Verb:        "kyc.collect",
			Category:    stringPtr("kyc_compliance"),
			Description: stringPtr("Collect KYC document"),
			Parameters: map[string]interface{}{
				"document_id": VerbParameter{Name: "document_id", Type: "string", Required: true, Description: "Document identifier"},
				"type":        VerbParameter{Name: "type", Type: "string", Required: true, Description: "Document type"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Collect document", Usage: `(kyc.collect (document.id "DOC-123") (type "CertificateOfIncorporation"))`, Context: "Document collection"},
			},
			Active:  true,
			Version: "1.0.0",
		},
		{
			Verb:        "kyc.verify",
			Category:    stringPtr("kyc_compliance"),
			Description: stringPtr("Verify KYC document"),
			Parameters: map[string]interface{}{
				"document_id": VerbParameter{Name: "document_id", Type: "string", Required: true, Description: "Document identifier"},
				"verifier_id": VerbParameter{Name: "verifier_id", Type: "string", Required: true, Description: "Verifier identifier"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Verify document", Usage: `(kyc.verify (document.id "DOC-123") (verifier.id "VERIFIER-456"))`, Context: "Document verification"},
			},
			Active:  true,
			Version: "1.0.0",
		},
		{
			Verb:        "kyc.assess",
			Category:    stringPtr("kyc_compliance"),
			Description: stringPtr("Assess KYC risk"),
			Parameters: map[string]interface{}{
				"risk_rating":  VerbParameter{Name: "risk_rating", Type: "enum", Required: true, Description: "Risk rating", EnumValues: []string{"LOW", "MEDIUM", "HIGH"}},
				"rationale_id": VerbParameter{Name: "rationale_id", Type: "string", Required: true, Description: "Risk assessment rationale identifier"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Assess risk", Usage: `(kyc.assess (risk-rating "MEDIUM") (rationale.id "RATIONALE-123"))`, Context: "Risk assessment"},
			},
			Active:  true,
			Version: "1.0.0",
		},
		{
			Verb:        "compliance.screen",
			Category:    stringPtr("kyc_compliance"),
			Description: stringPtr("Screen against compliance lists"),
			Parameters: map[string]interface{}{
				"list":      VerbParameter{Name: "list", Type: "string", Required: true, Description: "Screening list identifier"},
				"result_id": VerbParameter{Name: "result_id", Type: "string", Required: true, Description: "Screening result identifier"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Screen entity", Usage: `(compliance.screen (list "SANCTIONS") (result.id "SCREEN-123"))`, Context: "Compliance screening"},
			},
			Active:  true,
			Version: "1.0.0",
		},
		{
			Verb:        "compliance.monitor",
			Category:    stringPtr("kyc_compliance"),
			Description: stringPtr("Set up ongoing compliance monitoring"),
			Parameters: map[string]interface{}{
				"trigger_id": VerbParameter{Name: "trigger_id", Type: "string", Required: true, Description: "Monitoring trigger identifier"},
				"frequency":  VerbParameter{Name: "frequency", Type: "string", Required: true, Description: "Monitoring frequency"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Set up monitoring", Usage: `(compliance.monitor (trigger.id "TRIGGER-123") (frequency "MONTHLY"))`, Context: "Ongoing monitoring"},
			},
			Active:  true,
			Version: "1.0.0",
		},
	}...)

	// Resource Infrastructure Verbs
	vocabs = append(vocabs, []DomainVocabulary{
		{
			Verb:        "resources.plan",
			Category:    stringPtr("resource_infrastructure"),
			Description: stringPtr("Plan resource creation"),
			Parameters: map[string]interface{}{
				"name":        VerbParameter{Name: "name", Type: "string", Required: true, Description: "Resource name"},
				"owner":       VerbParameter{Name: "owner", Type: "string", Required: true, Description: "Resource owner"},
				"var_attr_id": VerbParameter{Name: "var_attr_id", Type: "string", Required: true, Description: "Variable attribute identifier"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Plan custody account", Usage: `(resources.plan (resource.create "CustodyAccount" (owner "CustodyTech") (var (attr-id "uuid-123"))))`, Context: "Resource planning"},
			},
			Active:  true,
			Version: "1.0.0",
		},
		{
			Verb:        "resources.provision",
			Category:    stringPtr("resource_infrastructure"),
			Description: stringPtr("Provision a resource"),
			Parameters: map[string]interface{}{
				"resource_id": VerbParameter{Name: "resource_id", Type: "string", Required: true, Description: "Resource identifier"},
				"provider_id": VerbParameter{Name: "provider_id", Type: "string", Required: true, Description: "Provider identifier"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Provision resource", Usage: `(resources.provision (resource.id "RES-123") (provider.id "PROVIDER-456"))`, Context: "Resource provisioning"},
			},
			Active:  true,
			Version: "1.0.0",
		},
		{
			Verb:        "resources.configure",
			Category:    stringPtr("resource_infrastructure"),
			Description: stringPtr("Configure a resource"),
			Parameters: map[string]interface{}{
				"resource_id": VerbParameter{Name: "resource_id", Type: "string", Required: true, Description: "Resource identifier"},
				"config_id":   VerbParameter{Name: "config_id", Type: "string", Required: true, Description: "Configuration identifier"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Configure resource", Usage: `(resources.configure (resource.id "RES-123") (config.id "CONFIG-456"))`, Context: "Resource configuration"},
			},
			Active:  true,
			Version: "1.0.0",
		},
		{
			Verb:        "resources.test",
			Category:    stringPtr("resource_infrastructure"),
			Description: stringPtr("Test a resource"),
			Parameters: map[string]interface{}{
				"resource_id":   VerbParameter{Name: "resource_id", Type: "string", Required: true, Description: "Resource identifier"},
				"test_suite_id": VerbParameter{Name: "test_suite_id", Type: "string", Required: true, Description: "Test suite identifier"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Test resource", Usage: `(resources.test (resource.id "RES-123") (test-suite.id "SUITE-456"))`, Context: "Resource testing"},
			},
			Active:  true,
			Version: "1.0.0",
		},
		{
			Verb:        "resources.deploy",
			Category:    stringPtr("resource_infrastructure"),
			Description: stringPtr("Deploy a resource"),
			Parameters: map[string]interface{}{
				"resource_id": VerbParameter{Name: "resource_id", Type: "string", Required: true, Description: "Resource identifier"},
				"environment": VerbParameter{Name: "environment", Type: "enum", Required: true, Description: "Deployment environment", EnumValues: []string{"DEV", "TEST", "PROD"}},
			},
			Examples: []interface{}{
				VerbExample{Description: "Deploy resource", Usage: `(resources.deploy (resource.id "RES-123") (environment "PROD"))`, Context: "Resource deployment"},
			},
			Active:  true,
			Version: "1.0.0",
		},
	}...)

	// Attribute Data Verbs
	vocabs = append(vocabs, []DomainVocabulary{
		{
			Verb:        "attributes.define",
			Category:    stringPtr("attribute_data"),
			Description: stringPtr("Define a new attribute"),
			Parameters: map[string]interface{}{
				"attr_id":   VerbParameter{Name: "attr_id", Type: "string", Required: true, Description: "Attribute identifier"},
				"attr_type": VerbParameter{Name: "attr_type", Type: "string", Required: true, Description: "Attribute type"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Define attribute", Usage: `(attributes.define (attr.id "ATTR-123") (type "string"))`, Context: "Attribute definition"},
			},
			Active:  true,
			Version: "1.0.0",
		},
		{
			Verb:        "attributes.resolve",
			Category:    stringPtr("attribute_data"),
			Description: stringPtr("Resolve attribute from source"),
			Parameters: map[string]interface{}{
				"attr_id":   VerbParameter{Name: "attr_id", Type: "string", Required: true, Description: "Attribute identifier"},
				"source_id": VerbParameter{Name: "source_id", Type: "string", Required: true, Description: "Source identifier"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Resolve attribute", Usage: `(attributes.resolve (attr.id "ATTR-123") (source.id "SOURCE-456"))`, Context: "Attribute resolution"},
			},
			Active:  true,
			Version: "1.0.0",
		},
		{
			Verb:        "values.bind",
			Category:    stringPtr("attribute_data"),
			Description: stringPtr("Bind value to attribute"),
			Parameters: map[string]interface{}{
				"attr_id": VerbParameter{Name: "attr_id", Type: "string", Required: true, Description: "Attribute identifier"},
				"value":   VerbParameter{Name: "value", Type: "string", Required: true, Description: "Value to bind"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Bind value", Usage: `(values.bind (bind (attr-id "ATTR-123") (value "example_value")))`, Context: "Value binding"},
			},
			Active:  true,
			Version: "1.0.0",
		},
		{
			Verb:        "values.validate",
			Category:    stringPtr("attribute_data"),
			Description: stringPtr("Validate attribute value"),
			Parameters: map[string]interface{}{
				"attr_id": VerbParameter{Name: "attr_id", Type: "string", Required: true, Description: "Attribute identifier"},
				"rule_id": VerbParameter{Name: "rule_id", Type: "string", Required: true, Description: "Validation rule identifier"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Validate value", Usage: `(values.validate (attr.id "ATTR-123") (rule.id "RULE-456"))`, Context: "Value validation"},
			},
			Active:  true,
			Version: "1.0.0",
		},
		{
			Verb:        "values.encrypt",
			Category:    stringPtr("attribute_data"),
			Description: stringPtr("Encrypt attribute value"),
			Parameters: map[string]interface{}{
				"attr_id": VerbParameter{Name: "attr_id", Type: "string", Required: true, Description: "Attribute identifier"},
				"key_id":  VerbParameter{Name: "key_id", Type: "string", Required: true, Description: "Encryption key identifier"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Encrypt value", Usage: `(values.encrypt (attr.id "ATTR-123") (key.id "KEY-456"))`, Context: "Value encryption"},
			},
			Active:  true,
			Version: "1.0.0",
		},
	}...)

	return vocabs
}

// extractHedgeFundVocabulary extracts verbs from the hedge fund investor domain
func (m *MigrationServiceImpl) extractHedgeFundVocabulary() []DomainVocabulary {
	var vocabs []DomainVocabulary

	vocabs = append(vocabs, []DomainVocabulary{
		{
			Verb:        "investor.start-opportunity",
			Category:    stringPtr("investor_lifecycle"),
			Description: stringPtr("Start investor opportunity process"),
			Parameters: map[string]interface{}{
				"investor_id":   VerbParameter{Name: "investor_id", Type: "uuid", Required: true, Description: "Investor identifier"},
				"legal_name":    VerbParameter{Name: "legal_name", Type: "string", Required: true, Description: "Legal name of investor"},
				"investor_type": VerbParameter{Name: "investor_type", Type: "enum", Required: true, Description: "Type of investor", EnumValues: []string{"PROPER_PERSON", "CORPORATE", "TRUST", "FOHF"}},
				"domicile":      VerbParameter{Name: "domicile", Type: "string", Required: true, Description: "Investor domicile country code"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Start corporate investor opportunity", Usage: `(investor.start-opportunity @attr{uuid-investor} @attr{uuid-name} @attr{uuid-type} @attr{uuid-domicile})`, Context: "Hedge fund investor onboarding"},
			},
			Active:  true,
			Version: "1.0.0",
		},
		{
			Verb:        "kyc.begin",
			Category:    stringPtr("compliance"),
			Description: stringPtr("Begin KYC process for investor"),
			Parameters: map[string]interface{}{
				"investor_id": VerbParameter{Name: "investor_id", Type: "uuid", Required: true, Description: "Investor identifier"},
				"kyc_tier":    VerbParameter{Name: "kyc_tier", Type: "enum", Required: true, Description: "KYC tier", EnumValues: []string{"SIMPLIFIED", "STANDARD", "ENHANCED"}},
			},
			Examples: []interface{}{
				VerbExample{Description: "Begin standard KYC", Usage: `(kyc.begin @attr{uuid-investor} @attr{uuid-tier})`, Context: "KYC initiation"},
			},
			Active:  true,
			Version: "1.0.0",
		},
		{
			Verb:        "kyc.collect-doc",
			Category:    stringPtr("compliance"),
			Description: stringPtr("Collect KYC document from investor"),
			Parameters: map[string]interface{}{
				"investor_id":   VerbParameter{Name: "investor_id", Type: "uuid", Required: true, Description: "Investor identifier"},
				"document_type": VerbParameter{Name: "document_type", Type: "string", Required: true, Description: "Type of document"},
				"subject":       VerbParameter{Name: "subject", Type: "string", Required: true, Description: "Document subject"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Collect incorporation certificate", Usage: `(kyc.collect-doc @attr{uuid-investor} @attr{uuid-doc-type} @attr{uuid-subject})`, Context: "Document collection"},
			},
			Active:  true,
			Version: "1.0.0",
		},
		{
			Verb:        "subscription.create",
			Category:    stringPtr("investment"),
			Description: stringPtr("Create subscription for investor"),
			Parameters: map[string]interface{}{
				"investor_id": VerbParameter{Name: "investor_id", Type: "uuid", Required: true, Description: "Investor identifier"},
				"fund_id":     VerbParameter{Name: "fund_id", Type: "uuid", Required: true, Description: "Fund identifier"},
				"class_id":    VerbParameter{Name: "class_id", Type: "uuid", Required: true, Description: "Share class identifier"},
				"amount":      VerbParameter{Name: "amount", Type: "number", Required: true, Description: "Subscription amount"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Create USD 1M subscription", Usage: `(subscription.create @attr{uuid-investor} @attr{uuid-fund} @attr{uuid-class} @attr{uuid-amount})`, Context: "Investment subscription"},
			},
			Active:  true,
			Version: "1.0.0",
		},
		{
			Verb:        "trade.execute",
			Category:    stringPtr("investment"),
			Description: stringPtr("Execute trade for investor"),
			Parameters: map[string]interface{}{
				"investor_id": VerbParameter{Name: "investor_id", Type: "uuid", Required: true, Description: "Investor identifier"},
				"trade_type":  VerbParameter{Name: "trade_type", Type: "enum", Required: true, Description: "Trade type", EnumValues: []string{"SUBSCRIPTION", "REDEMPTION", "SWITCH"}},
				"amount":      VerbParameter{Name: "amount", Type: "number", Required: true, Description: "Trade amount"},
				"trade_date":  VerbParameter{Name: "trade_date", Type: "date", Required: true, Description: "Trade execution date"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Execute subscription", Usage: `(trade.execute @attr{uuid-investor} @attr{uuid-trade-type} @attr{uuid-amount} @attr{uuid-date})`, Context: "Trade execution"},
			},
			Active:  true,
			Version: "1.0.0",
		},
	}...)

	return vocabs
}

// extractOrchestrationVocabulary extracts verbs from the orchestration domain
func (m *MigrationServiceImpl) extractOrchestrationVocabulary() []DomainVocabulary {
	var vocabs []DomainVocabulary

	vocabs = append(vocabs, []DomainVocabulary{
		{
			Verb:        "orchestration.create",
			Category:    stringPtr("orchestration"),
			Description: stringPtr("Create orchestration session"),
			Parameters: map[string]interface{}{
				"session_id":   VerbParameter{Name: "session_id", Type: "uuid", Required: true, Description: "Orchestration session identifier"},
				"entity_type":  VerbParameter{Name: "entity_type", Type: "enum", Required: true, Description: "Entity type", EnumValues: []string{"PROPER_PERSON", "CORPORATE", "TRUST", "PARTNERSHIP"}},
				"products":     VerbParameter{Name: "products", Type: "array", Required: true, Description: "Required products"},
				"jurisdiction": VerbParameter{Name: "jurisdiction", Type: "string", Required: true, Description: "Legal jurisdiction"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Create corporate orchestration", Usage: `(orchestration.create @attr{uuid-session} @attr{uuid-type} @attr{uuid-products} @attr{uuid-jurisdiction})`, Context: "Multi-domain orchestration"},
			},
			Active:  true,
			Version: "1.0.0",
		},
		{
			Verb:        "orchestration.route",
			Category:    stringPtr("orchestration"),
			Description: stringPtr("Route instruction to appropriate domains"),
			Parameters: map[string]interface{}{
				"instruction": VerbParameter{Name: "instruction", Type: "string", Required: true, Description: "User instruction"},
				"domains":     VerbParameter{Name: "domains", Type: "array", Required: true, Description: "Target domains"},
			},
			Examples: []interface{}{
				VerbExample{Description: "Route KYC instruction", Usage: `(orchestration.route @attr{uuid-instruction} @attr{uuid-domains})`, Context: "Domain routing"},
			},
			Active:  true,
			Version: "1.0.0",
		},
	}...)

	return vocabs
}

// =============================================================================
// Migration Operations
// =============================================================================

func (m *MigrationServiceImpl) MigrateHardcodedVocabulary(ctx context.Context, data MigrationData) error {
	// Validate migration data first
	if err := m.ValidateMigrationData(ctx, data); err != nil {
		return fmt.Errorf("migration data validation failed: %w", err)
	}

	// Initialize the domain with extracted vocabulary
	if err := m.service.InitializeDomain(ctx, data.Domain, data.Verbs); err != nil {
		return fmt.Errorf("failed to initialize domain %s: %w", data.Domain, err)
	}

	return nil
}

func (m *MigrationServiceImpl) ValidateMigrationData(ctx context.Context, data MigrationData) error {
	if data.Domain == "" {
		return fmt.Errorf("domain cannot be empty")
	}

	if len(data.Verbs) == 0 {
		return fmt.Errorf("no verbs provided for migration")
	}

	// Validate each verb
	for i, verb := range data.Verbs {
		if verb.Verb == "" {
			return fmt.Errorf("verb %d has empty verb name", i)
		}
		if verb.Domain == "" {
			verb.Domain = data.Domain // Set domain if not specified
		}
		if verb.Domain != data.Domain {
			return fmt.Errorf("verb %s has mismatched domain: expected %s, got %s", verb.Verb, data.Domain, verb.Domain)
		}
	}

	return nil
}

func (m *MigrationServiceImpl) GetMigrationStatus(ctx context.Context) (map[string]bool, error) {
	// Check if domains have been migrated by looking for vocabulary entries
	domains := []string{"onboarding", "hedge-fund-investor", "orchestration"}
	status := make(map[string]bool)

	for _, domain := range domains {
		vocabs, err := m.repo.ListDomainVocabs(ctx, &domain, nil, nil)
		if err != nil {
			return nil, fmt.Errorf("failed to check migration status for domain %s: %w", domain, err)
		}
		status[domain] = len(vocabs) > 0
	}

	return status, nil
}

func (m *MigrationServiceImpl) RemoveHardcodedReferences(ctx context.Context, domain string) error {
	// This would remove hardcoded references in the codebase
	// For now, this is a placeholder that would be implemented with code analysis
	// and file modifications to remove hardcoded vocabulary maps

	// Log the domains that need hardcoded reference removal
	fmt.Printf("TODO: Remove hardcoded references for domain: %s\n", domain)
	fmt.Printf("Files to modify:\n")
	switch domain {
	case "onboarding":
		fmt.Printf("  - internal/dsl/vocab.go (mark as deprecated)\n")
		fmt.Printf("  - internal/agent/dsl_agent.go (replace validateDSLVerbs)\n")
		fmt.Printf("  - internal/domains/onboarding/domain.go (use database lookups)\n")
	case "hedge-fund-investor":
		fmt.Printf("  - hedge-fund-investor-source/hf-investor/dsl/ (replace hardcoded maps)\n")
	case "orchestration":
		fmt.Printf("  - internal/orchestration/orchestration_vocabulary.go (use database)\n")
	}

	return nil
}

func (m *MigrationServiceImpl) VerifyMigrationIntegrity(ctx context.Context, domain string) error {
	// Get all vocabularies for the domain
	vocabs, err := m.repo.ListDomainVocabs(ctx, &domain, nil, nil)
	if err != nil {
		return fmt.Errorf("failed to get vocabularies for integrity check: %w", err)
	}

	if len(vocabs) == 0 {
		return fmt.Errorf("no vocabularies found for domain %s - migration incomplete", domain)
	}

	// Verify each vocabulary has required fields
	for _, vocab := range vocabs {
		if vocab.Verb == "" {
			return fmt.Errorf("vocabulary %s has empty verb", vocab.VocabID)
		}
		if vocab.Domain != domain {
			return fmt.Errorf("vocabulary %s has wrong domain: expected %s, got %s", vocab.Verb, domain, vocab.Domain)
		}
		if !vocab.Active {
			// Warning, not error - some verbs might be intentionally inactive
			fmt.Printf("Warning: verb %s in domain %s is inactive\n", vocab.Verb, domain)
		}
	}

	fmt.Printf("Migration integrity verified for domain %s: %d vocabularies found\n", domain, len(vocabs))
	return nil
}

// =============================================================================
// Convenience Functions for Standard Migrations
// =============================================================================

func (m *MigrationServiceImpl) MigrateOnboardingDomain(ctx context.Context) error {
	vocabs := m.extractOnboardingVocabulary()

	data := MigrationData{
		Domain:     "onboarding",
		Verbs:      vocabs,
		MigratedBy: "system",
		MigratedAt: time.Now(),
		Source:     "internal/dsl/vocab.go",
	}

	return m.MigrateHardcodedVocabulary(ctx, data)
}

func (m *MigrationServiceImpl) MigrateHedgeFundDomain(ctx context.Context) error {
	vocabs := m.extractHedgeFundVocabulary()

	data := MigrationData{
		Domain:     "hedge-fund-investor",
		Verbs:      vocabs,
		MigratedBy: "system",
		MigratedAt: time.Now(),
		Source:     "hedge-fund-investor-source/hf-investor/dsl/",
	}

	return m.MigrateHardcodedVocabulary(ctx, data)
}

func (m *MigrationServiceImpl) MigrateOrchestrationDomain(ctx context.Context) error {
	vocabs := m.extractOrchestrationVocabulary()

	data := MigrationData{
		Domain:     "orchestration",
		Verbs:      vocabs,
		MigratedBy: "system",
		MigratedAt: time.Now(),
		Source:     "internal/orchestration/orchestration_vocabulary.go",
	}

	return m.MigrateHardcodedVocabulary(ctx, data)
}

func (m *MigrationServiceImpl) MigrateAllDomains(ctx context.Context) error {
	domains := []struct {
		name      string
		migrateFn func(context.Context) error
	}{
		{"onboarding", m.MigrateOnboardingDomain},
		{"hedge-fund-investor", m.MigrateHedgeFundDomain},
		{"orchestration", m.MigrateOrchestrationDomain},
	}

	for _, domain := range domains {
		fmt.Printf("Migrating domain: %s\n", domain.name)
		if err := domain.migrateFn(ctx); err != nil {
			return fmt.Errorf("failed to migrate domain %s: %w", domain.name, err)
		}
		fmt.Printf("Successfully migrated domain: %s\n", domain.name)
	}

	return nil
}

// Helper function to create string pointers
func stringPtr(s string) *string {
	return &s
}
