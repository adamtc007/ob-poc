package resolver

import (
	"strings"
	"testing"
)

// =============================================================================
// Basic Resolution Tests
// =============================================================================

func TestNewResolver(t *testing.T) {
	resolver := NewResolver()
	if resolver == nil {
		t.Fatal("NewResolver returned nil")
	}

	if resolver.placeholderPattern == nil {
		t.Fatal("placeholderPattern not initialized")
	}
}

func TestResolve_NoPlaceholders(t *testing.T) {
	resolver := NewResolver()
	dsl := `(case.create (cbu.id "CBU-1234"))`

	result, err := resolver.Resolve(dsl, map[string]interface{}{})
	if err != nil {
		t.Fatalf("Resolve failed: %v", err)
	}

	if result != dsl {
		t.Errorf("Expected unchanged DSL, got: %s", result)
	}
}

func TestResolve_SinglePlaceholder(t *testing.T) {
	resolver := NewResolver()
	dsl := `(investor.start <investor_id>)`

	context := map[string]interface{}{
		"investor_id": "uuid-investor-123",
	}

	result, err := resolver.Resolve(dsl, context)
	if err != nil {
		t.Fatalf("Resolve failed: %v", err)
	}

	expected := `(investor.start "uuid-investor-123")`
	if result != expected {
		t.Errorf("Expected %s, got %s", expected, result)
	}
}

func TestResolve_MultiplePlaceholders(t *testing.T) {
	resolver := NewResolver()
	dsl := `(subscription.submit
  (investor <investor_id>)
  (fund <fund_id>)
  (class <class_id>)
)`

	context := map[string]interface{}{
		"investor_id": "uuid-investor",
		"fund_id":     "uuid-fund",
		"class_id":    "uuid-class",
	}

	result, err := resolver.Resolve(dsl, context)
	if err != nil {
		t.Fatalf("Resolve failed: %v", err)
	}

	if !strings.Contains(result, `"uuid-investor"`) {
		t.Error("investor_id not resolved")
	}
	if !strings.Contains(result, `"uuid-fund"`) {
		t.Error("fund_id not resolved")
	}
	if !strings.Contains(result, `"uuid-class"`) {
		t.Error("class_id not resolved")
	}
}

func TestResolve_MissingContext(t *testing.T) {
	resolver := NewResolver()
	dsl := `(investor.start <investor_id>)`

	context := map[string]interface{}{} // Empty context

	_, err := resolver.Resolve(dsl, context)
	if err == nil {
		t.Error("Expected error for missing context value")
	}

	if !strings.Contains(err.Error(), "unresolved placeholders") {
		t.Errorf("Expected 'unresolved placeholders' error, got: %v", err)
	}
}

func TestResolve_PartialContext(t *testing.T) {
	resolver := NewResolver()
	dsl := `(subscription.submit
  (investor <investor_id>)
  (fund <fund_id>)
)`

	context := map[string]interface{}{
		"investor_id": "uuid-investor",
		// fund_id missing
	}

	_, err := resolver.Resolve(dsl, context)
	if err == nil {
		t.Error("Expected error for missing fund_id")
	}

	if !strings.Contains(err.Error(), "fund_id") {
		t.Error("Error should mention missing fund_id")
	}
}

// =============================================================================
// Onboarding DSL Resolution Tests
// =============================================================================

func TestResolve_OnboardingCBUID(t *testing.T) {
	resolver := NewResolver()
	dsl := `(case.create (cbu.id <cbu_id>))`

	context := map[string]interface{}{
		"cbu_id": "CBU-1234",
	}

	result, err := resolver.Resolve(dsl, context)
	if err != nil {
		t.Fatalf("Resolve failed: %v", err)
	}

	if !strings.Contains(result, `"CBU-1234"`) {
		t.Errorf("cbu_id not resolved correctly: %s", result)
	}
}

func TestResolve_OnboardingMultipleAttributes(t *testing.T) {
	resolver := NewResolver()
	dsl := `(case.create
  (cbu.id <cbu_id>)
  (entity.name <entity_name>)
  (jurisdiction <jurisdiction>)
)`

	context := map[string]interface{}{
		"cbu_id":       "CBU-5678",
		"entity_name":  "Acme Fund",
		"jurisdiction": "LU",
	}

	result, err := resolver.Resolve(dsl, context)
	if err != nil {
		t.Fatalf("Resolve failed: %v", err)
	}

	if !strings.Contains(result, `"CBU-5678"`) {
		t.Error("cbu_id not resolved")
	}
	if !strings.Contains(result, `"Acme Fund"`) {
		t.Error("entity_name not resolved")
	}
	if !strings.Contains(result, `"LU"`) {
		t.Error("jurisdiction not resolved")
	}
}

// =============================================================================
// Hedge Fund DSL Resolution Tests
// =============================================================================

func TestResolve_HedgeFundInvestorID(t *testing.T) {
	resolver := NewResolver()
	dsl := `(kyc.begin (investor <investor_id>))`

	context := map[string]interface{}{
		"investor_id": "uuid-hf-investor-123",
	}

	result, err := resolver.Resolve(dsl, context)
	if err != nil {
		t.Fatalf("Resolve failed: %v", err)
	}

	if !strings.Contains(result, `"uuid-hf-investor-123"`) {
		t.Errorf("investor_id not resolved: %s", result)
	}
}

func TestResolve_HedgeFundFullWorkflow(t *testing.T) {
	resolver := NewResolver()
	dsl := `(subscription.submit
  (investor <investor_id>)
  (fund <fund_id>)
  (class <class_id>)
  (amount <amount>)
  (currency <currency>)
)`

	context := map[string]interface{}{
		"investor_id": "uuid-investor-hf",
		"fund_id":     "uuid-fund-hf",
		"class_id":    "uuid-class-a",
		"amount":      1000000.00,
		"currency":    "USD",
	}

	result, err := resolver.Resolve(dsl, context)
	if err != nil {
		t.Fatalf("Resolve failed: %v", err)
	}

	if !strings.Contains(result, `"uuid-investor-hf"`) {
		t.Error("investor_id not resolved")
	}
	if !strings.Contains(result, `"uuid-fund-hf"`) {
		t.Error("fund_id not resolved")
	}
	if !strings.Contains(result, `"uuid-class-a"`) {
		t.Error("class_id not resolved")
	}
	if !strings.Contains(result, `1000000.00`) {
		t.Error("amount not resolved")
	}
	if !strings.Contains(result, `"USD"`) {
		t.Error("currency not resolved")
	}
}

// =============================================================================
// Value Type Tests
// =============================================================================

func TestResolve_StringValue(t *testing.T) {
	resolver := NewResolver()
	dsl := `(test <value>)`

	context := map[string]interface{}{
		"value": "test-string",
	}

	result, err := resolver.Resolve(dsl, context)
	if err != nil {
		t.Fatalf("Resolve failed: %v", err)
	}

	if !strings.Contains(result, `"test-string"`) {
		t.Errorf("String value not quoted correctly: %s", result)
	}
}

func TestResolve_IntegerValue(t *testing.T) {
	resolver := NewResolver()
	dsl := `(test <value>)`

	context := map[string]interface{}{
		"value": 12345,
	}

	result, err := resolver.Resolve(dsl, context)
	if err != nil {
		t.Fatalf("Resolve failed: %v", err)
	}

	if !strings.Contains(result, `12345`) {
		t.Errorf("Integer value not formatted correctly: %s", result)
	}

	// Integer should NOT be quoted
	if strings.Contains(result, `"12345"`) {
		t.Error("Integer should not be quoted")
	}
}

func TestResolve_FloatValue(t *testing.T) {
	resolver := NewResolver()
	dsl := `(test <value>)`

	context := map[string]interface{}{
		"value": 123.45,
	}

	result, err := resolver.Resolve(dsl, context)
	if err != nil {
		t.Fatalf("Resolve failed: %v", err)
	}

	if !strings.Contains(result, `123.45`) {
		t.Errorf("Float value not formatted correctly: %s", result)
	}
}

func TestResolve_BooleanValue(t *testing.T) {
	resolver := NewResolver()

	tests := []struct {
		name     string
		value    bool
		expected string
	}{
		{"true", true, "true"},
		{"false", false, "false"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			dsl := `(test <value>)`
			context := map[string]interface{}{
				"value": tt.value,
			}

			result, err := resolver.Resolve(dsl, context)
			if err != nil {
				t.Fatalf("Resolve failed: %v", err)
			}

			if !strings.Contains(result, tt.expected) {
				t.Errorf("Expected %s in result: %s", tt.expected, result)
			}
		})
	}
}

func TestResolve_AlreadyQuotedString(t *testing.T) {
	resolver := NewResolver()
	dsl := `(test <value>)`

	context := map[string]interface{}{
		"value": `"already-quoted"`,
	}

	result, err := resolver.Resolve(dsl, context)
	if err != nil {
		t.Fatalf("Resolve failed: %v", err)
	}

	// Should not double-quote
	if strings.Contains(result, `""already-quoted""`) {
		t.Error("String should not be double-quoted")
	}

	if !strings.Contains(result, `"already-quoted"`) {
		t.Error("Quoted string not preserved")
	}
}

// =============================================================================
// FindPlaceholders Tests
// =============================================================================

func TestFindPlaceholders_None(t *testing.T) {
	resolver := NewResolver()
	dsl := `(case.create (cbu.id "CBU-1234"))`

	placeholders := resolver.FindPlaceholders(dsl)
	if len(placeholders) != 0 {
		t.Errorf("Expected 0 placeholders, got %d: %v", len(placeholders), placeholders)
	}
}

func TestFindPlaceholders_Single(t *testing.T) {
	resolver := NewResolver()
	dsl := `(investor.start <investor_id>)`

	placeholders := resolver.FindPlaceholders(dsl)
	if len(placeholders) != 1 {
		t.Fatalf("Expected 1 placeholder, got %d", len(placeholders))
	}

	if placeholders[0] != "investor_id" {
		t.Errorf("Expected 'investor_id', got '%s'", placeholders[0])
	}
}

func TestFindPlaceholders_Multiple(t *testing.T) {
	resolver := NewResolver()
	dsl := `(subscription.submit
  (investor <investor_id>)
  (fund <fund_id>)
  (class <class_id>)
)`

	placeholders := resolver.FindPlaceholders(dsl)
	if len(placeholders) != 3 {
		t.Fatalf("Expected 3 placeholders, got %d", len(placeholders))
	}

	expected := map[string]bool{
		"investor_id": false,
		"fund_id":     false,
		"class_id":    false,
	}

	for _, p := range placeholders {
		if _, ok := expected[p]; ok {
			expected[p] = true
		} else {
			t.Errorf("Unexpected placeholder: %s", p)
		}
	}

	for k, found := range expected {
		if !found {
			t.Errorf("Expected placeholder not found: %s", k)
		}
	}
}

func TestFindPlaceholders_Duplicates(t *testing.T) {
	resolver := NewResolver()
	dsl := `(test <investor_id> <investor_id> <investor_id>)`

	placeholders := resolver.FindPlaceholders(dsl)
	if len(placeholders) != 1 {
		t.Errorf("Expected 1 unique placeholder, got %d: %v", len(placeholders), placeholders)
	}
}

func TestFindPlaceholders_MixedCase(t *testing.T) {
	resolver := NewResolver()
	dsl := `(test <InvestorID> <fund_id> <CBUID>)`

	placeholders := resolver.FindPlaceholders(dsl)
	if len(placeholders) != 3 {
		t.Fatalf("Expected 3 placeholders, got %d", len(placeholders))
	}

	// Should preserve case
	expected := []string{"InvestorID", "fund_id", "CBUID"}
	for i, exp := range expected {
		if placeholders[i] != exp {
			t.Errorf("Placeholder %d: expected '%s', got '%s'", i, exp, placeholders[i])
		}
	}
}

// =============================================================================
// HasUnresolvedPlaceholders Tests
// =============================================================================

func TestHasUnresolvedPlaceholders_True(t *testing.T) {
	resolver := NewResolver()
	dsl := `(investor.start <investor_id>)`

	if !resolver.HasUnresolvedPlaceholders(dsl) {
		t.Error("Expected HasUnresolvedPlaceholders to return true")
	}
}

func TestHasUnresolvedPlaceholders_False(t *testing.T) {
	resolver := NewResolver()
	dsl := `(investor.start "uuid-123")`

	if resolver.HasUnresolvedPlaceholders(dsl) {
		t.Error("Expected HasUnresolvedPlaceholders to return false")
	}
}

// =============================================================================
// ValidatePlaceholders Tests
// =============================================================================

func TestValidatePlaceholders_AllPresent(t *testing.T) {
	resolver := NewResolver()
	dsl := `(subscription.submit
  (investor <investor_id>)
  (fund <fund_id>)
)`

	context := map[string]interface{}{
		"investor_id": "uuid-investor",
		"fund_id":     "uuid-fund",
	}

	err := resolver.ValidatePlaceholders(dsl, context)
	if err != nil {
		t.Errorf("ValidatePlaceholders failed: %v", err)
	}
}

func TestValidatePlaceholders_Missing(t *testing.T) {
	resolver := NewResolver()
	dsl := `(subscription.submit
  (investor <investor_id>)
  (fund <fund_id>)
)`

	context := map[string]interface{}{
		"investor_id": "uuid-investor",
		// fund_id missing
	}

	err := resolver.ValidatePlaceholders(dsl, context)
	if err == nil {
		t.Error("Expected validation error for missing placeholder")
	}

	if !strings.Contains(err.Error(), "missing context values") {
		t.Errorf("Expected 'missing context values' error, got: %v", err)
	}

	if !strings.Contains(err.Error(), "fund_id") {
		t.Error("Error should mention missing fund_id")
	}
}

func TestValidatePlaceholders_NoPlaceholders(t *testing.T) {
	resolver := NewResolver()
	dsl := `(case.create (cbu.id "CBU-1234"))`

	context := map[string]interface{}{}

	err := resolver.ValidatePlaceholders(dsl, context)
	if err != nil {
		t.Errorf("ValidatePlaceholders should succeed with no placeholders: %v", err)
	}
}

// =============================================================================
// Alternate Forms Tests
// =============================================================================

func TestResolve_AlternateForms_CamelCase(t *testing.T) {
	resolver := NewResolver()
	dsl := `(investor.start <investor_id>)`

	// Context uses camelCase
	context := map[string]interface{}{
		"investorId": "uuid-camel-case",
	}

	result, err := resolver.Resolve(dsl, context)
	if err != nil {
		t.Fatalf("Resolve with camelCase failed: %v", err)
	}

	if !strings.Contains(result, `"uuid-camel-case"`) {
		t.Error("Failed to resolve using camelCase alternate form")
	}
}

func TestResolve_AlternateForms_Lowercase(t *testing.T) {
	resolver := NewResolver()
	dsl := `(investor.start <INVESTOR_ID>)`

	// Context uses lowercase
	context := map[string]interface{}{
		"investor_id": "uuid-lowercase",
	}

	result, err := resolver.Resolve(dsl, context)
	if err != nil {
		t.Fatalf("Resolve with lowercase failed: %v", err)
	}

	if !strings.Contains(result, `"uuid-lowercase"`) {
		t.Error("Failed to resolve using lowercase alternate form")
	}
}

func TestResolve_AlternateForms_Hyphens(t *testing.T) {
	resolver := NewResolver()
	dsl := `(investor.start <investor_id>)`

	// Context uses hyphens
	context := map[string]interface{}{
		"investor-id": "uuid-hyphens",
	}

	result, err := resolver.Resolve(dsl, context)
	if err != nil {
		t.Fatalf("Resolve with hyphens failed: %v", err)
	}

	if !strings.Contains(result, `"uuid-hyphens"`) {
		t.Error("Failed to resolve using hyphen alternate form")
	}
}

// =============================================================================
// ResolveWithDefaults Tests
// =============================================================================

func TestResolveWithDefaults_UseContext(t *testing.T) {
	resolver := NewResolver()
	dsl := `(test <value>)`

	context := map[string]interface{}{
		"value": "from-context",
	}

	defaults := map[string]string{
		"value": "from-defaults",
	}

	result := resolver.ResolveWithDefaults(dsl, context, defaults)

	// Should use context value, not default
	if !strings.Contains(result, `"from-context"`) {
		t.Error("Should use context value over default")
	}

	if strings.Contains(result, `"from-defaults"`) {
		t.Error("Should not use default when context has value")
	}
}

func TestResolveWithDefaults_UseDefaults(t *testing.T) {
	resolver := NewResolver()
	dsl := `(test <value>)`

	context := map[string]interface{}{
		// value not in context
	}

	defaults := map[string]string{
		"value": "from-defaults",
	}

	result := resolver.ResolveWithDefaults(dsl, context, defaults)

	// Should use default value
	if !strings.Contains(result, `"from-defaults"`) {
		t.Errorf("Should use default value: %s", result)
	}
}

func TestResolveWithDefaults_NoDefault(t *testing.T) {
	resolver := NewResolver()
	dsl := `(test <value>)`

	context := map[string]interface{}{}
	defaults := map[string]string{}

	result := resolver.ResolveWithDefaults(dsl, context, defaults)

	// Placeholder should remain unresolved
	if !strings.Contains(result, `<value>`) {
		t.Error("Placeholder should remain when no context or default available")
	}
}

// =============================================================================
// ExtractContextFromDSL Tests
// =============================================================================

func TestExtractContextFromDSL_InvestorID(t *testing.T) {
	resolver := NewResolver()
	dsl := `(kyc.begin (investor "uuid-investor-123"))`

	patterns := GetCommonEntityPatterns()
	context := resolver.ExtractContextFromDSL(dsl, patterns)

	if val, ok := context["investor_id"]; !ok {
		t.Error("investor_id not extracted")
	} else if val != "uuid-investor-123" {
		t.Errorf("Expected 'uuid-investor-123', got '%v'", val)
	}
}

func TestExtractContextFromDSL_MultipleEntities(t *testing.T) {
	resolver := NewResolver()
	dsl := `(subscription.submit
  (investor "aaaa-bbbb-cccc-dddd-eeeeeeeeeeee")
  (fund "1111-2222-3333-4444-555555555555")
  (class "ffff-gggg-hhhh-iiii-jjjjjjjjjjjj")
)`

	patterns := GetCommonEntityPatterns()
	context := resolver.ExtractContextFromDSL(dsl, patterns)

	if len(context) != 3 {
		t.Errorf("Expected 3 extracted values, got %d", len(context))
	}

	if _, ok := context["investor_id"]; !ok {
		t.Error("investor_id not extracted")
	}
	if _, ok := context["fund_id"]; !ok {
		t.Error("fund_id not extracted")
	}
	if _, ok := context["class_id"]; !ok {
		t.Error("class_id not extracted")
	}
}

func TestExtractContextFromDSL_CBUID(t *testing.T) {
	resolver := NewResolver()
	dsl := `(case.create (cbu.id "CBU-1234"))`

	patterns := GetCommonEntityPatterns()
	context := resolver.ExtractContextFromDSL(dsl, patterns)

	if val, ok := context["cbu_id"]; !ok {
		t.Error("cbu_id not extracted")
	} else if val != "CBU-1234" {
		t.Errorf("Expected 'CBU-1234', got '%v'", val)
	}
}

// =============================================================================
// Edge Cases and Error Handling
// =============================================================================

func TestResolve_EmptyDSL(t *testing.T) {
	resolver := NewResolver()

	result, err := resolver.Resolve("", map[string]interface{}{})
	if err != nil {
		t.Errorf("Empty DSL should not error: %v", err)
	}

	if result != "" {
		t.Error("Empty DSL should return empty string")
	}
}

func TestResolve_InvalidPlaceholderFormat(t *testing.T) {
	resolver := NewResolver()
	dsl := `(test <invalid placeholder with spaces>)`

	placeholders := resolver.FindPlaceholders(dsl)

	// Should not match invalid placeholder format
	if len(placeholders) > 0 {
		t.Error("Invalid placeholder format should not be matched")
	}
}

func TestResolve_PlaceholderInString(t *testing.T) {
	resolver := NewResolver()
	dsl := `(test "this is <not_a_placeholder> in string")`

	placeholders := resolver.FindPlaceholders(dsl)

	// Should still find it (resolver doesn't parse strings)
	if len(placeholders) != 1 {
		t.Logf("Note: Resolver finds placeholders in strings - this is expected behavior")
	}
}

func TestResolve_NestedPlaceholders(t *testing.T) {
	resolver := NewResolver()
	dsl := `(outer
  (inner <value1>)
  (inner <value2>)
)`

	context := map[string]interface{}{
		"value1": "resolved1",
		"value2": "resolved2",
	}

	result, err := resolver.Resolve(dsl, context)
	if err != nil {
		t.Fatalf("Resolve failed: %v", err)
	}

	if !strings.Contains(result, `"resolved1"`) {
		t.Error("value1 not resolved in nested structure")
	}
	if !strings.Contains(result, `"resolved2"`) {
		t.Error("value2 not resolved in nested structure")
	}
}

func TestResolve_SamePlaceholderMultipleTimes(t *testing.T) {
	resolver := NewResolver()
	dsl := `(test
  (first <value>)
  (second <value>)
  (third <value>)
)`

	context := map[string]interface{}{
		"value": "same-value",
	}

	result, err := resolver.Resolve(dsl, context)
	if err != nil {
		t.Fatalf("Resolve failed: %v", err)
	}

	// Should replace all occurrences
	count := strings.Count(result, `"same-value"`)
	if count != 3 {
		t.Errorf("Expected placeholder to be replaced 3 times, got %d", count)
	}
}

// =============================================================================
// Cross-Domain Resolution Tests
// =============================================================================

func TestResolve_CrossDomain_OnboardingAndHedgeFund(t *testing.T) {
	resolver := NewResolver()
	dsl := `(case.create (cbu.id <cbu_id>))

(investor.start-opportunity
  (investor <investor_id>)
  (legal-name <legal_name>)
)

(subscription.submit
  (investor <investor_id>)
  (fund <fund_id>)
)`

	context := map[string]interface{}{
		"cbu_id":      "CBU-9999",
		"investor_id": "uuid-cross-domain",
		"legal_name":  "Cross Domain Corp",
		"fund_id":     "uuid-fund-xyz",
	}

	result, err := resolver.Resolve(dsl, context)
	if err != nil {
		t.Fatalf("Cross-domain resolve failed: %v", err)
	}

	if !strings.Contains(result, `"CBU-9999"`) {
		t.Error("onboarding cbu_id not resolved")
	}
	if strings.Count(result, `"uuid-cross-domain"`) != 2 {
		t.Error("investor_id should be resolved twice (used in both domains)")
	}
	if !strings.Contains(result, `"Cross Domain Corp"`) {
		t.Error("legal_name not resolved")
	}
	if !strings.Contains(result, `"uuid-fund-xyz"`) {
		t.Error("fund_id not resolved")
	}
}
