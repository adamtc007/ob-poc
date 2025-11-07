package orchestration

import (
	"context"
	"fmt"
	"strings"

	"dsl-ob-poc/internal/dictionary"
)

// OrchestrationVocabulary defines orchestration-specific DSL verbs and validation
type OrchestrationVocabulary struct {
	// Core orchestration verbs - Phase 3 implementation
	orchestrationVerbs map[string]VerbDefinition

	// Cross-domain state management
	stateManagementVerbs map[string]VerbDefinition

	// Workflow coordination verbs
	workflowVerbs map[string]VerbDefinition

	// Domain communication verbs
	communicationVerbs map[string]VerbDefinition

	// Product integration verbs
	productVerbs map[string]VerbDefinition
}

// VerbDefinition describes an orchestration DSL verb
type VerbDefinition struct {
	Verb        string
	Category    string
	Description string
	Parameters  []VerbParameter
	Examples    []string
	Domains     []string // Domains this verb applies to
	Phase       int      // Implementation phase
}

// VerbParameter defines a verb parameter
type VerbParameter struct {
	Name        string
	Type        string // "string", "attributeID", "domainList", "entityRef"
	Required    bool
	Description string
	Examples    []string
}

// NewOrchestrationVocabulary creates the orchestration vocabulary
func NewOrchestrationVocabulary() *OrchestrationVocabulary {
	vocab := &OrchestrationVocabulary{
		orchestrationVerbs:   make(map[string]VerbDefinition),
		stateManagementVerbs: make(map[string]VerbDefinition),
		workflowVerbs:        make(map[string]VerbDefinition),
		communicationVerbs:   make(map[string]VerbDefinition),
		productVerbs:         make(map[string]VerbDefinition),
	}

	vocab.initializeOrchestrationVerbs()
	vocab.initializeStateManagementVerbs()
	vocab.initializeWorkflowVerbs()
	vocab.initializeCommunicationVerbs()
	vocab.initializeProductVerbs()

	return vocab
}

// initializeOrchestrationVerbs defines core orchestration coordination verbs
func (ov *OrchestrationVocabulary) initializeOrchestrationVerbs() {
	// Context Management Verbs
	ov.orchestrationVerbs["orchestration.initialize"] = VerbDefinition{
		Verb:        "orchestration.initialize",
		Category:    "context",
		Description: "Initialize orchestration session with cross-domain context",
		Parameters: []VerbParameter{
			{Name: "session.id", Type: "string", Required: true, Description: "Unique session identifier"},
			{Name: "cbu.id", Type: "string", Required: true, Description: "CBU identifier"},
			{Name: "entity.name", Type: "string", Required: true, Description: "Primary entity name"},
			{Name: "entity.type", Type: "string", Required: true, Description: "Entity type (PROPER_PERSON, CORPORATE, TRUST)"},
			{Name: "jurisdiction", Type: "string", Required: true, Description: "Primary jurisdiction"},
			{Name: "products", Type: "stringList", Required: true, Description: "Required products"},
		},
		Examples: []string{
			`(orchestration.initialize
			  (session.id "orch-001")
			  (cbu.id "CBU-GS-001")
			  (entity.name "Goldman Sachs Asset Management")
			  (entity.type "CORPORATE")
			  (jurisdiction "US")
			  (products "CUSTODY" "TRADING"))`,
		},
		Domains: []string{"orchestration"},
		Phase:   3,
	}

	ov.orchestrationVerbs["orchestration.context.analyze"] = VerbDefinition{
		Verb:        "orchestration.context.analyze",
		Category:    "context",
		Description: "Analyze entity context to determine required domains and dependencies",
		Parameters: []VerbParameter{
			{Name: "entity.type", Type: "string", Required: true, Description: "Entity type to analyze"},
			{Name: "products", Type: "stringList", Required: true, Description: "Requested products"},
			{Name: "jurisdiction", Type: "string", Required: true, Description: "Entity jurisdiction"},
			{Name: "complexity.assessment", Type: "string", Required: false, Description: "Complexity level (LOW, MEDIUM, HIGH)"},
		},
		Examples: []string{
			`(orchestration.context.analyze
			  (entity.type "TRUST")
			  (products "CUSTODY" "TRADING")
			  (jurisdiction "LU")
			  (complexity.assessment "HIGH"))`,
		},
		Domains: []string{"orchestration"},
		Phase:   3,
	}

	ov.orchestrationVerbs["orchestration.domains.determine"] = VerbDefinition{
		Verb:        "orchestration.domains.determine",
		Category:    "context",
		Description: "Determine required domains based on context analysis",
		Parameters: []VerbParameter{
			{Name: "primary.domain", Type: "string", Required: true, Description: "Primary orchestrating domain"},
			{Name: "required.domains", Type: "domainList", Required: true, Description: "List of required domains"},
			{Name: "dependencies", Type: "dependencyMap", Required: true, Description: "Domain dependency relationships"},
			{Name: "parallel.groups", Type: "domainGroupList", Required: false, Description: "Domains that can run in parallel"},
		},
		Examples: []string{
			`(orchestration.domains.determine
			  (primary.domain "onboarding")
			  (required.domains "onboarding" "kyc" "ubo" "custody" "trading")
			  (dependencies (ubo ["kyc"]) (custody ["onboarding"]) (trading ["custody"]))
			  (parallel.groups [["kyc" "onboarding"] ["custody" "trading"]]))`,
		},
		Domains: []string{"orchestration"},
		Phase:   3,
	}

	ov.orchestrationVerbs["orchestration.execution.plan"] = VerbDefinition{
		Verb:        "orchestration.execution.plan",
		Category:    "execution",
		Description: "Define execution plan with stages and dependencies",
		Parameters: []VerbParameter{
			{Name: "session.id", Type: "string", Required: true, Description: "Session identifier"},
			{Name: "total.stages", Type: "number", Required: true, Description: "Total number of execution stages"},
			{Name: "stage", Type: "executionStage", Required: true, Description: "Individual execution stage definition"},
		},
		Examples: []string{
			`(orchestration.execution.plan
			  (session.id "orch-001")
			  (total.stages 3)
			  (stage 1 (domains "onboarding" "kyc") (parallel.execution true))
			  (stage 2 (domains "ubo") (depends.on "kyc"))
			  (stage 3 (domains "custody" "trading") (depends.on "onboarding")))`,
		},
		Domains: []string{"orchestration"},
		Phase:   3,
	}
}

// initializeStateManagementVerbs defines cross-domain state management verbs
func (ov *OrchestrationVocabulary) initializeStateManagementVerbs() {
	ov.stateManagementVerbs["state.initialize.shared"] = VerbDefinition{
		Verb:        "state.initialize.shared",
		Category:    "state",
		Description: "Initialize shared cross-domain state with AttributeID references",
		Parameters: []VerbParameter{
			{Name: "session.id", Type: "string", Required: true, Description: "Orchestration session ID"},
			{Name: "primary.entity", Type: "attributeID", Required: true, Description: "Primary entity AttributeID"},
			{Name: "shared.attributes", Type: "attributeIDList", Required: true, Description: "Shared attribute references"},
			{Name: "accessible.domains", Type: "domainList", Required: true, Description: "Domains with access to shared state"},
		},
		Examples: []string{
			`(state.initialize.shared
			  (session.id "orch-001")
			  (primary.entity @attr{entity.primary.id})
			  (shared.attributes @attr{entity.legal_name} @attr{entity.jurisdiction} @attr{entity.risk_profile})
			  (accessible.domains "onboarding" "kyc" "ubo" "custody"))`,
		},
		Domains: []string{"orchestration", "onboarding", "kyc", "ubo"},
		Phase:   3,
	}

	ov.stateManagementVerbs["state.share.cross.domain"] = VerbDefinition{
		Verb:        "state.share.cross.domain",
		Category:    "state",
		Description: "Share state attributes across domains with referential integrity",
		Parameters: []VerbParameter{
			{Name: "from.domain", Type: "string", Required: true, Description: "Source domain"},
			{Name: "to.domains", Type: "domainList", Required: true, Description: "Target domains"},
			{Name: "attributes", Type: "attributeIDList", Required: true, Description: "Attributes to share"},
			{Name: "access.mode", Type: "string", Required: false, Description: "Access mode (READ, WRITE, READ_WRITE)"},
			{Name: "sync.strategy", Type: "string", Required: false, Description: "Synchronization strategy"},
		},
		Examples: []string{
			`(state.share.cross.domain
			  (from.domain "kyc")
			  (to.domains "ubo" "compliance")
			  (attributes @attr{kyc.risk_rating} @attr{kyc.pep_status} @attr{kyc.sanctions_status})
			  (access.mode "READ")
			  (sync.strategy "IMMEDIATE"))`,
		},
		Domains: []string{"orchestration", "kyc", "ubo", "compliance"},
		Phase:   3,
	}

	ov.stateManagementVerbs["state.sync.attributes"] = VerbDefinition{
		Verb:        "state.sync.attributes",
		Category:    "state",
		Description: "Synchronize attribute values across domains",
		Parameters: []VerbParameter{
			{Name: "attributes", Type: "attributeIDList", Required: true, Description: "Attributes to synchronize"},
			{Name: "between.domains", Type: "domainList", Required: true, Description: "Domains to synchronize between"},
			{Name: "conflict.resolution", Type: "string", Required: false, Description: "Conflict resolution strategy"},
			{Name: "validation.required", Type: "boolean", Required: false, Description: "Whether validation is required"},
		},
		Examples: []string{
			`(state.sync.attributes
			  (attributes @attr{entity.legal_name} @attr{entity.address})
			  (between.domains "onboarding" "kyc" "custody")
			  (conflict.resolution "SOURCE_WINS")
			  (validation.required true))`,
		},
		Domains: []string{"orchestration"},
		Phase:   3,
	}

	ov.stateManagementVerbs["state.validate.consistency"] = VerbDefinition{
		Verb:        "state.validate.consistency",
		Category:    "state",
		Description: "Validate cross-domain state consistency and referential integrity",
		Parameters: []VerbParameter{
			{Name: "scope.domains", Type: "domainList", Required: true, Description: "Domains to validate"},
			{Name: "critical.attributes", Type: "attributeIDList", Required: true, Description: "Critical attributes for consistency"},
			{Name: "validation.rules", Type: "ruleList", Required: false, Description: "Custom validation rules"},
			{Name: "fail.on.inconsistency", Type: "boolean", Required: false, Description: "Whether to fail on inconsistency"},
		},
		Examples: []string{
			`(state.validate.consistency
			  (scope.domains "kyc" "ubo" "compliance")
			  (critical.attributes @attr{entity.legal_name} @attr{ubo.control_persons})
			  (validation.rules "CROSS_REFERENCE_CHECK" "COMPLETENESS_CHECK")
			  (fail.on.inconsistency true))`,
		},
		Domains: []string{"orchestration"},
		Phase:   3,
	}
}

// initializeWorkflowVerbs defines workflow coordination verbs
func (ov *OrchestrationVocabulary) initializeWorkflowVerbs() {
	ov.workflowVerbs["workflow.execute.subdomain"] = VerbDefinition{
		Verb:        "workflow.execute.subdomain",
		Category:    "workflow",
		Description: "Execute workflow in a specific domain with context passing",
		Parameters: []VerbParameter{
			{Name: "domain", Type: "string", Required: true, Description: "Target domain name"},
			{Name: "template", Type: "string", Required: false, Description: "Workflow template to use"},
			{Name: "entity.target", Type: "attributeID", Required: true, Description: "Target entity AttributeID"},
			{Name: "depends.on", Type: "attributeIDList", Required: false, Description: "Dependencies (AttributeIDs)"},
			{Name: "result.binding", Type: "attributeID", Required: false, Description: "Where to bind results"},
			{Name: "context.data", Type: "contextMap", Required: false, Description: "Context data to pass"},
		},
		Examples: []string{
			`(workflow.execute.subdomain
			  (domain "ubo")
			  (template "trust-fatf-ubo")
			  (entity.target @attr{trust.entity.id})
			  (depends.on @attr{kyc.completion.status})
			  (result.binding @attr{ubo.analysis.complete})
			  (context.data (jurisdiction "LU") (trust.type "DISCRETIONARY")))`,
		},
		Domains: []string{"orchestration"},
		Phase:   3,
	}

	ov.workflowVerbs["workflow.coordinate.parallel"] = VerbDefinition{
		Verb:        "workflow.coordinate.parallel",
		Category:    "workflow",
		Description: "Coordinate parallel workflow execution with synchronization points",
		Parameters: []VerbParameter{
			{Name: "workflows", Type: "workflowList", Required: true, Description: "Parallel workflows to coordinate"},
			{Name: "sync.points", Type: "syncPointList", Required: true, Description: "Synchronization checkpoints"},
			{Name: "timeout", Type: "duration", Required: false, Description: "Execution timeout"},
			{Name: "failure.strategy", Type: "string", Required: false, Description: "Strategy on workflow failure"},
		},
		Examples: []string{
			`(workflow.coordinate.parallel
			  (workflows
			    (workflow "kyc" (domain "kyc"))
			    (workflow "onboarding" (domain "onboarding")))
			  (sync.points
			    (sync.point "initial_data_collection")
			    (sync.point "entity_verification_complete"))
			  (timeout "30m")
			  (failure.strategy "CONTINUE_OTHERS"))`,
		},
		Domains: []string{"orchestration"},
		Phase:   3,
	}

	ov.workflowVerbs["workflow.wait.for.completion"] = VerbDefinition{
		Verb:        "workflow.wait.for.completion",
		Category:    "workflow",
		Description: "Wait for workflow completion before proceeding",
		Parameters: []VerbParameter{
			{Name: "workflows", Type: "workflowRefList", Required: true, Description: "Workflows to wait for"},
			{Name: "timeout", Type: "duration", Required: false, Description: "Maximum wait time"},
			{Name: "partial.completion", Type: "boolean", Required: false, Description: "Accept partial completion"},
			{Name: "required.attributes", Type: "attributeIDList", Required: false, Description: "Required output attributes"},
		},
		Examples: []string{
			`(workflow.wait.for.completion
			  (workflows @workflow{kyc.verification} @workflow{ubo.discovery})
			  (timeout "45m")
			  (partial.completion false)
			  (required.attributes @attr{kyc.status} @attr{ubo.persons.identified}))`,
		},
		Domains: []string{"orchestration"},
		Phase:   3,
	}

	ov.workflowVerbs["workflow.apply.product.requirements"] = VerbDefinition{
		Verb:        "workflow.apply.product.requirements",
		Category:    "workflow",
		Description: "Apply product-specific workflow requirements to entities",
		Parameters: []VerbParameter{
			{Name: "products", Type: "stringList", Required: true, Description: "Products to apply"},
			{Name: "to.entities", Type: "attributeIDList", Required: true, Description: "Target entities"},
			{Name: "depends.on", Type: "attributeIDList", Required: false, Description: "Prerequisite completions"},
			{Name: "customizations", Type: "customizationMap", Required: false, Description: "Product customizations"},
		},
		Examples: []string{
			`(workflow.apply.product.requirements
			  (products "CUSTODY" "TRADING")
			  (to.entities @attr{corporate.entity.id})
			  (depends.on @attr{kyc.complete} @attr{ubo.complete})
			  (customizations (custody.type "PRIME_BROKERAGE") (trading.level "PROFESSIONAL")))`,
		},
		Domains: []string{"orchestration", "custody", "trading"},
		Phase:   3,
	}
}

// initializeCommunicationVerbs defines domain communication verbs
func (ov *OrchestrationVocabulary) initializeCommunicationVerbs() {
	ov.communicationVerbs["domain.route.to"] = VerbDefinition{
		Verb:        "domain.route.to",
		Category:    "communication",
		Description: "Route DSL fragment to specific domain with context",
		Parameters: []VerbParameter{
			{Name: "domain", Type: "string", Required: true, Description: "Target domain"},
			{Name: "dsl.fragment", Type: "string", Required: true, Description: "DSL to route"},
			{Name: "context", Type: "attributeID", Required: false, Description: "Context AttributeID"},
			{Name: "priority", Type: "string", Required: false, Description: "Routing priority"},
			{Name: "correlation.id", Type: "string", Required: false, Description: "Message correlation ID"},
		},
		Examples: []string{
			`(domain.route.to
			  (domain "kyc")
			  (dsl.fragment "(kyc.enhanced.verification (entity @attr{corporate.entity.id}))")
			  (context @attr{orchestration.session.context})
			  (priority "HIGH")
			  (correlation.id "kyc-req-001"))`,
		},
		Domains: []string{"orchestration"},
		Phase:   3,
	}

	ov.communicationVerbs["domain.collect.results"] = VerbDefinition{
		Verb:        "domain.collect.results",
		Category:    "communication",
		Description: "Collect results from multiple domains and bind to attributes",
		Parameters: []VerbParameter{
			{Name: "from.domains", Type: "domainList", Required: true, Description: "Source domains"},
			{Name: "result.binding", Type: "attributeID", Required: true, Description: "Where to bind collected results"},
			{Name: "aggregation.strategy", Type: "string", Required: false, Description: "How to aggregate results"},
			{Name: "timeout", Type: "duration", Required: false, Description: "Collection timeout"},
		},
		Examples: []string{
			`(domain.collect.results
			  (from.domains "kyc" "ubo" "compliance")
			  (result.binding @attr{consolidated.risk.assessment})
			  (aggregation.strategy "RISK_WEIGHTED_AVERAGE")
			  (timeout "15m"))`,
		},
		Domains: []string{"orchestration"},
		Phase:   3,
	}

	ov.communicationVerbs["domain.broadcast.state"] = VerbDefinition{
		Verb:        "domain.broadcast.state",
		Category:    "communication",
		Description: "Broadcast state changes to interested domains",
		Parameters: []VerbParameter{
			{Name: "attributes", Type: "attributeIDList", Required: true, Description: "Attributes that changed"},
			{Name: "to.domains", Type: "domainList", Required: true, Description: "Target domains"},
			{Name: "change.type", Type: "string", Required: false, Description: "Type of change"},
			{Name: "immediate", Type: "boolean", Required: false, Description: "Whether broadcast is immediate"},
		},
		Examples: []string{
			`(domain.broadcast.state
			  (attributes @attr{entity.risk_profile} @attr{kyc.completion.status})
			  (to.domains "compliance" "custody" "trading")
			  (change.type "RISK_PROFILE_UPDATE")
			  (immediate true))`,
		},
		Domains: []string{"orchestration"},
		Phase:   3,
	}
}

// initializeProductVerbs defines product integration verbs
func (ov *OrchestrationVocabulary) initializeProductVerbs() {
	ov.productVerbs["products.validate.compatibility"] = VerbDefinition{
		Verb:        "products.validate.compatibility",
		Category:    "products",
		Description: "Validate product compatibility with entities and jurisdictions",
		Parameters: []VerbParameter{
			{Name: "entities", Type: "attributeIDList", Required: true, Description: "Entity AttributeIDs"},
			{Name: "products", Type: "stringList", Required: true, Description: "Products to validate"},
			{Name: "jurisdictions", Type: "stringList", Required: false, Description: "Relevant jurisdictions"},
			{Name: "compliance.tier", Type: "string", Required: false, Description: "Required compliance tier"},
		},
		Examples: []string{
			`(products.validate.compatibility
			  (entities @attr{trust.entity.id})
			  (products "CUSTODY" "TRADING" "LENDING")
			  (jurisdictions "LU" "US")
			  (compliance.tier "ENHANCED"))`,
		},
		Domains: []string{"orchestration", "products"},
		Phase:   3,
	}

	ov.productVerbs["products.configure.cross.domain"] = VerbDefinition{
		Verb:        "products.configure.cross.domain",
		Category:    "products",
		Description: "Configure product requirements across multiple domains",
		Parameters: []VerbParameter{
			{Name: "products", Type: "stringList", Required: true, Description: "Products to configure"},
			{Name: "configuration", Type: "configurationMap", Required: true, Description: "Product configuration"},
			{Name: "affected.domains", Type: "domainList", Required: true, Description: "Domains affected by configuration"},
			{Name: "dependencies", Type: "dependencyMap", Required: false, Description: "Inter-product dependencies"},
		},
		Examples: []string{
			`(products.configure.cross.domain
			  (products "CUSTODY" "TRADING")
			  (configuration
			    (custody.segregation "FULL")
			    (trading.authorization "PRE_APPROVED"))
			  (affected.domains "custody" "trading" "compliance")
			  (dependencies (trading ["custody.account.active"])))`,
		},
		Domains: []string{"orchestration", "products"},
		Phase:   3,
	}
}

// GetAllVerbs returns all orchestration verbs
func (ov *OrchestrationVocabulary) GetAllVerbs() map[string]VerbDefinition {
	allVerbs := make(map[string]VerbDefinition)

	// Combine all verb categories
	for verb, def := range ov.orchestrationVerbs {
		allVerbs[verb] = def
	}
	for verb, def := range ov.stateManagementVerbs {
		allVerbs[verb] = def
	}
	for verb, def := range ov.workflowVerbs {
		allVerbs[verb] = def
	}
	for verb, def := range ov.communicationVerbs {
		allVerbs[verb] = def
	}
	for verb, def := range ov.productVerbs {
		allVerbs[verb] = def
	}

	return allVerbs
}

// ValidateOrchestrationVerbs validates orchestration DSL contains only approved verbs
func (ov *OrchestrationVocabulary) ValidateOrchestrationVerbs(dsl string) error {
	allVerbs := ov.GetAllVerbs()

	// Extract verbs from DSL using simple regex pattern
	lines := strings.Split(dsl, "\n")
	var errors []string

	for lineNum, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, ";") {
			continue // Skip empty lines and comments
		}

		// Find verb patterns (verb.action)
		if strings.HasPrefix(line, "(") {
			// Extract verb from line like "(verb.action"
			verbEnd := strings.Index(line[1:], " ")
			if verbEnd == -1 {
				verbEnd = strings.Index(line[1:], ")")
			}
			if verbEnd == -1 {
				continue // No space or closing paren found
			}

			verb := line[1 : verbEnd+1]
			if verb != "" && !ov.isApprovedOrchestrationVerb(verb, allVerbs) {
				errors = append(errors, fmt.Sprintf("line %d: unknown orchestration verb '%s'", lineNum+1, verb))
			}
		}
	}

	if len(errors) > 0 {
		return fmt.Errorf("orchestration DSL validation failed:\n%s", strings.Join(errors, "\n"))
	}

	return nil
}

// isApprovedOrchestrationVerb checks if a verb is in the approved orchestration vocabulary
func (ov *OrchestrationVocabulary) isApprovedOrchestrationVerb(verb string, allVerbs map[string]VerbDefinition) bool {
	_, exists := allVerbs[verb]
	return exists
}

// GetVerbsByCategory returns verbs filtered by category
func (ov *OrchestrationVocabulary) GetVerbsByCategory(category string) []VerbDefinition {
	var verbs []VerbDefinition
	allVerbs := ov.GetAllVerbs()

	for _, def := range allVerbs {
		if def.Category == category {
			verbs = append(verbs, def)
		}
	}

	return verbs
}

// GetVerbsByDomain returns verbs applicable to a specific domain
func (ov *OrchestrationVocabulary) GetVerbsByDomain(domain string) []VerbDefinition {
	var verbs []VerbDefinition
	allVerbs := ov.GetAllVerbs()

	for _, def := range allVerbs {
		for _, d := range def.Domains {
			if d == domain {
				verbs = append(verbs, def)
				break
			}
		}
	}

	return verbs
}

// GetVerbsByPhase returns verbs for a specific implementation phase
func (ov *OrchestrationVocabulary) GetVerbsByPhase(phase int) []VerbDefinition {
	var verbs []VerbDefinition
	allVerbs := ov.GetAllVerbs()

	for _, def := range allVerbs {
		if def.Phase == phase {
			verbs = append(verbs, def)
		}
	}

	return verbs
}

// GenerateVerbDocumentation generates documentation for orchestration verbs
func (ov *OrchestrationVocabulary) GenerateVerbDocumentation() string {
	var doc strings.Builder

	doc.WriteString("# Orchestration DSL Vocabulary - Phase 3\n\n")
	doc.WriteString("This document defines the orchestration-specific DSL verbs for cross-domain coordination and state management.\n\n")

	// Document by category
	categories := []string{"context", "state", "workflow", "communication", "products", "execution"}

	for _, category := range categories {
		verbs := ov.GetVerbsByCategory(category)
		if len(verbs) == 0 {
			continue
		}

		doc.WriteString(fmt.Sprintf("## %s Verbs\n\n", strings.Title(category)))

		for _, verb := range verbs {
			doc.WriteString(fmt.Sprintf("### %s\n\n", verb.Verb))
			doc.WriteString(fmt.Sprintf("**Description**: %s\n\n", verb.Description))
			doc.WriteString(fmt.Sprintf("**Domains**: %s\n\n", strings.Join(verb.Domains, ", ")))

			if len(verb.Parameters) > 0 {
				doc.WriteString("**Parameters**:\n")
				for _, param := range verb.Parameters {
					required := ""
					if param.Required {
						required = " (required)"
					}
					doc.WriteString(fmt.Sprintf("- `%s` (%s)%s: %s\n", param.Name, param.Type, required, param.Description))
				}
				doc.WriteString("\n")
			}

			if len(verb.Examples) > 0 {
				doc.WriteString("**Example**:\n```lisp\n")
				doc.WriteString(verb.Examples[0])
				doc.WriteString("\n```\n\n")
			}
		}
	}

	return doc.String()
}

// CrossDomainAttributeManager manages AttributeID references across domains
type CrossDomainAttributeManager struct {
	attributeRegistry map[string]*dictionary.Attribute
	crossReferences   map[string][]string // attributeID -> domains using it
	conflictResolver  *AttributeConflictResolver
}

// AttributeConflictResolver handles conflicts in cross-domain attribute updates
type AttributeConflictResolver struct {
	strategies map[string]func(oldValue, newValue interface{}, context map[string]interface{}) interface{}
}

// NewCrossDomainAttributeManager creates a cross-domain attribute manager
func NewCrossDomainAttributeManager() *CrossDomainAttributeManager {
	return &CrossDomainAttributeManager{
		attributeRegistry: make(map[string]*dictionary.Attribute),
		crossReferences:   make(map[string][]string),
		conflictResolver:  NewAttributeConflictResolver(),
	}
}

// NewAttributeConflictResolver creates an attribute conflict resolver
func NewAttributeConflictResolver() *AttributeConflictResolver {
	resolver := &AttributeConflictResolver{
		strategies: make(map[string]func(oldValue, newValue interface{}, context map[string]interface{}) interface{}),
	}

	// Default conflict resolution strategies
	resolver.strategies["SOURCE_WINS"] = func(oldValue, newValue interface{}, context map[string]interface{}) interface{} {
		return newValue // Source always wins
	}

	resolver.strategies["TIMESTAMP_WINS"] = func(oldValue, newValue interface{}, context map[string]interface{}) interface{} {
		// In a real implementation, this would check timestamps
		return newValue
	}

	resolver.strategies["MERGE_VALUES"] = func(oldValue, newValue interface{}, context map[string]interface{}) interface{} {
		// Simple merge strategy - in reality this would be type-specific
		return fmt.Sprintf("%v; %v", oldValue, newValue)
	}

	return resolver
}

// RegisterAttributeUsage registers that a domain is using a specific attribute
func (cdam *CrossDomainAttributeManager) RegisterAttributeUsage(attributeID, domain string) {
	if cdam.crossReferences[attributeID] == nil {
		cdam.crossReferences[attributeID] = make([]string, 0)
	}

	// Add domain if not already present
	for _, existingDomain := range cdam.crossReferences[attributeID] {
		if existingDomain == domain {
			return // Already registered
		}
	}

	cdam.crossReferences[attributeID] = append(cdam.crossReferences[attributeID], domain)
}

// SyncAttributeAcrossDomains synchronizes an attribute value across all domains that use it
func (cdam *CrossDomainAttributeManager) SyncAttributeAcrossDomains(ctx context.Context, attributeID string, newValue interface{}, sourceDomain string, strategy string) error {
	domains := cdam.crossReferences[attributeID]
	if len(domains) <= 1 {
		return nil // No need to sync if only one domain uses it
	}

	// Get conflict resolution strategy
	_, exists := cdam.conflictResolver.strategies[strategy]
	if !exists {
		strategy = "SOURCE_WINS"
	}

	// Apply conflict resolution and sync to all domains
	for _, domain := range domains {
		if domain != sourceDomain {
			// In a real implementation, this would call the domain's sync API
			// For now, we just log the sync operation
		}
	}

	return nil
}

// GetCrossReferences returns all domains that reference a specific attribute
func (cdam *CrossDomainAttributeManager) GetCrossReferences(attributeID string) []string {
	return cdam.crossReferences[attributeID]
}

// ValidateAttributeConsistency validates that attribute values are consistent across domains
func (cdam *CrossDomainAttributeManager) ValidateAttributeConsistency(ctx context.Context, attributeID string) error {
	domains := cdam.crossReferences[attributeID]
	if len(domains) <= 1 {
		return nil // No consistency issues with single domain
	}

	// In a real implementation, this would:
	// 1. Fetch attribute values from all domains
	// 2. Compare values for consistency
	// 3. Report any inconsistencies
	// 4. Optionally resolve conflicts

	return nil
}
