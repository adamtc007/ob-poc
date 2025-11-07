package runtime

import (
	"testing"
)

// TestBasicStructCreation verifies that core runtime structures can be created
func TestBasicStructCreation(t *testing.T) {
	// Test ActionDefinition creation
	action := &ActionDefinition{
		ActionName:  "test-action",
		VerbPattern: "resources.create",
		ActionType:  ActionTypeHTTPAPI,
		Active:      true,
		Version:     1,
		Environment: "test",
	}

	if action.ActionName != "test-action" {
		t.Errorf("Expected ActionName to be 'test-action', got %s", action.ActionName)
	}

	if action.VerbPattern != "resources.create" {
		t.Errorf("Expected VerbPattern to be 'resources.create', got %s", action.VerbPattern)
	}

	if action.ActionType != ActionTypeHTTPAPI {
		t.Errorf("Expected ActionType to be ActionTypeHTTPAPI, got %s", action.ActionType)
	}
}

// TestResourceType verifies ResourceType structure
func TestResourceType(t *testing.T) {
	resourceType := &ResourceType{
		ResourceTypeName: "CustodyAccount",
		Description:      "A custody account resource",
		Active:           true,
		Version:          1,
		Environment:      "development",
	}

	if resourceType.ResourceTypeName != "CustodyAccount" {
		t.Errorf("Expected ResourceTypeName to be 'CustodyAccount', got %s", resourceType.ResourceTypeName)
	}

	if !resourceType.Active {
		t.Error("Expected ResourceType to be active")
	}
}

// TestActionExecution verifies ActionExecution structure
func TestActionExecution(t *testing.T) {
	execution := &ActionExecution{
		ActionID:        "test-action-id",
		CBUID:           "CBU-1234",
		DSLVersionID:    "version-1",
		ExecutionStatus: ExecutionStatusPending,
		RetryCount:      0,
	}

	if execution.ActionID != "test-action-id" {
		t.Errorf("Expected ActionID to be 'test-action-id', got %s", execution.ActionID)
	}

	if execution.ExecutionStatus != ExecutionStatusPending {
		t.Errorf("Expected ExecutionStatus to be PENDING, got %s", execution.ExecutionStatus)
	}
}

// TestExecutionRequest verifies ExecutionRequest structure
func TestExecutionRequest(t *testing.T) {
	req := &ExecutionRequest{
		ActionID:     "action-123",
		CBUID:        "CBU-5678",
		DSLVersionID: "v2",
		Environment:  "production",
	}

	if req.ActionID != "action-123" {
		t.Errorf("Expected ActionID to be 'action-123', got %s", req.ActionID)
	}

	if req.Environment != "production" {
		t.Errorf("Expected Environment to be 'production', got %s", req.Environment)
	}
}

// TestAttributeTransformer verifies AttributeTransformer creation
func TestAttributeTransformer(t *testing.T) {
	transformer := NewAttributeTransformer()
	if transformer == nil {
		t.Error("Expected NewAttributeTransformer to return a non-nil transformer")
	}
}

// TestCredentialInfo verifies CredentialInfo structure
func TestCredentialInfo(t *testing.T) {
	credInfo := &CredentialInfo{
		Name:        "test-api-key",
		Type:        "api_key",
		Environment: "development",
		Active:      true,
	}

	if credInfo.Name != "test-api-key" {
		t.Errorf("Expected Name to be 'test-api-key', got %s", credInfo.Name)
	}

	if credInfo.Type != "api_key" {
		t.Errorf("Expected Type to be 'api_key', got %s", credInfo.Type)
	}

	if !credInfo.Active {
		t.Error("Expected credential to be active")
	}
}

// TestAPIRequest verifies APIRequest structure
func TestAPIRequest(t *testing.T) {
	apiReq := &APIRequest{
		Method:         "POST",
		URL:            "https://api.example.com/create",
		Headers:        make(map[string]string),
		Body:           make(map[string]interface{}),
		TimeoutSeconds: 30,
	}

	if apiReq.Method != "POST" {
		t.Errorf("Expected Method to be 'POST', got %s", apiReq.Method)
	}

	if apiReq.URL != "https://api.example.com/create" {
		t.Errorf("Expected URL to be 'https://api.example.com/create', got %s", apiReq.URL)
	}

	if apiReq.TimeoutSeconds != 30 {
		t.Errorf("Expected TimeoutSeconds to be 30, got %d", apiReq.TimeoutSeconds)
	}
}

// TestAPIResponse verifies APIResponse structure
func TestAPIResponse(t *testing.T) {
	apiResp := &APIResponse{
		StatusCode: 201,
		Headers:    make(map[string]string),
		Body:       make(map[string]interface{}),
		RawBody:    `{"id":"123","status":"created"}`,
		DurationMS: 250,
	}

	if apiResp.StatusCode != 201 {
		t.Errorf("Expected StatusCode to be 201, got %d", apiResp.StatusCode)
	}

	if apiResp.DurationMS != 250 {
		t.Errorf("Expected DurationMS to be 250, got %d", apiResp.DurationMS)
	}

	if apiResp.RawBody == "" {
		t.Error("Expected RawBody to be non-empty")
	}
}