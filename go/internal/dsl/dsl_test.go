package dsl

import (
	"strings"
	"testing"

	"dsl-ob-poc/internal/dictionary"
	"dsl-ob-poc/internal/store"
)

func TestCreateCase(t *testing.T) {
	cbuID := "CBU-1234"
	naturePurpose := "UCITS equity fund"

	result := CreateCase(cbuID, naturePurpose)

	if !strings.Contains(result, "(case.create") {
		t.Errorf("Expected DSL to contain '(case.create', got: %s", result)
	}

	if !strings.Contains(result, cbuID) {
		t.Errorf("Expected DSL to contain CBU ID '%s', got: %s", cbuID, result)
	}

	if !strings.Contains(result, naturePurpose) {
		t.Errorf("Expected DSL to contain nature-purpose '%s', got: %s", naturePurpose, result)
	}

	// Verify it's valid S-expression format
	if !strings.HasPrefix(result, "(") || !strings.HasSuffix(result, ")") {
		t.Errorf("Expected DSL to be wrapped in parentheses, got: %s", result)
	}
}

func TestAddProducts(t *testing.T) {
	currentDSL := `(case.create
  (cbu.id "CBU-1234")
  (nature-purpose "Test fund")
)`

	products := []*store.Product{
		{ProductID: "p1", Name: "CUSTODY", Description: "Custody services"},
		{ProductID: "p2", Name: "FUND_ACCOUNTING", Description: "Fund accounting"},
	}

	result, err := AddProducts(currentDSL, products)
	if err != nil {
		t.Fatalf("AddProducts failed: %v", err)
	}

	if !strings.Contains(result, "(products.add") {
		t.Errorf("Expected DSL to contain '(products.add', got: %s", result)
	}

	if !strings.Contains(result, "CUSTODY") {
		t.Errorf("Expected DSL to contain 'CUSTODY', got: %s", result)
	}

	if !strings.Contains(result, "FUND_ACCOUNTING") {
		t.Errorf("Expected DSL to contain 'FUND_ACCOUNTING', got: %s", result)
	}

	// Original DSL should still be present
	if !strings.Contains(result, "(case.create") {
		t.Errorf("Expected DSL to preserve original case.create block, got: %s", result)
	}
}

func TestAddProductsEmpty(t *testing.T) {
	currentDSL := "(case.create)"
	var products []*store.Product

	result, err := AddProducts(currentDSL, products)
	if err != nil {
		t.Fatalf("AddProducts with empty list should not error: %v", err)
	}

	if result != currentDSL {
		t.Errorf("Expected DSL unchanged with empty products, got: %s", result)
	}
}

func TestParseProductNames(t *testing.T) {
	dsl := `(case.create
  (cbu.id "CBU-1234")
)

(products.add "CUSTODY" "FUND_ACCOUNTING" "TRANSFER_AGENCY")`

	names, err := ParseProductNames(dsl)
	if err != nil {
		t.Fatalf("ParseProductNames failed: %v", err)
	}

	expectedCount := 3
	if len(names) != expectedCount {
		t.Errorf("Expected %d product names, got %d: %v", expectedCount, len(names), names)
	}

	expectedNames := map[string]bool{
		"CUSTODY":         true,
		"FUND_ACCOUNTING": true,
		"TRANSFER_AGENCY": true,
	}

	for _, name := range names {
		if !expectedNames[name] {
			t.Errorf("Unexpected product name: %s", name)
		}
	}
}

func TestParseProductNamesNoBlock(t *testing.T) {
	dsl := "(case.create)"

	_, err := ParseProductNames(dsl)
	if err == nil {
		t.Error("Expected error when no products.add block found")
	}
}

func TestAddDiscoveredServices(t *testing.T) {
	currentDSL := `(case.create
  (cbu.id "CBU-1234")
)

(products.add "CUSTODY")`

	plan := ServiceDiscoveryPlan{
		ProductServices: map[string][]store.Service{
			"CUSTODY": {
				{ServiceID: "s1", Name: "CustodyService", Description: "Custody"},
				{ServiceID: "s2", Name: "SettlementService", Description: "Settlement"},
			},
		},
	}

	result, err := AddDiscoveredServices(currentDSL, plan)
	if err != nil {
		t.Fatalf("AddDiscoveredServices failed: %v", err)
	}

	if !strings.Contains(result, "(services.discover") {
		t.Errorf("Expected DSL to contain '(services.discover', got: %s", result)
	}

	if !strings.Contains(result, "(for.product \"CUSTODY\"") {
		t.Errorf("Expected DSL to contain product block, got: %s", result)
	}

	if !strings.Contains(result, "CustodyService") {
		t.Errorf("Expected DSL to contain 'CustodyService', got: %s", result)
	}

	if !strings.Contains(result, "SettlementService") {
		t.Errorf("Expected DSL to contain 'SettlementService', got: %s", result)
	}
}

func TestParseServiceNames(t *testing.T) {
	dsl := `(services.discover
  (for.product "CUSTODY"
    (service "CustodyService")
    (service "SettlementService")
  )
  (for.product "FUND_ACCOUNTING"
    (service "FundAccountingService")
  )
)`

	names, err := ParseServiceNames(dsl)
	if err != nil {
		t.Fatalf("ParseServiceNames failed: %v", err)
	}

	if len(names) != 3 {
		t.Errorf("Expected 3 service names, got %d: %v", len(names), names)
	}

	expectedNames := map[string]bool{
		"CustodyService":        true,
		"SettlementService":     true,
		"FundAccountingService": true,
	}

	for _, name := range names {
		if !expectedNames[name] {
			t.Errorf("Unexpected service name: %s", name)
		}
	}
}

func TestParseServiceNamesNoBlock(t *testing.T) {
	dsl := "(case.create)"

	_, err := ParseServiceNames(dsl)
	if err == nil {
		t.Error("Expected error when no service blocks found")
	}
}

func TestAddDiscoveredResources(t *testing.T) {
	currentDSL := `(case.create)

(services.discover
  (for.product "CUSTODY"
    (service "CustodyService")
  )
)`

	plan := ResourceDiscoveryPlan{
		ServiceResources: map[string][]store.ProdResource{
			"CustodyService": {
				{
					ResourceID:      "r1",
					Name:            "CustodyAccount",
					Owner:           "CustodyTech",
					DictionaryGroup: "CustodyAccount",
				},
			},
		},
		ResourceAttributes: map[string][]dictionary.Attribute{
			"CustodyAccount": {
				{
					AttributeID:     "a1",
					Name:            "custody.account_number",
					LongDescription: "Custody account identifier",
					GroupID:         "CustodyAccount",
					Mask:            "string",
					Domain:          "Custody",
				},
			},
		},
	}

	result, err := AddDiscoveredResources(currentDSL, plan)
	if err != nil {
		t.Fatalf("AddDiscoveredResources failed: %v", err)
	}

	if !strings.Contains(result, "(resources.plan") {
		t.Errorf("Expected DSL to contain '(resources.plan', got: %s", result)
	}

	if !strings.Contains(result, "(resource.create \"CustodyAccount\"") {
		t.Errorf("Expected DSL to contain resource block, got: %s", result)
	}

	if !strings.Contains(result, "(owner \"CustodyTech\")") {
		t.Errorf("Expected DSL to contain owner, got: %s", result)
	}

	if !strings.Contains(result, "(var (attr-id \"a1\"))") {
		t.Errorf("Expected DSL to contain attribute variable, got: %s", result)
	}
}

func TestAddDiscoveredResourcesMultiple(t *testing.T) {
	currentDSL := "(case.create)"

	plan := ResourceDiscoveryPlan{
		ServiceResources: map[string][]store.ProdResource{
			"CustodyService": {
				{ResourceID: "r1", Name: "CustodyAccount", Owner: "CustodyTech", DictionaryGroup: "CustodyAccount"},
			},
			"AccountingService": {
				{ResourceID: "r2", Name: "AccountingRecord", Owner: "AcctTech", DictionaryGroup: "FundAccounting"},
			},
		},
		ResourceAttributes: map[string][]dictionary.Attribute{
			"CustodyAccount": {{AttributeID: "a1", Name: "custody.account_number", GroupID: "CustodyAccount"}},
			"FundAccounting": {{AttributeID: "a2", Name: "accounting.nav_value", GroupID: "FundAccounting"}},
		},
	}

	result, err := AddDiscoveredResources(currentDSL, plan)
	if err != nil {
		t.Fatalf("AddDiscoveredResources failed: %v", err)
	}

	// Should contain both resources
	if !strings.Contains(result, "CustodyAccount") {
		t.Errorf("Expected DSL to contain 'CustodyAccount', got: %s", result)
	}

	if !strings.Contains(result, "AccountingRecord") {
		t.Errorf("Expected DSL to contain 'AccountingRecord', got: %s", result)
	}

	if !strings.Contains(result, "(var (attr-id \"a1\"))") {
		t.Errorf("Expected DSL to contain custody account attribute variable, got: %s", result)
	}

	if !strings.Contains(result, "(var (attr-id \"a2\"))") {
		t.Errorf("Expected DSL to contain accounting attribute variable, got: %s", result)
	}
}

// --- Tests for State 6: Populate Attributes ---

func TestVarByAttrID(t *testing.T) {
	name := "test_var"
	id := "123e4567-e89b-12d3-a456-426614174000"
	result := VarByAttrID(name, id)
	expected := `(var (name "test_var") (id "123e4567-e89b-12d3-a456-426614174000"))`

	if result != expected {
		t.Errorf("Expected %q, got %q", expected, result)
	}
}

func TestExtractVarAttrIDs(t *testing.T) {
	dsl := `(case.create
  (var (attr-id "123e4567-e89b-12d3-a456-426614174000"))
  (var (attr-id "987fcdeb-51a2-43f7-8765-ba9876543210"))
)`

	ids := ExtractVarAttrIDs(dsl)

	if len(ids) != 2 {
		t.Fatalf("Expected 2 attribute IDs, got %d", len(ids))
	}

	expected1 := "123e4567-e89b-12d3-a456-426614174000"
	expected2 := "987fcdeb-51a2-43f7-8765-ba9876543210"

	if ids[0] != expected1 {
		t.Errorf("Expected first ID to be %q, got %q", expected1, ids[0])
	}

	if ids[1] != expected2 {
		t.Errorf("Expected second ID to be %q, got %q", expected2, ids[1])
	}
}

func TestNormalizeVars(t *testing.T) {
	dsl := `(case.create
  (VAR_onboard.cbu_id)
  (VAR_entity.legal_name)
  (VAR_unknown)
)`

	// Mock resolver that maps attribute names to dictionary attributes
	resolver := func(sym string) (attr *dictionary.Attribute, ok bool) {
		switch sym {
		case "onboard.cbu_id":
			return &dictionary.Attribute{
				AttributeID: "123e4567-e89b-12d3-a456-426614174000",
				Name:        "onboard.cbu_id",
			}, true
		case "entity.legal_name":
			return &dictionary.Attribute{
				AttributeID: "987fcdeb-51a2-43f7-8765-ba9876543210",
				Name:        "entity.legal_name",
			}, true
		default:
			return nil, false
		}
	}

	result := NormalizeVars(dsl, resolver)

	// Should convert known symbols to canonical form, leave unknown unchanged
	if !strings.Contains(result, `(var (attr-id "123e4567-e89b-12d3-a456-426614174000"))`) {
		t.Errorf("Expected DSL to contain normalized CBU variable, got: %s", result)
	}

	if !strings.Contains(result, `(var (attr-id "987fcdeb-51a2-43f7-8765-ba9876543210"))`) {
		t.Errorf("Expected DSL to contain normalized legal name variable, got: %s", result)
	}

	if !strings.Contains(result, "(VAR_unknown)") {
		t.Errorf("Expected DSL to preserve unknown variable, got: %s", result)
	}
}

func TestParseAttributeReferences(t *testing.T) {
	dsl := `(resources.plan
  (resource.create "CustodyAccount"
    (var (attr-id "123e4567-e89b-12d3-a456-426614174000"))
    (var (attr-id "987fcdeb-51a2-43f7-8765-ba9876543210"))
  )
)`

	refs, err := ParseAttributeReferences(dsl)
	if err != nil {
		t.Fatalf("ParseAttributeReferences failed: %v", err)
	}

	if len(refs) != 2 {
		t.Fatalf("Expected 2 attribute references, got %d", len(refs))
	}

	// Check first reference
	if refs[0].AttributeID != "123e4567-e89b-12d3-a456-426614174000" {
		t.Errorf("Expected first AttributeID to be '123e4567-e89b-12d3-a456-426614174000', got '%s'", refs[0].AttributeID)
	}

	// Check second reference
	if refs[1].AttributeID != "987fcdeb-51a2-43f7-8765-ba9876543210" {
		t.Errorf("Expected second AttributeID to be '987fcdeb-51a2-43f7-8765-ba9876543210', got '%s'", refs[1].AttributeID)
	}
}

func TestRenderBindings(t *testing.T) {
	assignments := map[string]string{
		"123e4567-e89b-12d3-a456-426614174000": `"CBU-1234"`,
		"987fcdeb-51a2-43f7-8765-ba9876543210": `"Aviva Investors Global Fund"`,
	}

	result := RenderBindings(assignments)

	if !strings.Contains(result, "(values.bind") {
		t.Errorf("Expected result to contain '(values.bind', got: %s", result)
	}

	if !strings.Contains(result, `(bind (attr-id "123e4567-e89b-12d3-a456-426614174000") (value "CBU-1234"))`) {
		t.Errorf("Expected result to contain CBU binding, got: %s", result)
	}

	if !strings.Contains(result, `(bind (attr-id "987fcdeb-51a2-43f7-8765-ba9876543210") (value "Aviva Investors Global Fund"))`) {
		t.Errorf("Expected result to contain legal name binding, got: %s", result)
	}
}
