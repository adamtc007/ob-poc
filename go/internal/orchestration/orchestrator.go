// Package orchestration provides the core orchestration engine for multi-domain DSL workflows.
//
// This package implements the orchestration layer that coordinates between multiple domains
// (onboarding, hedge-fund-investor, kyc, compliance, etc.) to create unified, entity-type
// and product-specific workflows. It analyzes context, determines required domains,
// builds dependency graphs, and manages cross-domain state sharing.
//
// Key Features:
// - Context analysis from CBU, entities, and products
// - Automatic domain discovery and dependency resolution
// - Cross-domain DSL accumulation and state management
// - Product-driven workflow customization
// - Execution planning and optimization
// - Shared AttributeID resolution across domains
//
// Architecture Pattern: DSL-as-State + AttributeID-as-Type + Domain Orchestration
// The orchestrator maintains a unified DSL document that accumulates contributions
// from multiple domains while ensuring referential integrity through shared AttributeIDs.
package orchestration

import (
	"context"
	"fmt"
	"sort"
	"strings"
	"sync"
	"time"

	registry "dsl-ob-poc/internal/domain-registry"
	"dsl-ob-poc/internal/shared-dsl/session"

	"github.com/google/uuid"
)

// Orchestrator coordinates multi-domain DSL workflows
type Orchestrator struct {
	registry       *registry.Registry
	sessionManager *session.Manager

	// Persistent session storage
	sessionStore *PersistentOrchestrationStore

	// Phase 2: Dynamic DSL Generation Engine
	dslGenerator      *DSLGenerator
	compositionEngine *DSLCompositionEngine

	// Phase 3: Orchestration DSL Verbs
	vocabulary   *OrchestrationVocabulary
	verbExecutor *OrchestrationVerbExecutor

	// Active orchestration sessions (in-memory cache)
	sessions map[string]*OrchestrationSession
	mu       sync.RWMutex

	// Configuration
	config *OrchestratorConfig

	// Metrics
	metrics   *OrchestratorMetrics
	startTime time.Time
}

// OrchestrationSession represents a unified session across multiple domains
type OrchestrationSession struct {
	// Identity
	SessionID     string    `json:"session_id"`
	PrimaryDomain string    `json:"primary_domain"`
	CreatedAt     time.Time `json:"created_at"`
	LastUsed      time.Time `json:"last_used"`

	// Domain coordination
	ActiveDomains map[string]*DomainSession `json:"active_domains"`
	SharedContext *SharedContext            `json:"shared_context"`
	ExecutionPlan *ExecutionPlan            `json:"execution_plan"`

	// DSL state (DSL-as-State pattern)
	UnifiedDSL    string            `json:"unified_dsl"`    // Accumulated DSL from all domains
	DomainDSL     map[string]string `json:"domain_dsl"`     // DSL by domain
	VersionNumber int               `json:"version_number"` // Incremented on each DSL update

	// State machine
	CurrentState string              `json:"current_state"`
	StateHistory []StateTransition   `json:"state_history"`
	PendingTasks []OrchestrationTask `json:"pending_tasks"`

	// Cross-domain references
	EntityRefs    map[string]string `json:"entity_refs"`    // entity_type -> UUID
	AttributeRefs map[string]string `json:"attribute_refs"` // attr_name -> attr_id (UUID)

	mu sync.RWMutex
}

// DomainSession represents a domain-specific session within orchestration
type DomainSession struct {
	Domain         string                 `json:"domain"`
	SessionID      string                 `json:"session_id"`      // Domain's internal session ID
	State          string                 `json:"state"`           // Domain-specific state
	Context        map[string]interface{} `json:"context"`         // Domain context
	ContributedDSL string                 `json:"contributed_dsl"` // DSL from this domain
	LastActivity   time.Time              `json:"last_activity"`
	Dependencies   []string               `json:"dependencies"` // Other domains this depends on
}

// SharedContext holds cross-domain shared state and entity references
type SharedContext struct {
	// Primary entities
	CBUID      string `json:"cbu_id,omitempty"`
	InvestorID string `json:"investor_id,omitempty"`
	FundID     string `json:"fund_id,omitempty"`
	EntityID   string `json:"entity_id,omitempty"`

	// Entity attributes
	EntityType   string   `json:"entity_type,omitempty"` // PROPER_PERSON, CORPORATE, TRUST, etc.
	EntityName   string   `json:"entity_name,omitempty"`
	Jurisdiction string   `json:"jurisdiction,omitempty"`
	Products     []string `json:"products,omitempty"` // Requested products
	Services     []string `json:"services,omitempty"` // Required services

	// Workflow context
	WorkflowType   string `json:"workflow_type,omitempty"`   // ONBOARDING, INVESTMENT, KYC_REFRESH
	RiskProfile    string `json:"risk_profile,omitempty"`    // LOW, MEDIUM, HIGH
	ComplianceTier string `json:"compliance_tier,omitempty"` // SIMPLIFIED, STANDARD, ENHANCED

	// Dynamic attributes (AttributeID-as-Type)
	AttributeValues map[string]interface{} `json:"attribute_values,omitempty"` // attr_id -> value

	// Flexible data storage
	Data map[string]interface{} `json:"data,omitempty"`

	mu sync.RWMutex
}

// ExecutionPlan defines the order and dependencies for domain execution
type ExecutionPlan struct {
	Stages               []ExecutionStage    `json:"stages"`
	Dependencies         map[string][]string `json:"dependencies"`          // domain -> [dependent_domains]
	ParallelGroups       [][]string          `json:"parallel_groups"`       // Groups that can run in parallel
	ResourceDependencies []ResourceDep       `json:"resource_dependencies"` // Cross-domain resource deps
	EstimatedDuration    time.Duration       `json:"estimated_duration"`
	CreatedAt            time.Time           `json:"created_at"`
}

// ExecutionStage represents a stage in the orchestrated workflow
type ExecutionStage struct {
	Name            string         `json:"name"`             // "kyc_discovery", "resource_planning"
	Domains         []string       `json:"domains"`          // Domains involved in this stage
	RequiredInputs  []string       `json:"required_inputs"`  // AttributeIDs needed
	ProducedOutputs []string       `json:"produced_outputs"` // AttributeIDs produced
	Prerequisites   []string       `json:"prerequisites"`    // Previous stages required
	EstimatedTime   time.Duration  `json:"estimated_time"`
	State           ExecutionState `json:"state"` // PENDING, RUNNING, COMPLETED, FAILED
}

// ResourceDep represents a cross-domain resource dependency
type ResourceDep struct {
	SourceDomain  string `json:"source_domain"`
	TargetDomain  string `json:"target_domain"`
	ResourceType  string `json:"resource_type"`  // "account", "entity_record", "kyc_profile"
	ResourceID    string `json:"resource_id"`    // AttributeID or entity reference
	WaitCondition string `json:"wait_condition"` // "created", "validated", "approved"
}

// OrchestrationTask represents a unit of work in the orchestration
type OrchestrationTask struct {
	TaskID       string                 `json:"task_id"`
	Domain       string                 `json:"domain"`
	Verb         string                 `json:"verb"`         // DSL verb to execute
	Parameters   map[string]interface{} `json:"parameters"`   // Verb parameters
	Dependencies []string               `json:"dependencies"` // Task IDs this depends on
	Status       TaskStatus             `json:"status"`
	ScheduledAt  time.Time              `json:"scheduled_at"`
	StartedAt    *time.Time             `json:"started_at,omitempty"`
	CompletedAt  *time.Time             `json:"completed_at,omitempty"`
	Error        string                 `json:"error,omitempty"`
	GeneratedDSL string                 `json:"generated_dsl,omitempty"`
}

// StateTransition tracks state changes in the orchestration
type StateTransition struct {
	FromState   string    `json:"from_state"`
	ToState     string    `json:"to_state"`
	Domain      string    `json:"domain,omitempty"` // Domain that triggered transition
	Timestamp   time.Time `json:"timestamp"`
	Reason      string    `json:"reason,omitempty"`
	GeneratedBy string    `json:"generated_by,omitempty"` // User, AI agent, system
}

// OrchestratorConfig configures orchestrator behavior
type OrchestratorConfig struct {
	MaxConcurrentSessions int           `json:"max_concurrent_sessions"`
	SessionTimeout        time.Duration `json:"session_timeout"`
	EnableOptimization    bool          `json:"enable_optimization"`
	EnableParallelExec    bool          `json:"enable_parallel_execution"`
	MaxDomainDepth        int           `json:"max_domain_depth"`        // Max dependency depth
	ContextPropagationTTL time.Duration `json:"context_propagation_ttl"` // How long context is valid
}

// OrchestratorMetrics tracks orchestrator performance
type OrchestratorMetrics struct {
	TotalSessions         int64            `json:"total_sessions"`
	ActiveSessions        int64            `json:"active_sessions"`
	CompletedWorkflows    int64            `json:"completed_workflows"`
	FailedWorkflows       int64            `json:"failed_workflows"`
	AverageExecutionTime  time.Duration    `json:"average_execution_time"`
	DomainsCoordinated    map[string]int64 `json:"domains_coordinated"` // domain -> count
	CrossDomainReferences int64            `json:"cross_domain_references"`
	UptimeSeconds         int64            `json:"uptime_seconds"`
	LastUpdated           time.Time        `json:"last_updated"`
}

// Enum types
type ExecutionState string

const (
	ExecutionStatePending   ExecutionState = "PENDING"
	ExecutionStateRunning   ExecutionState = "RUNNING"
	ExecutionStateCompleted ExecutionState = "COMPLETED"
	ExecutionStateFailed    ExecutionState = "FAILED"
	ExecutionStateSkipped   ExecutionState = "SKIPPED"
)

type TaskStatus string

const (
	TaskStatusPending   TaskStatus = "PENDING"
	TaskStatusScheduled TaskStatus = "SCHEDULED"
	TaskStatusRunning   TaskStatus = "RUNNING"
	TaskStatusCompleted TaskStatus = "COMPLETED"
	TaskStatusFailed    TaskStatus = "FAILED"
	TaskStatusSkipped   TaskStatus = "SKIPPED"
)

// NewOrchestrator creates a new orchestration engine
func NewOrchestrator(domainRegistry *registry.Registry, sessionManager *session.Manager, config *OrchestratorConfig) *Orchestrator {
	if config == nil {
		config = DefaultOrchestratorConfig()
	}

	// Initialize DSL generation engine
	dslGenerator := NewDSLGenerator(&DSLGeneratorConfig{
		EnableTemplateCache: true,
		MaxTemplateDepth:    10,
		ValidateGenerated:   true,
		IncludeComments:     false,
	})

	compositionEngine := NewDSLCompositionEngine(dslGenerator, &CompositionConfig{
		EnableDependencyOptimization: true,
		EnableParallelGeneration:     true,
		MaxCompositionDepth:          5,
		ValidateComposedDSL:          true,
		IncludeGenerationMetadata:    true,
		OptimizeExecutionOrder:       true,
	})

	// Initialize Phase 3: Orchestration vocabulary and verb executor
	vocabulary := NewOrchestrationVocabulary()

	orchestrator := &Orchestrator{
		registry:          domainRegistry,
		sessionManager:    sessionManager,
		dslGenerator:      dslGenerator,
		compositionEngine: compositionEngine,
		vocabulary:        vocabulary,
		sessions:          make(map[string]*OrchestrationSession),
		config:            config,
		metrics: &OrchestratorMetrics{
			DomainsCoordinated: make(map[string]int64),
			LastUpdated:        time.Now(),
		},
		startTime: time.Now(),
	}

	// Initialize verb executor with orchestrator reference
	orchestrator.verbExecutor = NewOrchestrationVerbExecutor(orchestrator, vocabulary)

	return orchestrator
}

// NewPersistentOrchestrator creates a new orchestration engine with persistent storage
func NewPersistentOrchestrator(domainRegistry *registry.Registry, sessionManager *session.Manager, sessionStore *PersistentOrchestrationStore, config *OrchestratorConfig) *Orchestrator {
	if config == nil {
		config = DefaultOrchestratorConfig()
	}

	// Initialize DSL generation engine
	dslGenerator := NewDSLGenerator(&DSLGeneratorConfig{
		EnableTemplateCache: true,
		MaxTemplateDepth:    10,
		ValidateGenerated:   true,
		IncludeComments:     false,
	})

	compositionEngine := NewDSLCompositionEngine(dslGenerator, &CompositionConfig{
		EnableDependencyOptimization: true,
		EnableParallelGeneration:     true,
		MaxCompositionDepth:          5,
		ValidateComposedDSL:          true,
		IncludeGenerationMetadata:    true,
		OptimizeExecutionOrder:       true,
	})

	// Initialize Phase 3: Orchestration vocabulary and verb executor
	vocabulary := NewOrchestrationVocabulary()

	orchestrator := &Orchestrator{
		registry:          domainRegistry,
		sessionManager:    sessionManager,
		sessionStore:      sessionStore,
		dslGenerator:      dslGenerator,
		compositionEngine: compositionEngine,
		vocabulary:        vocabulary,
		sessions:          make(map[string]*OrchestrationSession),
		config:            config,
		metrics: &OrchestratorMetrics{
			DomainsCoordinated: make(map[string]int64),
			LastUpdated:        time.Now(),
		},
		startTime: time.Now(),
	}

	// Initialize verb executor with orchestrator reference
	orchestrator.verbExecutor = NewOrchestrationVerbExecutor(orchestrator, vocabulary)

	return orchestrator
}

// DefaultOrchestratorConfig returns default configuration
func DefaultOrchestratorConfig() *OrchestratorConfig {
	return &OrchestratorConfig{
		MaxConcurrentSessions: 100,
		SessionTimeout:        24 * time.Hour,
		EnableOptimization:    true,
		EnableParallelExec:    true,
		MaxDomainDepth:        5,
		ContextPropagationTTL: 1 * time.Hour,
	}
}

// CreateOrchestrationSession creates a new multi-domain session
func (o *Orchestrator) CreateOrchestrationSession(ctx context.Context, req *OrchestrationRequest) (*OrchestrationSession, error) {
	o.mu.Lock()
	defer o.mu.Unlock()

	// Check session limits
	if len(o.sessions) >= o.config.MaxConcurrentSessions {
		return nil, fmt.Errorf("maximum concurrent sessions (%d) reached", o.config.MaxConcurrentSessions)
	}

	sessionID := req.SessionID
	if sessionID == "" {
		sessionID = uuid.New().String()
	}

	// Analyze context to determine required domains
	analysis, err := o.analyzeOnboardingContext(ctx, req)
	if err != nil {
		return nil, fmt.Errorf("context analysis failed: %w", err)
	}

	// Build execution plan
	execPlan, err := o.buildExecutionPlan(ctx, analysis)
	if err != nil {
		return nil, fmt.Errorf("execution planning failed: %w", err)
	}

	// Create shared context
	sharedContext := &SharedContext{
		CBUID:           req.CBUID,
		InvestorID:      req.InvestorID,
		EntityID:        req.EntityID,
		EntityType:      req.EntityType,
		EntityName:      req.EntityName,
		Jurisdiction:    req.Jurisdiction,
		Products:        req.Products,
		Services:        req.Services,
		WorkflowType:    req.WorkflowType,
		RiskProfile:     req.RiskProfile,
		ComplianceTier:  req.ComplianceTier,
		AttributeValues: make(map[string]interface{}),
		Data:            make(map[string]interface{}),
	}

	// Create orchestration session
	orchSession := &OrchestrationSession{
		SessionID:     sessionID,
		PrimaryDomain: analysis.PrimaryDomain,
		CreatedAt:     time.Now(),
		LastUsed:      time.Now(),
		ActiveDomains: make(map[string]*DomainSession),
		SharedContext: sharedContext,
		ExecutionPlan: execPlan,
		UnifiedDSL:    "",
		DomainDSL:     make(map[string]string),
		VersionNumber: 0,
		CurrentState:  "CREATED",
		StateHistory:  []StateTransition{{FromState: "", ToState: "CREATED", Timestamp: time.Now(), Reason: "session_created"}},
		PendingTasks:  make([]OrchestrationTask, 0),
		EntityRefs:    make(map[string]string),
		AttributeRefs: make(map[string]string),
	}

	// Initialize domain sessions
	for _, domainName := range analysis.RequiredDomains {
		domainSession := &DomainSession{
			Domain:         domainName,
			SessionID:      uuid.New().String(), // Each domain gets its own session ID
			State:          "CREATED",
			Context:        make(map[string]interface{}),
			ContributedDSL: "",
			LastActivity:   time.Now(),
			Dependencies:   analysis.Dependencies[domainName],
		}
		orchSession.ActiveDomains[domainName] = domainSession

		// Update metrics
		o.metrics.DomainsCoordinated[domainName]++
	}

	// Generate Master DSL using composition engine
	masterDSL, err := o.generateMasterDSL(ctx, req, orchSession)
	if err != nil {
		return nil, fmt.Errorf("failed to generate master DSL: %w", err)
	}
	orchSession.UnifiedDSL = masterDSL
	orchSession.VersionNumber = 1

	// Store session in memory cache
	o.sessions[sessionID] = orchSession

	// Store session persistently if store is available
	if o.sessionStore != nil {
		if err := o.sessionStore.SaveSession(ctx, orchSession); err != nil {
			// Log error but don't fail session creation
			// Remove from memory cache if persistence failed
			delete(o.sessions, sessionID)
			return nil, fmt.Errorf("failed to persist session: %w", err)
		}
	}

	// Update metrics
	o.metrics.TotalSessions++
	o.metrics.ActiveSessions++

	return orchSession, nil
}

// ExecuteOrchestrationInstruction executes an orchestration instruction with Phase 3 verb support
func (o *Orchestrator) ExecuteOrchestrationInstruction(ctx context.Context, sessionID string, instruction string) (*OrchestrationInstructionResult, error) {
	// Get orchestration session
	session, err := o.GetOrchestrationSession(sessionID)
	if err != nil {
		return nil, fmt.Errorf("failed to get orchestration session: %w", err)
	}

	// Parse instruction into DSL (simplified - would use AI agent in production)
	dsl, err := o.parseInstructionToDSL(ctx, instruction, session)
	if err != nil {
		return nil, fmt.Errorf("failed to parse instruction to DSL: %w", err)
	}

	// Validate orchestration verbs
	if err := o.vocabulary.ValidateOrchestrationVerbs(dsl); err != nil {
		return nil, fmt.Errorf("DSL validation failed: %w", err)
	}

	// Execute orchestration DSL
	result, err := o.verbExecutor.ProcessOrchestrationDSL(ctx, dsl, sessionID)
	if err != nil {
		return nil, fmt.Errorf("failed to execute orchestration DSL: %w", err)
	}

	// Update session with generated DSL
	if len(result.GeneratedDSL) > 0 {
		for _, generatedDSL := range result.GeneratedDSL {
			err = o.accumulateDSL(ctx, session, "orchestration", generatedDSL)
			if err != nil {
				return nil, fmt.Errorf("failed to accumulate generated DSL: %w", err)
			}
		}
	}

	// Convert processing result to instruction result
	instrResult := &OrchestrationInstructionResult{
		SessionID:        sessionID,
		Success:          result.Success,
		GeneratedDSL:     strings.Join(result.GeneratedDSL, "\n\n"),
		ProcessedVerbs:   result.ProcessedVerbs,
		DomainUpdates:    result.DomainUpdates,
		AttributeUpdates: result.AttributeUpdates,
		Errors:           result.Errors,
		Warnings:         result.Warnings,
		ExecutionTime:    result.ProcessingTime,
	}

	return instrResult, nil
}

// OrchestrationInstructionResult represents the result of executing an orchestration instruction
type OrchestrationInstructionResult struct {
	SessionID        string
	Success          bool
	GeneratedDSL     string
	ProcessedVerbs   []string
	DomainUpdates    map[string][]string
	AttributeUpdates map[string]interface{}
	Errors           []string
	Warnings         []string
	ExecutionTime    time.Duration
}

// parseInstructionToDSL converts natural language instruction to orchestration DSL
func (o *Orchestrator) parseInstructionToDSL(ctx context.Context, instruction string, session *OrchestrationSession) (string, error) {
	// Simplified instruction parsing - in production this would use AI agent
	instruction = strings.ToLower(instruction)

	if strings.Contains(instruction, "initialize shared state") {
		return fmt.Sprintf(`(state.initialize.shared
  (session.id "%s")
  (primary.entity "@attr{entity.primary.id}")
  (shared.attributes "@attr{entity.legal_name}" "@attr{entity.jurisdiction}" "@attr{entity.risk_profile}")
  (accessible.domains "onboarding" "kyc" "ubo" "custody")
)`, session.SessionID), nil
	}

	if strings.Contains(instruction, "execute") && strings.Contains(instruction, "kyc") {
		return `(workflow.execute.subdomain
  (domain "kyc")
  (template "enhanced-kyc-workflow")
  (entity.target "@attr{entity.primary.id}")
  (depends.on "@attr{onboarding.complete}")
  (result.binding "@attr{kyc.completion.status}")
)`, nil
	}

	if strings.Contains(instruction, "execute") && strings.Contains(instruction, "ubo") {
		return fmt.Sprintf(`(workflow.execute.subdomain
  (domain "ubo")
  (template "%s-ubo-workflow")
  (entity.target "@attr{entity.primary.id}")
  (depends.on "@attr{kyc.completion.status}")
  (result.binding "@attr{ubo.analysis.complete}")
)`, strings.ToLower(session.SharedContext.EntityType)), nil
	}

	if strings.Contains(instruction, "validate") && strings.Contains(instruction, "products") {
		products := `"` + strings.Join(session.SharedContext.Products, `" "`) + `"`
		return fmt.Sprintf(`(products.validate.compatibility
  (entities "@attr{entity.primary.id}")
  (products %s)
  (jurisdictions "%s")
  (compliance.tier "%s")
)`, products, session.SharedContext.Jurisdiction, session.SharedContext.ComplianceTier), nil
	}

	if strings.Contains(instruction, "sync") && strings.Contains(instruction, "state") {
		return `(state.sync.attributes
  (attributes "@attr{entity.legal_name}" "@attr{entity.address}")
  (between.domains "onboarding" "kyc" "custody")
  (conflict.resolution "SOURCE_WINS")
  (validation.required true)
)`, nil
	}

	// Default: generate basic orchestration coordination
	return fmt.Sprintf(`(orchestration.coordinate
  (session.id "%s")
  (instruction "%s")
  (domains %s)
  (timestamp "%s")
)`, session.SessionID, instruction,
		strings.Join(o.getActiveDomainNames(session), `" "`),
		time.Now().Format(time.RFC3339)), nil
}

// getActiveDomainNames gets the names of active domains from session
func (o *Orchestrator) getActiveDomainNames(session *OrchestrationSession) []string {
	var domains []string
	for domainName := range session.ActiveDomains {
		domains = append(domains, domainName)
	}
	return domains
}

// GetOrchestrationVocabulary returns the orchestration vocabulary for external access
func (o *Orchestrator) GetOrchestrationVocabulary() *OrchestrationVocabulary {
	return o.vocabulary
}

// generateMasterDSL creates a comprehensive Master DSL using the composition engine
func (o *Orchestrator) generateMasterDSL(ctx context.Context, req *OrchestrationRequest, session *OrchestrationSession) (string, error) {
	// Build composition request from orchestration request and session
	compositionReq := &CompositionRequest{
		EntityName:         req.EntityName,
		EntityType:         req.EntityType,
		Jurisdiction:       req.Jurisdiction,
		Products:           req.Products,
		ProductMetadata:    make(map[string]*ProductComposition),
		ServiceRequests:    req.Services,
		WorkflowType:       req.WorkflowType,
		ComplianceTier:     req.ComplianceTier,
		EntityAttributes:   make(map[string]interface{}),
		RequiredDomains:    make([]string, 0, len(session.ActiveDomains)),
		DomainDependencies: make(map[string][]string),
		ExecutionStrategy:  "DEPENDENCY_OPTIMIZED",
		SessionID:          session.SessionID,
		CBUID:              req.CBUID,
		RequestedAt:        session.CreatedAt,
	}

	// Extract required domains from active domains
	for domainName := range session.ActiveDomains {
		compositionReq.RequiredDomains = append(compositionReq.RequiredDomains, domainName)
	}

	// Add entity-specific attributes based on type
	switch req.EntityType {
	case "CORPORATE":
		compositionReq.EntityAttributes["RequiresUBOAnalysis"] = true
		compositionReq.EntityAttributes["UBOThreshold"] = 25
		compositionReq.EntityAttributes["IsComplexStructure"] = false
	case "TRUST":
		compositionReq.EntityAttributes["HasProtector"] = false
		compositionReq.EntityAttributes["IsDiscretionary"] = true
		compositionReq.EntityAttributes["TrustType"] = "DISCRETIONARY"
	case "PROPER_PERSON":
		compositionReq.EntityAttributes["RequiresEnhancedKYC"] = false
	}

	// Add jurisdiction-specific attributes
	switch req.Jurisdiction {
	case "US":
		compositionReq.EntityAttributes["RequiresFINCENCompliance"] = true
		compositionReq.EntityAttributes["RequiresPatriotAct"] = true
	case "LU", "DE", "FR", "IE", "NL":
		compositionReq.EntityAttributes["Requires5MLD"] = true
		compositionReq.EntityAttributes["RequiresGDPR"] = true
	}

	// Add product-specific metadata
	for _, product := range req.Products {
		switch product {
		case "CUSTODY":
			compositionReq.ProductMetadata[product] = &ProductComposition{
				ProductID:         product,
				Priority:          1,
				RequiredTemplates: []string{"custody_requirements"},
				AttributeOverrides: map[string]interface{}{
					"RequiresSegregation":       true,
					"RequiresPrimeBrokerage":    req.EntityType != "PROPER_PERSON",
					"RequiresRealTimeReporting": req.ComplianceTier == "ENHANCED",
				},
			}
		case "TRADING":
			compositionReq.ProductMetadata[product] = &ProductComposition{
				ProductID:         product,
				Priority:          2,
				RequiredTemplates: []string{"trading_requirements"},
				DependencyModifiers: map[string][]string{
					"trading": {"custody"}, // Trading depends on custody
				},
			}
		case "HEDGE_FUND_INVESTMENT":
			compositionReq.ProductMetadata[product] = &ProductComposition{
				ProductID:         product,
				Priority:          1,
				RequiredTemplates: []string{"hedge_fund_requirements"},
				AttributeOverrides: map[string]interface{}{
					"RequiresAccreditationCheck": true,
					"InvestorType":               req.EntityType,
				},
			}
		}
	}

	// Use composition engine to generate master DSL
	result, err := o.compositionEngine.ComposeMasterDSL(ctx, compositionReq)
	if err != nil {
		return "", fmt.Errorf("DSL composition failed: %w", err)
	}

	// Store component DSLs in domain sessions
	for componentName, componentDSL := range result.ComponentDSLs {
		// Map component names to domain names
		var domainName string
		if componentName == "entity" {
			domainName = "onboarding"
		} else if strings.HasPrefix(componentName, "product_") {
			domainName = strings.TrimPrefix(componentName, "product_")
		} else if strings.HasPrefix(componentName, "compliance_") {
			domainName = "compliance"
		} else {
			domainName = componentName
		}

		if domainSession, exists := session.ActiveDomains[domainName]; exists {
			domainSession.ContributedDSL = componentDSL
		}
	}

	// Update execution plan with optimized plan from composition
	if result.ExecutionPlan != nil {
		session.ExecutionPlan = o.convertCompositionExecutionPlan(result.ExecutionPlan)
	}

	return result.MasterDSL, nil
}

// convertCompositionExecutionPlan converts CompositionExecutionPlan to ExecutionPlan
func (o *Orchestrator) convertCompositionExecutionPlan(compositionPlan *CompositionExecutionPlan) *ExecutionPlan {
	execStages := make([]ExecutionStage, len(compositionPlan.Stages))
	for i, compStage := range compositionPlan.Stages {
		execStages[i] = ExecutionStage{
			Name:            fmt.Sprintf("stage_%d", compStage.StageNumber),
			Domains:         compStage.Domains,
			RequiredInputs:  []string{},
			ProducedOutputs: []string{},
			Prerequisites:   compStage.Dependencies,
			EstimatedTime:   compStage.EstimatedTime,
			State:           ExecutionState("PENDING"),
		}
	}

	return &ExecutionPlan{
		Stages:               execStages,
		Dependencies:         make(map[string][]string),
		ParallelGroups:       compositionPlan.ParallelGroups,
		ResourceDependencies: []ResourceDep{},
		EstimatedDuration:    compositionPlan.EstimatedDuration,
		CreatedAt:            time.Now(),
	}
}

// GetOrchestrationSession retrieves an existing orchestration session
func (o *Orchestrator) GetOrchestrationSession(sessionID string) (*OrchestrationSession, error) {
	o.mu.RLock()
	session, exists := o.sessions[sessionID]
	o.mu.RUnlock()

	if exists {
		session.LastUsed = time.Now()
		return session, nil
	}

	// Try to load from persistent store if not in memory cache
	if o.sessionStore != nil {
		ctx := context.Background()
		session, err := o.sessionStore.LoadSession(ctx, sessionID)
		if err != nil {
			return nil, err
		}

		// Add to memory cache
		o.mu.Lock()
		o.sessions[sessionID] = session
		o.mu.Unlock()

		session.LastUsed = time.Now()
		return session, nil
	}

	return nil, fmt.Errorf("orchestration session not found: %s", sessionID)
}

// ExecuteInstruction processes a natural language instruction across domains
func (o *Orchestrator) ExecuteInstruction(ctx context.Context, sessionID, instruction string) (*OrchestrationResult, error) {
	session, err := o.GetOrchestrationSession(sessionID)
	if err != nil {
		return nil, err
	}

	session.mu.Lock()
	defer session.mu.Unlock()

	// Analyze instruction to determine target domains
	targetDomains, err := o.analyzeInstruction(ctx, instruction, session)
	if err != nil {
		return nil, fmt.Errorf("instruction analysis failed: %w", err)
	}

	result := &OrchestrationResult{
		SessionID:     sessionID,
		Instruction:   instruction,
		TargetDomains: targetDomains,
		DomainResults: make(map[string]*registry.GenerationResponse),
		StartTime:     time.Now(),
	}

	// Execute across target domains
	for _, domainName := range targetDomains {
		domainResult, err := o.executeDomainInstruction(ctx, session, domainName, instruction)
		if err != nil {
			result.Errors = append(result.Errors, fmt.Sprintf("domain %s failed: %v", domainName, err))
			continue
		}

		result.DomainResults[domainName] = domainResult

		// Accumulate DSL
		if domainResult.DSL != "" {
			if err := o.accumulateDSL(ctx, session, domainName, domainResult.DSL); err != nil {
				result.Warnings = append(result.Warnings, fmt.Sprintf("DSL accumulation warning: %v", err))
			}
		}

		// Update domain session state
		if domainSession, exists := session.ActiveDomains[domainName]; exists {
			domainSession.State = domainResult.ToState
			domainSession.ContributedDSL = domainResult.DSL
			domainSession.LastActivity = time.Now()
		}
	}

	result.EndTime = time.Now()
	result.Duration = result.EndTime.Sub(result.StartTime)
	result.UnifiedDSL = session.UnifiedDSL
	result.CurrentState = session.CurrentState

	return result, nil
}

// AccumulateDSL adds DSL from a domain to the unified DSL document
func (o *Orchestrator) accumulateDSL(ctx context.Context, session *OrchestrationSession, domainName, dsl string) error {
	if dsl == "" {
		return nil
	}

	session.mu.Lock()
	defer session.mu.Unlock()

	// Store domain-specific DSL
	session.DomainDSL[domainName] = dsl
	session.VersionNumber++

	// Accumulate in unified DSL
	if session.UnifiedDSL == "" {
		session.UnifiedDSL = dsl
	} else {
		session.UnifiedDSL = session.UnifiedDSL + "\n\n" + dsl
	}

	session.LastUsed = time.Now()

	// Update domain session
	if domainSession, exists := session.ActiveDomains[domainName]; exists {
		domainSession.ContributedDSL = dsl
		domainSession.LastActivity = time.Now()
	}

	// Persist changes if store is available
	if o.sessionStore != nil {
		if err := o.sessionStore.SaveSession(ctx, session); err != nil {
			return fmt.Errorf("failed to persist DSL accumulation: %w", err)
		}
	}

	return nil
}

// ContextAnalysis contains the results of analyzing onboarding context
type ContextAnalysis struct {
	PrimaryDomain       string              `json:"primary_domain"`
	RequiredDomains     []string            `json:"required_domains"`
	EntityTypes         []string            `json:"entity_types"`
	Products            []string            `json:"products"`
	ComplianceTier      string              `json:"compliance_tier"`
	Dependencies        map[string][]string `json:"dependencies"`         // domain -> dependent domains
	EstimatedComplexity string              `json:"estimated_complexity"` // LOW, MEDIUM, HIGH
}

// OrchestrationRequest represents a request to create an orchestrated workflow
type OrchestrationRequest struct {
	SessionID    string   `json:"session_id,omitempty"`
	CBUID        string   `json:"cbu_id,omitempty"`
	InvestorID   string   `json:"investor_id,omitempty"`
	EntityID     string   `json:"entity_id,omitempty"`
	EntityType   string   `json:"entity_type,omitempty"` // PROPER_PERSON, CORPORATE, TRUST
	EntityName   string   `json:"entity_name,omitempty"`
	Jurisdiction string   `json:"jurisdiction,omitempty"`
	Products     []string `json:"products,omitempty"`
	Services     []string `json:"services,omitempty"`
	WorkflowType string   `json:"workflow_type,omitempty"` // ONBOARDING, INVESTMENT, KYC_REFRESH

	// Optional context
	RiskProfile    string                 `json:"risk_profile,omitempty"`
	ComplianceTier string                 `json:"compliance_tier,omitempty"`
	InitialContext map[string]interface{} `json:"initial_context,omitempty"`
}

// OrchestrationResult contains the result of executing an instruction
type OrchestrationResult struct {
	SessionID     string                                  `json:"session_id"`
	Instruction   string                                  `json:"instruction"`
	TargetDomains []string                                `json:"target_domains"`
	DomainResults map[string]*registry.GenerationResponse `json:"domain_results"`
	UnifiedDSL    string                                  `json:"unified_dsl"`
	CurrentState  string                                  `json:"current_state"`
	Errors        []string                                `json:"errors,omitempty"`
	Warnings      []string                                `json:"warnings,omitempty"`
	StartTime     time.Time                               `json:"start_time"`
	EndTime       time.Time                               `json:"end_time"`
	Duration      time.Duration                           `json:"duration"`
}

// analyzeOnboardingContext determines required domains based on context
func (o *Orchestrator) analyzeOnboardingContext(ctx context.Context, req *OrchestrationRequest) (*ContextAnalysis, error) {
	analysis := &ContextAnalysis{
		RequiredDomains: make([]string, 0),
		EntityTypes:     make([]string, 0),
		Products:        req.Products,
		Dependencies:    make(map[string][]string),
	}

	// Determine primary domain
	if req.WorkflowType == "INVESTMENT" && len(req.Products) > 0 {
		// Check if hedge fund products are involved
		for _, product := range req.Products {
			if strings.Contains(strings.ToLower(product), "hedge") ||
				strings.Contains(strings.ToLower(product), "fund") {
				analysis.PrimaryDomain = "hedge-fund-investor"
				break
			}
		}
	}

	if analysis.PrimaryDomain == "" {
		analysis.PrimaryDomain = "onboarding" // Default primary domain
	}

	// Always include primary domain
	analysis.RequiredDomains = append(analysis.RequiredDomains, analysis.PrimaryDomain)

	// Determine entity-specific domains
	if req.EntityType != "" {
		analysis.EntityTypes = append(analysis.EntityTypes, req.EntityType)

		switch req.EntityType {
		case "CORPORATE":
			analysis.RequiredDomains = append(analysis.RequiredDomains, "kyc", "ubo")
			analysis.Dependencies["ubo"] = []string{"kyc"}
		case "TRUST":
			analysis.RequiredDomains = append(analysis.RequiredDomains, "kyc", "ubo", "trust-kyc")
			analysis.Dependencies["trust-kyc"] = []string{"kyc"}
			analysis.Dependencies["ubo"] = []string{"trust-kyc"}
		case "PARTNERSHIP":
			analysis.RequiredDomains = append(analysis.RequiredDomains, "kyc", "ubo")
			analysis.Dependencies["ubo"] = []string{"kyc"}
		case "PROPER_PERSON":
			analysis.RequiredDomains = append(analysis.RequiredDomains, "kyc")
		}
	}

	// Product-driven domain inclusion
	for _, product := range req.Products {
		productLower := strings.ToLower(product)

		if strings.Contains(productLower, "custody") {
			analysis.RequiredDomains = append(analysis.RequiredDomains, "custody")
		}
		if strings.Contains(productLower, "trading") || strings.Contains(productLower, "execution") {
			analysis.RequiredDomains = append(analysis.RequiredDomains, "trading")
		}
		if strings.Contains(productLower, "compliance") || strings.Contains(productLower, "reporting") {
			analysis.RequiredDomains = append(analysis.RequiredDomains, "compliance")
		}
	}

	// Jurisdiction-driven requirements
	if req.Jurisdiction != "" {
		// EU jurisdictions require additional compliance
		if isEUJurisdiction(req.Jurisdiction) {
			analysis.RequiredDomains = append(analysis.RequiredDomains, "eu-compliance")
			analysis.ComplianceTier = "ENHANCED"
		}
		// US jurisdictions
		if req.Jurisdiction == "US" {
			analysis.RequiredDomains = append(analysis.RequiredDomains, "us-compliance")
			analysis.ComplianceTier = "ENHANCED"
		}
	}

	// Remove duplicates and sort
	analysis.RequiredDomains = removeDuplicates(analysis.RequiredDomains)
	sort.Strings(analysis.RequiredDomains)

	// Estimate complexity
	if len(analysis.RequiredDomains) <= 2 {
		analysis.EstimatedComplexity = "LOW"
	} else if len(analysis.RequiredDomains) <= 4 {
		analysis.EstimatedComplexity = "MEDIUM"
	} else {
		analysis.EstimatedComplexity = "HIGH"
	}

	return analysis, nil
}

// buildExecutionPlan creates an optimized execution plan for the domains
func (o *Orchestrator) buildExecutionPlan(ctx context.Context, analysis *ContextAnalysis) (*ExecutionPlan, error) {
	plan := &ExecutionPlan{
		Stages:               make([]ExecutionStage, 0),
		Dependencies:         analysis.Dependencies,
		ParallelGroups:       make([][]string, 0),
		ResourceDependencies: make([]ResourceDep, 0),
		CreatedAt:            time.Now(),
	}

	// Build execution stages based on dependencies
	executed := make(map[string]bool)
	stageNum := 1

	for len(executed) < len(analysis.RequiredDomains) {
		currentStage := ExecutionStage{
			Name:            fmt.Sprintf("stage_%d", stageNum),
			Domains:         make([]string, 0),
			RequiredInputs:  make([]string, 0),
			ProducedOutputs: make([]string, 0),
			Prerequisites:   make([]string, 0),
			State:           ExecutionStatePending,
			EstimatedTime:   30 * time.Second, // Default estimate
		}

		// Find domains with satisfied dependencies
		for _, domain := range analysis.RequiredDomains {
			if executed[domain] {
				continue
			}

			// Check if all dependencies are satisfied
			deps := analysis.Dependencies[domain]
			allSatisfied := true
			for _, dep := range deps {
				if !executed[dep] {
					allSatisfied = false
					break
				}
			}

			if allSatisfied {
				currentStage.Domains = append(currentStage.Domains, domain)
				executed[domain] = true
			}
		}

		if len(currentStage.Domains) == 0 {
			return nil, fmt.Errorf("circular dependency detected in domains")
		}

		// Domains in the same stage can run in parallel
		if len(currentStage.Domains) > 1 {
			plan.ParallelGroups = append(plan.ParallelGroups, currentStage.Domains)
		}

		plan.Stages = append(plan.Stages, currentStage)
		stageNum++
	}

	// Estimate total duration
	totalDuration := time.Duration(0)
	for _, stage := range plan.Stages {
		totalDuration += stage.EstimatedTime
	}
	plan.EstimatedDuration = totalDuration

	return plan, nil
}

// analyzeInstruction determines which domains should handle an instruction
func (o *Orchestrator) analyzeInstruction(ctx context.Context, instruction string, session *OrchestrationSession) ([]string, error) {
	instructionLower := strings.ToLower(instruction)
	targetDomains := make([]string, 0)

	// Keyword-based domain routing (can be enhanced with AI later)
	domainKeywords := map[string][]string{
		"onboarding":          {"case", "cbu", "onboard", "client", "create case"},
		"kyc":                 {"kyc", "know your customer", "identity", "verification", "document", "passport", "id"},
		"ubo":                 {"ubo", "beneficial owner", "ownership", "shareholder", "trust", "beneficiary"},
		"hedge-fund-investor": {"investor", "investment", "subscription", "fund", "hedge", "accredited"},
		"compliance":          {"compliance", "regulatory", "report", "filing", "regulation"},
		"custody":             {"custody", "safekeeping", "asset", "securities", "account"},
		"trading":             {"trade", "execution", "order", "market", "buy", "sell"},
	}

	// Find matching domains
	for domain, keywords := range domainKeywords {
		// Only consider domains that are active in this session
		if _, isActive := session.ActiveDomains[domain]; !isActive {
			continue
		}

		for _, keyword := range keywords {
			if strings.Contains(instructionLower, keyword) {
				targetDomains = append(targetDomains, domain)
				break
			}
		}
	}

	// If no specific domains found, use primary domain
	if len(targetDomains) == 0 {
		targetDomains = append(targetDomains, session.PrimaryDomain)
	}

	// Remove duplicates
	targetDomains = removeDuplicates(targetDomains)

	return targetDomains, nil
}

// executeDomainInstruction executes an instruction within a specific domain
func (o *Orchestrator) executeDomainInstruction(ctx context.Context, session *OrchestrationSession, domainName, instruction string) (*registry.GenerationResponse, error) {
	// Get domain from registry
	domain, err := o.registry.Get(domainName)
	if err != nil {
		return nil, fmt.Errorf("domain not found: %w", err)
	}

	// Get domain session
	domainSession, exists := session.ActiveDomains[domainName]
	if !exists {
		return nil, fmt.Errorf("domain session not found: %s", domainName)
	}

	// Build generation request
	genReq := &registry.GenerationRequest{
		Instruction:   instruction,
		SessionID:     domainSession.SessionID,
		CurrentDomain: domainName,
		Context:       o.buildDomainContext(session, domainName),
		ExistingDSL:   session.DomainDSL[domainName],
		Timestamp:     time.Now(),
	}

	// Execute domain generation
	response, err := domain.GenerateDSL(ctx, genReq)
	if err != nil {
		return nil, fmt.Errorf("domain generation failed: %w", err)
	}

	return response, nil
}

// buildDomainContext creates domain-specific context from shared context
func (o *Orchestrator) buildDomainContext(session *OrchestrationSession, domainName string) map[string]interface{} {
	context := make(map[string]interface{})

	// Copy shared context
	session.SharedContext.mu.RLock()
	defer session.SharedContext.mu.RUnlock()

	context["cbu_id"] = session.SharedContext.CBUID
	context["investor_id"] = session.SharedContext.InvestorID
	context["entity_id"] = session.SharedContext.EntityID
	context["entity_type"] = session.SharedContext.EntityType
	context["entity_name"] = session.SharedContext.EntityName
	context["jurisdiction"] = session.SharedContext.Jurisdiction
	context["products"] = session.SharedContext.Products
	context["services"] = session.SharedContext.Services
	context["workflow_type"] = session.SharedContext.WorkflowType
	context["risk_profile"] = session.SharedContext.RiskProfile
	context["compliance_tier"] = session.SharedContext.ComplianceTier

	// Copy attribute values
	for attrID, value := range session.SharedContext.AttributeValues {
		context[attrID] = value
	}

	// Copy flexible data
	for key, value := range session.SharedContext.Data {
		context[key] = value
	}

	// Add domain-specific context if exists
	if domainSession, exists := session.ActiveDomains[domainName]; exists {
		for key, value := range domainSession.Context {
			context[key] = value
		}
	}

	// Add cross-domain references
	for refType, refID := range session.EntityRefs {
		context[refType+"_ref"] = refID
	}

	return context
}

// GetMetrics returns orchestrator performance metrics
func (o *Orchestrator) GetMetrics() *OrchestratorMetrics {
	o.mu.RLock()
	defer o.mu.RUnlock()

	// Update uptime
	o.metrics.UptimeSeconds = time.Since(o.startTime).Milliseconds() / 1000
	o.metrics.ActiveSessions = int64(len(o.sessions))
	o.metrics.LastUpdated = time.Now()

	// Create a copy to avoid race conditions
	metricsCopy := &OrchestratorMetrics{
		TotalSessions:         o.metrics.TotalSessions,
		ActiveSessions:        o.metrics.ActiveSessions,
		CompletedWorkflows:    o.metrics.CompletedWorkflows,
		FailedWorkflows:       o.metrics.FailedWorkflows,
		AverageExecutionTime:  o.metrics.AverageExecutionTime,
		DomainsCoordinated:    make(map[string]int64),
		CrossDomainReferences: o.metrics.CrossDomainReferences,
		UptimeSeconds:         o.metrics.UptimeSeconds,
		LastUpdated:           o.metrics.LastUpdated,
	}

	// Copy domains coordinated map
	for domain, count := range o.metrics.DomainsCoordinated {
		metricsCopy.DomainsCoordinated[domain] = count
	}

	return metricsCopy
}

// CleanupExpiredSessions removes sessions older than configured timeout
func (o *Orchestrator) CleanupExpiredSessions() int {
	// Clean up persistent storage if available
	if o.sessionStore != nil {
		ctx := context.Background()
		removedPersistent, err := o.sessionStore.CleanupExpiredSessions(ctx)
		if err == nil {
			// Clear memory cache for consistency
			o.mu.Lock()
			o.sessions = make(map[string]*OrchestrationSession)
			o.mu.Unlock()
			return int(removedPersistent)
		}
		// Fall through to memory cleanup on error
	}

	// Clean up memory cache
	o.mu.Lock()
	defer o.mu.Unlock()

	now := time.Now()
	removed := 0

	for sessionID, session := range o.sessions {
		if now.Sub(session.LastUsed) > o.config.SessionTimeout {
			delete(o.sessions, sessionID)
			removed++
			o.metrics.ActiveSessions--
		}
	}

	return removed
}

// ListActiveSessions returns IDs of all active sessions
func (o *Orchestrator) ListActiveSessions() []string {
	// If we have persistent storage, get sessions from there
	if o.sessionStore != nil {
		ctx := context.Background()
		sessionIDs, err := o.sessionStore.ListActiveSessions(ctx)
		if err != nil {
			// Fall back to memory cache on error
			return o.listMemorySessionIDs()
		}
		return sessionIDs
	}

	return o.listMemorySessionIDs()
}

func (o *Orchestrator) listMemorySessionIDs() []string {
	o.mu.RLock()
	defer o.mu.RUnlock()

	sessionIDs := make([]string, 0, len(o.sessions))
	for sessionID := range o.sessions {
		sessionIDs = append(sessionIDs, sessionID)
	}

	return sessionIDs
}

// GetSessionStatus returns detailed status of an orchestration session
func (o *Orchestrator) GetSessionStatus(sessionID string) (*SessionStatus, error) {
	session, err := o.GetOrchestrationSession(sessionID)
	if err != nil {
		return nil, err
	}

	session.mu.RLock()
	defer session.mu.RUnlock()

	status := &SessionStatus{
		SessionID:      sessionID,
		PrimaryDomain:  session.PrimaryDomain,
		CurrentState:   session.CurrentState,
		CreatedAt:      session.CreatedAt,
		LastUsed:       session.LastUsed,
		VersionNumber:  session.VersionNumber,
		ActiveDomains:  make([]DomainStatus, 0),
		PendingTasks:   len(session.PendingTasks),
		CompletedTasks: 0,
		UnifiedDSLSize: len(session.UnifiedDSL),
	}

	// Count completed tasks
	for _, task := range session.PendingTasks {
		if task.Status == TaskStatusCompleted {
			status.CompletedTasks++
		}
	}

	// Build domain status
	for domainName, domainSession := range session.ActiveDomains {
		domainStatus := DomainStatus{
			Domain:       domainName,
			State:        domainSession.State,
			LastActivity: domainSession.LastActivity,
			HasDSL:       domainSession.ContributedDSL != "",
			Dependencies: domainSession.Dependencies,
		}
		status.ActiveDomains = append(status.ActiveDomains, domainStatus)
	}

	return status, nil
}

// SessionStatus represents the status of an orchestration session
type SessionStatus struct {
	SessionID      string         `json:"session_id"`
	PrimaryDomain  string         `json:"primary_domain"`
	CurrentState   string         `json:"current_state"`
	CreatedAt      time.Time      `json:"created_at"`
	LastUsed       time.Time      `json:"last_used"`
	VersionNumber  int            `json:"version_number"`
	ActiveDomains  []DomainStatus `json:"active_domains"`
	PendingTasks   int            `json:"pending_tasks"`
	CompletedTasks int            `json:"completed_tasks"`
	UnifiedDSLSize int            `json:"unified_dsl_size"`
}

// DomainStatus represents the status of a domain within an orchestration session
type DomainStatus struct {
	Domain       string    `json:"domain"`
	State        string    `json:"state"`
	LastActivity time.Time `json:"last_activity"`
	HasDSL       bool      `json:"has_dsl"`
	Dependencies []string  `json:"dependencies"`
}

// Utility functions

// isEUJurisdiction checks if a jurisdiction is in the European Union
func isEUJurisdiction(jurisdiction string) bool {
	euCountries := map[string]bool{
		"AT": true, "BE": true, "BG": true, "HR": true, "CY": true, "CZ": true,
		"DK": true, "EE": true, "FI": true, "FR": true, "DE": true, "GR": true,
		"HU": true, "IE": true, "IT": true, "LV": true, "LT": true, "LU": true,
		"MT": true, "NL": true, "PL": true, "PT": true, "RO": true, "SK": true,
		"SI": true, "ES": true, "SE": true,
	}
	return euCountries[strings.ToUpper(jurisdiction)]
}

// removeDuplicates removes duplicate strings from a slice
func removeDuplicates(slice []string) []string {
	seen := make(map[string]bool)
	result := make([]string, 0)

	for _, item := range slice {
		if !seen[item] {
			seen[item] = true
			result = append(result, item)
		}
	}

	return result
}
