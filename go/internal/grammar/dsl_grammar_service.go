package grammar

import (
	"context"
	"fmt"
	"time"

	"dsl-ob-poc/internal/vocabulary"
)

// DSLGrammarService provides DSL grammar management and validation
type DSLGrammarService struct {
	repo vocabulary.GrammarRepository
}

// NewDSLGrammarService creates a new DSL grammar service
func NewDSLGrammarService(repo vocabulary.GrammarRepository) *DSLGrammarService {
	return &DSLGrammarService{
		repo: repo,
	}
}

// InitializeDefaultGrammar sets up the default DSL grammar rules
func (s *DSLGrammarService) InitializeDefaultGrammar(ctx context.Context) error {
	rules := s.getDefaultGrammarRules()

	for _, rule := range rules {
		// Check if rule already exists
		existing, err := s.repo.GetGrammarRuleByName(ctx, rule.RuleName)
		if err == nil && existing != nil {
			// Rule exists, skip
			continue
		}

		// Create new rule
		if err := s.repo.CreateGrammarRule(ctx, rule); err != nil {
			return fmt.Errorf("failed to create grammar rule %s: %w", rule.RuleName, err)
		}
	}

	return nil
}

// GetParserForDomain returns an EBNF parser configured for the specified domain
func (s *DSLGrammarService) GetParserForDomain(domain string) *EBNFParser {
	return NewEBNFParser(s.repo, domain)
}

// ValidateDSLForDomain validates DSL text against domain-specific grammar
func (s *DSLGrammarService) ValidateDSLForDomain(ctx context.Context, dsl, domain string) error {
	parser := s.GetParserForDomain(domain)
	return parser.ValidateDSL(ctx, dsl)
}

// getDefaultGrammarRules returns the default set of DSL grammar rules
func (s *DSLGrammarService) getDefaultGrammarRules() []*vocabulary.GrammarRule {
	now := time.Now()

	return []*vocabulary.GrammarRule{
		{
			RuleName:       "dsl_document",
			RuleDefinition: "s_expression+",
			RuleType:       "production",
			Description:    stringPtr("Top-level DSL document containing one or more S-expressions"),
			Active:         true,
			Version:        "1.0.0",
			CreatedAt:      now,
			UpdatedAt:      now,
		},
		{
			RuleName:       "s_expression",
			RuleDefinition: `"(" verb_call ")" | "(" compound_expression ")"`,
			RuleType:       "production",
			Description:    stringPtr("S-expression: parenthesized verb call or compound expression"),
			Active:         true,
			Version:        "1.0.0",
			CreatedAt:      now,
			UpdatedAt:      now,
		},
		{
			RuleName:       "verb_call",
			RuleDefinition: "verb_name argument*",
			RuleType:       "production",
			Description:    stringPtr("Verb call with optional arguments"),
			Active:         true,
			Version:        "1.0.0",
			CreatedAt:      now,
			UpdatedAt:      now,
		},
		{
			RuleName:       "compound_expression",
			RuleDefinition: "s_expression+",
			RuleType:       "production",
			Description:    stringPtr("Compound expression containing nested S-expressions"),
			Active:         true,
			Version:        "1.0.0",
			CreatedAt:      now,
			UpdatedAt:      now,
		},
		{
			RuleName:       "verb_name",
			RuleDefinition: "identifier",
			RuleType:       "production",
			Description:    stringPtr("DSL verb name"),
			Active:         true,
			Version:        "1.0.0",
			CreatedAt:      now,
			UpdatedAt:      now,
		},
		{
			RuleName:       "argument",
			RuleDefinition: "string_literal | uuid_literal | identifier | number | s_expression",
			RuleType:       "production",
			Description:    stringPtr("Argument to a verb call"),
			Active:         true,
			Version:        "1.0.0",
			CreatedAt:      now,
			UpdatedAt:      now,
		},
		{
			RuleName:       "identifier",
			RuleDefinition: "[a-zA-Z][a-zA-Z0-9_.-]*",
			RuleType:       "terminal",
			Description:    stringPtr("Identifier: alphanumeric with dots, dashes, underscores"),
			Active:         true,
			Version:        "1.0.0",
			CreatedAt:      now,
			UpdatedAt:      now,
		},
		{
			RuleName:       "string_literal",
			RuleDefinition: `"\"[^\"]*\""`,
			RuleType:       "terminal",
			Description:    stringPtr("Quoted string literal"),
			Active:         true,
			Version:        "1.0.0",
			CreatedAt:      now,
			UpdatedAt:      now,
		},
		{
			RuleName:       "uuid_literal",
			RuleDefinition: "[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}",
			RuleType:       "terminal",
			Description:    stringPtr("UUID literal"),
			Active:         true,
			Version:        "1.0.0",
			CreatedAt:      now,
			UpdatedAt:      now,
		},
		{
			RuleName:       "number",
			RuleDefinition: "[0-9]+(\\.[0-9]+)?",
			RuleType:       "terminal",
			Description:    stringPtr("Numeric literal (integer or decimal)"),
			Active:         true,
			Version:        "1.0.0",
			CreatedAt:      now,
			UpdatedAt:      now,
		},
		// Domain-specific rules for onboarding
		{
			RuleName:       "case_create",
			RuleDefinition: `"case.create" cbu_id_arg nature_purpose_arg`,
			RuleType:       "production",
			Domain:         stringPtr("onboarding"),
			Description:    stringPtr("Case creation with CBU ID and nature purpose"),
			Active:         true,
			Version:        "1.0.0",
			CreatedAt:      now,
			UpdatedAt:      now,
		},
		{
			RuleName:       "cbu_id_arg",
			RuleDefinition: `"(" "cbu.id" string_literal ")"`,
			RuleType:       "production",
			Domain:         stringPtr("onboarding"),
			Description:    stringPtr("CBU ID argument"),
			Active:         true,
			Version:        "1.0.0",
			CreatedAt:      now,
			UpdatedAt:      now,
		},
		{
			RuleName:       "nature_purpose_arg",
			RuleDefinition: `"(" "nature-purpose" string_literal ")"`,
			RuleType:       "production",
			Domain:         stringPtr("onboarding"),
			Description:    stringPtr("Nature and purpose argument"),
			Active:         true,
			Version:        "1.0.0",
			CreatedAt:      now,
			UpdatedAt:      now,
		},
		{
			RuleName:       "products_add",
			RuleDefinition: `"products.add" string_literal+`,
			RuleType:       "production",
			Domain:         stringPtr("onboarding"),
			Description:    stringPtr("Add products with list of product names"),
			Active:         true,
			Version:        "1.0.0",
			CreatedAt:      now,
			UpdatedAt:      now,
		},
		{
			RuleName:       "var_declaration",
			RuleDefinition: `"(" "var" "(" "attr-id" string_literal ")" ")"`,
			RuleType:       "production",
			Description:    stringPtr("Variable declaration with attribute ID"),
			Active:         true,
			Version:        "1.0.0",
			CreatedAt:      now,
			UpdatedAt:      now,
		},
		{
			RuleName:       "value_binding",
			RuleDefinition: `"(" "bind" "(" "attr-id" string_literal ")" "(" "value" string_literal ")" ")"`,
			RuleType:       "production",
			Description:    stringPtr("Bind value to attribute"),
			Active:         true,
			Version:        "1.0.0",
			CreatedAt:      now,
			UpdatedAt:      now,
		},
	}
}

// stringPtr returns a pointer to a string (helper for optional fields)
func stringPtr(s string) *string {
	return &s
}
