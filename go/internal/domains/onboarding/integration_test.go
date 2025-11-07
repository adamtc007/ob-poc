package onboarding

import (
	"context"
	"strings"
	"testing"

	"dsl-ob-poc/internal/dictionary"
	"dsl-ob-poc/internal/dictionary/repository"
	registry "dsl-ob-poc/internal/domain-registry"
)

// mockDictionaryRepository implements repository.DictionaryRepository for testing
type mockDictionaryRepository struct {
	attributes map[string]*dictionary.Attribute
}

func newMockDictionaryRepository() *mockDictionaryRepository {
	return &mockDictionaryRepository{
		attributes: make(map[string]*dictionary.Attribute),
	}
}

func (m *mockDictionaryRepository) addAttribute(attr *dictionary.Attribute) {
	m.attributes[attr.AttributeID] = attr
}

func (m *mockDictionaryRepository) Create(ctx context.Context, attribute *dictionary.Attribute) error {
	m.attributes[attribute.AttributeID] = attribute
	return nil
}

func (m *mockDictionaryRepository) GetByID(ctx context.Context, attributeID string) (*dictionary.Attribute, error) {
	if attr, exists := m.attributes[attributeID]; exists {
		return attr, nil
	}
	return nil, repository.ErrAttributeNotFound
}

func (m *mockDictionaryRepository) GetByName(ctx context.Context, name string) (*dictionary.Attribute, error) {
	for _, attr := range m.attributes {
		if attr.Name == name {
			return attr, nil
		}
	}
	return nil, repository.ErrAttributeNotFound
}

func (m *mockDictionaryRepository) Update(ctx context.Context, attribute *dictionary.Attribute) error {
	m.attributes[attribute.AttributeID] = attribute
	return nil
}

func (m *mockDictionaryRepository) Delete(ctx context.Context, attributeID string) error {
	delete(m.attributes, attributeID)
	return nil
}

func (m *mockDictionaryRepository) List(ctx context.Context, opts *repository.ListOptions) ([]dictionary.Attribute, error) {
	var result []dictionary.Attribute
	for _, attr := range m.attributes {
		result = append(result, *attr)
	}
	return result, nil
}

func (m *mockDictionaryRepository) Count(ctx context.Context, opts *repository.ListOptions) (int, error) {
	return len(m.attributes), nil
}

func (m *mockDictionaryRepository) Close() error {
	return nil
}

// Helper to create test attributes
func createTestAttribute(id, name, domain string) *dictionary.Attribute {
	return &dictionary.Attribute{
		AttributeID:     id,
		Name:            name,
		LongDescription: "Test attribute for " + name,
		GroupID:         "test",
		Mask:            "string",
		Domain:          domain,
		Source: dictionary.SourceMetadata{
			Primary: "user_input",
		},
		Sink: dictionary.SinkMetadata{
			Primary: "database",
		},
	}
}

// =============================================================================
// Integration Tests - Full AttributeID-as-Type Workflow
// =============================================================================

func TestDSLGenerationWithRealAttributes(t *testing.T) {
	ctx := context.Background()

	// Setup mock repository with real attributes from seed data
	mockRepo := newMockDictionaryRepository()

	// Add core onboarding attributes
	attributes := []*dictionary.Attribute{
		createTestAttribute(AttrCustodyAccountNumber, "custody.account_number", "Custody"),
		createTestAttribute(AttrCustodyAccountType, "custody.account_type", "Custody"),
		createTestAttribute(AttrAccountingFundCode, "accounting.fund_code", "Accounting"),
		createTestAttribute(AttrFundBaseCurrency, "fund.base_currency", "Fund"),
		createTestAttribute(AttrTAFundIdentifier, "transfer_agency.fund_identifier", "TransferAgency"),
		createTestAttribute(AttrTAShareClass, "transfer_agency.share_class", "TransferAgency"),
		createTestAttribute(AttrOnboardNaturePurpose, "onboard.nature_purpose", "Onboarding"),
	}

	for _, attr := range attributes {
		mockRepo.addAttribute(attr)
	}

	// Create domain with dictionary integration
	domain := NewDomainWithDictionary(mockRepo)

	t.Run("Resource Planning DSL Generation", func(t *testing.T) {
		req := &registry.GenerationRequest{
			Instruction: "plan resources for custody and accounting systems",
			Context:     map[string]interface{}{},
		}

		resp, err := domain.GenerateDSL(ctx, req)
		if err != nil {
			t.Fatalf("DSL generation failed: %v", err)
		}

		t.Logf("Generated DSL:\n%s", resp.DSL)

		// Verify the DSL contains @attr{uuid:name} references
		expectedAttributes := []string{
			AttrCustodyAccountNumber + ":custody.account_number",
			AttrCustodyAccountType + ":custody.account_type",
			AttrAccountingFundCode + ":accounting.fund_code",
			AttrFundBaseCurrency + ":fund.base_currency",
			AttrTAFundIdentifier + ":transfer_agency.fund_identifier",
			AttrTAShareClass + ":transfer_agency.share_class",
		}

		for _, expectedAttr := range expectedAttributes {
			if !strings.Contains(resp.DSL, "@attr{"+expectedAttr+"}") {
				t.Errorf("Generated DSL should contain @attr{%s}", expectedAttr)
			}
		}

		// Verify DSL structure
		if !strings.Contains(resp.DSL, "resources.plan") {
			t.Error("Generated DSL should contain resources.plan verb")
		}

		if !strings.Contains(resp.DSL, "resource.create") {
			t.Error("Generated DSL should contain resource.create verb")
		}
	})

	t.Run("Values Binding DSL Generation", func(t *testing.T) {
		req := &registry.GenerationRequest{
			Instruction: "bind attributes for custody account and fund code",
			Context:     map[string]interface{}{},
		}

		resp, err := domain.GenerateDSL(ctx, req)
		if err != nil {
			t.Fatalf("DSL generation failed: %v", err)
		}

		t.Logf("Generated Values Binding DSL:\n%s", resp.DSL)

		// Verify the DSL contains attribute bindings
		expectedBindings := []string{
			AttrCustodyAccountNumber + ":custody.account_number",
			AttrAccountingFundCode + ":accounting.fund_code",
		}

		for _, expectedBinding := range expectedBindings {
			if !strings.Contains(resp.DSL, "@attr{"+expectedBinding+"}") {
				t.Errorf("Generated DSL should contain @attr{%s}", expectedBinding)
			}
		}

		// Verify DSL structure
		if !strings.Contains(resp.DSL, "values.bind") {
			t.Error("Generated DSL should contain values.bind verb")
		}

		// Verify actual values are bound
		if !strings.Contains(resp.DSL, "\"CUST-EGOF-001\"") {
			t.Error("Generated DSL should contain custody account value")
		}

		if !strings.Contains(resp.DSL, "\"FA-EGOF-LU-001\"") {
			t.Error("Generated DSL should contain fund accounting code value")
		}
	})
}

func TestAttributeResolutionIntegration(t *testing.T) {
	ctx := context.Background()

	// Setup mock repository
	mockRepo := newMockDictionaryRepository()
	mockRepo.addAttribute(createTestAttribute(AttrCustodyAccountNumber, "custody.account_number", "Custody"))
	mockRepo.addAttribute(createTestAttribute(AttrAccountingFundCode, "accounting.fund_code", "Accounting"))

	domain := NewDomainWithDictionary(mockRepo)

	t.Run("Generate Attribute Reference", func(t *testing.T) {
		ref, err := domain.GenerateAttributeReference(ctx, AttrCustodyAccountNumber)
		if err != nil {
			t.Fatalf("Failed to generate attribute reference: %v", err)
		}

		expected := "@attr{" + AttrCustodyAccountNumber + ":custody.account_number}"
		if ref != expected {
			t.Errorf("Expected %s, got %s", expected, ref)
		}

		t.Logf("Generated attribute reference: %s", ref)
	})

	t.Run("Enhance DSL with Attribute Names", func(t *testing.T) {
		// Start with UUID-only DSL
		uuidOnlyDSL := "(resources.plan @attr{" + AttrCustodyAccountNumber + "} @attr{" + AttrAccountingFundCode + "})"
		t.Logf("Original DSL: %s", uuidOnlyDSL)

		// Enhance with human-readable names
		enhancedDSL, err := domain.EnhanceDSLWithAttributeNames(ctx, uuidOnlyDSL)
		if err != nil {
			t.Fatalf("Failed to enhance DSL: %v", err)
		}

		t.Logf("Enhanced DSL: %s", enhancedDSL)

		// Verify enhancements
		expectedEnhancements := []string{
			AttrCustodyAccountNumber + ":custody.account_number",
			AttrAccountingFundCode + ":accounting.fund_code",
		}

		for _, enhancement := range expectedEnhancements {
			if !strings.Contains(enhancedDSL, "@attr{"+enhancement+"}") {
				t.Errorf("Enhanced DSL should contain @attr{%s}", enhancement)
			}
		}
	})

	t.Run("Validate Attribute References", func(t *testing.T) {
		validDSL := "(case.create @attr{" + AttrCustodyAccountNumber + ":custody.account_number})"
		err := domain.ValidateAttributeReferences(ctx, validDSL)
		if err != nil {
			t.Errorf("Valid DSL should pass validation: %v", err)
		}

		invalidDSL := "(case.create @attr{aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee:nonexistent.attr})"
		err = domain.ValidateAttributeReferences(ctx, invalidDSL)
		if err == nil {
			t.Error("Invalid DSL should fail validation")
		} else {
			t.Logf("Invalid DSL correctly rejected: %v", err)
		}
	})
}

func TestVocabularyExamplesWithRealAttributes(t *testing.T) {
	// Create domain with dictionary (no repo needed for this test)
	domain := NewDomain()
	vocab := domain.GetVocabulary()

	t.Run("Verify Vocabulary Examples Use Real UUIDs", func(t *testing.T) {
		// Check resources.plan examples
		resourcesPlanVerb := vocab.Verbs["resources.plan"]
		if resourcesPlanVerb == nil {
			t.Fatal("resources.plan verb not found")
		}

		if len(resourcesPlanVerb.Examples) == 0 {
			t.Fatal("resources.plan verb has no examples")
		}

		example := resourcesPlanVerb.Examples[0]
		t.Logf("resources.plan example: %s", example)

		// Verify it contains real attribute UUIDs
		if !strings.Contains(example, AttrCustodyAccountNumber) {
			t.Errorf("Example should contain real custody account number UUID: %s", AttrCustodyAccountNumber)
		}

		if !strings.Contains(example, ":custody.account_number") {
			t.Error("Example should contain human-readable attribute name")
		}
	})

	t.Run("Verify Values Bind Examples", func(t *testing.T) {
		valuesBindVerb := vocab.Verbs["values.bind"]
		if valuesBindVerb == nil {
			t.Fatal("values.bind verb not found")
		}

		if len(valuesBindVerb.Examples) == 0 {
			t.Fatal("values.bind verb has no examples")
		}

		example := valuesBindVerb.Examples[0]
		t.Logf("values.bind example: %s", example)

		// Verify it contains real attribute UUIDs
		expectedUUIDs := []string{AttrCustodyAccountNumber, AttrAccountingFundCode}
		for _, uuid := range expectedUUIDs {
			if !strings.Contains(example, uuid) {
				t.Errorf("Example should contain real UUID: %s", uuid)
			}
		}
	})
}

func TestFullWorkflowIntegration(t *testing.T) {
	ctx := context.Background()

	// Setup complete mock repository
	mockRepo := newMockDictionaryRepository()

	// Add all core attributes
	coreAttributes := []*dictionary.Attribute{
		createTestAttribute(AttrOnboardCBUID, "onboard.cbu_id", "Onboarding"),
		createTestAttribute(AttrOnboardNaturePurpose, "onboard.nature_purpose", "Onboarding"),
		createTestAttribute(AttrCustodyAccountNumber, "custody.account_number", "Custody"),
		createTestAttribute(AttrCustodyAccountType, "custody.account_type", "Custody"),
		createTestAttribute(AttrAccountingFundCode, "accounting.fund_code", "Accounting"),
		createTestAttribute(AttrFundBaseCurrency, "fund.base_currency", "Fund"),
		createTestAttribute(AttrTAFundIdentifier, "transfer_agency.fund_identifier", "TransferAgency"),
		createTestAttribute(AttrTAShareClass, "transfer_agency.share_class", "TransferAgency"),
	}

	for _, attr := range coreAttributes {
		mockRepo.addAttribute(attr)
	}

	domain := NewDomainWithDictionary(mockRepo)

	t.Run("Complete Onboarding Workflow", func(t *testing.T) {
		// Step 1: Generate resource planning DSL
		planReq := &registry.GenerationRequest{
			Instruction: "plan resources for comprehensive onboarding",
			Context:     map[string]interface{}{},
		}

		planResp, err := domain.GenerateDSL(ctx, planReq)
		if err != nil {
			t.Fatalf("Resource planning DSL generation failed: %v", err)
		}

		t.Logf("=== Step 1: Resource Planning ===\n%s\n", planResp.DSL)

		// Step 2: Generate attribute binding DSL
		bindReq := &registry.GenerationRequest{
			Instruction: "bind attributes with real values",
			Context:     map[string]interface{}{},
		}

		bindResp, err := domain.GenerateDSL(ctx, bindReq)
		if err != nil {
			t.Fatalf("Attribute binding DSL generation failed: %v", err)
		}

		t.Logf("=== Step 2: Attribute Binding ===\n%s\n", bindResp.DSL)

		// Step 3: Validate both DSL documents
		if err := domain.ValidateAttributeReferences(ctx, planResp.DSL); err != nil {
			t.Errorf("Resource planning DSL validation failed: %v", err)
		}

		if err := domain.ValidateAttributeReferences(ctx, bindResp.DSL); err != nil {
			t.Errorf("Attribute binding DSL validation failed: %v", err)
		}

		// Step 4: Demonstrate DSL enhancement (UUID-only to UUID:name)
		uuidOnlyDSL := "(test.verb @attr{" + AttrCustodyAccountNumber + "} @attr{" + AttrAccountingFundCode + "})"
		enhanced, err := domain.EnhanceDSLWithAttributeNames(ctx, uuidOnlyDSL)
		if err != nil {
			t.Errorf("DSL enhancement failed: %v", err)
		}

		t.Logf("=== Step 3: DSL Enhancement ===")
		t.Logf("Before: %s", uuidOnlyDSL)
		t.Logf("After:  %s\n", enhanced)

		// Verify the workflow demonstrates the key features
		features := []struct {
			name        string
			dsl         string
			shouldHave  []string
			description string
		}{
			{
				name: "AttributeID-as-Type Pattern",
				dsl:  planResp.DSL,
				shouldHave: []string{
					"@attr{" + AttrCustodyAccountNumber,
					":custody.account_number",
				},
				description: "DSL uses @attr{uuid:name} syntax for semantic typing",
			},
			{
				name: "Human Readability",
				dsl:  enhanced,
				shouldHave: []string{
					":custody.account_number",
					":accounting.fund_code",
				},
				description: "Enhanced DSL includes human-readable attribute names",
			},
			{
				name: "Real Database UUIDs",
				dsl:  bindResp.DSL,
				shouldHave: []string{
					AttrCustodyAccountNumber,
					AttrAccountingFundCode,
				},
				description: "DSL uses actual UUIDs from dictionary seed data",
			},
		}

		for _, feature := range features {
			t.Run(feature.name, func(t *testing.T) {
				for _, expected := range feature.shouldHave {
					if !strings.Contains(feature.dsl, expected) {
						t.Errorf("%s: DSL should contain '%s'", feature.description, expected)
					}
				}
			})
		}
	})
}
