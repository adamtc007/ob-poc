// Package vocabulary provides database-driven DSL vocabulary management
// This replaces all hardcoded vocabulary maps with dynamic database storage
package vocabulary

import (
	"context"
	"time"
)

// =============================================================================
// Core Database Models
// =============================================================================

// GrammarRule represents a database-stored EBNF grammar rule
type GrammarRule struct {
	RuleID         string    `json:"rule_id" db:"rule_id"`
	RuleName       string    `json:"rule_name" db:"rule_name"`
	RuleDefinition string    `json:"rule_definition" db:"rule_definition"`
	RuleType       string    `json:"rule_type" db:"rule_type"`
	Domain         *string   `json:"domain" db:"domain"` // Nullable for universal rules
	Version        string    `json:"version" db:"version"`
	Active         bool      `json:"active" db:"active"`
	Description    *string   `json:"description" db:"description"`
	CreatedAt      time.Time `json:"created_at" db:"created_at"`
	UpdatedAt      time.Time `json:"updated_at" db:"updated_at"`
}

// DomainVocabulary represents a domain-specific DSL verb
type DomainVocabulary struct {
	VocabID     string                 `json:"vocab_id" db:"vocab_id"`
	Domain      string                 `json:"domain" db:"domain"`
	Verb        string                 `json:"verb" db:"verb"`
	Category    *string                `json:"category" db:"category"`
	Description *string                `json:"description" db:"description"`
	Parameters  map[string]interface{} `json:"parameters" db:"parameters"` // JSONB
	Examples    []interface{}          `json:"examples" db:"examples"`     // JSONB array
	Phase       *string                `json:"phase" db:"phase"`
	Active      bool                   `json:"active" db:"active"`
	Version     string                 `json:"version" db:"version"`
	CreatedAt   time.Time              `json:"created_at" db:"created_at"`
	UpdatedAt   time.Time              `json:"updated_at" db:"updated_at"`
}

// VerbRegistry represents global verb registry for conflict detection
type VerbRegistry struct {
	Verb            string    `json:"verb" db:"verb"`
	PrimaryDomain   string    `json:"primary_domain" db:"primary_domain"`
	Shared          bool      `json:"shared" db:"shared"`
	Deprecated      bool      `json:"deprecated" db:"deprecated"`
	ReplacementVerb *string   `json:"replacement_verb" db:"replacement_verb"`
	Description     *string   `json:"description" db:"description"`
	CreatedAt       time.Time `json:"created_at" db:"created_at"`
	UpdatedAt       time.Time `json:"updated_at" db:"updated_at"`
}

// VocabularyAudit represents vocabulary change audit trail
type VocabularyAudit struct {
	AuditID       string                 `json:"audit_id" db:"audit_id"`
	Domain        string                 `json:"domain" db:"domain"`
	Verb          string                 `json:"verb" db:"verb"`
	ChangeType    string                 `json:"change_type" db:"change_type"`       // CREATE, UPDATE, DELETE, DEPRECATE
	OldDefinition map[string]interface{} `json:"old_definition" db:"old_definition"` // JSONB
	NewDefinition map[string]interface{} `json:"new_definition" db:"new_definition"` // JSONB
	ChangedBy     *string                `json:"changed_by" db:"changed_by"`
	ChangeReason  *string                `json:"change_reason" db:"change_reason"`
	CreatedAt     time.Time              `json:"created_at" db:"created_at"`
}

// =============================================================================
// Parameter and Example Models
// =============================================================================

// VerbParameter represents a parameter definition for a DSL verb
type VerbParameter struct {
	Name        string   `json:"name"`
	Type        string   `json:"type"` // "string", "uuid", "enum", "array", etc.
	Required    bool     `json:"required"`
	Description string   `json:"description"`
	Examples    []string `json:"examples"`
	EnumValues  []string `json:"enum_values,omitempty"` // For enum types
}

// VerbExample represents a usage example for a DSL verb
type VerbExample struct {
	Description string `json:"description"`
	Usage       string `json:"usage"`
	Context     string `json:"context,omitempty"`
}

// =============================================================================
// Repository Interfaces
// =============================================================================

// GrammarRepository provides database access for grammar rules
type GrammarRepository interface {
	// Grammar Rule Operations
	CreateGrammarRule(ctx context.Context, rule *GrammarRule) error
	GetGrammarRule(ctx context.Context, ruleID string) (*GrammarRule, error)
	GetGrammarRuleByName(ctx context.Context, ruleName string) (*GrammarRule, error)
	ListGrammarRules(ctx context.Context, domain *string, active *bool) ([]*GrammarRule, error)
	UpdateGrammarRule(ctx context.Context, rule *GrammarRule) error
	DeleteGrammarRule(ctx context.Context, ruleID string) error

	// Grammar Validation
	ValidateGrammarSyntax(ctx context.Context, ruleDefinition string) error
	GetActiveGrammarForDomain(ctx context.Context, domain string) ([]*GrammarRule, error)
}

// VocabularyRepository provides database access for domain vocabularies
type VocabularyRepository interface {
	// Domain Vocabulary Operations
	CreateDomainVocab(ctx context.Context, vocab *DomainVocabulary) error
	GetDomainVocab(ctx context.Context, vocabID string) (*DomainVocabulary, error)
	GetDomainVocabByVerb(ctx context.Context, domain, verb string) (*DomainVocabulary, error)
	ListDomainVocabs(ctx context.Context, domain *string, category *string, active *bool) ([]*DomainVocabulary, error)
	UpdateDomainVocab(ctx context.Context, vocab *DomainVocabulary) error
	DeleteDomainVocab(ctx context.Context, vocabID string) error

	// Vocabulary Validation
	ValidateVerb(ctx context.Context, domain, verb string) error
	GetApprovedVerbs(ctx context.Context, domain string) ([]string, error)
	GetAllApprovedVerbs(ctx context.Context) (map[string][]string, error) // domain -> verbs
}

// VerbRegistryRepository provides database access for global verb registry
type VerbRegistryRepository interface {
	// Verb Registry Operations
	RegisterVerb(ctx context.Context, registry *VerbRegistry) error
	GetVerbRegistry(ctx context.Context, verb string) (*VerbRegistry, error)
	ListVerbRegistry(ctx context.Context, domain *string, shared *bool, deprecated *bool) ([]*VerbRegistry, error)
	UpdateVerbRegistry(ctx context.Context, registry *VerbRegistry) error
	DeprecateVerb(ctx context.Context, verb, replacementVerb string, reason string) error

	// Conflict Detection
	CheckVerbConflicts(ctx context.Context, verb string, domains []string) ([]*VerbRegistry, error)
	GetSharedVerbs(ctx context.Context) ([]*VerbRegistry, error)
}

// AuditRepository provides database access for vocabulary change auditing
type AuditRepository interface {
	// Audit Operations
	CreateAudit(ctx context.Context, audit *VocabularyAudit) error
	GetAuditHistory(ctx context.Context, domain, verb string, limit int) ([]*VocabularyAudit, error)
	GetRecentChanges(ctx context.Context, domain *string, changeType *string, limit int) ([]*VocabularyAudit, error)

	// Compliance Reporting
	GetChangesSince(ctx context.Context, since time.Time, domain *string) ([]*VocabularyAudit, error)
	GetChangeSummary(ctx context.Context, domain string, period time.Duration) (*ChangeSummary, error)
}

// =============================================================================
// Aggregate Interfaces
// =============================================================================

// Repository combines all vocabulary-related database operations
type Repository interface {
	GrammarRepository
	VocabularyRepository
	VerbRegistryRepository
	AuditRepository

	// Transaction Support
	BeginTx(ctx context.Context) (Repository, error)
	Commit() error
	Rollback() error
}

// =============================================================================
// Service Layer Interfaces
// =============================================================================

// VocabularyService provides high-level vocabulary management operations
type VocabularyService interface {
	// Domain Management
	InitializeDomain(ctx context.Context, domain string, verbs []DomainVocabulary) error
	GetDomainVocabulary(ctx context.Context, domain string) (map[string]DomainVocabulary, error)

	// Vocabulary Validation
	ValidateDSLVerbs(ctx context.Context, dsl string, domain *string) error
	GetApprovedVerbsForDomain(ctx context.Context, domain string) ([]string, error)

	// Dynamic Updates
	AddVerbToDomain(ctx context.Context, domain string, vocab DomainVocabulary, changedBy string) error
	UpdateVerbDefinition(ctx context.Context, domain, verb string, updates map[string]interface{}, changedBy string) error
	DeprecateVerb(ctx context.Context, domain, verb, replacement, reason, changedBy string) error

	// Cross-Domain Operations
	ShareVerbAcrossDomains(ctx context.Context, verb string, domains []string, changedBy string) error
	ResolveVerbConflicts(ctx context.Context, verb string, preferredDomain string) error
}

// =============================================================================
// Utility Types
// =============================================================================

// ChangeSummary provides aggregate statistics for vocabulary changes
type ChangeSummary struct {
	Domain          string         `json:"domain"`
	Period          string         `json:"period"`
	TotalChanges    int            `json:"total_changes"`
	ChangesByType   map[string]int `json:"changes_by_type"` // CREATE: 5, UPDATE: 3, etc.
	NewVerbs        []string       `json:"new_verbs"`
	UpdatedVerbs    []string       `json:"updated_verbs"`
	DeprecatedVerbs []string       `json:"deprecated_verbs"`
	ChangedBy       map[string]int `json:"changed_by"` // user -> change_count
}

// VocabularyValidationError represents validation errors
type VocabularyValidationError struct {
	Domain       string   `json:"domain"`
	InvalidVerbs []string `json:"invalid_verbs"`
	Message      string   `json:"message"`
}

func (e *VocabularyValidationError) Error() string {
	return e.Message
}

// =============================================================================
// Migration Support
// =============================================================================

// MigrationData represents vocabulary data to be migrated from hardcoded sources
type MigrationData struct {
	Domain     string             `json:"domain"`
	Verbs      []DomainVocabulary `json:"verbs"`
	Grammar    []GrammarRule      `json:"grammar,omitempty"`
	MigratedBy string             `json:"migrated_by"`
	MigratedAt time.Time          `json:"migrated_at"`
	Source     string             `json:"source"` // e.g., "internal/dsl/vocab.go"
}

// MigrationService provides vocabulary migration utilities
type MigrationService interface {
	// Migration Operations
	MigrateHardcodedVocabulary(ctx context.Context, data MigrationData) error
	ValidateMigrationData(ctx context.Context, data MigrationData) error
	GetMigrationStatus(ctx context.Context) (map[string]bool, error) // domain -> migrated

	// Domain-specific migration methods
	MigrateOnboardingDomain(ctx context.Context) error
	MigrateHedgeFundDomain(ctx context.Context) error
	MigrateOrchestrationDomain(ctx context.Context) error
	MigrateAllDomains(ctx context.Context) error

	// Cleanup Operations
	RemoveHardcodedReferences(ctx context.Context, domain string) error
	VerifyMigrationIntegrity(ctx context.Context, domain string) error
}

// =============================================================================
// Caching Interface
// =============================================================================

// VocabularyCache provides caching layer for vocabulary data
type VocabularyCache interface {
	// Cache Operations
	GetDomainVocabulary(ctx context.Context, domain string) (map[string]DomainVocabulary, error)
	SetDomainVocabulary(ctx context.Context, domain string, vocab map[string]DomainVocabulary, ttl time.Duration) error
	InvalidateDomainCache(ctx context.Context, domain string) error

	// Grammar Cache
	GetGrammarRules(ctx context.Context, domain string) ([]*GrammarRule, error)
	SetGrammarRules(ctx context.Context, domain string, rules []*GrammarRule, ttl time.Duration) error

	// Global Operations
	InvalidateAll(ctx context.Context) error
}
