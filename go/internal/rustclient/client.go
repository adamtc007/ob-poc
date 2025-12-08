// Package rustclient provides a minimal HTTP client for the Rust DSL API.
// Most API calls are handled via direct HTTP proxying in main.go.
// This client is used for: index page health/verbs, validation, and test harness.
package rustclient

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"time"

	"github.com/google/uuid"
)

// Client is an HTTP client for the Rust DSL API.
type Client struct {
	baseURL    string
	httpClient *http.Client
}

// NewClient creates a new API client.
func NewClient(baseURL string) *Client {
	return &Client{
		baseURL: baseURL,
		httpClient: &http.Client{
			Timeout: 30 * time.Second,
		},
	}
}

// Health checks API health.
func (c *Client) Health(ctx context.Context) (*HealthResponse, error) {
	var resp HealthResponse
	if err := c.get(ctx, "/health", &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

// ListVerbs returns all available verbs.
func (c *Client) ListVerbs(ctx context.Context) (*VerbsResponse, error) {
	var resp VerbsResponse
	if err := c.get(ctx, "/verbs", &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

// ValidateDSL validates DSL syntax.
func (c *Client) ValidateDSL(ctx context.Context, dsl string) (*ValidationResult, error) {
	req := ValidateDSLRequest{DSL: dsl}
	var resp ValidationResult
	if err := c.post(ctx, "/validate", req, &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

// ExecuteDSL executes DSL and returns results with bindings.
// Used by the test harness.
func (c *Client) ExecuteDSL(ctx context.Context, dsl string) (*ExecuteResponse, error) {
	req := ExecuteDSLRequest{DSL: dsl}
	var resp ExecuteResponse
	if err := c.post(ctx, "/execute", req, &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

// CleanupCBU deletes a CBU and all related data.
// Used by the test harness for cleanup.
func (c *Client) CleanupCBU(ctx context.Context, cbuID uuid.UUID) (*CleanupResponse, error) {
	var resp CleanupResponse
	path := fmt.Sprintf("/cleanup/cbu/%s", cbuID)
	if err := c.delete(ctx, path, &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

func (c *Client) get(ctx context.Context, path string, result any) error {
	req, err := http.NewRequestWithContext(ctx, "GET", c.baseURL+path, nil)
	if err != nil {
		return fmt.Errorf("creating request: %w", err)
	}
	return c.do(req, result)
}

func (c *Client) post(ctx context.Context, path string, body, result any) error {
	data, err := json.Marshal(body)
	if err != nil {
		return fmt.Errorf("marshaling request: %w", err)
	}
	req, err := http.NewRequestWithContext(ctx, "POST", c.baseURL+path, bytes.NewReader(data))
	if err != nil {
		return fmt.Errorf("creating request: %w", err)
	}
	req.Header.Set("Content-Type", "application/json")
	return c.do(req, result)
}

func (c *Client) delete(ctx context.Context, path string, result any) error {
	req, err := http.NewRequestWithContext(ctx, "DELETE", c.baseURL+path, nil)
	if err != nil {
		return fmt.Errorf("creating request: %w", err)
	}
	return c.do(req, result)
}

func (c *Client) do(req *http.Request, result any) error {
	resp, err := c.httpClient.Do(req)
	if err != nil {
		return fmt.Errorf("executing request: %w", err)
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return fmt.Errorf("reading response: %w", err)
	}

	if resp.StatusCode >= 400 {
		return fmt.Errorf("API error %d: %s", resp.StatusCode, string(body))
	}

	if result != nil && len(body) > 0 {
		if err := json.Unmarshal(body, result); err != nil {
			return fmt.Errorf("unmarshaling response: %w", err)
		}
	}
	return nil
}
