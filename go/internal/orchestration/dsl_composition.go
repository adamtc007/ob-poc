package orchestration

import (
	"context"
	"fmt"
	"sort"
	"strings"
	"time"
)

// Entity type constants
const (
	EntityTypeProperPerson = "PROPER_PERSON"
	EntityTypeCorporate    = "CORPORATE"
	EntityTypeTrust        = "TRUST"
	EntityTypePartnership  = "PARTNERSHIP"
)

// Product constants
const (
	ProductCustody             = "CUSTODY"
	ProductTrading             = "TRADING"
	ProductHedgeFundInvestment = "HEDGE_FUND_INVESTMENT"
)

// Compliance tier constants
const (
	ComplianceTierEnhanced = "ENHANCED"
)

// DSLCompositionEngine handles the merging and composition of multiple DSL templates
// into a single, coherent Master DSL document for orchestration workflows
type DSLCompositionEngine struct {
	generator *DSLGenerator
	config    *CompositionConfig
}

// CompositionConfig configures DSL composition behavior
type CompositionConfig struct {
	EnableDependencyOptimization bool
	EnableParallelGeneration     bool
	MaxCompositionDepth          int
	ValidateComposedDSL          bool
	IncludeGenerationMetadata    bool
	OptimizeExecutionOrder       bool
}

// CompositionRequest holds all requirements for DSL composition
type CompositionRequest struct {
	// Core entity information
	EntityName   string
	EntityType   string
	Jurisdiction string

	// Products and services
	Products        []string
	ProductMetadata map[string]*ProductComposition
	ServiceRequests []string

	// Workflow configuration
	WorkflowType   string
	ComplianceTier string
	UBOThreshold   int

	// Entity-specific attributes
	EntityAttributes map[string]interface{}

	// Cross-domain requirements
	RequiredDomains    []string
	DomainDependencies map[string][]string
	ExecutionStrategy  string

	// Session metadata
	SessionID   string
	CBUID       string
	RequestedAt time.Time
}

// ProductComposition defines product-specific composition requirements
type ProductComposition struct {
	ProductID            string
	Priority             int
	RequiredTemplates    []string
	ConditionalTemplates map[string]string // condition -> template
	AttributeOverrides   map[string]interface{}
	DependencyModifiers  map[string][]string
}

// CompositionResult contains the composed DSL and metadata
type CompositionResult struct {
	MasterDSL          string
	ComponentDSLs      map[string]string
	ExecutionPlan      *CompositionExecutionPlan
	DependencyGraph    map[string][]string
	GenerationMetadata *CompositionMetadata
	ValidationResults  []ValidationResult
	Warnings           []string
}

// CompositionMetadata tracks the composition process
type CompositionMetadata struct {
	TemplatesUsed        []string
	GenerationDuration   time.Duration
	ComponentCounts      map[string]int
	OptimizationsApplied []string
	ConflictsResolved    []string
}

// ValidationResult represents validation outcomes
type ValidationResult struct {
	Component string
	IsValid   bool
	Errors    []string
	Warnings  []string
}

// CompositionExecutionPlan defines the optimized execution strategy for composition
type CompositionExecutionPlan struct {
	Stages            []CompositionExecutionStage
	ParallelGroups    [][]string
	CriticalPath      []string
	EstimatedDuration time.Duration
}

// CompositionExecutionStage represents a stage in the composition execution plan
type CompositionExecutionStage struct {
	StageNumber      int
	Domains          []string
	Dependencies     []string
	EstimatedTime    time.Duration
	CanRunInParallel bool
}

// NewDSLCompositionEngine creates a new composition engine
func NewDSLCompositionEngine(generator *DSLGenerator, config *CompositionConfig) *DSLCompositionEngine {
	if config == nil {
		config = &CompositionConfig{
			EnableDependencyOptimization: true,
			EnableParallelGeneration:     true,
			MaxCompositionDepth:          5,
			ValidateComposedDSL:          true,
			IncludeGenerationMetadata:    true,
			OptimizeExecutionOrder:       true,
		}
	}

	return &DSLCompositionEngine{
		generator: generator,
		config:    config,
	}
}

// ComposeMasterDSL orchestrates the composition of all DSL components
func (ce *DSLCompositionEngine) ComposeMasterDSL(ctx context.Context, req *CompositionRequest) (*CompositionResult, error) {
	startTime := time.Now()

	result := &CompositionResult{
		ComponentDSLs: make(map[string]string),
		GenerationMetadata: &CompositionMetadata{
			ComponentCounts: make(map[string]int),
		},
	}

	// 1. Analyze composition requirements and build dependency graph
	depGraph, err := ce.buildDependencyGraph(req)
	if err != nil {
		return nil, fmt.Errorf("failed to build dependency graph: %w", err)
	}
	result.DependencyGraph = depGraph

	// 2. Generate execution plan based on dependencies
	execPlan, err := ce.generateExecutionPlan(req, depGraph)
	if err != nil {
		return nil, fmt.Errorf("failed to generate execution plan: %w", err)
	}
	result.ExecutionPlan = execPlan

	// 3. Compose entity-specific workflow DSL
	entityDSL, err := ce.composeEntityWorkflow(ctx, req)
	if err != nil {
		return nil, fmt.Errorf("failed to compose entity workflow: %w", err)
	}
	if entityDSL != "" {
		result.ComponentDSLs["entity"] = entityDSL
		result.GenerationMetadata.ComponentCounts["entity"] = 1
	}

	// 4. Compose product-specific DSLs
	productDSLs, err := ce.composeProductWorkflows(ctx, req)
	if err != nil {
		return nil, fmt.Errorf("failed to compose product workflows: %w", err)
	}
	for productID, dsl := range productDSLs {
		result.ComponentDSLs["product_"+productID] = dsl
		result.GenerationMetadata.ComponentCounts["products"]++
	}

	// 5. Compose regulatory compliance DSLs
	complianceDSLs, err := ce.composeComplianceWorkflows(ctx, req)
	if err != nil {
		return nil, fmt.Errorf("failed to compose compliance workflows: %w", err)
	}
	for complianceType, dsl := range complianceDSLs {
		result.ComponentDSLs["compliance_"+complianceType] = dsl
		result.GenerationMetadata.ComponentCounts["compliance"]++
	}

	// 6. Generate cross-domain coordination DSL
	coordinationDSL, err := ce.composeCoordinationWorkflow(ctx, req, execPlan)
	if err != nil {
		return nil, fmt.Errorf("failed to compose coordination workflow: %w", err)
	}
	if coordinationDSL != "" {
		result.ComponentDSLs["coordination"] = coordinationDSL
		result.GenerationMetadata.ComponentCounts["coordination"] = 1
	}

	// 7. Merge all components into master DSL
	masterDSL, err := ce.mergeDSLComponents(ctx, req, result.ComponentDSLs, execPlan)
	if err != nil {
		return nil, fmt.Errorf("failed to merge DSL components: %w", err)
	}
	result.MasterDSL = masterDSL

	// 8. Validate composed DSL if configured
	if ce.config.ValidateComposedDSL {
		validationResults, err := ce.validateComposedDSL(result.MasterDSL, result.ComponentDSLs)
		if err != nil {
			return nil, fmt.Errorf("DSL validation failed: %w", err)
		}
		result.ValidationResults = validationResults
	}

	// 9. Finalize metadata
	result.GenerationMetadata.GenerationDuration = time.Since(startTime)
	result.GenerationMetadata.TemplatesUsed = ce.getUsedTemplates(req)

	return result, nil
}

// buildDependencyGraph analyzes requirements and builds domain dependency relationships
func (ce *DSLCompositionEngine) buildDependencyGraph(req *CompositionRequest) (map[string][]string, error) {
	depGraph := make(map[string][]string)

	// Base entity dependencies
	switch req.EntityType {
	case EntityTypeProperPerson:
		depGraph["kyc"] = []string{"onboarding"}
		depGraph["hedge-fund-investor"] = []string{"kyc"} // if applicable
	case EntityTypeCorporate:
		depGraph["kyc"] = []string{"onboarding"}
		depGraph["ubo"] = []string{"kyc"}
	case EntityTypeTrust:
		depGraph["kyc"] = []string{"onboarding"}
		depGraph["trust-kyc"] = []string{"kyc"}
		depGraph["ubo"] = []string{"trust-kyc"}
	case EntityTypePartnership:
		depGraph["kyc"] = []string{"onboarding"}
		depGraph["ubo"] = []string{"kyc"}
	}

	// Product dependencies
	for _, product := range req.Products {
		switch product {
		case ProductCustody:
			depGraph["custody"] = []string{"onboarding"}
			if req.EntityType != EntityTypeProperPerson {
				depGraph["custody"] = append(depGraph["custody"], "ubo")
			}
		case ProductTrading:
			depGraph["trading"] = []string{"onboarding", "custody"}
		case "FUND_ACCOUNTING":
			depGraph["fund-accounting"] = []string{"custody"}
		case ProductHedgeFundInvestment:
			depGraph["hedge-fund-investor"] = []string{"kyc"}
		}
	}

	// Jurisdiction-based compliance dependencies
	switch req.Jurisdiction {
	case "US":
		depGraph["us-compliance"] = []string{"kyc"}
		if req.EntityType != EntityTypeProperPerson {
			depGraph["us-compliance"] = append(depGraph["us-compliance"], "ubo")
		}
	case "LU", "DE", "FR", "IE", "NL": // EU jurisdictions
		depGraph["eu-compliance"] = []string{"kyc"}
		if req.EntityType != "PROPER_PERSON" {
			depGraph["eu-compliance"] = append(depGraph["eu-compliance"], "ubo")
		}
	case "CH":
		depGraph["swiss-compliance"] = []string{"kyc"}
	case "GB":
		depGraph["uk-compliance"] = []string{"kyc"}
	}

	// Apply any explicit dependency modifiers from request
	for domain, additionalDeps := range req.DomainDependencies {
		if existingDeps, exists := depGraph[domain]; exists {
			// Merge dependencies, avoiding duplicates
			merged := make(map[string]bool)
			for _, dep := range existingDeps {
				merged[dep] = true
			}
			for _, dep := range additionalDeps {
				merged[dep] = true
			}

			var finalDeps []string
			for dep := range merged {
				finalDeps = append(finalDeps, dep)
			}
			depGraph[domain] = finalDeps
		} else {
			depGraph[domain] = additionalDeps
		}
	}

	return depGraph, nil
}

// generateExecutionPlan creates an optimized execution strategy
func (ce *DSLCompositionEngine) generateExecutionPlan(req *CompositionRequest, depGraph map[string][]string) (*CompositionExecutionPlan, error) {
	// Topological sort to determine execution order
	stages, err := ce.topologicalSort(depGraph)
	if err != nil {
		return nil, fmt.Errorf("failed to resolve dependencies: %w", err)
	}

	execStages := make([]CompositionExecutionStage, len(stages))
	for i, stageDomains := range stages {
		execStages[i] = CompositionExecutionStage{
			StageNumber:      i + 1,
			Domains:          stageDomains,
			Dependencies:     ce.getStageDependencies(stageDomains, depGraph),
			EstimatedTime:    ce.estimateStageTime(stageDomains),
			CanRunInParallel: len(stageDomains) > 1,
		}
	}

	return &CompositionExecutionPlan{
		Stages:            execStages,
		ParallelGroups:    ce.identifyParallelGroups(stages),
		CriticalPath:      ce.findCriticalPath(execStages),
		EstimatedDuration: ce.calculateTotalDuration(execStages),
	}, nil
}

// composeEntityWorkflow generates entity-specific workflow DSL
func (ce *DSLCompositionEngine) composeEntityWorkflow(ctx context.Context, req *CompositionRequest) (string, error) {
	templateCtx := ce.buildTemplateContext(req)

	var entityDSL string
	var err error

	switch req.EntityType {
	case EntityTypeProperPerson:
		entityDSL, err = ce.renderTemplate("individual_workflow", templateCtx)
	case EntityTypeCorporate:
		if ce.requiresUBOAnalysis(req) {
			entityDSL, err = ce.renderTemplate("corporate_ubo_workflow", templateCtx)
		} else {
			entityDSL, err = ce.renderTemplate("corporate_basic_workflow", templateCtx)
		}
	case EntityTypeTrust:
		entityDSL, err = ce.renderTemplate("trust_ubo_workflow", templateCtx)
	case EntityTypePartnership:
		entityDSL, err = ce.renderTemplate("partnership_ubo_workflow", templateCtx)
	default:
		return "", fmt.Errorf("unsupported entity type: %s", req.EntityType)
	}

	return entityDSL, err
}

// composeProductWorkflows generates product-specific DSLs
func (ce *DSLCompositionEngine) composeProductWorkflows(ctx context.Context, req *CompositionRequest) (map[string]string, error) {
	productDSLs := make(map[string]string)
	templateCtx := ce.buildTemplateContext(req)

	for _, product := range req.Products {
		var templateName string
		switch product {
		case "CUSTODY":
			templateName = "custody_requirements"
		case "TRADING":
			templateName = "trading_requirements"
		case "FUND_ACCOUNTING":
			templateName = "fund_accounting_requirements"
		case "HEDGE_FUND_INVESTMENT":
			templateName = "hedge_fund_requirements"
		default:
			templateName = "generic_product_requirements"
		}

		// Apply product-specific customizations
		if productMeta, exists := req.ProductMetadata[product]; exists {
			ce.applyProductCustomizations(templateCtx, productMeta)
		}

		dsl, err := ce.renderTemplate(templateName, templateCtx)
		if err != nil {
			return nil, fmt.Errorf("failed to render template for product %s: %w", product, err)
		}

		productDSLs[product] = dsl
	}

	return productDSLs, nil
}

// composeComplianceWorkflows generates regulatory compliance DSLs
func (ce *DSLCompositionEngine) composeComplianceWorkflows(ctx context.Context, req *CompositionRequest) (map[string]string, error) {
	complianceDSLs := make(map[string]string)
	templateCtx := ce.buildTemplateContext(req)

	// Generate jurisdiction-specific compliance
	switch req.Jurisdiction {
	case "US":
		if req.EntityType != "PROPER_PERSON" {
			dsl, err := ce.renderTemplate("fincen_control_prong", templateCtx)
			if err != nil {
				return nil, fmt.Errorf("failed to render FinCEN control prong: %w", err)
			}
			complianceDSLs["fincen_control_prong"] = dsl

			dsl, err = ce.renderTemplate("fincen_ownership_prong", templateCtx)
			if err != nil {
				return nil, fmt.Errorf("failed to render FinCEN ownership prong: %w", err)
			}
			complianceDSLs["fincen_ownership_prong"] = dsl
		}
	case "LU", "DE", "FR", "IE", "NL":
		dsl, err := ce.renderTemplate("eu_5mld_dual_prong", templateCtx)
		if err != nil {
			return nil, fmt.Errorf("failed to render EU 5MLD compliance: %w", err)
		}
		complianceDSLs["eu_5mld"] = dsl
	}

	// Generate compliance tier-specific requirements
	if req.ComplianceTier == ComplianceTierEnhanced {
		dsl, err := ce.renderTemplate("enhanced_due_diligence", templateCtx)
		if err != nil {
			return nil, fmt.Errorf("failed to render enhanced due diligence: %w", err)
		}
		complianceDSLs["enhanced_dd"] = dsl
	}

	return complianceDSLs, nil
}

// composeCoordinationWorkflow generates cross-domain coordination DSL
func (ce *DSLCompositionEngine) composeCoordinationWorkflow(ctx context.Context, req *CompositionRequest, execPlan *CompositionExecutionPlan) (string, error) {
	var coordDSL strings.Builder

	coordDSL.WriteString("; Cross-domain execution coordination\n")
	coordDSL.WriteString("(orchestration.execution.plan\n")
	coordDSL.WriteString(fmt.Sprintf("  (session.id \"%s\")\n", req.SessionID))
	coordDSL.WriteString(fmt.Sprintf("  (total.stages %d)\n", len(execPlan.Stages)))

	for _, stage := range execPlan.Stages {
		coordDSL.WriteString(fmt.Sprintf("  (stage %d\n", stage.StageNumber))
		coordDSL.WriteString("    (domains")
		for _, domain := range stage.Domains {
			coordDSL.WriteString(fmt.Sprintf(" \"%s\"", domain))
		}
		coordDSL.WriteString(")\n")

		if len(stage.Dependencies) > 0 {
			coordDSL.WriteString("    (depends.on")
			for _, dep := range stage.Dependencies {
				coordDSL.WriteString(fmt.Sprintf(" \"%s\"", dep))
			}
			coordDSL.WriteString(")\n")
		}

		if stage.CanRunInParallel {
			coordDSL.WriteString("    (parallel.execution true)\n")
		}

		coordDSL.WriteString("  )\n")
	}

	coordDSL.WriteString(")")
	return coordDSL.String(), nil
}

// mergeDSLComponents combines all DSL components into a master document
func (ce *DSLCompositionEngine) mergeDSLComponents(ctx context.Context, req *CompositionRequest, components map[string]string, execPlan *CompositionExecutionPlan) (string, error) {
	var masterDSL strings.Builder

	// Header
	masterDSL.WriteString(fmt.Sprintf("; Master DSL for %s (%s)\n", req.EntityName, req.EntityType))
	masterDSL.WriteString(fmt.Sprintf("; Generated: %s\n", req.RequestedAt.Format(time.RFC3339)))
	masterDSL.WriteString(fmt.Sprintf("; Jurisdiction: %s\n", req.Jurisdiction))
	masterDSL.WriteString(fmt.Sprintf("; Products: %s\n", strings.Join(req.Products, ", ")))
	masterDSL.WriteString("\n")

	// Session initialization
	masterDSL.WriteString("(orchestration.session.initialize\n")
	masterDSL.WriteString(fmt.Sprintf("  (session.id \"%s\")\n", req.SessionID))
	masterDSL.WriteString(fmt.Sprintf("  (cbu.id \"%s\")\n", req.CBUID))
	masterDSL.WriteString(fmt.Sprintf("  (entity.name \"%s\")\n", req.EntityName))
	masterDSL.WriteString(fmt.Sprintf("  (entity.type \"%s\")\n", req.EntityType))
	masterDSL.WriteString(fmt.Sprintf("  (jurisdiction \"%s\")\n", req.Jurisdiction))
	masterDSL.WriteString("  (products")
	for _, product := range req.Products {
		masterDSL.WriteString(fmt.Sprintf(" \"%s\"", product))
	}
	masterDSL.WriteString(")\n")
	masterDSL.WriteString(")\n\n")

	// Add components in execution order
	orderedComponents := ce.orderComponentsByExecution(components, execPlan)
	for _, componentName := range orderedComponents {
		if dsl, exists := components[componentName]; exists && dsl != "" {
			masterDSL.WriteString(fmt.Sprintf("; === %s ===\n", strings.ToUpper(componentName)))
			masterDSL.WriteString(dsl)
			masterDSL.WriteString("\n\n")
		}
	}

	return masterDSL.String(), nil
}

// Helper methods

func (ce *DSLCompositionEngine) buildTemplateContext(req *CompositionRequest) *TemplateContext {
	return &TemplateContext{
		EntityType:       req.EntityType,
		EntityName:       req.EntityName,
		Jurisdiction:     req.Jurisdiction,
		Products:         req.Products,
		ProductMetadata:  ce.convertProductMetadata(req.ProductMetadata),
		WorkflowType:     req.WorkflowType,
		ComplianceTier:   req.ComplianceTier,
		RequiredDomains:  req.RequiredDomains,
		SharedAttributes: req.EntityAttributes,
		DependencyGraph:  req.DomainDependencies,
		SessionID:        req.SessionID,
		CBUID:            req.CBUID,
		CreatedAt:        req.RequestedAt.Format(time.RFC3339),
	}
}

func (ce *DSLCompositionEngine) convertProductMetadata(metadata map[string]*ProductComposition) map[string]ProductTemplate {
	result := make(map[string]ProductTemplate)
	for productID, composition := range metadata {
		result[productID] = ProductTemplate{
			ProductID:    productID,
			Attributes:   ce.extractAttributes(composition.AttributeOverrides),
			DSLFragments: composition.RequiredTemplates,
		}
	}
	return result
}

func (ce *DSLCompositionEngine) extractAttributes(overrides map[string]interface{}) []string {
	var attrs []string
	for key := range overrides {
		attrs = append(attrs, key)
	}
	sort.Strings(attrs)
	return attrs
}

func (ce *DSLCompositionEngine) requiresUBOAnalysis(req *CompositionRequest) bool {
	return req.EntityType != "PROPER_PERSON" && (req.Jurisdiction == "US" || ce.isEUJurisdiction(req.Jurisdiction))
}

func (ce *DSLCompositionEngine) isEUJurisdiction(jurisdiction string) bool {
	euJurisdictions := map[string]bool{
		"LU": true, "DE": true, "FR": true, "IE": true, "NL": true,
		"IT": true, "ES": true, "AT": true, "BE": true, "PT": true,
	}
	return euJurisdictions[jurisdiction]
}

func (ce *DSLCompositionEngine) renderTemplate(templateName string, ctx *TemplateContext) (string, error) {
	// This is a placeholder - in a real implementation, you'd load templates from files
	// or use the embedded templates in the DSLGenerator
	return ce.generator.generateEntityWorkflow(ctx)
}

func (ce *DSLCompositionEngine) applyProductCustomizations(ctx *TemplateContext, meta *ProductComposition) {
	// Apply product-specific customizations to template context
	for key, value := range meta.AttributeOverrides {
		ctx.SharedAttributes[key] = value
	}
}

func (ce *DSLCompositionEngine) topologicalSort(depGraph map[string][]string) ([][]string, error) {
	// Simplified topological sort - returns stages of execution
	visited := make(map[string]bool)
	var stages [][]string

	// This is a simplified implementation - a real one would handle complex dependency cycles
	// and create proper execution stages

	// Stage 1: No dependencies
	var stage1 []string
	for domain := range depGraph {
		if len(depGraph[domain]) == 0 {
			stage1 = append(stage1, domain)
			visited[domain] = true
		}
	}
	if len(stage1) > 0 {
		stages = append(stages, stage1)
	}

	// Subsequent stages
	for len(visited) < len(depGraph) {
		var currentStage []string
		for domain := range depGraph {
			if visited[domain] {
				continue
			}

			// Check if all dependencies are satisfied
			allDepsSatisfied := true
			for _, dep := range depGraph[domain] {
				if !visited[dep] {
					allDepsSatisfied = false
					break
				}
			}

			if allDepsSatisfied {
				currentStage = append(currentStage, domain)
			}
		}

		if len(currentStage) == 0 {
			return nil, fmt.Errorf("circular dependency detected")
		}

		for _, domain := range currentStage {
			visited[domain] = true
		}
		stages = append(stages, currentStage)
	}

	return stages, nil
}

func (ce *DSLCompositionEngine) getStageDependencies(domains []string, depGraph map[string][]string) []string {
	depSet := make(map[string]bool)
	for _, domain := range domains {
		for _, dep := range depGraph[domain] {
			depSet[dep] = true
		}
	}

	var deps []string
	for dep := range depSet {
		deps = append(deps, dep)
	}
	sort.Strings(deps)
	return deps
}

func (ce *DSLCompositionEngine) estimateStageTime(domains []string) time.Duration {
	// Simple estimation based on domain complexity
	baseTime := 30 * time.Second
	complexityMultiplier := map[string]float64{
		"onboarding": 1.0,
		"kyc":        2.0,
		"ubo":        3.0,
		"trust-kyc":  2.5,
		"custody":    2.0,
		"trading":    1.5,
		"compliance": 2.5,
	}

	maxMultiplier := 1.0
	for _, domain := range domains {
		if mult, exists := complexityMultiplier[domain]; exists && mult > maxMultiplier {
			maxMultiplier = mult
		}
	}

	return time.Duration(float64(baseTime) * maxMultiplier)
}

func (ce *DSLCompositionEngine) identifyParallelGroups(stages [][]string) [][]string {
	var parallelGroups [][]string
	for _, stage := range stages {
		if len(stage) > 1 {
			parallelGroups = append(parallelGroups, stage)
		}
	}
	return parallelGroups
}

func (ce *DSLCompositionEngine) findCriticalPath(stages []CompositionExecutionStage) []string {
	// Simplified critical path - just return the longest dependency chain
	var criticalPath []string
	for _, stage := range stages {
		if len(stage.Domains) == 1 {
			criticalPath = append(criticalPath, stage.Domains[0])
		}
	}
	return criticalPath
}

func (ce *DSLCompositionEngine) calculateTotalDuration(stages []CompositionExecutionStage) time.Duration {
	total := time.Duration(0)
	for _, stage := range stages {
		total += stage.EstimatedTime
	}
	return total
}

func (ce *DSLCompositionEngine) validateComposedDSL(masterDSL string, components map[string]string) ([]ValidationResult, error) {
	var results []ValidationResult

	// Validate master DSL
	masterResult := ValidationResult{
		Component: "master",
		IsValid:   true,
	}

	// Basic syntax validation
	if strings.Count(masterDSL, "(") != strings.Count(masterDSL, ")") {
		masterResult.IsValid = false
		masterResult.Errors = append(masterResult.Errors, "Unmatched parentheses in master DSL")
	}

	results = append(results, masterResult)

	// Validate each component
	for name, dsl := range components {
		componentResult := ValidationResult{
			Component: name,
			IsValid:   true,
		}

		if strings.Count(dsl, "(") != strings.Count(dsl, ")") {
			componentResult.IsValid = false
			componentResult.Errors = append(componentResult.Errors, "Unmatched parentheses")
		}

		results = append(results, componentResult)
	}

	return results, nil
}

func (ce *DSLCompositionEngine) orderComponentsByExecution(components map[string]string, execPlan *CompositionExecutionPlan) []string {
	var orderedComponents []string

	// Add components in execution order
	for _, stage := range execPlan.Stages {
		for _, domain := range stage.Domains {
			// Map domain names to component names
			for componentName := range components {
				if strings.Contains(componentName, domain) ||
					strings.Contains(domain, componentName) ||
					componentName == "entity" ||
					componentName == "coordination" {
					orderedComponents = append(orderedComponents, componentName)
				}
			}
		}
	}

	// Add any remaining components
	for componentName := range components {
		found := false
		for _, existing := range orderedComponents {
			if existing == componentName {
				found = true
				break
			}
		}
		if !found {
			orderedComponents = append(orderedComponents, componentName)
		}
	}

	return ce.deduplicateSlice(orderedComponents)
}

func (ce *DSLCompositionEngine) deduplicateSlice(slice []string) []string {
	seen := make(map[string]bool)
	var result []string

	for _, item := range slice {
		if !seen[item] {
			seen[item] = true
			result = append(result, item)
		}
	}

	return result
}

func (ce *DSLCompositionEngine) getUsedTemplates(req *CompositionRequest) []string {
	var templates []string

	// Entity templates
	templates = append(templates, req.EntityType+"_workflow")

	// Product templates
	for _, product := range req.Products {
		templates = append(templates, product+"_requirements")
	}

	// Compliance templates
	templates = append(templates, req.Jurisdiction+"_compliance")

	return templates
}
