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
	Suite     string
	Case      string
	Passed    bool
	Duration  time.Duration
	Error     error
	Response  *rustclient.ExecuteResponse
	Skipped   bool
	SkipReason string
}

// SuiteResult aggregates results for a suite.
type SuiteResult struct {
	Name     string
	Passed   int
	Failed   int
	Skipped  int
	Duration time.Duration
	Results  []Result
}

// Runner executes test suites.
type Runner struct {
	client    *rustclient.Client
	sessionID uuid.UUID
	verbose   bool
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

	// Create session
	sess, err := r.client.CreateSession(ctx, nil)
	if err != nil {
		return nil, fmt.Errorf("creating session: %w", err)
	}
	r.sessionID = sess.SessionID

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
			// Log but don't fail
			fmt.Printf("teardown warning: %v\n", err)
		}
	}

	result.Duration = time.Since(start)
	return result, nil
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
	resp, err := r.client.ExecuteDSL(ctx, r.sessionID, tc.DSL)
	result.Duration = time.Since(start)
	result.Response = resp

	if err != nil {
		result.Error = err
		result.Passed = false
		return result
	}

	// Check expectations
	if resp.Success != tc.Expect.Success {
		result.Error = fmt.Errorf("expected success=%v, got %v", tc.Expect.Success, resp.Success)
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
			result.Error = fmt.Errorf("expected error containing %q", *tc.Expect.ErrorContains)
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
			result.Error = fmt.Errorf("expected %d entities, got %d", *tc.Expect.EntityCount, count)
			result.Passed = false
			return result
		}
	}

	if tc.Expect.Validate != nil {
		if err := tc.Expect.Validate(resp); err != nil {
			result.Error = err
			result.Passed = false
			return result
		}
	}

	result.Passed = true
	return result
}

func contains(s, substr string) bool {
	return len(s) >= len(substr) && (s == substr || len(substr) == 0 ||
		(len(s) > 0 && len(substr) > 0 && searchString(s, substr)))
}

func searchString(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}
