package cli

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"strconv"
	"strings"
	"time"

	"dsl-ob-poc/internal/datastore"
	"dsl-ob-poc/internal/store"
)

// RunOptimize executes the Phase 6 compile-time optimization command
func RunOptimize(ctx context.Context, dataStore datastore.DataStore, args []string) error {
	var cbu string
	var output string
	var format string = "json"
	var verbose bool = false
	var skipValidation bool = false
	var strategy string = "BALANCED"
	var maxCost float64 = 1000.0
	var sessionID string
	var generatePlan bool = true
	var saveResults bool = false

	// Parse command line arguments
	for i := 0; i < len(args); i++ {
		arg := args[i]

		if strings.HasPrefix(arg, "--cbu=") {
			cbu = strings.TrimPrefix(arg, "--cbu=")
		} else if arg == "--cbu" && i+1 < len(args) {
			cbu = args[i+1]
			i++
		} else if strings.HasPrefix(arg, "--output=") {
			output = strings.TrimPrefix(arg, "--output=")
		} else if arg == "--output" && i+1 < len(args) {
			output = args[i+1]
			i++
		} else if strings.HasPrefix(arg, "--format=") {
			format = strings.TrimPrefix(arg, "--format=")
		} else if arg == "--format" && i+1 < len(args) {
			format = args[i+1]
			i++
		} else if arg == "--verbose" || arg == "-v" {
			verbose = true
		} else if arg == "--skip-validation" {
			skipValidation = true
		} else if strings.HasPrefix(arg, "--strategy=") {
			strategy = strings.TrimPrefix(arg, "--strategy=")
		} else if arg == "--strategy" && i+1 < len(args) {
			strategy = args[i+1]
			i++
		} else if strings.HasPrefix(arg, "--max-cost=") {
			costStr := strings.TrimPrefix(arg, "--max-cost=")
			if cost, err := strconv.ParseFloat(costStr, 64); err == nil {
				maxCost = cost
			}
		} else if arg == "--max-cost" && i+1 < len(args) {
			if cost, err := strconv.ParseFloat(args[i+1], 64); err == nil {
				maxCost = cost
			}
			i++
		} else if strings.HasPrefix(arg, "--session-id=") {
			sessionID = strings.TrimPrefix(arg, "--session-id=")
		} else if arg == "--session-id" && i+1 < len(args) {
			sessionID = args[i+1]
			i++
		} else if arg == "--no-plan" {
			generatePlan = false
		} else if arg == "--save-results" {
			saveResults = true
		}
	}

	// Validate required parameters
	if cbu == "" {
		return fmt.Errorf("CBU ID is required (use --cbu=<cbu-id>)")
	}

	// Validate format
	switch format {
	case "json", "yaml", "text":
		// Valid formats
	default:
		return fmt.Errorf("invalid format: %s (must be json, yaml, or text)", format)
	}

	// Validate strategy
	switch strategy {
	case "COST_OPTIMIZED", "PERFORMANCE_OPTIMIZED", "BALANCED", "AVAILABILITY_FIRST":
		// Valid strategies
	default:
		return fmt.Errorf("invalid strategy: %s", strategy)
	}

	if verbose {
		fmt.Printf("🔧 Starting Phase 6: Compile-Time Optimization & Execution Planning\n")
		fmt.Printf("CBU ID: %s\n", cbu)
		fmt.Printf("Strategy: %s\n", strategy)
		fmt.Printf("Max Cost: $%.2f/hour\n", maxCost)
		fmt.Printf("Format: %s\n", format)
		if output != "" {
			fmt.Printf("Output File: %s\n", output)
		}
	}

	// Phase 1: Retrieve DSL document
	if verbose {
		fmt.Printf("📖 Retrieving DSL document for CBU %s...\n", cbu)
	}

	dslRecord, err := dataStore.GetLatestDSLWithState(ctx, cbu)
	if err != nil {
		return fmt.Errorf("error retrieving DSL for CBU %s: %w", cbu, err)
	}

	if dslRecord == nil {
		return fmt.Errorf("no DSL found for CBU %s", cbu)
	}

	if verbose {
		fmt.Printf("✅ Retrieved DSL document (Version: %d, State: %s)\n",
			dslRecord.VersionNumber, dslRecord.OnboardingState)
		fmt.Printf("DSL Length: %d characters\n", len(dslRecord.DSLText))
	}

	// Phase 2: Perform basic DSL analysis
	if verbose {
		fmt.Printf("🔍 Analyzing DSL structure...\n")
	}

	analysisResults := analyzeDSLDocument(dslRecord.DSLText)

	if verbose {
		fmt.Printf("   Operations found: %d\n", analysisResults.OperationCount)
		fmt.Printf("   Domains detected: %d (%s)\n",
			len(analysisResults.Domains), strings.Join(analysisResults.Domains, ", "))
		fmt.Printf("   Resource operations: %d\n", analysisResults.ResourceOperations)
		fmt.Printf("   Attribute references: %d\n", analysisResults.AttributeReferences)
	}

	// Phase 3: Generate optimization report
	optimizationReport := generateOptimizationReport(dslRecord, analysisResults, OptimizationConfig{
		Strategy:       strategy,
		MaxCost:        maxCost,
		SessionID:      sessionID,
		GeneratePlan:   generatePlan,
		SkipValidation: skipValidation,
	})

	// Phase 4: Output results
	if err := outputOptimizationResults(optimizationReport, output, format, verbose); err != nil {
		return fmt.Errorf("error outputting results: %w", err)
	}

	// Phase 5: Save results (if requested)
	if saveResults {
		if verbose {
			fmt.Printf("💾 Saving optimization results to database...\n")
		}
		if err := saveOptimizationResultsToDatabase(ctx, dataStore, optimizationReport); err != nil {
			return fmt.Errorf("error saving results: %w", err)
		}
	}

	if verbose {
		fmt.Printf("✅ Phase 6 optimization completed successfully!\n")
		if optimizationReport.Improvements.CostReduction > 0 {
			fmt.Printf("💰 Potential cost savings: $%.2f\n", optimizationReport.Improvements.CostReduction)
		}
		if optimizationReport.Improvements.TimeReduction > 0 {
			fmt.Printf("⏱️  Potential time savings: %dms\n", optimizationReport.Improvements.TimeReduction)
		}
	}

	return nil
}

// DSLAnalysisResult represents the results of DSL document analysis
type DSLAnalysisResult struct {
	OperationCount      int      `json:"operation_count"`
	Domains             []string `json:"domains"`
	ResourceOperations  int      `json:"resource_operations"`
	AttributeReferences int      `json:"attribute_references"`
	Verbs               []string `json:"verbs"`
	Dependencies        []string `json:"dependencies"`
	ComplexityScore     int      `json:"complexity_score"`
}

// OptimizationConfig holds configuration for optimization
type OptimizationConfig struct {
	Strategy       string  `json:"strategy"`
	MaxCost        float64 `json:"max_cost"`
	SessionID      string  `json:"session_id"`
	GeneratePlan   bool    `json:"generate_plan"`
	SkipValidation bool    `json:"skip_validation"`
}

// OptimizationReport represents the comprehensive optimization results
type OptimizationReport struct {
	CBU             string                       `json:"cbu"`
	Timestamp       string                       `json:"timestamp"`
	Configuration   OptimizationConfig           `json:"configuration"`
	Analysis        DSLAnalysisResult            `json:"analysis"`
	Improvements    OptimizationImprovements     `json:"improvements"`
	Recommendations []OptimizationRecommendation `json:"recommendations"`
	ExecutionPlan   *SimpleExecutionPlan         `json:"execution_plan,omitempty"`
	Summary         OptimizationSummary          `json:"summary"`
}

// OptimizationImprovements represents potential improvements
type OptimizationImprovements struct {
	CostReduction       float64 `json:"cost_reduction"`
	TimeReduction       int     `json:"time_reduction_ms"`
	ComplexityReduction int     `json:"complexity_reduction"`
	ParallelizationOps  int     `json:"parallelization_opportunities"`
}

// OptimizationRecommendation represents a recommendation for optimization
type OptimizationRecommendation struct {
	Category    string `json:"category"`
	Priority    string `json:"priority"`
	Title       string `json:"title"`
	Description string `json:"description"`
	Impact      string `json:"impact"`
	Effort      string `json:"effort"`
}

// SimpleExecutionPlan represents a simplified execution plan
type SimpleExecutionPlan struct {
	TotalPhases        int      `json:"total_phases"`
	ParallelOperations int      `json:"parallel_operations"`
	CriticalPath       []string `json:"critical_path"`
	EstimatedDuration  int      `json:"estimated_duration_ms"`
}

// OptimizationSummary provides a high-level summary
type OptimizationSummary struct {
	OverallScore      int      `json:"overall_score"`
	OptimizationLevel string   `json:"optimization_level"`
	KeyBenefits       []string `json:"key_benefits"`
	NextSteps         []string `json:"next_steps"`
}

// analyzeDSLDocument performs analysis on the DSL document
func analyzeDSLDocument(dslBody string) DSLAnalysisResult {
	lines := strings.Split(dslBody, "\n")

	result := DSLAnalysisResult{
		Domains:      make([]string, 0),
		Verbs:        make([]string, 0),
		Dependencies: make([]string, 0),
	}

	domainMap := make(map[string]bool)
	verbMap := make(map[string]bool)

	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, ";") {
			continue
		}

		// Count operations (lines with verbs)
		if strings.Contains(line, "(") && strings.Contains(line, ".") {
			result.OperationCount++

			// Extract verb
			if verb := extractVerbFromLine(line); verb != "" {
				if !verbMap[verb] {
					result.Verbs = append(result.Verbs, verb)
					verbMap[verb] = true
				}

				// Extract domain
				if domain := extractDomainFromVerb(verb); domain != "" {
					if !domainMap[domain] {
						result.Domains = append(result.Domains, domain)
						domainMap[domain] = true
					}
				}

				// Count resource operations
				if strings.Contains(verb, "resources.") {
					result.ResourceOperations++
				}
			}
		}

		// Count attribute references
		result.AttributeReferences += strings.Count(line, "@attr{")
	}

	// Calculate complexity score
	result.ComplexityScore = result.OperationCount*10 +
		len(result.Domains)*5 +
		result.ResourceOperations*15 +
		result.AttributeReferences*2

	return result
}

// generateOptimizationReport creates a comprehensive optimization report
func generateOptimizationReport(dslRecord *store.DSLVersionWithState, analysis DSLAnalysisResult, config OptimizationConfig) *OptimizationReport {
	report := &OptimizationReport{
		CBU:             dslRecord.CBUID,
		Timestamp:       time.Now().Format("2006-01-02T15:04:05Z07:00"),
		Configuration:   config,
		Analysis:        analysis,
		Improvements:    calculateImprovements(analysis, config),
		Recommendations: generateRecommendations(analysis, config),
	}

	if config.GeneratePlan {
		report.ExecutionPlan = generateSimpleExecutionPlan(analysis)
	}

	report.Summary = generateOptimizationSummary(analysis, report.Improvements, len(report.Recommendations))

	return report
}

// calculateImprovements estimates potential improvements
func calculateImprovements(analysis DSLAnalysisResult, config OptimizationConfig) OptimizationImprovements {
	improvements := OptimizationImprovements{}

	// Estimate cost reduction based on optimization strategy
	switch config.Strategy {
	case "COST_OPTIMIZED":
		improvements.CostReduction = float64(analysis.ResourceOperations) * 25.0 // $25 per resource
	case "PERFORMANCE_OPTIMIZED":
		improvements.CostReduction = float64(analysis.ResourceOperations) * 10.0 // $10 per resource
	case "BALANCED":
		improvements.CostReduction = float64(analysis.ResourceOperations) * 15.0 // $15 per resource
	default:
		improvements.CostReduction = float64(analysis.ResourceOperations) * 12.0 // $12 per resource
	}

	// Estimate time reduction
	improvements.TimeReduction = analysis.OperationCount * 500 // 500ms per operation

	// Estimate complexity reduction
	improvements.ComplexityReduction = analysis.ComplexityScore / 4

	// Estimate parallelization opportunities
	if len(analysis.Domains) > 1 {
		improvements.ParallelizationOps = analysis.OperationCount / 3
	}

	return improvements
}

// generateRecommendations creates optimization recommendations
func generateRecommendations(analysis DSLAnalysisResult, config OptimizationConfig) []OptimizationRecommendation {
	recommendations := make([]OptimizationRecommendation, 0)

	// Resource optimization recommendation
	if analysis.ResourceOperations > 3 {
		recommendations = append(recommendations, OptimizationRecommendation{
			Category:    "Resource Management",
			Priority:    "HIGH",
			Title:       "Optimize Resource Creation Order",
			Description: "Consider batching resource creation operations to reduce overhead and improve efficiency.",
			Impact:      "15-25% cost reduction",
			Effort:      "MEDIUM",
		})
	}

	// Multi-domain parallelization
	if len(analysis.Domains) > 2 {
		recommendations = append(recommendations, OptimizationRecommendation{
			Category:    "Execution Strategy",
			Priority:    "MEDIUM",
			Title:       "Enable Cross-Domain Parallelization",
			Description: "Operations across different domains can potentially run in parallel.",
			Impact:      "30-50% time reduction",
			Effort:      "LOW",
		})
	}

	// Complexity reduction
	if analysis.ComplexityScore > 100 {
		recommendations = append(recommendations, OptimizationRecommendation{
			Category:    "DSL Structure",
			Priority:    "MEDIUM",
			Title:       "Simplify DSL Structure",
			Description: "Consider breaking complex operations into smaller, more manageable chunks.",
			Impact:      "Improved maintainability",
			Effort:      "HIGH",
		})
	}

	// Strategy-specific recommendations
	switch config.Strategy {
	case "COST_OPTIMIZED":
		recommendations = append(recommendations, OptimizationRecommendation{
			Category:    "Cost Management",
			Priority:    "HIGH",
			Title:       "Implement Resource Sharing",
			Description: "Share resources across similar operations to reduce provisioning costs.",
			Impact:      "20-35% cost reduction",
			Effort:      "MEDIUM",
		})
	case "PERFORMANCE_OPTIMIZED":
		recommendations = append(recommendations, OptimizationRecommendation{
			Category:    "Performance",
			Priority:    "HIGH",
			Title:       "Pre-allocate Critical Resources",
			Description: "Pre-allocate resources for critical operations to eliminate setup time.",
			Impact:      "40-60% time reduction",
			Effort:      "LOW",
		})
	}

	return recommendations
}

// generateSimpleExecutionPlan creates a simplified execution plan
func generateSimpleExecutionPlan(analysis DSLAnalysisResult) *SimpleExecutionPlan {
	// Simplified calculation based on analysis
	phases := (analysis.OperationCount + 2) / 3 // Group operations into phases
	if phases < 1 {
		phases = 1
	}

	parallelOps := 0
	if len(analysis.Domains) > 1 {
		parallelOps = analysis.OperationCount / 2 // Assume half can be parallelized
	}

	criticalPath := make([]string, 0)
	if analysis.ResourceOperations > 0 {
		criticalPath = append(criticalPath, "resource-creation")
	}
	if len(analysis.Verbs) > 0 {
		criticalPath = append(criticalPath, "verification")
	}

	return &SimpleExecutionPlan{
		TotalPhases:        phases,
		ParallelOperations: parallelOps,
		CriticalPath:       criticalPath,
		EstimatedDuration:  analysis.OperationCount * 2000, // 2 seconds per operation
	}
}

// generateOptimizationSummary creates a high-level summary
func generateOptimizationSummary(analysis DSLAnalysisResult, improvements OptimizationImprovements, recommendationCount int) OptimizationSummary {
	score := calculateOverallScore(analysis, improvements)
	level := determineOptimizationLevel(score)

	benefits := make([]string, 0)
	if improvements.CostReduction > 0 {
		benefits = append(benefits, fmt.Sprintf("$%.0f potential cost savings", improvements.CostReduction))
	}
	if improvements.TimeReduction > 0 {
		benefits = append(benefits, fmt.Sprintf("%dms potential time savings", improvements.TimeReduction))
	}
	if improvements.ParallelizationOps > 0 {
		benefits = append(benefits, fmt.Sprintf("%d operations can be parallelized", improvements.ParallelizationOps))
	}

	nextSteps := []string{
		"Review and implement high-priority recommendations",
		"Consider enabling parallel execution for multi-domain operations",
		"Monitor resource utilization during execution",
	}

	return OptimizationSummary{
		OverallScore:      score,
		OptimizationLevel: level,
		KeyBenefits:       benefits,
		NextSteps:         nextSteps,
	}
}

// Helper functions

func extractVerbFromLine(line string) string {
	// Simple extraction: find text between ( and first space
	if start := strings.Index(line, "("); start >= 0 {
		remaining := line[start+1:]
		if end := strings.Index(remaining, " "); end >= 0 {
			return remaining[:end]
		} else if end := strings.Index(remaining, ")"); end >= 0 {
			return remaining[:end]
		}
	}
	return ""
}

func extractDomainFromVerb(verb string) string {
	if dot := strings.Index(verb, "."); dot >= 0 {
		return verb[:dot]
	}
	return ""
}

func calculateOverallScore(analysis DSLAnalysisResult, improvements OptimizationImprovements) int {
	score := 50 // Base score

	// Add points for potential improvements
	if improvements.CostReduction > 0 {
		score += int(improvements.CostReduction / 10)
	}
	if improvements.TimeReduction > 0 {
		score += improvements.TimeReduction / 1000
	}
	if improvements.ParallelizationOps > 0 {
		score += improvements.ParallelizationOps * 5
	}

	// Cap score at 100
	if score > 100 {
		score = 100
	}

	return score
}

func determineOptimizationLevel(score int) string {
	switch {
	case score >= 85:
		return "EXCELLENT"
	case score >= 70:
		return "GOOD"
	case score >= 55:
		return "MODERATE"
	default:
		return "LIMITED"
	}
}

// outputOptimizationResults outputs the optimization results in the specified format
func outputOptimizationResults(report *OptimizationReport, outputFile, format string, verbose bool) error {
	var output []byte
	var err error

	switch format {
	case "json":
		if verbose {
			output, err = jsonMarshalIndent(report, "", "  ")
		} else {
			output, err = jsonMarshal(report)
		}
	case "yaml":
		return fmt.Errorf("YAML output not yet implemented")
	case "text":
		output = []byte(formatTextReport(report))
	default:
		return fmt.Errorf("unsupported format: %s", format)
	}

	if err != nil {
		return fmt.Errorf("failed to format output: %w", err)
	}

	if outputFile != "" {
		if err := os.WriteFile(outputFile, output, 0644); err != nil {
			return fmt.Errorf("failed to write output file: %w", err)
		}
		if verbose {
			fmt.Printf("📁 Results saved to: %s\n", outputFile)
		}
	} else {
		fmt.Printf("\n%s\n", string(output))
	}

	return nil
}

// formatTextReport formats the report as human-readable text
func formatTextReport(report *OptimizationReport) string {
	var sb strings.Builder

	sb.WriteString("DSL OPTIMIZATION REPORT\n")
	sb.WriteString("=======================\n\n")

	sb.WriteString(fmt.Sprintf("CBU: %s\n", report.CBU))
	sb.WriteString(fmt.Sprintf("Timestamp: %s\n", report.Timestamp))
	sb.WriteString(fmt.Sprintf("Strategy: %s\n\n", report.Configuration.Strategy))

	sb.WriteString("ANALYSIS RESULTS:\n")
	sb.WriteString(fmt.Sprintf("  Operations: %d\n", report.Analysis.OperationCount))
	sb.WriteString(fmt.Sprintf("  Domains: %d (%s)\n", len(report.Analysis.Domains), strings.Join(report.Analysis.Domains, ", ")))
	sb.WriteString(fmt.Sprintf("  Resource Operations: %d\n", report.Analysis.ResourceOperations))
	sb.WriteString(fmt.Sprintf("  Complexity Score: %d\n\n", report.Analysis.ComplexityScore))

	sb.WriteString("POTENTIAL IMPROVEMENTS:\n")
	if report.Improvements.CostReduction > 0 {
		sb.WriteString(fmt.Sprintf("  Cost Reduction: $%.2f\n", report.Improvements.CostReduction))
	}
	if report.Improvements.TimeReduction > 0 {
		sb.WriteString(fmt.Sprintf("  Time Reduction: %dms\n", report.Improvements.TimeReduction))
	}
	if report.Improvements.ParallelizationOps > 0 {
		sb.WriteString(fmt.Sprintf("  Parallelizable Operations: %d\n", report.Improvements.ParallelizationOps))
	}
	sb.WriteString("\n")

	if len(report.Recommendations) > 0 {
		sb.WriteString("RECOMMENDATIONS:\n")
		for i, rec := range report.Recommendations {
			sb.WriteString(fmt.Sprintf("  %d. [%s] %s\n", i+1, rec.Priority, rec.Title))
			sb.WriteString(fmt.Sprintf("     %s\n", rec.Description))
			sb.WriteString(fmt.Sprintf("     Impact: %s, Effort: %s\n\n", rec.Impact, rec.Effort))
		}
	}

	sb.WriteString("SUMMARY:\n")
	sb.WriteString(fmt.Sprintf("  Overall Score: %d/100 (%s)\n", report.Summary.OverallScore, report.Summary.OptimizationLevel))
	if len(report.Summary.KeyBenefits) > 0 {
		sb.WriteString("  Key Benefits:\n")
		for _, benefit := range report.Summary.KeyBenefits {
			sb.WriteString(fmt.Sprintf("    - %s\n", benefit))
		}
	}

	return sb.String()
}

// saveOptimizationResultsToDatabase saves the optimization results to the database
func saveOptimizationResultsToDatabase(ctx context.Context, dataStore datastore.DataStore, report *OptimizationReport) error {
	// Convert report to JSON for storage
	reportJSON, err := jsonMarshal(report)
	if err != nil {
		return fmt.Errorf("failed to marshal report for storage: %w", err)
	}

	// Create optimization DSL fragment
	optimizationDSL := fmt.Sprintf(`;; Phase 6 Optimization Results
;; CBU: %s
;; Generated: %s
;; Strategy: %s

(optimization.complete
  (cbu.id "%s")
  (timestamp "%s")
  (strategy "%s")
  (overall-score %d)
  (optimization-level "%s")
  (cost-reduction %.2f)
  (time-reduction %d)
  (report.json %q)
)`,
		report.CBU,
		report.Timestamp,
		report.Configuration.Strategy,
		report.CBU,
		report.Timestamp,
		report.Configuration.Strategy,
		report.Summary.OverallScore,
		report.Summary.OptimizationLevel,
		report.Improvements.CostReduction,
		report.Improvements.TimeReduction,
		string(reportJSON),
	)

	// Create new DSL record
	_, err = dataStore.InsertDSLWithState(ctx, report.CBU, optimizationDSL, store.StateCompleted)
	return err
}

// JSON marshaling functions
func jsonMarshal(v interface{}) ([]byte, error) {
	return json.Marshal(v)
}

func jsonMarshalIndent(v interface{}, prefix, indent string) ([]byte, error) {
	return json.MarshalIndent(v, prefix, indent)
}
