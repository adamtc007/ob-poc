package dslmanagerintegration

import (
	"context"
	"os"
	"testing"

	"dsl-ob-poc/internal/config"
	"dsl-ob-poc/internal/datastore"
	"dsl-ob-poc/internal/store"
)

// TestConfig provides a centralized configuration for integration tests
type TestConfig struct {
	DataStore     datastore.DataStore
	PostgresStore *store.Store
	Context       context.Context
	Cancel        context.CancelFunc
}

// SetupTestEnvironment initializes the testing environment with database configuration
func SetupTestEnvironment(t *testing.T) *TestConfig {
	// Ensure test environment variables
	if os.Getenv("GO_TEST_MODE") != "integration" {
		t.Skip("Skipping integration tests. Set GO_TEST_MODE=integration to run.")
	}

	// Use test configuration
	cfg := config.GetDataStoreConfig()
	cfg.Type = datastore.PostgreSQLStore

	// Create context with cancellation
	ctx, cancel := context.WithCancel(context.Background())

	// Initialize DataStore
	dataStore, err := datastore.NewDataStore(cfg)
	if err != nil {
		t.Fatalf("Failed to initialize data store: %v", err)
	}

	// Initialize PostgreSQL store
	postgresStore, err := store.NewStore(cfg.ConnectionString)
	if err != nil {
		t.Fatalf("Failed to initialize postgres store: %v", err)
	}

	// Initialize database and seed catalog
	if err := dataStore.InitDB(ctx); err != nil {
		t.Fatalf("Failed to initialize database: %v", err)
	}

	if err := dataStore.SeedCatalog(ctx); err != nil {
		t.Fatalf("Failed to seed catalog: %v", err)
	}

	return &TestConfig{
		DataStore:     dataStore,
		PostgresStore: postgresStore,
		Context:       ctx,
		Cancel:        cancel,
	}
}

// TeardownTestEnvironment cleans up resources after integration tests
func (tc *TestConfig) TeardownTestEnvironment() {
	if tc.Cancel != nil {
		tc.Cancel()
	}

	if tc.DataStore != nil {
		tc.DataStore.Close()
	}

	if tc.PostgresStore != nil {
		tc.PostgresStore.Close()
	}
}

// GenerateTestDomains provides sample domain configurations for testing
type TestDomain struct {
	Name     string
	Workflow []struct {
		State string
		DSL   string
	}
}

func GenerateTestDomains() []TestDomain {
	return []TestDomain{
		{
			Name: "investor",
			Workflow: []struct {
				State string
				DSL   string
			}{
				{State: "CREATED", DSL: "(investor.create (name \"Test Investor\") (type \"PROPER_PERSON\"))"},
				{State: "KYC_STARTED", DSL: "(kyc.start (document \"Passport\") (jurisdiction \"US\"))"},
				{State: "KYC_COMPLETED", DSL: "(kyc.complete (risk-rating \"LOW\"))"},
				{State: "PRODUCTS_ADDED", DSL: "(products.add \"CUSTODY\" \"REPORTING\")"},
			},
		},
		{
			Name: "fund",
			Workflow: []struct {
				State string
				DSL   string
			}{
				{State: "FUND_CREATED", DSL: "(fund.create (name \"Alpha Strategy Fund\") (strategy \"Long/Short\"))"},
				{State: "SUBSCRIPTION_STARTED", DSL: "(subscription.start (amount \"1000000\") (currency \"USD\"))"},
				{State: "DOCUMENTS_COLLECTED", DSL: "(documents.collect (type \"PrivateOfferingMemo\"))"},
			},
		},
	}
}

// MockDSLGenerator provides a flexible DSL generation utility for testing
func MockDSLGenerator(domain, state string, customData map[string]string) string {
	switch domain {
	case "investor":
		switch state {
		case "CREATED":
			return "(investor.create " +
				"(name \"" + customData["name"] + "\") " +
				"(type \"" + customData["type"] + "\"))"
		case "KYC_STARTED":
			return "(kyc.start " +
				"(document \"" + customData["document"] + "\") " +
				"(jurisdiction \"" + customData["jurisdiction"] + "\"))"
		}
	case "hedge-fund":
		switch state {
		case "FUND_CREATED":
			return "(fund.create " +
				"(name \"" + customData["name"] + "\") " +
				"(strategy \"" + customData["strategy"] + "\"))"
		}
	}
	return ""
}

/*
This base integration test file provides:
1. A centralized setup for integration testing
2. Context and resource management
3. Test environment configuration
4. Sample domain workflows
5. A mock DSL generator for flexible testing scenarios

Key features:
- Uses environment variable `GO_TEST_MODE=integration` to control test execution
- Provides shared test utilities and configuration
- Includes sample workflows for different business domains
- Sets up database connections and test data

The file sets the stage for more specific integration tests in subsequent files.
*/
