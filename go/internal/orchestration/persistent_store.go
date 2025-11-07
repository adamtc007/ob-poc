// Package orchestration provides persistent storage for orchestration sessions.
//
// This file implements the PersistentOrchestrationStore that saves orchestration
// sessions to the database, enabling sessions to persist across CLI invocations
// and system restarts. This is a key component for making the orchestration
// system production-ready.
//
// Key Features:
// - Database-backed session persistence with PostgreSQL
// - Cross-invocation session continuity
// - Session expiration and cleanup
// - Atomic session updates with optimistic locking
// - Full DSL accumulation history preservation
//
// Architecture Pattern: Repository Pattern + DSL-as-State
// The store maintains the DSL-as-State pattern while providing durable persistence.
package orchestration

import (
	"context"
	"encoding/json"
	"time"

	"dsl-ob-poc/internal/datastore"
	"dsl-ob-poc/internal/store"
)

// PersistentOrchestrationStore provides database-backed orchestration session storage
type PersistentOrchestrationStore struct {
	dataStore datastore.DataStore
}

// NewPersistentOrchestrationStore creates a new persistent orchestration store
func NewPersistentOrchestrationStore(dataStore datastore.DataStore) *PersistentOrchestrationStore {
	return &PersistentOrchestrationStore{
		dataStore: dataStore,
	}
}

// SaveSession persists an orchestration session to the database
func (s *PersistentOrchestrationStore) SaveSession(ctx context.Context, session *OrchestrationSession) error {
	session.mu.RLock()
	defer session.mu.RUnlock()

	// Convert OrchestrationSession to OrchestrationSessionData
	sessionData := s.convertToSessionData(session)

	return s.dataStore.SaveOrchestrationSession(ctx, sessionData)
}

// LoadSession retrieves an orchestration session from the database
func (s *PersistentOrchestrationStore) LoadSession(ctx context.Context, sessionID string) (*OrchestrationSession, error) {
	sessionData, err := s.dataStore.LoadOrchestrationSession(ctx, sessionID)
	if err != nil {
		return nil, err
	}

	// Convert OrchestrationSessionData back to OrchestrationSession
	return s.convertFromSessionData(sessionData), nil
}

// ListActiveSessions returns IDs of all active (non-expired) sessions
func (s *PersistentOrchestrationStore) ListActiveSessions(ctx context.Context) ([]string, error) {
	return s.dataStore.ListActiveOrchestrationSessions(ctx)
}

// DeleteSession removes a session and all its related data
func (s *PersistentOrchestrationStore) DeleteSession(ctx context.Context, sessionID string) error {
	return s.dataStore.DeleteOrchestrationSession(ctx, sessionID)
}

// CleanupExpiredSessions removes sessions that have passed their expiration time
func (s *PersistentOrchestrationStore) CleanupExpiredSessions(ctx context.Context) (int64, error) {
	return s.dataStore.CleanupExpiredOrchestrationSessions(ctx)
}

// UpdateSessionDSL updates the unified DSL and version for a session
func (s *PersistentOrchestrationStore) UpdateSessionDSL(ctx context.Context, sessionID, dsl string, version int) error {
	return s.dataStore.UpdateOrchestrationSessionDSL(ctx, sessionID, dsl, version)
}

// Helper methods for converting between OrchestrationSession and OrchestrationSessionData

func (s *PersistentOrchestrationStore) convertToSessionData(session *OrchestrationSession) *store.OrchestrationSessionData {
	sessionData := &store.OrchestrationSessionData{
		SessionID:     session.SessionID,
		PrimaryDomain: session.PrimaryDomain,
		CurrentState:  session.CurrentState,
		VersionNumber: session.VersionNumber,
		UnifiedDSL:    session.UnifiedDSL,
		EntityRefs:    session.EntityRefs,
		AttributeRefs: session.AttributeRefs,
		CreatedAt:     session.CreatedAt,
		UpdatedAt:     time.Now(),
		LastUsed:      session.LastUsed,
	}

	// Convert SharedContext to map
	if session.SharedContext != nil {
		sessionData.SharedContext = map[string]interface{}{
			"cbu_id":          session.SharedContext.CBUID,
			"investor_id":     session.SharedContext.InvestorID,
			"fund_id":         session.SharedContext.FundID,
			"entity_id":       session.SharedContext.EntityID,
			"entity_type":     session.SharedContext.EntityType,
			"entity_name":     session.SharedContext.EntityName,
			"jurisdiction":    session.SharedContext.Jurisdiction,
			"products":        session.SharedContext.Products,
			"services":        session.SharedContext.Services,
			"workflow_type":   session.SharedContext.WorkflowType,
			"risk_profile":    session.SharedContext.RiskProfile,
			"compliance_tier": session.SharedContext.ComplianceTier,
		}

		// Set pointer fields
		if session.SharedContext.CBUID != "" {
			sessionData.CBUID = &session.SharedContext.CBUID
		}
		if session.SharedContext.EntityType != "" {
			sessionData.EntityType = &session.SharedContext.EntityType
		}
		if session.SharedContext.EntityName != "" {
			sessionData.EntityName = &session.SharedContext.EntityName
		}
		if session.SharedContext.Jurisdiction != "" {
			sessionData.Jurisdiction = &session.SharedContext.Jurisdiction
		}
		if session.SharedContext.WorkflowType != "" {
			sessionData.WorkflowType = &session.SharedContext.WorkflowType
		}

		sessionData.Products = session.SharedContext.Products
		sessionData.Services = session.SharedContext.Services
	}

	// Convert ExecutionPlan to map
	if session.ExecutionPlan != nil {
		planData, _ := json.Marshal(session.ExecutionPlan)
		json.Unmarshal(planData, &sessionData.ExecutionPlan)
	}

	// Convert DomainSessions
	for domainName, domainSession := range session.ActiveDomains {
		domainData := store.DomainSessionData{
			DomainName:      domainName,
			DomainSessionID: domainSession.SessionID,
			State:           domainSession.State,
			ContributedDSL:  domainSession.ContributedDSL,
			Context:         domainSession.Context,
			Dependencies:    domainSession.Dependencies,
			LastActivity:    domainSession.LastActivity,
		}
		sessionData.DomainSessions = append(sessionData.DomainSessions, domainData)
	}

	// Convert StateHistory
	for _, transition := range session.StateHistory {
		transitionData := store.StateTransitionData{
			FromState:   transition.FromState,
			ToState:     transition.ToState,
			Domain:      transition.Domain,
			Reason:      transition.Reason,
			GeneratedBy: transition.GeneratedBy,
			Timestamp:   transition.Timestamp,
		}
		sessionData.StateHistory = append(sessionData.StateHistory, transitionData)
	}

	return sessionData
}

func (s *PersistentOrchestrationStore) convertFromSessionData(sessionData *store.OrchestrationSessionData) *OrchestrationSession {
	session := &OrchestrationSession{
		SessionID:     sessionData.SessionID,
		PrimaryDomain: sessionData.PrimaryDomain,
		CurrentState:  sessionData.CurrentState,
		VersionNumber: sessionData.VersionNumber,
		UnifiedDSL:    sessionData.UnifiedDSL,
		EntityRefs:    sessionData.EntityRefs,
		AttributeRefs: sessionData.AttributeRefs,
		CreatedAt:     sessionData.CreatedAt,
		LastUsed:      sessionData.LastUsed,
		DomainDSL:     make(map[string]string),
		ActiveDomains: make(map[string]*DomainSession),
		PendingTasks:  make([]OrchestrationTask, 0),
	}

	// Convert SharedContext
	sharedContext := &SharedContext{
		AttributeValues: make(map[string]interface{}),
		Data:            make(map[string]interface{}),
	}

	if sessionData.CBUID != nil {
		sharedContext.CBUID = *sessionData.CBUID
	}
	if sessionData.EntityType != nil {
		sharedContext.EntityType = *sessionData.EntityType
	}
	if sessionData.EntityName != nil {
		sharedContext.EntityName = *sessionData.EntityName
	}
	if sessionData.Jurisdiction != nil {
		sharedContext.Jurisdiction = *sessionData.Jurisdiction
	}
	if sessionData.WorkflowType != nil {
		sharedContext.WorkflowType = *sessionData.WorkflowType
	}

	sharedContext.Products = sessionData.Products
	sharedContext.Services = sessionData.Services

	// Extract additional fields from SharedContext map
	if sessionData.SharedContext != nil {
		if investorID, ok := sessionData.SharedContext["investor_id"].(string); ok {
			sharedContext.InvestorID = investorID
		}
		if fundID, ok := sessionData.SharedContext["fund_id"].(string); ok {
			sharedContext.FundID = fundID
		}
		if entityID, ok := sessionData.SharedContext["entity_id"].(string); ok {
			sharedContext.EntityID = entityID
		}
		if riskProfile, ok := sessionData.SharedContext["risk_profile"].(string); ok {
			sharedContext.RiskProfile = riskProfile
		}
		if complianceTier, ok := sessionData.SharedContext["compliance_tier"].(string); ok {
			sharedContext.ComplianceTier = complianceTier
		}
	}

	session.SharedContext = sharedContext

	// Convert ExecutionPlan
	if len(sessionData.ExecutionPlan) > 0 {
		planData, _ := json.Marshal(sessionData.ExecutionPlan)
		var executionPlan ExecutionPlan
		json.Unmarshal(planData, &executionPlan)
		session.ExecutionPlan = &executionPlan
	}

	// Convert DomainSessions
	for _, domainData := range sessionData.DomainSessions {
		domainSession := &DomainSession{
			Domain:         domainData.DomainName,
			SessionID:      domainData.DomainSessionID,
			State:          domainData.State,
			ContributedDSL: domainData.ContributedDSL,
			Context:        domainData.Context,
			Dependencies:   domainData.Dependencies,
			LastActivity:   domainData.LastActivity,
		}
		session.ActiveDomains[domainData.DomainName] = domainSession
		session.DomainDSL[domainData.DomainName] = domainData.ContributedDSL
	}

	// Convert StateHistory
	for _, transitionData := range sessionData.StateHistory {
		transition := StateTransition{
			FromState:   transitionData.FromState,
			ToState:     transitionData.ToState,
			Domain:      transitionData.Domain,
			Reason:      transitionData.Reason,
			GeneratedBy: transitionData.GeneratedBy,
			Timestamp:   transitionData.Timestamp,
		}
		session.StateHistory = append(session.StateHistory, transition)
	}

	return session
}

// Helper functions

// SessionStoreMetrics provides metrics about stored sessions
type SessionStoreMetrics struct {
	TotalSessions        int64     `json:"total_sessions"`
	ActiveSessions       int64     `json:"active_sessions"`
	ExpiredSessions      int64     `json:"expired_sessions"`
	ActiveEntityTypes    int64     `json:"active_entity_types"`
	ActiveWorkflowTypes  int64     `json:"active_workflow_types"`
	AverageVersionNumber float64   `json:"average_version_number"`
	LastUpdated          time.Time `json:"last_updated"`
}
