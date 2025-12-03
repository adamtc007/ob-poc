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
	if err := c.get(ctx, "/api/agent/health", &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

// ListDomains returns available DSL domains.
func (c *Client) ListDomains(ctx context.Context) (*DomainsResponse, error) {
	var resp DomainsResponse
	if err := c.get(ctx, "/api/agent/domains", &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

// GetVocabulary returns verb vocabulary, optionally filtered by domain.
func (c *Client) GetVocabulary(ctx context.Context, domain *string) (*VocabResponse, error) {
	path := "/api/agent/vocabulary"
	if domain != nil {
		path += "?domain=" + *domain
	}
	var resp VocabResponse
	if err := c.get(ctx, path, &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

// ValidateDSL validates DSL syntax.
func (c *Client) ValidateDSL(ctx context.Context, dsl string) (*ValidationResult, error) {
	req := ValidateDSLRequest{DSL: dsl}
	var resp ValidationResult
	if err := c.post(ctx, "/api/agent/validate", req, &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

// GenerateDSL generates DSL from natural language.
func (c *Client) GenerateDSL(ctx context.Context, instruction string, domain *string) (*GenerateDSLResponse, error) {
	req := GenerateDSLRequest{Instruction: instruction, Domain: domain}
	var resp GenerateDSLResponse
	if err := c.post(ctx, "/api/agent/generate", req, &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

// CreateSession creates a new session.
func (c *Client) CreateSession(ctx context.Context, domainHint *string) (*CreateSessionResponse, error) {
	req := CreateSessionRequest{DomainHint: domainHint}
	var resp CreateSessionResponse
	if err := c.post(ctx, "/api/session", req, &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

// ExecuteDSL executes DSL in a session.
func (c *Client) ExecuteDSL(ctx context.Context, sessionID uuid.UUID, dsl string) (*ExecuteResponse, error) {
	req := ExecuteDSLRequest{DSL: dsl}
	var resp ExecuteResponse
	path := fmt.Sprintf("/api/session/%s/execute", sessionID)
	if err := c.post(ctx, path, req, &resp); err != nil {
		return nil, err
	}
	return &resp, nil
}

// ListCBUs returns all CBUs.
func (c *Client) ListCBUs(ctx context.Context) ([]CbuSummary, error) {
	var resp []CbuSummary
	if err := c.get(ctx, "/api/cbus", &resp); err != nil {
		return nil, err
	}
	return resp, nil
}

// GetCBUGraph returns graph data for a CBU.
func (c *Client) GetCBUGraph(ctx context.Context, cbuID uuid.UUID) (*CbuGraph, error) {
	var resp CbuGraph
	path := fmt.Sprintf("/api/cbus/%s/graph", cbuID)
	if err := c.get(ctx, path, &resp); err != nil {
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
