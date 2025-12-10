package animate

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"strings"
	"time"

	"github.com/adamtc007/ob-poc/go/internal/rustclient"
	"github.com/google/uuid"
)

// Runner executes animation scenarios.
type Runner struct {
	config     RunConfig
	httpClient *http.Client
	output     io.Writer
}

// NewRunner creates a new scenario runner.
func NewRunner(config RunConfig) *Runner {
	if config.AgentURL == "" {
		config.AgentURL = "http://127.0.0.1:3000"
	}
	if config.Speed == 0 {
		config.Speed = 1.0
	}

	return &Runner{
		config: config,
		httpClient: &http.Client{
			Timeout: 60 * time.Second,
		},
		output: os.Stdout,
	}
}

// SetOutput sets the output writer (for testing).
func (r *Runner) SetOutput(w io.Writer) {
	r.output = w
}

// Run executes a scenario and returns results.
func (r *Runner) Run(ctx context.Context, scenario Scenario) (*ScenarioResult, error) {
	result := &ScenarioResult{
		ScenarioName: scenario.Name,
		StartTime:    time.Now(),
		TotalSteps:   len(scenario.Steps),
	}

	// Print header
	r.printHeader(scenario)

	// Create session
	sessionID, err := r.createSession(ctx)
	if err != nil {
		result.Error = fmt.Errorf("failed to create session: %w", err)
		result.EndTime = time.Now()
		return result, result.Error
	}

	r.printf("\n%s Session: %s\n\n", r.icon("session"), sessionID)

	// Run each step
	var accumulatedDSL []string
	for i, step := range scenario.Steps {
		stepResult := r.runStep(ctx, sessionID, i, step, scenario, accumulatedDSL)
		result.Steps = append(result.Steps, stepResult)

		// Accumulate DSL for diff display
		accumulatedDSL = append(accumulatedDSL, stepResult.GeneratedDSL...)

		// Track created CBUs
		for name, id := range stepResult.CreatedIDs {
			if strings.Contains(strings.ToLower(name), "cbu") || strings.HasPrefix(name, "fund") {
				result.CreatedCBUs = append(result.CreatedCBUs, id)
			}
		}

		if stepResult.Error != nil {
			result.FailedSteps++
			if r.config.StopOnError {
				r.printf("\n%s Stopping on error\n", r.icon("error"))
				break
			}
		} else if stepResult.VerbsMatched || len(step.ExpectVerbs) == 0 {
			result.PassedSteps++
		} else {
			result.FailedSteps++
		}

		// Pause between steps
		r.pauseAfterStep(step, scenario)
	}

	result.EndTime = time.Now()
	result.TotalDuration = result.EndTime.Sub(result.StartTime)
	result.Success = result.FailedSteps == 0

	// Cleanup if requested
	if scenario.CleanupAfter && len(result.CreatedCBUs) > 0 {
		r.cleanup(ctx, result.CreatedCBUs)
	}

	// Print summary
	r.printSummary(result)

	return result, nil
}

func (r *Runner) runStep(ctx context.Context, sessionID uuid.UUID, index int, step Step, scenario Scenario, previousDSL []string) StepResult {
	result := StepResult{
		StepIndex:     index,
		Prompt:        step.Prompt,
		StartTime:     time.Now(),
		ExpectedVerbs: step.ExpectVerbs,
	}

	// Print step header
	r.printf("%s Step %d: ", r.icon("step"), index+1)

	// Simulate typing
	r.typeText(step.Prompt, step, scenario)
	r.printf("\n")

	// Send chat message
	chatResp, err := r.sendChat(ctx, sessionID, step.Prompt)
	if err != nil {
		result.Error = err
		result.EndTime = time.Now()
		r.printf("  %s Error: %v\n", r.icon("error"), err)
		return result
	}

	result.AgentMessage = chatResp.Message
	if chatResp.AssembledDsl != nil {
		result.GeneratedDSL = chatResp.AssembledDsl.Statements
	}

	// Extract binding names from intents
	for _, intent := range chatResp.Intents {
		if ref, ok := intent.Params["as"].(string); ok {
			result.Bindings = append(result.Bindings, ref)
		}
	}

	// Print agent response
	r.printf("  %s Agent: %s\n", r.icon("agent"), truncate(chatResp.Message, 100))

	// Print generated DSL
	if len(result.GeneratedDSL) > 0 {
		r.printf("  %s DSL:\n", r.icon("dsl"))
		for _, stmt := range result.GeneratedDSL {
			// Highlight new statements
			isNew := !containsStatement(previousDSL, stmt)
			if isNew && r.config.ShowDiff {
				r.printf("    %s%s%s\n", r.color("green"), stmt, r.color("reset"))
			} else {
				r.printf("    %s\n", stmt)
			}
		}
	}

	// Validate expected verbs
	if len(step.ExpectVerbs) > 0 {
		result.FoundVerbs, result.VerbsMatched = VerbMatch(step.ExpectVerbs, result.GeneratedDSL)
		if result.VerbsMatched {
			r.printf("  %s Verbs matched: %v\n", r.icon("check"), result.FoundVerbs)
		} else {
			r.printf("  %s Expected verbs: %v, found: %v\n", r.icon("warn"), step.ExpectVerbs, result.FoundVerbs)
		}
	}

	// Auto-execute if requested
	if step.AutoExecute && chatResp.CanExecute {
		r.printf("  %s Executing...\n", r.icon("exec"))
		execResp, err := r.execute(ctx, sessionID)
		result.Executed = true
		if err != nil {
			result.ExecuteSuccess = false
			result.ExecuteErrors = []string{err.Error()}
			r.printf("  %s Execution failed: %v\n", r.icon("error"), err)
		} else {
			result.ExecuteSuccess = execResp.Success
			result.ExecuteErrors = execResp.Errors
			result.CreatedIDs = execResp.Bindings
			if execResp.Success {
				r.printf("  %s Executed successfully\n", r.icon("check"))
				for name, id := range execResp.Bindings {
					r.printf("    @%s → %s\n", name, id)
				}
			} else {
				r.printf("  %s Execution errors: %v\n", r.icon("error"), execResp.Errors)
			}
		}
	}

	// Wait for keypress if interactive
	if step.WaitForKey || (r.config.Interactive && index < len(scenario.Steps)-1) {
		r.printf("\n  Press Enter to continue...")
		var b [1]byte
		_, _ = os.Stdin.Read(b[:])
	}

	result.EndTime = time.Now()
	return result
}

// API calls

func (r *Runner) createSession(ctx context.Context) (uuid.UUID, error) {
	resp, err := r.post(ctx, "/api/session", map[string]any{})
	if err != nil {
		return uuid.Nil, err
	}

	var result rustclient.CreateSessionResponse
	if err := json.Unmarshal(resp, &result); err != nil {
		return uuid.Nil, fmt.Errorf("parsing session response: %w", err)
	}

	return result.SessionID, nil
}

func (r *Runner) sendChat(ctx context.Context, sessionID uuid.UUID, message string) (*rustclient.ChatResponse, error) {
	path := fmt.Sprintf("/api/session/%s/chat", sessionID)
	resp, err := r.post(ctx, path, map[string]string{"message": message})
	if err != nil {
		return nil, err
	}

	var result rustclient.ChatResponse
	if err := json.Unmarshal(resp, &result); err != nil {
		return nil, fmt.Errorf("parsing chat response: %w", err)
	}

	return &result, nil
}

func (r *Runner) execute(ctx context.Context, sessionID uuid.UUID) (*rustclient.ExecuteResponse, error) {
	path := fmt.Sprintf("/api/session/%s/execute", sessionID)
	resp, err := r.post(ctx, path, map[string]any{"dry_run": false})
	if err != nil {
		return nil, err
	}

	var result rustclient.ExecuteResponse
	if err := json.Unmarshal(resp, &result); err != nil {
		return nil, fmt.Errorf("parsing execute response: %w", err)
	}

	return &result, nil
}

func (r *Runner) cleanup(ctx context.Context, cbuIDs []uuid.UUID) {
	r.printf("\n%s Cleaning up %d CBUs...\n", r.icon("cleanup"), len(cbuIDs))
	client := rustclient.NewClient(r.config.AgentURL)
	for _, id := range cbuIDs {
		if _, err := client.CleanupCBU(ctx, id); err != nil {
			r.printf("  %s Failed to cleanup %s: %v\n", r.icon("warn"), id, err)
		} else {
			r.printf("  %s Deleted %s\n", r.icon("check"), id)
		}
	}
}

func (r *Runner) post(ctx context.Context, path string, body any) ([]byte, error) {
	data, err := json.Marshal(body)
	if err != nil {
		return nil, err
	}

	req, err := http.NewRequestWithContext(ctx, "POST", r.config.AgentURL+path, bytes.NewReader(data))
	if err != nil {
		return nil, err
	}
	req.Header.Set("Content-Type", "application/json")

	resp, err := r.httpClient.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, err
	}

	if resp.StatusCode >= 400 {
		return nil, fmt.Errorf("API error %d: %s", resp.StatusCode, string(respBody))
	}

	return respBody, nil
}

// Output helpers

func (r *Runner) printf(format string, args ...any) {
	fmt.Fprintf(r.output, format, args...)
}

func (r *Runner) printHeader(scenario Scenario) {
	r.printf("\n%s\n", strings.Repeat("═", 60))
	r.printf("%s %s\n", r.icon("scenario"), scenario.Name)
	if scenario.Description != "" {
		r.printf("   %s\n", scenario.Description)
	}
	r.printf("%s\n", strings.Repeat("═", 60))
}

func (r *Runner) printSummary(result *ScenarioResult) {
	r.printf("\n%s\n", strings.Repeat("─", 60))
	r.printf("%s Summary\n", r.icon("summary"))
	r.printf("   Duration: %s\n", result.TotalDuration.Round(time.Millisecond))
	r.printf("   Steps:    %d total, %d passed, %d failed\n",
		result.TotalSteps, result.PassedSteps, result.FailedSteps)

	if result.Success {
		r.printf("   Result:   %s PASSED\n", r.icon("check"))
	} else {
		r.printf("   Result:   %s FAILED\n", r.icon("error"))
	}
	r.printf("%s\n\n", strings.Repeat("─", 60))
}

func (r *Runner) typeText(text string, step Step, scenario Scenario) {
	speed := scenario.TypingSpeedMs
	if step.TypingSpeedMs != nil {
		speed = *step.TypingSpeedMs
	}

	if speed == 0 || r.config.Speed > 10 {
		// Instant
		r.printf("%s", text)
		return
	}

	delay := time.Duration(float64(speed)/r.config.Speed) * time.Millisecond
	for _, ch := range text {
		r.printf("%c", ch)
		time.Sleep(delay)
	}
}

func (r *Runner) pauseAfterStep(step Step, scenario Scenario) {
	pause := scenario.PauseAfterMs
	if step.PauseAfterMs != nil {
		pause = *step.PauseAfterMs
	}

	if pause > 0 {
		adjusted := time.Duration(float64(pause)/r.config.Speed) * time.Millisecond
		time.Sleep(adjusted)
	}
}

func (r *Runner) icon(name string) string {
	if r.config.NoColor {
		return iconPlain[name]
	}
	return iconColor[name]
}

func (r *Runner) color(name string) string {
	if r.config.NoColor {
		return ""
	}
	return colors[name]
}

// Icons and colors

var iconColor = map[string]string{
	"scenario": "\033[1;36m▶\033[0m",
	"session":  "\033[1;34m⚡\033[0m",
	"step":     "\033[1;33m→\033[0m",
	"agent":    "\033[1;35m🤖\033[0m",
	"dsl":      "\033[1;32m📝\033[0m",
	"check":    "\033[1;32m✓\033[0m",
	"error":    "\033[1;31m✗\033[0m",
	"warn":     "\033[1;33m⚠\033[0m",
	"exec":     "\033[1;34m⚙\033[0m",
	"cleanup":  "\033[1;33m🧹\033[0m",
	"summary":  "\033[1;36m📊\033[0m",
}

var iconPlain = map[string]string{
	"scenario": ">",
	"session":  "*",
	"step":     "->",
	"agent":    "[A]",
	"dsl":      "[D]",
	"check":    "[OK]",
	"error":    "[ERR]",
	"warn":     "[WARN]",
	"exec":     "[EXEC]",
	"cleanup":  "[CLEAN]",
	"summary":  "[SUM]",
}

var colors = map[string]string{
	"green": "\033[32m",
	"red":   "\033[31m",
	"reset": "\033[0m",
}

// Helpers

func truncate(s string, n int) string {
	if len(s) <= n {
		return s
	}
	return s[:n-3] + "..."
}

func containsStatement(stmts []string, stmt string) bool {
	for _, s := range stmts {
		if s == stmt {
			return true
		}
	}
	return false
}
