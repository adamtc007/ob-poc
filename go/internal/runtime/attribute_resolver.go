package runtime

import (
	"context"
	"encoding/json"
	"fmt"
	"regexp"
	"strconv"
	"strings"
	"time"

	"dsl-ob-poc/internal/datastore"
	"dsl-ob-poc/internal/dictionary"
)

// AttributeResolver resolves attribute values from DSL state and applies transformations
type AttributeResolver struct {
	dataStore   datastore.DataStore
	transformer *AttributeTransformer
}

// NewAttributeResolver creates a new attribute resolver
func NewAttributeResolver(dataStore datastore.DataStore) *AttributeResolver {
	return &AttributeResolver{
		dataStore:   dataStore,
		transformer: NewAttributeTransformer(),
	}
}

// ResolvedAttribute represents an attribute with its resolved value
type ResolvedAttribute struct {
	AttributeID      string      `json:"attribute_id"`
	AttributeName    string      `json:"attribute_name"`
	Value            interface{} `json:"value"`
	RawValue         string      `json:"raw_value"`
	TransformedValue interface{} `json:"transformed_value,omitempty"`
	Source           string      `json:"source"`
	Confidence       float64     `json:"confidence"`
}

// AttributeResolutionContext provides context for attribute resolution
type AttributeResolutionContext struct {
	CBUID        string         `json:"cbu_id"`
	DSLVersionID string         `json:"dsl_version_id"`
	DSLContent   string         `json:"dsl_content"`
	Environment  string         `json:"environment"`
	ExtraContext map[string]any `json:"extra_context,omitempty"`
}

// ResolveAttributesForAction resolves all required attributes for an action execution
func (ar *AttributeResolver) ResolveAttributesForAction(ctx context.Context, actionDef *ActionDefinition, resolutionCtx *AttributeResolutionContext) (map[string]*ResolvedAttribute, error) {
	resolvedAttributes := make(map[string]*ResolvedAttribute)

	// Resolve input mapping attributes
	for _, mapping := range actionDef.AttributeMapping.InputMapping {
		resolved, err := ar.ResolveAttribute(ctx, mapping.DSLAttributeID, resolutionCtx)
		if err != nil {
			return nil, fmt.Errorf("failed to resolve attribute %s: %w", mapping.DSLAttributeID, err)
		}

		// Apply transformation if specified
		if mapping.Transformation != "" {
			transformed, err := ar.transformer.Transform(ctx, resolved.Value, mapping.Transformation)
			if err != nil {
				return nil, fmt.Errorf("failed to transform attribute %s: %w", mapping.DSLAttributeID, err)
			}
			resolved.TransformedValue = transformed
		}

		resolvedAttributes[mapping.DSLAttributeID] = resolved
	}

	return resolvedAttributes, nil
}

// ResolveAttribute resolves a single attribute value from various sources
func (ar *AttributeResolver) ResolveAttribute(ctx context.Context, attributeID string, resolutionCtx *AttributeResolutionContext) (*ResolvedAttribute, error) {
	// First, try to get attribute metadata from dictionary
	attr, err := ar.dataStore.GetDictionaryAttributeByID(ctx, attributeID)
	if err != nil {
		return nil, fmt.Errorf("failed to get attribute metadata: %w", err)
	}

	// Try multiple resolution strategies in order of preference
	resolvers := []func(context.Context, *dictionary.Attribute, *AttributeResolutionContext) (*ResolvedAttribute, error){
		ar.resolveFromDSLState,     // 1. From current DSL state
		ar.resolveFromDatabase,     // 2. From stored attribute values
		ar.resolveFromSourceSystem, // 3. From source system (if configured)
		ar.resolveFromDefaultValue, // 4. From default value
	}

	var lastErr error
	for _, resolver := range resolvers {
		resolved, err := resolver(ctx, attr, resolutionCtx)
		if err == nil && resolved != nil {
			return resolved, nil
		}
		lastErr = err
	}

	return nil, fmt.Errorf("failed to resolve attribute %s (%s): %w", attr.Name, attributeID, lastErr)
}

// resolveFromDSLState attempts to resolve attribute from current DSL state
func (ar *AttributeResolver) resolveFromDSLState(ctx context.Context, attr *dictionary.Attribute, resolutionCtx *AttributeResolutionContext) (*ResolvedAttribute, error) {
	if resolutionCtx.DSLContent == "" {
		return nil, fmt.Errorf("no DSL content provided")
	}

	// Parse DSL to find attribute bindings
	// Look for patterns like: (values.bind (bind (attr-id "uuid") (value "some-value")))
	pattern := fmt.Sprintf(`\(values\.bind\s+\(bind\s+\(attr-id\s+"%s"\)\s+\(value\s+"([^"]+)"\)\)\)`, regexp.QuoteMeta(attr.AttributeID))
	re := regexp.MustCompile(pattern)

	matches := re.FindStringSubmatch(resolutionCtx.DSLContent)
	if len(matches) >= 2 {
		value := matches[1]
		return &ResolvedAttribute{
			AttributeID:   attr.AttributeID,
			AttributeName: attr.Name,
			Value:         value,
			RawValue:      value,
			Source:        "dsl_state",
			Confidence:    1.0,
		}, nil
	}

	// Also look for variable declarations: (var (attr-id "uuid"))
	varPattern := fmt.Sprintf(`\(var\s+\(attr-id\s+"%s"\)\)`, regexp.QuoteMeta(attr.AttributeID))
	varRe := regexp.MustCompile(varPattern)

	if varRe.MatchString(resolutionCtx.DSLContent) {
		// Variable is declared but not bound - check if we can infer value from context
		if inferredValue := ar.inferValueFromContext(attr, resolutionCtx); inferredValue != nil {
			return &ResolvedAttribute{
				AttributeID:   attr.AttributeID,
				AttributeName: attr.Name,
				Value:         inferredValue,
				RawValue:      fmt.Sprintf("%v", inferredValue),
				Source:        "dsl_inference",
				Confidence:    0.8,
			}, nil
		}
	}

	return nil, fmt.Errorf("attribute not found in DSL state")
}

// resolveFromDatabase attempts to resolve attribute from stored values
func (ar *AttributeResolver) resolveFromDatabase(ctx context.Context, attr *dictionary.Attribute, resolutionCtx *AttributeResolutionContext) (*ResolvedAttribute, error) {
	// Use the datastore to resolve stored attribute values
	payload, _, status, err := ar.dataStore.ResolveValueFor(ctx, resolutionCtx.CBUID, attr.AttributeID)
	if err != nil {
		return nil, fmt.Errorf("failed to resolve from database: %w", err)
	}

	if len(payload) == 0 {
		return nil, fmt.Errorf("no stored value found")
	}

	// Parse the JSON payload
	var value interface{}
	if err := json.Unmarshal(payload, &value); err != nil {
		// If not JSON, treat as string
		value = string(payload)
	}

	confidence := 0.9
	if status == "provisional" {
		confidence = 0.7
	}

	return &ResolvedAttribute{
		AttributeID:   attr.AttributeID,
		AttributeName: attr.Name,
		Value:         value,
		RawValue:      string(payload),
		Source:        fmt.Sprintf("database_%s", status),
		Confidence:    confidence,
	}, nil
}

// resolveFromSourceSystem attempts to resolve attribute from configured source system
func (ar *AttributeResolver) resolveFromSourceSystem(ctx context.Context, attr *dictionary.Attribute, resolutionCtx *AttributeResolutionContext) (*ResolvedAttribute, error) {
	// Check if attribute has source metadata configured
	if attr.Source.Primary == "" {
		return nil, fmt.Errorf("no source system configured")
	}

	// This would integrate with external systems based on source metadata
	// For now, return a placeholder implementation

	// Example source system integration would look like:
	// switch attr.Source.Primary {
	// case "CRM":
	//     return ar.resolveFromCRM(ctx, attr, resolutionCtx)
	// case "Core_Banking":
	//     return ar.resolveFromCoreBanking(ctx, attr, resolutionCtx)
	// case "KYC_Provider":
	//     return ar.resolveFromKYCProvider(ctx, attr, resolutionCtx)
	// }

	return nil, fmt.Errorf("source system integration not yet implemented for %s", attr.Source.Primary)
}

// resolveFromDefaultValue attempts to resolve attribute from configured default value
func (ar *AttributeResolver) resolveFromDefaultValue(ctx context.Context, attr *dictionary.Attribute, resolutionCtx *AttributeResolutionContext) (*ResolvedAttribute, error) {
	// Check for default value in constraints or derivation rules
	if attr.DefaultValue != "" {
		return &ResolvedAttribute{
			AttributeID:   attr.AttributeID,
			AttributeName: attr.Name,
			Value:         attr.DefaultValue,
			RawValue:      attr.DefaultValue,
			Source:        "default_value",
			Confidence:    0.5,
		}, nil
	}

	// Check mask for default patterns
	switch attr.Mask {
	case "timestamp":
		return &ResolvedAttribute{
			AttributeID:   attr.AttributeID,
			AttributeName: attr.Name,
			Value:         time.Now().Format(time.RFC3339),
			RawValue:      time.Now().Format(time.RFC3339),
			Source:        "generated_timestamp",
			Confidence:    0.6,
		}, nil
	case "uuid":
		// Would generate a new UUID
		return &ResolvedAttribute{
			AttributeID:   attr.AttributeID,
			AttributeName: attr.Name,
			Value:         "generated-uuid-placeholder",
			RawValue:      "generated-uuid-placeholder",
			Source:        "generated_uuid",
			Confidence:    0.6,
		}, nil
	}

	return nil, fmt.Errorf("no default value available")
}

// inferValueFromContext attempts to infer attribute value from execution context
func (ar *AttributeResolver) inferValueFromContext(attr *dictionary.Attribute, resolutionCtx *AttributeResolutionContext) interface{} {
	// Check extra context for attribute values
	if resolutionCtx.ExtraContext != nil {
		if value, exists := resolutionCtx.ExtraContext[attr.Name]; exists {
			return value
		}
		if value, exists := resolutionCtx.ExtraContext[attr.AttributeID]; exists {
			return value
		}
	}

	// Infer based on attribute name patterns
	name := strings.ToLower(attr.Name)
	switch {
	case strings.Contains(name, "environment"):
		return resolutionCtx.Environment
	case strings.Contains(name, "cbu_id") || strings.Contains(name, "client_id"):
		return resolutionCtx.CBUID
	case strings.Contains(name, "dsl_version"):
		return resolutionCtx.DSLVersionID
	case strings.Contains(name, "timestamp") || strings.Contains(name, "created_at"):
		return time.Now().Format(time.RFC3339)
	}

	return nil
}

// ValidateRequiredAttributes checks if all required attributes for a resource type are available
func (ar *AttributeResolver) ValidateRequiredAttributes(ctx context.Context, resourceTypeID string, resolvedAttributes map[string]*ResolvedAttribute) error {
	// Get resource type attribute requirements
	_ = `
		SELECT rta.attribute_id, rta.required, d.name
		FROM resource_type_attributes rta
		JOIN "dsl-ob-poc".dictionary d ON rta.attribute_id = d.attribute_id
		WHERE rta.resource_type_id = $1`

	// This would need to be implemented with the runtime repository
	// For now, assume all resolved attributes are sufficient

	var missingRequired []string
	for _, attr := range resolvedAttributes {
		if attr.Confidence < 0.7 { // Low confidence threshold
			missingRequired = append(missingRequired, attr.AttributeName)
		}
	}

	if len(missingRequired) > 0 {
		return fmt.Errorf("required attributes missing or low confidence: %v", missingRequired)
	}

	return nil
}

// BuildAPIRequestPayload builds the API request payload from resolved attributes
func (ar *AttributeResolver) BuildAPIRequestPayload(ctx context.Context, attributeMapping AttributeMapping, resolvedAttributes map[string]*ResolvedAttribute) (map[string]interface{}, error) {
	payload := make(map[string]interface{})

	for _, mapping := range attributeMapping.InputMapping {
		resolved, exists := resolvedAttributes[mapping.DSLAttributeID]
		if !exists {
			return nil, fmt.Errorf("resolved attribute %s not found", mapping.DSLAttributeID)
		}

		// Use transformed value if available, otherwise use original value
		value := resolved.Value
		if resolved.TransformedValue != nil {
			value = resolved.TransformedValue
		}

		payload[mapping.APIParameter] = value
	}

	return payload, nil
}

// AttributeTransformer handles attribute value transformations
type AttributeTransformer struct {
	// Custom transformation functions can be registered here
}

// NewAttributeTransformer creates a new attribute transformer
func NewAttributeTransformer() *AttributeTransformer {
	return &AttributeTransformer{}
}

// Transform applies a transformation to an attribute value
func (at *AttributeTransformer) Transform(ctx context.Context, value interface{}, transformation string) (interface{}, error) {
	if transformation == "" {
		return value, nil
	}

	valueStr := fmt.Sprintf("%v", value)

	switch strings.ToLower(transformation) {
	case "uppercase":
		return strings.ToUpper(valueStr), nil

	case "lowercase":
		return strings.ToLower(valueStr), nil

	case "trim":
		return strings.TrimSpace(valueStr), nil

	case "iso_currency_code":
		// Convert currency name to ISO code
		return at.transformToCurrencyCode(valueStr), nil

	case "iso_country_code":
		// Convert country name to ISO code
		return at.transformToCountryCode(valueStr), nil

	case "phone_e164":
		// Convert phone number to E.164 format
		return at.transformToE164Phone(valueStr), nil

	case "date_iso8601":
		// Convert date to ISO 8601 format
		return at.transformToISO8601Date(valueStr), nil

	case "boolean":
		// Convert string to boolean
		val, err := at.transformToBoolean(valueStr)
		if err != nil {
			return nil, err
		}
		return val, nil

	case "integer":
		// Convert string to integer
		return strconv.Atoi(valueStr)

	case "float":
		// Convert string to float
		return strconv.ParseFloat(valueStr, 64)

	default:
		// Check if it's a regex transformation
		if strings.HasPrefix(transformation, "regex:") {
			return at.transformWithRegex(valueStr, transformation[6:])
		}

		return nil, fmt.Errorf("unsupported transformation: %s", transformation)
	}
}

// transformToCurrencyCode converts currency names to ISO codes
func (at *AttributeTransformer) transformToCurrencyCode(value string) string {
	currencyMap := map[string]string{
		"dollar":        "USD",
		"us dollar":     "USD",
		"euro":          "EUR",
		"pound":         "GBP",
		"british pound": "GBP",
		"yen":           "JPY",
		"japanese yen":  "JPY",
		"swiss franc":   "CHF",
		"franc":         "CHF",
	}

	normalized := strings.ToLower(strings.TrimSpace(value))
	if code, exists := currencyMap[normalized]; exists {
		return code
	}

	// If already looks like a currency code, return uppercase
	if len(value) == 3 && regexp.MustCompile(`^[A-Za-z]{3}$`).MatchString(value) {
		return strings.ToUpper(value)
	}

	return value // Return as-is if no transformation applies
}

// transformToCountryCode converts country names to ISO codes
func (at *AttributeTransformer) transformToCountryCode(value string) string {
	countryMap := map[string]string{
		"united states":  "US",
		"usa":            "US",
		"united kingdom": "GB",
		"uk":             "GB",
		"germany":        "DE",
		"france":         "FR",
		"switzerland":    "CH",
		"japan":          "JP",
		"canada":         "CA",
		"australia":      "AU",
		"luxembourg":     "LU",
	}

	normalized := strings.ToLower(strings.TrimSpace(value))
	if code, exists := countryMap[normalized]; exists {
		return code
	}

	// If already looks like a country code, return uppercase
	if len(value) == 2 && regexp.MustCompile(`^[A-Za-z]{2}$`).MatchString(value) {
		return strings.ToUpper(value)
	}

	return value
}

// transformToE164Phone converts phone numbers to E.164 format
func (at *AttributeTransformer) transformToE164Phone(value string) string {
	// Remove all non-digit characters except +
	re := regexp.MustCompile(`[^\d+]`)
	cleaned := re.ReplaceAllString(value, "")

	// Add + if not present and looks like international number
	if !strings.HasPrefix(cleaned, "+") && len(cleaned) > 10 {
		cleaned = "+" + cleaned
	}

	return cleaned
}

// transformToISO8601Date converts dates to ISO 8601 format
func (at *AttributeTransformer) transformToISO8601Date(value string) string {
	// Try parsing common date formats
	formats := []string{
		"2006-01-02",
		"01/02/2006",
		"01-02-2006",
		"2006/01/02",
		"Jan 2, 2006",
		"January 2, 2006",
		time.RFC3339,
		time.RFC822,
	}

	for _, format := range formats {
		if t, err := time.Parse(format, value); err == nil {
			return t.Format("2006-01-02")
		}
	}

	return value // Return as-is if no format matches
}

// transformToBoolean converts strings to boolean values
func (at *AttributeTransformer) transformToBoolean(value string) (bool, error) {
	normalized := strings.ToLower(strings.TrimSpace(value))

	switch normalized {
	case "true", "yes", "y", "1", "on", "enabled":
		return true, nil
	case "false", "no", "n", "0", "off", "disabled":
		return false, nil
	default:
		return false, fmt.Errorf("cannot convert '%s' to boolean", value)
	}
}

// transformWithRegex applies regex transformation
func (at *AttributeTransformer) transformWithRegex(value, pattern string) (string, error) {
	// Pattern format: "s/search/replace/" or "s/search/replace/flags"
	parts := strings.Split(pattern, "/")
	if len(parts) < 3 || parts[0] != "s" {
		return "", fmt.Errorf("invalid regex pattern format: %s", pattern)
	}

	searchPattern := parts[1]
	replacement := parts[2]

	re, err := regexp.Compile(searchPattern)
	if err != nil {
		return "", fmt.Errorf("invalid regex pattern: %w", err)
	}

	return re.ReplaceAllString(value, replacement), nil
}
