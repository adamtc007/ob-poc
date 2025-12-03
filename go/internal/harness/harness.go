// Package harness provides a test harness framework for DSL testing.
package harness

import (
	"context"
	"fmt"
	"time"

	"github.com/adamtc007/ob-poc/go/internal/rustclient"
	"github.com/google/uuid"
)

// Suite represents a test suite with multiple test cases.
type Suite struct {
	Name        string
	Description string
	Cases       []Case
	Setup       func(ctx context.Context, c *rustclient.Client) error
	Teardown    func(ctx context.Context, c *rustclient.Client) error
}

// Case represents a single test case.
type Case struct {
	Name        string
	Description string
	DSL         string
	Expect      Expectation
	Skip        bool
	SkipReason  string
}

// Expectation defines what we expect from execution.
type Expectation struct {
	Success       bool
	ErrorContains *string
	EntityCount   *int
	Validate      func(*rustclient.ExecuteResponse) error
}

// Result captures test execution results.
type Result struct {
	Suite      string                       `json:"suite,omitempty"`
	Case       string                       `json:"case"`
	Passed     bool                         `json:"passed"`
	Duration   time.Duration                `json:"duration"`
	Error      string                       `json:"error,omitempty"`
	Response   *rustclient.ExecuteResponse  `json:"response,omitempty"`
	Skipped    bool                         `json:"skipped,omitempty"`
	SkipReason string                       `json:"skip_reason,omitempty"`
}

// SuiteResult aggregates results for a suite.
type SuiteResult struct {
	Name       string        `json:"name"`
	Passed     int           `json:"passed"`
	Failed     int           `json:"failed"`
	Skipped    int           `json:"skipped"`
	Duration   time.Duration                `json:"duration"`
	Results    []Result      `json:"results"`
	CreatedIDs []uuid.UUID // Track created CBU IDs for cleanup
}

// Runner executes test suites.
type Runner struct {
	client     *rustclient.Client
	verbose    bool
	createdIDs []uuid.UUID
}

// NewRunner creates a new test runner.
func NewRunner(baseURL string) *Runner {
	return &Runner{
		client: rustclient.NewClient(baseURL),
	}
}

// WithVerbose enables verbose output.
func (r *Runner) WithVerbose(v bool) *Runner {
	r.verbose = v
	return r
}

// Run executes a suite and returns results.
func (r *Runner) Run(ctx context.Context, suite Suite) (*SuiteResult, error) {
	start := time.Now()
	result := &SuiteResult{Name: suite.Name}
	r.createdIDs = nil

	// Run setup if defined
	if suite.Setup != nil {
		if err := suite.Setup(ctx, r.client); err != nil {
			return nil, fmt.Errorf("setup failed: %w", err)
		}
	}

	// Run cases
	for _, tc := range suite.Cases {
		tcResult := r.runCase(ctx, tc)
		result.Results = append(result.Results, tcResult)
		if tcResult.Skipped {
			result.Skipped++
		} else if tcResult.Passed {
			result.Passed++
		} else {
			result.Failed++
		}
	}

	// Run teardown if defined
	if suite.Teardown != nil {
		if err := suite.Teardown(ctx, r.client); err != nil {
			fmt.Printf("teardown warning: %v\n", err)
		}
	}

	result.Duration = time.Since(start)
	result.CreatedIDs = r.createdIDs
	return result, nil
}

// Cleanup removes all CBUs created during tests.
func (r *Runner) Cleanup(ctx context.Context, ids []uuid.UUID) error {
	for _, id := range ids {
		if _, err := r.client.CleanupCBU(ctx, id); err != nil {
			return fmt.Errorf("cleanup CBU %s: %w", id, err)
		}
	}
	return nil
}

func (r *Runner) runCase(ctx context.Context, tc Case) Result {
	start := time.Now()
	result := Result{Case: tc.Name}

	if tc.Skip {
		result.Skipped = true
		result.SkipReason = tc.SkipReason
		return result
	}

	// Execute DSL
	resp, err := r.client.ExecuteDSL(ctx, tc.DSL)
	result.Duration = time.Since(start)
	result.Response = resp

	if err != nil {
		result.Error = err.Error()
		result.Passed = false
		return result
	}

	// Track created IDs for cleanup
	for _, id := range resp.Bindings {
		r.createdIDs = append(r.createdIDs, id)
	}

	// Check expectations
	if resp.Success != tc.Expect.Success {
		result.Error = fmt.Sprintf("expected success=%v, got %v", tc.Expect.Success, resp.Success)
		result.Passed = false
		return result
	}

	if tc.Expect.ErrorContains != nil && len(resp.Errors) > 0 {
		found := false
		for _, e := range resp.Errors {
			if contains(e, *tc.Expect.ErrorContains) {
				found = true
				break
			}
		}
		if !found {
			result.Error = fmt.Sprintf("expected error containing %q", *tc.Expect.ErrorContains)
			result.Passed = false
			return result
		}
	}

	if tc.Expect.EntityCount != nil {
		count := 0
		for _, r := range resp.Results {
			if r.EntityID != nil {
				count++
			}
		}
		if count != *tc.Expect.EntityCount {
			result.Error = fmt.Sprintf("expected %d entities, got %d", *tc.Expect.EntityCount, count)
			result.Passed = false
			return result
		}
	}

	if tc.Expect.Validate != nil {
		if err := tc.Expect.Validate(resp); err != nil {
			result.Error = err.Error()
			result.Passed = false
			return result
		}
	}

	result.Passed = true
	return result
}

func contains(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}
