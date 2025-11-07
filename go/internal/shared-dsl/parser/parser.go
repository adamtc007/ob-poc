// Package parser provides domain-agnostic S-expression parsing for DSL.
//
// This parser is shared across ALL domains (onboarding, hedge-fund-investor, kyc, etc.)
// and only validates syntax structure, not semantic meaning (verb validity).
//
// The parser converts DSL S-expressions into an Abstract Syntax Tree (AST) that can be
// traversed and analyzed by domain-specific validators and executors.
package parser

import (
	"fmt"
	"regexp"
	"strings"
	"unicode"
)

// NodeType represents the type of a node in the AST
type NodeType int

const (
	// RootNode is the top-level node containing all expressions
	RootNode NodeType = iota
	// ExpressionNode is an S-expression: (verb args...)
	ExpressionNode
	// VerbNode is a verb identifier: verb.action
	VerbNode
	// IdentifierNode is a parameter name: cbu.id, attr-id, etc.
	IdentifierNode
	// StringNode is a quoted string value
	StringNode
	// NumberNode is a numeric value
	NumberNode
	// BooleanNode is a boolean value
	BooleanNode
	// AttributeNode is an attribute reference: @attr{uuid:name} or @attr{uuid}
	AttributeNode
)

// String returns the string representation of a NodeType
func (nt NodeType) String() string {
	switch nt {
	case RootNode:
		return "Root"
	case ExpressionNode:
		return "Expression"
	case VerbNode:
		return "Verb"
	case IdentifierNode:
		return "Identifier"
	case StringNode:
		return "String"
	case NumberNode:
		return "Number"
	case BooleanNode:
		return "Boolean"
	case AttributeNode:
		return "Attribute"
	default:
		return "Unknown"
	}
}

// Node represents a node in the Abstract Syntax Tree
type Node struct {
	Type        NodeType
	Value       string
	Children    []*Node
	Line        int    // Line number in source DSL (for error reporting)
	Column      int    // Column number in source DSL
	AttributeID string // For AttributeNode: the UUID part
	Name        string // For AttributeNode: the human-readable name part
}

// AST represents the Abstract Syntax Tree of parsed DSL
type AST struct {
	Root *Node
}

// Parser parses DSL S-expressions into an AST
type Parser struct {
	input  string
	pos    int
	line   int
	column int
}

// NewParser creates a new parser for the given DSL input
func NewParser(input string) *Parser {
	return &Parser{
		input:  input,
		pos:    0,
		line:   1,
		column: 1,
	}
}

// Parse parses the DSL input and returns an AST
func Parse(input string) (*AST, error) {
	p := NewParser(input)
	root, err := p.parseRoot()
	if err != nil {
		return nil, err
	}
	return &AST{Root: root}, nil
}

// parseRoot parses the root level, which can contain multiple top-level expressions
func (p *Parser) parseRoot() (*Node, error) {
	root := &Node{
		Type:     RootNode,
		Value:    "",
		Children: make([]*Node, 0),
		Line:     1,
		Column:   1,
	}

	for {
		p.skipWhitespaceAndComments()
		if p.isEOF() {
			break
		}

		expr, err := p.parseExpression()
		if err != nil {
			return nil, err
		}
		root.Children = append(root.Children, expr)
	}

	return root, nil
}

// parseExpression parses a single S-expression: (verb args...)
func (p *Parser) parseExpression() (*Node, error) {
	p.skipWhitespaceAndComments()

	if !p.match('(') {
		return nil, p.error("expected '(' at start of expression")
	}

	line, column := p.line, p.column
	p.advance() // consume '('

	node := &Node{
		Type:     ExpressionNode,
		Children: make([]*Node, 0),
		Line:     line,
		Column:   column,
	}

	p.skipWhitespaceAndComments()

	// First element should be the verb (identifier with dot: verb.action)
	if p.isEOF() {
		return nil, p.error("unexpected EOF, expected verb")
	}

	verb, err := p.parseVerb()
	if err != nil {
		return nil, err
	}
	node.Value = verb.Value // Store verb as expression value for convenience
	node.Children = append(node.Children, verb)

	// Parse arguments until we hit ')'
	for {
		p.skipWhitespaceAndComments()

		if p.match(')') {
			p.advance() // consume ')'
			break
		}

		if p.isEOF() {
			return nil, p.error("unexpected EOF, expected ')' to close expression")
		}

		arg, parseErr := p.parseArgument()
		if parseErr != nil {
			return nil, parseErr
		}
		node.Children = append(node.Children, arg)
	}

	return node, nil
}

// parseVerb parses a verb (identifier with dot notation: verb.action)
func (p *Parser) parseVerb() (*Node, error) {
	line, column := p.line, p.column
	identifier := p.readIdentifier()

	if identifier == "" {
		return nil, p.error("expected verb identifier")
	}

	// Verbs should contain a dot (e.g., case.create, products.add)
	// But we don't enforce this at parse time - domain validators check semantics
	node := &Node{
		Type:   VerbNode,
		Value:  identifier,
		Line:   line,
		Column: column,
	}

	return node, nil
}

// parseArgument parses an argument which can be:
// - A nested expression: (...)
// - A string: "..."
// - A number: 123, 45.67
// - A boolean: true, false
// - An identifier: attr-id, cbu.id, etc.
func (p *Parser) parseArgument() (*Node, error) {
	p.skipWhitespaceAndComments()

	line, column := p.line, p.column

	// Nested expression
	if p.match('(') {
		return p.parseExpression()
	}

	// String literal
	if p.match('"') {
		return p.parseString()
	}

	// Attribute reference: @attr{uuid:name} or @attr{uuid}
	if p.match('@') {
		return p.parseAttribute()
	}

	// Number or identifier or boolean
	if p.isDigit(p.peek()) || p.peek() == '-' {
		// Could be a number or negative number
		return p.parseNumberOrIdentifier()
	}

	// Identifier or boolean
	identifier := p.readIdentifier()
	if identifier == "" {
		return nil, p.error("expected argument value")
	}

	// Check if it's a boolean
	if identifier == "true" || identifier == "false" {
		return &Node{
			Type:   BooleanNode,
			Value:  identifier,
			Line:   line,
			Column: column,
		}, nil
	}

	// Otherwise it's an identifier (like attr-id, for.product, etc.)
	return &Node{
		Type:   IdentifierNode,
		Value:  identifier,
		Line:   line,
		Column: column,
	}, nil
}

// parseString parses a quoted string literal
func (p *Parser) parseString() (*Node, error) {
	line, column := p.line, p.column
	p.advance() // consume opening quote

	var sb strings.Builder
	for !p.isEOF() && !p.match('"') {
		if p.match('\\') {
			p.advance() // consume backslash
			if p.isEOF() {
				return nil, p.error("unexpected EOF in string escape sequence")
			}
			// Handle escape sequences
			switch p.peek() {
			case 'n':
				sb.WriteRune('\n')
			case 't':
				sb.WriteRune('\t')
			case 'r':
				sb.WriteRune('\r')
			case '"':
				sb.WriteRune('"')
			case '\\':
				sb.WriteRune('\\')
			default:
				sb.WriteRune(p.peek())
			}
			p.advance()
		} else {
			sb.WriteRune(p.peek())
			p.advance()
		}
	}

	if !p.match('"') {
		return nil, p.error("unterminated string literal")
	}
	p.advance() // consume closing quote

	return &Node{
		Type:   StringNode,
		Value:  sb.String(),
		Line:   line,
		Column: column,
	}, nil
}

// parseNumberOrIdentifier attempts to parse a number, falls back to identifier
func (p *Parser) parseNumberOrIdentifier() (*Node, error) {
	line, column := p.line, p.column
	start := p.pos

	// Try to match a number pattern
	if p.match('-') {
		p.advance()
	}

	hasDigits := false
	for !p.isEOF() && p.isDigit(p.peek()) {
		p.advance()
		hasDigits = true
	}

	// Check for decimal point
	if !p.isEOF() && p.match('.') {
		p.advance()
		for !p.isEOF() && p.isDigit(p.peek()) {
			p.advance()
		}
	}

	value := p.input[start:p.pos]

	// If we found digits and next char is whitespace or delimiter, it's a number
	if hasDigits && (p.isEOF() || unicode.IsSpace(p.peek()) || p.match(')')) {
		return &Node{
			Type:   NumberNode,
			Value:  value,
			Line:   line,
			Column: column,
		}, nil
	}

	// Otherwise, reset and parse as identifier
	p.pos = start
	p.line = line
	p.column = column
	identifier := p.readIdentifier()

	return &Node{
		Type:   IdentifierNode,
		Value:  identifier,
		Line:   line,
		Column: column,
	}, nil
}

// readIdentifier reads an identifier (letters, digits, dots, hyphens, underscores)
func (p *Parser) readIdentifier() string {
	start := p.pos
	for !p.isEOF() && p.isIdentifierChar(p.peek()) {
		p.advance()
	}
	return p.input[start:p.pos]
}

// parseAttribute parses an attribute reference: @attr{uuid:name} or @attr{uuid}
func (p *Parser) parseAttribute() (*Node, error) {
	line, column := p.line, p.column

	if !p.match('@') {
		return nil, p.error("expected '@' at start of attribute")
	}
	p.advance() // consume '@'

	// Expect 'attr{'
	if !p.matchSequence("attr{") {
		return nil, p.error("expected 'attr{' after '@'")
	}
	p.advanceN(5) // consume 'attr{'

	// Read until '}' or ':'
	var attrID strings.Builder
	for !p.isEOF() && !p.match('}') && !p.match(':') {
		attrID.WriteRune(p.peek())
		p.advance()
	}

	if attrID.Len() == 0 {
		return nil, p.error("expected attribute ID")
	}

	node := &Node{
		Type:        AttributeNode,
		Value:       fmt.Sprintf("@attr{%s}", attrID.String()), // Will be updated if name provided
		Line:        line,
		Column:      column,
		AttributeID: attrID.String(),
	}

	// Check if there's a human-readable name after ':'
	if p.match(':') {
		p.advance() // consume ':'

		var name strings.Builder
		for !p.isEOF() && !p.match('}') {
			name.WriteRune(p.peek())
			p.advance()
		}

		if name.Len() == 0 {
			return nil, p.error("expected attribute name after ':'")
		}

		node.Name = name.String()
		node.Value = fmt.Sprintf("@attr{%s:%s}", attrID.String(), name.String())
	}

	if !p.match('}') {
		return nil, p.error("expected '}' to close attribute")
	}
	p.advance() // consume '}'

	return node, nil
}

// matchSequence checks if the input matches a sequence of characters
func (p *Parser) matchSequence(seq string) bool {
	for i, r := range seq {
		if p.pos+i >= len(p.input) || rune(p.input[p.pos+i]) != r {
			return false
		}
	}
	return true
}

// advanceN advances the parser position by n characters
func (p *Parser) advanceN(n int) {
	for i := 0; i < n && !p.isEOF(); i++ {
		p.advance()
	}
}

// isIdentifierChar checks if a rune is valid in an identifier
func (p *Parser) isIdentifierChar(r rune) bool {
	return unicode.IsLetter(r) || unicode.IsDigit(r) || r == '.' || r == '-' || r == '_'
}

// isDigit checks if a rune is a digit
func (p *Parser) isDigit(r rune) bool {
	return r >= '0' && r <= '9'
}

// skipWhitespaceAndComments skips whitespace and comments (lines starting with ;)
func (p *Parser) skipWhitespaceAndComments() {
	for !p.isEOF() {
		if unicode.IsSpace(p.peek()) {
			p.advance()
		} else if p.match(';') {
			// Skip until end of line (comment)
			for !p.isEOF() && p.peek() != '\n' {
				p.advance()
			}
		} else {
			break
		}
	}
}

// peek returns the current rune without advancing
func (p *Parser) peek() rune {
	if p.isEOF() {
		return 0
	}
	return rune(p.input[p.pos])
}

// advance moves to the next rune
func (p *Parser) advance() {
	if !p.isEOF() {
		if p.peek() == '\n' {
			p.line++
			p.column = 1
		} else {
			p.column++
		}
		p.pos++
	}
}

// match checks if the current rune matches the given rune
func (p *Parser) match(r rune) bool {
	return p.peek() == r
}

// isEOF checks if we've reached the end of input
func (p *Parser) isEOF() bool {
	return p.pos >= len(p.input)
}

// error creates a parse error with line/column information
func (p *Parser) error(message string) error {
	return fmt.Errorf("parse error at line %d, column %d: %s", p.line, p.column, message)
}

// ExtractVerbs extracts all verbs from the AST (useful for domain validation)
func (ast *AST) ExtractVerbs() []string {
	verbs := make([]string, 0)
	seen := make(map[string]bool)
	ast.traverse(ast.Root, func(node *Node) {
		if node.Type == VerbNode && !seen[node.Value] {
			verbs = append(verbs, node.Value)
			seen[node.Value] = true
		}
	})
	return verbs
}

// ExtractAttributeIDs extracts all attribute IDs from the AST (both old attr-id and new @attr syntax)
func (ast *AST) ExtractAttributeIDs() []string {
	attrIDs := make([]string, 0)
	seen := make(map[string]bool)

	ast.traverse(ast.Root, func(node *Node) {
		// New @attr{uuid:name} syntax
		if node.Type == AttributeNode && node.AttributeID != "" && !seen[node.AttributeID] {
			attrIDs = append(attrIDs, node.AttributeID)
			seen[node.AttributeID] = true
			return
		}

		// Legacy patterns: (var (attr-id "uuid")) and (bind (attr-id "uuid") ...)
		if node.Type == ExpressionNode && len(node.Children) >= 2 {
			firstChild := node.Children[0]
			if (firstChild.Type == VerbNode || firstChild.Type == IdentifierNode) && firstChild.Value == "var" {
				// Look for nested (attr-id "uuid") expression
				for _, child := range node.Children[1:] {
					if child.Type == ExpressionNode && len(child.Children) >= 2 {
						attrChild := child.Children[0]
						if (attrChild.Type == VerbNode || attrChild.Type == IdentifierNode) && attrChild.Value == "attr-id" {
							if len(child.Children) > 1 && child.Children[1].Type == StringNode && !seen[child.Children[1].Value] {
								attrIDs = append(attrIDs, child.Children[1].Value)
								seen[child.Children[1].Value] = true
							}
						}
					}
				}
			}
			// Also look for pattern: (bind (attr-id "uuid") (value "..."))
			if (firstChild.Type == VerbNode || firstChild.Type == IdentifierNode) && firstChild.Value == "bind" {
				for _, child := range node.Children[1:] {
					if child.Type == ExpressionNode && len(child.Children) >= 2 {
						attrChild := child.Children[0]
						if (attrChild.Type == VerbNode || attrChild.Type == IdentifierNode) && attrChild.Value == "attr-id" {
							if len(child.Children) > 1 && child.Children[1].Type == StringNode && !seen[child.Children[1].Value] {
								attrIDs = append(attrIDs, child.Children[1].Value)
								seen[child.Children[1].Value] = true
							}
						}
					}
				}
			}
		}
	})
	return attrIDs
}

// ExtractAttributes extracts all attribute references from the AST with their metadata
func (ast *AST) ExtractAttributes() []AttributeReference {
	attributes := make([]AttributeReference, 0)
	seen := make(map[string]bool)

	ast.traverse(ast.Root, func(node *Node) {
		if node.Type == AttributeNode && !seen[node.AttributeID] {
			attributes = append(attributes, AttributeReference{
				ID:     node.AttributeID,
				Name:   node.Name,
				Line:   node.Line,
				Column: node.Column,
			})
			seen[node.AttributeID] = true
		}
	})
	return attributes
}

// AttributeReference represents an attribute reference found in the DSL
type AttributeReference struct {
	ID     string
	Name   string
	Line   int
	Column int
}

// traverse performs depth-first traversal of the AST
func (ast *AST) traverse(node *Node, visitor func(*Node)) {
	if node == nil {
		return
	}
	visitor(node)
	for _, child := range node.Children {
		ast.traverse(child, visitor)
	}
}

// String returns a string representation of the AST (for debugging)
func (ast *AST) String() string {
	var sb strings.Builder
	ast.printNode(ast.Root, 0, &sb)
	return sb.String()
}

// printNode recursively prints the AST structure
func (ast *AST) printNode(node *Node, depth int, sb *strings.Builder) {
	if node == nil {
		return
	}
	indent := strings.Repeat("  ", depth)
	sb.WriteString(fmt.Sprintf("%s%s: %q\n", indent, node.Type.String(), node.Value))
	for _, child := range node.Children {
		ast.printNode(child, depth+1, sb)
	}
}

// ValidatePlaceholders checks for unresolved placeholders like <investor_id>
func ValidatePlaceholders(dsl string) error {
	placeholderPattern := regexp.MustCompile(`<[a-zA-Z_][a-zA-Z0-9_]*>`)
	matches := placeholderPattern.FindAllString(dsl, -1)
	if len(matches) > 0 {
		return fmt.Errorf("found %d unresolved placeholder(s): %v", len(matches), matches)
	}
	return nil
}
