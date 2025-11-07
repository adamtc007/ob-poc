package dsl

import (
	"context"
	"dsl-ob-poc/internal/datastore"
	"fmt"
	"strings"
)

// Validator provides methods for validating DSL.
// It checks for syntactic, semantic, and attribute validity.
type Validator struct {
	ds         datastore.DataStore
	vocabulary *Vocabulary
}

// NewValidator creates a new Validator.
func NewValidator(ds datastore.DataStore, vocab *Vocabulary) *Validator {
	return &Validator{
		ds:         ds,
		vocabulary: vocab,
	}
}

// Validate performs all validation checks on the given DSL string.
func (v *Validator) Validate(ctx context.Context, dsl string) error {
	if err := v.validateSyntax(dsl); err != nil {
		return fmt.Errorf("syntax validation failed: %w", err)
	}

	if err := v.validateSemantics(dsl); err != nil {
		return fmt.Errorf("semantic validation failed: %w", err)
	}

	if err := v.validateAttributes(ctx, dsl); err != nil {
		return fmt.Errorf("attribute validation failed: %w", err)
	}

	return nil
}

// validateSyntax checks for basic syntactic correctness (e.g., balanced parentheses).
func (v *Validator) validateSyntax(dsl string) error {
	balance := 0
	for _, r := range dsl {
		if r == '(' {
			balance++
		} else if r == ')' {
			balance--
		}
		if balance < 0 {
			return fmt.Errorf("unbalanced parentheses")
		}
	}
	if balance != 0 {
		return fmt.Errorf("unbalanced parentheses")
	}
	return nil
}

// validateSemantics checks if the verbs used in the DSL are valid.
func (v *Validator) validateSemantics(dsl string) error {
	lines := strings.Split(dsl, "\n")
	for _, line := range lines {
		trimmed := strings.TrimSpace(line)
		if strings.HasPrefix(trimmed, "(") {
			parts := strings.SplitN(trimmed[1:], " ", 2)
			verb := parts[0]

			if !v.vocabulary.IsValidVerb(verb) {
				// More robustly ignore non-verb expressions
				if !isNonVerb(verb) {
					return fmt.Errorf("unknown verb: %s", verb)
				}
			}
		}
	}
	return nil
}

// isNonVerb checks if a given token is a non-verb keyword.
func isNonVerb(token string) bool {
	switch token {
	case "var", "bind", "attr-id", "value", "for.product", "cbu.id", "nature-purpose", "owner", "name", "id":
		return true
	default:
		return false
	}
}

// validateAttributes checks if the attribute UUIDs used in the DSL exist in the dictionary.
func (v *Validator) validateAttributes(ctx context.Context, dsl string) error {
    ids := ExtractAttributeIDs(dsl)
    for _, id := range ids {
        _, err := v.ds.GetDictionaryAttributeByID(ctx, id)
        if err != nil {
            return fmt.Errorf("attribute with ID %s not found in dictionary: %w", id, err)
        }
    }

    return nil
}
