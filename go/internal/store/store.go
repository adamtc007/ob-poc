package store

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"dsl-ob-poc/internal/dictionary"

	"github.com/lib/pq"
	_ "github.com/lib/pq"
)

// Store represents the database connection and operations.
type Store struct {
	db *sql.DB
}

// CBU represents a Client Business Unit in the catalog.
type CBU struct {
	CBUID         string `json:"cbu_id"` // UUID string - references database UUID primary key
	Name          string `json:"name"`
	Description   string `json:"description"`
	NaturePurpose string `json:"nature_purpose"`
}

// Product represents a product in the catalog.
type Product struct {
	ProductID   string `json:"product_id"`
	Name        string `json:"name"`
	Description string `json:"description"`
}

// Service represents a service in the catalog.
type Service struct {
	ServiceID   string `json:"service_id"`
	Name        string `json:"name"`
	Description string `json:"description"`
}

// Phase 5 Product Requirements Types
type ProductRequirements struct {
	ProductID        string                   `json:"product_id"`
	ProductName      string                   `json:"product_name"`
	EntityTypes      JSONBStringArray         `json:"entity_types"`      // JSONB array
	RequiredDSL      JSONBStringArray         `json:"required_dsl"`      // JSONB array
	Attributes       JSONBStringArray         `json:"attributes"`        // JSONB array
	Compliance       []ProductComplianceRule  `json:"compliance"`        // Will be marshaled as JSONB
	Prerequisites    JSONBStringArray         `json:"prerequisites"`     // JSONB array
	ConditionalRules []ProductConditionalRule `json:"conditional_rules"` // Will be marshaled as JSONB
	CreatedAt        time.Time                `json:"created_at"`
	UpdatedAt        time.Time                `json:"updated_at"`
}

type ProductComplianceRule struct {
	RuleID      string `json:"rule_id"`
	Framework   string `json:"framework"`
	Description string `json:"description"`
	Required    bool   `json:"required"`
}

type ProductConditionalRule struct {
	Condition   string   `json:"condition"`
	RequiredDSL []string `json:"required_dsl"`
	Attributes  []string `json:"attributes"`
}

type EntityProductMapping struct {
	EntityType     string           `json:"entity_type"`
	ProductID      string           `json:"product_id"`
	Compatible     bool             `json:"compatible"`
	Restrictions   JSONBStringArray `json:"restrictions"`    // JSONB array
	RequiredFields JSONBStringArray `json:"required_fields"` // JSONB array
	CreatedAt      time.Time        `json:"created_at"`
}

// ProdResource represents a resource required by products/services.
type ProdResource struct {
	ResourceID      string `json:"resource_id"`
	Name            string `json:"name"`
	Description     string `json:"description"`
	Owner           string `json:"owner"`
	DictionaryGroup string `json:"dictionary_group"`
}

// Attribute represents an attribute in the dictionary (v3 schema).
type Attribute struct {
	AttributeID     string              `json:"attribute_id"`
	Name            string              `json:"name"`
	LongDescription string              `json:"long_description"`
	GroupID         string              `json:"group_id"`
	Mask            string              `json:"mask"`
	Domain          string              `json:"domain"`
	Vector          string              `json:"vector"`
	Source          JSONBSourceMetadata `json:"source"` // Structured JSONB
	Sink            JSONBSinkMetadata   `json:"sink"`   // Structured JSONB
}

// Role represents a role that entities can play within a CBU.
type Role struct {
	RoleID      string `json:"role_id"`
	Name        string `json:"name"`
	Description string `json:"description"`
}

// EntityType represents the different types of entities.
type EntityType struct {
	EntityTypeID string `json:"entity_type_id"`
	Name         string `json:"name"`
	Description  string `json:"description"`
	TableName    string `json:"table_name"`
}

// Entity represents an entity in the central registry.
type Entity struct {
	EntityID     string `json:"entity_id"`
	EntityTypeID string `json:"entity_type_id"`
	ExternalID   string `json:"external_id"`
	Name         string `json:"name"`
}

// CBUEntityRole represents the relationship between CBUs, entities, and roles.
type CBUEntityRole struct {
	CBUEntityRoleID string `json:"cbu_entity_role_id"`
	CBUID           string `json:"cbu_id"`
	EntityID        string `json:"entity_id"`
	RoleID          string `json:"role_id"`
}

// LimitedCompany represents a limited company entity.
type LimitedCompany struct {
	LimitedCompanyID   string     `json:"limited_company_id"`
	CompanyName        string     `json:"company_name"`
	RegistrationNumber string     `json:"registration_number"`
	Jurisdiction       string     `json:"jurisdiction"`
	IncorporationDate  *time.Time `json:"incorporation_date"`
	RegisteredAddress  string     `json:"registered_address"`
	BusinessNature     string     `json:"business_nature"`
}

// Partnership represents a partnership entity.
type Partnership struct {
	PartnershipID            string     `json:"partnership_id"`
	PartnershipName          string     `json:"partnership_name"`
	PartnershipType          string     `json:"partnership_type"`
	Jurisdiction             string     `json:"jurisdiction"`
	FormationDate            *time.Time `json:"formation_date"`
	PrincipalPlaceBusiness   string     `json:"principal_place_business"`
	PartnershipAgreementDate *time.Time `json:"partnership_agreement_date"`
}

// Individual represents an individual (proper person) entity.
type Individual struct {
	ProperProperPersonID string     `json:"proper_proper_person_id"`
	FirstName            string     `json:"first_name"`
	LastName             string     `json:"last_name"`
	MiddleNames          string     `json:"middle_names"`
	DateOfBirth          *time.Time `json:"date_of_birth"`
	Nationality          string     `json:"nationality"`
	ResidenceAddress     string     `json:"residence_address"`
	IDDocumentType       string     `json:"id_document_type"`
	IDDocumentNumber     string     `json:"id_document_number"`
}

// NewStore creates a new Store instance and opens a database connection.
func NewStore(connString string) (*Store, error) {
	db, err := sql.Open("postgres", connString)
	if err != nil {
		return nil, fmt.Errorf("failed to open database: %w", err)
	}

	// Test the connection
	if pingErr := db.Ping(); pingErr != nil {
		db.Close()
		return nil, fmt.Errorf("failed to ping database: %w", pingErr)
	}

	return &Store{db: db}, nil
}

// NewStoreFromDB constructs a Store from an existing *sql.DB. Useful for tests.
func NewStoreFromDB(db *sql.DB) *Store {
	return &Store{db: db}
}

// Close closes the database connection.
func (s *Store) Close() error {
	if s.db != nil {
		return s.db.Close()
	}
	return nil
}

// DB returns the underlying database connection.
func (s *Store) DB() *sql.DB {
	return s.db
}

// InitDB initializes the database schema from the SQL file.
func (s *Store) InitDB(ctx context.Context) error {
	// Read the shared schema file
	sqlFilePath := filepath.Join("..", "sql", "00_init_schema.sql")
	sqlBytes, err := os.ReadFile(sqlFilePath)
	if err != nil {
		return fmt.Errorf("failed to read SQL file: %w", err)
	}

	// Execute the SQL
	_, err = s.db.ExecContext(ctx, string(sqlBytes))
	if err != nil {
		return fmt.Errorf("failed to execute init SQL: %w", err)
	}

	return nil
}

// SeedCatalog seeds the catalog tables with mock data.
func (s *Store) SeedCatalog(ctx context.Context) error {
	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return fmt.Errorf("failed to begin transaction: %w", err)
	}
	defer func() {
		_ = tx.Rollback()
	}()

	// Insert CBUs
	cbus := []struct {
		name          string
		description   string
		naturePurpose string
	}{
		{"CBU-1234", "Aviva Investors Global Fund", "UCITS equity fund domiciled in LU"},
		{"CBU-5678", "Blackrock US Debt Fund", "Corporate debt fund domiciled in IE"},
		{"CBU-9999", "Test Development Fund", "Mock fund for testing and development"},
	}

	cbuIDs := make(map[string]string)
	for _, c := range cbus {
		var cbuID string
		queryErr := tx.QueryRowContext(ctx,
			`INSERT INTO "dsl-ob-poc".cbus (name, description, nature_purpose)
			 VALUES ($1, $2, $3)
			 ON CONFLICT (name) DO UPDATE SET description = EXCLUDED.description, nature_purpose = EXCLUDED.nature_purpose
			 RETURNING cbu_id`,
			c.name, c.description, c.naturePurpose).Scan(&cbuID)
		if queryErr != nil {
			return fmt.Errorf("failed to insert CBU %s: %w", c.name, queryErr)
		}
		cbuIDs[c.name] = cbuID
	}

	// Insert Products
	products := []struct {
		name        string
		description string
	}{
		{"CUSTODY", "Custody and safekeeping services"},
		{"FUND_ACCOUNTING", "Fund accounting and NAV calculation"},
		{"TRANSFER_AGENCY", "Transfer agency and registry services"},
	}

	productIDs := make(map[string]string)
	for _, p := range products {
		var productID string
		queryErr := tx.QueryRowContext(ctx,
			`INSERT INTO "dsl-ob-poc".products (name, description)
			 VALUES ($1, $2)
			 ON CONFLICT (name) DO UPDATE SET description = EXCLUDED.description
			 RETURNING product_id`,
			p.name, p.description).Scan(&productID)
		if queryErr != nil {
			return fmt.Errorf("failed to insert product %s: %w", p.name, queryErr)
		}
		productIDs[p.name] = productID
	}

	// Insert Services
	services := []struct {
		name        string
		description string
	}{
		{"CustodyService", "Asset custody and safekeeping"},
		{"SettlementService", "Trade settlement processing"},
		{"FundAccountingService", "Daily NAV calculation and reporting"},
		{"TransferAgencyService", "Shareholder registry management"},
	}

	serviceIDs := make(map[string]string)
	for _, srv := range services {
		var serviceID string
		queryErr := tx.QueryRowContext(ctx,
			`INSERT INTO "dsl-ob-poc".services (name, description)
			 VALUES ($1, $2)
			 ON CONFLICT (name) DO UPDATE SET description = EXCLUDED.description
			 RETURNING service_id`,
			srv.name, srv.description).Scan(&serviceID)
		if queryErr != nil {
			return fmt.Errorf("failed to insert service %s: %w", srv.name, queryErr)
		}
		serviceIDs[srv.name] = serviceID
	}

	// Link Products to Services
	productServiceLinks := []struct {
		product string
		service string
	}{
		{"CUSTODY", "CustodyService"},
		{"CUSTODY", "SettlementService"},
		{"FUND_ACCOUNTING", "FundAccountingService"},
		{"TRANSFER_AGENCY", "TransferAgencyService"},
	}

	for _, link := range productServiceLinks {
		_, execErr := tx.ExecContext(ctx,
			`INSERT INTO "dsl-ob-poc".product_services (product_id, service_id)
			 VALUES ($1, $2)
			 ON CONFLICT DO NOTHING`,
			productIDs[link.product], serviceIDs[link.service])
		if execErr != nil {
			return fmt.Errorf("failed to link product %s to service %s: %w", link.product, link.service, execErr)
		}
	}

	// Insert Dictionary Attributes (v3 schema)
	attributes := []struct {
		name            string
		longDescription string
		groupID         string
		mask            string
		domain          string
		sourceJSON      string
		sinkJSON        string
	}{
		{
			"onboard.cbu_id",
			"Client Business Unit identifier for onboarding case tracking and workflow management",
			"Onboarding",
			"string",
			"Onboarding",
			`{"type": "manual", "url": "https://onboarding.example.com/cbu", "required": true, "format": "CBU-[0-9]+"}`,
			`{"type": "database", "url": "postgres://onboarding_db/cases", "table": "onboarding_cases", "field": "cbu_id"}`,
		},
		{
			"entity.legal_name",
			"Legal name of the entity for KYC purposes",
			"KYC",
			"string",
			"KYC",
			`{"type": "manual", "url": "https://kyc.example.com/entity", "required": true}`,
			`{"type": "database", "url": "postgres://kyc_db/entities", "table": "legal_entities", "field": "legal_name"}`,
		},
		{
			"custody.account_number",
			"Custody account identifier for asset safekeeping",
			"CustodyAccount",
			"string",
			"Custody",
			`{"type": "api", "url": "https://custody.example.com/accounts", "method": "GET"}`,
			`{"type": "database", "url": "postgres://custody_db/accounts", "table": "accounts", "field": "account_number"}`,
		},
		{
			"entity.domicile",
			"Domicile jurisdiction of the fund or entity",
			"KYC",
			"string",
			"KYC",
			`{"type": "registry", "url": "https://registry.example.com/jurisdictions", "validated": true}`,
			`{"type": "database", "url": "postgres://kyc_db/entities", "table": "entities", "field": "domicile"}`,
		},
		{
			"security.isin",
			"International Securities Identification Number",
			"Security",
			"string",
			"Trading",
			`{"type": "api", "url": "https://isin-registry.example.com/lookup", "authoritative": true}`,
			`{"type": "database", "url": "postgres://trading_db/securities", "table": "securities", "field": "isin"}`,
		},
		{
			"accounting.nav_value",
			"Net Asset Value calculated daily",
			"FundAccounting",
			"string",
			"Accounting",
			`{"type": "calculated", "formula": "total_assets - total_liabilities", "frequency": "daily"}`,
			`{"type": "database", "url": "postgres://accounting_db/nav", "table": "daily_nav", "field": "nav_value"}`,
		},
	}

	for _, attr := range attributes {
		_, execErr := tx.ExecContext(ctx,
			`INSERT INTO "dsl-ob-poc".dictionary (name, long_description, group_id, mask, domain, source, sink)
			 VALUES ($1, $2, $3, $4, $5, $6::jsonb, $7::jsonb)
			 ON CONFLICT (name) DO UPDATE SET
				long_description = EXCLUDED.long_description,
				group_id = EXCLUDED.group_id,
				mask = EXCLUDED.mask,
				domain = EXCLUDED.domain,
				source = EXCLUDED.source,
				sink = EXCLUDED.sink`,
			attr.name, attr.longDescription, attr.groupID, attr.mask, attr.domain, attr.sourceJSON, attr.sinkJSON)
		if execErr != nil {
			return fmt.Errorf("failed to insert dictionary attribute %s: %w", attr.name, execErr)
		}
	}

	// Insert Resources
	resources := []struct {
		name            string
		description     string
		owner           string
		dictionaryGroup string
	}{
		{"CustodyAccount", "Custody account resource", "CustodyTech", "CustodyAccount"},
		{"FundAccountingRecord", "Fund accounting record resource", "AccountingEng", "FundAccounting"},
		{"ShareholderRegistry", "Shareholder registry resource", "TransferAgencyTeam", "KYC"},
	}

	resourceIDs := make(map[string]string)
	for _, res := range resources {
		var resourceID string
		queryErr := tx.QueryRowContext(ctx,
			`INSERT INTO "dsl-ob-poc".prod_resources (name, description, owner, dictionary_group)
			 VALUES ($1, $2, $3, $4)
			 ON CONFLICT (name) DO UPDATE SET description = EXCLUDED.description
			 RETURNING resource_id`,
			res.name, res.description, res.owner, res.dictionaryGroup).Scan(&resourceID)
		if queryErr != nil {
			return fmt.Errorf("failed to insert resource %s: %w", res.name, queryErr)
		}
		resourceIDs[res.name] = resourceID
	}

	// Link Services to Resources
	serviceResourceLinks := []struct {
		service  string
		resource string
	}{
		{"CustodyService", "CustodyAccount"},
		{"SettlementService", "CustodyAccount"},
		{"FundAccountingService", "FundAccountingRecord"},
		{"TransferAgencyService", "ShareholderRegistry"},
	}

	for _, link := range serviceResourceLinks {
		_, execErr := tx.ExecContext(ctx,
			`INSERT INTO "dsl-ob-poc".service_resources (service_id, resource_id)
			 VALUES ($1, $2)
			 ON CONFLICT DO NOTHING`,
			serviceIDs[link.service], resourceIDs[link.resource])
		if execErr != nil {
			return fmt.Errorf("failed to link service %s to resource %s: %w", link.service, link.resource, execErr)
		}
	}

	return tx.Commit()
}

// InsertDSL inserts a new DSL version and returns its version ID.
func (s *Store) InsertDSL(ctx context.Context, cbuID, dslText string) (string, error) {
	var versionID string
	err := s.db.QueryRowContext(ctx,
		`INSERT INTO "dsl-ob-poc".dsl_ob (cbu_id, dsl_text) VALUES ($1, $2) RETURNING version_id`,
		cbuID, dslText).Scan(&versionID)
	if err != nil {
		return "", fmt.Errorf("failed to insert DSL: %w", err)
	}
	return versionID, nil
}

// GetLatestDSL retrieves the most recent DSL for a given CBU ID.
func (s *Store) GetLatestDSL(ctx context.Context, cbuID string) (string, error) {
	var dslText string
	err := s.db.QueryRowContext(ctx,
		`SELECT dsl_text FROM "dsl-ob-poc".dsl_ob
		 WHERE cbu_id = $1
		 ORDER BY created_at DESC
		 LIMIT 1`,
		cbuID).Scan(&dslText)
	if err == sql.ErrNoRows {
		return "", fmt.Errorf("no DSL found for CBU_ID: %s", cbuID)
	}
	if err != nil {
		return "", fmt.Errorf("failed to get latest DSL: %w", err)
	}
	return dslText, nil
}

// DSLVersion represents a single versioned DSL entry.
type DSLVersion struct {
	VersionID string
	CreatedAt time.Time
	DSLText   string
}

// GetDSLHistory returns all DSL versions for a given CBU ID ordered by creation time.
func (s *Store) GetDSLHistory(ctx context.Context, cbuID string) ([]DSLVersion, error) {
	rows, err := s.db.QueryContext(ctx,
		`SELECT version_id::text, created_at, dsl_text
         FROM "dsl-ob-poc".dsl_ob
         WHERE cbu_id = $1
         ORDER BY created_at ASC`, cbuID)
	if err != nil {
		return nil, fmt.Errorf("failed to query DSL history: %w", err)
	}
	defer rows.Close()

	var history []DSLVersion
	for rows.Next() {
		var v DSLVersion
		if scanErr := rows.Scan(&v.VersionID, &v.CreatedAt, &v.DSLText); scanErr != nil {
			return nil, fmt.Errorf("failed to scan DSL history row: %w", scanErr)
		}
		history = append(history, v)
	}
	if rowsErr := rows.Err(); rowsErr != nil {
		return nil, fmt.Errorf("error iterating DSL history: %w", rowsErr)
	}

	return history, nil
}

// getDictionaryAttribute is a helper function to retrieve an attribute with a specific WHERE clause
func (s *Store) getDictionaryAttribute(ctx context.Context, whereClause string, param interface{}, notFoundMsg string) (*dictionary.Attribute, error) {
	var attr dictionary.Attribute
	var sourceJSON, sinkJSON string

	query := `SELECT attribute_id, name, long_description, group_id, mask, domain,
	                 COALESCE(vector, ''), COALESCE(source::text, '{}'), COALESCE(sink::text, '{}')
	          FROM "dsl-ob-poc".dictionary WHERE ` + whereClause

	err := s.db.QueryRowContext(ctx, query, param).Scan(
		&attr.AttributeID, &attr.Name, &attr.LongDescription, &attr.GroupID,
		&attr.Mask, &attr.Domain, &attr.Vector, &sourceJSON, &sinkJSON)

	if err == sql.ErrNoRows {
		return nil, fmt.Errorf(notFoundMsg, param)
	}
	if err != nil {
		return nil, fmt.Errorf("failed to get attribute: %w", err)
	}

	// Parse JSON metadata
	if parseErr := json.Unmarshal([]byte(sourceJSON), &attr.Source); parseErr != nil {
		return nil, fmt.Errorf("failed to parse source metadata: %w", parseErr)
	}
	if parseErr := json.Unmarshal([]byte(sinkJSON), &attr.Sink); parseErr != nil {
		return nil, fmt.Errorf("failed to parse sink metadata: %w", parseErr)
	}

	return &attr, nil
}

// GetDictionaryAttributeByName retrieves an attribute from the dictionary by name
func (s *Store) GetDictionaryAttributeByName(ctx context.Context, name string) (*dictionary.Attribute, error) {
	return s.getDictionaryAttribute(ctx, "name = $1", name, "attribute '%s' not found in dictionary")
}

// GetDictionaryAttributeByID retrieves an attribute from the dictionary by UUID
func (s *Store) GetDictionaryAttributeByID(ctx context.Context, id string) (*dictionary.Attribute, error) {
	return s.getDictionaryAttribute(ctx, "attribute_id = $1", id, "attribute with ID '%s' not found in dictionary")
}

// GetCBUByName retrieves a CBU by name from the catalog
func (s *Store) GetCBUByName(ctx context.Context, name string) (*CBU, error) {
	var cbu CBU
	err := s.db.QueryRowContext(ctx,
		`SELECT cbu_id, name, description, nature_purpose FROM "dsl-ob-poc".cbus WHERE name = $1`,
		name).Scan(&cbu.CBUID, &cbu.Name, &cbu.Description, &cbu.NaturePurpose)
	if err == sql.ErrNoRows {
		return nil, fmt.Errorf("CBU '%s' not found in catalog", name)
	}
	if err != nil {
		return nil, fmt.Errorf("failed to get CBU: %w", err)
	}
	return &cbu, nil
}

// ResolveValueFor resolves attribute values using source metadata
func (s *Store) ResolveValueFor(ctx context.Context, cbuID, attributeID string) (payload json.RawMessage, provenance map[string]any, status string, err error) {
	a, err := s.GetDictionaryAttributeByID(ctx, attributeID)
	if err != nil {
		return nil, nil, "", err
	}

	// Super simple: if source indicates "cbus" table, fetch by cbuID
	sourceMap := make(map[string]interface{})
	sourceJSON, _ := json.Marshal(a.Source)
	if parseErr := json.Unmarshal(sourceJSON, &sourceMap); parseErr != nil {
		return nil, nil, "", fmt.Errorf("failed to parse source metadata: %w", parseErr)
	}

	if table, ok := sourceMap["table"].(string); ok && table == "cbus" {
		if field, fieldOk := sourceMap["field"].(string); fieldOk && field != "" {
			query := fmt.Sprintf(`SELECT %s FROM "dsl-ob-poc".cbus WHERE cbu_id=$1`, field)
			var val interface{}
			scanErr := s.db.QueryRowContext(ctx, query, cbuID).Scan(&val)
			if scanErr != nil {
				return nil, nil, "", scanErr
			}
			payload, _ := json.Marshal(val)
			prov := map[string]any{"table": "cbus", "field": field}
			return payload, prov, "resolved", nil
		}
	}

	// Unknown source → pending solicit
	return json.RawMessage(`null`), map[string]any{"reason": "no_resolver"}, "pending", nil
}

// UpsertAttributeValue stores or updates an attribute value
func (s *Store) UpsertAttributeValue(ctx context.Context, cbuID string, dslVersion int, attributeID string, value json.RawMessage, state string, source map[string]any) error {
	srcJSON, _ := json.Marshal(source)
	_, err := s.db.ExecContext(ctx, `
		INSERT INTO "dsl-ob-poc".attribute_values (cbu_id, dsl_version, attribute_id, value, state, source)
		VALUES ($1, $2, $3, $4, $5, $6)
		ON CONFLICT (cbu_id, dsl_version, attribute_id)
		DO UPDATE SET value = EXCLUDED.value, state = EXCLUDED.state, source = EXCLUDED.source, observed_at = (now() at time zone 'utc')`,
		cbuID, dslVersion, attributeID, value, state, string(srcJSON))
	return err
}

// StoreAttributeValue is a simple wrapper for UpsertAttributeValue
func (s *Store) StoreAttributeValue(ctx context.Context, onboardingID, attributeID, value string, sourceInfo map[string]interface{}) error {
	valueJSON, _ := json.Marshal(value)
	// For POC, use dsl_version = 1
	return s.UpsertAttributeValue(ctx, onboardingID, 1, attributeID, valueJSON, "resolved", sourceInfo)
}

// GetProductByName retrieves a product by name from the catalog.
func (s *Store) GetProductByName(ctx context.Context, name string) (*Product, error) {
	var p Product
	err := s.db.QueryRowContext(ctx,
		`SELECT product_id, name, description FROM "dsl-ob-poc".products WHERE name = $1`,
		name).Scan(&p.ProductID, &p.Name, &p.Description)
	if err == sql.ErrNoRows {
		return nil, fmt.Errorf("product '%s' not found in catalog", name)
	}
	if err != nil {
		return nil, fmt.Errorf("failed to get product: %w", err)
	}
	return &p, nil
}

// GetServicesForProduct retrieves all services associated with a product.
func (s *Store) GetServicesForProduct(ctx context.Context, productID string) ([]Service, error) {
	rows, err := s.db.QueryContext(ctx,
		`SELECT s.service_id, s.name, s.description
         FROM "dsl-ob-poc".services s
         JOIN "dsl-ob-poc".product_services ps ON s.service_id = ps.service_id
		 WHERE ps.product_id = $1`,
		productID)
	if err != nil {
		return nil, fmt.Errorf("failed to query services: %w", err)
	}
	defer rows.Close()

	var services []Service
	for rows.Next() {
		var srv Service
		if scanErr := rows.Scan(&srv.ServiceID, &srv.Name, &srv.Description); scanErr != nil {
			return nil, fmt.Errorf("failed to scan service: %w", scanErr)
		}
		services = append(services, srv)
	}

	if rowsErr := rows.Err(); rowsErr != nil {
		return nil, fmt.Errorf("error iterating services: %w", rowsErr)
	}

	return services, nil
}

// GetServiceByName retrieves a service by name from the catalog.
func (s *Store) GetServiceByName(ctx context.Context, name string) (*Service, error) {
	var srv Service
	err := s.db.QueryRowContext(ctx,
		`SELECT service_id, name, description FROM "dsl-ob-poc".services WHERE name = $1`,
		name).Scan(&srv.ServiceID, &srv.Name, &srv.Description)
	if err == sql.ErrNoRows {
		return nil, fmt.Errorf("service '%s' not found in catalog", name)
	}
	if err != nil {
		return nil, fmt.Errorf("failed to get service: %w", err)
	}
	return &srv, nil
}

// GetResourcesForService retrieves all resources associated with a service.
func (s *Store) GetResourcesForService(ctx context.Context, serviceID string) ([]ProdResource, error) {
	rows, err := s.db.QueryContext(ctx,
		`SELECT r.resource_id, r.name, r.description, r.owner, COALESCE(r.dictionary_id::text, '')
         FROM "dsl-ob-poc".prod_resources r
         JOIN "dsl-ob-poc".service_resources sr ON r.resource_id = sr.resource_id
		 WHERE sr.service_id = $1`,
		serviceID)
	if err != nil {
		return nil, fmt.Errorf("failed to query resources: %w", err)
	}
	defer rows.Close()

	var resources []ProdResource
	for rows.Next() {
		var res ProdResource
		if scanErr := rows.Scan(&res.ResourceID, &res.Name, &res.Description, &res.Owner, &res.DictionaryGroup); scanErr != nil {
			return nil, fmt.Errorf("failed to scan resource: %w", scanErr)
		}
		resources = append(resources, res)
	}

	if rowsErr := rows.Err(); rowsErr != nil {
		return nil, fmt.Errorf("error iterating resources: %w", rowsErr)
	}

	return resources, nil
}

// Orchestration session persistence methods

// SaveOrchestrationSession persists an orchestration session to the database
func (s *Store) SaveOrchestrationSession(ctx context.Context, sessionData *OrchestrationSessionData) error {
	// Serialize JSON fields
	sharedContextJSON, err := json.Marshal(sessionData.SharedContext)
	if err != nil {
		return fmt.Errorf("failed to marshal shared context: %w", err)
	}

	executionPlanJSON, err := json.Marshal(sessionData.ExecutionPlan)
	if err != nil {
		return fmt.Errorf("failed to marshal execution plan: %w", err)
	}

	entityRefsJSON, err := json.Marshal(sessionData.EntityRefs)
	if err != nil {
		return fmt.Errorf("failed to marshal entity refs: %w", err)
	}

	attributeRefsJSON, err := json.Marshal(sessionData.AttributeRefs)
	if err != nil {
		return fmt.Errorf("failed to marshal attribute refs: %w", err)
	}

	expiresAt := time.Now().Add(24 * time.Hour) // Default 24 hour expiration

	// Upsert orchestration session
	query := `
		INSERT INTO "dsl-ob-poc".orchestration_sessions (
			session_id, primary_domain, cbu_id, entity_type, entity_name,
			jurisdiction, products, services, workflow_type, current_state,
			version_number, unified_dsl, shared_context, execution_plan,
			entity_refs, attribute_refs, created_at, updated_at, last_used, expires_at
		) VALUES (
			$1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20
		)
		ON CONFLICT (session_id) DO UPDATE SET
			primary_domain = EXCLUDED.primary_domain,
			current_state = EXCLUDED.current_state,
			version_number = EXCLUDED.version_number,
			unified_dsl = EXCLUDED.unified_dsl,
			shared_context = EXCLUDED.shared_context,
			execution_plan = EXCLUDED.execution_plan,
			entity_refs = EXCLUDED.entity_refs,
			attribute_refs = EXCLUDED.attribute_refs,
			updated_at = EXCLUDED.updated_at,
			last_used = EXCLUDED.last_used,
			expires_at = EXCLUDED.expires_at
	`

	_, err = s.db.ExecContext(ctx, query,
		sessionData.SessionID,
		sessionData.PrimaryDomain,
		sessionData.CBUID,
		sessionData.EntityType,
		sessionData.EntityName,
		sessionData.Jurisdiction,
		pq.Array(sessionData.Products),
		pq.Array(sessionData.Services),
		sessionData.WorkflowType,
		sessionData.CurrentState,
		sessionData.VersionNumber,
		sessionData.UnifiedDSL,
		sharedContextJSON,
		executionPlanJSON,
		entityRefsJSON,
		attributeRefsJSON,
		sessionData.CreatedAt,
		sessionData.UpdatedAt,
		sessionData.LastUsed,
		expiresAt,
	)
	if err != nil {
		return fmt.Errorf("failed to save orchestration session: %w", err)
	}

	// Save domain sessions
	if err := s.saveDomainSessions(ctx, sessionData); err != nil {
		return fmt.Errorf("failed to save domain sessions: %w", err)
	}

	return nil
}

// LoadOrchestrationSession retrieves an orchestration session from the database
func (s *Store) LoadOrchestrationSession(ctx context.Context, sessionID string) (*OrchestrationSessionData, error) {
	query := `
		SELECT session_id, primary_domain, cbu_id, entity_type, entity_name,
			   jurisdiction, products, services, workflow_type, current_state,
			   version_number, unified_dsl, shared_context, execution_plan,
			   entity_refs, attribute_refs, created_at, updated_at, last_used
		FROM "dsl-ob-poc".orchestration_sessions
		WHERE session_id = $1 AND expires_at > NOW()
	`

	row := s.db.QueryRowContext(ctx, query, sessionID)

	var session OrchestrationSessionData
	var products, services pq.StringArray
	var sharedContextJSON, executionPlanJSON, entityRefsJSON, attributeRefsJSON []byte

	err := row.Scan(
		&session.SessionID,
		&session.PrimaryDomain,
		&session.CBUID,
		&session.EntityType,
		&session.EntityName,
		&session.Jurisdiction,
		&products,
		&services,
		&session.WorkflowType,
		&session.CurrentState,
		&session.VersionNumber,
		&session.UnifiedDSL,
		&sharedContextJSON,
		&executionPlanJSON,
		&entityRefsJSON,
		&attributeRefsJSON,
		&session.CreatedAt,
		&session.UpdatedAt,
		&session.LastUsed,
	)
	if err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return nil, fmt.Errorf("orchestration session not found: %s", sessionID)
		}
		return nil, fmt.Errorf("failed to load orchestration session: %w", err)
	}

	session.Products = []string(products)
	session.Services = []string(services)

	// Deserialize JSON fields
	if err := json.Unmarshal(sharedContextJSON, &session.SharedContext); err != nil {
		return nil, fmt.Errorf("failed to unmarshal shared context: %w", err)
	}

	if err := json.Unmarshal(executionPlanJSON, &session.ExecutionPlan); err != nil {
		return nil, fmt.Errorf("failed to unmarshal execution plan: %w", err)
	}

	if err := json.Unmarshal(entityRefsJSON, &session.EntityRefs); err != nil {
		return nil, fmt.Errorf("failed to unmarshal entity refs: %w", err)
	}

	if err := json.Unmarshal(attributeRefsJSON, &session.AttributeRefs); err != nil {
		return nil, fmt.Errorf("failed to unmarshal attribute refs: %w", err)
	}

	// Load domain sessions
	domainSessions, err := s.loadDomainSessions(ctx, sessionID)
	if err != nil {
		return nil, fmt.Errorf("failed to load domain sessions: %w", err)
	}
	session.DomainSessions = domainSessions

	// Load state history
	stateHistory, err := s.loadStateHistory(ctx, sessionID)
	if err != nil {
		return nil, fmt.Errorf("failed to load state history: %w", err)
	}
	session.StateHistory = stateHistory

	// Update last used timestamp
	s.updateLastUsed(ctx, sessionID)

	return &session, nil
}

// ListActiveOrchestrationSessions returns IDs of all active sessions
func (s *Store) ListActiveOrchestrationSessions(ctx context.Context) ([]string, error) {
	query := `
		SELECT session_id
		FROM "dsl-ob-poc".orchestration_sessions
		WHERE expires_at > NOW()
		ORDER BY last_used DESC
	`

	rows, err := s.db.QueryContext(ctx, query)
	if err != nil {
		return nil, fmt.Errorf("failed to list active sessions: %w", err)
	}
	defer rows.Close()

	var sessionIDs []string
	for rows.Next() {
		var sessionID string
		if err := rows.Scan(&sessionID); err != nil {
			return nil, fmt.Errorf("failed to scan session ID: %w", err)
		}
		sessionIDs = append(sessionIDs, sessionID)
	}

	return sessionIDs, nil
}

// DeleteOrchestrationSession removes a session and all its related data
func (s *Store) DeleteOrchestrationSession(ctx context.Context, sessionID string) error {
	query := `DELETE FROM "dsl-ob-poc".orchestration_sessions WHERE session_id = $1`

	result, err := s.db.ExecContext(ctx, query, sessionID)
	if err != nil {
		return fmt.Errorf("failed to delete session: %w", err)
	}

	rowsAffected, err := result.RowsAffected()
	if err != nil {
		return fmt.Errorf("failed to get rows affected: %w", err)
	}

	if rowsAffected == 0 {
		return fmt.Errorf("session not found: %s", sessionID)
	}

	return nil
}

// CleanupExpiredOrchestrationSessions removes expired sessions
func (s *Store) CleanupExpiredOrchestrationSessions(ctx context.Context) (int64, error) {
	query := `DELETE FROM "dsl-ob-poc".orchestration_sessions WHERE expires_at <= NOW()`

	result, err := s.db.ExecContext(ctx, query)
	if err != nil {
		return 0, fmt.Errorf("failed to cleanup expired sessions: %w", err)
	}

	rowsAffected, err := result.RowsAffected()
	if err != nil {
		return 0, fmt.Errorf("failed to get rows affected: %w", err)
	}

	return rowsAffected, nil
}

// UpdateOrchestrationSessionDSL updates the unified DSL and version for a session
func (s *Store) UpdateOrchestrationSessionDSL(ctx context.Context, sessionID, dsl string, version int) error {
	query := `
		UPDATE "dsl-ob-poc".orchestration_sessions
		SET unified_dsl = $2, version_number = $3, updated_at = NOW(), last_used = NOW()
		WHERE session_id = $1
	`

	result, err := s.db.ExecContext(ctx, query, sessionID, dsl, version)
	if err != nil {
		return fmt.Errorf("failed to update session DSL: %w", err)
	}

	rowsAffected, err := result.RowsAffected()
	if err != nil {
		return fmt.Errorf("failed to get rows affected: %w", err)
	}

	if rowsAffected == 0 {
		return fmt.Errorf("session not found: %s", sessionID)
	}

	return nil
}

// Helper methods for orchestration session persistence

func (s *Store) saveDomainSessions(ctx context.Context, sessionData *OrchestrationSessionData) error {
	// Delete existing domain sessions for this orchestration session
	deleteQuery := `DELETE FROM "dsl-ob-poc".orchestration_domain_sessions WHERE orchestration_session_id = $1`
	_, err := s.db.ExecContext(ctx, deleteQuery, sessionData.SessionID)
	if err != nil {
		return fmt.Errorf("failed to delete existing domain sessions: %w", err)
	}

	// Insert current domain sessions
	insertQuery := `
		INSERT INTO "dsl-ob-poc".orchestration_domain_sessions (
			orchestration_session_id, domain_name, domain_session_id, state,
			contributed_dsl, domain_context, dependencies, last_activity, created_at
		) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
	`

	for _, domainSession := range sessionData.DomainSessions {
		contextJSON, err := json.Marshal(domainSession.Context)
		if err != nil {
			return fmt.Errorf("failed to marshal domain context for %s: %w", domainSession.DomainName, err)
		}

		_, err = s.db.ExecContext(ctx, insertQuery,
			sessionData.SessionID,
			domainSession.DomainName,
			domainSession.DomainSessionID,
			domainSession.State,
			domainSession.ContributedDSL,
			contextJSON,
			pq.Array(domainSession.Dependencies),
			domainSession.LastActivity,
			time.Now(),
		)
		if err != nil {
			return fmt.Errorf("failed to save domain session for %s: %w", domainSession.DomainName, err)
		}
	}

	return nil
}

func (s *Store) loadDomainSessions(ctx context.Context, sessionID string) ([]DomainSessionData, error) {
	query := `
		SELECT domain_name, domain_session_id, state, contributed_dsl,
			   domain_context, dependencies, last_activity
		FROM "dsl-ob-poc".orchestration_domain_sessions
		WHERE orchestration_session_id = $1
	`

	rows, err := s.db.QueryContext(ctx, query, sessionID)
	if err != nil {
		return nil, fmt.Errorf("failed to query domain sessions: %w", err)
	}
	defer rows.Close()

	var domainSessions []DomainSessionData

	for rows.Next() {
		var domainSession DomainSessionData
		var contextJSON []byte
		var dependencies pq.StringArray

		err := rows.Scan(
			&domainSession.DomainName,
			&domainSession.DomainSessionID,
			&domainSession.State,
			&domainSession.ContributedDSL,
			&contextJSON,
			&dependencies,
			&domainSession.LastActivity,
		)
		if err != nil {
			return nil, fmt.Errorf("failed to scan domain session: %w", err)
		}

		// Unmarshal context
		if err := json.Unmarshal(contextJSON, &domainSession.Context); err != nil {
			return nil, fmt.Errorf("failed to unmarshal domain context: %w", err)
		}

		domainSession.Dependencies = []string(dependencies)
		domainSessions = append(domainSessions, domainSession)
	}

	return domainSessions, nil
}

func (s *Store) loadStateHistory(ctx context.Context, sessionID string) ([]StateTransitionData, error) {
	query := `
		SELECT from_state, to_state, domain_name, reason, generated_by, created_at
		FROM "dsl-ob-poc".orchestration_state_history
		WHERE orchestration_session_id = $1
		ORDER BY created_at ASC
	`

	rows, err := s.db.QueryContext(ctx, query, sessionID)
	if err != nil {
		return nil, fmt.Errorf("failed to query state history: %w", err)
	}
	defer rows.Close()

	var stateHistory []StateTransitionData

	for rows.Next() {
		var transition StateTransitionData
		var fromState, domain, reason, generatedBy sql.NullString

		err := rows.Scan(
			&fromState,
			&transition.ToState,
			&domain,
			&reason,
			&generatedBy,
			&transition.Timestamp,
		)
		if err != nil {
			return nil, fmt.Errorf("failed to scan state transition: %w", err)
		}

		if fromState.Valid {
			transition.FromState = fromState.String
		}
		if domain.Valid {
			transition.Domain = domain.String
		}
		if reason.Valid {
			transition.Reason = reason.String
		}
		if generatedBy.Valid {
			transition.GeneratedBy = generatedBy.String
		}

		stateHistory = append(stateHistory, transition)
	}

	return stateHistory, nil
}

func (s *Store) updateLastUsed(ctx context.Context, sessionID string) {
	// Update in background, don't block on this
	go func() {
		query := `UPDATE "dsl-ob-poc".orchestration_sessions SET last_used = NOW() WHERE session_id = $1`
		s.db.ExecContext(context.Background(), query, sessionID)
	}()
}

// OrchestrationSessionData represents the data structure for persistent orchestration sessions
type OrchestrationSessionData struct {
	SessionID      string                 `json:"session_id"`
	PrimaryDomain  string                 `json:"primary_domain"`
	CBUID          *string                `json:"cbu_id,omitempty"`
	EntityType     *string                `json:"entity_type,omitempty"`
	EntityName     *string                `json:"entity_name,omitempty"`
	Jurisdiction   *string                `json:"jurisdiction,omitempty"`
	Products       []string               `json:"products,omitempty"`
	Services       []string               `json:"services,omitempty"`
	WorkflowType   *string                `json:"workflow_type,omitempty"`
	CurrentState   string                 `json:"current_state"`
	VersionNumber  int                    `json:"version_number"`
	UnifiedDSL     string                 `json:"unified_dsl"`
	SharedContext  map[string]interface{} `json:"shared_context"`
	ExecutionPlan  map[string]interface{} `json:"execution_plan"`
	EntityRefs     map[string]string      `json:"entity_refs"`
	AttributeRefs  map[string]string      `json:"attribute_refs"`
	DomainSessions []DomainSessionData    `json:"domain_sessions"`
	StateHistory   []StateTransitionData  `json:"state_history"`
	CreatedAt      time.Time              `json:"created_at"`
	UpdatedAt      time.Time              `json:"updated_at"`
	LastUsed       time.Time              `json:"last_used"`
}

// DomainSessionData represents a domain session within orchestration
type DomainSessionData struct {
	DomainName      string                 `json:"domain_name"`
	DomainSessionID string                 `json:"domain_session_id"`
	State           string                 `json:"state"`
	ContributedDSL  string                 `json:"contributed_dsl"`
	Context         map[string]interface{} `json:"context"`
	Dependencies    []string               `json:"dependencies"`
	LastActivity    time.Time              `json:"last_activity"`
}

// StateTransitionData represents a state transition record
type StateTransitionData struct {
	FromState   string    `json:"from_state"`
	ToState     string    `json:"to_state"`
	Domain      string    `json:"domain,omitempty"`
	Reason      string    `json:"reason,omitempty"`
	GeneratedBy string    `json:"generated_by,omitempty"`
	Timestamp   time.Time `json:"timestamp"`
}

// GetAttributesForDictionaryGroup retrieves all attributes for a given dictionary group.
func (s *Store) GetAttributesForDictionaryGroup(ctx context.Context, groupID string) ([]dictionary.Attribute, error) {
	rows, err := s.db.QueryContext(ctx,
		`SELECT attribute_id, name, COALESCE(long_description, ''), group_id,
                COALESCE(mask, 'string'), COALESCE(domain, ''), COALESCE(vector, ''),
                COALESCE(source::text, '{}'), COALESCE(sink::text, '{}')
         FROM "dsl-ob-poc".dictionary
         WHERE group_id = $1`,
		groupID)
	if err != nil {
		return nil, fmt.Errorf("failed to query dictionary attributes: %w", err)
	}
	defer rows.Close()

	var attributes []dictionary.Attribute
	for rows.Next() {
		var attr dictionary.Attribute
		var sourceJSON, sinkJSON string

		if scanErr := rows.Scan(&attr.AttributeID, &attr.Name, &attr.LongDescription,
			&attr.GroupID, &attr.Mask, &attr.Domain, &attr.Vector,
			&sourceJSON, &sinkJSON); scanErr != nil {
			return nil, fmt.Errorf("failed to scan attribute: %w", scanErr)
		}

		// Parse JSON metadata
		if sourceErr := json.Unmarshal([]byte(sourceJSON), &attr.Source); sourceErr != nil {
			return nil, fmt.Errorf("failed to parse source metadata: %w", sourceErr)
		}
		if sinkErr := json.Unmarshal([]byte(sinkJSON), &attr.Sink); sinkErr != nil {
			return nil, fmt.Errorf("failed to parse sink metadata: %w", sinkErr)
		}

		attributes = append(attributes, attr)
	}

	if rowsErr := rows.Err(); rowsErr != nil {
		return nil, fmt.Errorf("error iterating attributes: %w", rowsErr)
	}

	return attributes, nil
}

// ============================================================================
// CBU CRUD OPERATIONS
// ============================================================================

// CreateCBU creates a new CBU
func (s *Store) CreateCBU(ctx context.Context, name, description, naturePurpose string) (string, error) {
	query := `INSERT INTO "dsl-ob-poc".cbus (name, description, nature_purpose)
	         VALUES ($1, $2, $3) RETURNING cbu_id`

	var cbuID string
	err := s.db.QueryRowContext(ctx, query, name, description, naturePurpose).Scan(&cbuID)
	if err != nil {
		return "", fmt.Errorf("failed to create CBU: %w", err)
	}

	return cbuID, nil
}

// ListCBUs retrieves all CBUs
func (s *Store) ListCBUs(ctx context.Context) ([]CBU, error) {
	query := `SELECT cbu_id, name, description, nature_purpose
	         FROM "dsl-ob-poc".cbus
	         ORDER BY name`

	rows, err := s.db.QueryContext(ctx, query)
	if err != nil {
		return nil, fmt.Errorf("failed to list CBUs: %w", err)
	}
	defer rows.Close()

	var cbus []CBU
	for rows.Next() {
		var cbu CBU
		if scanErr := rows.Scan(&cbu.CBUID, &cbu.Name, &cbu.Description, &cbu.NaturePurpose); scanErr != nil {
			return nil, fmt.Errorf("failed to scan CBU: %w", scanErr)
		}
		cbus = append(cbus, cbu)
	}

	if rowsErr := rows.Err(); rowsErr != nil {
		return nil, fmt.Errorf("error iterating CBUs: %w", rowsErr)
	}

	return cbus, nil
}

// GetCBUByID retrieves a CBU by ID
func (s *Store) GetCBUByID(ctx context.Context, cbuID string) (*CBU, error) {
	query := `SELECT cbu_id, name, description, nature_purpose
	         FROM "dsl-ob-poc".cbus
	         WHERE cbu_id = $1`

	var cbu CBU
	err := s.db.QueryRowContext(ctx, query, cbuID).Scan(
		&cbu.CBUID, &cbu.Name, &cbu.Description, &cbu.NaturePurpose)
	if err != nil {
		if err == sql.ErrNoRows {
			return nil, fmt.Errorf("CBU not found: %s", cbuID)
		}
		return nil, fmt.Errorf("failed to get CBU: %w", err)
	}

	return &cbu, nil
}

// UpdateCBU updates a CBU
func (s *Store) UpdateCBU(ctx context.Context, cbuID, name, description, naturePurpose string) error {
	setParts := []string{}
	args := []interface{}{}
	argIndex := 1

	if name != "" {
		setParts = append(setParts, fmt.Sprintf("name = $%d", argIndex))
		args = append(args, name)
		argIndex++
	}
	if description != "" {
		setParts = append(setParts, fmt.Sprintf("description = $%d", argIndex))
		args = append(args, description)
		argIndex++
	}
	if naturePurpose != "" {
		setParts = append(setParts, fmt.Sprintf("nature_purpose = $%d", argIndex))
		args = append(args, naturePurpose)
		argIndex++
	}

	if len(setParts) == 0 {
		return fmt.Errorf("no fields to update")
	}

	setParts = append(setParts, fmt.Sprintf("updated_at = $%d", argIndex))
	args = append(args, time.Now())
	argIndex++

	args = append(args, cbuID)

	query := fmt.Sprintf(`UPDATE "dsl-ob-poc".cbus SET %s WHERE cbu_id = $%d`,
		strings.Join(setParts, ", "), argIndex)

	result, err := s.db.ExecContext(ctx, query, args...)
	if err != nil {
		return fmt.Errorf("failed to update CBU: %w", err)
	}

	rowsAffected, err := result.RowsAffected()
	if err != nil {
		return fmt.Errorf("failed to get rows affected: %w", err)
	}

	if rowsAffected == 0 {
		return fmt.Errorf("CBU not found: %s", cbuID)
	}

	return nil
}

// DeleteCBU deletes a CBU
func (s *Store) DeleteCBU(ctx context.Context, cbuID string) error {
	query := `DELETE FROM "dsl-ob-poc".cbus WHERE cbu_id = $1`

	result, err := s.db.ExecContext(ctx, query, cbuID)
	if err != nil {
		return fmt.Errorf("failed to delete CBU: %w", err)
	}

	rowsAffected, err := result.RowsAffected()
	if err != nil {
		return fmt.Errorf("failed to get rows affected: %w", err)
	}

	if rowsAffected == 0 {
		return fmt.Errorf("CBU not found: %s", cbuID)
	}

	return nil
}

// ============================================================================
// ROLE CRUD OPERATIONS
// ============================================================================

// CreateRole creates a new role
func (s *Store) CreateRole(ctx context.Context, name, description string) (string, error) {
	query := `INSERT INTO "dsl-ob-poc".roles (name, description)
	         VALUES ($1, $2) RETURNING role_id`

	var roleID string
	err := s.db.QueryRowContext(ctx, query, name, description).Scan(&roleID)
	if err != nil {
		return "", fmt.Errorf("failed to create role: %w", err)
	}

	return roleID, nil
}

// ListRoles retrieves all roles
func (s *Store) ListRoles(ctx context.Context) ([]Role, error) {
	query := `SELECT role_id, name, description
	         FROM "dsl-ob-poc".roles
	         ORDER BY name`

	rows, err := s.db.QueryContext(ctx, query)
	if err != nil {
		return nil, fmt.Errorf("failed to list roles: %w", err)
	}
	defer rows.Close()

	var roles []Role
	for rows.Next() {
		var role Role
		if scanErr := rows.Scan(&role.RoleID, &role.Name, &role.Description); scanErr != nil {
			return nil, fmt.Errorf("failed to scan role: %w", scanErr)
		}
		roles = append(roles, role)
	}

	if rowsErr := rows.Err(); rowsErr != nil {
		return nil, fmt.Errorf("error iterating roles: %w", rowsErr)
	}

	return roles, nil
}

// GetRoleByID retrieves a role by ID
func (s *Store) GetRoleByID(ctx context.Context, roleID string) (*Role, error) {
	query := `SELECT role_id, name, description
	         FROM "dsl-ob-poc".roles
	         WHERE role_id = $1`

	var role Role
	err := s.db.QueryRowContext(ctx, query, roleID).Scan(
		&role.RoleID, &role.Name, &role.Description)
	if err != nil {
		if err == sql.ErrNoRows {
			return nil, fmt.Errorf("role not found: %s", roleID)
		}
		return nil, fmt.Errorf("failed to get role: %w", err)
	}

	return &role, nil
}

// UpdateRole updates a role
func (s *Store) UpdateRole(ctx context.Context, roleID, name, description string) error {
	setParts := []string{}
	args := []interface{}{}
	argIndex := 1

	if name != "" {
		setParts = append(setParts, fmt.Sprintf("name = $%d", argIndex))
		args = append(args, name)
		argIndex++
	}
	if description != "" {
		setParts = append(setParts, fmt.Sprintf("description = $%d", argIndex))
		args = append(args, description)
		argIndex++
	}

	if len(setParts) == 0 {
		return fmt.Errorf("no fields to update")
	}

	setParts = append(setParts, fmt.Sprintf("updated_at = $%d", argIndex))
	args = append(args, time.Now())
	argIndex++

	args = append(args, roleID)

	query := fmt.Sprintf(`UPDATE "dsl-ob-poc".roles SET %s WHERE role_id = $%d`,
		strings.Join(setParts, ", "), argIndex)

	result, err := s.db.ExecContext(ctx, query, args...)
	if err != nil {
		return fmt.Errorf("failed to update role: %w", err)
	}

	rowsAffected, err := result.RowsAffected()
	if err != nil {
		return fmt.Errorf("failed to get rows affected: %w", err)
	}

	if rowsAffected == 0 {
		return fmt.Errorf("role not found: %s", roleID)
	}

	return nil
}

// DeleteRole deletes a role
func (s *Store) DeleteRole(ctx context.Context, roleID string) error {
	query := `DELETE FROM "dsl-ob-poc".roles WHERE role_id = $1`

	result, err := s.db.ExecContext(ctx, query, roleID)
	if err != nil {
		return fmt.Errorf("failed to delete role: %w", err)
	}

	rowsAffected, err := result.RowsAffected()
	if err != nil {
		return fmt.Errorf("failed to get rows affected: %w", err)
	}

	if rowsAffected == 0 {
		return fmt.Errorf("role not found: %s", roleID)
	}

	return nil
}

// ============================================================================
// EXPORT OPERATIONS (for mock data generation and testing)
// ============================================================================

// GetAllProducts retrieves all products from the catalog
func (s *Store) GetAllProducts(ctx context.Context) ([]Product, error) {
	query := `SELECT product_id, name, description
	         FROM "dsl-ob-poc".products
	         ORDER BY name`

	rows, err := s.db.QueryContext(ctx, query)
	if err != nil {
		return nil, fmt.Errorf("failed to get all products: %w", err)
	}
	defer rows.Close()

	var products []Product
	for rows.Next() {
		var p Product
		if scanErr := rows.Scan(&p.ProductID, &p.Name, &p.Description); scanErr != nil {
			return nil, fmt.Errorf("failed to scan product: %w", scanErr)
		}
		products = append(products, p)
	}

	if rowsErr := rows.Err(); rowsErr != nil {
		return nil, fmt.Errorf("error iterating products: %w", rowsErr)
	}

	return products, nil
}

// GetAllServices retrieves all services from the catalog
func (s *Store) GetAllServices(ctx context.Context) ([]Service, error) {
	query := `SELECT service_id, name, description
	         FROM "dsl-ob-poc".services
	         ORDER BY name`

	rows, err := s.db.QueryContext(ctx, query)
	if err != nil {
		return nil, fmt.Errorf("failed to get all services: %w", err)
	}
	defer rows.Close()

	var services []Service
	for rows.Next() {
		var s Service
		if scanErr := rows.Scan(&s.ServiceID, &s.Name, &s.Description); scanErr != nil {
			return nil, fmt.Errorf("failed to scan service: %w", scanErr)
		}
		services = append(services, s)
	}

	if rowsErr := rows.Err(); rowsErr != nil {
		return nil, fmt.Errorf("error iterating services: %w", rowsErr)
	}

	return services, nil
}

// GetAllDictionaryAttributes retrieves all dictionary attributes
func (s *Store) GetAllDictionaryAttributes(ctx context.Context) ([]dictionary.Attribute, error) {
	query := `SELECT attribute_id, name, COALESCE(long_description, ''),
	                 COALESCE(group_id, ''), COALESCE(mask, 'string'),
	                 COALESCE(domain, ''), COALESCE(vector, ''),
	                 COALESCE(source::text, '{}'), COALESCE(sink::text, '{}')
	         FROM "dsl-ob-poc".dictionary
	         ORDER BY name`

	rows, err := s.db.QueryContext(ctx, query)
	if err != nil {
		return nil, fmt.Errorf("failed to get all dictionary attributes: %w", err)
	}
	defer rows.Close()

	var attributes []dictionary.Attribute
	for rows.Next() {
		var attr dictionary.Attribute
		var sourceJSON, sinkJSON string

		if scanErr := rows.Scan(&attr.AttributeID, &attr.Name, &attr.LongDescription,
			&attr.GroupID, &attr.Mask, &attr.Domain, &attr.Vector,
			&sourceJSON, &sinkJSON); scanErr != nil {
			return nil, fmt.Errorf("failed to scan dictionary attribute: %w", scanErr)
		}

		// Parse JSON metadata
		if sourceErr := json.Unmarshal([]byte(sourceJSON), &attr.Source); sourceErr != nil {
			return nil, fmt.Errorf("failed to parse source metadata: %w", sourceErr)
		}
		if sinkErr := json.Unmarshal([]byte(sinkJSON), &attr.Sink); sinkErr != nil {
			return nil, fmt.Errorf("failed to parse sink metadata: %w", sinkErr)
		}

		attributes = append(attributes, attr)
	}

	if rowsErr := rows.Err(); rowsErr != nil {
		return nil, fmt.Errorf("error iterating dictionary attributes: %w", rowsErr)
	}

	return attributes, nil
}

// GetAllDSLRecords retrieves all DSL records with state information
func (s *Store) GetAllDSLRecords(ctx context.Context) ([]DSLVersionWithState, error) {
	query := `SELECT d.version_id::text, d.cbu_id, d.dsl_text,
	                 COALESCE(o.current_state, 'CREATED'),
	                 ROW_NUMBER() OVER (PARTITION BY d.cbu_id ORDER BY d.created_at) as version_number,
	                 d.created_at
	         FROM "dsl-ob-poc".dsl_ob d
	         LEFT JOIN "dsl-ob-poc".onboarding_sessions o ON d.cbu_id = o.cbu_id
	         ORDER BY d.cbu_id, d.created_at`

	rows, err := s.db.QueryContext(ctx, query)
	if err != nil {
		return nil, fmt.Errorf("failed to get all DSL records: %w", err)
	}
	defer rows.Close()

	var records []DSLVersionWithState
	for rows.Next() {
		var record DSLVersionWithState
		var stateStr string

		if scanErr := rows.Scan(&record.VersionID, &record.CBUID, &record.DSLText,
			&stateStr, &record.VersionNumber, &record.CreatedAt); scanErr != nil {
			return nil, fmt.Errorf("failed to scan DSL record: %w", scanErr)
		}

		// Parse state string to OnboardingState enum
		record.OnboardingState = parseOnboardingState(stateStr)

		records = append(records, record)
	}

	if rowsErr := rows.Err(); rowsErr != nil {
		return nil, fmt.Errorf("error iterating DSL records: %w", rowsErr)
	}

	return records, nil
}

// parseOnboardingState converts string state to OnboardingState enum
func parseOnboardingState(stateStr string) OnboardingState {
	switch stateStr {
	case "CREATED":
		return StateCreated
	case "PRODUCTS_ADDED":
		return StateProductsAdded
	case "KYC_DISCOVERED":
		return StateKYCDiscovered
	case "SERVICES_DISCOVERED":
		return StateServicesDiscovered
	case "RESOURCES_DISCOVERED":
		return StateResourcesDiscovered
	case "ATTRIBUTES_POPULATED":
		return StateAttributesPopulated
	case "COMPLETED":
		return StateCompleted
	default:
		return StateCreated
	}
}

// ============================================================================
// PRODUCT REQUIREMENTS OPERATIONS (PHASE 5)
// ============================================================================

// SeedProductRequirements populates the database with initial product requirements data
func (s *Store) SeedProductRequirements(ctx context.Context) error {
	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return fmt.Errorf("failed to begin transaction: %w", err)
	}
	defer func() {
		_ = tx.Rollback()
	}()

	// Get existing product IDs
	productMap := make(map[string]string)
	rows, err := tx.QueryContext(ctx, `SELECT product_id, name FROM "dsl-ob-poc".products`)
	if err != nil {
		return fmt.Errorf("failed to query products: %w", err)
	}
	defer rows.Close()

	for rows.Next() {
		var id, name string
		if err := rows.Scan(&id, &name); err != nil {
			return fmt.Errorf("failed to scan product: %w", err)
		}
		productMap[name] = id
	}

	// Seed product requirements
	productReqs := []struct {
		name             string
		entityTypes      []string
		requiredDSL      []string
		attributes       []string
		compliance       []map[string]interface{}
		prerequisites    []string
		conditionalRules []map[string]interface{}
	}{
		{
			name:        "CUSTODY",
			entityTypes: []string{"TRUST", "CORPORATION", "PARTNERSHIP", "PROPER_PERSON"},
			requiredDSL: []string{"custody.account-setup", "custody.signatory-verification", "custody.asset-safekeeping"},
			attributes:  []string{"custody.account_number", "custody.signatory_authority", "custody.asset_types"},
			compliance: []map[string]interface{}{
				{
					"rule_id":     "CUSTODY_FINCEN_CONTROL",
					"framework":   "FINCEN",
					"description": "FinCEN control prong verification required for custody services",
					"required":    true,
				},
			},
			prerequisites: []string{"kyc.complete", "aml.screening-complete"},
			conditionalRules: []map[string]interface{}{
				{
					"condition":    "entity_type == 'TRUST'",
					"required_dsl": []string{"custody.trust-specific-verification"},
					"attributes":   []string{"trust.deed_verification", "trust.beneficiary_disclosure"},
				},
			},
		},
		{
			name:        "FUND_ACCOUNTING",
			entityTypes: []string{"CORPORATION", "TRUST", "PARTNERSHIP"},
			requiredDSL: []string{"accounting.nav-calculation", "accounting.reporting-setup", "accounting.compliance-monitoring"},
			attributes:  []string{"accounting.nav_value", "accounting.reporting_frequency", "accounting.base_currency"},
			compliance: []map[string]interface{}{
				{
					"rule_id":     "ACCOUNTING_GAAP_COMPLIANCE",
					"framework":   "GAAP",
					"description": "Generally Accepted Accounting Principles compliance required",
					"required":    true,
				},
			},
			prerequisites: []string{"custody.account-setup", "entity.legal_structure_verified"},
		},
		{
			name:        "TRANSFER_AGENCY",
			entityTypes: []string{"CORPORATION", "TRUST"},
			requiredDSL: []string{"transfer.registry-setup", "transfer.shareholder-services", "transfer.dividend-processing"},
			attributes:  []string{"transfer.share_classes", "transfer.dividend_policy", "transfer.registry_type"},
			compliance: []map[string]interface{}{
				{
					"rule_id":     "TRANSFER_SEC_COMPLIANCE",
					"framework":   "SEC",
					"description": "Securities and Exchange Commission transfer agency regulations",
					"required":    true,
				},
			},
			prerequisites: []string{"entity.incorporation_verified", "custody.account-setup"},
			conditionalRules: []map[string]interface{}{
				{
					"condition":    "entity_type == 'TRUST'",
					"required_dsl": []string{"transfer.trust-unit-tracking"},
					"attributes":   []string{"trust.unit_classes", "trust.distribution_rights"},
				},
			},
		},
	}

	for _, req := range productReqs {
		productID, exists := productMap[req.name]
		if !exists {
			continue // Skip if product doesn't exist
		}

		// Marshal JSON fields
		entityTypesJSON, _ := json.Marshal(req.entityTypes)
		requiredDSLJSON, _ := json.Marshal(req.requiredDSL)
		attributesJSON, _ := json.Marshal(req.attributes)
		complianceJSON, _ := json.Marshal(req.compliance)
		prerequisitesJSON, _ := json.Marshal(req.prerequisites)
		conditionalRulesJSON, _ := json.Marshal(req.conditionalRules)

		_, err := tx.ExecContext(ctx, `
			INSERT INTO "dsl-ob-poc".product_requirements
			(product_id, entity_types, required_dsl, attributes, compliance, prerequisites, conditional_rules)
			VALUES ($1, $2, $3, $4, $5, $6, $7)
			ON CONFLICT (product_id) DO UPDATE SET
				entity_types = EXCLUDED.entity_types,
				required_dsl = EXCLUDED.required_dsl,
				attributes = EXCLUDED.attributes,
				compliance = EXCLUDED.compliance,
				prerequisites = EXCLUDED.prerequisites,
				conditional_rules = EXCLUDED.conditional_rules,
				updated_at = NOW()`,
			productID, entityTypesJSON, requiredDSLJSON, attributesJSON,
			complianceJSON, prerequisitesJSON, conditionalRulesJSON)
		if err != nil {
			return fmt.Errorf("failed to insert product requirements for %s: %w", req.name, err)
		}
	}

	// Seed entity-product mappings
	entityMappings := []struct {
		entityType     string
		productName    string
		compatible     bool
		restrictions   []string
		requiredFields []string
	}{
		{"TRUST", "CUSTODY", true, []string{}, []string{"trust_deed", "beneficiary_list"}},
		{"TRUST", "FUND_ACCOUNTING", true, []string{}, []string{"accounting_method", "distribution_policy"}},
		{"TRUST", "TRANSFER_AGENCY", true, []string{}, []string{"unit_classes", "registry_type"}},
		{"CORPORATION", "CUSTODY", true, []string{}, []string{"board_resolution", "authorized_signatories"}},
		{"CORPORATION", "FUND_ACCOUNTING", true, []string{}, []string{"corporate_structure", "reporting_standards"}},
		{"CORPORATION", "TRANSFER_AGENCY", true, []string{}, []string{"share_classes", "transfer_restrictions"}},
		{"PARTNERSHIP", "CUSTODY", true, []string{}, []string{"partnership_agreement", "general_partner_authority"}},
		{"PARTNERSHIP", "FUND_ACCOUNTING", true, []string{}, []string{"capital_account_method", "allocation_method"}},
		{"PARTNERSHIP", "TRANSFER_AGENCY", false, []string{"Partnerships typically use different investor tracking mechanisms"}, []string{}},
		{"PROPER_PERSON", "CUSTODY", true, []string{}, []string{"identity_verification", "investment_capacity"}},
		{"PROPER_PERSON", "FUND_ACCOUNTING", false, []string{"Fund accounting typically not required for individual accounts"}, []string{}},
		{"PROPER_PERSON", "TRANSFER_AGENCY", false, []string{"Transfer agency services not applicable to individual accounts"}, []string{}},
	}

	for _, mapping := range entityMappings {
		productID, exists := productMap[mapping.productName]
		if !exists {
			continue // Skip if product doesn't exist
		}

		restrictionsJSON, _ := json.Marshal(mapping.restrictions)
		requiredFieldsJSON, _ := json.Marshal(mapping.requiredFields)

		_, err := tx.ExecContext(ctx, `
			INSERT INTO "dsl-ob-poc".entity_product_mappings
			(entity_type, product_id, compatible, restrictions, required_fields)
			VALUES ($1, $2, $3, $4, $5)
			ON CONFLICT (entity_type, product_id) DO UPDATE SET
				compatible = EXCLUDED.compatible,
				restrictions = EXCLUDED.restrictions,
				required_fields = EXCLUDED.required_fields`,
			mapping.entityType, productID, mapping.compatible, restrictionsJSON, requiredFieldsJSON)
		if err != nil {
			return fmt.Errorf("failed to insert entity-product mapping %s-%s: %w", mapping.entityType, mapping.productName, err)
		}
	}

	return tx.Commit()
}

// GetProductRequirements retrieves requirements for a specific product
func (s *Store) GetProductRequirements(ctx context.Context, productID string) (*ProductRequirements, error) {
	query := `
		SELECT pr.product_id, p.name as product_name, pr.entity_types, pr.required_dsl,
		       pr.attributes, pr.compliance, pr.prerequisites, pr.conditional_rules,
		       pr.created_at, pr.updated_at
		FROM "dsl-ob-poc".product_requirements pr
		JOIN "dsl-ob-poc".products p ON pr.product_id = p.product_id
		WHERE pr.product_id = $1`

	row := s.db.QueryRowContext(ctx, query, productID)

	var req ProductRequirements
	var entityTypesJSON, requiredDSLJSON, attributesJSON, complianceJSON, prerequisitesJSON, conditionalRulesJSON []byte

	err := row.Scan(
		&req.ProductID, &req.ProductName, &entityTypesJSON, &requiredDSLJSON,
		&attributesJSON, &complianceJSON, &prerequisitesJSON, &conditionalRulesJSON,
		&req.CreatedAt, &req.UpdatedAt,
	)
	if err != nil {
		if err == sql.ErrNoRows {
			return nil, fmt.Errorf("product requirements not found for product %s", productID)
		}
		return nil, fmt.Errorf("failed to get product requirements: %w", err)
	}

	// Unmarshal JSON fields
	if err := json.Unmarshal(entityTypesJSON, &req.EntityTypes); err != nil {
		return nil, fmt.Errorf("failed to unmarshal entity_types: %w", err)
	}
	if err := json.Unmarshal(requiredDSLJSON, &req.RequiredDSL); err != nil {
		return nil, fmt.Errorf("failed to unmarshal required_dsl: %w", err)
	}
	if err := json.Unmarshal(attributesJSON, &req.Attributes); err != nil {
		return nil, fmt.Errorf("failed to unmarshal attributes: %w", err)
	}
	if err := json.Unmarshal(complianceJSON, &req.Compliance); err != nil {
		return nil, fmt.Errorf("failed to unmarshal compliance: %w", err)
	}
	if err := json.Unmarshal(prerequisitesJSON, &req.Prerequisites); err != nil {
		return nil, fmt.Errorf("failed to unmarshal prerequisites: %w", err)
	}
	if err := json.Unmarshal(conditionalRulesJSON, &req.ConditionalRules); err != nil {
		return nil, fmt.Errorf("failed to unmarshal conditional_rules: %w", err)
	}

	return &req, nil
}

// GetEntityProductMapping retrieves compatibility mapping for entity type and product
func (s *Store) GetEntityProductMapping(ctx context.Context, entityType, productID string) (*EntityProductMapping, error) {
	query := `
		SELECT entity_type, product_id, compatible, restrictions, required_fields, created_at
		FROM "dsl-ob-poc".entity_product_mappings
		WHERE entity_type = $1 AND product_id = $2`

	row := s.db.QueryRowContext(ctx, query, entityType, productID)

	var mapping EntityProductMapping
	var restrictionsJSON, requiredFieldsJSON []byte

	err := row.Scan(
		&mapping.EntityType, &mapping.ProductID, &mapping.Compatible,
		&restrictionsJSON, &requiredFieldsJSON, &mapping.CreatedAt,
	)
	if err != nil {
		if err == sql.ErrNoRows {
			return nil, fmt.Errorf("entity-product mapping not found for %s-%s", entityType, productID)
		}
		return nil, fmt.Errorf("failed to get entity-product mapping: %w", err)
	}

	if err := json.Unmarshal(restrictionsJSON, &mapping.Restrictions); err != nil {
		return nil, fmt.Errorf("failed to unmarshal restrictions: %w", err)
	}
	if err := json.Unmarshal(requiredFieldsJSON, &mapping.RequiredFields); err != nil {
		return nil, fmt.Errorf("failed to unmarshal required_fields: %w", err)
	}

	return &mapping, nil
}

// ListProductRequirements returns all product requirements
func (s *Store) ListProductRequirements(ctx context.Context) ([]ProductRequirements, error) {
	query := `
		SELECT pr.product_id, p.name as product_name, pr.entity_types, pr.required_dsl,
		       pr.attributes, pr.compliance, pr.prerequisites, pr.conditional_rules,
		       pr.created_at, pr.updated_at
		FROM "dsl-ob-poc".product_requirements pr
		JOIN "dsl-ob-poc".products p ON pr.product_id = p.product_id
		ORDER BY pr.created_at DESC`

	rows, err := s.db.QueryContext(ctx, query)
	if err != nil {
		return nil, fmt.Errorf("failed to list product requirements: %w", err)
	}
	defer rows.Close()

	var requirements []ProductRequirements
	for rows.Next() {
		var req ProductRequirements
		var entityTypesJSON, requiredDSLJSON, attributesJSON, complianceJSON, prerequisitesJSON, conditionalRulesJSON []byte

		err := rows.Scan(
			&req.ProductID, &req.ProductName, &entityTypesJSON, &requiredDSLJSON,
			&attributesJSON, &complianceJSON, &prerequisitesJSON, &conditionalRulesJSON,
			&req.CreatedAt, &req.UpdatedAt,
		)
		if err != nil {
			return nil, fmt.Errorf("failed to scan product requirement: %w", err)
		}

		// Unmarshal JSON fields
		if err := json.Unmarshal(entityTypesJSON, &req.EntityTypes); err != nil {
			return nil, fmt.Errorf("failed to unmarshal entity_types: %w", err)
		}
		if err := json.Unmarshal(requiredDSLJSON, &req.RequiredDSL); err != nil {
			return nil, fmt.Errorf("failed to unmarshal required_dsl: %w", err)
		}
		if err := json.Unmarshal(attributesJSON, &req.Attributes); err != nil {
			return nil, fmt.Errorf("failed to unmarshal attributes: %w", err)
		}
		if err := json.Unmarshal(complianceJSON, &req.Compliance); err != nil {
			return nil, fmt.Errorf("failed to unmarshal compliance: %w", err)
		}
		if err := json.Unmarshal(prerequisitesJSON, &req.Prerequisites); err != nil {
			return nil, fmt.Errorf("failed to unmarshal prerequisites: %w", err)
		}
		if err := json.Unmarshal(conditionalRulesJSON, &req.ConditionalRules); err != nil {
			return nil, fmt.Errorf("failed to unmarshal conditional_rules: %w", err)
		}

		requirements = append(requirements, req)
	}

	return requirements, rows.Err()
}

// CreateProductRequirements creates new product requirements
func (s *Store) CreateProductRequirements(ctx context.Context, req *ProductRequirements) error {
	// Marshal JSON fields
	entityTypesJSON, err := json.Marshal(req.EntityTypes)
	if err != nil {
		return fmt.Errorf("failed to marshal entity_types: %w", err)
	}
	requiredDSLJSON, err := json.Marshal(req.RequiredDSL)
	if err != nil {
		return fmt.Errorf("failed to marshal required_dsl: %w", err)
	}
	attributesJSON, err := json.Marshal(req.Attributes)
	if err != nil {
		return fmt.Errorf("failed to marshal attributes: %w", err)
	}
	complianceJSON, err := json.Marshal(req.Compliance)
	if err != nil {
		return fmt.Errorf("failed to marshal compliance: %w", err)
	}
	prerequisitesJSON, err := json.Marshal(req.Prerequisites)
	if err != nil {
		return fmt.Errorf("failed to marshal prerequisites: %w", err)
	}
	conditionalRulesJSON, err := json.Marshal(req.ConditionalRules)
	if err != nil {
		return fmt.Errorf("failed to marshal conditional_rules: %w", err)
	}

	query := `
		INSERT INTO "dsl-ob-poc".product_requirements
		(product_id, entity_types, required_dsl, attributes, compliance, prerequisites, conditional_rules, created_at, updated_at)
		VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW())`

	_, err = s.db.ExecContext(ctx, query, req.ProductID, entityTypesJSON, requiredDSLJSON,
		attributesJSON, complianceJSON, prerequisitesJSON, conditionalRulesJSON)
	if err != nil {
		return fmt.Errorf("failed to create product requirements: %w", err)
	}

	return nil
}

// UpdateProductRequirements updates existing product requirements
func (s *Store) UpdateProductRequirements(ctx context.Context, req *ProductRequirements) error {
	// Marshal JSON fields
	entityTypesJSON, err := json.Marshal(req.EntityTypes)
	if err != nil {
		return fmt.Errorf("failed to marshal entity_types: %w", err)
	}
	requiredDSLJSON, err := json.Marshal(req.RequiredDSL)
	if err != nil {
		return fmt.Errorf("failed to marshal required_dsl: %w", err)
	}
	attributesJSON, err := json.Marshal(req.Attributes)
	if err != nil {
		return fmt.Errorf("failed to marshal attributes: %w", err)
	}
	complianceJSON, err := json.Marshal(req.Compliance)
	if err != nil {
		return fmt.Errorf("failed to marshal compliance: %w", err)
	}
	prerequisitesJSON, err := json.Marshal(req.Prerequisites)
	if err != nil {
		return fmt.Errorf("failed to marshal prerequisites: %w", err)
	}
	conditionalRulesJSON, err := json.Marshal(req.ConditionalRules)
	if err != nil {
		return fmt.Errorf("failed to marshal conditional_rules: %w", err)
	}

	query := `
		UPDATE "dsl-ob-poc".product_requirements
		SET entity_types = $2, required_dsl = $3, attributes = $4, compliance = $5,
		    prerequisites = $6, conditional_rules = $7, updated_at = NOW()
		WHERE product_id = $1`

	result, err := s.db.ExecContext(ctx, query, req.ProductID, entityTypesJSON, requiredDSLJSON,
		attributesJSON, complianceJSON, prerequisitesJSON, conditionalRulesJSON)
	if err != nil {
		return fmt.Errorf("failed to update product requirements: %w", err)
	}

	rowsAffected, err := result.RowsAffected()
	if err != nil {
		return fmt.Errorf("failed to get rows affected: %w", err)
	}
	if rowsAffected == 0 {
		return fmt.Errorf("product requirements not found for product %s", req.ProductID)
	}

	return nil
}

// CreateEntityProductMapping creates new entity-product mapping
func (s *Store) CreateEntityProductMapping(ctx context.Context, mapping *EntityProductMapping) error {
	// Marshal JSON fields
	restrictionsJSON, err := json.Marshal(mapping.Restrictions)
	if err != nil {
		return fmt.Errorf("failed to marshal restrictions: %w", err)
	}
	requiredFieldsJSON, err := json.Marshal(mapping.RequiredFields)
	if err != nil {
		return fmt.Errorf("failed to marshal required_fields: %w", err)
	}

	query := `
		INSERT INTO "dsl-ob-poc".entity_product_mappings
		(entity_type, product_id, compatible, restrictions, required_fields, created_at)
		VALUES ($1, $2, $3, $4, $5, NOW())`

	_, err = s.db.ExecContext(ctx, query, mapping.EntityType, mapping.ProductID,
		mapping.Compatible, restrictionsJSON, requiredFieldsJSON)
	if err != nil {
		return fmt.Errorf("failed to create entity-product mapping: %w", err)
	}

	return nil
}
