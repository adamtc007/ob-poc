package entity_ubo_workflows

import (
	"context"
	"fmt"
	"log"
	"strings"
	"time"

	"dsl-ob-poc/internal/datastore"
	"dsl-ob-poc/internal/domains/ubo"
)

// Entity type constants
const (
	EntityTypeTrust              = "TRUST"
	EntityTypeLimitedPartnership = "LIMITED_PARTNERSHIP"
	EntityTypeHedgeFund          = "HEDGE_FUND"
	EntityTypePrivateEquity      = "PRIVATE_EQUITY"
	EntityTypeCorporation        = "CORPORATION"
	EntityTypeLLC                = "LLC"
)

// EntityUBOWorkflowsCommand demonstrates entity-type-specific UBO identification workflows
type EntityUBOWorkflowsCommand struct {
	datastore datastore.DataStore
}

// NewEntityUBOWorkflowsCommand creates a new entity UBO workflows command
func NewEntityUBOWorkflowsCommand(ds datastore.DataStore) *EntityUBOWorkflowsCommand {
	return &EntityUBOWorkflowsCommand{
		datastore: ds,
	}
}

// EntityUBORequest represents a request for entity-specific UBO identification
type EntityUBORequest struct {
	EntityName   string `json:"entity_name"`
	EntityType   string `json:"entity_type"` // TRUST, LIMITED_PARTNERSHIP, CORPORATION
	Jurisdiction string `json:"jurisdiction"`
	CBUID        string `json:"cbu_id"`
}

// EntityUBOResponse represents the response from entity-specific UBO identification
type EntityUBOResponse struct {
	WorkflowType     string                 `json:"workflow_type"`
	EntityType       string                 `json:"entity_type"`
	DSLGenerated     string                 `json:"dsl_generated"`
	WorkflowResults  map[string]interface{} `json:"workflow_results"`
	UBOsSummary      []UBOIdentified        `json:"ubos_summary"`
	RegulatoryStatus string                 `json:"regulatory_status"`
	ExecutedAt       time.Time              `json:"executed_at"`
}

// UBOIdentified represents an identified UBO from entity-specific workflow
type UBOIdentified struct {
	ProperPersonID     string  `json:"proper_person_id"`
	Name               string  `json:"name"`
	RelationshipType   string  `json:"relationship_type"`
	QualifyingReason   string  `json:"qualifying_reason"`
	OwnershipPercent   float64 `json:"ownership_percent,omitempty"`
	ControlType        string  `json:"control_type,omitempty"`
	VerificationStatus string  `json:"verification_status"`
	ScreeningResult    string  `json:"screening_result"`
	RiskSignificance   string  `json:"risk_significance"`
}

// RunEntityUBOWorkflows executes entity-type-specific UBO identification workflows
func (cmd *EntityUBOWorkflowsCommand) RunEntityUBOWorkflows(ctx context.Context, request EntityUBORequest) (*EntityUBOResponse, error) {
	log.Printf("🎯 Starting Entity-Type-Specific UBO Workflow")
	log.Printf("   Entity: %s (%s)", request.EntityName, request.EntityType)
	log.Printf("   Jurisdiction: %s", request.Jurisdiction)

	// Initialize UBO domain
	uboDomain := ubo.NewUBODomain(cmd.datastore)

	// Generate entity-type-specific DSL workflow
	var generatedDSL string
	var workflowType string

	switch strings.ToUpper(request.EntityType) {
	case EntityTypeTrust:
		generatedDSL = uboDomain.GenerateTrustUBOWorkflow(request.EntityName, request.Jurisdiction)
		workflowType = "TRUST_SPECIFIC_UBO"
		log.Printf("📋 Generated Trust-Specific UBO Workflow (FATF Compliant)")

	case EntityTypeLimitedPartnership, EntityTypeHedgeFund, EntityTypePrivateEquity:
		generatedDSL = uboDomain.GeneratePartnershipUBOWorkflow(request.EntityName, request.Jurisdiction)
		workflowType = "PARTNERSHIP_DUAL_PRONG_UBO"
		log.Printf("📋 Generated Partnership Dual-Prong UBO Workflow (EU 5MLD)")

	case EntityTypeCorporation, EntityTypeLLC:
		generatedDSL = uboDomain.GenerateSampleUBOWorkflow(request.EntityName, request.Jurisdiction)
		workflowType = "STANDARD_CORPORATE_UBO"
		log.Printf("📋 Generated Standard Corporate UBO Workflow")

	default:
		return nil, fmt.Errorf("unsupported entity type: %s", request.EntityType)
	}

	// Store the generated DSL
	err := cmd.storeDSLWorkflow(ctx, request.CBUID, generatedDSL, workflowType)
	if err != nil {
		log.Printf("⚠️  Warning: Failed to store DSL workflow: %v", err)
	}

	// Execute the entity-specific UBO workflow
	workflowResults, err := uboDomain.ExecuteDSL(ctx, generatedDSL)
	if err != nil {
		return nil, fmt.Errorf("failed to execute UBO workflow: %w", err)
	}

	// Extract UBOs based on entity type
	ubosSummary := cmd.extractUBOsSummary(workflowResults, request.EntityType)

	// Determine regulatory compliance status
	regulatoryStatus := cmd.determineRegulatoryStatus(request.EntityType, ubosSummary)

	// Display workflow execution results
	cmd.displayWorkflowResults(request, workflowType, ubosSummary, regulatoryStatus)

	response := &EntityUBOResponse{
		WorkflowType:     workflowType,
		EntityType:       request.EntityType,
		DSLGenerated:     generatedDSL,
		WorkflowResults:  workflowResults,
		UBOsSummary:      ubosSummary,
		RegulatoryStatus: regulatoryStatus,
		ExecutedAt:       time.Now(),
	}

	log.Printf("✅ Entity-Type-Specific UBO Workflow Completed")
	return response, nil
}

// extractUBOsSummary extracts UBO summary based on entity type and workflow results
func (cmd *EntityUBOWorkflowsCommand) extractUBOsSummary(workflowResults map[string]interface{}, entityType string) []UBOIdentified {
	var ubosSummary []UBOIdentified

	switch strings.ToUpper(entityType) {
	case EntityTypeTrust:
		// Extract Trust UBOs
		if trustUBOs, exists := workflowResults["trust_ubos"]; exists {
			if trustData, dataOK := trustUBOs.(map[string]interface{}); dataOK {
				if ubos, uboOK := trustData["trust_ubos"].([]map[string]interface{}); uboOK {
					for _, ubo := range ubos {
						ubosSummary = append(ubosSummary, UBOIdentified{
							ProperPersonID:     getStringValue(ubo, "proper_person_id"),
							Name:               getStringValue(ubo, "name"),
							RelationshipType:   getStringValue(ubo, "relationship_type"),
							QualifyingReason:   getStringValue(ubo, "qualifying_reason"),
							VerificationStatus: "REQUIRED",
							ScreeningResult:    "PENDING",
							RiskSignificance:   getStringValue(ubo, "risk_significance"),
						})
					}
				}
			}
		}

	case EntityTypeLimitedPartnership, EntityTypeHedgeFund, EntityTypePrivateEquity:
		// Extract Partnership UBOs (both ownership and control prongs)
		if partnershipUBOs, exists := workflowResults["partnership_ubos"]; exists {
			if partnershipData, dataOK := partnershipUBOs.(map[string]interface{}); dataOK {
				if combinedAnalysis, analysisOK := partnershipData["combined_analysis"].(map[string]interface{}); analysisOK {
					// Ownership prong UBOs
					if ownershipUBOs, ownershipOK := combinedAnalysis["ownership_prong_ubos"].([]map[string]interface{}); ownershipOK {
						for _, ubo := range ownershipUBOs {
							ubosSummary = append(ubosSummary, UBOIdentified{
								ProperPersonID:     getStringValue(ubo, "proper_person_id"),
								Name:               getStringValue(ubo, "name"),
								RelationshipType:   getStringValue(ubo, "relationship_type"),
								QualifyingReason:   getStringValue(ubo, "qualifying_reason"),
								OwnershipPercent:   getFloatValue(ubo, "ownership_percentage"),
								VerificationStatus: "REQUIRED",
								ScreeningResult:    "PENDING",
								RiskSignificance:   "HIGH",
							})
						}
					}
					// Control prong UBOs
					if controlUBOs, controlOK := combinedAnalysis["control_prong_ubos"].([]map[string]interface{}); controlOK {
						for _, ubo := range controlUBOs {
							ubosSummary = append(ubosSummary, UBOIdentified{
								ProperPersonID:     getStringValue(ubo, "proper_person_id"),
								Name:               getStringValue(ubo, "name"),
								RelationshipType:   getStringValue(ubo, "relationship_type"),
								QualifyingReason:   getStringValue(ubo, "qualifying_reason"),
								ControlType:        getStringValue(ubo, "control_type"),
								VerificationStatus: "REQUIRED",
								ScreeningResult:    "PENDING",
								RiskSignificance:   "VERY_HIGH",
							})
						}
					}
				}
			}
		}

	default:
		// Standard corporate UBO extraction
		if ubos, exists := workflowResults["ubos"]; exists {
			if uboData, dataOK := ubos.(map[string]interface{}); dataOK {
				if ubosList, listOK := uboData["ubos"].([]map[string]interface{}); listOK {
					for _, ubo := range ubosList {
						ubosSummary = append(ubosSummary, UBOIdentified{
							ProperPersonID:     getStringValue(ubo, "proper_person_id"),
							Name:               getStringValue(ubo, "name"),
							RelationshipType:   getStringValue(ubo, "relationship_type"),
							QualifyingReason:   getStringValue(ubo, "qualifying_reason"),
							OwnershipPercent:   getFloatValue(ubo, "total_ownership"),
							VerificationStatus: "REQUIRED",
							ScreeningResult:    "PENDING",
							RiskSignificance:   "MEDIUM",
						})
					}
				}
			}
		}
	}

	return ubosSummary
}

// determineRegulatoryStatus determines regulatory compliance status based on entity type and UBOs
func (cmd *EntityUBOWorkflowsCommand) determineRegulatoryStatus(entityType string, ubos []UBOIdentified) string {
	switch strings.ToUpper(entityType) {
	case EntityTypeTrust:
		// Trust UBO identification is FATF compliant if all parties identified
		if len(ubos) >= 2 { // At least settlor and trustee
			return "FATF_TRUST_COMPLIANT"
		}
		return "INCOMPLETE_TRUST_IDENTIFICATION"

	case EntityTypeLimitedPartnership, EntityTypeHedgeFund, EntityTypePrivateEquity:
		// Partnership is EU 5MLD compliant if both prongs analyzed
		ownershipProngFound := false
		controlProngFound := false

		for _, ubo := range ubos {
			if strings.Contains(ubo.RelationshipType, "OWNERSHIP") {
				ownershipProngFound = true
			}
			if strings.Contains(ubo.RelationshipType, "CONTROL") {
				controlProngFound = true
			}
		}

		if ownershipProngFound && controlProngFound {
			return "EU_5MLD_DUAL_PRONG_COMPLIANT"
		}
		return "INCOMPLETE_DUAL_PRONG_ANALYSIS"

	default:
		if len(ubos) > 0 {
			return "STANDARD_UBO_COMPLIANT"
		}
		return "NO_UBOS_IDENTIFIED"
	}
}

// displayWorkflowResults displays the results of entity-specific UBO workflow execution
func (cmd *EntityUBOWorkflowsCommand) displayWorkflowResults(request EntityUBORequest, workflowType string, ubos []UBOIdentified, regulatoryStatus string) {
	fmt.Println()
	fmt.Println("════════════════════════════════════════════════════════════════")
	fmt.Printf("🎯 ENTITY-TYPE-SPECIFIC UBO IDENTIFICATION RESULTS\n")
	fmt.Println("════════════════════════════════════════════════════════════════")
	fmt.Printf("Entity: %s\n", request.EntityName)
	fmt.Printf("Type: %s\n", request.EntityType)
	fmt.Printf("Workflow: %s\n", workflowType)
	fmt.Printf("Regulatory Status: %s\n", regulatoryStatus)
	fmt.Println()

	if len(ubos) == 0 {
		fmt.Println("❌ No UBOs identified through this workflow")
		return
	}

	fmt.Printf("✅ %d Ultimate Beneficial Owners Identified:\n", len(ubos))
	fmt.Println()

	for i, ubo := range ubos {
		fmt.Printf("%d. %s\n", i+1, ubo.Name)
		fmt.Printf("   └─ Person ID: %s\n", ubo.ProperPersonID)
		fmt.Printf("   └─ Relationship: %s\n", ubo.RelationshipType)
		fmt.Printf("   └─ Qualifying Reason: %s\n", ubo.QualifyingReason)

		if ubo.OwnershipPercent > 0 {
			fmt.Printf("   └─ Ownership: %.2f%%\n", ubo.OwnershipPercent)
		}

		if ubo.ControlType != "" {
			fmt.Printf("   └─ Control Type: %s\n", ubo.ControlType)
		}

		fmt.Printf("   └─ Risk Significance: %s\n", ubo.RiskSignificance)
		fmt.Printf("   └─ Status: Verification %s, Screening %s\n",
			ubo.VerificationStatus, ubo.ScreeningResult)
		fmt.Println()
	}

	// Display workflow-specific insights
	cmd.displayWorkflowInsights(request.EntityType, regulatoryStatus)
}

// displayWorkflowInsights displays entity-type-specific insights and next steps
func (cmd *EntityUBOWorkflowsCommand) displayWorkflowInsights(entityType, regulatoryStatus string) {
	fmt.Println("💡 Entity-Type-Specific Insights:")
	fmt.Println("─────────────────────────────────")

	switch strings.ToUpper(entityType) {
	case EntityTypeTrust:
		fmt.Println("• Trust UBO identification does NOT rely on 25% ownership thresholds")
		fmt.Println("• ALL trust parties must be identified: Settlors, Trustees, Beneficiaries, Protectors")
		fmt.Println("• Corporate Trustees require separate recursive UBO analysis")
		fmt.Println("• Beneficiary classes require ongoing monitoring for distributions")
		fmt.Println("• FATF guidance requires identification regardless of percentage")

	case EntityTypeLimitedPartnership, EntityTypeHedgeFund, EntityTypePrivateEquity:
		fmt.Println("• Partnership UBO uses DUAL PRONG analysis (ownership + control)")
		fmt.Println("• Ownership Prong: Limited Partners with ≥25% capital commitment")
		fmt.Println("• Control Prong: Natural persons controlling General Partner")
		fmt.Println("• General Partner entities require separate recursive UBO analysis")
		fmt.Println("• EU 5MLD requires BOTH prongs to be analyzed for compliance")

	default:
		fmt.Println("• Standard corporate UBO identification with 25% threshold")
		fmt.Println("• Recursive analysis through ownership chains")
		fmt.Println("• Both ownership and control prongs considered")
	}

	fmt.Println()
	fmt.Println("📋 Next Steps:")
	fmt.Println("─────────────")
	fmt.Println("1. Execute identity verification for all identified UBOs")
	fmt.Println("2. Perform sanctions and PEP screening")
	fmt.Println("3. Conduct risk assessment based on UBO profiles")
	fmt.Println("4. Set up ongoing monitoring for changes")
	if regulatoryStatus != "EU_5MLD_DUAL_PRONG_COMPLIANT" && regulatoryStatus != "FATF_TRUST_COMPLIANT" {
		fmt.Println("5. ⚠️ Complete additional analysis to achieve full regulatory compliance")
	}
	fmt.Println()
}

// storeDSLWorkflow stores the generated DSL workflow in the database
func (cmd *EntityUBOWorkflowsCommand) storeDSLWorkflow(ctx context.Context, cbuID, dsl, workflowType string) error {
	// In a real implementation, this would:
	// 1. Store the DSL in the dsl_ob table with appropriate metadata
	// 2. Link it to the CBU case
	// 3. Tag it with the workflow type for tracking
	// 4. Enable history tracking of DSL evolution

	log.Printf("📦 Storing DSL workflow (type: %s) for case: %s", workflowType, cbuID)
	// Mock storage - in real implementation would use cmd.datastore
	return nil
}

// Helper functions for safe type conversion
func getStringValue(m map[string]interface{}, key string) string {
	if val, exists := m[key]; exists {
		if str, strOK := val.(string); strOK {
			return str
		}
	}
	return ""
}

func getFloatValue(m map[string]interface{}, key string) float64 {
	if val, exists := m[key]; exists {
		if f, floatOK := val.(float64); floatOK {
			return f
		}
	}
	return 0.0
}

// RunEntityUBODemo runs a comprehensive demo of all entity-type-specific UBO workflows
func RunEntityUBODemo(ctx context.Context, ds datastore.DataStore) error {
	log.Println("🚀 Starting Comprehensive Entity-Type-Specific UBO Workflow Demo")

	cmd := NewEntityUBOWorkflowsCommand(ds)

	// Demo entities with different structures
	demoEntities := []EntityUBORequest{
		{
			EntityName:   "Smith Family Trust",
			EntityType:   EntityTypeTrust,
			Jurisdiction: "US",
			CBUID:        "CBU-TRUST-001",
		},
		{
			EntityName:   "Alpha Hedge Fund LP",
			EntityType:   EntityTypeLimitedPartnership,
			Jurisdiction: "GB",
			CBUID:        "CBU-HEDGE-001",
		},
		{
			EntityName:   "Beta Private Equity Fund",
			EntityType:   EntityTypePrivateEquity,
			Jurisdiction: "LU",
			CBUID:        "CBU-PE-001",
		},
		{
			EntityName:   "Standard Corporation Ltd",
			EntityType:   EntityTypeCorporation,
			Jurisdiction: "GB",
			CBUID:        "CBU-CORP-001",
		},
	}

	for i, entity := range demoEntities {
		log.Printf("\n📍 Demo %d/%d: %s", i+1, len(demoEntities), entity.EntityName)

		response, err := cmd.RunEntityUBOWorkflows(ctx, entity)
		if err != nil {
			log.Printf("❌ Demo failed for %s: %v", entity.EntityName, err)
			continue
		}

		log.Printf("✅ Demo completed for %s - %d UBOs identified",
			entity.EntityName, len(response.UBOsSummary))

		// Add separator between demos
		if i < len(demoEntities)-1 {
			fmt.Println("\n" + strings.Repeat("═", 80))
		}
	}

	log.Println("\n🎉 Entity-Type-Specific UBO Workflow Demo Complete!")
	log.Println("Key Takeaway: Different entity types require fundamentally different UBO workflows!")

	return nil
}
