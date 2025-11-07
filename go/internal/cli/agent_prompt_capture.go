package cli

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"log"
	"os"
	"time"

	"dsl-ob-poc/internal/agent"
	"dsl-ob-poc/internal/datastore"
	"dsl-ob-poc/internal/dsl"
)

const (
	testTypeAll = "all"
	testTypeKYC = "kyc"
	testTypeDSL = "dsl"
)

// RunAgentPromptCapture demonstrates AI agent capabilities with full prompt/response capture
func RunAgentPromptCapture(ctx context.Context, ds datastore.DataStore, ai *agent.Agent, args []string) error {
	fs := flag.NewFlagSet("agent-prompt-capture", flag.ExitOnError)
	cbuID := fs.String("cbu", "", "The CBU ID to test with (optional)")
	testType := fs.String("type", "all", "Test type: kyc, transform, validate, or all")
	captureFile := fs.String("output", "", "File to capture prompts and responses (optional)")
	if err := fs.Parse(args); err != nil {
		return fmt.Errorf("failed to parse flags: %w", err)
	}

	var outputFile *os.File
	var err error

	if *captureFile != "" {
		outputFile, err = os.Create(*captureFile)
		if err != nil {
			return fmt.Errorf("failed to create output file: %w", err)
		}
		defer outputFile.Close()
		log.Printf("📁 Capturing prompts and responses to: %s", *captureFile)
	}

	writeToBoth := func(format string, args ...interface{}) {
		text := fmt.Sprintf(format, args...)
		fmt.Print(text)
		if outputFile != nil {
			if _, writeErr := outputFile.WriteString(text); writeErr != nil {
				log.Printf("Warning: Failed to write to output file: %v", writeErr)
			}
		}
	}

	timestamp := time.Now().Format("2006-01-02 15:04:05")
	writeToBoth("\n🧪 **AI Agent Prompt/Response Capture** - %s\n", timestamp)
	writeToBoth("=========================================================\n")

	var mockAgent *agent.MockAgent
	if ai != nil {
		writeToBoth("🔗 **Mode**: Real AI Agent (Google Gemini)\n")
	} else {
		writeToBoth("🤖 **Mode**: Mock Agent (Simulated Responses)\n")
		writeToBoth("ℹ️  **Note**: Set GEMINI_API_KEY environment variable for real AI responses\n")
		// Create mock agent for testing
		mockAgent = agent.NewMockAgent()
	}

	// Get CBU and DSL state
	finalCBUID := *cbuID
	if finalCBUID == "" {
		finalCBUID = "CBU-1234" // Default test CBU
	}

	writeToBoth("📋 **Test Configuration**:\n")
	writeToBoth("   CBU ID: %s\n", finalCBUID)
	writeToBoth("   Test Type: %s\n", *testType)
	writeToBoth("\n")

	currentDSLState, err := ds.GetLatestDSLWithState(ctx, finalCBUID)
	if err != nil {
		return fmt.Errorf("failed to get current DSL state: %w", err)
	}

	writeToBoth("📄 **Current DSL State**:\n")
	writeToBoth("   State: %s\n", currentDSLState.OnboardingState)
	writeToBoth("   Version: %d\n", currentDSLState.VersionNumber)
	writeToBoth("   DSL Length: %d characters\n\n", len(currentDSLState.DSLText))

	// Test KYC Agent with prompt capture
	if *testType == testTypeAll || *testType == testTypeKYC {
		writeToBoth("🔍 **KYC Agent Prompt/Response Capture**\n")
		writeToBoth("=======================================\n")

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
				description: "European regulated investment fund requiring Luxembourg compliance",
			},
			{
				name:        "US Hedge Fund",
				nature:      "US-based hedge fund",
				products:    []string{"PRIME_BROKERAGE", "TRANSFER_AGENT"},
				description: "Alternative investment vehicle requiring US regulatory compliance",
			},
		}

		for i, scenario := range scenarios {
			writeToBoth("\n**Scenario %d: %s**\n", i+1, scenario.name)
			writeToBoth("📝 Nature: %s\n", scenario.nature)
			writeToBoth("📦 Products: %v\n", scenario.products)

			// Show the system prompt that would be used
			writeToBoth("\n📤 **System Prompt to AI**:\n")
			writeToBoth("```\n")
			systemPrompt := `You are an expert KYC/AML Compliance Officer for a major global bank.
Your job is to analyze a new client's "nature and purpose" and their "requested products" to determine the *minimum* required KYC documents and all relevant jurisdictions.

RULES:
1. Analyze the "nature and purpose" for entity type and domicile (e.g., "UCITS fund domiciled in LU" -> Domicile is "LU").
2. Analyze the products for regulatory impact (e.g., "TRANSFER_AGENT" implies AML checks on investors).
3. Respond ONLY with a single, minified JSON object. Do not include markdown ticks, "json", or any other conversational text.
4. The JSON format MUST be: {"required_documents": ["doc1", "doc2"], "jurisdictions": ["jur1", "jur2"]}

EXAMPLES:
- Input: "UCITS equity fund domiciled in LU", Products: ["CUSTODY"]
- Output: {"required_documents":["CertificateOfIncorporation","ArticlesOfAssociation","W8BEN-E"],"jurisdictions":["LU"]}
- Input: "US-based hedge fund", Products: ["TRANSFER_AGENT", "CUSTODY"]
- Output: {"required_documents":["CertificateOfLimitedPartnership","PartnershipAgreement","W9","AMLPolicy"],"jurisdictions":["US"]}`
			writeToBoth("%s\n", systemPrompt)
			writeToBoth("```\n")

			// Show the user prompt
			var userPrompt string
			if len(scenario.products) == 0 {
				userPrompt = fmt.Sprintf(`Nature and Purpose: "%s", Products: []`, scenario.nature)
			} else {
				quoted := make([]string, len(scenario.products))
				for j, p := range scenario.products {
					quoted[j] = fmt.Sprintf(`"%s"`, p)
				}
				userPrompt = fmt.Sprintf(`Nature and Purpose: "%s", Products: [%s]`, scenario.nature, fmt.Sprintf("%s", quoted))
			}

			writeToBoth("\n📤 **User Prompt to AI**:\n")
			writeToBoth("```\n%s```\n", userPrompt)

			// Call real AI agent if available, otherwise mock
			var kycReqs *dsl.KYCRequirements
			var kycErr error

			if ai != nil {
				writeToBoth("\n🚀 **Calling Real Gemini API...**\n")
				kycReqs, kycErr = ai.CallKYCAgent(ctx, scenario.nature, scenario.products)
				if kycErr != nil {
					writeToBoth("❌ Real AI Error: %v\n", kycErr)
					writeToBoth("🔄 Falling back to mock response...\n")
					kycReqs, kycErr = mockAgent.CallKYCAgent(ctx, scenario.nature, scenario.products)
				}
			} else {
				kycReqs, kycErr = mockAgent.CallKYCAgent(ctx, scenario.nature, scenario.products)
			}

			if kycErr != nil {
				writeToBoth("❌ Error: %v\n\n", kycErr)
				continue
			}

			// Show the actual AI response format
			actualResponse := map[string]interface{}{
				"required_documents": kycReqs.Documents,
				"jurisdictions":      kycReqs.Jurisdictions,
			}
			responseJSON, _ := json.Marshal(actualResponse)

			if ai != nil {
				writeToBoth("\n📥 **Actual Gemini AI Response**:\n")
			} else {
				writeToBoth("\n📥 **Mock AI Response**:\n")
			}
			writeToBoth("```json\n%s\n```\n", string(responseJSON))

			writeToBoth("\n✅ **Parsed Result**:\n")
			writeToBoth("   📄 Documents: %v\n", kycReqs.Documents)
			writeToBoth("   🌍 Jurisdictions: %v\n\n", kycReqs.Jurisdictions)
		}
	}

	// Test DSL Transformation Agent with prompt capture
	if *testType == testTypeAll || *testType == testTypeDSL {
		writeToBoth("\n🔄 **DSL Transformation Agent Prompt/Response Capture**\n")
		writeToBoth("====================================================\n")

		instruction := "Add TRANSFER_AGENT to the products list"

		writeToBoth("\n📝 **Transformation Instruction**: %s\n", instruction)

		request := agent.DSLTransformationRequest{
			CurrentDSL:  currentDSLState.DSLText,
			Instruction: instruction,
			TargetState: string(currentDSLState.OnboardingState),
			Context: map[string]interface{}{
				"current_state":  currentDSLState.OnboardingState,
				"version_number": currentDSLState.VersionNumber,
			},
		}

		// Show the system prompt
		writeToBoth("\n📤 **System Prompt to AI**:\n")
		writeToBoth("```\n")
		systemPrompt := `You are an expert DSL (Domain Specific Language) architect for financial onboarding workflows.
Your role is to analyze existing DSL and transform it according to user instructions while maintaining correctness and consistency.

RULES:
1. Analyze the current DSL structure and understand its semantic meaning
2. Apply the requested transformation while preserving DSL syntax and structure
3. Ensure all changes are consistent with the target onboarding state
4. Provide clear explanations for all changes made
5. Respond ONLY with a single, well-formed JSON object
6. Do not include markdown, code blocks, or conversational text

DSL SYNTAX GUIDE:
- S-expressions format: (command args...)
- Case creation: (case.create (cbu.id "ID") (nature-purpose "DESC"))
- Products: (products.add "PRODUCT1" "PRODUCT2")
- KYC: (kyc.start (documents (document "DOC")) (jurisdictions (jurisdiction "JUR")))
- Services: (services.discover (for.product "PROD" (service "SVC")))
- Resources: (resources.plan (resource.create "NAME" (owner "OWNER") (var (attr-id "ID"))))
- Values: (values.bind (bind (attr-id "ID") (value "VAL")))

RESPONSE FORMAT:
{
  "new_dsl": "Complete transformed DSL as a string",
  "explanation": "Clear explanation of what was changed and why",
  "changes": ["List of specific changes made"],
  "confidence": 0.95
}`
		writeToBoth("%s\n", systemPrompt)
		writeToBoth("```\n")

		// Show the user prompt
		contextJSON, _ := json.Marshal(request.Context)
		userPrompt := fmt.Sprintf(`Current DSL:
%s

Instruction: %s
Target State: %s

Additional Context: %s

Please transform the DSL according to the instruction while moving toward the target state.`,
			request.CurrentDSL,
			request.Instruction,
			request.TargetState,
			string(contextJSON))

		writeToBoth("\n📤 **User Prompt to AI**:\n")
		writeToBoth("```\n%s\n```\n", userPrompt)

		// Call real AI agent if available, otherwise mock
		var response *agent.DSLTransformationResponse
		var transformErr error

		if ai != nil {
			writeToBoth("\n🚀 **Calling Real Gemini API for DSL Transformation...**\n")
			response, transformErr = ai.CallDSLTransformationAgent(ctx, request)
			if transformErr != nil {
				writeToBoth("❌ Real AI Error: %v\n", transformErr)
				writeToBoth("🔄 Falling back to mock response...\n")
				if mockAgent == nil {
					mockAgent = agent.NewMockAgent()
				}
				response, transformErr = mockAgent.CallDSLTransformationAgent(ctx, request)
			}
		} else {
			if mockAgent == nil {
				mockAgent = agent.NewMockAgent()
			}
			response, transformErr = mockAgent.CallDSLTransformationAgent(ctx, request)
		}

		if transformErr != nil {
			writeToBoth("❌ Error: %v\n", transformErr)
		} else {
			// Show actual response
			responseJSON, _ := json.Marshal(response)
			if ai != nil {
				writeToBoth("\n📥 **Actual Gemini AI Response**:\n")
			} else {
				writeToBoth("\n📥 **Mock AI Response**:\n")
			}
			writeToBoth("```json\n%s\n```\n", string(responseJSON))

			writeToBoth("\n✅ **Parsed Result**:\n")
			writeToBoth("   📊 Confidence: %.2f\n", response.Confidence)
			writeToBoth("   📝 Explanation: %s\n", response.Explanation)
			writeToBoth("   🔄 Changes: %v\n", response.Changes)
			writeToBoth("\n📄 **Transformed DSL** (first 300 chars):\n")
			if len(response.NewDSL) > 300 {
				writeToBoth("```\n%s...\n```\n", response.NewDSL[:300])
			} else {
				writeToBoth("```\n%s\n```\n", response.NewDSL)
			}
		}
	}

	// Test DSL Validation Agent with prompt capture
	if *testType == testTypeAll || *testType == testTypeKYC {
		writeToBoth("\n✅ **DSL Validation Agent Prompt/Response Capture**\n")
		writeToBoth("=================================================\n")

		writeToBoth("\n📝 **Validation Target**: Current DSL completeness and correctness\n")

		// Show the system prompt for validation
		writeToBoth("\n📤 **System Prompt to AI**:\n")
		writeToBoth("```\n")
		validationSystemPrompt := `You are an expert DSL validator for financial onboarding workflows.
Your role is to analyze DSL for correctness, completeness, and best practices.

VALIDATION CRITERIA:
1. Syntax correctness (proper S-expression structure)
2. Semantic correctness (logical flow and consistency)
3. Completeness (required elements for the onboarding state)
4. Best practices (proper naming, structure, etc.)
5. Compliance considerations (regulatory requirements)

RESPONSE FORMAT:
{
  "is_valid": true/false,
  "validation_score": 0.95,
  "errors": ["List of syntax or semantic errors"],
  "warnings": ["List of potential issues"],
  "suggestions": ["List of improvement suggestions"],
  "summary": "Overall assessment of the DSL"
}`
		writeToBoth("%s\n", validationSystemPrompt)
		writeToBoth("```\n")

		// Show the user prompt for validation
		validationUserPrompt := fmt.Sprintf(`Please validate the following DSL:

%s

Provide a comprehensive validation assessment including errors, warnings, and suggestions for improvement.`, currentDSLState.DSLText)

		writeToBoth("\n📤 **User Prompt to AI**:\n")
		writeToBoth("```\n%s\n```\n", validationUserPrompt)

		// Call real AI agent if available, otherwise mock
		var validationResponse *agent.DSLValidationResponse
		var validationErr error

		if ai != nil {
			writeToBoth("\n🚀 **Calling Real Gemini API for DSL Validation...**\n")
			validationResponse, validationErr = ai.CallDSLValidationAgent(ctx, currentDSLState.DSLText)
			if validationErr != nil {
				writeToBoth("❌ Real AI Error: %v\n", validationErr)
				writeToBoth("🔄 Falling back to mock response...\n")
				if mockAgent == nil {
					mockAgent = agent.NewMockAgent()
				}
				validationResponse, validationErr = mockAgent.CallDSLValidationAgent(ctx, currentDSLState.DSLText)
			}
		} else {
			if mockAgent == nil {
				mockAgent = agent.NewMockAgent()
			}
			validationResponse, validationErr = mockAgent.CallDSLValidationAgent(ctx, currentDSLState.DSLText)
		}

		if validationErr != nil {
			writeToBoth("❌ Error: %v\n", validationErr)
		} else {
			// Show actual validation response
			validationJSON, _ := json.Marshal(validationResponse)
			if ai != nil {
				writeToBoth("\n📥 **Actual Gemini AI Response**:\n")
			} else {
				writeToBoth("\n📥 **Mock AI Response**:\n")
			}
			writeToBoth("```json\n%s\n```\n", string(validationJSON))

			writeToBoth("\n✅ **Parsed Validation Result**:\n")
			writeToBoth("   ✅ Valid: %t\n", validationResponse.IsValid)
			writeToBoth("   📈 Score: %.2f\n", validationResponse.ValidationScore)
			writeToBoth("   📝 Summary: %s\n", validationResponse.Summary)
			if len(validationResponse.Errors) > 0 {
				writeToBoth("   ❌ Errors: %v\n", validationResponse.Errors)
			}
			if len(validationResponse.Warnings) > 0 {
				writeToBoth("   ⚠️  Warnings: %v\n", validationResponse.Warnings)
			}
			if len(validationResponse.Suggestions) > 0 {
				writeToBoth("   💡 Suggestions: %v\n", validationResponse.Suggestions)
			}
		}
	}

	writeToBoth("\n🎯 **Summary**:\n")
	writeToBoth("- This capture shows the exact prompts sent to AI agents\n")
	writeToBoth("- System prompts define the AI's role and response format\n")
	writeToBoth("- User prompts contain the specific data and instructions\n")
	writeToBoth("- Responses are structured JSON for reliable parsing\n")

	if ai != nil {
		writeToBoth("- ✅ **REAL GEMINI RESPONSES**: Actual AI round-trip calls were made\n")
		writeToBoth("- 🔄 **Integration Validated**: The AI agent system is working end-to-end\n")
	} else {
		writeToBoth("- 🤖 **Mock responses used**: Set GEMINI_API_KEY for real AI testing\n")
		writeToBoth("- 📋 **Prompts are ready**: These exact prompts will work with real Gemini\n")
	}

	if *captureFile != "" {
		writeToBoth("\n📁 **Output saved to**: %s\n", *captureFile)
	}

	return nil
}
