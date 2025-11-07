package vocabulary

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"strings"
	"time"

	"github.com/jmoiron/sqlx"
	"github.com/lib/pq"
)

// PostgresRepository implements all vocabulary repository interfaces using PostgreSQL
type PostgresRepository struct {
	db *sqlx.DB
	tx *sqlx.Tx
}

// NewPostgresRepository creates a new PostgreSQL vocabulary repository
func NewPostgresRepository(db *sqlx.DB) Repository {
	return &PostgresRepository{
		db: db,
	}
}

// =============================================================================
// Transaction Support
// =============================================================================

func (r *PostgresRepository) BeginTx(ctx context.Context) (Repository, error) {
	tx, err := r.db.BeginTxx(ctx, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to begin transaction: %w", err)
	}

	return &PostgresRepository{
		db: r.db,
		tx: tx,
	}, nil
}

func (r *PostgresRepository) Commit() error {
	if r.tx == nil {
		return fmt.Errorf("no active transaction")
	}
	return r.tx.Commit()
}

func (r *PostgresRepository) Rollback() error {
	if r.tx == nil {
		return fmt.Errorf("no active transaction")
	}
	return r.tx.Rollback()
}

func (r *PostgresRepository) getContext(ctx context.Context, dest interface{}, query string, args ...interface{}) error {
	if r.tx != nil {
		return r.tx.GetContext(ctx, dest, query, args...)
	}
	return r.db.GetContext(ctx, dest, query, args...)
}

func (r *PostgresRepository) selectContext(ctx context.Context, dest interface{}, query string, args ...interface{}) error {
	if r.tx != nil {
		return r.tx.SelectContext(ctx, dest, query, args...)
	}
	return r.db.SelectContext(ctx, dest, query, args...)
}

func (r *PostgresRepository) queryRowxContext(ctx context.Context, query string, args ...interface{}) *sqlx.Row {
	if r.tx != nil {
		return r.tx.QueryRowxContext(ctx, query, args...)
	}
	return r.db.QueryRowxContext(ctx, query, args...)
}

func (r *PostgresRepository) queryxContext(ctx context.Context, query string, args ...interface{}) (*sqlx.Rows, error) {
	if r.tx != nil {
		return r.tx.QueryxContext(ctx, query, args...)
	}
	return r.db.QueryxContext(ctx, query, args...)
}

func (r *PostgresRepository) execContext(ctx context.Context, query string, args ...interface{}) (sql.Result, error) {
	if r.tx != nil {
		return r.tx.ExecContext(ctx, query, args...)
	}
	return r.db.ExecContext(ctx, query, args...)
}

// =============================================================================
// Grammar Repository Implementation
// =============================================================================

func (r *PostgresRepository) CreateGrammarRule(ctx context.Context, rule *GrammarRule) error {
	query := `
		INSERT INTO "dsl-ob-poc".grammar_rules
		(rule_name, rule_definition, rule_type, domain, version, active, description)
		VALUES ($1, $2, $3, $4, $5, $6, $7)
		RETURNING rule_id, created_at, updated_at`

	err := r.queryRowxContext(ctx, query,
		rule.RuleName, rule.RuleDefinition, rule.RuleType, rule.Domain,
		rule.Version, rule.Active, rule.Description,
	).Scan(&rule.RuleID, &rule.CreatedAt, &rule.UpdatedAt)

	if err != nil {
		return fmt.Errorf("failed to create grammar rule: %w", err)
	}
	return nil
}

func (r *PostgresRepository) GetGrammarRule(ctx context.Context, ruleID string) (*GrammarRule, error) {
	var rule GrammarRule
	query := `
		SELECT rule_id, rule_name, rule_definition, rule_type, domain, version,
		       active, description, created_at, updated_at
		FROM "dsl-ob-poc".grammar_rules
		WHERE rule_id = $1`

	err := r.getContext(ctx, &rule, query, ruleID)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, fmt.Errorf("grammar rule not found: %s", ruleID)
	}
	if err != nil {
		return nil, fmt.Errorf("failed to get grammar rule: %w", err)
	}
	return &rule, nil
}

func (r *PostgresRepository) GetGrammarRuleByName(ctx context.Context, ruleName string) (*GrammarRule, error) {
	var rule GrammarRule
	query := `
		SELECT rule_id, rule_name, rule_definition, rule_type, domain, version,
		       active, description, created_at, updated_at
		FROM "dsl-ob-poc".grammar_rules
		WHERE rule_name = $1 AND active = true
		ORDER BY created_at DESC
		LIMIT 1`

	err := r.getContext(ctx, &rule, query, ruleName)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, fmt.Errorf("grammar rule not found: %s", ruleName)
	}
	if err != nil {
		return nil, fmt.Errorf("failed to get grammar rule by name: %w", err)
	}
	return &rule, nil
}

func (r *PostgresRepository) ListGrammarRules(ctx context.Context, domain *string, active *bool) ([]*GrammarRule, error) {
	var rules []*GrammarRule

	var args []interface{}
	var conditions []string
	paramIndex := 1

	conditions = append(conditions, "TRUE")

	if domain != nil {
		conditions = append(conditions, fmt.Sprintf("domain = $%d", paramIndex))
		args = append(args, *domain)
		paramIndex++
	}

	if active != nil {
		conditions = append(conditions, fmt.Sprintf("active = $%d", paramIndex))
		args = append(args, *active)
		paramIndex++
	}

	whereClause := strings.Join(conditions, " AND ")

	query := fmt.Sprintf(`
		SELECT rule_id, rule_name, rule_definition, rule_type, domain, version,
		       active, description, created_at, updated_at
		FROM "dsl-ob-poc".grammar_rules
		WHERE %s
		ORDER BY rule_name`, whereClause)

	err := r.selectContext(ctx, &rules, query, args...)
	if err != nil {
		return nil, fmt.Errorf("failed to list grammar rules: %w", err)
	}
	return rules, nil
}

func (r *PostgresRepository) UpdateGrammarRule(ctx context.Context, rule *GrammarRule) error {
	query := `
		UPDATE "dsl-ob-poc".grammar_rules
		SET rule_definition = $2, rule_type = $3, domain = $4, version = $5,
		    active = $6, description = $7, updated_at = now()
		WHERE rule_id = $1`

	result, err := r.execContext(ctx, query,
		rule.RuleID, rule.RuleDefinition, rule.RuleType, rule.Domain,
		rule.Version, rule.Active, rule.Description)

	if err != nil {
		return fmt.Errorf("failed to update grammar rule: %w", err)
	}

	rowsAffected, err := result.RowsAffected()
	if err != nil {
		return fmt.Errorf("failed to get rows affected: %w", err)
	}
	if rowsAffected == 0 {
		return fmt.Errorf("grammar rule not found: %s", rule.RuleID)
	}

	return nil
}

func (r *PostgresRepository) DeleteGrammarRule(ctx context.Context, ruleID string) error {
	query := `DELETE FROM "dsl-ob-poc".grammar_rules WHERE rule_id = $1`

	result, err := r.execContext(ctx, query, ruleID)
	if err != nil {
		return fmt.Errorf("failed to delete grammar rule: %w", err)
	}

	rowsAffected, err := result.RowsAffected()
	if err != nil {
		return fmt.Errorf("failed to get rows affected: %w", err)
	}
	if rowsAffected == 0 {
		return fmt.Errorf("grammar rule not found: %s", ruleID)
	}

	return nil
}

func (r *PostgresRepository) ValidateGrammarSyntax(ctx context.Context, ruleDefinition string) error {
	// Basic EBNF syntax validation - could be enhanced with proper EBNF parser
	if len(ruleDefinition) == 0 {
		return fmt.Errorf("empty rule definition")
	}
	// TODO: Implement proper EBNF validation
	return nil
}

func (r *PostgresRepository) GetActiveGrammarForDomain(ctx context.Context, domain string) ([]*GrammarRule, error) {
	var rules []*GrammarRule
	query := `
		SELECT rule_id, rule_name, rule_definition, rule_type, domain, version,
		       active, description, created_at, updated_at
		FROM "dsl-ob-poc".grammar_rules
		WHERE (domain = $1 OR domain IS NULL) AND active = true
		ORDER BY domain NULLS LAST, rule_name`

	err := r.selectContext(ctx, &rules, query, domain)
	if err != nil {
		return nil, fmt.Errorf("failed to get active grammar for domain: %w", err)
	}
	return rules, nil
}

// =============================================================================
// Vocabulary Repository Implementation
// =============================================================================

func (r *PostgresRepository) CreateDomainVocab(ctx context.Context, vocab *DomainVocabulary) error {
	parametersJSON, err := json.Marshal(vocab.Parameters)
	if err != nil {
		return fmt.Errorf("failed to marshal parameters: %w", err)
	}

	examplesJSON, err := json.Marshal(vocab.Examples)
	if err != nil {
		return fmt.Errorf("failed to marshal examples: %w", err)
	}

	query := `
		INSERT INTO "dsl-ob-poc".domain_vocabularies
		(domain, verb, category, description, parameters, examples, phase, active, version)
		VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
		RETURNING vocab_id, created_at, updated_at`

	err = r.queryRowxContext(ctx, query,
		vocab.Domain, vocab.Verb, vocab.Category, vocab.Description,
		parametersJSON, examplesJSON, vocab.Phase, vocab.Active, vocab.Version,
	).Scan(&vocab.VocabID, &vocab.CreatedAt, &vocab.UpdatedAt)

	if err != nil {
		return fmt.Errorf("failed to create domain vocabulary: %w", err)
	}
	return nil
}

func (r *PostgresRepository) GetDomainVocab(ctx context.Context, vocabID string) (*DomainVocabulary, error) {
	var vocab DomainVocabulary
	var parametersJSON, examplesJSON []byte

	query := `
		SELECT vocab_id, domain, verb, category, description, parameters, examples,
		       phase, active, version, created_at, updated_at
		FROM "dsl-ob-poc".domain_vocabularies
		WHERE vocab_id = $1`

	err := r.queryRowxContext(ctx, query, vocabID).Scan(
		&vocab.VocabID, &vocab.Domain, &vocab.Verb, &vocab.Category, &vocab.Description,
		&parametersJSON, &examplesJSON, &vocab.Phase, &vocab.Active, &vocab.Version,
		&vocab.CreatedAt, &vocab.UpdatedAt,
	)

	if errors.Is(err, sql.ErrNoRows) {
		return nil, fmt.Errorf("domain vocabulary not found: %s", vocabID)
	}
	if err != nil {
		return nil, fmt.Errorf("failed to get domain vocabulary: %w", err)
	}

	if err := json.Unmarshal(parametersJSON, &vocab.Parameters); err != nil {
		return nil, fmt.Errorf("failed to unmarshal parameters: %w", err)
	}
	if err := json.Unmarshal(examplesJSON, &vocab.Examples); err != nil {
		return nil, fmt.Errorf("failed to unmarshal examples: %w", err)
	}

	return &vocab, nil
}

func (r *PostgresRepository) GetDomainVocabByVerb(ctx context.Context, domain, verb string) (*DomainVocabulary, error) {
	var vocab DomainVocabulary
	var parametersJSON, examplesJSON []byte

	query := `
		SELECT vocab_id, domain, verb, category, description, parameters, examples,
		       phase, active, version, created_at, updated_at
		FROM "dsl-ob-poc".domain_vocabularies
		WHERE domain = $1 AND verb = $2 AND active = true
		ORDER BY created_at DESC
		LIMIT 1`

	err := r.queryRowxContext(ctx, query, domain, verb).Scan(
		&vocab.VocabID, &vocab.Domain, &vocab.Verb, &vocab.Category, &vocab.Description,
		&parametersJSON, &examplesJSON, &vocab.Phase, &vocab.Active, &vocab.Version,
		&vocab.CreatedAt, &vocab.UpdatedAt,
	)

	if errors.Is(err, sql.ErrNoRows) {
		return nil, fmt.Errorf("domain vocabulary not found: %s.%s", domain, verb)
	}
	if err != nil {
		return nil, fmt.Errorf("failed to get domain vocabulary by verb: %w", err)
	}

	if err := json.Unmarshal(parametersJSON, &vocab.Parameters); err != nil {
		return nil, fmt.Errorf("failed to unmarshal parameters: %w", err)
	}
	if err := json.Unmarshal(examplesJSON, &vocab.Examples); err != nil {
		return nil, fmt.Errorf("failed to unmarshal examples: %w", err)
	}

	return &vocab, nil
}

func (r *PostgresRepository) ListDomainVocabs(ctx context.Context, domain *string, category *string, active *bool) ([]*DomainVocabulary, error) {
	var vocabs []*DomainVocabulary

	var args []interface{}
	var conditions []string
	paramIndex := 1

	conditions = append(conditions, "TRUE") // Always true condition to simplify logic

	if domain != nil {
		conditions = append(conditions, fmt.Sprintf("domain = $%d", paramIndex))
		args = append(args, *domain)
		paramIndex++
	}

	if category != nil {
		conditions = append(conditions, fmt.Sprintf("category = $%d", paramIndex))
		args = append(args, *category)
		paramIndex++
	}

	if active != nil {
		conditions = append(conditions, fmt.Sprintf("active = $%d", paramIndex))
		args = append(args, *active)
		paramIndex++
	}

	whereClause := strings.Join(conditions, " AND ")

	query := fmt.Sprintf(`
		SELECT vocab_id, domain, verb, category, description, parameters, examples,
		       phase, active, version, created_at, updated_at
		FROM "dsl-ob-poc".domain_vocabularies
		WHERE %s
		ORDER BY domain, category, verb`, whereClause)

	rows, err := r.queryxContext(ctx, query, args...)
	if err != nil {
		return nil, fmt.Errorf("failed to list domain vocabularies: %w", err)
	}
	defer rows.Close()

	for rows.Next() {
		var vocab DomainVocabulary
		var parametersJSON, examplesJSON []byte

		err := rows.Scan(
			&vocab.VocabID, &vocab.Domain, &vocab.Verb, &vocab.Category, &vocab.Description,
			&parametersJSON, &examplesJSON, &vocab.Phase, &vocab.Active, &vocab.Version,
			&vocab.CreatedAt, &vocab.UpdatedAt,
		)
		if err != nil {
			return nil, fmt.Errorf("failed to scan vocabulary row: %w", err)
		}

		if err := json.Unmarshal(parametersJSON, &vocab.Parameters); err != nil {
			return nil, fmt.Errorf("failed to unmarshal parameters: %w", err)
		}
		if err := json.Unmarshal(examplesJSON, &vocab.Examples); err != nil {
			return nil, fmt.Errorf("failed to unmarshal examples: %w", err)
		}

		vocabs = append(vocabs, &vocab)
	}

	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("error iterating vocabulary rows: %w", err)
	}

	return vocabs, nil
}

func (r *PostgresRepository) UpdateDomainVocab(ctx context.Context, vocab *DomainVocabulary) error {
	parametersJSON, err := json.Marshal(vocab.Parameters)
	if err != nil {
		return fmt.Errorf("failed to marshal parameters: %w", err)
	}

	examplesJSON, err := json.Marshal(vocab.Examples)
	if err != nil {
		return fmt.Errorf("failed to marshal examples: %w", err)
	}

	query := `
		UPDATE "dsl-ob-poc".domain_vocabularies
		SET category = $2, description = $3, parameters = $4, examples = $5,
		    phase = $6, active = $7, version = $8, updated_at = now()
		WHERE vocab_id = $1`

	result, err := r.execContext(ctx, query,
		vocab.VocabID, vocab.Category, vocab.Description, parametersJSON,
		examplesJSON, vocab.Phase, vocab.Active, vocab.Version)

	if err != nil {
		return fmt.Errorf("failed to update domain vocabulary: %w", err)
	}

	rowsAffected, err := result.RowsAffected()
	if err != nil {
		return fmt.Errorf("failed to get rows affected: %w", err)
	}
	if rowsAffected == 0 {
		return fmt.Errorf("domain vocabulary not found: %s", vocab.VocabID)
	}

	return nil
}

func (r *PostgresRepository) DeleteDomainVocab(ctx context.Context, vocabID string) error {
	query := `DELETE FROM "dsl-ob-poc".domain_vocabularies WHERE vocab_id = $1`

	result, err := r.execContext(ctx, query, vocabID)
	if err != nil {
		return fmt.Errorf("failed to delete domain vocabulary: %w", err)
	}

	rowsAffected, err := result.RowsAffected()
	if err != nil {
		return fmt.Errorf("failed to get rows affected: %w", err)
	}
	if rowsAffected == 0 {
		return fmt.Errorf("domain vocabulary not found: %s", vocabID)
	}

	return nil
}

func (r *PostgresRepository) ValidateVerb(ctx context.Context, domain, verb string) error {
	var exists bool
	query := `
		SELECT EXISTS(
			SELECT 1 FROM "dsl-ob-poc".domain_vocabularies
			WHERE domain = $1 AND verb = $2 AND active = true
		)`

	err := r.getContext(ctx, &exists, query, domain, verb)
	if err != nil {
		return fmt.Errorf("failed to validate verb: %w", err)
	}

	if !exists {
		return fmt.Errorf("invalid verb %s for domain %s", verb, domain)
	}

	return nil
}

func (r *PostgresRepository) GetApprovedVerbs(ctx context.Context, domain string) ([]string, error) {
	var verbs []string
	query := `
		SELECT verb FROM "dsl-ob-poc".domain_vocabularies
		WHERE domain = $1 AND active = true
		ORDER BY verb`

	err := r.selectContext(ctx, &verbs, query, domain)
	if err != nil {
		return nil, fmt.Errorf("failed to get approved verbs: %w", err)
	}

	return verbs, nil
}

func (r *PostgresRepository) GetAllApprovedVerbs(ctx context.Context) (map[string][]string, error) {
	type VerbRow struct {
		Domain string `db:"domain"`
		Verb   string `db:"verb"`
	}

	var rows []VerbRow
	query := `
		SELECT domain, verb FROM "dsl-ob-poc".domain_vocabularies
		WHERE active = true
		ORDER BY domain, verb`

	err := r.selectContext(ctx, &rows, query)
	if err != nil {
		return nil, fmt.Errorf("failed to get all approved verbs: %w", err)
	}

	result := make(map[string][]string)
	for _, row := range rows {
		result[row.Domain] = append(result[row.Domain], row.Verb)
	}

	return result, nil
}

// =============================================================================
// Verb Registry Repository Implementation
// =============================================================================

func (r *PostgresRepository) RegisterVerb(ctx context.Context, registry *VerbRegistry) error {
	query := `
		INSERT INTO "dsl-ob-poc".verb_registry
		(verb, primary_domain, shared, deprecated, replacement_verb, description)
		VALUES ($1, $2, $3, $4, $5, $6)
		ON CONFLICT (verb) DO UPDATE SET
			primary_domain = EXCLUDED.primary_domain,
			shared = EXCLUDED.shared,
			deprecated = EXCLUDED.deprecated,
			replacement_verb = EXCLUDED.replacement_verb,
			description = EXCLUDED.description,
			updated_at = now()
		RETURNING created_at, updated_at`

	err := r.queryRowxContext(ctx, query,
		registry.Verb, registry.PrimaryDomain, registry.Shared,
		registry.Deprecated, registry.ReplacementVerb, registry.Description,
	).Scan(&registry.CreatedAt, &registry.UpdatedAt)

	if err != nil {
		return fmt.Errorf("failed to register verb: %w", err)
	}
	return nil
}

func (r *PostgresRepository) GetVerbRegistry(ctx context.Context, verb string) (*VerbRegistry, error) {
	var registry VerbRegistry
	query := `
		SELECT verb, primary_domain, shared, deprecated, replacement_verb,
		       description, created_at, updated_at
		FROM "dsl-ob-poc".verb_registry
		WHERE verb = $1`

	err := r.getContext(ctx, &registry, query, verb)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, fmt.Errorf("verb registry not found: %s", verb)
	}
	if err != nil {
		return nil, fmt.Errorf("failed to get verb registry: %w", err)
	}
	return &registry, nil
}

func (r *PostgresRepository) ListVerbRegistry(ctx context.Context, domain *string, shared *bool, deprecated *bool) ([]*VerbRegistry, error) {
	var registries []*VerbRegistry

	var args []interface{}
	var conditions []string
	paramIndex := 1

	conditions = append(conditions, "TRUE")

	if domain != nil {
		conditions = append(conditions, fmt.Sprintf("primary_domain = $%d", paramIndex))
		args = append(args, *domain)
		paramIndex++
	}

	if shared != nil {
		conditions = append(conditions, fmt.Sprintf("shared = $%d", paramIndex))
		args = append(args, *shared)
		paramIndex++
	}

	if deprecated != nil {
		conditions = append(conditions, fmt.Sprintf("deprecated = $%d", paramIndex))
		args = append(args, *deprecated)
		paramIndex++
	}

	whereClause := strings.Join(conditions, " AND ")

	query := fmt.Sprintf(`
		SELECT verb, primary_domain, shared, deprecated, replacement_verb,
		       description, created_at, updated_at
		FROM "dsl-ob-poc".verb_registry
		WHERE %s
		ORDER BY verb`, whereClause)

	err := r.selectContext(ctx, &registries, query, args...)
	if err != nil {
		return nil, fmt.Errorf("failed to list verb registry: %w", err)
	}
	return registries, nil
}

func (r *PostgresRepository) UpdateVerbRegistry(ctx context.Context, registry *VerbRegistry) error {
	query := `
		UPDATE "dsl-ob-poc".verb_registry
		SET primary_domain = $2, shared = $3, deprecated = $4,
		    replacement_verb = $5, description = $6, updated_at = now()
		WHERE verb = $1`

	result, err := r.execContext(ctx, query,
		registry.Verb, registry.PrimaryDomain, registry.Shared,
		registry.Deprecated, registry.ReplacementVerb, registry.Description)

	if err != nil {
		return fmt.Errorf("failed to update verb registry: %w", err)
	}

	rowsAffected, err := result.RowsAffected()
	if err != nil {
		return fmt.Errorf("failed to get rows affected: %w", err)
	}
	if rowsAffected == 0 {
		return fmt.Errorf("verb registry not found: %s", registry.Verb)
	}

	return nil
}

func (r *PostgresRepository) DeprecateVerb(ctx context.Context, verb, replacementVerb string, reason string) error {
	query := `
		UPDATE "dsl-ob-poc".verb_registry
		SET deprecated = true, replacement_verb = $2, updated_at = now()
		WHERE verb = $1`

	result, err := r.execContext(ctx, query, verb, replacementVerb)
	if err != nil {
		return fmt.Errorf("failed to deprecate verb: %w", err)
	}

	rowsAffected, err := result.RowsAffected()
	if err != nil {
		return fmt.Errorf("failed to get rows affected: %w", err)
	}
	if rowsAffected == 0 {
		return fmt.Errorf("verb registry not found: %s", verb)
	}

	// Create audit record
	audit := &VocabularyAudit{
		Domain:     "global", // Use global for registry changes
		Verb:       verb,
		ChangeType: "DEPRECATE",
		OldDefinition: map[string]interface{}{
			"deprecated": false,
		},
		NewDefinition: map[string]interface{}{
			"deprecated":       true,
			"replacement_verb": replacementVerb,
		},
		ChangeReason: &reason,
		CreatedAt:    time.Now(),
	}

	return r.CreateAudit(ctx, audit)
}

func (r *PostgresRepository) CheckVerbConflicts(ctx context.Context, verb string, domains []string) ([]*VerbRegistry, error) {
	if len(domains) == 0 {
		return nil, nil
	}

	var registries []*VerbRegistry
	query := `
		SELECT verb, primary_domain, shared, deprecated, replacement_verb,
		       description, created_at, updated_at
		FROM "dsl-ob-poc".verb_registry
		WHERE verb = $1 AND primary_domain = ANY($2) AND NOT shared`

	err := r.selectContext(ctx, &registries, query, verb, pq.Array(domains))
	if err != nil {
		return nil, fmt.Errorf("failed to check verb conflicts: %w", err)
	}
	return registries, nil
}

func (r *PostgresRepository) GetSharedVerbs(ctx context.Context) ([]*VerbRegistry, error) {
	var registries []*VerbRegistry
	query := `
		SELECT verb, primary_domain, shared, deprecated, replacement_verb,
		       description, created_at, updated_at
		FROM "dsl-ob-poc".verb_registry
		WHERE shared = true AND NOT deprecated
		ORDER BY verb`

	err := r.selectContext(ctx, &registries, query)
	if err != nil {
		return nil, fmt.Errorf("failed to get shared verbs: %w", err)
	}
	return registries, nil
}

// =============================================================================
// Audit Repository Implementation
// =============================================================================

func (r *PostgresRepository) CreateAudit(ctx context.Context, audit *VocabularyAudit) error {
	oldDefJSON, err := json.Marshal(audit.OldDefinition)
	if err != nil {
		return fmt.Errorf("failed to marshal old definition: %w", err)
	}

	newDefJSON, err := json.Marshal(audit.NewDefinition)
	if err != nil {
		return fmt.Errorf("failed to marshal new definition: %w", err)
	}

	query := `
		INSERT INTO "dsl-ob-poc".vocabulary_audit
		(domain, verb, change_type, old_definition, new_definition, changed_by, change_reason)
		VALUES ($1, $2, $3, $4, $5, $6, $7)
		RETURNING audit_id, created_at`

	err = r.queryRowxContext(ctx, query,
		audit.Domain, audit.Verb, audit.ChangeType, oldDefJSON, newDefJSON,
		audit.ChangedBy, audit.ChangeReason,
	).Scan(&audit.AuditID, &audit.CreatedAt)

	if err != nil {
		return fmt.Errorf("failed to create audit record: %w", err)
	}
	return nil
}

func (r *PostgresRepository) GetAuditHistory(ctx context.Context, domain, verb string, limit int) ([]*VocabularyAudit, error) {
	var audits []*VocabularyAudit

	query := `
		SELECT audit_id, domain, verb, change_type, old_definition, new_definition,
		       changed_by, change_reason, created_at
		FROM "dsl-ob-poc".vocabulary_audit
		WHERE domain = $1 AND verb = $2
		ORDER BY created_at DESC
		LIMIT $3`

	rows, err := r.queryxContext(ctx, query, domain, verb, limit)
	if err != nil {
		return nil, fmt.Errorf("failed to get audit history: %w", err)
	}
	defer rows.Close()

	for rows.Next() {
		var audit VocabularyAudit
		var oldDefJSON, newDefJSON []byte

		err := rows.Scan(
			&audit.AuditID, &audit.Domain, &audit.Verb, &audit.ChangeType,
			&oldDefJSON, &newDefJSON, &audit.ChangedBy, &audit.ChangeReason,
			&audit.CreatedAt,
		)
		if err != nil {
			return nil, fmt.Errorf("failed to scan audit row: %w", err)
		}

		if err := json.Unmarshal(oldDefJSON, &audit.OldDefinition); err != nil {
			return nil, fmt.Errorf("failed to unmarshal old definition: %w", err)
		}
		if err := json.Unmarshal(newDefJSON, &audit.NewDefinition); err != nil {
			return nil, fmt.Errorf("failed to unmarshal new definition: %w", err)
		}

		audits = append(audits, &audit)
	}

	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("error iterating audit rows: %w", err)
	}

	return audits, nil
}

func (r *PostgresRepository) GetRecentChanges(ctx context.Context, domain *string, changeType *string, limit int) ([]*VocabularyAudit, error) {
	var audits []*VocabularyAudit

	var args []interface{}
	var conditions []string
	paramIndex := 1

	conditions = append(conditions, "TRUE")

	if domain != nil {
		conditions = append(conditions, fmt.Sprintf("domain = $%d", paramIndex))
		args = append(args, *domain)
		paramIndex++
	}

	if changeType != nil {
		conditions = append(conditions, fmt.Sprintf("change_type = $%d", paramIndex))
		args = append(args, *changeType)
		paramIndex++
	}

	whereClause := strings.Join(conditions, " AND ")

	query := fmt.Sprintf(`
		SELECT audit_id, domain, verb, change_type, old_definition, new_definition,
		       changed_by, change_reason, created_at
		FROM "dsl-ob-poc".vocabulary_audit
		WHERE %s
		ORDER BY created_at DESC
		LIMIT $%d`, whereClause, paramIndex)

	args = append(args, limit)

	rows, err := r.queryxContext(ctx, query, args...)
	if err != nil {
		return nil, fmt.Errorf("failed to get recent changes: %w", err)
	}
	defer rows.Close()

	for rows.Next() {
		var audit VocabularyAudit
		var oldDefJSON, newDefJSON []byte

		err := rows.Scan(
			&audit.AuditID, &audit.Domain, &audit.Verb, &audit.ChangeType,
			&oldDefJSON, &newDefJSON, &audit.ChangedBy, &audit.ChangeReason,
			&audit.CreatedAt,
		)
		if err != nil {
			return nil, fmt.Errorf("failed to scan audit row: %w", err)
		}

		if err := json.Unmarshal(oldDefJSON, &audit.OldDefinition); err != nil {
			return nil, fmt.Errorf("failed to unmarshal old definition: %w", err)
		}
		if err := json.Unmarshal(newDefJSON, &audit.NewDefinition); err != nil {
			return nil, fmt.Errorf("failed to unmarshal new definition: %w", err)
		}

		audits = append(audits, &audit)
	}

	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("error iterating audit rows: %w", err)
	}

	return audits, nil
}

func (r *PostgresRepository) GetChangesSince(ctx context.Context, since time.Time, domain *string) ([]*VocabularyAudit, error) {
	var audits []*VocabularyAudit

	var args []interface{}
	var conditions []string
	paramIndex := 1

	conditions = append(conditions, fmt.Sprintf("created_at >= $%d", paramIndex))
	args = append(args, since)
	paramIndex++

	if domain != nil {
		conditions = append(conditions, fmt.Sprintf("domain = $%d", paramIndex))
		args = append(args, *domain)
		paramIndex++
	}

	whereClause := strings.Join(conditions, " AND ")

	query := fmt.Sprintf(`
		SELECT audit_id, domain, verb, change_type, old_definition, new_definition,
		       changed_by, change_reason, created_at
		FROM "dsl-ob-poc".vocabulary_audit
		WHERE %s
		ORDER BY created_at DESC`, whereClause)

	rows, err := r.queryxContext(ctx, query, args...)
	if err != nil {
		return nil, fmt.Errorf("failed to get changes since: %w", err)
	}
	defer rows.Close()

	for rows.Next() {
		var audit VocabularyAudit
		var oldDefJSON, newDefJSON []byte

		err := rows.Scan(
			&audit.AuditID, &audit.Domain, &audit.Verb, &audit.ChangeType,
			&oldDefJSON, &newDefJSON, &audit.ChangedBy, &audit.ChangeReason,
			&audit.CreatedAt,
		)
		if err != nil {
			return nil, fmt.Errorf("failed to scan audit row: %w", err)
		}

		if err := json.Unmarshal(oldDefJSON, &audit.OldDefinition); err != nil {
			return nil, fmt.Errorf("failed to unmarshal old definition: %w", err)
		}
		if err := json.Unmarshal(newDefJSON, &audit.NewDefinition); err != nil {
			return nil, fmt.Errorf("failed to unmarshal new definition: %w", err)
		}

		audits = append(audits, &audit)
	}

	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("error iterating audit rows: %w", err)
	}

	return audits, nil
}

func (r *PostgresRepository) GetChangeSummary(ctx context.Context, domain string, period time.Duration) (*ChangeSummary, error) {
	since := time.Now().Add(-period)

	var summary ChangeSummary
	summary.Domain = domain
	summary.Period = period.String()
	summary.ChangesByType = make(map[string]int)
	summary.ChangedBy = make(map[string]int)

	// Get all changes for the domain in the period
	audits, err := r.GetChangesSince(ctx, since, &domain)
	if err != nil {
		return nil, fmt.Errorf("failed to get changes for summary: %w", err)
	}

	summary.TotalChanges = len(audits)

	for _, audit := range audits {
		// Count by change type
		summary.ChangesByType[audit.ChangeType]++

		// Count by user
		if audit.ChangedBy != nil {
			summary.ChangedBy[*audit.ChangedBy]++
		}

		// Categorize verbs by change type
		switch audit.ChangeType {
		case "CREATE":
			summary.NewVerbs = append(summary.NewVerbs, audit.Verb)
		case "UPDATE":
			summary.UpdatedVerbs = append(summary.UpdatedVerbs, audit.Verb)
		case "DEPRECATE", "DELETE":
			summary.DeprecatedVerbs = append(summary.DeprecatedVerbs, audit.Verb)
		}
	}

	return &summary, nil
}
