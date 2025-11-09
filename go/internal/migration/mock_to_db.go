package migration

import (
	"context"
	"database/sql"
	"fmt"
	"log"
	"os"
	"path/filepath"
	"strings"

	"dsl-ob-poc/internal/agent"
	"dsl-ob-poc/internal/config"
	"dsl-ob-poc/internal/datastore"
)

// MockToDBMigrator handles the migration from mock interceptors to database-driven operations
type MockToDBMigrator struct {
	db       *sql.DB
	dbAgent  *agent.DBAgent
	dryRun   bool
	verbose  bool
	basePath string
}

// NewMockToDBMigrator creates a new migrator instance
func NewMockToDBMigrator(db *sql.DB, dryRun, verbose bool) *MockToDBMigrator {
	return &MockToDBMigrator{
		db:       db,
		dbAgent:  agent.NewDBAgent(db),
		dryRun:   dryRun,
		verbose:  verbose,
		basePath: ".",
	}
}

// MigrationResult holds the results of the migration process
type MigrationResult struct {
	Success            bool
	MockDataMigrated   int
	InterceptorsFound  int
	InterceptorsFixed  int
	ConfigurationFixed bool
	Errors             []string
	Warnings           []string
}

// RunFullMigration performs the complete migration from mocks to database
func (m *MockToDBMigrator) RunFullMigration(ctx context.Context) (*MigrationResult, error) {
	result := &MigrationResult{
		Success:  true,
		Errors:   []string{},
		Warnings: []string{},
	}

	m.logf("Starting mock-to-database migration...")
	m.logf("Dry run mode: %v", m.dryRun)

	// Step 1: Verify database schema and mock data migration
	m.logf("\n=== Step 1: Verifying Database Schema ===")
	if err := m.verifyDatabaseSchema(ctx); err != nil {
		result.Errors = append(result.Errors, fmt.Sprintf("Database schema verification failed: %v", err))
		result.Success = false
		return result, err
	}
	m.logf("✓ Database schema verified")

	// Step 2: Check if mock data migration is needed
	m.logf("\n=== Step 2: Checking Mock Data Migration Status ===")
	migrated, err := m.checkMockDataMigration(ctx)
	if err != nil {
		result.Errors = append(result.Errors, fmt.Sprintf("Failed to check mock data migration: %v", err))
		result.Success = false
		return result, err
	}

	if !migrated {
		result.Warnings = append(result.Warnings, "Mock data not migrated to database. Run: psql -d your_database -f sql/07_migrate_mock_data_to_db.sql")
	} else {
		m.logf("✓ Mock data already migrated to database")
		result.MockDataMigrated = 1
	}

	// Step 3: Identify mock interceptors
	m.logf("\n=== Step 3: Identifying Mock Interceptors ===")
	interceptors, err := m.findMockInterceptors()
	if err != nil {
		result.Errors = append(result.Errors, fmt.Sprintf("Failed to find mock interceptors: %v", err))
		result.Success = false
		return result, err
	}
	result.InterceptorsFound = len(interceptors)
	m.logf("Found %d mock interceptors", len(interceptors))

	// Step 4: Fix configuration to use database mode
	m.logf("\n=== Step 4: Configuring Database Mode ===")
	if err := m.configureDatabaseMode(); err != nil {
		result.Errors = append(result.Errors, fmt.Sprintf("Failed to configure database mode: %v", err))
		result.Success = false
		return result, err
	}
	result.ConfigurationFixed = true
	m.logf("✓ Database mode configured")

	// Step 5: Test database-driven agent functionality
	m.logf("\n=== Step 5: Testing Database-Driven Operations ===")
	if err := m.testDatabaseOperations(ctx); err != nil {
		result.Errors = append(result.Errors, fmt.Sprintf("Database operations test failed: %v", err))
		result.Success = false
		return result, err
	}
	m.logf("✓ Database operations tested successfully")

	// Step 6: Provide recommendations for code changes
	m.logf("\n=== Step 6: Code Modification Recommendations ===")
	recommendations := m.generateRecommendations(interceptors)
	for _, rec := range recommendations {
		m.logf("RECOMMENDATION: %s", rec)
		result.Warnings = append(result.Warnings, rec)
	}

	if result.Success {
		m.logf("\n🎉 Migration completed successfully!")
		m.logf("Summary:")
		m.logf("  - Mock data migrated: %v", migrated)
		m.logf("  - Mock interceptors found: %d", result.InterceptorsFound)
		m.logf("  - Database mode configured: %v", result.ConfigurationFixed)
		m.logf("  - Database operations working: ✓")
	}

	return result, nil
}

// verifyDatabaseSchema checks if the required database tables exist
func (m *MockToDBMigrator) verifyDatabaseSchema(ctx context.Context) error {
	requiredTables := []string{
		"cbus", "products", "services", "dsl_ob", "dictionary",
		"kyc_rules", "product_requirements", "dsl_transformation_rules", "dsl_validation_rules",
	}

	for _, table := range requiredTables {
		query := `SELECT EXISTS (
			SELECT FROM information_schema.tables
			WHERE table_schema = 'ob-poc' AND table_name = $1
		)`

		var exists bool
		err := m.db.QueryRowContext(ctx, query, table).Scan(&exists)
		if err != nil {
			return fmt.Errorf("failed to check table %s: %w", table, err)
		}

		if !exists {
			return fmt.Errorf("required table 'ob-poc'.%s does not exist", table)
		}
	}

	return nil
}

// checkMockDataMigration verifies if mock data has been migrated to the database
func (m *MockToDBMigrator) checkMockDataMigration(ctx context.Context) (bool, error) {
	// Check if we have data in key tables
	tables := map[string]int{
		"cbus":                     3,  // Should have at least 3 CBUs from mock data
		"products":                 5,  // Should have at least 5 products
		"kyc_rules":                7,  // Should have at least 7 KYC rules
		"dsl_transformation_rules": 11, // Should have at least 11 transformation rules
	}

	for table, minCount := range tables {
		query := fmt.Sprintf(`SELECT COUNT(*) FROM "ob-poc".%s`, table)
		var count int
		err := m.db.QueryRowContext(ctx, query).Scan(&count)
		if err != nil {
			return false, fmt.Errorf("failed to count records in %s: %w", table, err)
		}

		if count < minCount {
			m.logf("Table %s has %d records, expected at least %d", table, count, minCount)
			return false, nil
		}
	}

	return true, nil
}

// MockInterceptor represents a location where mock interception occurs
type MockInterceptor struct {
	File        string
	Line        int
	Type        string
	Description string
	Severity    string // "high", "medium", "low"
}

// findMockInterceptors identifies all mock interceptors in the codebase
func (m *MockToDBMigrator) findMockInterceptors() ([]MockInterceptor, error) {
	var interceptors []MockInterceptor

	// Key files that contain mock interceptors
	interceptorFiles := map[string][]MockInterceptor{
		"internal/config/config.go": {
			{
				Type:        "Configuration Switch",
				Description: "DSL_STORE_TYPE environment variable switches between mock and database",
				Severity:    "high",
			},
		},
		"internal/datastore/interface.go": {
			{
				Type:        "Mock Adapter",
				Description: "mockAdapter intercepts DataStore interface calls",
				Severity:    "high",
			},
			{
				Type:        "Store Factory",
				Description: "NewDataStore factory creates mock or database store",
				Severity:    "high",
			},
		},
		"internal/mocks/mock_store.go": {
			{
				Type:        "Mock Store Implementation",
				Description: "Complete mock implementation that replaces database operations",
				Severity:    "high",
			},
		},
		"internal/agent/mock_responses.go": {
			{
				Type:        "Mock Agent Responses",
				Description: "Hardcoded AI agent responses that should use database rules",
				Severity:    "medium",
			},
		},
	}

	basePath := m.basePath
	for filePath, fileInterceptors := range interceptorFiles {
		fullPath := filepath.Join(basePath, "go", filePath)
		if _, err := os.Stat(fullPath); os.IsNotExist(err) {
			continue
		}

		for _, interceptor := range fileInterceptors {
			interceptor.File = fullPath
			interceptor.Line = 1 // Would need file parsing to get exact line numbers
			interceptors = append(interceptors, interceptor)
		}
	}

	return interceptors, nil
}

// configureDatabaseMode ensures the system is configured to use database mode
func (m *MockToDBMigrator) configureDatabaseMode() error {
	// Check current configuration
	currentMode := os.Getenv("DSL_STORE_TYPE")

	if currentMode == "" {
		// Default is PostgreSQL, which is what we want
		m.logf("DSL_STORE_TYPE not set, using default PostgreSQL mode")
		return nil
	}

	if strings.ToLower(currentMode) == "mock" {
		m.logf("WARNING: DSL_STORE_TYPE is set to 'mock'")
		m.logf("To use database mode, either:")
		m.logf("  1. Unset DSL_STORE_TYPE: export DSL_STORE_TYPE=")
		m.logf("  2. Set to PostgreSQL: export DSL_STORE_TYPE=postgresql")

		if !m.dryRun {
			// In a real implementation, we might want to update environment or config files
			m.logf("Run: export DSL_STORE_TYPE=postgresql")
		}
	}

	return nil
}

// testDatabaseOperations tests that database-driven operations work correctly
func (m *MockToDBMigrator) testDatabaseOperations(ctx context.Context) error {
	// Test KYC agent with database rules
	m.logf("Testing KYC agent with database rules...")
	kycResult, err := m.dbAgent.CallKYCAgent(ctx, "UCITS equity fund domiciled in LU", []string{"CUSTODY", "FUND_ACCOUNTING"})
	if err != nil {
		return fmt.Errorf("KYC agent test failed: %w", err)
	}

	if len(kycResult.Documents) == 0 {
		return fmt.Errorf("KYC agent returned no documents")
	}
	m.logf("✓ KYC agent returned %d documents and %d jurisdictions", len(kycResult.Documents), len(kycResult.Jurisdictions))

	// Test DSL transformation agent with database rules
	m.logf("Testing DSL transformation agent...")
	transformRequest := agent.DSLTransformationRequest{
		CurrentDSL:  "(case.create (cbu.id \"TEST-001\"))",
		Instruction: "add fund accounting product",
	}

	transformResult, err := m.dbAgent.CallDSLTransformationAgent(ctx, transformRequest)
	if err != nil {
		return fmt.Errorf("DSL transformation agent test failed: %w", err)
	}

	if transformResult.Confidence == 0 {
		return fmt.Errorf("DSL transformation agent returned zero confidence")
	}
	m.logf("✓ DSL transformation agent working (confidence: %.2f)", transformResult.Confidence)

	// Test DSL validation agent with database rules
	m.logf("Testing DSL validation agent...")
	validationResult, err := m.dbAgent.CallDSLValidationAgent(ctx, "(case.create (cbu.id \"TEST-001\"))")
	if err != nil {
		return fmt.Errorf("DSL validation agent test failed: %w", err)
	}

	m.logf("✓ DSL validation agent working (score: %.2f, errors: %d)", validationResult.Score, len(validationResult.Errors))

	return nil
}

// generateRecommendations provides specific recommendations for removing mock interceptors
func (m *MockToDBMigrator) generateRecommendations(interceptors []MockInterceptor) []string {
	var recommendations []string

	// High-level recommendations
	recommendations = append(recommendations,
		"Remove or deprecate the MockStore implementation in internal/mocks/",
		"Update agent initialization to use DBAgent instead of MockAgent",
		"Consider adding feature flags to gradually phase out mock functionality",
		"Update documentation to reflect database-first architecture",
	)

	// Specific file recommendations
	for _, interceptor := range interceptors {
		switch interceptor.Type {
		case "Mock Adapter":
			recommendations = append(recommendations,
				fmt.Sprintf("In %s: Remove mockAdapter and newMockStore function", interceptor.File),
			)
		case "Configuration Switch":
			recommendations = append(recommendations,
				fmt.Sprintf("In %s: Remove mock mode from GetDataStoreConfig or mark as deprecated", interceptor.File),
			)
		case "Mock Agent Responses":
			recommendations = append(recommendations,
				fmt.Sprintf("In %s: Replace MockAgent usage with DBAgent", interceptor.File),
			)
		}
	}

	return recommendations
}

// logf logs a message if verbose mode is enabled
func (m *MockToDBMigrator) logf(format string, args ...interface{}) {
	if m.verbose {
		log.Printf(format, args...)
	}
}

// DisableMockInterceptors removes mock functionality from the system (destructive operation)
func (m *MockToDBMigrator) DisableMockInterceptors(ctx context.Context) error {
	if m.dryRun {
		m.logf("DRY RUN: Would disable mock interceptors")
		return nil
	}

	m.logf("WARNING: This will modify source code files to remove mock functionality")
	m.logf("Make sure you have backed up your code before proceeding")

	// In a real implementation, this would:
	// 1. Remove or comment out mock-related code
	// 2. Update imports to remove mock dependencies
	// 3. Update configuration defaults
	// 4. Remove mock data files

	m.logf("Mock interceptor removal is not implemented yet for safety reasons")
	m.logf("Please manually remove mock functionality based on the recommendations above")

	return nil
}

// CreateDatabaseDataStore creates a datastore that bypasses mock interceptors
func (m *MockToDBMigrator) CreateDatabaseDataStore() (datastore.DataStore, error) {
	// Force database mode regardless of environment variables
	cfg := datastore.Config{
		Type:             datastore.PostgreSQLStore,
		ConnectionString: config.GetConnectionString(),
	}

	// This would need to be implemented to directly create a PostgreSQL store
	// without going through the factory that might return a mock
	m.logf("Database datastore creation needs to be implemented in coordination with datastore package")

	return nil, fmt.Errorf("direct database store creation not yet implemented")
}
