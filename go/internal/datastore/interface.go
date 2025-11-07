package datastore

import (
	"context"
	"encoding/json"
	"fmt"

	"dsl-ob-poc/internal/dictionary"
	"dsl-ob-poc/internal/mocks"
	"dsl-ob-poc/internal/store"
)

// DataStore defines the interface for all data access operations
// This interface can be implemented by both real database store and mock store
type DataStore interface {
	// Lifecycle
	Close() error

	// CBU Operations
	ListCBUs(ctx context.Context) ([]store.CBU, error)
	GetCBUByID(ctx context.Context, cbuID string) (*store.CBU, error)
	GetCBUByName(ctx context.Context, name string) (*store.CBU, error)
	CreateCBU(ctx context.Context, name, description, naturePurpose string) (string, error)
	UpdateCBU(ctx context.Context, cbuID, name, description, naturePurpose string) error
	DeleteCBU(ctx context.Context, cbuID string) error

	// Role Operations
	ListRoles(ctx context.Context) ([]store.Role, error)
	GetRoleByID(ctx context.Context, roleID string) (*store.Role, error)
	CreateRole(ctx context.Context, name, description string) (string, error)
	UpdateRole(ctx context.Context, roleID, name, description string) error
	DeleteRole(ctx context.Context, roleID string) error

	// Product Operations
	GetProductByName(ctx context.Context, name string) (*store.Product, error)

	// Service Operations
	GetServicesForProduct(ctx context.Context, productID string) ([]store.Service, error)
	GetServiceByName(ctx context.Context, name string) (*store.Service, error)

	// Resource Operations
	GetResourcesForService(ctx context.Context, serviceID string) ([]store.ProdResource, error)

	// Dictionary Operations
	GetDictionaryAttributeByName(ctx context.Context, name string) (*dictionary.Attribute, error)
	GetDictionaryAttributeByID(ctx context.Context, id string) (*dictionary.Attribute, error)
	GetAttributesForDictionaryGroup(ctx context.Context, groupID string) ([]dictionary.Attribute, error)

	// DSL Operations
	GetLatestDSL(ctx context.Context, cbuID string) (string, error)
	InsertDSL(ctx context.Context, cbuID, dslText string) (string, error)
	GetDSLHistory(ctx context.Context, cbuID string) ([]store.DSLVersion, error)

	// Enhanced Onboarding State Management
	CreateOnboardingSession(ctx context.Context, cbuID string) (*store.OnboardingSession, error)
	GetOnboardingSession(ctx context.Context, cbuID string) (*store.OnboardingSession, error)
	UpdateOnboardingState(ctx context.Context, cbuID string, newState store.OnboardingState, dslVersionID string) error
	InsertDSLWithState(ctx context.Context, cbuID, dslText string, state store.OnboardingState) (string, error)
	GetLatestDSLWithState(ctx context.Context, cbuID string) (*store.DSLVersionWithState, error)
	GetDSLHistoryWithState(ctx context.Context, cbuID string) ([]store.DSLVersionWithState, error)
	GetDSLByVersion(ctx context.Context, cbuID string, versionNumber int) (*store.DSLVersionWithState, error)
	ListOnboardingSessions(ctx context.Context) ([]store.OnboardingSession, error)

	// Attribute Value Operations
	ResolveValueFor(ctx context.Context, cbuID, attributeID string) (json.RawMessage, map[string]any, string, error)
	UpsertAttributeValue(ctx context.Context, cbuID string, dslVersion int, attributeID string, value json.RawMessage, state string, source map[string]any) error

	// Export Operations (for mock data generation)
	GetAllProducts(ctx context.Context) ([]store.Product, error)
	GetAllServices(ctx context.Context) ([]store.Service, error)
	GetAllDictionaryAttributes(ctx context.Context) ([]dictionary.Attribute, error)
	GetAllDSLRecords(ctx context.Context) ([]store.DSLVersionWithState, error)

	// Product Requirements Operations (Phase 5)
	GetProductRequirements(ctx context.Context, productID string) (*store.ProductRequirements, error)
	GetEntityProductMapping(ctx context.Context, entityType, productID string) (*store.EntityProductMapping, error)
	ListProductRequirements(ctx context.Context) ([]store.ProductRequirements, error)
	CreateProductRequirements(ctx context.Context, req *store.ProductRequirements) error
	UpdateProductRequirements(ctx context.Context, req *store.ProductRequirements) error
	CreateEntityProductMapping(ctx context.Context, mapping *store.EntityProductMapping) error

	// Catalog Seeding (for database initialization)
	SeedCatalog(ctx context.Context) error
	SeedProductRequirements(ctx context.Context) error
	InitDB(ctx context.Context) error

	// Orchestration session persistence methods
	SaveOrchestrationSession(ctx context.Context, session *store.OrchestrationSessionData) error
	LoadOrchestrationSession(ctx context.Context, sessionID string) (*store.OrchestrationSessionData, error)
	ListActiveOrchestrationSessions(ctx context.Context) ([]string, error)
	DeleteOrchestrationSession(ctx context.Context, sessionID string) error
	CleanupExpiredOrchestrationSessions(ctx context.Context) (int64, error)
	UpdateOrchestrationSessionDSL(ctx context.Context, sessionID, dsl string, version int) error
}

// Phase 5 Product Requirements Types are now defined in the store package
// Use store.ProductRequirements, store.EntityProductMapping, etc.

// DataStoreType represents the type of data store to use
type Type string

const (
	// PostgreSQLStore uses real PostgreSQL database
	PostgreSQLStore Type = "postgresql"
	// MockStore uses JSON mock data
	MockStore Type = "mock"
)

// Config holds configuration for data store creation
type Config struct {
	Type             Type
	ConnectionString string
	MockDataPath     string
}

// NewDataStore creates a new data store based on configuration
func NewDataStore(config Config) (DataStore, error) {
	switch config.Type {
	case PostgreSQLStore:
		return newPostgreSQLStore(config.ConnectionString)
	case MockStore:
		return newMockStore(config.MockDataPath)
	default:
		return nil, &UnsupportedStoreTypeError{Type: string(config.Type)}
	}
}

// newPostgreSQLStore creates a new PostgreSQL store adapter
func newPostgreSQLStore(connectionString string) (DataStore, error) {
	store, err := store.NewStore(connectionString)
	if err != nil {
		return nil, err
	}
	return &postgresAdapter{store: store}, nil
}

// newMockStore creates a new mock store adapter
func newMockStore(mockDataPath string) (DataStore, error) {
	mockStore := mocks.NewMockStore(mockDataPath)
	return &mockAdapter{store: mockStore}, nil
}

// UnsupportedStoreTypeError is returned when an unsupported store type is requested
type UnsupportedStoreTypeError struct {
	Type string
}

func (e *UnsupportedStoreTypeError) Error() string {
	return "unsupported store type: " + e.Type
}

// postgresAdapter adapts the PostgreSQL store to the DataStore interface
type postgresAdapter struct {
	store *store.Store
}

func (p *postgresAdapter) Close() error {
	return p.store.Close()
}

func (p *postgresAdapter) ListCBUs(ctx context.Context) ([]store.CBU, error) {
	return p.store.ListCBUs(ctx)
}

func (p *postgresAdapter) GetCBUByID(ctx context.Context, cbuID string) (*store.CBU, error) {
	return p.store.GetCBUByID(ctx, cbuID)
}

func (p *postgresAdapter) GetCBUByName(ctx context.Context, name string) (*store.CBU, error) {
	return p.store.GetCBUByName(ctx, name)
}

func (p *postgresAdapter) CreateCBU(ctx context.Context, name, description, naturePurpose string) (string, error) {
	return p.store.CreateCBU(ctx, name, description, naturePurpose)
}

func (p *postgresAdapter) UpdateCBU(ctx context.Context, cbuID, name, description, naturePurpose string) error {
	return p.store.UpdateCBU(ctx, cbuID, name, description, naturePurpose)
}

func (p *postgresAdapter) DeleteCBU(ctx context.Context, cbuID string) error {
	return p.store.DeleteCBU(ctx, cbuID)
}

func (p *postgresAdapter) ListRoles(ctx context.Context) ([]store.Role, error) {
	return p.store.ListRoles(ctx)
}

func (p *postgresAdapter) GetRoleByID(ctx context.Context, roleID string) (*store.Role, error) {
	return p.store.GetRoleByID(ctx, roleID)
}

func (p *postgresAdapter) CreateRole(ctx context.Context, name, description string) (string, error) {
	return p.store.CreateRole(ctx, name, description)
}

func (p *postgresAdapter) UpdateRole(ctx context.Context, roleID, name, description string) error {
	return p.store.UpdateRole(ctx, roleID, name, description)
}

func (p *postgresAdapter) DeleteRole(ctx context.Context, roleID string) error {
	return p.store.DeleteRole(ctx, roleID)
}

func (p *postgresAdapter) GetProductByName(ctx context.Context, name string) (*store.Product, error) {
	return p.store.GetProductByName(ctx, name)
}

func (p *postgresAdapter) GetServicesForProduct(ctx context.Context, productID string) ([]store.Service, error) {
	return p.store.GetServicesForProduct(ctx, productID)
}

func (p *postgresAdapter) GetServiceByName(ctx context.Context, name string) (*store.Service, error) {
	return p.store.GetServiceByName(ctx, name)
}

func (p *postgresAdapter) GetResourcesForService(ctx context.Context, serviceID string) ([]store.ProdResource, error) {
	return p.store.GetResourcesForService(ctx, serviceID)
}

func (p *postgresAdapter) GetDictionaryAttributeByName(ctx context.Context, name string) (*dictionary.Attribute, error) {
	return p.store.GetDictionaryAttributeByName(ctx, name)
}

func (p *postgresAdapter) GetDictionaryAttributeByID(ctx context.Context, id string) (*dictionary.Attribute, error) {
	return p.store.GetDictionaryAttributeByID(ctx, id)
}

func (p *postgresAdapter) GetAttributesForDictionaryGroup(ctx context.Context, groupID string) ([]dictionary.Attribute, error) {
	return p.store.GetAttributesForDictionaryGroup(ctx, groupID)
}

func (p *postgresAdapter) GetLatestDSL(ctx context.Context, cbuID string) (string, error) {
	return p.store.GetLatestDSL(ctx, cbuID)
}

func (p *postgresAdapter) InsertDSL(ctx context.Context, cbuID, dslText string) (string, error) {
	return p.store.InsertDSL(ctx, cbuID, dslText)
}

func (p *postgresAdapter) GetDSLHistory(ctx context.Context, cbuID string) ([]store.DSLVersion, error) {
	return p.store.GetDSLHistory(ctx, cbuID)
}

// Enhanced Onboarding State Management for postgres adapter
func (p *postgresAdapter) CreateOnboardingSession(ctx context.Context, cbuID string) (*store.OnboardingSession, error) {
	return p.store.CreateOnboardingSession(ctx, cbuID)
}

func (p *postgresAdapter) GetOnboardingSession(ctx context.Context, cbuID string) (*store.OnboardingSession, error) {
	return p.store.GetOnboardingSession(ctx, cbuID)
}

func (p *postgresAdapter) UpdateOnboardingState(ctx context.Context, cbuID string, newState store.OnboardingState, dslVersionID string) error {
	return p.store.UpdateOnboardingState(ctx, cbuID, newState, dslVersionID)
}

func (p *postgresAdapter) InsertDSLWithState(ctx context.Context, cbuID, dslText string, state store.OnboardingState) (string, error) {
	return p.store.InsertDSLWithState(ctx, cbuID, dslText, state)
}

func (p *postgresAdapter) GetLatestDSLWithState(ctx context.Context, cbuID string) (*store.DSLVersionWithState, error) {
	return p.store.GetLatestDSLWithState(ctx, cbuID)
}

func (p *postgresAdapter) GetDSLHistoryWithState(ctx context.Context, cbuID string) ([]store.DSLVersionWithState, error) {
	return p.store.GetDSLHistoryWithState(ctx, cbuID)
}

func (p *postgresAdapter) GetDSLByVersion(ctx context.Context, cbuID string, versionNumber int) (*store.DSLVersionWithState, error) {
	return p.store.GetDSLByVersion(ctx, cbuID, versionNumber)
}

func (p *postgresAdapter) ListOnboardingSessions(ctx context.Context) ([]store.OnboardingSession, error) {
	return p.store.ListOnboardingSessions(ctx)
}

func (p *postgresAdapter) ResolveValueFor(ctx context.Context, cbuID, attributeID string) (payload json.RawMessage, provenance map[string]any, status string, err error) {
	return p.store.ResolveValueFor(ctx, cbuID, attributeID)
}

func (p *postgresAdapter) UpsertAttributeValue(ctx context.Context, cbuID string, dslVersion int, attributeID string, value json.RawMessage, state string, source map[string]any) error {
	return p.store.UpsertAttributeValue(ctx, cbuID, dslVersion, attributeID, value, state, source)
}

func (p *postgresAdapter) SeedCatalog(ctx context.Context) error {
	return p.store.SeedCatalog(ctx)
}

func (p *postgresAdapter) SeedProductRequirements(ctx context.Context) error {
	return p.store.SeedProductRequirements(ctx)
}

// Product Requirements Operations (Phase 5)
func (p *postgresAdapter) GetProductRequirements(ctx context.Context, productID string) (*store.ProductRequirements, error) {
	return p.store.GetProductRequirements(ctx, productID)
}

func (p *postgresAdapter) GetEntityProductMapping(ctx context.Context, entityType, productID string) (*store.EntityProductMapping, error) {
	return p.store.GetEntityProductMapping(ctx, entityType, productID)
}

func (p *postgresAdapter) ListProductRequirements(ctx context.Context) ([]store.ProductRequirements, error) {
	return p.store.ListProductRequirements(ctx)
}

func (p *postgresAdapter) CreateProductRequirements(ctx context.Context, req *store.ProductRequirements) error {
	return p.store.CreateProductRequirements(ctx, req)
}

func (p *postgresAdapter) UpdateProductRequirements(ctx context.Context, req *store.ProductRequirements) error {
	return p.store.UpdateProductRequirements(ctx, req)
}

func (p *postgresAdapter) CreateEntityProductMapping(ctx context.Context, mapping *store.EntityProductMapping) error {
	return p.store.CreateEntityProductMapping(ctx, mapping)
}

func (p *postgresAdapter) InitDB(ctx context.Context) error {
	return p.store.InitDB(ctx)
}

func (p *postgresAdapter) SaveOrchestrationSession(ctx context.Context, session *store.OrchestrationSessionData) error {
	return p.store.SaveOrchestrationSession(ctx, session)
}

func (p *postgresAdapter) LoadOrchestrationSession(ctx context.Context, sessionID string) (*store.OrchestrationSessionData, error) {
	return p.store.LoadOrchestrationSession(ctx, sessionID)
}

func (p *postgresAdapter) ListActiveOrchestrationSessions(ctx context.Context) ([]string, error) {
	return p.store.ListActiveOrchestrationSessions(ctx)
}

func (p *postgresAdapter) DeleteOrchestrationSession(ctx context.Context, sessionID string) error {
	return p.store.DeleteOrchestrationSession(ctx, sessionID)
}

func (p *postgresAdapter) CleanupExpiredOrchestrationSessions(ctx context.Context) (int64, error) {
	return p.store.CleanupExpiredOrchestrationSessions(ctx)
}

func (p *postgresAdapter) UpdateOrchestrationSessionDSL(ctx context.Context, sessionID, dsl string, version int) error {
	return p.store.UpdateOrchestrationSessionDSL(ctx, sessionID, dsl, version)
}

// Export Operations for postgres adapter
func (p *postgresAdapter) GetAllProducts(ctx context.Context) ([]store.Product, error) {
	return p.store.GetAllProducts(ctx)
}

func (p *postgresAdapter) GetAllServices(ctx context.Context) ([]store.Service, error) {
	return p.store.GetAllServices(ctx)
}

func (p *postgresAdapter) GetAllDictionaryAttributes(ctx context.Context) ([]dictionary.Attribute, error) {
	return p.store.GetAllDictionaryAttributes(ctx)
}

func (p *postgresAdapter) GetAllDSLRecords(ctx context.Context) ([]store.DSLVersionWithState, error) {
	return p.store.GetAllDSLRecords(ctx)
}

// mockAdapter adapts the mock store to the DataStore interface
type mockAdapter struct {
	store *mocks.MockStore
}

func (m *mockAdapter) Close() error {
	return m.store.Close()
}

func (m *mockAdapter) ListCBUs(ctx context.Context) ([]store.CBU, error) {
	return m.store.ListCBUs(ctx)
}

func (m *mockAdapter) GetCBUByID(ctx context.Context, cbuID string) (*store.CBU, error) {
	return m.store.GetCBUByID(ctx, cbuID)
}

func (m *mockAdapter) GetCBUByName(ctx context.Context, name string) (*store.CBU, error) {
	return m.store.GetCBUByName(ctx, name)
}

func (m *mockAdapter) CreateCBU(ctx context.Context, name, description, naturePurpose string) (string, error) {
	return m.store.CreateCBU(ctx, name, description, naturePurpose)
}

func (m *mockAdapter) UpdateCBU(ctx context.Context, cbuID, name, description, naturePurpose string) error {
	return m.store.UpdateCBU(ctx, cbuID, name, description, naturePurpose)
}

func (m *mockAdapter) DeleteCBU(ctx context.Context, cbuID string) error {
	return m.store.DeleteCBU(ctx, cbuID)
}

func (m *mockAdapter) ListRoles(ctx context.Context) ([]store.Role, error) {
	return m.store.ListRoles(ctx)
}

func (m *mockAdapter) GetRoleByID(ctx context.Context, roleID string) (*store.Role, error) {
	return m.store.GetRoleByID(ctx, roleID)
}

func (m *mockAdapter) CreateRole(ctx context.Context, name, description string) (string, error) {
	return m.store.CreateRole(ctx, name, description)
}

func (m *mockAdapter) UpdateRole(ctx context.Context, roleID, name, description string) error {
	return m.store.UpdateRole(ctx, roleID, name, description)
}

func (m *mockAdapter) DeleteRole(ctx context.Context, roleID string) error {
	return m.store.DeleteRole(ctx, roleID)
}

func (m *mockAdapter) GetProductByName(ctx context.Context, name string) (*store.Product, error) {
	return m.store.GetProductByName(ctx, name)
}

func (m *mockAdapter) GetServicesForProduct(ctx context.Context, productID string) ([]store.Service, error) {
	return m.store.GetServicesForProduct(ctx, productID)
}

func (m *mockAdapter) GetServiceByName(ctx context.Context, name string) (*store.Service, error) {
	return m.store.GetServiceByName(ctx, name)
}

func (m *mockAdapter) GetResourcesForService(ctx context.Context, serviceID string) ([]store.ProdResource, error) {
	return m.store.GetResourcesForService(ctx, serviceID)
}

func (m *mockAdapter) GetDictionaryAttributeByName(ctx context.Context, name string) (*dictionary.Attribute, error) {
	return m.store.GetDictionaryAttributeByName(ctx, name)
}

func (m *mockAdapter) GetDictionaryAttributeByID(ctx context.Context, id string) (*dictionary.Attribute, error) {
	return m.store.GetDictionaryAttributeByID(ctx, id)
}

func (m *mockAdapter) GetAttributesForDictionaryGroup(ctx context.Context, groupID string) ([]dictionary.Attribute, error) {
	return m.store.GetAttributesForDictionaryGroup(ctx, groupID)
}

func (m *mockAdapter) GetLatestDSL(ctx context.Context, cbuID string) (string, error) {
	return m.store.GetLatestDSL(ctx, cbuID)
}

func (m *mockAdapter) InsertDSL(ctx context.Context, cbuID, dslText string) (string, error) {
	return m.store.InsertDSL(ctx, cbuID, dslText)
}

func (m *mockAdapter) GetDSLHistory(ctx context.Context, cbuID string) ([]store.DSLVersion, error) {
	return m.store.GetDSLHistory(ctx, cbuID)
}

func (m *mockAdapter) ResolveValueFor(ctx context.Context, cbuID, attributeID string) (payload json.RawMessage, provenance map[string]any, status string, err error) {
	return m.store.ResolveValueFor(ctx, cbuID, attributeID)
}

func (m *mockAdapter) UpsertAttributeValue(ctx context.Context, cbuID string, dslVersion int, attributeID string, value json.RawMessage, state string, source map[string]any) error {
	return m.store.UpsertAttributeValue(ctx, cbuID, dslVersion, attributeID, value, state, source)
}

func (m *mockAdapter) SeedCatalog(ctx context.Context) error {
	return nil // Mock store doesn't need seeding
}

func (m *mockAdapter) SeedProductRequirements(ctx context.Context) error {
	return nil // Mock store doesn't need seeding
}

// Product Requirements Operations (Phase 5) - Mock implementations
func (m *mockAdapter) GetProductRequirements(ctx context.Context, productID string) (*store.ProductRequirements, error) {
	return nil, fmt.Errorf("DEPRECATED: product requirements mocks disabled - use database via PostgreSQL adapter")
}

func (m *mockAdapter) GetEntityProductMapping(ctx context.Context, entityType, productID string) (*store.EntityProductMapping, error) {
	return nil, fmt.Errorf("DEPRECATED: entity product mapping mocks disabled - use database via PostgreSQL adapter")
}

func (m *mockAdapter) ListProductRequirements(ctx context.Context) ([]store.ProductRequirements, error) {
	return nil, fmt.Errorf("DEPRECATED: product requirements list mocks disabled - use database via PostgreSQL adapter")
}

func (m *mockAdapter) CreateProductRequirements(ctx context.Context, req *store.ProductRequirements) error {
	return fmt.Errorf("DEPRECATED: product requirements creation mocks disabled - use database via PostgreSQL adapter")
}

func (m *mockAdapter) UpdateProductRequirements(ctx context.Context, req *store.ProductRequirements) error {
	return fmt.Errorf("DEPRECATED: product requirements update mocks disabled - use database via PostgreSQL adapter")
}

func (m *mockAdapter) CreateEntityProductMapping(ctx context.Context, mapping *store.EntityProductMapping) error {
	return fmt.Errorf("DEPRECATED: entity product mapping creation mocks disabled - use database via PostgreSQL adapter")
}

func (m *mockAdapter) InitDB(ctx context.Context) error {
	return nil // Mock store doesn't need DB initialization
}

// Enhanced Onboarding State Management for mock adapter
func (m *mockAdapter) CreateOnboardingSession(ctx context.Context, cbuID string) (*store.OnboardingSession, error) {
	return m.store.CreateOnboardingSession(ctx, cbuID)
}

func (m *mockAdapter) GetOnboardingSession(ctx context.Context, cbuID string) (*store.OnboardingSession, error) {
	return m.store.GetOnboardingSession(ctx, cbuID)
}

func (m *mockAdapter) UpdateOnboardingState(ctx context.Context, cbuID string, newState store.OnboardingState, dslVersionID string) error {
	return m.store.UpdateOnboardingState(ctx, cbuID, newState, dslVersionID)
}

func (m *mockAdapter) InsertDSLWithState(ctx context.Context, cbuID, dslText string, state store.OnboardingState) (string, error) {
	return m.store.InsertDSLWithState(ctx, cbuID, dslText, state)
}

func (m *mockAdapter) GetLatestDSLWithState(ctx context.Context, cbuID string) (*store.DSLVersionWithState, error) {
	return m.store.GetLatestDSLWithState(ctx, cbuID)
}

func (m *mockAdapter) GetDSLHistoryWithState(ctx context.Context, cbuID string) ([]store.DSLVersionWithState, error) {
	return m.store.GetDSLHistoryWithState(ctx, cbuID)
}

func (m *mockAdapter) GetDSLByVersion(ctx context.Context, cbuID string, versionNumber int) (*store.DSLVersionWithState, error) {
	return m.store.GetDSLByVersion(ctx, cbuID, versionNumber)
}

func (m *mockAdapter) ListOnboardingSessions(ctx context.Context) ([]store.OnboardingSession, error) {
	return m.store.ListOnboardingSessions(ctx)
}

// Export Operations for mock adapter
func (m *mockAdapter) GetAllProducts(ctx context.Context) ([]store.Product, error) {
	return m.store.GetAllProducts(ctx)
}

func (m *mockAdapter) GetAllServices(ctx context.Context) ([]store.Service, error) {
	return m.store.GetAllServices(ctx)
}

func (m *mockAdapter) GetAllDictionaryAttributes(ctx context.Context) ([]dictionary.Attribute, error) {
	return m.store.GetAllDictionaryAttributes(ctx)
}

func (m *mockAdapter) GetAllDSLRecords(ctx context.Context) ([]store.DSLVersionWithState, error) {
	return m.store.GetAllDSLRecords(ctx)
}

func (m *mockAdapter) SaveOrchestrationSession(ctx context.Context, session *store.OrchestrationSessionData) error {
	return m.store.SaveOrchestrationSession(ctx, session)
}

func (m *mockAdapter) LoadOrchestrationSession(ctx context.Context, sessionID string) (*store.OrchestrationSessionData, error) {
	return m.store.LoadOrchestrationSession(ctx, sessionID)
}

func (m *mockAdapter) ListActiveOrchestrationSessions(ctx context.Context) ([]string, error) {
	return m.store.ListActiveOrchestrationSessions(ctx)
}

func (m *mockAdapter) DeleteOrchestrationSession(ctx context.Context, sessionID string) error {
	return m.store.DeleteOrchestrationSession(ctx, sessionID)
}

func (m *mockAdapter) CleanupExpiredOrchestrationSessions(ctx context.Context) (int64, error) {
	return m.store.CleanupExpiredOrchestrationSessions(ctx)
}

func (m *mockAdapter) UpdateOrchestrationSessionDSL(ctx context.Context, sessionID, dsl string, version int) error {
	return m.store.UpdateOrchestrationSessionDSL(ctx, sessionID, dsl, version)
}
