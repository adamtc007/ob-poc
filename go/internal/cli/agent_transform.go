package cli

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"log"

	"dsl-ob-poc/internal/agent"
	"dsl-ob-poc/internal/datastore"
	"dsl-ob-poc/internal/shared-dsl/session"
	"dsl-ob-poc/internal/store"
)

// RunAgentTransform handles the 'agent-transform' command for AI-powered DSL transformations
func RunAgentTransform(ctx context.Context, ds datastore.DataStore, ai *agent.Agent, args []string) error {
	var finalDSL string
	fs := flag.NewFlagSet("agent-transform", flag.ExitOnError)
	cbuID := fs.String("cbu", "", "The CBU ID of the case to transform (required)")
	instruction := fs.String("instruction", "", "Transformation instruction for the AI agent (required)")
	targetState := fs.String("target-state", "", "Target onboarding state (optional)")
	saveChanges := fs.Bool("save", false, "Save the transformed DSL to the database")
	if err := fs.Parse(args); err != nil {
		return fmt.Errorf("failed to parse flags: %w", err)
	}

	if *cbuID == "" || *instruction == "" {
		fs.Usage()
		return fmt.Errorf("error: --cbu and --instruction flags are required")
	}

	if ai == nil {
		return fmt.Errorf("ai agent is not configured; set GEMINI_API_KEY and try again")
	}

	log.Printf("🤖 Starting AI-powered DSL transformation for CBU: %s", *cbuID)
	log.Printf("📝 Instruction: %s", *instruction)

	// 2. Get the current onboarding session (for validation)
	_, err := ds.GetOnboardingSession(ctx, *cbuID)
	if err != nil {
		return fmt.Errorf("failed to get onboarding session for CBU %s: %w", *cbuID, err)
	}

	currentDSLState, err := ds.GetLatestDSLWithState(ctx, *cbuID)
	if err != nil {
		return fmt.Errorf("failed to get current DSL for CBU %s: %w", *cbuID, err)
	}

	// 2. Prepare transformation request
	var finalTargetState string
	if *targetState != "" {
		finalTargetState = *targetState
	} else {
		finalTargetState = string(currentDSLState.OnboardingState)
	}

	transformRequest := agent.DSLTransformationRequest{
		CurrentDSL:  currentDSLState.DSLText,
		Instruction: *instruction,
		TargetState: finalTargetState,
		Context: map[string]interface{}{
			"current_state":  currentDSLState.OnboardingState,
			"version_number": currentDSLState.VersionNumber,
			"cbu_id":         *cbuID,
		},
	}

	// 3. Call the AI agent for DSL transformation
	log.Println("🧠 Calling AI Agent for DSL transformation...")
	response, err := ai.CallDSLTransformationAgent(ctx, transformRequest)
	if err != nil {
		return fmt.Errorf("ai agent transformation failed: %w", err)
	}

	// 4. Display the transformation results
	fmt.Printf("\n🎯 **AI DSL Transformation Results**\n")
	fmt.Printf("==========================================\n")
	fmt.Printf("📊 **Confidence Score**: %.2f\n", response.Confidence)
	fmt.Printf("📝 **Explanation**: %s\n\n", response.Explanation)

	if len(response.Changes) > 0 {
		fmt.Printf("🔄 **Changes Made**:\n")
		for i, change := range response.Changes {
			fmt.Printf("   %d. %s\n", i+1, change)
		}
		fmt.Println()
	}

	fmt.Printf("📄 **Original DSL**:\n")
	fmt.Printf("-------------------------------------------\n")
	fmt.Println(currentDSLState.DSLText)
	fmt.Printf("-------------------------------------------\n\n")

	// Create DSL session manager and accumulate DSL (single source of truth)
	sessionMgr := session.NewManager()
	dslSession := sessionMgr.GetOrCreate(*cbuID, "onboarding")

	// Accumulate current DSL
	err = dslSession.AccumulateDSL(currentDSLState.DSLText)
	if err != nil {
		return fmt.Errorf("failed to accumulate current DSL: %w", err)
	}

	// Accumulate transformed DSL from agent
	err = dslSession.AccumulateDSL(response.NewDSL)
	if err != nil {
		return fmt.Errorf("failed to accumulate transformed DSL: %w", err)
	}

	// Get final DSL from state manager
	finalDSL = dslSession.GetDSL()

	fmt.Printf("✨ **Transformed DSL**:\n")
	fmt.Printf("-------------------------------------------\n")
	fmt.Println(finalDSL)
	fmt.Printf("-------------------------------------------\n\n")

	// 5. Optionally save the transformed DSL
	if *saveChanges {
		log.Println("💾 Saving transformed DSL to database...")

		// Determine the appropriate state for saving
		var saveState store.OnboardingState
		if *targetState != "" {
			saveState = store.OnboardingState(*targetState)
		} else {
			saveState = currentDSLState.OnboardingState
		}

		versionID, insertErr := ds.InsertDSLWithState(ctx, *cbuID, finalDSL, saveState)
		if insertErr != nil {
			return fmt.Errorf("failed to save transformed DSL: %w", insertErr)
		}

		// Update onboarding session state
		updateErr := ds.UpdateOnboardingState(ctx, *cbuID, saveState, versionID)
		if updateErr != nil {
			return fmt.Errorf("failed to update onboarding state: %w", updateErr)
		}

		fmt.Printf("✅ **Transformed DSL saved successfully**\n")
		fmt.Printf("🆔 Version ID: %s\n", versionID)
		fmt.Printf("🎯 State: %s\n", saveState)
		fmt.Printf("📊 Final DSL: %d characters\n", len(finalDSL))
	} else {
		fmt.Printf("ℹ️  **Note**: Use --save flag to persist the transformed DSL to the database\n")
	}

	return nil
}

// RunAgentValidate handles the 'agent-validate' command for AI-powered DSL validation
func RunAgentValidate(ctx context.Context, ds datastore.DataStore, ai *agent.Agent, args []string) error {
	fs := flag.NewFlagSet("agent-validate", flag.ExitOnError)
	cbuID := fs.String("cbu", "", "The CBU ID of the case to validate (required)")
	if err := fs.Parse(args); err != nil {
		return fmt.Errorf("failed to parse flags: %w", err)
	}

	if *cbuID == "" {
		fs.Usage()
		return fmt.Errorf("error: --cbu flag is required")
	}

	if ai == nil {
		return fmt.Errorf("ai agent is not configured; set GEMINI_API_KEY and try again")
	}

	log.Printf("🔍 Starting AI-powered DSL validation for CBU: %s", *cbuID)

	// 1. Get the current DSL
	currentDSLState, err := ds.GetLatestDSLWithState(ctx, *cbuID)
	if err != nil {
		return fmt.Errorf("failed to get current DSL for CBU %s: %w", *cbuID, err)
	}

	// 2. Call the AI agent for DSL validation
	log.Println("🧠 Calling AI Agent for DSL validation...")
	validation, err := ai.CallDSLValidationAgent(ctx, currentDSLState.DSLText)
	if err != nil {
		return fmt.Errorf("ai agent validation failed: %w", err)
	}

	// 3. Display validation results
	fmt.Printf("\n🔍 **AI DSL Validation Results**\n")
	fmt.Printf("==========================================\n")
	fmt.Printf("✅ **Valid**: %t\n", validation.IsValid)
	fmt.Printf("📊 **Validation Score**: %.2f\n", validation.ValidationScore)
	fmt.Printf("📝 **Summary**: %s\n\n", validation.Summary)

	if len(validation.Errors) > 0 {
		fmt.Printf("❌ **Errors**:\n")
		for i, err := range validation.Errors {
			fmt.Printf("   %d. %s\n", i+1, err)
		}
		fmt.Println()
	}

	if len(validation.Warnings) > 0 {
		fmt.Printf("⚠️  **Warnings**:\n")
		for i, warning := range validation.Warnings {
			fmt.Printf("   %d. %s\n", i+1, warning)
		}
		fmt.Println()
	}

	if len(validation.Suggestions) > 0 {
		fmt.Printf("💡 **Suggestions**:\n")
		for i, suggestion := range validation.Suggestions {
			fmt.Printf("   %d. %s\n", i+1, suggestion)
		}
		fmt.Println()
	}

	fmt.Printf("📄 **Validated DSL**:\n")
	fmt.Printf("-------------------------------------------\n")
	fmt.Println(currentDSLState.DSLText)
	fmt.Printf("-------------------------------------------\n")

	return nil
}

// RunAgentDemo demonstrates AI agent capabilities without requiring API key
func RunAgentDemo(ctx context.Context, ds datastore.DataStore, args []string) error {
	fs := flag.NewFlagSet("agent-demo", flag.ExitOnError)
	cbuID := fs.String("cbu", "", "The CBU ID to demonstrate with (optional)")
	if err := fs.Parse(args); err != nil {
		return fmt.Errorf("failed to parse flags: %w", err)
	}

	fmt.Printf("\n🤖 **AI Agent Integration Demo**\n")
	fmt.Printf("==========================================\n\n")

	fmt.Printf("🎯 **How AI Agent Integration Works:**\n\n")

	fmt.Printf("1. **Agent Architecture**:\n")
	fmt.Printf("   - Uses Google Gemini 2.5 Flash model\n")
	fmt.Printf("   - Structured JSON responses with strong typing\n")
	fmt.Printf("   - Comprehensive system prompting\n")
	fmt.Printf("   - Integration with DSL state management\n\n")

	fmt.Printf("2. **Current Capabilities**:\n")
	fmt.Printf("   ✅ KYC Discovery (discover-kyc command)\n")
	fmt.Printf("   ✅ DSL Transformation (agent-transform command)\n")
	fmt.Printf("   ✅ DSL Validation (agent-validate command)\n\n")

	fmt.Printf("3. **Example Use Cases**:\n")
	fmt.Printf("   🔄 Transform DSL based on natural language instructions\n")
	fmt.Printf("   🔍 Validate DSL correctness and suggest improvements\n")
	fmt.Printf("   📋 Generate KYC requirements from business context\n")
	fmt.Printf("   🎯 Manage onboarding state transitions intelligently\n\n")

	if *cbuID != "" {
		// Show current DSL if CBU provided
		currentDSLState, err := ds.GetLatestDSLWithState(ctx, *cbuID)
		if err == nil {
			fmt.Printf("📄 **Current DSL for %s** (State: %s, Version: %d):\n", *cbuID, currentDSLState.OnboardingState, currentDSLState.VersionNumber)
			fmt.Printf("-------------------------------------------\n")
			fmt.Println(currentDSLState.DSLText)
			fmt.Printf("-------------------------------------------\n\n")
		}
	}

	fmt.Printf("🚀 **To Use AI Agent Features**:\n")
	fmt.Printf("   1. Set GEMINI_API_KEY environment variable\n")
	fmt.Printf("   2. Run: ./dsl-poc agent-transform --cbu=CBU-1234 --instruction=\"Add FUND_ACCOUNTING product\"\n")
	fmt.Printf("   3. Run: ./dsl-poc agent-validate --cbu=CBU-1234\n")
	fmt.Printf("   4. Run: ./dsl-poc discover-kyc --cbu=CBU-1234\n\n")

	fmt.Printf("💡 **Example Transformation Instructions**:\n")
	fmt.Printf("   - \"Add TRANSFER_AGENT to the products list\"\n")
	fmt.Printf("   - \"Change the jurisdiction from US to LU\"\n")
	fmt.Printf("   - \"Add a W8BEN-E document to KYC requirements\"\n")
	fmt.Printf("   - \"Remove the CUSTODY service and add SETTLEMENT\"\n")
	fmt.Printf("   - \"Update the nature-purpose to indicate hedge fund\"\n\n")

	// Show mock examples
	fmt.Printf("📊 **Example Transformation Request**:\n")
	exampleRequest := agent.DSLTransformationRequest{
		CurrentDSL:  "(case.create\\n  (cbu.id \"CBU-1234\")\\n  (nature-purpose \"UCITS equity fund\")\\n)\\n\\n(products.add \"CUSTODY\")",
		Instruction: "Add FUND_ACCOUNTING to the products list",
		TargetState: "PRODUCTS_ADDED",
		Context: map[string]interface{}{
			"current_state":  "PRODUCTS_ADDED",
			"version_number": 2,
		},
	}

	requestJSON, _ := json.Marshal(exampleRequest)
	fmt.Println(string(requestJSON))

	fmt.Printf("\n📋 **Example Response**:\n")
	exampleResponse := agent.DSLTransformationResponse{
		NewDSL:      "(case.create\\n  (cbu.id \"CBU-1234\")\\n  (nature-purpose \"UCITS equity fund\")\\n)\\n\\n(products.add \"CUSTODY\" \"FUND_ACCOUNTING\")",
		Explanation: "Added FUND_ACCOUNTING to the products list as requested",
		Changes:     []string{"Modified products.add block to include FUND_ACCOUNTING", "Maintained existing CUSTODY product"},
		Confidence:  0.95,
	}

	responseJSON, _ := json.Marshal(exampleResponse)
	fmt.Println(string(responseJSON))

	return nil
}
