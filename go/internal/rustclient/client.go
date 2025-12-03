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

// WithTimeout sets a custom timeout.
func (c *Client) WithTimeout(d time.Duration) *Client {
	c.httpClient.Timeout = d
	return c
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
func (c *Client) ExecuteDSL(ctx context.Context, dsl string) (*ExecuteResponse, error) {
	req := ExecuteDSLRequest{DSL: dsl}
	var resp ExecuteResponse
	if err := c.post(ctx, "/execute", req, &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

// ListCBUs returns all CBUs.
func (c *Client) ListCBUs(ctx context.Context) ([]CbuSummary, error) {
	var resp []CbuSummary
	if err := c.get(ctx, "/query/cbus", &resp); err != nil {
		return nil, err
	}
	return resp, nil
}

// GetCBU returns a CBU with full details (entities, roles).
func (c *Client) GetCBU(ctx context.Context, cbuID uuid.UUID) (*CbuDetail, error) {
	var resp CbuDetail
	path := fmt.Sprintf("/query/cbus/%s", cbuID)
	if err := c.get(ctx, path, &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

// GetKYCCase returns a KYC case with workstreams and flags.
func (c *Client) GetKYCCase(ctx context.Context, caseID uuid.UUID) (*KycCaseDetail, error) {
	var resp KycCaseDetail
	path := fmt.Sprintf("/query/kyc/cases/%s", caseID)
	if err := c.get(ctx, path, &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

// CleanupCBU deletes a CBU and all related data.
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
