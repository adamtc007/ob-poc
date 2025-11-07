package dsl_manager

import (
	"context"
	"fmt"
	"time"

	"github.com/google/uuid"

	"dsl-ob-poc/internal/datastore"
	"dsl-ob-poc/internal/store"
)

// DSLLifecycleState represents the lifecycle state of a DSL document
type DSLLifecycleState string

const (
	// DSL Lifecycle States - document progression
	DSLStateCreating  DSLLifecycleState = "CREATING"  // DSL being built/designed
	DSLStateReady     DSLLifecycleState = "READY"     // DSL complete and validated
	DSLStateExecuting DSLLifecycleState = "EXECUTING" // DSL being executed
	DSLStateExecuted  DSLLifecycleState = "EXECUTED"  // DSL execution completed
	DSLStateArchived  DSLLifecycleState = "ARCHIVED"  // DSL archived for compliance
	DSLStateFailed    DSLLifecycleState = "FAILED"    // DSL execution failed
	DSLStateSuspended DSLLifecycleState = "SUSPENDED" // DSL execution suspended
)

// DSLDomainState represents the business domain state
type DSLDomainState string

const (
	// Business Domain States - onboarding progression
	DomainStateCreated             DSLDomainState = "CREATED"
	DomainStateProductsAdded       DSLDomainState = "PRODUCTS_ADDED"
	DomainStateKYCDiscovered       DSLDomainState = "KYC_DISCOVERED"
	DomainStateServicesDiscovered  DSLDomainState = "SERVICES_DISCOVERED"
	DomainStateResourcesDiscovered DSLDomainState = "RESOURCES_DISCOVERED"
	DomainStateAttributesPopulated DSLDomainState = "ATTRIBUTES_POPULATED"
	DomainStateValuesBound         DSLDomainState = "VALUES_BOUND"
	DomainStateCompleted           DSLDomainState = "COMPLETED"
)

// DSLSnapshot represents a point-in-time DSL state capture
type DSLSnapshot struct {
	SnapshotID       string            `json:"snapshot_id"`
	OnboardingID     string            `json:"onboarding_id"`
	LifecycleState   DSLLifecycleState `json:"lifecycle_state"`
	DomainState      DSLDomainState    `json:"domain_state"`
	DSLContent       string            `json:"dsl_content"`
	VersionNumber    int               `json:"version_number"`
	Domain           string            `json:"domain"`
	CreatedAt        time.Time         `json:"created_at"`
	ExecutedAt       *time.Time        `json:"executed_at,omitempty"`
	ArchivedAt       *time.Time        `json:"archived_at,omitempty"`
	ExecutionResults map[string]any    `json:"execution_results,omitempty"`
	ValidationErrors []string          `json:"validation_errors,omitempty"`
}

// DSLLifecycleManager manages DSL document lifecycle and state transitions
type DSLLifecycleManager struct {
	dataStore datastore.DataStore
}

// NewDSLLifecycleManager creates a new DSL lifecycle manager
func NewDSLLifecycleManager(dataStore datastore.DataStore) *DSLLifecycleManager {
	return &DSLLifecycleManager{
		dataStore: dataStore,
	}
}

// CreateOnboardingProcess starts a new onboarding process with unique ID
func (lm *DSLLifecycleManager) CreateOnboardingProcess(domain string, cbuID string, initialData map[string]interface{}) (*DSLSnapshot, error) {
	onboardingID := uuid.New().String()

	// Generate initial DSL
	initialDSL, err := lm.generateInitialDSL(domain, initialData)
	if err != nil {
		return nil, fmt.Errorf("failed to generate initial DSL: %w", err)
	}

	// Create initial snapshot
	snapshot := &DSLSnapshot{
		SnapshotID:     uuid.New().String(),
		OnboardingID:   onboardingID,
		LifecycleState: DSLStateCreating,
		DomainState:    DomainStateCreated,
		DSLContent:     initialDSL,
		VersionNumber:  1,
		Domain:         domain,
		CreatedAt:      time.Now(),
	}

	// Store initial DSL with lifecycle state
	ctx := context.Background()
	versionID, err := lm.dataStore.InsertDSLWithState(ctx, cbuID, initialDSL, store.StateCreated)
	if err != nil {
		return nil, fmt.Errorf("failed to store initial DSL: %w", err)
	}

	snapshot.SnapshotID = versionID
	return snapshot, nil
}

// ExtendDSL adds new DSL fragment and updates domain state
func (lm *DSLLifecycleManager) ExtendDSL(onboardingID, cbuID string, dslFragment string, newDomainState DSLDomainState) (*DSLSnapshot, error) {
	ctx := context.Background()

	// Get current DSL
	currentDSL, err := lm.dataStore.GetLatestDSL(ctx, cbuID)
	if err != nil {
		return nil, fmt.Errorf("failed to get current DSL: %w", err)
	}

	// Validate domain state transition
	if err := lm.validateDomainStateTransition(currentDSL, newDomainState); err != nil {
		return nil, fmt.Errorf("invalid domain state transition: %w", err)
	}

	// Accumulate DSL (never replace)
	accumulatedDSL := currentDSL + "\n\n" + dslFragment

	// Determine lifecycle state
	lifecycleState := lm.determineLifecycleState(newDomainState)

	// Get next version number
	history, err := lm.dataStore.GetDSLHistoryWithState(ctx, cbuID)
	if err != nil {
		return nil, fmt.Errorf("failed to get DSL history: %w", err)
	}
	nextVersion := len(history) + 1

	// Create new snapshot
	snapshot := &DSLSnapshot{
		SnapshotID:     uuid.New().String(),
		OnboardingID:   onboardingID,
		LifecycleState: lifecycleState,
		DomainState:    newDomainState,
		DSLContent:     accumulatedDSL,
		VersionNumber:  nextVersion,
		CreatedAt:      time.Now(),
	}

	// Store extended DSL
	storeState := lm.mapDomainStateToStoreState(newDomainState)
	versionID, err := lm.dataStore.InsertDSLWithState(ctx, cbuID, accumulatedDSL, storeState)
	if err != nil {
		return nil, fmt.Errorf("failed to store extended DSL: %w", err)
	}

	snapshot.SnapshotID = versionID
	return snapshot, nil
}

// TransitionLifecycleState transitions DSL lifecycle state (READY → EXECUTING → EXECUTED)
func (lm *DSLLifecycleManager) TransitionLifecycleState(onboardingID, cbuID string, newLifecycleState DSLLifecycleState) (*DSLSnapshot, error) {
	ctx := context.Background()

	// Get current snapshot
	currentSnapshot, err := lm.GetCurrentSnapshot(onboardingID, cbuID)
	if err != nil {
		return nil, fmt.Errorf("failed to get current snapshot: %w", err)
	}

	// Validate lifecycle state transition
	if err := lm.validateLifecycleStateTransition(currentSnapshot.LifecycleState, newLifecycleState); err != nil {
		return nil, fmt.Errorf("invalid lifecycle state transition: %w", err)
	}

	// Create new snapshot with lifecycle state change
	snapshot := &DSLSnapshot{
		SnapshotID:     uuid.New().String(),
		OnboardingID:   onboardingID,
		LifecycleState: newLifecycleState,
		DomainState:    currentSnapshot.DomainState,
		DSLContent:     currentSnapshot.DSLContent,
		VersionNumber:  currentSnapshot.VersionNumber + 1,
		CreatedAt:      time.Now(),
	}

	// Update timestamps based on lifecycle state
	switch newLifecycleState {
	case DSLStateExecuted:
		now := time.Now()
		snapshot.ExecutedAt = &now
	case DSLStateArchived:
		now := time.Now()
		snapshot.ArchivedAt = &now
		if currentSnapshot.ExecutedAt != nil {
			snapshot.ExecutedAt = currentSnapshot.ExecutedAt
		}
	}

	// Store lifecycle state change
	storeState := lm.mapDomainStateToStoreState(currentSnapshot.DomainState)
	versionID, err := lm.dataStore.InsertDSLWithState(ctx, cbuID, currentSnapshot.DSLContent, storeState)
	if err != nil {
		return nil, fmt.Errorf("failed to store lifecycle state change: %w", err)
	}

	snapshot.SnapshotID = versionID
	return snapshot, nil
}

// GetCurrentSnapshot retrieves the current DSL snapshot
func (lm *DSLLifecycleManager) GetCurrentSnapshot(onboardingID, cbuID string) (*DSLSnapshot, error) {
	ctx := context.Background()

	dslWithState, err := lm.dataStore.GetLatestDSLWithState(ctx, cbuID)
	if err != nil {
		return nil, fmt.Errorf("failed to get latest DSL: %w", err)
	}

	// Map to snapshot format
	snapshot := &DSLSnapshot{
		SnapshotID:     dslWithState.VersionID,
		OnboardingID:   onboardingID,
		LifecycleState: lm.determineLifecycleState(lm.mapStoreStateToDomainState(dslWithState.OnboardingState)),
		DomainState:    lm.mapStoreStateToDomainState(dslWithState.OnboardingState),
		DSLContent:     dslWithState.DSLText,
		VersionNumber:  dslWithState.VersionNumber,
		CreatedAt:      dslWithState.CreatedAt,
	}

	return snapshot, nil
}

// GetSnapshotHistory retrieves all snapshots for an onboarding process
func (lm *DSLLifecycleManager) GetSnapshotHistory(onboardingID, cbuID string) ([]DSLSnapshot, error) {
	ctx := context.Background()

	history, err := lm.dataStore.GetDSLHistoryWithState(ctx, cbuID)
	if err != nil {
		return nil, fmt.Errorf("failed to get DSL history: %w", err)
	}

	var snapshots []DSLSnapshot
	for _, entry := range history {
		domainState := lm.mapStoreStateToDomainState(entry.OnboardingState)
		snapshot := DSLSnapshot{
			SnapshotID:     entry.VersionID,
			OnboardingID:   onboardingID,
			LifecycleState: lm.determineLifecycleState(domainState),
			DomainState:    domainState,
			DSLContent:     entry.DSLText,
			VersionNumber:  entry.VersionNumber,
			CreatedAt:      entry.CreatedAt,
		}
		snapshots = append(snapshots, snapshot)
	}

	return snapshots, nil
}

// validateLifecycleStateTransition ensures valid DSL lifecycle progression
func (lm *DSLLifecycleManager) validateLifecycleStateTransition(current, new DSLLifecycleState) error {
	validTransitions := map[DSLLifecycleState][]DSLLifecycleState{
		DSLStateCreating:  {DSLStateReady, DSLStateFailed},
		DSLStateReady:     {DSLStateExecuting, DSLStateArchived},
		DSLStateExecuting: {DSLStateExecuted, DSLStateFailed, DSLStateSuspended},
		DSLStateExecuted:  {DSLStateArchived},
		DSLStateFailed:    {DSLStateCreating, DSLStateArchived},
		DSLStateSuspended: {DSLStateExecuting, DSLStateArchived},
		DSLStateArchived:  {}, // Terminal state
	}

	allowedTransitions, exists := validTransitions[current]
	if !exists {
		return fmt.Errorf("invalid current lifecycle state: %s", current)
	}

	for _, allowed := range allowedTransitions {
		if allowed == new {
			return nil
		}
	}

	return fmt.Errorf("invalid lifecycle transition from %s to %s", current, new)
}

// validateDomainStateTransition ensures valid business domain progression
func (lm *DSLLifecycleManager) validateDomainStateTransition(currentDSL string, newState DSLDomainState) error {
	// Parse current DSL to determine current domain state
	// This is a simplified version - in practice, you'd parse the DSL

	// For now, assume progression is valid
	// In practice, parse DSL to determine current state and validate transitions
	return nil
}

// determineLifecycleState maps domain state to appropriate lifecycle state
func (lm *DSLLifecycleManager) determineLifecycleState(domainState DSLDomainState) DSLLifecycleState {
	switch domainState {
	case DomainStateCreated, DomainStateProductsAdded, DomainStateKYCDiscovered,
		DomainStateServicesDiscovered, DomainStateResourcesDiscovered,
		DomainStateAttributesPopulated:
		return DSLStateCreating
	case DomainStateValuesBound:
		return DSLStateReady
	case DomainStateCompleted:
		return DSLStateExecuted
	default:
		return DSLStateCreating
	}
}

// mapDomainStateToStoreState maps domain states to store states
func (lm *DSLLifecycleManager) mapDomainStateToStoreState(domainState DSLDomainState) store.OnboardingState {
	switch domainState {
	case DomainStateCreated:
		return store.StateCreated
	case DomainStateProductsAdded:
		return store.StateProductsAdded
	case DomainStateKYCDiscovered:
		return store.StateKYCDiscovered
	case DomainStateServicesDiscovered:
		return store.StateServicesDiscovered
	case DomainStateResourcesDiscovered:
		return store.StateResourcesDiscovered
	case DomainStateAttributesPopulated:
		return store.StateAttributesPopulated
	case DomainStateValuesBound, DomainStateCompleted:
		return store.StateCompleted
	default:
		return store.StateCreated
	}
}

// mapStoreStateToDomainState maps store states to domain states
func (lm *DSLLifecycleManager) mapStoreStateToDomainState(storeState store.OnboardingState) DSLDomainState {
	switch storeState {
	case store.StateCreated:
		return DomainStateCreated
	case store.StateProductsAdded:
		return DomainStateProductsAdded
	case store.StateKYCDiscovered:
		return DomainStateKYCDiscovered
	case store.StateServicesDiscovered:
		return DomainStateServicesDiscovered
	case store.StateResourcesDiscovered:
		return DomainStateResourcesDiscovered
	case store.StateAttributesPopulated:
		return DomainStateAttributesPopulated
	case store.StateCompleted:
		return DomainStateCompleted
	default:
		return DomainStateCreated
	}
}

// generateInitialDSL creates domain-specific initial DSL
func (lm *DSLLifecycleManager) generateInitialDSL(domain string, initialData map[string]interface{}) (string, error) {
	switch domain {
	case "custody":
		clientName, _ := initialData["client-name"].(string)
		cbuID, _ := initialData["cbu-id"].(string)
		if clientName == "" {
			clientName = "Unknown Client"
		}
		return fmt.Sprintf("(case.create\n  (cbu.id \"%s\")\n  (client.name \"%s\")\n  (domain \"custody\"))", cbuID, clientName), nil

	case "investor":
		name, _ := initialData["investor-name"].(string)
		investorType, _ := initialData["investor-type"].(string)
		if name == "" {
			name = "Unknown Investor"
		}
		if investorType == "" {
			investorType = "PROPER_PERSON"
		}
		return fmt.Sprintf("(investor.create\n  (name \"%s\")\n  (type \"%s\"))", name, investorType), nil

	default:
		return fmt.Sprintf("(case.create\n  (domain \"%s\"))", domain), nil
	}
}
