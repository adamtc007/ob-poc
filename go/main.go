package main

import (
	"context"
	"fmt"
	"log"
	"os"

	"dsl-ob-poc/internal/agent"
	"dsl-ob-poc/internal/cli"
	"dsl-ob-poc/internal/config"
	"dsl-ob-poc/internal/datastore"
)

// getAPIKey looks for GEMINI_API_KEY first, then falls back to GOOGLE_API_KEY
func getAPIKey() string {
	if apiKey := os.Getenv("GEMINI_API_KEY"); apiKey != "" {
		return apiKey
	}
	if apiKey := os.Getenv("GOOGLE_API_KEY"); apiKey != "" {
		log.Println("ℹ️ Using GOOGLE_API_KEY for Gemini API (consider setting GEMINI_API_KEY)")
		return apiKey
	}
	return ""
}

func main() {
	os.Exit(run())
}

func run() int {
	if len(os.Args) < 2 {
		printUsage()
		return 1
	}

	command := os.Args[1]
	args := os.Args[2:]

	// Handle help command without DB connection
	if command == "help" {
		printUsage()
		return 0
	}

	// All other commands require data store connection
	cfg := config.GetDataStoreConfig()

	dataStore, err := datastore.NewDataStore(cfg)
	if err != nil {
		log.Printf("Failed to initialize data store: %v", err)
		return 1
	}
	defer dataStore.Close()

	// Print mode information for clarity
	if config.IsMockMode() {
		fmt.Printf("Running in MOCK mode (data from: %s)\n", cfg.MockDataPath)
	} else {
		fmt.Println("Running in DATABASE mode")
	}

	ctx := context.Background()

	switch command {
	case "init-db":
		err = dataStore.InitDB(ctx)
		if err != nil {
			log.Printf("Failed to initialize database: %v", err)
			return 1
		}
		fmt.Println("Database initialized successfully.")

	case "seed-catalog":
		err = dataStore.SeedCatalog(ctx)
		if err != nil {
			log.Printf("Failed to seed catalog: %v", err)
			return 1
		}
		fmt.Println("Catalog seeded successfully with mock data.")


	case "create":
		err = cli.RunCreate(ctx, dataStore, args)

	case "add-products":
		err = cli.RunAddProducts(ctx, dataStore, args)

	case "discover-kyc":
		apiKey := getAPIKey()
		aiAgent, agentErr := agent.NewAgent(ctx, apiKey)
		if agentErr != nil {
			log.Printf("Failed to initialize AI agent: %v", agentErr)
			return 1
		}
		if aiAgent == nil {
			log.Println("Error: Neither GEMINI_API_KEY nor GOOGLE_API_KEY environment variable is set.")
			return 1
		}
		defer aiAgent.Close()

		err = cli.RunDiscoverKYC(ctx, dataStore, aiAgent, args)

	case "agent-transform":
		apiKey := getAPIKey()
		aiAgent, agentErr := agent.NewAgent(ctx, apiKey)
		if agentErr != nil {
			log.Printf("Failed to initialize AI agent: %v", agentErr)
			return 1
		}
		if aiAgent == nil {
			log.Println("Error: Neither GEMINI_API_KEY nor GOOGLE_API_KEY environment variable is set.")
			return 1
		}
		defer aiAgent.Close()

		err = cli.RunAgentTransform(ctx, dataStore, aiAgent, args)

	case "agent-validate":
		apiKey := getAPIKey()
		aiAgent, agentErr := agent.NewAgent(ctx, apiKey)
		if agentErr != nil {
			log.Printf("Failed to initialize AI agent: %v", agentErr)
			return 1
		}
		if aiAgent == nil {
			log.Println("Error: Neither GEMINI_API_KEY nor GOOGLE_API_KEY environment variable is set.")
			return 1
		}
		defer aiAgent.Close()

		err = cli.RunAgentValidate(ctx, dataStore, aiAgent, args)

	case "agent-demo":
		err = cli.RunAgentDemo(ctx, dataStore, args)

	case "agent-test":
		err = cli.RunAgentTest(ctx, dataStore, args)

	case "agent-prompt-capture":
		apiKey := getAPIKey()
		aiAgent, agentErr := agent.NewAgent(ctx, apiKey)
		if agentErr != nil {
			log.Printf("Failed to initialize AI agent: %v", agentErr)
		}
		// Allow running with or without API key for prompt capture demonstration
		err = cli.RunAgentPromptCapture(ctx, dataStore, aiAgent, args)

	case "discover-services":
		err = cli.RunDiscoverServices(ctx, dataStore, args)

	case "discover-resources":
		err = cli.RunDiscoverResources(ctx, dataStore, args)

	case "discover-ubo":
		apiKey := getAPIKey()
		aiAgent, agentErr := agent.NewAgent(ctx, apiKey)
		if agentErr != nil {
			log.Printf("Warning: Failed to initialize AI agent: %v", agentErr)
		}
		// Allow running with or without AI agent for UBO discovery
		err = cli.RunDiscoverUBO(ctx, dataStore, aiAgent, args)

	case "populate-attributes":
		err = cli.RunPopulateAttributes(ctx, dataStore, args)

	case "get-attribute-values":
		err = cli.RunGetAttributeValues(ctx, dataStore, args)

	// NEW COMMAND
	case "history":
		err = cli.RunHistory(ctx, dataStore, args)

	// MULTI-DOMAIN ORCHESTRATION COMMANDS
	case "orchestration-init-db":
		err = cli.RunOrchestrationInitDB(ctx, dataStore, args)
	case "orchestrate-create":
		err = cli.RunOrchestrationCreate(ctx, dataStore, args)
	case "orchestrate-execute":
		err = cli.RunOrchestrationExecute(ctx, dataStore, args)
	case "orchestrate-status":
		err = cli.RunOrchestrationStatus(ctx, dataStore, args)
	case "orchestrate-list":
		err = cli.RunOrchestrationList(ctx, dataStore, args)
	case "orchestrate-demo":
		err = cli.RunOrchestrationDemo(ctx, dataStore, args)

	// CBU CRUD COMMANDS
	case "cbu-create":
		err = cli.RunCBUCreate(ctx, dataStore, args)
	case "cbu-list":
		err = cli.RunCBUList(ctx, dataStore, args)
	case "cbu-get":
		err = cli.RunCBUGet(ctx, dataStore, args)
	case "cbu-update":
		err = cli.RunCBUUpdate(ctx, dataStore, args)
	case "cbu-delete":
		err = cli.RunCBUDelete(ctx, dataStore, args)

	// ROLE CRUD COMMANDS
	case "role-create":
		err = cli.RunRoleCreate(ctx, dataStore, args)
	case "role-list":
		err = cli.RunRoleList(ctx, dataStore, args)
	case "role-get":
		err = cli.RunRoleGet(ctx, dataStore, args)
	case "role-update":
		err = cli.RunRoleUpdate(ctx, dataStore, args)
	case "role-delete":
		err = cli.RunRoleDelete(ctx, dataStore, args)

	// MOCK DATA EXPORT
	case "export-mock-data":
		err = cli.RunExportMockData(ctx, dataStore, args)

	// DSL S-EXPRESSION EXECUTION
	case "dsl-execute":
		err = cli.RunDSLExecute(ctx, dataStore, args)

	case "validate-dsl":
		err = cli.RunValidateDSL(ctx, dataStore, args)

	// PHASE 6 COMPILE-TIME OPTIMIZATION
	case "optimize":
		err = cli.RunOptimize(ctx, dataStore, args)

	// PHASE 4 DATABASE MIGRATION
	case "migrate-vocabulary":
		err = cli.RunMigrateVocabulary(ctx, args)

	case "test-db-vocabulary":
		err = cli.RunTestDBVocabulary(ctx, args)

	// Vector and semantic search commands
	case "regenerate-vectors":
		err = cli.RegenerateVectorsCommand(args)
	case "search-attributes":
		err = cli.SearchAttributesCommand(args)

	// Grammar and EBNF commands
	case "init-grammar":
		err = cli.InitializeGrammarCommand(ctx, dataStore, args)
	case "validate-grammar":
		err = cli.ValidateGrammarCommand(ctx, dataStore, args)

	// RUNTIME API ENDPOINTS COMMANDS
	case "list-resource-types":
		err = cli.ListResourceTypesCommand(args)
	case "list-actions":
		err = cli.ListActionsCommand(args)
	case "execute-action":
		err = cli.ExecuteActionCommand(args)
	case "list-executions":
		err = cli.ListExecutionsCommand(args)
	case "trigger-workflow":
		err = cli.TriggerWorkflowCommand(args)
	case "create-action":
		err = cli.CreateActionCommand(args)
	case "manage-credentials":
		err = cli.ManageCredentialsCommand(args)

	default:
		fmt.Printf("Unknown command: %s\n", command)
		printUsage()
		return 1
	}

	if err != nil {
		log.Printf("Command failed: %v", err)
		return 1
	}

	return 0
}

func printUsage() {
	fmt.Println("Onboarding DSL POC CLI (Core Onboarding System)")
	fmt.Println("Usage: dsl-poc <command> [options]")
	fmt.Println("\nDSL Manager Commands:")
	fmt.Println("  dsl-create-case --domain=<domain> [--investor-name=<name>] [--investor-type=<type>]")
	fmt.Println("                 Create a new DSL case with optional domain-specific details")
	fmt.Println("  dsl-update-case --onboarding-id=<id> --state=<new_state> [--dsl=<dsl_fragment>]")
	fmt.Println("                 Update an existing DSL case with a new state and optional DSL fragment")
	fmt.Println("  dsl-get-case --onboarding-id=<id>")
	fmt.Println("                 Retrieve full details of a specific DSL case")
	fmt.Println("  dsl-list-cases")
	fmt.Println("                 List all active DSL case IDs")
	fmt.Println("\nEnvironment Variables:")
	fmt.Println("  DSL_STORE_TYPE         Set to 'mock' for disconnected mode, 'postgresql' for database mode (default)")
	fmt.Println("  DSL_MOCK_DATA_PATH     Path to mock data directory (default: data/mocks)")
	fmt.Println("  DB_CONN_STRING         PostgreSQL connection string (required for database mode)")
	fmt.Println("\nSetup Commands:")
	fmt.Println("  init-db                      (One-time) Initializes the PostgreSQL schema and all tables.")
	fmt.Println("  seed-catalog                 (One-time) Populates catalog tables with mock data.")
	fmt.Println("  seed-product-requirements    (Phase 5) Seeds product requirements into database.")
	fmt.Println("\nState Machine Commands:")
	fmt.Println("  create --cbu=<cbu-id>        (v1) Creates a new onboarding case.")
	fmt.Println("  add-products --cbu=<cbu-id>  (v2) Adds products to an existing case.")
	fmt.Println("               --products=<p1,p2>")
	fmt.Println("  discover-kyc --cbu=<cbu-id>  (v3) Performs AI-assisted KYC discovery.")
	fmt.Println("  discover-ubo --cbu=<cbu-id>  (v4) Performs Ultimate Beneficial Owner discovery.")
	fmt.Println("               --entity=<name> --jurisdiction=<code> [--threshold=<pct>]")
	fmt.Println("  discover-services --cbu=<cbu-id> (v5) Discovers and appends services plan.")
	fmt.Println("  discover-resources --cbu=<cbu-id> (v6) Discovers and appends resources plan.")
	fmt.Println("  populate-attributes --cbu=<cbu-id> (v7) Populates attribute values from runtime sources.")
	fmt.Println("  get-attribute-values --cbu=<cbu-id> (v8) Resolves and binds attribute values deterministically.")

	fmt.Println("\nDSL Lifecycle Management Commands:")
	fmt.Println("  validate-dsl <file_path>     Validates a DSL file.")

	fmt.Println("\nAI Agent Commands (requires GEMINI_API_KEY):")
	fmt.Println("  agent-transform --cbu=<cbu-id>   AI-powered DSL transformation with natural language instructions")
	fmt.Println("                  --instruction=<text> [--target-state=<state>] [--save]")
	fmt.Println("  agent-validate --cbu=<cbu-id>    AI-powered DSL validation and improvement suggestions")
	fmt.Println("  agent-demo [--cbu=<cbu-id>]      Demonstrates AI agent capabilities (no API key required)")
	fmt.Println("  agent-test [--cbu=<cbu-id>]      Tests AI agents with mock responses (no API key required)")
	fmt.Println("             [--type=<kyc|transform|validate|all>]")
	fmt.Println("  agent-prompt-capture [--cbu=<cbu-id>] [--type=<kyc|transform|validate|all>] [--output=<file>]")
	fmt.Println("                       Captures and displays exact AI prompts and responses for analysis")
	fmt.Println("\nCBU Management Commands:")
	fmt.Println("  cbu-create --name=<name> [--description=<desc>] [--nature-purpose=<purpose>]")
	fmt.Println("  cbu-list                     Lists all CBUs")
	fmt.Println("  cbu-get --id=<cbu-id>        Get CBU details")
	fmt.Println("  cbu-update --id=<cbu-id> [--name=<name>] [--description=<desc>] [--nature-purpose=<purpose>]")
	fmt.Println("  cbu-delete --id=<cbu-id>     Delete CBU")
	fmt.Println("\nRole Management Commands:")
	fmt.Println("  role-create --name=<name> [--description=<desc>]")
	fmt.Println("  role-list                    Lists all roles")
	fmt.Println("  role-get --id=<role-id>      Get role details")
	fmt.Println("  role-update --id=<role-id> [--name=<name>] [--description=<desc>]")
	fmt.Println("  role-delete --id=<role-id>   Delete role")
	fmt.Println("\nUtility Commands:")
	fmt.Println("  history --cbu=<cbu-id>       Views the full, versioned DSL evolution for a case.")
	fmt.Println("  export-mock-data [--dir=<path>] Exports existing database records to JSON mock files")
	fmt.Println("\nMulti-Domain Orchestration Commands:")
	fmt.Println("  orchestration-init-db        (One-time) Initialize orchestration session tables")
	fmt.Println("  orchestrate-create --entity-name=<name> --entity-type=<type> [--products=<list>]")
	fmt.Println("                     Create a new multi-domain orchestrated workflow")
	fmt.Println("  orchestrate-execute --session-id=<id> --instruction=<text>")
	fmt.Println("                     Execute an instruction across multiple domains")
	fmt.Println("  orchestrate-status --session-id=<id> [--show-dsl]")
	fmt.Println("                     Show status of an orchestration session")
	fmt.Println("  orchestrate-list [--metrics]")
	fmt.Println("                     List all active orchestration sessions")
	fmt.Println("  orchestrate-demo [--entity-type=<type>] [--fast]")
	fmt.Println("                     Run a comprehensive orchestration demo")
	fmt.Println("\nDSL Execution Engine:")
	fmt.Println("  dsl-execute [--cbu=<cbu-id>] [--demo] [--file=<path>] [dsl-command]")
	fmt.Println("              Execute S-expression DSL commands with UUID attribute handling")
	fmt.Println("              --demo: Run comprehensive demo workflow")
	fmt.Println("              --file: Execute commands from file")
	fmt.Println("              Interactive mode if no command specified")
	fmt.Println("\nPhase 4 Database Migration:")
	fmt.Println("  migrate-vocabulary [--domain=<domain>] [--all] [--verify] [--cleanup] [--dry-run]")
	fmt.Println("                     Migrate hardcoded vocabularies to database storage")
	fmt.Println("                     --domain: Migrate specific domain (onboarding, hedge-fund-investor, orchestration)")
	fmt.Println("                     --all: Migrate all domains")
	fmt.Println("                     --verify: Verify migration integrity")
	fmt.Println("                     --cleanup: Remove hardcoded references (WARNING: modifies code)")
	fmt.Println("                     --dry-run: Show what would be migrated without making changes")
	fmt.Println("  test-db-vocabulary [--domain=<domain>] [--verb=<verb>] [--dsl=<dsl>]")
	fmt.Println("                     Test database-backed vocabulary validation (Phase 4 verification)")

	fmt.Println("\nVector Database Commands:")
	fmt.Println("  regenerate-vectors [--attribute-id=<id>] [--validate] [--stats]")
	fmt.Println("                     Regenerate semantic vectors for dictionary attributes")
	fmt.Println("  search-attributes --query=<text> [--limit=<n>]")
	fmt.Println("                     Search for similar attributes using semantic vectors")

	fmt.Println("\nGrammar and EBNF Commands:")
	fmt.Println("  init-grammar [--force]")
	fmt.Println("                     Initialize EBNF grammar system with default rules")
	fmt.Println("  validate-grammar --file=<dsl_file> [--domain=<domain>] [--verbose]")
	fmt.Println("                     Validate DSL using EBNF grammar rules")
	fmt.Println("  validate-grammar --dsl=<dsl_text> [--domain=<domain>] [--verbose]")
	fmt.Println("                     Validate DSL text using EBNF grammar rules")

	fmt.Println("\nPhase 6 Compile-Time Optimization:")
	fmt.Println("  optimize --cbu=<cbu-id> [--output=<file>] [--format=json|yaml|text] [--strategy=BALANCED|COST_OPTIMIZED|PERFORMANCE_OPTIMIZED]")
	fmt.Println("           [--max-cost=<amount>] [--session-id=<id>] [--save-results] [--verbose]")
	fmt.Println("                     Perform compile-time optimization and execution planning for DSL documents")
	fmt.Println("                     Includes dependency analysis, resource optimization, and execution planning")

	fmt.Println("\nRuntime API Endpoints & Execution:")
	fmt.Println("  list-resource-types [--environment=<env>] [--active-only] [--verbose]")
	fmt.Println("                     List available resource types for runtime execution")
	fmt.Println("  list-actions --verb=<pattern> [--environment=<env>] [--verbose]")
	fmt.Println("                     List action definitions for specific verb patterns")
	fmt.Println("  execute-action --action-id=<id> --cbu-id=<cbu> [--dsl-version-id=<id>] [--environment=<env>] [--async] [--verbose]")
	fmt.Println("                     Manually execute a specific action definition")
	fmt.Println("  list-executions --cbu-id=<cbu> [--limit=<n>] [--status=<status>] [--verbose] [--show-payload]")
	fmt.Println("                     List action execution history and results")
	fmt.Println("  trigger-workflow --cbu-id=<cbu> [--environment=<env>] [--dry-run] [--verbose]")
	fmt.Println("                     Trigger actions based on current DSL state")
	fmt.Println("  create-action --name=<name> --verb=<pattern> --endpoint=<url> [--type=<type>] [--method=<method>] [--timeout=<sec>]")
	fmt.Println("                     [--resource-type=<type>] [--environment=<env>] [--config-file=<file>]")
	fmt.Println("                     Create new action definition for runtime execution")
	fmt.Println("  manage-credentials --action=<list|create|delete|test> [--name=<name>] [--type=<type>] [--environment=<env>]")
	fmt.Println("                     [--api-key=<key>] [--token=<token>] [--username=<user>] [--password=<pass>] [--verbose]")
	fmt.Println("                     Manage encrypted credentials for API authentication")
}
