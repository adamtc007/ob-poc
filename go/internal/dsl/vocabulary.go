package dsl

import (
	"context"
	"dsl-ob-poc/internal/datastore"
	"strings"
)

// Vocabulary represents the DSL vocabulary loaded from the database.
type Vocabulary struct {
	verbs map[string]bool
}

// NewVocabulary creates a new Vocabulary.
func NewVocabulary(verbs []string) *Vocabulary {
	verbMap := make(map[string]bool)
	for _, verb := range verbs {
		verbMap[strings.ToLower(verb)] = true
	}
	return &Vocabulary{verbs: verbMap}
}

// IsValidVerb checks if a verb is in the vocabulary.
func (v *Vocabulary) IsValidVerb(verb string) bool {
	_, ok := v.verbs[strings.ToLower(verb)]
	return ok
}

// VocabularyLoader loads the DSL vocabulary from the database.
type VocabularyLoader struct {
	ds datastore.DataStore
}

// NewVocabularyLoader creates a new VocabularyLoader.
func NewVocabularyLoader(ds datastore.DataStore) *VocabularyLoader {
	return &VocabularyLoader{ds: ds}
}

// LoadVocabulary loads the vocabulary from the database.
func (vl *VocabularyLoader) LoadVocabulary(ctx context.Context) (*Vocabulary, error) {
	// This is a placeholder for the actual database query.
	// In a real implementation, you would query the 'vocabulary' table.
	verbs := []string{
		"case.create",
		"products.add",
		"kyc.start",
		"kyc.modify",
		"services.discover",
		"resources.plan",
		"values.bind",
		"attributes.populated",
		"resource.create",
	}

	return NewVocabulary(verbs), nil
}
