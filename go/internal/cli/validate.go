package cli

import (
	"context"
	"fmt"
	"os"

	"dsl-ob-poc/internal/datastore"
	"dsl-ob-poc/internal/dsl"
	"dsl-ob-poc/internal/vocabulary"

	"github.com/jmoiron/sqlx"
	_ "github.com/lib/pq"
)

func RunValidateDSL(ctx context.Context, ds datastore.DataStore, args []string) error {
	if len(args) != 1 {
		return fmt.Errorf("usage: validate-dsl <file_path>")
	}

	filePath := args[0]
	fileContent, err := os.ReadFile(filePath)
	if err != nil {
		return fmt.Errorf("failed to read file: %w", err)
	}

	// Prefer database-backed vocabulary if DB connection is available
	var vocab *dsl.Vocabulary
	if connStr := os.Getenv("DB_CONN_STRING"); connStr != "" {
		if dbx, err := sqlx.Open("postgres", connStr); err == nil {
			defer dbx.Close()
			repo := vocabulary.NewPostgresRepository(dbx)
			if all, err := repo.GetAllApprovedVerbs(ctx); err == nil {
				// Flatten and de-duplicate across domains
				verbSet := make(map[string]bool)
				verbs := make([]string, 0)
				for _, list := range all {
					for _, verb := range list {
						if !verbSet[verb] {
							verbSet[verb] = true
							verbs = append(verbs, verb)
						}
					}
				}
				vocab = dsl.NewVocabulary(verbs)
			}
		}
	}

	// Fallback to static loader if DB not available
	if vocab == nil {
		loader := dsl.NewVocabularyLoader(ds)
		var err error
		vocab, err = loader.LoadVocabulary(ctx)
		if err != nil {
			return fmt.Errorf("failed to load vocabulary: %w", err)
		}
	}

	validator := dsl.NewValidator(ds, vocab)

	if err := validator.Validate(ctx, string(fileContent)); err != nil {
		return fmt.Errorf("DSL validation failed: %w", err)
	}

	fmt.Println("DSL is valid.")

	// Show referenced attributes with names (not just UUIDs)
	ids := dsl.ExtractAttributeIDs(string(fileContent))
	if len(ids) > 0 {
		fmt.Println("Referenced attributes:")
		for _, id := range ids {
			if attr, err := ds.GetDictionaryAttributeByID(ctx, id); err == nil {
				fmt.Printf("- %s (%s)\n", attr.Name, id)
			} else {
				// Fallback if name lookup fails
				fmt.Printf("- %s\n", id)
			}
		}
	}
	return nil
}
