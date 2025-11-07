package config

import (
	"os"
	"strings"

	"dsl-ob-poc/internal/datastore"
)

// GetDataStoreConfig returns the data store configuration based on environment variables and flags
func GetDataStoreConfig() datastore.Config {
	// Check for DSL_STORE_TYPE environment variable or use default
	storeType := os.Getenv("DSL_STORE_TYPE")
	if storeType == "" {
		storeType = "postgresql" // Default to PostgreSQL
	}

	config := datastore.Config{}

	switch strings.ToLower(storeType) {
	case "mock":
		config.Type = datastore.MockStore
		config.MockDataPath = getMockDataPath()
	case "postgresql", "postgres", "db":
		config.Type = datastore.PostgreSQLStore
		config.ConnectionString = getConnectionString()
	default:
		// Default to PostgreSQL if unknown type
		config.Type = datastore.PostgreSQLStore
		config.ConnectionString = getConnectionString()
	}

	return config
}

// getMockDataPath returns the path to mock data files
func getMockDataPath() string {
	path := os.Getenv("DSL_MOCK_DATA_PATH")
	if path == "" {
		return "data/mocks" // Default path
	}
	return path
}

// getConnectionString returns the database connection string
func getConnectionString() string {
	connStr := os.Getenv("DB_CONN_STRING")
	if connStr == "" {
		// Default connection string for local development
		return "postgres://localhost:5432/postgres?sslmode=disable"
	}
	return connStr
}

// IsMockMode returns true if running in mock mode
func IsMockMode() bool {
	storeType := os.Getenv("DSL_STORE_TYPE")
	return strings.EqualFold(storeType, "mock")
}
