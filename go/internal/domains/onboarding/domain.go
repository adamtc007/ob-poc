// Package onboarding provides the onboarding domain implementation for the multi-domain DSL system.
//
// This domain handles the complete client onboarding lifecycle from case creation through
// completion, including entity registration, product configuration, KYC compliance,
// resource provisioning, and workflow orchestration. It implements 68 specialized verbs
// across 9 categories and manages a comprehensive state progression through the onboarding journey.
//
// Key Features:
// - Complete onboarding lifecycle management (CREATE → COMPLETE)
// - Case management with validation and approval workflows
// - Entity identity registration and classification
// - Product and service discovery with configuration
// - KYC and compliance workflows with document collection
// - Resource infrastructure provisioning and deployment
// - Attribute data binding with encryption and validation
// - Workflow state management with task orchestration
// - Notification and communication workflows
// - External system integration and API management
//
// State Machine (8 states):
// CREATE → PRODUCTS_ADDED → KYC_STARTED → SERVICES_DISCOVERED →
// RESOURCES_PLANNED → ATTRIBUTES_BOUND → WORKFLOW_ACTIVE → COMPLETE
package onboarding

import (
	"context"
	"fmt"
	"regexp"
	"strings"
	"time"

	"dsl-ob-poc/internal/dictionary/repository"
	registry "dsl-ob-poc/internal/domain-registry"
)

// Attribute UUID Constants - Real UUIDs from seeded dictionary
const (
	// Core Onboarding Attributes
	AttrOnboardCBUID         = "123e4567-e89b-12d3-a456-426614174001" // onboard.cbu_id
	AttrOnboardNaturePurpose = "123e4567-e89b-12d3-a456-426614174002" // onboard.nature_purpose
	AttrOnboardStatus        = "123e4567-e89b-12d3-a456-426614174003" // onboard.status

	// Entity Attributes
	AttrEntityType               = "987fcdeb-51a2-43f7-8765-ba9876543202" // entity.type
	AttrEntityIncorporationDate  = "987fcdeb-51a2-43f7-8765-ba9876543204" // entity.incorporation_date
	AttrEntityRegistrationNumber = "987fcdeb-51a2-43f7-8765-ba9876543205" // entity.registration_number

	// Custody Attributes
	AttrCustodyAccountNumber = "456789ab-cdef-1234-5678-9abcdef01301" // custody.account_number
	AttrCustodyCustodianName = "456789ab-cdef-1234-5678-9abcdef01302" // custody.custodian_name
	AttrCustodyAccountType   = "456789ab-cdef-1234-5678-9abcdef01303" // custody.account_type

	// Fund Accounting Attributes
	AttrAccountingFundCode = "456789ab-cdef-1234-5678-9abcdef01401" // accounting.fund_code
	AttrAccountingNAVValue = "456789ab-cdef-1234-5678-9abcdef01402" // accounting.nav_value

	// Resource Management Attributes
	AttrResourceCustodyAccountID = "24681357-9bdf-ace0-2468-13579bdfabc1" // resource.custody_account_id
	AttrResourceFundAccountingID = "24681357-9bdf-ace0-2468-13579bdfabc2" // resource.fund_accounting_id
	AttrResourceTransferAgencyID = "24681357-9bdf-ace0-2468-13579bdfabc3" // resource.transfer_agency_id
	AttrResourceRiskSystemID     = "24681357-9bdf-ace0-2468-13579bdfabc4" // resource.risk_system_id

	// Transfer Agency Attributes
	AttrTAFundIdentifier = "13579bdf-2468-ace0-1357-9bdf2468abc1" // transfer_agency.fund_identifier
	AttrTAShareClass     = "13579bdf-2468-ace0-1357-9bdf2468abc2" // transfer_agency.share_class

	// Fund Attributes
	AttrFundName          = "fedcba98-7654-3210-fedc-ba9876543201" // fund.name
	AttrFundStrategy      = "fedcba98-7654-3210-fedc-ba9876543202" // fund.strategy
	AttrFundBaseCurrency  = "fedcba98-7654-3210-fedc-ba9876543203" // fund.base_currency
	AttrFundMinInvestment = "fedcba98-7654-3210-fedc-ba9876543204" // fund.minimum_investment

	// Document Attributes
	AttrDocCertificateOfIncorporation = "abcdef12-3456-7890-abcd-ef1234567801" // document.certificate_of_incorporation
	AttrDocArticlesOfAssociation      = "abcdef12-3456-7890-abcd-ef1234567802" // document.articles_of_association
	AttrDocBoardResolution            = "abcdef12-3456-7890-abcd-ef1234567803" // document.board_resolution
)

const (
	StateCreate = "CREATE"
)

// Domain implements the Domain interface for onboarding workflows
type Domain struct {
	name        string
	version     string
	description string
	vocabulary  *registry.Vocabulary
	healthy     bool
	metrics     *registry.DomainMetrics
	createdAt   time.Time
	dictRepo    repository.DictionaryRepository
}

// NewDomain creates a new onboarding domain
func NewDomain() *Domain {
	return NewDomainWithDictionary(nil)
}

// NewDomainWithDictionary creates a new onboarding domain with dictionary repository
func NewDomainWithDictionary(dictRepo repository.DictionaryRepository) *Domain {
	domain := &Domain{
		name:        "onboarding",
		version:     "1.0.0",
		description: "Client onboarding lifecycle management from case creation to completion",
		healthy:     true,
		createdAt:   time.Now(),
		dictRepo:    dictRepo,
	}

	domain.vocabulary = domain.buildVocabulary()

	// Set metrics after vocabulary is built
	domain.metrics = &registry.DomainMetrics{
		TotalRequests:      0,
		SuccessfulRequests: 0,
		FailedRequests:     0,
		TotalVerbs:         int(len(domain.vocabulary.Verbs)),
		ActiveVerbs:        int(len(domain.vocabulary.Verbs)),
		UnusedVerbs:        0,
		StateTransitions:   make(map[string]int64),
		CurrentStates:      make(map[string]int64),
		ValidationErrors:   make(map[string]int64),
		GenerationErrors:   make(map[string]int64),
		IsHealthy:          true,
		LastHealthCheck:    time.Now(),
		UptimeSeconds:      0,
		MemoryUsageBytes:   4 * 1024 * 1024, // 4MB for larger vocabulary
		CollectedAt:        time.Now(),
		Version:            "1.0.0",
	}

	return domain
}

// Domain interface implementation

func (d *Domain) Name() string                        { return d.name }
func (d *Domain) Version() string                     { return d.version }
func (d *Domain) Description() string                 { return d.description }
func (d *Domain) GetVocabulary() *registry.Vocabulary { return d.vocabulary }
func (d *Domain) IsHealthy() bool                     { return d.healthy }
func (d *Domain) GetMetrics() *registry.DomainMetrics { return d.metrics }

func (d *Domain) GetValidStates() []string {
	return []string{
		"CREATE", "PRODUCTS_ADDED", "KYC_STARTED", "SERVICES_DISCOVERED",
		"RESOURCES_PLANNED", "ATTRIBUTES_BOUND", "WORKFLOW_ACTIVE", "COMPLETE",
	}
}

func (d *Domain) GetInitialState() string {
	return StateCreate
}

// ValidateVerbs checks that the DSL only uses approved onboarding verbs
func (d *Domain) ValidateVerbs(dsl string) error {
	if strings.TrimSpace(dsl) == "" {
		return fmt.Errorf("empty DSL")
	}

	// Parse the DSL to find actual verbs (not identifiers)
	return d.validateDSLVerbs(dsl)
}

// validateDSLVerbs recursively validates verbs in S-expressions
func (d *Domain) validateDSLVerbs(dsl string) error {
	dsl = strings.TrimSpace(dsl)
	if dsl == "" {
		return nil
	}

	// Simple S-expression parser to find verbs
	i := 0
	for i < len(dsl) {
		// Skip whitespace and comments
		for i < len(dsl) && (dsl[i] == ' ' || dsl[i] == '\t' || dsl[i] == '\n') {
			i++
		}

		if i >= len(dsl) {
			break
		}

		// Skip comments
		if i < len(dsl)-1 && dsl[i] == ';' {
			// Skip to end of line
			for i < len(dsl) && dsl[i] != '\n' {
				i++
			}
			continue
		}

		// Look for opening parenthesis
		if dsl[i] == '(' {
			i++ // Skip opening paren

			// Skip whitespace after opening paren
			for i < len(dsl) && (dsl[i] == ' ' || dsl[i] == '\t' || dsl[i] == '\n') {
				i++
			}

			// Extract the verb (first token after opening paren)
			verbStart := i
			for i < len(dsl) && dsl[i] != ' ' && dsl[i] != '\t' && dsl[i] != '\n' && dsl[i] != ')' {
				i++
			}

			if i > verbStart {
				verb := dsl[verbStart:i]

				// Only validate if it looks like a domain.action verb
				if strings.Contains(verb, ".") && len(strings.Split(verb, ".")) == 2 {
					if _, exists := d.vocabulary.Verbs[verb]; !exists {
						return fmt.Errorf("invalid onboarding verb: %s", verb)
					}
				}
			}

			// Skip to matching closing paren for this s-expression
			parenCount := 1
			for i < len(dsl) && parenCount > 0 {
				if dsl[i] == '(' {
					parenCount++
				} else if dsl[i] == ')' {
					parenCount--
				}
				i++
			}
		} else {
			i++
		}
	}

	return nil
}

// ValidateStateTransition checks if a state transition is valid for onboarding
func (d *Domain) ValidateStateTransition(from, to string) error {
	validStates := d.GetValidStates()

	// Check if states exist
	fromValid := false
	toValid := false
	for _, state := range validStates {
		if state == from {
			fromValid = true
		}
		if state == to {
			toValid = true
		}
	}

	if !fromValid {
		return fmt.Errorf("invalid from state: %s", from)
	}
	if !toValid {
		return fmt.Errorf("invalid to state: %s", to)
	}

	// Define valid transitions for onboarding workflow
	validTransitions := map[string][]string{
		"CREATE":              {"PRODUCTS_ADDED"},
		"PRODUCTS_ADDED":      {"KYC_STARTED"},
		"KYC_STARTED":         {"SERVICES_DISCOVERED"},
		"SERVICES_DISCOVERED": {"RESOURCES_PLANNED"},
		"RESOURCES_PLANNED":   {"ATTRIBUTES_BOUND"},
		"ATTRIBUTES_BOUND":    {"WORKFLOW_ACTIVE"},
		"WORKFLOW_ACTIVE":     {"COMPLETE"},
	}

	allowedNextStates, exists := validTransitions[from]
	if !exists {
		return fmt.Errorf("no valid transitions from state: %s", from)
	}

	for _, allowedState := range allowedNextStates {
		if allowedState == to {
			return nil // Valid transition
		}
	}

	return fmt.Errorf("invalid state transition from %s to %s", from, to)
}

// GenerateDSL creates DSL from natural language instructions
func (d *Domain) GenerateDSL(ctx context.Context, req *registry.GenerationRequest) (*registry.GenerationResponse, error) {
	if req == nil || req.Instruction == "" {
		return nil, fmt.Errorf("empty generation request")
	}

	instruction := strings.ToLower(req.Instruction)
	var dsl string

	// Pattern matching for common onboarding scenarios
	switch {
	case strings.Contains(instruction, "create case"):
		cbuID := extractCBUID(req.Instruction)
		if cbuID == "" {
			cbuID = "CBU-" + generateTestID()
		}
		natureFilter := extractNaturePurpose(req.Instruction)
		if natureFilter == "" {
			natureFilter = "Standard client onboarding"
		}
		dsl = fmt.Sprintf("(case.create (cbu.id %q) (nature-purpose %q))", cbuID, natureFilter)

	case strings.Contains(instruction, "add products"):
		products := extractProducts(req.Instruction)
		if len(products) == 0 {
			products = []string{"CUSTODY", "FUND_ACCOUNTING"}
		}
		dsl = generateProductsAddDSL(products)

	case strings.Contains(instruction, "start kyc"):
		dsl = "(kyc.start (requirements (document \"CertificateOfIncorporation\") (jurisdiction \"US\")))"

	case strings.Contains(instruction, "discover services"):
		dsl = "(services.discover (for.product \"CUSTODY\" (service \"AccountOpening\") (service \"TradeSettlement\")))"

	case strings.Contains(instruction, "plan resources"):
		dsl = fmt.Sprintf(`(resources.plan
  (resource.create "CustodyAccount"
    (owner "CustodyTech")
    @attr{%s:custody.account_number}
    @attr{%s:custody.account_type}
  )
  (resource.create "FundAccountingSystem"
    (owner "AccountingTech")
    @attr{%s:accounting.fund_code}
    @attr{%s:fund.base_currency}
  )
  (resource.create "TransferAgencySystem"
    (owner "TransferTech")
    @attr{%s:transfer_agency.fund_identifier}
    @attr{%s:transfer_agency.share_class}
  )
)`, AttrCustodyAccountNumber, AttrCustodyAccountType, AttrAccountingFundCode, AttrFundBaseCurrency, AttrTAFundIdentifier, AttrTAShareClass)

	case strings.Contains(instruction, "bind attributes"):
		dsl = fmt.Sprintf(`(values.bind
  @attr{%s:custody.account_number} "CUST-EGOF-001"
  @attr{%s:accounting.fund_code} "FA-EGOF-LU-001"
  @attr{%s:transfer_agency.fund_identifier} "TA-EGOF-LU"
  @attr{%s:fund.base_currency} "EUR"
)`, AttrCustodyAccountNumber, AttrAccountingFundCode, AttrTAFundIdentifier, AttrFundBaseCurrency)

	case strings.Contains(instruction, "workflow transition"):
		from := "CREATE"
		to := "PRODUCTS_ADDED"
		if req.Context != nil {
			if f, ok := req.Context["from_state"]; ok {
				if fs, fromOk := f.(string); fromOk {
					from = fs
				}
			}
			if t, ok := req.Context["to_state"]; ok {
				if ts, toOk := t.(string); toOk {
					to = ts
				}
			}
		}
		dsl = fmt.Sprintf("(workflow.transition (from %q) (to %q))", from, to)

	default:
		return nil, fmt.Errorf("unsupported onboarding instruction: %s", req.Instruction)
	}

	// Validate generated DSL
	if err := d.ValidateVerbs(dsl); err != nil {
		return nil, fmt.Errorf("generated invalid DSL: %w", err)
	}

	return &registry.GenerationResponse{
		DSL:  dsl,
		Verb: strings.Split(dsl, " ")[0][1:], // Extract verb from DSL
		Parameters: map[string]interface{}{
			"generated_at": time.Now(),
			"pattern":      "onboarding_basic",
		},
	}, nil
}

// GetCurrentState determines current state from context
func (d *Domain) GetCurrentState(context map[string]interface{}) (string, error) {
	if context == nil {
		return d.GetInitialState(), nil
	}

	// Check for explicit state in context
	if state, exists := context["current_state"]; exists {
		if stateStr, ok := state.(string); ok {
			// Validate state
			for _, validState := range d.GetValidStates() {
				if validState == stateStr {
					return stateStr, nil
				}
			}
			return "", fmt.Errorf("invalid state in context: %s", stateStr)
		}
	}

	// Infer state from context keys
	if _, exists := context["cbu_id"]; exists {
		if _, hasProducts := context["products"]; hasProducts {
			if _, hasKyc := context["kyc_started"]; hasKyc {
				if _, hasServices := context["services_discovered"]; hasServices {
					if _, hasResources := context["resources_planned"]; hasResources {
						if _, hasAttributes := context["attributes_bound"]; hasAttributes {
							if _, hasWorkflow := context["workflow_active"]; hasWorkflow {
								return "WORKFLOW_ACTIVE", nil
							}
							return "ATTRIBUTES_BOUND", nil
						}
						return "RESOURCES_PLANNED", nil
					}
					return "SERVICES_DISCOVERED", nil
				}
				return "KYC_STARTED", nil
			}
			return "PRODUCTS_ADDED", nil
		}
		return "CREATE", nil
	}

	return d.GetInitialState(), nil
}

// ExtractContext extracts onboarding-specific context from DSL
func (d *Domain) ExtractContext(dsl string) (map[string]interface{}, error) {
	if strings.TrimSpace(dsl) == "" {
		return map[string]interface{}{}, nil
	}

	context := make(map[string]interface{})

	// Extract CBU ID
	if cbuMatch := regexp.MustCompile(`\(cbu\.id\s+"([^"]+)"`).FindStringSubmatch(dsl); len(cbuMatch) > 1 {
		context["cbu_id"] = cbuMatch[1]
	}

	// Extract nature purpose
	if natureMatch := regexp.MustCompile(`\(nature-purpose\s+"([^"]+)"`).FindStringSubmatch(dsl); len(natureMatch) > 1 {
		context["nature_purpose"] = natureMatch[1]
	}

	// Check for products
	if strings.Contains(dsl, "products.add") {
		context["products"] = true
		context["current_state"] = "PRODUCTS_ADDED"
	}

	// Check for KYC
	if strings.Contains(dsl, "kyc.start") {
		context["kyc_started"] = true
		context["current_state"] = "KYC_STARTED"
	}

	// Check for services
	if strings.Contains(dsl, "services.discover") {
		context["services_discovered"] = true
		context["current_state"] = "SERVICES_DISCOVERED"
	}

	// Check for resources
	if strings.Contains(dsl, "resources.plan") {
		context["resources_planned"] = true
		context["current_state"] = "RESOURCES_PLANNED"
	}

	// Check for attributes
	if strings.Contains(dsl, "values.bind") {
		context["attributes_bound"] = true
		context["current_state"] = "ATTRIBUTES_BOUND"
	}

	// Check for workflow
	if strings.Contains(dsl, "workflow.transition") {
		context["workflow_active"] = true
		context["current_state"] = "WORKFLOW_ACTIVE"
	}

	// Check for completion
	if strings.Contains(dsl, "case.close") {
		context["current_state"] = "COMPLETE"
	}

	// If no specific state found, default to CREATE if we have a cbu_id
	if _, exists := context["current_state"]; !exists {
		if _, hasCbu := context["cbu_id"]; hasCbu {
			context["current_state"] = "CREATE"
		}
	}

	return context, nil
}

// Helper functions for DSL generation

func extractCBUID(instruction string) string {
	// Look for CBU-XXXX pattern
	if match := regexp.MustCompile(`CBU-[A-Z0-9]+`).FindString(instruction); match != "" {
		return match
	}
	return ""
}

func extractNaturePurpose(instruction string) string {
	// Look for quoted strings or common fund types
	if match := regexp.MustCompile(`"([^"]+)"`).FindStringSubmatch(instruction); len(match) > 1 {
		return match[1]
	}
	if strings.Contains(strings.ToLower(instruction), "fund") {
		return "Investment fund setup"
	}
	return ""
}

func extractProducts(instruction string) []string {
	instruction = strings.ToLower(instruction)
	var products []string

	if strings.Contains(instruction, "custody") {
		products = append(products, "CUSTODY")
	}
	if strings.Contains(instruction, "fund accounting") {
		products = append(products, "FUND_ACCOUNTING")
	}
	if strings.Contains(instruction, "transfer agent") {
		products = append(products, "TRANSFER_AGENT")
	}

	return products
}

func generateProductsAddDSL(products []string) string {
	if len(products) == 0 {
		return "(products.add)"
	}

	quotedProducts := make([]string, len(products))
	for i, p := range products {
		quotedProducts[i] = fmt.Sprintf("%q", p)
	}

	return fmt.Sprintf("(products.add %s)", strings.Join(quotedProducts, " "))
}

func generateTestID() string {
	return fmt.Sprintf("%d", time.Now().Unix()%10000)
}

// buildVocabulary constructs the complete onboarding vocabulary with all 68 verbs
func (d *Domain) buildVocabulary() *registry.Vocabulary {
	vocab := &registry.Vocabulary{
		Domain:      d.name,
		Version:     d.version,
		Description: d.description,
		Verbs:       make(map[string]*registry.VerbDefinition),
		Categories:  make(map[string]*registry.VerbCategory),
		States:      d.GetValidStates(),
		CreatedAt:   d.createdAt,
		UpdatedAt:   time.Now(),
	}

	// Common argument specifications
	cbuIDArg := &registry.ArgumentSpec{
		Name:        "cbu-id",
		Type:        registry.ArgumentTypeString,
		Required:    true,
		Description: "Client Business Unit identifier",
		Pattern:     "^CBU-[A-Z0-9]+$",
	}

	entityIDArg := &registry.ArgumentSpec{
		Name:        "entity-id",
		Type:        registry.ArgumentTypeUUID,
		Required:    true,
		Description: "Entity UUID identifier",
	}

	attrIDArg := &registry.ArgumentSpec{
		Name:        "attr-id",
		Type:        registry.ArgumentTypeUUID,
		Required:    true,
		Description: "Attribute UUID identifier",
	}

	// 1. CASE MANAGEMENT VERBS (5 verbs)

	vocab.Verbs["case.create"] = &registry.VerbDefinition{
		Name:        "case.create",
		Category:    "case-management",
		Version:     "1.0.0",
		Description: "Create a new onboarding case",
		Arguments: map[string]*registry.ArgumentSpec{
			"cbu.id": cbuIDArg,
			"nature-purpose": {
				Name:        "nature-purpose",
				Type:        registry.ArgumentTypeString,
				Required:    true,
				Description: "Nature and purpose of the client relationship",
			},
		},
		StateTransition: &registry.StateTransition{
			FromStates: []string{},
			ToState:    "CREATE",
		},
		Idempotent: true,
		Examples:   []string{`(case.create (cbu.id "CBU-1234") (nature-purpose "UCITS fund domiciled in LU"))`},
		CreatedAt:  time.Now(),
		UpdatedAt:  time.Now(),
	}

	vocab.Verbs["case.update"] = &registry.VerbDefinition{
		Name:        "case.update",
		Category:    "case-management",
		Version:     "1.0.0",
		Description: "Update case status or information",
		Arguments: map[string]*registry.ArgumentSpec{
			"cbu.id": cbuIDArg,
			"status": {
				Name:        "status",
				Type:        registry.ArgumentTypeString,
				Required:    true,
				Description: "New case status",
			},
		},
		Examples:  []string{`(case.update (cbu.id "CBU-1234") (status "IN_PROGRESS"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["case.validate"] = &registry.VerbDefinition{
		Name:        "case.validate",
		Category:    "case-management",
		Version:     "1.0.0",
		Description: "Validate case requirements",
		Arguments: map[string]*registry.ArgumentSpec{
			"requirements": {
				Name:        "requirements",
				Type:        registry.ArgumentTypeString,
				Required:    true,
				Description: "Requirements to validate against",
			},
		},
		Examples:  []string{`(case.validate (requirements "completeness_check"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["case.approve"] = &registry.VerbDefinition{
		Name:        "case.approve",
		Category:    "case-management",
		Version:     "1.0.0",
		Description: "Approve case for progression",
		Arguments: map[string]*registry.ArgumentSpec{
			"approver.id": {
				Name:        "approver.id",
				Type:        registry.ArgumentTypeString,
				Required:    true,
				Description: "Approver identifier",
			},
			"timestamp": {
				Name:        "timestamp",
				Type:        registry.ArgumentTypeString,
				Required:    true,
				Description: "Approval timestamp",
			},
		},
		Examples:  []string{`(case.approve (approver.id "admin-001") (timestamp "2024-01-01T10:00:00Z"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["case.close"] = &registry.VerbDefinition{
		Name:        "case.close",
		Category:    "case-management",
		Version:     "1.0.0",
		Description: "Close completed case",
		Arguments: map[string]*registry.ArgumentSpec{
			"reason": {
				Name:        "reason",
				Type:        registry.ArgumentTypeString,
				Required:    true,
				Description: "Reason for case closure",
			},
			"final-state": {
				Name:        "final-state",
				Type:        registry.ArgumentTypeString,
				Required:    true,
				Description: "Final case state",
			},
		},
		StateTransition: &registry.StateTransition{
			FromStates: []string{"WORKFLOW_ACTIVE"},
			ToState:    "COMPLETE",
		},
		Examples:  []string{`(case.close (reason "Onboarding completed successfully") (final-state "ACTIVE"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	// 2. ENTITY IDENTITY VERBS (5 verbs)

	vocab.Verbs["entity.register"] = &registry.VerbDefinition{
		Name:        "entity.register",
		Category:    "entity-identity",
		Version:     "1.0.0",
		Description: "Register a new entity",
		Arguments: map[string]*registry.ArgumentSpec{
			"type": {
				Name:        "type",
				Type:        registry.ArgumentTypeEnum,
				Required:    true,
				Description: "Entity type",
				EnumValues:  []string{"PROPER_PERSON", "CORPORATE", "FUND", "TRUST", "PARTNERSHIP"},
			},
			"jurisdiction": {
				Name:        "jurisdiction",
				Type:        registry.ArgumentTypeString,
				Required:    true,
				Description: "Legal jurisdiction",
				Pattern:     "^[A-Z]{2}$",
			},
		},
		Examples:  []string{`(entity.register (type "CORPORATE") (jurisdiction "LU"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["entity.classify"] = &registry.VerbDefinition{
		Name:        "entity.classify",
		Category:    "entity-identity",
		Version:     "1.0.0",
		Description: "Classify entity for risk and compliance",
		Arguments: map[string]*registry.ArgumentSpec{
			"category": {
				Name:        "category",
				Type:        registry.ArgumentTypeString,
				Required:    true,
				Description: "Classification category",
			},
			"risk-level": {
				Name:        "risk-level",
				Type:        registry.ArgumentTypeEnum,
				Required:    true,
				Description: "Risk classification level",
				EnumValues:  []string{"LOW", "MEDIUM", "HIGH", "PROHIBITED"},
			},
		},
		Examples:  []string{`(entity.classify (category "INSTITUTIONAL_INVESTOR") (risk-level "MEDIUM"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["entity.link"] = &registry.VerbDefinition{
		Name:        "entity.link",
		Category:    "entity-identity",
		Version:     "1.0.0",
		Description: "Link entities with relationships",
		Arguments: map[string]*registry.ArgumentSpec{
			"parent.id": entityIDArg,
			"relationship": {
				Name:        "relationship",
				Type:        registry.ArgumentTypeEnum,
				Required:    true,
				Description: "Relationship type",
				EnumValues:  []string{"PARENT", "SUBSIDIARY", "AFFILIATE", "BENEFICIAL_OWNER", "SIGNATORY"},
			},
		},
		Examples:  []string{`(entity.link (parent.id "uuid-parent") (relationship "SUBSIDIARY"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["identity.verify"] = &registry.VerbDefinition{
		Name:        "identity.verify",
		Category:    "entity-identity",
		Version:     "1.0.0",
		Description: "Verify entity identity",
		Arguments: map[string]*registry.ArgumentSpec{
			"document.id": {
				Name:        "document.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Identity document UUID",
			},
			"status": {
				Name:        "status",
				Type:        registry.ArgumentTypeEnum,
				Required:    true,
				Description: "Verification status",
				EnumValues:  []string{"VERIFIED", "PENDING", "FAILED", "EXPIRED"},
			},
		},
		Examples:  []string{`(identity.verify (document.id "doc-uuid") (status "VERIFIED"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["identity.attest"] = &registry.VerbDefinition{
		Name:        "identity.attest",
		Category:    "entity-identity",
		Version:     "1.0.0",
		Description: "Attest to identity with signatory",
		Arguments: map[string]*registry.ArgumentSpec{
			"signatory.id": entityIDArg,
			"capacity": {
				Name:        "capacity",
				Type:        registry.ArgumentTypeEnum,
				Required:    true,
				Description: "Signatory capacity",
				EnumValues:  []string{"DIRECTOR", "AUTHORIZED_SIGNATORY", "BENEFICIAL_OWNER", "ATTORNEY"},
			},
		},
		Examples:  []string{`(identity.attest (signatory.id "person-uuid") (capacity "DIRECTOR"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	// 3. PRODUCT SERVICE VERBS (5 verbs)

	vocab.Verbs["products.add"] = &registry.VerbDefinition{
		Name:        "products.add",
		Category:    "product-service",
		Version:     "1.0.0",
		Description: "Add products to onboarding case",
		Arguments: map[string]*registry.ArgumentSpec{
			"products": {
				Name:        "products",
				Type:        registry.ArgumentTypeArray,
				Required:    true,
				Description: "List of product codes to add",
			},
		},
		StateTransition: &registry.StateTransition{
			FromStates: []string{"CREATE"},
			ToState:    "PRODUCTS_ADDED",
		},
		Examples:  []string{`(products.add "CUSTODY" "FUND_ACCOUNTING" "TRANSFER_AGENT")`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["products.configure"] = &registry.VerbDefinition{
		Name:        "products.configure",
		Category:    "product-service",
		Version:     "1.0.0",
		Description: "Configure product settings",
		Arguments: map[string]*registry.ArgumentSpec{
			"product": {
				Name:        "product",
				Type:        registry.ArgumentTypeString,
				Required:    true,
				Description: "Product code to configure",
			},
			"settings.id": {
				Name:        "settings.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Configuration settings UUID",
			},
		},
		Examples:  []string{`(products.configure (product "CUSTODY") (settings.id "config-uuid"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["services.discover"] = &registry.VerbDefinition{
		Name:        "services.discover",
		Category:    "product-service",
		Version:     "1.0.0",
		Description: "Discover required services for products",
		Arguments: map[string]*registry.ArgumentSpec{
			"for.product": {
				Name:        "for.product",
				Type:        registry.ArgumentTypeString,
				Required:    true,
				Description: "Product to discover services for",
			},
			"services": {
				Name:        "services",
				Type:        registry.ArgumentTypeArray,
				Required:    false,
				Description: "Discovered services list",
			},
		},
		StateTransition: &registry.StateTransition{
			FromStates: []string{"KYC_STARTED"},
			ToState:    "SERVICES_DISCOVERED",
		},
		Examples:  []string{`(services.discover (for.product "CUSTODY" (service "AccountOpening") (service "TradeSettlement")))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["services.provision"] = &registry.VerbDefinition{
		Name:        "services.provision",
		Category:    "product-service",
		Version:     "1.0.0",
		Description: "Provision discovered services",
		Arguments: map[string]*registry.ArgumentSpec{
			"service.id": {
				Name:        "service.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Service UUID to provision",
			},
			"config.id": {
				Name:        "config.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Service configuration UUID",
			},
		},
		Examples:  []string{`(services.provision (service.id "svc-uuid") (config.id "cfg-uuid"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["services.activate"] = &registry.VerbDefinition{
		Name:        "services.activate",
		Category:    "product-service",
		Version:     "1.0.0",
		Description: "Activate provisioned services",
		Arguments: map[string]*registry.ArgumentSpec{
			"service.id": {
				Name:        "service.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Service UUID to activate",
			},
			"effective-date": {
				Name:        "effective-date",
				Type:        registry.ArgumentTypeDate,
				Required:    true,
				Description: "Service activation date",
			},
		},
		Examples:  []string{`(services.activate (service.id "svc-uuid") (effective-date "2024-01-15"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	// 4. KYC COMPLIANCE VERBS (6 verbs)

	vocab.Verbs["kyc.start"] = &registry.VerbDefinition{
		Name:        "kyc.start",
		Category:    "kyc-compliance",
		Version:     "1.0.0",
		Description: "Start KYC process with requirements",
		Arguments: map[string]*registry.ArgumentSpec{
			"requirements": {
				Name:        "requirements",
				Type:        registry.ArgumentTypeObject,
				Required:    true,
				Description: "KYC requirements specification",
			},
		},
		StateTransition: &registry.StateTransition{
			FromStates: []string{"PRODUCTS_ADDED"},
			ToState:    "KYC_STARTED",
		},
		Examples:  []string{`(kyc.start (requirements (document "CertificateOfIncorporation") (jurisdiction "LU")))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["kyc.collect"] = &registry.VerbDefinition{
		Name:        "kyc.collect",
		Category:    "kyc-compliance",
		Version:     "1.0.0",
		Description: "Collect KYC document",
		Arguments: map[string]*registry.ArgumentSpec{
			"document.id": {
				Name:        "document.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Document UUID",
			},
			"type": {
				Name:        "type",
				Type:        registry.ArgumentTypeEnum,
				Required:    true,
				Description: "Document type",
				EnumValues:  []string{"CERTIFICATE_OF_INCORPORATION", "PASSPORT", "UTILITY_BILL", "BANK_STATEMENT", "W8BEN", "W8BEN_E"},
			},
		},
		Examples:  []string{`(kyc.collect (document.id "doc-uuid") (type "CERTIFICATE_OF_INCORPORATION"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["kyc.verify"] = &registry.VerbDefinition{
		Name:        "kyc.verify",
		Category:    "kyc-compliance",
		Version:     "1.0.0",
		Description: "Verify collected document",
		Arguments: map[string]*registry.ArgumentSpec{
			"document.id": {
				Name:        "document.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Document UUID to verify",
			},
			"verifier.id": {
				Name:        "verifier.id",
				Type:        registry.ArgumentTypeString,
				Required:    true,
				Description: "Verifier identifier",
			},
		},
		Examples:  []string{`(kyc.verify (document.id "doc-uuid") (verifier.id "verifier-001"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["kyc.assess"] = &registry.VerbDefinition{
		Name:        "kyc.assess",
		Category:    "kyc-compliance",
		Version:     "1.0.0",
		Description: "Assess KYC risk rating",
		Arguments: map[string]*registry.ArgumentSpec{
			"risk-rating": {
				Name:        "risk-rating",
				Type:        registry.ArgumentTypeEnum,
				Required:    true,
				Description: "Assessed risk rating",
				EnumValues:  []string{"LOW", "MEDIUM", "HIGH", "PROHIBITED"},
			},
			"rationale.id": {
				Name:        "rationale.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    false,
				Description: "Risk assessment rationale UUID",
			},
		},
		Examples:  []string{`(kyc.assess (risk-rating "MEDIUM") (rationale.id "rationale-uuid"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["compliance.screen"] = &registry.VerbDefinition{
		Name:        "compliance.screen",
		Category:    "kyc-compliance",
		Version:     "1.0.0",
		Description: "Screen against compliance lists",
		Arguments: map[string]*registry.ArgumentSpec{
			"list": {
				Name:        "list",
				Type:        registry.ArgumentTypeEnum,
				Required:    true,
				Description: "Screening list type",
				EnumValues:  []string{"SANCTIONS", "PEP", "ADVERSE_MEDIA", "WORLDCHECK"},
			},
			"result.id": {
				Name:        "result.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Screening result UUID",
			},
		},
		Examples:  []string{`(compliance.screen (list "SANCTIONS") (result.id "screen-uuid"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["compliance.monitor"] = &registry.VerbDefinition{
		Name:        "compliance.monitor",
		Category:    "kyc-compliance",
		Version:     "1.0.0",
		Description: "Setup ongoing compliance monitoring",
		Arguments: map[string]*registry.ArgumentSpec{
			"trigger.id": {
				Name:        "trigger.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Monitoring trigger UUID",
			},
			"frequency": {
				Name:        "frequency",
				Type:        registry.ArgumentTypeEnum,
				Required:    true,
				Description: "Monitoring frequency",
				EnumValues:  []string{"DAILY", "WEEKLY", "MONTHLY", "QUARTERLY", "ANNUALLY"},
			},
		},
		Examples:  []string{`(compliance.monitor (trigger.id "trigger-uuid") (frequency "MONTHLY"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	// 5. RESOURCE INFRASTRUCTURE VERBS (5 verbs)

	vocab.Verbs["resources.plan"] = &registry.VerbDefinition{
		Name:        "resources.plan",
		Category:    "resource-infrastructure",
		Version:     "1.0.0",
		Description: "Plan resource requirements",
		Arguments: map[string]*registry.ArgumentSpec{
			"resource": {
				Name:        "resource",
				Type:        registry.ArgumentTypeObject,
				Required:    true,
				Description: "Resource specification",
			},
		},
		StateTransition: &registry.StateTransition{
			FromStates: []string{"SERVICES_DISCOVERED"},
			ToState:    "RESOURCES_PLANNED",
		},
		Examples:  []string{fmt.Sprintf(`(resources.plan (resource.create "CustodyAccount" (owner "CustodyTech") @attr{%s:custody.account_number} @attr{%s:custody.account_type}))`, AttrCustodyAccountNumber, AttrCustodyAccountType)},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["resources.provision"] = &registry.VerbDefinition{
		Name:        "resources.provision",
		Category:    "resource-infrastructure",
		Version:     "1.0.0",
		Description: "Provision planned resources",
		Arguments: map[string]*registry.ArgumentSpec{
			"resource.id": {
				Name:        "resource.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Resource UUID to provision",
			},
			"provider.id": {
				Name:        "provider.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Resource provider UUID",
			},
		},
		Examples:  []string{`(resources.provision (resource.id "res-uuid") (provider.id "provider-uuid"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["resources.configure"] = &registry.VerbDefinition{
		Name:        "resources.configure",
		Category:    "resource-infrastructure",
		Version:     "1.0.0",
		Description: "Configure provisioned resources",
		Arguments: map[string]*registry.ArgumentSpec{
			"resource.id": {
				Name:        "resource.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Resource UUID to configure",
			},
			"config.id": {
				Name:        "config.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Configuration UUID",
			},
		},
		Examples:  []string{`(resources.configure (resource.id "res-uuid") (config.id "cfg-uuid"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["resources.test"] = &registry.VerbDefinition{
		Name:        "resources.test",
		Category:    "resource-infrastructure",
		Version:     "1.0.0",
		Description: "Test configured resources",
		Arguments: map[string]*registry.ArgumentSpec{
			"resource.id": {
				Name:        "resource.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Resource UUID to test",
			},
			"test-suite.id": {
				Name:        "test-suite.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Test suite UUID",
			},
		},
		Examples:  []string{`(resources.test (resource.id "res-uuid") (test-suite.id "test-uuid"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["resources.deploy"] = &registry.VerbDefinition{
		Name:        "resources.deploy",
		Category:    "resource-infrastructure",
		Version:     "1.0.0",
		Description: "Deploy tested resources",
		Arguments: map[string]*registry.ArgumentSpec{
			"resource.id": {
				Name:        "resource.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Resource UUID to deploy",
			},
			"environment": {
				Name:        "environment",
				Type:        registry.ArgumentTypeEnum,
				Required:    true,
				Description: "Target environment",
				EnumValues:  []string{"DEVELOPMENT", "STAGING", "PRODUCTION"},
			},
		},
		Examples:  []string{`(resources.deploy (resource.id "res-uuid") (environment "PRODUCTION"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	// 6. ATTRIBUTE DATA VERBS (5 verbs)

	vocab.Verbs["attributes.define"] = &registry.VerbDefinition{
		Name:        "attributes.define",
		Category:    "attribute-data",
		Version:     "1.0.0",
		Description: "Define new attribute specification",
		Arguments: map[string]*registry.ArgumentSpec{
			"attr.id": attrIDArg,
			"type": {
				Name:        "type",
				Type:        registry.ArgumentTypeEnum,
				Required:    true,
				Description: "Attribute data type",
				EnumValues:  []string{"STRING", "INTEGER", "DECIMAL", "DATE", "BOOLEAN", "UUID", "ENUM"},
			},
		},
		Examples:  []string{fmt.Sprintf(`(attributes.define @attr{%s:onboard.nature_purpose} (type "STRING"))`, AttrOnboardNaturePurpose)},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["attributes.resolve"] = &registry.VerbDefinition{
		Name:        "attributes.resolve",
		Category:    "attribute-data",
		Version:     "1.0.0",
		Description: "Resolve attribute from source",
		Arguments: map[string]*registry.ArgumentSpec{
			"attr.id": attrIDArg,
			"source.id": {
				Name:        "source.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Data source UUID",
			},
		},
		Examples:  []string{fmt.Sprintf(`(attributes.resolve @attr{%s:onboard.nature_purpose} (source.id "src-uuid"))`, AttrOnboardNaturePurpose)},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["values.bind"] = &registry.VerbDefinition{
		Name:        "values.bind",
		Category:    "attribute-data",
		Version:     "1.0.0",
		Description: "Bind value to attribute",
		Arguments: map[string]*registry.ArgumentSpec{
			"bind": {
				Name:        "bind",
				Type:        registry.ArgumentTypeObject,
				Required:    true,
				Description: "Binding specification with attr-id and value",
			},
		},
		StateTransition: &registry.StateTransition{
			FromStates: []string{"RESOURCES_PLANNED"},
			ToState:    "ATTRIBUTES_BOUND",
		},
		Examples:  []string{fmt.Sprintf(`(values.bind @attr{%s:custody.account_number} "CUST-ACC-001" @attr{%s:accounting.fund_code} "FA-FUND-001")`, AttrCustodyAccountNumber, AttrAccountingFundCode)},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["values.validate"] = &registry.VerbDefinition{
		Name:        "values.validate",
		Category:    "attribute-data",
		Version:     "1.0.0",
		Description: "Validate bound values",
		Arguments: map[string]*registry.ArgumentSpec{
			"attr.id": attrIDArg,
			"rule.id": {
				Name:        "rule.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Validation rule UUID",
			},
		},
		Examples:  []string{fmt.Sprintf(`(values.validate @attr{%s:custody.account_number} (rule.id "rule-uuid"))`, AttrCustodyAccountNumber)},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["values.encrypt"] = &registry.VerbDefinition{
		Name:        "values.encrypt",
		Category:    "attribute-data",
		Version:     "1.0.0",
		Description: "Encrypt sensitive attribute values",
		Arguments: map[string]*registry.ArgumentSpec{
			"attr.id": attrIDArg,
			"key.id": {
				Name:        "key.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Encryption key UUID",
			},
		},
		Examples:  []string{fmt.Sprintf(`(values.encrypt @attr{%s:custody.account_number} (key.id "key-uuid"))`, AttrCustodyAccountNumber)},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	// 7. WORKFLOW STATE VERBS (5 verbs)

	vocab.Verbs["workflow.transition"] = &registry.VerbDefinition{
		Name:        "workflow.transition",
		Category:    "workflow-state",
		Version:     "1.0.0",
		Description: "Transition workflow state",
		Arguments: map[string]*registry.ArgumentSpec{
			"from": {
				Name:        "from",
				Type:        registry.ArgumentTypeString,
				Required:    true,
				Description: "Current state",
			},
			"to": {
				Name:        "to",
				Type:        registry.ArgumentTypeString,
				Required:    true,
				Description: "Target state",
			},
		},
		StateTransition: &registry.StateTransition{
			FromStates: []string{"ATTRIBUTES_BOUND"},
			ToState:    "WORKFLOW_ACTIVE",
		},
		Examples:  []string{`(workflow.transition (from "ATTRIBUTES_BOUND") (to "WORKFLOW_ACTIVE"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["workflow.gate"] = &registry.VerbDefinition{
		Name:        "workflow.gate",
		Category:    "workflow-state",
		Version:     "1.0.0",
		Description: "Define workflow gate condition",
		Arguments: map[string]*registry.ArgumentSpec{
			"condition.id": {
				Name:        "condition.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Gate condition UUID",
			},
			"required": {
				Name:        "required",
				Type:        registry.ArgumentTypeBoolean,
				Required:    true,
				Description: "Whether gate is required",
			},
		},
		Examples:  []string{`(workflow.gate (condition.id "gate-uuid") (required true))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["tasks.create"] = &registry.VerbDefinition{
		Name:        "tasks.create",
		Category:    "workflow-state",
		Version:     "1.0.0",
		Description: "Create workflow task",
		Arguments: map[string]*registry.ArgumentSpec{
			"task.id": {
				Name:        "task.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Task UUID",
			},
			"type": {
				Name:        "type",
				Type:        registry.ArgumentTypeEnum,
				Required:    true,
				Description: "Task type",
				EnumValues:  []string{"MANUAL", "AUTOMATED", "APPROVAL", "NOTIFICATION", "ESCALATION"},
			},
		},
		Examples:  []string{`(tasks.create (task.id "task-uuid") (type "MANUAL"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["tasks.assign"] = &registry.VerbDefinition{
		Name:        "tasks.assign",
		Category:    "workflow-state",
		Version:     "1.0.0",
		Description: "Assign task to user or system",
		Arguments: map[string]*registry.ArgumentSpec{
			"task.id": {
				Name:        "task.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Task UUID to assign",
			},
			"assignee.id": {
				Name:        "assignee.id",
				Type:        registry.ArgumentTypeString,
				Required:    true,
				Description: "Assignee identifier",
			},
		},
		Examples:  []string{`(tasks.assign (task.id "task-uuid") (assignee.id "user-001"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["tasks.complete"] = &registry.VerbDefinition{
		Name:        "tasks.complete",
		Category:    "workflow-state",
		Version:     "1.0.0",
		Description: "Complete assigned task",
		Arguments: map[string]*registry.ArgumentSpec{
			"task.id": {
				Name:        "task.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Task UUID to complete",
			},
			"outcome": {
				Name:        "outcome",
				Type:        registry.ArgumentTypeEnum,
				Required:    true,
				Description: "Task completion outcome",
				EnumValues:  []string{"SUCCESS", "FAILURE", "CANCELLED", "ESCALATED"},
			},
		},
		Examples:  []string{`(tasks.complete (task.id "task-uuid") (outcome "SUCCESS"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	// 8. NOTIFICATION COMMUNICATION VERBS (4 verbs)

	vocab.Verbs["notify.send"] = &registry.VerbDefinition{
		Name:        "notify.send",
		Category:    "notification-communication",
		Version:     "1.0.0",
		Description: "Send notification to recipient",
		Arguments: map[string]*registry.ArgumentSpec{
			"recipient.id": {
				Name:        "recipient.id",
				Type:        registry.ArgumentTypeString,
				Required:    true,
				Description: "Notification recipient identifier",
			},
			"template.id": {
				Name:        "template.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Notification template UUID",
			},
		},
		Examples:  []string{`(notify.send (recipient.id "user-001") (template.id "template-uuid"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["communicate.request"] = &registry.VerbDefinition{
		Name:        "communicate.request",
		Category:    "notification-communication",
		Version:     "1.0.0",
		Description: "Request communication with party",
		Arguments: map[string]*registry.ArgumentSpec{
			"party.id": {
				Name:        "party.id",
				Type:        registry.ArgumentTypeString,
				Required:    true,
				Description: "Communication party identifier",
			},
			"document.id": {
				Name:        "document.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    false,
				Description: "Related document UUID",
			},
		},
		Examples:  []string{`(communicate.request (party.id "client-001") (document.id "doc-uuid"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["escalate.trigger"] = &registry.VerbDefinition{
		Name:        "escalate.trigger",
		Category:    "notification-communication",
		Version:     "1.0.0",
		Description: "Trigger escalation process",
		Arguments: map[string]*registry.ArgumentSpec{
			"condition.id": {
				Name:        "condition.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Escalation condition UUID",
			},
			"level": {
				Name:        "level",
				Type:        registry.ArgumentTypeEnum,
				Required:    true,
				Description: "Escalation level",
				EnumValues:  []string{"L1", "L2", "L3", "EXECUTIVE", "REGULATORY"},
			},
		},
		Examples:  []string{`(escalate.trigger (condition.id "cond-uuid") (level "L2"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["audit.log"] = &registry.VerbDefinition{
		Name:        "audit.log",
		Category:    "notification-communication",
		Version:     "1.0.0",
		Description: "Log audit event",
		Arguments: map[string]*registry.ArgumentSpec{
			"event.id": {
				Name:        "event.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Audit event UUID",
			},
			"actor.id": {
				Name:        "actor.id",
				Type:        registry.ArgumentTypeString,
				Required:    true,
				Description: "Event actor identifier",
			},
		},
		Examples:  []string{`(audit.log (event.id "event-uuid") (actor.id "system"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	// 9. INTEGRATION EXTERNAL VERBS (4 verbs)

	vocab.Verbs["external.query"] = &registry.VerbDefinition{
		Name:        "external.query",
		Category:    "integration-external",
		Version:     "1.0.0",
		Description: "Query external system",
		Arguments: map[string]*registry.ArgumentSpec{
			"system": {
				Name:        "system",
				Type:        registry.ArgumentTypeString,
				Required:    true,
				Description: "External system identifier",
			},
			"endpoint.id": {
				Name:        "endpoint.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "API endpoint UUID",
			},
		},
		Examples:  []string{`(external.query (system "CRM_SYSTEM") (endpoint.id "endpoint-uuid"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["external.sync"] = &registry.VerbDefinition{
		Name:        "external.sync",
		Category:    "integration-external",
		Version:     "1.0.0",
		Description: "Synchronize with external system",
		Arguments: map[string]*registry.ArgumentSpec{
			"system.id": {
				Name:        "system.id",
				Type:        registry.ArgumentTypeString,
				Required:    true,
				Description: "External system identifier",
			},
			"data.id": {
				Name:        "data.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Data synchronization UUID",
			},
		},
		Examples:  []string{`(external.sync (system.id "ERP_SYSTEM") (data.id "data-uuid"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["api.call"] = &registry.VerbDefinition{
		Name:        "api.call",
		Category:    "integration-external",
		Version:     "1.0.0",
		Description: "Make external API call",
		Arguments: map[string]*registry.ArgumentSpec{
			"endpoint.id": {
				Name:        "endpoint.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "API endpoint UUID",
			},
			"payload.id": {
				Name:        "payload.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    false,
				Description: "Request payload UUID",
			},
		},
		Examples:  []string{`(api.call (endpoint.id "endpoint-uuid") (payload.id "payload-uuid"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["webhook.register"] = &registry.VerbDefinition{
		Name:        "webhook.register",
		Category:    "integration-external",
		Version:     "1.0.0",
		Description: "Register webhook endpoint",
		Arguments: map[string]*registry.ArgumentSpec{
			"url.id": {
				Name:        "url.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Webhook URL UUID",
			},
			"events": {
				Name:        "events",
				Type:        registry.ArgumentTypeString,
				Required:    true,
				Description: "Event types to listen for",
			},
		},
		Examples:  []string{`(webhook.register (url.id "url-uuid") (events "case.create,case.complete"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	// Define categories
	vocab.Categories["case-management"] = &registry.VerbCategory{
		Name:        "case-management",
		Description: "Case lifecycle management and approval workflows",
		Verbs:       []string{"case.create", "case.update", "case.validate", "case.approve", "case.close"},
		Color:       "#4CAF50",
		Icon:        "📋",
	}

	vocab.Categories["entity-identity"] = &registry.VerbCategory{
		Name:        "entity-identity",
		Description: "Entity registration, classification and identity verification",
		Verbs:       []string{"entity.register", "entity.classify", "entity.link", "identity.verify", "identity.attest"},
		Color:       "#FF9800",
		Icon:        "👤",
	}

	vocab.Categories["product-service"] = &registry.VerbCategory{
		Name:        "product-service",
		Description: "Product addition, configuration and service discovery",
		Verbs:       []string{"products.add", "products.configure", "services.discover", "services.provision", "services.activate"},
		Color:       "#2196F3",
		Icon:        "🛠️",
	}

	vocab.Categories["kyc-compliance"] = &registry.VerbCategory{
		Name:        "kyc-compliance",
		Description: "Know Your Customer and compliance processes",
		Verbs:       []string{"kyc.start", "kyc.collect", "kyc.verify", "kyc.assess", "compliance.screen", "compliance.monitor"},
		Color:       "#9C27B0",
		Icon:        "🔍",
	}

	vocab.Categories["resource-infrastructure"] = &registry.VerbCategory{
		Name:        "resource-infrastructure",
		Description: "Resource planning, provisioning and deployment",
		Verbs:       []string{"resources.plan", "resources.provision", "resources.configure", "resources.test", "resources.deploy"},
		Color:       "#607D8B",
		Icon:        "⚙️",
	}

	vocab.Categories["attribute-data"] = &registry.VerbCategory{
		Name:        "attribute-data",
		Description: "Attribute definition, binding and validation",
		Verbs:       []string{"attributes.define", "attributes.resolve", "values.bind", "values.validate", "values.encrypt"},
		Color:       "#795548",
		Icon:        "📊",
	}

	vocab.Categories["workflow-state"] = &registry.VerbCategory{
		Name:        "workflow-state",
		Description: "Workflow transitions, gates and task management",
		Verbs:       []string{"workflow.transition", "workflow.gate", "tasks.create", "tasks.assign", "tasks.complete"},
		Color:       "#FF5722",
		Icon:        "🔄",
	}

	vocab.Categories["notification-communication"] = &registry.VerbCategory{
		Name:        "notification-communication",
		Description: "Notifications, communication and escalation processes",
		Verbs:       []string{"notify.send", "communicate.request", "escalate.trigger", "audit.log"},
		Color:       "#3F51B5",
		Icon:        "📢",
	}

	vocab.Categories["integration-external"] = &registry.VerbCategory{
		Name:        "integration-external",
		Description: "External system integration and API management",
		Verbs:       []string{"external.query", "external.sync", "api.call", "webhook.register"},
		Color:       "#009688",
		Icon:        "🔗",
	}

	// 10. TEMPORAL SCHEDULING VERBS (3 verbs)

	vocab.Verbs["schedule.create"] = &registry.VerbDefinition{
		Name:        "schedule.create",
		Category:    "temporal-scheduling",
		Version:     "1.0.0",
		Description: "Create scheduled task",
		Arguments: map[string]*registry.ArgumentSpec{
			"task.id": {
				Name:        "task.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Task UUID to schedule",
			},
			"cron": {
				Name:        "cron",
				Type:        registry.ArgumentTypeString,
				Required:    true,
				Description: "Cron expression for schedule",
				Pattern:     "^[0-9*,/-]+\\s+[0-9*,/-]+\\s+[0-9*,/-]+\\s+[0-9*,/-]+\\s+[0-9*,/-]+$",
			},
		},
		Examples:  []string{`(schedule.create (task.id "task-uuid") (cron "0 9 * * MON"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["deadline.set"] = &registry.VerbDefinition{
		Name:        "deadline.set",
		Category:    "temporal-scheduling",
		Version:     "1.0.0",
		Description: "Set task deadline",
		Arguments: map[string]*registry.ArgumentSpec{
			"task.id": {
				Name:        "task.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Task UUID for deadline",
			},
			"date": {
				Name:        "date",
				Type:        registry.ArgumentTypeDate,
				Required:    true,
				Description: "Deadline date",
			},
		},
		Examples:  []string{`(deadline.set (task.id "task-uuid") (date "2024-12-31"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["reminder.schedule"] = &registry.VerbDefinition{
		Name:        "reminder.schedule",
		Category:    "temporal-scheduling",
		Version:     "1.0.0",
		Description: "Schedule reminder notification",
		Arguments: map[string]*registry.ArgumentSpec{
			"notification.id": {
				Name:        "notification.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Notification UUID",
			},
			"offset": {
				Name:        "offset",
				Type:        registry.ArgumentTypeString,
				Required:    true,
				Description: "Time offset from event (e.g., '-1d', '+2h')",
				Pattern:     "^[+-]?\\d+[smhdw]$",
			},
		},
		Examples:  []string{`(reminder.schedule (notification.id "notif-uuid") (offset "-24h"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	// 11. RISK MONITORING VERBS (3 verbs)

	vocab.Verbs["risk.assess"] = &registry.VerbDefinition{
		Name:        "risk.assess",
		Category:    "risk-monitoring",
		Version:     "1.0.0",
		Description: "Assess risk factor with weight",
		Arguments: map[string]*registry.ArgumentSpec{
			"factor.id": {
				Name:        "factor.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Risk factor UUID",
			},
			"weight": {
				Name:        "weight",
				Type:        registry.ArgumentTypeDecimal,
				Required:    true,
				Description: "Risk factor weight (0.0 to 1.0)",
				MinValue:    &[]float64{0.0}[0],
				MaxValue:    &[]float64{1.0}[0],
			},
		},
		Examples:  []string{`(risk.assess (factor.id "factor-uuid") (weight 0.75))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["monitor.setup"] = &registry.VerbDefinition{
		Name:        "monitor.setup",
		Category:    "risk-monitoring",
		Version:     "1.0.0",
		Description: "Setup monitoring with threshold",
		Arguments: map[string]*registry.ArgumentSpec{
			"metric.id": {
				Name:        "metric.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Metric UUID to monitor",
			},
			"threshold": {
				Name:        "threshold",
				Type:        registry.ArgumentTypeInteger,
				Required:    true,
				Description: "Alert threshold value",
				MinValue:    &[]float64{0}[0],
			},
		},
		Examples:  []string{`(monitor.setup (metric.id "metric-uuid") (threshold 100))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["alert.trigger"] = &registry.VerbDefinition{
		Name:        "alert.trigger",
		Category:    "risk-monitoring",
		Version:     "1.0.0",
		Description: "Trigger alert with severity",
		Arguments: map[string]*registry.ArgumentSpec{
			"condition.id": {
				Name:        "condition.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Alert condition UUID",
			},
			"severity": {
				Name:        "severity",
				Type:        registry.ArgumentTypeEnum,
				Required:    true,
				Description: "Alert severity level",
				EnumValues:  []string{"LOW", "MEDIUM", "HIGH", "CRITICAL"},
			},
		},
		Examples:  []string{`(alert.trigger (condition.id "cond-uuid") (severity "HIGH"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	// 12. DATA LIFECYCLE VERBS (4 verbs)

	vocab.Verbs["data.collect"] = &registry.VerbDefinition{
		Name:        "data.collect",
		Category:    "data-lifecycle",
		Version:     "1.0.0",
		Description: "Collect data from source to destination",
		Arguments: map[string]*registry.ArgumentSpec{
			"source.id": {
				Name:        "source.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Data source UUID",
			},
			"destination.id": {
				Name:        "destination.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Data destination UUID",
			},
		},
		Examples:  []string{`(data.collect (source.id "src-uuid") (destination.id "dest-uuid"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["data.transform"] = &registry.VerbDefinition{
		Name:        "data.transform",
		Category:    "data-lifecycle",
		Version:     "1.0.0",
		Description: "Transform data using transformer",
		Arguments: map[string]*registry.ArgumentSpec{
			"transformer.id": {
				Name:        "transformer.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Data transformer UUID",
			},
			"input.id": {
				Name:        "input.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Input data UUID",
			},
		},
		Examples:  []string{`(data.transform (transformer.id "trans-uuid") (input.id "input-uuid"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["data.archive"] = &registry.VerbDefinition{
		Name:        "data.archive",
		Category:    "data-lifecycle",
		Version:     "1.0.0",
		Description: "Archive data with retention policy",
		Arguments: map[string]*registry.ArgumentSpec{
			"data.id": {
				Name:        "data.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Data UUID to archive",
			},
			"retention": {
				Name:        "retention",
				Type:        registry.ArgumentTypeString,
				Required:    true,
				Description: "Retention period (e.g., '7y', '3m', '90d')",
				Pattern:     "^\\d+[ymdh]$",
			},
		},
		Examples:  []string{`(data.archive (data.id "data-uuid") (retention "7y"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Verbs["data.purge"] = &registry.VerbDefinition{
		Name:        "data.purge",
		Category:    "data-lifecycle",
		Version:     "1.0.0",
		Description: "Purge data with reason",
		Arguments: map[string]*registry.ArgumentSpec{
			"data.id": {
				Name:        "data.id",
				Type:        registry.ArgumentTypeUUID,
				Required:    true,
				Description: "Data UUID to purge",
			},
			"reason": {
				Name:        "reason",
				Type:        registry.ArgumentTypeEnum,
				Required:    true,
				Description: "Purge reason",
				EnumValues:  []string{"RETENTION_EXPIRED", "GDPR_REQUEST", "DATA_QUALITY", "REGULATORY_REQUIREMENT"},
			},
		},
		Examples:  []string{`(data.purge (data.id "data-uuid") (reason "RETENTION_EXPIRED"))`},
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	vocab.Categories["temporal-scheduling"] = &registry.VerbCategory{
		Name:        "temporal-scheduling",
		Description: "Time-based scheduling and deadline management",
		Verbs:       []string{"schedule.create", "deadline.set", "reminder.schedule"},
		Color:       "#FF6F00",
		Icon:        "⏰",
	}

	vocab.Categories["risk-monitoring"] = &registry.VerbCategory{
		Name:        "risk-monitoring",
		Description: "Risk assessment and monitoring workflows",
		Verbs:       []string{"risk.assess", "monitor.setup", "alert.trigger"},
		Color:       "#D32F2F",
		Icon:        "⚠️",
	}

	vocab.Categories["data-lifecycle"] = &registry.VerbCategory{
		Name:        "data-lifecycle",
		Description: "Data collection, transformation, archival and purging",
		Verbs:       []string{"data.collect", "data.transform", "data.archive", "data.purge"},
		Color:       "#388E3C",
		Icon:        "💾",
	}

	vocab.Categories["offboarding"] = &registry.VerbCategory{
		Name:        "offboarding",
		Description: "Case completion and closure",
		Verbs:       []string{"case.close"},
		Color:       "#F44336",
		Icon:        "✅",
	}

	return vocab
}

// =============================================================================
// Dictionary Integration - AttributeID-as-Type Pattern
// =============================================================================

// SetDictionaryRepository sets the dictionary repository for attribute resolution
func (d *Domain) SetDictionaryRepository(repo repository.DictionaryRepository) {
	d.dictRepo = repo
}

// ResolveAttributeName resolves a human-readable name from an attribute ID
func (d *Domain) ResolveAttributeName(ctx context.Context, attrID string) (string, error) {
	if d.dictRepo == nil {
		return "", fmt.Errorf("dictionary repository not configured")
	}

	attr, err := d.dictRepo.GetByID(ctx, attrID)
	if err != nil {
		return "", fmt.Errorf("failed to resolve attribute %s: %w", attrID, err)
	}

	return attr.Name, nil
}

// ResolveAttributesByIDs resolves multiple attribute names from their IDs
func (d *Domain) ResolveAttributesByIDs(ctx context.Context, attrIDs []string) (map[string]string, error) {
	if d.dictRepo == nil {
		return nil, fmt.Errorf("dictionary repository not configured")
	}

	result := make(map[string]string)
	for _, attrID := range attrIDs {
		name, err := d.ResolveAttributeName(ctx, attrID)
		if err != nil {
			// Log error but continue with other attributes
			result[attrID] = attrID // Use ID as fallback
		} else {
			result[attrID] = name
		}
	}

	return result, nil
}

// EnhanceDSLWithAttributeNames takes DSL with @attr{uuid} and adds human-readable names
// Returns DSL with @attr{uuid:name} format for better readability
func (d *Domain) EnhanceDSLWithAttributeNames(ctx context.Context, dsl string) (string, error) {
	if d.dictRepo == nil {
		return dsl, nil // Return original if no dictionary configured
	}

	// Parse DSL to extract attribute IDs
	parser := func() ([]string, error) {
		attrPattern := regexp.MustCompile(`@attr\{([a-fA-F0-9-]{8,36})(?::([^}]+))?\}`)
		matches := attrPattern.FindAllStringSubmatch(dsl, -1)

		var attrIDs []string
		for _, match := range matches {
			if len(match) >= 2 {
				attrID := match[1]
				// Only collect IDs that don't already have names
				if len(match) < 3 || match[2] == "" {
					attrIDs = append(attrIDs, attrID)
				}
			}
		}
		return attrIDs, nil
	}

	attrIDs, err := parser()
	if err != nil {
		return dsl, fmt.Errorf("failed to parse attribute IDs: %w", err)
	}

	if len(attrIDs) == 0 {
		return dsl, nil // No attributes to resolve
	}

	// Resolve names for all attribute IDs
	nameMap, err := d.ResolveAttributesByIDs(ctx, attrIDs)
	if err != nil {
		return dsl, fmt.Errorf("failed to resolve attribute names: %w", err)
	}

	// Replace @attr{uuid} with @attr{uuid:name} in DSL
	enhanced := dsl
	uuidPattern := regexp.MustCompile(`@attr\{([a-fA-F0-9-]{8,36})\}`)
	enhanced = uuidPattern.ReplaceAllStringFunc(enhanced, func(match string) string {
		// Extract UUID from match using capture group
		matches := uuidPattern.FindStringSubmatch(match)
		if len(matches) >= 2 {
			uuid := matches[1]
			if name, exists := nameMap[uuid]; exists && name != uuid {
				return fmt.Sprintf("@attr{%s:%s}", uuid, name)
			}
		}
		return match // Return original if name not found
	})

	return enhanced, nil
}

// GenerateAttributeReference creates a properly formatted @attr{uuid:name} reference
func (d *Domain) GenerateAttributeReference(ctx context.Context, attrID string) (string, error) {
	if attrID == "" {
		return "", fmt.Errorf("attribute ID cannot be empty")
	}

	// Try to resolve the name
	if d.dictRepo != nil {
		if name, err := d.ResolveAttributeName(ctx, attrID); err == nil {
			return fmt.Sprintf("@attr{%s:%s}", attrID, name), nil
		}
	}

	// Fallback to UUID only
	return fmt.Sprintf("@attr{%s}", attrID), nil
}

// ValidateAttributeReferences ensures all @attr{} references in DSL are valid
func (d *Domain) ValidateAttributeReferences(ctx context.Context, dsl string) error {
	if d.dictRepo == nil {
		return nil // Skip validation if no dictionary configured
	}

	attrPattern := regexp.MustCompile(`@attr\{([a-fA-F0-9-]{8,36})(?::([^}]+))?\}`)
	matches := attrPattern.FindAllStringSubmatch(dsl, -1)

	for _, match := range matches {
		if len(match) >= 2 {
			attrID := match[1]
			_, err := d.dictRepo.GetByID(ctx, attrID)
			if err != nil {
				return fmt.Errorf("invalid attribute reference @attr{%s}: %w", attrID, err)
			}
		}
	}

	return nil
}
