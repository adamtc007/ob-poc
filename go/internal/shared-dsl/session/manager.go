// Package session provides domain-agnostic session management for DSL accumulation.
//
// This package is shared across ALL domains (onboarding, hedge-fund-investor, kyc, etc.)
// and manages stateful DSL accumulation, context tracking, and session lifecycle.
package session

import (
	"fmt"
	"sync"
	"time"

	"github.com/google/uuid"
)

// Manager manages multiple chat sessions with DSL accumulation
type Manager struct {
	sessions map[string]*Session
	mu       sync.RWMutex
}

// Session represents a user's chat session with accumulated DSL state
type Session struct {
	SessionID string
	Domain    string // Current active domain (e.g., "onboarding", "hedge-fund-investor")
	BuiltDSL  string // Accumulated DSL document (the state)
	Context   Context
	History   []Message
	CreatedAt time.Time
	LastUsed  time.Time
	mu        sync.RWMutex
}

// Context holds session context for entity references and state
type Context struct {
	// Common entity IDs (used across domains)
	InvestorID string
	FundID     string
	ClassID    string
	SeriesID   string
	CBUID      string

	// Entity attributes
	InvestorName string
	InvestorType string
	Domicile     string
	LegalName    string

	// State machine
	CurrentState string

	// Domain-specific data (flexible storage)
	Data map[string]interface{}

	mu sync.RWMutex
}

// Message represents a single message in the chat history
type Message struct {
	Role      string                 // "user" or "agent"
	Content   string                 // Message text
	DSL       string                 // Generated DSL (if any)
	Timestamp time.Time              // When message was sent
	Metadata  map[string]interface{} // Additional metadata
}

// NewManager creates a new session manager
func NewManager() *Manager {
	return &Manager{
		sessions: make(map[string]*Session),
	}
}

// NewSession creates a new session with the given domain
func NewSession(sessionID, domain string) *Session {
	if sessionID == "" {
		sessionID = uuid.New().String()
	}

	return &Session{
		SessionID: sessionID,
		Domain:    domain,
		BuiltDSL:  "",
		Context: Context{
			Data: make(map[string]interface{}),
		},
		History:   make([]Message, 0),
		CreatedAt: time.Now(),
		LastUsed:  time.Now(),
	}
}

// GetOrCreate gets an existing session or creates a new one
func (m *Manager) GetOrCreate(sessionID, domain string) *Session {
	m.mu.Lock()
	defer m.mu.Unlock()

	if sessionID != "" {
		if session, exists := m.sessions[sessionID]; exists {
			session.LastUsed = time.Now()
			// Update domain if provided and different
			if domain != "" && session.Domain != domain {
				session.Domain = domain
			}
			return session
		}
	}

	// Create new session
	session := NewSession(sessionID, domain)
	m.sessions[session.SessionID] = session
	return session
}

// Get retrieves an existing session
func (m *Manager) Get(sessionID string) (*Session, error) {
	m.mu.RLock()
	defer m.mu.RUnlock()

	session, exists := m.sessions[sessionID]
	if !exists {
		return nil, fmt.Errorf("session not found: %s", sessionID)
	}

	session.LastUsed = time.Now()
	return session, nil
}

// Delete removes a session
func (m *Manager) Delete(sessionID string) error {
	m.mu.Lock()
	defer m.mu.Unlock()

	if _, exists := m.sessions[sessionID]; !exists {
		return fmt.Errorf("session not found: %s", sessionID)
	}

	delete(m.sessions, sessionID)
	return nil
}

// List returns all session IDs
func (m *Manager) List() []string {
	m.mu.RLock()
	defer m.mu.RUnlock()

	ids := make([]string, 0, len(m.sessions))
	for id := range m.sessions {
		ids = append(ids, id)
	}
	return ids
}

// Count returns the number of active sessions
func (m *Manager) Count() int {
	m.mu.RLock()
	defer m.mu.RUnlock()
	return len(m.sessions)
}

// AccumulateDSL appends new DSL to the session's accumulated DSL
// This is the core function for DSL-as-State pattern
func (m *Manager) AccumulateDSL(sessionID, newDSL string) error {
	session, err := m.Get(sessionID)
	if err != nil {
		return err
	}

	return session.AccumulateDSL(newDSL)
}

// UpdateContext updates the session context with new values
func (m *Manager) UpdateContext(sessionID string, updates map[string]interface{}) error {
	session, err := m.Get(sessionID)
	if err != nil {
		return err
	}

	return session.UpdateContext(updates)
}

// SwitchDomain switches the active domain for a session
func (m *Manager) SwitchDomain(sessionID, newDomain string) error {
	session, err := m.Get(sessionID)
	if err != nil {
		return err
	}

	session.mu.Lock()
	defer session.mu.Unlock()

	session.Domain = newDomain
	session.LastUsed = time.Now()
	return nil
}

// CleanupExpired removes sessions older than the given duration
func (m *Manager) CleanupExpired(maxAge time.Duration) int {
	m.mu.Lock()
	defer m.mu.Unlock()

	now := time.Now()
	removed := 0

	for id, session := range m.sessions {
		if now.Sub(session.LastUsed) > maxAge {
			delete(m.sessions, id)
			removed++
		}
	}

	return removed
}

// Session Methods

// AccumulateDSL appends new DSL to the session's accumulated DSL
func (s *Session) AccumulateDSL(newDSL string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	if newDSL == "" {
		return nil // Nothing to append
	}

	if s.BuiltDSL == "" {
		s.BuiltDSL = newDSL
	} else {
		s.BuiltDSL = s.BuiltDSL + "\n\n" + newDSL
	}

	s.LastUsed = time.Now()
	return nil
}

// GetDSL returns the current accumulated DSL (read-only)
func (s *Session) GetDSL() string {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return s.BuiltDSL
}

// UpdateContext updates the session context with new values
func (s *Session) UpdateContext(updates map[string]interface{}) error {
	s.Context.mu.Lock()
	defer s.Context.mu.Unlock()

	for key, value := range updates {
		switch key {
		case "investor_id":
			if v, ok := value.(string); ok {
				s.Context.InvestorID = v
			}
		case "fund_id":
			if v, ok := value.(string); ok {
				s.Context.FundID = v
			}
		case "class_id":
			if v, ok := value.(string); ok {
				s.Context.ClassID = v
			}
		case "series_id":
			if v, ok := value.(string); ok {
				s.Context.SeriesID = v
			}
		case "cbu_id":
			if v, ok := value.(string); ok {
				s.Context.CBUID = v
			}
		case "investor_name":
			if v, ok := value.(string); ok {
				s.Context.InvestorName = v
			}
		case "investor_type":
			if v, ok := value.(string); ok {
				s.Context.InvestorType = v
			}
		case "domicile":
			if v, ok := value.(string); ok {
				s.Context.Domicile = v
			}
		case "legal_name":
			if v, ok := value.(string); ok {
				s.Context.LegalName = v
			}
		case "current_state":
			if v, ok := value.(string); ok {
				s.Context.CurrentState = v
			}
		default:
			// Store in flexible data map for domain-specific values
			s.Context.Data[key] = value
		}
	}

	s.LastUsed = time.Now()
	return nil
}

// GetContext returns a copy of the current context (read-only)
func (s *Session) GetContext() Context {
	s.Context.mu.RLock()
	defer s.Context.mu.RUnlock()

	// Create a copy to avoid external modifications
	dataCopy := make(map[string]interface{})
	for k, v := range s.Context.Data {
		dataCopy[k] = v
	}

	return Context{
		InvestorID:   s.Context.InvestorID,
		FundID:       s.Context.FundID,
		ClassID:      s.Context.ClassID,
		SeriesID:     s.Context.SeriesID,
		CBUID:        s.Context.CBUID,
		InvestorName: s.Context.InvestorName,
		InvestorType: s.Context.InvestorType,
		Domicile:     s.Context.Domicile,
		LegalName:    s.Context.LegalName,
		CurrentState: s.Context.CurrentState,
		Data:         dataCopy,
	}
}

// AddMessage adds a message to the session history
func (s *Session) AddMessage(role, content, dsl string, metadata map[string]interface{}) {
	s.mu.Lock()
	defer s.mu.Unlock()

	message := Message{
		Role:      role,
		Content:   content,
		DSL:       dsl,
		Timestamp: time.Now(),
		Metadata:  metadata,
	}

	s.History = append(s.History, message)
	s.LastUsed = time.Now()
}

// GetHistory returns the message history (read-only)
func (s *Session) GetHistory() []Message {
	s.mu.RLock()
	defer s.mu.RUnlock()

	// Return a copy to avoid external modifications
	historyCopy := make([]Message, len(s.History))
	copy(historyCopy, s.History)
	return historyCopy
}

// Reset clears the DSL state and context (for restarting a session)
func (s *Session) Reset() {
	s.mu.Lock()
	defer s.mu.Unlock()

	s.BuiltDSL = ""
	s.Context = Context{
		Data: make(map[string]interface{}),
	}
	s.History = make([]Message, 0)
	s.LastUsed = time.Now()
}

// GetContextValue retrieves a value from the flexible context data
func (c *Context) GetContextValue(key string) (interface{}, bool) {
	c.mu.RLock()
	defer c.mu.RUnlock()

	val, exists := c.Data[key]
	return val, exists
}

// SetContextValue sets a value in the flexible context data
func (c *Context) SetContextValue(key string, value interface{}) {
	c.mu.Lock()
	defer c.mu.Unlock()

	c.Data[key] = value
}
