package test

import (
	"context"
	"regexp"
	"testing"

	"dsl-ob-poc/internal/dictionary"
	"dsl-ob-poc/internal/dictionary/repository"
	"dsl-ob-poc/internal/domains/onboarding"
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
// Attribute Resolution Tests
// =============================================================================

func TestDomain_ResolveAttributeName(t *testing.T) {
	ctx := context.Background()
	mockRepo := newMockDictionaryRepository()
	domain := onboarding.NewDomainWithDictionary(mockRepo)

	// Add test attributes using real UUIDs from seed data
	testAttr := createTestAttribute(
		"456789ab-cdef-1234-5678-9abcdef01301",
		"custody.account_number",
		"Custody",
	)
	mockRepo.addAttribute(testAttr)

	tests := []struct {
		name     string
		attrID   string
		expected string
		wantErr  bool
	}{
		{
			name:     "Valid attribute ID",
			attrID:   "456789ab-cdef-1234-5678-9abcdef01301",
			expected: "custody.account_number",
			wantErr:  false,
		},
		{
			name:     "Non-existent attribute ID",
			attrID:   "non-existent-uuid",
			expected: "",
			wantErr:  true,
		},
		{
			name:     "Empty attribute ID",
			attrID:   "",
			expected: "",
			wantErr:  true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result, err := domain.ResolveAttributeName(ctx, tt.attrID)

			if tt.wantErr && err == nil {
				t.Errorf("Expected error but got none")
			}
			if !tt.wantErr && err != nil {
				t.Errorf("Unexpected error: %v", err)
			}
			if result != tt.expected {
				t.Errorf("Expected %s, got %s", tt.expected, result)
			}
		})
	}
}

func TestDomain_ResolveAttributesByIDs(t *testing.T) {
	ctx := context.Background()
	mockRepo := newMockDictionaryRepository()
	domain := onboarding.NewDomainWithDictionary(mockRepo)

	// Add test attributes using real UUIDs
	attr1 := createTestAttribute("456789ab-cdef-1234-5678-9abcdef01301", "custody.account_number", "Custody")
	attr2 := createTestAttribute("456789ab-cdef-1234-5678-9abcdef01401", "accounting.fund_code", "Accounting")
	mockRepo.addAttribute(attr1)
	mockRepo.addAttribute(attr2)

	attrIDs := []string{"456789ab-cdef-1234-5678-9abcdef01301", "456789ab-cdef-1234-5678-9abcdef01401", "non-existent"}
	result, err := domain.ResolveAttributesByIDs(ctx, attrIDs)

	if err != nil {
		t.Fatalf("Unexpected error: %v", err)
	}

	expected := map[string]string{
		"456789ab-cdef-1234-5678-9abcdef01301": "custody.account_number",
		"456789ab-cdef-1234-5678-9abcdef01401": "accounting.fund_code",
		"non-existent":                         "non-existent", // Should use ID as fallback
	}

	if len(result) != len(expected) {
		t.Errorf("Expected %d results, got %d", len(expected), len(result))
	}

	for id, expectedName := range expected {
		if actualName, exists := result[id]; !exists || actualName != expectedName {
			t.Errorf("Expected %s for ID %s, got %s", expectedName, id, actualName)
		}
	}
}

func TestDomain_ResolveAttributeName_NoDictionary(t *testing.T) {
	ctx := context.Background()
	domain := onboarding.NewDomain() // No dictionary repository

	_, err := domain.ResolveAttributeName(ctx, "some-uuid")
	if err == nil {
		t.Error("Expected error when no dictionary repository configured")
	}
}

// =============================================================================
// DSL Enhancement Tests
// =============================================================================

func TestDomain_EnhanceDSLWithAttributeNames(t *testing.T) {
	ctx := context.Background()
	mockRepo := newMockDictionaryRepository()
	domain := onboarding.NewDomainWithDictionary(mockRepo)

	// Add test attributes using real UUIDs
	attr1 := createTestAttribute(
		"456789ab-cdef-1234-5678-9abcdef01301",
		"custody.account_number",
		"Custody",
	)
	attr2 := createTestAttribute(
		"456789ab-cdef-1234-5678-9abcdef01401",
		"accounting.fund_code",
		"Accounting",
	)
	mockRepo.addAttribute(attr1)
	mockRepo.addAttribute(attr2)

	tests := []struct {
		name     string
		inputDSL string
		expected string
	}{
		{
			name:     "Single attribute without name",
			inputDSL: "(case.create @attr{456789ab-cdef-1234-5678-9abcdef01301})",
			expected: "(case.create @attr{456789ab-cdef-1234-5678-9abcdef01301:custody.account_number})",
		},
		{
			name:     "Multiple attributes without names",
			inputDSL: "(kyc.start @attr{456789ab-cdef-1234-5678-9abcdef01301} @attr{456789ab-cdef-1234-5678-9abcdef01401})",
			expected: "(kyc.start @attr{456789ab-cdef-1234-5678-9abcdef01301:custody.account_number} @attr{456789ab-cdef-1234-5678-9abcdef01401:accounting.fund_code})",
		},
		{
			name:     "Attribute already has name - should not change",
			inputDSL: "(case.create @attr{456789ab-cdef-1234-5678-9abcdef01301:existing.name})",
			expected: "(case.create @attr{456789ab-cdef-1234-5678-9abcdef01301:existing.name})",
		},
		{
			name:     "Mixed - some with names, some without",
			inputDSL: "(resources.plan @attr{456789ab-cdef-1234-5678-9abcdef01301} @attr{456789ab-cdef-1234-5678-9abcdef01401:existing.name})",
			expected: "(resources.plan @attr{456789ab-cdef-1234-5678-9abcdef01301:custody.account_number} @attr{456789ab-cdef-1234-5678-9abcdef01401:existing.name})",
		},
		{
			name:     "No attributes",
			inputDSL: "(case.create (cbu.id \"CBU-001\"))",
			expected: "(case.create (cbu.id \"CBU-001\"))",
		},
		{
			name:     "Non-existent attribute ID - should remain unchanged",
			inputDSL: "(case.create @attr{non-existent-uuid})",
			expected: "(case.create @attr{non-existent-uuid})",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result, err := domain.EnhanceDSLWithAttributeNames(ctx, tt.inputDSL)
			if err != nil {
				t.Errorf("Unexpected error: %v", err)
			}
			if result != tt.expected {
				t.Errorf("Expected:\n%s\nGot:\n%s", tt.expected, result)
			}
		})
	}
}

func TestDomain_EnhanceDSLWithAttributeNames_NoDictionary(t *testing.T) {
	ctx := context.Background()
	domain := onboarding.NewDomain() // No dictionary repository

	inputDSL := "(case.create @attr{some-uuid})"
	result, err := domain.EnhanceDSLWithAttributeNames(ctx, inputDSL)

	if err != nil {
		t.Errorf("Unexpected error: %v", err)
	}
	if result != inputDSL {
		t.Errorf("Expected original DSL to be returned unchanged, got: %s", result)
	}
}

// =============================================================================
// Attribute Reference Generation Tests
// =============================================================================

func TestDomain_GenerateAttributeReference(t *testing.T) {
	ctx := context.Background()
	mockRepo := newMockDictionaryRepository()
	domain := onboarding.NewDomainWithDictionary(mockRepo)

	// Add test attribute using real UUID
	testAttr := createTestAttribute(
		"456789ab-cdef-1234-5678-9abcdef01301",
		"custody.account_number",
		"Custody",
	)
	mockRepo.addAttribute(testAttr)

	tests := []struct {
		name     string
		attrID   string
		expected string
		wantErr  bool
	}{
		{
			name:     "Valid attribute ID with dictionary",
			attrID:   "456789ab-cdef-1234-5678-9abcdef01301",
			expected: "@attr{456789ab-cdef-1234-5678-9abcdef01301:custody.account_number}",
			wantErr:  false,
		},
		{
			name:     "Non-existent attribute ID - should fallback to UUID only",
			attrID:   "non-existent-uuid",
			expected: "@attr{non-existent-uuid}",
			wantErr:  false,
		},
		{
			name:     "Empty attribute ID",
			attrID:   "",
			expected: "",
			wantErr:  true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result, err := domain.GenerateAttributeReference(ctx, tt.attrID)

			if tt.wantErr && err == nil {
				t.Errorf("Expected error but got none")
			}
			if !tt.wantErr && err != nil {
				t.Errorf("Unexpected error: %v", err)
			}
			if result != tt.expected {
				t.Errorf("Expected %s, got %s", tt.expected, result)
			}
		})
	}
}

func TestDomain_GenerateAttributeReference_NoDictionary(t *testing.T) {
	ctx := context.Background()
	domain := onboarding.NewDomain() // No dictionary repository

	result, err := domain.GenerateAttributeReference(ctx, "some-uuid")
	if err != nil {
		t.Errorf("Unexpected error: %v", err)
	}

	expected := "@attr{some-uuid}"
	if result != expected {
		t.Errorf("Expected %s, got %s", expected, result)
	}
}

// =============================================================================
// Attribute Validation Tests
// =============================================================================

func TestDomain_ValidateAttributeReferences(t *testing.T) {
	ctx := context.Background()
	mockRepo := newMockDictionaryRepository()
	domain := onboarding.NewDomainWithDictionary(mockRepo)

	// Add valid test attribute using real UUID
	testAttr := createTestAttribute(
		"456789ab-cdef-1234-5678-9abcdef01301",
		"custody.account_number",
		"Custody",
	)
	mockRepo.addAttribute(testAttr)

	tests := []struct {
		name    string
		dsl     string
		wantErr bool
	}{
		{
			name:    "Valid attribute references",
			dsl:     "(case.create @attr{456789ab-cdef-1234-5678-9abcdef01301} @attr{456789ab-cdef-1234-5678-9abcdef01301:custody.account_number})",
			wantErr: false,
		},
		{
			name:    "Invalid attribute reference",
			dsl:     "(case.create @attr{aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee})",
			wantErr: true,
		},
		{
			name:    "No attribute references",
			dsl:     "(case.create (cbu.id \"CBU-001\"))",
			wantErr: false,
		},
		{
			name:    "Mixed valid and invalid",
			dsl:     "(case.create @attr{456789ab-cdef-1234-5678-9abcdef01301} @attr{aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee})",
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := domain.ValidateAttributeReferences(ctx, tt.dsl)

			if tt.wantErr && err == nil {
				t.Errorf("Expected validation error but got none")
			}
			if !tt.wantErr && err != nil {
				t.Errorf("Unexpected validation error: %v", err)
			}
		})
	}
}

func TestDomain_ValidateAttributeReferences_NoDictionary(t *testing.T) {
	ctx := context.Background()
	domain := onboarding.NewDomain() // No dictionary repository

	// Should not error when no dictionary is configured
	err := domain.ValidateAttributeReferences(ctx, "(case.create @attr{some-uuid})")
	if err != nil {
		t.Errorf("Expected no error when dictionary not configured, got: %v", err)
	}
}

// =============================================================================
// Integration Tests - Complete Workflow
// =============================================================================

func TestDomain_AttributeWorkflowIntegration(t *testing.T) {
	ctx := context.Background()
	mockRepo := newMockDictionaryRepository()
	domain := onboarding.NewDomainWithDictionary(mockRepo)

	// Setup test attributes using real UUIDs
	attributes := []*dictionary.Attribute{
		createTestAttribute("456789ab-cdef-1234-5678-9abcdef01301", "custody.account_number", "Custody"),
		createTestAttribute("456789ab-cdef-1234-5678-9abcdef01401", "accounting.fund_code", "Accounting"),
		createTestAttribute("fedcba98-7654-3210-fedc-ba9876543203", "fund.base_currency", "Fund"),
	}

	for _, attr := range attributes {
		mockRepo.addAttribute(attr)
	}

	// Test complete workflow
	t.Run("Complete attribute workflow", func(t *testing.T) {
		// 1. Generate attribute references
		ref1, err := domain.GenerateAttributeReference(ctx, "456789ab-cdef-1234-5678-9abcdef01301")
		if err != nil {
			t.Fatalf("Failed to generate reference: %v", err)
		}
		expected1 := "@attr{456789ab-cdef-1234-5678-9abcdef01301:custody.account_number}"
		if ref1 != expected1 {
			t.Errorf("Expected %s, got %s", expected1, ref1)
		}

		// 2. Create DSL with generated references
		originalDSL := "(resources.plan (resource.create \"CustodyAccount\" " + ref1 + "))"

		// 3. Validate attribute references in DSL
		if err := domain.ValidateAttributeReferences(ctx, originalDSL); err != nil {
			t.Errorf("Validation failed: %v", err)
		}

		// 4. Enhance DSL (should be no-op since references already have names)
		enhancedDSL, err := domain.EnhanceDSLWithAttributeNames(ctx, originalDSL)
		if err != nil {
			t.Errorf("Enhancement failed: %v", err)
		}
		if enhancedDSL != originalDSL {
			t.Errorf("DSL should remain unchanged after enhancement")
		}

		// 5. Test with UUID-only DSL
		uuidOnlyDSL := "(resources.plan (resource.create \"CustodyAccount\" @attr{456789ab-cdef-1234-5678-9abcdef01401}))"
		enhancedUUIDDSL, err := domain.EnhanceDSLWithAttributeNames(ctx, uuidOnlyDSL)
		if err != nil {
			t.Errorf("Enhancement failed: %v", err)
		}

		expectedEnhanced := "(resources.plan (resource.create \"CustodyAccount\" @attr{456789ab-cdef-1234-5678-9abcdef01401:accounting.fund_code}))"
		if enhancedUUIDDSL != expectedEnhanced {
			t.Errorf("Expected:\n%s\nGot:\n%s", expectedEnhanced, enhancedUUIDDSL)
		}
	})
}

// =============================================================================
// Debug Test - Understanding Validation Issue
// =============================================================================

func TestDomain_ValidateAttributeReferences_Debug(t *testing.T) {
	ctx := context.Background()
	mockRepo := newMockDictionaryRepository()
	domain := onboarding.NewDomainWithDictionary(mockRepo)

	// Add valid test attribute using real UUID
	testAttr := createTestAttribute(
		"456789ab-cdef-1234-5678-9abcdef01301",
		"custody.account_number",
		"Custody",
	)
	mockRepo.addAttribute(testAttr)

	// Test invalid reference manually - use a UUID format that will match regex but not exist in repo
	invalidDSL := "(case.create @attr{aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee})"
	t.Logf("Testing DSL: %s", invalidDSL)

	// Let's manually check what the regex matches
	attrPattern := regexp.MustCompile(`@attr\{([a-fA-F0-9-]{8,36})(?::([^}]+))?\}`)
	matches := attrPattern.FindAllStringSubmatch(invalidDSL, -1)

	t.Logf("Found %d matches", len(matches))
	for i, match := range matches {
		t.Logf("Match %d: full=%s, uuid=%s", i, match[0], match[1])
	}

	err := domain.ValidateAttributeReferences(ctx, invalidDSL)
	if err != nil {
		t.Logf("Validation error (expected): %v", err)
	} else {
		t.Logf("No validation error - this is the problem!")
	}

	// Test valid reference
	validDSL := "(case.create @attr{456789ab-cdef-1234-5678-9abcdef01301})"
	t.Logf("Testing valid DSL: %s", validDSL)

	validMatches := attrPattern.FindAllStringSubmatch(validDSL, -1)
	t.Logf("Valid DSL found %d matches", len(validMatches))
	for i, match := range validMatches {
		t.Logf("Valid Match %d: full=%s, uuid=%s", i, match[0], match[1])
	}

	validErr := domain.ValidateAttributeReferences(ctx, validDSL)
	if validErr != nil {
		t.Logf("Valid validation error (unexpected): %v", validErr)
	} else {
		t.Logf("No validation error for valid DSL (expected)")
	}
}
