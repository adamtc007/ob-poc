package grammar

import (
	"context"
	"fmt"
	"time"

	"dsl-ob-poc/internal/datastore"
	"dsl-ob-poc/internal/vocabulary"
)

// GrammarIntegration provides integration between grammar parsing and the DSL system
type GrammarIntegration struct {
	grammarService *DSLGrammarService
	vocabRepo      vocabulary.Repository
}

// NewGrammarIntegration creates a new grammar integration service
func NewGrammarIntegration(vocabRepo vocabulary.Repository) *GrammarIntegration {
	return &GrammarIntegration{
		grammarService: NewDSLGrammarService(vocabRepo),
		vocabRepo:      vocabRepo,
	}
}

// InitializeGrammarSystem sets up the complete grammar system
func (g *GrammarIntegration) InitializeGrammarSystem(ctx context.Context) error {
	fmt.Println("🔧 Initializing DSL Grammar System...")

	// Initialize default grammar rules
	if err := g.grammarService.InitializeDefaultGrammar(ctx); err != nil {
		return fmt.Errorf("failed to initialize default grammar: %w", err)
	}

	fmt.Println("✅ Grammar system initialized successfully")
	return nil
}

// ValidateOnboardingDSL validates onboarding DSL using grammar rules
func (g *GrammarIntegration) ValidateOnboardingDSL(ctx context.Context, dsl string) error {
	return g.grammarService.ValidateDSLForDomain(ctx, dsl, "onboarding")
}

// ValidateHedgeFundDSL validates hedge fund DSL using grammar rules
func (g *GrammarIntegration) ValidateHedgeFundDSL(ctx context.Context, dsl string) error {
	return g.grammarService.ValidateDSLForDomain(ctx, dsl, "hedge-fund-investor")
}

// ValidateGenericDSL validates DSL using universal grammar rules
func (g *GrammarIntegration) ValidateGenericDSL(ctx context.Context, dsl string) error {
	return g.grammarService.ValidateDSLForDomain(ctx, dsl, "")
}

// GetParserForDomain returns a parser configured for a specific domain
func (g *GrammarIntegration) GetParserForDomain(domain string) *EBNFParser {
	return g.grammarService.GetParserForDomain(domain)
}

// ParseAndValidateDSL performs comprehensive DSL parsing and validation
func (g *GrammarIntegration) ParseAndValidateDSL(ctx context.Context, dsl, domain string) (*ParseResult, error) {
	parser := g.GetParserForDomain(domain)

	// Load grammar for the domain
	if err := parser.LoadGrammar(ctx); err != nil {
		return nil, fmt.Errorf("failed to load grammar for domain %s: %w", domain, err)
	}

	// Parse the DSL
	result, err := parser.Parse(dsl, "dsl_document")
	if err != nil {
		return nil, fmt.Errorf("parsing failed: %w", err)
	}

	return result, nil
}

// ==============================================================================
// CLI Integration Functions
// ==============================================================================

// InitializeGrammarCLI initializes the grammar system from CLI
func InitializeGrammarCLI(ctx context.Context, ds datastore.DataStore) error {
	// Create vocabulary repository (assuming it implements the interface)
	// In a real implementation, you'd have a proper vocabulary repository
	vocabRepo := &MockVocabRepo{} // Placeholder

	integration := NewGrammarIntegration(vocabRepo)
	return integration.InitializeGrammarSystem(ctx)
}

// ValidateDSLCLI validates DSL from CLI
func ValidateDSLCLI(ctx context.Context, ds datastore.DataStore, dsl, domain string) error {
	vocabRepo := &MockVocabRepo{} // Placeholder

	integration := NewGrammarIntegration(vocabRepo)

	fmt.Printf("🔍 Validating DSL for domain: %s\n", domain)
	fmt.Printf("📝 DSL content:\n%s\n\n", dsl)

	result, err := integration.ParseAndValidateDSL(ctx, dsl, domain)
	if err != nil {
		return fmt.Errorf("validation failed: %w", err)
	}

	if result.Success {
		fmt.Println("✅ DSL validation successful!")
		fmt.Printf("📊 Parsed %d characters successfully\n", len(result.Matched))

		if result.AST != nil {
			fmt.Println("🌳 Abstract Syntax Tree:")
			printAST(result.AST, 0)
		}
	} else {
		fmt.Println("❌ DSL validation failed!")
		for _, err := range result.Errors {
			fmt.Printf("   Error at position %d: %s\n", err.Position, err.Message)
		}
	}

	return nil
}

// printAST prints the Abstract Syntax Tree in a readable format
func printAST(node *ASTNode, indent int) {
	if node == nil {
		return
	}

	indentStr := ""
	for i := 0; i < indent; i++ {
		indentStr += "  "
	}

	if node.Value != "" {
		fmt.Printf("%s%s: %s\n", indentStr, node.Type, node.Value)
	} else {
		fmt.Printf("%s%s\n", indentStr, node.Type)
	}

	for _, child := range node.Children {
		printAST(child, indent+1)
	}
}

// ==============================================================================
// Mock Implementation (for demonstration)
// ==============================================================================

// MockVocabRepo is a placeholder vocabulary repository
type MockVocabRepo struct{}

func (m *MockVocabRepo) CreateGrammarRule(ctx context.Context, rule *vocabulary.GrammarRule) error {
	fmt.Printf("📝 Creating grammar rule: %s\n", rule.RuleName)
	return nil
}

func (m *MockVocabRepo) GetGrammarRule(ctx context.Context, ruleID string) (*vocabulary.GrammarRule, error) {
	return nil, fmt.Errorf("not implemented")
}

func (m *MockVocabRepo) GetGrammarRuleByName(ctx context.Context, ruleName string) (*vocabulary.GrammarRule, error) {
	// Return nil to indicate rule doesn't exist (so it will be created)
	return nil, fmt.Errorf("rule not found")
}

func (m *MockVocabRepo) ListGrammarRules(ctx context.Context, domain *string, active *bool) ([]*vocabulary.GrammarRule, error) {
	return []*vocabulary.GrammarRule{}, nil
}

func (m *MockVocabRepo) UpdateGrammarRule(ctx context.Context, rule *vocabulary.GrammarRule) error {
	return fmt.Errorf("not implemented")
}

func (m *MockVocabRepo) DeleteGrammarRule(ctx context.Context, ruleID string) error {
	return fmt.Errorf("not implemented")
}

func (m *MockVocabRepo) ValidateGrammarSyntax(ctx context.Context, ruleDefinition string) error {
	return nil // Always valid for demo
}

func (m *MockVocabRepo) GetActiveGrammarForDomain(ctx context.Context, domain string) ([]*vocabulary.GrammarRule, error) {
	// Return empty list for now - in production this would query the database
	return []*vocabulary.GrammarRule{}, nil
}

// Implement other required methods for the Repository interface
func (m *MockVocabRepo) CreateDomainVocab(ctx context.Context, vocab *vocabulary.DomainVocabulary) error {
	return fmt.Errorf("not implemented")
}

func (m *MockVocabRepo) GetDomainVocab(ctx context.Context, vocabID string) (*vocabulary.DomainVocabulary, error) {
	return nil, fmt.Errorf("not implemented")
}

func (m *MockVocabRepo) GetDomainVocabByVerb(ctx context.Context, domain, verb string) (*vocabulary.DomainVocabulary, error) {
	return nil, fmt.Errorf("not implemented")
}

func (m *MockVocabRepo) ListDomainVocabs(ctx context.Context, domain *string, category *string, active *bool) ([]*vocabulary.DomainVocabulary, error) {
	return []*vocabulary.DomainVocabulary{}, nil
}

func (m *MockVocabRepo) UpdateDomainVocab(ctx context.Context, vocab *vocabulary.DomainVocabulary) error {
	return fmt.Errorf("not implemented")
}

func (m *MockVocabRepo) DeleteDomainVocab(ctx context.Context, vocabID string) error {
	return fmt.Errorf("not implemented")
}

func (m *MockVocabRepo) ValidateVerb(ctx context.Context, domain, verb string) error {
	return nil
}

func (m *MockVocabRepo) GetApprovedVerbs(ctx context.Context, domain string) ([]string, error) {
	return []string{}, nil
}

func (m *MockVocabRepo) GetAllApprovedVerbs(ctx context.Context) (map[string][]string, error) {
	return map[string][]string{}, nil
}

func (m *MockVocabRepo) RegisterVerb(ctx context.Context, registry *vocabulary.VerbRegistry) error {
	return fmt.Errorf("not implemented")
}

func (m *MockVocabRepo) GetVerbRegistry(ctx context.Context, verb string) (*vocabulary.VerbRegistry, error) {
	return nil, fmt.Errorf("not implemented")
}

func (m *MockVocabRepo) ListVerbRegistry(ctx context.Context, domain *string, shared *bool, deprecated *bool) ([]*vocabulary.VerbRegistry, error) {
	return []*vocabulary.VerbRegistry{}, nil
}

func (m *MockVocabRepo) UpdateVerbRegistry(ctx context.Context, registry *vocabulary.VerbRegistry) error {
	return fmt.Errorf("not implemented")
}

func (m *MockVocabRepo) DeprecateVerb(ctx context.Context, verb, replacementVerb string, reason string) error {
	return fmt.Errorf("not implemented")
}

func (m *MockVocabRepo) CheckVerbConflicts(ctx context.Context, verb string, domains []string) ([]*vocabulary.VerbRegistry, error) {
	return []*vocabulary.VerbRegistry{}, nil
}

func (m *MockVocabRepo) GetSharedVerbs(ctx context.Context) ([]*vocabulary.VerbRegistry, error) {
	return []*vocabulary.VerbRegistry{}, nil
}

func (m *MockVocabRepo) CreateAudit(ctx context.Context, audit *vocabulary.VocabularyAudit) error {
	return fmt.Errorf("not implemented")
}

func (m *MockVocabRepo) GetAuditHistory(ctx context.Context, domain, verb string, limit int) ([]*vocabulary.VocabularyAudit, error) {
	return []*vocabulary.VocabularyAudit{}, nil
}

func (m *MockVocabRepo) GetRecentChanges(ctx context.Context, domain *string, changeType *string, limit int) ([]*vocabulary.VocabularyAudit, error) {
	return []*vocabulary.VocabularyAudit{}, nil
}

func (m *MockVocabRepo) GetChangesSince(ctx context.Context, since time.Time, domain *string) ([]*vocabulary.VocabularyAudit, error) {
	return []*vocabulary.VocabularyAudit{}, nil
}

func (m *MockVocabRepo) GetChangeSummary(ctx context.Context, domain string, period time.Duration) (*vocabulary.ChangeSummary, error) {
	return nil, fmt.Errorf("not implemented")
}

func (m *MockVocabRepo) BeginTx(ctx context.Context) (vocabulary.Repository, error) {
	return m, nil
}

func (m *MockVocabRepo) Commit() error {
	return nil
}

func (m *MockVocabRepo) Rollback() error {
	return nil
}
