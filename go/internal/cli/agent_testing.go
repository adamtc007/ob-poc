package cli

import (
	"context"
	"flag"
	"fmt"

	"dsl-ob-poc/internal/agent"
	"dsl-ob-poc/internal/datastore"
)

// RunAgentTest demonstrates AI agent capabilities with mock responses (no API key required)
func RunAgentTest(ctx context.Context, ds datastore.DataStore, args []string) error {
	fs := flag.NewFlagSet("agent-test", flag.ExitOnError)
	cbuID := fs.String("cbu", "CBU-1234", "The CBU ID to test with (default: CBU-1234)")
	testType := fs.String("type", "all", "Test type: kyc, transform, validate, or all (default: all)")
	if err := fs.Parse(args); err != nil {
		return fmt.Errorf("failed to parse flags: %w", err)
	}

	fmt.Printf("\n🧪 **AI Agent Integration Test** (Mock Mode)\n")
	fmt.Printf("==========================================\n")
	fmt.Printf("Testing with CBU: %s\n", *cbuID)
	fmt.Printf("Test type: %s\n\n", *testType)

	// Get current DSL for testing
	currentDSLState, err := ds.GetLatestDSLWithState(ctx, *cbuID)
	if err != nil {
		return fmt.Errorf("failed to get current DSL for CBU %s: %w", *cbuID, err)
	}

	fmt.Printf("📄 **Current DSL** (State: %s, Version: %d):\n", currentDSLState.OnboardingState, currentDSLState.VersionNumber)
	fmt.Printf("-------------------------------------------\n")
	fmt.Println(currentDSLState.DSLText)
	fmt.Printf("-------------------------------------------\n\n")

	// Create mock agent
	mockAgent := agent.NewMockAgent()

	// Test KYC agent
	if *testType == "all" || *testType == "kyc" {
		fmt.Printf("🔍 **Testing KYC Agent**\n")
		fmt.Printf("================================\n")

		// Test different scenarios
		scenarios := []struct {
			name        string
			nature      string
			products    []string
			description string
		}{
			{
				name:        "UCITS Fund",
				nature:      "UCITS equity fund domiciled in LU",
				products:    []string{"CUSTODY", "FUND_ACCOUNTING"},
				description: "European fund requiring EU compliance",
			},
			{
				name:        "US Hedge Fund",
				nature:      "US-based hedge fund",
				products:    []string{"TRANSFER_AGENT", "PRIME_BROKERAGE"},
				description: "US alternative investment fund",
			},
			{
				name:        "Corporate Entity",
				nature:      "Delaware corporation",
				products:    []string{"CUSTODY"},
				description: "Standard corporate entity",
			},
		}

		for i, scenario := range scenarios {
			fmt.Printf("**Scenario %d: %s**\n", i+1, scenario.name)
			fmt.Printf("📝 Nature: %s\n", scenario.nature)
			fmt.Printf("📦 Products: %v\n", scenario.products)
			fmt.Printf("📋 Description: %s\n", scenario.description)

			kycReqs, callErr := mockAgent.CallKYCAgent(ctx, scenario.nature, scenario.products)
			if callErr != nil {
				fmt.Printf("❌ Error: %v\n\n", callErr)
				continue
			}

			fmt.Printf("✅ **KYC Requirements Generated:**\n")
			fmt.Printf("   📄 Documents: %v\n", kycReqs.Documents)
			fmt.Printf("   🌍 Jurisdictions: %v\n\n", kycReqs.Jurisdictions)
		}
	}

	// Test DSL transformation agent
	if *testType == "all" || *testType == "transform" {
		fmt.Printf("🔄 **Testing DSL Transformation Agent**\n")
		fmt.Printf("=====================================\n")

		// Test different transformation instructions
		instructions := []string{
			"Add TRANSFER_AGENT to the products list",
			"Add LU jurisdiction to KYC requirements",
			"Add W8BEN-E document to KYC requirements",
			"Change nature-purpose to indicate hedge fund",
		}

		for i, instruction := range instructions {
			fmt.Printf("**Transformation %d:**\n", i+1)
			fmt.Printf("📝 Instruction: %s\n", instruction)

			request := agent.DSLTransformationRequest{
				CurrentDSL:  currentDSLState.DSLText,
				Instruction: instruction,
				TargetState: string(currentDSLState.OnboardingState),
				Context: map[string]interface{}{
					"current_state":  currentDSLState.OnboardingState,
					"version_number": currentDSLState.VersionNumber,
				},
			}

			response, transformErr := mockAgent.CallDSLTransformationAgent(ctx, request)
			if transformErr != nil {
				fmt.Printf("❌ Error: %v\n\n", transformErr)
				continue
			}

			fmt.Printf("✅ **Transformation Result:**\n")
			fmt.Printf("   📊 Confidence: %.2f\n", response.Confidence)
			fmt.Printf("   📝 Explanation: %s\n", response.Explanation)
			fmt.Printf("   🔄 Changes: %v\n", response.Changes)
			fmt.Printf("   📄 **New DSL Preview:**\n")

			// Show first few lines of transformed DSL
			dslText := response.NewDSL
			if len(dslText) > 200 {
				fmt.Printf("      %s...\n", dslText[:200])
			} else {
				fmt.Printf("      %s\n", dslText)
			}
			fmt.Println()
		}
	}

	// Test DSL validation agent
	if *testType == "all" || *testType == "validate" {
		fmt.Printf("✅ **Testing DSL Validation Agent**\n")
		fmt.Printf("==================================\n")

		validation, validateErr := mockAgent.CallDSLValidationAgent(ctx, currentDSLState.DSLText)
		if validateErr != nil {
			return fmt.Errorf("validation failed: %w", validateErr)
		}

		fmt.Printf("📊 **Validation Results:**\n")
		fmt.Printf("   ✅ Valid: %t\n", validation.IsValid)
		fmt.Printf("   📈 Score: %.2f\n", validation.ValidationScore)
		fmt.Printf("   📝 Summary: %s\n", validation.Summary)

		if len(validation.Errors) > 0 {
			fmt.Printf("   ❌ Errors: %v\n", validation.Errors)
		}

		if len(validation.Warnings) > 0 {
			fmt.Printf("   ⚠️  Warnings: %v\n", validation.Warnings)
		}

		if len(validation.Suggestions) > 0 {
			fmt.Printf("   💡 Suggestions: %v\n", validation.Suggestions)
		}
		fmt.Println()
	}

	fmt.Printf("🎯 **Key Insights from Testing:**\n")
	fmt.Printf("================================\n")
	fmt.Printf("1. **KYC Context**: AI analyzes business nature + products → generates compliance requirements\n")
	fmt.Printf("2. **DSL Context**: AI transforms existing DSL based on natural language instructions\n")
	fmt.Printf("3. **Validation Context**: AI evaluates DSL completeness and suggests improvements\n")
	fmt.Printf("4. **State Integration**: All operations are aware of current onboarding state\n")
	fmt.Printf("5. **Structured Responses**: JSON responses enable reliable programmatic integration\n\n")

	fmt.Printf("🚀 **Next Steps:**\n")
	fmt.Printf("   1. Set GEMINI_API_KEY to test with real AI\n")
	fmt.Printf("   2. Use agent-transform with --save to persist AI changes\n")
	fmt.Printf("   3. Integrate AI agents into automated onboarding workflows\n")
	fmt.Printf("   4. Extend agent capabilities for other onboarding steps\n")

	return nil
}
