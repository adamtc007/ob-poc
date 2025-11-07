package repository

import (
	"context"
	"database/sql"
	"fmt"
	"os"
	"testing"
	"time"

	"github.com/google/uuid"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	"dsl-ob-poc/internal/dictionary"
)

func setupTestDB(t *testing.T) (*sql.DB, func()) {
	connString := os.Getenv("TEST_DB_CONN_STRING")
	if connString == "" {
		t.Skip("TEST_DB_CONN_STRING not set")
	}

	db, err := sql.Open("postgres", connString)
	require.NoError(t, err, "Failed to connect to test database")

	// Ensure connection
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	require.NoError(t, db.PingContext(ctx), "Failed to ping test database")

	// Clean up existing data
	_, err = db.Exec(`DELETE FROM "dsl-ob-poc".dictionary`)
	require.NoError(t, err, "Failed to clear dictionary table")

	cleanup := func() {
		_, err := db.Exec(`DELETE FROM "dsl-ob-poc".dictionary`)
		if err != nil {
			t.Logf("Warning: Failed to clean up test database: %v", err)
		}
		db.Close()
	}

	return db, cleanup
}

func createTestAttribute() *dictionary.Attribute {
	return &dictionary.Attribute{
		AttributeID:     uuid.New().String(),
		Name:            fmt.Sprintf("test_attr_%s", uuid.New().String()),
		LongDescription: "Test attribute for repository testing",
		GroupID:         "test_group",
		Mask:            "STRING",
		Domain:          "TEST",
		Source: dictionary.SourceMetadata{
			Primary: "test_source",
		},
		Sink: dictionary.SinkMetadata{
			Primary: "test_sink",
		},
		Constraints: []string{"REQUIRED"},
		Tags:        []string{"TEST_TAG"},
		Sensitivity: "LOW",
	}
}

func TestPostgresDictionaryRepository_Create(t *testing.T) {
	_, cleanup := setupTestDB(t)
	defer cleanup()

	repo, err := newPostgresDictionaryRepository(
		os.Getenv("TEST_DB_CONN_STRING"),
		&repositoryConfig{},
	)
	require.NoError(t, err)

	ctx := context.Background()
	attr := createTestAttribute()

	// Test successful creation
	err = repo.Create(ctx, attr)
	assert.NoError(t, err, "Failed to create attribute")

	// Test duplicate creation (same UUID) should fail
	err = repo.Create(ctx, attr)
	assert.Error(t, err, "Expected error creating duplicate attribute")
}

func TestPostgresDictionaryRepository_GetByID(t *testing.T) {
	_, cleanup := setupTestDB(t)
	defer cleanup()

	repo, err := newPostgresDictionaryRepository(
		os.Getenv("TEST_DB_CONN_STRING"),
		&repositoryConfig{},
	)
	require.NoError(t, err)

	ctx := context.Background()
	originalAttr := createTestAttribute()

	// Create attribute
	err = repo.Create(ctx, originalAttr)
	require.NoError(t, err)

	// Retrieve by ID
	retrievedAttr, err := repo.GetByID(ctx, originalAttr.AttributeID)
	assert.NoError(t, err)
	assert.NotNil(t, retrievedAttr)

	// Verify retrieved attribute matches original
	assert.Equal(t, originalAttr.AttributeID, retrievedAttr.AttributeID)
	assert.Equal(t, originalAttr.Name, retrievedAttr.Name)
	assert.Equal(t, originalAttr.Domain, retrievedAttr.Domain)
	assert.Equal(t, originalAttr.Constraints, retrievedAttr.Constraints)
}

func TestPostgresDictionaryRepository_GetByName(t *testing.T) {
	_, cleanup := setupTestDB(t)
	defer cleanup()

	repo, err := newPostgresDictionaryRepository(
		os.Getenv("TEST_DB_CONN_STRING"),
		&repositoryConfig{},
	)
	require.NoError(t, err)

	ctx := context.Background()
	originalAttr := createTestAttribute()

	// Create attribute
	err = repo.Create(ctx, originalAttr)
	require.NoError(t, err)

	// Retrieve by Name
	retrievedAttr, err := repo.GetByName(ctx, originalAttr.Name)
	assert.NoError(t, err)
	assert.NotNil(t, retrievedAttr)

	// Verify retrieved attribute matches original
	assert.Equal(t, originalAttr.AttributeID, retrievedAttr.AttributeID)
	assert.Equal(t, originalAttr.Name, retrievedAttr.Name)
	assert.Equal(t, originalAttr.Domain, retrievedAttr.Domain)
}

func TestPostgresDictionaryRepository_Update(t *testing.T) {
	_, cleanup := setupTestDB(t)
	defer cleanup()

	repo, err := newPostgresDictionaryRepository(
		os.Getenv("TEST_DB_CONN_STRING"),
		&repositoryConfig{},
	)
	require.NoError(t, err)

	ctx := context.Background()
	originalAttr := createTestAttribute()

	// Create attribute
	err = repo.Create(ctx, originalAttr)
	require.NoError(t, err)

	// Update attribute
	originalAttr.LongDescription = "Updated description"
	originalAttr.Constraints = []string{"UPDATED"}
	err = repo.Update(ctx, originalAttr)
	assert.NoError(t, err)

	// Retrieve and verify updates
	updatedAttr, err := repo.GetByID(ctx, originalAttr.AttributeID)
	assert.NoError(t, err)
	assert.Equal(t, "Updated description", updatedAttr.LongDescription)
	assert.Equal(t, []string{"UPDATED"}, updatedAttr.Constraints)
}

func TestPostgresDictionaryRepository_Delete(t *testing.T) {
	_, cleanup := setupTestDB(t)
	defer cleanup()

	repo, err := newPostgresDictionaryRepository(
		os.Getenv("TEST_DB_CONN_STRING"),
		&repositoryConfig{},
	)
	require.NoError(t, err)

	ctx := context.Background()
	originalAttr := createTestAttribute()

	// Create attribute
	err = repo.Create(ctx, originalAttr)
	require.NoError(t, err)

	// Delete attribute
	err = repo.Delete(ctx, originalAttr.AttributeID)
	assert.NoError(t, err)

	// Try to retrieve deleted attribute
	_, err = repo.GetByID(ctx, originalAttr.AttributeID)
	assert.Error(t, err, "Expected error retrieving deleted attribute")
}

func TestPostgresDictionaryRepository_List(t *testing.T) {
	_, cleanup := setupTestDB(t)
	defer cleanup()

	repo, err := newPostgresDictionaryRepository(
		os.Getenv("TEST_DB_CONN_STRING"),
		&repositoryConfig{},
	)
	require.NoError(t, err)

	ctx := context.Background()

	// Create multiple test attributes
	testAttrs := []*dictionary.Attribute{
		createTestAttribute(),
		createTestAttribute(),
		createTestAttribute(),
	}

	for _, attr := range testAttrs {
		attr.Domain = "TEST_LIST"
		err = repo.Create(ctx, attr)
		require.NoError(t, err)
	}

	// List attributes
	listOpts := &ListOptions{
		Domain: "TEST_LIST",
		Limit:  10,
	}

	attrs, err := repo.List(ctx, listOpts)
	assert.NoError(t, err)
	assert.GreaterOrEqual(t, len(attrs), 3)

	// Count attributes
	count, err := repo.Count(ctx, listOpts)
	assert.NoError(t, err)
	assert.GreaterOrEqual(t, count, 3)
}

func TestPostgresDictionaryRepository_Count(t *testing.T) {
	_, cleanup := setupTestDB(t)
	defer cleanup()

	repo, err := newPostgresDictionaryRepository(
		os.Getenv("TEST_DB_CONN_STRING"),
		&repositoryConfig{},
	)
	require.NoError(t, err)

	ctx := context.Background()

	// Test attribute with complex constraints
	attr := createTestAttribute()
	attr.Constraints = []string{
		"REQUIRED",
		"MIN_LENGTH:2",
		"MAX_LENGTH:100",
		"REGEX:^[A-Za-z0-9_]+$",
	}
	attr.Tags = []string{"COMPLIANCE", "VALIDATION"}

	err = repo.Create(ctx, attr)
	assert.NoError(t, err)

	// Retrieve and verify constraints
	retrievedAttr, err := repo.GetByID(ctx, attr.AttributeID)
	assert.NoError(t, err)
	assert.ElementsMatch(t, attr.Constraints, retrievedAttr.Constraints)
	assert.ElementsMatch(t, attr.Tags, retrievedAttr.Tags)
}
