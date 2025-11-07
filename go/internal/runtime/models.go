package runtime

import (
	"encoding/json"
	"time"
)

// ActionType represents the type of action to execute
type ActionType string

const (
	ActionTypeHTTPAPI         ActionType = "HTTP_API"
	ActionTypeBPMNWorkflow    ActionType = "BPMN_WORKFLOW"
	ActionTypeMessageQueue    ActionType = "MESSAGE_QUEUE"
	ActionTypeDatabaseOp      ActionType = "DATABASE_OPERATION"
	ActionTypeExternalService ActionType = "EXTERNAL_SERVICE"
)

// ExecutionStatus represents the current status of an action execution
type ExecutionStatus string

const (
	ExecutionStatusPending   ExecutionStatus = "PENDING"
	ExecutionStatusRunning   ExecutionStatus = "RUNNING"
	ExecutionStatusCompleted ExecutionStatus = "COMPLETED"
	ExecutionStatusFailed    ExecutionStatus = "FAILED"
	ExecutionStatusCancelled ExecutionStatus = "CANCELLED"
)

// ResourceType represents a concrete runtime resource definition
type ResourceType struct {
	ResourceTypeID   string    `json:"resource_type_id" db:"resource_type_id"`
	ResourceTypeName string    `json:"resource_type_name" db:"resource_type_name"`
	Description      string    `json:"description" db:"description"`
	Active           bool      `json:"active" db:"active"`
	Version          int       `json:"version" db:"version"`
	Environment      string    `json:"environment" db:"environment"`
	CreatedAt        time.Time `json:"created_at" db:"created_at"`
	UpdatedAt        time.Time `json:"updated_at" db:"updated_at"`
}

// ResourceTypeAttribute represents an attribute requirement for a resource type
type ResourceTypeAttribute struct {
	ResourceTypeID string          `json:"resource_type_id" db:"resource_type_id"`
	AttributeID    string          `json:"attribute_id" db:"attribute_id"`
	Required       bool            `json:"required" db:"required"`
	Constraints    json.RawMessage `json:"constraints" db:"constraints"`
	Transformation string          `json:"transformation" db:"transformation"`
	CreatedAt      time.Time       `json:"created_at" db:"created_at"`
}

// ResourceTypeEndpoint represents a lifecycle endpoint for a resource type
type ResourceTypeEndpoint struct {
	EndpointID      string          `json:"endpoint_id" db:"endpoint_id"`
	ResourceTypeID  string          `json:"resource_type_id" db:"resource_type_id"`
	LifecycleAction string          `json:"lifecycle_action" db:"lifecycle_action"`
	EndpointURL     string          `json:"endpoint_url" db:"endpoint_url"`
	Method          string          `json:"method" db:"method"`
	Authentication  json.RawMessage `json:"authentication" db:"authentication"`
	TimeoutSeconds  int             `json:"timeout_seconds" db:"timeout_seconds"`
	RetryConfig     json.RawMessage `json:"retry_config" db:"retry_config"`
	Environment     string          `json:"environment" db:"environment"`
	CreatedAt       time.Time       `json:"created_at" db:"created_at"`
	UpdatedAt       time.Time       `json:"updated_at" db:"updated_at"`
}

// ActionDefinition represents a DSL verb to API endpoint mapping
type ActionDefinition struct {
	ActionID          string           `json:"action_id" db:"action_id"`
	ActionName        string           `json:"action_name" db:"action_name"`
	VerbPattern       string           `json:"verb_pattern" db:"verb_pattern"`
	ActionType        ActionType       `json:"action_type" db:"action_type"`
	ResourceTypeID    *string          `json:"resource_type_id" db:"resource_type_id"`
	Domain            *string          `json:"domain" db:"domain"`
	TriggerConditions json.RawMessage  `json:"trigger_conditions" db:"trigger_conditions"`
	ExecutionConfig   ExecutionConfig  `json:"execution_config" db:"execution_config"`
	AttributeMapping  AttributeMapping `json:"attribute_mapping" db:"attribute_mapping"`
	SuccessCriteria   json.RawMessage  `json:"success_criteria" db:"success_criteria"`
	FailureHandling   json.RawMessage  `json:"failure_handling" db:"failure_handling"`
	Active            bool             `json:"active" db:"active"`
	Version           int              `json:"version" db:"version"`
	Environment       string           `json:"environment" db:"environment"`
	CreatedAt         time.Time        `json:"created_at" db:"created_at"`
	UpdatedAt         time.Time        `json:"updated_at" db:"updated_at"`
}

// ExecutionConfig holds configuration for how to execute an action
type ExecutionConfig struct {
	EndpointURL            string            `json:"endpoint_url"`
	EndpointLookupFallback string            `json:"endpoint_lookup_fallback,omitempty"`
	Method                 string            `json:"method"`
	Authentication         map[string]any    `json:"authentication"`
	TimeoutSeconds         int               `json:"timeout_seconds"`
	RetryConfig            RetryConfig       `json:"retry_config"`
	Idempotency            IdempotencyConfig `json:"idempotency"`
	Telemetry              TelemetryConfig   `json:"telemetry"`
}

// RetryConfig defines retry behavior
type RetryConfig struct {
	MaxRetries      int     `json:"max_retries"`
	BackoffStrategy string  `json:"backoff_strategy"`
	BaseDelayMS     int     `json:"base_delay_ms"`
	MaxDelayMS      int     `json:"max_delay_ms,omitempty"`
	Multiplier      float64 `json:"multiplier,omitempty"`
}

// IdempotencyConfig defines idempotency behavior
type IdempotencyConfig struct {
	Header        string `json:"header"`
	KeyTemplate   string `json:"key_template"`
	DedupeTTLSecs int    `json:"dedupe_ttl_seconds"`
}

// TelemetryConfig defines observability configuration
type TelemetryConfig struct {
	CorrelationIDTemplate string `json:"correlation_id_template"`
	PropagateTrace        bool   `json:"propagate_trace"`
}

// AttributeMapping defines input/output attribute mappings
type AttributeMapping struct {
	InputMapping  []AttributeMap `json:"input_mapping"`
	OutputMapping []AttributeMap `json:"output_mapping"`
}

// AttributeMap represents a single attribute mapping
type AttributeMap struct {
	DSLAttributeID  string `json:"dsl_attribute_id"`
	APIParameter    string `json:"api_parameter,omitempty"`
	APIResponsePath string `json:"api_response_path,omitempty"`
	AttributeName   string `json:"attribute_name,omitempty"`
	Transformation  string `json:"transformation,omitempty"`
}

// ActionExecution represents a single execution of an action
type ActionExecution struct {
	ExecutionID         string          `json:"execution_id" db:"execution_id"`
	ActionID            string          `json:"action_id" db:"action_id"`
	CBUID               string          `json:"cbu_id" db:"cbu_id"`
	DSLVersionID        string          `json:"dsl_version_id" db:"dsl_version_id"`
	ExecutionStatus     ExecutionStatus `json:"execution_status" db:"execution_status"`
	TriggerContext      json.RawMessage `json:"trigger_context" db:"trigger_context"`
	RequestPayload      json.RawMessage `json:"request_payload" db:"request_payload"`
	ResponsePayload     json.RawMessage `json:"response_payload" db:"response_payload"`
	ResultAttributes    json.RawMessage `json:"result_attributes" db:"result_attributes"`
	ErrorDetails        json.RawMessage `json:"error_details" db:"error_details"`
	ExecutionDurationMS *int            `json:"execution_duration_ms" db:"execution_duration_ms"`
	StartedAt           time.Time       `json:"started_at" db:"started_at"`
	CompletedAt         *time.Time      `json:"completed_at" db:"completed_at"`
	RetryCount          int             `json:"retry_count" db:"retry_count"`
	NextRetryAt         *time.Time      `json:"next_retry_at" db:"next_retry_at"`
	IdempotencyKey      *string         `json:"idempotency_key" db:"idempotency_key"`
	CorrelationID       *string         `json:"correlation_id" db:"correlation_id"`
	TraceID             *string         `json:"trace_id" db:"trace_id"`
	SpanID              *string         `json:"span_id" db:"span_id"`
	HTTPStatus          *int            `json:"http_status" db:"http_status"`
	Endpoint            *string         `json:"endpoint" db:"endpoint"`
	Headers             json.RawMessage `json:"headers" db:"headers"`
}

// ActionExecutionAttempt represents a single attempt within an execution
type ActionExecutionAttempt struct {
	AttemptID       string          `json:"attempt_id" db:"attempt_id"`
	ExecutionID     string          `json:"execution_id" db:"execution_id"`
	AttemptNo       int             `json:"attempt_no" db:"attempt_no"`
	StartedAt       time.Time       `json:"started_at" db:"started_at"`
	CompletedAt     *time.Time      `json:"completed_at" db:"completed_at"`
	Status          ExecutionStatus `json:"status" db:"status"`
	RequestPayload  json.RawMessage `json:"request_payload" db:"request_payload"`
	ResponsePayload json.RawMessage `json:"response_payload" db:"response_payload"`
	ErrorDetails    json.RawMessage `json:"error_details" db:"error_details"`
	HTTPStatus      *int            `json:"http_status" db:"http_status"`
	DurationMS      *int            `json:"duration_ms" db:"duration_ms"`
	EndpointURL     *string         `json:"endpoint_url" db:"endpoint_url"`
	RequestHeaders  json.RawMessage `json:"request_headers" db:"request_headers"`
	ResponseHeaders json.RawMessage `json:"response_headers" db:"response_headers"`
}

// CredentialVault represents stored credentials
type CredentialVault struct {
	CredentialID   string     `json:"credential_id" db:"credential_id"`
	CredentialName string     `json:"credential_name" db:"credential_name"`
	CredentialType string     `json:"credential_type" db:"credential_type"`
	EncryptedData  []byte     `json:"encrypted_data" db:"encrypted_data"`
	Environment    string     `json:"environment" db:"environment"`
	CreatedAt      time.Time  `json:"created_at" db:"created_at"`
	ExpiresAt      *time.Time `json:"expires_at" db:"expires_at"`
	Active         bool       `json:"active" db:"active"`
}

// TriggerConditions represents conditions that must be met to trigger an action
type TriggerConditions struct {
	Domain                *string        `json:"domain,omitempty"`
	State                 *string        `json:"state,omitempty"`
	AttributeRequirements []string       `json:"attribute_requirements,omitempty"`
	CustomConditions      map[string]any `json:"custom_conditions,omitempty"`
}

// SuccessCriteria represents criteria for determining execution success
type SuccessCriteria struct {
	HTTPStatusCodes    []int    `json:"http_status_codes"`
	ResponseValidation *string  `json:"response_validation,omitempty"`
	RequiredOutputs    []string `json:"required_outputs,omitempty"`
}

// FailureHandling represents how to handle execution failures
type FailureHandling struct {
	RetryOnCodes         []int    `json:"retry_on_codes,omitempty"`
	FallbackAction       *string  `json:"fallback_action,omitempty"`
	NotificationChannels []string `json:"notification_channels,omitempty"`
}

// ExecutionRequest represents a request to execute an action
type ExecutionRequest struct {
	ActionID        string         `json:"action_id"`
	CBUID           string         `json:"cbu_id"`
	DSLVersionID    string         `json:"dsl_version_id"`
	TriggerContext  map[string]any `json:"trigger_context"`
	AttributeValues map[string]any `json:"attribute_values"`
	Environment     string         `json:"environment"`
	TraceID         *string        `json:"trace_id,omitempty"`
	SpanID          *string        `json:"span_id,omitempty"`
}

// ExecutionResult represents the result of an action execution
type ExecutionResult struct {
	ExecutionID      string         `json:"execution_id"`
	Success          bool           `json:"success"`
	HTTPStatus       *int           `json:"http_status,omitempty"`
	ResponsePayload  map[string]any `json:"response_payload,omitempty"`
	ResultAttributes map[string]any `json:"result_attributes,omitempty"`
	ErrorDetails     *string        `json:"error_details,omitempty"`
	DurationMS       int            `json:"duration_ms"`
	IdempotencyKey   *string        `json:"idempotency_key,omitempty"`
	CorrelationID    *string        `json:"correlation_id,omitempty"`
}
