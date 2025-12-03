package rustclient

import (
	"time"

	"github.com/google/uuid"
)

// SessionState represents session lifecycle.
type SessionState string

const (
	SessionStateNew               SessionState = "new"
	SessionStatePendingValidation SessionState = "pending_validation"
	SessionStateReadyToExecute    SessionState = "ready_to_execute"
	SessionStateExecuting         SessionState = "executing"
	SessionStateExecuted          SessionState = "executed"
	SessionStateClosed            SessionState = "closed"
)

// ValidationResult from DSL validation.
type ValidationResult struct {
	Valid    bool              `json:"valid"`
	Errors   []ValidationError `json:"errors"`
	Warnings []string          `json:"warnings"`
}

// ValidationError details.
type ValidationError struct {
	Line       *int    `json:"line,omitempty"`
	Column     *int    `json:"column,omitempty"`
	Message    string  `json:"message"`
	Suggestion *string `json:"suggestion,omitempty"`
}

// ExecutionResult from DSL execution.
type ExecutionResult struct {
	StatementIndex int        `json:"statement_index"`
	DSL            string     `json:"dsl"`
	Success        bool       `json:"success"`
	Message        string     `json:"message"`
	EntityID       *uuid.UUID `json:"entity_id,omitempty"`
	EntityType     *string    `json:"entity_type,omitempty"`
}

// ExecuteResponse from /api/session/:id/execute
type ExecuteResponse struct {
	Success  bool              `json:"success"`
	Results  []ExecutionResult `json:"results"`
	Errors   []string          `json:"errors"`
	NewState SessionState      `json:"new_state"`
}

// ValidateDSLRequest for /api/agent/validate
type ValidateDSLRequest struct {
	DSL string `json:"dsl"`
}

// GenerateDSLRequest for /api/agent/generate
type GenerateDSLRequest struct {
	Instruction string  `json:"instruction"`
	Domain      *string `json:"domain,omitempty"`
}

// GenerateDSLResponse from /api/agent/generate
type GenerateDSLResponse struct {
	DSL         *string `json:"dsl,omitempty"`
	Explanation *string `json:"explanation,omitempty"`
	Error       *string `json:"error,omitempty"`
}

// DomainsResponse from /api/agent/domains
type DomainsResponse struct {
	Domains    []DomainInfo `json:"domains"`
	TotalVerbs int          `json:"total_verbs"`
}

// DomainInfo describes a DSL domain.
type DomainInfo struct {
	Name        string `json:"name"`
	Description string `json:"description"`
	VerbCount   int    `json:"verb_count"`
}

// VocabResponse from /api/agent/vocabulary
type VocabResponse struct {
	Verbs []VerbInfo `json:"verbs"`
}

// VerbInfo describes a DSL verb.
type VerbInfo struct {
	Domain       string   `json:"domain"`
	Name         string   `json:"name"`
	FullName     string   `json:"full_name"`
	Description  string   `json:"description"`
	RequiredArgs []string `json:"required_args"`
	OptionalArgs []string `json:"optional_args"`
}

// HealthResponse from /api/agent/health
type HealthResponse struct {
	Status      string `json:"status"`
	Version     string `json:"version"`
	VerbCount   int    `json:"verb_count"`
	DomainCount int    `json:"domain_count"`
}

// CreateSessionRequest for POST /api/session
type CreateSessionRequest struct {
	DomainHint *string `json:"domain_hint,omitempty"`
}

// CreateSessionResponse from POST /api/session
type CreateSessionResponse struct {
	SessionID uuid.UUID    `json:"session_id"`
	CreatedAt time.Time    `json:"created_at"`
	State     SessionState `json:"state"`
}

// SessionContext maintained across session.
type SessionContext struct {
	LastCbuID    *uuid.UUID           `json:"last_cbu_id,omitempty"`
	LastEntityID *uuid.UUID           `json:"last_entity_id,omitempty"`
	CbuIDs       []uuid.UUID          `json:"cbu_ids"`
	EntityIDs    []uuid.UUID          `json:"entity_ids"`
	DomainHint   *string              `json:"domain_hint,omitempty"`
	NamedRefs    map[string]uuid.UUID `json:"named_refs"`
}

// ExecuteDSLRequest for POST /api/session/:id/execute
type ExecuteDSLRequest struct {
	DSL string `json:"dsl"`
}
