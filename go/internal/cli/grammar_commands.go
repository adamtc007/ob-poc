package cli

import (
	"context"
	"flag"
	"fmt"
	"os"

	"dsl-ob-poc/internal/datastore"
	"dsl-ob-poc/internal/grammar"
)

// InitializeGrammarCommand initializes the DSL grammar system
func InitializeGrammarCommand(ctx context.Context, ds datastore.DataStore, args []string) error {
	fs := flag.NewFlagSet("init-grammar", flag.ExitOnError)

	var (
		force = fs.Bool("force", false, "Force reinitialize even if grammar rules exist")
	)

	if err := fs.Parse(args); err != nil {
		return err
	}

	fmt.Println("🔧 Initializing DSL Grammar System...")

	if err := grammar.InitializeGrammarCLI(ctx, ds); err != nil {
		return fmt.Errorf("failed to initialize grammar: %w", err)
	}

	if *force {
		fmt.Println("🔄 Force mode enabled - existing rules may be updated")
	}

	fmt.Println("✅ Grammar system initialized successfully!")
	fmt.Println()
	fmt.Println("📚 Available grammar features:")
	fmt.Println("   • EBNF-based DSL parsing")
	fmt.Println("   • Domain-specific grammar rules")
	fmt.Println("   • Syntax validation")
	fmt.Println("   • Abstract Syntax Tree generation")
	fmt.Println()
	fmt.Println("💡 Use 'validate-grammar' to test DSL validation")

	return nil
}

// ValidateGrammarCommand validates DSL using grammar rules
func ValidateGrammarCommand(ctx context.Context, ds datastore.DataStore, args []string) error {
	fs := flag.NewFlagSet("validate-grammar", flag.ExitOnError)

	var (
		dslFile = fs.String("file", "", "DSL file to validate (required)")
		dslText = fs.String("dsl", "", "DSL text to validate (alternative to -file)")
		domain  = fs.String("domain", "onboarding", "Domain for validation (onboarding, hedge-fund-investor, etc.)")
		verbose = fs.Bool("verbose", false, "Show detailed parsing information")
	)

	if err := fs.Parse(args); err != nil {
		return err
	}

	// Get DSL content
	var dsl string
	if *dslFile != "" {
		content, err := os.ReadFile(*dslFile)
		if err != nil {
			return fmt.Errorf("failed to read DSL file %s: %w", *dslFile, err)
		}
		dsl = string(content)
		fmt.Printf("📁 Validating DSL file: %s\n", *dslFile)
	} else if *dslText != "" {
		dsl = *dslText
		fmt.Println("📝 Validating provided DSL text")
	} else {
		return fmt.Errorf("either -file or -dsl must be provided")
	}

	if *verbose {
		fmt.Printf("🔍 Domain: %s\n", *domain)
		fmt.Printf("📊 DSL length: %d characters\n", len(dsl))
		fmt.Println("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")
	}

	// Validate the DSL
	if err := grammar.ValidateDSLCLI(ctx, ds, dsl, *domain); err != nil {
		return fmt.Errorf("DSL validation failed: %w", err)
	}

	return nil
}
