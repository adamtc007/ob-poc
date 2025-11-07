package parser

import (
	"fmt"
	"strings"
	"testing"
)

// =============================================================================
// Basic Parsing Tests
// =============================================================================

func TestParse_SimpleVerb(t *testing.T) {
	dsl := `(case.create)`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	if ast.Root == nil {
		t.Fatal("Expected non-nil root node")
	}

	if len(ast.Root.Children) != 1 {
		t.Fatalf("Expected 1 top-level expression, got %d", len(ast.Root.Children))
	}

	expr := ast.Root.Children[0]
	if expr.Type != ExpressionNode {
		t.Errorf("Expected ExpressionNode, got %v", expr.Type)
	}

	if expr.Value != "case.create" {
		t.Errorf("Expected verb 'case.create', got '%s'", expr.Value)
	}
}

func TestParse_NestedExpressions(t *testing.T) {
	dsl := `(case.create
  (cbu.id "CBU-1234")
  (nature-purpose "Test fund")
)`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	expr := ast.Root.Children[0]
	if len(expr.Children) != 3 { // verb + 2 nested expressions
		t.Fatalf("Expected 3 children (verb + 2 args), got %d", len(expr.Children))
	}

	// First child should be the verb
	if expr.Children[0].Type != VerbNode {
		t.Errorf("Expected VerbNode, got %v", expr.Children[0].Type)
	}

	// Second child should be nested expression (cbu.id ...)
	if expr.Children[1].Type != ExpressionNode {
		t.Errorf("Expected nested ExpressionNode, got %v", expr.Children[1].Type)
	}
}

func TestParse_MultipleTopLevel(t *testing.T) {
	dsl := `(case.create (cbu.id "CBU-1234"))

(products.add "CUSTODY" "FUND_ACCOUNTING")

(kyc.start)`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	if len(ast.Root.Children) != 3 {
		t.Fatalf("Expected 3 top-level expressions, got %d", len(ast.Root.Children))
	}

	verbs := []string{"case.create", "products.add", "kyc.start"}
	for i, expected := range verbs {
		if ast.Root.Children[i].Value != expected {
			t.Errorf("Expression %d: expected verb '%s', got '%s'", i, expected, ast.Root.Children[i].Value)
		}
	}
}

func TestParse_EmptyDSL(t *testing.T) {
	dsl := ""

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	if len(ast.Root.Children) != 0 {
		t.Errorf("Expected 0 expressions in empty DSL, got %d", len(ast.Root.Children))
	}
}

func TestParse_MalformedSyntax(t *testing.T) {
	tests := []struct {
		name string
		dsl  string
	}{
		{"Missing closing paren", "(case.create (cbu.id \"CBU-1234\""},
		{"Missing opening paren", "case.create)"},
		{"Unterminated string", "(case.create \"unclosed"},
		{"Extra closing paren", "(case.create))"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			_, err := Parse(tt.dsl)
			if err == nil {
				t.Errorf("Expected parse error for malformed DSL: %s", tt.dsl)
			}
		})
	}
}

// =============================================================================
// Onboarding DSL Tests (CRITICAL - Must Work)
// =============================================================================

func TestParse_OnboardingCaseCreate(t *testing.T) {
	dsl := `(case.create
  (cbu.id "CBU-1234")
  (nature-purpose "UCITS equity fund domiciled in Luxembourg")
)`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	expr := ast.Root.Children[0]
	if expr.Value != "case.create" {
		t.Errorf("Expected verb 'case.create', got '%s'", expr.Value)
	}

	// Verify children: verb + 2 nested expressions
	if len(expr.Children) != 3 {
		t.Fatalf("Expected 3 children, got %d", len(expr.Children))
	}

	// Check cbu.id expression
	cbuExpr := expr.Children[1]
	if cbuExpr.Type != ExpressionNode {
		t.Errorf("Expected ExpressionNode for cbu.id, got %v", cbuExpr.Type)
	}
	if len(cbuExpr.Children) < 2 {
		t.Fatalf("cbu.id expression should have at least 2 children (identifier + value)")
	}
	if cbuExpr.Children[0].Value != "cbu.id" {
		t.Errorf("Expected 'cbu.id', got '%s'", cbuExpr.Children[0].Value)
	}
	if cbuExpr.Children[1].Type != StringNode {
		t.Errorf("Expected StringNode for CBU ID value, got %v", cbuExpr.Children[1].Type)
	}
	if cbuExpr.Children[1].Value != "CBU-1234" {
		t.Errorf("Expected 'CBU-1234', got '%s'", cbuExpr.Children[1].Value)
	}
}

func TestParse_OnboardingProductsAdd(t *testing.T) {
	dsl := `(products.add "CUSTODY" "FUND_ACCOUNTING" "TRANSFER_AGENCY")`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	expr := ast.Root.Children[0]
	if expr.Value != "products.add" {
		t.Errorf("Expected verb 'products.add', got '%s'", expr.Value)
	}

	// Should have verb + 3 string arguments
	if len(expr.Children) != 4 {
		t.Fatalf("Expected 4 children (verb + 3 products), got %d", len(expr.Children))
	}

	products := []string{"CUSTODY", "FUND_ACCOUNTING", "TRANSFER_AGENCY"}
	for i, expected := range products {
		child := expr.Children[i+1] // Skip verb
		if child.Type != StringNode {
			t.Errorf("Product %d: expected StringNode, got %v", i, child.Type)
		}
		if child.Value != expected {
			t.Errorf("Product %d: expected '%s', got '%s'", i, expected, child.Value)
		}
	}
}

func TestParse_OnboardingKYCStart(t *testing.T) {
	dsl := `(kyc.start
  (documents
    (document "CertificateOfIncorporation")
    (document "ArticlesOfAssociation")
    (document "W8BEN-E")
  )
  (jurisdictions
    (jurisdiction "LU")
  )
)`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	expr := ast.Root.Children[0]
	if expr.Value != "kyc.start" {
		t.Errorf("Expected verb 'kyc.start', got '%s'", expr.Value)
	}

	// Should have verb + 2 nested blocks (documents, jurisdictions)
	if len(expr.Children) != 3 {
		t.Fatalf("Expected 3 children (verb + 2 blocks), got %d", len(expr.Children))
	}

	// Check documents block
	docsBlock := expr.Children[1]
	if docsBlock.Type != ExpressionNode {
		t.Errorf("Expected ExpressionNode for documents block, got %v", docsBlock.Type)
	}
	if docsBlock.Children[0].Value != "documents" {
		t.Errorf("Expected 'documents', got '%s'", docsBlock.Children[0].Value)
	}

	// Should have 3 document expressions
	docCount := 0
	for _, child := range docsBlock.Children[1:] { // Skip 'documents' identifier
		if child.Type == ExpressionNode && child.Children[0].Value == "document" {
			docCount++
		}
	}
	if docCount != 3 {
		t.Errorf("Expected 3 document expressions, got %d", docCount)
	}
}

func TestParse_OnboardingServicesDiscover(t *testing.T) {
	dsl := `(services.discover
  (for.product "CUSTODY"
    (service "CustodyService")
    (service "SettlementService")
  )
  (for.product "FUND_ACCOUNTING"
    (service "FundAccountingService")
    (service "NAVCalculationService")
  )
)`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	expr := ast.Root.Children[0]
	if expr.Value != "services.discover" {
		t.Errorf("Expected verb 'services.discover', got '%s'", expr.Value)
	}

	// Should have verb + 2 for.product blocks
	if len(expr.Children) != 3 {
		t.Fatalf("Expected 3 children (verb + 2 product blocks), got %d", len(expr.Children))
	}

	// Check first for.product block
	productBlock := expr.Children[1]
	if productBlock.Type != ExpressionNode {
		t.Errorf("Expected ExpressionNode for product block, got %v", productBlock.Type)
	}
	if productBlock.Children[0].Value != "for.product" {
		t.Errorf("Expected 'for.product', got '%s'", productBlock.Children[0].Value)
	}
	if productBlock.Children[1].Type != StringNode || productBlock.Children[1].Value != "CUSTODY" {
		t.Errorf("Expected CUSTODY product name")
	}
}

func TestParse_OnboardingResourcesPlan(t *testing.T) {
	dsl := `(resources.plan
  (resource.create "CustodyAccount"
    (owner "CustodyTech")
    (var (attr-id "8a5d1a77-e89b-12d3-a456-426614174000"))
  )
)`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	expr := ast.Root.Children[0]
	if expr.Value != "resources.plan" {
		t.Errorf("Expected verb 'resources.plan', got '%s'", expr.Value)
	}

	// Navigate to resource.create
	resourceCreate := expr.Children[1]
	if resourceCreate.Children[0].Value != "resource.create" {
		t.Errorf("Expected 'resource.create', got '%s'", resourceCreate.Children[0].Value)
	}

	// Check that we have the resource name
	if resourceCreate.Children[1].Type != StringNode {
		t.Errorf("Expected StringNode for resource name, got %v", resourceCreate.Children[1].Type)
	}
	if resourceCreate.Children[1].Value != "CustodyAccount" {
		t.Errorf("Expected 'CustodyAccount', got '%s'", resourceCreate.Children[1].Value)
	}
}

func TestParse_OnboardingValuesBinds(t *testing.T) {
	dsl := `(values.bind
  (bind (attr-id "123e4567-e89b-12d3-a456-426614174000") (value "CBU-1234"))
  (bind (attr-id "987fcdeb-51a2-43f7-8765-ba9876543210") (value "Aviva Investors Global Fund"))
)`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	expr := ast.Root.Children[0]
	if expr.Value != "values.bind" {
		t.Errorf("Expected verb 'values.bind', got '%s'", expr.Value)
	}

	// Should have verb + 2 bind expressions
	if len(expr.Children) != 3 {
		t.Fatalf("Expected 3 children (verb + 2 binds), got %d", len(expr.Children))
	}

	// Check first bind
	firstBind := expr.Children[1]
	if firstBind.Children[0].Value != "bind" {
		t.Errorf("Expected 'bind', got '%s'", firstBind.Children[0].Value)
	}
}

func TestParse_OnboardingCompleteWorkflow(t *testing.T) {
	// This is a REAL onboarding workflow - must parse correctly
	dsl := `(case.create
  (cbu.id "CBU-1234")
  (nature-purpose "UCITS equity fund domiciled in LU")
)

(products.add "CUSTODY" "FUND_ACCOUNTING")

(kyc.start
  (documents
    (document "CertificateOfIncorporation")
  )
  (jurisdictions
    (jurisdiction "LU")
  )
)

(services.discover
  (for.product "CUSTODY"
    (service "CustodyService")
  )
)

(resources.plan
  (resource.create "CustodyAccount"
    (owner "CustodyTech")
    (var (attr-id "uuid-123"))
  )
)

(values.bind
  (bind (attr-id "uuid-123") (value "CUST-ACC-001"))
)`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed for complete workflow: %v", err)
	}

	// Should have 6 top-level expressions
	expectedExpressions := 6
	if len(ast.Root.Children) != expectedExpressions {
		t.Fatalf("Expected %d top-level expressions, got %d", expectedExpressions, len(ast.Root.Children))
	}

	// Verify verbs in order
	expectedVerbs := []string{
		"case.create",
		"products.add",
		"kyc.start",
		"services.discover",
		"resources.plan",
		"values.bind",
	}

	for i, expected := range expectedVerbs {
		if ast.Root.Children[i].Value != expected {
			t.Errorf("Expression %d: expected verb '%s', got '%s'", i, expected, ast.Root.Children[i].Value)
		}
	}
}

func TestParse_OnboardingWithAttributes(t *testing.T) {
	dsl := `(resources.plan
  (resource.create "CustodyAccount"
    (var (attr-id "attr-1"))
    (var (attr-id "attr-2"))
    (var (attr-id "attr-3"))
  )
)`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	// Extract attribute IDs
	attrIDs := ast.ExtractAttributeIDs()
	if len(attrIDs) != 3 {
		t.Fatalf("Expected 3 attribute IDs, got %d", len(attrIDs))
	}

	expectedIDs := []string{"attr-1", "attr-2", "attr-3"}
	for i, expected := range expectedIDs {
		if attrIDs[i] != expected {
			t.Errorf("Attribute %d: expected '%s', got '%s'", i, expected, attrIDs[i])
		}
	}
}

func TestParse_OnboardingMultiProduct(t *testing.T) {
	dsl := `(case.create (cbu.id "CBU-1234"))

(products.add "CUSTODY")

(products.add "FUND_ACCOUNTING")

(products.add "TRANSFER_AGENCY")`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	// Should have 4 expressions
	if len(ast.Root.Children) != 4 {
		t.Fatalf("Expected 4 expressions, got %d", len(ast.Root.Children))
	}

	// Three should be products.add
	productCount := 0
	for _, expr := range ast.Root.Children {
		if expr.Value == "products.add" {
			productCount++
		}
	}
	if productCount != 3 {
		t.Errorf("Expected 3 products.add expressions, got %d", productCount)
	}
}

func TestParse_OnboardingNestedResources(t *testing.T) {
	dsl := `(resources.plan
  (resource.create "CustodyAccount"
    (owner "CustodyTech")
    (var (attr-id "custody-account-number"))
  )
  (resource.create "SettlementAccount"
    (owner "SettlementTech")
    (var (attr-id "settlement-account-number"))
  )
)`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	expr := ast.Root.Children[0]
	if expr.Value != "resources.plan" {
		t.Errorf("Expected verb 'resources.plan', got '%s'", expr.Value)
	}

	// Should have verb + 2 resource.create blocks
	if len(expr.Children) != 3 {
		t.Fatalf("Expected 3 children (verb + 2 resources), got %d", len(expr.Children))
	}

	// Verify both resource names
	resources := []string{"CustodyAccount", "SettlementAccount"}
	for i, expected := range resources {
		resourceExpr := expr.Children[i+1] // Skip verb
		if resourceExpr.Children[1].Value != expected {
			t.Errorf("Resource %d: expected '%s', got '%s'", i, expected, resourceExpr.Children[1].Value)
		}
	}
}

// =============================================================================
// Hedge Fund DSL Tests (Verify Domain-Agnostic)
// =============================================================================

func TestParse_HedgeFundInvestorStart(t *testing.T) {
	dsl := `(investor.start-opportunity
  (legal-name "Acme Capital LP")
  (type "CORPORATE")
  (domicile "CH")
)`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed for hedge fund DSL: %v", err)
	}

	expr := ast.Root.Children[0]
	if expr.Value != "investor.start-opportunity" {
		t.Errorf("Expected verb 'investor.start-opportunity', got '%s'", expr.Value)
	}

	// Should have verb + 3 parameter expressions
	if len(expr.Children) != 4 {
		t.Fatalf("Expected 4 children (verb + 3 params), got %d", len(expr.Children))
	}
}

func TestParse_HedgeFundKYCBegin(t *testing.T) {
	dsl := `(kyc.begin
  (investor "uuid-investor-123")
  (tier "STANDARD")
)`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	expr := ast.Root.Children[0]
	if expr.Value != "kyc.begin" {
		t.Errorf("Expected verb 'kyc.begin', got '%s'", expr.Value)
	}
}

func TestParse_HedgeFundSubscription(t *testing.T) {
	dsl := `(subscription.submit
  (investor "uuid-investor")
  (fund "uuid-fund")
  (class "uuid-class")
  (amount 1000000.00)
  (currency "USD")
)`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	expr := ast.Root.Children[0]
	if expr.Value != "subscription.submit" {
		t.Errorf("Expected verb 'subscription.submit', got '%s'", expr.Value)
	}

	// Check that numeric amount parsed correctly
	found := false
	for _, child := range expr.Children {
		if child.Type == ExpressionNode && len(child.Children) >= 2 {
			if child.Children[0].Value == "amount" && child.Children[1].Type == NumberNode {
				if child.Children[1].Value == "1000000.00" {
					found = true
				}
			}
		}
	}
	if !found {
		t.Error("Failed to parse numeric amount correctly")
	}
}

func TestParse_HedgeFundRedemption(t *testing.T) {
	dsl := `(redemption.request
  (investor "uuid-investor")
  (fund "uuid-fund")
  (shares 500.50)
)`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	expr := ast.Root.Children[0]
	if expr.Value != "redemption.request" {
		t.Errorf("Expected verb 'redemption.request', got '%s'", expr.Value)
	}
}

func TestParse_HedgeFundCompleteWorkflow(t *testing.T) {
	dsl := `(investor.start-opportunity
  (legal-name "Acme Capital LP")
  (type "CORPORATE")
)

(kyc.begin
  (investor "uuid-123")
  (tier "STANDARD")
)

(subscription.submit
  (investor "uuid-123")
  (fund "uuid-fund")
  (class "uuid-class")
  (amount 1000000.00)
  (currency "USD")
)

(register.issue
  (investor "uuid-123")
  (shares 1000.00)
)`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed for hedge fund workflow: %v", err)
	}

	// Should have 4 expressions
	if len(ast.Root.Children) != 4 {
		t.Fatalf("Expected 4 expressions, got %d", len(ast.Root.Children))
	}

	expectedVerbs := []string{
		"investor.start-opportunity",
		"kyc.begin",
		"subscription.submit",
		"register.issue",
	}

	for i, expected := range expectedVerbs {
		if ast.Root.Children[i].Value != expected {
			t.Errorf("Expression %d: expected '%s', got '%s'", i, expected, ast.Root.Children[i].Value)
		}
	}
}

// =============================================================================
// Cross-Domain Tests
// =============================================================================

func TestParse_MixedDomainDSL(t *testing.T) {
	// Mix onboarding and hedge fund DSL in one document
	dsl := `(case.create (cbu.id "CBU-1234"))

(investor.start-opportunity (legal-name "Acme Corp"))

(products.add "CUSTODY")

(subscription.submit (amount 100000.00))`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed for mixed domain DSL: %v", err)
	}

	if len(ast.Root.Children) != 4 {
		t.Fatalf("Expected 4 expressions, got %d", len(ast.Root.Children))
	}

	// Verify we have both onboarding and hedge fund verbs
	verbs := ast.ExtractVerbs()
	hasOnboarding := false
	hasHedgeFund := false

	for _, verb := range verbs {
		if strings.HasPrefix(verb, "case.") || strings.HasPrefix(verb, "products.") {
			hasOnboarding = true
		}
		if strings.HasPrefix(verb, "investor.") || strings.HasPrefix(verb, "subscription.") {
			hasHedgeFund = true
		}
	}

	if !hasOnboarding {
		t.Error("Expected onboarding verbs in mixed DSL")
	}
	if !hasHedgeFund {
		t.Error("Expected hedge fund verbs in mixed DSL")
	}
}

func TestParse_OnboardingCallsHedgeFund(t *testing.T) {
	// Onboarding orchestrates hedge fund operations
	dsl := `(case.create
  (cbu.id "CBU-1234")
  (nature-purpose "Hedge fund investor onboarding")
)

(investor.start-opportunity
  (legal-name "Pension Fund ABC")
  (type "CORPORATE")
)

(kyc.start
  (documents (document "CertificateOfIncorporation"))
)

(kyc.begin
  (investor "uuid-investor")
  (tier "ENHANCED")
)`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed for orchestrated DSL: %v", err)
	}

	if len(ast.Root.Children) != 4 {
		t.Fatalf("Expected 4 expressions, got %d", len(ast.Root.Children))
	}

	// Should have both kyc.start (onboarding) and kyc.begin (hedge fund)
	verbs := ast.ExtractVerbs()
	hasKYCStart := false
	hasKYCBegin := false

	for _, verb := range verbs {
		if verb == "kyc.start" {
			hasKYCStart = true
		}
		if verb == "kyc.begin" {
			hasKYCBegin = true
		}
	}

	if !hasKYCStart || !hasKYCBegin {
		t.Error("Expected both kyc.start and kyc.begin verbs")
	}
}

func TestParse_LargeDSLDocument(t *testing.T) {
	// Build a large DSL with 20+ expressions
	var sb strings.Builder
	for i := 0; i < 20; i++ {
		sb.WriteString(fmt.Sprintf("(case.update (status \"Step-%d\"))\n\n", i))
	}

	ast, err := Parse(sb.String())
	if err != nil {
		t.Fatalf("Parse failed for large DSL: %v", err)
	}

	if len(ast.Root.Children) != 20 {
		t.Fatalf("Expected 20 expressions, got %d", len(ast.Root.Children))
	}
}

// =============================================================================
// AST Operation Tests
// =============================================================================

func TestAST_VerbExtraction(t *testing.T) {
	dsl := `(case.create)
(products.add "CUSTODY")
(kyc.start)
(resources.plan)
(values.bind)`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	verbs := ast.ExtractVerbs()
	if len(verbs) != 5 {
		t.Fatalf("Expected 5 verbs, got %d: %v", len(verbs), verbs)
	}

	expectedVerbs := map[string]bool{
		"case.create":    true,
		"products.add":   true,
		"kyc.start":      true,
		"resources.plan": true,
		"values.bind":    true,
	}

	for _, verb := range verbs {
		if !expectedVerbs[verb] {
			t.Errorf("Unexpected verb: %s", verb)
		}
	}
}

func TestAST_AttributeIDExtraction(t *testing.T) {
	dsl := `(resources.plan
  (resource.create "Account"
    (var (attr-id "uuid-1"))
    (var (attr-id "uuid-2"))
  )
)

(values.bind
  (bind (attr-id "uuid-3") (value "test"))
)`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	attrIDs := ast.ExtractAttributeIDs()
	if len(attrIDs) != 3 {
		t.Fatalf("Expected 3 attribute IDs, got %d: %v", len(attrIDs), attrIDs)
	}

	expected := []string{"uuid-1", "uuid-2", "uuid-3"}
	for i, expectedID := range expected {
		if attrIDs[i] != expectedID {
			t.Errorf("Attribute %d: expected '%s', got '%s'", i, expectedID, attrIDs[i])
		}
	}
}

// =============================================================================
// Special Character and Edge Case Tests
// =============================================================================

func TestParse_StringWithEscapes(t *testing.T) {
	dsl := `(case.create (description "Line 1\nLine 2\tTabbed"))`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	expr := ast.Root.Children[0]
	descExpr := expr.Children[1]
	descValue := descExpr.Children[1]

	if descValue.Type != StringNode {
		t.Errorf("Expected StringNode, got %v", descValue.Type)
	}

	if !strings.Contains(descValue.Value, "\n") {
		t.Error("Expected newline escape to be processed")
	}
	if !strings.Contains(descValue.Value, "\t") {
		t.Error("Expected tab escape to be processed")
	}
}

func TestParse_NumberTypes(t *testing.T) {
	tests := []struct {
		name     string
		dsl      string
		expected string
	}{
		{"Integer", "(amount 100)", "100"},
		{"Decimal", "(amount 100.50)", "100.50"},
		{"Negative", "(amount -50)", "-50"},
		{"Negative decimal", "(amount -123.45)", "-123.45"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			ast, err := Parse(tt.dsl)
			if err != nil {
				t.Fatalf("Parse failed: %v", err)
			}

			expr := ast.Root.Children[0]
			if len(expr.Children) < 2 {
				t.Fatal("Expected at least 2 children")
			}

			// Find the number node
			var numNode *Node
			for _, child := range expr.Children {
				if child.Type == NumberNode {
					numNode = child
					break
				}
			}

			if numNode == nil {
				t.Fatal("Expected to find NumberNode")
			}

			if numNode.Value != tt.expected {
				t.Errorf("Expected number '%s', got '%s'", tt.expected, numNode.Value)
			}
		})
	}
}

func TestParse_BooleanValues(t *testing.T) {
	dsl := `(config (enabled true) (readonly false))`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	expr := ast.Root.Children[0]

	// Check for boolean nodes
	boolCount := 0
	for _, child := range expr.Children {
		if child.Type == ExpressionNode {
			for _, subChild := range child.Children {
				if subChild.Type == BooleanNode {
					boolCount++
				}
			}
		}
	}

	if boolCount != 2 {
		t.Errorf("Expected 2 boolean values, got %d", boolCount)
	}
}

func TestParse_CommentsIgnored(t *testing.T) {
	dsl := `; This is a comment
(case.create (cbu.id "CBU-1234"))

; Another comment
(products.add "CUSTODY") ; inline comment`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	// Comments should be ignored, only 2 expressions
	if len(ast.Root.Children) != 2 {
		t.Fatalf("Expected 2 expressions (comments ignored), got %d", len(ast.Root.Children))
	}
}

func TestParse_WhitespaceVariations(t *testing.T) {
	tests := []struct {
		name string
		dsl  string
	}{
		{"Extra spaces", "(case.create    (cbu.id    \"CBU-1234\")   )"},
		{"Tabs", "(case.create\t(cbu.id\t\"CBU-1234\"))"},
		{"Multiple newlines", "(case.create\n\n\n(cbu.id \"CBU-1234\"))"},
		{"Mixed whitespace", "(case.create \t\n (cbu.id  \t \"CBU-1234\"))"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			ast, err := Parse(tt.dsl)
			if err != nil {
				t.Fatalf("Parse failed for %s: %v", tt.name, err)
			}

			if len(ast.Root.Children) != 1 {
				t.Errorf("Expected 1 expression, got %d", len(ast.Root.Children))
			}

			if ast.Root.Children[0].Value != "case.create" {
				t.Errorf("Expected 'case.create', got '%s'", ast.Root.Children[0].Value)
			}
		})
	}
}

func TestParse_IdentifiersWithSpecialChars(t *testing.T) {
	// Test identifiers with dots, hyphens, underscores
	dsl := `(case.create
  (cbu.id "test")
  (nature-purpose "test")
  (internal_ref "test")
  (for.product "test")
)`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	expr := ast.Root.Children[0]

	// Should have verb + 4 nested expressions
	if len(expr.Children) != 5 {
		t.Fatalf("Expected 5 children (verb + 4 params), got %d", len(expr.Children))
	}

	expectedIdentifiers := []string{"cbu.id", "nature-purpose", "internal_ref", "for.product"}
	for i, expected := range expectedIdentifiers {
		paramExpr := expr.Children[i+1] // Skip verb
		if paramExpr.Children[0].Value != expected {
			t.Errorf("Parameter %d: expected '%s', got '%s'", i, expected, paramExpr.Children[0].Value)
		}
	}
}

func TestValidatePlaceholders_WithPlaceholders(t *testing.T) {
	dsl := `(investor.start-opportunity
  (legal-name <investor_name>)
  (investor-id <investor_id>)
)`

	err := ValidatePlaceholders(dsl)
	if err == nil {
		t.Error("Expected error for unresolved placeholders")
	}

	if !strings.Contains(err.Error(), "2") {
		t.Error("Expected error to mention 2 placeholders")
	}
}

func TestValidatePlaceholders_WithoutPlaceholders(t *testing.T) {
	dsl := `(investor.start-opportunity
  (legal-name "Acme Corp")
  (investor-id "uuid-123")
)`

	err := ValidatePlaceholders(dsl)
	if err != nil {
		t.Errorf("Expected no error, got: %v", err)
	}
}

func TestParse_ErrorLineNumbers(t *testing.T) {
	dsl := `(case.create (cbu.id "CBU-1234"))

(products.add "CUSTODY"

(kyc.start)` // Missing closing paren on products.add

	_, err := Parse(dsl)
	if err == nil {
		t.Fatal("Expected parse error")
	}

	// Error should mention line number
	if !strings.Contains(err.Error(), "line") {
		t.Error("Expected error to include line number information")
	}
}

// =============================================================================
// Performance Benchmark Tests
// =============================================================================

func BenchmarkParse_SimpleExpression(b *testing.B) {
	dsl := `(case.create (cbu.id "CBU-1234"))`

	for i := 0; i < b.N; i++ {
		_, _ = Parse(dsl)
	}
}

func BenchmarkParse_OnboardingWorkflow(b *testing.B) {
	dsl := `(case.create (cbu.id "CBU-1234") (nature-purpose "UCITS fund"))
(products.add "CUSTODY" "FUND_ACCOUNTING")
(kyc.start (documents (document "CertificateOfIncorporation")))
(services.discover (for.product "CUSTODY" (service "CustodyService")))
(resources.plan (resource.create "CustodyAccount" (owner "CustodyTech")))
(values.bind (bind (attr-id "uuid-123") (value "CUST-001")))`

	for i := 0; i < b.N; i++ {
		_, _ = Parse(dsl)
	}
}

func BenchmarkParse_LargeDSL(b *testing.B) {
	// Build a large DSL with 100 expressions
	var sb strings.Builder
	for i := 0; i < 100; i++ {
		sb.WriteString("(case.update (status \"Step-")
		sb.WriteString(string(rune(i)))
		sb.WriteString("\"))\n")
	}
	dsl := sb.String()

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_, _ = Parse(dsl)
	}
}

func BenchmarkAST_ExtractVerbs(b *testing.B) {
	dsl := `(case.create) (products.add) (kyc.start) (services.discover) (resources.plan) (values.bind)`
	ast, _ := Parse(dsl)

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_ = ast.ExtractVerbs()
	}
}

func BenchmarkAST_ExtractAttributeIDs(b *testing.B) {
	dsl := `(resources.plan
  (resource.create "A" (var (attr-id "uuid-1")))
  (resource.create "B" (var (attr-id "uuid-2")))
  (resource.create "C" (var (attr-id "uuid-3")))
)`
	ast, _ := Parse(dsl)

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_ = ast.ExtractAttributeIDs()
	}
}

// =============================================================================
// Debug Test - Understanding AST Structure
// =============================================================================

func TestDebug_AttributeIDStructure(t *testing.T) {
	dsl := `(resources.plan
  (resource.create "Account"
    (var (attr-id "uuid-1"))
  )
)`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	t.Logf("AST Structure:\n%s", ast.String())

	ids := ast.ExtractAttributeIDs()
	t.Logf("Extracted %d attribute IDs: %v", len(ids), ids)

	if len(ids) == 0 {
		t.Error("Failed to extract attribute IDs - examining structure manually")
		// Manual traversal to debug
		ast.traverse(ast.Root, func(node *Node) {
			if node.Type == IdentifierNode {
				t.Logf("Found identifier: %s", node.Value)
			}
			if node.Type == ExpressionNode && len(node.Children) > 0 {
				t.Logf("Expression with first child: %v = %s", node.Children[0].Type, node.Children[0].Value)
			}
		})
	}
}

// =============================================================================
// AttributeID-as-Type Pattern Tests - @attr{uuid:name} syntax
// =============================================================================

func TestParse_AttributeUUIDOnly(t *testing.T) {
	dsl := `(case.create @attr{8a5d1a77-e4b3-4c2d-9f1e-7a8b9c0d1e2f})`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	// Verify AST structure
	if ast.Root.Type != RootNode {
		t.Errorf("Expected RootNode, got %v", ast.Root.Type)
	}

	if len(ast.Root.Children) != 1 {
		t.Errorf("Expected 1 expression, got %d", len(ast.Root.Children))
	}

	expr := ast.Root.Children[0]
	if expr.Type != ExpressionNode {
		t.Errorf("Expected ExpressionNode, got %v", expr.Type)
	}

	if len(expr.Children) != 2 {
		t.Errorf("Expected 2 children (verb + attribute), got %d", len(expr.Children))
	}

	// Check attribute node
	attrNode := expr.Children[1]
	if attrNode.Type != AttributeNode {
		t.Errorf("Expected AttributeNode, got %v", attrNode.Type)
	}

	expectedID := "8a5d1a77-e4b3-4c2d-9f1e-7a8b9c0d1e2f"
	if attrNode.AttributeID != expectedID {
		t.Errorf("Expected AttributeID %s, got %s", expectedID, attrNode.AttributeID)
	}

	if attrNode.Name != "" {
		t.Errorf("Expected empty Name, got %s", attrNode.Name)
	}

	expectedValue := "@attr{8a5d1a77-e4b3-4c2d-9f1e-7a8b9c0d1e2f}"
	if attrNode.Value != expectedValue {
		t.Errorf("Expected Value %s, got %s", expectedValue, attrNode.Value)
	}
}

func TestParse_AttributeUUIDWithName(t *testing.T) {
	dsl := `(kyc.start @attr{8a5d1a77-e4b3-4c2d-9f1e-7a8b9c0d1e2f:custody.account_number})`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	expr := ast.Root.Children[0]
	attrNode := expr.Children[1]

	if attrNode.Type != AttributeNode {
		t.Errorf("Expected AttributeNode, got %v", attrNode.Type)
	}

	expectedID := "8a5d1a77-e4b3-4c2d-9f1e-7a8b9c0d1e2f"
	if attrNode.AttributeID != expectedID {
		t.Errorf("Expected AttributeID %s, got %s", expectedID, attrNode.AttributeID)
	}

	expectedName := "custody.account_number"
	if attrNode.Name != expectedName {
		t.Errorf("Expected Name %s, got %s", expectedName, attrNode.Name)
	}

	expectedValue := "@attr{8a5d1a77-e4b3-4c2d-9f1e-7a8b9c0d1e2f:custody.account_number}"
	if attrNode.Value != expectedValue {
		t.Errorf("Expected Value %s, got %s", expectedValue, attrNode.Value)
	}
}

func TestParse_MultipleAttributes(t *testing.T) {
	dsl := `(investor.start-opportunity
  @attr{a1b2c3d4-e5f6-7a8b-9c0d-1e2f3a4b5c6d:investor.legal_name}
  @attr{e5f6a7b8-c9d0-e1f2-a3b4-c5d6e7f8a9b0:investor.type}
  @attr{c9d0e1f2-a3b4-c5d6-e7f8-a9b0c1d2e3f4}
)`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	expr := ast.Root.Children[0]
	if len(expr.Children) != 4 { // verb + 3 attributes
		t.Errorf("Expected 4 children, got %d", len(expr.Children))
	}

	// Check first attribute (with name)
	attr1 := expr.Children[1]
	if attr1.Type != AttributeNode {
		t.Errorf("Expected AttributeNode, got %v", attr1.Type)
	}
	if attr1.AttributeID != "a1b2c3d4-e5f6-7a8b-9c0d-1e2f3a4b5c6d" {
		t.Errorf("Wrong AttributeID for first attribute")
	}
	if attr1.Name != "investor.legal_name" {
		t.Errorf("Wrong Name for first attribute")
	}

	// Check third attribute (UUID only)
	attr3 := expr.Children[3]
	if attr3.Type != AttributeNode {
		t.Errorf("Expected AttributeNode, got %v", attr3.Type)
	}
	if attr3.Name != "" {
		t.Errorf("Expected empty Name for third attribute, got %s", attr3.Name)
	}
}

func TestAST_ExtractAttributes(t *testing.T) {
	dsl := `(investor.start-opportunity
  @attr{a1b2c3d4-e5f6-7a8b-9c0d-1e2f3a4b5c6d:investor.legal_name}
  @attr{e5f6a7b8-c9d0-e1f2-a3b4-c5d6e7f8a9b0:investor.type}
  @attr{c9d0e1f2-a3b4-c5d6-e7f8-a9b0c1d2e3f4}
)`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	attributes := ast.ExtractAttributes()
	if len(attributes) != 3 {
		t.Errorf("Expected 3 attributes, got %d", len(attributes))
	}

	// Check first attribute
	attr0 := attributes[0]
	if attr0.ID != "a1b2c3d4-e5f6-7a8b-9c0d-1e2f3a4b5c6d" {
		t.Errorf("Wrong ID for first attribute")
	}
	if attr0.Name != "investor.legal_name" {
		t.Errorf("Wrong Name for first attribute")
	}

	// Check third attribute (UUID only)
	attr2 := attributes[2]
	if attr2.ID != "c9d0e1f2-a3b4-c5d6-e7f8-a9b0c1d2e3f4" {
		t.Errorf("Wrong ID for third attribute")
	}
	if attr2.Name != "" {
		t.Errorf("Expected empty Name for third attribute, got %s", attr2.Name)
	}
}

func TestAST_ExtractAttributeIDs_NewSyntax(t *testing.T) {
	dsl := `(resources.plan
  (resource.create "CustodyAccount" @attr{8a5d1a77-e4b3-4c2d-9f1e-7a8b9c0d1e2f:custody.account_number})
  (resource.create "AccountingSystem" @attr{2c3d4e5f-6a7b-8c9d-0e1f-2a3b4c5d6e7f})
)`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	attrIDs := ast.ExtractAttributeIDs()
	expected := []string{
		"8a5d1a77-e4b3-4c2d-9f1e-7a8b9c0d1e2f",
		"2c3d4e5f-6a7b-8c9d-0e1f-2a3b4c5d6e7f",
	}

	if len(attrIDs) != len(expected) {
		t.Errorf("Expected %d attribute IDs, got %d", len(expected), len(attrIDs))
	}

	for i, expectedID := range expected {
		if i >= len(attrIDs) || attrIDs[i] != expectedID {
			t.Errorf("Expected attribute ID %s at position %d, got %v", expectedID, i, attrIDs)
		}
	}
}

func TestAST_ExtractAttributeIDs_MixedSyntax(t *testing.T) {
	dsl := `(resources.plan
  (resource.create "CustodyAccount"
    (var (attr-id "legacy-uuid-1"))
    @attr{new-uuid-1:account.number}
  )
  (values.bind
    (bind (attr-id "legacy-uuid-2") (value "CUST-001"))
    @attr{new-uuid-2}
  )
)`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	attrIDs := ast.ExtractAttributeIDs()
	expectedCount := 4 // 2 legacy + 2 new syntax
	if len(attrIDs) != expectedCount {
		t.Errorf("Expected %d attribute IDs, got %d: %v", expectedCount, len(attrIDs), attrIDs)
	}

	// Should contain both legacy and new syntax IDs
	hasLegacy1 := false
	hasLegacy2 := false
	hasNew1 := false
	hasNew2 := false

	for _, id := range attrIDs {
		switch id {
		case "legacy-uuid-1":
			hasLegacy1 = true
		case "legacy-uuid-2":
			hasLegacy2 = true
		case "new-uuid-1":
			hasNew1 = true
		case "new-uuid-2":
			hasNew2 = true
		}
	}

	if !hasLegacy1 || !hasLegacy2 || !hasNew1 || !hasNew2 {
		t.Errorf("Missing expected attribute IDs. Got: %v", attrIDs)
	}
}

func TestParse_AttributeMalformed(t *testing.T) {
	testCases := []struct {
		name string
		dsl  string
	}{
		{"Missing attr", `(case.create @{uuid})`},
		{"Missing opening brace", `(case.create @attr uuid)`},
		{"Missing closing brace", `(case.create @attr{uuid)`},
		{"Empty attribute", `(case.create @attr{})`},
		{"Colon but no name", `(case.create @attr{uuid:})`},
	}

	for _, tc := range testCases {
		t.Run(tc.name, func(t *testing.T) {
			_, err := Parse(tc.dsl)
			if err == nil {
				t.Errorf("Expected parse error for malformed DSL: %s", tc.dsl)
			}
		})
	}
}

func TestParse_AttributeComplexWorkflow(t *testing.T) {
	dsl := `(case.create
  (cbu.id "CBU-2024-001")
  (nature-purpose "UCITS equity fund")
)

(kyc.start
  @attr{a1b2c3d4-e5f6-7a8b-9c0d-1e2f3a4b5c6d:investor.legal_name}
  @attr{e5f6a7b8-c9d0-e1f2-a3b4-c5d6e7f8a9b0:investor.type}
)

(resources.plan
  (resource.create "CustodyAccount"
    (owner "CustodyTech")
    @attr{8a5d1a77-e4b3-4c2d-9f1e-7a8b9c0d1e2f:custody.account_number}
  )
)`

	ast, err := Parse(dsl)
	if err != nil {
		t.Fatalf("Parse failed: %v", err)
	}

	// Should have 3 top-level expressions
	if len(ast.Root.Children) != 3 {
		t.Errorf("Expected 3 expressions, got %d", len(ast.Root.Children))
	}

	// Extract attributes
	attributes := ast.ExtractAttributes()
	if len(attributes) != 3 {
		t.Errorf("Expected 3 attributes, got %d", len(attributes))
	}

	// Verify attribute extraction
	attrIDs := ast.ExtractAttributeIDs()
	expectedIDs := []string{
		"a1b2c3d4-e5f6-7a8b-9c0d-1e2f3a4b5c6d",
		"e5f6a7b8-c9d0-e1f2-a3b4-c5d6e7f8a9b0",
		"8a5d1a77-e4b3-4c2d-9f1e-7a8b9c0d1e2f",
	}

	if len(attrIDs) != len(expectedIDs) {
		t.Errorf("Expected %d attribute IDs, got %d", len(expectedIDs), len(attrIDs))
	}

	for _, expectedID := range expectedIDs {
		found := false
		for _, actualID := range attrIDs {
			if actualID == expectedID {
				found = true
				break
			}
		}
		if !found {
			t.Errorf("Missing expected attribute ID: %s", expectedID)
		}
	}
}
