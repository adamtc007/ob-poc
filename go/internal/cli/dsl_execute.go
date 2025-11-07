package cli

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"os"
	"strings"

	"dsl-ob-poc/internal/datastore"
	"dsl-ob-poc/internal/dsl"
)

// RunDSLExecute executes S-expression DSL commands and shows results
func RunDSLExecute(ctx context.Context, ds datastore.DataStore, args []string) error {
	if len(args) < 1 {
		return fmt.Errorf("usage: dsl-execute [--cbu=CBU-ID] [--file=path] [--demo] [dsl-command]")
	}

	// Parse command line arguments
	var cbuID, filePath, dslCommand string
	var demo bool

	for _, arg := range args {
		switch {
		case strings.HasPrefix(arg, "--cbu="):
			cbuID = strings.TrimPrefix(arg, "--cbu=")
		case strings.HasPrefix(arg, "--file="):
			filePath = strings.TrimPrefix(arg, "--file=")
		case arg == "--demo":
			demo = true
		case strings.HasPrefix(arg, "(") && strings.HasSuffix(arg, ")"):
			dslCommand = arg
		}
	}

	// Default CBU ID
	if cbuID == "" {
		cbuID = "CBU-DEMO-001"
	}

	log.Printf("🧮 DSL S-Expression Executor")
	log.Printf("🆔 CBU ID: %s", cbuID)

	// Create DSL executor
	executor := dsl.NewDSLExecutor(cbuID)

	if demo {
		return runDemoWorkflow(executor)
	}

	if filePath != "" {
		return executeFromFile(executor, filePath)
	}

	if dslCommand != "" {
		return executeSingleCommand(executor, dslCommand)
	}

	return runInteractiveMode(executor)
}

// runDemoWorkflow executes a comprehensive demo of DSL capabilities
func runDemoWorkflow(executor *dsl.DSLExecutor) error {
	log.Printf("\n🎯 Running Complete DSL Onboarding Demo")
	log.Printf("=====================================")

	// Generate UUIDs for attributes
	custodyAttrID := dsl.GenerateTestUUID("custody-attr")
	kycAttrID := dsl.GenerateTestUUID("kyc-attr")
	complianceAttrID := dsl.GenerateTestUUID("compliance-attr")

	// Demo workflow commands
	commands := []string{
		// 1. Case creation
		`(case.create (cbu.id "CBU-DEMO-001") (nature-purpose "UCITS equity fund domiciled in Luxembourg"))`,

		// 2. Product selection
		`(products.add "CUSTODY" "FUND_ACCOUNTING" "TRANSFER_AGENT")`,

		// 3. KYC initiation
		`(kyc.start
		  (documents
		    (document "CertificateOfIncorporation")
		    (document "ArticlesOfAssociation")
		    (document "W8BEN-E")
		    (document "Prospectus")
		  )
		  (jurisdictions
		    (jurisdiction "LU")
		    (jurisdiction "US")
		  )
		)`,

		// 4. Resource planning
		`(resources.plan
		  (resource.create "CustodyAccount"
		    (owner "CustodyTech")
		    (var (attr-id "` + custodyAttrID + `"))
		  )
		)`,

		// 5. Attribute value bindings
		`(values.bind (bind (attr-id "` + custodyAttrID + `") (value "CUSTODY-ACCOUNT-001")))`,
		`(values.bind (bind (attr-id "` + kycAttrID + `") (value "KYC-PROFILE-HIGH-RISK")))`,
		`(values.bind (bind (attr-id "` + complianceAttrID + `") (value "COMPLIANCE-UCITS-LU")))`,

		// 6. Advanced verbs
		`(workflow.transition (from "CREATED") (to "IN_PROGRESS"))`,
		`(tasks.create (task.id "KYC-REVIEW-001") (type "manual-review"))`,
	}

	log.Printf("\n📝 Executing %d DSL Commands:\n", len(commands))

	// Execute all commands
	results, err := executor.ExecuteBatch(commands)
	if err != nil {
		return fmt.Errorf("demo execution failed: %w", err)
	}

	// Display results
	for i, result := range results {
		log.Printf("🔷 Command %d: %s", i+1, result.Command)
		if result.Success {
			log.Printf("  ✅ Success")
			if result.Output != nil {
				outputJSON, _ := json.MarshalIndent(result.Output, "  ", "  ")
				log.Printf("  📊 Output: %s", string(outputJSON))
			}
		} else {
			log.Printf("  ❌ Failed: %s", result.Error)
		}
		log.Printf("")
	}

	// Show execution summary
	log.Printf("\n📈 Execution Summary")
	log.Printf("===================")
	summary := executor.GetExecutionSummary()
	summaryJSON, _ := json.MarshalIndent(summary, "", "  ")
	log.Printf("%s", string(summaryJSON))

	// Show variable bindings
	log.Printf("\n🔗 Variable Bindings")
	log.Printf("===================")
	for key, value := range executor.Context.Variables {
		log.Printf("🔹 %s = %v", key, value)
	}

	log.Printf("\n🎉 Demo completed successfully!")
	log.Printf("Generated UUIDs used:")
	log.Printf("  • Custody Attribute: %s", custodyAttrID)
	log.Printf("  • KYC Attribute: %s", kycAttrID)
	log.Printf("  • Compliance Attribute: %s", complianceAttrID)

	return nil
}

// executeFromFile reads DSL commands from a file and executes them
func executeFromFile(executor *dsl.DSLExecutor, filePath string) error {
	log.Printf("📁 Reading DSL commands from: %s", filePath)

	content, err := os.ReadFile(filePath)
	if err != nil {
		return fmt.Errorf("failed to read file %s: %w", filePath, err)
	}

	// Split content into individual S-expressions
	commands := parseDSLFile(string(content))

	log.Printf("📝 Found %d DSL commands in file", len(commands))

	// Execute all commands
	results, err := executor.ExecuteBatch(commands)
	if err != nil {
		return fmt.Errorf("file execution failed: %w", err)
	}

	// Display results
	for i, result := range results {
		log.Printf("Command %d: %s - %s", i+1, result.Command,
			map[bool]string{true: "✅ Success", false: "❌ Failed"}[result.Success])
		if !result.Success {
			log.Printf("  Error: %s", result.Error)
		}
	}

	// Show final summary
	summary := executor.GetExecutionSummary()
	log.Printf("\nExecution completed: %d/%d commands successful",
		summary["commands_run"].(int)-summary["errors"].(int), summary["commands_run"])

	return nil
}

// executeSingleCommand executes a single DSL command
func executeSingleCommand(executor *dsl.DSLExecutor, command string) error {
	log.Printf("⚡ Executing single DSL command:")
	log.Printf("   %s", command)

	result, err := executor.Execute(command)
	if err != nil {
		return fmt.Errorf("execution failed: %w", err)
	}

	log.Printf("\n📊 Result:")
	if result.Success {
		log.Printf("✅ Success: %s", result.Command)
		if result.Output != nil {
			outputJSON, _ := json.MarshalIndent(result.Output, "", "  ")
			log.Printf("Output: %s", string(outputJSON))
		}
	} else {
		log.Printf("❌ Failed: %s", result.Error)
	}

	// Show variables if any were set
	if len(result.Variables) > 0 {
		log.Printf("\n🔗 Variables updated:")
		for key, value := range result.Variables {
			log.Printf("  %s = %v", key, value)
		}
	}

	return nil
}

// runInteractiveMode provides an interactive DSL shell
func runInteractiveMode(executor *dsl.DSLExecutor) error {
	log.Printf("\n🖥️  Interactive DSL Shell")
	log.Printf("========================")
	log.Printf("Enter DSL S-expressions to execute (type 'exit' to quit, 'help' for commands)")
	log.Printf("Example: (case.create (cbu.id \"CBU-123\") (nature-purpose \"Test fund\"))")

	for {
		fmt.Print("\ndsl> ")

		var input string
		if _, err := fmt.Scanln(&input); err != nil {
			fmt.Printf("Error reading input: %v\n", err)
		}

		if input == "exit" {
			log.Printf("👋 Goodbye!")
			break
		}

		if input == "help" {
			showHelp()
			continue
		}

		if input == "summary" {
			summary := executor.GetExecutionSummary()
			summaryJSON, _ := json.MarshalIndent(summary, "", "  ")
			log.Printf("Current state: %s", string(summaryJSON))
			continue
		}

		if input == "vars" {
			log.Printf("Current variables:")
			for key, value := range executor.Context.Variables {
				log.Printf("  %s = %v", key, value)
			}
			continue
		}

		if strings.TrimSpace(input) == "" {
			continue
		}

		// Execute the command
		result, err := executor.Execute(input)
		if err != nil {
			log.Printf("❌ Parse error: %v", err)
			continue
		}

		if result.Success {
			log.Printf("✅ %s executed successfully", result.Command)
			if result.Output != nil {
				outputJSON, _ := json.MarshalIndent(result.Output, "  ", "  ")
				log.Printf("  Output: %s", string(outputJSON))
			}
		} else {
			log.Printf("❌ Execution failed: %s", result.Error)
		}
	}

	return nil
}

// parseDSLFile parses a file containing multiple S-expressions
func parseDSLFile(content string) []string {
	var commands []string
	var current strings.Builder
	depth := 0
	inQuotes := false

	for _, char := range content {
		switch char {
		case '"':
			inQuotes = !inQuotes
			current.WriteRune(char)
		case '(':
			if !inQuotes {
				depth++
			}
			current.WriteRune(char)
		case ')':
			if !inQuotes {
				depth--
			}
			current.WriteRune(char)

			// Complete S-expression found
			if depth == 0 {
				cmd := strings.TrimSpace(current.String())
				if cmd != "" {
					commands = append(commands, cmd)
				}
				current.Reset()
			}
		case '\n', '\r', '\t', ' ':
			if depth > 0 {
				current.WriteRune(char)
			}
		default:
			current.WriteRune(char)
		}
	}

	return commands
}

// showHelp displays available commands
func showHelp() {
	log.Printf(`
💡 Available Commands:
===================

DSL Commands:
  (case.create (cbu.id "ID") (nature-purpose "DESC"))
  (products.add "PROD1" "PROD2" ...)
  (kyc.start (documents (document "DOC")) ...)
  (resources.plan (resource.create "NAME" ...))
  (values.bind (bind (attr-id "UUID") (value "VAL")))

Shell Commands:
  help     - Show this help
  summary  - Show execution summary
  vars     - Show current variables
  exit     - Exit shell

Examples:
  (case.create (cbu.id "CBU-123") (nature-purpose "Test fund"))
  (products.add "CUSTODY" "FUND_ACCOUNTING")
  (values.bind (bind (attr-id "test-uuid-1") (value "test-value")))
`)
}
