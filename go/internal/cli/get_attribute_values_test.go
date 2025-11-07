package cli

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"dsl-ob-poc/internal/datastore"
)

func TestGetAttributeValues_Integration(t *testing.T) {
	// Create a temporary directory for mock data
	tempDir := t.TempDir()
	mockDataDir := filepath.Join(tempDir, "mocks")
	if err := os.MkdirAll(mockDataDir, 0o755); err != nil {
		t.Fatalf("Failed to create mock data directory: %v", err)
	}

	// Create minimal mock data files required for the test
	createMockFile(t, mockDataDir, "cbus.json", `[
		{"cbu_id": "test-cbu", "name": "test-cbu", "description": "Test CBU", "nature_purpose": "Test fund domiciled in LU"}
	]`)
	createMockFile(t, mockDataDir, "roles.json", `[
		{"role_id": "test-role", "name": "Test Role", "description": "Test role"}
	]`)
	createMockFile(t, mockDataDir, "entity_types.json", `[]`)
	createMockFile(t, mockDataDir, "entities.json", `[]`)
	createMockFile(t, mockDataDir, "entity_limited_companies.json", `[]`)
	createMockFile(t, mockDataDir, "entity_partnerships.json", `[]`)
	createMockFile(t, mockDataDir, "entity_proper_persons.json", `[]`)
	createMockFile(t, mockDataDir, "cbu_entity_roles.json", `[]`)
	createMockFile(t, mockDataDir, "products.json", `[
		{"product_id": "test-product", "name": "Test Product", "description": "Test product"}
	]`)
	createMockFile(t, mockDataDir, "services.json", `[
		{"service_id": "test-service", "name": "Test Service", "description": "Test service"}
	]`)
	createMockFile(t, mockDataDir, "prod_resources.json", `[]`)
	createMockFile(t, mockDataDir, "product_services.json", `[]`)
	createMockFile(t, mockDataDir, "service_resources.json", `[]`)
	createMockFile(t, mockDataDir, "dictionary.json", `[
		{"attribute_id": "test-attr-id", "name": "test.attribute", "long_description": "Test attribute", "group_id": "test-group", "mask": "string", "domain": "test"}
	]`)
	createMockFile(t, mockDataDir, "attribute_values.json", `[]`)
	createMockFile(t, mockDataDir, "dsl_ob.json", `[
		{"version_id": "test-version-1", "cbu_id": "test-cbu", "dsl_text": "(case.create\n  (cbu.id \"test-cbu\")\n  (nature-purpose \"Test fund\")\n)\n\n(variables.declare\n  (var (attr-id \"test-attr-id\"))\n)", "created_at": "2024-01-01T00:00:00Z"}
	]`)

	// Create DataStore using mock configuration
	config := datastore.Config{
		Type:         datastore.MockStore,
		MockDataPath: mockDataDir,
	}

	ds, err := datastore.NewDataStore(config)
	if err != nil {
		t.Fatalf("Failed to create mock DataStore: %v", err)
	}
	defer ds.Close()

	// Test the get-attribute-values command
	ctx := context.Background()
	args := []string{"--cbu", "test-cbu"}

	err = RunGetAttributeValues(ctx, ds, args)
	if err != nil {
		t.Errorf("RunGetAttributeValues failed: %v", err)
	}
}

// createMockFile creates a mock JSON file with the given content
func createMockFile(t *testing.T, dir, filename, content string) {
	filePath := filepath.Join(dir, filename)
	if err := os.WriteFile(filePath, []byte(content), 0o644); err != nil {
		t.Fatalf("Failed to create mock file %s: %v", filename, err)
	}
}

func TestLooksLikeUUID(t *testing.T) {
	validUUID := "123e4567-e89b-12d3-a456-426614174000"
	invalidUUID := "not-a-uuid"

	if !looksLikeUUID(validUUID) {
		t.Errorf("Expected %s to be recognized as UUID", validUUID)
	}

	if looksLikeUUID(invalidUUID) {
		t.Errorf("Expected %s to NOT be recognized as UUID", invalidUUID)
	}
}
