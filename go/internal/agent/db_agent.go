package agent

import (
	"context"
	"database/sql"
	"fmt"
	"regexp"
	"strings"

	"dsl-ob-poc/internal/dsl"

	"github.com/lib/pq"
)

// DBAgent provides database-driven AI responses replacing hardcoded mock responses
type DBAgent struct {
	db *sql.DB
}

// NewDBAgent creates a database-driven agent
func NewDBAgent(db *sql.DB) *DBAgent {
	return &DBAgent{db: db}
}

// CallKYCAgent performs KYC discovery using database-driven rules
func (a *DBAgent) CallKYCAgent(ctx context.Context, naturePurpose string, products []string) (*dsl.KYCRequirements, error) {
	var docs []string
	var jurisdictions []string

	// Analyze nature/purpose for entity type and domicile
	entityType, jurisdiction := a.parseNaturePurpose(naturePurpose)

	// Query KYC rules from database
	kycDocs, kycJurisdictions, err := a.getKYCRules(ctx, entityType, jurisdiction)
	if err != nil {
		return nil, fmt.Errorf("failed to get KYC rules: %w", err)
	}

	docs = append(docs, kycDocs...)
	jurisdictions = append(jurisdictions, kycJurisdictions...)

	// Query product requirements from database
	for _, product := range products {
		productDocs, err := a.getProductRequirements(ctx, product)
		if err != nil {
			return nil, fmt.Errorf("failed to get requirements for product %s: %w", product, err)
		}
		docs = append(docs, productDocs...)
	}

	// Remove duplicates
	docs = uniqueStrings(docs)
	jurisdictions = uniqueStrings(jurisdictions)

	return &dsl.KYCRequirements{
		Documents:     docs,
		Jurisdictions: jurisdictions,
	}, nil
}

// CallDSLTransformationAgent performs DSL transformation using database-driven rules
func (a *DBAgent) CallDSLTransformationAgent(ctx context.Context, request DSLTransformationRequest) (*DSLTransformationResponse, error) {
	instruction := strings.ToLower(request.Instruction)

	// Query transformation rules from database
	rule, err := a.getTransformationRule(ctx, instruction)
	if err != nil {
		return nil, fmt.Errorf("failed to get transformation rule: %w", err)
	}

	if rule == nil {
		// No specific rule found, return generic transformation
		return a.genericTransformation(request)
	}

	// Apply the database-driven transformation
	newDSL := request.CurrentDSL
	if !strings.Contains(newDSL, strings.TrimSpace(rule.DSLTemplate)) {
		newDSL += "\n\n" + rule.DSLTemplate
	}

	explanation := fmt.Sprintf("Applied %s transformation based on database rule", rule.TransformationType)
	changes := []string{fmt.Sprintf("Added %s", rule.TransformationType)}

	return &DSLTransformationResponse{
		NewDSL:      newDSL,
		Explanation: explanation,
		Changes:     changes,
		Confidence:  float64(rule.ConfidenceScore),
	}, nil
}

// CallDSLValidationAgent performs DSL validation using database-driven rules
func (a *DBAgent) CallDSLValidationAgent(ctx context.Context, dslToValidate string) (*DSLValidationResponse, error) {
	var errors []string
	var warnings []string
	var suggestions []string

	// Query validation rules from database
	rules, err := a.getValidationRules(ctx)
	if err != nil {
		return nil, fmt.Errorf("failed to get validation rules: %w", err)
	}

	// Apply each validation rule
	for _, rule := range rules {
		matched, err := a.validateRule(dslToValidate, rule)
		if err != nil {
			continue // Skip rules that can't be evaluated
		}

		if !matched {
			switch rule.Severity {
			case "error":
				if rule.ErrorMessage != "" {
					errors = append(errors, rule.ErrorMessage)
				}
			case "warning":
				if rule.WarningMessage != "" {
					warnings = append(warnings, rule.WarningMessage)
				}
			}
		}

		// Add suggestions regardless of rule match
		if rule.Suggestion != "" && rule.Severity == "info" {
			suggestions = append(suggestions, rule.Suggestion)
		}
	}

	// Determine overall validation result
	isValid := len(errors) == 0
	score := 1.0 - (float64(len(errors))*0.3 + float64(len(warnings))*0.1)
	if score < 0 {
		score = 0
	}

	return &DSLValidationResponse{
		IsValid:     isValid,
		Score:       score,
		Errors:      errors,
		Warnings:    warnings,
		Suggestions: suggestions,
	}, nil
}

// parseNaturePurpose extracts entity type and jurisdiction from nature/purpose string
func (a *DBAgent) parseNaturePurpose(naturePurpose string) (string, string) {
	natureLower := strings.ToLower(naturePurpose)

	var entityType string
	var jurisdiction string

	// Entity type detection
	switch {
	case strings.Contains(natureLower, "ucits"):
		entityType = "ucits"
	case strings.Contains(natureLower, "hedge fund"):
		entityType = "hedge_fund"
	case strings.Contains(natureLower, "corporation"):
		entityType = "corporation"
	case strings.Contains(natureLower, "company"):
		entityType = "company"
	default:
		entityType = "default"
	}

	// Jurisdiction detection
	switch {
	case strings.Contains(natureLower, " lu") || strings.Contains(natureLower, "luxembourg"):
		jurisdiction = "LU"
	case strings.Contains(natureLower, " us") || strings.Contains(natureLower, "united states"):
		jurisdiction = "US"
	case strings.Contains(natureLower, " uk") || strings.Contains(natureLower, "united kingdom"):
		jurisdiction = "UK"
	case strings.Contains(natureLower, "cayman"):
		jurisdiction = "CAYMAN"
	case strings.Contains(natureLower, " eu") || strings.Contains(natureLower, "european"):
		jurisdiction = "EU"
	}

	return entityType, jurisdiction
}

// getKYCRules queries KYC rules from the database
func (a *DBAgent) getKYCRules(ctx context.Context, entityType, jurisdiction string) ([]string, []string, error) {
	query := `
		SELECT required_documents, COALESCE(jurisdiction, '') as jurisdiction
		FROM "ob-poc".kyc_rules
		WHERE entity_type = $1 AND (jurisdiction = $2 OR jurisdiction IS NULL)
		ORDER BY CASE WHEN jurisdiction IS NULL THEN 1 ELSE 0 END
		LIMIT 1`

	var documents pq.StringArray
	var dbJurisdiction string

	err := a.db.QueryRowContext(ctx, query, entityType, jurisdiction).Scan(&documents, &dbJurisdiction)
	if err != nil {
		if err == sql.ErrNoRows {
			// Try default rules
			err = a.db.QueryRowContext(ctx,
				`SELECT required_documents, COALESCE(jurisdiction, '') FROM "ob-poc".kyc_rules WHERE entity_type = 'default' LIMIT 1`,
			).Scan(&documents, &dbJurisdiction)
			if err != nil {
				return nil, nil, fmt.Errorf("no KYC rules found: %w", err)
			}
		} else {
			return nil, nil, err
		}
	}

	jurisdictions := []string{}
	if jurisdiction != "" {
		jurisdictions = append(jurisdictions, jurisdiction)
	} else if dbJurisdiction != "" {
		jurisdictions = append(jurisdictions, dbJurisdiction)
	}

	return []string(documents), jurisdictions, nil
}

// getProductRequirements queries product requirements from the database
func (a *DBAgent) getProductRequirements(ctx context.Context, productName string) ([]string, error) {
	query := `SELECT required_documents FROM "ob-poc".product_requirements WHERE product_name = $1`

	var documents pq.StringArray
	err := a.db.QueryRowContext(ctx, query, strings.ToUpper(productName)).Scan(&documents)
	if err != nil {
		if err == sql.ErrNoRows {
			return []string{}, nil // No requirements for this product
		}
		return nil, err
	}

	return []string(documents), nil
}

// DSLTransformationRule represents a transformation rule from the database
type DSLTransformationRule struct {
	RuleID             string
	InstructionPattern string
	TransformationType string
	TargetValues       string
	DSLTemplate        string
	ConfidenceScore    float32
}

// getTransformationRule queries transformation rules from the database
func (a *DBAgent) getTransformationRule(ctx context.Context, instruction string) (*DSLTransformationRule, error) {
	query := `
		SELECT rule_id, instruction_pattern, transformation_type,
		       COALESCE(target_values::text, '{}'), dsl_template, confidence_score
		FROM "ob-poc".dsl_transformation_rules
		ORDER BY confidence_score DESC`

	rows, err := a.db.QueryContext(ctx, query)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	for rows.Next() {
		var rule DSLTransformationRule
		err := rows.Scan(&rule.RuleID, &rule.InstructionPattern, &rule.TransformationType,
			&rule.TargetValues, &rule.DSLTemplate, &rule.ConfidenceScore)
		if err != nil {
			continue
		}

		// Check if instruction matches pattern
		matched, err := regexp.MatchString(rule.InstructionPattern, instruction)
		if err != nil {
			continue
		}

		if matched {
			return &rule, nil
		}
	}

	return nil, nil // No matching rule found
}

// DSLValidationRule represents a validation rule from the database
type DSLValidationRule struct {
	RuleID         string
	RuleType       string
	TargetPattern  string
	ErrorMessage   string
	WarningMessage string
	Suggestion     string
	Severity       string
}

// getValidationRules queries validation rules from the database
func (a *DBAgent) getValidationRules(ctx context.Context) ([]DSLValidationRule, error) {
	query := `
		SELECT rule_id, rule_type, target_pattern,
		       COALESCE(error_message, ''), COALESCE(warning_message, ''),
		       COALESCE(suggestion, ''), severity
		FROM "ob-poc".dsl_validation_rules
		ORDER BY severity DESC, rule_type`

	rows, err := a.db.QueryContext(ctx, query)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var rules []DSLValidationRule
	for rows.Next() {
		var rule DSLValidationRule
		err := rows.Scan(&rule.RuleID, &rule.RuleType, &rule.TargetPattern,
			&rule.ErrorMessage, &rule.WarningMessage, &rule.Suggestion, &rule.Severity)
		if err != nil {
			continue
		}
		rules = append(rules, rule)
	}

	return rules, nil
}

// validateRule applies a single validation rule to the DSL text
func (a *DBAgent) validateRule(dslText string, rule DSLValidationRule) (bool, error) {
	switch rule.RuleType {
	case "required":
		return strings.Contains(dslText, rule.TargetPattern), nil
	case "format":
		matched, err := regexp.MatchString(rule.TargetPattern, dslText)
		return matched, err
	case "relationship":
		// For relationship rules, check if both parts of the pattern exist
		parts := strings.Split(rule.TargetPattern, ".*")
		if len(parts) == 2 {
			return strings.Contains(dslText, parts[0]) && strings.Contains(dslText, parts[1]), nil
		}
		return true, nil
	case "suggestion":
		// Suggestions always pass but provide recommendations
		return true, nil
	default:
		return true, nil
	}
}

// genericTransformation provides a fallback transformation when no specific rule is found
func (a *DBAgent) genericTransformation(request DSLTransformationRequest) (*DSLTransformationResponse, error) {
	return &DSLTransformationResponse{
		NewDSL:      request.CurrentDSL + "\n\n; Transformation applied: " + request.Instruction,
		Explanation: "Applied generic transformation based on the instruction",
		Changes:     []string{"Added transformation comment"},
		Confidence:  0.6,
	}, nil
}

// uniqueStrings removes duplicate strings from a slice
func uniqueStrings(slice []string) []string {
	keys := make(map[string]bool)
	var unique []string

	for _, item := range slice {
		if !keys[item] {
			keys[item] = true
			unique = append(unique, item)
		}
	}

	return unique
}
