package cli

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"strings"
	"time"

	"dsl-ob-poc/internal/datastore"
	"dsl-ob-poc/internal/runtime"
	"dsl-ob-poc/internal/store"
)

// ListResourceTypesCommand lists available resource types
func ListResourceTypesCommand(args []string) error {
	fs := flag.NewFlagSet("list-resource-types", flag.ExitOnError)

	var (
		environment = fs.String("environment", "development", "Environment to list resource types for")
		activeOnly  = fs.Bool("active-only", true, "Show only active resource types")
		verbose     = fs.Bool("verbose", false, "Show detailed information")
	)

	if err := fs.Parse(args); err != nil {
		return err
	}

	// Initialize data store and repository
	ds, err := datastore.NewDataStore(datastore.Config{
		Type:             datastore.PostgreSQLStore,
		ConnectionString: os.Getenv("DB_CONN_STRING"),
	})
	if err != nil {
		return fmt.Errorf("failed to initialize data store: %w", err)
	}
	defer ds.Close()

	// Create PostgreSQL connection for runtime repository
	storeInstance, err := store.NewStore(os.Getenv("DB_CONN_STRING"))
	if err != nil {
		return fmt.Errorf("failed to initialize store: %w", err)
	}
	defer storeInstance.Close()

	repository := runtime.NewRepository(storeInstance.DB())
	ctx := context.Background()

	// List resource types
	resourceTypes, err := repository.ListResourceTypes(ctx, *environment, *activeOnly)
	if err != nil {
		return fmt.Errorf("failed to list resource types: %w", err)
	}

	fmt.Printf("📋 Resource Types (%s environment)\n", *environment)
	fmt.Println()

	if len(resourceTypes) == 0 {
		fmt.Println("❌ No resource types found")
		return nil
	}

	for i, rt := range resourceTypes {
		fmt.Printf("%d. %s", i+1, rt.ResourceTypeName)
		if !rt.Active {
			fmt.Print(" (inactive)")
		}
		fmt.Printf(" [v%d]\n", rt.Version)

		if *verbose {
			fmt.Printf("   ID: %s\n", rt.ResourceTypeID)
			fmt.Printf("   Description: %s\n", rt.Description)
			fmt.Printf("   Created: %s\n", rt.CreatedAt.Format(time.RFC3339))
		}
		fmt.Println()
	}

	return nil
}

// ListActionsCommand lists available action definitions
func ListActionsCommand(args []string) error {
	fs := flag.NewFlagSet("list-actions", flag.ExitOnError)

	var (
		environment = fs.String("environment", "development", "Environment to list actions for")
		verbPattern = fs.String("verb", "", "Filter by verb pattern (e.g., 'resources.create')")
		verbose     = fs.Bool("verbose", false, "Show detailed information")
	)

	if err := fs.Parse(args); err != nil {
		return err
	}

	// Initialize data store and repository
	storeInstance, err := store.NewStore(os.Getenv("DB_CONN_STRING"))
	if err != nil {
		return fmt.Errorf("failed to initialize store: %w", err)
	}
	defer storeInstance.Close()

	repository := runtime.NewRepository(storeInstance.DB())
	ctx := context.Background()

	fmt.Printf("🎯 Action Definitions (%s environment)\n", *environment)
	fmt.Println()

	var actions []*runtime.ActionDefinition
	if *verbPattern != "" {
		// Get actions for specific verb pattern
		actions, err = repository.GetActionDefinitionsByVerbPattern(ctx, *verbPattern, *environment)
		if err != nil {
			return fmt.Errorf("failed to get actions for verb pattern: %w", err)
		}
		fmt.Printf("Filtered by verb pattern: %s\n\n", *verbPattern)
	} else {
		// This would need a new repository method to list all actions
		fmt.Println("💡 Use --verb flag to filter by specific verb pattern")
		return nil
	}

	if len(actions) == 0 {
		fmt.Printf("❌ No actions found for verb pattern: %s\n", *verbPattern)
		return nil
	}

	for i, action := range actions {
		fmt.Printf("%d. %s\n", i+1, action.ActionName)
		fmt.Printf("   Verb: %s\n", action.VerbPattern)
		fmt.Printf("   Type: %s\n", action.ActionType)

		if action.ResourceTypeID != nil {
			rt, err := repository.GetResourceType(ctx, *action.ResourceTypeID)
			if err == nil {
				fmt.Printf("   Resource Type: %s\n", rt.ResourceTypeName)
			}
		}

		if *verbose {
			fmt.Printf("   ID: %s\n", action.ActionID)
			fmt.Printf("   Domain: %v\n", action.Domain)
			fmt.Printf("   Active: %t\n", action.Active)
			fmt.Printf("   Created: %s\n", action.CreatedAt.Format(time.RFC3339))

			// Show execution config summary
			fmt.Printf("   Endpoint: %s\n", action.ExecutionConfig.EndpointURL)
			fmt.Printf("   Method: %s\n", action.ExecutionConfig.Method)
			fmt.Printf("   Timeout: %ds\n", action.ExecutionConfig.TimeoutSeconds)
		}
		fmt.Println()
	}

	return nil
}

// ExecuteActionCommand manually executes an action
func ExecuteActionCommand(args []string) error {
	fs := flag.NewFlagSet("execute-action", flag.ExitOnError)

	var (
		actionID     = fs.String("action-id", "", "Action ID to execute (required)")
		cbuID        = fs.String("cbu-id", "", "CBU ID for execution context (required)")
		dslVersionID = fs.String("dsl-version-id", "", "DSL version ID (optional, uses latest if not specified)")
		environment  = fs.String("environment", "development", "Environment for execution")
		async        = fs.Bool("async", false, "Execute asynchronously (don't wait for completion)")
		verbose      = fs.Bool("verbose", false, "Show detailed execution information")
	)

	if err := fs.Parse(args); err != nil {
		return err
	}

	if *actionID == "" {
		return fmt.Errorf("action-id is required")
	}
	if *cbuID == "" {
		return fmt.Errorf("cbu-id is required")
	}

	// Initialize data store and execution engine
	ds, err := datastore.NewDataStore(datastore.Config{
		Type:             datastore.PostgreSQLStore,
		ConnectionString: os.Getenv("DB_CONN_STRING"),
	})
	if err != nil {
		return fmt.Errorf("failed to initialize data store: %w", err)
	}
	defer ds.Close()

	storeInstance, err := store.NewStore(os.Getenv("DB_CONN_STRING"))
	if err != nil {
		return fmt.Errorf("failed to initialize store: %w", err)
	}
	defer storeInstance.Close()

	engine, err := runtime.NewExecutionEngine(storeInstance.DB(), ds)
	if err != nil {
		return fmt.Errorf("failed to create execution engine: %w", err)
	}

	ctx := context.Background()

	// Get DSL version ID if not provided
	if *dslVersionID == "" {
		latest, err := ds.GetLatestDSLWithState(ctx, *cbuID)
		if err != nil {
			return fmt.Errorf("failed to get latest DSL version: %w", err)
		}
		*dslVersionID = latest.VersionID
	}

	// Create execution request
	req := &runtime.ExecutionRequest{
		ActionID:     *actionID,
		CBUID:        *cbuID,
		DSLVersionID: *dslVersionID,
		Environment:  *environment,
		TriggerContext: map[string]interface{}{
			"triggered_by": "manual_execution",
			"operator":     "cli",
			"timestamp":    time.Now().Format(time.RFC3339),
		},
	}

	fmt.Printf("🚀 Executing Action\n")
	fmt.Printf("   Action ID: %s\n", *actionID)
	fmt.Printf("   CBU ID: %s\n", *cbuID)
	fmt.Printf("   DSL Version ID: %s\n", *dslVersionID)
	fmt.Printf("   Environment: %s\n", *environment)
	fmt.Println()

	if *async {
		fmt.Println("⏳ Starting asynchronous execution...")
		// In a real implementation, you'd start the execution in a goroutine
		// and return immediately with the execution ID
	}

	startTime := time.Now()

	// Execute the action
	result, err := engine.ExecuteAction(ctx, req)
	if err != nil {
		return fmt.Errorf("execution failed: %w", err)
	}

	duration := time.Since(startTime)

	// Display results
	if result.Success {
		fmt.Printf("✅ Execution Successful\n")
		fmt.Printf("   Execution ID: %s\n", result.ExecutionID)
		fmt.Printf("   Duration: %dms\n", result.DurationMS)

		if result.HTTPStatus != nil {
			fmt.Printf("   HTTP Status: %d\n", *result.HTTPStatus)
		}

		if result.IdempotencyKey != nil {
			fmt.Printf("   Idempotency Key: %s\n", *result.IdempotencyKey)
		}

		if result.CorrelationID != nil {
			fmt.Printf("   Correlation ID: %s\n", *result.CorrelationID)
		}

		if *verbose && len(result.ResultAttributes) > 0 {
			fmt.Println("\n📋 Result Attributes:")
			for key, value := range result.ResultAttributes {
				fmt.Printf("   %s: %v\n", key, value)
			}
		}

		if *verbose && len(result.ResponsePayload) > 0 {
			fmt.Println("\n📤 Response Payload:")
			responseJSON, _ := json.MarshalIndent(result.ResponsePayload, "   ", "  ")
			fmt.Printf("   %s\n", string(responseJSON))
		}
	} else {
		fmt.Printf("❌ Execution Failed\n")
		fmt.Printf("   Execution ID: %s\n", result.ExecutionID)
		fmt.Printf("   Duration: %dms\n", result.DurationMS)

		if result.ErrorDetails != nil {
			fmt.Printf("   Error: %s\n", *result.ErrorDetails)
		}

		if result.HTTPStatus != nil {
			fmt.Printf("   HTTP Status: %d\n", *result.HTTPStatus)
		}
	}

	fmt.Printf("\n⏱️  Total CLI Duration: %dms\n", duration.Milliseconds())

	return nil
}

// ListExecutionsCommand lists action executions
func ListExecutionsCommand(args []string) error {
	fs := flag.NewFlagSet("list-executions", flag.ExitOnError)

	var (
		cbuID       = fs.String("cbu-id", "", "CBU ID to filter executions (optional)")
		limit       = fs.Int("limit", 20, "Maximum number of executions to show")
		status      = fs.String("status", "", "Filter by execution status (PENDING, RUNNING, COMPLETED, FAILED)")
		verbose     = fs.Bool("verbose", false, "Show detailed execution information")
		showPayload = fs.Bool("show-payload", false, "Show request/response payloads")
	)

	if err := fs.Parse(args); err != nil {
		return err
	}

	// Initialize repository
	storeInstance, err := store.NewStore(os.Getenv("DB_CONN_STRING"))
	if err != nil {
		return fmt.Errorf("failed to initialize store: %w", err)
	}
	defer storeInstance.Close()

	repository := runtime.NewRepository(storeInstance.DB())
	ctx := context.Background()

	fmt.Printf("📊 Action Executions\n")
	if *cbuID != "" {
		fmt.Printf("   Filtered by CBU: %s\n", *cbuID)
	}
	if *status != "" {
		fmt.Printf("   Filtered by status: %s\n", *status)
	}
	fmt.Printf("   Limit: %d\n", *limit)
	fmt.Println()

	var executions []*runtime.ActionExecution

	if *cbuID != "" {
		executions, err = repository.ListActionExecutionsByCBU(ctx, *cbuID, *limit)
		if err != nil {
			return fmt.Errorf("failed to list executions: %w", err)
		}
	} else {
		// This would need a new repository method to list all executions
		fmt.Println("💡 Use --cbu-id flag to filter by specific CBU")
		return nil
	}

	if len(executions) == 0 {
		fmt.Println("❌ No executions found")
		return nil
	}

	for i, exec := range executions {
		// Filter by status if specified
		if *status != "" && string(exec.ExecutionStatus) != strings.ToUpper(*status) {
			continue
		}

		fmt.Printf("%d. Execution %s\n", i+1, exec.ExecutionID)
		fmt.Printf("   Status: %s\n", exec.ExecutionStatus)
		fmt.Printf("   Started: %s\n", exec.StartedAt.Format(time.RFC3339))

		if exec.CompletedAt != nil {
			fmt.Printf("   Completed: %s\n", exec.CompletedAt.Format(time.RFC3339))
		}

		if exec.ExecutionDurationMS != nil {
			fmt.Printf("   Duration: %dms\n", *exec.ExecutionDurationMS)
		}

		if exec.RetryCount > 0 {
			fmt.Printf("   Retries: %d\n", exec.RetryCount)
		}

		if exec.HTTPStatus != nil {
			fmt.Printf("   HTTP Status: %d\n", *exec.HTTPStatus)
		}

		if *verbose {
			fmt.Printf("   Action ID: %s\n", exec.ActionID)
			fmt.Printf("   CBU ID: %s\n", exec.CBUID)
			fmt.Printf("   DSL Version: %s\n", exec.DSLVersionID)

			if exec.IdempotencyKey != nil {
				fmt.Printf("   Idempotency Key: %s\n", *exec.IdempotencyKey)
			}

			if exec.CorrelationID != nil {
				fmt.Printf("   Correlation ID: %s\n", *exec.CorrelationID)
			}

			if exec.Endpoint != nil {
				fmt.Printf("   Endpoint: %s\n", *exec.Endpoint)
			}
		}

		if *showPayload {
			if len(exec.RequestPayload) > 0 {
				fmt.Println("   📤 Request Payload:")
				var requestData interface{}
				if err := json.Unmarshal(exec.RequestPayload, &requestData); err == nil {
					payloadJSON, _ := json.MarshalIndent(requestData, "      ", "  ")
					fmt.Printf("      %s\n", string(payloadJSON))
				}
			}

			if len(exec.ResponsePayload) > 0 {
				fmt.Println("   📥 Response Payload:")
				var responseData interface{}
				if err := json.Unmarshal(exec.ResponsePayload, &responseData); err == nil {
					payloadJSON, _ := json.MarshalIndent(responseData, "      ", "  ")
					fmt.Printf("      %s\n", string(payloadJSON))
				}
			}

			if len(exec.ErrorDetails) > 0 {
				fmt.Println("   ❌ Error Details:")
				var errorData interface{}
				if err := json.Unmarshal(exec.ErrorDetails, &errorData); err == nil {
					errorJSON, _ := json.MarshalIndent(errorData, "      ", "  ")
					fmt.Printf("      %s\n", string(errorJSON))
				}
			}
		}

		fmt.Println()
	}

	return nil
}

// TriggerWorkflowCommand triggers actions based on DSL changes
func TriggerWorkflowCommand(args []string) error {
	fs := flag.NewFlagSet("trigger-workflow", flag.ExitOnError)

	var (
		cbuID       = fs.String("cbu-id", "", "CBU ID to trigger workflow for (required)")
		environment = fs.String("environment", "development", "Environment for execution")
		dryRun      = fs.Bool("dry-run", false, "Show what actions would be triggered without executing")
		verbose     = fs.Bool("verbose", false, "Show detailed information")
	)

	if err := fs.Parse(args); err != nil {
		return err
	}

	if *cbuID == "" {
		return fmt.Errorf("cbu-id is required")
	}

	// Initialize data store and execution engine
	ds, err := datastore.NewDataStore(datastore.Config{
		Type:             datastore.PostgreSQLStore,
		ConnectionString: os.Getenv("DB_CONN_STRING"),
	})
	if err != nil {
		return fmt.Errorf("failed to initialize data store: %w", err)
	}
	defer ds.Close()

	storeInstance, err := store.NewStore(os.Getenv("DB_CONN_STRING"))
	if err != nil {
		return fmt.Errorf("failed to initialize store: %w", err)
	}
	defer storeInstance.Close()

	ctx := context.Background()

	// Get latest DSL for the CBU
	latestDSL, err := ds.GetLatestDSLWithState(ctx, *cbuID)
	if err != nil {
		return fmt.Errorf("failed to get latest DSL: %w", err)
	}

	fmt.Printf("🔄 Triggering Workflow for CBU: %s\n", *cbuID)
	fmt.Printf("   DSL Version: %s\n", latestDSL.VersionID)
	fmt.Printf("   Current State: %s\n", latestDSL.OnboardingState)
	fmt.Printf("   Environment: %s\n", *environment)

	if *dryRun {
		fmt.Printf("   🧪 DRY RUN MODE - No actions will be executed\n")
	}
	fmt.Println()

	if *dryRun {
		// In dry run mode, just show what would be triggered
		fmt.Println("📋 Actions that would be triggered:")
		fmt.Println("   (This would analyze the DSL and show matching action definitions)")
		fmt.Println("   💡 Implement DSL analysis logic to show potential actions")
		return nil
	}

	// Create execution engine and trigger actions
	engine, err := runtime.NewExecutionEngine(storeInstance.DB(), ds)
	if err != nil {
		return fmt.Errorf("failed to create execution engine: %w", err)
	}

	// Trigger actions based on current DSL state
	results, err := engine.TriggerActionsForDSLChange(ctx, *cbuID, latestDSL.VersionID, latestDSL.DSLText, *environment)
	if err != nil {
		return fmt.Errorf("failed to trigger actions: %w", err)
	}

	if len(results) == 0 {
		fmt.Println("ℹ️  No actions were triggered for the current DSL state")
		return nil
	}

	fmt.Printf("🎯 Triggered %d action(s):\n\n", len(results))

	for i, result := range results {
		fmt.Printf("%d. Execution ID: %s\n", i+1, result.ExecutionID)
		if result.Success {
			fmt.Printf("   ✅ Status: SUCCESS\n")
		} else {
			fmt.Printf("   ❌ Status: FAILED\n")
		}
		fmt.Printf("   Duration: %dms\n", result.DurationMS)

		if result.HTTPStatus != nil {
			fmt.Printf("   HTTP Status: %d\n", *result.HTTPStatus)
		}

		if result.ErrorDetails != nil {
			fmt.Printf("   Error: %s\n", *result.ErrorDetails)
		}

		if *verbose && len(result.ResultAttributes) > 0 {
			fmt.Println("   📋 Result Attributes:")
			for key, value := range result.ResultAttributes {
				fmt.Printf("      %s: %v\n", key, value)
			}
		}

		fmt.Println()
	}

	// Show summary
	successCount := 0
	for _, result := range results {
		if result.Success {
			successCount++
		}
	}

	fmt.Printf("📊 Summary: %d/%d actions succeeded\n", successCount, len(results))

	return nil
}

// CreateActionCommand creates a new action definition
func CreateActionCommand(args []string) error {
	fs := flag.NewFlagSet("create-action", flag.ExitOnError)

	var (
		name            = fs.String("name", "", "Action name (required)")
		verb            = fs.String("verb", "", "Verb pattern (e.g., 'resources.create') (required)")
		actionType      = fs.String("type", "HTTP_API", "Action type (HTTP_API, BPMN_WORKFLOW, etc.)")
		resourceType    = fs.String("resource-type", "", "Resource type name (optional)")
		endpointURL     = fs.String("endpoint", "", "Endpoint URL (required)")
		method          = fs.String("method", "POST", "HTTP method")
		environment     = fs.String("environment", "development", "Environment")
		timeout         = fs.Int("timeout", 300, "Timeout in seconds")
		configFile      = fs.String("config-file", "", "JSON file with complete action configuration")
	)

	if err := fs.Parse(args); err != nil {
		return err
	}

	// Initialize repository
	storeInstance, err := store.NewStore(os.Getenv("DB_CONN_STRING"))
	if err != nil {
		return fmt.Errorf("failed to initialize store: %w", err)
	}
	defer storeInstance.Close()

	repository := runtime.NewRepository(storeInstance.DB())
	ctx := context.Background()

	if *configFile != "" {
		// Load action definition from JSON file
		return fmt.Errorf("config file loading not yet implemented")
	}

	// Validate required parameters
	if *name == "" {
		return fmt.Errorf("name is required")
	}
	if *verb == "" {
		return fmt.Errorf("verb pattern is required")
	}
	if *endpointURL == "" {
		return fmt.Errorf("endpoint URL is required")
	}

	// Get resource type ID if specified
	var resourceTypeID *string
	if *resourceType != "" {
		rt, err := repository.GetResourceTypeByName(ctx, *resourceType, *environment)
		if err != nil {
			return fmt.Errorf("failed to find resource type '%s': %w", *resourceType, err)
		}
		resourceTypeID = &rt.ResourceTypeID
	}

	// Create basic action definition
	action := &runtime.ActionDefinition{
		ActionName:  *name,
		VerbPattern: *verb,
		ActionType:  runtime.ActionType(*actionType),
		ResourceTypeID: resourceTypeID,
		ExecutionConfig: runtime.ExecutionConfig{
			EndpointURL:    *endpointURL,
			Method:         *method,
			TimeoutSeconds: *timeout,
			Authentication: map[string]any{},
			RetryConfig: runtime.RetryConfig{
				MaxRetries:      3,
				BackoffStrategy: "exponential",
				BaseDelayMS:     1000,
			},
			Idempotency: runtime.IdempotencyConfig{
				Header:        "Idempotency-Key",
				KeyTemplate:   "{{resource_type}}:{{environment}}:{{cbu_id}}:{{action_id}}:{{dsl_version_id}}",
				DedupeTTLSecs: 86400,
			},
			Telemetry: runtime.TelemetryConfig{
				CorrelationIDTemplate: "{{cbu_id}}:{{action_id}}:{{resource_type}}",
				PropagateTrace:        true,
			},
		},
		AttributeMapping: runtime.AttributeMapping{
			InputMapping:  []runtime.AttributeMap{},
			OutputMapping: []runtime.AttributeMap{},
		},
		TriggerConditions: []byte("{}"),
		SuccessCriteria:   []byte(`{"http_status_codes": [200, 201, 202]}`),
		FailureHandling:   []byte(`{"retry_on_codes": [500, 502, 503, 504]}`),
		Active:            true,
		Version:           1,
		Environment:       *environment,
	}

	// Create the action
	if err := repository.CreateActionDefinition(ctx, action); err != nil {
		return fmt.Errorf("failed to create action definition: %w", err)
	}

	fmt.Printf("✅ Action Created Successfully\n")
	fmt.Printf("   Action ID: %s\n", action.ActionID)
	fmt.Printf("   Name: %s\n", action.ActionName)
	fmt.Printf("   Verb Pattern: %s\n", action.VerbPattern)
	fmt.Printf("   Type: %s\n", action.ActionType)
	fmt.Printf("   Endpoint: %s\n", action.ExecutionConfig.EndpointURL)
	fmt.Printf("   Environment: %s\n", action.Environment)
	fmt.Println()
	fmt.Println("💡 Use 'list-actions --verb=" + *verb + "' to see the created action")

	return nil
}

// ManageCredentialsCommand manages stored credentials
func ManageCredentialsCommand(args []string) error {
	fs := flag.NewFlagSet("manage-credentials", flag.ExitOnError)

	var (
		action      = fs.String("action", "list", "Action: list, create, update, delete, test")
		name        = fs.String("name", "", "Credential name")
		credType    = fs.String("type", "", "Credential type (api_key, bearer, basic, oauth2, custom)")
		environment = fs.String("environment", "development", "Environment")
		apiKey      = fs.String("api-key", "", "API key value (for api_key type)")
		token       = fs.String("token", "", "Token value (for bearer type)")
		username    = fs.String("username", "", "Username (for basic type)")
		password    = fs.String("password", "", "Password (for basic type)")
		verbose     = fs.Bool("verbose", false, "Show detailed information")
	)

	if err := fs.Parse(args); err != nil {
		return err
	}

	// Initialize credential manager
	storeInstance, err := store.NewStore(os.Getenv("DB_CONN_STRING"))
	if err != nil {
		return fmt.Errorf("failed to initialize store: %w", err)
	}
	defer storeInstance.Close()

	credMgr, err := runtime.NewCredentialManager(storeInstance.DB())
	if err != nil {
		return fmt.Errorf("failed to create credential manager: %w", err)
	}

	ctx := context.Background()

	switch *action {
	case "list":
		credentials, err := credMgr.ListCredentials(ctx, *environment)
		if err != nil {
			return fmt.Errorf("failed to list credentials: %w", err)
		}

		fmt.Printf("🔐 Stored Credentials (%s environment)\n\n", *environment)

		if len(credentials) == 0 {
			fmt.Println("❌ No credentials found")
			return nil
		}

		for i, cred := range credentials {
			fmt.Printf("%d. %s (%s)\n", i+1, cred.Name, cred.Type)
			if *verbose {
				fmt.Printf("   Environment: %s\n", cred.Environment)
				fmt.Printf("   Created: %s\n", cred.CreatedAt.Format(time.RFC3339))
				if cred.ExpiresAt != nil {
					fmt.Printf("   Expires: %s\n", cred.ExpiresAt.Format(time.RFC3339))
				}
				fmt.Printf("   Active: %t\n", cred.Active)
			}
			fmt.Println()
		}

	case "create":
		if *name == "" || *credType == "" {
			return fmt.Errorf("name and type are required for create action")
		}

		fmt.Printf("🔐 Creating Credential: %s (%s)\n", *name, *credType)

		switch *credType {
		case "api_key":
			if *apiKey == "" {
				return fmt.Errorf("api-key is required for api_key type")
			}
			err = credMgr.CreateAPIKeyCredentials(ctx, *name, *environment, *apiKey)
		case "bearer":
			if *token == "" {
				return fmt.Errorf("token is required for bearer type")
			}
			err = credMgr.CreateBearerTokenCredentials(ctx, *name, *environment, *token)
		case "basic":
			if *username == "" || *password == "" {
				return fmt.Errorf("username and password are required for basic type")
			}
			err = credMgr.CreateBasicAuthCredentials(ctx, *name, *environment, *username, *password)
		default:
			return fmt.Errorf("unsupported credential type: %s", *credType)
		}

		if err != nil {
			return fmt.Errorf("failed to create credentials: %w", err)
		}

		fmt.Printf("✅ Credential created successfully\n")

	case "delete":
		if *name == "" {
			return fmt.Errorf("name is required for delete action")
		}

		fmt.Printf("🗑️  Deleting Credential: %s\n", *name)

		if err := credMgr.DeleteCredentials(ctx, *name); err != nil {
			return fmt.Errorf("failed to delete credentials: %w", err)
		}

		fmt.Printf("✅ Credential deleted successfully\n")

	case "test":
		if *name == "" {
			return fmt.Errorf("name is required for test action")
		}

		fmt.Printf("🧪 Testing Credential: %s\n", *name)

		if err := credMgr.TestCredentialConnection(ctx, *name, ""); err != nil {
			return fmt.Errorf("credential test failed: %w", err)
		}

		fmt.Printf("✅ Credential test successful\n")

	default:
		return fmt.Errorf("unsupported action: %s", *action)
	}

	return nil
}

