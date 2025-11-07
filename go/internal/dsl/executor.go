package dsl

import (
	"fmt"
	"regexp"
	"strconv"
	"strings"
	"time"
)

// executor.go implements S-expression parsing and execution for DSL verbs with UUID attributes

// =============================================================================
// S-Expression Parser and Execution Engine
// =============================================================================

// ExecutionContext holds the state and resources for executing DSL commands
type ExecutionContext struct {
	Variables    map[string]interface{} // Variable bindings (UUID -> value)
	CurrentCBU   string                 // Current CBU being processed
	ExecutionLog []string               // Log of executed commands
	ErrorLog     []string               // Log of execution errors
	CreatedAt    time.Time              // Execution start time
	Version      int                    // DSL version being executed
	State        string                 // Current onboarding state
}

// ExecutionResult represents the result of executing a DSL command
type ExecutionResult struct {
	Success     bool                   `json:"success"`
	Command     string                 `json:"command"`
	Output      interface{}            `json:"output"`
	Error       string                 `json:"error,omitempty"`
	Variables   map[string]interface{} `json:"variables"`
	StateChange *StateChange           `json:"state_change,omitempty"`
}

// StateChange represents a change in onboarding state
type StateChange struct {
	From string `json:"from"`
	To   string `json:"to"`
}

// DSLExecutor handles parsing and execution of S-expressions
type DSLExecutor struct {
	Context *ExecutionContext
	Vocab   *DSLVocabulary
}

// NewDSLExecutor creates a new S-expression executor
func NewDSLExecutor(cbuID string) *DSLExecutor {
	return &DSLExecutor{
		Context: &ExecutionContext{
			Variables:    make(map[string]interface{}),
			CurrentCBU:   cbuID,
			ExecutionLog: []string{},
			ErrorLog:     []string{},
			CreatedAt:    time.Now(),
			Version:      1,
			State:        "CREATED",
		},
		Vocab: NewDSLVocabulary(),
	}
}

// =============================================================================
// S-Expression Parsing
// =============================================================================

// SExpression represents a parsed S-expression
type SExpression struct {
	Operator string        `json:"operator"`
	Args     []interface{} `json:"args"`
	Raw      string        `json:"raw"`
}

// ParseSExpression parses a single S-expression from a string
func (e *DSLExecutor) ParseSExpression(input string) (*SExpression, error) {
	// Clean and normalize the input
	cleaned := strings.TrimSpace(input)
	if !strings.HasPrefix(cleaned, "(") || !strings.HasSuffix(cleaned, ")") {
		return nil, fmt.Errorf("invalid S-expression: must start with '(' and end with ')'")
	}

	// Remove outer parentheses
	content := cleaned[1 : len(cleaned)-1]
	content = strings.TrimSpace(content)

	// Parse the operator and arguments
	parts := e.tokenize(content)
	if len(parts) == 0 {
		return nil, fmt.Errorf("empty S-expression")
	}

	sexpr := &SExpression{
		Operator: parts[0],
		Args:     []interface{}{},
		Raw:      input,
	}

	// Parse arguments
	for i := 1; i < len(parts); i++ {
		arg := e.parseArgument(parts[i])
		sexpr.Args = append(sexpr.Args, arg)
	}

	return sexpr, nil
}

// tokenize splits S-expression content into tokens, handling nested expressions
func (e *DSLExecutor) tokenize(content string) []string {
	var tokens []string
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
		case ' ', '\t', '\n':
			if !inQuotes && depth == 0 {
				if current.Len() > 0 {
					tokens = append(tokens, current.String())
					current.Reset()
				}
			} else {
				current.WriteRune(char)
			}
		default:
			current.WriteRune(char)
		}
	}

	if current.Len() > 0 {
		tokens = append(tokens, current.String())
	}

	return tokens
}

// parseArgument parses a single argument, which can be a string, number, or nested S-expression
func (e *DSLExecutor) parseArgument(arg string) interface{} {
	arg = strings.TrimSpace(arg)

	// Handle quoted strings
	if strings.HasPrefix(arg, "\"") && strings.HasSuffix(arg, "\"") {
		return strings.Trim(arg, "\"")
	}

	// Handle nested S-expressions
	if strings.HasPrefix(arg, "(") && strings.HasSuffix(arg, ")") {
		nested, err := e.ParseSExpression(arg)
		if err != nil {
			return arg // Return as string if parsing fails
		}
		return nested
	}

	// Handle numbers
	if num, err := strconv.ParseFloat(arg, 64); err == nil {
		if num == float64(int64(num)) {
			return int64(num)
		}
		return num
	}

	// Handle booleans
	if arg == "true" {
		return true
	}
	if arg == "false" {
		return false
	}

	// Return as string
	return arg
}

// =============================================================================
// DSL Command Execution
// =============================================================================

// Execute parses and executes a DSL command
func (e *DSLExecutor) Execute(dslCommand string) (*ExecutionResult, error) {
	// Parse the S-expression
	sexpr, err := e.ParseSExpression(dslCommand)
	if err != nil {
		return &ExecutionResult{
			Success: false,
			Command: dslCommand,
			Error:   fmt.Sprintf("Parse error: %v", err),
		}, err
	}

	// Execute the command based on operator
	result := e.executeCommand(sexpr)

	// Log the execution
	e.Context.ExecutionLog = append(e.Context.ExecutionLog, dslCommand)
	if !result.Success {
		e.Context.ErrorLog = append(e.Context.ErrorLog, result.Error)
	}

	return result, nil
}

// executeCommand executes a parsed S-expression command
func (e *DSLExecutor) executeCommand(sexpr *SExpression) *ExecutionResult {
	switch sexpr.Operator {
	case "case.create":
		return e.executeCaseCreate(sexpr)
	case "case.update":
		return e.executeCaseUpdate(sexpr)
	case "case.approve":
		return e.executeCaseApprove(sexpr)
	case "products.add":
		return e.executeProductsAdd(sexpr)
	case "kyc.start":
		return e.executeKYCStart(sexpr)
	case "services.discover":
		return e.executeServicesDiscover(sexpr)
	case "resources.plan":
		return e.executeResourcesPlan(sexpr)
	case "values.bind":
		return e.executeValuesBind(sexpr)
	case "attributes.define":
		return e.executeAttributesDefine(sexpr)
	case "workflow.transition":
		return e.executeWorkflowTransition(sexpr)
	case "tasks.create":
		return e.executeTasksCreate(sexpr)
	default:
		return &ExecutionResult{
			Success: false,
			Command: sexpr.Operator,
			Error:   fmt.Sprintf("Unknown command: %s", sexpr.Operator),
		}
	}
}

// =============================================================================
// Individual Command Executors
// =============================================================================

// executeCaseCreate handles case creation commands
func (e *DSLExecutor) executeCaseCreate(sexpr *SExpression) *ExecutionResult {
	var cbuID, naturePurpose string

	// Parse arguments
	for _, arg := range sexpr.Args {
		if nested, ok := arg.(*SExpression); ok {
			switch nested.Operator {
			case "cbu.id":
				if len(nested.Args) > 0 {
					if idStr, ok2 := nested.Args[0].(string); ok2 {
						cbuID = idStr
					}
				}
			case "nature-purpose":
				if len(nested.Args) > 0 {
					if purpose, ok2 := nested.Args[0].(string); ok2 {
						naturePurpose = purpose
					}
				}
			}
		}
	}

	if cbuID == "" {
		return &ExecutionResult{
			Success: false,
			Command: "case.create",
			Error:   "Missing required cbu.id",
		}
	}

	// Update context
	e.Context.CurrentCBU = cbuID
	e.Context.Variables["cbu.id"] = cbuID
	if naturePurpose != "" {
		e.Context.Variables["nature-purpose"] = naturePurpose
	}

	return &ExecutionResult{
		Success: true,
		Command: "case.create",
		Output: map[string]interface{}{
			"cbu_id":         cbuID,
			"nature_purpose": naturePurpose,
			"state":          e.Context.State,
		},
		Variables: e.Context.Variables,
	}
}

// executeProductsAdd handles product addition commands
func (e *DSLExecutor) executeProductsAdd(sexpr *SExpression) *ExecutionResult {
	var products []string

	// Parse product arguments
	for _, arg := range sexpr.Args {
		if product, ok := arg.(string); ok {
			products = append(products, product)
		}
	}

	if len(products) == 0 {
		return &ExecutionResult{
			Success: false,
			Command: "products.add",
			Error:   "No products specified",
		}
	}

	// Update context
	e.Context.Variables["products"] = products

	return &ExecutionResult{
		Success: true,
		Command: "products.add",
		Output: map[string]interface{}{
			"products": products,
			"count":    len(products),
		},
		Variables: e.Context.Variables,
	}
}

// executeValuesBind handles value binding to attributes
func (e *DSLExecutor) executeValuesBind(sexpr *SExpression) *ExecutionResult {
	var attrID, value string

	// Parse bind argument
	for _, arg := range sexpr.Args {
		if nested, ok := arg.(*SExpression); ok && nested.Operator == "bind" {
			for _, bindArg := range nested.Args {
				if bindNested, okBind := bindArg.(*SExpression); okBind {
					switch bindNested.Operator {
					case "attr-id":
						if len(bindNested.Args) > 0 {
							if id, okId := bindNested.Args[0].(string); okId {
								attrID = id
							}
						}
					case "value":
						if len(bindNested.Args) > 0 {
							if val, okVal := bindNested.Args[0].(string); okVal {
								value = val
							}
						}
					}
				}
			}
		}
	}

	if attrID == "" {
		return &ExecutionResult{
			Success: false,
			Command: "values.bind",
			Error:   "Missing required attr-id",
		}
	}

	// Bind the value to the attribute ID
	e.Context.Variables[attrID] = value

	return &ExecutionResult{
		Success: true,
		Command: "values.bind",
		Output: map[string]interface{}{
			"attr_id": attrID,
			"value":   value,
			"bound":   true,
		},
		Variables: e.Context.Variables,
	}
}

// executeKYCStart handles KYC initiation commands
func (e *DSLExecutor) executeKYCStart(sexpr *SExpression) *ExecutionResult {
	var documents, jurisdictions []string

	// Parse KYC arguments
	for _, arg := range sexpr.Args {
		if nested, ok := arg.(*SExpression); ok {
			switch nested.Operator {
			case "documents":
				for _, docArg := range nested.Args {
					if docNested, okDoc := docArg.(*SExpression); okDoc && docNested.Operator == "document" {
						if len(docNested.Args) > 0 {
							if doc, okDocValue := docNested.Args[0].(string); okDocValue {
								documents = append(documents, doc)
							}
						}
					}
				}
			case "jurisdictions":
				for _, jurArg := range nested.Args {
					if jurNested, okJur := jurArg.(*SExpression); okJur && jurNested.Operator == "jurisdiction" {
						if len(jurNested.Args) > 0 {
							if jur, okJurValue := jurNested.Args[0].(string); okJurValue {
								jurisdictions = append(jurisdictions, jur)
							}
						}
					}
				}
			}
		}
	}

	// Update context
	e.Context.Variables["kyc.documents"] = documents
	e.Context.Variables["kyc.jurisdictions"] = jurisdictions

	return &ExecutionResult{
		Success: true,
		Command: "kyc.start",
		Output: map[string]interface{}{
			"documents":     documents,
			"jurisdictions": jurisdictions,
			"started":       true,
		},
		Variables: e.Context.Variables,
	}
}

// executeResourcesPlan handles resource planning commands
func (e *DSLExecutor) executeResourcesPlan(sexpr *SExpression) *ExecutionResult {
	var resourceName, owner, attrID string

	// Parse resource.create nested command
	for _, arg := range sexpr.Args {
		if nested, ok := arg.(*SExpression); ok && nested.Operator == "resource.create" {
			if len(nested.Args) > 0 {
				if name, ok1 := nested.Args[0].(string); ok1 {
					resourceName = name
				}
			}

			for i := 1; i < len(nested.Args); i++ {
				if nestedArg, ok2 := nested.Args[i].(*SExpression); ok2 {
					switch nestedArg.Operator {
					case "owner":
						if len(nestedArg.Args) > 0 {
							if ownerVal, ok3 := nestedArg.Args[0].(string); ok3 {
								owner = ownerVal
							}
						}
					case "var":
						for _, varArg := range nestedArg.Args {
							if varNested, ok4 := varArg.(*SExpression); ok4 && varNested.Operator == "attr-id" {
								if len(varNested.Args) > 0 {
									if id, ok5 := varNested.Args[0].(string); ok5 {
										attrID = id
									}
								}
							}
						}
					}
				}
			}
		}
	}

	if resourceName == "" {
		return &ExecutionResult{
			Success: false,
			Command: "resources.plan",
			Error:   "Missing required resource name",
		}
	}

	// Create resource entry
	resourceID := GenerateTestUUID("resource")
	resource := map[string]interface{}{
		"id":      resourceID,
		"name":    resourceName,
		"owner":   owner,
		"attr_id": attrID,
		"planned": true,
	}

	// Update context
	e.Context.Variables[resourceID] = resource
	if attrID != "" {
		e.Context.Variables[attrID+".resource"] = resourceID
	}

	return &ExecutionResult{
		Success:   true,
		Command:   "resources.plan",
		Output:    resource,
		Variables: e.Context.Variables,
	}
}

// Placeholder implementations for other commands
func (e *DSLExecutor) executeCaseUpdate(_ *SExpression) *ExecutionResult {
	return &ExecutionResult{Success: true, Command: "case.update", Output: "Updated"}
}

func (e *DSLExecutor) executeCaseApprove(_ *SExpression) *ExecutionResult {
	return &ExecutionResult{Success: true, Command: "case.approve", Output: "Approved"}
}

func (e *DSLExecutor) executeServicesDiscover(_ *SExpression) *ExecutionResult {
	return &ExecutionResult{Success: true, Command: "services.discover", Output: "Services discovered"}
}

func (e *DSLExecutor) executeAttributesDefine(_ *SExpression) *ExecutionResult {
	return &ExecutionResult{Success: true, Command: "attributes.define", Output: "Attribute defined"}
}

func (e *DSLExecutor) executeWorkflowTransition(_ *SExpression) *ExecutionResult {
	return &ExecutionResult{Success: true, Command: "workflow.transition", Output: "State transitioned"}
}

func (e *DSLExecutor) executeTasksCreate(sexpr *SExpression) *ExecutionResult {
	var taskID, taskType string

	// Parse task arguments
	for _, arg := range sexpr.Args {
		if nested, ok := arg.(*SExpression); ok {
			switch nested.Operator {
			case "task.id":
				if len(nested.Args) > 0 {
					if id, ok1 := nested.Args[0].(string); ok1 {
						taskID = id
					}
				}
			case "type":
				if len(nested.Args) > 0 {
					if typ, ok1 := nested.Args[0].(string); ok1 {
						taskType = typ
					}
				}
			}
		}
	}

	if taskID == "" {
		return &ExecutionResult{
			Success: false,
			Command: "tasks.create",
			Error:   "Missing required task.id",
		}
	}

	// Create task entry
	task := map[string]interface{}{
		"id":      taskID,
		"type":    taskType,
		"created": true,
		"status":  "pending",
	}

	// Update context
	e.Context.Variables[taskID] = task

	return &ExecutionResult{
		Success:   true,
		Command:   "tasks.create",
		Output:    task,
		Variables: e.Context.Variables,
	}
}

// =============================================================================
// Utility Functions
// =============================================================================

// ExecuteBatch executes multiple DSL commands in sequence
func (e *DSLExecutor) ExecuteBatch(commands []string) ([]*ExecutionResult, error) {
	results := make([]*ExecutionResult, 0, len(commands))

	for _, command := range commands {
		result, err := e.Execute(command)
		if err != nil {
			return results, err
		}
		results = append(results, result)

		// Stop on first error if desired
		if !result.Success {
			break
		}
	}

	return results, nil
}

// GetExecutionSummary returns a summary of the execution context
func (e *DSLExecutor) GetExecutionSummary() map[string]interface{} {
	return map[string]interface{}{
		"cbu_id":        e.Context.CurrentCBU,
		"state":         e.Context.State,
		"version":       e.Context.Version,
		"variables":     e.Context.Variables,
		"commands_run":  len(e.Context.ExecutionLog),
		"errors":        len(e.Context.ErrorLog),
		"execution_log": e.Context.ExecutionLog,
		"created_at":    e.Context.CreatedAt,
	}
}

// ValidateUUID checks if a string is a valid UUID format
func ValidateUUID(uuid string) bool {
	uuidRegex := regexp.MustCompile(`^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$`)
	return uuidRegex.MatchString(uuid)
}
