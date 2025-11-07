// Package orchestration tests for the multi-domain DSL orchestration engine.
//
// This file provides comprehensive tests for the orchestrator's core functionality:
// - Context analysis and domain discovery
// - Execution plan generation with dependency resolution
// - Multi-domain session creation and management
// - Cross-domain instruction execution
// - DSL accumulation and state management
// - Session lifecycle and cleanup
//
// The tests cover various entity types (CORPORATE, TRUST, PROPER_PERSON) and
// product combinations to verify the orchestrator correctly determines
// required domains and builds appropriate execution plans.
package orchestration

import (
	"context"
	"fmt"
	"strings"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	registry "dsl-ob-poc/internal/domain-registry"
	"dsl-ob-poc/internal/shared-dsl/session"
)

// MockDomain implements the Domain interface for testing
type MockDomain struct {
	name        string
	version     string
	description string
	vocabulary  *registry.Vocabulary
	healthy     bool
}

func NewMockDomain(name string) *MockDomain {
	return &MockDomain{
		name:        name,
		version:     "1.0.0",
		description: "Mock domain for testing: " + name,
		healthy:     true,
		vocabulary: &registry.Vocabulary{
			Domain:      name,
			Version:     "1.0.0",
			Description: "Mock vocabulary for " + name,
			Verbs:       make(map[string]*registry.VerbDefinition),
			Categories:  make(map[string]*registry.VerbCategory),
			States:      []string{"CREATED", "ACTIVE", "COMPLETED"},
			CreatedAt:   time.Now(),
			UpdatedAt:   time.Now(),
		},
	}
}

func (m *MockDomain) Name() string                        { return m.name }
func (m *MockDomain) Version() string                     { return m.version }
func (m *MockDomain) Description() string                 { return m.description }
func (m *MockDomain) GetVocabulary() *registry.Vocabulary { return m.vocabulary }
func (m *MockDomain) IsHealthy() bool                     { return m.healthy }

func (m *MockDomain) ValidateVerbs(dsl string) error                { return nil }
func (m *MockDomain) ValidateStateTransition(from, to string) error { return nil }

func (m *MockDomain) GenerateDSL(ctx context.Context, req *registry.GenerationRequest) (*registry.GenerationResponse, error) {
	// Mock DSL generation
	mockDSL := "(" + m.name + ".mock (instruction \"" + req.Instruction + "\"))"
	return &registry.GenerationResponse{
		DSL:         mockDSL,
		Verb:        m.name + ".mock",
		Parameters:  map[string]interface{}{"instruction": req.Instruction},
		IsValid:     true,
		Confidence:  0.9,
		Explanation: "Mock DSL generated for " + m.name,
		Timestamp:   time.Now(),
	}, nil
}

func (m *MockDomain) GetCurrentState(context map[string]interface{}) (string, error) {
	return "ACTIVE", nil
}

func (m *MockDomain) GetValidStates() []string {
	return []string{"CREATED", "ACTIVE", "COMPLETED"}
}

func (m *MockDomain) GetInitialState() string {
	return "CREATED"
}

func (m *MockDomain) ExtractContext(dsl string) (map[string]interface{}, error) {
	return map[string]interface{}{
		"domain": m.name,
		"dsl":    dsl,
	}, nil
}

func (m *MockDomain) GetMetrics() *registry.DomainMetrics {
	return &registry.DomainMetrics{
		TotalRequests:      10,
		SuccessfulRequests: 9,
		FailedRequests:     1,
		TotalVerbs:         5,
		ActiveVerbs:        4,
		UnusedVerbs:        1,
		IsHealthy:          m.healthy,
		LastHealthCheck:    time.Now(),
		CollectedAt:        time.Now(),
		Version:            m.version,
	}
}

// Test helper functions

func setupTestOrchestrator(t *testing.T) *Orchestrator {
	// Create registry and register mock domains
	reg := registry.NewRegistry()

	domains := []string{
		"onboarding", "kyc", "ubo", "hedge-fund-investor",
		"compliance", "custody", "trading", "trust-kyc",
	}

	for _, domainName := range domains {
		domain := NewMockDomain(domainName)
		err := reg.Register(domain)
		require.NoError(t, err, "Failed to register domain %s", domainName)
	}

	// Create session manager
	sessionMgr := session.NewManager()

	// Create orchestrator with test config
	config := &OrchestratorConfig{
		MaxConcurrentSessions: 10,
		SessionTimeout:        1 * time.Hour,
		EnableOptimization:    true,
		EnableParallelExec:    true,
		MaxDomainDepth:        3,
		ContextPropagationTTL: 30 * time.Minute,
	}

	return NewOrchestrator(reg, sessionMgr, config)
}

// TestOrchestratorCreation tests basic orchestrator creation and configuration
func TestOrchestratorCreation(t *testing.T) {
	orchestrator := setupTestOrchestrator(t)

	assert.NotNil(t, orchestrator)
	assert.NotNil(t, orchestrator.registry)
	assert.NotNil(t, orchestrator.sessionManager)
	assert.NotNil(t, orchestrator.config)
	assert.NotNil(t, orchestrator.metrics)

	// Test metrics initialization
	metrics := orchestrator.GetMetrics()
	assert.Equal(t, int64(0), metrics.TotalSessions)
	assert.Equal(t, int64(0), metrics.ActiveSessions)
	assert.NotNil(t, metrics.DomainsCoordinated)
}

// TestContextAnalysis tests context analysis for different entity types and products
func TestContextAnalysis(t *testing.T) {
	orchestrator := setupTestOrchestrator(t)
	ctx := context.Background()

	testCases := []struct {
		name               string
		request            *OrchestrationRequest
		expectedPrimary    string
		expectedDomains    []string
		expectedComplexity string
		expectDependencies bool
	}{
		{
			name: "Individual Onboarding",
			request: &OrchestrationRequest{
				EntityType:   "PROPER_PERSON",
				EntityName:   "John Smith",
				Jurisdiction: "US",
				Products:     []string{"CUSTODY"},
				WorkflowType: "ONBOARDING",
			},
			expectedPrimary:    "onboarding",
			expectedDomains:    []string{"onboarding", "kyc", "custody"},
			expectedComplexity: "MEDIUM",
			expectDependencies: false,
		},
		{
			name: "Corporate Entity with Multiple Products",
			request: &OrchestrationRequest{
				EntityType:   "CORPORATE",
				EntityName:   "Acme Corp",
				Jurisdiction: "US",
				Products:     []string{"CUSTODY", "TRADING", "COMPLIANCE"},
				WorkflowType: "ONBOARDING",
			},
			expectedPrimary:    "onboarding",
			expectedDomains:    []string{"onboarding", "kyc", "ubo", "custody", "trading", "compliance", "us-compliance"},
			expectedComplexity: "HIGH",
			expectDependencies: true,
		},
		{
			name: "Trust Entity EU Jurisdiction",
			request: &OrchestrationRequest{
				EntityType:   "TRUST",
				EntityName:   "Smith Family Trust",
				Jurisdiction: "LU",
				Products:     []string{"CUSTODY"},
				WorkflowType: "ONBOARDING",
			},
			expectedPrimary:    "onboarding",
			expectedDomains:    []string{"onboarding", "kyc", "ubo", "trust-kyc", "custody", "eu-compliance"},
			expectedComplexity: "HIGH",
			expectDependencies: true,
		},
		{
			name: "Hedge Fund Investment Workflow",
			request: &OrchestrationRequest{
				EntityType:   "PROPER_PERSON",
				EntityName:   "John Investor",
				Products:     []string{"HEDGE_FUND_INVESTMENT"},
				WorkflowType: "INVESTMENT",
			},
			expectedPrimary:    "hedge-fund-investor",
			expectedDomains:    []string{"hedge-fund-investor", "kyc"},
			expectedComplexity: "LOW",
			expectDependencies: false,
		},
	}

	for _, tc := range testCases {
		t.Run(tc.name, func(t *testing.T) {
			analysis, err := orchestrator.analyzeOnboardingContext(ctx, tc.request)
			require.NoError(t, err)

			assert.Equal(t, tc.expectedPrimary, analysis.PrimaryDomain, "Primary domain mismatch")
			assert.Equal(t, tc.expectedComplexity, analysis.EstimatedComplexity, "Complexity mismatch")

			// Check that all expected domains are present
			for _, expectedDomain := range tc.expectedDomains {
				assert.Contains(t, analysis.RequiredDomains, expectedDomain,
					"Expected domain %s not found in required domains", expectedDomain)
			}

			// Check dependencies
			if tc.expectDependencies {
				assert.True(t, len(analysis.Dependencies) > 0, "Expected dependencies but none found")
			}
		})
	}
}

// TestExecutionPlanGeneration tests execution plan creation with dependencies
func TestExecutionPlanGeneration(t *testing.T) {
	orchestrator := setupTestOrchestrator(t)
	ctx := context.Background()

	// Corporate entity with complex dependencies
	analysis := &ContextAnalysis{
		PrimaryDomain:   "onboarding",
		RequiredDomains: []string{"onboarding", "kyc", "ubo", "custody"},
		Dependencies: map[string][]string{
			"ubo":     {"kyc"},
			"custody": {"onboarding"},
		},
		EstimatedComplexity: "MEDIUM",
	}

	plan, err := orchestrator.buildExecutionPlan(ctx, analysis)
	require.NoError(t, err)
	assert.NotNil(t, plan)

	// Verify execution stages
	assert.True(t, len(plan.Stages) > 0, "Execution plan should have stages")

	// Verify dependencies are respected in stage ordering
	stageMap := make(map[string]int) // domain -> stage number
	for i, stage := range plan.Stages {
		for _, domain := range stage.Domains {
			stageMap[domain] = i
		}
	}

	// UBO should come after KYC (if they are in different stages)
	if uboStage, exists := stageMap["ubo"]; exists {
		if kycStage, exists := stageMap["kyc"]; exists {
			assert.True(t, uboStage >= kycStage, "UBO stage should come after or with KYC stage")
		}
	}

	// Verify that dependencies are reflected in the plan
	assert.NotNil(t, plan.Dependencies)
	assert.Equal(t, []string{"kyc"}, plan.Dependencies["ubo"], "UBO should depend on KYC")
	assert.Equal(t, []string{"onboarding"}, plan.Dependencies["custody"], "Custody should depend on onboarding")

	// Verify parallel groups are identified
	if len(plan.ParallelGroups) > 0 {
		for _, group := range plan.ParallelGroups {
			assert.True(t, len(group) > 1, "Parallel groups should have multiple domains")
		}
	}
}

// TestOrchestrationSessionCreation tests orchestration session creation
func TestOrchestrationSessionCreation(t *testing.T) {
	orchestrator := setupTestOrchestrator(t)
	ctx := context.Background()

	req := &OrchestrationRequest{
		CBUID:        "CBU-TEST-001",
		EntityType:   "CORPORATE",
		EntityName:   "Test Corp",
		Jurisdiction: "US",
		Products:     []string{"CUSTODY", "TRADING"},
		WorkflowType: "ONBOARDING",
	}

	session, err := orchestrator.CreateOrchestrationSession(ctx, req)
	require.NoError(t, err)
	assert.NotNil(t, session)

	// Verify session properties
	assert.NotEmpty(t, session.SessionID)
	assert.Equal(t, "onboarding", session.PrimaryDomain)
	assert.Equal(t, "CREATED", session.CurrentState)
	assert.Equal(t, 0, session.VersionNumber)
	assert.NotNil(t, session.SharedContext)
	assert.NotNil(t, session.ExecutionPlan)
	assert.NotNil(t, session.ActiveDomains)

	// Verify shared context
	assert.Equal(t, req.CBUID, session.SharedContext.CBUID)
	assert.Equal(t, req.EntityType, session.SharedContext.EntityType)
	assert.Equal(t, req.EntityName, session.SharedContext.EntityName)
	assert.Equal(t, req.Products, session.SharedContext.Products)

	// Verify domain sessions are created
	assert.True(t, len(session.ActiveDomains) > 0, "Should have active domain sessions")

	// Verify session is tracked by orchestrator
	retrieved, err := orchestrator.GetOrchestrationSession(session.SessionID)
	require.NoError(t, err)
	assert.Equal(t, session.SessionID, retrieved.SessionID)

	// Verify metrics updated
	metrics := orchestrator.GetMetrics()
	assert.Equal(t, int64(1), metrics.TotalSessions)
	assert.Equal(t, int64(1), metrics.ActiveSessions)
}

// TestInstructionAnalysis tests instruction analysis and domain routing
func TestInstructionAnalysis(t *testing.T) {
	orchestrator := setupTestOrchestrator(t)
	ctx := context.Background()

	// Create a test session
	req := &OrchestrationRequest{
		EntityType:   "CORPORATE",
		EntityName:   "Test Corp",
		Products:     []string{"CUSTODY", "TRADING"},
		WorkflowType: "ONBOARDING",
	}

	session, err := orchestrator.CreateOrchestrationSession(ctx, req)
	require.NoError(t, err)

	testCases := []struct {
		instruction     string
		expectedDomains []string
	}{
		{
			instruction:     "Create a new client case",
			expectedDomains: []string{"onboarding"},
		},
		{
			instruction:     "Start KYC verification for the client",
			expectedDomains: []string{"kyc"},
		},
		{
			instruction:     "Discover beneficial owners",
			expectedDomains: []string{"ubo"},
		},
		{
			instruction:     "Set up custody account",
			expectedDomains: []string{"custody"},
		},
		{
			instruction:     "Execute trades",
			expectedDomains: []string{"trading"},
		},
	}

	for _, tc := range testCases {
		t.Run(tc.instruction, func(t *testing.T) {
			domains, err := orchestrator.analyzeInstruction(ctx, tc.instruction, session)
			require.NoError(t, err)

			// Check that all expected domains are identified
			for _, expectedDomain := range tc.expectedDomains {
				assert.Contains(t, domains, expectedDomain,
					"Expected domain %s not found for instruction: %s", expectedDomain, tc.instruction)
			}
		})
	}
}

// TestDSLAccumulation tests DSL accumulation across domains
func TestDSLAccumulation(t *testing.T) {
	orchestrator := setupTestOrchestrator(t)
	ctx := context.Background()

	// Create a test session
	req := &OrchestrationRequest{
		EntityType: "PROPER_PERSON",
		EntityName: "John Smith",
		Products:   []string{"CUSTODY"},
	}

	session, err := orchestrator.CreateOrchestrationSession(ctx, req)
	require.NoError(t, err)

	// Test DSL accumulation
	dsl1 := "(onboarding.create (client \"John Smith\"))"
	err = orchestrator.accumulateDSL(ctx, session, "onboarding", dsl1)
	require.NoError(t, err)

	assert.Equal(t, dsl1, session.UnifiedDSL)
	assert.Equal(t, dsl1, session.DomainDSL["onboarding"])
	assert.Equal(t, 1, session.VersionNumber)

	// Add DSL from another domain
	dsl2 := "(kyc.start (client \"John Smith\") (documents \"passport\"))"
	err = orchestrator.accumulateDSL(ctx, session, "kyc", dsl2)
	require.NoError(t, err)

	expectedUnified := dsl1 + "\n\n" + dsl2
	assert.Equal(t, expectedUnified, session.UnifiedDSL)
	assert.Equal(t, dsl2, session.DomainDSL["kyc"])
	assert.Equal(t, 2, session.VersionNumber)

	// Test empty DSL (should be ignored)
	err = orchestrator.accumulateDSL(ctx, session, "custody", "")
	require.NoError(t, err)
	assert.Equal(t, expectedUnified, session.UnifiedDSL) // Should remain unchanged
	assert.Equal(t, 2, session.VersionNumber)            // Version should remain same
}

// TestSessionManagement tests session lifecycle management
func TestSessionManagement(t *testing.T) {
	orchestrator := setupTestOrchestrator(t)
	ctx := context.Background()

	// Create multiple sessions
	sessions := make([]*OrchestrationSession, 3)
	for i := 0; i < 3; i++ {
		req := &OrchestrationRequest{
			EntityType: "PROPER_PERSON",
			EntityName: fmt.Sprintf("Client %d", i+1),
			Products:   []string{"CUSTODY"},
		}

		session, err := orchestrator.CreateOrchestrationSession(ctx, req)
		require.NoError(t, err)
		sessions[i] = session
	}

	// Test session listing
	activeSessionIDs := orchestrator.ListActiveSessions()
	assert.Equal(t, 3, len(activeSessionIDs))

	for _, session := range sessions {
		assert.Contains(t, activeSessionIDs, session.SessionID)
	}

	// Test session status
	status, err := orchestrator.GetSessionStatus(sessions[0].SessionID)
	require.NoError(t, err)
	assert.Equal(t, sessions[0].SessionID, status.SessionID)
	assert.Equal(t, sessions[0].PrimaryDomain, status.PrimaryDomain)
	assert.True(t, len(status.ActiveDomains) > 0)

	// Test metrics
	metrics := orchestrator.GetMetrics()
	assert.Equal(t, int64(3), metrics.TotalSessions)
	assert.Equal(t, int64(3), metrics.ActiveSessions)
}

// TestSessionTimeout tests session cleanup and timeout handling
func TestSessionTimeout(t *testing.T) {
	// Create orchestrator with short timeout for testing
	reg := registry.NewRegistry()
	reg.Register(NewMockDomain("onboarding"))

	sessionMgr := session.NewManager()
	config := &OrchestratorConfig{
		MaxConcurrentSessions: 10,
		SessionTimeout:        100 * time.Millisecond, // Very short for testing
		EnableOptimization:    true,
		EnableParallelExec:    true,
		MaxDomainDepth:        3,
		ContextPropagationTTL: 30 * time.Minute,
	}

	orchestrator := NewOrchestrator(reg, sessionMgr, config)
	ctx := context.Background()

	// Create a session
	req := &OrchestrationRequest{
		EntityType: "PROPER_PERSON",
		EntityName: "Test Client",
		Products:   []string{"CUSTODY"},
	}

	session, err := orchestrator.CreateOrchestrationSession(ctx, req)
	require.NoError(t, err)

	// Verify session exists
	retrieved, err := orchestrator.GetOrchestrationSession(session.SessionID)
	require.NoError(t, err)
	assert.Equal(t, session.SessionID, retrieved.SessionID)

	// Wait for timeout
	time.Sleep(200 * time.Millisecond)

	// Run cleanup
	cleaned := orchestrator.CleanupExpiredSessions()
	assert.Equal(t, 1, cleaned, "Should have cleaned up 1 expired session")

	// Verify session no longer exists
	_, err = orchestrator.GetOrchestrationSession(session.SessionID)
	assert.Error(t, err, "Should get error for expired session")
}

// TestConcurrentSessions tests concurrent session creation and management
func TestConcurrentSessions(t *testing.T) {
	orchestrator := setupTestOrchestrator(t)
	ctx := context.Background()

	// Create sessions concurrently
	sessionCount := 5
	sessionChan := make(chan *OrchestrationSession, sessionCount)
	errorChan := make(chan error, sessionCount)

	for i := 0; i < sessionCount; i++ {
		go func(id int) {
			req := &OrchestrationRequest{
				EntityType: "PROPER_PERSON",
				EntityName: fmt.Sprintf("Concurrent Client %d", id),
				Products:   []string{"CUSTODY"},
			}

			session, err := orchestrator.CreateOrchestrationSession(ctx, req)
			if err != nil {
				errorChan <- err
				return
			}
			sessionChan <- session
		}(i)
	}

	// Collect results
	sessions := make([]*OrchestrationSession, 0, sessionCount)
	errors := make([]error, 0)

	for i := 0; i < sessionCount; i++ {
		select {
		case session := <-sessionChan:
			sessions = append(sessions, session)
		case err := <-errorChan:
			errors = append(errors, err)
		case <-time.After(5 * time.Second):
			t.Fatal("Timeout waiting for concurrent session creation")
		}
	}

	// Verify results
	assert.Empty(t, errors, "Should have no errors in concurrent session creation")
	assert.Equal(t, sessionCount, len(sessions), "Should have created all sessions")

	// Verify all sessions are unique
	sessionIDs := make(map[string]bool)
	for _, session := range sessions {
		assert.False(t, sessionIDs[session.SessionID], "Session IDs should be unique")
		sessionIDs[session.SessionID] = true
	}

	// Verify metrics
	metrics := orchestrator.GetMetrics()
	assert.Equal(t, int64(sessionCount), metrics.TotalSessions)
	assert.Equal(t, int64(sessionCount), metrics.ActiveSessions)
}

// TestSessionLimits tests orchestrator session limits
func TestSessionLimits(t *testing.T) {
	// Create orchestrator with low session limit
	reg := registry.NewRegistry()
	reg.Register(NewMockDomain("onboarding"))

	sessionMgr := session.NewManager()
	config := &OrchestratorConfig{
		MaxConcurrentSessions: 2, // Very low limit for testing
		SessionTimeout:        1 * time.Hour,
		EnableOptimization:    true,
		EnableParallelExec:    true,
		MaxDomainDepth:        3,
		ContextPropagationTTL: 30 * time.Minute,
	}

	orchestrator := NewOrchestrator(reg, sessionMgr, config)
	ctx := context.Background()

	// Create sessions up to limit
	for i := 0; i < 2; i++ {
		req := &OrchestrationRequest{
			EntityType: "PROPER_PERSON",
			EntityName: fmt.Sprintf("Client %d", i+1),
			Products:   []string{"CUSTODY"},
		}

		_, err := orchestrator.CreateOrchestrationSession(ctx, req)
		require.NoError(t, err, "Should create session within limit")
	}

	// Attempt to create session beyond limit
	req := &OrchestrationRequest{
		EntityType: "PROPER_PERSON",
		EntityName: "Excess Client",
		Products:   []string{"CUSTODY"},
	}

	_, err := orchestrator.CreateOrchestrationSession(ctx, req)
	assert.Error(t, err, "Should get error when exceeding session limit")
	assert.Contains(t, err.Error(), "maximum concurrent sessions", "Error should mention session limit")
}

// TestDomainContextBuilding tests building domain-specific context from shared context
func TestDomainContextBuilding(t *testing.T) {
	orchestrator := setupTestOrchestrator(t)
	ctx := context.Background()

	// Create a session with rich context
	req := &OrchestrationRequest{
		CBUID:          "CBU-TEST-001",
		EntityType:     "CORPORATE",
		EntityName:     "Test Corp",
		Jurisdiction:   "US",
		Products:       []string{"CUSTODY", "TRADING"},
		WorkflowType:   "ONBOARDING",
		RiskProfile:    "HIGH",
		ComplianceTier: "ENHANCED",
		InitialContext: map[string]interface{}{
			"custom_field": "custom_value",
		},
	}

	session, err := orchestrator.CreateOrchestrationSession(ctx, req)
	require.NoError(t, err)

	// Add some attribute values to shared context
	session.SharedContext.AttributeValues["attr-001"] = "test_value"
	session.SharedContext.Data["workflow_step"] = "kyc_verification"

	// Add entity references
	session.EntityRefs["investor"] = "uuid-investor-001"
	session.EntityRefs["fund"] = "uuid-fund-001"

	// Build domain context
	domainContext := orchestrator.buildDomainContext(session, "kyc")

	// Verify context contains expected values
	assert.Equal(t, req.CBUID, domainContext["cbu_id"])
	assert.Equal(t, req.EntityType, domainContext["entity_type"])
	assert.Equal(t, req.EntityName, domainContext["entity_name"])
	assert.Equal(t, req.Jurisdiction, domainContext["jurisdiction"])
	assert.Equal(t, req.Products, domainContext["products"])
	assert.Equal(t, req.WorkflowType, domainContext["workflow_type"])
	assert.Equal(t, "HIGH", domainContext["risk_profile"])
	assert.Equal(t, "ENHANCED", domainContext["compliance_tier"])

	// Verify attribute values are included
	assert.Equal(t, "test_value", domainContext["attr-001"])

	// Verify flexible data is included
	assert.Equal(t, "kyc_verification", domainContext["workflow_step"])

	// Verify entity references are included
	assert.Equal(t, "uuid-investor-001", domainContext["investor_ref"])
	assert.Equal(t, "uuid-fund-001", domainContext["fund_ref"])
}

// TestUtilityFunctions tests utility functions
func TestUtilityFunctions(t *testing.T) {
	// Test isEUJurisdiction
	euCountries := []string{"DE", "FR", "IT", "ES", "NL", "LU"}
	for _, country := range euCountries {
		assert.True(t, isEUJurisdiction(country), "Country %s should be recognized as EU", country)
		assert.True(t, isEUJurisdiction(strings.ToLower(country)), "Lowercase %s should work", country)
	}

	nonEUCountries := []string{"US", "UK", "CH", "JP", "CA"}
	for _, country := range nonEUCountries {
		assert.False(t, isEUJurisdiction(country), "Country %s should not be recognized as EU", country)
	}

	// Test removeDuplicates
	testCases := []struct {
		input    []string
		expected []string
	}{
		{
			input:    []string{"a", "b", "c"},
			expected: []string{"a", "b", "c"},
		},
		{
			input:    []string{"a", "b", "a", "c", "b"},
			expected: []string{"a", "b", "c"},
		},
		{
			input:    []string{},
			expected: []string{},
		},
		{
			input:    []string{"single"},
			expected: []string{"single"},
		},
	}

	for _, tc := range testCases {
		result := removeDuplicates(tc.input)
		assert.Equal(t, tc.expected, result, "removeDuplicates failed for input %v", tc.input)
	}
}
