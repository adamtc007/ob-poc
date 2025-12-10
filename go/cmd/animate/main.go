// CLI for running animated DSL demo scenarios.
//
// Usage:
//
//	animate -f scenarios/fund_onboarding.yaml
//	animate -f scenarios/fund_onboarding.yaml --speed 2.0
//	animate --list scenarios/
//	animate -f scenario.yaml --execute --cleanup
package main

import (
	"context"
	"flag"
	"fmt"
	"os"
	"os/signal"
	"syscall"

	"github.com/adamtc007/ob-poc/go/internal/animate"
)

func main() {
	// Flags
	scenarioFile := flag.String("f", "", "Scenario YAML file to run")
	scenarioDir := flag.String("list", "", "List scenarios in directory")
	agentURL := flag.String("agent-url", "http://127.0.0.1:3000", "Rust agent API URL")
	speed := flag.Float64("speed", 1.0, "Speed multiplier (1.0 = normal, 2.0 = 2x faster)")
	verbose := flag.Bool("v", false, "Verbose output")
	showDiff := flag.Bool("diff", true, "Highlight new DSL statements")
	noColor := flag.Bool("no-color", false, "Disable color output")
	interactive := flag.Bool("i", false, "Interactive mode (pause between steps)")
	stopOnError := flag.Bool("stop-on-error", false, "Stop if a step fails")

	flag.Usage = func() {
		fmt.Fprintf(os.Stderr, "Usage: animate [options]\n\n")
		fmt.Fprintf(os.Stderr, "Run animated DSL demo scenarios against the agent API.\n\n")
		fmt.Fprintf(os.Stderr, "Options:\n")
		flag.PrintDefaults()
		fmt.Fprintf(os.Stderr, "\nExamples:\n")
		fmt.Fprintf(os.Stderr, "  animate -f scenarios/fund_onboarding.yaml\n")
		fmt.Fprintf(os.Stderr, "  animate -f scenario.yaml --speed 2.0 --no-color\n")
		fmt.Fprintf(os.Stderr, "  animate --list scenarios/\n")
	}

	flag.Parse()

	// Handle list mode
	if *scenarioDir != "" {
		if err := listScenarios(*scenarioDir); err != nil {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			os.Exit(1)
		}
		return
	}

	// Require scenario file
	if *scenarioFile == "" {
		fmt.Fprintf(os.Stderr, "Error: -f <scenario.yaml> is required\n\n")
		flag.Usage()
		os.Exit(1)
	}

	// Load scenario
	scenario, err := animate.LoadScenario(*scenarioFile)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error loading scenario: %v\n", err)
		os.Exit(1)
	}

	// Create runner
	config := animate.RunConfig{
		AgentURL:    *agentURL,
		Speed:       *speed,
		Verbose:     *verbose,
		ShowDiff:    *showDiff,
		NoColor:     *noColor,
		Interactive: *interactive,
		StopOnError: *stopOnError,
	}
	runner := animate.NewRunner(config)

	// Setup signal handling for graceful shutdown
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, os.Interrupt, syscall.SIGTERM)
	go func() {
		<-sigCh
		fmt.Println("\n\nInterrupted. Exiting...")
		cancel()
	}()

	// Run scenario
	result, err := runner.Run(ctx, *scenario)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}

	// Exit with appropriate code
	if !result.Success {
		os.Exit(1)
	}
}

func listScenarios(dir string) error {
	scenarios, err := animate.LoadAllScenarios(dir)
	if err != nil {
		return err
	}

	if len(scenarios) == 0 {
		fmt.Println("No scenarios found in", dir)
		return nil
	}

	fmt.Printf("Scenarios in %s:\n\n", dir)
	for _, s := range scenarios {
		fmt.Printf("  %-30s %d steps\n", s.Name, len(s.Steps))
		if s.Description != "" {
			fmt.Printf("    %s\n", s.Description)
		}
	}
	fmt.Println()

	return nil
}
