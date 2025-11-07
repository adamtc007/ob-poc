package runtime

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
)

// Repository provides data access for runtime execution components
type Repository struct {
	db *sql.DB
}

// NewRepository creates a new runtime repository
func NewRepository(db *sql.DB) *Repository {
	return &Repository{db: db}
}

// ==============================================================================
// Resource Type Operations
// ==============================================================================

// CreateResourceType creates a new resource type
func (r *Repository) CreateResourceType(ctx context.Context, rt *ResourceType) error {
	query := `
		INSERT INTO resource_types (
			resource_type_id, resource_type_name, description, active,
			version, environment, created_at, updated_at
		) VALUES (
			COALESCE(NULLIF($1, ''), uuid_generate_v4()), $2, $3, $4, $5, $6, NOW(), NOW()
		) RETURNING resource_type_id, created_at, updated_at`

	err := r.db.QueryRowContext(ctx, query,
		rt.ResourceTypeID, rt.ResourceTypeName, rt.Description, rt.Active,
		rt.Version, rt.Environment,
	).Scan(&rt.ResourceTypeID, &rt.CreatedAt, &rt.UpdatedAt)

	return err
}

// GetResourceType retrieves a resource type by ID
func (r *Repository) GetResourceType(ctx context.Context, resourceTypeID string) (*ResourceType, error) {
	query := `
		SELECT resource_type_id, resource_type_name, description, active,
			   version, environment, created_at, updated_at
		FROM resource_types
		WHERE resource_type_id = $1`

	rt := &ResourceType{}
	err := r.db.QueryRowContext(ctx, query, resourceTypeID).Scan(
		&rt.ResourceTypeID, &rt.ResourceTypeName, &rt.Description, &rt.Active,
		&rt.Version, &rt.Environment, &rt.CreatedAt, &rt.UpdatedAt,
	)

	if err == sql.ErrNoRows {
		return nil, fmt.Errorf("resource type %s not found", resourceTypeID)
	}
	return rt, err
}

// GetResourceTypeByName retrieves a resource type by name and environment
func (r *Repository) GetResourceTypeByName(ctx context.Context, name, environment string) (*ResourceType, error) {
	query := `
		SELECT resource_type_id, resource_type_name, description, active,
			   version, environment, created_at, updated_at
		FROM resource_types
		WHERE resource_type_name = $1 AND environment = $2 AND active = true
		ORDER BY version DESC
		LIMIT 1`

	rt := &ResourceType{}
	err := r.db.QueryRowContext(ctx, query, name, environment).Scan(
		&rt.ResourceTypeID, &rt.ResourceTypeName, &rt.Description, &rt.Active,
		&rt.Version, &rt.Environment, &rt.CreatedAt, &rt.UpdatedAt,
	)

	if err == sql.ErrNoRows {
		return nil, fmt.Errorf("resource type %s not found in environment %s", name, environment)
	}
	return rt, err
}

// ListResourceTypes retrieves all resource types for an environment
func (r *Repository) ListResourceTypes(ctx context.Context, environment string, activeOnly bool) ([]*ResourceType, error) {
	query := `
		SELECT resource_type_id, resource_type_name, description, active,
			   version, environment, created_at, updated_at
		FROM resource_types
		WHERE environment = $1`

	args := []interface{}{environment}
	if activeOnly {
		query += " AND active = $2"
		args = append(args, true)
	}

	query += " ORDER BY resource_type_name, version DESC"

	rows, err := r.db.QueryContext(ctx, query, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var resourceTypes []*ResourceType
	for rows.Next() {
		rt := &ResourceType{}
		err := rows.Scan(
			&rt.ResourceTypeID, &rt.ResourceTypeName, &rt.Description, &rt.Active,
			&rt.Version, &rt.Environment, &rt.CreatedAt, &rt.UpdatedAt,
		)
		if err != nil {
			return nil, err
		}
		resourceTypes = append(resourceTypes, rt)
	}

	return resourceTypes, rows.Err()
}

// ==============================================================================
// Resource Type Endpoint Operations
// ==============================================================================

// GetResourceTypeEndpoint retrieves an endpoint for a resource type and lifecycle action
func (r *Repository) GetResourceTypeEndpoint(ctx context.Context, resourceTypeID, lifecycleAction, environment string) (*ResourceTypeEndpoint, error) {
	query := `
		SELECT endpoint_id, resource_type_id, lifecycle_action, endpoint_url,
			   method, authentication, timeout_seconds, retry_config,
			   environment, created_at, updated_at
		FROM resource_type_endpoints
		WHERE resource_type_id = $1 AND lifecycle_action = $2 AND environment = $3`

	endpoint := &ResourceTypeEndpoint{}
	err := r.db.QueryRowContext(ctx, query, resourceTypeID, lifecycleAction, environment).Scan(
		&endpoint.EndpointID, &endpoint.ResourceTypeID, &endpoint.LifecycleAction,
		&endpoint.EndpointURL, &endpoint.Method, &endpoint.Authentication,
		&endpoint.TimeoutSeconds, &endpoint.RetryConfig, &endpoint.Environment,
		&endpoint.CreatedAt, &endpoint.UpdatedAt,
	)

	if err == sql.ErrNoRows {
		return nil, fmt.Errorf("endpoint for resource type %s, action %s, environment %s not found",
			resourceTypeID, lifecycleAction, environment)
	}
	return endpoint, err
}

// GetResourceTypeEndpointByName retrieves an endpoint by resource type name
func (r *Repository) GetResourceTypeEndpointByName(ctx context.Context, resourceTypeName, lifecycleAction, environment string) (*ResourceTypeEndpoint, error) {
	query := `
		SELECT rte.endpoint_id, rte.resource_type_id, rte.lifecycle_action, rte.endpoint_url,
			   rte.method, rte.authentication, rte.timeout_seconds, rte.retry_config,
			   rte.environment, rte.created_at, rte.updated_at
		FROM resource_type_endpoints rte
		JOIN resource_types rt ON rte.resource_type_id = rt.resource_type_id
		WHERE rt.resource_type_name = $1 AND rte.lifecycle_action = $2
		  AND rte.environment = $3 AND rt.active = true`

	endpoint := &ResourceTypeEndpoint{}
	err := r.db.QueryRowContext(ctx, query, resourceTypeName, lifecycleAction, environment).Scan(
		&endpoint.EndpointID, &endpoint.ResourceTypeID, &endpoint.LifecycleAction,
		&endpoint.EndpointURL, &endpoint.Method, &endpoint.Authentication,
		&endpoint.TimeoutSeconds, &endpoint.RetryConfig, &endpoint.Environment,
		&endpoint.CreatedAt, &endpoint.UpdatedAt,
	)

	if err == sql.ErrNoRows {
		return nil, fmt.Errorf("endpoint for resource type %s, action %s, environment %s not found",
			resourceTypeName, lifecycleAction, environment)
	}
	return endpoint, err
}

// ==============================================================================
// Action Definition Operations
// ==============================================================================

// CreateActionDefinition creates a new action definition
func (r *Repository) CreateActionDefinition(ctx context.Context, action *ActionDefinition) error {
	// Serialize JSON fields
	executionConfigJSON, err := json.Marshal(action.ExecutionConfig)
	if err != nil {
		return fmt.Errorf("failed to marshal execution config: %w", err)
	}

	attributeMappingJSON, err := json.Marshal(action.AttributeMapping)
	if err != nil {
		return fmt.Errorf("failed to marshal attribute mapping: %w", err)
	}

	query := `
		INSERT INTO actions_registry (
			action_id, action_name, verb_pattern, action_type, resource_type_id,
			domain, trigger_conditions, execution_config, attribute_mapping,
			success_criteria, failure_handling, active, version, environment,
			created_at, updated_at
		) VALUES (
			COALESCE(NULLIF($1, ''), uuid_generate_v4()), $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, NOW(), NOW()
		) RETURNING action_id, created_at, updated_at`

	err = r.db.QueryRowContext(ctx, query,
		action.ActionID, action.ActionName, action.VerbPattern, action.ActionType,
		action.ResourceTypeID, action.Domain, action.TriggerConditions,
		executionConfigJSON, attributeMappingJSON, action.SuccessCriteria,
		action.FailureHandling, action.Active, action.Version, action.Environment,
	).Scan(&action.ActionID, &action.CreatedAt, &action.UpdatedAt)

	return err
}

// GetActionDefinition retrieves an action definition by ID
func (r *Repository) GetActionDefinition(ctx context.Context, actionID string) (*ActionDefinition, error) {
	query := `
		SELECT action_id, action_name, verb_pattern, action_type, resource_type_id,
			   domain, trigger_conditions, execution_config, attribute_mapping,
			   success_criteria, failure_handling, active, version, environment,
			   created_at, updated_at
		FROM actions_registry
		WHERE action_id = $1`

	action := &ActionDefinition{}
	var executionConfigJSON, attributeMappingJSON []byte

	err := r.db.QueryRowContext(ctx, query, actionID).Scan(
		&action.ActionID, &action.ActionName, &action.VerbPattern, &action.ActionType,
		&action.ResourceTypeID, &action.Domain, &action.TriggerConditions,
		&executionConfigJSON, &attributeMappingJSON, &action.SuccessCriteria,
		&action.FailureHandling, &action.Active, &action.Version, &action.Environment,
		&action.CreatedAt, &action.UpdatedAt,
	)

	if err == sql.ErrNoRows {
		return nil, fmt.Errorf("action definition %s not found", actionID)
	}
	if err != nil {
		return nil, err
	}

	// Deserialize JSON fields
	if err := json.Unmarshal(executionConfigJSON, &action.ExecutionConfig); err != nil {
		return nil, fmt.Errorf("failed to unmarshal execution config: %w", err)
	}
	if err := json.Unmarshal(attributeMappingJSON, &action.AttributeMapping); err != nil {
		return nil, fmt.Errorf("failed to unmarshal attribute mapping: %w", err)
	}

	return action, nil
}

// GetActionDefinitionsByVerbPattern retrieves action definitions matching a verb pattern
func (r *Repository) GetActionDefinitionsByVerbPattern(ctx context.Context, verbPattern, environment string) ([]*ActionDefinition, error) {
	query := `
		SELECT action_id, action_name, verb_pattern, action_type, resource_type_id,
			   domain, trigger_conditions, execution_config, attribute_mapping,
			   success_criteria, failure_handling, active, version, environment,
			   created_at, updated_at
		FROM actions_registry
		WHERE verb_pattern = $1 AND environment = $2 AND active = true
		ORDER BY created_at DESC`

	rows, err := r.db.QueryContext(ctx, query, verbPattern, environment)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var actions []*ActionDefinition
	for rows.Next() {
		action := &ActionDefinition{}
		var executionConfigJSON, attributeMappingJSON []byte

		err := rows.Scan(
			&action.ActionID, &action.ActionName, &action.VerbPattern, &action.ActionType,
			&action.ResourceTypeID, &action.Domain, &action.TriggerConditions,
			&executionConfigJSON, &attributeMappingJSON, &action.SuccessCriteria,
			&action.FailureHandling, &action.Active, &action.Version, &action.Environment,
			&action.CreatedAt, &action.UpdatedAt,
		)
		if err != nil {
			return nil, err
		}

		// Deserialize JSON fields
		if err := json.Unmarshal(executionConfigJSON, &action.ExecutionConfig); err != nil {
			return nil, fmt.Errorf("failed to unmarshal execution config: %w", err)
		}
		if err := json.Unmarshal(attributeMappingJSON, &action.AttributeMapping); err != nil {
			return nil, fmt.Errorf("failed to unmarshal attribute mapping: %w", err)
		}

		actions = append(actions, action)
	}

	return actions, rows.Err()
}

// ==============================================================================
// Action Execution Operations
// ==============================================================================

// CreateActionExecution creates a new action execution record
func (r *Repository) CreateActionExecution(ctx context.Context, execution *ActionExecution) error {
	query := `
		INSERT INTO action_executions (
			execution_id, action_id, cbu_id, dsl_version_id, execution_status,
			trigger_context, request_payload, response_payload, result_attributes,
			error_details, execution_duration_ms, started_at, completed_at,
			retry_count, next_retry_at, idempotency_key, correlation_id,
			trace_id, span_id, http_status, endpoint, headers
		) VALUES (
			COALESCE(NULLIF($1, ''), uuid_generate_v4()), $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22
		) RETURNING execution_id, started_at`

	err := r.db.QueryRowContext(ctx, query,
		execution.ExecutionID, execution.ActionID, execution.CBUID, execution.DSLVersionID,
		execution.ExecutionStatus, execution.TriggerContext, execution.RequestPayload,
		execution.ResponsePayload, execution.ResultAttributes, execution.ErrorDetails,
		execution.ExecutionDurationMS, execution.StartedAt, execution.CompletedAt,
		execution.RetryCount, execution.NextRetryAt, execution.IdempotencyKey,
		execution.CorrelationID, execution.TraceID, execution.SpanID,
		execution.HTTPStatus, execution.Endpoint, execution.Headers,
	).Scan(&execution.ExecutionID, &execution.StartedAt)

	return err
}

// UpdateActionExecution updates an existing action execution
func (r *Repository) UpdateActionExecution(ctx context.Context, execution *ActionExecution) error {
	query := `
		UPDATE action_executions SET
			execution_status = $2,
			response_payload = $3,
			result_attributes = $4,
			error_details = $5,
			execution_duration_ms = $6,
			completed_at = $7,
			retry_count = $8,
			next_retry_at = $9,
			http_status = $10,
			endpoint = $11,
			headers = $12
		WHERE execution_id = $1`

	result, err := r.db.ExecContext(ctx, query,
		execution.ExecutionID, execution.ExecutionStatus, execution.ResponsePayload,
		execution.ResultAttributes, execution.ErrorDetails, execution.ExecutionDurationMS,
		execution.CompletedAt, execution.RetryCount, execution.NextRetryAt,
		execution.HTTPStatus, execution.Endpoint, execution.Headers,
	)

	if err != nil {
		return err
	}

	rowsAffected, err := result.RowsAffected()
	if err != nil {
		return err
	}
	if rowsAffected == 0 {
		return fmt.Errorf("action execution %s not found", execution.ExecutionID)
	}

	return nil
}

// GetActionExecution retrieves an action execution by ID
func (r *Repository) GetActionExecution(ctx context.Context, executionID string) (*ActionExecution, error) {
	query := `
		SELECT execution_id, action_id, cbu_id, dsl_version_id, execution_status,
			   trigger_context, request_payload, response_payload, result_attributes,
			   error_details, execution_duration_ms, started_at, completed_at,
			   retry_count, next_retry_at, idempotency_key, correlation_id,
			   trace_id, span_id, http_status, endpoint, headers
		FROM action_executions
		WHERE execution_id = $1`

	execution := &ActionExecution{}
	err := r.db.QueryRowContext(ctx, query, executionID).Scan(
		&execution.ExecutionID, &execution.ActionID, &execution.CBUID, &execution.DSLVersionID,
		&execution.ExecutionStatus, &execution.TriggerContext, &execution.RequestPayload,
		&execution.ResponsePayload, &execution.ResultAttributes, &execution.ErrorDetails,
		&execution.ExecutionDurationMS, &execution.StartedAt, &execution.CompletedAt,
		&execution.RetryCount, &execution.NextRetryAt, &execution.IdempotencyKey,
		&execution.CorrelationID, &execution.TraceID, &execution.SpanID,
		&execution.HTTPStatus, &execution.Endpoint, &execution.Headers,
	)

	if err == sql.ErrNoRows {
		return nil, fmt.Errorf("action execution %s not found", executionID)
	}
	return execution, err
}

// GetActionExecutionByIdempotencyKey retrieves an execution by idempotency key
func (r *Repository) GetActionExecutionByIdempotencyKey(ctx context.Context, idempotencyKey string) (*ActionExecution, error) {
	query := `
		SELECT execution_id, action_id, cbu_id, dsl_version_id, execution_status,
			   trigger_context, request_payload, response_payload, result_attributes,
			   error_details, execution_duration_ms, started_at, completed_at,
			   retry_count, next_retry_at, idempotency_key, correlation_id,
			   trace_id, span_id, http_status, endpoint, headers
		FROM action_executions
		WHERE idempotency_key = $1`

	execution := &ActionExecution{}
	err := r.db.QueryRowContext(ctx, query, idempotencyKey).Scan(
		&execution.ExecutionID, &execution.ActionID, &execution.CBUID, &execution.DSLVersionID,
		&execution.ExecutionStatus, &execution.TriggerContext, &execution.RequestPayload,
		&execution.ResponsePayload, &execution.ResultAttributes, &execution.ErrorDetails,
		&execution.ExecutionDurationMS, &execution.StartedAt, &execution.CompletedAt,
		&execution.RetryCount, &execution.NextRetryAt, &execution.IdempotencyKey,
		&execution.CorrelationID, &execution.TraceID, &execution.SpanID,
		&execution.HTTPStatus, &execution.Endpoint, &execution.Headers,
	)

	if err == sql.ErrNoRows {
		return nil, fmt.Errorf("action execution with idempotency key %s not found", idempotencyKey)
	}
	return execution, err
}

// ListActionExecutionsByCBU retrieves action executions for a CBU
func (r *Repository) ListActionExecutionsByCBU(ctx context.Context, cbuID string, limit int) ([]*ActionExecution, error) {
	query := `
		SELECT execution_id, action_id, cbu_id, dsl_version_id, execution_status,
			   trigger_context, request_payload, response_payload, result_attributes,
			   error_details, execution_duration_ms, started_at, completed_at,
			   retry_count, next_retry_at, idempotency_key, correlation_id,
			   trace_id, span_id, http_status, endpoint, headers
		FROM action_executions
		WHERE cbu_id = $1
		ORDER BY started_at DESC
		LIMIT $2`

	rows, err := r.db.QueryContext(ctx, query, cbuID, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var executions []*ActionExecution
	for rows.Next() {
		execution := &ActionExecution{}
		err := rows.Scan(
			&execution.ExecutionID, &execution.ActionID, &execution.CBUID, &execution.DSLVersionID,
			&execution.ExecutionStatus, &execution.TriggerContext, &execution.RequestPayload,
			&execution.ResponsePayload, &execution.ResultAttributes, &execution.ErrorDetails,
			&execution.ExecutionDurationMS, &execution.StartedAt, &execution.CompletedAt,
			&execution.RetryCount, &execution.NextRetryAt, &execution.IdempotencyKey,
			&execution.CorrelationID, &execution.TraceID, &execution.SpanID,
			&execution.HTTPStatus, &execution.Endpoint, &execution.Headers,
		)
		if err != nil {
			return nil, err
		}
		executions = append(executions, execution)
	}

	return executions, rows.Err()
}

// ==============================================================================
// Utility Operations
// ==============================================================================

// GenerateIdempotencyKey generates an idempotency key using database function
func (r *Repository) GenerateIdempotencyKey(ctx context.Context, template, resourceTypeName, environment, cbuID, actionID, dslVersionID string) (string, error) {
	query := `SELECT generate_idempotency_key($1, $2, $3, $4, $5, $6)`

	var idempotencyKey string
	err := r.db.QueryRowContext(ctx, query, template, resourceTypeName, environment, cbuID, actionID, dslVersionID).Scan(&idempotencyKey)
	return idempotencyKey, err
}

// GenerateCorrelationID generates a correlation ID using database function
func (r *Repository) GenerateCorrelationID(ctx context.Context, template, cbuID, actionID, resourceTypeName string) (string, error) {
	query := `SELECT generate_correlation_id($1, $2, $3, $4)`

	var correlationID string
	err := r.db.QueryRowContext(ctx, query, template, cbuID, actionID, resourceTypeName).Scan(&correlationID)
	return correlationID, err
}

// GetResourceEndpointURL gets an endpoint URL using database function
func (r *Repository) GetResourceEndpointURL(ctx context.Context, resourceTypeName, lifecycleAction, environment string) (string, error) {
	query := `SELECT get_resource_endpoint_url($1, $2, $3)`

	var endpointURL sql.NullString
	err := r.db.QueryRowContext(ctx, query, resourceTypeName, lifecycleAction, environment).Scan(&endpointURL)

	if err != nil {
		return "", err
	}
	if !endpointURL.Valid {
		return "", fmt.Errorf("no endpoint found for resource type %s, action %s, environment %s",
			resourceTypeName, lifecycleAction, environment)
	}

	return endpointURL.String, nil
}
