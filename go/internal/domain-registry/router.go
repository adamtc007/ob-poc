// Package registry provides intelligent routing capabilities to determine
// which domain should handle a given request.
//
// The Router implements multiple routing strategies:
// 1. Explicit domain switching: "switch to hedge fund investor domain"
// 2. Context-based routing: If investor_id exists, route to hedge-fund-investor
// 3. Verb-based routing: Parse DSL to find which domain owns the verb
// 4. Keyword-based routing: "onboard" → onboarding, "subscribe" → hedge fund
// 5. Default routing: Use session's current domain
//
// Usage:
//
//	router := NewRouter(registry)
//	domain, err := router.Route(ctx, &RoutingRequest{
//		Message: "Start KYC for this investor",
//		Context: map[string]interface{}{"investor_id": "uuid-123"},
//	})
package registry

import (
	"context"
	"fmt"
	"regexp"
	"sort"
	"strings"
	"time"

	"dsl-ob-poc/internal/shared-dsl/parser"
)

// Router determines which domain should handle a given request using
// multiple intelligent routing strategies.
type Router struct {
	registry *Registry

	// verbPattern extracts verbs from DSL using regex
	verbPattern *regexp.Regexp

	// domainSwitchPattern detects explicit domain switching requests
	domainSwitchPattern *regexp.Regexp

	// keywordMappings maps keywords to domain names
	keywordMappings map[string]string

	// contextMappings maps context keys to domain names
	contextMappings map[string]string

	// routingMetrics tracks routing decisions and performance
	routingMetrics *RoutingMetrics
}

// RoutingRequest contains all information needed to make routing decisions
type RoutingRequest struct {
	// Primary inputs
	Message   string `json:"message"`    // User's natural language message
	SessionID string `json:"session_id"` // Session identifier

	// Current state
	CurrentDomain string                 `json:"current_domain,omitempty"` // Current domain in session
	Context       map[string]interface{} `json:"context,omitempty"`        // Session context
	ExistingDSL   string                 `json:"existing_dsl,omitempty"`   // Accumulated DSL

	// Routing hints (optional)
	PreferredDomain string   `json:"preferred_domain,omitempty"` // User's preferred domain
	ExcludeDomains  []string `json:"exclude_domains,omitempty"`  // Domains to avoid

	// Metadata
	RequestID string    `json:"request_id,omitempty"`
	Timestamp time.Time `json:"timestamp"`
	UserID    string    `json:"user_id,omitempty"`
}

// RoutingResponse contains the routing decision and metadata
type RoutingResponse struct {
	// Primary result
	Domain     Domain `json:"-"`           // Selected domain (not serialized)
	DomainName string `json:"domain_name"` // Name of selected domain

	// Routing decision metadata
	Strategy     RoutingStrategy `json:"strategy"`               // Which strategy was used
	Confidence   float64         `json:"confidence"`             // Confidence in decision (0.0-1.0)
	Reason       string          `json:"reason"`                 // Human-readable explanation
	Alternatives []string        `json:"alternatives,omitempty"` // Other possible domains

	// Context for debugging
	MatchedKeywords []string      `json:"matched_keywords,omitempty"`
	MatchedVerbs    []string      `json:"matched_verbs,omitempty"`
	ContextKeys     []string      `json:"context_keys,omitempty"`
	ProcessingTime  time.Duration `json:"processing_time"`

	// Metadata
	RequestID string    `json:"request_id,omitempty"`
	Timestamp time.Time `json:"timestamp"`
}

// RoutingStrategy indicates which routing method was used
type RoutingStrategy string

const (
	StrategyExplicit RoutingStrategy = "EXPLICIT" // Explicit domain switch request
	StrategyContext  RoutingStrategy = "CONTEXT"  // Based on session context
	StrategyVerb     RoutingStrategy = "VERB"     // Based on DSL verb analysis
	StrategyKeyword  RoutingStrategy = "KEYWORD"  // Based on keyword matching
	StrategyDefault  RoutingStrategy = "DEFAULT"  // Use current/fallback domain
	StrategyFallback RoutingStrategy = "FALLBACK" // Emergency fallback
)

// RoutingMetrics tracks routing performance and decisions
type RoutingMetrics struct {
	TotalRequests       int64                     `json:"total_requests"`
	StrategyUsage       map[RoutingStrategy]int64 `json:"strategy_usage"`
	DomainSelections    map[string]int64          `json:"domain_selections"`
	AverageConfidence   float64                   `json:"average_confidence"`
	AverageResponseTime time.Duration             `json:"average_response_time"`
	FailedRoutings      int64                     `json:"failed_routings"`
	LastUpdated         time.Time                 `json:"last_updated"`
}

// NewRouter creates a new domain router with the given registry
func NewRouter(registry *Registry) *Router {
	router := &Router{
		registry:            registry,
		verbPattern:         regexp.MustCompile(`\(([a-z]+\.[a-z][a-z-]*)\s`),
		domainSwitchPattern: regexp.MustCompile(`(?i)switch\s+to\s+([a-z][a-z-]*(?:\s+[a-z][a-z-]*)*)\s+domain`),
		keywordMappings:     make(map[string]string),
		contextMappings:     make(map[string]string),
		routingMetrics: &RoutingMetrics{
			StrategyUsage:    make(map[RoutingStrategy]int64),
			DomainSelections: make(map[string]int64),
		},
	}

	router.initializeDefaultMappings()
	return router
}

// Route determines which domain should handle the request using multiple strategies
func (r *Router) Route(ctx context.Context, request *RoutingRequest) (*RoutingResponse, error) {
	startTime := time.Now()

	// Validate request
	if request == nil {
		return nil, fmt.Errorf("routing request cannot be nil")
	}
	if request.Message == "" {
		return nil, fmt.Errorf("message cannot be empty")
	}
	if request.Timestamp.IsZero() {
		request.Timestamp = time.Now()
	}

	// Try routing strategies in order of confidence
	strategies := []func(context.Context, *RoutingRequest) (*RoutingResponse, error){
		r.routeByExplicitSwitch,
		r.routeByDSLVerbs,
		r.routeByContext,
		r.routeByKeywords,
		r.routeByDefault,
	}

	var lastError error
	for _, strategy := range strategies {
		response, err := strategy(ctx, request)
		if err != nil {
			lastError = err
			continue
		}
		if response != nil && response.Domain != nil {
			// Update metrics
			r.updateMetrics(response, time.Since(startTime))
			return response, nil
		}
	}

	// All strategies failed - try fallback
	if response, err := r.routeByFallback(ctx, request); err == nil && response != nil {
		r.updateMetrics(response, time.Since(startTime))
		return response, nil
	}

	// Complete failure
	r.routingMetrics.FailedRoutings++
	return nil, fmt.Errorf("failed to route request: %w", lastError)
}

// routeByExplicitSwitch handles explicit domain switching requests
// Example: "switch to hedge fund investor domain"
func (r *Router) routeByExplicitSwitch(ctx context.Context, request *RoutingRequest) (*RoutingResponse, error) {
	matches := r.domainSwitchPattern.FindStringSubmatch(request.Message)
	if len(matches) < 2 {
		return nil, nil // No explicit switch detected
	}

	// Extract domain name and normalize it
	domainPhrase := strings.TrimSpace(matches[1])
	domainName := r.normalizeDomainName(domainPhrase)

	// Try to find the domain
	domain, err := r.registry.Get(domainName)
	if err != nil {
		// Try alternative names
		alternativeName := r.findAlternativeDomainName(domainPhrase)
		if alternativeName != "" {
			if altDomain, altErr := r.registry.Get(alternativeName); altErr == nil {
				domain = altDomain
				domainName = alternativeName
			}
		}
	}

	if domain == nil {
		return nil, fmt.Errorf("unknown domain in switch request: %s", domainPhrase)
	}

	return &RoutingResponse{
		Domain:         domain,
		DomainName:     domainName,
		Strategy:       StrategyExplicit,
		Confidence:     1.0, // Highest confidence for explicit requests
		Reason:         fmt.Sprintf("Explicit domain switch to '%s'", domainName),
		ProcessingTime: time.Since(request.Timestamp),
		RequestID:      request.RequestID,
		Timestamp:      time.Now(),
	}, nil
}

// routeByDSLVerbs analyzes DSL content to determine domain ownership
func (r *Router) routeByDSLVerbs(ctx context.Context, request *RoutingRequest) (*RoutingResponse, error) {
	// Try parsing message as DSL first
	dslContent := request.Message
	if request.ExistingDSL != "" {
		dslContent = request.ExistingDSL + "\n" + request.Message
	}

	// Extract verbs using parser
	ast, err := parser.Parse(dslContent)
	if err != nil {
		// Not valid DSL - try regex extraction as fallback
		return r.routeByVerbRegex(ctx, request)
	}

	verbs := ast.ExtractVerbs()
	if len(verbs) == 0 {
		return nil, nil // No verbs found
	}

	// Find domains that support these verbs
	domainScores := make(map[string]int)
	var matchedVerbs []string

	for _, verb := range verbs {
		domains := r.registry.FindDomainsByVerb(verb)
		for _, domainName := range domains {
			domainScores[domainName]++
			matchedVerbs = append(matchedVerbs, verb)
		}
	}

	if len(domainScores) == 0 {
		return nil, fmt.Errorf("no domains support verbs: %v", verbs)
	}

	// Select domain with highest score
	bestDomain, bestScore := r.selectBestDomain(domainScores)
	domain, err := r.registry.Get(bestDomain)
	if err != nil {
		return nil, err
	}

	// Calculate confidence based on verb coverage
	confidence := float64(bestScore) / float64(len(verbs))
	if confidence > 1.0 {
		confidence = 1.0
	}

	alternatives := r.getAlternatives(domainScores, bestDomain)

	return &RoutingResponse{
		Domain:         domain,
		DomainName:     bestDomain,
		Strategy:       StrategyVerb,
		Confidence:     confidence,
		Reason:         fmt.Sprintf("Domain '%s' supports %d/%d verbs", bestDomain, bestScore, len(verbs)),
		Alternatives:   alternatives,
		MatchedVerbs:   matchedVerbs,
		ProcessingTime: time.Since(request.Timestamp),
		RequestID:      request.RequestID,
		Timestamp:      time.Now(),
	}, nil
}

// routeByVerbRegex uses regex to extract potential verbs when parsing fails
func (r *Router) routeByVerbRegex(ctx context.Context, request *RoutingRequest) (*RoutingResponse, error) {
	matches := r.verbPattern.FindAllStringSubmatch(request.Message, -1)
	if len(matches) == 0 {
		return nil, nil
	}

	var verbs []string
	for _, match := range matches {
		if len(match) > 1 {
			verbs = append(verbs, match[1])
		}
	}

	// Use same logic as routeByDSLVerbs for consistency
	domainScores := make(map[string]int)
	var matchedVerbs []string

	for _, verb := range verbs {
		domains := r.registry.FindDomainsByVerb(verb)
		for _, domainName := range domains {
			domainScores[domainName]++
			matchedVerbs = append(matchedVerbs, verb)
		}
	}

	if len(domainScores) == 0 {
		return nil, nil
	}

	bestDomain, bestScore := r.selectBestDomain(domainScores)
	domain, err := r.registry.Get(bestDomain)
	if err != nil {
		return nil, err
	}

	confidence := float64(bestScore) / float64(len(verbs)) * 0.8 // Lower confidence for regex
	alternatives := r.getAlternatives(domainScores, bestDomain)

	return &RoutingResponse{
		Domain:         domain,
		DomainName:     bestDomain,
		Strategy:       StrategyVerb,
		Confidence:     confidence,
		Reason:         fmt.Sprintf("Regex matched verbs suggest domain '%s'", bestDomain),
		Alternatives:   alternatives,
		MatchedVerbs:   matchedVerbs,
		ProcessingTime: time.Since(request.Timestamp),
		RequestID:      request.RequestID,
		Timestamp:      time.Now(),
	}, nil
}

// routeByContext uses session context to determine appropriate domain
func (r *Router) routeByContext(ctx context.Context, request *RoutingRequest) (*RoutingResponse, error) {
	if len(request.Context) == 0 {
		return nil, nil
	}

	var bestDomain string
	var contextKeys []string
	confidence := 0.0

	// Check for entity-specific context keys
	for key, domainName := range r.contextMappings {
		if _, exists := request.Context[key]; exists {
			bestDomain = domainName
			contextKeys = append(contextKeys, key)
			confidence += 0.8 // High confidence for entity presence
		}
	}

	// Check for current_state context
	if currentState, exists := request.Context["current_state"]; exists {
		if stateStr, ok := currentState.(string); ok {
			if domainName := r.inferDomainFromState(stateStr); domainName != "" {
				if bestDomain == "" {
					bestDomain = domainName
					confidence = 0.6
				}
				contextKeys = append(contextKeys, "current_state")
			}
		}
	}

	if bestDomain == "" {
		return nil, nil
	}

	domain, err := r.registry.Get(bestDomain)
	if err != nil {
		return nil, err
	}

	return &RoutingResponse{
		Domain:         domain,
		DomainName:     bestDomain,
		Strategy:       StrategyContext,
		Confidence:     confidence,
		Reason:         fmt.Sprintf("Context contains keys suggesting domain '%s'", bestDomain),
		ContextKeys:    contextKeys,
		ProcessingTime: time.Since(request.Timestamp),
		RequestID:      request.RequestID,
		Timestamp:      time.Now(),
	}, nil
}

// routeByKeywords uses keyword matching to determine domain
func (r *Router) routeByKeywords(ctx context.Context, request *RoutingRequest) (*RoutingResponse, error) {
	message := strings.ToLower(request.Message)

	var bestDomain string
	var matchedKeywords []string
	highestScore := 0

	for keyword, domainName := range r.keywordMappings {
		if strings.Contains(message, keyword) {
			score := len(keyword) // Longer keywords have higher priority
			if score > highestScore {
				highestScore = score
				bestDomain = domainName
				matchedKeywords = []string{keyword}
			} else if score == highestScore {
				matchedKeywords = append(matchedKeywords, keyword)
			}
		}
	}

	if bestDomain == "" {
		return nil, nil
	}

	domain, err := r.registry.Get(bestDomain)
	if err != nil {
		return nil, err
	}

	// Lower confidence for keyword matching
	confidence := 0.4
	if len(matchedKeywords) > 1 {
		confidence = 0.6 // Higher if multiple keywords match
	}

	return &RoutingResponse{
		Domain:          domain,
		DomainName:      bestDomain,
		Strategy:        StrategyKeyword,
		Confidence:      confidence,
		Reason:          fmt.Sprintf("Keywords %v suggest domain '%s'", matchedKeywords, bestDomain),
		MatchedKeywords: matchedKeywords,
		ProcessingTime:  time.Since(request.Timestamp),
		RequestID:       request.RequestID,
		Timestamp:       time.Now(),
	}, nil
}

// routeByDefault uses the current domain or first available domain
func (r *Router) routeByDefault(ctx context.Context, request *RoutingRequest) (*RoutingResponse, error) {
	var domainName string
	var reason string

	// Try current domain first
	if request.CurrentDomain != "" {
		domainName = request.CurrentDomain
		reason = fmt.Sprintf("Using current session domain '%s'", domainName)
	} else {
		// Fall back to first available domain
		domains := r.registry.List()
		if len(domains) == 0 {
			return nil, fmt.Errorf("no domains registered")
		}
		domainName = domains[0] // Alphabetically first domain
		reason = fmt.Sprintf("Using default domain '%s'", domainName)
	}

	domain, err := r.registry.Get(domainName)
	if err != nil {
		return nil, err
	}

	return &RoutingResponse{
		Domain:         domain,
		DomainName:     domainName,
		Strategy:       StrategyDefault,
		Confidence:     0.2, // Low confidence for default routing
		Reason:         reason,
		ProcessingTime: time.Since(request.Timestamp),
		RequestID:      request.RequestID,
		Timestamp:      time.Now(),
	}, nil
}

// routeByFallback emergency fallback when all other strategies fail
func (r *Router) routeByFallback(ctx context.Context, request *RoutingRequest) (*RoutingResponse, error) {
	domains := r.registry.List()
	if len(domains) == 0 {
		return nil, fmt.Errorf("no domains registered for fallback")
	}

	// Use onboarding as fallback if available, otherwise first domain
	fallbackDomain := domains[0]
	for _, name := range domains {
		if name == "onboarding" {
			fallbackDomain = name
			break
		}
	}

	domain, err := r.registry.Get(fallbackDomain)
	if err != nil {
		return nil, err
	}

	return &RoutingResponse{
		Domain:         domain,
		DomainName:     fallbackDomain,
		Strategy:       StrategyFallback,
		Confidence:     0.1, // Lowest confidence
		Reason:         "Emergency fallback routing",
		ProcessingTime: time.Since(request.Timestamp),
		RequestID:      request.RequestID,
		Timestamp:      time.Now(),
	}, nil
}

// initializeDefaultMappings sets up default keyword and context mappings
func (r *Router) initializeDefaultMappings() {
	// Keyword mappings
	r.keywordMappings = map[string]string{
		// Onboarding keywords
		"onboard":   "onboarding",
		"case":      "onboarding",
		"cbu":       "onboarding",
		"products":  "onboarding",
		"services":  "onboarding",
		"resources": "onboarding",

		// Hedge fund keywords
		"investor":     "hedge-fund-investor",
		"subscription": "hedge-fund-investor",
		"redemption":   "hedge-fund-investor",
		"fund":         "hedge-fund-investor",
		"kyc":          "hedge-fund-investor",
		"aml":          "hedge-fund-investor",
		"screening":    "hedge-fund-investor",

		// Generic financial keywords
		"compliance": "hedge-fund-investor",
		"tax":        "hedge-fund-investor",
		"banking":    "hedge-fund-investor",
	}

	// Context mappings (context key -> domain)
	r.contextMappings = map[string]string{
		"cbu_id":      "onboarding",
		"case_id":     "onboarding",
		"investor_id": "hedge-fund-investor",
		"fund_id":     "hedge-fund-investor",
		"class_id":    "hedge-fund-investor",
		"series_id":   "hedge-fund-investor",
		"trade_id":    "hedge-fund-investor",
	}
}

// Helper functions

func (r *Router) normalizeDomainName(phrase string) string {
	// Convert "hedge fund investor" to "hedge-fund-investor"
	return strings.ReplaceAll(strings.ToLower(strings.TrimSpace(phrase)), " ", "-")
}

func (r *Router) findAlternativeDomainName(phrase string) string {
	alternatives := map[string]string{
		"hedge fund": "hedge-fund-investor",
		"hf":         "hedge-fund-investor",
		"investor":   "hedge-fund-investor",
		"ob":         "onboarding",
		"client":     "onboarding",
	}

	normalized := strings.ToLower(strings.TrimSpace(phrase))
	return alternatives[normalized]
}

func (r *Router) selectBestDomain(scores map[string]int) (string, int) {
	var bestDomain string
	bestScore := 0

	for domain, score := range scores {
		if score > bestScore {
			bestScore = score
			bestDomain = domain
		}
	}

	return bestDomain, bestScore
}

func (r *Router) getAlternatives(scores map[string]int, exclude string) []string {
	var alternatives []string
	for domain := range scores {
		if domain != exclude {
			alternatives = append(alternatives, domain)
		}
	}
	sort.Strings(alternatives)
	return alternatives
}

func (r *Router) inferDomainFromState(state string) string {
	state = strings.ToUpper(state)

	// Hedge fund states
	hfStates := []string{
		"OPPORTUNITY", "PRECHECKS", "KYC_PENDING", "KYC_APPROVED",
		"SUB_PENDING_CASH", "FUNDED_PENDING_NAV", "ISSUED", "ACTIVE",
		"REDEEM_PENDING", "REDEEMED", "OFFBOARDED",
	}

	for _, hfState := range hfStates {
		if state == hfState {
			return "hedge-fund-investor"
		}
	}

	// Onboarding states
	obStates := []string{
		"CREATE", "ADD_PRODUCTS", "DISCOVER_KYC", "DISCOVER_SERVICES",
		"DISCOVER_RESOURCES", "PROVISION", "COMPLETE",
	}

	for _, obState := range obStates {
		if state == obState {
			return "onboarding"
		}
	}

	return ""
}

func (r *Router) updateMetrics(response *RoutingResponse, processingTime time.Duration) {
	r.routingMetrics.TotalRequests++
	r.routingMetrics.StrategyUsage[response.Strategy]++
	r.routingMetrics.DomainSelections[response.DomainName]++

	// Update running averages
	totalRequests := float64(r.routingMetrics.TotalRequests)
	r.routingMetrics.AverageConfidence =
		(r.routingMetrics.AverageConfidence*(totalRequests-1) + response.Confidence) / totalRequests
	r.routingMetrics.AverageResponseTime =
		time.Duration((float64(r.routingMetrics.AverageResponseTime)*(totalRequests-1) + float64(processingTime)) / totalRequests)

	r.routingMetrics.LastUpdated = time.Now()
}

// GetRoutingMetrics returns comprehensive routing metrics
func (r *Router) GetRoutingMetrics() *RoutingMetrics {
	// Return a copy to avoid race conditions
	metrics := &RoutingMetrics{
		TotalRequests:       r.routingMetrics.TotalRequests,
		StrategyUsage:       make(map[RoutingStrategy]int64),
		DomainSelections:    make(map[string]int64),
		AverageConfidence:   r.routingMetrics.AverageConfidence,
		AverageResponseTime: r.routingMetrics.AverageResponseTime,
		FailedRoutings:      r.routingMetrics.FailedRoutings,
		LastUpdated:         r.routingMetrics.LastUpdated,
	}

	for strategy, count := range r.routingMetrics.StrategyUsage {
		metrics.StrategyUsage[strategy] = count
	}

	for domain, count := range r.routingMetrics.DomainSelections {
		metrics.DomainSelections[domain] = count
	}

	return metrics
}
