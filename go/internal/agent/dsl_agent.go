// DEPRECATED: This file is marked for deletion as part of multi-domain migration.
//
// Migration Status: Phase 4 - Create Onboarding Domain
// New Location: internal/domains/onboarding/agent.go
// Deprecation Date: 2024-01-XX
// Planned Deletion: After Phase 6 complete and all tests passing
//
// DO NOT MODIFY THIS FILE - It is kept for reference and regression testing only.
// See MIGRATION_DEPRECATION_TRACKER.md for details.
//
// This file contains the onboarding DSL transformation agent and verb validation
// that will be migrated to the new domain-specific agent system.

package agent

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"regexp"
	"strings"

	"github.com/google/generative-ai-go/genai"
)

// DSLTransformationRequest represents a request to transform DSL
type DSLTransformationRequest struct {
	CurrentDSL  string                 `json:"current_dsl"`
	Instruction string                 `json:"instruction"`
	TargetState string                 `json:"target_state"`
	Context     map[string]interface{} `json:"context,omitempty"`
}

// DSLTransformationResponse represents the AI agent's response
type DSLTransformationResponse struct {
	NewDSL      string   `json:"new_dsl"`
	Explanation string   `json:"explanation"`
	Changes     []string `json:"changes"`
	Confidence  float64  `json:"confidence"`
}

// CallDSLTransformationAgent handles general DSL transformations using AI
func (a *Agent) CallDSLTransformationAgent(ctx context.Context, request DSLTransformationRequest) (*DSLTransformationResponse, error) {
	if a == nil || a.model == nil {
		return nil, fmt.Errorf("ai agent is not initialized")
	}

	systemPrompt := `You are an expert DSL (Domain Specific Language) architect for financial onboarding workflows.
Your role is to analyze existing DSL and transform it according to user instructions while maintaining correctness and consistency.

APPROVED DSL VERBS (MUST USE ONLY THESE):
- case.create, case.update, case.validate, case.approve, case.close
- entity.register, entity.classify, entity.link, identity.verify, identity.attest
- products.add, products.configure, services.discover, services.provision, services.activate
- kyc.start, kyc.collect, kyc.verify, kyc.assess, compliance.screen, compliance.monitor
- ubo.collect-entity-data, ubo.get-ownership-structure, ubo.unroll-structure, ubo.resolve-ubos
- ubo.calculate-indirect-ownership, ubo.identify-control-prong, ubo.apply-thresholds
- ubo.identify-trust-parties, ubo.resolve-trust-ubos, ubo.identify-ownership-prong, ubo.resolve-partnership-ubos
- ubo.recursive-entity-resolve, ubo.identify-fincen-control-roles, ubo.apply-fincen-control-prong
- ubo.verify-identity, ubo.screen-person, ubo.assess-risk, ubo.monitor-changes, ubo.refresh-data
- resources.plan, resources.provision, resources.configure, resources.test, resources.deploy
- attributes.define, attributes.resolve, values.bind, values.validate, values.encrypt
- workflow.transition, workflow.gate, tasks.create, tasks.assign, tasks.complete
- notify.send, communicate.request, escalate.trigger, audit.log
- external.query, external.sync, api.call, webhook.register

RULES:
1. ONLY use verbs from the approved vocabulary listed above
2. Analyze the current DSL structure and understand its semantic meaning
3. Apply the requested transformation while preserving DSL syntax and structure
4. Ensure all changes are consistent with the target onboarding state
5. Provide clear explanations for all changes made
6. Respond ONLY with a single, well-formed JSON object
7. Do not include markdown, code blocks, or conversational text

DSL SYNTAX GUIDE:
- S-expressions format: (command args...)
- Case creation: (case.create (cbu.id "ID") (nature-purpose "DESC"))
- Products: (products.add "PRODUCT1" "PRODUCT2")
- KYC: (kyc.start (documents (document "DOC")) (jurisdictions (jurisdiction "JUR")))
- Services: (services.discover (for.product "PROD" (service "SVC")))
- Resources: (resources.plan (resource.create "NAME" (owner "OWNER") (var (attr-id "ID"))))
- Values: (values.bind (bind (attr-id "ID") (value "VAL")))

RESPONSE FORMAT:
{
  "new_dsl": "Complete transformed DSL as a string",
  "explanation": "Clear explanation of what was changed and why",
  "changes": ["List of specific changes made"],
  "confidence": 0.95
}

EXAMPLES:
- Adding a product: Transform (products.add "CUSTODY") to (products.add "CUSTODY" "FUND_ACCOUNTING")
- Updating jurisdiction: Change (jurisdiction "US") to (jurisdiction "LU")
- Adding KYC document: Add (document "W8BEN-E") to existing documents list`

	// Format the user prompt with the transformation request
	userPrompt := fmt.Sprintf(`Current DSL:
%s

Instruction: %s
Target State: %s

Additional Context: %s

Please transform the DSL according to the instruction while moving toward the target state.`,
		request.CurrentDSL,
		request.Instruction,
		request.TargetState,
		jsonString(request.Context))

	a.model.SystemInstruction = &genai.Content{Parts: []genai.Part{genai.Text(systemPrompt)}}

	resp, err := a.model.GenerateContent(ctx, genai.Text(userPrompt))
	if err != nil {
		return nil, fmt.Errorf("failed to generate content: %w", err)
	}

	if len(resp.Candidates) == 0 || resp.Candidates[0] == nil || len(resp.Candidates[0].Content.Parts) == 0 {
		return nil, fmt.Errorf("no response from agent: %v", resp)
	}

	part := resp.Candidates[0].Content.Parts[0]
	textPart, ok := part.(genai.Text)
	if !ok {
		return nil, fmt.Errorf("unexpected response type from agent: %T", part)
	}

	log.Printf("DSL Agent Raw Response: %s", textPart)

	// Clean potential markdown-wrapped JSON using jsonv2's robust parsing
	cleanedJSON := cleanJSONResponse(string(textPart))

	var transformResp DSLTransformationResponse
	if uErr := json.Unmarshal([]byte(cleanedJSON), &transformResp); uErr != nil {
		return nil, fmt.Errorf("failed to parse agent's JSON response: %w (cleaned response was: %s)", uErr, cleanedJSON)
	}

	// Validate that only approved DSL verbs are used
	if validateErr := validateDSLVerbs(transformResp.NewDSL); validateErr != nil {
		return nil, fmt.Errorf("DSL validation failed: %w", validateErr)
	}

	return &transformResp, nil
}

// CallDSLValidationAgent validates DSL correctness and suggests improvements
func (a *Agent) CallDSLValidationAgent(ctx context.Context, dslToValidate string) (*DSLValidationResponse, error) {
	if a == nil || a.model == nil {
		return nil, fmt.Errorf("ai agent is not initialized")
	}

	systemPrompt := `You are an expert DSL validator for financial onboarding workflows.
Your role is to analyze DSL for correctness, completeness, and best practices.

VALIDATION CRITERIA:
1. Syntax correctness (proper S-expression structure)
2. Semantic correctness (logical flow and consistency)
3. Completeness (required elements for the onboarding state)
4. Best practices (proper naming, structure, etc.)
5. Compliance considerations (regulatory requirements)

RESPONSE FORMAT:
{
  "is_valid": true/false,
  "validation_score": 0.95,
  "errors": ["List of syntax or semantic errors"],
  "warnings": ["List of potential issues"],
  "suggestions": ["List of improvement suggestions"],
  "summary": "Overall assessment of the DSL"
}`

	userPrompt := fmt.Sprintf(`Please validate the following DSL:

%s

Provide a comprehensive validation assessment including errors, warnings, and suggestions for improvement.`, dslToValidate)

	a.model.SystemInstruction = &genai.Content{Parts: []genai.Part{genai.Text(systemPrompt)}}

	resp, err := a.model.GenerateContent(ctx, genai.Text(userPrompt))
	if err != nil {
		return nil, fmt.Errorf("failed to generate content: %w", err)
	}

	if len(resp.Candidates) == 0 || resp.Candidates[0] == nil || len(resp.Candidates[0].Content.Parts) == 0 {
		return nil, fmt.Errorf("no response from agent: %v", resp)
	}

	part := resp.Candidates[0].Content.Parts[0]
	textPart, ok := part.(genai.Text)
	if !ok {
		return nil, fmt.Errorf("unexpected response type from agent: %T", part)
	}

	log.Printf("DSL Validation Agent Raw Response: %s", textPart)

	// Clean potential markdown-wrapped JSON using jsonv2's robust parsing
	cleanedJSON := cleanJSONResponse(string(textPart))

	var validationResp DSLValidationResponse
	if uErr := json.Unmarshal([]byte(cleanedJSON), &validationResp); uErr != nil {
		return nil, fmt.Errorf("failed to parse agent's JSON response: %w (cleaned response was: %s)", uErr, cleanedJSON)
	}

	return &validationResp, nil
}

// DSLValidationResponse represents validation results
type DSLValidationResponse struct {
	IsValid         bool     `json:"is_valid"`
	ValidationScore float64  `json:"validation_score"`
	Errors          []string `json:"errors"`
	Warnings        []string `json:"warnings"`
	Suggestions     []string `json:"suggestions"`
	Summary         string   `json:"summary"`
}

// Helper function to safely convert context to JSON string
func jsonString(v interface{}) string {
	if v == nil {
		return "{}"
	}

	data, err := json.Marshal(v)
	if err != nil {
		return "{}"
	}

	return string(data)
}

// cleanJSONResponse removes markdown code block wrappers and cleans JSON
// Takes advantage of jsonv2's improved error handling and validation
func cleanJSONResponse(response string) string {
	// Trim whitespace
	cleaned := strings.TrimSpace(response)

	// Remove markdown JSON code blocks (```json ... ```)
	if strings.HasPrefix(cleaned, "```json") {
		// Find the first newline after ```json
		if firstNewline := strings.Index(cleaned, "\n"); firstNewline != -1 {
			cleaned = cleaned[firstNewline+1:]
		}
	}

	// Remove trailing ```
	cleaned = strings.TrimSuffix(cleaned, "```")

	// Remove any other markdown code block markers
	cleaned = strings.TrimPrefix(cleaned, "```")

	// Clean up any remaining whitespace
	cleaned = strings.TrimSpace(cleaned)

	// Validate that we have valid JSON using jsonv2's robust validation
	if err := json.Unmarshal([]byte(cleaned), new(interface{})); err == nil {
		return cleaned
	}

	// If still not valid, try to extract JSON from the response
	// Look for the first { and last } to extract potential JSON object
	firstBrace := strings.Index(cleaned, "{")
	lastBrace := strings.LastIndex(cleaned, "}")

	if firstBrace != -1 && lastBrace != -1 && lastBrace > firstBrace {
		extracted := cleaned[firstBrace : lastBrace+1]
		if err := json.Unmarshal([]byte(extracted), new(interface{})); err == nil {
			return extracted
		}
	}

	// Return original if we can't clean it
	return response
}

// validateDSLVerbs checks that the DSL only uses approved verbs from the vocabulary
func validateDSLVerbs(dsl string) error {
	// Approved DSL verbs based on vocab.go
	approvedVerbs := map[string]bool{
		// Case Management
		"case.create":   true,
		"case.update":   true,
		"case.validate": true,
		"case.approve":  true,
		"case.close":    true,
		// Entity Identity
		"entity.register": true,
		"entity.classify": true,
		"entity.link":     true,
		"identity.verify": true,
		"identity.attest": true,
		// Product Service
		"products.add":       true,
		"products.configure": true,
		"services.discover":  true,
		"services.provision": true,
		"services.activate":  true,
		// KYC Compliance
		"kyc.start":          true,
		"kyc.collect":        true,
		"kyc.verify":         true,
		"kyc.assess":         true,
		"compliance.screen":  true,
		"compliance.monitor": true,
		// UBO Ultimate Beneficial Ownership
		"ubo.collect-entity-data":          true,
		"ubo.get-ownership-structure":      true,
		"ubo.unroll-structure":             true,
		"ubo.resolve-ubos":                 true,
		"ubo.calculate-indirect-ownership": true,
		"ubo.identify-control-prong":       true,
		"ubo.apply-thresholds":             true,
		"ubo.verify-identity":              true,
		"ubo.screen-person":                true,
		"ubo.assess-risk":                  true,
		"ubo.monitor-changes":              true,
		"ubo.refresh-data":                 true,
		"ubo.trigger-review":               true,
		// Entity-Type-Specific UBO Workflows
		"ubo.identify-trust-parties":   true,
		"ubo.resolve-trust-ubos":       true,
		"ubo.identify-ownership-prong": true,
		"ubo.resolve-partnership-ubos": true,
		"ubo.recursive-entity-resolve": true,
		// FinCEN Control Prong Specific
		"ubo.identify-fincen-control-roles": true,
		"ubo.apply-fincen-control-prong":    true,
		// Resource Infrastructure
		"resources.plan":      true,
		"resources.provision": true,
		"resources.configure": true,
		"resources.test":      true,
		"resources.deploy":    true,
		// Attribute Data
		"attributes.define":  true,
		"attributes.resolve": true,
		"values.bind":        true,
		"values.validate":    true,
		"values.encrypt":     true,
		// Workflow State
		"workflow.transition": true,
		"workflow.gate":       true,
		"tasks.create":        true,
		"tasks.assign":        true,
		"tasks.complete":      true,
		// Notification Communication
		"notify.send":         true,
		"communicate.request": true,
		"escalate.trigger":    true,
		"audit.log":           true,
		// Integration External
		"external.query":   true,
		"external.sync":    true,
		"api.call":         true,
		"webhook.register": true,
		// Temporal Scheduling
		"schedule.create":   true,
		"deadline.set":      true,
		"reminder.schedule": true,
		// Risk Monitoring
		"risk.assess":   true,
		"monitor.setup": true,
		"alert.trigger": true,
		// Data Lifecycle
		"data.collect":   true,
		"data.transform": true,
		"data.archive":   true,
		"data.purge":     true,
	}

	// Extract all verbs from the DSL using regex
	// Only match verbs at the START of an s-expression: (verb ...
	// This avoids matching parameter names like (attr-id ...) or (nature-purpose ...)
	verbPattern := regexp.MustCompile(`\(([a-z]+\.[a-z][a-z-]*)\s`)
	matches := verbPattern.FindAllStringSubmatch(dsl, -1)

	var unapprovedVerbs []string
	seen := make(map[string]bool) // Track seen verbs to avoid duplicates

	for _, match := range matches {
		if len(match) <= 1 {
			continue
		}
		verb := match[1]

		// Skip if already processed
		if seen[verb] {
			continue
		}
		seen[verb] = true

		// Skip non-verb constructs (parameters, attributes, etc.)
		// These appear inside s-expressions but aren't verbs themselves
		if strings.HasSuffix(verb, ".id") ||
			verb == "for.product" ||
			verb == "resource.create" ||
			verb == "attr.id" ||
			verb == "bind" {
			continue
		}

		// Check if verb is approved
		if !approvedVerbs[verb] {
			unapprovedVerbs = append(unapprovedVerbs, verb)
		}
	}

	if len(unapprovedVerbs) > 0 {
		return fmt.Errorf("unapproved DSL verbs detected: %v (only approved vocabulary verbs are allowed)", unapprovedVerbs)
	}

	return nil
}
