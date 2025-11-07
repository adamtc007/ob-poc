// Package resolver provides domain-agnostic placeholder resolution for DSL.
//
// This package resolves placeholders like <investor_id>, <fund_id>, <cbu_id> etc.
// in DSL text by replacing them with actual UUIDs from session context.
package resolver

import (
	"fmt"
	"regexp"
	"strings"
)

// Resolver resolves placeholders in DSL using context values
type Resolver struct {
	// Placeholder pattern: <identifier>
	placeholderPattern *regexp.Regexp
}

// NewResolver creates a new placeholder resolver
func NewResolver() *Resolver {
	return &Resolver{
		placeholderPattern: regexp.MustCompile(`<([a-zA-Z_][a-zA-Z0-9_]*)>`),
	}
}

// Resolve replaces all placeholders in DSL with actual values from context
// Example: "(investor.start <investor_id>)" + {investor_id: "uuid-123"} -> "(investor.start \"uuid-123\")"
func (r *Resolver) Resolve(dsl string, context map[string]interface{}) (string, error) {
	if dsl == "" {
		return dsl, nil
	}

	unresolved := make([]string, 0)

	// Find all placeholders
	placeholders := r.FindPlaceholders(dsl)
	if len(placeholders) == 0 {
		return dsl, nil // No placeholders to resolve
	}

	result := dsl

	// Replace each placeholder
	for _, placeholder := range placeholders {
		key := placeholder // e.g., "investor_id"

		// Look up value in context
		value, found := context[key]
		if !found {
			// Try alternate forms (with/without underscores, camelCase, etc.)
			value, found = r.tryAlternateForms(key, context)
		}

		if !found {
			unresolved = append(unresolved, placeholder)
			continue
		}

		// Convert value to string
		valueStr := r.formatValue(value)

		// Replace placeholder with actual value
		placeholderPattern := fmt.Sprintf("<%s>", placeholder)
		result = strings.ReplaceAll(result, placeholderPattern, valueStr)
	}

	// Return error if any placeholders couldn't be resolved
	if len(unresolved) > 0 {
		return result, fmt.Errorf("unresolved placeholders: %v", unresolved)
	}

	return result, nil
}

// ResolveWithDefaults resolves placeholders, using default values for missing ones
func (r *Resolver) ResolveWithDefaults(dsl string, context map[string]interface{}, defaults map[string]string) string {
	if dsl == "" {
		return dsl
	}

	placeholders := r.FindPlaceholders(dsl)
	if len(placeholders) == 0 {
		return dsl
	}

	result := dsl

	for _, placeholder := range placeholders {
		key := placeholder

		// Try context first
		value, found := context[key]
		if !found {
			value, found = r.tryAlternateForms(key, context)
		}

		// Try defaults if not in context
		if !found {
			if defaultVal, ok := defaults[key]; ok {
				value = defaultVal
				found = true
			}
		}

		if found {
			valueStr := r.formatValue(value)
			placeholderPattern := fmt.Sprintf("<%s>", placeholder)
			result = strings.ReplaceAll(result, placeholderPattern, valueStr)
		}
	}

	return result
}

// FindPlaceholders returns all unique placeholders found in the DSL
// Returns the placeholder names without angle brackets
// Example: "<investor_id> <fund_id>" -> ["investor_id", "fund_id"]
func (r *Resolver) FindPlaceholders(dsl string) []string {
	matches := r.placeholderPattern.FindAllStringSubmatch(dsl, -1)

	seen := make(map[string]bool)
	placeholders := make([]string, 0)

	for _, match := range matches {
		if len(match) > 1 {
			placeholder := match[1] // Captured group (without < >)
			if !seen[placeholder] {
				placeholders = append(placeholders, placeholder)
				seen[placeholder] = true
			}
		}
	}

	return placeholders
}

// HasUnresolvedPlaceholders checks if the DSL contains any unresolved placeholders
func (r *Resolver) HasUnresolvedPlaceholders(dsl string) bool {
	return r.placeholderPattern.MatchString(dsl)
}

// ValidatePlaceholders checks if all placeholders in DSL can be resolved from context
func (r *Resolver) ValidatePlaceholders(dsl string, context map[string]interface{}) error {
	placeholders := r.FindPlaceholders(dsl)
	if len(placeholders) == 0 {
		return nil
	}

	missing := make([]string, 0)

	for _, placeholder := range placeholders {
		_, found := context[placeholder]
		if !found {
			_, found = r.tryAlternateForms(placeholder, context)
		}

		if !found {
			missing = append(missing, placeholder)
		}
	}

	if len(missing) > 0 {
		return fmt.Errorf("missing context values for placeholders: %v", missing)
	}

	return nil
}

// tryAlternateForms attempts to find context value using alternate key forms
// Tries: camelCase, snake_case, lowercase, etc.
func (r *Resolver) tryAlternateForms(key string, context map[string]interface{}) (interface{}, bool) {
	// Try as-is (already done by caller, but harmless)
	if val, ok := context[key]; ok {
		return val, true
	}

	// Try lowercase
	lowerKey := strings.ToLower(key)
	if val, ok := context[lowerKey]; ok {
		return val, true
	}

	// Try with hyphens instead of underscores
	hyphenKey := strings.ReplaceAll(key, "_", "-")
	if val, ok := context[hyphenKey]; ok {
		return val, true
	}

	// Try without underscores/hyphens
	noSepKey := strings.ReplaceAll(strings.ReplaceAll(key, "_", ""), "-", "")
	if val, ok := context[noSepKey]; ok {
		return val, true
	}

	// Try camelCase conversion
	camelKey := toCamelCase(key)
	if val, ok := context[camelKey]; ok {
		return val, true
	}

	return nil, false
}

// formatValue converts a context value to a string suitable for DSL
func (r *Resolver) formatValue(value interface{}) string {
	switch v := value.(type) {
	case string:
		// If it's already quoted, return as-is
		if strings.HasPrefix(v, "\"") && strings.HasSuffix(v, "\"") {
			return v
		}
		// Otherwise, quote it
		return fmt.Sprintf("\"%s\"", v)
	case int, int32, int64:
		return fmt.Sprintf("%d", v)
	case float32, float64:
		return fmt.Sprintf("%.2f", v)
	case bool:
		if v {
			return "true"
		}
		return "false"
	default:
		// Convert to string and quote
		return fmt.Sprintf("\"%v\"", v)
	}
}

// toCamelCase converts snake_case to camelCase
func toCamelCase(s string) string {
	parts := strings.Split(s, "_")
	if len(parts) <= 1 {
		return s
	}

	result := parts[0]
	for i := 1; i < len(parts); i++ {
		if len(parts[i]) > 0 {
			result += strings.ToUpper(parts[i][:1]) + parts[i][1:]
		}
	}
	return result
}

// ExtractContextFromDSL extracts entity IDs from resolved DSL back into context
// This is useful for updating context after DSL generation
// Example: "(investor.start \"uuid-123\")" -> {investor_id: "uuid-123"}
func (r *Resolver) ExtractContextFromDSL(dsl string, entityPatterns map[string]*regexp.Regexp) map[string]interface{} {
	context := make(map[string]interface{})

	for key, pattern := range entityPatterns {
		matches := pattern.FindStringSubmatch(dsl)
		if len(matches) > 1 {
			context[key] = matches[1]
		}
	}

	return context
}

// Common entity patterns for extracting from DSL
var (
	// InvestorIDPattern matches investor UUIDs in DSL (flexible format)
	InvestorIDPattern = regexp.MustCompile(`\(investor\s+"([a-zA-Z0-9-]+)"\)`)

	// FundIDPattern matches fund UUIDs in DSL (flexible format)
	FundIDPattern = regexp.MustCompile(`\(fund\s+"([a-zA-Z0-9-]+)"\)`)

	// ClassIDPattern matches class UUIDs in DSL (flexible format)
	ClassIDPattern = regexp.MustCompile(`\(class\s+"([a-zA-Z0-9-]+)"\)`)

	// CBUIDPattern matches CBU IDs in DSL
	CBUIDPattern = regexp.MustCompile(`\(cbu\.id\s+"([A-Z0-9-]+)"\)`)
)

// GetCommonEntityPatterns returns common entity extraction patterns
func GetCommonEntityPatterns() map[string]*regexp.Regexp {
	return map[string]*regexp.Regexp{
		"investor_id": InvestorIDPattern,
		"fund_id":     FundIDPattern,
		"class_id":    ClassIDPattern,
		"cbu_id":      CBUIDPattern,
	}
}
