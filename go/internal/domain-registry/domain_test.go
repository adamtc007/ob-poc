package registry

import (
	"context"
	"fmt"
	"strings"
	"testing"
	"time"
)

// MockDomain is a test implementation of the Domain interface
type MockDomain struct {
	name         string
	version      string
	description  string
	vocabulary   *Vocabulary
	healthy      bool
	states       []string
	initialState string
	metrics      *DomainMetrics
}

// NewMockDomain creates a new mock domain for testing
func NewMockDomain(name, version string) *MockDomain {
	vocab := &Vocabulary{
		Domain:      name,
		Version:     version,
		Description: fmt.Sprintf("Mock domain for %s", name),
		Verbs:       make(map[string]*VerbDefinition),
		Categories:  make(map[string]*VerbCategory),
		States:      []string{"INITIAL", "ACTIVE", "COMPLETE"},
		CreatedAt:   time.Now(),
		UpdatedAt:   time.Now(),
	}

	// Add some mock verbs
	verb1 := &VerbDefinition{
		Name:        fmt.Sprintf("%s.start", name),
		Category:    "lifecycle",
		Version:     "1.0.0",
		Description: "Start the workflow",
		Arguments: map[string]*ArgumentSpec{
			"entity_id": {
				Name:        "entity_id",
				Type:        ArgumentTypeUUID,
				Required:    true,
				Description: "Entity identifier",
			},
		},
		StateTransition: &StateTransition{
			FromStates: []string{"INITIAL"},
			ToState:    "ACTIVE",
		},
		Idempotent: true,
		Examples:   []string{fmt.Sprintf("(%s.start \"uuid-123\")", name)},
		CreatedAt:  time.Now(),
		UpdatedAt:  time.Now(),
	}

	verb2 := &VerbDefinition{
		Name:        fmt.Sprintf("%s.complete", name),
		Category:    "lifecycle",
		Version:     "1.0.0",
		Description: "Complete the workflow",
		Arguments: map[string]*ArgumentSpec{
			"entity_id": {
				Name:        "entity_id",
				Type:        ArgumentTypeUUID,
				Required:    true,
				Description: "Entity identifier",
			},
		},
		StateTransition: &StateTransition{
			FromStates: []string{"ACTIVE"},
			ToState:    "COMPLETE",
		},
		Idempotent: false,
		Examples:   []string{fmt.Sprintf("(%s.complete \"uuid-123\")", name)},
		CreatedAt:  time.Now(),
		UpdatedAt:  time.Now(),
	}

	vocab.Verbs[verb1.Name] = verb1
	vocab.Verbs[verb2.Name] = verb2

	// Add category
	category := &VerbCategory{
		Name:        "lifecycle",
		Description: "Lifecycle management verbs",
		Verbs:       []string{verb1.Name, verb2.Name},
	}
	vocab.Categories["lifecycle"] = category

	return &MockDomain{
		name:         name,
		version:      version,
		description:  fmt.Sprintf("Mock %s domain for testing", name),
		vocabulary:   vocab,
		healthy:      true,
		states:       []string{"INITIAL", "ACTIVE", "COMPLETE"},
		initialState: "INITIAL",
		metrics: &DomainMetrics{
			TotalRequests:      0,
			SuccessfulRequests: 0,
			FailedRequests:     0,
			TotalVerbs:         2,
			ActiveVerbs:        2,
			UnusedVerbs:        0,
			StateTransitions:   make(map[string]int64),
			CurrentStates:      make(map[string]int64),
			ValidationErrors:   make(map[string]int64),
			GenerationErrors:   make(map[string]int64),
			IsHealthy:          true,
			LastHealthCheck:    time.Now(),
			UptimeSeconds:      0,
			MemoryUsageBytes:   1024 * 1024, // 1MB
			CollectedAt:        time.Now(),
			Version:            version,
		},
	}
}

// Domain interface implementation
func (m *MockDomain) Name() string               { return m.name }
func (m *MockDomain) Version() string            { return m.version }
func (m *MockDomain) Description() string        { return m.description }
func (m *MockDomain) GetVocabulary() *Vocabulary { return m.vocabulary }
func (m *MockDomain) IsHealthy() bool            { return m.healthy }
func (m *MockDomain) GetValidStates() []string   { return m.states }
func (m *MockDomain) GetInitialState() string    { return m.initialState }
func (m *MockDomain) GetMetrics() *DomainMetrics { return m.metrics }

func (m *MockDomain) ValidateVerbs(dsl string) error {
	// Simple mock validation - just check if DSL contains domain verbs
	if dsl == "" {
		return fmt.Errorf("empty DSL")
	}

	for verbName := range m.vocabulary.Verbs {
		verbPattern := fmt.Sprintf("(%s", verbName)
		if strings.Contains(dsl, verbPattern) {
			return nil // Valid verb found
		}
	}
	return fmt.Errorf("no valid verbs found for domain %s", m.name)
}

func (m *MockDomain) ValidateStateTransition(from, to string) error {
	// Mock state transition validation
	validTransitions := map[string][]string{
		"INITIAL":  {"ACTIVE"},
		"ACTIVE":   {"COMPLETE"},
		"COMPLETE": {},
	}

	if validStates, exists := validTransitions[from]; exists {
		for _, validTo := range validStates {
			if validTo == to {
				return nil
			}
		}
	}
	return fmt.Errorf("invalid state transition from %s to %s", from, to)
}

func (m *MockDomain) GenerateDSL(ctx context.Context, req *GenerationRequest) (*GenerationResponse, error) {
	if req == nil {
		return nil, fmt.Errorf("generation request cannot be nil")
	}

	// Mock DSL generation based on instruction
	var dsl, verb string
	var toState string

	if req.Instruction == "start workflow" || req.Instruction == "begin" {
		verb = fmt.Sprintf("%s.start", m.name)
		dsl = fmt.Sprintf("(%s \"test-entity-id\")", verb)
		toState = "ACTIVE"
	} else if req.Instruction == "complete workflow" || req.Instruction == "finish" {
		verb = fmt.Sprintf("%s.complete", m.name)
		dsl = fmt.Sprintf("(%s \"test-entity-id\")", verb)
		toState = "COMPLETE"
	} else {
		return nil, fmt.Errorf("unknown instruction: %s", req.Instruction)
	}

	return &GenerationResponse{
		DSL:         dsl,
		Verb:        verb,
		Parameters:  map[string]interface{}{"entity_id": "test-entity-id"},
		FromState:   "INITIAL",
		ToState:     toState,
		IsValid:     true,
		Confidence:  0.9,
		Explanation: fmt.Sprintf("Generated %s operation", verb),
		RequestID:   req.RequestID,
		Timestamp:   time.Now(),
	}, nil
}

func (m *MockDomain) GetCurrentState(context map[string]interface{}) (string, error) {
	if context == nil {
		return m.initialState, nil
	}

	if state, exists := context["current_state"]; exists {
		if stateStr, ok := state.(string); ok {
			// Validate state
			for _, validState := range m.states {
				if validState == stateStr {
					return stateStr, nil
				}
			}
			return "", fmt.Errorf("invalid state: %s", stateStr)
		}
	}

	return m.initialState, nil
}

func (m *MockDomain) ExtractContext(dsl string) (map[string]interface{}, error) {
	// Mock context extraction
	context := make(map[string]interface{})

	// Extract entity ID if present
	if dsl != "" {
		context["entity_id"] = "test-entity-id"

		// Infer state from verb
		if fmt.Sprintf("(%s.start", m.name) == dsl[:len(fmt.Sprintf("(%s.start", m.name))] {
			context["current_state"] = "ACTIVE"
		} else if fmt.Sprintf("(%s.complete", m.name) == dsl[:len(fmt.Sprintf("(%s.complete", m.name))] {
			context["current_state"] = "COMPLETE"
		}
	}

	return context, nil
}

// Test helper methods
func (m *MockDomain) SetHealthy(healthy bool) {
	m.healthy = healthy
}

func (m *MockDomain) AddVerb(verbDef *VerbDefinition) {
	m.vocabulary.Verbs[verbDef.Name] = verbDef
}

func (m *MockDomain) UpdateMetrics(metrics *DomainMetrics) {
	m.metrics = metrics
}

// Test functions

func TestMockDomain_BasicFunctionality(t *testing.T) {
	domain := NewMockDomain("test", "1.0.0")

	// Test basic properties
	if domain.Name() != "test" {
		t.Errorf("Expected name 'test', got %s", domain.Name())
	}

	if domain.Version() != "1.0.0" {
		t.Errorf("Expected version '1.0.0', got %s", domain.Version())
	}

	if !domain.IsHealthy() {
		t.Error("Expected domain to be healthy")
	}

	// Test vocabulary
	vocab := domain.GetVocabulary()
	if vocab == nil {
		t.Fatal("Vocabulary should not be nil")
	}

	if vocab.Domain != "test" {
		t.Errorf("Expected vocabulary domain 'test', got %s", vocab.Domain)
	}

	if len(vocab.Verbs) != 2 {
		t.Errorf("Expected 2 verbs, got %d", len(vocab.Verbs))
	}
}

func TestMockDomain_VerbValidation(t *testing.T) {
	domain := NewMockDomain("test", "1.0.0")

	tests := []struct {
		name    string
		dsl     string
		wantErr bool
	}{
		{
			name:    "Valid start verb",
			dsl:     "(test.start \"uuid-123\")",
			wantErr: false,
		},
		{
			name:    "Valid complete verb",
			dsl:     "(test.complete \"uuid-123\")",
			wantErr: false,
		},
		{
			name:    "Invalid verb",
			dsl:     "(other.start \"uuid-123\")",
			wantErr: true,
		},
		{
			name:    "Empty DSL",
			dsl:     "",
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := domain.ValidateVerbs(tt.dsl)
			if (err != nil) != tt.wantErr {
				t.Errorf("ValidateVerbs() error = %v, wantErr %v", err, tt.wantErr)
			}
		})
	}
}

func TestMockDomain_StateTransitions(t *testing.T) {
	domain := NewMockDomain("test", "1.0.0")

	tests := []struct {
		name    string
		from    string
		to      string
		wantErr bool
	}{
		{
			name:    "Valid initial to active",
			from:    "INITIAL",
			to:      "ACTIVE",
			wantErr: false,
		},
		{
			name:    "Valid active to complete",
			from:    "ACTIVE",
			to:      "COMPLETE",
			wantErr: false,
		},
		{
			name:    "Invalid initial to complete",
			from:    "INITIAL",
			to:      "COMPLETE",
			wantErr: true,
		},
		{
			name:    "Invalid from state",
			from:    "INVALID",
			to:      "ACTIVE",
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := domain.ValidateStateTransition(tt.from, tt.to)
			if (err != nil) != tt.wantErr {
				t.Errorf("ValidateStateTransition() error = %v, wantErr %v", err, tt.wantErr)
			}
		})
	}
}

func TestMockDomain_DSLGeneration(t *testing.T) {
	domain := NewMockDomain("test", "1.0.0")
	ctx := context.Background()

	tests := []struct {
		name         string
		instruction  string
		wantErr      bool
		expectedVerb string
	}{
		{
			name:         "Start workflow",
			instruction:  "start workflow",
			wantErr:      false,
			expectedVerb: "test.start",
		},
		{
			name:         "Begin workflow",
			instruction:  "begin",
			wantErr:      false,
			expectedVerb: "test.start",
		},
		{
			name:         "Complete workflow",
			instruction:  "complete workflow",
			wantErr:      false,
			expectedVerb: "test.complete",
		},
		{
			name:         "Finish workflow",
			instruction:  "finish",
			wantErr:      false,
			expectedVerb: "test.complete",
		},
		{
			name:         "Unknown instruction",
			instruction:  "unknown command",
			wantErr:      true,
			expectedVerb: "",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			req := &GenerationRequest{
				Instruction: tt.instruction,
				SessionID:   "test-session",
				Timestamp:   time.Now(),
			}

			resp, err := domain.GenerateDSL(ctx, req)
			if (err != nil) != tt.wantErr {
				t.Errorf("GenerateDSL() error = %v, wantErr %v", err, tt.wantErr)
				return
			}

			if !tt.wantErr {
				if resp == nil {
					t.Fatal("Expected response, got nil")
				}

				if resp.Verb != tt.expectedVerb {
					t.Errorf("Expected verb %s, got %s", tt.expectedVerb, resp.Verb)
				}

				if !resp.IsValid {
					t.Error("Expected valid response")
				}

				if resp.Confidence <= 0 {
					t.Error("Expected positive confidence")
				}
			}
		})
	}
}

func TestMockDomain_GetCurrentState(t *testing.T) {
	domain := NewMockDomain("test", "1.0.0")

	tests := []struct {
		name          string
		context       map[string]interface{}
		expectedState string
		wantErr       bool
	}{
		{
			name:          "Nil context",
			context:       nil,
			expectedState: "INITIAL",
			wantErr:       false,
		},
		{
			name:          "Empty context",
			context:       map[string]interface{}{},
			expectedState: "INITIAL",
			wantErr:       false,
		},
		{
			name: "Valid state in context",
			context: map[string]interface{}{
				"current_state": "ACTIVE",
			},
			expectedState: "ACTIVE",
			wantErr:       false,
		},
		{
			name: "Invalid state in context",
			context: map[string]interface{}{
				"current_state": "INVALID",
			},
			expectedState: "",
			wantErr:       true,
		},
		{
			name: "Non-string state in context",
			context: map[string]interface{}{
				"current_state": 123,
			},
			expectedState: "INITIAL",
			wantErr:       false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			state, err := domain.GetCurrentState(tt.context)
			if (err != nil) != tt.wantErr {
				t.Errorf("GetCurrentState() error = %v, wantErr %v", err, tt.wantErr)
				return
			}

			if state != tt.expectedState {
				t.Errorf("Expected state %s, got %s", tt.expectedState, state)
			}
		})
	}
}

func TestMockDomain_ExtractContext(t *testing.T) {
	domain := NewMockDomain("test", "1.0.0")

	tests := []struct {
		name        string
		dsl         string
		expectedKey string
		expectedVal interface{}
	}{
		{
			name:        "Empty DSL",
			dsl:         "",
			expectedKey: "",
			expectedVal: nil,
		},
		{
			name:        "Start verb DSL",
			dsl:         "(test.start \"uuid-123\")",
			expectedKey: "current_state",
			expectedVal: "ACTIVE",
		},
		{
			name:        "Complete verb DSL",
			dsl:         "(test.complete \"uuid-123\")",
			expectedKey: "current_state",
			expectedVal: "COMPLETE",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			context, err := domain.ExtractContext(tt.dsl)
			if err != nil {
				t.Errorf("ExtractContext() error = %v", err)
				return
			}

			if tt.expectedKey == "" {
				// For empty DSL, should still have entity_id if DSL is not empty
				if tt.dsl == "" {
					return // Expected empty context for empty DSL
				}
			}

			if tt.expectedKey != "" {
				if val, exists := context[tt.expectedKey]; !exists {
					t.Errorf("Expected context key %s not found", tt.expectedKey)
				} else if val != tt.expectedVal {
					t.Errorf("Expected %v for key %s, got %v", tt.expectedVal, tt.expectedKey, val)
				}
			}
		})
	}
}

func TestMockDomain_HealthStatus(t *testing.T) {
	domain := NewMockDomain("test", "1.0.0")

	// Should be healthy by default
	if !domain.IsHealthy() {
		t.Error("Expected domain to be healthy by default")
	}

	// Test setting unhealthy
	domain.SetHealthy(false)
	if domain.IsHealthy() {
		t.Error("Expected domain to be unhealthy after setting")
	}

	// Test setting healthy again
	domain.SetHealthy(true)
	if !domain.IsHealthy() {
		t.Error("Expected domain to be healthy after setting")
	}
}

func TestMockDomain_Metrics(t *testing.T) {
	domain := NewMockDomain("test", "1.0.0")

	metrics := domain.GetMetrics()
	if metrics == nil {
		t.Fatal("Expected metrics, got nil")
	}

	if metrics.TotalVerbs != 2 {
		t.Errorf("Expected 2 total verbs, got %d", metrics.TotalVerbs)
	}

	if !metrics.IsHealthy {
		t.Error("Expected metrics to show healthy domain")
	}

	// Test updating metrics
	newMetrics := &DomainMetrics{
		TotalRequests:      100,
		SuccessfulRequests: 95,
		FailedRequests:     5,
		TotalVerbs:         3,
		ActiveVerbs:        3,
		UnusedVerbs:        0,
		IsHealthy:          true,
		Version:            "1.0.0",
	}

	domain.UpdateMetrics(newMetrics)
	updatedMetrics := domain.GetMetrics()

	if updatedMetrics.TotalRequests != 100 {
		t.Errorf("Expected 100 total requests, got %d", updatedMetrics.TotalRequests)
	}

	if updatedMetrics.TotalVerbs != 3 {
		t.Errorf("Expected 3 total verbs, got %d", updatedMetrics.TotalVerbs)
	}
}

func TestVerbDefinition_Validation(t *testing.T) {
	domain := NewMockDomain("test", "1.0.0")

	// Test adding custom verb
	customVerb := &VerbDefinition{
		Name:        "test.custom",
		Category:    "custom",
		Version:     "1.0.0",
		Description: "Custom test verb",
		Arguments: map[string]*ArgumentSpec{
			"name": {
				Name:        "name",
				Type:        ArgumentTypeString,
				Required:    true,
				Description: "Entity name",
				MinLength:   &[]int{1}[0],
				MaxLength:   &[]int{100}[0],
			},
			"amount": {
				Name:        "amount",
				Type:        ArgumentTypeDecimal,
				Required:    false,
				Description: "Optional amount",
				MinValue:    &[]float64{0.0}[0],
				MaxValue:    &[]float64{1000000.0}[0],
			},
			"type": {
				Name:        "type",
				Type:        ArgumentTypeEnum,
				Required:    true,
				Description: "Entity type",
				EnumValues:  []string{"A", "B", "C"},
			},
		},
		StateTransition: &StateTransition{
			FromStates: []string{"INITIAL"},
			ToState:    "CUSTOM",
		},
		Idempotent:      false,
		GuardConditions: []string{"entity_exists"},
		SideEffects:     []string{"sends_notification"},
		Examples:        []string{"(test.custom :name \"example\" :type \"A\")"},
		CreatedAt:       time.Now(),
		UpdatedAt:       time.Now(),
	}

	domain.AddVerb(customVerb)

	// Verify verb was added
	vocab := domain.GetVocabulary()
	if _, exists := vocab.Verbs["test.custom"]; !exists {
		t.Error("Custom verb was not added to vocabulary")
	}

	// Test argument specifications
	verb := vocab.Verbs["test.custom"]

	// Test string argument
	nameArg := verb.Arguments["name"]
	if nameArg.Type != ArgumentTypeString {
		t.Errorf("Expected string type, got %s", nameArg.Type)
	}
	if !nameArg.Required {
		t.Error("Expected name argument to be required")
	}
	if nameArg.MinLength == nil || *nameArg.MinLength != 1 {
		t.Error("Expected min length of 1")
	}

	// Test decimal argument
	amountArg := verb.Arguments["amount"]
	if amountArg.Type != ArgumentTypeDecimal {
		t.Errorf("Expected decimal type, got %s", amountArg.Type)
	}
	if amountArg.Required {
		t.Error("Expected amount argument to be optional")
	}

	// Test enum argument
	typeArg := verb.Arguments["type"]
	if typeArg.Type != ArgumentTypeEnum {
		t.Errorf("Expected enum type, got %s", typeArg.Type)
	}
	if len(typeArg.EnumValues) != 3 {
		t.Errorf("Expected 3 enum values, got %d", len(typeArg.EnumValues))
	}
}

func TestDomainError_Interface(t *testing.T) {
	err := &DomainError{
		Domain:    "test",
		Code:      "TEST_ERROR",
		Message:   "Test error message",
		Details:   map[string]interface{}{"key": "value"},
		Timestamp: time.Now(),
	}

	expectedMsg := "[test] TEST_ERROR: Test error message"
	if err.Error() != expectedMsg {
		t.Errorf("Expected error message %s, got %s", expectedMsg, err.Error())
	}
}

func TestValidationError_Interface(t *testing.T) {
	err := &ValidationError{
		Code:        "VALIDATION_FAILED",
		Message:     "Validation failed",
		Field:       "test_field",
		Value:       "invalid_value",
		Context:     map[string]interface{}{"reason": "too_short"},
		Suggestions: []string{"Use longer value", "Check format"},
	}

	if err.Error() != "Validation failed" {
		t.Errorf("Expected error message 'Validation failed', got %s", err.Error())
	}
}
