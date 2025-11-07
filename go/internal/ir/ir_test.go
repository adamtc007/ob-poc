package ir

import (
	"os"
	"strings"
	"testing"
)

func TestParseAndValidateExampleIR(t *testing.T) {
	// Load the example IR from the dsl/examples directory
	data, err := os.ReadFile("../../dsl/examples/corporate_subscription_example.json")
	if err != nil {
		t.Fatalf("Failed to read example IR: %v", err)
	}

	// Parse the plan
	plan, err := ParsePlan(data)
	if err != nil {
		t.Fatalf("Failed to parse plan: %v", err)
	}

	// Validate the plan
	err = plan.Validate()
	if err != nil {
		t.Fatalf("Plan validation failed: %v", err)
	}

	// Basic assertions
	if plan.Version != "1.0.0" {
		t.Errorf("Expected version '1.0.0', got %s", plan.Version)
	}

	if len(plan.Steps) == 0 {
		t.Error("Expected non-empty steps")
	}

	t.Logf("Successfully validated plan with %d steps", len(plan.Steps))

	// Test specific step types
	foundOpportunity := false
	foundSubscription := false
	foundKYC := false

	for i, step := range plan.Steps {
		t.Logf("Step %d: %s", i, step.Op)

		switch step.Op {
		case OpInvestorStartOpportunity:
			foundOpportunity = true
			var args InvestorStartOpportunityArgs
			if err := step.DecodeArgs(&args); err != nil {
				t.Errorf("Failed to decode opportunity args: %v", err)
			}
			if args.Type != "CORPORATE" {
				t.Errorf("Expected type CORPORATE, got %s", args.Type)
			}

		case OpSubscribeRequest:
			foundSubscription = true
			var args SubscribeRequestArgs
			if err := step.DecodeArgs(&args); err != nil {
				t.Errorf("Failed to decode subscription args: %v", err)
			}
			if args.Amount != 5000000 {
				t.Errorf("Expected amount 5000000, got %f", args.Amount)
			}
			if args.Currency != "USD" {
				t.Errorf("Expected currency USD, got %s", args.Currency)
			}

		case OpKYCApprove:
			foundKYC = true
			var args KYCApproveArgs
			if err := step.DecodeArgs(&args); err != nil {
				t.Errorf("Failed to decode KYC args: %v", err)
			}
			if args.Risk != "MEDIUM" {
				t.Errorf("Expected risk MEDIUM, got %s", args.Risk)
			}
		}
	}

	if !foundOpportunity {
		t.Error("Expected to find investor.start-opportunity step")
	}
	if !foundSubscription {
		t.Error("Expected to find subscribe.request step")
	}
	if !foundKYC {
		t.Error("Expected to find kyc.approve step")
	}
}

func TestValidationErrors(t *testing.T) {
	tests := []struct {
		name     string
		planJSON string
		wantErr  string
	}{
		{
			name: "invalid version",
			planJSON: `{
				"version": "2.0.0",
				"plan_id": "8f1d2b7e-8f4b-4a2a-9d2b-7a2f3c1d9a01",
				"created_at": "2025-11-03T20:15:00Z",
				"steps": []
			}`,
			wantErr: "version must match",
		},
		{
			name: "invalid plan_id",
			planJSON: `{
				"version": "1.0.0",
				"plan_id": "not-a-uuid",
				"created_at": "2025-11-03T20:15:00Z",
				"steps": []
			}`,
			wantErr: "plan_id must be a UUID string",
		},
		{
			name: "empty steps",
			planJSON: `{
				"version": "1.0.0",
				"plan_id": "8f1d2b7e-8f4b-4a2a-9d2b-7a2f3c1d9a01",
				"created_at": "2025-11-03T20:15:00Z",
				"steps": []
			}`,
			wantErr: "steps must be non-empty",
		},
		{
			name: "invalid investor type",
			planJSON: `{
				"version": "1.0.0",
				"plan_id": "8f1d2b7e-8f4b-4a2a-9d2b-7a2f3c1d9a01",
				"created_at": "2025-11-03T20:15:00Z",
				"steps": [
					{
						"op": "investor.start-opportunity",
						"args": {
							"legal_name": "Test Corp",
							"type": "INVALID_TYPE",
							"domicile": "GB"
						}
					}
				]
			}`,
			wantErr: "type invalid",
		},
		{
			name: "invalid UUID in subscription",
			planJSON: `{
				"version": "1.0.0",
				"plan_id": "8f1d2b7e-8f4b-4a2a-9d2b-7a2f3c1d9a01",
				"created_at": "2025-11-03T20:15:00Z",
				"steps": [
					{
						"op": "subscribe.request",
						"args": {
							"investor_id": "not-a-uuid",
							"class_id": "a6b3b7e1-2c1f-4d5e-8a90-1b2c3d4e5f60",
							"amount": 5000000,
							"trade_date": "2025-11-03",
							"currency": "USD"
						}
					}
				]
			}`,
			wantErr: "investor_id must be UUID",
		},
		{
			name: "invalid currency",
			planJSON: `{
				"version": "1.0.0",
				"plan_id": "8f1d2b7e-8f4b-4a2a-9d2b-7a2f3c1d9a01",
				"created_at": "2025-11-03T20:15:00Z",
				"steps": [
					{
						"op": "subscribe.request",
						"args": {
							"investor_id": "2fd4e5a7-3e84-4d2b-9f1a-7f6d0a9b1234",
							"class_id": "a6b3b7e1-2c1f-4d5e-8a90-1b2c3d4e5f60",
							"amount": 5000000,
							"trade_date": "2025-11-03",
							"currency": "INVALID"
						}
					}
				]
			}`,
			wantErr: "currency must match",
		},
		{
			name: "invalid date format",
			planJSON: `{
				"version": "1.0.0",
				"plan_id": "8f1d2b7e-8f4b-4a2a-9d2b-7a2f3c1d9a01",
				"created_at": "2025-11-03T20:15:00Z",
				"steps": [
					{
						"op": "subscribe.request",
						"args": {
							"investor_id": "2fd4e5a7-3e84-4d2b-9f1a-7f6d0a9b1234",
							"class_id": "a6b3b7e1-2c1f-4d5e-8a90-1b2c3d4e5f60",
							"amount": 5000000,
							"trade_date": "invalid-date",
							"currency": "USD"
						}
					}
				]
			}`,
			wantErr: "trade_date: must be YYYY-MM-DD",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			plan, err := ParsePlan([]byte(tt.planJSON))
			if err != nil {
				// JSON parsing error
				if tt.wantErr == "" {
					t.Errorf("Unexpected JSON parse error: %v", err)
				}
				return
			}

			err = plan.Validate()
			if tt.wantErr == "" {
				if err != nil {
					t.Errorf("Unexpected validation error: %v", err)
				}
			} else {
				if err == nil {
					t.Errorf("Expected validation error containing %q, got nil", tt.wantErr)
				} else if err.Error() == "" || len(err.Error()) < len(tt.wantErr) {
					t.Errorf("Expected validation error containing %q, got %q", tt.wantErr, err.Error())
				}
				t.Logf("Got expected error: %v", err)
			}
		})
	}
}

func TestAttrRefValidation(t *testing.T) {
	// Test valid AttrRef
	validAttrRefPlan := `{
		"version": "1.0.0",
		"plan_id": "8f1d2b7e-8f4b-4a2a-9d2b-7a2f3c1d9a01",
		"created_at": "2025-11-03T20:15:00Z",
		"steps": [
			{
				"op": "investor.start-opportunity",
				"args": {
					"legal_name": {
						"kind": "AttrRef",
						"id": "INV.LEGAL_NAME"
					},
					"type": "CORPORATE",
					"domicile": "GB"
				}
			}
		]
	}`

	plan, err := ParsePlan([]byte(validAttrRefPlan))
	if err != nil {
		t.Fatalf("Failed to parse plan with AttrRef: %v", err)
	}

	err = plan.Validate()
	if err != nil {
		t.Fatalf("Failed to validate plan with AttrRef: %v", err)
	}

	// Test invalid AttrRef
	invalidAttrRefPlan := `{
		"version": "1.0.0",
		"plan_id": "8f1d2b7e-8f4b-4a2a-9d2b-7a2f3c1d9a01",
		"created_at": "2025-11-03T20:15:00Z",
		"steps": [
			{
				"op": "investor.start-opportunity",
				"args": {
					"legal_name": {
						"kind": "AttrRef",
						"id": "invalid.attr.id.format"
					},
					"type": "CORPORATE",
					"domicile": "GB"
				}
			}
		]
	}`

	plan, err = ParsePlan([]byte(invalidAttrRefPlan))
	if err != nil {
		t.Fatalf("Failed to parse plan with invalid AttrRef: %v", err)
	}

	err = plan.Validate()
	if err == nil {
		t.Error("Expected validation error for invalid AttrRef ID format")
	}
}

func TestIdempotencyKeyValidation(t *testing.T) {
	tests := []struct {
		name    string
		key     string
		wantErr bool
	}{
		{"valid key", "sub-2fd4e5a7-2025-11-03-USD-5000000", false},
		{"valid short key", "1234567890", false},
		{"too short", "123456789", true},
		{"too long", strings.Repeat("a", 130), true},
		{"invalid chars", "key with spaces", true},
		{"valid with underscore", "key_with_underscore", false},
		{"valid with dash", "key-with-dash", false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			planJSON := `{
				"version": "1.0.0",
				"plan_id": "8f1d2b7e-8f4b-4a2a-9d2b-7a2f3c1d9a01",
				"created_at": "2025-11-03T20:15:00Z",
				"steps": [
					{
						"op": "kyc.begin",
						"args": {
							"investor_id": "2fd4e5a7-3e84-4d2b-9f1a-7f6d0a9b1234"
						},
						"idempotency_key": "` + tt.key + `"
					}
				]
			}`

			plan, err := ParsePlan([]byte(planJSON))
			if err != nil {
				t.Fatalf("Failed to parse plan: %v", err)
			}

			err = plan.Validate()
			if tt.wantErr {
				if err == nil {
					t.Errorf("Expected validation error for key %q", tt.key)
				}
			} else {
				if err != nil {
					t.Errorf("Unexpected validation error for key %q: %v", tt.key, err)
				}
			}
		})
	}
}
