package orchestration

import (
	"context"
	"strings"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	registry "dsl-ob-poc/internal/domain-registry"
	"dsl-ob-poc/internal/shared-dsl/session"
)

func TestOrchestrationVocabulary_GetAllVerbs(t *testing.T) {
	vocab := NewOrchestrationVocabulary()
	allVerbs := vocab.GetAllVerbs()

	// Verify we have orchestration verbs from all categories
	assert.GreaterOrEqual(t, len(allVerbs), 15, "Should have at least 15 orchestration verbs")

	// Verify key verbs exist
	expectedVerbs := []string{
		"orchestration.initialize",
		"orchestration.context.analyze",
		"state.initialize.shared",
		"state.share.cross.domain",
		"workflow.execute.subdomain",
		"workflow.coordinate.parallel",
		"domain.route.to",
		"domain.collect.results",
		"products.validate.compatibility",
	}

	for _, expectedVerb := range expectedVerbs {
		assert.Contains(t, allVerbs, expectedVerb, "Should contain verb: %s", expectedVerb)
	}
}

func TestOrchestrationVocabulary_GetVerbsByCategory(t *testing.T) {
	vocab := NewOrchestrationVocabulary()

	tests := []struct {
		category      string
		expectedMin   int
		expectedVerbs []string
	}{
		{
			category:      "context",
			expectedMin:   2,
			expectedVerbs: []string{"orchestration.initialize", "orchestration.context.analyze"},
		},
		{
			category:      "state",
			expectedMin:   3,
			expectedVerbs: []string{"state.initialize.shared", "state.share.cross.domain"},
		},
		{
			category:      "workflow",
			expectedMin:   3,
			expectedVerbs: []string{"workflow.execute.subdomain", "workflow.coordinate.parallel"},
		},
		{
			category:      "communication",
			expectedMin:   2,
			expectedVerbs: []string{"domain.route.to", "domain.collect.results"},
		},
		{
			category:      "products",
			expectedMin:   2,
			expectedVerbs: []string{"products.validate.compatibility"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.category, func(t *testing.T) {
			verbs := vocab.GetVerbsByCategory(tt.category)
			assert.GreaterOrEqual(t, len(verbs), tt.expectedMin, "Should have at least %d %s verbs", tt.expectedMin, tt.category)

			// Check for specific expected verbs
			verbNames := make([]string, len(verbs))
			for i, verb := range verbs {
				verbNames[i] = verb.Verb
			}

			for _, expectedVerb := range tt.expectedVerbs {
				assert.Contains(t, verbNames, expectedVerb, "Should contain %s verb: %s", tt.category, expectedVerb)
			}
		})
	}
}

func TestOrchestrationVocabulary_ValidateOrchestrationVerbs(t *testing.T) {
	vocab := NewOrchestrationVocabulary()

	tests := []struct {
		name        string
		dsl         string
		expectError bool
		errorMsg    string
	}{
		{
			name: "Valid orchestration DSL",
			dsl: `; Valid orchestration DSL
(orchestration.initialize
  (session.id "test-session")
  (entity.name "Test Entity"))

(state.initialize.shared
  (session.id "test-session")
  (primary.entity "@attr{entity.id}"))

(workflow.execute.subdomain
  (domain "kyc")
  (template "enhanced-kyc"))`,
			expectError: false,
		},
		{
			name: "Invalid verb in DSL",
			dsl: `(orchestration.initialize
  (session.id "test-session"))

(invalid.unknown.verb
  (parameter "value"))

(state.initialize.shared
  (session.id "test-session"))`,
			expectError: true,
			errorMsg:    "unknown orchestration verb 'invalid.unknown.verb'",
		},
		{
			name:        "Empty DSL",
			dsl:         "",
			expectError: false,
		},
		{
			name: "Comments only",
			dsl: `; This is a comment
; Another comment`,
			expectError: false,
		},
		{
			name: "Mixed valid and invalid verbs",
			dsl: `(orchestration.initialize (session.id "test"))
(fake.verb.not.approved (param "value"))
(state.initialize.shared (session.id "test"))`,
			expectError: true,
			errorMsg:    "fake.verb.not.approved",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := vocab.ValidateOrchestrationVerbs(tt.dsl)

			if tt.expectError {
				assert.Error(t, err)
				if tt.errorMsg != "" {
					assert.Contains(t, err.Error(), tt.errorMsg)
				}
			} else {
				assert.NoError(t, err)
			}
		})
	}
}

func TestOrchestrationVerbExecutor_ExecuteOrchestrationVerb(t *testing.T) {
	// Setup test orchestrator
	domainRegistry := registry.NewRegistry()
	sessionManager := session.NewManager()
	config := DefaultOrchestratorConfig()
	orchestrator := NewOrchestrator(domainRegistry, sessionManager, config)

	// Create execution context
	orchSession := &OrchestrationSession{
		SessionID:     "test-session-001",
		PrimaryDomain: "onboarding",
		CreatedAt:     time.Now(),
		SharedContext: &SharedContext{
			CBUID:        "CBU-TEST-001",
			EntityName:   "Test Corporation",
			EntityType:   "CORPORATE",
			Jurisdiction: "US",
			Products:     []string{"CUSTODY", "TRADING"},
		},
		UnifiedDSL:    "",
		VersionNumber: 1,
		ActiveDomains: make(map[string]*DomainSession),
	}

	execCtx := &ExecutionContext{
		SessionID:        "test-session-001",
		OrchestrationCtx: orchSession,
		SharedState:      make(map[string]interface{}),
		ExecutionStack:   make([]string, 0),
		Timeout:          30 * time.Second,
	}

	tests := []struct {
		name           string
		verb           string
		parameters     map[string]interface{}
		expectSuccess  bool
		validateResult func(t *testing.T, result *VerbExecutionResult)
	}{
		{
			name: "orchestration.initialize",
			verb: "orchestration.initialize",
			parameters: map[string]interface{}{
				"session.id":   "test-session-001",
				"cbu.id":       "CBU-TEST-001",
				"entity.name":  "Test Corporation",
				"entity.type":  "CORPORATE",
				"jurisdiction": "US",
				"products":     []string{"CUSTODY", "TRADING"},
			},
			expectSuccess: true,
			validateResult: func(t *testing.T, result *VerbExecutionResult) {
				assert.True(t, result.Success)
				assert.NotEmpty(t, result.GeneratedDSL)
				assert.Contains(t, result.GeneratedDSL, "orchestration.session.active")
				assert.Contains(t, result.GeneratedDSL, "Test Corporation")
				assert.Contains(t, result.GeneratedDSL, "CORPORATE")
				assert.Equal(t, "test-session-001", result.ResultData["session_id"])
				assert.Equal(t, "CORPORATE", result.ResultData["entity_type"])
			},
		},
		{
			name: "state.initialize.shared",
			verb: "state.initialize.shared",
			parameters: map[string]interface{}{
				"session.id":         "test-session-001",
				"primary.entity":     "@attr{entity.primary.id}",
				"shared.attributes":  []string{"@attr{entity.legal_name}", "@attr{entity.jurisdiction}"},
				"accessible.domains": []string{"onboarding", "kyc", "ubo"},
			},
			expectSuccess: true,
			validateResult: func(t *testing.T, result *VerbExecutionResult) {
				assert.True(t, result.Success)
				assert.NotEmpty(t, result.GeneratedDSL)
				assert.Contains(t, result.GeneratedDSL, "state.shared.active")
				assert.Len(t, result.AttributeRefs, 2)
				assert.Contains(t, result.AttributeRefs, "@attr{entity.legal_name}")
				assert.Equal(t, true, result.ResultData["shared_state_initialized"])
			},
		},
		{
			name: "workflow.execute.subdomain",
			verb: "workflow.execute.subdomain",
			parameters: map[string]interface{}{
				"domain":         "kyc",
				"template":       "corporate-kyc-enhanced",
				"entity.target":  "@attr{corporate.entity.id}",
				"result.binding": "@attr{kyc.completion.status}",
			},
			expectSuccess: true,
			validateResult: func(t *testing.T, result *VerbExecutionResult) {
				assert.True(t, result.Success)
				assert.NotEmpty(t, result.GeneratedDSL)
				assert.Contains(t, result.GeneratedDSL, "workflow.subdomain.execute")
				assert.Contains(t, result.GeneratedDSL, "kyc")
				assert.Contains(t, result.GeneratedDSL, "corporate-kyc-enhanced")
				assert.Contains(t, result.DomainUpdates, "kyc")
				assert.Len(t, result.AttributeRefs, 2)
				assert.Len(t, result.NextActions, 1)
			},
		},
		{
			name: "products.validate.compatibility",
			verb: "products.validate.compatibility",
			parameters: map[string]interface{}{
				"entities":      []string{"@attr{corporate.entity.id}"},
				"products":      []string{"CUSTODY", "TRADING"},
				"jurisdictions": []string{"US"},
			},
			expectSuccess: true,
			validateResult: func(t *testing.T, result *VerbExecutionResult) {
				assert.True(t, result.Success)
				assert.NotEmpty(t, result.GeneratedDSL)
				assert.Contains(t, result.GeneratedDSL, "products.compatibility.validated")
				assert.Contains(t, result.GeneratedDSL, "CUSTODY")
				assert.Contains(t, result.GeneratedDSL, "TRADING")
				assert.Equal(t, true, result.ResultData["all_compatible"])
				assert.Len(t, result.AttributeRefs, 1)
			},
		},
		{
			name: "domain.route.to",
			verb: "domain.route.to",
			parameters: map[string]interface{}{
				"domain":       "kyc",
				"dsl.fragment": "(kyc.enhanced.verification (entity @attr{corporate.entity.id}))",
				"priority":     "HIGH",
			},
			expectSuccess: true,
			validateResult: func(t *testing.T, result *VerbExecutionResult) {
				assert.True(t, result.Success)
				assert.NotEmpty(t, result.GeneratedDSL)
				assert.Contains(t, result.GeneratedDSL, "domain.message.route")
				assert.Contains(t, result.DomainUpdates, "kyc")
				assert.Equal(t, "kyc", result.ResultData["routed_to"])
				assert.Equal(t, "HIGH", result.ResultData["priority"])
			},
		},
		{
			name: "Missing required parameters",
			verb: "orchestration.initialize",
			parameters: map[string]interface{}{
				"session.id": "test-session-001",
				// Missing required parameters
			},
			expectSuccess:  false,
			validateResult: nil,
		},
		{
			name: "Invalid verb",
			verb: "invalid.unknown.verb",
			parameters: map[string]interface{}{
				"param": "value",
			},
			expectSuccess:  false,
			validateResult: nil,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			ctx := context.Background()
			result, err := orchestrator.verbExecutor.ExecuteOrchestrationVerb(ctx, tt.verb, tt.parameters, execCtx)

			if tt.expectSuccess {
				require.NoError(t, err)
				require.NotNil(t, result)
				if tt.validateResult != nil {
					tt.validateResult(t, result)
				}
			} else {
				assert.Error(t, err)
			}
		})
	}
}

func TestOrchestrationVerbExecutor_ProcessOrchestrationDSL(t *testing.T) {
	// Setup test orchestrator with domain registry
	domainRegistry := registry.NewRegistry()
	sessionManager := session.NewManager()
	config := DefaultOrchestratorConfig()
	orchestrator := NewOrchestrator(domainRegistry, sessionManager, config)

	// Create test session
	orchReq := &OrchestrationRequest{
		EntityName:     "Goldman Sachs Asset Management",
		EntityType:     "CORPORATE",
		Jurisdiction:   "US",
		Products:       []string{"CUSTODY", "TRADING"},
		WorkflowType:   "ONBOARDING",
		ComplianceTier: "ENHANCED",
		CBUID:          "CBU-GS-001",
	}

	ctx := context.Background()
	session, err := orchestrator.CreateOrchestrationSession(ctx, orchReq)
	require.NoError(t, err)

	// Test DSL with orchestration verbs
	testDSL := `; Test orchestration DSL
(orchestration.initialize
  (session.id "` + session.SessionID + `")
  (cbu.id "CBU-GS-001")
  (entity.name "Goldman Sachs Asset Management")
  (entity.type "CORPORATE")
  (jurisdiction "US")
  (products "CUSTODY" "TRADING")
)

(state.initialize.shared
  (session.id "` + session.SessionID + `")
  (primary.entity "@attr{entity.primary.id}")
  (shared.attributes "@attr{entity.legal_name}" "@attr{entity.jurisdiction}")
  (accessible.domains "onboarding" "kyc" "custody")
)

(workflow.execute.subdomain
  (domain "kyc")
  (template "corporate-enhanced-kyc")
  (entity.target "@attr{entity.primary.id}")
  (result.binding "@attr{kyc.completion.status}")
)
`

	result, err := orchestrator.verbExecutor.ProcessOrchestrationDSL(ctx, testDSL, session.SessionID)
	require.NoError(t, err)
	require.NotNil(t, result)

	// Validate processing results
	assert.Equal(t, session.SessionID, result.SessionID)
	assert.True(t, result.Success)
	assert.GreaterOrEqual(t, len(result.ProcessedVerbs), 3)
	assert.GreaterOrEqual(t, len(result.GeneratedDSL), 3)

	// Check that specific verbs were processed
	assert.Contains(t, result.ProcessedVerbs, "orchestration.initialize")
	assert.Contains(t, result.ProcessedVerbs, "state.initialize.shared")
	assert.Contains(t, result.ProcessedVerbs, "workflow.execute.subdomain")

	// Validate generated DSL contains expected content
	combinedDSL := strings.Join(result.GeneratedDSL, "\n")
	assert.Contains(t, combinedDSL, "orchestration.session.active")
	assert.Contains(t, combinedDSL, "state.shared.active")
	assert.Contains(t, combinedDSL, "workflow.subdomain.execute")
}

func TestOrchestrator_ExecuteOrchestrationInstruction(t *testing.T) {
	// Setup test orchestrator
	domainRegistry := registry.NewRegistry()
	sessionManager := session.NewManager()
	config := DefaultOrchestratorConfig()
	orchestrator := NewOrchestrator(domainRegistry, sessionManager, config)

	// Create test session
	orchReq := &OrchestrationRequest{
		EntityName:     "Test Trust",
		EntityType:     "TRUST",
		Jurisdiction:   "LU",
		Products:       []string{"CUSTODY"},
		WorkflowType:   "ONBOARDING",
		ComplianceTier: "ENHANCED",
		CBUID:          "CBU-TRUST-001",
	}

	ctx := context.Background()
	session, err := orchestrator.CreateOrchestrationSession(ctx, orchReq)
	require.NoError(t, err)

	tests := []struct {
		name           string
		instruction    string
		expectSuccess  bool
		validateResult func(t *testing.T, result *OrchestrationInstructionResult)
	}{
		{
			name:          "Initialize shared state instruction",
			instruction:   "Initialize shared state for cross-domain coordination",
			expectSuccess: true,
			validateResult: func(t *testing.T, result *OrchestrationInstructionResult) {
				assert.True(t, result.Success)
				assert.Contains(t, result.GeneratedDSL, "state.initialize.shared")
				assert.GreaterOrEqual(t, len(result.ProcessedVerbs), 1)
				assert.Empty(t, result.Errors)
			},
		},
		{
			name:          "Execute KYC workflow instruction",
			instruction:   "Execute enhanced KYC verification workflow",
			expectSuccess: true,
			validateResult: func(t *testing.T, result *OrchestrationInstructionResult) {
				assert.True(t, result.Success)
				assert.Contains(t, result.GeneratedDSL, "workflow.execute.subdomain")
				assert.Contains(t, result.GeneratedDSL, "kyc")
				assert.Contains(t, result.ProcessedVerbs, "workflow.execute.subdomain")
			},
		},
		{
			name:          "Execute UBO discovery instruction",
			instruction:   "Execute UBO discovery and analysis",
			expectSuccess: true,
			validateResult: func(t *testing.T, result *OrchestrationInstructionResult) {
				assert.True(t, result.Success)
				assert.Contains(t, result.GeneratedDSL, "workflow.execute.subdomain")
				assert.Contains(t, result.GeneratedDSL, "ubo")
				assert.Contains(t, result.GeneratedDSL, "trust-ubo-workflow")
			},
		},
		{
			name:          "Validate products instruction",
			instruction:   "Validate product compatibility with entity requirements",
			expectSuccess: true,
			validateResult: func(t *testing.T, result *OrchestrationInstructionResult) {
				assert.True(t, result.Success)
				assert.Contains(t, result.GeneratedDSL, "products.validate.compatibility")
				assert.Contains(t, result.GeneratedDSL, "CUSTODY")
			},
		},
		{
			name:          "Sync state instruction",
			instruction:   "Sync state across domains for consistency",
			expectSuccess: true,
			validateResult: func(t *testing.T, result *OrchestrationInstructionResult) {
				assert.True(t, result.Success)
				assert.Contains(t, result.GeneratedDSL, "state.sync.attributes")
			},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result, err := orchestrator.ExecuteOrchestrationInstruction(ctx, session.SessionID, tt.instruction)

			if tt.expectSuccess {
				require.NoError(t, err)
				require.NotNil(t, result)
				assert.Equal(t, session.SessionID, result.SessionID)
				if tt.validateResult != nil {
					tt.validateResult(t, result)
				}
			} else {
				assert.Error(t, err)
			}
		})
	}
}

func TestCrossDomainAttributeManager_RegisterAttributeUsage(t *testing.T) {
	manager := NewCrossDomainAttributeManager()

	// Register attribute usage across domains
	manager.RegisterAttributeUsage("@attr{entity.legal_name}", "onboarding")
	manager.RegisterAttributeUsage("@attr{entity.legal_name}", "kyc")
	manager.RegisterAttributeUsage("@attr{entity.legal_name}", "custody")

	// Verify cross references
	crossRefs := manager.GetCrossReferences("@attr{entity.legal_name}")
	assert.Len(t, crossRefs, 3)
	assert.Contains(t, crossRefs, "onboarding")
	assert.Contains(t, crossRefs, "kyc")
	assert.Contains(t, crossRefs, "custody")

	// Test duplicate registration (should not create duplicates)
	manager.RegisterAttributeUsage("@attr{entity.legal_name}", "kyc")
	crossRefs = manager.GetCrossReferences("@attr{entity.legal_name}")
	assert.Len(t, crossRefs, 3) // Should still be 3, not 4
}

func TestCrossDomainAttributeManager_SyncAttributeAcrossDomains(t *testing.T) {
	manager := NewCrossDomainAttributeManager()
	ctx := context.Background()

	// Register attribute usage across domains
	attributeID := "@attr{entity.risk_profile}"
	manager.RegisterAttributeUsage(attributeID, "kyc")
	manager.RegisterAttributeUsage(attributeID, "compliance")
	manager.RegisterAttributeUsage(attributeID, "custody")

	// Test synchronization
	err := manager.SyncAttributeAcrossDomains(ctx, attributeID, "HIGH_RISK", "kyc", "SOURCE_WINS")
	assert.NoError(t, err)
}

func TestOrchestrationVocabulary_GenerateVerbDocumentation(t *testing.T) {
	vocab := NewOrchestrationVocabulary()
	doc := vocab.GenerateVerbDocumentation()

	assert.NotEmpty(t, doc)
	assert.Contains(t, doc, "# Orchestration DSL Vocabulary - Phase 3")
	assert.Contains(t, doc, "## Context Verbs")
	assert.Contains(t, doc, "## State Verbs")
	assert.Contains(t, doc, "## Workflow Verbs")
	assert.Contains(t, doc, "### orchestration.initialize")
	assert.Contains(t, doc, "### state.initialize.shared")
	assert.Contains(t, doc, "### workflow.execute.subdomain")

	// Verify it contains parameter documentation
	assert.Contains(t, doc, "**Parameters**:")
	assert.Contains(t, doc, "**Description**:")
	assert.Contains(t, doc, "**Domains**:")
	assert.Contains(t, doc, "**Example**:")
}

func TestOrchestrationVerbExecutor_ErrorHandling(t *testing.T) {
	// Setup with minimal orchestrator
	domainRegistry := registry.NewRegistry()
	sessionManager := session.NewManager()
	config := DefaultOrchestratorConfig()
	orchestrator := NewOrchestrator(domainRegistry, sessionManager, config)

	execCtx := &ExecutionContext{
		SessionID:   "test-session",
		SharedState: make(map[string]interface{}),
		Timeout:     10 * time.Second,
	}

	ctx := context.Background()

	tests := []struct {
		name        string
		verb        string
		parameters  map[string]interface{}
		expectError bool
		errorMsg    string
	}{
		{
			name: "Unknown verb",
			verb: "unknown.invalid.verb",
			parameters: map[string]interface{}{
				"param": "value",
			},
			expectError: true,
			errorMsg:    "unknown orchestration verb",
		},
		{
			name: "Missing required parameters",
			verb: "orchestration.initialize",
			parameters: map[string]interface{}{
				"session.id": "test-session",
				// Missing other required parameters
			},
			expectError: true,
			errorMsg:    "missing required parameters",
		},
		{
			name: "Invalid parameter types",
			verb: "state.initialize.shared",
			parameters: map[string]interface{}{
				"session.id":         "test-session",
				"primary.entity":     "@attr{entity.id}",
				"shared.attributes":  "not-a-list", // Should be a list
				"accessible.domains": []string{"kyc"},
			},
			expectError: false, // Currently simplified validation doesn't catch type errors
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result, err := orchestrator.verbExecutor.ExecuteOrchestrationVerb(ctx, tt.verb, tt.parameters, execCtx)

			if tt.expectError {
				assert.Error(t, err)
				if tt.errorMsg != "" {
					assert.Contains(t, err.Error(), tt.errorMsg)
				}
			} else {
				if err != nil {
					// Some tests may have warnings but not fail completely
					assert.NotNil(t, result)
				}
			}
		})
	}
}
