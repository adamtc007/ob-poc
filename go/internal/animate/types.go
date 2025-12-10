// Package animate provides types and logic for running animated DSL demo scenarios.
package animate

import (
	"time"

	"github.com/google/uuid"
)

// Scenario defines an animated demo sequence.
type Scenario struct {
	Name        string `yaml:"name"`
	Description string `yaml:"description,omitempty"`

	// Timing defaults (can be overridden per step)
	TypingSpeedMs int `yaml:"typing_speed_ms,omitempty"` // Simulated typing speed (0 = instant)
	PauseAfterMs  int `yaml:"pause_after_ms,omitempty"`  // Default pause after each step

	// Cleanup settings
	CleanupAfter bool `yaml:"cleanup_after,omitempty"` // Delete created CBUs after run

	// Steps in the scenario
	Steps []Step `yaml:"steps"`
}

// Step is a single prompt in the scenario.
type Step struct {
	// The natural language prompt to send
	Prompt string `yaml:"prompt"`

	// Optional: expected verbs in the generated DSL (for validation)
	ExpectVerbs []string `yaml:"expect_verbs,omitempty"`

	// Optional: expected binding names to be created
	ExpectBindings []string `yaml:"expect_bindings,omitempty"`

	// Timing overrides
	PauseAfterMs  *int `yaml:"pause_after_ms,omitempty"`  // Override default pause
	TypingSpeedMs *int `yaml:"typing_speed_ms,omitempty"` // Override typing speed

	// Execution control
	AutoExecute bool `yaml:"auto_execute,omitempty"` // Execute DSL after this step
	WaitForKey  bool `yaml:"wait_for_key,omitempty"` // Wait for keypress before continuing

	// Validation
	ExpectSuccess *bool `yaml:"expect_success,omitempty"` // Expected execution result
}

// RunConfig controls how the scenario is executed.
type RunConfig struct {
	AgentURL string  // Rust agent API URL (default: http://127.0.0.1:3000)
	Speed    float64 // Speed multiplier (1.0 = normal, 2.0 = 2x faster)

	// Output options
	Verbose     bool // Show full API responses
	ShowDiff    bool // Highlight new DSL statements
	NoColor     bool // Disable color output
	Interactive bool // Pause for keypress between steps

	// Validation
	StopOnError bool // Stop if validation fails
}

// StepResult captures the outcome of a single step.
type StepResult struct {
	StepIndex int
	Prompt    string
	StartTime time.Time
	EndTime   time.Time

	// Response from agent
	AgentMessage string
	GeneratedDSL []string
	Bindings     []string

	// Validation
	ExpectedVerbs []string
	FoundVerbs    []string
	VerbsMatched  bool

	// Execution (if auto_execute)
	Executed       bool
	ExecuteSuccess bool
	ExecuteErrors  []string
	CreatedIDs     map[string]uuid.UUID

	// Errors
	Error error
}

// ScenarioResult summarizes the full run.
type ScenarioResult struct {
	ScenarioName string
	StartTime    time.Time
	EndTime      time.Time
	Steps        []StepResult

	// Aggregates
	TotalSteps    int
	PassedSteps   int
	FailedSteps   int
	SkippedSteps  int
	TotalDuration time.Duration

	// Created entities (for cleanup)
	CreatedCBUs []uuid.UUID

	// Overall success
	Success bool
	Error   error
}

// VerbMatch checks if expected verbs are present in DSL statements.
func VerbMatch(expected []string, dslStatements []string) (found []string, matched bool) {
	foundSet := make(map[string]bool)

	for _, stmt := range dslStatements {
		for _, verb := range expected {
			// Simple substring match for now
			// e.g., "cbu.ensure" in "(cbu.ensure :name ...)"
			if containsVerb(stmt, verb) {
				foundSet[verb] = true
			}
		}
	}

	for verb := range foundSet {
		found = append(found, verb)
	}

	matched = len(found) == len(expected)
	return found, matched
}

// containsVerb checks if a DSL statement contains a verb.
func containsVerb(stmt, verb string) bool {
	// Look for "(verb " pattern
	pattern := "(" + verb + " "
	return len(stmt) > 0 && (contains(stmt, pattern) || contains(stmt, "("+verb+")"))
}

func contains(s, substr string) bool {
	return len(s) >= len(substr) && searchSubstring(s, substr)
}

func searchSubstring(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}
