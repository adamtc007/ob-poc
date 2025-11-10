// Package grammar provides EBNF grammar parsing and validation for the DSL system
// This integrates with the database-stored grammar rules to provide dynamic grammar parsing
package grammar

import (
	"context"
	"fmt"
	"regexp"
	"strings"
	"sync"

	"dsl-ob-poc/internal/vocabulary"
)

// EBNFParser provides EBNF grammar parsing and validation
type EBNFParser struct {
	repo   vocabulary.GrammarRepository
	rules  map[string]*ParsedRule
	mutex  sync.RWMutex
	domain string
}

// ParsedRule represents a compiled EBNF rule
type ParsedRule struct {
	Name       string
	Definition string
	RuleType   string
	Compiled   *CompiledRule
}

// CompiledRule represents a compiled grammar rule for efficient parsing
type CompiledRule struct {
	Pattern      *regexp.Regexp
	Alternatives []Alternative
	IsTerminal   bool
	IsOptional   bool
	IsRepeating  bool
}

// Alternative represents one alternative in a rule definition
type Alternative struct {
	Tokens []Token
	Action string // Optional action to take when this alternative matches
}

// Token represents a single token in a grammar rule
type Token struct {
	Type     TokenType
	Value    string
	Optional bool
	Repeat   RepeatType
	RuleRef  string // Reference to another rule
}

// TokenType represents the type of a grammar token
type TokenType int

const (
	TokenLiteral TokenType = iota
	TokenRuleRef
	TokenRegex
	TokenGroup
)

// RepeatType represents repetition patterns
type RepeatType int

const (
	RepeatNone RepeatType = iota
	RepeatZeroOrMore
	RepeatOneOrMore
	RepeatOptional
)

// ParseResult represents the result of parsing input against grammar rules
type ParseResult struct {
	Success   bool
	Matched   string
	Remaining string
	Rule      string
	Errors    []ParseError
	AST       *ASTNode
}

// ParseError represents a parsing error
type ParseError struct {
	Position int
	Expected string
	Found    string
	Rule     string
	Message  string
}

// ASTNode represents a node in the Abstract Syntax Tree
type ASTNode struct {
	Type     string
	Value    string
	Children []*ASTNode
	Position int
	Length   int
}

// NewEBNFParser creates a new EBNF parser for the specified domain
func NewEBNFParser(repo vocabulary.GrammarRepository, domain string) *EBNFParser {
	return &EBNFParser{
		repo:   repo,
		rules:  make(map[string]*ParsedRule),
		domain: domain,
	}
}

// LoadGrammar loads grammar rules from the database for the specified domain
func (p *EBNFParser) LoadGrammar(ctx context.Context) error {
	p.mutex.Lock()
	defer p.mutex.Unlock()

	// Load domain-specific rules
	domainRules, err := p.repo.GetActiveGrammarForDomain(ctx, p.domain)
	if err != nil {
		return fmt.Errorf("failed to load domain grammar: %w", err)
	}

	// Load universal rules (domain = NULL)
	universalRules, err := p.repo.GetActiveGrammarForDomain(ctx, "")
	if err != nil {
		return fmt.Errorf("failed to load universal grammar: %w", err)
	}

	// Combine and compile rules
	allRules := append(domainRules, universalRules...)
	p.rules = make(map[string]*ParsedRule)

	for _, rule := range allRules {
		parsed, err := p.parseRule(rule)
		if err != nil {
			return fmt.Errorf("failed to parse rule %s: %w", rule.RuleName, err)
		}
		p.rules[rule.RuleName] = parsed
	}

	return nil
}

// Parse parses input text against the loaded grammar rules
func (p *EBNFParser) Parse(input string, startRule string) (*ParseResult, error) {
	p.mutex.RLock()
	defer p.mutex.RUnlock()

	rule, exists := p.rules[startRule]
	if !exists {
		return &ParseResult{
			Success: false,
			Errors: []ParseError{{
				Rule:    startRule,
				Message: fmt.Sprintf("rule '%s' not found", startRule),
			}},
		}, nil
	}

	return p.parseWithRule(input, rule, 0)
}

// ValidateDSL validates DSL text against the grammar
func (p *EBNFParser) ValidateDSL(ctx context.Context, dsl string) error {
	// Ensure grammar is loaded
	if len(p.rules) == 0 {
		if err := p.LoadGrammar(ctx); err != nil {
			return fmt.Errorf("failed to load grammar: %w", err)
		}
	}

	// Parse starting with the main DSL rule
	result, err := p.Parse(strings.TrimSpace(dsl), "dsl_document")
	if err != nil {
		return fmt.Errorf("parse error: %w", err)
	}

	if !result.Success {
		return fmt.Errorf("DSL validation failed: %v", result.Errors)
	}

	if strings.TrimSpace(result.Remaining) != "" {
		return fmt.Errorf("unexpected content after parsing: %s", result.Remaining)
	}

	return nil
}

// parseRule compiles a database grammar rule into a ParsedRule
func (p *EBNFParser) parseRule(rule *vocabulary.GrammarRule) (*ParsedRule, error) {
	parsed := &ParsedRule{
		Name:       rule.RuleName,
		Definition: rule.RuleDefinition,
		RuleType:   rule.RuleType,
	}

	compiled, err := p.compileRule(rule.RuleDefinition)
	if err != nil {
		return nil, err
	}

	parsed.Compiled = compiled
	return parsed, nil
}

// compileRule compiles an EBNF rule definition into a CompiledRule
func (p *EBNFParser) compileRule(definition string) (*CompiledRule, error) {
	compiled := &CompiledRule{}

	// Parse alternatives separated by |
	alternatives := strings.Split(definition, "|")

	for _, alt := range alternatives {
		alt = strings.TrimSpace(alt)
		if alt == "" {
			continue
		}

		alternative, err := p.parseAlternative(alt)
		if err != nil {
			return nil, err
		}

		compiled.Alternatives = append(compiled.Alternatives, alternative)
	}

	return compiled, nil
}

// parseAlternative parses a single alternative in an EBNF rule
func (p *EBNFParser) parseAlternative(alt string) (Alternative, error) {
	alternative := Alternative{}

	// Simple tokenization - this could be more sophisticated
	tokens := p.tokenizeAlternative(alt)

	for _, tokenStr := range tokens {
		token, err := p.parseToken(tokenStr)
		if err != nil {
			return alternative, err
		}
		alternative.Tokens = append(alternative.Tokens, token)
	}

	return alternative, nil
}

// tokenizeAlternative splits an alternative into tokens
func (p *EBNFParser) tokenizeAlternative(alt string) []string {
	// Simple space-based tokenization for now
	// In a full implementation, this would handle quoted strings, groups, etc.
	return strings.Fields(alt)
}

// parseToken parses a single token string into a Token
func (p *EBNFParser) parseToken(tokenStr string) (Token, error) {
	token := Token{}

	// Handle repetition operators
	if strings.HasSuffix(tokenStr, "*") {
		token.Repeat = RepeatZeroOrMore
		tokenStr = strings.TrimSuffix(tokenStr, "*")
	} else if strings.HasSuffix(tokenStr, "+") {
		token.Repeat = RepeatOneOrMore
		tokenStr = strings.TrimSuffix(tokenStr, "+")
	} else if strings.HasSuffix(tokenStr, "?") {
		token.Repeat = RepeatOptional
		tokenStr = strings.TrimSuffix(tokenStr, "?")
	}

	// Handle quoted literals
	if strings.HasPrefix(tokenStr, `"`) && strings.HasSuffix(tokenStr, `"`) {
		token.Type = TokenLiteral
		token.Value = strings.Trim(tokenStr, `"`)
	} else {
		// Assume it's a rule reference
		token.Type = TokenRuleRef
		token.RuleRef = tokenStr
	}

	return token, nil
}

// parseWithRule parses input using a specific rule
func (p *EBNFParser) parseWithRule(input string, rule *ParsedRule, position int) (*ParseResult, error) {
	for _, alt := range rule.Compiled.Alternatives {
		result := p.tryAlternative(input, alt, position, rule.Name)
		if result.Success {
			return result, nil
		}
	}

	return &ParseResult{
		Success: false,
		Errors: []ParseError{{
			Position: position,
			Rule:     rule.Name,
			Message:  fmt.Sprintf("no alternative matched in rule '%s'", rule.Name),
		}},
	}, nil
}

// tryAlternative attempts to match an alternative against input
func (p *EBNFParser) tryAlternative(input string, alt Alternative, position int, ruleName string) *ParseResult {
	currentPos := position
	currentInput := input[position:]
	ast := &ASTNode{
		Type:     ruleName,
		Position: position,
	}

	for _, token := range alt.Tokens {
		result := p.matchToken(currentInput, token, currentPos)
		if !result.Success {
			return &ParseResult{
				Success: false,
				Errors:  result.Errors,
			}
		}

		if result.AST != nil {
			ast.Children = append(ast.Children, result.AST)
		}

		matchedLen := len(result.Matched)
		currentPos += matchedLen
		currentInput = currentInput[matchedLen:]
	}

	ast.Length = currentPos - position

	return &ParseResult{
		Success:   true,
		Matched:   input[position:currentPos],
		Remaining: currentInput,
		Rule:      ruleName,
		AST:       ast,
	}
}

// matchToken attempts to match a single token against input
func (p *EBNFParser) matchToken(input string, token Token, position int) *ParseResult {
	switch token.Type {
	case TokenLiteral:
		return p.matchLiteral(input, token.Value, position)
	case TokenRuleRef:
		rule, exists := p.rules[token.RuleRef]
		if !exists {
			return &ParseResult{
				Success: false,
				Errors: []ParseError{{
					Position: position,
					Rule:     token.RuleRef,
					Message:  fmt.Sprintf("referenced rule '%s' not found", token.RuleRef),
				}},
			}
		}
		return p.parseWithRuleFromPosition(input, rule, position)
	default:
		return &ParseResult{
			Success: false,
			Errors: []ParseError{{
				Position: position,
				Message:  fmt.Sprintf("unsupported token type: %v", token.Type),
			}},
		}
	}
}

// matchLiteral matches a literal string
func (p *EBNFParser) matchLiteral(input, literal string, position int) *ParseResult {
	if strings.HasPrefix(input, literal) {
		return &ParseResult{
			Success:   true,
			Matched:   literal,
			Remaining: input[len(literal):],
			AST: &ASTNode{
				Type:     "literal",
				Value:    literal,
				Position: position,
				Length:   len(literal),
			},
		}
	}

	return &ParseResult{
		Success: false,
		Errors: []ParseError{{
			Position: position,
			Expected: literal,
			Found:    input[:min(len(input), 10)],
			Message:  fmt.Sprintf("expected '%s'", literal),
		}},
	}
}

// parseWithRuleFromPosition is a helper to parse from a specific position
func (p *EBNFParser) parseWithRuleFromPosition(input string, rule *ParsedRule, position int) *ParseResult {
	result, _ := p.parseWithRule(input, rule, 0)
	if result.Success {
		result.AST.Position = position
	}
	return result
}

// min returns the minimum of two integers
func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}

// GetLoadedRules returns the names of all loaded rules
func (p *EBNFParser) GetLoadedRules() []string {
	p.mutex.RLock()
	defer p.mutex.RUnlock()

	var rules []string
	for name := range p.rules {
		rules = append(rules, name)
	}
	return rules
}
