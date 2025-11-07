package registry

import (
	"fmt"
	"sync"
	"testing"
	"time"
)

func TestNewRegistry(t *testing.T) {
	registry := NewRegistry()
	if registry == nil {
		t.Fatal("NewRegistry() returned nil")
	}

	if registry.domains == nil {
		t.Error("Expected domains map to be initialized")
	}

	if registry.domainMetadata == nil {
		t.Error("Expected domainMetadata map to be initialized")
	}

	if registry.metrics == nil {
		t.Error("Expected metrics to be initialized")
	}

	// Test default options
	if registry.healthCheckInterval != 30*time.Second {
		t.Errorf("Expected default health check interval 30s, got %v", registry.healthCheckInterval)
	}

	// Cleanup
	registry.Shutdown()
}

func TestNewRegistryWithOptions(t *testing.T) {
	opts := &RegistryOptions{
		HealthCheckInterval: 10 * time.Second,
		EnableHealthChecks:  false,
	}

	registry := NewRegistryWithOptions(opts)
	if registry == nil {
		t.Fatal("NewRegistryWithOptions() returned nil")
	}

	if registry.healthCheckInterval != 10*time.Second {
		t.Errorf("Expected health check interval 10s, got %v", registry.healthCheckInterval)
	}

	registry.Shutdown()
}

func TestNewRegistryWithOptions_NilOptions(t *testing.T) {
	registry := NewRegistryWithOptions(nil)
	if registry == nil {
		t.Fatal("NewRegistryWithOptions(nil) returned nil")
	}

	// Should use defaults
	if registry.healthCheckInterval != 30*time.Second {
		t.Errorf("Expected default health check interval 30s, got %v", registry.healthCheckInterval)
	}

	registry.Shutdown()
}

func TestRegistry_Register(t *testing.T) {
	registry := NewRegistry()
	defer registry.Shutdown()

	domain := NewMockDomain("test", "1.0.0")

	// Test successful registration
	err := registry.Register(domain)
	if err != nil {
		t.Errorf("Register() failed: %v", err)
	}

	// Verify domain was registered
	domains := registry.List()
	if len(domains) != 1 {
		t.Errorf("Expected 1 domain, got %d", len(domains))
	}
	if domains[0] != "test" {
		t.Errorf("Expected domain 'test', got %s", domains[0])
	}
}

func TestRegistry_Register_Errors(t *testing.T) {
	registry := NewRegistry()
	defer registry.Shutdown()

	tests := []struct {
		name    string
		domain  Domain
		wantErr string
	}{
		{
			name:    "Nil domain",
			domain:  nil,
			wantErr: "domain cannot be nil",
		},
		{
			name:    "Empty name",
			domain:  &MockDomain{name: "", version: "1.0.0", vocabulary: &Vocabulary{}},
			wantErr: "domain name cannot be empty",
		},
		{
			name:    "Empty version",
			domain:  &MockDomain{name: "test", version: "", vocabulary: &Vocabulary{}},
			wantErr: "domain version cannot be empty",
		},
		{
			name:    "Nil vocabulary",
			domain:  &MockDomain{name: "test", version: "1.0.0", vocabulary: nil},
			wantErr: "domain 'test' has no vocabulary",
		},
		{
			name: "Vocabulary domain name mismatch",
			domain: &MockDomain{
				name:    "test",
				version: "1.0.0",
				vocabulary: &Vocabulary{
					Domain: "other",
				},
			},
			wantErr: "domain name mismatch: expected 'test', got 'other'",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := registry.Register(tt.domain)
			if err == nil {
				t.Error("Expected error, got nil")
			} else if err.Error() != tt.wantErr {
				t.Errorf("Expected error %q, got %q", tt.wantErr, err.Error())
			}
		})
	}
}

func TestRegistry_Register_DuplicateName(t *testing.T) {
	registry := NewRegistry()
	defer registry.Shutdown()

	domain1 := NewMockDomain("test", "1.0.0")
	domain2 := NewMockDomain("test", "2.0.0")

	// Register first domain
	err := registry.Register(domain1)
	if err != nil {
		t.Fatalf("First registration failed: %v", err)
	}

	// Try to register duplicate
	err = registry.Register(domain2)
	if err == nil {
		t.Error("Expected error for duplicate registration")
	} else if err.Error() != "domain 'test' is already registered" {
		t.Errorf("Unexpected error: %v", err)
	}
}

func TestRegistry_Unregister(t *testing.T) {
	registry := NewRegistry()
	defer registry.Shutdown()

	domain := NewMockDomain("test", "1.0.0")
	registry.Register(domain)

	// Test successful unregistration
	err := registry.Unregister("test")
	if err != nil {
		t.Errorf("Unregister() failed: %v", err)
	}

	// Verify domain was removed
	domains := registry.List()
	if len(domains) != 0 {
		t.Errorf("Expected 0 domains after unregister, got %d", len(domains))
	}

	// Test unregistering non-existent domain
	err = registry.Unregister("nonexistent")
	if err == nil {
		t.Error("Expected error for unregistering non-existent domain")
	}
}

func TestRegistry_Get(t *testing.T) {
	registry := NewRegistry()
	defer registry.Shutdown()

	domain := NewMockDomain("test", "1.0.0")
	registry.Register(domain)

	// Test successful get
	retrieved, err := registry.Get("test")
	if err != nil {
		t.Errorf("Get() failed: %v", err)
	}
	if retrieved.Name() != "test" {
		t.Errorf("Expected domain name 'test', got %s", retrieved.Name())
	}

	// Test getting non-existent domain
	_, err = registry.Get("nonexistent")
	if err == nil {
		t.Error("Expected error for getting non-existent domain")
	}
}

func TestRegistry_List(t *testing.T) {
	registry := NewRegistry()
	defer registry.Shutdown()

	// Test empty registry
	domains := registry.List()
	if len(domains) != 0 {
		t.Errorf("Expected empty list, got %d domains", len(domains))
	}

	// Add domains
	domain1 := NewMockDomain("alpha", "1.0.0")
	domain2 := NewMockDomain("beta", "1.0.0")
	domain3 := NewMockDomain("gamma", "1.0.0")

	registry.Register(domain1)
	registry.Register(domain2)
	registry.Register(domain3)

	// Test list is sorted alphabetically
	domains = registry.List()
	expected := []string{"alpha", "beta", "gamma"}
	if len(domains) != len(expected) {
		t.Errorf("Expected %d domains, got %d", len(expected), len(domains))
	}

	for i, name := range domains {
		if name != expected[i] {
			t.Errorf("Expected domain %s at position %d, got %s", expected[i], i, name)
		}
	}
}

func TestRegistry_ListWithMetadata(t *testing.T) {
	registry := NewRegistry()
	defer registry.Shutdown()

	domain := NewMockDomain("test", "1.0.0")
	registry.Register(domain)

	// Allow some time for metadata to be set
	time.Sleep(10 * time.Millisecond)

	domainInfos := registry.ListWithMetadata()
	if len(domainInfos) != 1 {
		t.Errorf("Expected 1 domain info, got %d", len(domainInfos))
	}

	info, exists := domainInfos["test"]
	if !exists {
		t.Error("Expected 'test' domain info to exist")
	}

	if info.Name != "test" {
		t.Errorf("Expected name 'test', got %s", info.Name)
	}

	if info.Version != "1.0.0" {
		t.Errorf("Expected version '1.0.0', got %s", info.Version)
	}

	if info.Metadata == nil {
		t.Error("Expected metadata to be present")
	}
}

func TestRegistry_GetVocabulary(t *testing.T) {
	registry := NewRegistry()
	defer registry.Shutdown()

	domain := NewMockDomain("test", "1.0.0")
	registry.Register(domain)

	// Test successful vocabulary retrieval
	vocab, err := registry.GetVocabulary("test")
	if err != nil {
		t.Errorf("GetVocabulary() failed: %v", err)
	}
	if vocab.Domain != "test" {
		t.Errorf("Expected vocabulary domain 'test', got %s", vocab.Domain)
	}

	// Test getting vocabulary for non-existent domain
	_, err = registry.GetVocabulary("nonexistent")
	if err == nil {
		t.Error("Expected error for getting vocabulary of non-existent domain")
	}
}

func TestRegistry_GetAllVocabularies(t *testing.T) {
	registry := NewRegistry()
	defer registry.Shutdown()

	domain1 := NewMockDomain("test1", "1.0.0")
	domain2 := NewMockDomain("test2", "1.0.0")
	registry.Register(domain1)
	registry.Register(domain2)

	vocabularies := registry.GetAllVocabularies()
	if len(vocabularies) != 2 {
		t.Errorf("Expected 2 vocabularies, got %d", len(vocabularies))
	}

	if vocab1, exists := vocabularies["test1"]; !exists {
		t.Error("Expected 'test1' vocabulary to exist")
	} else if vocab1.Domain != "test1" {
		t.Errorf("Expected domain 'test1', got %s", vocab1.Domain)
	}

	if vocab2, exists := vocabularies["test2"]; !exists {
		t.Error("Expected 'test2' vocabulary to exist")
	} else if vocab2.Domain != "test2" {
		t.Errorf("Expected domain 'test2', got %s", vocab2.Domain)
	}
}

func TestRegistry_FindDomainsByVerb(t *testing.T) {
	registry := NewRegistry()
	defer registry.Shutdown()

	domain1 := NewMockDomain("test1", "1.0.0")
	domain2 := NewMockDomain("test2", "1.0.0")
	registry.Register(domain1)
	registry.Register(domain2)

	// Find domains that have the start verb
	domains := registry.FindDomainsByVerb("test1.start")
	if len(domains) != 1 {
		t.Errorf("Expected 1 domain for 'test1.start', got %d", len(domains))
	}
	if domains[0] != "test1" {
		t.Errorf("Expected 'test1', got %s", domains[0])
	}

	// Find non-existent verb
	domains = registry.FindDomainsByVerb("nonexistent.verb")
	if len(domains) != 0 {
		t.Errorf("Expected 0 domains for non-existent verb, got %d", len(domains))
	}
}

func TestRegistry_FindDomainsByCategory(t *testing.T) {
	registry := NewRegistry()
	defer registry.Shutdown()

	domain1 := NewMockDomain("test1", "1.0.0")
	domain2 := NewMockDomain("test2", "1.0.0")
	registry.Register(domain1)
	registry.Register(domain2)

	// Find domains that have the lifecycle category
	domains := registry.FindDomainsByCategory("lifecycle")
	if len(domains) != 2 {
		t.Errorf("Expected 2 domains for 'lifecycle' category, got %d", len(domains))
	}

	// Should be sorted alphabetically
	expected := []string{"test1", "test2"}
	for i, domain := range domains {
		if domain != expected[i] {
			t.Errorf("Expected domain %s at position %d, got %s", expected[i], i, domain)
		}
	}

	// Find non-existent category
	domains = registry.FindDomainsByCategory("nonexistent")
	if len(domains) != 0 {
		t.Errorf("Expected 0 domains for non-existent category, got %d", len(domains))
	}
}

func TestRegistry_IsHealthy(t *testing.T) {
	registry := NewRegistry()
	defer registry.Shutdown()

	// Empty registry should be healthy
	if !registry.IsHealthy() {
		t.Error("Empty registry should be healthy")
	}

	// Add healthy domain
	domain1 := NewMockDomain("test1", "1.0.0")
	registry.Register(domain1)

	if !registry.IsHealthy() {
		t.Error("Registry with healthy domain should be healthy")
	}

	// Add unhealthy domain
	domain2 := NewMockDomain("test2", "1.0.0")
	domain2.SetHealthy(false)
	registry.Register(domain2)

	if registry.IsHealthy() {
		t.Error("Registry with unhealthy domain should be unhealthy")
	}
}

func TestRegistry_GetMetrics(t *testing.T) {
	registry := NewRegistry()
	defer registry.Shutdown()

	domain := NewMockDomain("test", "1.0.0")
	registry.Register(domain)

	metrics := registry.GetMetrics()
	if metrics == nil {
		t.Fatal("Expected metrics, got nil")
	}

	if metrics.TotalDomains != 1 {
		t.Errorf("Expected 1 total domain, got %d", metrics.TotalDomains)
	}

	if metrics.UptimeSeconds < 0 {
		t.Error("Expected non-negative uptime")
	}

	// Test that we get a copy (not original)
	metrics.TotalDomains = 999
	newMetrics := registry.GetMetrics()
	if newMetrics.TotalDomains == 999 {
		t.Error("Expected to get a copy of metrics, not original")
	}
}

func TestRegistry_Shutdown(t *testing.T) {
	registry := NewRegistry()

	// Test that shutdown doesn't panic
	registry.Shutdown()

	// Test that we can call shutdown multiple times
	registry.Shutdown()
}

func TestRegistry_ThreadSafety(t *testing.T) {
	registry := NewRegistry()
	defer registry.Shutdown()

	const numGoroutines = 10
	const numOperations = 100

	var wg sync.WaitGroup
	wg.Add(numGoroutines * 3) // 3 types of operations

	// Concurrent registrations
	for i := 0; i < numGoroutines; i++ {
		go func(id int) {
			defer wg.Done()
			for j := 0; j < numOperations; j++ {
				domain := NewMockDomain(fmt.Sprintf("test-%d-%d", id, j), "1.0.0")
				registry.Register(domain) // May fail due to duplicates, that's OK
			}
		}(i)
	}

	// Concurrent reads
	for i := 0; i < numGoroutines; i++ {
		go func(id int) {
			defer wg.Done()
			for j := 0; j < numOperations; j++ {
				registry.List()
				registry.GetMetrics()
				registry.IsHealthy()
			}
		}(i)
	}

	// Concurrent gets
	for i := 0; i < numGoroutines; i++ {
		go func(id int) {
			defer wg.Done()
			for j := 0; j < numOperations; j++ {
				registry.Get(fmt.Sprintf("test-%d-%d", id, j)) // May fail, that's OK
			}
		}(i)
	}

	wg.Wait()

	// Registry should still be functional
	domains := registry.List()
	if len(domains) == 0 {
		t.Error("Registry appears to be corrupted after concurrent access - no domains found")
	}
}

func TestRegistry_UsageTracking(t *testing.T) {
	registry := NewRegistry()
	defer registry.Shutdown()

	domain := NewMockDomain("test", "1.0.0")
	registry.Register(domain)

	// Get domain multiple times to trigger usage tracking
	for i := 0; i < 5; i++ {
		registry.Get("test")
	}

	// Allow time for async usage recording
	time.Sleep(10 * time.Millisecond)

	metrics := registry.GetMetrics()
	if metrics.TotalRequests != 5 {
		t.Errorf("Expected 5 total requests, got %d", metrics.TotalRequests)
	}

	if count, exists := metrics.RequestsPerDomain["test"]; !exists {
		t.Error("Expected usage count for 'test' domain")
	} else if count != 5 {
		t.Errorf("Expected 5 requests for 'test' domain, got %d", count)
	}
}

func TestRegistry_HealthMonitoring(t *testing.T) {
	// Create registry with short health check interval
	opts := &RegistryOptions{
		HealthCheckInterval: 50 * time.Millisecond,
		EnableHealthChecks:  true,
	}
	registry := NewRegistryWithOptions(opts)
	defer registry.Shutdown()

	// Add a healthy domain
	domain := NewMockDomain("test", "1.0.0")
	registry.Register(domain)

	// Wait for at least one health check cycle
	time.Sleep(100 * time.Millisecond)

	metrics := registry.GetMetrics()
	if metrics.HealthyDomains != 1 {
		t.Errorf("Expected 1 healthy domain, got %d", metrics.HealthyDomains)
	}

	// Make domain unhealthy
	domain.SetHealthy(false)

	// Wait for health check to detect change
	time.Sleep(100 * time.Millisecond)

	metrics = registry.GetMetrics()
	if metrics.UnhealthyDomains != 1 {
		t.Errorf("Expected 1 unhealthy domain, got %d", metrics.UnhealthyDomains)
	}
}

func TestRegistry_HealthMonitoringDisabled(t *testing.T) {
	// Create registry with health monitoring disabled
	opts := &RegistryOptions{
		HealthCheckInterval: 10 * time.Millisecond,
		EnableHealthChecks:  false,
	}
	registry := NewRegistryWithOptions(opts)
	defer registry.Shutdown()

	domain := NewMockDomain("test", "1.0.0")
	registry.Register(domain)

	// Make domain unhealthy
	domain.SetHealthy(false)

	// Wait a bit
	time.Sleep(50 * time.Millisecond)

	// Health status should not be automatically updated since monitoring is disabled
	// We need to manually trigger an update by calling a method that updates metrics
	registry.IsHealthy() // This will trigger a manual health check

	// Now check if the unhealthy status was detected
	if registry.IsHealthy() {
		t.Error("Registry should detect unhealthy domain even with monitoring disabled")
	}
}

func TestDefaultRegistryOptions(t *testing.T) {
	opts := DefaultRegistryOptions()
	if opts == nil {
		t.Fatal("DefaultRegistryOptions() returned nil")
	}

	if opts.HealthCheckInterval != 30*time.Second {
		t.Errorf("Expected default health check interval 30s, got %v", opts.HealthCheckInterval)
	}

	if !opts.EnableHealthChecks {
		t.Error("Expected health checks to be enabled by default")
	}
}

func TestDomainMetadata_Updates(t *testing.T) {
	registry := NewRegistry()
	defer registry.Shutdown()

	domain := NewMockDomain("test", "1.0.0")
	registry.Register(domain)

	// Get domain to trigger usage update
	registry.Get("test")

	// Allow time for async update
	time.Sleep(10 * time.Millisecond)

	infos := registry.ListWithMetadata()
	info := infos["test"]

	if info.Metadata.UsageCount != 1 {
		t.Errorf("Expected usage count 1, got %d", info.Metadata.UsageCount)
	}

	if info.Metadata.LastUsed.IsZero() {
		t.Error("Expected LastUsed to be set")
	}

	if info.Metadata.RegisteredAt.IsZero() {
		t.Error("Expected RegisteredAt to be set")
	}
}

func TestRegistry_EmptyOperations(t *testing.T) {
	registry := NewRegistry()
	defer registry.Shutdown()

	// Test operations on empty registry
	domains := registry.List()
	if len(domains) != 0 {
		t.Errorf("Expected empty domain list, got %d domains", len(domains))
	}

	vocabularies := registry.GetAllVocabularies()
	if len(vocabularies) != 0 {
		t.Errorf("Expected empty vocabulary map, got %d vocabularies", len(vocabularies))
	}

	if !registry.IsHealthy() {
		t.Error("Empty registry should be considered healthy")
	}

	metrics := registry.GetMetrics()
	if metrics.TotalDomains != 0 {
		t.Errorf("Expected 0 total domains, got %d", metrics.TotalDomains)
	}
}
