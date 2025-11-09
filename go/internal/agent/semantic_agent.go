package agent

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"log"
	"regexp"
	"strings"

	"github.com/lib/pq"
)

// SemanticAgent provides AI-powered DSL construction using rich verb metadata
type SemanticAgent struct {
	db *sql.DB
}

// VerbContext represents comprehensive semantic context for a verb
type VerbContext struct {
	Domain                 string
	Verb                   string
	Category               string
	SyntaxDescription      string
	SemanticDescription    string
	IntentCategory         string
	BusinessPurpose        string
	SideEffects            pq.StringArray
	Prerequisites          pq.StringArray
	Postconditions         pq.StringArray
	AgentPrompt            string
	UsagePatterns          pq.StringArray
	SelectionCriteria      string
	ParameterSemantics     json.RawMessage
	WorkflowStage          string
	ComplianceImplications pq.StringArray
	ConfidenceScore        float64
	RelatedVerbs           pq.StringArray
	RequiredBy             pq.StringArray
	HistoricalUsageCount   int
	HistoricalSuccessRate  float64
	AvgAgentConfidence     float64
}

// WorkflowSequence represents available verbs for a workflow stage
type WorkflowSequence struct {
	WorkflowStage   string
	Domain          string
	AvailableVerbs  pq.StringArray
	EnablesVerbs    pq.StringArray
	RequiredByVerbs pq.StringArray
}

// VerbPattern represents a common usage pattern
type VerbPattern struct {
	PatternName         string
	PatternCategory     string
	PatternDescription  string
	PatternTemplate     string
	UseCases            pq.StringArray
	BusinessScenarios   pq.StringArray
	ComplexityLevel     string
	RequiredVerbs       pq.StringArray
	AgentSelectionRules string
}

// DecisionRule represents logic for verb selection and validation
type DecisionRule struct {
	RuleName            string
	RuleType            string
	ConditionExpression string
	ActionExpression    string
	ApplicableDomains   pq.StringArray
	BusinessContext     string
	LLMPromptAddition   string
	Priority            int
	Active              bool
}

// DSLContext represents the current state of DSL construction
type DSLContext struct {
	CurrentDSL           string
	CompletedVerbs       []string
	CurrentWorkflowStage string
	RequiredDomains      []string
	BusinessContext      map[string]interface{}
}

// NewSemanticAgent creates a new semantic-aware agent
func NewSemanticAgent(db *sql.DB) *SemanticAgent {
	return &SemanticAgent{db: db}
}

// GetVerbContext retrieves comprehensive context for a specific verb
func (a *SemanticAgent) GetVerbContext(ctx context.Context, domain, verb string) (*VerbContext, error) {
	query := `
		SELECT
			domain, verb, category,
			COALESCE(syntax_description, ''),
			COALESCE(semantic_description, ''),
			COALESCE(intent_category, ''),
			COALESCE(business_purpose, ''),
			COALESCE(side_effects, ARRAY[]::text[]),
			COALESCE(prerequisites, ARRAY[]::text[]),
			COALESCE(postconditions, ARRAY[]::text[]),
			COALESCE(agent_prompt, ''),
			COALESCE(usage_patterns, ARRAY[]::text[]),
			COALESCE(selection_criteria, ''),
			COALESCE(parameter_semantics, '{}'::jsonb),
			COALESCE(workflow_stage, ''),
			COALESCE(compliance_implications, ARRAY[]::text[]),
			COALESCE(confidence_score, 0.0),
			COALESCE(related_verbs, ARRAY[]::text[]),
			COALESCE(required_by, ARRAY[]::text[]),
			COALESCE(historical_usage_count, 0),
			COALESCE(historical_success_rate, 0.0),
			COALESCE(avg_agent_confidence, 0.0)
		FROM "ob-poc".v_agent_verb_context
		WHERE domain = $1 AND verb = $2`

	vc := &VerbContext{}
	err := a.db.QueryRowContext(ctx, query, domain, verb).Scan(
		&vc.Domain, &vc.Verb, &vc.Category,
		&vc.SyntaxDescription, &vc.SemanticDescription, &vc.IntentCategory,
		&vc.BusinessPurpose, &vc.SideEffects, &vc.Prerequisites, &vc.Postconditions,
		&vc.AgentPrompt, &vc.UsagePatterns, &vc.SelectionCriteria,
		&vc.ParameterSemantics, &vc.WorkflowStage, &vc.ComplianceImplications,
		&vc.ConfidenceScore, &vc.RelatedVerbs, &vc.RequiredBy,
		&vc.HistoricalUsageCount, &vc.HistoricalSuccessRate, &vc.AvgAgentConfidence,
	)

	if err != nil {
		if err == sql.ErrNoRows {
			return nil, fmt.Errorf("verb %s.%s not found in semantic registry", domain, verb)
		}
		return nil, fmt.Errorf("failed to retrieve verb context: %w", err)
	}

	return vc, nil
}

// GetAvailableVerbs returns verbs available for the current workflow stage
func (a *SemanticAgent) GetAvailableVerbs(ctx context.Context, workflowStage, domain string) ([]VerbContext, error) {
	query := `
		SELECT
			domain, verb, category,
			COALESCE(syntax_description, ''),
			COALESCE(semantic_description, ''),
			COALESCE(intent_category, ''),
			COALESCE(business_purpose, ''),
			COALESCE(agent_prompt, ''),
			COALESCE(selection_criteria, ''),
			COALESCE(workflow_stage, ''),
			COALESCE(confidence_score, 0.0)
		FROM "ob-poc".v_agent_verb_context
		WHERE (workflow_stage = $1 OR workflow_stage IS NULL)
			AND ($2 = '' OR domain = $2)
			AND confidence_score > 0.5
		ORDER BY confidence_score DESC, historical_success_rate DESC`

	rows, err := a.db.QueryContext(ctx, query, workflowStage, domain)
	if err != nil {
		return nil, fmt.Errorf("failed to query available verbs: %w", err)
	}
	defer rows.Close()

	var verbs []VerbContext
	for rows.Next() {
		var vc VerbContext
		err := rows.Scan(
			&vc.Domain, &vc.Verb, &vc.Category,
			&vc.SyntaxDescription, &vc.SemanticDescription, &vc.IntentCategory,
			&vc.BusinessPurpose, &vc.AgentPrompt, &vc.SelectionCriteria,
			&vc.WorkflowStage, &vc.ConfidenceScore,
		)
		if err != nil {
			log.Printf("Warning: failed to scan verb row: %v", err)
			continue
		}
		verbs = append(verbs, vc)
	}

	return verbs, nil
}

// GetUsagePatterns retrieves patterns relevant to the current context
func (a *SemanticAgent) GetUsagePatterns(ctx context.Context, category, complexityLevel string) ([]VerbPattern, error) {
	query := `
		SELECT
			pattern_name, pattern_category, pattern_description,
			pattern_template, use_cases, business_scenarios,
			complexity_level, required_verbs, agent_selection_rules
		FROM "ob-poc".verb_patterns
		WHERE ($1 = '' OR pattern_category = $1)
			AND ($2 = '' OR complexity_level = $2)
		ORDER BY success_rate DESC, usage_frequency DESC
		LIMIT 10`

	rows, err := a.db.QueryContext(ctx, query, category, complexityLevel)
	if err != nil {
		return nil, fmt.Errorf("failed to query usage patterns: %w", err)
	}
	defer rows.Close()

	var patterns []VerbPattern
	for rows.Next() {
		var vp VerbPattern
		err := rows.Scan(
			&vp.PatternName, &vp.PatternCategory, &vp.PatternDescription,
			&vp.PatternTemplate, &vp.UseCases, &vp.BusinessScenarios,
			&vp.ComplexityLevel, &vp.RequiredVerbs, &vp.AgentSelectionRules,
		)
		if err != nil {
			log.Printf("Warning: failed to scan pattern row: %v", err)
			continue
		}
		patterns = append(patterns, vp)
	}

	return patterns, nil
}

// GetDecisionRules retrieves rules for verb selection and validation
func (a *SemanticAgent) GetDecisionRules(ctx context.Context, ruleType string, domains []string) ([]DecisionRule, error) {
	query := `
		SELECT
			rule_name, rule_type, condition_expression, action_expression,
			applicable_domains, business_context, llm_prompt_addition,
			priority_weight, active
		FROM "ob-poc".verb_decision_rules
		WHERE ($1 = '' OR rule_type = $1)
			AND ($2::text[] IS NULL OR applicable_domains && $2::text[])
			AND active = true
		ORDER BY priority_weight DESC`

	rows, err := a.db.QueryContext(ctx, query, ruleType, pq.Array(domains))
	if err != nil {
		return nil, fmt.Errorf("failed to query decision rules: %w", err)
	}
	defer rows.Close()

	var rules []DecisionRule
	for rows.Next() {
		var dr DecisionRule
		err := rows.Scan(
			&dr.RuleName, &dr.RuleType, &dr.ConditionExpression, &dr.ActionExpression,
			&dr.ApplicableDomains, &dr.BusinessContext, &dr.LLMPromptAddition,
			&dr.Priority, &dr.Active,
		)
		if err != nil {
			log.Printf("Warning: failed to scan decision rule row: %v", err)
			continue
		}
		rules = append(rules, dr)
	}

	return rules, nil
}

// SuggestNextVerbs provides intelligent verb suggestions based on current DSL context
func (a *SemanticAgent) SuggestNextVerbs(ctx context.Context, dslContext *DSLContext) ([]VerbContext, error) {
	// Analyze current DSL to determine completed verbs and workflow stage
	completedVerbs := a.analyzeCompletedVerbs(dslContext.CurrentDSL)
	workflowStage := a.determineWorkflowStage(completedVerbs)

	// Get verbs that are enabled by completed verbs
	enabledVerbs, err := a.getEnabledVerbs(ctx, completedVerbs)
	if err != nil {
		return nil, fmt.Errorf("failed to get enabled verbs: %w", err)
	}

	// Apply decision rules to filter suggestions
	filteredVerbs, err := a.applyDecisionRules(ctx, enabledVerbs, dslContext)
	if err != nil {
		log.Printf("Warning: failed to apply decision rules: %v", err)
		// Continue without filtering if rules fail
		filteredVerbs = enabledVerbs
	}

	// Rank by confidence and relevance
	rankedVerbs := a.rankVerbSuggestions(filteredVerbs, dslContext)

	return rankedVerbs[:min(len(rankedVerbs), 5)], nil
}

// ValidateDSLSemantics performs semantic validation beyond syntax checking
func (a *SemanticAgent) ValidateDSLSemantics(ctx context.Context, dsl string) (*DSLValidationResponse, error) {
	var errors []string
	var warnings []string
	var suggestions []string

	// Extract verbs from DSL
	verbs := a.extractVerbs(dsl)

	// Validate each verb's semantic requirements
	for _, verbRef := range verbs {
		vc, err := a.GetVerbContext(ctx, verbRef.Domain, verbRef.Verb)
		if err != nil {
			errors = append(errors, fmt.Sprintf("Unknown verb: %s.%s", verbRef.Domain, verbRef.Verb))
			continue
		}

		// Check prerequisites
		if !a.checkPrerequisites(vc, verbs) {
			errors = append(errors, fmt.Sprintf("Prerequisites not met for %s.%s: %v",
				verbRef.Domain, verbRef.Verb, vc.Prerequisites))
		}

		// Check compliance implications
		if len(vc.ComplianceImplications) > 0 {
			warnings = append(warnings, fmt.Sprintf("Compliance considerations for %s.%s: %v",
				verbRef.Domain, verbRef.Verb, vc.ComplianceImplications))
		}
	}

	// Check workflow sequence validity
	sequenceErrors := a.validateWorkflowSequence(ctx, verbs)
	errors = append(errors, sequenceErrors...)

	// Generate suggestions based on patterns
	patternSuggestions, err := a.generatePatternSuggestions(ctx, verbs)
	if err == nil {
		suggestions = append(suggestions, patternSuggestions...)
	}

	// Calculate overall score
	score := 1.0 - (float64(len(errors))*0.3 + float64(len(warnings))*0.1)
	if score < 0 {
		score = 0
	}

	return &DSLValidationResponse{
		IsValid:     len(errors) == 0,
		Score:       score,
		Errors:      errors,
		Warnings:    warnings,
		Suggestions: suggestions,
	}, nil
}

// GenerateSemanticPrompt creates rich prompts for LLM interactions
func (a *SemanticAgent) GenerateSemanticPrompt(ctx context.Context, intent string, context map[string]interface{}) (string, error) {
	// Get relevant verbs for this intent
	verbs, err := a.GetAvailableVerbs(ctx, "", "")
	if err != nil {
		return "", fmt.Errorf("failed to get available verbs: %w", err)
	}

	// Get relevant patterns
	patterns, err := a.GetUsagePatterns(ctx, "", "beginner")
	if err != nil {
		return "", fmt.Errorf("failed to get usage patterns: %w", err)
	}

	// Build comprehensive prompt
	var promptBuilder strings.Builder

	promptBuilder.WriteString("# DSL Construction Context\n\n")
	promptBuilder.WriteString(fmt.Sprintf("**Intent**: %s\n\n", intent))

	// Add verb context
	promptBuilder.WriteString("## Available Verbs\n\n")
	for _, verb := range verbs {
		if verb.ConfidenceScore > 0.7 { // Only include high-confidence verbs
			promptBuilder.WriteString(fmt.Sprintf("### %s.%s\n", verb.Domain, verb.Verb))
			promptBuilder.WriteString(fmt.Sprintf("**Purpose**: %s\n\n", verb.BusinessPurpose))
			promptBuilder.WriteString(fmt.Sprintf("**When to use**: %s\n\n", verb.SelectionCriteria))

			if len(verb.Prerequisites) > 0 {
				promptBuilder.WriteString(fmt.Sprintf("**Prerequisites**: %s\n\n",
					strings.Join(verb.Prerequisites, ", ")))
			}
		}
	}

	// Add pattern examples
	promptBuilder.WriteString("## Common Patterns\n\n")
	for _, pattern := range patterns {
		if pattern.ComplexityLevel != "advanced" { // Focus on simpler patterns
			promptBuilder.WriteString(fmt.Sprintf("### %s\n", pattern.PatternName))
			promptBuilder.WriteString(fmt.Sprintf("**Use for**: %s\n\n", pattern.PatternDescription))
			promptBuilder.WriteString(fmt.Sprintf("**Template**:\n```\n%s\n```\n\n", pattern.PatternTemplate))
		}
	}

	// Add decision rules as guidance
	rules, err := a.GetDecisionRules(ctx, "", []string{"onboarding", "kyc", "ubo"})
	if err == nil {
		promptBuilder.WriteString("## Key Rules\n\n")
		for _, rule := range rules {
			if rule.LLMPromptAddition != "" {
				promptBuilder.WriteString(fmt.Sprintf("- %s\n", rule.LLMPromptAddition))
			}
		}
	}

	return promptBuilder.String(), nil
}

// RecordAgentUsage tracks agent verb usage for learning and improvement
func (a *SemanticAgent) RecordAgentUsage(ctx context.Context, sessionID, agentType, domain, verb string,
	contextPrompt, selectionReasoning string, confidence float64, success bool, userFeedback string) error {

	query := `
		INSERT INTO "ob-poc".agent_verb_usage (
			session_id, agent_type, domain, verb, context_prompt,
			selection_reasoning, confidence_reported, execution_success, user_feedback
		) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)`

	_, err := a.db.ExecContext(ctx, query, sessionID, agentType, domain, verb,
		contextPrompt, selectionReasoning, confidence, success, userFeedback)

	return err
}

// Helper methods

func (a *SemanticAgent) analyzeCompletedVerbs(dsl string) []string {
	// Simple regex-based extraction - could be enhanced with proper parsing
	verbRegex := regexp.MustCompile(`\(([a-z\-]+\.[a-z\-]+)`)
	matches := verbRegex.FindAllStringSubmatch(dsl, -1)

	var verbs []string
	for _, match := range matches {
		if len(match) > 1 {
			verbs = append(verbs, match[1])
		}
	}
	return verbs
}

func (a *SemanticAgent) determineWorkflowStage(completedVerbs []string) string {
	// Simple heuristic - could be enhanced with more sophisticated analysis
	verbSet := make(map[string]bool)
	for _, verb := range completedVerbs {
		verbSet[verb] = true
	}

	if verbSet["case.create"] && !verbSet["products.add"] {
		return "initialization"
	} else if verbSet["products.add"] && !verbSet["services.discover"] {
		return "configuration"
	} else if verbSet["kyc.start"] || verbSet["ubo.collect-entity-data"] {
		return "compliance"
	} else if verbSet["services.discover"] {
		return "service_planning"
	}

	return "unknown"
}

func (a *SemanticAgent) getEnabledVerbs(ctx context.Context, completedVerbs []string) ([]VerbContext, error) {
	if len(completedVerbs) == 0 {
		// Return initialization verbs
		return a.GetAvailableVerbs(ctx, "initialization", "")
	}

	// Get verbs enabled by the most recent completed verb
	lastVerb := completedVerbs[len(completedVerbs)-1]
	parts := strings.Split(lastVerb, ".")
	if len(parts) != 2 {
		return nil, fmt.Errorf("invalid verb format: %s", lastVerb)
	}

	// This would query the relationship table to find enabled verbs
	query := `
		SELECT DISTINCT vc.domain, vc.verb, vc.semantic_description,
			   vc.business_purpose, vc.agent_prompt, vc.confidence_score
		FROM "ob-poc".v_agent_verb_context vc
		JOIN "ob-poc".verb_relationships vr ON vc.domain = vr.target_domain AND vc.verb = vr.target_verb
		WHERE vr.source_domain = $1 AND vr.source_verb = $2
			  AND vr.relationship_type IN ('enables', 'suggests')
		ORDER BY vr.relationship_strength DESC, vc.confidence_score DESC`

	rows, err := a.db.QueryContext(ctx, query, parts[0], parts[1])
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var verbs []VerbContext
	for rows.Next() {
		var vc VerbContext
		err := rows.Scan(&vc.Domain, &vc.Verb, &vc.SemanticDescription,
			&vc.BusinessPurpose, &vc.AgentPrompt, &vc.ConfidenceScore)
		if err != nil {
			continue
		}
		verbs = append(verbs, vc)
	}

	return verbs, nil
}

func (a *SemanticAgent) applyDecisionRules(ctx context.Context, verbs []VerbContext, dslContext *DSLContext) ([]VerbContext, error) {
	// Simple implementation - could be enhanced with rule engine
	var filtered []VerbContext

	for _, verb := range verbs {
		// Apply basic filtering rules
		if verb.ConfidenceScore > 0.6 {
			filtered = append(filtered, verb)
		}
	}

	return filtered, nil
}

func (a *SemanticAgent) rankVerbSuggestions(verbs []VerbContext, context *DSLContext) []VerbContext {
	// Simple ranking by confidence score and historical success rate
	// Could be enhanced with ML-based ranking
	for i := range verbs {
		for j := i + 1; j < len(verbs); j++ {
			score_i := verbs[i].ConfidenceScore * (1.0 + verbs[i].HistoricalSuccessRate)
			score_j := verbs[j].ConfidenceScore * (1.0 + verbs[j].HistoricalSuccessRate)
			if score_j > score_i {
				verbs[i], verbs[j] = verbs[j], verbs[i]
			}
		}
	}
	return verbs
}

type VerbReference struct {
	Domain string
	Verb   string
}

func (a *SemanticAgent) extractVerbs(dsl string) []VerbReference {
	verbRegex := regexp.MustCompile(`\(([a-z\-]+)\.([a-z\-]+)`)
	matches := verbRegex.FindAllStringSubmatch(dsl, -1)

	var verbs []VerbReference
	for _, match := range matches {
		if len(match) > 2 {
			verbs = append(verbs, VerbReference{
				Domain: match[1],
				Verb:   match[2],
			})
		}
	}
	return verbs
}

func (a *SemanticAgent) checkPrerequisites(vc *VerbContext, completedVerbs []VerbReference) bool {
	if len(vc.Prerequisites) == 0 {
		return true
	}

	// Simple check - could be enhanced with more sophisticated logic
	completedSet := make(map[string]bool)
	for _, verb := range completedVerbs {
		completedSet[verb.Domain+"."+verb.Verb] = true
	}

	for _, prereq := range vc.Prerequisites {
		if !completedSet[prereq] {
			return false
		}
	}

	return true
}

func (a *SemanticAgent) validateWorkflowSequence(ctx context.Context, verbs []VerbReference) []string {
	var errors []string
	// Implementation would check verb relationships and sequencing rules
	return errors
}

func (a *SemanticAgent) generatePatternSuggestions(ctx context.Context, verbs []VerbReference) ([]string, error) {
	var suggestions []string
	// Implementation would suggest patterns based on current verb usage
	return suggestions, nil
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}
