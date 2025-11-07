package dsl_templates

import (
	"regexp"
	"strings"
	"testing"
)

func TestDSLTemplateGenerator(t *testing.T) {
	generator := NewDSLTemplateGenerator()

	testCases := []struct {
		name       string
		domain     string
		state      string
		params     map[string]interface{}
		wantError  bool
		wantPrefix string
	}{
		{
			name:   "Investor Created Valid",
			domain: "investor",
			state:  "CREATED",
			params: map[string]interface{}{
				"name": "Alice Johnson",
				"type": "PROPER_PERSON",
			},
			wantError:  false,
			wantPrefix: "(investor.create",
		},
		{
			name:   "Hedge Fund Created Valid",
			domain: "hedge-fund",
			state:  "FUND_CREATED",
			params: map[string]interface{}{
				"name":     "Quantum Alpha",
				"strategy": "LONG/SHORT",
			},
			wantError:  false,
			wantPrefix: "(fund.create",
		},
		{
			name:   "Trust Created Valid",
			domain: "trust",
			state:  "CREATED",
			params: map[string]interface{}{
				"type":    "REVOCABLE",
				"grantor": "John Smith",
			},
			wantError:  false,
			wantPrefix: "(trust.create",
		},
		{
			name:      "Invalid Domain",
			domain:    "unknown",
			state:     "CREATED",
			params:    map[string]interface{}{},
			wantError: true,
		},
		{
			name:      "Missing Required Params",
			domain:    "investor",
			state:     "CREATED",
			params:    map[string]interface{}{},
			wantError: true,
		},
	}

	for _, tc := range testCases {
		t.Run(tc.name, func(t *testing.T) {
			dsl, err := generator.GenerateDSL(tc.domain, tc.state, tc.params)

			if tc.wantError {
				if err == nil {
					t.Errorf("Expected error, got nil")
				}
				return
			}

			if err != nil {
				t.Fatalf("Unexpected error: %v", err)
			}

			if tc.wantPrefix != "" && !strings.HasPrefix(dsl, tc.wantPrefix) {
				t.Errorf("DSL does not start with expected prefix. Got: %s, Want prefix: %s", dsl, tc.wantPrefix)
			}

			// Validate DSL contains a timestamp
			timestampRegex := regexp.MustCompile(`\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}`)
			if !timestampRegex.MatchString(dsl) {
				t.Errorf("DSL is missing valid timestamp")
			}
		})
	}
}

func TestDSLTemplateGeneratorComplexScenarios(t *testing.T) {
	generator := NewDSLTemplateGenerator()

	complexTestCases := []struct {
		name       string
		domain     string
		state      string
		params     map[string]interface{}
		validation func(string) bool
	}{
		{
			name:   "Investor KYC Complex",
			domain: "investor",
			state:  "KYC_STARTED",
			params: map[string]interface{}{
				"document":     "Passport",
				"jurisdiction": "US",
			},
			validation: func(dsl string) bool {
				return strings.Contains(dsl, "document") &&
					strings.Contains(dsl, "jurisdiction") &&
					strings.Contains(dsl, "started-at")
			},
		},
		{
			name:   "Hedge Fund Risk Assessment",
			domain: "hedge-fund",
			state:  "RISK_ASSESSMENT",
			params: map[string]interface{}{
				"category":    "MARKET_NEUTRAL",
				"score":       "8.5",
				"mitigations": []string{"Diversification", "Hedging"},
			},
			validation: func(dsl string) bool {
				return strings.Contains(dsl, "MARKET_NEUTRAL") &&
					strings.Contains(dsl, "8.5") &&
					strings.Contains(dsl, "Diversification") &&
					strings.Contains(dsl, "Hedging")
			},
		},
	}

	for _, tc := range complexTestCases {
		t.Run(tc.name, func(t *testing.T) {
			dsl, err := generator.GenerateDSL(tc.domain, tc.state, tc.params)

			if err != nil {
				t.Fatalf("Unexpected error: %v", err)
			}

			if !tc.validation(dsl) {
				t.Errorf("DSL validation failed. DSL: %s", dsl)
			}
		})
	}
}

func BenchmarkDSLGeneration(b *testing.B) {
	generator := NewDSLTemplateGenerator()

	benchmarkParams := map[string]map[string]interface{}{
		"investor": {
			"name": "Performance Test Investor",
			"type": "CORPORATE",
		},
		"hedge-fund": {
			"name":     "Performance Test Fund",
			"strategy": "QUANT",
		},
		"trust": {
			"type":    "REVOCABLE",
			"grantor": "Performance Test Grantor",
		},
	}

	for domain, params := range benchmarkParams {
		b.Run(domain, func(b *testing.B) {
			b.ResetTimer()
			for i := 0; i < b.N; i++ {
				_, err := generator.GenerateDSL(domain, "CREATED", params)
				if err != nil {
					b.Fatalf("Error generating DSL: %v", err)
				}
			}
		})
	}
}

/*
This test file provides comprehensive coverage for the DSL Template Generator, including:

1. Basic domain and state generation tests
2. Error handling scenarios
3. Complex DSL generation validation
4. Performance benchmarks
5. Timestamp and content validation

The tests cover various scenarios:
- Successful DSL generation for different domains
- Invalid domain and parameter handling
- Complex parameter scenarios
- Performance testing of DSL generation

Key test strategies:
- Table-driven tests
- Validation functions
- Performance benchmarking
- Error case coverage
*/
