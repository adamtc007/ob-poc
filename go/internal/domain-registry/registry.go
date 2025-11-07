// Package registry provides thread-safe domain management and lookup capabilities.
//
// The Registry is the central coordination point for all domains in the system.
// It provides:
// - Thread-safe domain registration and lookup
// - Domain discovery by name, category, or capability
// - Health monitoring and metrics aggregation
// - Domain lifecycle management
//
// Usage:
//
//	registry := NewRegistry()
//	registry.Register(onboardingDomain)
//	registry.Register(hedgeFundDomain)
//
//	domain, err := registry.Get("onboarding")
//	vocab := registry.GetVocabulary("hedge-fund-investor")
package registry

import (
	"context"
	"fmt"
	"sort"
	"sync"
	"time"
)

// Registry manages multiple domains with thread-safe operations.
// It acts as the central coordination point for domain discovery,
// health monitoring, and lifecycle management.
type Registry struct {
	// domains maps domain name to Domain implementation
	domains map[string]Domain

	// domainMetadata stores additional metadata about each domain
	domainMetadata map[string]*DomainMetadata

	// mu protects concurrent access to domains map
	mu sync.RWMutex

	// healthCheckInterval determines how often to check domain health
	healthCheckInterval time.Duration

	// healthContext for canceling health checks
	healthContext context.Context
	healthCancel  context.CancelFunc

	// metrics tracks registry-level statistics
	metrics *RegistryMetrics

	// createdAt tracks when the registry was created
	createdAt time.Time
}

// DomainMetadata stores additional information about a registered domain
type DomainMetadata struct {
	RegisteredAt    time.Time `json:"registered_at"`
	LastUsed        time.Time `json:"last_used"`
	UsageCount      int64     `json:"usage_count"`
	HealthStatus    string    `json:"health_status"` // "healthy", "unhealthy", "unknown"
	LastHealthCheck time.Time `json:"last_health_check"`
	Tags            []string  `json:"tags,omitempty"` // Custom tags for categorization
}

// RegistryMetrics provides overall registry statistics
type RegistryMetrics struct {
	TotalDomains        int              `json:"total_domains"`
	HealthyDomains      int              `json:"healthy_domains"`
	UnhealthyDomains    int              `json:"unhealthy_domains"`
	TotalRequests       int64            `json:"total_requests"`
	RequestsPerDomain   map[string]int64 `json:"requests_per_domain"`
	AverageResponseTime time.Duration    `json:"average_response_time"`
	UptimeSeconds       int64            `json:"uptime_seconds"`
	LastUpdated         time.Time        `json:"last_updated"`
}

// RegistryOptions configures registry behavior
type RegistryOptions struct {
	HealthCheckInterval time.Duration // How often to check domain health (default: 30s)
	EnableHealthChecks  bool          // Whether to enable automatic health checks (default: true)
}

// DefaultRegistryOptions returns sensible defaults for registry configuration
func DefaultRegistryOptions() *RegistryOptions {
	return &RegistryOptions{
		HealthCheckInterval: 30 * time.Second,
		EnableHealthChecks:  true,
	}
}

// NewRegistry creates a new domain registry with default configuration
func NewRegistry() *Registry {
	return NewRegistryWithOptions(DefaultRegistryOptions())
}

// NewRegistryWithOptions creates a new domain registry with custom configuration
func NewRegistryWithOptions(opts *RegistryOptions) *Registry {
	if opts == nil {
		opts = DefaultRegistryOptions()
	}

	ctx, cancel := context.WithCancel(context.Background())

	registry := &Registry{
		domains:             make(map[string]Domain),
		domainMetadata:      make(map[string]*DomainMetadata),
		healthCheckInterval: opts.HealthCheckInterval,
		healthContext:       ctx,
		healthCancel:        cancel,
		metrics: &RegistryMetrics{
			RequestsPerDomain: make(map[string]int64),
		},
		createdAt: time.Now(),
	}

	// Start health monitoring if enabled
	if opts.EnableHealthChecks {
		go registry.startHealthMonitoring()
	}

	return registry
}

// Register adds a new domain to the registry.
// Returns error if domain name already exists or domain is invalid.
func (r *Registry) Register(domain Domain) error {
	if domain == nil {
		return fmt.Errorf("domain cannot be nil")
	}

	name := domain.Name()
	if name == "" {
		return fmt.Errorf("domain name cannot be empty")
	}

	version := domain.Version()
	if version == "" {
		return fmt.Errorf("domain version cannot be empty")
	}

	r.mu.Lock()
	defer r.mu.Unlock()

	// Check if domain already exists
	if _, exists := r.domains[name]; exists {
		return fmt.Errorf("domain '%s' is already registered", name)
	}

	// Validate domain by checking its vocabulary
	vocab := domain.GetVocabulary()
	if vocab == nil {
		return fmt.Errorf("domain '%s' has no vocabulary", name)
	}

	if vocab.Domain != name {
		return fmt.Errorf("domain name mismatch: expected '%s', got '%s'", name, vocab.Domain)
	}

	// Register the domain
	r.domains[name] = domain
	r.domainMetadata[name] = &DomainMetadata{
		RegisteredAt:    time.Now(),
		HealthStatus:    "unknown",
		LastHealthCheck: time.Time{},
		Tags:            []string{},
	}

	// Update metrics
	r.updateMetricsLocked()

	return nil
}

// Unregister removes a domain from the registry
func (r *Registry) Unregister(domainName string) error {
	r.mu.Lock()
	defer r.mu.Unlock()

	if _, exists := r.domains[domainName]; !exists {
		return fmt.Errorf("domain '%s' is not registered", domainName)
	}

	delete(r.domains, domainName)
	delete(r.domainMetadata, domainName)
	delete(r.metrics.RequestsPerDomain, domainName)

	// Update metrics
	r.updateMetricsLocked()

	return nil
}

// Get retrieves a domain by name.
// Returns error if domain is not found.
func (r *Registry) Get(domainName string) (Domain, error) {
	r.mu.RLock()
	defer r.mu.RUnlock()

	domain, exists := r.domains[domainName]
	if !exists {
		return nil, fmt.Errorf("domain '%s' is not registered", domainName)
	}

	// Update usage statistics
	go r.recordUsage(domainName)

	return domain, nil
}

// List returns all registered domain names sorted alphabetically
func (r *Registry) List() []string {
	r.mu.RLock()
	defer r.mu.RUnlock()

	names := make([]string, 0, len(r.domains))
	for name := range r.domains {
		names = append(names, name)
	}

	sort.Strings(names)
	return names
}

// ListWithMetadata returns all domains with their metadata
func (r *Registry) ListWithMetadata() map[string]*DomainInfo {
	r.mu.RLock()
	defer r.mu.RUnlock()

	result := make(map[string]*DomainInfo)
	for name, domain := range r.domains {
		metadata := r.domainMetadata[name]
		result[name] = &DomainInfo{
			Name:        name,
			Version:     domain.Version(),
			Description: domain.Description(),
			IsHealthy:   domain.IsHealthy(),
			Metadata:    metadata,
		}
	}

	return result
}

// GetVocabulary returns the vocabulary for a specific domain
func (r *Registry) GetVocabulary(domainName string) (*Vocabulary, error) {
	domain, err := r.Get(domainName)
	if err != nil {
		return nil, err
	}

	return domain.GetVocabulary(), nil
}

// GetAllVocabularies returns vocabularies for all registered domains
func (r *Registry) GetAllVocabularies() map[string]*Vocabulary {
	r.mu.RLock()
	defer r.mu.RUnlock()

	result := make(map[string]*Vocabulary)
	for name, domain := range r.domains {
		result[name] = domain.GetVocabulary()
	}

	return result
}

// FindDomainsByVerb finds all domains that support a specific verb
func (r *Registry) FindDomainsByVerb(verb string) []string {
	r.mu.RLock()
	defer r.mu.RUnlock()

	var matchingDomains []string
	for name, domain := range r.domains {
		vocab := domain.GetVocabulary()
		if _, exists := vocab.Verbs[verb]; exists {
			matchingDomains = append(matchingDomains, name)
		}
	}

	sort.Strings(matchingDomains)
	return matchingDomains
}

// FindDomainsByCategory finds all domains that have verbs in a specific category
func (r *Registry) FindDomainsByCategory(category string) []string {
	r.mu.RLock()
	defer r.mu.RUnlock()

	var matchingDomains []string
	for name, domain := range r.domains {
		vocab := domain.GetVocabulary()
		if _, exists := vocab.Categories[category]; exists {
			matchingDomains = append(matchingDomains, name)
		}
	}

	sort.Strings(matchingDomains)
	return matchingDomains
}

// IsHealthy returns true if all registered domains are healthy
func (r *Registry) IsHealthy() bool {
	r.mu.RLock()
	defer r.mu.RUnlock()

	for _, domain := range r.domains {
		if !domain.IsHealthy() {
			return false
		}
	}

	return true
}

// GetMetrics returns comprehensive registry metrics
func (r *Registry) GetMetrics() *RegistryMetrics {
	r.mu.RLock()
	defer r.mu.RUnlock()

	// Create a copy to avoid race conditions
	metrics := &RegistryMetrics{
		TotalDomains:        r.metrics.TotalDomains,
		HealthyDomains:      r.metrics.HealthyDomains,
		UnhealthyDomains:    r.metrics.UnhealthyDomains,
		TotalRequests:       r.metrics.TotalRequests,
		RequestsPerDomain:   make(map[string]int64),
		AverageResponseTime: r.metrics.AverageResponseTime,
		UptimeSeconds:       time.Since(r.createdAt).Milliseconds() / 1000,
		LastUpdated:         time.Now(),
	}

	// Copy per-domain request counts
	for domain, count := range r.metrics.RequestsPerDomain {
		metrics.RequestsPerDomain[domain] = count
	}

	return metrics
}

// Shutdown gracefully stops the registry and all background processes
func (r *Registry) Shutdown() {
	r.healthCancel()
}

// DomainInfo contains comprehensive information about a registered domain
type DomainInfo struct {
	Name        string          `json:"name"`
	Version     string          `json:"version"`
	Description string          `json:"description"`
	IsHealthy   bool            `json:"is_healthy"`
	Metadata    *DomainMetadata `json:"metadata"`
}

// recordUsage updates usage statistics for a domain (called asynchronously)
func (r *Registry) recordUsage(domainName string) {
	r.mu.Lock()
	defer r.mu.Unlock()

	// Update domain metadata
	if metadata, exists := r.domainMetadata[domainName]; exists {
		metadata.LastUsed = time.Now()
		metadata.UsageCount++
	}

	// Update registry metrics
	r.metrics.TotalRequests++
	r.metrics.RequestsPerDomain[domainName]++
}

// updateMetricsLocked updates registry metrics (must be called with lock held)
func (r *Registry) updateMetricsLocked() {
	r.metrics.TotalDomains = len(r.domains)
	r.metrics.HealthyDomains = 0
	r.metrics.UnhealthyDomains = 0

	for name, domain := range r.domains {
		if domain.IsHealthy() {
			r.metrics.HealthyDomains++
			r.domainMetadata[name].HealthStatus = "healthy"
		} else {
			r.metrics.UnhealthyDomains++
			r.domainMetadata[name].HealthStatus = "unhealthy"
		}
		r.domainMetadata[name].LastHealthCheck = time.Now()
	}

	r.metrics.LastUpdated = time.Now()
}

// startHealthMonitoring runs periodic health checks on all registered domains
func (r *Registry) startHealthMonitoring() {
	ticker := time.NewTicker(r.healthCheckInterval)
	defer ticker.Stop()

	for {
		select {
		case <-r.healthContext.Done():
			return
		case <-ticker.C:
			r.mu.Lock()
			r.updateMetricsLocked()
			r.mu.Unlock()
		}
	}
}
