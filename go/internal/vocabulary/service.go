package vocabulary

import (
	"context"
	"fmt"
	"regexp"
	"strings"
	"time"
)

// VocabularyServiceImpl implements the VocabularyService interface
type VocabularyServiceImpl struct {
	repo  Repository
	cache VocabularyCache
}

// NewVocabularyService creates a new vocabulary service
func NewVocabularyService(repo Repository, cache VocabularyCache) VocabularyService {
	return &VocabularyServiceImpl{
		repo:  repo,
		cache: cache,
	}
}

// =============================================================================
// Domain Management
// =============================================================================

func (s *VocabularyServiceImpl) InitializeDomain(ctx context.Context, domain string, verbs []DomainVocabulary) error {
	// Start transaction
	tx, err := s.repo.BeginTx(ctx)
	if err != nil {
		return fmt.Errorf("failed to begin transaction: %w", err)
	}
	defer func() {
		if err != nil {
			tx.Rollback()
		}
	}()

	// Create domain vocabularies
	for _, vocab := range verbs {
		vocab.Domain = domain // Ensure domain is set
		if vocab.Active == false && vocab.Version == "" {
			vocab.Active = true
			vocab.Version = "1.0.0"
		}

		err = tx.CreateDomainVocab(ctx, &vocab)
		if err != nil {
			return fmt.Errorf("failed to create vocab %s.%s: %w", domain, vocab.Verb, err)
		}

		// Register in global verb registry
		registry := &VerbRegistry{
			Verb:          vocab.Verb,
			PrimaryDomain: domain,
			Shared:        false, // Default to domain-specific
			Deprecated:    false,
			Description:   vocab.Description,
		}

		err = tx.RegisterVerb(ctx, registry)
		if err != nil {
			return fmt.Errorf("failed to register verb %s: %w", vocab.Verb, err)
		}

		// Create audit record
		audit := &VocabularyAudit{
			Domain:     domain,
			Verb:       vocab.Verb,
			ChangeType: "CREATE",
			NewDefinition: map[string]interface{}{
				"category":    vocab.Category,
				"description": vocab.Description,
				"parameters":  vocab.Parameters,
				"examples":    vocab.Examples,
				"active":      vocab.Active,
				"version":     vocab.Version,
			},
			ChangedBy:    stringPtr("system"),
			ChangeReason: stringPtr("Domain initialization"),
		}

		err = tx.CreateAudit(ctx, audit)
		if err != nil {
			return fmt.Errorf("failed to create audit record: %w", err)
		}
	}

	err = tx.Commit()
	if err != nil {
		return fmt.Errorf("failed to commit transaction: %w", err)
	}

	// Invalidate cache
	if s.cache != nil {
		s.cache.InvalidateDomainCache(ctx, domain)
	}

	return nil
}

func (s *VocabularyServiceImpl) GetDomainVocabulary(ctx context.Context, domain string) (map[string]DomainVocabulary, error) {
	// Try cache first
	if s.cache != nil {
		cachedVocab, err := s.cache.GetDomainVocabulary(ctx, domain)
		if err == nil {
			return cachedVocab, nil
		}
		// Continue on cache miss
	}

	// Get from database
	active := true
	vocabs, err := s.repo.ListDomainVocabs(ctx, &domain, nil, &active)
	if err != nil {
		return nil, fmt.Errorf("failed to get domain vocabulary: %w", err)
	}

	result := make(map[string]DomainVocabulary)
	for _, vocab := range vocabs {
		result[vocab.Verb] = *vocab
	}

	// Cache the result
	if s.cache != nil {
		s.cache.SetDomainVocabulary(ctx, domain, result, 15*time.Minute)
	}

	return result, nil
}

// =============================================================================
// Vocabulary Validation
// =============================================================================

func (s *VocabularyServiceImpl) ValidateDSLVerbs(ctx context.Context, dsl string, domain *string) error {
	if strings.TrimSpace(dsl) == "" {
		return nil // Empty DSL is valid
	}

	// Extract verbs from DSL using regex - match verbs followed by space, ) or end of string
	verbRegex := regexp.MustCompile(`\(\s*([a-zA-Z][a-zA-Z0-9]*(?:\.[a-zA-Z][a-zA-Z0-9]*)*)(?:\s|\)|$)`)
	matches := verbRegex.FindAllStringSubmatch(dsl, -1)

	if len(matches) == 0 {
		return nil // No verbs found, could be just data
	}

	var invalidVerbs []string
	seenVerbs := make(map[string]bool)

	for _, match := range matches {
		if len(match) < 2 {
			continue
		}

		verb := match[1]
		if seenVerbs[verb] {
			continue // Skip duplicates
		}
		seenVerbs[verb] = true

		// Validate verb exists in database
		if domain != nil {
			err := s.repo.ValidateVerb(ctx, *domain, verb)
			if err != nil {
				invalidVerbs = append(invalidVerbs, verb)
			}
		} else {
			// Check if verb exists in any domain
			_, err := s.repo.GetVerbRegistry(ctx, verb)
			if err != nil {
				invalidVerbs = append(invalidVerbs, verb)
			}
		}
	}

	if len(invalidVerbs) > 0 {
		domainStr := "any domain"
		if domain != nil {
			domainStr = *domain
		}
		return &VocabularyValidationError{
			Domain:       domainStr,
			InvalidVerbs: invalidVerbs,
			Message:      fmt.Sprintf("invalid verbs for %s: %s", domainStr, strings.Join(invalidVerbs, ", ")),
		}
	}

	return nil
}

func (s *VocabularyServiceImpl) GetApprovedVerbsForDomain(ctx context.Context, domain string) ([]string, error) {
	verbs, err := s.repo.GetApprovedVerbs(ctx, domain)
	if err != nil {
		return nil, fmt.Errorf("failed to get approved verbs for domain %s: %w", domain, err)
	}
	return verbs, nil
}

// =============================================================================
// Dynamic Updates
// =============================================================================

func (s *VocabularyServiceImpl) AddVerbToDomain(ctx context.Context, domain string, vocab DomainVocabulary, changedBy string) error {
	// Start transaction
	tx, err := s.repo.BeginTx(ctx)
	if err != nil {
		return fmt.Errorf("failed to begin transaction: %w", err)
	}
	defer func() {
		if err != nil {
			tx.Rollback()
		}
	}()

	// Ensure domain and defaults are set
	vocab.Domain = domain
	if vocab.Version == "" {
		vocab.Version = "1.0.0"
	}
	if !vocab.Active {
		vocab.Active = true
	}

	// Check if verb already exists
	existing, existingErr := tx.GetDomainVocabByVerb(ctx, domain, vocab.Verb)
	if existingErr == nil && existing != nil {
		return fmt.Errorf("verb %s already exists in domain %s", vocab.Verb, domain)
	}

	// Create the vocabulary entry
	err = tx.CreateDomainVocab(ctx, &vocab)
	if err != nil {
		return fmt.Errorf("failed to create domain vocab: %w", err)
	}

	// Register in global registry
	registry := &VerbRegistry{
		Verb:          vocab.Verb,
		PrimaryDomain: domain,
		Shared:        false,
		Deprecated:    false,
		Description:   vocab.Description,
	}

	err = tx.RegisterVerb(ctx, registry)
	if err != nil {
		return fmt.Errorf("failed to register verb: %w", err)
	}

	// Create audit record
	audit := &VocabularyAudit{
		Domain:     domain,
		Verb:       vocab.Verb,
		ChangeType: "CREATE",
		NewDefinition: map[string]interface{}{
			"category":    vocab.Category,
			"description": vocab.Description,
			"parameters":  vocab.Parameters,
			"examples":    vocab.Examples,
			"active":      vocab.Active,
			"version":     vocab.Version,
		},
		ChangedBy:    &changedBy,
		ChangeReason: stringPtr("Manual verb addition"),
	}

	err = tx.CreateAudit(ctx, audit)
	if err != nil {
		return fmt.Errorf("failed to create audit record: %w", err)
	}

	err = tx.Commit()
	if err != nil {
		return fmt.Errorf("failed to commit transaction: %w", err)
	}

	// Invalidate cache
	if s.cache != nil {
		s.cache.InvalidateDomainCache(ctx, domain)
	}

	return nil
}

func (s *VocabularyServiceImpl) UpdateVerbDefinition(ctx context.Context, domain, verb string, updates map[string]interface{}, changedBy string) error {
	// Start transaction
	tx, err := s.repo.BeginTx(ctx)
	if err != nil {
		return fmt.Errorf("failed to begin transaction: %w", err)
	}
	defer func() {
		if err != nil {
			tx.Rollback()
		}
	}()

	// Get existing vocabulary
	existing, err := tx.GetDomainVocabByVerb(ctx, domain, verb)
	if err != nil {
		return fmt.Errorf("failed to get existing vocab: %w", err)
	}

	// Create old definition for audit
	oldDef := map[string]interface{}{
		"category":    existing.Category,
		"description": existing.Description,
		"parameters":  existing.Parameters,
		"examples":    existing.Examples,
		"active":      existing.Active,
		"version":     existing.Version,
	}

	// Apply updates
	if category, ok := updates["category"]; ok {
		if categoryStr, ok := category.(string); ok {
			existing.Category = &categoryStr
		}
	}
	if description, ok := updates["description"]; ok {
		if descStr, ok := description.(string); ok {
			existing.Description = &descStr
		}
	}
	if parameters, ok := updates["parameters"]; ok {
		if paramMap, ok := parameters.(map[string]interface{}); ok {
			existing.Parameters = paramMap
		}
	}
	if examples, ok := updates["examples"]; ok {
		if exampleSlice, ok := examples.([]interface{}); ok {
			existing.Examples = exampleSlice
		}
	}
	if active, ok := updates["active"]; ok {
		if activeBool, ok := active.(bool); ok {
			existing.Active = activeBool
		}
	}
	if version, ok := updates["version"]; ok {
		if versionStr, ok := version.(string); ok {
			existing.Version = versionStr
		}
	}

	// Update in database
	err = tx.UpdateDomainVocab(ctx, existing)
	if err != nil {
		return fmt.Errorf("failed to update domain vocab: %w", err)
	}

	// Create new definition for audit
	newDef := map[string]interface{}{
		"category":    existing.Category,
		"description": existing.Description,
		"parameters":  existing.Parameters,
		"examples":    existing.Examples,
		"active":      existing.Active,
		"version":     existing.Version,
	}

	// Create audit record
	audit := &VocabularyAudit{
		Domain:        domain,
		Verb:          verb,
		ChangeType:    "UPDATE",
		OldDefinition: oldDef,
		NewDefinition: newDef,
		ChangedBy:     &changedBy,
		ChangeReason:  stringPtr("Manual verb update"),
	}

	err = tx.CreateAudit(ctx, audit)
	if err != nil {
		return fmt.Errorf("failed to create audit record: %w", err)
	}

	err = tx.Commit()
	if err != nil {
		return fmt.Errorf("failed to commit transaction: %w", err)
	}

	// Invalidate cache
	if s.cache != nil {
		s.cache.InvalidateDomainCache(ctx, domain)
	}

	return nil
}

func (s *VocabularyServiceImpl) DeprecateVerb(ctx context.Context, domain, verb, replacement, reason, changedBy string) error {
	// Start transaction
	tx, err := s.repo.BeginTx(ctx)
	if err != nil {
		return fmt.Errorf("failed to begin transaction: %w", err)
	}
	defer func() {
		if err != nil {
			tx.Rollback()
		}
	}()

	// Get existing vocabulary
	existing, err := tx.GetDomainVocabByVerb(ctx, domain, verb)
	if err != nil {
		return fmt.Errorf("failed to get existing vocab: %w", err)
	}

	// Deactivate the vocabulary
	existing.Active = false
	err = tx.UpdateDomainVocab(ctx, existing)
	if err != nil {
		return fmt.Errorf("failed to deactivate vocab: %w", err)
	}

	// Update verb registry
	err = tx.DeprecateVerb(ctx, verb, replacement, reason)
	if err != nil {
		return fmt.Errorf("failed to deprecate verb in registry: %w", err)
	}

	// Create audit record
	audit := &VocabularyAudit{
		Domain:     domain,
		Verb:       verb,
		ChangeType: "DEPRECATE",
		OldDefinition: map[string]interface{}{
			"active":     true,
			"deprecated": false,
		},
		NewDefinition: map[string]interface{}{
			"active":           false,
			"deprecated":       true,
			"replacement_verb": replacement,
		},
		ChangedBy:    &changedBy,
		ChangeReason: &reason,
	}

	err = tx.CreateAudit(ctx, audit)
	if err != nil {
		return fmt.Errorf("failed to create audit record: %w", err)
	}

	err = tx.Commit()
	if err != nil {
		return fmt.Errorf("failed to commit transaction: %w", err)
	}

	// Invalidate cache
	if s.cache != nil {
		s.cache.InvalidateDomainCache(ctx, domain)
	}

	return nil
}

// =============================================================================
// Cross-Domain Operations
// =============================================================================

func (s *VocabularyServiceImpl) ShareVerbAcrossDomains(ctx context.Context, verb string, domains []string, changedBy string) error {
	if len(domains) == 0 {
		return fmt.Errorf("no domains specified")
	}

	// Start transaction
	tx, err := s.repo.BeginTx(ctx)
	if err != nil {
		return fmt.Errorf("failed to begin transaction: %w", err)
	}
	defer func() {
		if err != nil {
			tx.Rollback()
		}
	}()

	// Check for conflicts
	conflicts, err := tx.CheckVerbConflicts(ctx, verb, domains)
	if err != nil {
		return fmt.Errorf("failed to check verb conflicts: %w", err)
	}

	if len(conflicts) > 0 {
		conflictDomains := make([]string, len(conflicts))
		for i, conflict := range conflicts {
			conflictDomains[i] = conflict.PrimaryDomain
		}
		return fmt.Errorf("verb %s conflicts with domains: %s", verb, strings.Join(conflictDomains, ", "))
	}

	// Get or create verb registry
	registry, err := tx.GetVerbRegistry(ctx, verb)
	if err != nil {
		// Create new registry entry
		registry = &VerbRegistry{
			Verb:          verb,
			PrimaryDomain: domains[0], // Use first domain as primary
			Shared:        true,
			Deprecated:    false,
		}
		err = tx.RegisterVerb(ctx, registry)
		if err != nil {
			return fmt.Errorf("failed to register shared verb: %w", err)
		}
	} else {
		// Update existing registry to be shared
		registry.Shared = true
		err = tx.UpdateVerbRegistry(ctx, registry)
		if err != nil {
			return fmt.Errorf("failed to update verb registry: %w", err)
		}
	}

	// Create audit record
	audit := &VocabularyAudit{
		Domain:     "global",
		Verb:       verb,
		ChangeType: "UPDATE",
		OldDefinition: map[string]interface{}{
			"shared": false,
		},
		NewDefinition: map[string]interface{}{
			"shared":  true,
			"domains": domains,
		},
		ChangedBy:    &changedBy,
		ChangeReason: stringPtr("Shared verb across domains"),
	}

	err = tx.CreateAudit(ctx, audit)
	if err != nil {
		return fmt.Errorf("failed to create audit record: %w", err)
	}

	err = tx.Commit()
	if err != nil {
		return fmt.Errorf("failed to commit transaction: %w", err)
	}

	// Invalidate cache for all affected domains
	if s.cache != nil {
		for _, domain := range domains {
			s.cache.InvalidateDomainCache(ctx, domain)
		}
	}

	return nil
}

func (s *VocabularyServiceImpl) ResolveVerbConflicts(ctx context.Context, verb string, preferredDomain string) error {
	// Get current conflicts
	conflicts, err := s.repo.CheckVerbConflicts(ctx, verb, []string{preferredDomain})
	if err != nil {
		return fmt.Errorf("failed to check verb conflicts: %w", err)
	}

	if len(conflicts) == 0 {
		return nil // No conflicts to resolve
	}

	// Start transaction
	tx, err := s.repo.BeginTx(ctx)
	if err != nil {
		return fmt.Errorf("failed to begin transaction: %w", err)
	}
	defer func() {
		if err != nil {
			tx.Rollback()
		}
	}()

	// Update verb registry to set preferred domain as primary
	registry := conflicts[0] // Use first conflict as base
	registry.PrimaryDomain = preferredDomain
	registry.Shared = true // Make it shared to resolve conflicts

	err = tx.UpdateVerbRegistry(ctx, registry)
	if err != nil {
		return fmt.Errorf("failed to resolve verb conflict: %w", err)
	}

	err = tx.Commit()
	if err != nil {
		return fmt.Errorf("failed to commit transaction: %w", err)
	}

	return nil
}

// =============================================================================
// Helper Functions
// =============================================================================

// stringPtr helper function is defined in migration.go
