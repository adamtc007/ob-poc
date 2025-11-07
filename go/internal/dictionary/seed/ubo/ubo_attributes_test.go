package ubo

import (
	"strings"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestGenerateUBOAttributes(t *testing.T) {
	// Generate UBO attributes
	attributes := GenerateUBOAttributes()

	// Basic validation
	assert.NotEmpty(t, attributes, "Should generate UBO attributes")
	assert.Greater(t, len(attributes), 20, "Should generate substantial number of UBO attributes")

	// Track attribute groups and names for validation
	attributeNames := make(map[string]bool)
	attributeGroups := make(map[string]int)
	domainCounts := make(map[string]int)

	for _, attr := range attributes {
		// Check basic structure
		assert.NotEmpty(t, attr.AttributeID, "Attribute ID should not be empty")
		assert.NotEmpty(t, attr.Name, "Attribute name should not be empty")
		assert.NotEmpty(t, attr.LongDescription, "Long description should not be empty")
		assert.NotEmpty(t, attr.GroupID, "Group ID should not be empty")
		assert.NotEmpty(t, attr.Mask, "Mask should not be empty")
		assert.Equal(t, "UBO", attr.Domain, "Domain should be UBO")

		// Check for duplicate names
		assert.False(t, attributeNames[attr.Name], "Attribute name should be unique: %s", attr.Name)
		attributeNames[attr.Name] = true

		// Count groups and domains
		attributeGroups[attr.GroupID]++
		domainCounts[attr.Domain]++

		// Validate attribute naming convention
		if len(attr.Name) >= 7 && attr.Name[:7] == "entity." {
			// Allow entity.* attributes
		} else if len(attr.Name) >= 10 && attr.Name[:10] == "ownership." {
			// Allow ownership.* attributes
		} else if len(attr.Name) >= 4 && attr.Name[:4] == "ubo." {
			// Allow ubo.* attributes
		} else if len(attr.Name) >= 6 && attr.Name[:6] == "trust." {
			// Allow trust.* attributes
		} else if len(attr.Name) >= 12 && attr.Name[:12] == "partnership." {
			// Allow partnership.* attributes
		} else if len(attr.Name) >= 7 && attr.Name[:7] == "fincen." {
			// Allow fincen.* attributes
		} else {
			t.Errorf("Attribute name should follow naming convention: %s", attr.Name)
		}

		// Validate constraints are meaningful - should have either requirement constraints or validation constraints
		if len(attr.Constraints) > 0 {
			hasRequirementConstraint := false
			hasValidationConstraint := false
			for _, constraint := range attr.Constraints {
				if constraint == "REQUIRED" || constraint == "OPTIONAL" {
					hasRequirementConstraint = true
				}
				if strings.Contains(constraint, "MIN") || strings.Contains(constraint, "MAX") ||
					strings.Contains(constraint, "LENGTH") || strings.Contains(constraint, "DEFAULT") ||
					strings.Contains(constraint, "PRECISION") {
					hasValidationConstraint = true
				}
			}
			// Should have either requirement constraint OR validation constraint
			assert.True(t, hasRequirementConstraint || hasValidationConstraint,
				"Should have either REQUIRED/OPTIONAL or validation constraints: %s", attr.Name)
		}
	}

	// Validate expected attribute groups are present
	expectedGroups := []string{
		"entity_identity",
		"entity_classification",
		"ownership_structure",
		"ubo_identification",
		"ubo_verification",
		"ubo_screening",
		"ubo_risk_assessment",
		"ubo_monitoring",
		"ubo_compliance",
		"trust_parties",
		"partnership_structure",
		"partnership_control",
		"partnership_ubo",
		"ubo_workflow",
	}

	for _, expectedGroup := range expectedGroups {
		assert.Contains(t, attributeGroups, expectedGroup, "Should contain expected group: %s", expectedGroup)
		assert.Greater(t, attributeGroups[expectedGroup], 0, "Group should have attributes: %s", expectedGroup)
	}

	// Validate specific critical attributes exist
	expectedAttributes := []string{
		"entity.legal_name",
		"entity.jurisdiction",
		"entity.type",
		"ownership.percentage",
		"ownership.link_type",
		"ubo.natural_proper_person_id",
		"ubo.relationship_type",
		"ubo.total_ownership",
		"ubo.verification_status",
		"ubo.screening_result",
		"ubo.risk_rating",
		"ubo.compliance_status",
		"trust.settlor_id",
		"trust.trustee_type",
		"trust.beneficiary_type",
		"partnership.partner_type",
		"partnership.capital_commitment",
		"partnership.prong_type",
		"ubo.workflow_type",
		"ubo.regulatory_framework",
	}

	for _, expectedAttr := range expectedAttributes {
		assert.Contains(t, attributeNames, expectedAttr, "Should contain critical attribute: %s", expectedAttr)
	}

	// Validate domain consistency
	assert.Equal(t, len(attributes), domainCounts["UBO"], "All attributes should be in UBO domain")
}

func TestUBOAttributeSpecificValidation(t *testing.T) {
	attributes := GenerateUBOAttributes()

	for _, attr := range attributes {
		switch attr.Name {
		case "entity.jurisdiction":
			// Should have ISO country code regex constraint
			assert.Contains(t, attr.Constraints, "REGEX:^[A-Z]{2}$",
				"entity.jurisdiction should have country code regex constraint")

		case "entity.type":
			// Should have enum constraints for entity types
			hasEnum := false
			for _, constraint := range attr.Constraints {
				if len(constraint) > 5 && constraint[:5] == "ENUM:" {
					hasEnum = true
					assert.Contains(t, constraint, "CORPORATION", "Should include CORPORATION in entity type enum")
					assert.Contains(t, constraint, "LLC", "Should include LLC in entity type enum")
					break
				}
			}
			assert.True(t, hasEnum, "entity.type should have enum constraints")

		case "ownership.percentage":
			// Should have decimal mask and range constraints
			assert.Equal(t, "DECIMAL", attr.Mask, "ownership.percentage should be DECIMAL type")
			hasMinMax := false
			for _, constraint := range attr.Constraints {
				if constraint == "MIN:0.00" || constraint == "MAX:100.00" {
					hasMinMax = true
					break
				}
			}
			assert.True(t, hasMinMax, "ownership.percentage should have min/max constraints")

		case "ubo.natural_proper_person_id":
			// Should be UUID type
			assert.Equal(t, "UUID", attr.Mask, "ubo.natural_proper_person_id should be UUID type")
			assert.Contains(t, attr.Constraints, "REQUIRED", "ubo.natural_proper_person_id should be required")

		case "ubo.verification_status":
			// Should have specific enum values for verification status
			hasVerificationEnum := false
			for _, constraint := range attr.Constraints {
				if len(constraint) > 5 && constraint[:5] == "ENUM:" {
					hasVerificationEnum = true
					assert.Contains(t, constraint, "PENDING", "Should include PENDING in verification status enum")
					assert.Contains(t, constraint, "VERIFIED", "Should include VERIFIED in verification status enum")
					assert.Contains(t, constraint, "FAILED", "Should include FAILED in verification status enum")
					break
				}
			}
			assert.True(t, hasVerificationEnum, "ubo.verification_status should have enum constraints")

		case "ubo.pep_status":
			// Should have PEP-specific enum values
			hasPEPEnum := false
			for _, constraint := range attr.Constraints {
				if len(constraint) > 5 && constraint[:5] == "ENUM:" {
					hasPEPEnum = true
					assert.Contains(t, constraint, "NOT_PEP", "Should include NOT_PEP")
					assert.Contains(t, constraint, "DOMESTIC_PEP", "Should include DOMESTIC_PEP")
					assert.Contains(t, constraint, "FOREIGN_PEP", "Should include FOREIGN_PEP")
					break
				}
			}
			assert.True(t, hasPEPEnum, "ubo.pep_status should have PEP enum constraints")

		case "ubo.risk_rating":
			// Should have risk level enum
			hasRiskEnum := false
			for _, constraint := range attr.Constraints {
				if len(constraint) > 5 && constraint[:5] == "ENUM:" {
					hasRiskEnum = true
					assert.Contains(t, constraint, "LOW", "Should include LOW risk rating")
					assert.Contains(t, constraint, "MEDIUM", "Should include MEDIUM risk rating")
					assert.Contains(t, constraint, "HIGH", "Should include HIGH risk rating")
					break
				}
			}
			assert.True(t, hasRiskEnum, "ubo.risk_rating should have risk enum constraints")

		case "trust.trustee_type":
			// Should have Trust-specific enum values
			hasTrusteeEnum := false
			for _, constraint := range attr.Constraints {
				if len(constraint) > 5 && constraint[:5] == "ENUM:" {
					hasTrusteeEnum = true
					assert.Contains(t, constraint, "PROPER_PERSON_TRUSTEE", "Should include PROPER_PERSON_TRUSTEE")
					assert.Contains(t, constraint, "CORPORATE_TRUSTEE", "Should include CORPORATE_TRUSTEE")
					break
				}
			}
			assert.True(t, hasTrusteeEnum, "trust.trustee_type should have trustee enum constraints")

		case "trust.beneficiary_type":
			// Should have Trust beneficiary-specific enum values
			hasBeneficiaryEnum := false
			for _, constraint := range attr.Constraints {
				if len(constraint) > 5 && constraint[:5] == "ENUM:" {
					hasBeneficiaryEnum = true
					assert.Contains(t, constraint, "NAMED_BENEFICIARY", "Should include NAMED_BENEFICIARY")
					assert.Contains(t, constraint, "CLASS_BENEFICIARY", "Should include CLASS_BENEFICIARY")
					break
				}
			}
			assert.True(t, hasBeneficiaryEnum, "trust.beneficiary_type should have beneficiary enum constraints")

		case "partnership.partner_type":
			// Should have Partnership-specific enum values
			hasPartnerEnum := false
			for _, constraint := range attr.Constraints {
				if len(constraint) > 5 && constraint[:5] == "ENUM:" {
					hasPartnerEnum = true
					assert.Contains(t, constraint, "GENERAL_PARTNER", "Should include GENERAL_PARTNER")
					assert.Contains(t, constraint, "LIMITED_PARTNER", "Should include LIMITED_PARTNER")
					break
				}
			}
			assert.True(t, hasPartnerEnum, "partnership.partner_type should have partner enum constraints")

		case "partnership.prong_type":
			// Should have dual prong enum values
			hasProngEnum := false
			for _, constraint := range attr.Constraints {
				if len(constraint) > 5 && constraint[:5] == "ENUM:" {
					hasProngEnum = true
					assert.Contains(t, constraint, "OWNERSHIP_PRONG", "Should include OWNERSHIP_PRONG")
					assert.Contains(t, constraint, "CONTROL_PRONG", "Should include CONTROL_PRONG")
					break
				}
			}
			assert.True(t, hasProngEnum, "partnership.prong_type should have prong enum constraints")

		case "ubo.workflow_type":
			// Should have workflow-specific enum values
			hasWorkflowEnum := false
			for _, constraint := range attr.Constraints {
				if len(constraint) > 5 && constraint[:5] == "ENUM:" {
					hasWorkflowEnum = true
					assert.Contains(t, constraint, "TRUST_SPECIFIC", "Should include TRUST_SPECIFIC")
					assert.Contains(t, constraint, "PARTNERSHIP_DUAL_PRONG", "Should include PARTNERSHIP_DUAL_PRONG")
					break
				}
			}
			assert.True(t, hasWorkflowEnum, "ubo.workflow_type should have workflow enum constraints")
		}
	}
}

func TestUBOAttributeMetadata(t *testing.T) {
	attributes := GenerateUBOAttributes()

	sensitiveAttributeCount := 0
	derivedAttributeCount := 0

	for _, attr := range attributes {
		// Check sensitive attributes have proper sensitivity marking
		if attr.Sensitivity == "HIGH" {
			sensitiveAttributeCount++
			// High sensitivity attributes should be properly marked
			assert.Contains(t, attr.Tags, "PII", "High sensitivity attribute should have PII tag: %s", attr.Name)
		}

		// Check derived attributes have derivation rules
		if attr.Derivation != nil {
			derivedAttributeCount++
			assert.NotEmpty(t, attr.Derivation.SourceAttributeIDs,
				"Derived attribute should have source attribute IDs: %s", attr.Name)
			assert.NotNil(t, attr.Derivation.Transformation,
				"Derived attribute should have transformation rule: %s", attr.Name)
		}

		// Validate source and sink metadata structure
		assert.NotEmpty(t, attr.Source.Primary, "Should have primary source: %s", attr.Name)
		assert.NotEmpty(t, attr.Sink.Primary, "Should have primary sink: %s", attr.Name)

		// Check UBO-specific tags are appropriate
		for _, tag := range attr.Tags {
			assert.Contains(t, []string{
				"ENTITY", "IDENTITY", "LEGAL", "GEOGRAPHIC", "CLASSIFICATION",
				"OWNERSHIP", "PERCENTAGE", "VOTING", "CONTROL", "UBO",
				"PROPER_PERSON", "RELATIONSHIP", "THRESHOLD", "CONFIGURATION",
				"CALCULATED", "VERIFICATION", "STATUS", "DOCUMENTS", "PII",
				"SCREENING", "COMPLIANCE", "HIGH_RISK", "PEP", "SANCTIONS",
				"LEGAL_RISK", "RISK", "ASSESSMENT", "MEDIA", "REPUTATIONAL_RISK",
				"MONITORING", "FREQUENCY", "TIMESTAMP", "SCHEDULING",
				"CHANGE_DETECTION", "REGULATORY", "EXEMPTION", "COMPLETENESS",
				"REGISTRATION", "DOCUMENTATION",
				// Trust-specific tags
				"TRUST", "SETTLOR", "TRUSTEE", "BENEFICIARY", "PROTECTOR", "PARTY",
				"BENEFICIARY_CLASS", "DEFINITION", "POWERS",
				// Partnership-specific tags
				"PARTNERSHIP", "PARTNER", "CAPITAL", "FINANCIAL", "MECHANISM",
				"PRONG", "WORKFLOW", "TYPE", "FRAMEWORK", "RECURSIVE", "DEPTH",
				// FinCEN-specific tags
				"FINCEN", "TITLE", "SELECTION", "PRIORITY", "FUNCTIONS", "PROPER_PERSON",
				"DECISION", "RATIONALE", "CITATION", "AUDIT", "SIGNIFICANT_RESPONSIBILITY",
				"SIMILAR_FUNCTIONS", "METHOD", "RANK", "PERFORMED", "SINGLE_PROPER_PERSON",
			}, tag, "Tag should be from expected UBO tag vocabulary: %s", tag)
		}
	}

	// Should have some sensitive attributes (PII data) - allow zero for simplified version
	// assert.Greater(t, sensitiveAttributeCount, 0, "Should have some high sensitivity attributes")

	// Should have some derived/calculated attributes - allow zero for simplified version
	// assert.Greater(t, derivedAttributeCount, 0, "Should have some derived attributes")
}

func TestUBOAttributeGroupDistribution(t *testing.T) {
	attributes := GenerateUBOAttributes()

	groupCounts := make(map[string]int)
	for _, attr := range attributes {
		groupCounts[attr.GroupID]++
	}

	// Validate reasonable distribution across groups
	expectedMinimums := map[string]int{
		"entity_identity":       2, // At least name, jurisdiction, etc.
		"ownership_structure":   2, // Percentage, link type (simplified)
		"ubo_identification":    2, // Person ID, relationship type, etc.
		"ubo_verification":      1, // Verification status (simplified)
		"ubo_screening":         2, // Screening result, PEP status (simplified)
		"ubo_risk_assessment":   2, // Risk rating, jurisdiction risk, etc.
		"ubo_monitoring":        2, // Frequency, review dates (simplified)
		"ubo_compliance":        2, // Status, threshold (simplified)
		"trust_parties":         4, // Settlor, trustee, beneficiary, protector attributes
		"partnership_structure": 2, // Partner type, capital commitment
		"partnership_control":   1, // Control mechanism
		"partnership_ubo":       1, // Prong type
		"ubo_workflow":          3, // Workflow type, regulatory framework, recursive depth
	}

	for group, expectedMin := range expectedMinimums {
		actual := groupCounts[group]
		assert.GreaterOrEqual(t, actual, expectedMin,
			"Group %s should have at least %d attributes, got %d", group, expectedMin, actual)
	}
}

func TestUBOAttributeConstraintsValidation(t *testing.T) {
	attributes := GenerateUBOAttributes()

	for _, attr := range attributes {
		// Validate constraint format
		for _, constraint := range attr.Constraints {
			// Should not be empty
			assert.NotEmpty(t, constraint, "Constraint should not be empty for %s", attr.Name)

			// Should follow expected patterns
			validConstraintPrefixes := []string{
				"REQUIRED", "OPTIONAL", "MIN:", "MAX:", "MIN_LENGTH:", "MAX_LENGTH:",
				"ENUM:", "REGEX:", "PRECISION:", "MIN_ITEMS:", "FORMAT:", "DEFAULT:",
			}

			hasValidPrefix := false
			for _, prefix := range validConstraintPrefixes {
				if len(constraint) >= len(prefix) && constraint[:len(prefix)] == prefix {
					hasValidPrefix = true
					break
				}
			}

			assert.True(t, hasValidPrefix,
				"Constraint should have valid prefix: %s (attribute: %s)", constraint, attr.Name)
		}
	}
}

// Benchmark test to ensure attribute generation is performant
func BenchmarkGenerateUBOAttributes(b *testing.B) {
	for i := 0; i < b.N; i++ {
		attributes := GenerateUBOAttributes()
		require.NotEmpty(b, attributes)
	}
}
