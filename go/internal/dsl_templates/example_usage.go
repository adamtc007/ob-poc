package dsl_templates

import (
	"fmt"
	"log"
)

// ExampleUsage demonstrates how to use the DSL Template Generator
func ExampleUsage() {
	// Create a new template generator
	templateGenerator := NewDSLTemplateGenerator()

	// Investor Domain Example
	investorParams := map[string]interface{}{
		"name": "Alice Johnson",
		"type": "PROPER_PERSON",
	}

	investorCreatedDSL, err := templateGenerator.GenerateDSL("investor", "CREATED", investorParams)
	if err != nil {
		log.Fatalf("Failed to generate investor created DSL: %v", err)
	}
	fmt.Println("Investor Created DSL:", investorCreatedDSL)

	// Hedge Fund Domain Example
	hedgeFundParams := map[string]interface{}{
		"name":     "Quantum Alpha Fund",
		"strategy": "LONG/SHORT",
	}

	hedgeFundCreatedDSL, err := templateGenerator.GenerateDSL("hedge-fund", "FUND_CREATED", hedgeFundParams)
	if err != nil {
		log.Fatalf("Failed to generate hedge fund created DSL: %v", err)
	}
	fmt.Println("Hedge Fund Created DSL:", hedgeFundCreatedDSL)

	// Trust Domain Example
	trustParams := map[string]interface{}{
		"type":    "REVOCABLE",
		"grantor": "John Smith",
	}

	trustCreatedDSL, err := templateGenerator.GenerateDSL("trust", "CREATED", trustParams)
	if err != nil {
		log.Fatalf("Failed to generate trust created DSL: %v", err)
	}
	fmt.Println("Trust Created DSL:", trustCreatedDSL)

	// More Complex Example with Multiple Parameters
	kycStartParams := map[string]interface{}{
		"document":     "Passport",
		"jurisdiction": "US",
		"risk_rating":  "LOW",
	}

	kycStartedDSL, err := templateGenerator.GenerateDSL("investor", "KYC_STARTED", kycStartParams)
	if err != nil {
		log.Fatalf("Failed to generate KYC started DSL: %v", err)
	}
	fmt.Println("KYC Started DSL:", kycStartedDSL)

	// Hedge Fund Subscription Example
	subscriptionParams := map[string]interface{}{
		"amount":   "1000000",
		"currency": "USD",
	}

	subscriptionDSL, err := templateGenerator.GenerateDSL("hedge-fund", "SUBSCRIPTION_STARTED", subscriptionParams)
	if err != nil {
		log.Fatalf("Failed to generate subscription DSL: %v", err)
	}
	fmt.Println("Subscription Started DSL:", subscriptionDSL)

	// Error Handling Example
	invalidParams := map[string]interface{}{
		"invalid": "parameter",
	}

	_, err = templateGenerator.GenerateDSL("investor", "CREATED", invalidParams)
	if err != nil {
		fmt.Println("Expected Error:", err)
	}
}

// Main method to run the example (optional) - removed to eliminate unused function warning
// func main() {
// 	ExampleUsage()
// }
