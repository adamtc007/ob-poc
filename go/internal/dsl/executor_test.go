package dsl

import (
	"encoding/json"
	"strings"
	"testing"
)

func TestDSLExecutorBasicParsing(t *testing.T) {
	executor := NewDSLExecutor("CBU-1234")

	t.Run("Parse Simple Case Create", func(t *testing.T) {
		dsl := `(case.create (cbu.id "CBU-1234") (nature-purpose "UCITS equity fund"))`

		sexpr, err := executor.ParseSExpression(dsl)
		if err != nil {
			t.Fatalf("Failed to parse S-expression: %v", err)
		}

		if sexpr.Operator != "case.create" {
			t.Errorf("Expected operator 'case.create', got '%s'", sexpr.Operator)
		}

		if len(sexpr.Args) != 2 {
			t.Errorf("Expected 2 arguments, got %d", len(sexpr.Args))
		}
	})

	t.Run("Parse Nested Products Add", func(t *testing.T) {
		dsl := `(products.add "CUSTODY" "FUND_ACCOUNTING" "TRANSFER_AGENT")`

		sexpr, err := executor.ParseSExpression(dsl)
		if err != nil {
			t.Fatalf("Failed to parse S-expression: %v", err)
		}

		if sexpr.Operator != "products.add" {
			t.Errorf("Expected operator 'products.add', got '%s'", sexpr.Operator)
		}

		if len(sexpr.Args) != 3 {
			t.Errorf("Expected 3 arguments, got %d", len(sexpr.Args))
		}

		// Check that all arguments are strings
		for i, arg := range sexpr.Args {
			if _, ok := arg.(string); !ok {
				t.Errorf("Argument %d should be string, got %T", i, arg)
			}
		}
	})

	t.Run("Parse Complex KYC Start", func(t *testing.T) {
		dsl := `(kyc.start
		  (documents
		    (document "CertificateOfIncorporation")
		    (document "W8BEN-E")
		  )
		  (jurisdictions
		    (jurisdiction "LU")
		  )
		)`

		sexpr, err := executor.ParseSExpression(dsl)
		if err != nil {
			t.Fatalf("Failed to parse S-expression: %v", err)
		}

		if sexpr.Operator != "kyc.start" {
			t.Errorf("Expected operator 'kyc.start', got '%s'", sexpr.Operator)
		}

		// Should have documents and jurisdictions nested
		if len(sexpr.Args) != 2 {
			t.Errorf("Expected 2 arguments, got %d", len(sexpr.Args))
		}
	})
}

func TestDSLExecutorCommandExecution(t *testing.T) {
	executor := NewDSLExecutor("CBU-1234")

	t.Run("Execute Case Create", func(t *testing.T) {
		dsl := `(case.create (cbu.id "CBU-1234") (nature-purpose "UCITS equity fund domiciled in LU"))`

		result, err := executor.Execute(dsl)
		if err != nil {
			t.Fatalf("Failed to execute DSL: %v", err)
		}

		if !result.Success {
			t.Errorf("Expected successful execution, got error: %s", result.Error)
		}

		if result.Command != "case.create" {
			t.Errorf("Expected command 'case.create', got '%s'", result.Command)
		}

		// Check variables were set
		if executor.Context.Variables["cbu.id"] != "CBU-1234" {
			t.Errorf("Expected CBU ID to be set in variables")
		}

		if executor.Context.Variables["nature-purpose"] != "UCITS equity fund domiciled in LU" {
			t.Errorf("Expected nature-purpose to be set in variables")
		}
	})

	t.Run("Execute Products Add", func(t *testing.T) {
		dsl := `(products.add "CUSTODY" "FUND_ACCOUNTING")`

		result, err := executor.Execute(dsl)
		if err != nil {
			t.Fatalf("Failed to execute DSL: %v", err)
		}

		if !result.Success {
			t.Errorf("Expected successful execution, got error: %s", result.Error)
		}

		// Check products were stored
		products, exists := executor.Context.Variables["products"]
		if !exists {
			t.Errorf("Expected products to be stored in variables")
		}

		productList, ok := products.([]string)
		if !ok {
			t.Errorf("Expected products to be []string, got %T", products)
		}

		if len(productList) != 2 {
			t.Errorf("Expected 2 products, got %d", len(productList))
		}

		if productList[0] != "CUSTODY" || productList[1] != "FUND_ACCOUNTING" {
			t.Errorf("Products not stored correctly: %v", productList)
		}
	})

	t.Run("Execute Values Bind", func(t *testing.T) {
		attrID := GenerateTestUUID("test-attr")
		dsl := `(values.bind (bind (attr-id "` + attrID + `") (value "CBU-1234")))`

		result, err := executor.Execute(dsl)
		if err != nil {
			t.Fatalf("Failed to execute DSL: %v", err)
		}

		if !result.Success {
			t.Errorf("Expected successful execution, got error: %s", result.Error)
		}

		// Check value was bound to attribute
		value, exists := executor.Context.Variables[attrID]
		if !exists {
			t.Errorf("Expected attribute %s to be bound", attrID)
		}

		if value != "CBU-1234" {
			t.Errorf("Expected bound value 'CBU-1234', got '%v'", value)
		}
	})

	t.Run("Execute Resources Plan", func(t *testing.T) {
		attrID := GenerateTestUUID("custody-attr")
		dsl := `(resources.plan
		  (resource.create "CustodyAccount"
		    (owner "CustodyTech")
		    (var (attr-id "` + attrID + `"))
		  )
		)`

		result, err := executor.Execute(dsl)
		if err != nil {
			t.Fatalf("Failed to execute DSL: %v", err)
		}

		if !result.Success {
			t.Errorf("Expected successful execution, got error: %s", result.Error)
		}

		// Check resource was created
		output, ok := result.Output.(map[string]interface{})
		if !ok {
			t.Errorf("Expected output to be map, got %T", result.Output)
		}

		if output["name"] != "CustodyAccount" {
			t.Errorf("Expected resource name 'CustodyAccount', got '%v'", output["name"])
		}

		if output["owner"] != "CustodyTech" {
			t.Errorf("Expected owner 'CustodyTech', got '%v'", output["owner"])
		}

		if output["attr_id"] != attrID {
			t.Errorf("Expected attr_id '%s', got '%v'", attrID, output["attr_id"])
		}
	})
}

func TestDSLExecutorCompleteWorkflow(t *testing.T) {
	executor := NewDSLExecutor("CBU-1234")

	// Execute a complete onboarding workflow
	commands := []string{
		`(case.create (cbu.id "CBU-1234") (nature-purpose "UCITS equity fund domiciled in LU"))`,
		`(products.add "CUSTODY" "FUND_ACCOUNTING")`,
		`(kyc.start
		  (documents
		    (document "CertificateOfIncorporation")
		    (document "W8BEN-E")
		  )
		  (jurisdictions
		    (jurisdiction "LU")
		  )
		)`,
	}

	// Add resource planning with UUID binding
	attrID := GenerateTestUUID("custody-attr")
	commands = append(commands,
		`(resources.plan
		  (resource.create "CustodyAccount"
		    (owner "CustodyTech")
		    (var (attr-id "`+attrID+`"))
		  )
		)`,
		`(values.bind (bind (attr-id "`+attrID+`") (value "CBU-1234")))`,
	)

	t.Run("Execute Complete Workflow", func(t *testing.T) {
		results, err := executor.ExecuteBatch(commands)
		if err != nil {
			t.Fatalf("Failed to execute batch: %v", err)
		}

		if len(results) != len(commands) {
			t.Errorf("Expected %d results, got %d", len(commands), len(results))
		}

		// Check all commands succeeded
		for i, result := range results {
			if !result.Success {
				t.Errorf("Command %d failed: %s", i, result.Error)
			}
		}

		// Verify context state
		summary := executor.GetExecutionSummary()

		if summary["cbu_id"] != "CBU-1234" {
			t.Errorf("Expected CBU ID 'CBU-1234', got '%v'", summary["cbu_id"])
		}

		if summary["commands_run"] != len(commands) {
			t.Errorf("Expected %d commands run, got %v", len(commands), summary["commands_run"])
		}

		// Check specific variables were set
		variables := summary["variables"].(map[string]interface{})

		if variables["cbu.id"] != "CBU-1234" {
			t.Errorf("CBU ID not set correctly in variables")
		}

		if products, exists := variables["products"]; exists {
			productList := products.([]string)
			if len(productList) != 2 {
				t.Errorf("Expected 2 products, got %d", len(productList))
			}
		} else {
			t.Errorf("Products not set in variables")
		}

		// Check attribute binding
		if variables[attrID] != "CBU-1234" {
			t.Errorf("Attribute %s not bound correctly", attrID)
		}
	})
}

func TestDSLExecutorErrorHandling(t *testing.T) {
	executor := NewDSLExecutor("CBU-1234")

	t.Run("Invalid S-Expression Syntax", func(t *testing.T) {
		dsl := `invalid syntax without parentheses`

		result, err := executor.Execute(dsl)
		if err == nil {
			t.Errorf("Expected error for invalid syntax")
		}

		if result.Success {
			t.Errorf("Expected failed execution for invalid syntax")
		}
	})

	t.Run("Unknown Command", func(t *testing.T) {
		dsl := `(unknown.command "arg1" "arg2")`

		result, err := executor.Execute(dsl)
		if err != nil {
			t.Fatalf("Unexpected parse error: %v", err)
		}

		if result.Success {
			t.Errorf("Expected failed execution for unknown command")
		}

		if !strings.Contains(result.Error, "Unknown command") {
			t.Errorf("Expected 'Unknown command' error, got: %s", result.Error)
		}
	})

	t.Run("Missing Required Arguments", func(t *testing.T) {
		dsl := `(case.create)`

		result, err := executor.Execute(dsl)
		if err != nil {
			t.Fatalf("Unexpected parse error: %v", err)
		}

		if result.Success {
			t.Errorf("Expected failed execution for missing arguments")
		}

		if !strings.Contains(result.Error, "Missing required") {
			t.Errorf("Expected 'Missing required' error, got: %s", result.Error)
		}
	})
}

func TestDSLExecutorUUIDHandling(t *testing.T) {
	executor := NewDSLExecutor("CBU-1234")

	t.Run("UUID Validation", func(t *testing.T) {
		validUUID := "123e4567-e89b-12d3-a456-426614174000"
		invalidUUID := "not-a-uuid"

		if !ValidateUUID(validUUID) {
			t.Errorf("Valid UUID should pass validation: %s", validUUID)
		}

		if ValidateUUID(invalidUUID) {
			t.Errorf("Invalid UUID should fail validation: %s", invalidUUID)
		}
	})

	t.Run("Multiple UUID Attribute Binding", func(t *testing.T) {
		attr1 := GenerateTestUUID("attr1")
		attr2 := GenerateTestUUID("attr2")
		attr3 := GenerateTestUUID("attr3")

		commands := []string{
			`(values.bind (bind (attr-id "` + attr1 + `") (value "value1")))`,
			`(values.bind (bind (attr-id "` + attr2 + `") (value "value2")))`,
			`(values.bind (bind (attr-id "` + attr3 + `") (value "value3")))`,
		}

		results, err := executor.ExecuteBatch(commands)
		if err != nil {
			t.Fatalf("Failed to execute batch: %v", err)
		}

		// Check all bindings succeeded
		for i, result := range results {
			if !result.Success {
				t.Errorf("Binding %d failed: %s", i, result.Error)
			}
		}

		// Verify all attributes are bound
		variables := executor.Context.Variables

		if variables[attr1] != "value1" {
			t.Errorf("Attribute %s not bound correctly", attr1)
		}
		if variables[attr2] != "value2" {
			t.Errorf("Attribute %s not bound correctly", attr2)
		}
		if variables[attr3] != "value3" {
			t.Errorf("Attribute %s not bound correctly", attr3)
		}
	})

	t.Run("UUID Cross-Reference in Resources", func(t *testing.T) {
		// Create resource with UUID attribute
		attrID := GenerateTestUUID("custody-attr")
		resourceDSL := `(resources.plan
		  (resource.create "CustodyAccount"
		    (owner "CustodyTech")
		    (var (attr-id "` + attrID + `"))
		  )
		)`

		result, err := executor.Execute(resourceDSL)
		if err != nil {
			t.Fatalf("Failed to execute resource planning: %v", err)
		}

		if !result.Success {
			t.Errorf("Resource planning failed: %s", result.Error)
		}

		// Now bind value to the same attribute
		bindDSL := `(values.bind (bind (attr-id "` + attrID + `") (value "CUSTODY-ACCOUNT-001")))`

		bindResult, err := executor.Execute(bindDSL)
		if err != nil {
			t.Fatalf("Failed to execute value binding: %v", err)
		}

		if !bindResult.Success {
			t.Errorf("Value binding failed: %s", bindResult.Error)
		}

		// Verify the attribute is bound
		if executor.Context.Variables[attrID] != "CUSTODY-ACCOUNT-001" {
			t.Errorf("Attribute %s not bound correctly after resource creation", attrID)
		}

		// Verify resource reference exists
		resourceRef := attrID + ".resource"
		if _, exists := executor.Context.Variables[resourceRef]; !exists {
			t.Errorf("Resource reference %s not created", resourceRef)
		}
	})
}

func TestDSLExecutorJSONSerialization(t *testing.T) {
	executor := NewDSLExecutor("CBU-1234")

	// Execute a command
	dsl := `(case.create (cbu.id "CBU-1234") (nature-purpose "UCITS equity fund"))`
	result, err := executor.Execute(dsl)
	if err != nil {
		t.Fatalf("Failed to execute DSL: %v", err)
	}

	t.Run("Serialize Execution Result", func(t *testing.T) {
		jsonData, err := json.Marshal(result)
		if err != nil {
			t.Fatalf("Failed to serialize result to JSON: %v", err)
		}

		// Deserialize and check
		var deserialized ExecutionResult
		err = json.Unmarshal(jsonData, &deserialized)
		if err != nil {
			t.Fatalf("Failed to deserialize result from JSON: %v", err)
		}

		if deserialized.Command != result.Command {
			t.Errorf("Command not preserved in JSON: expected %s, got %s",
				result.Command, deserialized.Command)
		}

		if deserialized.Success != result.Success {
			t.Errorf("Success status not preserved in JSON")
		}
	})

	t.Run("Serialize Execution Summary", func(t *testing.T) {
		summary := executor.GetExecutionSummary()

		jsonData, err := json.Marshal(summary)
		if err != nil {
			t.Fatalf("Failed to serialize summary to JSON: %v", err)
		}

		var deserialized map[string]interface{}
		err = json.Unmarshal(jsonData, &deserialized)
		if err != nil {
			t.Fatalf("Failed to deserialize summary from JSON: %v", err)
		}

		if deserialized["cbu_id"] != summary["cbu_id"] {
			t.Errorf("CBU ID not preserved in JSON serialization")
		}
	})
}

// Benchmark tests for performance validation
func BenchmarkDSLExecutorParsing(b *testing.B) {
	executor := NewDSLExecutor("CBU-1234")
	dsl := `(case.create (cbu.id "CBU-1234") (nature-purpose "UCITS equity fund domiciled in LU"))`

	for i := 0; i < b.N; i++ {
		_, err := executor.ParseSExpression(dsl)
		if err != nil {
			b.Fatalf("Parse error: %v", err)
		}
	}
}

func BenchmarkDSLExecutorExecution(b *testing.B) {
	executor := NewDSLExecutor("CBU-1234")
	dsl := `(case.create (cbu.id "CBU-1234") (nature-purpose "UCITS equity fund domiciled in LU"))`

	for i := 0; i < b.N; i++ {
		result, err := executor.Execute(dsl)
		if err != nil {
			b.Fatalf("Execution error: %v", err)
		}
		if !result.Success {
			b.Fatalf("Execution failed: %s", result.Error)
		}
	}
}

func BenchmarkDSLExecutorComplexWorkflow(b *testing.B) {
	commands := []string{
		`(case.create (cbu.id "CBU-1234") (nature-purpose "UCITS equity fund"))`,
		`(products.add "CUSTODY" "FUND_ACCOUNTING")`,
		`(kyc.start (documents (document "CertificateOfIncorporation")))`,
	}

	for i := 0; i < b.N; i++ {
		executor := NewDSLExecutor("CBU-1234")
		_, err := executor.ExecuteBatch(commands)
		if err != nil {
			b.Fatalf("Batch execution error: %v", err)
		}
	}
}
