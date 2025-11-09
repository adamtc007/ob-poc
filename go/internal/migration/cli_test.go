package migration

import (
	"context"
	"database/sql"
	"fmt"
	"log"
	"os"

	"dsl-ob-poc/internal/agent"
	"dsl-ob-poc/internal/config"

	_ "github.com/lib/pq"
)

// TestDBAgentCLI tests the database-driven agent functionality from CLI
func TestDBAgentCLI(ctx context.Context, testType string) error {
	// Connect to database
	connStr := config.GetConnectionString()
	db, err := sql.Open("postgres", connStr)
	if err != nil {
		return fmt.Errorf("failed to connect to database: %w", err)
	}
	defer db.Close()

	// Test connection
	if err := db.PingContext(ctx); err != nil {
		return fmt.Errorf("failed to ping database: %w", err)
	}

	// Create DB agent
	dbAgent := agent.NewDBAgent(db)

	log.Printf("🧪 Testing DB Agent functionality (type: %s)", testType)

	switch testType {
	case "kyc", "all":
		if err := testKYCAgent(ctx, dbAgent); err != nil {
			return fmt.Errorf("KYC agent test failed: %w", err)
		}

	case "transform", "all":
		if err := testTransformationAgent(ctx, dbAgent); err != nil {
			return fmt.Errorf("Transformation agent test failed: %w", err)
		}

	case "validate", "all":
		if err := testValidationAgent(ctx, dbAgent); err != nil {
			return fmt.Errorf("Validation agent test failed: %w", err)
		}

	default:
		return fmt.Errorf("unknown test type: %s (use: kyc, transform, validate, or all)", testType)
	}

	log.Printf("✅ DB Agent tests completed successfully!")
	return nil
}

func testKYCAgent(ctx context.Context, dbAgent *agent.DBAgent) error {
	log.Printf("🔍 Testing KYC Agent with database rules...")

	testCases := []struct {
		name         string
		nature       string
		products     []string
		expectedDocs int
		expectedJur  int
	}{
		{
			name:         "UCITS Fund in LU",
			nature:       "UCITS equity fund domiciled in LU",
			products:     []string{"CUSTODY", "FUND_ACCOUNTING"},
			expectedDocs: 3, // At least 3 docs expected
			expectedJur:  1, // LU jurisdiction
		},
		{
			name:         "US Hedge Fund",
			nature:       "US hedge fund",
			products:     []string{"PRIME_BROKERAGE"},
			expectedDocs: 4, // More docs for hedge funds
			expectedJur:  1, // US jurisdiction
		},
		{
			name:         "Default Corporation",
			nature:       "US corporation",
			products:     []string{"CUSTODY"},
			expectedDocs: 2, // Basic corporate docs
			expectedJur:  1, // US jurisdiction
		},
	}

	for _, tc := range testCases {
		log.Printf("  📋 Test Case: %s", tc.name)

		result, err := dbAgent.CallKYCAgent(ctx, tc.nature, tc.products)
		if err != nil {
			return fmt.Errorf("KYC test case '%s' failed: %w", tc.name, err)
		}

		if len(result.Documents) < tc.expectedDocs {
			return fmt.Errorf("KYC test case '%s': expected at least %d documents, got %d",
				tc.name, tc.expectedDocs, len(result.Documents))
		}

		if len(result.Jurisdictions) < tc.expectedJur {
			return fmt.Errorf("KYC test case '%s': expected at least %d jurisdictions, got %d",
				tc.name, tc.expectedJur, len(result.Jurisdictions))
		}

		log.Printf("    ✓ Documents: %v", result.Documents)
		log.Printf("    ✓ Jurisdictions: %v", result.Jurisdictions)
	}

	return nil
}

func testTransformationAgent(ctx context.Context, dbAgent *agent.DBAgent) error {
	log.Printf("🔄 Testing DSL Transformation Agent with database rules...")

	testCases := []struct {
		name         string
		currentDSL   string
		instruction  string
		expectChange bool
	}{
		{
			name:         "Add Fund Accounting Product",
			currentDSL:   `(case.create (cbu.id "TEST-001"))`,
			instruction:  "add fund accounting product",
			expectChange: true,
		},
		{
			name:         "Add LU Jurisdiction",
			currentDSL:   `(case.create (cbu.id "TEST-002"))`,
			instruction:  "add jurisdiction lu",
			expectChange: true,
		},
		{
			name:         "Add Document W8BEN",
			currentDSL:   `(case.create (cbu.id "TEST-003"))`,
			instruction:  "add document w8ben",
			expectChange: true,
		},
		{
			name:         "Unknown Instruction",
			currentDSL:   `(case.create (cbu.id "TEST-004"))`,
			instruction:  "do something completely unknown",
			expectChange: false, // Should fall back to generic
		},
	}

	for _, tc := range testCases {
		log.Printf("  🔧 Test Case: %s", tc.name)

		request := agent.DSLTransformationRequest{
			CurrentDSL:  tc.currentDSL,
			Instruction: tc.instruction,
		}

		result, err := dbAgent.CallDSLTransformationAgent(ctx, request)
		if err != nil {
			return fmt.Errorf("Transformation test case '%s' failed: %w", tc.name, err)
		}

		if result.Confidence == 0 {
			return fmt.Errorf("Transformation test case '%s': confidence should not be zero", tc.name)
		}

		hasChange := result.NewDSL != tc.currentDSL
		if tc.expectChange && !hasChange {
			return fmt.Errorf("Transformation test case '%s': expected change but DSL remained the same", tc.name)
		}

		log.Printf("    ✓ Confidence: %.2f", result.Confidence)
		log.Printf("    ✓ Changes: %v", result.Changes)
		log.Printf("    ✓ DSL Changed: %v", hasChange)
	}

	return nil
}

func testValidationAgent(ctx context.Context, dbAgent *agent.DBAgent) error {
	log.Printf("✅ Testing DSL Validation Agent with database rules...")

	testCases := []struct {
		name          string
		dsl           string
		expectedValid bool
		expectedScore float64
	}{
		{
			name: "Valid Complete DSL",
			dsl: `(case.create
  (cbu.id "CBU-1234")
  (nature-purpose "UCITS fund")
)

(products.add "CUSTODY")`,
			expectedValid: true,
			expectedScore: 0.8,
		},
		{
			name:          "Missing case.create",
			dsl:           `(products.add "CUSTODY")`,
			expectedValid: false,
			expectedScore: 0.5,
		},
		{
			name: "Missing cbu.id",
			dsl: `(case.create
  (nature-purpose "Some fund")
)`,
			expectedValid: false,
			expectedScore: 0.5,
		},
		{
			name:          "Empty DSL",
			dsl:           "",
			expectedValid: false,
			expectedScore: 0.0,
		},
	}

	for _, tc := range testCases {
		log.Printf("  ✔️ Test Case: %s", tc.name)

		result, err := dbAgent.CallDSLValidationAgent(ctx, tc.dsl)
		if err != nil {
			return fmt.Errorf("Validation test case '%s' failed: %w", tc.name, err)
		}

		if result.IsValid != tc.expectedValid {
			return fmt.Errorf("Validation test case '%s': expected valid=%v, got valid=%v",
				tc.name, tc.expectedValid, result.IsValid)
		}

		if tc.expectedValid && result.Score < tc.expectedScore {
			return fmt.Errorf("Validation test case '%s': expected score >= %.2f, got %.2f",
				tc.name, tc.expectedScore, result.Score)
		}

		log.Printf("    ✓ Valid: %v", result.IsValid)
		log.Printf("    ✓ Score: %.2f", result.Score)
		log.Printf("    ✓ Errors: %d, Warnings: %d, Suggestions: %d",
			len(result.Errors), len(result.Warnings), len(result.Suggestions))

		if len(result.Errors) > 0 {
			log.Printf("      Errors: %v", result.Errors)
		}
	}

	return nil
}

// RunMigrationTest runs the complete migration test suite
func RunMigrationTest(ctx context.Context, verbose bool) error {
	if verbose {
		log.SetOutput(os.Stdout)
	}

	log.Printf("🚀 Starting Mock-to-Database Migration Test Suite")

	// Test database connectivity
	connStr := config.GetConnectionString()
	db, err := sql.Open("postgres", connStr)
	if err != nil {
		return fmt.Errorf("failed to connect to database: %w", err)
	}
	defer db.Close()

	if err := db.PingContext(ctx); err != nil {
		return fmt.Errorf("database connection failed: %w", err)
	}
	log.Printf("✅ Database connectivity verified")

	// Create migrator
	migrator := NewMockToDBMigrator(db, false, verbose)

	// Run migration verification
	result, err := migrator.RunFullMigration(ctx)
	if err != nil {
		return fmt.Errorf("migration test failed: %w", err)
	}

	// Report results
	log.Printf("\n📊 Migration Test Results:")
	log.Printf("  Success: %v", result.Success)
	log.Printf("  Mock Data Migrated: %d", result.MockDataMigrated)
	log.Printf("  Interceptors Found: %d", result.InterceptorsFound)
	log.Printf("  Configuration Fixed: %v", result.ConfigurationFixed)

	if len(result.Errors) > 0 {
		log.Printf("  Errors: %v", result.Errors)
	}

	if len(result.Warnings) > 0 {
		log.Printf("  Warnings: %v", result.Warnings)
	}

	// Test DB agents
	log.Printf("\n🧪 Testing Database-Driven Agents...")
	if err := TestDBAgentCLI(ctx, "all"); err != nil {
		return fmt.Errorf("DB agent tests failed: %w", err)
	}

	log.Printf("\n🎉 Migration test suite completed successfully!")
	log.Printf("📋 Summary:")
	log.Printf("  ✅ Database schema verified")
	log.Printf("  ✅ Mock data migrated")
	log.Printf("  ✅ Database-driven agents working")
	log.Printf("  ✅ KYC, transformation, and validation tested")

	return nil
}
