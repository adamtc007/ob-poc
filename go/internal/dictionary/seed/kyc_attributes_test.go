package seed

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestGenerateKYCAttributes(t *testing.T) {
	attributes := GenerateKYCAttributes()

	assert.NotEmpty(t, attributes, "KYC attributes should not be empty")
	assert.Len(t, attributes, 10, "Expected 10 KYC attributes")

	requiredFields := []string{
		"AttributeID",
		"Name",
		"Domain",
		"Mask",
		"GroupID",
	}

	for _, attr := range attributes {
		t.Run(attr.Name, func(t *testing.T) {
			// Check required fields are populated
			for _, field := range requiredFields {
				assert.NotEmptyf(t, attr.Name, "%s field should not be empty", field)
			}

			// Validate AttributeID is a valid UUID
			assert.Len(t, attr.AttributeID, 36, "AttributeID should be a valid UUID")

			// Validate Domain
			assert.Contains(t, []string{"KYC"}, attr.Domain, "Domain should be 'KYC'")

			// Validate Sensitivity (optional)
			if attr.Sensitivity != "" {
				expectedSensitivities := []string{"HIGH", "MEDIUM", "LOW"}
				assert.Contains(t, expectedSensitivities, attr.Sensitivity, "Invalid sensitivity level")
			}

			// Check source and sink metadata
			assert.NotEmptyf(t, attr.Source.Primary, "Source primary for %s should not be empty", attr.Name)

			// Optional: More specific checks for individual attributes
			switch attr.Name {
			case "investor.type":
				assert.Contains(t, attr.Constraints, "ENUM:PROPER_PERSON,CORPORATE,INSTITUTIONAL",
					"investor.type should have specific enum constraints")
			case "investor.nationality":
				assert.Contains(t, attr.Constraints, "REGEX:^[A-Z]{2}$",
					"investor.nationality should have country code regex constraint")
			case "kyc.risk_rating":
				assert.Contains(t, attr.Constraints, "ENUM:LOW,MEDIUM,HIGH",
					"kyc.risk_rating should have specific enum constraints")
			}
		})
	}
}

func TestAttributeUniqueness(t *testing.T) {
	attributes := GenerateKYCAttributes()

	// Check for unique AttributeIDs
	attributeIDs := make(map[string]bool)
	for _, attr := range attributes {
		assert.False(t, attributeIDs[attr.AttributeID],
			"Duplicate AttributeID found: %s", attr.AttributeID)
		attributeIDs[attr.AttributeID] = true
	}

	// Check for unique Names
	attributeNames := make(map[string]bool)
	for _, attr := range attributes {
		assert.False(t, attributeNames[attr.Name],
			"Duplicate Attribute Name found: %s", attr.Name)
		attributeNames[attr.Name] = true
	}
}

func TestAttributeValidation(t *testing.T) {
	attributes := GenerateKYCAttributes()

	for _, attr := range attributes {
		t.Run(attr.Name+" Constraints", func(t *testing.T) {
			// Validate constraint formats
			for _, constraint := range attr.Constraints {
				assert.True(t,
					constraint == "REQUIRED" ||
						constraint == "OPTIONAL" ||
						constraint == "" ||
						startsWithAnyOf(constraint, []string{"MIN_LENGTH:", "MAX_LENGTH:", "REGEX:", "ENUM:"}),
					"Invalid constraint format: "+constraint)
			}

			// Validate tags
			if len(attr.Tags) > 0 {
				validTags := []string{
					"PII", "IDENTITY", "CLASSIFICATION", "COMPLIANCE",
					"RISK", "GEOGRAPHIC", "TAX_JURISDICTION", "WORKFLOW",
					"HIGH_RISK", "LEGAL_RISK", "FINANCIAL",
				}
				for _, tag := range attr.Tags {
					assert.Contains(t, validTags, tag, "Invalid tag: "+tag)
				}
			}
		})
	}
}

// Helper function to check if a string starts with any of the given prefixes
func startsWithAnyOf(str string, prefixes []string) bool {
	for _, prefix := range prefixes {
		if len(str) >= len(prefix) && str[:len(prefix)] == prefix {
			return true
		}
	}
	return false
}

func BenchmarkGenerateKYCAttributes(b *testing.B) {
	for i := 0; i < b.N; i++ {
		_ = GenerateKYCAttributes()
	}
}
