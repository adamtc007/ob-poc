package orchestration

import (
	"bytes"
	"context"
	"fmt"
	"strings"
	"text/template"
)

// DSLGenerator handles dynamic DSL generation based on templates and context
type DSLGenerator struct {
	templates map[string]*template.Template
	config    *DSLGeneratorConfig
}

// DSLGeneratorConfig configures DSL generation behavior
type DSLGeneratorConfig struct {
	EnableTemplateCache bool
	MaxTemplateDepth    int
	ValidateGenerated   bool
	IncludeComments     bool
}

// TemplateContext holds all data needed for DSL template generation
type TemplateContext struct {
	// Entity information
	EntityType   string
	EntityName   string
	Jurisdiction string

	// Product requirements
	Products        []string
	ProductMetadata map[string]ProductTemplate

	// Workflow configuration
	WorkflowType    string
	ComplianceTier  string
	RequiredDomains []string

	// Cross-domain context
	SharedAttributes map[string]interface{}
	DependencyGraph  map[string][]string
	ExecutionStages  [][]string

	// Session metadata
	SessionID string
	CBUID     string
	CreatedAt string
}

// ProductTemplate defines product-specific DSL requirements
type ProductTemplate struct {
	ProductID       string
	RequiredVerbs   []string
	Attributes      []string
	Dependencies    []string
	ComplianceRules []string
	DSLFragments    []string
}

// EntityTemplate defines entity-type specific workflows
type EntityTemplate struct {
	EntityType      string
	BaseWorkflow    string
	RequiredDomains []string
	Attributes      []string
	ComplianceRules []string
}

// NewDSLGenerator creates a new DSL generator with default configuration
func NewDSLGenerator(config *DSLGeneratorConfig) *DSLGenerator {
	if config == nil {
		config = &DSLGeneratorConfig{
			EnableTemplateCache: true,
			MaxTemplateDepth:    10,
			ValidateGenerated:   true,
			IncludeComments:     false,
		}
	}

	return &DSLGenerator{
		templates: make(map[string]*template.Template),
		config:    config,
	}
}

// GenerateMasterDSL generates the complete DSL document for an orchestration session
func (g *DSLGenerator) GenerateMasterDSL(ctx context.Context, templateCtx *TemplateContext) (string, error) {
	var masterDSL strings.Builder

	// Generate header with context initialization
	header, err := g.generateHeader(templateCtx)
	if err != nil {
		return "", fmt.Errorf("failed to generate DSL header: %w", err)
	}
	masterDSL.WriteString(header)

	// Generate entity-specific workflows
	entityDSL, err := g.generateEntityWorkflow(templateCtx)
	if err != nil {
		return "", fmt.Errorf("failed to generate entity workflow: %w", err)
	}
	if entityDSL != "" {
		masterDSL.WriteString("\n\n")
		masterDSL.WriteString(entityDSL)
	}

	// Generate product-specific requirements
	productDSL, err := g.generateProductRequirements(templateCtx)
	if err != nil {
		return "", fmt.Errorf("failed to generate product requirements: %w", err)
	}
	if productDSL != "" {
		masterDSL.WriteString("\n\n")
		masterDSL.WriteString(productDSL)
	}

	// Generate compliance requirements
	complianceDSL, err := g.generateComplianceRequirements(templateCtx)
	if err != nil {
		return "", fmt.Errorf("failed to generate compliance requirements: %w", err)
	}
	if complianceDSL != "" {
		masterDSL.WriteString("\n\n")
		masterDSL.WriteString(complianceDSL)
	}

	// Generate execution coordination
	coordinationDSL, err := g.generateExecutionCoordination(templateCtx)
	if err != nil {
		return "", fmt.Errorf("failed to generate execution coordination: %w", err)
	}
	if coordinationDSL != "" {
		masterDSL.WriteString("\n\n")
		masterDSL.WriteString(coordinationDSL)
	}

	result := masterDSL.String()

	// Validate generated DSL if configured
	if g.config.ValidateGenerated {
		if err := g.validateGeneratedDSL(result); err != nil {
			return "", fmt.Errorf("generated DSL validation failed: %w", err)
		}
	}

	return result, nil
}

// generateHeader creates the DSL header with context initialization
func (g *DSLGenerator) generateHeader(ctx *TemplateContext) (string, error) {
	headerTemplate := `; Auto-generated Master DSL for {{.SessionID}}
; Generated at: {{.CreatedAt}}
; Entity: {{.EntityName}} ({{.EntityType}})
; Jurisdiction: {{.Jurisdiction}}
; Products: {{range $i, $p := .Products}}{{if $i}}, {{end}}{{$p}}{{end}}

(orchestration.initialize
  (session.id "{{.SessionID}}")
  (cbu.id "{{.CBUID}}")
  (entity.name "{{.EntityName}}")
  (entity.type "{{.EntityType}}")
  (jurisdiction "{{.Jurisdiction}}")
  (products {{range .Products}}"{{.}}" {{end}})
  (workflow.type "{{.WorkflowType}}")
  (compliance.tier "{{.ComplianceTier}}")
)`

	tmpl, err := template.New("header").Parse(headerTemplate)
	if err != nil {
		return "", fmt.Errorf("failed to parse header template: %w", err)
	}

	var buf bytes.Buffer
	if err := tmpl.Execute(&buf, ctx); err != nil {
		return "", fmt.Errorf("failed to execute header template: %w", err)
	}

	return buf.String(), nil
}

// generateEntityWorkflow creates entity-type specific workflow DSL
func (g *DSLGenerator) generateEntityWorkflow(ctx *TemplateContext) (string, error) {
	var entityTemplate string

	switch ctx.EntityType {
	case "PROPER_PERSON":
		entityTemplate = g.getIndividualTemplate()
	case "CORPORATE":
		entityTemplate = g.getCorporateTemplate()
	case "TRUST":
		entityTemplate = g.getTrustTemplate()
	case "PARTNERSHIP":
		entityTemplate = g.getPartnershipTemplate()
	default:
		return "", fmt.Errorf("unsupported entity type: %s", ctx.EntityType)
	}

	tmpl, err := template.New("entity").Parse(entityTemplate)
	if err != nil {
		return "", fmt.Errorf("failed to parse entity template: %w", err)
	}

	var buf bytes.Buffer
	if err := tmpl.Execute(&buf, ctx); err != nil {
		return "", fmt.Errorf("failed to execute entity template: %w", err)
	}

	return buf.String(), nil
}

// generateProductRequirements creates product-specific DSL requirements
func (g *DSLGenerator) generateProductRequirements(ctx *TemplateContext) (string, error) {
	if len(ctx.Products) == 0 {
		return "", nil
	}

	var productDSL strings.Builder

	productDSL.WriteString("; Product-specific requirements\n")
	productDSL.WriteString("(products.configure\n")

	for _, product := range ctx.Products {
		productTemplate, exists := ctx.ProductMetadata[product]
		if !exists {
			// Use default product template
			productTemplate = g.getDefaultProductTemplate(product)
		}

		productDSL.WriteString(fmt.Sprintf("  (product \"%s\"\n", product))

		// Add product-specific attributes
		if len(productTemplate.Attributes) > 0 {
			productDSL.WriteString("    (attributes")
			for _, attr := range productTemplate.Attributes {
				productDSL.WriteString(fmt.Sprintf(" \"%s\"", attr))
			}
			productDSL.WriteString(")\n")
		}

		// Add product-specific dependencies
		if len(productTemplate.Dependencies) > 0 {
			productDSL.WriteString("    (depends.on")
			for _, dep := range productTemplate.Dependencies {
				productDSL.WriteString(fmt.Sprintf(" \"%s\"", dep))
			}
			productDSL.WriteString(")\n")
		}

		// Add DSL fragments
		for _, fragment := range productTemplate.DSLFragments {
			productDSL.WriteString(fmt.Sprintf("    %s\n", fragment))
		}

		productDSL.WriteString("  )\n")
	}

	productDSL.WriteString(")")

	return productDSL.String(), nil
}

// generateComplianceRequirements creates jurisdiction-specific compliance DSL
func (g *DSLGenerator) generateComplianceRequirements(ctx *TemplateContext) (string, error) {
	var complianceTemplate string

	switch ctx.Jurisdiction {
	case "US":
		complianceTemplate = g.getUSComplianceTemplate()
	case "LU", "DE", "FR", "IE", "NL": // EU jurisdictions
		complianceTemplate = g.getEUComplianceTemplate()
	case "CH":
		complianceTemplate = g.getSwissComplianceTemplate()
	case "GB":
		complianceTemplate = g.getUKComplianceTemplate()
	default:
		// Use generic compliance template
		complianceTemplate = g.getGenericComplianceTemplate()
	}

	if complianceTemplate == "" {
		return "", nil
	}

	tmpl, err := template.New("compliance").Parse(complianceTemplate)
	if err != nil {
		return "", fmt.Errorf("failed to parse compliance template: %w", err)
	}

	var buf bytes.Buffer
	if err := tmpl.Execute(&buf, ctx); err != nil {
		return "", fmt.Errorf("failed to execute compliance template: %w", err)
	}

	return buf.String(), nil
}

// generateExecutionCoordination creates cross-domain execution coordination DSL
func (g *DSLGenerator) generateExecutionCoordination(ctx *TemplateContext) (string, error) {
	if len(ctx.ExecutionStages) == 0 {
		return "", nil
	}

	var coordDSL strings.Builder

	coordDSL.WriteString("; Execution coordination and dependencies\n")
	coordDSL.WriteString("(execution.plan\n")

	for i, stage := range ctx.ExecutionStages {
		coordDSL.WriteString(fmt.Sprintf("  (stage %d\n", i+1))
		coordDSL.WriteString("    (domains")
		for _, domain := range stage {
			coordDSL.WriteString(fmt.Sprintf(" \"%s\"", domain))
		}
		coordDSL.WriteString(")\n")

		// Add dependencies if not the first stage
		if i > 0 {
			coordDSL.WriteString(fmt.Sprintf("    (depends.on.stage %d)\n", i))
		}

		coordDSL.WriteString("  )\n")
	}

	coordDSL.WriteString(")")

	return coordDSL.String(), nil
}

// validateGeneratedDSL performs basic validation on generated DSL
func (g *DSLGenerator) validateGeneratedDSL(dsl string) error {
	// Basic parenthesis matching
	openCount := strings.Count(dsl, "(")
	closeCount := strings.Count(dsl, ")")

	if openCount != closeCount {
		return fmt.Errorf("unmatched parentheses: %d open, %d close", openCount, closeCount)
	}

	// Check for empty DSL
	if strings.TrimSpace(dsl) == "" {
		return fmt.Errorf("generated DSL is empty")
	}

	// TODO: Add more sophisticated validation
	// - Verb validation against approved vocabulary
	// - AttributeID format validation
	// - Cross-reference validation

	return nil
}

// Entity template methods
func (g *DSLGenerator) getIndividualTemplate() string {
	return `; Individual entity workflow
(workflow.entity.individual
  (entity.name "{{.EntityName}}")
  (kyc.tier "STANDARD")
  (domains "onboarding" "kyc")
  {{if contains .Products "HEDGE_FUND_INVESTMENT"}}(enhanced.kyc true){{end}}
)`
}

func (g *DSLGenerator) getCorporateTemplate() string {
	return `; Corporate entity workflow
(workflow.entity.corporate
  (entity.name "{{.EntityName}}")
  (entity.type "{{.EntityType}}")
  (jurisdiction "{{.Jurisdiction}}")
  (kyc.tier "{{.ComplianceTier}}")
  (domains "onboarding" "kyc" "ubo")
  (ubo.threshold 25)
  {{if eq .ComplianceTier "ENHANCED"}}(enhanced.due.diligence true){{end}}
)`
}

func (g *DSLGenerator) getTrustTemplate() string {
	return `; Trust entity workflow
(workflow.entity.trust
  (entity.name "{{.EntityName}}")
  (jurisdiction "{{.Jurisdiction}}")
  (kyc.tier "{{.ComplianceTier}}")
  (domains "onboarding" "kyc" "ubo" "trust-kyc")
  (trust.type "DISCRETIONARY")
  (beneficiary.discovery true)
  (settlor.verification true)
)`
}

func (g *DSLGenerator) getPartnershipTemplate() string {
	return `; Partnership entity workflow
(workflow.entity.partnership
  (entity.name "{{.EntityName}}")
  (jurisdiction "{{.Jurisdiction}}")
  (domains "onboarding" "kyc" "ubo")
  (partner.verification true)
  (managing.partner.identification true)
)`
}

// Compliance template methods
func (g *DSLGenerator) getUSComplianceTemplate() string {
	return `; US regulatory compliance
(compliance.us
  (jurisdiction "{{.Jurisdiction}}")
  (patriot.act true)
  (fincen.beneficial.ownership true)
  (ofac.screening true)
  {{if eq .EntityType "CORPORATE"}}(control.prong true){{end}}
  {{if eq .EntityType "TRUST"}}(ownership.prong true){{end}}
)`
}

func (g *DSLGenerator) getEUComplianceTemplate() string {
	return `; EU regulatory compliance (5MLD)
(compliance.eu
  (jurisdiction "{{.Jurisdiction}}")
  (amld5 true)
  (dual.prong.test true)
  (gdpr.compliance true)
  (pep.screening true)
  (sanctions.screening true)
)`
}

func (g *DSLGenerator) getSwissComplianceTemplate() string {
	return `; Swiss regulatory compliance
(compliance.swiss
  (jurisdiction "CH")
  (amla true)
  (due.diligence.enhanced {{eq .ComplianceTier "ENHANCED"}})
  (finma.requirements true)
)`
}

func (g *DSLGenerator) getUKComplianceTemplate() string {
	return `; UK regulatory compliance
(compliance.uk
  (jurisdiction "GB")
  (money.laundering.regulations true)
  (psc.register true)
  (fca.requirements true)
)`
}

func (g *DSLGenerator) getGenericComplianceTemplate() string {
	return `; Generic compliance requirements
(compliance.generic
  (jurisdiction "{{.Jurisdiction}}")
  (kyc.standard true)
  (aml.screening true)
  (sanctions.check true)
)`
}

// Helper method for default product templates
func (g *DSLGenerator) getDefaultProductTemplate(product string) ProductTemplate {
	switch product {
	case "CUSTODY":
		return ProductTemplate{
			ProductID:     "CUSTODY",
			RequiredVerbs: []string{"custody.account.create", "custody.signatory.verify"},
			Attributes:    []string{"custody.account_number", "custody.signatory_authority"},
			Dependencies:  []string{"onboarding"},
			DSLFragments:  []string{"(custody.account.type \"PRIME_BROKERAGE\")"},
		}
	case "TRADING":
		return ProductTemplate{
			ProductID:     "TRADING",
			RequiredVerbs: []string{"trading.permissions.grant", "trading.limits.set"},
			Attributes:    []string{"trading.permissions", "trading.limits"},
			Dependencies:  []string{"onboarding", "custody"},
			DSLFragments:  []string{"(trading.instruments \"EQUITIES\" \"FIXED_INCOME\")"},
		}
	case "FUND_ACCOUNTING":
		return ProductTemplate{
			ProductID:     "FUND_ACCOUNTING",
			RequiredVerbs: []string{"fund-accounting.setup", "fund-accounting.valuation"},
			Attributes:    []string{"fund-accounting.frequency", "fund-accounting.method"},
			Dependencies:  []string{"custody"},
			DSLFragments:  []string{"(reporting \"DAILY\") (valuation \"MARK_TO_MARKET\")"},
		}
	case "HEDGE_FUND_INVESTMENT":
		return ProductTemplate{
			ProductID:     "HEDGE_FUND_INVESTMENT",
			RequiredVerbs: []string{"hedge-fund.investor.onboard", "hedge-fund.subscription.process"},
			Attributes:    []string{"hedge-fund.investor_type", "hedge-fund.accreditation"},
			Dependencies:  []string{"kyc"},
			DSLFragments:  []string{"(investor.accreditation \"QUALIFIED_PURCHASER\")"},
		}
	default:
		return ProductTemplate{
			ProductID:    product,
			DSLFragments: []string{fmt.Sprintf("(product.generic \"%s\")", product)},
		}
	}
}

// Template helper functions that would be available in templates
func init() {
	// This would register template helper functions if needed
	// For now, we keep it simple with basic Go template functions
}
