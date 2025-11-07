package orchestration

import (
	"context"
	"strings"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestDSLCompositionEngine_ComposeMasterDSL(t *testing.T) {
	tests := []struct {
		name        string
		request     *CompositionRequest
		expectError bool
		validate    func(t *testing.T, result *CompositionResult)
	}{
		{
			name: "Corporate Entity with Custody and Trading",
			request: &CompositionRequest{
				EntityName:     "Goldman Sachs Asset Management",
				EntityType:     "CORPORATE",
				Jurisdiction:   "US",
				Products:       []string{"CUSTODY", "TRADING"},
				WorkflowType:   "ONBOARDING",
				ComplianceTier: "ENHANCED",
				RequiredDomains: []string{
					"onboarding", "kyc", "ubo", "custody", "trading", "us-compliance",
				},
				DomainDependencies: map[string][]string{
					"kyc":           {"onboarding"},
					"ubo":           {"kyc"},
					"custody":       {"onboarding", "ubo"},
					"trading":       {"custody"},
					"us-compliance": {"kyc", "ubo"},
				},
				SessionID:   "test-session-001",
				CBUID:       "CBU-GS-001",
				RequestedAt: time.Now(),
			},
			expectError: false,
			validate: func(t *testing.T, result *CompositionResult) {
				// Verify master DSL contains expected components
				assert.Contains(t, result.MasterDSL, "orchestration.session.initialize")
				assert.Contains(t, result.MasterDSL, "Goldman Sachs Asset Management")
				assert.Contains(t, result.MasterDSL, "CORPORATE")
				assert.Contains(t, result.MasterDSL, "US")
				assert.Contains(t, result.MasterDSL, "CUSTODY")
				assert.Contains(t, result.MasterDSL, "TRADING")

				// Verify execution plan has proper stages
				assert.NotNil(t, result.ExecutionPlan)
				assert.GreaterOrEqual(t, len(result.ExecutionPlan.Stages), 3)

				// Verify dependency graph
				assert.Contains(t, result.DependencyGraph, "ubo")
				assert.Contains(t, result.DependencyGraph["ubo"], "kyc")

				// Verify component DSLs generated
				assert.NotEmpty(t, result.ComponentDSLs)
				assert.Contains(t, result.ComponentDSLs, "entity")

				// Verify validation results
				if len(result.ValidationResults) > 0 {
					for _, vr := range result.ValidationResults {
						assert.True(t, vr.IsValid, "Component %s should be valid: %v", vr.Component, vr.Errors)
					}
				}
			},
		},
		{
			name: "Trust Entity with UBO Discovery",
			request: &CompositionRequest{
				EntityName:     "Luxembourg Family Trust",
				EntityType:     "TRUST",
				Jurisdiction:   "LU",
				Products:       []string{"CUSTODY"},
				WorkflowType:   "ONBOARDING",
				ComplianceTier: "STANDARD",
				RequiredDomains: []string{
					"onboarding", "kyc", "trust-kyc", "ubo", "custody", "eu-compliance",
				},
				DomainDependencies: map[string][]string{
					"kyc":           {"onboarding"},
					"trust-kyc":     {"kyc"},
					"ubo":           {"trust-kyc"},
					"custody":       {"onboarding", "ubo"},
					"eu-compliance": {"kyc", "ubo"},
				},
				SessionID:   "test-session-002",
				CBUID:       "CBU-LU-TRUST-001",
				RequestedAt: time.Now(),
			},
			expectError: false,
			validate: func(t *testing.T, result *CompositionResult) {
				// Verify trust-specific DSL elements
				assert.Contains(t, result.MasterDSL, "Luxembourg Family Trust")
				assert.Contains(t, result.MasterDSL, "TRUST")
				assert.Contains(t, result.MasterDSL, "LU")

				// Verify EU compliance elements would be included
				assert.NotEmpty(t, result.MasterDSL)

				// Verify trust-specific execution plan
				assert.NotNil(t, result.ExecutionPlan)

				// Trust workflows should have trust-kyc dependency
				assert.Contains(t, result.DependencyGraph, "ubo")
				assert.Contains(t, result.DependencyGraph["ubo"], "trust-kyc")
			},
		},
		{
			name: "Individual Hedge Fund Investor",
			request: &CompositionRequest{
				EntityName:     "John Smith",
				EntityType:     "PROPER_PERSON",
				Jurisdiction:   "US",
				Products:       []string{"HEDGE_FUND_INVESTMENT"},
				WorkflowType:   "INVESTMENT",
				ComplianceTier: "STANDARD",
				RequiredDomains: []string{
					"onboarding", "kyc", "hedge-fund-investor",
				},
				DomainDependencies: map[string][]string{
					"kyc":                 {"onboarding"},
					"hedge-fund-investor": {"kyc"},
				},
				SessionID:   "test-session-003",
				CBUID:       "CBU-HF-PROPER_PERSON-001",
				RequestedAt: time.Now(),
			},
			expectError: false,
			validate: func(t *testing.T, result *CompositionResult) {
				// Verify individual-specific DSL elements
				assert.Contains(t, result.MasterDSL, "John Smith")
				assert.Contains(t, result.MasterDSL, "PROPER_PERSON")
				assert.Contains(t, result.MasterDSL, "HEDGE_FUND_INVESTMENT")

				// Individual workflows should be simpler (no UBO)
				assert.NotContains(t, result.DependencyGraph, "ubo")

				// Should contain hedge fund specific elements
				assert.NotEmpty(t, result.ComponentDSLs)
			},
		},
		{
			name: "Empty Request - Should Error",
			request: &CompositionRequest{
				SessionID:   "empty-test",
				RequestedAt: time.Now(),
			},
			expectError: true,
			validate:    nil,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			// Initialize DSL composition engine
			generator := NewDSLGenerator(&DSLGeneratorConfig{
				EnableTemplateCache: true,
				MaxTemplateDepth:    10,
				ValidateGenerated:   true,
				IncludeComments:     false,
			})

			engine := NewDSLCompositionEngine(generator, &CompositionConfig{
				EnableDependencyOptimization: true,
				EnableParallelGeneration:     true,
				MaxCompositionDepth:          5,
				ValidateComposedDSL:          true,
				IncludeGenerationMetadata:    true,
				OptimizeExecutionOrder:       true,
			})

			ctx := context.Background()
			result, err := engine.ComposeMasterDSL(ctx, tt.request)

			if tt.expectError {
				assert.Error(t, err)
				return
			}

			require.NoError(t, err)
			require.NotNil(t, result)
			assert.NotEmpty(t, result.MasterDSL)

			if tt.validate != nil {
				tt.validate(t, result)
			}
		})
	}
}

func TestDSLCompositionEngine_BuildDependencyGraph(t *testing.T) {
	generator := NewDSLGenerator(nil)
	engine := NewDSLCompositionEngine(generator, nil)

	tests := []struct {
		name     string
		request  *CompositionRequest
		expected map[string][]string
	}{
		{
			name: "Corporate Entity Dependencies",
			request: &CompositionRequest{
				EntityType:   "CORPORATE",
				Jurisdiction: "US",
				Products:     []string{"CUSTODY", "TRADING"},
			},
			expected: map[string][]string{
				"kyc":           {"onboarding"},
				"ubo":           {"kyc"},
				"custody":       {"onboarding"},
				"trading":       {"onboarding", "custody"},
				"us-compliance": {"kyc"},
			},
		},
		{
			name: "Trust Entity Dependencies",
			request: &CompositionRequest{
				EntityType:   "TRUST",
				Jurisdiction: "LU",
				Products:     []string{"CUSTODY"},
			},
			expected: map[string][]string{
				"kyc":           {"onboarding"},
				"trust-kyc":     {"kyc"},
				"ubo":           {"trust-kyc"},
				"custody":       {"onboarding"},
				"eu-compliance": {"kyc"},
			},
		},
		{
			name: "Individual Simple Dependencies",
			request: &CompositionRequest{
				EntityType:   "PROPER_PERSON",
				Jurisdiction: "US",
				Products:     []string{"HEDGE_FUND_INVESTMENT"},
			},
			expected: map[string][]string{
				"kyc":                 {"onboarding"},
				"hedge-fund-investor": {"kyc"},
			},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result, err := engine.buildDependencyGraph(tt.request)
			require.NoError(t, err)

			// Verify key dependencies exist
			for domain, deps := range tt.expected {
				assert.Contains(t, result, domain, "Domain %s should be in dependency graph", domain)
				for _, expectedDep := range deps {
					assert.Contains(t, result[domain], expectedDep, "Domain %s should depend on %s", domain, expectedDep)
				}
			}
		})
	}
}

func TestDSLCompositionEngine_GenerateExecutionPlan(t *testing.T) {
	generator := NewDSLGenerator(nil)
	engine := NewDSLCompositionEngine(generator, nil)

	request := &CompositionRequest{
		EntityType:   "CORPORATE",
		Jurisdiction: "US",
		Products:     []string{"CUSTODY", "TRADING"},
	}

	depGraph := map[string][]string{
		"kyc":           {"onboarding"},
		"ubo":           {"kyc"},
		"custody":       {"onboarding"},
		"trading":       {"custody"},
		"us-compliance": {"kyc"},
	}

	plan, err := engine.generateExecutionPlan(request, depGraph)
	require.NoError(t, err)
	require.NotNil(t, plan)

	// Verify execution plan structure
	assert.GreaterOrEqual(t, len(plan.Stages), 3, "Should have at least 3 execution stages")

	// Verify stage ordering - onboarding should be in first stage
	foundOnboarding := false
	for _, domain := range plan.Stages[0].Domains {
		if domain == "onboarding" {
			foundOnboarding = true
			break
		}
	}
	assert.True(t, foundOnboarding, "onboarding should be in first execution stage")

	// Verify dependencies are respected
	for _, stage := range plan.Stages {
		assert.GreaterOrEqual(t, stage.StageNumber, 1)
		assert.NotEmpty(t, stage.Domains)
	}

	// Verify parallel groups
	assert.NotNil(t, plan.ParallelGroups)

	// Verify estimated duration
	assert.Greater(t, plan.EstimatedDuration, time.Duration(0))
}

func TestDSLGenerator_GenerateMasterDSL(t *testing.T) {
	generator := NewDSLGenerator(&DSLGeneratorConfig{
		EnableTemplateCache: true,
		ValidateGenerated:   true,
	})

	templateCtx := &TemplateContext{
		EntityType:   "CORPORATE",
		EntityName:   "Test Corporation",
		Jurisdiction: "US",
		Products:     []string{"CUSTODY"},
		WorkflowType: "ONBOARDING",
		SessionID:    "test-session",
		CBUID:        "CBU-TEST-001",
		CreatedAt:    time.Now().Format(time.RFC3339),
	}

	ctx := context.Background()
	result, err := generator.GenerateMasterDSL(ctx, templateCtx)

	require.NoError(t, err)
	assert.NotEmpty(t, result)

	// Verify basic DSL structure
	assert.Contains(t, result, "orchestration.initialize")
	assert.Contains(t, result, "Test Corporation")
	assert.Contains(t, result, "CORPORATE")
	assert.Contains(t, result, "US")
	assert.Contains(t, result, "CUSTODY")

	// Verify DSL syntax is valid (basic parentheses matching)
	openCount := strings.Count(result, "(")
	closeCount := strings.Count(result, ")")
	assert.Equal(t, openCount, closeCount, "DSL should have matching parentheses")
}

func TestDSLCompositionEngine_ValidationResults(t *testing.T) {
	generator := NewDSLGenerator(nil)
	engine := NewDSLCompositionEngine(generator, &CompositionConfig{
		ValidateComposedDSL: true,
	})

	masterDSL := `
(orchestration.session.initialize
  (session.id "test-session")
  (entity.name "Test Entity")
)

(workflow.entity.corporate
  (entity.name "Test Entity")
  (kyc.tier "STANDARD")
)
`

	components := map[string]string{
		"entity":  "(workflow.entity.corporate (entity.name \"Test Entity\"))",
		"custody": "(custody.account.create (account.type \"PRIME_BROKERAGE\"))",
		"invalid": "(unclosed.expression (missing.close", // Invalid DSL
	}

	results, err := engine.validateComposedDSL(masterDSL, components)
	require.NoError(t, err)
	require.NotEmpty(t, results)

	// Find validation results
	var masterResult, invalidResult *ValidationResult
	for i := range results {
		if results[i].Component == "master" {
			masterResult = &results[i]
		}
		if results[i].Component == "invalid" {
			invalidResult = &results[i]
		}
	}

	// Master DSL should be valid
	require.NotNil(t, masterResult)
	assert.True(t, masterResult.IsValid)
	assert.Empty(t, masterResult.Errors)

	// Invalid component should be marked invalid
	require.NotNil(t, invalidResult)
	assert.False(t, invalidResult.IsValid)
	assert.NotEmpty(t, invalidResult.Errors)
}

func TestDSLCompositionEngine_ProductMetadata(t *testing.T) {
	generator := NewDSLGenerator(nil)
	engine := NewDSLCompositionEngine(generator, nil)

	// Test product metadata conversion
	productComposition := map[string]*ProductComposition{
		"CUSTODY": {
			ProductID:         "CUSTODY",
			Priority:          1,
			RequiredTemplates: []string{"custody_requirements"},
			AttributeOverrides: map[string]interface{}{
				"RequiresSegregation": true,
				"AccountType":         "PRIME_BROKERAGE",
			},
		},
	}

	templateCtx := engine.buildTemplateContext(&CompositionRequest{
		EntityName:      "Test Entity",
		EntityType:      "CORPORATE",
		Jurisdiction:    "US",
		Products:        []string{"CUSTODY"},
		ProductMetadata: productComposition,
	})

	// Verify template context built correctly
	assert.Equal(t, "Test Entity", templateCtx.EntityName)
	assert.Equal(t, "CORPORATE", templateCtx.EntityType)
	assert.Equal(t, "US", templateCtx.Jurisdiction)
	assert.Contains(t, templateCtx.Products, "CUSTODY")

	// Verify product metadata conversion
	assert.Contains(t, templateCtx.ProductMetadata, "CUSTODY")
	custodyTemplate := templateCtx.ProductMetadata["CUSTODY"]
	assert.Equal(t, "CUSTODY", custodyTemplate.ProductID)
	assert.Contains(t, custodyTemplate.DSLFragments, "custody_requirements")
}

func TestDSLCompositionEngine_ComplexDependencyResolution(t *testing.T) {
	generator := NewDSLGenerator(nil)
	engine := NewDSLCompositionEngine(generator, nil)

	// Complex dependency graph with potential conflicts
	depGraph := map[string][]string{
		"kyc":             {"onboarding"},
		"trust-kyc":       {"kyc"},
		"ubo":             {"trust-kyc"},
		"custody":         {"onboarding", "ubo"},
		"trading":         {"custody"},
		"compliance":      {"kyc", "ubo"},
		"fund-accounting": {"custody"},
	}

	stages, err := engine.topologicalSort(depGraph)
	require.NoError(t, err)
	require.NotEmpty(t, stages)

	// Verify topological ordering
	domainToStage := make(map[string]int)
	for stageIdx, stage := range stages {
		for _, domain := range stage {
			domainToStage[domain] = stageIdx
		}
	}

	// Verify dependencies are respected
	for domain, deps := range depGraph {
		domainStage := domainToStage[domain]
		for _, dep := range deps {
			depStage, exists := domainToStage[dep]
			assert.True(t, exists, "Dependency %s should exist in stages", dep)
			assert.Less(t, depStage, domainStage, "Dependency %s should come before %s", dep, domain)
		}
	}

	// Verify no circular dependencies
	assert.GreaterOrEqual(t, len(stages), 4, "Should have multiple execution stages for complex workflow")
}

func TestDSLCompositionEngine_ErrorHandling(t *testing.T) {
	generator := NewDSLGenerator(nil)
	engine := NewDSLCompositionEngine(generator, nil)

	ctx := context.Background()

	tests := []struct {
		name    string
		request *CompositionRequest
		wantErr bool
		errMsg  string
	}{
		{
			name:    "Nil Request",
			request: nil,
			wantErr: true,
		},
		{
			name: "Empty Entity Type",
			request: &CompositionRequest{
				EntityName:   "Test Entity",
				EntityType:   "",
				Jurisdiction: "US",
				SessionID:    "test-session",
				RequestedAt:  time.Now(),
			},
			wantErr: true,
		},
		{
			name: "Unsupported Entity Type",
			request: &CompositionRequest{
				EntityName:   "Test Entity",
				EntityType:   "UNSUPPORTED_TYPE",
				Jurisdiction: "US",
				SessionID:    "test-session",
				RequestedAt:  time.Now(),
			},
			wantErr: false, // Should handle gracefully or use fallback
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result, err := engine.ComposeMasterDSL(ctx, tt.request)

			if tt.wantErr {
				assert.Error(t, err)
				if tt.errMsg != "" {
					assert.Contains(t, err.Error(), tt.errMsg)
				}
				return
			}

			// Even for edge cases, should not crash
			if err != nil {
				t.Logf("Non-fatal error for edge case: %v", err)
			} else {
				assert.NotNil(t, result)
			}
		})
	}
}
