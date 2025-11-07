package agent

import (
	"context"
	"fmt"
	"strings"

	"dsl-ob-poc/internal/dsl"
)

// MockAgent provides simulated AI responses for testing and demonstration
// NOTE: This contains hardcoded response patterns. For production, use database-driven
// rules and vocabulary to generate responses instead of hardcoded switch statements.
type MockAgent struct{}

// NewMockAgent creates a mock agent for testing without API keys
func NewMockAgent() *MockAgent {
	return &MockAgent{}
}

// CallKYCAgent simulates KYC discovery responses
func (m *MockAgent) CallKYCAgent(ctx context.Context, naturePurpose string, products []string) (*dsl.KYCRequirements, error) {
	// FIXME: Replace hardcoded KYC rules with database-driven requirements
	// These should come from product_requirements table and compliance_matrix
	var docs []string
	var jurisdictions []string

	// Analyze nature/purpose for entity type and domicile
	natureLower := strings.ToLower(naturePurpose)

	switch {
	case strings.Contains(natureLower, "ucits"):
		docs = append(docs, "CertificateOfIncorporation", "ArticlesOfAssociation", "W8BEN-E")
		if strings.Contains(natureLower, "lu") {
			jurisdictions = append(jurisdictions, "LU")
		} else {
			jurisdictions = append(jurisdictions, "EU")
		}
	case strings.Contains(natureLower, "hedge fund"):
		docs = append(docs, "CertificateOfLimitedPartnership", "PartnershipAgreement", "W9", "AMLPolicy")
		if strings.Contains(natureLower, "us") {
			jurisdictions = append(jurisdictions, "US")
		} else {
			jurisdictions = append(jurisdictions, "US", "CAYMAN")
		}
	case strings.Contains(natureLower, "corporation") || strings.Contains(natureLower, "company"):
		docs = append(docs, "CertificateOfIncorporation", "ArticlesOfAssociation")
		jurisdictions = append(jurisdictions, "US")
	default:
		// Default requirements
		docs = append(docs, "CertificateOfIncorporation", "ArticlesOfAssociation", "W8BEN-E")
		jurisdictions = append(jurisdictions, "US")
	}

	// FIXME: Replace hardcoded product requirements with database lookup
	// Should query product_requirements table for each product
	for _, product := range products {
		switch strings.ToUpper(product) {
		case "TRANSFER_AGENT":
			docs = append(docs, "AMLPolicy", "InvestorQuestionnaire")
		case "CUSTODY":
			docs = append(docs, "CustodyAgreement")
		case "FUND_ACCOUNTING":
			docs = append(docs, "AccountingPolicy")
		case "PRIME_BROKERAGE":
			docs = append(docs, "PrimeBrokerageAgreement", "RiskManagementPolicy")
		}
	}

	// Remove duplicates
	docs = uniqueStrings(docs)
	jurisdictions = uniqueStrings(jurisdictions)

	return &dsl.KYCRequirements{
		Documents:     docs,
		Jurisdictions: jurisdictions,
	}, nil
}

// CallDSLTransformationAgent simulates DSL transformation responses
// FIXME: Replace hardcoded transformation logic with database-driven vocabulary
func (m *MockAgent) CallDSLTransformationAgent(ctx context.Context, request DSLTransformationRequest) (*DSLTransformationResponse, error) {
	instruction := strings.ToLower(request.Instruction)

	// Simulate intelligent transformations based on instruction
	switch {
	case strings.Contains(instruction, "add") && strings.Contains(instruction, "product"):
		return m.simulateAddProduct(request)
	case strings.Contains(instruction, "add") && strings.Contains(instruction, "jurisdiction"):
		return m.simulateAddJurisdiction(request)
	case strings.Contains(instruction, "add") && strings.Contains(instruction, "document"):
		return m.simulateAddDocument(request)
	case strings.Contains(instruction, "change") && strings.Contains(instruction, "nature"):
		return m.simulateChangeNature(request)
	default:
		return m.simulateGenericTransformation(request)
	}
}

// CallDSLValidationAgent simulates DSL validation responses
func (m *MockAgent) CallDSLValidationAgent(ctx context.Context, dslToValidate string) (*DSLValidationResponse, error) {
	errors := []string{}
	warnings := []string{}
	suggestions := []string{}

	// Simulate validation logic
	if !strings.Contains(dslToValidate, "case.create") {
		errors = append(errors, "Missing required case.create block")
	}

	if !strings.Contains(dslToValidate, "cbu.id") {
		errors = append(errors, "Missing required cbu.id in case.create")
	}

	if !strings.Contains(dslToValidate, "products.add") {
		warnings = append(warnings, "No products defined - consider adding products")
		suggestions = append(suggestions, "Add products using (products.add \"PRODUCT_NAME\")")
	}

	if !strings.Contains(dslToValidate, "kyc.start") {
		warnings = append(warnings, "No KYC requirements defined")
		suggestions = append(suggestions, "Consider running 'discover-kyc' to generate KYC requirements")
	}

	if strings.Contains(dslToValidate, "nature-purpose") {
		suggestions = append(suggestions, "Ensure nature-purpose accurately reflects the entity type and domicile")
	}

	isValid := len(errors) == 0
	score := 1.0
	if len(errors) > 0 {
		score = 0.3
	} else if len(warnings) > 0 {
		score = 0.8
	}

	summary := "DSL structure is valid"
	if !isValid {
		summary = "DSL has structural errors that need to be addressed"
	} else if len(warnings) > 0 {
		summary = "DSL is valid but could be improved with additional onboarding steps"
	}

	return &DSLValidationResponse{
		IsValid:         isValid,
		ValidationScore: score,
		Errors:          errors,
		Warnings:        warnings,
		Suggestions:     suggestions,
		Summary:         summary,
	}, nil
}

// Helper functions for mock transformations
func (m *MockAgent) simulateAddProduct(request DSLTransformationRequest) (*DSLTransformationResponse, error) {
	instruction := strings.ToLower(request.Instruction)

	// Extract product name from instruction
	var productName string
	if strings.Contains(instruction, "fund_accounting") {
		productName = "FUND_ACCOUNTING"
	} else if strings.Contains(instruction, "transfer_agent") {
		productName = "TRANSFER_AGENT"
	} else if strings.Contains(instruction, "custody") {
		productName = "CUSTODY"
	} else if strings.Contains(instruction, "prime_brokerage") {
		productName = "PRIME_BROKERAGE"
	} else {
		productName = "NEW_PRODUCT"
	}

	// Simulate adding product to existing DSL
	newDSL := request.CurrentDSL
	if strings.Contains(newDSL, "(products.add") {
		// Add to existing products
		newDSL = strings.ReplaceAll(newDSL, "(products.add", fmt.Sprintf("(products.add \"%s\"", productName))
	} else {
		// Add new products block
		newDSL += fmt.Sprintf("\n\n(products.add \"%s\")", productName)
	}

	return &DSLTransformationResponse{
		NewDSL:      newDSL,
		Explanation: fmt.Sprintf("Added %s to the products list as requested", productName),
		Changes:     []string{fmt.Sprintf("Added %s product", productName)},
		Confidence:  0.9,
	}, nil
}

func (m *MockAgent) simulateAddJurisdiction(request DSLTransformationRequest) (*DSLTransformationResponse, error) {
	instruction := strings.ToLower(request.Instruction)

	// Extract jurisdiction from instruction
	var jurisdiction string
	if strings.Contains(instruction, "lu") || strings.Contains(instruction, "luxembourg") {
		jurisdiction = "LU"
	} else if strings.Contains(instruction, "us") || strings.Contains(instruction, "united states") {
		jurisdiction = "US"
	} else if strings.Contains(instruction, "uk") || strings.Contains(instruction, "united kingdom") {
		jurisdiction = "UK"
	} else {
		jurisdiction = "NEW_JURISDICTION"
	}

	// Simulate adding jurisdiction to KYC block
	newDSL := request.CurrentDSL
	if strings.Contains(newDSL, "(jurisdictions") {
		// Add to existing jurisdictions
		newDSL = strings.ReplaceAll(newDSL, "(jurisdictions", fmt.Sprintf("(jurisdictions\\n    (jurisdiction \"%s\")", jurisdiction))
	} else if strings.Contains(newDSL, "(kyc.start") {
		// Add jurisdictions block to existing KYC
		newDSL = strings.ReplaceAll(newDSL, "(kyc.start", fmt.Sprintf("(kyc.start\\n  (jurisdictions\\n    (jurisdiction \"%s\")\\n  )", jurisdiction))
	} else {
		// Add new KYC block
		newDSL += fmt.Sprintf("\n\n(kyc.start\\n  (jurisdictions\\n    (jurisdiction \"%s\")\\n  )\\n)", jurisdiction)
	}

	return &DSLTransformationResponse{
		NewDSL:      newDSL,
		Explanation: fmt.Sprintf("Added %s jurisdiction to KYC requirements", jurisdiction),
		Changes:     []string{fmt.Sprintf("Added %s jurisdiction", jurisdiction)},
		Confidence:  0.85,
	}, nil
}

func (m *MockAgent) simulateAddDocument(request DSLTransformationRequest) (*DSLTransformationResponse, error) {
	instruction := strings.ToLower(request.Instruction)

	// Extract document type from instruction
	var document string
	if strings.Contains(instruction, "w8ben") {
		document = "W8BEN-E"
	} else if strings.Contains(instruction, "w9") {
		document = "W9"
	} else if strings.Contains(instruction, "certificate") {
		document = "CertificateOfIncorporation"
	} else if strings.Contains(instruction, "articles") {
		document = "ArticlesOfAssociation"
	} else {
		document = "NewDocument"
	}

	// Simulate adding document to KYC block
	newDSL := request.CurrentDSL
	if strings.Contains(newDSL, "(documents") {
		// Add to existing documents
		newDSL = strings.ReplaceAll(newDSL, "(documents", fmt.Sprintf("(documents\\n    (document \"%s\")", document))
	} else if strings.Contains(newDSL, "(kyc.start") {
		// Add documents block to existing KYC
		newDSL = strings.ReplaceAll(newDSL, "(kyc.start", fmt.Sprintf("(kyc.start\\n  (documents\\n    (document \"%s\")\\n  )", document))
	} else {
		// Add new KYC block
		newDSL += fmt.Sprintf("\n\n(kyc.start\\n  (documents\\n    (document \"%s\")\\n  )\\n)", document)
	}

	return &DSLTransformationResponse{
		NewDSL:      newDSL,
		Explanation: fmt.Sprintf("Added %s document to KYC requirements", document),
		Changes:     []string{fmt.Sprintf("Added %s document", document)},
		Confidence:  0.9,
	}, nil
}

func (m *MockAgent) simulateChangeNature(request DSLTransformationRequest) (*DSLTransformationResponse, error) {
	instruction := strings.ToLower(request.Instruction)

	// Extract new nature from instruction
	var newNature string
	if strings.Contains(instruction, "hedge fund") {
		newNature = "US-based hedge fund"
	} else if strings.Contains(instruction, "ucits") {
		newNature = "UCITS equity fund domiciled in LU"
	} else if strings.Contains(instruction, "corporation") {
		newNature = "US corporation"
	} else {
		newNature = "Updated entity description"
	}

	// Simulate updating nature-purpose
	newDSL := request.CurrentDSL
	if strings.Contains(newDSL, "nature-purpose") {
		// Replace existing nature-purpose
		lines := strings.Split(newDSL, "\n")
		for i, line := range lines {
			if strings.Contains(line, "nature-purpose") {
				lines[i] = fmt.Sprintf("  (nature-purpose \"%s\")", newNature)
				break
			}
		}
		newDSL = strings.Join(lines, "\n")
	}

	return &DSLTransformationResponse{
		NewDSL:      newDSL,
		Explanation: fmt.Sprintf("Updated nature-purpose to: %s", newNature),
		Changes:     []string{"Modified nature-purpose field"},
		Confidence:  0.95,
	}, nil
}

func (m *MockAgent) simulateGenericTransformation(request DSLTransformationRequest) (*DSLTransformationResponse, error) {
	return &DSLTransformationResponse{
		NewDSL:      request.CurrentDSL + "\n\n; Transformation applied: " + request.Instruction,
		Explanation: "Applied generic transformation based on the instruction",
		Changes:     []string{"Added transformation comment"},
		Confidence:  0.6,
	}, nil
}

// Helper function to remove duplicates from string slice
func uniqueStrings(input []string) []string {
	seen := make(map[string]bool)
	result := []string{}

	for _, item := range input {
		if !seen[item] {
			seen[item] = true
			result = append(result, item)
		}
	}

	return result
}
