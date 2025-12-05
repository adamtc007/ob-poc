package rustclient

import (
	"time"

	"github.com/google/uuid"
)

// HealthResponse from /health
type HealthResponse struct {
	Status      string `json:"status"`
	Version     string `json:"version"`
	VerbCount   int    `json:"verb_count"`
	DomainCount int    `json:"domain_count"`
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

// ExecuteResponse from /execute
type ExecuteResponse struct {
	Success  bool                 `json:"success"`
	Results  []ExecuteResultItem  `json:"results"`
	Bindings map[string]uuid.UUID `json:"bindings"`
	Errors   []string             `json:"errors"`
}

// ExecuteResultItem for each statement.
type ExecuteResultItem struct {
	StatementIndex int        `json:"statement_index"`
	Success        bool       `json:"success"`
	Message        string     `json:"message"`
	EntityID       *uuid.UUID `json:"entity_id,omitempty"`
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
