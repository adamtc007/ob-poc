package dsl_manager

import (
	"context"
	"fmt"
	"log"
	"strings"
	"sync"
	"time"

	"github.com/google/uuid"

	"dsl-ob-poc/internal/datastore"
	"dsl-ob-poc/internal/shared-dsl/session"
	"dsl-ob-poc/internal/store"
)

// OnboardingState represents the structured onboarding progression states
type OnboardingState string

const (
	// Core Onboarding State Machine - Sequential Progression
	StateOnboardingRequested     OnboardingState = "ONBOARDING_REQUESTED"      // Initial request created
	StateCBUAssociated           OnboardingState = "CBU_ASSOCIATED"            // CBU linked to onboarding
	StateProductsSelected        OnboardingState = "PRODUCTS_SELECTED"         // Products chosen
	StateServicesDiscovered      OnboardingState = "SERVICES_DISCOVERED"       // Business services mapped
	StateResourcesDiscovered     OnboardingState = "RESOURCES_DISCOVERED"      // Implementation resources identified
	StateDataDictionaryCreated   OnboardingState = "DATA_DICTIONARY_CREATED"   // Consolidated resource dictionaries
	StateAttributesPopulated     OnboardingState = "ATTRIBUTES_POPULATED"      // Attribute values resolved
	StateResourceLifecyclesReady OnboardingState = "RESOURCE_LIFECYCLES_READY" // Resource instances ready for creation
	StateResourcesProvisioned    OnboardingState = "RESOURCES_PROVISIONED"     // Actual resources created
	StateOnboardingCompleted     OnboardingState = "ONBOARDING_COMPLETED"      // Full onboarding complete
	StateOnboardingArchived      OnboardingState = "ONBOARDING_ARCHIVED"       // Archived for compliance

	// Error and Exception States
	StateOnboardingFailed    OnboardingState = "ONBOARDING_FAILED"    // Onboarding failed
	StateOnboardingSuspended OnboardingState = "ONBOARDING_SUSPENDED" // Temporarily suspended
)

// OnboardingProcess represents a complete onboarding process
type OnboardingProcess struct {
	OnboardingID   string            `json:"onboarding_id"`
	CBUID          string            `json:"cbu_id"`
	CurrentState   OnboardingState   `json:"current_state"`
	DSLLifecycle   DSLLifecycleState `json:"dsl_lifecycle"`
	AccumulatedDSL string            `json:"accumulated_dsl"`
	VersionNumber  int               `json:"version_number"`
	Domain         string            `json:"domain"`
	CreatedAt      time.Time         `json:"created_at"`
	UpdatedAt      time.Time         `json:"updated_at"`
	CompletedAt    *time.Time        `json:"completed_at,omitempty"`

	// Products and Services
	SelectedProducts    []string `json:"selected_products"`
	DiscoveredServices  []string `json:"discovered_services"`
	DiscoveredResources []string `json:"discovered_resources"`

	// Data Dictionary
	ConsolidatedDictionary map[string]interface{} `json:"consolidated_dictionary"`
	ResolvedAttributes     map[string]interface{} `json:"resolved_attributes"`

	// Resource Instances
	ProvisionedResources []ResourceInstance `json:"provisioned_resources"`
}

// ResourceInstance represents a provisioned resource
type ResourceInstance struct {
	InstanceID    string                 `json:"instance_id"`
	ResourceType  string                 `json:"resource_type"`
	Owner         string                 `json:"owner"`
	Status        string                 `json:"status"`
	Configuration map[string]interface{} `json:"configuration"`
	CreatedAt     time.Time              `json:"created_at"`
}

// DSLManager provides structured onboarding state machine management
type DSLManager struct {
	sessionManager *session.Manager
	dataStore      datastore.DataStore

	// Active onboarding processes
	processes map[string]*OnboardingProcess
	mu        sync.RWMutex
}

// NewDSLManager creates a new DSL Manager with clean state machine
func NewDSLManager(dataStore datastore.DataStore) *DSLManager {
	return &DSLManager{
		sessionManager: session.NewManager(),
		dataStore:      dataStore,
		processes:      make(map[string]*OnboardingProcess),
	}
}

// ============================================================================
// ONBOARDING STATE MACHINE - PHASE 1: INITIATION
// ============================================================================

// CreateOnboardingRequest initiates a new onboarding process with unique ID
func (m *DSLManager) CreateOnboardingRequest(domain string, clientName string, initiatorInfo map[string]interface{}) (*OnboardingProcess, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	// Generate unique onboarding ID
	onboardingID := fmt.Sprintf("OB-%s-%s",
		strings.ToUpper(domain),
		strings.ReplaceAll(uuid.New().String(), "-", "")[:8])

	// Create initial DSL
	initialDSL := fmt.Sprintf(`(onboarding.request.create
  (onboarding.id "%s")
  (domain "%s")
  (client.name "%s")
  (requested.at "%s")
  (initiated.by "%v"))`,
		onboardingID,
		domain,
		clientName,
		time.Now().Format(time.RFC3339),
		initiatorInfo,
	)

	// Create onboarding process
	process := &OnboardingProcess{
		OnboardingID:           onboardingID,
		CurrentState:           StateOnboardingRequested,
		DSLLifecycle:           DSLStateCreating,
		AccumulatedDSL:         initialDSL,
		VersionNumber:          1,
		Domain:                 domain,
		CreatedAt:              time.Now(),
		UpdatedAt:              time.Now(),
		SelectedProducts:       make([]string, 0),
		DiscoveredServices:     make([]string, 0),
		DiscoveredResources:    make([]string, 0),
		ConsolidatedDictionary: make(map[string]interface{}),
		ResolvedAttributes:     make(map[string]interface{}),
		ProvisionedResources:   make([]ResourceInstance, 0),
	}

	// Store in active processes
	m.processes[onboardingID] = process

	// Persist initial DSL to database
	ctx := context.Background()
	storeState := m.mapOnboardingStateToStoreState(StateOnboardingRequested)
	versionID, err := m.dataStore.InsertDSLWithState(ctx, onboardingID, initialDSL, storeState)
	if err != nil {
		delete(m.processes, onboardingID)
		return nil, fmt.Errorf("failed to persist initial DSL: %w", err)
	}

	log.Printf("✅ Onboarding Request Created: ID=%s, Domain=%s, Client=%s, VersionID=%s",
		onboardingID, domain, clientName, versionID)

	return process, nil
}

// ============================================================================
// ONBOARDING STATE MACHINE - PHASE 2: CBU ASSOCIATION
// ============================================================================

// AssociateCBU links a Client Business Unit to the onboarding process
func (m *DSLManager) AssociateCBU(onboardingID string, cbuID string, cbuInfo map[string]interface{}) (*OnboardingProcess, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	process, exists := m.processes[onboardingID]
	if !exists {
		return nil, fmt.Errorf("onboarding process not found: %s", onboardingID)
	}

	// Validate state transition
	if err := m.validateStateTransition(process.CurrentState, StateCBUAssociated); err != nil {
		return nil, fmt.Errorf("invalid state transition: %w", err)
	}

	// Generate CBU association DSL
	cbuDSL := fmt.Sprintf(`
(cbu.associate
  (cbu.id "%s")
  (association.type "PRIMARY")
  (cbu.details %v)
  (associated.at "%s"))`,
		cbuID,
		cbuInfo,
		time.Now().Format(time.RFC3339),
	)

	// Update process
	process.CBUID = cbuID
	process.CurrentState = StateCBUAssociated
	process.AccumulatedDSL += cbuDSL
	process.VersionNumber++
	process.UpdatedAt = time.Now()

	// Persist updated DSL
	ctx := context.Background()
	storeState := m.mapOnboardingStateToStoreState(StateCBUAssociated)
	_, err := m.dataStore.InsertDSLWithState(ctx, cbuID, process.AccumulatedDSL, storeState)
	if err != nil {
		return nil, fmt.Errorf("failed to persist CBU association: %w", err)
	}

	log.Printf("✅ CBU Associated: OnboardingID=%s, CBUID=%s, Version=%d",
		onboardingID, cbuID, process.VersionNumber)

	return process, nil
}

// ============================================================================
// ONBOARDING STATE MACHINE - PHASE 3: PRODUCT SELECTION
// ============================================================================

// SelectProducts adds products to the onboarding process with validation
func (m *DSLManager) SelectProducts(onboardingID string, products []string, selectionReason string) (*OnboardingProcess, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	process, exists := m.processes[onboardingID]
	if !exists {
		return nil, fmt.Errorf("onboarding process not found: %s", onboardingID)
	}

	// Validate state transition
	if err := m.validateStateTransition(process.CurrentState, StateProductsSelected); err != nil {
		return nil, fmt.Errorf("invalid state transition: %w", err)
	}

	// Generate products selection DSL
	productsArray := `"` + strings.Join(products, `" "`) + `"`
	productsDSL := fmt.Sprintf(`
(products.select
  (products [%s])
  (selection.reason "%s")
  (selected.at "%s"))`,
		productsArray,
		selectionReason,
		time.Now().Format(time.RFC3339),
	)

	// Update process
	process.SelectedProducts = products
	process.CurrentState = StateProductsSelected
	process.AccumulatedDSL += productsDSL
	process.VersionNumber++
	process.UpdatedAt = time.Now()

	// Persist updated DSL
	ctx := context.Background()
	storeState := m.mapOnboardingStateToStoreState(StateProductsSelected)
	_, err := m.dataStore.InsertDSLWithState(ctx, process.CBUID, process.AccumulatedDSL, storeState)
	if err != nil {
		return nil, fmt.Errorf("failed to persist product selection: %w", err)
	}

	log.Printf("✅ Products Selected: OnboardingID=%s, Products=%v, Version=%d",
		onboardingID, products, process.VersionNumber)

	return process, nil
}

// ============================================================================
// ONBOARDING STATE MACHINE - PHASE 4: SERVICE DISCOVERY
// ============================================================================

// DiscoverServices identifies and maps business services for selected products
func (m *DSLManager) DiscoverServices(onboardingID string) (*OnboardingProcess, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	process, exists := m.processes[onboardingID]
	if !exists {
		return nil, fmt.Errorf("onboarding process not found: %s", onboardingID)
	}

	// Validate state transition
	if err := m.validateStateTransition(process.CurrentState, StateServicesDiscovered); err != nil {
		return nil, fmt.Errorf("invalid state transition: %w", err)
	}

	// Discover services for each product
	ctx := context.Background()
	var allServices []string
	var servicesDSL strings.Builder

	servicesDSL.WriteString("\n(services.discover\n")

	for _, product := range process.SelectedProducts {
		// Get services for product from datastore
		productEntity, err := m.dataStore.GetProductByName(ctx, product)
		if err != nil {
			log.Printf("⚠️ Warning: Could not find product %s: %v", product, err)
			continue
		}

		services, err := m.dataStore.GetServicesForProduct(ctx, productEntity.ProductID)
		if err != nil {
			log.Printf("⚠️ Warning: Could not get services for product %s: %v", product, err)
			continue
		}

		servicesDSL.WriteString(fmt.Sprintf("  (for.product \"%s\"\n", product))
		for _, service := range services {
			servicesDSL.WriteString(fmt.Sprintf("    (service \"%s\")\n", service.Name))
			allServices = append(allServices, service.Name)
		}
		servicesDSL.WriteString("  )\n")
	}

	servicesDSL.WriteString(fmt.Sprintf("  (discovered.at \"%s\"))", time.Now().Format(time.RFC3339)))

	// Update process
	process.DiscoveredServices = allServices
	process.CurrentState = StateServicesDiscovered
	process.AccumulatedDSL += servicesDSL.String()
	process.VersionNumber++
	process.UpdatedAt = time.Now()

	// Persist updated DSL
	storeState := m.mapOnboardingStateToStoreState(StateServicesDiscovered)
	_, err := m.dataStore.InsertDSLWithState(ctx, process.CBUID, process.AccumulatedDSL, storeState)
	if err != nil {
		return nil, fmt.Errorf("failed to persist service discovery: %w", err)
	}

	log.Printf("✅ Services Discovered: OnboardingID=%s, Services=%v, Version=%d",
		onboardingID, allServices, process.VersionNumber)

	return process, nil
}

// ============================================================================
// ONBOARDING STATE MACHINE - PHASE 5: RESOURCE DISCOVERY
// ============================================================================

// DiscoverResources identifies implementation resources for discovered services
func (m *DSLManager) DiscoverResources(onboardingID string) (*OnboardingProcess, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	process, exists := m.processes[onboardingID]
	if !exists {
		return nil, fmt.Errorf("onboarding process not found: %s", onboardingID)
	}

	// Validate state transition
	if err := m.validateStateTransition(process.CurrentState, StateResourcesDiscovered); err != nil {
		return nil, fmt.Errorf("invalid state transition: %w", err)
	}

	// Discover resources for each service
	ctx := context.Background()
	var allResources []string
	var resourcesDSL strings.Builder

	resourcesDSL.WriteString("\n(resources.discover\n")

	for _, serviceName := range process.DiscoveredServices {
		// Get service entity
		service, err := m.dataStore.GetServiceByName(ctx, serviceName)
		if err != nil {
			log.Printf("⚠️ Warning: Could not find service %s: %v", serviceName, err)
			continue
		}

		// Get resources for service
		resources, err := m.dataStore.GetResourcesForService(ctx, service.ServiceID)
		if err != nil {
			log.Printf("⚠️ Warning: Could not get resources for service %s: %v", serviceName, err)
			continue
		}

		resourcesDSL.WriteString(fmt.Sprintf("  (for.service \"%s\"\n", serviceName))
		for _, resource := range resources {
			resourcesDSL.WriteString(fmt.Sprintf("    (resource \"%s\" (owner \"%s\") (type \"%s\"))\n",
				resource.Name, resource.Owner, resource.DictionaryGroup))
			allResources = append(allResources, resource.Name)
		}
		resourcesDSL.WriteString("  )\n")
	}

	resourcesDSL.WriteString(fmt.Sprintf("  (discovered.at \"%s\"))", time.Now().Format(time.RFC3339)))

	// Update process
	process.DiscoveredResources = allResources
	process.CurrentState = StateResourcesDiscovered
	process.AccumulatedDSL += resourcesDSL.String()
	process.VersionNumber++
	process.UpdatedAt = time.Now()

	// Persist updated DSL
	storeState := m.mapOnboardingStateToStoreState(StateResourcesDiscovered)
	_, err := m.dataStore.InsertDSLWithState(ctx, process.CBUID, process.AccumulatedDSL, storeState)
	if err != nil {
		return nil, fmt.Errorf("failed to persist resource discovery: %w", err)
	}

	log.Printf("✅ Resources Discovered: OnboardingID=%s, Resources=%v, Version=%d",
		onboardingID, allResources, process.VersionNumber)

	return process, nil
}

// ============================================================================
// ONBOARDING STATE MACHINE - PHASE 6: DATA DICTIONARY CONSOLIDATION
// ============================================================================

// CreateConsolidatedDataDictionary merges and deduplicates resource dictionaries
func (m *DSLManager) CreateConsolidatedDataDictionary(onboardingID string) (*OnboardingProcess, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	process, exists := m.processes[onboardingID]
	if !exists {
		return nil, fmt.Errorf("onboarding process not found: %s", onboardingID)
	}

	// Validate state transition
	if err := m.validateStateTransition(process.CurrentState, StateDataDictionaryCreated); err != nil {
		return nil, fmt.Errorf("invalid state transition: %w", err)
	}

	// Consolidate dictionaries from discovered resources
	ctx := context.Background()
	consolidatedDict := make(map[string]interface{})
	var dictDSL strings.Builder

	dictDSL.WriteString("\n(data.dictionary.consolidate\n")

	// Get unique dictionary groups from resources
	uniqueGroups := make(map[string]bool)
	for _, resourceName := range process.DiscoveredResources {
		// In a full implementation, we'd query the resource to get its dictionary group
		// For now, we'll simulate this
		dictGroup := fmt.Sprintf("%sDict", resourceName)
		if !uniqueGroups[dictGroup] {
			uniqueGroups[dictGroup] = true

			// Get attributes for this dictionary group
			attributes, err := m.dataStore.GetAttributesForDictionaryGroup(ctx, dictGroup)
			if err != nil {
				log.Printf("⚠️ Warning: Could not get attributes for group %s: %v", dictGroup, err)
				continue
			}

			dictDSL.WriteString(fmt.Sprintf("  (dictionary.group \"%s\"\n", dictGroup))
			for _, attr := range attributes {
				dictDSL.WriteString(fmt.Sprintf("    (attribute \"%s\" (type \"%s\") (domain \"%s\"))\n",
					attr.Name, attr.Mask, attr.Domain))

				// Add to consolidated dictionary
				consolidatedDict[attr.Name] = map[string]interface{}{
					"id":     attr.AttributeID,
					"type":   attr.Mask,
					"domain": attr.Domain,
					"group":  attr.GroupID,
				}
			}
			dictDSL.WriteString("  )\n")
		}
	}

	dictDSL.WriteString(fmt.Sprintf("  (consolidated.at \"%s\"))", time.Now().Format(time.RFC3339)))

	// Update process
	process.ConsolidatedDictionary = consolidatedDict
	process.CurrentState = StateDataDictionaryCreated
	process.AccumulatedDSL += dictDSL.String()
	process.VersionNumber++
	process.UpdatedAt = time.Now()

	// Persist updated DSL
	storeState := m.mapOnboardingStateToStoreState(StateDataDictionaryCreated)
	_, err := m.dataStore.InsertDSLWithState(ctx, process.CBUID, process.AccumulatedDSL, storeState)
	if err != nil {
		return nil, fmt.Errorf("failed to persist data dictionary consolidation: %w", err)
	}

	log.Printf("✅ Data Dictionary Consolidated: OnboardingID=%s, Attributes=%d, Version=%d",
		onboardingID, len(consolidatedDict), process.VersionNumber)

	return process, nil
}

// ============================================================================
// ONBOARDING STATE MACHINE - PHASE 7: ATTRIBUTE POPULATION
// ============================================================================

// PopulateAttributes resolves attribute values for the onboarding process
func (m *DSLManager) PopulateAttributes(onboardingID string, attributeValues map[string]interface{}) (*OnboardingProcess, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	process, exists := m.processes[onboardingID]
	if !exists {
		return nil, fmt.Errorf("onboarding process not found: %s", onboardingID)
	}

	// Validate state transition
	if err := m.validateStateTransition(process.CurrentState, StateAttributesPopulated); err != nil {
		return nil, fmt.Errorf("invalid state transition: %w", err)
	}

	// Generate attributes population DSL
	var attrDSL strings.Builder
	attrDSL.WriteString("\n(attributes.populate\n")

	for attrName, value := range attributeValues {
		attrDSL.WriteString(fmt.Sprintf("  (attribute \"%s\" (value \"%v\"))\n", attrName, value))
		process.ResolvedAttributes[attrName] = value
	}

	attrDSL.WriteString(fmt.Sprintf("  (populated.at \"%s\"))", time.Now().Format(time.RFC3339)))

	// Update process
	process.CurrentState = StateAttributesPopulated
	process.AccumulatedDSL += attrDSL.String()
	process.VersionNumber++
	process.UpdatedAt = time.Now()

	// Persist updated DSL
	ctx := context.Background()
	storeState := m.mapOnboardingStateToStoreState(StateAttributesPopulated)
	_, err := m.dataStore.InsertDSLWithState(ctx, process.CBUID, process.AccumulatedDSL, storeState)
	if err != nil {
		return nil, fmt.Errorf("failed to persist attribute population: %w", err)
	}

	log.Printf("✅ Attributes Populated: OnboardingID=%s, Attributes=%d, Version=%d",
		onboardingID, len(attributeValues), process.VersionNumber)

	return process, nil
}

// ============================================================================
// ONBOARDING STATE MACHINE - PHASE 8: RESOURCE LIFECYCLE READINESS
// ============================================================================

// PrepareResourceLifecycles prepares resource instances for creation
func (m *DSLManager) PrepareResourceLifecycles(onboardingID string) (*OnboardingProcess, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	process, exists := m.processes[onboardingID]
	if !exists {
		return nil, fmt.Errorf("onboarding process not found: %s", onboardingID)
	}

	// Validate state transition
	if err := m.validateStateTransition(process.CurrentState, StateResourceLifecyclesReady); err != nil {
		return nil, fmt.Errorf("invalid state transition: %w", err)
	}

	// Prepare resource lifecycle definitions
	var lifecycleDSL strings.Builder
	lifecycleDSL.WriteString("\n(resource.lifecycles.prepare\n")

	for _, resourceName := range process.DiscoveredResources {
		instanceID := fmt.Sprintf("%s-%s-%s", resourceName, process.OnboardingID[:8],
			strings.ToLower(uuid.New().String()[:8]))

		lifecycleDSL.WriteString(fmt.Sprintf("  (resource.lifecycle \"%s\"\n", resourceName))
		lifecycleDSL.WriteString(fmt.Sprintf("    (instance.id \"%s\")\n", instanceID))
		lifecycleDSL.WriteString("    (status \"READY_FOR_CREATION\")\n")
		lifecycleDSL.WriteString("    (configuration (from \"RESOLVED_ATTRIBUTES\"))\n")
		lifecycleDSL.WriteString("  )\n")
	}

	lifecycleDSL.WriteString(fmt.Sprintf("  (prepared.at \"%s\"))", time.Now().Format(time.RFC3339)))

	// Update process
	process.CurrentState = StateResourceLifecyclesReady
	process.AccumulatedDSL += lifecycleDSL.String()
	process.VersionNumber++
	process.UpdatedAt = time.Now()

	// Mark DSL as validated and ready for execution
	process.DSLLifecycle = DSLStateReady

	// Persist updated DSL
	ctx := context.Background()
	storeState := m.mapOnboardingStateToStoreState(StateResourceLifecyclesReady)
	_, err := m.dataStore.InsertDSLWithState(ctx, process.CBUID, process.AccumulatedDSL, storeState)
	if err != nil {
		return nil, fmt.Errorf("failed to persist resource lifecycle preparation: %w", err)
	}

	log.Printf("✅ Resource Lifecycles Ready: OnboardingID=%s, DSLState=%s, Version=%d",
		onboardingID, process.DSLLifecycle, process.VersionNumber)

	return process, nil
}

// ============================================================================
// ONBOARDING STATE MACHINE - PHASE 9: RESOURCE PROVISIONING
// ============================================================================

// ProvisionResources creates actual resource instances
func (m *DSLManager) ProvisionResources(onboardingID string) (*OnboardingProcess, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	process, exists := m.processes[onboardingID]
	if !exists {
		return nil, fmt.Errorf("onboarding process not found: %s", onboardingID)
	}

	// Validate state transition
	if err := m.validateStateTransition(process.CurrentState, StateResourcesProvisioned); err != nil {
		return nil, fmt.Errorf("invalid state transition: %w", err)
	}

	// Mark DSL as executing
	process.DSLLifecycle = DSLStateExecuting

	// Provision each resource
	var provisionDSL strings.Builder
	provisionDSL.WriteString("\n(resources.provision\n")

	for _, resourceName := range process.DiscoveredResources {
		instanceID := fmt.Sprintf("%s-%s", resourceName, process.OnboardingID[:8])

		// Create resource instance
		instance := ResourceInstance{
			InstanceID:   instanceID,
			ResourceType: resourceName,
			Owner:        fmt.Sprintf("%sOwner", resourceName), // Would be looked up from resource definition
			Status:       "PROVISIONED",
			Configuration: map[string]interface{}{
				"onboarding_id": process.OnboardingID,
				"cbu_id":        process.CBUID,
				"created_for":   process.SelectedProducts,
			},
			CreatedAt: time.Now(),
		}

		process.ProvisionedResources = append(process.ProvisionedResources, instance)

		provisionDSL.WriteString(fmt.Sprintf("  (resource.provision \"%s\"\n", resourceName))
		provisionDSL.WriteString(fmt.Sprintf("    (instance.id \"%s\")\n", instanceID))
		provisionDSL.WriteString("    (status \"PROVISIONED\")\n")
		provisionDSL.WriteString("    (owner \"" + instance.Owner + "\")\n")
		provisionDSL.WriteString("  )\n")
	}

	provisionDSL.WriteString(fmt.Sprintf("  (provisioned.at \"%s\"))", time.Now().Format(time.RFC3339)))

	// Update process
	process.CurrentState = StateResourcesProvisioned
	process.DSLLifecycle = DSLStateExecuted
	process.AccumulatedDSL += provisionDSL.String()
	process.VersionNumber++
	process.UpdatedAt = time.Now()

	// Persist updated DSL
	ctx := context.Background()
	storeState := m.mapOnboardingStateToStoreState(StateResourcesProvisioned)
	_, err := m.dataStore.InsertDSLWithState(ctx, process.CBUID, process.AccumulatedDSL, storeState)
	if err != nil {
		return nil, fmt.Errorf("failed to persist resource provisioning: %w", err)
	}

	log.Printf("✅ Resources Provisioned: OnboardingID=%s, Resources=%d, Version=%d",
		onboardingID, len(process.ProvisionedResources), process.VersionNumber)

	return process, nil
}

// ============================================================================
// ONBOARDING STATE MACHINE - PHASE 10: COMPLETION
// ============================================================================

// CompleteOnboarding finalizes the onboarding process
func (m *DSLManager) CompleteOnboarding(onboardingID string, completionNotes string) (*OnboardingProcess, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	process, exists := m.processes[onboardingID]
	if !exists {
		return nil, fmt.Errorf("onboarding process not found: %s", onboardingID)
	}

	// Validate state transition
	if err := m.validateStateTransition(process.CurrentState, StateOnboardingCompleted); err != nil {
		return nil, fmt.Errorf("invalid state transition: %w", err)
	}

	// Generate completion DSL
	completionDSL := fmt.Sprintf(`
(onboarding.complete
  (completion.status "SUCCESS")
  (completion.notes "%s")
  (resources.provisioned %d)
  (services.activated %d)
  (products.enabled %v)
  (completed.at "%s"))`,
		completionNotes,
		len(process.ProvisionedResources),
		len(process.DiscoveredServices),
		process.SelectedProducts,
		time.Now().Format(time.RFC3339),
	)

	// Update process
	process.CurrentState = StateOnboardingCompleted
	process.AccumulatedDSL += completionDSL
	process.VersionNumber++
	process.UpdatedAt = time.Now()
	now := time.Now()
	process.CompletedAt = &now

	// Persist final DSL
	ctx := context.Background()
	storeState := m.mapOnboardingStateToStoreState(StateOnboardingCompleted)
	_, err := m.dataStore.InsertDSLWithState(ctx, process.CBUID, process.AccumulatedDSL, storeState)
	if err != nil {
		return nil, fmt.Errorf("failed to persist onboarding completion: %w", err)
	}

	log.Printf("✅ Onboarding Completed: OnboardingID=%s, Version=%d, CompletedAt=%s",
		onboardingID, process.VersionNumber, process.CompletedAt.Format(time.RFC3339))

	return process, nil
}

// ArchiveOnboarding moves completed onboarding to archived state
func (m *DSLManager) ArchiveOnboarding(onboardingID string, archivalReason string) (*OnboardingProcess, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	process, exists := m.processes[onboardingID]
	if !exists {
		return nil, fmt.Errorf("onboarding process not found: %s", onboardingID)
	}

	// Validate state transition
	if err := m.validateStateTransition(process.CurrentState, StateOnboardingArchived); err != nil {
		return nil, fmt.Errorf("invalid state transition: %w", err)
	}

	// Generate archival DSL
	archivalDSL := fmt.Sprintf(`
(onboarding.archive
  (archival.reason "%s")
  (retention.period "7_YEARS")
  (compliance.status "COMPLIANT")
  (archived.at "%s"))`,
		archivalReason,
		time.Now().Format(time.RFC3339),
	)

	// Update process
	process.CurrentState = StateOnboardingArchived
	process.DSLLifecycle = DSLStateArchived
	process.AccumulatedDSL += archivalDSL
	process.VersionNumber++
	process.UpdatedAt = time.Now()

	// Persist archived DSL
	ctx := context.Background()
	storeState := m.mapOnboardingStateToStoreState(StateOnboardingArchived)
	_, err := m.dataStore.InsertDSLWithState(ctx, process.CBUID, process.AccumulatedDSL, storeState)
	if err != nil {
		return nil, fmt.Errorf("failed to persist onboarding archival: %w", err)
	}

	log.Printf("✅ Onboarding Archived: OnboardingID=%s, Reason=%s, Version=%d",
		onboardingID, archivalReason, process.VersionNumber)

	return process, nil
}

// ============================================================================
// ONBOARDING STATE MACHINE - UTILITY METHODS
// ============================================================================

// GetOnboardingProcess retrieves an onboarding process by ID
func (m *DSLManager) GetOnboardingProcess(onboardingID string) (*OnboardingProcess, error) {
	m.mu.RLock()
	defer m.mu.RUnlock()

	process, exists := m.processes[onboardingID]
	if !exists {
		return nil, fmt.Errorf("onboarding process not found: %s", onboardingID)
	}

	return process, nil
}

// ListOnboardingProcesses returns all active onboarding processes
func (m *DSLManager) ListOnboardingProcesses() []*OnboardingProcess {
	m.mu.RLock()
	defer m.mu.RUnlock()

	processes := make([]*OnboardingProcess, 0, len(m.processes))
	for _, process := range m.processes {
		processes = append(processes, process)
	}
	return processes
}

// GetProcessesByState returns all processes in a specific state
func (m *DSLManager) GetProcessesByState(state OnboardingState) []*OnboardingProcess {
	m.mu.RLock()
	defer m.mu.RUnlock()

	var processes []*OnboardingProcess
	for _, process := range m.processes {
		if process.CurrentState == state {
			processes = append(processes, process)
		}
	}
	return processes
}

// ValidateStateTransition validates if a state transition is allowed
func (m *DSLManager) validateStateTransition(currentState, newState OnboardingState) error {
	validTransitions := map[OnboardingState][]OnboardingState{
		StateOnboardingRequested:     {StateCBUAssociated, StateOnboardingFailed},
		StateCBUAssociated:           {StateProductsSelected, StateOnboardingFailed},
		StateProductsSelected:        {StateServicesDiscovered, StateOnboardingFailed},
		StateServicesDiscovered:      {StateResourcesDiscovered, StateOnboardingFailed},
		StateResourcesDiscovered:     {StateDataDictionaryCreated, StateOnboardingFailed},
		StateDataDictionaryCreated:   {StateAttributesPopulated, StateOnboardingFailed},
		StateAttributesPopulated:     {StateResourceLifecyclesReady, StateOnboardingFailed},
		StateResourceLifecyclesReady: {StateResourcesProvisioned, StateOnboardingFailed, StateOnboardingSuspended},
		StateResourcesProvisioned:    {StateOnboardingCompleted, StateOnboardingFailed},
		StateOnboardingCompleted:     {StateOnboardingArchived},
		StateOnboardingFailed:        {StateOnboardingRequested, StateOnboardingArchived},   // Can restart or archive
		StateOnboardingSuspended:     {StateResourceLifecyclesReady, StateOnboardingFailed}, // Can resume or fail
		StateOnboardingArchived:      {},                                                    // Terminal state
	}

	allowedTransitions, exists := validTransitions[currentState]
	if !exists {
		return fmt.Errorf("invalid current state: %s", currentState)
	}

	for _, allowed := range allowedTransitions {
		if allowed == newState {
			return nil
		}
	}

	return fmt.Errorf("invalid transition from %s to %s", currentState, newState)
}

// mapOnboardingStateToStoreState maps internal states to store states
func (m *DSLManager) mapOnboardingStateToStoreState(state OnboardingState) store.OnboardingState {
	switch state {
	case StateOnboardingRequested:
		return store.StateCreated
	case StateCBUAssociated:
		return store.StateCreated
	case StateProductsSelected:
		return store.StateProductsAdded
	case StateServicesDiscovered:
		return store.StateServicesDiscovered
	case StateResourcesDiscovered:
		return store.StateResourcesDiscovered
	case StateDataDictionaryCreated:
		return store.StateResourcesDiscovered
	case StateAttributesPopulated:
		return store.StateAttributesPopulated
	case StateResourceLifecyclesReady:
		return store.StateAttributesPopulated
	case StateResourcesProvisioned:
		return store.StateCompleted
	case StateOnboardingCompleted:
		return store.StateCompleted
	case StateOnboardingArchived:
		return store.StateCompleted
	default:
		return store.StateCreated
	}
}

// GetStateTransitionPath returns the sequence of states from current to target
func (m *DSLManager) GetStateTransitionPath(currentState, targetState OnboardingState) ([]OnboardingState, error) {
	// Define the standard onboarding path
	standardPath := []OnboardingState{
		StateOnboardingRequested,
		StateCBUAssociated,
		StateProductsSelected,
		StateServicesDiscovered,
		StateResourcesDiscovered,
		StateDataDictionaryCreated,
		StateAttributesPopulated,
		StateResourceLifecyclesReady,
		StateResourcesProvisioned,
		StateOnboardingCompleted,
		StateOnboardingArchived,
	}

	// Find indices
	currentIndex := -1
	targetIndex := -1
	for i, state := range standardPath {
		if state == currentState {
			currentIndex = i
		}
		if state == targetState {
			targetIndex = i
		}
	}

	if currentIndex == -1 {
		return nil, fmt.Errorf("current state not found in standard path: %s", currentState)
	}
	if targetIndex == -1 {
		return nil, fmt.Errorf("target state not found in standard path: %s", targetState)
	}
	if targetIndex <= currentIndex {
		return nil, fmt.Errorf("target state must be after current state")
	}

	// Return the path from current to target
	return standardPath[currentIndex : targetIndex+1], nil
}

// ExecuteFullOnboarding runs the complete onboarding sequence
func (m *DSLManager) ExecuteFullOnboarding(domain, clientName, cbuID string, products []string, attributeValues map[string]interface{}) (*OnboardingProcess, error) {
	log.Printf("🚀 Starting Full Onboarding: Client=%s, Domain=%s, Products=%v", clientName, domain, products)

	// Phase 1: Create Request
	process, err := m.CreateOnboardingRequest(domain, clientName, map[string]interface{}{
		"automated": true,
		"full_flow": true,
	})
	if err != nil {
		return nil, fmt.Errorf("failed to create onboarding request: %w", err)
	}

	// Phase 2: Associate CBU
	process, err = m.AssociateCBU(process.OnboardingID, cbuID, map[string]interface{}{
		"association_method": "AUTOMATED",
	})
	if err != nil {
		return nil, fmt.Errorf("failed to associate CBU: %w", err)
	}

	// Phase 3: Select Products
	process, err = m.SelectProducts(process.OnboardingID, products, "AUTOMATED_SELECTION")
	if err != nil {
		return nil, fmt.Errorf("failed to select products: %w", err)
	}

	// Phase 4: Discover Services
	process, err = m.DiscoverServices(process.OnboardingID)
	if err != nil {
		return nil, fmt.Errorf("failed to discover services: %w", err)
	}

	// Phase 5: Discover Resources
	process, err = m.DiscoverResources(process.OnboardingID)
	if err != nil {
		return nil, fmt.Errorf("failed to discover resources: %w", err)
	}

	// Phase 6: Consolidate Data Dictionary
	process, err = m.CreateConsolidatedDataDictionary(process.OnboardingID)
	if err != nil {
		return nil, fmt.Errorf("failed to consolidate data dictionary: %w", err)
	}

	// Phase 7: Populate Attributes
	process, err = m.PopulateAttributes(process.OnboardingID, attributeValues)
	if err != nil {
		return nil, fmt.Errorf("failed to populate attributes: %w", err)
	}

	// Phase 8: Prepare Resource Lifecycles
	process, err = m.PrepareResourceLifecycles(process.OnboardingID)
	if err != nil {
		return nil, fmt.Errorf("failed to prepare resource lifecycles: %w", err)
	}

	// Phase 9: Provision Resources
	process, err = m.ProvisionResources(process.OnboardingID)
	if err != nil {
		return nil, fmt.Errorf("failed to provision resources: %w", err)
	}

	// Phase 10: Complete Onboarding
	process, err = m.CompleteOnboarding(process.OnboardingID, "AUTOMATED_FULL_ONBOARDING_COMPLETED")
	if err != nil {
		return nil, fmt.Errorf("failed to complete onboarding: %w", err)
	}

	log.Printf("✅ Full Onboarding Completed: OnboardingID=%s, Resources=%d, Services=%d, FinalVersion=%d",
		process.OnboardingID, len(process.ProvisionedResources), len(process.DiscoveredServices), process.VersionNumber)

	return process, nil
}
