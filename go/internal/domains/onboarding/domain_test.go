package onboarding

import (
	"context"
	"strings"
	"testing"

	registry "dsl-ob-poc/internal/domain-registry"
)

func TestNewDomain(t *testing.T) {
	domain := NewDomain()

	if domain == nil {
		t.Fatal("NewDomain() returned nil")
	}

	if domain.Name() != "onboarding" {
		t.Errorf("Expected domain name 'onboarding', got '%s'", domain.Name())
	}

	if domain.Version() != "1.0.0" {
		t.Errorf("Expected version '1.0.0', got '%s'", domain.Version())
	}

	if !strings.Contains(domain.Description(), "onboarding") {
		t.Errorf("Expected description to contain 'onboarding', got '%s'", domain.Description())
	}

	if !domain.IsHealthy() {
		t.Error("Expected domain to be healthy")
	}

	vocab := domain.GetVocabulary()
	if vocab == nil {
		t.Fatal("GetVocabulary() returned nil")
	}

	// Verify we have exactly 54 verbs as implemented
	expectedVerbCount := 54
	if len(vocab.Verbs) != expectedVerbCount {
		t.Errorf("Expected %d verbs, got %d", expectedVerbCount, len(vocab.Verbs))

		// Debug: list all verbs that were created
		t.Logf("Created verbs (%d):", len(vocab.Verbs))
		verbNames := make([]string, 0, len(vocab.Verbs))
		for verbName := range vocab.Verbs {
			verbNames = append(verbNames, verbName)
		}
		for i, verbName := range verbNames {
			t.Logf("  %d: %s", i+1, verbName)
		}
	}

	// Verify we have 12 main categories plus offboarding
	expectedCategoryCount := 13
	if len(vocab.Categories) != expectedCategoryCount {
		t.Errorf("Expected %d categories, got %d", expectedCategoryCount, len(vocab.Categories))
	}
}

func TestGetValidStates(t *testing.T) {
	domain := NewDomain()
	states := domain.GetValidStates()

	expectedStates := []string{
		"CREATE", "PRODUCTS_ADDED", "KYC_STARTED", "SERVICES_DISCOVERED",
		"RESOURCES_PLANNED", "ATTRIBUTES_BOUND", "WORKFLOW_ACTIVE", "COMPLETE",
	}

	if len(states) != len(expectedStates) {
		t.Errorf("Expected %d states, got %d", len(expectedStates), len(states))
	}

	for i, expected := range expectedStates {
		if i >= len(states) || states[i] != expected {
			t.Errorf("Expected state[%d] = '%s', got '%s'", i, expected, states[i])
		}
	}

	if domain.GetInitialState() != "CREATE" {
		t.Errorf("Expected initial state 'CREATE', got '%s'", domain.GetInitialState())
	}
}

func TestVocabularyStructure(t *testing.T) {
	domain := NewDomain()
	vocab := domain.GetVocabulary()

	// Test vocabulary metadata
	if vocab.Domain != "onboarding" {
		t.Errorf("Expected vocabulary domain 'onboarding', got '%s'", vocab.Domain)
	}

	if vocab.Version != "1.0.0" {
		t.Errorf("Expected vocabulary version '1.0.0', got '%s'", vocab.Version)
	}

	// Test categories structure - we should have exactly these categories
	expectedCategories := map[string]int{
		"case-management":            5, // case.create, case.update, case.validate, case.approve, case.close
		"entity-identity":            5, // entity.register, entity.classify, entity.link, identity.verify, identity.attest
		"product-service":            5, // products.add, products.configure, services.discover, services.provision, services.activate
		"kyc-compliance":             6, // kyc.start, kyc.collect, kyc.verify, kyc.assess, compliance.screen, compliance.monitor
		"resource-infrastructure":    5, // resources.plan, resources.provision, resources.configure, resources.test, resources.deploy
		"attribute-data":             5, // attributes.define, attributes.resolve, values.bind, values.validate, values.encrypt
		"workflow-state":             5, // workflow.transition, workflow.gate, tasks.create, tasks.assign, tasks.complete
		"notification-communication": 4, // notify.send, communicate.request, escalate.trigger, audit.log
		"integration-external":       4, // external.query, external.sync, api.call, webhook.register
		"offboarding":                1, // case.close (also in case-management for proper categorization)
	}

	for categoryName, expectedVerbCount := range expectedCategories {
		category, exists := vocab.Categories[categoryName]
		if !exists {
			t.Errorf("Expected category '%s' not found", categoryName)
			continue
		}

		if len(category.Verbs) != expectedVerbCount {
			t.Errorf("Category '%s' expected %d verbs, got %d", categoryName, expectedVerbCount, len(category.Verbs))
		}

		// Verify category has proper metadata
		if category.Name != categoryName {
			t.Errorf("Category name mismatch: expected '%s', got '%s'", categoryName, category.Name)
		}

		if category.Description == "" {
			t.Errorf("Category '%s' has empty description", categoryName)
		}

		if category.Color == "" {
			t.Errorf("Category '%s' has empty color", categoryName)
		}

		if category.Icon == "" {
			t.Errorf("Category '%s' has empty icon", categoryName)
		}
	}
}

func TestCoreVerbsPresent(t *testing.T) {
	domain := NewDomain()
	vocab := domain.GetVocabulary()

	// Test that all expected core verbs are present
	coreVerbs := []string{
		// Case Management
		"case.create", "case.update", "case.validate", "case.approve", "case.close",

		// Entity Identity
		"entity.register", "entity.classify", "entity.link", "identity.verify", "identity.attest",

		// Product Service
		"products.add", "products.configure", "services.discover", "services.provision", "services.activate",

		// KYC Compliance
		"kyc.start", "kyc.collect", "kyc.verify", "kyc.assess", "compliance.screen", "compliance.monitor",

		// Resource Infrastructure
		"resources.plan", "resources.provision", "resources.configure", "resources.test", "resources.deploy",

		// Attribute Data
		"attributes.define", "attributes.resolve", "values.bind", "values.validate", "values.encrypt",

		// Workflow State
		"workflow.transition", "workflow.gate", "tasks.create", "tasks.assign", "tasks.complete",

		// Notification Communication
		"notify.send", "communicate.request", "escalate.trigger", "audit.log",

		// Integration External
		"external.query", "external.sync", "api.call", "webhook.register",
	}

	for _, verbName := range coreVerbs {
		verb, exists := vocab.Verbs[verbName]
		if !exists {
			t.Errorf("Expected core verb '%s' not found", verbName)
			continue
		}

		// Verify verb has proper metadata
		if verb.Name != verbName {
			t.Errorf("Verb name mismatch: expected '%s', got '%s'", verbName, verb.Name)
		}

		if verb.Description == "" {
			t.Errorf("Verb '%s' has empty description", verbName)
		}

		if verb.Category == "" {
			t.Errorf("Verb '%s' has empty category", verbName)
		}

		if verb.Version == "" {
			t.Errorf("Verb '%s' has empty version", verbName)
		}

		if len(verb.Examples) == 0 {
			t.Errorf("Verb '%s' has no examples", verbName)
		}
	}
}

func TestValidateVerbs(t *testing.T) {
	domain := NewDomain()

	// Test empty DSL
	err := domain.ValidateVerbs("")
	if err == nil {
		t.Error("Expected error for empty DSL")
	}

	// Test valid single verb
	validDSL := `(case.create (cbu.id "CBU-1234") (nature-purpose "Test case"))`
	err = domain.ValidateVerbs(validDSL)
	if err != nil {
		t.Errorf("Expected no error for valid DSL, got: %v", err)
	}

	// Test valid multi-line DSL
	multiLineDSL := `(case.create (cbu.id "CBU-1234") (nature-purpose "Test case"))
(products.add "CUSTODY" "FUND_ACCOUNTING")
(kyc.start (requirements (document "CertificateOfIncorporation")))`

	err = domain.ValidateVerbs(multiLineDSL)
	if err != nil {
		t.Errorf("Expected no error for valid multi-line DSL, got: %v", err)
	}

	// Test invalid verb
	invalidDSL := `(invalid.verb (param "value"))`
	err = domain.ValidateVerbs(invalidDSL)
	if err == nil {
		t.Error("Expected error for invalid verb")
	}
	if !strings.Contains(err.Error(), "invalid onboarding verb: invalid.verb") {
		t.Errorf("Expected specific error message, got: %v", err)
	}

	// Test mixed valid and invalid verbs
	mixedDSL := `(case.create (cbu.id "CBU-1234") (nature-purpose "Test"))
(invalid.action (param "value"))`

	err = domain.ValidateVerbs(mixedDSL)
	if err == nil {
		t.Error("Expected error for mixed valid/invalid DSL")
	}

	// Test comments and empty lines (should be ignored)
	dslWithComments := `; This is a comment
(case.create (cbu.id "CBU-1234") (nature-purpose "Test"))

; Another comment
(products.add "CUSTODY")`

	err = domain.ValidateVerbs(dslWithComments)
	if err != nil {
		t.Errorf("Expected no error for DSL with comments, got: %v", err)
	}
}

func TestValidateStateTransition(t *testing.T) {
	domain := NewDomain()

	// Test valid transitions
	validTransitions := [][2]string{
		{"CREATE", "PRODUCTS_ADDED"},
		{"PRODUCTS_ADDED", "KYC_STARTED"},
		{"KYC_STARTED", "SERVICES_DISCOVERED"},
		{"SERVICES_DISCOVERED", "RESOURCES_PLANNED"},
		{"RESOURCES_PLANNED", "ATTRIBUTES_BOUND"},
		{"ATTRIBUTES_BOUND", "WORKFLOW_ACTIVE"},
		{"WORKFLOW_ACTIVE", "COMPLETE"},
	}

	for _, transition := range validTransitions {
		from, to := transition[0], transition[1]
		err := domain.ValidateStateTransition(from, to)
		if err != nil {
			t.Errorf("Expected valid transition %s -> %s, got error: %v", from, to, err)
		}
	}

	// Test invalid transitions (skipping states)
	invalidTransitions := [][2]string{
		{"CREATE", "KYC_STARTED"},      // Skip PRODUCTS_ADDED
		{"PRODUCTS_ADDED", "COMPLETE"}, // Skip multiple states
		{"COMPLETE", "CREATE"},         // Backward transition
		{"KYC_STARTED", "CREATE"},      // Backward transition
	}

	for _, transition := range invalidTransitions {
		from, to := transition[0], transition[1]
		err := domain.ValidateStateTransition(from, to)
		if err == nil {
			t.Errorf("Expected invalid transition %s -> %s to fail", from, to)
		}
	}

	// Test invalid states
	err := domain.ValidateStateTransition("INVALID_STATE", "CREATE")
	if err == nil {
		t.Error("Expected error for invalid from state")
	}

	err = domain.ValidateStateTransition("CREATE", "INVALID_STATE")
	if err == nil {
		t.Error("Expected error for invalid to state")
	}
}

func TestGenerateDSL(t *testing.T) {
	domain := NewDomain()
	ctx := context.Background()

	testCases := []struct {
		instruction      string
		expectedContains string
		shouldFail       bool
	}{
		{
			instruction:      "create case CBU-1234",
			expectedContains: "(case.create",
			shouldFail:       false,
		},
		{
			instruction:      "add products CUSTODY and FUND_ACCOUNTING",
			expectedContains: "(products.add",
			shouldFail:       false,
		},
		{
			instruction:      "start kyc process",
			expectedContains: "(kyc.start",
			shouldFail:       false,
		},
		{
			instruction:      "discover services for custody",
			expectedContains: "(services.discover",
			shouldFail:       false,
		},
		{
			instruction:      "plan resources",
			expectedContains: "(resources.plan",
			shouldFail:       false,
		},
		{
			instruction:      "bind attributes",
			expectedContains: "(values.bind",
			shouldFail:       false,
		},
		{
			instruction:      "workflow transition",
			expectedContains: "(workflow.transition",
			shouldFail:       false,
		},
		{
			instruction: "unsupported random instruction",
			shouldFail:  true,
		},
	}

	for _, tc := range testCases {
		req := &registry.GenerationRequest{
			Instruction: tc.instruction,
		}

		resp, err := domain.GenerateDSL(ctx, req)

		if tc.shouldFail {
			if err == nil {
				t.Errorf("Expected error for instruction '%s'", tc.instruction)
			}
			continue
		}

		if err != nil {
			t.Errorf("Unexpected error for instruction '%s': %v", tc.instruction, err)
			continue
		}

		if resp == nil {
			t.Errorf("Expected response for instruction '%s'", tc.instruction)
			continue
		}

		if !strings.Contains(resp.DSL, tc.expectedContains) {
			t.Errorf("Expected DSL to contain '%s' for instruction '%s', got: %s",
				tc.expectedContains, tc.instruction, resp.DSL)
		}

		if resp.Verb == "" {
			t.Error("Expected verb to be set in response")
		}

		if resp.Parameters == nil {
			t.Error("Expected parameters to be set in response")
		}

		// Verify generated DSL is valid
		err = domain.ValidateVerbs(resp.DSL)
		if err != nil {
			t.Errorf("Generated DSL failed validation: %v", err)
		}
	}

	// Test empty request
	_, err := domain.GenerateDSL(ctx, nil)
	if err == nil {
		t.Error("Expected error for nil request")
	}

	_, err = domain.GenerateDSL(ctx, &registry.GenerationRequest{})
	if err == nil {
		t.Error("Expected error for empty instruction")
	}
}

func TestGetCurrentState(t *testing.T) {
	domain := NewDomain()

	// Test nil context
	state, err := domain.GetCurrentState(nil)
	if err != nil {
		t.Errorf("Unexpected error for nil context: %v", err)
	}
	if state != "CREATE" {
		t.Errorf("Expected initial state 'CREATE' for nil context, got '%s'", state)
	}

	// Test explicit state in context
	context := map[string]interface{}{
		"current_state": "KYC_STARTED",
	}
	state, err = domain.GetCurrentState(context)
	if err != nil {
		t.Errorf("Unexpected error for explicit state: %v", err)
	}
	if state != "KYC_STARTED" {
		t.Errorf("Expected 'KYC_STARTED', got '%s'", state)
	}

	// Test invalid state in context
	context = map[string]interface{}{
		"current_state": "INVALID_STATE",
	}
	_, err = domain.GetCurrentState(context)
	if err == nil {
		t.Error("Expected error for invalid state")
	}

	// Test state inference from context keys
	inferenceTests := []struct {
		context       map[string]interface{}
		expectedState string
	}{
		{
			context:       map[string]interface{}{"cbu_id": "CBU-1234"},
			expectedState: "CREATE",
		},
		{
			context: map[string]interface{}{
				"cbu_id":   "CBU-1234",
				"products": true,
			},
			expectedState: "PRODUCTS_ADDED",
		},
		{
			context: map[string]interface{}{
				"cbu_id":      "CBU-1234",
				"products":    true,
				"kyc_started": true,
			},
			expectedState: "KYC_STARTED",
		},
		{
			context: map[string]interface{}{
				"cbu_id":              "CBU-1234",
				"products":            true,
				"kyc_started":         true,
				"services_discovered": true,
			},
			expectedState: "SERVICES_DISCOVERED",
		},
		{
			context: map[string]interface{}{
				"cbu_id":              "CBU-1234",
				"products":            true,
				"kyc_started":         true,
				"services_discovered": true,
				"resources_planned":   true,
			},
			expectedState: "RESOURCES_PLANNED",
		},
		{
			context: map[string]interface{}{
				"cbu_id":              "CBU-1234",
				"products":            true,
				"kyc_started":         true,
				"services_discovered": true,
				"resources_planned":   true,
				"attributes_bound":    true,
			},
			expectedState: "ATTRIBUTES_BOUND",
		},
		{
			context: map[string]interface{}{
				"cbu_id":              "CBU-1234",
				"products":            true,
				"kyc_started":         true,
				"services_discovered": true,
				"resources_planned":   true,
				"attributes_bound":    true,
				"workflow_active":     true,
			},
			expectedState: "WORKFLOW_ACTIVE",
		},
	}

	for _, tt := range inferenceTests {
		state, err := domain.GetCurrentState(tt.context)
		if err != nil {
			t.Errorf("Unexpected error for context %v: %v", tt.context, err)
		}
		if state != tt.expectedState {
			t.Errorf("Expected state '%s' for context %v, got '%s'",
				tt.expectedState, tt.context, state)
		}
	}
}

func TestExtractContext(t *testing.T) {
	domain := NewDomain()

	// Test empty DSL
	context, err := domain.ExtractContext("")
	if err != nil {
		t.Errorf("Unexpected error for empty DSL: %v", err)
	}
	if len(context) != 0 {
		t.Errorf("Expected empty context for empty DSL, got %v", context)
	}

	// Test basic case creation DSL
	dsl := `(case.create (cbu.id "CBU-1234") (nature-purpose "Test fund"))`
	context, err = domain.ExtractContext(dsl)
	if err != nil {
		t.Errorf("Unexpected error: %v", err)
	}

	expectedCbuID := "CBU-1234"
	if context["cbu_id"] != expectedCbuID {
		t.Errorf("Expected cbu_id '%s', got '%v'", expectedCbuID, context["cbu_id"])
	}

	expectedNature := "Test fund"
	if context["nature_purpose"] != expectedNature {
		t.Errorf("Expected nature_purpose '%s', got '%v'", expectedNature, context["nature_purpose"])
	}

	if context["current_state"] != "CREATE" {
		t.Errorf("Expected current_state 'CREATE', got '%v'", context["current_state"])
	}

	// Test multi-step DSL
	multiStepDSL := `(case.create (cbu.id "CBU-1234") (nature-purpose "Test fund"))
(products.add "CUSTODY" "FUND_ACCOUNTING")
(kyc.start (requirements (document "CertificateOfIncorporation")))
(services.discover (for.product "CUSTODY"))
(resources.plan (resource.create "Account"))
(values.bind (bind (attr-id "attr-1") (value "test")))
(workflow.transition (from "ATTRIBUTES_BOUND") (to "WORKFLOW_ACTIVE"))`

	context, err = domain.ExtractContext(multiStepDSL)
	if err != nil {
		t.Errorf("Unexpected error for multi-step DSL: %v", err)
	}

	// Should detect the highest state based on DSL content
	if context["current_state"] != "WORKFLOW_ACTIVE" {
		t.Errorf("Expected current_state 'WORKFLOW_ACTIVE' for multi-step DSL, got '%v'", context["current_state"])
	}

	// Should have all the flags set
	expectedFlags := []string{
		"products", "kyc_started", "services_discovered",
		"resources_planned", "attributes_bound", "workflow_active",
	}

	for _, flag := range expectedFlags {
		if context[flag] != true {
			t.Errorf("Expected flag '%s' to be true", flag)
		}
	}

	// Test case completion
	completeDSL := `(case.create (cbu.id "CBU-1234") (nature-purpose "Test fund"))
(case.close (reason "Completed") (final-state "ACTIVE"))`

	context, err = domain.ExtractContext(completeDSL)
	if err != nil {
		t.Errorf("Unexpected error for complete DSL: %v", err)
	}

	if context["current_state"] != "COMPLETE" {
		t.Errorf("Expected current_state 'COMPLETE' for case.close DSL, got '%v'", context["current_state"])
	}
}

func TestStateTransitionVerbs(t *testing.T) {
	domain := NewDomain()
	vocab := domain.GetVocabulary()

	// Test verbs that should have state transitions
	stateTransitionVerbs := map[string]string{
		"case.create":         "CREATE",
		"products.add":        "PRODUCTS_ADDED",
		"kyc.start":           "KYC_STARTED",
		"services.discover":   "SERVICES_DISCOVERED",
		"resources.plan":      "RESOURCES_PLANNED",
		"values.bind":         "ATTRIBUTES_BOUND",
		"workflow.transition": "WORKFLOW_ACTIVE",
		"case.close":          "COMPLETE",
	}

	for verbName, expectedToState := range stateTransitionVerbs {
		verb, exists := vocab.Verbs[verbName]
		if !exists {
			t.Errorf("State transition verb '%s' not found", verbName)
			continue
		}

		if verb.StateTransition == nil {
			t.Errorf("Verb '%s' missing state transition", verbName)
			continue
		}

		if verb.StateTransition.ToState != expectedToState {
			t.Errorf("Verb '%s' expected ToState '%s', got '%s'",
				verbName, expectedToState, verb.StateTransition.ToState)
		}
	}
}

func TestArgumentSpecifications(t *testing.T) {
	domain := NewDomain()
	vocab := domain.GetVocabulary()

	// Test that key verbs have proper argument specifications
	keyVerbs := []string{
		"case.create", "products.add", "kyc.start", "entity.register",
		"resources.plan", "values.bind", "workflow.transition",
	}

	for _, verbName := range keyVerbs {
		verb, exists := vocab.Verbs[verbName]
		if !exists {
			t.Errorf("Key verb '%s' not found", verbName)
			continue
		}

		if len(verb.Arguments) == 0 {
			t.Errorf("Verb '%s' has no argument specifications", verbName)
		}

		// Check that arguments have proper specifications
		for argName, argSpec := range verb.Arguments {
			if argSpec.Name == "" {
				t.Errorf("Verb '%s' argument '%s' has empty name", verbName, argName)
			}

			if argSpec.Type == "" {
				t.Errorf("Verb '%s' argument '%s' has empty type", verbName, argName)
			}

			if argSpec.Description == "" {
				t.Errorf("Verb '%s' argument '%s' has empty description", verbName, argName)
			}
		}
	}

	// Test specific argument types
	caseCreateVerb := vocab.Verbs["case.create"]
	if caseCreateVerb != nil {
		cbuArg := caseCreateVerb.Arguments["cbu.id"]
		if cbuArg == nil {
			t.Error("case.create missing cbu.id argument")
		} else {
			if !cbuArg.Required {
				t.Error("case.create cbu.id argument should be required")
			}
			if cbuArg.Type != registry.ArgumentTypeString {
				t.Error("case.create cbu.id should be string type")
			}
			if cbuArg.Pattern == "" {
				t.Error("case.create cbu.id should have pattern validation")
			}
		}
	}
}

func TestDomainMetrics(t *testing.T) {
	domain := NewDomain()
	metrics := domain.GetMetrics()

	if metrics == nil {
		t.Fatal("GetMetrics() returned nil")
	}

	if metrics.TotalVerbs != 54 {
		t.Errorf("Expected 54 total verbs, got %d", metrics.TotalVerbs)
	}

	if metrics.ActiveVerbs != 54 {
		t.Errorf("Expected 54 active verbs, got %d", metrics.ActiveVerbs)
	}

	if metrics.UnusedVerbs != 0 {
		t.Errorf("Expected 0 unused verbs, got %d", metrics.UnusedVerbs)
	}

	if !metrics.IsHealthy {
		t.Error("Expected domain to be healthy")
	}

	if metrics.Version != "1.0.0" {
		t.Errorf("Expected version '1.0.0', got '%s'", metrics.Version)
	}

	if metrics.MemoryUsageBytes <= 0 {
		t.Error("Expected positive memory usage")
	}
}

func TestVerbExamples(t *testing.T) {
	domain := NewDomain()
	vocab := domain.GetVocabulary()

	// Every verb should have at least one example
	for verbName, verb := range vocab.Verbs {
		if len(verb.Examples) == 0 {
			t.Errorf("Verb '%s' has no examples", verbName)
			continue
		}

		// Each example should be valid DSL
		for i, example := range verb.Examples {
			err := domain.ValidateVerbs(example)
			if err != nil {
				t.Errorf("Verb '%s' example %d is invalid: %v", verbName, i, err)
			}

			// Example should contain the verb name
			if !strings.Contains(example, verbName) {
				t.Errorf("Verb '%s' example %d doesn't contain verb name: %s", verbName, i, example)
			}
		}
	}
}

func TestCompleteOnboardingWorkflow(t *testing.T) {
	domain := NewDomain()

	// Simulate a complete onboarding workflow
	workflowSteps := []struct {
		dsl           string
		expectedState string
	}{
		{
			dsl:           `(case.create (cbu.id "CBU-WORKFLOW-TEST") (nature-purpose "Complete workflow test"))`,
			expectedState: "CREATE",
		},
		{
			dsl: `(case.create (cbu.id "CBU-WORKFLOW-TEST") (nature-purpose "Complete workflow test"))
(products.add "CUSTODY" "FUND_ACCOUNTING")`,
			expectedState: "PRODUCTS_ADDED",
		},
		{
			dsl: `(case.create (cbu.id "CBU-WORKFLOW-TEST") (nature-purpose "Complete workflow test"))
(products.add "CUSTODY" "FUND_ACCOUNTING")
(kyc.start (requirements (document "CertificateOfIncorporation")))`,
			expectedState: "KYC_STARTED",
		},
		{
			dsl: `(case.create (cbu.id "CBU-WORKFLOW-TEST") (nature-purpose "Complete workflow test"))
(products.add "CUSTODY" "FUND_ACCOUNTING")
(kyc.start (requirements (document "CertificateOfIncorporation")))
(services.discover (for.product "CUSTODY"))`,
			expectedState: "SERVICES_DISCOVERED",
		},
		{
			dsl: `(case.create (cbu.id "CBU-WORKFLOW-TEST") (nature-purpose "Complete workflow test"))
(products.add "CUSTODY" "FUND_ACCOUNTING")
(kyc.start (requirements (document "CertificateOfIncorporation")))
(services.discover (for.product "CUSTODY"))
(resources.plan (resource.create "CustodyAccount" (owner "CustodyTech")))`,
			expectedState: "RESOURCES_PLANNED",
		},
		{
			dsl: `(case.create (cbu.id "CBU-WORKFLOW-TEST") (nature-purpose "Complete workflow test"))
(products.add "CUSTODY" "FUND_ACCOUNTING")
(kyc.start (requirements (document "CertificateOfIncorporation")))
(services.discover (for.product "CUSTODY"))
(resources.plan (resource.create "CustodyAccount" (owner "CustodyTech")))
(values.bind (bind (attr-id "attr-test") (value "TEST-VALUE")))`,
			expectedState: "ATTRIBUTES_BOUND",
		},
		{
			dsl: `(case.create (cbu.id "CBU-WORKFLOW-TEST") (nature-purpose "Complete workflow test"))
(products.add "CUSTODY" "FUND_ACCOUNTING")
(kyc.start (requirements (document "CertificateOfIncorporation")))
(services.discover (for.product "CUSTODY"))
(resources.plan (resource.create "CustodyAccount" (owner "CustodyTech")))
(values.bind (bind (attr-id "attr-test") (value "TEST-VALUE")))
(workflow.transition (from "ATTRIBUTES_BOUND") (to "WORKFLOW_ACTIVE"))`,
			expectedState: "WORKFLOW_ACTIVE",
		},
		{
			dsl: `(case.create (cbu.id "CBU-WORKFLOW-TEST") (nature-purpose "Complete workflow test"))
(products.add "CUSTODY" "FUND_ACCOUNTING")
(kyc.start (requirements (document "CertificateOfIncorporation")))
(services.discover (for.product "CUSTODY"))
(resources.plan (resource.create "CustodyAccount" (owner "CustodyTech")))
(values.bind (bind (attr-id "attr-test") (value "TEST-VALUE")))
(workflow.transition (from "ATTRIBUTES_BOUND") (to "WORKFLOW_ACTIVE"))
(case.close (reason "Workflow test completed successfully") (final-state "ACTIVE"))`,
			expectedState: "COMPLETE",
		},
	}

	for i, step := range workflowSteps {
		// Validate DSL is correct
		err := domain.ValidateVerbs(step.dsl)
		if err != nil {
			t.Errorf("Step %d DSL validation failed: %v", i+1, err)
			continue
		}

		// Extract context and verify state
		context, err := domain.ExtractContext(step.dsl)
		if err != nil {
			t.Errorf("Step %d context extraction failed: %v", i+1, err)
			continue
		}

		if context["current_state"] != step.expectedState {
			t.Errorf("Step %d expected state '%s', got '%v'",
				i+1, step.expectedState, context["current_state"])
		}
	}
}

func TestVerbCountByCategory(t *testing.T) {
	domain := NewDomain()
	vocab := domain.GetVocabulary()

	// Verify exact verb counts per category match our specification
	expectedCounts := map[string]int{
		"case-management":            5,
		"entity-identity":            5,
		"product-service":            5,
		"kyc-compliance":             6,
		"resource-infrastructure":    5,
		"attribute-data":             5,
		"workflow-state":             5,
		"notification-communication": 4,
		"integration-external":       4,
		"temporal-scheduling":        3,
		"risk-monitoring":            3,
		"data-lifecycle":             4,
		"offboarding":                1, // case.close is in both case-management and offboarding
	}

	totalExpectedVerbs := 0
	for _, count := range expectedCounts {
		totalExpectedVerbs += count
	}

	// Subtract 1 because case.close is counted in both case-management and offboarding
	// but should only be counted once in the total
	totalExpectedVerbs -= 1 // case.close is in both categories

	if totalExpectedVerbs != 54 {
		t.Errorf("Expected total calculation should equal 54, got %d", totalExpectedVerbs)
	}

	// Check each category
	for categoryName, expectedCount := range expectedCounts {
		category, exists := vocab.Categories[categoryName]
		if !exists {
			t.Errorf("Category '%s' not found", categoryName)
			continue
		}

		actualCount := len(category.Verbs)
		if actualCount != expectedCount {
			t.Errorf("Category '%s': expected %d verbs, got %d",
				categoryName, expectedCount, actualCount)

			// Debug: show which verbs are in this category
			t.Logf("  Verbs in '%s': %v", categoryName, category.Verbs)
		}
	}

	// Verify total unique verb count is 54
	if len(vocab.Verbs) != 54 {
		t.Errorf("Expected exactly 54 unique verbs, got %d", len(vocab.Verbs))
	}
}

func TestEnumArgumentValidation(t *testing.T) {
	domain := NewDomain()
	vocab := domain.GetVocabulary()

	// Test verbs with enum arguments have proper enum values
	verbsWithEnums := map[string]map[string][]string{
		"entity.register": {
			"type": {"PROPER_PERSON", "CORPORATE", "FUND", "TRUST", "PARTNERSHIP"},
		},
		"entity.classify": {
			"risk-level": {"LOW", "MEDIUM", "HIGH", "PROHIBITED"},
		},
		"entity.link": {
			"relationship": {"PARENT", "SUBSIDIARY", "AFFILIATE", "BENEFICIAL_OWNER", "SIGNATORY"},
		},
		"kyc.collect": {
			"type": {"CERTIFICATE_OF_INCORPORATION", "PASSPORT", "UTILITY_BILL", "BANK_STATEMENT", "W8BEN", "W8BEN_E"},
		},
		"compliance.screen": {
			"list": {"SANCTIONS", "PEP", "ADVERSE_MEDIA", "WORLDCHECK"},
		},
	}

	for verbName, expectedEnums := range verbsWithEnums {
		verb, exists := vocab.Verbs[verbName]
		if !exists {
			t.Errorf("Verb with enums '%s' not found", verbName)
			continue
		}

		for argName, expectedValues := range expectedEnums {
			arg, exists := verb.Arguments[argName]
			if !exists {
				t.Errorf("Verb '%s' missing enum argument '%s'", verbName, argName)
				continue
			}

			if arg.Type != registry.ArgumentTypeEnum {
				t.Errorf("Verb '%s' argument '%s' should be enum type", verbName, argName)
				continue
			}

			if len(arg.EnumValues) != len(expectedValues) {
				t.Errorf("Verb '%s' argument '%s': expected %d enum values, got %d",
					verbName, argName, len(expectedValues), len(arg.EnumValues))
			}

			// Check all expected values are present
			for _, expectedValue := range expectedValues {
				found := false
				for _, actualValue := range arg.EnumValues {
					if actualValue == expectedValue {
						found = true
						break
					}
				}
				if !found {
					t.Errorf("Verb '%s' argument '%s' missing enum value '%s'",
						verbName, argName, expectedValue)
				}
			}
		}
	}
}

func TestIdempotentVerbs(t *testing.T) {
	domain := NewDomain()
	vocab := domain.GetVocabulary()

	// case.create should be idempotent (can be called multiple times safely)
	caseCreateVerb, exists := vocab.Verbs["case.create"]
	if !exists {
		t.Fatal("case.create verb not found")
	}

	if !caseCreateVerb.Idempotent {
		t.Error("case.create should be marked as idempotent")
	}
}

func TestVerbTimestamps(t *testing.T) {
	domain := NewDomain()
	vocab := domain.GetVocabulary()

	// All verbs should have proper timestamps
	for verbName, verb := range vocab.Verbs {
		if verb.CreatedAt.IsZero() {
			t.Errorf("Verb '%s' has zero CreatedAt timestamp", verbName)
		}

		if verb.UpdatedAt.IsZero() {
			t.Errorf("Verb '%s' has zero UpdatedAt timestamp", verbName)
		}

		// UpdatedAt should be >= CreatedAt
		if verb.UpdatedAt.Before(verb.CreatedAt) {
			t.Errorf("Verb '%s' UpdatedAt is before CreatedAt", verbName)
		}
	}

	// Vocabulary should have timestamps
	if vocab.CreatedAt.IsZero() {
		t.Error("Vocabulary has zero CreatedAt timestamp")
	}

	if vocab.UpdatedAt.IsZero() {
		t.Error("Vocabulary has zero UpdatedAt timestamp")
	}
}
