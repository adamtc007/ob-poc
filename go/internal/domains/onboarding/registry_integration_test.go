package onboarding

import (
	"testing"

	registry "dsl-ob-poc/internal/domain-registry"
	"dsl-ob-poc/internal/shared-dsl/session"
)

// TestOnboardingRegistryIntegration tests basic integration with the domain registry
func TestOnboardingRegistryIntegration(t *testing.T) {
	// Create registry and onboarding domain
	reg := registry.NewRegistry()
	domain := NewDomain()

	// Test domain registration
	err := reg.Register(domain)
	if err != nil {
		t.Fatalf("Failed to register onboarding domain: %v", err)
	}

	// Test domain lookup
	retrievedDomain, err := reg.Get("onboarding")
	if err != nil {
		t.Fatalf("Failed to get onboarding domain from registry: %v", err)
	}

	if retrievedDomain.Name() != "onboarding" {
		t.Errorf("Expected domain name 'onboarding', got '%s'", retrievedDomain.Name())
	}

	// Test vocabulary retrieval through registry
	vocab, err := reg.GetVocabulary("onboarding")
	if err != nil {
		t.Fatalf("Failed to get vocabulary from registry: %v", err)
	}

	if vocab == nil {
		t.Fatal("Registry returned nil vocabulary")
	}

	if len(vocab.Verbs) != 54 {
		t.Errorf("Expected 54 verbs through registry, got %d", len(vocab.Verbs))
	}

	// Test specific verb lookup
	caseCreateVerb, exists := vocab.Verbs["case.create"]
	if !exists {
		t.Error("case.create verb not found through registry")
	} else {
		if caseCreateVerb.Name != "case.create" {
			t.Errorf("Expected verb name 'case.create', got '%s'", caseCreateVerb.Name)
		}
		if caseCreateVerb.Category != "case-management" {
			t.Errorf("Expected category 'case-management', got '%s'", caseCreateVerb.Category)
		}
	}

	// Test category lookup
	caseMgmtCategory, exists := vocab.Categories["case-management"]
	if !exists {
		t.Error("case-management category not found through registry")
	} else {
		if len(caseMgmtCategory.Verbs) != 5 {
			t.Errorf("Expected 5 verbs in case-management category, got %d", len(caseMgmtCategory.Verbs))
		}
	}

	// Test domain listing
	domains := reg.List()
	found := false
	for _, domainName := range domains {
		if domainName == "onboarding" {
			found = true
			break
		}
	}
	if !found {
		t.Error("Onboarding domain not found in registry list")
	}

	// Test health status
	if !retrievedDomain.IsHealthy() {
		t.Error("Onboarding domain should be healthy")
	}

	// Test metrics
	metrics := retrievedDomain.GetMetrics()
	if metrics == nil {
		t.Error("Expected metrics from onboarding domain")
	} else {
		if metrics.TotalVerbs != 54 {
			t.Errorf("Expected 54 verbs in metrics, got %d", metrics.TotalVerbs)
		}
		if !metrics.IsHealthy {
			t.Error("Domain metrics should show healthy status")
		}
	}

	// Test registry metrics
	regMetrics := reg.GetMetrics()
	if regMetrics.TotalDomains != 1 {
		t.Errorf("Expected 1 domain in registry, got %d", regMetrics.TotalDomains)
	}

	if regMetrics.HealthyDomains != 1 {
		t.Errorf("Expected 1 healthy domain in registry, got %d", regMetrics.HealthyDomains)
	}
}

// TestOnboardingVerbValidationIntegration tests verb validation through registry
func TestOnboardingVerbValidationIntegration(t *testing.T) {
	reg := registry.NewRegistry()
	domain := NewDomain()
	reg.Register(domain)

	retrievedDomain, _ := reg.Get("onboarding")

	// Test valid DSL
	validDSL := `(case.create (cbu.id "CBU-TEST") (nature-purpose "Registry integration test"))`
	err := retrievedDomain.ValidateVerbs(validDSL)
	if err != nil {
		t.Errorf("Expected valid DSL to pass validation, got: %v", err)
	}

	// Test invalid DSL
	invalidDSL := `(invalid.verb (param "value"))`
	err = retrievedDomain.ValidateVerbs(invalidDSL)
	if err == nil {
		t.Error("Expected invalid DSL to fail validation")
	}

	// Test complex valid DSL
	complexDSL := `(case.create (cbu.id "CBU-COMPLEX") (nature-purpose "Complex test"))
(products.add "CUSTODY" "FUND_ACCOUNTING")
(kyc.start (requirements (document "CertificateOfIncorporation")))`

	err = retrievedDomain.ValidateVerbs(complexDSL)
	if err != nil {
		t.Errorf("Expected complex valid DSL to pass validation, got: %v", err)
	}
}

// TestOnboardingStateManagementIntegration tests state management through registry
func TestOnboardingStateManagementIntegration(t *testing.T) {
	reg := registry.NewRegistry()
	domain := NewDomain()
	reg.Register(domain)

	retrievedDomain, _ := reg.Get("onboarding")

	// Test valid state transitions
	validTransitions := [][2]string{
		{"CREATE", "PRODUCTS_ADDED"},
		{"PRODUCTS_ADDED", "KYC_STARTED"},
		{"KYC_STARTED", "SERVICES_DISCOVERED"},
	}

	for _, transition := range validTransitions {
		from, to := transition[0], transition[1]
		err := retrievedDomain.ValidateStateTransition(from, to)
		if err != nil {
			t.Errorf("Expected valid transition %s -> %s, got error: %v", from, to, err)
		}
	}

	// Test invalid state transition
	err := retrievedDomain.ValidateStateTransition("CREATE", "COMPLETE")
	if err == nil {
		t.Error("Expected invalid state transition to fail")
	}

	// Test state extraction
	dsl := `(case.create (cbu.id "CBU-STATE-TEST") (nature-purpose "State test"))
(products.add "CUSTODY")`

	context, err := retrievedDomain.ExtractContext(dsl)
	if err != nil {
		t.Errorf("Failed to extract context: %v", err)
	}

	if context["current_state"] != "PRODUCTS_ADDED" {
		t.Errorf("Expected state 'PRODUCTS_ADDED', got '%v'", context["current_state"])
	}

	if context["cbu_id"] != "CBU-STATE-TEST" {
		t.Errorf("Expected CBU ID 'CBU-STATE-TEST', got '%v'", context["cbu_id"])
	}
}

// TestOnboardingEndToEndIntegration tests complete onboarding workflow through domain registry
func TestOnboardingEndToEndIntegration(t *testing.T) {
	// Setup multi-domain registry system
	reg := registry.NewRegistry()
	onboardingDomain := NewDomain()

	err := reg.Register(onboardingDomain)
	if err != nil {
		t.Fatalf("Failed to register onboarding domain: %v", err)
	}

	// Test complete onboarding workflow
	workflowSteps := []struct {
		description   string
		dsl           string
		expectedState string
		shouldPass    bool
	}{
		{
			description:   "Create initial case",
			dsl:           `(case.create (cbu.id "CBU-E2E-TEST") (nature-purpose "End-to-end integration test"))`,
			expectedState: "CREATE",
			shouldPass:    true,
		},
		{
			description:   "Add products to case",
			dsl:           `(products.add "CUSTODY" "FUND_ACCOUNTING" "TRANSFER_AGENT")`,
			expectedState: "PRODUCTS_ADDED",
			shouldPass:    true,
		},
		{
			description:   "Start KYC process",
			dsl:           `(kyc.start (requirements (document "CertificateOfIncorporation") (jurisdiction "LU")))`,
			expectedState: "KYC_STARTED",
			shouldPass:    true,
		},
		{
			description:   "Discover required services",
			dsl:           `(services.discover (for.product "CUSTODY" (service "AccountOpening") (service "TradeSettlement")))`,
			expectedState: "SERVICES_DISCOVERED",
			shouldPass:    true,
		},
		{
			description:   "Plan infrastructure resources",
			dsl:           `(resources.plan (resource.create "CustodyAccount" (owner "CustodyTech") (var (attr-id "acc-uuid-001"))))`,
			expectedState: "RESOURCES_PLANNED",
			shouldPass:    true,
		},
		{
			description:   "Bind attribute values",
			dsl:           `(values.bind (bind (attr-id "acc-uuid-001") (value "CUST-ACC-E2E-001")))`,
			expectedState: "ATTRIBUTES_BOUND",
			shouldPass:    true,
		},
		{
			description:   "Activate workflow",
			dsl:           `(workflow.transition (from "ATTRIBUTES_BOUND") (to "WORKFLOW_ACTIVE"))`,
			expectedState: "WORKFLOW_ACTIVE",
			shouldPass:    true,
		},
		{
			description:   "Close completed case",
			dsl:           `(case.close (reason "End-to-end test completed successfully") (final-state "ACTIVE"))`,
			expectedState: "COMPLETE",
			shouldPass:    true,
		},
		{
			description:   "Invalid verb should fail",
			dsl:           `(invalid.operation (param "test"))`,
			expectedState: "",
			shouldPass:    false,
		},
	}

	var accumulatedDSL string

	for i, step := range workflowSteps {
		t.Logf("Step %d: %s", i+1, step.description)

		// Get domain from registry
		domain, err := reg.Get("onboarding")
		if err != nil {
			t.Fatalf("Step %d: Failed to get domain: %v", i+1, err)
		}

		// Validate individual DSL step
		err = domain.ValidateVerbs(step.dsl)
		if step.shouldPass && err != nil {
			t.Errorf("Step %d: Expected valid DSL to pass, got: %v", i+1, err)
			continue
		}
		if !step.shouldPass && err == nil {
			t.Errorf("Step %d: Expected invalid DSL to fail validation", i+1)
			continue
		}

		if !step.shouldPass {
			continue // Skip accumulation for invalid steps
		}

		// Accumulate DSL
		// Use session manager for DSL accumulation (single source of truth)
		sessionMgr := session.NewManager()
		dslSession := sessionMgr.GetOrCreate("test-integration", "onboarding")

		if accumulatedDSL == "" {
			err := dslSession.AccumulateDSL(step.dsl)
			if err != nil {
				t.Fatalf("Failed to accumulate DSL: %v", err)
			}
			accumulatedDSL = dslSession.GetDSL()
		} else {
			err := dslSession.AccumulateDSL(accumulatedDSL)
			if err != nil {
				t.Fatalf("Failed to accumulate existing DSL: %v", err)
			}
			err = dslSession.AccumulateDSL(step.dsl)
			if err != nil {
				t.Fatalf("Failed to accumulate new DSL: %v", err)
			}
			accumulatedDSL = dslSession.GetDSL()
		}

		// Validate accumulated DSL
		err = domain.ValidateVerbs(accumulatedDSL)
		if err != nil {
			t.Errorf("Step %d: Accumulated DSL validation failed: %v", i+1, err)
			continue
		}

		// Extract context and verify state
		context, err := domain.ExtractContext(accumulatedDSL)
		if err != nil {
			t.Errorf("Step %d: Context extraction failed: %v", i+1, err)
			continue
		}

		// Verify state progression
		if context["current_state"] != step.expectedState {
			t.Errorf("Step %d: Expected state '%s', got '%v'",
				i+1, step.expectedState, context["current_state"])
		}

		// Verify CBU ID is maintained throughout
		if context["cbu_id"] != "CBU-E2E-TEST" {
			t.Errorf("Step %d: CBU ID not maintained in context", i+1)
		}

		t.Logf("  ✓ State: %s", step.expectedState)
	}

	// Final verification
	if accumulatedDSL == "" {
		t.Fatal("No DSL was accumulated")
	}

	// Test final accumulated DSL contains all expected verbs
	expectedVerbs := []string{
		"case.create", "products.add", "kyc.start", "services.discover",
		"resources.plan", "values.bind", "workflow.transition", "case.close",
	}

	for _, expectedVerb := range expectedVerbs {
		if !containsString(accumulatedDSL, expectedVerb) {
			t.Errorf("Final DSL missing expected verb: %s", expectedVerb)
		}
	}

	// Test vocabulary completeness through registry
	vocab, err := reg.GetVocabulary("onboarding")
	if err != nil {
		t.Fatalf("Failed to get final vocabulary: %v", err)
	}

	// Verify all major categories are present
	expectedCategories := []string{
		"case-management", "entity-identity", "product-service",
		"kyc-compliance", "resource-infrastructure", "attribute-data",
		"workflow-state", "notification-communication", "integration-external",
		"temporal-scheduling", "risk-monitoring", "data-lifecycle",
	}

	for _, expectedCategory := range expectedCategories {
		if _, exists := vocab.Categories[expectedCategory]; !exists {
			t.Errorf("Missing expected category: %s", expectedCategory)
		}
	}

	t.Logf("End-to-end test completed successfully")
	t.Logf("Final accumulated DSL length: %d characters", len(accumulatedDSL))
	t.Logf("Total verbs in vocabulary: %d", len(vocab.Verbs))
	t.Logf("Total categories: %d", len(vocab.Categories))
}

// containsString checks if a string contains a substring
func containsString(text, substr string) bool {
	return len(text) >= len(substr) &&
		(text == substr ||
			text[:len(substr)] == substr ||
			text[len(text)-len(substr):] == substr ||
			findInString(text, substr))
}

// findInString searches for substring in text
func findInString(text, substr string) bool {
	for i := 0; i <= len(text)-len(substr); i++ {
		if text[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}
