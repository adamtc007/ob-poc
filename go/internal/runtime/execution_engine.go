package runtime

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"regexp"
	"strings"
	"time"

	"dsl-ob-poc/internal/datastore"
)

// ExecutionEngine orchestrates the execution of DSL actions
type ExecutionEngine struct {
	repository        *Repository
	dataStore         datastore.DataStore
	httpClient        *HTTPClient
	attributeResolver *AttributeResolver
	credentialMgr     *CredentialManager
}

// NewExecutionEngine creates a new execution engine
func NewExecutionEngine(db *sql.DB, dataStore datastore.DataStore) (*ExecutionEngine, error) {
	repository := NewRepository(db)

	credentialMgr, err := NewCredentialManager(db)
	if err != nil {
		return nil, fmt.Errorf("failed to create credential manager: %w", err)
	}

	httpClient := NewHTTPClient(credentialMgr)
	attributeResolver := NewAttributeResolver(dataStore)

	return &ExecutionEngine{
		repository:        repository,
		dataStore:         dataStore,
		httpClient:        httpClient,
		attributeResolver: attributeResolver,
		credentialMgr:     credentialMgr,
	}, nil
}

// ExecuteAction executes a single action based on execution request
func (ee *ExecutionEngine) ExecuteAction(ctx context.Context, req *ExecutionRequest) (*ExecutionResult, error) {
	startTime := time.Now()

	// Get action definition
	actionDef, err := ee.repository.GetActionDefinition(ctx, req.ActionID)
	if err != nil {
		return nil, fmt.Errorf("failed to get action definition: %w", err)
	}

	// Create execution record
	execution := &ActionExecution{
		ActionID:        req.ActionID,
		CBUID:           req.CBUID,
		DSLVersionID:    req.DSLVersionID,
		ExecutionStatus: ExecutionStatusPending,
		TriggerContext:  mustMarshalJSON(req.TriggerContext),
		TraceID:         req.TraceID,
		SpanID:          req.SpanID,
	}

	// Generate idempotency key and correlation ID
	if err := ee.generateExecutionMetadata(ctx, execution, actionDef, req); err != nil {
		return nil, fmt.Errorf("failed to generate execution metadata: %w", err)
	}

	// Check for existing execution with same idempotency key
	if execution.IdempotencyKey != nil {
		existing, err := ee.repository.GetActionExecutionByIdempotencyKey(ctx, *execution.IdempotencyKey)
		if err == nil {
			// Return existing result if completed
			if existing.ExecutionStatus == ExecutionStatusCompleted {
				return ee.buildExecutionResult(existing, time.Since(startTime)), nil
			}
			// If still running or failed, continue with new execution
		}
	}

	// Create execution record in database
	if err := ee.repository.CreateActionExecution(ctx, execution); err != nil {
		return nil, fmt.Errorf("failed to create execution record: %w", err)
	}

	// Execute with comprehensive tracking
	result := ee.executeWithTracking(ctx, execution, actionDef, req)

	// Update execution record with final result
	ee.updateExecutionRecord(ctx, execution, result)

	return result, nil
}

// executeWithTracking executes action with comprehensive tracking and retry logic
func (ee *ExecutionEngine) executeWithTracking(ctx context.Context, execution *ActionExecution, actionDef *ActionDefinition, req *ExecutionRequest) *ExecutionResult {
	startTime := time.Now()

	// Update status to running
	execution.ExecutionStatus = ExecutionStatusRunning
	_ = ee.repository.UpdateActionExecution(ctx, execution)

	// Resolve attributes
	resolutionCtx := &AttributeResolutionContext{
		CBUID:        req.CBUID,
		DSLVersionID: req.DSLVersionID,
		Environment:  req.Environment,
		ExtraContext: req.AttributeValues,
	}

	// Get DSL content for resolution
	dslContent, err := ee.dataStore.GetLatestDSL(ctx, req.CBUID)
	if err == nil {
		resolutionCtx.DSLContent = dslContent
	}

	resolvedAttributes, err := ee.attributeResolver.ResolveAttributesForAction(ctx, actionDef, resolutionCtx)
	if err != nil {
		return ee.buildFailureResult(execution.ExecutionID, fmt.Sprintf("attribute resolution failed: %v", err), time.Since(startTime))
	}

	// Validate required attributes
	if actionDef.ResourceTypeID != nil {
		if err := ee.attributeResolver.ValidateRequiredAttributes(ctx, *actionDef.ResourceTypeID, resolvedAttributes); err != nil {
			return ee.buildFailureResult(execution.ExecutionID, fmt.Sprintf("attribute validation failed: %v", err), time.Since(startTime))
		}
	}

	// Build API request payload
	requestPayload, err := ee.attributeResolver.BuildAPIRequestPayload(ctx, actionDef.AttributeMapping, resolvedAttributes)
	if err != nil {
		return ee.buildFailureResult(execution.ExecutionID, fmt.Sprintf("payload construction failed: %v", err), time.Since(startTime))
	}

	// Store request payload
	execution.RequestPayload = mustMarshalJSON(requestPayload)

	// Execute with retry logic
	return ee.executeWithRetry(ctx, execution, actionDef, requestPayload, time.Since(startTime))
}

// executeWithRetry executes the action with retry logic
func (ee *ExecutionEngine) executeWithRetry(ctx context.Context, execution *ActionExecution, actionDef *ActionDefinition, requestPayload map[string]interface{}, baseDuration time.Duration) *ExecutionResult {
	maxRetries := actionDef.ExecutionConfig.RetryConfig.MaxRetries
	var lastResponse *APIResponse
	var lastErr error

	for attempt := 0; attempt <= maxRetries; attempt++ {
		attemptStart := time.Now()

		// Create execution attempt record
		attemptRecord := &ActionExecutionAttempt{
			ExecutionID:    execution.ExecutionID,
			AttemptNo:      attempt + 1,
			Status:         ExecutionStatusRunning,
			RequestPayload: mustMarshalJSON(requestPayload),
		}

		// Resolve endpoint URL
		endpointURL, err := ee.resolveEndpointURL(ctx, actionDef)
		if err != nil {
			lastErr = err
			continue
		}

		attemptRecord.EndpointURL = &endpointURL

		// Build API request
		apiRequest := &APIRequest{
			Method:         actionDef.ExecutionConfig.Method,
			URL:            endpointURL,
			Body:           requestPayload,
			Headers:        make(map[string]string),
			Authentication: actionDef.ExecutionConfig.Authentication,
			TimeoutSeconds: actionDef.ExecutionConfig.TimeoutSeconds,
			IdempotencyKey: execution.IdempotencyKey,
			CorrelationID:  execution.CorrelationID,
			TraceID:        execution.TraceID,
			SpanID:         execution.SpanID,
		}

		// Execute HTTP request
		response, err := ee.httpClient.Execute(ctx, apiRequest)
		lastResponse = response
		lastErr = err

		// Update attempt record
		attemptRecord.CompletedAt = timePtr(time.Now())
		attemptRecord.DurationMS = intPtr(int(time.Since(attemptStart).Milliseconds()))

		if response != nil {
			attemptRecord.HTTPStatus = &response.StatusCode
			attemptRecord.ResponsePayload = mustMarshalJSON(response.Body)
			attemptRecord.ResponseHeaders = mustMarshalJSON(response.Headers)
		}

		if err != nil {
			attemptRecord.Status = ExecutionStatusFailed
			attemptRecord.ErrorDetails = mustMarshalJSON(map[string]interface{}{
				"error": err.Error(),
			})
		} else {
			attemptRecord.Status = ExecutionStatusCompleted
		}

		// Store attempt record (would need to implement this in repository)
		// ee.repository.CreateActionExecutionAttempt(ctx, attemptRecord)

		// Check if we should retry
		if err == nil && response != nil {
			// Validate response against success criteria
			var successCriteria SuccessCriteria
			if err := json.Unmarshal(actionDef.SuccessCriteria, &successCriteria); err == nil {
				if err := ee.httpClient.ValidateResponse(response, successCriteria); err == nil {
					// Success! Process response and return
					return ee.processSuccessfulResponse(ctx, execution, actionDef, response, baseDuration+time.Since(attemptStart))
				}
			}
		}

		// Check if we should retry based on failure handling
		var failureHandling FailureHandling
		if len(actionDef.FailureHandling) > 0 {
			_ = json.Unmarshal(actionDef.FailureHandling, &failureHandling)
		}

		shouldRetry := ee.httpClient.ShouldRetry(response, failureHandling, attempt, maxRetries)
		if !shouldRetry {
			break
		}

		// Calculate backoff delay
		delay := ee.httpClient.CalculateBackoffDelay(attempt, actionDef.ExecutionConfig.RetryConfig)
		time.Sleep(delay)

		execution.RetryCount = attempt + 1
	}

	// All retries exhausted - return failure
	errorMsg := "execution failed after all retries"
	if lastErr != nil {
		errorMsg = fmt.Sprintf("execution failed: %v", lastErr)
	} else if lastResponse != nil && lastResponse.Error != nil {
		errorMsg = fmt.Sprintf("execution failed: %s", *lastResponse.Error)
	}

	return ee.buildFailureResult(execution.ExecutionID, errorMsg, baseDuration)
}

// processSuccessfulResponse processes a successful API response
func (ee *ExecutionEngine) processSuccessfulResponse(ctx context.Context, execution *ActionExecution, actionDef *ActionDefinition, response *APIResponse, duration time.Duration) *ExecutionResult {
	// Extract result attributes from response
	resultAttributes := make(map[string]interface{})

	for _, mapping := range actionDef.AttributeMapping.OutputMapping {
		// Simple JSONPath-like extraction (in production, use a proper JSONPath library)
		value := ee.extractFromResponse(response.Body, mapping.APIResponsePath)
		if value != nil {
			resultAttributes[mapping.AttributeName] = value

			// Store the result attribute in the database if attribute ID is provided
			if mapping.DSLAttributeID != "" {
				valueJSON, _ := json.Marshal(value)
				// Get latest DSL version for this CBU
				if dslVersion, err := ee.dataStore.GetLatestDSLWithState(ctx, execution.CBUID); err == nil {
					_ = ee.dataStore.UpsertAttributeValue(ctx, execution.CBUID, dslVersion.VersionNumber, mapping.DSLAttributeID, valueJSON, "resolved", map[string]any{
						"source":       "api_response",
						"execution_id": execution.ExecutionID,
						"endpoint":     response.Headers["endpoint"],
						"resolved_at":  time.Now().Format(time.RFC3339),
					})
				}
			}
		}
	}

	return &ExecutionResult{
		ExecutionID:      execution.ExecutionID,
		Success:          true,
		HTTPStatus:       &response.StatusCode,
		ResponsePayload:  response.Body,
		ResultAttributes: resultAttributes,
		DurationMS:       int(duration.Milliseconds()),
		IdempotencyKey:   execution.IdempotencyKey,
		CorrelationID:    execution.CorrelationID,
	}
}

// resolveEndpointURL resolves the endpoint URL for the action
func (ee *ExecutionEngine) resolveEndpointURL(ctx context.Context, actionDef *ActionDefinition) (string, error) {
	endpointURL := actionDef.ExecutionConfig.EndpointURL

	// Handle LOOKUP: prefix for dynamic endpoint resolution
	if strings.HasPrefix(endpointURL, "LOOKUP:") {
		lookupKey := strings.TrimPrefix(endpointURL, "LOOKUP:")

		if lookupKey == "resource_type.create" && actionDef.ResourceTypeID != nil {
			// Get resource type information
			_, err := ee.repository.GetResourceType(ctx, *actionDef.ResourceTypeID)
			if err != nil {
				// Fall back to configured fallback URL
				if actionDef.ExecutionConfig.EndpointLookupFallback != "" {
					return actionDef.ExecutionConfig.EndpointLookupFallback, nil
				}
				return "", fmt.Errorf("failed to resolve resource type endpoint: %w", err)
			}

			// Get the create endpoint for this resource type
			endpoint, err := ee.repository.GetResourceTypeEndpoint(ctx, *actionDef.ResourceTypeID, "create", actionDef.Environment)
			if err != nil {
				// Fall back to configured fallback URL
				if actionDef.ExecutionConfig.EndpointLookupFallback != "" {
					return actionDef.ExecutionConfig.EndpointLookupFallback, nil
				}
				return "", fmt.Errorf("failed to get resource type endpoint: %w", err)
			}

			return endpoint.EndpointURL, nil
		}

		return "", fmt.Errorf("unsupported lookup key: %s", lookupKey)
	}

	return endpointURL, nil
}

// extractFromResponse extracts a value from API response using JSONPath-like syntax
func (ee *ExecutionEngine) extractFromResponse(responseBody map[string]interface{}, path string) interface{} {
	// Simple JSONPath implementation - in production use a proper library
	if path == "" {
		return nil
	}

	// Remove leading $ if present
	path = strings.TrimPrefix(path, "$.")

	// Split path into parts
	parts := strings.Split(path, ".")
	var current interface{} = responseBody

	for _, part := range parts {
		if current == nil {
			return nil
		}

		switch v := current.(type) {
		case map[string]interface{}:
			if val, exists := v[part]; exists {
				current = val
			} else {
				return nil
			}
		default:
			return nil
		}
	}

	return current
}

// generateExecutionMetadata generates idempotency key and correlation ID
func (ee *ExecutionEngine) generateExecutionMetadata(ctx context.Context, execution *ActionExecution, actionDef *ActionDefinition, req *ExecutionRequest) error {
	// Generate idempotency key if configured
	if actionDef.ExecutionConfig.Idempotency.KeyTemplate != "" {
		resourceTypeName := ""
		if actionDef.ResourceTypeID != nil {
			if rt, err := ee.repository.GetResourceType(ctx, *actionDef.ResourceTypeID); err == nil {
				resourceTypeName = rt.ResourceTypeName
			}
		}

		idempotencyKey, err := ee.repository.GenerateIdempotencyKey(
			ctx,
			actionDef.ExecutionConfig.Idempotency.KeyTemplate,
			resourceTypeName,
			req.Environment,
			req.CBUID,
			req.ActionID,
			req.DSLVersionID,
		)
		if err != nil {
			return fmt.Errorf("failed to generate idempotency key: %w", err)
		}
		execution.IdempotencyKey = &idempotencyKey
	}

	// Generate correlation ID if configured
	if actionDef.ExecutionConfig.Telemetry.CorrelationIDTemplate != "" {
		resourceTypeName := ""
		if actionDef.ResourceTypeID != nil {
			if rt, err := ee.repository.GetResourceType(ctx, *actionDef.ResourceTypeID); err == nil {
				resourceTypeName = rt.ResourceTypeName
			}
		}

		correlationID, err := ee.repository.GenerateCorrelationID(
			ctx,
			actionDef.ExecutionConfig.Telemetry.CorrelationIDTemplate,
			req.CBUID,
			req.ActionID,
			resourceTypeName,
		)
		if err != nil {
			return fmt.Errorf("failed to generate correlation ID: %w", err)
		}
		execution.CorrelationID = &correlationID
	}

	return nil
}

// updateExecutionRecord updates the execution record with final results
func (ee *ExecutionEngine) updateExecutionRecord(ctx context.Context, execution *ActionExecution, result *ExecutionResult) {
	now := time.Now()
	execution.CompletedAt = &now
	execution.ExecutionDurationMS = &result.DurationMS

	if result.Success {
		execution.ExecutionStatus = ExecutionStatusCompleted
		execution.ResponsePayload = mustMarshalJSON(result.ResponsePayload)
		execution.ResultAttributes = mustMarshalJSON(result.ResultAttributes)
		if result.HTTPStatus != nil {
			execution.HTTPStatus = result.HTTPStatus
		}
	} else {
		execution.ExecutionStatus = ExecutionStatusFailed
		if result.ErrorDetails != nil {
			execution.ErrorDetails = mustMarshalJSON(map[string]interface{}{
				"error": *result.ErrorDetails,
			})
		}
	}

	_ = ee.repository.UpdateActionExecution(ctx, execution)
}

// buildExecutionResult creates an ExecutionResult from ActionExecution
func (ee *ExecutionEngine) buildExecutionResult(execution *ActionExecution, duration time.Duration) *ExecutionResult {
	result := &ExecutionResult{
		ExecutionID:    execution.ExecutionID,
		Success:        execution.ExecutionStatus == ExecutionStatusCompleted,
		DurationMS:     int(duration.Milliseconds()),
		IdempotencyKey: execution.IdempotencyKey,
		CorrelationID:  execution.CorrelationID,
	}

	if execution.HTTPStatus != nil {
		result.HTTPStatus = execution.HTTPStatus
	}

	if len(execution.ResponsePayload) > 0 {
		var responsePayload map[string]interface{}
		if err := json.Unmarshal(execution.ResponsePayload, &responsePayload); err == nil {
			result.ResponsePayload = responsePayload
		}
	}

	if len(execution.ResultAttributes) > 0 {
		var resultAttributes map[string]interface{}
		if err := json.Unmarshal(execution.ResultAttributes, &resultAttributes); err == nil {
			result.ResultAttributes = resultAttributes
		}
	}

	if len(execution.ErrorDetails) > 0 {
		var errorDetails map[string]interface{}
		if err := json.Unmarshal(execution.ErrorDetails, &errorDetails); err == nil {
			if errStr, ok := errorDetails["error"].(string); ok {
				result.ErrorDetails = &errStr
			}
		}
	}

	return result
}

// buildFailureResult creates a failure ExecutionResult
func (ee *ExecutionEngine) buildFailureResult(executionID, errorMsg string, duration time.Duration) *ExecutionResult {
	return &ExecutionResult{
		ExecutionID:  executionID,
		Success:      false,
		ErrorDetails: &errorMsg,
		DurationMS:   int(duration.Milliseconds()),
	}
}

// Helper functions

func mustMarshalJSON(v interface{}) json.RawMessage {
	if v == nil {
		return json.RawMessage("{}")
	}
	data, err := json.Marshal(v)
	if err != nil {
		return json.RawMessage("{}")
	}
	return data
}

func timePtr(t time.Time) *time.Time {
	return &t
}

func intPtr(i int) *int {
	return &i
}

// ==============================================================================
// Action Triggering and Discovery
// ==============================================================================

// TriggerActionsForDSLChange finds and triggers actions based on DSL state changes
func (ee *ExecutionEngine) TriggerActionsForDSLChange(ctx context.Context, cbuID, dslVersionID, dslContent, environment string) ([]*ExecutionResult, error) {
	// Parse DSL to extract verbs that were used
	verbs := ee.extractVerbsFromDSL(dslContent)

	var results []*ExecutionResult

	// For each verb, find matching action definitions
	for _, verb := range verbs {
		actions, err := ee.repository.GetActionDefinitionsByVerbPattern(ctx, verb, environment)
		if err != nil {
			continue // Skip if no actions found for this verb
		}

		// Execute each matching action
		for _, actionDef := range actions {
			// Check trigger conditions
			if ee.evaluateTriggerConditions(ctx, actionDef, dslContent, cbuID) {
				req := &ExecutionRequest{
					ActionID:     actionDef.ActionID,
					CBUID:        cbuID,
					DSLVersionID: dslVersionID,
					Environment:  environment,
					TriggerContext: map[string]interface{}{
						"triggered_by": "dsl_change",
						"verb":         verb,
						"dsl_content":  dslContent,
						"triggered_at": time.Now().Format(time.RFC3339),
					},
				}

				result, err := ee.ExecuteAction(ctx, req)
				if err != nil {
					// Log error but continue with other actions
					result = &ExecutionResult{
						Success:      false,
						ErrorDetails: stringPtr(err.Error()),
					}
				}

				results = append(results, result)
			}
		}
	}

	return results, nil
}

// extractVerbsFromDSL extracts verb patterns from DSL content
func (ee *ExecutionEngine) extractVerbsFromDSL(dslContent string) []string {
	// Simple regex to extract verbs like "resources.create", "kyc.start", etc.
	verbPattern := `\(([a-zA-Z_]+\.[a-zA-Z_]+)`
	re, _ := regexp.Compile(verbPattern)
	matches := re.FindAllStringSubmatch(dslContent, -1)

	var verbs []string
	seen := make(map[string]bool)

	for _, match := range matches {
		if len(match) > 1 {
			verb := match[1]
			if !seen[verb] {
				verbs = append(verbs, verb)
				seen[verb] = true
			}
		}
	}

	return verbs
}

// evaluateTriggerConditions checks if trigger conditions are met
func (ee *ExecutionEngine) evaluateTriggerConditions(ctx context.Context, actionDef *ActionDefinition, dslContent, cbuID string) bool {
	if len(actionDef.TriggerConditions) == 0 {
		return true // No conditions means always trigger
	}

	var conditions TriggerConditions
	if err := json.Unmarshal(actionDef.TriggerConditions, &conditions); err != nil {
		return false
	}

	// Check domain condition
	if conditions.Domain != nil && *conditions.Domain != "" {
		// This would need to be checked against the CBU's domain or session context
		// For now, assume it matches - placeholder for domain checking logic
		return true
	}

	// Check state condition
	if conditions.State != nil && *conditions.State != "" {
		// Check if DSL content indicates we're in the required state
		// This is a simplified check - in production you'd have more sophisticated state detection
		if !strings.Contains(dslContent, strings.ToLower(*conditions.State)) {
			return false
		}
	}

	// Check attribute requirements
	if len(conditions.AttributeRequirements) > 0 {
		for _, requiredAttr := range conditions.AttributeRequirements {
			// Check if the required attribute is available in DSL or can be resolved
			// This is a simplified check
			if !strings.Contains(dslContent, requiredAttr) {
				// Try to resolve from database
				if _, _, _, err := ee.dataStore.ResolveValueFor(ctx, cbuID, requiredAttr); err != nil {
					return false // Required attribute not available
				}
			}
		}
	}

	return true
}

func stringPtr(s string) *string {
	return &s
}
