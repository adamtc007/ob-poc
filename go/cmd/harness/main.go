// CLI harness runner for DSL test suites.
package main

import (
	"context"
	"flag"
	"fmt"
	"os"

	"github.com/adamtc007/ob-poc/go/internal/harness"
	"github.com/adamtc007/ob-poc/go/internal/rustclient"
)

func main() {
	baseURL := flag.String("url", "http://localhost:3000", "Rust API base URL")
	verbose := flag.Bool("v", false, "Verbose output")
	flag.Parse()

	ctx := context.Background()
	runner := harness.NewRunner(*baseURL).WithVerbose(*verbose)

	// Example suite - replace with real tests
	suite := harness.Suite{
		Name:        "Basic DSL Tests",
		Description: "Tests basic CBU and entity operations",
		Cases: []harness.Case{
			{
				Name: "Create CBU",
				DSL:  `(cbu.ensure :name "Test Fund" :jurisdiction "LU" :client-type "fund" :as @fund)`,
				Expect: harness.Expectation{
					Success:     true,
					EntityCount: intPtr(1),
				},
			},
			{
				Name: "Create Entity",
				DSL:  `(entity.create-proper-person :first-name "John" :last-name "Smith" :as @john)`,
				Expect: harness.Expectation{
					Success:     true,
					EntityCount: intPtr(1),
				},
			},
			{
				Name: "Invalid DSL",
				DSL:  `(invalid.verb :foo "bar")`,
				Expect: harness.Expectation{
					Success: false,
				},
			},
		},
	}

	// Check API health first
	client := rustclient.NewClient(*baseURL)
	health, err := client.Health(ctx)
	if err != nil {
		fmt.Fprintf(os.Stderr, "API not reachable: %v\n", err)
		os.Exit(1)
	}
	fmt.Printf("API: %s v%s (%d verbs)\n\n", health.Status, health.Version, health.VerbCount)

	// Run suite
	result, err := runner.Run(ctx, suite)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Suite error: %v\n", err)
		os.Exit(1)
	}

	// Print results
	fmt.Printf("Suite: %s\n", result.Name)
	fmt.Printf("Duration: %v\n", result.Duration)
	fmt.Printf("Passed: %d, Failed: %d, Skipped: %d\n\n", result.Passed, result.Failed, result.Skipped)

	for _, r := range result.Results {
		status := "PASS"
		if r.Skipped {
			status = "SKIP"
		} else if !r.Passed {
			status = "FAIL"
		}
		fmt.Printf("[%s] %s (%v)\n", status, r.Case, r.Duration)
		if r.Error != "" {
			fmt.Printf("       Error: %s\n", r.Error)
		}
	}

	if result.Failed > 0 {
		os.Exit(1)
	}
}

func intPtr(i int) *int { return &i }
