package repository

import (
	"context"
	"errors"

	"dsl-ob-poc/internal/dictionary"
)

// Common errors
var (
	ErrAttributeNotFound = errors.New("attribute not found")
)

// ListOptions provides flexible filtering for dictionary attributes
type ListOptions struct {
	Domain      string
	GroupID     string
	Tags        []string
	Sensitivity string
	Offset      int
	Limit       int
}

// DictionaryRepository defines the contract for managing dictionary attributes
type DictionaryRepository interface {
	// Create adds a new attribute to the dictionary
	Create(ctx context.Context, attribute *dictionary.Attribute) error

	// GetByID retrieves an attribute by its unique identifier
	GetByID(ctx context.Context, attributeID string) (*dictionary.Attribute, error)

	// GetByName retrieves an attribute by its name
	GetByName(ctx context.Context, name string) (*dictionary.Attribute, error)

	// Update modifies an existing dictionary attribute
	Update(ctx context.Context, attribute *dictionary.Attribute) error

	// Delete removes an attribute from the dictionary
	Delete(ctx context.Context, attributeID string) error

	// List retrieves attributes with optional filtering
	List(ctx context.Context, opts *ListOptions) ([]dictionary.Attribute, error)

	// Count returns the total number of attributes matching optional filters
	Count(ctx context.Context, opts *ListOptions) (int, error)

	// Close releases any resources used by the repository
	Close() error
}

// RepositoryOption allows for optional configuration of the repository
type Option func(*repositoryConfig)

// repositoryConfig holds configuration for the dictionary repository
type repositoryConfig struct {
	enableCache    bool
	logQueries     bool
	cacheExpiry    int
	maxConnections int
}

// NewDictionaryRepository creates a new dictionary repository
func NewDictionaryRepository(
	connectionString string,
	opts ...Option,
) (DictionaryRepository, error) {
	// Default configuration
	config := &repositoryConfig{
		enableCache:    false,
		logQueries:     false,
		cacheExpiry:    300, // 5 minutes
		maxConnections: 10,
	}

	// Apply provided options
	for _, opt := range opts {
		opt(config)
	}

	// Deprecated: DB-backed repository is removed; use Rust backend or mocks
	return nil, errors.New("PostgreSQL dictionary repository is deprecated; use Rust gRPC backend or mocks")
}

// WithCaching enables result caching for repository queries
func WithCaching() Option {
	return func(cfg *repositoryConfig) {
		cfg.enableCache = true
	}
}

// WithQueryLogging enables SQL query logging
func WithQueryLogging() Option {
	return func(cfg *repositoryConfig) {
		cfg.logQueries = true
	}
}

// WithCacheExpiry sets the cache expiration time in seconds
func WithCacheExpiry(seconds int) Option {
	return func(cfg *repositoryConfig) {
		cfg.cacheExpiry = seconds
	}
}

// WithMaxConnections sets the maximum number of database connections
func WithMaxConnections(maxConns int) Option {
	return func(cfg *repositoryConfig) {
		cfg.maxConnections = maxConns
	}
}
