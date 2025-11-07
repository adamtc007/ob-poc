package registry

import (
	"context"
	"strings"
	"testing"
	"time"
)

func TestNewRouter(t *testing.T) {
	registry := NewRegistry()
	defer registry.Shutdown()

	router := NewRouter(registry)
	if router == nil {
		t.Fatal("NewRouter() returned nil")
	}

	if router.registry != registry {
		t.Error("Router registry not set correctly")
	}

	if router.verbPattern == nil {
		t.Error("Verb pattern not initialized")
	}

	if router.domainSwitchPattern == nil {
		t.Error("Domain switch pattern not initialized")
	}

	if router.keywordMappings == nil {
		t.Error("Keyword mappings not initialized")
	}

	if router.contextMappings == nil {
		t.Error("Context mappings not initialized")
	}

	if router.routingMetrics == nil {
		t.Error("Routing metrics not initialized")
	}
}

func TestRouter_Route_ValidationErrors(t *testing.T) {
	registry := NewRegistry()
	defer registry.Shutdown()
	router := NewRouter(registry)
	ctx := context.Background()

	tests := []struct {
		name    string
		request *RoutingRequest
		wantErr string
	}{
		{
			name:    "Nil request",
			request: nil,
			wantErr: "routing request cannot be nil",
		},
		{
			name: "Empty message",
			request: &RoutingRequest{
				Message: "",
			},
			wantErr: "message cannot be empty",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			_, err := router.Route(ctx, tt.request)
			if err == nil {
				t.Error("Expected error, got nil")
			} else if !strings.Contains(err.Error(), tt.wantErr) {
				t.Errorf("Expected error containing %q, got %q", tt.wantErr, err.Error())
			}
		})
	}
}

func TestRouter_Route_NoDomainsRegistered(t *testing.T) {
	registry := NewRegistry()
	defer registry.Shutdown()
	router := NewRouter(registry)
	ctx := context.Background()

	request := &RoutingRequest{
		Message:   "test message",
		SessionID: "test-session",
		Timestamp: time.Now(),
	}

	_, err := router.Route(ctx, request)
	if err == nil {
		t.Error("Expected error when no domains registered")
	}
}

func TestRouter_RouteByExplicitSwitch(t *testing.T) {
	registry := NewRegistry()
	defer registry.Shutdown()

	// Register domains
	onboardingDomain := NewMockDomain("onboarding", "1.0.0")
	hedgeFundDomain := NewMockDomain("hedge-fund-investor", "1.0.0")
	registry.Register(onboardingDomain)
	registry.Register(hedgeFundDomain)

	router := NewRouter(registry)
	ctx := context.Background()

	tests := []struct {
		name           string
		message        string
		expectedDomain string
		expectedConf   float64
		wantErr        bool
	}{
		{
			name:           "Switch to onboarding",
			message:        "switch to onboarding domain",
			expectedDomain: "onboarding",
			expectedConf:   1.0,
			wantErr:        false,
		},
		{
			name:           "Switch to hedge fund investor",
			message:        "switch to hedge fund investor domain",
			expectedDomain: "hedge-fund-investor",
			expectedConf:   1.0,
			wantErr:        false,
		},
		{
			name:           "Switch to hedge fund (alternative)",
			message:        "switch to hedge fund domain",
			expectedDomain: "hedge-fund-investor",
			expectedConf:   1.0,
			wantErr:        false,
		},
		{
			name:    "Switch to unknown domain",
			message: "switch to unknown domain",
			wantErr: true,
		},
		{
			name:    "No explicit switch",
			message: "start workflow",
			wantErr: true, // This test expects explicit switch to fail for non-switch messages
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			request := &RoutingRequest{
				Message:   tt.message,
				SessionID: "test-session",
				Timestamp: time.Now(),
			}

			response, err := router.routeByExplicitSwitch(ctx, request)
			if tt.wantErr {
				if err == nil && response != nil {
					t.Error("Expected error or nil response")
				}
				return
			}

			if err != nil {
				t.Errorf("Unexpected error: %v", err)
				return
			}

			if response == nil {
				t.Fatal("Expected response, got nil")
			}

			if response.DomainName != tt.expectedDomain {
				t.Errorf("Expected domain %s, got %s", tt.expectedDomain, response.DomainName)
			}

			if response.Strategy != StrategyExplicit {
				t.Errorf("Expected strategy %s, got %s", StrategyExplicit, response.Strategy)
			}

			if response.Confidence != tt.expectedConf {
				t.Errorf("Expected confidence %.1f, got %.1f", tt.expectedConf, response.Confidence)
			}
		})
	}
}

func TestRouter_RouteByDSLVerbs(t *testing.T) {
	registry := NewRegistry()
	defer registry.Shutdown()

	// Register domains with specific verbs
	onboardingDomain := NewMockDomain("onboarding", "1.0.0")
	hedgeFundDomain := NewMockDomain("hedge-fund-investor", "1.0.0")
	registry.Register(onboardingDomain)
	registry.Register(hedgeFundDomain)

	router := NewRouter(registry)
	ctx := context.Background()

	tests := []struct {
		name           string
		message        string
		expectedDomain string
		expectSuccess  bool
	}{
		{
			name:           "Onboarding verb",
			message:        "(onboarding.start \"test-id\")",
			expectedDomain: "onboarding",
			expectSuccess:  true,
		},
		{
			name:           "Hedge fund verb",
			message:        "(hedge-fund-investor.start \"test-id\")",
			expectedDomain: "hedge-fund-investor",
			expectSuccess:  true,
		},
		{
			name:          "Invalid DSL",
			message:       "not valid dsl syntax",
			expectSuccess: false,
		},
		{
			name:          "No verbs",
			message:       "()",
			expectSuccess: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			request := &RoutingRequest{
				Message:   tt.message,
				SessionID: "test-session",
				Timestamp: time.Now(),
			}

			response, err := router.routeByDSLVerbs(ctx, request)

			if !tt.expectSuccess {
				if err == nil && response != nil {
					t.Error("Expected error or nil response for unsuccessful case")
				}
				return
			}

			if err != nil {
				t.Errorf("Unexpected error: %v", err)
				return
			}

			if response == nil {
				t.Fatal("Expected response, got nil")
			}

			if response.DomainName != tt.expectedDomain {
				t.Errorf("Expected domain %s, got %s", tt.expectedDomain, response.DomainName)
			}

			if response.Strategy != StrategyVerb {
				t.Errorf("Expected strategy %s, got %s", StrategyVerb, response.Strategy)
			}
		})
	}
}

func TestRouter_RouteByContext(t *testing.T) {
	registry := NewRegistry()
	defer registry.Shutdown()

	onboardingDomain := NewMockDomain("onboarding", "1.0.0")
	hedgeFundDomain := NewMockDomain("hedge-fund-investor", "1.0.0")
	registry.Register(onboardingDomain)
	registry.Register(hedgeFundDomain)

	router := NewRouter(registry)
	ctx := context.Background()

	tests := []struct {
		name           string
		context        map[string]interface{}
		expectedDomain string
		expectSuccess  bool
	}{
		{
			name: "Context with investor_id",
			context: map[string]interface{}{
				"investor_id": "uuid-123",
			},
			expectedDomain: "hedge-fund-investor",
			expectSuccess:  true,
		},
		{
			name: "Context with cbu_id",
			context: map[string]interface{}{
				"cbu_id": "CBU-123",
			},
			expectedDomain: "onboarding",
			expectSuccess:  true,
		},
		{
			name: "Context with hedge fund state",
			context: map[string]interface{}{
				"current_state": "KYC_PENDING",
			},
			expectedDomain: "hedge-fund-investor",
			expectSuccess:  true,
		},
		{
			name: "Context with onboarding state",
			context: map[string]interface{}{
				"current_state": "ADD_PRODUCTS",
			},
			expectedDomain: "onboarding",
			expectSuccess:  true,
		},
		{
			name:          "Empty context",
			context:       map[string]interface{}{},
			expectSuccess: false,
		},
		{
			name:          "Nil context",
			context:       nil,
			expectSuccess: false,
		},
		{
			name: "Context with unrecognized keys",
			context: map[string]interface{}{
				"unknown_key": "value",
			},
			expectSuccess: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			request := &RoutingRequest{
				Message:   "test message",
				SessionID: "test-session",
				Context:   tt.context,
				Timestamp: time.Now(),
			}

			response, err := router.routeByContext(ctx, request)

			if !tt.expectSuccess {
				if err == nil && response != nil {
					t.Error("Expected error or nil response for unsuccessful case")
				}
				return
			}

			if err != nil {
				t.Errorf("Unexpected error: %v", err)
				return
			}

			if response == nil {
				t.Fatal("Expected response, got nil")
			}

			if response.DomainName != tt.expectedDomain {
				t.Errorf("Expected domain %s, got %s", tt.expectedDomain, response.DomainName)
			}

			if response.Strategy != StrategyContext {
				t.Errorf("Expected strategy %s, got %s", StrategyContext, response.Strategy)
			}

			if response.Confidence <= 0 {
				t.Error("Expected positive confidence")
			}
		})
	}
}

func TestRouter_RouteByKeywords(t *testing.T) {
	registry := NewRegistry()
	defer registry.Shutdown()

	onboardingDomain := NewMockDomain("onboarding", "1.0.0")
	hedgeFundDomain := NewMockDomain("hedge-fund-investor", "1.0.0")
	registry.Register(onboardingDomain)
	registry.Register(hedgeFundDomain)

	router := NewRouter(registry)
	ctx := context.Background()

	tests := []struct {
		name           string
		message        string
		expectedDomain string
		expectSuccess  bool
	}{
		{
			name:           "Onboard keyword",
			message:        "I want to onboard a new client",
			expectedDomain: "onboarding",
			expectSuccess:  true,
		},
		{
			name:           "Case keyword",
			message:        "Create a new case",
			expectedDomain: "onboarding",
			expectSuccess:  true,
		},
		{
			name:           "Investor keyword",
			message:        "Add new investor",
			expectedDomain: "hedge-fund-investor",
			expectSuccess:  true,
		},
		{
			name:           "Subscription keyword",
			message:        "Process subscription request",
			expectedDomain: "hedge-fund-investor",
			expectSuccess:  true,
		},
		{
			name:           "Multiple keywords - longer wins",
			message:        "investor subscription", // Both present, subscription is longer
			expectedDomain: "hedge-fund-investor",
			expectSuccess:  true,
		},
		{
			name:          "No matching keywords",
			message:       "generic message with no keywords",
			expectSuccess: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			request := &RoutingRequest{
				Message:   tt.message,
				SessionID: "test-session",
				Timestamp: time.Now(),
			}

			response, err := router.routeByKeywords(ctx, request)

			if !tt.expectSuccess {
				if err == nil && response != nil {
					t.Error("Expected error or nil response for unsuccessful case")
				}
				return
			}

			if err != nil {
				t.Errorf("Unexpected error: %v", err)
				return
			}

			if response == nil {
				t.Fatal("Expected response, got nil")
			}

			if response.DomainName != tt.expectedDomain {
				t.Errorf("Expected domain %s, got %s", tt.expectedDomain, response.DomainName)
			}

			if response.Strategy != StrategyKeyword {
				t.Errorf("Expected strategy %s, got %s", StrategyKeyword, response.Strategy)
			}

			if len(response.MatchedKeywords) == 0 {
				t.Error("Expected at least one matched keyword")
			}
		})
	}
}

func TestRouter_RouteByDefault(t *testing.T) {
	registry := NewRegistry()
	defer registry.Shutdown()

	onboardingDomain := NewMockDomain("onboarding", "1.0.0")
	registry.Register(onboardingDomain)

	router := NewRouter(registry)
	ctx := context.Background()

	tests := []struct {
		name           string
		currentDomain  string
		expectedDomain string
	}{
		{
			name:           "Use current domain",
			currentDomain:  "onboarding",
			expectedDomain: "onboarding",
		},
		{
			name:           "No current domain - use first available",
			currentDomain:  "",
			expectedDomain: "onboarding", // Alphabetically first
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			request := &RoutingRequest{
				Message:       "test message",
				SessionID:     "test-session",
				CurrentDomain: tt.currentDomain,
				Timestamp:     time.Now(),
			}

			response, err := router.routeByDefault(ctx, request)
			if err != nil {
				t.Errorf("Unexpected error: %v", err)
				return
			}

			if response == nil {
				t.Fatal("Expected response, got nil")
			}

			if response.DomainName != tt.expectedDomain {
				t.Errorf("Expected domain %s, got %s", tt.expectedDomain, response.DomainName)
			}

			if response.Strategy != StrategyDefault {
				t.Errorf("Expected strategy %s, got %s", StrategyDefault, response.Strategy)
			}

			if response.Confidence != 0.2 {
				t.Errorf("Expected confidence 0.2, got %.1f", response.Confidence)
			}
		})
	}
}

func TestRouter_RouteByFallback(t *testing.T) {
	registry := NewRegistry()
	defer registry.Shutdown()

	router := NewRouter(registry)
	ctx := context.Background()

	// Test with no domains - should fail
	request := &RoutingRequest{
		Message:   "test message",
		SessionID: "test-session",
		Timestamp: time.Now(),
	}

	_, err := router.routeByFallback(ctx, request)
	if err == nil {
		t.Error("Expected error when no domains available for fallback")
	}

	// Add domains
	alpaDomain := NewMockDomain("alpha", "1.0.0")
	onboardingDomain := NewMockDomain("onboarding", "1.0.0")
	registry.Register(alpaDomain)
	registry.Register(onboardingDomain)

	response, err := router.routeByFallback(ctx, request)
	if err != nil {
		t.Errorf("Unexpected error: %v", err)
		return
	}

	if response == nil {
		t.Fatal("Expected response, got nil")
	}

	// Should prefer onboarding over alphabetically first
	if response.DomainName != "onboarding" {
		t.Errorf("Expected 'onboarding' domain, got %s", response.DomainName)
	}

	if response.Strategy != StrategyFallback {
		t.Errorf("Expected strategy %s, got %s", StrategyFallback, response.Strategy)
	}

	if response.Confidence != 0.1 {
		t.Errorf("Expected confidence 0.1, got %.1f", response.Confidence)
	}
}

func TestRouter_Route_StrategyPriority(t *testing.T) {
	registry := NewRegistry()
	defer registry.Shutdown()

	onboardingDomain := NewMockDomain("onboarding", "1.0.0")
	hedgeFundDomain := NewMockDomain("hedge-fund-investor", "1.0.0")
	registry.Register(onboardingDomain)
	registry.Register(hedgeFundDomain)

	router := NewRouter(registry)
	ctx := context.Background()

	// Test that explicit switch takes priority over everything else
	request := &RoutingRequest{
		Message:       "switch to hedge fund investor domain but also onboard and case",
		SessionID:     "test-session",
		CurrentDomain: "onboarding",
		Context: map[string]interface{}{
			"cbu_id": "CBU-123", // Would suggest onboarding
		},
		Timestamp: time.Now(),
	}

	response, err := router.Route(ctx, request)
	if err != nil {
		t.Errorf("Unexpected error: %v", err)
		return
	}

	if response.Strategy != StrategyExplicit {
		t.Errorf("Expected explicit strategy to take priority, got %s", response.Strategy)
	}

	if response.DomainName != "hedge-fund-investor" {
		t.Errorf("Expected hedge-fund-investor domain, got %s", response.DomainName)
	}
}

func TestRouter_Route_CompleteFlow(t *testing.T) {
	registry := NewRegistry()
	defer registry.Shutdown()

	onboardingDomain := NewMockDomain("onboarding", "1.0.0")
	hedgeFundDomain := NewMockDomain("hedge-fund-investor", "1.0.0")
	registry.Register(onboardingDomain)
	registry.Register(hedgeFundDomain)

	router := NewRouter(registry)
	ctx := context.Background()

	// Test various routing scenarios
	tests := []struct {
		name           string
		request        *RoutingRequest
		expectedDomain string
		expectedStrat  RoutingStrategy
	}{
		{
			name: "Explicit switch",
			request: &RoutingRequest{
				Message:   "switch to onboarding domain",
				SessionID: "test-session",
				Timestamp: time.Now(),
			},
			expectedDomain: "onboarding",
			expectedStrat:  StrategyExplicit,
		},
		{
			name: "Context routing",
			request: &RoutingRequest{
				Message:   "start workflow",
				SessionID: "test-session",
				Context: map[string]interface{}{
					"investor_id": "uuid-123",
				},
				Timestamp: time.Now(),
			},
			expectedDomain: "hedge-fund-investor",
			expectedStrat:  StrategyContext,
		},
		{
			name: "Keyword routing",
			request: &RoutingRequest{
				Message:   "onboard new client",
				SessionID: "test-session",
				Timestamp: time.Now(),
			},
			expectedDomain: "onboarding",
			expectedStrat:  StrategyKeyword,
		},
		{
			name: "Default routing",
			request: &RoutingRequest{
				Message:       "generic message",
				SessionID:     "test-session",
				CurrentDomain: "hedge-fund-investor",
				Timestamp:     time.Now(),
			},
			expectedDomain: "hedge-fund-investor",
			expectedStrat:  StrategyDefault,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			response, err := router.Route(ctx, tt.request)
			if err != nil {
				t.Errorf("Unexpected error: %v", err)
				return
			}

			if response == nil {
				t.Fatal("Expected response, got nil")
			}

			if response.DomainName != tt.expectedDomain {
				t.Errorf("Expected domain %s, got %s", tt.expectedDomain, response.DomainName)
			}

			if response.Strategy != tt.expectedStrat {
				t.Errorf("Expected strategy %s, got %s", tt.expectedStrat, response.Strategy)
			}

			if response.Domain == nil {
				t.Error("Expected domain object to be set")
			}

			if response.Confidence <= 0 {
				t.Error("Expected positive confidence")
			}

			if response.ProcessingTime <= 0 {
				t.Error("Expected positive processing time")
			}
		})
	}
}

func TestRouter_GetRoutingMetrics(t *testing.T) {
	registry := NewRegistry()
	defer registry.Shutdown()

	onboardingDomain := NewMockDomain("onboarding", "1.0.0")
	registry.Register(onboardingDomain)

	router := NewRouter(registry)
	ctx := context.Background()

	// Perform some routing operations
	request := &RoutingRequest{
		Message:   "onboard client",
		SessionID: "test-session",
		Timestamp: time.Now(),
	}

	for i := 0; i < 5; i++ {
		router.Route(ctx, request)
	}

	metrics := router.GetRoutingMetrics()
	if metrics == nil {
		t.Fatal("Expected metrics, got nil")
	}

	if metrics.TotalRequests != 5 {
		t.Errorf("Expected 5 total requests, got %d", metrics.TotalRequests)
	}

	if count, exists := metrics.StrategyUsage[StrategyKeyword]; !exists || count != 5 {
		t.Errorf("Expected 5 keyword strategy usages, got %d", count)
	}

	if count, exists := metrics.DomainSelections["onboarding"]; !exists || count != 5 {
		t.Errorf("Expected 5 onboarding selections, got %d", count)
	}

	if metrics.AverageConfidence <= 0 {
		t.Error("Expected positive average confidence")
	}

	if metrics.AverageResponseTime <= 0 {
		t.Error("Expected positive average response time")
	}
}

func TestRouter_RouteWithTimestamp(t *testing.T) {
	registry := NewRegistry()
	defer registry.Shutdown()

	onboardingDomain := NewMockDomain("onboarding", "1.0.0")
	registry.Register(onboardingDomain)

	router := NewRouter(registry)
	ctx := context.Background()

	// Test with explicit timestamp
	fixedTime := time.Date(2024, 1, 1, 12, 0, 0, 0, time.UTC)
	request := &RoutingRequest{
		Message:   "onboard client",
		SessionID: "test-session",
		Timestamp: fixedTime,
	}

	response, err := router.Route(ctx, request)
	if err != nil {
		t.Errorf("Unexpected error: %v", err)
		return
	}

	if response.ProcessingTime <= 0 {
		t.Error("Expected positive processing time")
	}

	// Test with zero timestamp (should be set automatically)
	request.Timestamp = time.Time{}
	response, err = router.Route(ctx, request)
	if err != nil {
		t.Errorf("Unexpected error: %v", err)
		return
	}

	if response.ProcessingTime <= 0 {
		t.Error("Expected positive processing time even with zero timestamp")
	}
}

func TestRouter_VerbRegexExtraction(t *testing.T) {
	registry := NewRegistry()
	defer registry.Shutdown()

	onboardingDomain := NewMockDomain("onboarding", "1.0.0")
	registry.Register(onboardingDomain)

	router := NewRouter(registry)
	ctx := context.Background()

	// Test verb extraction via regex when parsing fails
	request := &RoutingRequest{
		Message:   "(onboarding.start incomplete dsl", // Invalid DSL syntax
		SessionID: "test-session",
		Timestamp: time.Now(),
	}

	response, err := router.routeByVerbRegex(ctx, request)
	if err != nil {
		t.Errorf("Unexpected error: %v", err)
		return
	}

	if response == nil {
		t.Fatal("Expected response from regex routing, got nil")
	}

	if response.DomainName != "onboarding" {
		t.Errorf("Expected onboarding domain, got %s", response.DomainName)
	}

	if response.Strategy != StrategyVerb {
		t.Errorf("Expected verb strategy, got %s", response.Strategy)
	}

	// Confidence should be lower for regex matching
	if response.Confidence >= 1.0 {
		t.Errorf("Expected lower confidence for regex matching, got %.2f", response.Confidence)
	}
}

func TestRouter_StateInference(t *testing.T) {
	router := &Router{}

	tests := []struct {
		name           string
		state          string
		expectedDomain string
	}{
		{
			name:           "Hedge fund state",
			state:          "KYC_PENDING",
			expectedDomain: "hedge-fund-investor",
		},
		{
			name:           "Onboarding state",
			state:          "ADD_PRODUCTS",
			expectedDomain: "onboarding",
		},
		{
			name:           "Unknown state",
			state:          "UNKNOWN_STATE",
			expectedDomain: "",
		},
		{
			name:           "Empty state",
			state:          "",
			expectedDomain: "",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			domain := router.inferDomainFromState(tt.state)
			if domain != tt.expectedDomain {
				t.Errorf("Expected domain %s, got %s", tt.expectedDomain, domain)
			}
		})
	}
}

func TestRouter_DomainNameNormalization(t *testing.T) {
	router := &Router{}

	tests := []struct {
		name         string
		phrase       string
		expectedName string
	}{
		{
			name:         "Simple name",
			phrase:       "onboarding",
			expectedName: "onboarding",
		},
		{
			name:         "Multi-word with spaces",
			phrase:       "hedge fund investor",
			expectedName: "hedge-fund-investor",
		},
		{
			name:         "Mixed case",
			phrase:       "Hedge Fund Investor",
			expectedName: "hedge-fund-investor",
		},
		{
			name:         "Extra whitespace",
			phrase:       "  hedge fund  ",
			expectedName: "hedge-fund",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			normalized := router.normalizeDomainName(tt.phrase)
			if normalized != tt.expectedName {
				t.Errorf("Expected normalized name %s, got %s", tt.expectedName, normalized)
			}
		})
	}
}

func TestRouter_AlternativeDomainNames(t *testing.T) {
	router := &Router{}

	tests := []struct {
		name                string
		phrase              string
		expectedAlternative string
	}{
		{
			name:                "Hedge fund",
			phrase:              "hedge fund",
			expectedAlternative: "hedge-fund-investor",
		},
		{
			name:                "HF abbreviation",
			phrase:              "hf",
			expectedAlternative: "hedge-fund-investor",
		},
		{
			name:                "Investor",
			phrase:              "investor",
			expectedAlternative: "hedge-fund-investor",
		},
		{
			name:                "OB abbreviation",
			phrase:              "ob",
			expectedAlternative: "onboarding",
		},
		{
			name:                "Client",
			phrase:              "client",
			expectedAlternative: "onboarding",
		},
		{
			name:                "Unknown phrase",
			phrase:              "unknown",
			expectedAlternative: "",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			alternative := router.findAlternativeDomainName(tt.phrase)
			if alternative != tt.expectedAlternative {
				t.Errorf("Expected alternative %s, got %s", tt.expectedAlternative, alternative)
			}
		})
	}
}
