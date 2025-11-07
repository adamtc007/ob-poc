// Package registry provides the domain registry system for multi-domain DSL management.
//
// This package enables multiple domains (onboarding, hedge-fund-investor, KYC, etc.) to
// coexist within the same system while maintaining their own vocabularies, validation rules,
// and state machines. The registry acts as a central coordination point for domain discovery
// and routing.
//
// Key Components:
// - Domain: Interface that all domains must implement
// - Vocabulary: Domain-specific verb definitions and metadata
// - StateTransition: State machine transition rules
// - Registry: Central domain management and lookup
// - Router: Intelligent routing to appropriate domain
//
// Architecture Pattern: DSL-as-State + AttributeID-as-Type
// Each domain maintains its own DSL vocabulary while sharing common infrastructure
// (parser, session manager, UUID resolver) from internal/shared-dsl.
package registry

import (
	"context"
	"fmt"
	"time"
)

// Domain represents a business domain that can generate and validate DSL operations.
// Examples: "onboarding", "hedge-fund-investor", "kyc", "compliance"
//
// Each domain is responsible for:
// - Defining its vocabulary (approved verbs and their semantics)
// - Validating DSL operations within its scope
// - Generating DSL from natural language instructions
// - Managing its state machine transitions
// - Providing domain-specific business logic
type Domain interface {
	// Identity returns the domain's unique identifier and version
	Name() string    // "onboarding", "hedge-fund-investor", "kyc"
	Version() string // "1.0.0", "2.1.3"

	// Description returns human-readable description for domain discovery
	Description() string // "Client onboarding and case management"

	// Vocabulary returns the domain's complete DSL vocabulary
	GetVocabulary() *Vocabulary

	// Validation ensures DSL operations comply with domain rules
	ValidateVerbs(dsl string) error
	ValidateStateTransition(from, to string) error

	// Generation creates DSL from natural language instructions
	GenerateDSL(ctx context.Context, req *GenerationRequest) (*GenerationResponse, error)

	// State Management tracks domain-specific state transitions
	GetCurrentState(context map[string]interface{}) (string, error)
	GetValidStates() []string
	GetInitialState() string

	// Context Management extracts domain-specific context from DSL
	ExtractContext(dsl string) (map[string]interface{}, error)

	// Health and Monitoring
	IsHealthy() bool
	GetMetrics() *DomainMetrics
}

// Vocabulary defines the complete DSL vocabulary for a domain.
// This includes all verbs, their arguments, state transitions, and metadata.
type Vocabulary struct {
	Domain      string                     `json:"domain"`      // "onboarding"
	Version     string                     `json:"version"`     // "1.0.0"
	Description string                     `json:"description"` // Human-readable description
	Verbs       map[string]*VerbDefinition `json:"verbs"`       // All verbs in this domain
	Categories  map[string]*VerbCategory   `json:"categories"`  // Logical groupings of verbs
	States      []string                   `json:"states"`      // All valid states
	CreatedAt   time.Time                  `json:"created_at"`
	UpdatedAt   time.Time                  `json:"updated_at"`
}

// VerbDefinition defines a single DSL verb with its complete specification.
// This provides the semantic definition that enables AI agents to generate
// correct DSL operations.
type VerbDefinition struct {
	// Identity
	Name        string `json:"name"`        // "case.create", "kyc.begin"
	Category    string `json:"category"`    // "case-management", "compliance"
	Version     string `json:"version"`     // "1.0.0"
	Description string `json:"description"` // Human-readable description

	// Arguments specification
	Arguments map[string]*ArgumentSpec `json:"arguments"` // Required and optional args

	// State machine
	StateTransition *StateTransition `json:"state_transition,omitempty"` // State change rules

	// Semantic metadata
	Idempotent      bool     `json:"idempotent"`                 // Can be called multiple times safely
	GuardConditions []string `json:"guard_conditions,omitempty"` // Preconditions
	SideEffects     []string `json:"side_effects,omitempty"`     // External effects

	// Examples and documentation
	Examples []string `json:"examples,omitempty"` // Sample DSL usage
	SeeAlso  []string `json:"see_also,omitempty"` // Related verbs

	// Metadata
	CreatedAt time.Time `json:"created_at"`
	UpdatedAt time.Time `json:"updated_at"`
}

// ArgumentSpec defines a single argument to a DSL verb.
// This provides type information and validation rules for verb arguments.
type ArgumentSpec struct {
	Name        string       `json:"name"`        // "investor-id", "amount"
	Type        ArgumentType `json:"type"`        // UUID, STRING, DECIMAL, ENUM, etc.
	Required    bool         `json:"required"`    // Must be present
	Description string       `json:"description"` // Human-readable description

	// Type-specific constraints
	EnumValues []string `json:"enum_values,omitempty"` // For ENUM type
	MinLength  *int     `json:"min_length,omitempty"`  // For STRING type
	MaxLength  *int     `json:"max_length,omitempty"`  // For STRING type
	MinValue   *float64 `json:"min_value,omitempty"`   // For DECIMAL/INTEGER type
	MaxValue   *float64 `json:"max_value,omitempty"`   // For DECIMAL/INTEGER type
	Pattern    string   `json:"pattern,omitempty"`     // Regex pattern for validation

	// Context integration
	AttributeID  string      `json:"attribute_id,omitempty"`  // Links to dictionary table
	ContextKey   string      `json:"context_key,omitempty"`   // Key in session context
	DefaultValue interface{} `json:"default_value,omitempty"` // Default if not provided

	// Examples
	Examples []interface{} `json:"examples,omitempty"` // Example values
}

// ArgumentType defines the data type of a DSL verb argument
type ArgumentType string

const (
	ArgumentTypeUUID    ArgumentType = "UUID"    // UUID string
	ArgumentTypeString  ArgumentType = "STRING"  // Text string
	ArgumentTypeInteger ArgumentType = "INTEGER" // Whole number
	ArgumentTypeDecimal ArgumentType = "DECIMAL" // Decimal number
	ArgumentTypeBoolean ArgumentType = "BOOLEAN" // true/false
	ArgumentTypeDate    ArgumentType = "DATE"    // ISO date string
	ArgumentTypeEnum    ArgumentType = "ENUM"    // One of predefined values
	ArgumentTypeArray   ArgumentType = "ARRAY"   // Array of values
	ArgumentTypeObject  ArgumentType = "OBJECT"  // Nested object
	ArgumentTypeAny     ArgumentType = "ANY"     // Any value (use sparingly)
)

// VerbCategory groups related verbs for organization and discovery
type VerbCategory struct {
	Name        string   `json:"name"`            // "case-management"
	Description string   `json:"description"`     // Human-readable description
	Verbs       []string `json:"verbs"`           // List of verb names in this category
	Color       string   `json:"color,omitempty"` // UI color hint
	Icon        string   `json:"icon,omitempty"`  // UI icon hint
}

// StateTransition defines how a DSL verb affects the domain's state machine
type StateTransition struct {
	FromStates  []string              `json:"from_states,omitempty"` // Valid source states (empty = any)
	ToState     string                `json:"to_state,omitempty"`    // Target state (empty = no change)
	Conditional bool                  `json:"conditional"`           // Whether transition depends on conditions
	Conditions  []TransitionCondition `json:"conditions,omitempty"`  // Conditions for transition
}

// TransitionCondition defines a condition that must be met for state transition
type TransitionCondition struct {
	Type        ConditionType `json:"type"`        // TYPE of condition
	Field       string        `json:"field"`       // Field to check
	Operator    string        `json:"operator"`    // Comparison operator
	Value       interface{}   `json:"value"`       // Expected value
	Description string        `json:"description"` // Human-readable description
}

// ConditionType defines the type of transition condition
type ConditionType string

const (
	ConditionTypeContext   ConditionType = "CONTEXT"   // Check session context value
	ConditionTypeArgument  ConditionType = "ARGUMENT"  // Check verb argument value
	ConditionTypeAttribute ConditionType = "ATTRIBUTE" // Check attribute value
	ConditionTypeExternal  ConditionType = "EXTERNAL"  // External system check
)

// GenerationRequest represents a request to generate DSL from natural language
type GenerationRequest struct {
	// User instruction
	Instruction string `json:"instruction"` // "Create case for CBU-1234"

	// Session context
	SessionID     string                 `json:"session_id"`
	CurrentDomain string                 `json:"current_domain,omitempty"`
	Context       map[string]interface{} `json:"context,omitempty"`
	ExistingDSL   string                 `json:"existing_dsl,omitempty"`

	// Entity context (domain-specific)
	EntityContext map[string]interface{} `json:"entity_context,omitempty"`

	// Generation options
	MaxTokens    int     `json:"max_tokens,omitempty"`
	Temperature  float64 `json:"temperature,omitempty"`
	ValidateOnly bool    `json:"validate_only,omitempty"` // Only validate, don't generate
	DryRun       bool    `json:"dry_run,omitempty"`       // Explain what would happen

	// Metadata
	RequestID string    `json:"request_id,omitempty"`
	Timestamp time.Time `json:"timestamp"`
}

// GenerationResponse contains the result of DSL generation
type GenerationResponse struct {
	// Generated DSL
	DSL string `json:"dsl"` // Generated S-expression DSL

	// Metadata about generation
	Verb       string                 `json:"verb"`       // Primary verb used
	Parameters map[string]interface{} `json:"parameters"` // Extracted parameters

	// State machine
	FromState string `json:"from_state,omitempty"` // Current state
	ToState   string `json:"to_state,omitempty"`   // New state after operation

	// Validation
	IsValid         bool     `json:"is_valid"`
	Confidence      float64  `json:"confidence"` // 0.0 to 1.0
	GuardConditions []string `json:"guard_conditions,omitempty"`
	Warnings        []string `json:"warnings,omitempty"`
	Errors          []string `json:"errors,omitempty"`

	// Explanation
	Explanation      string   `json:"explanation"`                 // What the DSL does
	NextSteps        []string `json:"next_steps,omitempty"`        // Suggested next actions
	AlternativeVerbs []string `json:"alternative_verbs,omitempty"` // Other possible verbs

	// Context updates
	ContextUpdates map[string]interface{} `json:"context_updates,omitempty"`

	// Metadata
	GenerationTime time.Duration `json:"generation_time"`
	Model          string        `json:"model,omitempty"` // AI model used
	RequestID      string        `json:"request_id,omitempty"`
	Timestamp      time.Time     `json:"timestamp"`
}

// DomainMetrics provides health and performance metrics for a domain
type DomainMetrics struct {
	// Usage statistics
	TotalRequests       int64         `json:"total_requests"`
	SuccessfulRequests  int64         `json:"successful_requests"`
	FailedRequests      int64         `json:"failed_requests"`
	AverageResponseTime time.Duration `json:"average_response_time"`

	// Vocabulary statistics
	TotalVerbs  int `json:"total_verbs"`
	ActiveVerbs int `json:"active_verbs"` // Recently used verbs
	UnusedVerbs int `json:"unused_verbs"` // Never used verbs

	// State machine statistics
	StateTransitions map[string]int64 `json:"state_transitions"` // Count per transition
	CurrentStates    map[string]int64 `json:"current_states"`    // Count per state

	// Error statistics
	ValidationErrors map[string]int64 `json:"validation_errors"` // Count per error type
	GenerationErrors map[string]int64 `json:"generation_errors"` // Count per error type

	// Health indicators
	IsHealthy        bool      `json:"is_healthy"`
	LastHealthCheck  time.Time `json:"last_health_check"`
	UptimeSeconds    int64     `json:"uptime_seconds"`
	MemoryUsageBytes int64     `json:"memory_usage_bytes"`

	// Metadata
	CollectedAt time.Time `json:"collected_at"`
	Version     string    `json:"version"`
}

// ValidationError represents a domain-specific validation error
type ValidationError struct {
	Code        string                 `json:"code"`                  // Error code (e.g., "INVALID_VERB")
	Message     string                 `json:"message"`               // Human-readable message
	Field       string                 `json:"field,omitempty"`       // Field that caused error
	Value       interface{}            `json:"value,omitempty"`       // Invalid value
	Context     map[string]interface{} `json:"context,omitempty"`     // Additional context
	Suggestions []string               `json:"suggestions,omitempty"` // Suggested fixes
}

// Error returns the error message (implements error interface)
func (e *ValidationError) Error() string {
	return e.Message
}

// DomainError represents a domain-specific error
type DomainError struct {
	Domain    string                 `json:"domain"`            // Domain that generated the error
	Code      string                 `json:"code"`              // Error code
	Message   string                 `json:"message"`           // Human-readable message
	Details   map[string]interface{} `json:"details,omitempty"` // Additional details
	Timestamp time.Time              `json:"timestamp"`
}

// Error returns the error message (implements error interface)
func (e *DomainError) Error() string {
	return fmt.Sprintf("[%s] %s: %s", e.Domain, e.Code, e.Message)
}
