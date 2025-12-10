package rustclient

import (
	"time"

	"github.com/google/uuid"
)

// HealthResponse from /health (dsl_api) and /api/agent/health (agent API)
type HealthResponse struct {
	Status      string `json:"status"`
	Version     string `json:"version"`
	VerbCount   int    `json:"verb_count"`
	DomainCount int    `json:"domain_count,omitempty"` // Only in agent API response
}

// VerbsResponse from /verbs
type VerbsResponse struct {
	Verbs []VerbInfo `json:"verbs"`
	Total int        `json:"total"`
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

// ValidateDSLRequest for /validate
type ValidateDSLRequest struct {
	DSL string `json:"dsl"`
}

// ValidationResult from /validate
type ValidationResult struct {
	Valid  bool              `json:"valid"`
	Errors []ValidationError `json:"errors"`
}

// ValidationError details.
type ValidationError struct {
	Message string `json:"message"`
}

// ExecuteDSLRequest for /execute
type ExecuteDSLRequest struct {
	DSL string `json:"dsl"`
}

// SessionState represents the session lifecycle state
type SessionState string

const (
	SessionStateNew               SessionState = "new"
	SessionStatePendingValidation SessionState = "pending_validation"
	SessionStateReadyToExecute    SessionState = "ready_to_execute"
	SessionStateExecuting         SessionState = "executing"
	SessionStateExecuted          SessionState = "executed"
	SessionStateClosed            SessionState = "closed"
)

// ExecuteResponse from /execute (dsl_api)
// Note: NewState is only present in agent session responses, omitempty handles this
type ExecuteResponse struct {
	Success  bool                 `json:"success"`
	Results  []ExecuteResultItem  `json:"results"`
	Errors   []string             `json:"errors"`
	NewState SessionState         `json:"new_state,omitempty"` // Only in agent session responses
	Bindings map[string]uuid.UUID `json:"bindings,omitempty"`
}

// ExecuteResultItem for each statement.
// Note: DSL and EntityType are only present in agent session responses
type ExecuteResultItem struct {
	StatementIndex int        `json:"statement_index"`
	DSL            string     `json:"dsl,omitempty"` // Only in agent session responses
	Success        bool       `json:"success"`
	Message        string     `json:"message"`
	EntityID       *uuid.UUID `json:"entity_id,omitempty"`
	EntityType     *string    `json:"entity_type,omitempty"` // Only in agent session responses
}

// CbuSummary for list views.
type CbuSummary struct {
	CbuID        uuid.UUID `json:"cbu_id"`
	Name         string    `json:"name"`
	Jurisdiction *string   `json:"jurisdiction,omitempty"`
	ClientType   *string   `json:"client_type,omitempty"`
}

// CbuDetail with full entity information.
type CbuDetail struct {
	CbuID        uuid.UUID    `json:"cbu_id"`
	Name         string       `json:"name"`
	Jurisdiction *string      `json:"jurisdiction,omitempty"`
	ClientType   *string      `json:"client_type,omitempty"`
	Description  *string      `json:"description,omitempty"`
	CreatedAt    *time.Time   `json:"created_at,omitempty"`
	UpdatedAt    *time.Time   `json:"updated_at,omitempty"`
	Entities     []EntityRole `json:"entities"`
}

// EntityRole links entity to CBU with role.
type EntityRole struct {
	EntityID   uuid.UUID `json:"entity_id"`
	Name       string    `json:"name"`
	EntityType string    `json:"entity_type"`
	Role       string    `json:"role"`
}

// KycCaseDetail with workstreams and flags.
type KycCaseDetail struct {
	CaseID      uuid.UUID          `json:"case_id"`
	CbuID       uuid.UUID          `json:"cbu_id"`
	Status      string             `json:"status"`
	CaseType    *string            `json:"case_type,omitempty"`
	RiskRating  *string            `json:"risk_rating,omitempty"`
	OpenedAt    *time.Time         `json:"opened_at,omitempty"`
	ClosedAt    *time.Time         `json:"closed_at,omitempty"`
	Workstreams []WorkstreamDetail `json:"workstreams"`
	RedFlags    []RedFlagDetail    `json:"red_flags"`
}

// WorkstreamDetail for entity workstream.
type WorkstreamDetail struct {
	WorkstreamID uuid.UUID `json:"workstream_id"`
	EntityID     uuid.UUID `json:"entity_id"`
	Status       string    `json:"status"`
	IsUbo        *bool     `json:"is_ubo,omitempty"`
	RiskRating   *string   `json:"risk_rating,omitempty"`
}

// RedFlagDetail for KYC red flag.
type RedFlagDetail struct {
	RedFlagID   uuid.UUID `json:"red_flag_id"`
	FlagType    string    `json:"flag_type"`
	Severity    string    `json:"severity"`
	Status      string    `json:"status"`
	Description string    `json:"description"`
}

// CleanupResponse from /cleanup/cbu/:id
type CleanupResponse struct {
	Deleted bool      `json:"deleted"`
	CbuID   uuid.UUID `json:"cbu_id"`
}

// ============================================================================
// Agent Session Types (matching rust/src/api/session.rs)
// ============================================================================

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

// ChatRequest for POST /api/session/:id/chat
type ChatRequest struct {
	Message string `json:"message"`
}

// VerbIntent represents a single DSL verb invocation intent
// Params values can be string, number, boolean, uuid, list, or object (polymorphic JSON)
type VerbIntent struct {
	Verb     string            `json:"verb"`
	Params   map[string]any    `json:"params"`
	Refs     map[string]string `json:"refs"`
	Sequence *int              `json:"sequence,omitempty"`
}

// IntentError represents a structured validation error for an intent
type IntentError struct {
	Code    string  `json:"code"`
	Message string  `json:"message"`
	Param   *string `json:"param,omitempty"`
}

// IntentValidation represents validation result for an intent
type IntentValidation struct {
	Valid    bool          `json:"valid"`
	Intent   VerbIntent    `json:"intent"`
	Errors   []IntentError `json:"errors"`
	Warnings []string      `json:"warnings"`
}

// AssembledDsl represents DSL assembled from intents
type AssembledDsl struct {
	Statements  []string `json:"statements"`
	Combined    string   `json:"combined"`
	IntentCount int      `json:"intent_count"`
}

// BoundEntity represents a bound entity in the session context
type BoundEntity struct {
	ID          uuid.UUID `json:"id"`
	EntityType  string    `json:"entity_type"`
	DisplayName string    `json:"display_name"`
}

// ChatResponse from POST /api/session/:id/chat
type ChatResponse struct {
	Message           string                  `json:"message"`
	Intents           []VerbIntent            `json:"intents"`
	ValidationResults []IntentValidation      `json:"validation_results"`
	AssembledDsl      *AssembledDsl           `json:"assembled_dsl,omitempty"`
	SessionState      SessionState            `json:"session_state"`
	CanExecute        bool                    `json:"can_execute"`
	DslSource         *string                 `json:"dsl_source,omitempty"`
	Ast               []any                   `json:"ast,omitempty"` // AST statements (complex nested structure)
	Bindings          map[string]*BoundEntity `json:"bindings,omitempty"`
}

// MessageRole represents who sent a message
type MessageRole string

const (
	MessageRoleUser   MessageRole = "user"
	MessageRoleAgent  MessageRole = "agent"
	MessageRoleSystem MessageRole = "system"
)

// ChatMessage represents a message in the conversation
type ChatMessage struct {
	ID        uuid.UUID    `json:"id"`
	Role      MessageRole  `json:"role"`
	Content   string       `json:"content"`
	Timestamp time.Time    `json:"timestamp"`
	Intents   []VerbIntent `json:"intents,omitempty"`
	DSL       *string      `json:"dsl,omitempty"`
}

// SessionContext represents context maintained across the session
type SessionContext struct {
	LastCbuID    *uuid.UUID           `json:"last_cbu_id,omitempty"`
	LastEntityID *uuid.UUID           `json:"last_entity_id,omitempty"`
	CbuIDs       []uuid.UUID          `json:"cbu_ids"`
	EntityIDs    []uuid.UUID          `json:"entity_ids"`
	DomainHint   *string              `json:"domain_hint,omitempty"`
	NamedRefs    map[string]uuid.UUID `json:"named_refs"`
}

// SessionStateResponse from GET /api/session/:id
type SessionStateResponse struct {
	SessionID      uuid.UUID      `json:"session_id"`
	State          SessionState   `json:"state"`
	MessageCount   int            `json:"message_count"`
	PendingIntents []VerbIntent   `json:"pending_intents"`
	AssembledDsl   []string       `json:"assembled_dsl"`
	CombinedDsl    string         `json:"combined_dsl"`
	Context        SessionContext `json:"context"`
	Messages       []ChatMessage  `json:"messages"`
	CanExecute     bool           `json:"can_execute"`
}

// ExecuteSessionRequest for POST /api/session/:id/execute
type ExecuteSessionRequest struct {
	DryRun bool    `json:"dry_run"`
	DSL    *string `json:"dsl,omitempty"`
}

// GenerateDslRequest for POST /api/agent/generate
type GenerateDslRequest struct {
	Instruction string  `json:"instruction"`
	Domain      *string `json:"domain,omitempty"`
}

// GenerateDslResponse from POST /api/agent/generate
type GenerateDslResponse struct {
	DSL         *string `json:"dsl,omitempty"`
	Explanation *string `json:"explanation,omitempty"`
	Error       *string `json:"error,omitempty"`
}

// ============================================================================
// Completion Types (LSP-style via EntityGateway)
// ============================================================================

// CompleteRequest for POST /api/agent/complete
type CompleteRequest struct {
	EntityType string `json:"entity_type"` // cbu, entity, product, role, jurisdiction, etc.
	Query      string `json:"query"`       // Partial text to match
	Limit      int    `json:"limit"`       // Max results (default 10)
}

// CompletionItem represents a single completion suggestion
type CompletionItem struct {
	Value  string  `json:"value"`            // The value to insert (UUID or code)
	Label  string  `json:"label"`            // Display label
	Detail *string `json:"detail,omitempty"` // Additional detail
	Score  float32 `json:"score"`            // Relevance score (0.0-1.0)
}

// CompleteResponse from POST /api/agent/complete
type CompleteResponse struct {
	Items []CompletionItem `json:"items"`
	Total int              `json:"total"`
}
