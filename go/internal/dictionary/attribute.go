package dictionary

import (
	"encoding/json"
	"fmt"
	"regexp"
	"strconv"
	"strings"
)

// DerivationType represents the method of attribute derivation
type DerivationType string

const (
	DerivationTypeFormula    DerivationType = "FORMULA"
	DerivationTypeConcat     DerivationType = "CONCAT"
	DerivationTypeTransform  DerivationType = "TRANSFORM"
	DerivationTypeCalculated DerivationType = "CALCULATED"
)

// TransformationRule defines how an attribute is transformed
type TransformationRule struct {
	Type         string            `json:"type"`
	Params       map[string]string `json:"params"`
	DefaultValue string            `json:"default_value,omitempty"`
}

// DerivationRule defines how an attribute can be derived
type DerivationRule struct {
	Type               DerivationType         `json:"type"`
	SourceAttributeIDs []string               `json:"source_attribute_ids"`
	Formula            string                 `json:"formula,omitempty"`
	Transformation     *TransformationRule    `json:"transformation,omitempty"`
	DependencyRules    map[string]string      `json:"dependency_rules,omitempty"`
	ValidationRules    map[string]interface{} `json:"validation_rules,omitempty"`
}

// SourceMetadata defines the rich metadata for an attribute's source.
type SourceMetadata struct {
	Primary   string `json:"primary"`
	Secondary string `json:"secondary,omitempty"`
	Tertiary  string `json:"tertiary,omitempty"`
}

// SinkMetadata defines the rich metadata for an attribute's sink.
type SinkMetadata struct {
	Primary   string `json:"primary"`
	Secondary string `json:"secondary,omitempty"`
	Tertiary  string `json:"tertiary,omitempty"`
}

// Attribute represents a dictionary attribute with rich metadata
// This matches the actual database schema in "dsl-ob-poc".dictionary table
type Attribute struct {
	AttributeID     string         `json:"attribute_id"`
	Name            string         `json:"name"`
	LongDescription string         `json:"long_description"`
	GroupID         string         `json:"group_id"`
	Mask            string         `json:"mask"`
	Domain          string         `json:"domain"`
	Vector          string         `json:"vector,omitempty"`
	Source          SourceMetadata `json:"source"`
	Sink            SinkMetadata   `json:"sink"`

	// Extended fields (not yet in database schema but available for future use)
	Derivation   *DerivationRule `json:"derivation,omitempty"`
	Constraints  []string        `json:"constraints,omitempty"`
	DefaultValue string          `json:"default_value,omitempty"`
	Tags         []string        `json:"tags,omitempty"`
	Sensitivity  string          `json:"sensitivity,omitempty"`
}

// Validate checks attribute constraints (if any are defined)
func (a *Attribute) Validate(value string) error {
	// Basic validation based on mask
	// For now, we don't enforce required fields at this level
	// This can be extended when constraints are added to the schema

	// Extended constraint validation (when constraints are populated)
	for _, constraint := range a.Constraints {
		switch {
		case constraint == "REQUIRED" && value == "":
			return fmt.Errorf("attribute %s is required", a.Name)
		case strings.HasPrefix(constraint, "MIN_LENGTH:"):
			minLen, _ := strconv.Atoi(strings.TrimPrefix(constraint, "MIN_LENGTH:"))
			if len(value) < minLen {
				return fmt.Errorf("%s must be at least %d characters", a.Name, minLen)
			}
		case strings.HasPrefix(constraint, "MAX_LENGTH:"):
			maxLen, _ := strconv.Atoi(strings.TrimPrefix(constraint, "MAX_LENGTH:"))
			if len(value) > maxLen {
				return fmt.Errorf("%s must be no more than %d characters", a.Name, maxLen)
			}
		case strings.HasPrefix(constraint, "REGEX:"):
			pattern := strings.TrimPrefix(constraint, "REGEX:")
			match, _ := regexp.MatchString(pattern, value)
			if !match {
				return fmt.Errorf("%s does not match pattern %s", a.Name, pattern)
			}
		}
	}
	return nil
}

// IsCore returns true if this attribute is stored in the core database schema
func (a *Attribute) IsCore() bool {
	// Core attributes are those that exist in the current database schema
	return a.AttributeID != "" && a.Name != ""
}

// HasExtendedMetadata returns true if this attribute has extended metadata
func (a *Attribute) HasExtendedMetadata() bool {
	return len(a.Constraints) > 0 || len(a.Tags) > 0 ||
		a.Sensitivity != "" || a.Derivation != nil
}

// ToJSON converts the Attribute to a JSON string
func (a *Attribute) ToJSON() (string, error) {
	jsonBytes, err := json.MarshalIndent(a, "", "  ")
	if err != nil {
		return "", err
	}
	return string(jsonBytes), nil
}
