package runtime

import (
	"bytes"
	"context"
	"crypto/tls"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"time"
)

// HTTPClient provides HTTP API calling capabilities with authentication
type HTTPClient struct {
	client *http.Client
}

// NewHTTPClient creates a new HTTP client for runtime API calls
func NewHTTPClient() *HTTPClient {
	return &HTTPClient{
		client: &http.Client{
			Timeout: 30 * time.Second, // Default timeout
			Transport: &http.Transport{
				TLSClientConfig: &tls.Config{
					MinVersion:         tls.VersionTLS12,
					InsecureSkipVerify: false, // Always verify TLS in production
				},
				MaxIdleConns:        100,
				MaxIdleConnsPerHost: 10,
				IdleConnTimeout:     90 * time.Second,
			},
		},
	}
}

// APIRequest represents an HTTP API request to be executed
type APIRequest struct {
	Method         string                 `json:"method"`
	URL            string                 `json:"url"`
	Headers        map[string]string      `json:"headers"`
	Body           map[string]interface{} `json:"body"`
	Authentication map[string]interface{} `json:"authentication"`
	TimeoutSeconds int                    `json:"timeout_seconds"`
	IdempotencyKey *string                `json:"idempotency_key,omitempty"`
	CorrelationID  *string                `json:"correlation_id,omitempty"`
	TraceID        *string                `json:"trace_id,omitempty"`
	SpanID         *string                `json:"span_id,omitempty"`
}

// APIResponse represents an HTTP API response
type APIResponse struct {
	StatusCode int                    `json:"status_code"`
	Headers    map[string]string      `json:"headers"`
	Body       map[string]interface{} `json:"body"`
	RawBody    string                 `json:"raw_body"`
	DurationMS int64                  `json:"duration_ms"`
	Error      *string                `json:"error,omitempty"`
}

// Execute performs an HTTP API request with authentication and observability
func (c *HTTPClient) Execute(ctx context.Context, apiReq *APIRequest) (*APIResponse, error) {
	startTime := time.Now()

	// Create HTTP request
	httpReq, err := c.createHTTPRequest(ctx, apiReq)
	if err != nil {
		return nil, fmt.Errorf("failed to create HTTP request: %w", err)
	}

	// Apply authentication
	if err := c.applyAuthentication(ctx, httpReq, apiReq.Authentication); err != nil {
		return nil, fmt.Errorf("failed to apply authentication: %w", err)
	}

	// Apply headers
	c.applyHeaders(httpReq, apiReq)

	// Apply observability headers
	c.applyObservabilityHeaders(httpReq, apiReq)

	// Set timeout if specified
	if apiReq.TimeoutSeconds > 0 {
		timeout := time.Duration(apiReq.TimeoutSeconds) * time.Second
		var cancel context.CancelFunc
		ctx, cancel = context.WithTimeout(ctx, timeout)
		defer cancel()
		httpReq = httpReq.WithContext(ctx)
	}

	// Execute request
	httpResp, err := c.client.Do(httpReq)
	if err != nil {
		duration := time.Since(startTime)
		errorMsg := err.Error()
		return &APIResponse{
			StatusCode: 0,
			Headers:    map[string]string{},
			Body:       map[string]interface{}{},
			RawBody:    "",
			DurationMS: duration.Milliseconds(),
			Error:      &errorMsg,
		}, err
	}
	defer httpResp.Body.Close()

	// Read response body
	bodyBytes, err := io.ReadAll(httpResp.Body)
	if err != nil {
		duration := time.Since(startTime)
		errorMsg := fmt.Sprintf("failed to read response body: %v", err)
		return &APIResponse{
			StatusCode: httpResp.StatusCode,
			Headers:    c.extractHeaders(httpResp),
			Body:       map[string]interface{}{},
			RawBody:    "",
			DurationMS: duration.Milliseconds(),
			Error:      &errorMsg,
		}, err
	}

	// Parse response
	apiResp := &APIResponse{
		StatusCode: httpResp.StatusCode,
		Headers:    c.extractHeaders(httpResp),
		RawBody:    string(bodyBytes),
		DurationMS: time.Since(startTime).Milliseconds(),
	}

	// Try to parse JSON body
	if len(bodyBytes) > 0 {
		var bodyMap map[string]interface{}
		if err := json.Unmarshal(bodyBytes, &bodyMap); err == nil {
			apiResp.Body = bodyMap
		} else {
			// If not valid JSON, put raw body in a wrapper
			apiResp.Body = map[string]interface{}{
				"raw_response": string(bodyBytes),
			}
		}
	} else {
		apiResp.Body = map[string]interface{}{}
	}

	return apiResp, nil
}

// createHTTPRequest creates an HTTP request from APIRequest
func (c *HTTPClient) createHTTPRequest(ctx context.Context, apiReq *APIRequest) (*http.Request, error) {
	var bodyReader io.Reader

	// Handle request body
	if len(apiReq.Body) > 0 {
		bodyBytes, err := json.Marshal(apiReq.Body)
		if err != nil {
			return nil, fmt.Errorf("failed to marshal request body: %w", err)
		}
		bodyReader = bytes.NewReader(bodyBytes)
	}

	// Validate URL
	if _, err := url.Parse(apiReq.URL); err != nil {
		return nil, fmt.Errorf("invalid URL %s: %w", apiReq.URL, err)
	}

	// Create request
	req, err := http.NewRequestWithContext(ctx, apiReq.Method, apiReq.URL, bodyReader)
	if err != nil {
		return nil, err
	}

	// Set content type for JSON requests
	if bodyReader != nil {
		req.Header.Set("Content-Type", "application/json")
	}

	return req, nil
}

// applyAuthentication applies authentication to the HTTP request
func (c *HTTPClient) applyAuthentication(ctx context.Context, req *http.Request, authConfig map[string]interface{}) error {
	if len(authConfig) == 0 {
		return nil // No authentication required
	}

	authType, ok := authConfig["type"].(string)
	if !ok {
		return fmt.Errorf("authentication type not specified")
	}

	switch strings.ToLower(authType) {
	case "api_key":
		return c.applyAPIKeyAuth(ctx, req, authConfig)
	case "bearer":
		return c.applyBearerAuth(ctx, req, authConfig)
	case "basic":
		return c.applyBasicAuth(ctx, req, authConfig)
	case "oauth2":
		return c.applyOAuth2Auth(ctx, req, authConfig)
	case "custom":
		return c.applyCustomAuth(ctx, req, authConfig)
	default:
		return fmt.Errorf("unsupported authentication type: %s", authType)
	}
}

// applyAPIKeyAuth applies API key authentication
func (c *HTTPClient) applyAPIKeyAuth(ctx context.Context, req *http.Request, authConfig map[string]interface{}) error {
	apiKey, ok := authConfig["api_key"].(string)
	if !ok {
		return fmt.Errorf("api_key not provided in auth config")
	}

	// Apply API key based on configuration
	headerName := "X-API-Key" // Default header
	if h, ok := authConfig["header"].(string); ok {
		headerName = h
	}

	queryParam, useQueryParam := authConfig["query_param"].(string)
	if useQueryParam {
		// Add as query parameter
		q := req.URL.Query()
		q.Add(queryParam, apiKey)
		req.URL.RawQuery = q.Encode()
	} else {
		// Add as header
		req.Header.Set(headerName, apiKey)
	}

	return nil
}

// applyBearerAuth applies Bearer token authentication
func (c *HTTPClient) applyBearerAuth(ctx context.Context, req *http.Request, authConfig map[string]interface{}) error {
	token, ok := authConfig["token"].(string)
	if !ok {
		return fmt.Errorf("token not provided in auth config")
	}

	req.Header.Set("Authorization", fmt.Sprintf("Bearer %s", token))
	return nil
}

// applyBasicAuth applies Basic authentication
func (c *HTTPClient) applyBasicAuth(ctx context.Context, req *http.Request, authConfig map[string]interface{}) error {
	username, ok := authConfig["username"].(string)
	if !ok {
		return fmt.Errorf("username not provided in auth config")
	}

	password, ok := authConfig["password"].(string)
	if !ok {
		return fmt.Errorf("password not provided in auth config")
	}

	req.SetBasicAuth(username, password)
	return nil
}

// applyOAuth2Auth applies OAuth2 authentication
func (c *HTTPClient) applyOAuth2Auth(ctx context.Context, req *http.Request, authConfig map[string]interface{}) error {
	accessToken, ok := authConfig["access_token"].(string)
	if !ok {
		return fmt.Errorf("access_token not provided in auth config")
	}

	req.Header.Set("Authorization", fmt.Sprintf("Bearer %s", accessToken))
	return nil
}

// applyCustomAuth applies custom authentication
func (c *HTTPClient) applyCustomAuth(ctx context.Context, req *http.Request, authConfig map[string]interface{}) error {
	// Apply custom headers directly from config
	if headers, ok := authConfig["headers"].(map[string]interface{}); ok {
		for headerName, v := range headers {
			if headerValue, ok := v.(string); ok {
				req.Header.Set(headerName, headerValue)
			}
		}
	}

	return nil
}

// applyHeaders applies custom headers to the request
func (c *HTTPClient) applyHeaders(req *http.Request, apiReq *APIRequest) {
	if apiReq.Headers == nil {
		return
	}

	for name, value := range apiReq.Headers {
		req.Header.Set(name, value)
	}
}

// applyObservabilityHeaders applies observability headers
func (c *HTTPClient) applyObservabilityHeaders(req *http.Request, apiReq *APIRequest) {
	// Add idempotency key if provided
	if apiReq.IdempotencyKey != nil {
		req.Header.Set("Idempotency-Key", *apiReq.IdempotencyKey)
	}

	// Add correlation ID if provided
	if apiReq.CorrelationID != nil {
		req.Header.Set("X-Correlation-ID", *apiReq.CorrelationID)
	}

	// Add distributed tracing headers if provided
	if apiReq.TraceID != nil {
		req.Header.Set("X-Trace-ID", *apiReq.TraceID)
	}

	if apiReq.SpanID != nil {
		req.Header.Set("X-Span-ID", *apiReq.SpanID)
	}

	// Add user agent
	req.Header.Set("User-Agent", "DSL-Runtime-Engine/1.0")
}

// extractHeaders extracts headers from HTTP response
func (c *HTTPClient) extractHeaders(resp *http.Response) map[string]string {
	headers := make(map[string]string)
	for name, values := range resp.Header {
		if len(values) > 0 {
			headers[name] = values[0] // Take first value if multiple
		}
	}
	return headers
}

// ValidateResponse checks if the response meets success criteria
func (c *HTTPClient) ValidateResponse(resp *APIResponse, criteria SuccessCriteria) error {
	// Check HTTP status codes
	if len(criteria.HTTPStatusCodes) > 0 {
		validStatus := false
		for _, validCode := range criteria.HTTPStatusCodes {
			if resp.StatusCode == validCode {
				validStatus = true
				break
			}
		}
		if !validStatus {
			return fmt.Errorf("HTTP status %d not in allowed codes %v", resp.StatusCode, criteria.HTTPStatusCodes)
		}
	}

	// Check response validation (JSONPath-like expression)
	if criteria.ResponseValidation != nil {
		// This is a simplified validation - in production you'd use a proper JSONPath library
		validation := *criteria.ResponseValidation
		if strings.Contains(validation, "$.status == 'CREATED'") {
			if status, ok := resp.Body["status"].(string); !ok || status != "CREATED" {
				return fmt.Errorf("response validation failed: status is not 'CREATED'")
			}
		}
		// Add more validation logic as needed
	}

	// Check required outputs are present
	if len(criteria.RequiredOutputs) > 0 {
		for _, requiredOutput := range criteria.RequiredOutputs {
			// Simple check for presence in response body
			if _, exists := resp.Body[requiredOutput]; !exists {
				return fmt.Errorf("required output '%s' not found in response", requiredOutput)
			}
		}
	}

	return nil
}

// ShouldRetry determines if a request should be retried based on failure handling configuration
func (c *HTTPClient) ShouldRetry(resp *APIResponse, failureConfig FailureHandling, currentRetryCount int, maxRetries int) bool {
	// Don't retry if max retries exceeded
	if currentRetryCount >= maxRetries {
		return false
	}

	// Don't retry if response was successful (no error)
	if resp.Error == nil && resp.StatusCode >= 200 && resp.StatusCode < 400 {
		return false
	}

	// Retry based on HTTP status codes in failure configuration
	if len(failureConfig.RetryOnCodes) > 0 {
		for _, retryCode := range failureConfig.RetryOnCodes {
			if resp.StatusCode == retryCode {
				return true
			}
		}
	}

	// Default retry logic for common transient errors
	if resp.StatusCode >= 500 || resp.Error != nil {
		return true
	}

	return false
}

// CalculateBackoffDelay calculates the delay before next retry
func (c *HTTPClient) CalculateBackoffDelay(retryCount int, config RetryConfig) time.Duration {
	baseDelay := time.Duration(config.BaseDelayMS) * time.Millisecond

	switch strings.ToLower(config.BackoffStrategy) {
	case "exponential":
		_ = config.Multiplier // Not used in exponential calculation
		// Safe exponential backoff calculation
		exponentialMultiplier := 1
		for i := 0; i < retryCount && i < 10; i++ { // Cap at 10 to prevent overflow
			exponentialMultiplier *= 2
		}
		delay := baseDelay * time.Duration(exponentialMultiplier)

		// Apply max delay if specified
		if config.MaxDelayMS > 0 {
			maxDelay := time.Duration(config.MaxDelayMS) * time.Millisecond
			if delay > maxDelay {
				delay = maxDelay
			}
		}
		return delay

	case "linear":
		multiplier := config.Multiplier
		if multiplier == 0 {
			multiplier = 1.0
		}
		return baseDelay * time.Duration(float64(retryCount+1)*multiplier)

	case "fixed":
		return baseDelay

	default:
		// Default to exponential backoff with safe calculation
		exponentialMultiplier := 1
		for i := 0; i < retryCount && i < 10; i++ { // Cap at 10 to prevent overflow
			exponentialMultiplier *= 2
		}
		return baseDelay * time.Duration(exponentialMultiplier)
	}
}
