package cli

import (
	"context"
	"flag"
	"fmt"
	"log"
	"strings"
	"time"

	"dsl-ob-poc/internal/agent"
	"dsl-ob-poc/internal/cli/entity_ubo_workflows"
	"dsl-ob-poc/internal/datastore"
	"dsl-ob-poc/internal/domains/ubo"
)

// UBODiscoveryRequest represents a request for UBO discovery
type UBODiscoveryRequest struct {
	CBUID           string  `json:"cbu_id"`
	EntityName      string  `json:"entity_name"`
	Jurisdiction    string  `json:"jurisdiction"`
	EntityType      string  `json:"entity_type,omitempty"`
	Threshold       float64 `json:"threshold,omitempty"`
	RegulatoryFrame string  `json:"regulatory_framework,omitempty"`
}

// UBODiscoveryResponse represents the response from UBO discovery
type UBODiscoveryResponse struct {
	Status           string                 `json:"status"`
	UBOsIdentified   []UBOResult            `json:"ubos_identified"`
	RiskAssessment   map[string]interface{} `json:"risk_assessment"`
	ComplianceStatus string                 `json:"compliance_status"`
	GeneratedDSL     string                 `json:"generated_dsl"`
	Recommendations  []string               `json:"recommendations"`
	ExecutionTime    time.Duration          `json:"execution_time"`
}

// UBOResult represents an identified UBO
type UBOResult struct {
	ProperPersonID           string  `json:"proper_person_id"`
	Name               string  `json:"name"`
	RelationshipType   string  `json:"relationship_type"`
	OwnershipPercent   float64 `json:"ownership_percent"`
	ControlType        string  `json:"control_type,omitempty"`
	VerificationStatus string  `json:"verification_status"`
	ScreeningResult    string  `json:"screening_result"`
	RiskRating         string  `json:"risk_rating"`
}

// RunDiscoverUBO executes the UBO discovery workflow
func RunDiscoverUBO(ctx context.Context, ds datastore.DataStore, aiAgent *agent.Agent, args []string) error {
	// Check for entity-specific workflow flag
	if len(args) > 0 && args[0] == "--entity-workflows" {
		return RunEntityUBOWorkflowsDemo(ctx, ds, args[1:])
	}
	startTime := time.Now()

	// 1. Parse command line arguments
	request, dryRun, verbose, err := parseUBOFlags(args)
	if err != nil {
		return fmt.Errorf("failed to parse flags: %w", err)
	}

	if verbose {
		log.Printf("🚀 Starting UBO Discovery for %s (%s)", request.EntityName, request.Jurisdiction)
		log.Printf("📋 Parameters: Threshold=%.1f%%, Framework=%s", request.Threshold, request.RegulatoryFrame)
	}

	// 2. Get existing DSL (current state) - this will fail if case doesn't exist
	existingDSL, err := ds.GetLatestDSL(ctx, request.CBUID)
	if err != nil {
		if strings.Contains(err.Error(), "not found") || strings.Contains(err.Error(), "no rows") {
			return fmt.Errorf("case %s does not exist. Please create it first with 'dsl-poc create'", request.CBUID)
		}
		return fmt.Errorf("failed to get existing DSL: %w", err)
	}

	if verbose {
		log.Printf("📄 Current DSL length: %d characters", len(existingDSL))
	}

	// 4. Generate UBO discovery DSL workflow
	uboDomain := ubo.NewUBODomain(ds)
	uboWorkflowDSL := uboDomain.GenerateSampleUBOWorkflow(request.EntityName, request.Jurisdiction)

	// 5. If dry run, show what would be executed
	if dryRun {
		fmt.Println("🔍 DRY RUN - UBO Discovery Workflow:")
		fmt.Println("=====================================")
		fmt.Println(uboWorkflowDSL)
		fmt.Println("=====================================")
		fmt.Printf("✅ Dry run complete. This workflow would be added to case %s\n", request.CBUID)
		return nil
	}

	// 6. Execute UBO workflow using AI agent (if available)
	var newDSLFragment string

	if aiAgent == nil {
		// Fallback to direct DSL generation
		if verbose {
			log.Printf("⚠️  AI agent not available, using direct DSL generation")
		}
		newDSLFragment = combineExistingAndNewDSL(existingDSL, uboWorkflowDSL)
	} else {
		// Use AI agent for enhanced UBO discovery
		if verbose {
			log.Printf("🤖 Using AI agent for UBO discovery")
		}

		transformRequest := agent.DSLTransformationRequest{
			CurrentDSL: existingDSL,
			Instruction: fmt.Sprintf("Add comprehensive UBO discovery workflow for entity '%s' in jurisdiction '%s' with %.1f%% ownership threshold",
				request.EntityName, request.Jurisdiction, request.Threshold),
			TargetState: "UBO_DISCOVERY_COMPLETE",
			Context: map[string]interface{}{
				"entity_name":          request.EntityName,
				"jurisdiction":         request.Jurisdiction,
				"entity_type":          request.EntityType,
				"ownership_threshold":  request.Threshold,
				"regulatory_framework": request.RegulatoryFrame,
			},
		}

		transformResponse, err := aiAgent.CallDSLTransformationAgent(ctx, transformRequest)
		if err != nil {
			// Fallback to template-based generation
			if verbose {
				log.Printf("⚠️  AI transformation failed, using template: %v", err)
			}
			newDSLFragment = combineExistingAndNewDSL(existingDSL, uboWorkflowDSL)
		} else {
			newDSLFragment = transformResponse.NewDSL
		}
	}

	// 7. Execute UBO domain logic
	domainResults, err := uboDomain.ExecuteDSL(ctx, newDSLFragment)
	if err != nil {
		log.Printf("⚠️  UBO domain execution warning: %v", err)
		// Continue with DSL storage even if domain execution has issues
	}

	// 8. Store the new DSL version
	if verbose {
		log.Printf("💾 Storing UBO discovery DSL")
	}

	_, err = ds.InsertDSL(ctx, request.CBUID, newDSLFragment)
	if err != nil {
		return fmt.Errorf("failed to store UBO discovery DSL: %w", err)
	}

	// 9. Generate response with results
	response := &UBODiscoveryResponse{
		Status:           "completed",
		GeneratedDSL:     newDSLFragment,
		ExecutionTime:    time.Since(startTime),
		ComplianceStatus: "pending_verification",
	}

	// Parse UBO results from domain execution
	if domainResults != nil {
		if ubos, ok := domainResults["ubos"].(map[string]interface{}); ok {
			response.UBOsIdentified = parseUBOResults(ubos)
		}
		if riskData, ok := domainResults["risk_assessment"].(map[string]interface{}); ok {
			response.RiskAssessment = riskData
		}
	}

	// Add recommendations based on execution
	response.Recommendations = generateUBORecommendations(request, response)

	// 10. Display results
	displayUBOResults(response, verbose)

	if verbose {
		log.Printf("⏱️  Total execution time: %v", response.ExecutionTime)
		log.Printf("📊 UBO discovery DSL stored successfully")
	}

	fmt.Printf("✅ UBO discovery completed for case %s\n", request.CBUID)
	fmt.Printf("📋 Use 'dsl-poc history --cbu=%s' to view the complete DSL evolution\n", request.CBUID)

	return nil
}

// RunEntityUBOWorkflowsDemo executes the entity-type-specific UBO workflows demonstration
func RunEntityUBOWorkflowsDemo(ctx context.Context, ds datastore.DataStore, args []string) error {
	fs := flag.NewFlagSet("entity-ubo-workflows", flag.ContinueOnError)

	var (
		entityName   = fs.String("entity", "", "Entity name for specific workflow (optional)")
		entityType   = fs.String("type", "", "Entity type: TRUST, LIMITED_PARTNERSHIP, CORPORATION (optional)")
		jurisdiction = fs.String("jurisdiction", "GB", "Jurisdiction code")
		cbuID        = fs.String("cbu", "", "CBU ID (optional)")
	)

	if err := fs.Parse(args); err != nil {
		return fmt.Errorf("failed to parse flags: %w", err)
	}

	// If specific entity provided, run single workflow
	if *entityName != "" && *entityType != "" {
		return runSingleEntityUBOWorkflow(ctx, ds, *entityName, *entityType, *jurisdiction, *cbuID)
	}

	// Otherwise, run comprehensive demo
	log.Println("🎯 Running comprehensive entity-type-specific UBO workflows demo...")
	return entity_ubo_workflows.RunEntityUBODemo(ctx, ds)
}

// runSingleEntityUBOWorkflow runs UBO workflow for a specific entity
func runSingleEntityUBOWorkflow(ctx context.Context, ds datastore.DataStore, entityName, entityType, jurisdiction, cbuID string) error {
	if cbuID == "" {
		cbuID = fmt.Sprintf("CBU-%s-001", strings.ToUpper(entityType[:4]))
	}

	return entity_ubo_workflows.RunEntityUBODemo(ctx, ds)
}

// parseUBOFlags parses command line flags into UBODiscoveryRequest
func parseUBOFlags(args []string) (*UBODiscoveryRequest, bool, bool, error) {
	fs := flag.NewFlagSet("discover-ubo", flag.ExitOnError)

	// Required flags
	cbu := fs.String("cbu", "", "CBU ID for the client case (required)")
	entity := fs.String("entity", "", "Legal name of the entity (required)")
	jurisdiction := fs.String("jurisdiction", "", "Jurisdiction of incorporation (ISO 3166-1 alpha-2) (required)")

	// Optional flags
	entityType := fs.String("entity-type", "CORPORATION", "Type of entity (CORPORATION, LLC, PARTNERSHIP, TRUST, etc.)")
	threshold := fs.Float64("threshold", 25.0, "Ownership threshold percentage for UBO identification")
	framework := fs.String("framework", "EU_5MLD", "Regulatory framework (EU_5MLD, US_FINCEN, UK_PSC, FATF)")
	dryRun := fs.Bool("dry-run", false, "Show what would be executed without making changes")
	verbose := fs.Bool("verbose", false, "Show detailed execution logs")

	if err := fs.Parse(args); err != nil {
		return nil, false, false, fmt.Errorf("failed to parse flags: %w", err)
	}

	// Validate required fields
	if *cbu == "" || *entity == "" || *jurisdiction == "" {
		fs.Usage()
		fmt.Println("\nRequired flags:")
		fmt.Println("  --cbu           CBU ID for the client case")
		fmt.Println("  --entity        Legal name of the entity")
		fmt.Println("  --jurisdiction  Jurisdiction of incorporation (ISO 3166-1 alpha-2)")
		fmt.Println("\nOptional flags:")
		fmt.Println("  --entity-type   Type of entity (default: CORPORATION)")
		fmt.Println("  --threshold     Ownership threshold % (default: 25.0)")
		fmt.Println("  --framework     Regulatory framework (default: EU_5MLD)")
		fmt.Println("  --dry-run       Show what would be executed")
		fmt.Println("  --verbose       Show detailed logs")
		fmt.Println("\nExamples:")
		fmt.Println("  dsl-poc discover-ubo --cbu=CBU-1234 --entity=\"Acme Holdings Ltd\" --jurisdiction=GB")
		fmt.Println("  dsl-poc discover-ubo --cbu=CBU-5678 --entity=\"Global SA\" --jurisdiction=LU --threshold=30.0")
		return nil, false, false, fmt.Errorf("cbu, entity, and jurisdiction are required")
	}

	// Validate jurisdiction format (ISO 3166-1 alpha-2)
	if len(*jurisdiction) != 2 {
		return nil, false, false, fmt.Errorf("jurisdiction must be 2-letter ISO 3166-1 alpha-2 code (e.g., US, GB, LU)")
	}

	// Validate threshold range
	if *threshold < 0.01 || *threshold > 100.0 {
		return nil, false, false, fmt.Errorf("threshold must be between 0.01 and 100.0")
	}

	request := &UBODiscoveryRequest{
		CBUID:           *cbu,
		EntityName:      *entity,
		Jurisdiction:    *jurisdiction,
		EntityType:      *entityType,
		Threshold:       *threshold,
		RegulatoryFrame: *framework,
	}

	return request, *dryRun, *verbose, nil
}

// combineExistingAndNewDSL combines existing DSL with new UBO workflow DSL
func combineExistingAndNewDSL(existingDSL, newUBODSL string) string {
	if existingDSL == "" {
		return newUBODSL
	}
	return existingDSL + "\n\n; === UBO DISCOVERY WORKFLOW ===\n" + newUBODSL
}

// parseUBOResults converts domain execution results to UBOResult structs
func parseUBOResults(ubos map[string]interface{}) []UBOResult {
	var results []UBOResult

	if uboList, ok := ubos["ubos"].([]map[string]interface{}); ok {
		for _, uboData := range uboList {
			result := UBOResult{}

			if personID, ok := uboData["proper_person_id"].(string); ok {
				result.ProperPersonID = personID
			}
			if name, ok := uboData["name"].(string); ok {
				result.Name = name
			}
			if relType, ok := uboData["relationship_type"].(string); ok {
				result.RelationshipType = relType
			}
			if ownership, ok := uboData["total_ownership"].(float64); ok {
				result.OwnershipPercent = ownership
			}
			if controlType, ok := uboData["control_type"].(string); ok {
				result.ControlType = controlType
			}

			// Default values for verification and screening
			result.VerificationStatus = "pending"
			result.ScreeningResult = "pending"
			result.RiskRating = "medium"

			results = append(results, result)
		}
	}

	return results
}

// generateUBORecommendations creates recommendations based on UBO discovery results
func generateUBORecommendations(request *UBODiscoveryRequest, response *UBODiscoveryResponse) []string {
	var recommendations []string

	// Basic recommendations
	recommendations = append(recommendations, "Verify identity documents for all identified UBOs")
	recommendations = append(recommendations, "Conduct sanctions and PEP screening for each UBO")

	// Threshold-specific recommendations
	if request.Threshold < 25.0 {
		recommendations = append(recommendations, "Consider reviewing lower threshold compliance with local regulations")
	}

	// Jurisdiction-specific recommendations
	switch request.Jurisdiction {
	case "US":
		recommendations = append(recommendations, "Ensure compliance with FinCEN Customer Due Diligence requirements")
	case "GB":
		recommendations = append(recommendations, "Verify against UK PSC (Persons with Significant Control) register")
	case "LU":
		recommendations = append(recommendations, "Apply Luxembourg AML/CFT requirements for beneficial ownership")
	default:
		recommendations = append(recommendations, "Review local AML/CFT requirements for UBO identification")
	}

	// Risk-based recommendations
	if len(response.UBOsIdentified) == 0 {
		recommendations = append(recommendations, "⚠️  No UBOs identified - manual review required")
	} else if len(response.UBOsIdentified) > 5 {
		recommendations = append(recommendations, "Complex ownership structure detected - consider enhanced due diligence")
	}

	// Ongoing monitoring
	recommendations = append(recommendations, "Set up ongoing monitoring for ownership changes")
	recommendations = append(recommendations, "Schedule periodic UBO data refresh (recommended: quarterly)")

	return recommendations
}

// displayUBOResults presents the UBO discovery results in a user-friendly format
func displayUBOResults(response *UBODiscoveryResponse, verbose bool) {
	fmt.Println("\n🎯 UBO Discovery Results")
	fmt.Println("========================")

	if len(response.UBOsIdentified) > 0 {
		fmt.Printf("📊 Identified %d Ultimate Beneficial Owner(s):\n\n", len(response.UBOsIdentified))

		for i, ubo := range response.UBOsIdentified {
			fmt.Printf("%d. %s\n", i+1, ubo.Name)
			fmt.Printf("   └─ ID: %s\n", ubo.ProperPersonID)
			fmt.Printf("   └─ Relationship: %s\n", ubo.RelationshipType)

			if ubo.OwnershipPercent > 0 {
				fmt.Printf("   └─ Ownership: %.2f%%\n", ubo.OwnershipPercent)
			}
			if ubo.ControlType != "" {
				fmt.Printf("   └─ Control: %s\n", ubo.ControlType)
			}
			fmt.Printf("   └─ Status: Verification %s, Screening %s\n",
				ubo.VerificationStatus, ubo.ScreeningResult)
			fmt.Println()
		}
	} else {
		fmt.Println("⚠️  No UBOs identified above the specified threshold")
	}

	// Show compliance status
	fmt.Printf("📋 Compliance Status: %s\n", response.ComplianceStatus)

	// Show recommendations
	if len(response.Recommendations) > 0 {
		fmt.Println("\n💡 Recommendations:")
		for i, rec := range response.Recommendations {
			fmt.Printf("%d. %s\n", i+1, rec)
		}
	}

	// Show DSL snippet in verbose mode
	if verbose && response.GeneratedDSL != "" {
		fmt.Println("\n📜 Generated DSL (snippet):")
		fmt.Println("─────────────────────────────")
		lines := strings.Split(response.GeneratedDSL, "\n")
		maxLines := 10
		if len(lines) > maxLines {
			for i := 0; i < maxLines; i++ {
				fmt.Println(lines[i])
			}
			fmt.Printf("... (%d more lines)\n", len(lines)-maxLines)
		} else {
			fmt.Println(response.GeneratedDSL)
		}
		fmt.Println("─────────────────────────────")
	}
}
