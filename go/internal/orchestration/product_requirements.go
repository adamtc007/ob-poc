package orchestration

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"time"

	"github.com/google/uuid"

	"dsl-ob-poc/internal/datastore"
	"dsl-ob-poc/internal/store"
)

// ProductRequirements defines what DSL operations and attributes are required for a specific product
type ProductRequirements struct {
	ProductID        string                   `json:"product_id" db:"product_id"`
	ProductName      string                   `json:"product_name" db:"product_name"`
	EntityTypes      []string                 `json:"entity_types" db:"entity_types"`
	RequiredDSL      []string                 `json:"required_dsl" db:"required_dsl"`
	Attributes       []string                 `json:"attributes" db:"attributes"`
	Compliance       []ProductComplianceRule  `json:"compliance" db:"compliance"`
	Prerequisites    []string                 `json:"prerequisites" db:"prerequisites"`
	ConditionalRules []ProductConditionalRule `json:"conditional_rules" db:"conditional_rules"`
	CreatedAt        time.Time                `json:"created_at" db:"created_at"`
	UpdatedAt        time.Time                `json:"updated_at" db:"updated_at"`
}

// ProductComplianceRule represents a compliance requirement for a product
type ProductComplianceRule struct {
	RuleID      string `json:"rule_id"`
	Framework   string `json:"framework"` // FINCEN, SEC, EU_5MLD, etc.
	Description string `json:"description"`
	Required    bool   `json:"required"`
}

// ProductConditionalRule defines conditional DSL requirements based on entity characteristics
type ProductConditionalRule struct {
	Condition   string   `json:"condition"`    // e.g., "entity_type == 'TRUST'"
	RequiredDSL []string `json:"required_dsl"` // Additional DSL verbs if condition is met
	Attributes  []string `json:"attributes"`   // Additional attributes if condition is met
}

// EntityProductMapping represents which products an entity type can have
type EntityProductMapping struct {
	EntityType     string    `json:"entity_type" db:"entity_type"`
	ProductID      string    `json:"product_id" db:"product_id"`
	Compatible     bool      `json:"compatible" db:"compatible"`
	Restrictions   []string  `json:"restrictions" db:"restrictions"`
	RequiredFields []string  `json:"required_fields" db:"required_fields"`
	CreatedAt      time.Time `json:"created_at" db:"created_at"`
}

// ProductWorkflow represents the complete workflow generated for a specific product-entity combination
type ProductWorkflow struct {
	WorkflowID      uuid.UUID               `json:"workflow_id" db:"workflow_id"`
	CBUID           string                  `json:"cbu_id" db:"cbu_id"`
	ProductID       string                  `json:"product_id" db:"product_id"`
	EntityType      string                  `json:"entity_type" db:"entity_type"`
	RequiredDSL     []string                `json:"required_dsl" db:"required_dsl"`
	GeneratedDSL    string                  `json:"generated_dsl" db:"generated_dsl"`
	ComplianceRules []ProductComplianceRule `json:"compliance_rules" db:"compliance_rules"`
	Status          WorkflowStatus          `json:"status" db:"status"`
	CreatedAt       time.Time               `json:"created_at" db:"created_at"`
	UpdatedAt       time.Time               `json:"updated_at" db:"updated_at"`
}

// WorkflowStatus represents the status of a product workflow
type WorkflowStatus string

const (
	WorkflowStatusPending    WorkflowStatus = "PENDING"
	WorkflowStatusGenerating WorkflowStatus = "GENERATING"
	WorkflowStatusReady      WorkflowStatus = "READY"
	WorkflowStatusExecuting  WorkflowStatus = "EXECUTING"
	WorkflowStatusCompleted  WorkflowStatus = "COMPLETED"
	WorkflowStatusFailed     WorkflowStatus = "FAILED"
)

// ProductRequirementsService handles product-driven workflow customization
type ProductRequirementsService interface {
	GetProductRequirements(ctx context.Context, productID string) (*ProductRequirements, error)
	GetEntityProductMapping(ctx context.Context, entityType, productID string) (*EntityProductMapping, error)
	ValidateProductEntityCompatibility(ctx context.Context, entityType string, productIDs []string) ([]ProductValidationResult, error)
	GenerateProductWorkflow(ctx context.Context, cbuID, productID, entityType string) (*ProductWorkflow, error)
	GetProductWorkflow(ctx context.Context, workflowID uuid.UUID) (*ProductWorkflow, error)
	ListProductRequirements(ctx context.Context) ([]ProductRequirements, error)
	CreateProductRequirements(ctx context.Context, req *ProductRequirements) error
	UpdateProductRequirements(ctx context.Context, req *ProductRequirements) error
}

// ProductValidationResult represents the result of product-entity compatibility validation
type ProductValidationResult struct {
	ProductID    string   `json:"product_id"`
	EntityType   string   `json:"entity_type"`
	Compatible   bool     `json:"compatible"`
	Issues       []string `json:"issues"`
	Warnings     []string `json:"warnings"`
	Requirements []string `json:"requirements"`
}

// ProductRequirementsRepository handles database operations for product requirements
type ProductRequirementsRepository struct {
	db *sql.DB
}

// NewProductRequirementsRepository creates a new repository instance
func NewProductRequirementsRepository(db *sql.DB) *ProductRequirementsRepository {
	return &ProductRequirementsRepository{db: db}
}

// GetProductRequirements retrieves requirements for a specific product
func (r *ProductRequirementsRepository) GetProductRequirements(ctx context.Context, productID string) (*ProductRequirements, error) {
	query := `
		SELECT pr.product_id, p.name as product_name, pr.entity_types, pr.required_dsl,
		       pr.attributes, pr.compliance, pr.prerequisites, pr.conditional_rules,
		       pr.created_at, pr.updated_at
		FROM "dsl-ob-poc".product_requirements pr
		JOIN "dsl-ob-poc".products p ON pr.product_id = p.product_id
		WHERE pr.product_id = $1`

	row := r.db.QueryRowContext(ctx, query, productID)

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
func (r *ProductRequirementsRepository) GetEntityProductMapping(ctx context.Context, entityType, productID string) (*EntityProductMapping, error) {
	query := `
		SELECT entity_type, product_id, compatible, restrictions, required_fields, created_at
		FROM "dsl-ob-poc".entity_product_mappings
		WHERE entity_type = $1 AND product_id = $2`

	row := r.db.QueryRowContext(ctx, query, entityType, productID)

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
func (r *ProductRequirementsRepository) ListProductRequirements(ctx context.Context) ([]ProductRequirements, error) {
	query := `
		SELECT pr.product_id, p.name as product_name, pr.entity_types, pr.required_dsl,
		       pr.attributes, pr.compliance, pr.prerequisites, pr.conditional_rules,
		       pr.created_at, pr.updated_at
		FROM "dsl-ob-poc".product_requirements pr
		JOIN "dsl-ob-poc".products p ON pr.product_id = p.product_id
		ORDER BY pr.created_at DESC`

	rows, err := r.db.QueryContext(ctx, query)
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
func (r *ProductRequirementsRepository) CreateProductRequirements(ctx context.Context, req *ProductRequirements) error {
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

	_, err = r.db.ExecContext(ctx, query, req.ProductID, entityTypesJSON, requiredDSLJSON,
		attributesJSON, complianceJSON, prerequisitesJSON, conditionalRulesJSON)
	if err != nil {
		return fmt.Errorf("failed to create product requirements: %w", err)
	}

	return nil
}

// UpdateProductRequirements updates existing product requirements
func (r *ProductRequirementsRepository) UpdateProductRequirements(ctx context.Context, req *ProductRequirements) error {
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

	result, err := r.db.ExecContext(ctx, query, req.ProductID, entityTypesJSON, requiredDSLJSON,
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

// ProductRequirementsServiceImpl implements the ProductRequirementsService interface
type ProductRequirementsServiceImpl struct {
	dataStore datastore.DataStore
}

// NewProductRequirementsService creates a new service instance
func NewProductRequirementsService(ds datastore.DataStore) ProductRequirementsService {
	return &ProductRequirementsServiceImpl{
		dataStore: ds,
	}
}

// GetProductRequirements retrieves requirements for a specific product
func (s *ProductRequirementsServiceImpl) GetProductRequirements(ctx context.Context, productID string) (*ProductRequirements, error) {
	storeReq, err := s.dataStore.GetProductRequirements(ctx, productID)
	if err != nil {
		return nil, err
	}

	// Convert store type to orchestration type
	return &ProductRequirements{
		ProductID:        storeReq.ProductID,
		ProductName:      storeReq.ProductName,
		EntityTypes:      storeReq.EntityTypes,
		RequiredDSL:      storeReq.RequiredDSL,
		Attributes:       storeReq.Attributes,
		Compliance:       convertStoreComplianceRules(storeReq.Compliance),
		Prerequisites:    storeReq.Prerequisites,
		ConditionalRules: convertStoreConditionalRules(storeReq.ConditionalRules),
		CreatedAt:        storeReq.CreatedAt,
		UpdatedAt:        storeReq.UpdatedAt,
	}, nil
}

// GetEntityProductMapping retrieves compatibility mapping
func (s *ProductRequirementsServiceImpl) GetEntityProductMapping(ctx context.Context, entityType, productID string) (*EntityProductMapping, error) {
	storeMapping, err := s.dataStore.GetEntityProductMapping(ctx, entityType, productID)
	if err != nil {
		return nil, err
	}

	// Convert store type to orchestration type
	return &EntityProductMapping{
		EntityType:     storeMapping.EntityType,
		ProductID:      storeMapping.ProductID,
		Compatible:     storeMapping.Compatible,
		Restrictions:   storeMapping.Restrictions,
		RequiredFields: storeMapping.RequiredFields,
		CreatedAt:      storeMapping.CreatedAt,
	}, nil
}

// ValidateProductEntityCompatibility validates if entity types are compatible with products
func (s *ProductRequirementsServiceImpl) ValidateProductEntityCompatibility(ctx context.Context, entityType string, productIDs []string) ([]ProductValidationResult, error) {
	var results []ProductValidationResult

	for _, productID := range productIDs {
		result := ProductValidationResult{
			ProductID:  productID,
			EntityType: entityType,
		}

		// Get product requirements
		req, err := s.GetProductRequirements(ctx, productID)
		if err != nil {
			result.Compatible = false
			result.Issues = append(result.Issues, fmt.Sprintf("Cannot get product requirements: %v", err))
			results = append(results, result)
			continue
		}

		// Check if entity type is supported
		supported := false
		for _, supportedType := range req.EntityTypes {
			if supportedType == entityType || supportedType == "ALL" {
				supported = true
				break
			}
		}

		if !supported {
			result.Compatible = false
			result.Issues = append(result.Issues, fmt.Sprintf("Entity type %s not supported for product %s", entityType, productID))
		} else {
			result.Compatible = true
			result.Requirements = req.RequiredDSL
		}

		// Check entity-product mapping if exists
		mapping, err := s.GetEntityProductMapping(ctx, entityType, productID)
		if err == nil {
			if !mapping.Compatible {
				result.Compatible = false
				result.Issues = append(result.Issues, "Entity-product mapping indicates incompatibility")
			}
			if len(mapping.Restrictions) > 0 {
				result.Warnings = append(result.Warnings, "Product has restrictions for this entity type")
			}
		}

		results = append(results, result)
	}

	return results, nil
}

// GenerateProductWorkflow generates a complete workflow for product-entity combination
func (s *ProductRequirementsServiceImpl) GenerateProductWorkflow(ctx context.Context, cbuID, productID, entityType string) (*ProductWorkflow, error) {
	// Get product requirements
	req, err := s.GetProductRequirements(ctx, productID)
	if err != nil {
		return nil, fmt.Errorf("failed to get product requirements: %w", err)
	}

	// Validate compatibility
	validationResults, err := s.ValidateProductEntityCompatibility(ctx, entityType, []string{productID})
	if err != nil {
		return nil, fmt.Errorf("failed to validate compatibility: %w", err)
	}

	if len(validationResults) == 0 || !validationResults[0].Compatible {
		return nil, fmt.Errorf("product %s is not compatible with entity type %s", productID, entityType)
	}

	// Build DSL fragments
	var dslFragments []string
	dslFragments = append(dslFragments, req.RequiredDSL...)

	// Apply conditional rules
	for _, rule := range req.ConditionalRules {
		if s.evaluateCondition(rule.Condition, entityType) {
			dslFragments = append(dslFragments, rule.RequiredDSL...)
		}
	}

	// Generate complete DSL
	generatedDSL := s.buildWorkflowDSL(cbuID, productID, entityType, dslFragments, req.Attributes)

	// Create workflow record
	workflow := &ProductWorkflow{
		WorkflowID:      uuid.New(),
		CBUID:           cbuID,
		ProductID:       productID,
		EntityType:      entityType,
		RequiredDSL:     dslFragments,
		GeneratedDSL:    generatedDSL,
		ComplianceRules: req.Compliance,
		Status:          WorkflowStatusReady,
		CreatedAt:       time.Now(),
		UpdatedAt:       time.Now(),
	}

	return workflow, nil
}

// GetProductWorkflow retrieves a workflow by ID
func (s *ProductRequirementsServiceImpl) GetProductWorkflow(ctx context.Context, workflowID uuid.UUID) (*ProductWorkflow, error) {
	// This would be implemented with a proper workflow repository
	// For now, return a placeholder
	return nil, fmt.Errorf("workflow retrieval not yet implemented")
}

// ListProductRequirements returns all product requirements
func (s *ProductRequirementsServiceImpl) ListProductRequirements(ctx context.Context) ([]ProductRequirements, error) {
	storeReqs, err := s.dataStore.ListProductRequirements(ctx)
	if err != nil {
		return nil, err
	}

	// Convert store types to orchestration types
	var requirements []ProductRequirements
	for _, storeReq := range storeReqs {
		req := ProductRequirements{
			ProductID:        storeReq.ProductID,
			ProductName:      storeReq.ProductName,
			EntityTypes:      storeReq.EntityTypes,
			RequiredDSL:      storeReq.RequiredDSL,
			Attributes:       storeReq.Attributes,
			Compliance:       convertStoreComplianceRules(storeReq.Compliance),
			Prerequisites:    storeReq.Prerequisites,
			ConditionalRules: convertStoreConditionalRules(storeReq.ConditionalRules),
			CreatedAt:        storeReq.CreatedAt,
			UpdatedAt:        storeReq.UpdatedAt,
		}
		requirements = append(requirements, req)
	}

	return requirements, nil
}

// CreateProductRequirements creates new product requirements
func (s *ProductRequirementsServiceImpl) CreateProductRequirements(ctx context.Context, req *ProductRequirements) error {
	// Convert orchestration type to store type
	storeReq := &store.ProductRequirements{
		ProductID:        req.ProductID,
		ProductName:      req.ProductName,
		EntityTypes:      req.EntityTypes,
		RequiredDSL:      req.RequiredDSL,
		Attributes:       req.Attributes,
		Compliance:       convertOrchestrationComplianceRules(req.Compliance),
		Prerequisites:    req.Prerequisites,
		ConditionalRules: convertOrchestrationConditionalRules(req.ConditionalRules),
		CreatedAt:        req.CreatedAt,
		UpdatedAt:        req.UpdatedAt,
	}

	return s.dataStore.CreateProductRequirements(ctx, storeReq)
}

// UpdateProductRequirements updates existing product requirements
func (s *ProductRequirementsServiceImpl) UpdateProductRequirements(ctx context.Context, req *ProductRequirements) error {
	// Convert orchestration type to store type
	storeReq := &store.ProductRequirements{
		ProductID:        req.ProductID,
		ProductName:      req.ProductName,
		EntityTypes:      req.EntityTypes,
		RequiredDSL:      req.RequiredDSL,
		Attributes:       req.Attributes,
		Compliance:       convertOrchestrationComplianceRules(req.Compliance),
		Prerequisites:    req.Prerequisites,
		ConditionalRules: convertOrchestrationConditionalRules(req.ConditionalRules),
		CreatedAt:        req.CreatedAt,
		UpdatedAt:        req.UpdatedAt,
	}

	return s.dataStore.UpdateProductRequirements(ctx, storeReq)
}

// evaluateCondition evaluates a conditional rule condition
func (s *ProductRequirementsServiceImpl) evaluateCondition(condition, entityType string) bool {
	// Simple condition evaluation - can be enhanced with a proper expression evaluator
	switch condition {
	case "entity_type == 'TRUST'":
		return entityType == "TRUST"
	case "entity_type == 'PARTNERSHIP'":
		return entityType == "PARTNERSHIP"
	case "entity_type == 'CORPORATION'":
		return entityType == "CORPORATION"
	case "entity_type == 'PROPER_PERSON'":
		return entityType == "PROPER_PERSON"
	default:
		return false
	}
}

// buildWorkflowDSL constructs the complete DSL document for the workflow
func (s *ProductRequirementsServiceImpl) buildWorkflowDSL(cbuID, productID, entityType string, dslFragments, attributes []string) string {
	var dslBuilder []string

	// Add case creation
	dslBuilder = append(dslBuilder, fmt.Sprintf(`(case.create (cbu.id "%s") (product "%s") (entity-type "%s"))`, cbuID, productID, entityType))

	// Add product-specific DSL fragments
	for _, fragment := range dslFragments {
		dslBuilder = append(dslBuilder, fmt.Sprintf("(%s)", fragment))
	}

	// Add attribute declarations
	if len(attributes) > 0 {
		attributeDecls := "(attributes.declare"
		for _, attr := range attributes {
			attributeDecls += fmt.Sprintf(` (attr "%s")`, attr)
		}
		attributeDecls += ")"
		dslBuilder = append(dslBuilder, attributeDecls)
	}

	return fmt.Sprintf("(%s)", fmt.Sprintf("%s", dslBuilder))
}

// Note: Product requirements are now stored in the database via the product_requirements table.
// Use the seed-product-requirements CLI command to populate initial data.
// This removes hardcoded data mocks that could cause inconsistencies with database state.

// Helper functions to convert between store and orchestration types
func convertStoreComplianceRules(storeRules []store.ProductComplianceRule) []ProductComplianceRule {
	var rules []ProductComplianceRule
	for _, storeRule := range storeRules {
		rule := ProductComplianceRule{
			RuleID:      storeRule.RuleID,
			Framework:   storeRule.Framework,
			Description: storeRule.Description,
			Required:    storeRule.Required,
		}
		rules = append(rules, rule)
	}
	return rules
}

func convertStoreConditionalRules(storeRules []store.ProductConditionalRule) []ProductConditionalRule {
	var rules []ProductConditionalRule
	for _, storeRule := range storeRules {
		rule := ProductConditionalRule{
			Condition:   storeRule.Condition,
			RequiredDSL: storeRule.RequiredDSL,
			Attributes:  storeRule.Attributes,
		}
		rules = append(rules, rule)
	}
	return rules
}

func convertOrchestrationComplianceRules(orchRules []ProductComplianceRule) []store.ProductComplianceRule {
	var rules []store.ProductComplianceRule
	for _, orchRule := range orchRules {
		rule := store.ProductComplianceRule{
			RuleID:      orchRule.RuleID,
			Framework:   orchRule.Framework,
			Description: orchRule.Description,
			Required:    orchRule.Required,
		}
		rules = append(rules, rule)
	}
	return rules
}

func convertOrchestrationConditionalRules(orchRules []ProductConditionalRule) []store.ProductConditionalRule {
	var rules []store.ProductConditionalRule
	for _, orchRule := range orchRules {
		rule := store.ProductConditionalRule{
			Condition:   orchRule.Condition,
			RequiredDSL: orchRule.RequiredDSL,
			Attributes:  orchRule.Attributes,
		}
		rules = append(rules, rule)
	}
	return rules
}
