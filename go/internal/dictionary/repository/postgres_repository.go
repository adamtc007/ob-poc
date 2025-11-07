package repository

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"strings"

	"github.com/google/uuid"
	_ "github.com/lib/pq"

	"dsl-ob-poc/internal/dictionary"
)

type postgresDictionaryRepository struct {
	db     *sql.DB
	config *repositoryConfig
}

func newPostgresDictionaryRepository(
	connectionString string,
	config *repositoryConfig,
) (DictionaryRepository, error) {
	db, err := sql.Open("postgres", connectionString)
	if err != nil {
		return nil, fmt.Errorf("failed to connect to database: %w", err)
	}

	if err = db.Ping(); err != nil {
		_ = db.Close()
		return nil, fmt.Errorf("failed to ping database: %w", err)
	}

	// Set connection pool settings
	db.SetMaxOpenConns(config.maxConnections)
	db.SetMaxIdleConns(config.maxConnections / 2)

	return &postgresDictionaryRepository{
		db:     db,
		config: config,
	}, nil
}

func (r *postgresDictionaryRepository) Create(
	ctx context.Context,
	attr *dictionary.Attribute,
) error {
	// Generate UUID if not provided
	if attr.AttributeID == "" {
		attr.AttributeID = uuid.New().String()
	}

	// Serialize JSON metadata
	sourceJSON, err := json.Marshal(attr.Source)
	if err != nil {
		return fmt.Errorf("failed to marshal source metadata: %w", err)
	}

	sinkJSON, err := json.Marshal(attr.Sink)
	if err != nil {
		return fmt.Errorf("failed to marshal sink metadata: %w", err)
	}

	query := `
		INSERT INTO "dsl-ob-poc".dictionary (
			attribute_id, name, long_description, group_id,
			mask, domain, vector, source, sink
		) VALUES (
			$1, $2, $3, $4, $5, $6, $7, $8, $9
		)
	`

	if r.config.logQueries {
		fmt.Printf("SQL: %s\nArgs: %v\n", query, []interface{}{
			attr.AttributeID, attr.Name, attr.LongDescription, attr.GroupID,
			attr.Mask, attr.Domain, attr.Vector, string(sourceJSON), string(sinkJSON),
		})
	}

	_, err = r.db.ExecContext(ctx, query,
		attr.AttributeID,
		attr.Name,
		attr.LongDescription,
		attr.GroupID,
		attr.Mask,
		attr.Domain,
		attr.Vector,
		sourceJSON,
		sinkJSON,
	)

	if err != nil {
		return fmt.Errorf("failed to create dictionary attribute: %w", err)
	}

	return nil
}

func (r *postgresDictionaryRepository) GetByID(
	ctx context.Context,
	attributeID string,
) (*dictionary.Attribute, error) {
	return r.getAttribute(ctx, "attribute_id = $1", attributeID,
		"no attribute found with ID %s")
}

func (r *postgresDictionaryRepository) GetByName(
	ctx context.Context,
	name string,
) (*dictionary.Attribute, error) {
	return r.getAttribute(ctx, "name = $1", name,
		"no attribute found with name %s")
}

func (r *postgresDictionaryRepository) Update(
	ctx context.Context,
	attr *dictionary.Attribute,
) error {
	// Serialize JSON metadata
	sourceJSON, err := json.Marshal(attr.Source)
	if err != nil {
		return fmt.Errorf("failed to marshal source metadata: %w", err)
	}

	sinkJSON, err := json.Marshal(attr.Sink)
	if err != nil {
		return fmt.Errorf("failed to marshal sink metadata: %w", err)
	}

	query := `
		UPDATE "dsl-ob-poc".dictionary SET
			name = $2,
			long_description = $3,
			group_id = $4,
			mask = $5,
			domain = $6,
			vector = $7,
			source = $8,
			sink = $9,
			updated_at = (now() at time zone 'utc')
		WHERE attribute_id = $1
	`

	if r.config.logQueries {
		fmt.Printf("SQL: %s\nArgs: %v\n", query, []interface{}{
			attr.AttributeID, attr.Name, attr.LongDescription, attr.GroupID,
			attr.Mask, attr.Domain, attr.Vector, string(sourceJSON), string(sinkJSON),
		})
	}

	result, err := r.db.ExecContext(ctx, query,
		attr.AttributeID,
		attr.Name,
		attr.LongDescription,
		attr.GroupID,
		attr.Mask,
		attr.Domain,
		attr.Vector,
		sourceJSON,
		sinkJSON,
	)

	if err != nil {
		return fmt.Errorf("failed to update attribute: %w", err)
	}

	rowsAffected, err := result.RowsAffected()
	if err != nil {
		return fmt.Errorf("error checking update result: %w", err)
	}

	if rowsAffected == 0 {
		return fmt.Errorf("no attribute found with ID %s", attr.AttributeID)
	}

	return nil
}

func (r *postgresDictionaryRepository) Delete(
	ctx context.Context,
	attributeID string,
) error {
	query := `DELETE FROM "dsl-ob-poc".dictionary WHERE attribute_id = $1`

	if r.config.logQueries {
		fmt.Printf("SQL: %s\nArgs: %v\n", query, []interface{}{attributeID})
	}

	result, err := r.db.ExecContext(ctx, query, attributeID)
	if err != nil {
		return fmt.Errorf("failed to delete attribute: %w", err)
	}

	rowsAffected, err := result.RowsAffected()
	if err != nil {
		return fmt.Errorf("error checking delete result: %w", err)
	}

	if rowsAffected == 0 {
		return fmt.Errorf("no attribute found with ID %s", attributeID)
	}

	return nil
}

func (r *postgresDictionaryRepository) List(
	ctx context.Context,
	opts *ListOptions,
) ([]dictionary.Attribute, error) {
	if opts == nil {
		opts = &ListOptions{Limit: 100}
	}

	var conditions []string
	var args []interface{}
	argIndex := 1

	if opts.Domain != "" {
		conditions = append(conditions, fmt.Sprintf("domain = $%d", argIndex))
		args = append(args, opts.Domain)
		argIndex++
	}

	if opts.GroupID != "" {
		conditions = append(conditions, fmt.Sprintf("group_id = $%d", argIndex))
		args = append(args, opts.GroupID)
		argIndex++
	}

	// Note: Tags and Sensitivity filtering removed as they're not in our current schema
	// These can be re-added when the schema is extended

	whereClause := ""
	if len(conditions) > 0 {
		whereClause = "WHERE " + strings.Join(conditions, " AND ")
	}

	// Add default limit and offset
	if opts.Limit == 0 {
		opts.Limit = 100
	}
	args = append(args, opts.Limit, opts.Offset)

	query := fmt.Sprintf(`
		SELECT
			attribute_id, name, long_description, group_id,
			mask, domain, COALESCE(vector, ''),
			COALESCE(source::text, '{}'), COALESCE(sink::text, '{}')
		FROM "dsl-ob-poc".dictionary
		%s
		ORDER BY name
		LIMIT $%d OFFSET $%d
	`, whereClause, argIndex, argIndex+1)

	if r.config.logQueries {
		fmt.Printf("SQL: %s\nArgs: %v\n", query, args)
	}

	rows, err := r.db.QueryContext(ctx, query, args...)
	if err != nil {
		return nil, fmt.Errorf("failed to list attributes: %w", err)
	}
	defer rows.Close()

	var attributes []dictionary.Attribute
	for rows.Next() {
		var attr dictionary.Attribute
		var sourceJSON, sinkJSON string

		scanErr := rows.Scan(
			&attr.AttributeID,
			&attr.Name,
			&attr.LongDescription,
			&attr.GroupID,
			&attr.Mask,
			&attr.Domain,
			&attr.Vector,
			&sourceJSON,
			&sinkJSON,
		)

		if scanErr != nil {
			return nil, fmt.Errorf("failed to scan attribute: %w", scanErr)
		}

		// Deserialize JSON fields
		if parseErr := json.Unmarshal([]byte(sourceJSON), &attr.Source); parseErr != nil {
			return nil, fmt.Errorf("failed to parse source metadata: %w", parseErr)
		}
		if parseErr := json.Unmarshal([]byte(sinkJSON), &attr.Sink); parseErr != nil {
			return nil, fmt.Errorf("failed to parse sink metadata: %w", parseErr)
		}

		attributes = append(attributes, attr)
	}

	if err = rows.Err(); err != nil {
		return nil, fmt.Errorf("error iterating over rows: %w", err)
	}

	return attributes, nil
}

func (r *postgresDictionaryRepository) Count(
	ctx context.Context,
	opts *ListOptions,
) (int, error) {
	if opts == nil {
		opts = &ListOptions{}
	}

	var conditions []string
	var args []interface{}
	argIndex := 1

	if opts.Domain != "" {
		conditions = append(conditions, fmt.Sprintf("domain = $%d", argIndex))
		args = append(args, opts.Domain)
		argIndex++
	}

	if opts.GroupID != "" {
		conditions = append(conditions, fmt.Sprintf("group_id = $%d", argIndex))
		args = append(args, opts.GroupID)
		// argIndex++ removed as it's ineffectual (not used after increment)
	}

	// Note: Tags and Sensitivity filtering removed as they're not in our current schema

	whereClause := ""
	if len(conditions) > 0 {
		whereClause = "WHERE " + strings.Join(conditions, " AND ")
	}

	query := fmt.Sprintf(`SELECT COUNT(*) FROM "dsl-ob-poc".dictionary %s`, whereClause)

	if r.config.logQueries {
		fmt.Printf("SQL: %s\nArgs: %v\n", query, args)
	}

	var count int
	err := r.db.QueryRowContext(ctx, query, args...).Scan(&count)
	if err != nil {
		return 0, fmt.Errorf("failed to count attributes: %w", err)
	}

	return count, nil
}

// getAttribute is a helper function to reduce code duplication for GetByID and GetByName
func (r *postgresDictionaryRepository) getAttribute(
	ctx context.Context,
	whereClause string,
	param interface{},
	notFoundMsg string,
) (*dictionary.Attribute, error) {
	query := `
		SELECT
			attribute_id, name, long_description, group_id,
			mask, domain, COALESCE(vector, ''),
			COALESCE(source::text, '{}'), COALESCE(sink::text, '{}')
		FROM "dsl-ob-poc".dictionary
		WHERE ` + whereClause

	if r.config.logQueries {
		fmt.Printf("SQL: %s\nArgs: %v\n", query, []interface{}{param})
	}

	var (
		attr                 dictionary.Attribute
		sourceJSON, sinkJSON string
	)

	err := r.db.QueryRowContext(ctx, query, param).Scan(
		&attr.AttributeID,
		&attr.Name,
		&attr.LongDescription,
		&attr.GroupID,
		&attr.Mask,
		&attr.Domain,
		&attr.Vector,
		&sourceJSON,
		&sinkJSON,
	)

	if err == sql.ErrNoRows {
		return nil, fmt.Errorf(notFoundMsg, param)
	}
	if err != nil {
		return nil, fmt.Errorf("failed to retrieve attribute: %w", err)
	}

	// Deserialize JSON fields
	if parseErr := json.Unmarshal([]byte(sourceJSON), &attr.Source); parseErr != nil {
		return nil, fmt.Errorf("failed to parse source metadata: %w", parseErr)
	}
	if parseErr := json.Unmarshal([]byte(sinkJSON), &attr.Sink); parseErr != nil {
		return nil, fmt.Errorf("failed to parse sink metadata: %w", parseErr)
	}

	return &attr, nil
}

func (r *postgresDictionaryRepository) Close() error {
	if r.db != nil {
		return r.db.Close()
	}
	return nil
}
