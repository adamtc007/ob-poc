package cli

import (
	"context"
	"flag"
	"fmt"
	"os"

	"dsl-ob-poc/internal/datastore"
	"dsl-ob-poc/internal/dictionary"
)

// RegenerateVectorsCommand regenerates vector embeddings for all dictionary attributes
func RegenerateVectorsCommand(args []string) error {
	fs := flag.NewFlagSet("regenerate-vectors", flag.ExitOnError)

	var (
		attributeID = fs.String("attribute-id", "", "Regenerate vector for specific attribute ID (optional)")
		validate    = fs.Bool("validate", false, "Validate vector integrity instead of regenerating")
		stats       = fs.Bool("stats", false, "Show vector database statistics")
	)

	if err := fs.Parse(args); err != nil {
		return err
	}

	// Initialize data store
	ds, err := datastore.NewDataStore(datastore.Config{
		Type:             datastore.PostgreSQLStore,
		ConnectionString: os.Getenv("DB_CONN_STRING"),
	})
	if err != nil {
		return fmt.Errorf("failed to initialize data store: %w", err)
	}
	defer ds.Close()

	ctx := context.Background()

	// Initialize vector service
	vectorService := dictionary.NewVectorService()

	switch {
	case *stats:
		return showVectorStats(ctx, vectorService, ds)
	case *validate:
		return validateVectors(ctx, vectorService, ds)
	case *attributeID != "":
		return regenerateSpecificVector(ctx, vectorService, ds, *attributeID)
	default:
		return regenerateAllVectors(ctx, vectorService, ds)
	}
}

// regenerateAllVectors regenerates vectors for all attributes
func regenerateAllVectors(ctx context.Context, service *dictionary.VectorService, ds datastore.DataStore) error {
	fmt.Println("🚀 Starting vector regeneration for all dictionary attributes...")
	fmt.Println()

	// Show current stats
	fmt.Println("📊 Current Vector Database Status:")
	if err := showVectorStats(ctx, service, ds); err != nil {
		fmt.Printf("⚠️  Could not get current stats: %v\n", err)
	}
	fmt.Println()

	// Regenerate all vectors
	if err := service.RegenerateAllVectors(ctx, ds); err != nil {
		return fmt.Errorf("failed to regenerate vectors: %w", err)
	}

	fmt.Println()
	fmt.Println("✅ Vector regeneration completed successfully!")
	fmt.Println()
	fmt.Println("💡 Note: In production, this would call an embedding service like:")
	fmt.Println("   • OpenAI Embeddings API")
	fmt.Println("   • Sentence Transformers")
	fmt.Println("   • Custom embedding models")
	fmt.Println()
	fmt.Println("📚 The vectors enable AI-powered semantic search for attributes")
	fmt.Println("   based on their names, descriptions, and metadata.")

	return nil
}

// regenerateSpecificVector regenerates vector for a specific attribute
func regenerateSpecificVector(ctx context.Context, service *dictionary.VectorService, ds datastore.DataStore, attributeID string) error {
	fmt.Printf("🔄 Regenerating vector for attribute: %s\n", attributeID)

	if err := service.UpdateAttributeVector(ctx, ds, attributeID); err != nil {
		return fmt.Errorf("failed to update vector for attribute %s: %w", attributeID, err)
	}

	fmt.Printf("✅ Successfully updated vector for attribute %s\n", attributeID)
	return nil
}

// validateVectors validates vector integrity
func validateVectors(ctx context.Context, service *dictionary.VectorService, ds datastore.DataStore) error {
	fmt.Println("🔍 Validating vector database integrity...")

	if err := service.ValidateVectorIntegrity(ctx, ds); err != nil {
		return fmt.Errorf("vector validation failed: %w", err)
	}

	return nil
}

// showVectorStats displays vector database statistics
func showVectorStats(ctx context.Context, service *dictionary.VectorService, ds datastore.DataStore) error {
	stats, err := service.GetVectorStats(ctx, ds)
	if err != nil {
		return fmt.Errorf("failed to get vector stats: %w", err)
	}

	fmt.Printf("📊 Vector Database Statistics:\n")
	fmt.Printf("   Total Attributes: %d\n", stats.TotalAttributes)
	fmt.Printf("   With Vectors: %d\n", stats.AttributesWithVector)
	fmt.Printf("   Without Vectors: %d\n", stats.AttributesWithoutVector)

	if stats.TotalAttributes > 0 {
		coverage := float64(stats.AttributesWithVector) / float64(stats.TotalAttributes) * 100
		fmt.Printf("   Vector Coverage: %.1f%%\n", coverage)
	}

	if stats.LastUpdate != nil {
		fmt.Printf("   Last Update: %s\n", *stats.LastUpdate)
	}

	return nil
}

// SearchAttributesCommand searches for attributes using semantic similarity
func SearchAttributesCommand(args []string) error {
	fs := flag.NewFlagSet("search-attributes", flag.ExitOnError)

	var (
		query = fs.String("query", "", "Search query text (required)")
		limit = fs.Int("limit", 10, "Maximum number of results")
	)

	if err := fs.Parse(args); err != nil {
		return err
	}

	if *query == "" {
		return fmt.Errorf("search query is required (use -query flag)")
	}

	// Initialize data store
	ds, err := datastore.NewDataStore(datastore.Config{
		Type:             datastore.PostgreSQLStore,
		ConnectionString: os.Getenv("DB_CONN_STRING"),
	})
	if err != nil {
		return fmt.Errorf("failed to initialize data store: %w", err)
	}
	defer ds.Close()

	ctx := context.Background()
	vectorService := dictionary.NewVectorService()

	fmt.Printf("🔍 Searching for attributes similar to: %s\n", *query)
	fmt.Printf("📊 Limit: %d results\n", *limit)
	fmt.Println()

	results, err := vectorService.SearchSimilarAttributes(ctx, ds, *query, *limit)
	if err != nil {
		return fmt.Errorf("search failed: %w", err)
	}

	if len(results) == 0 {
		fmt.Println("❌ No similar attributes found")
		fmt.Println()
		fmt.Println("💡 Tips:")
		fmt.Println("   • Try broader search terms")
		fmt.Println("   • Ensure vectors are generated (run regenerate-vectors)")
		fmt.Println("   • Check if dictionary has populated attributes")
		return nil
	}

	fmt.Printf("✅ Found %d similar attributes:\n", len(results))
	fmt.Println()

	for i, attr := range results {
		fmt.Printf("%d. %s (ID: %s)\n", i+1, attr.Name, attr.AttributeID)
		if attr.LongDescription != "" {
			fmt.Printf("   Description: %s\n", attr.LongDescription)
		}
		if attr.Domain != "" {
			fmt.Printf("   Domain: %s\n", attr.Domain)
		}
		fmt.Println()
	}

	return nil
}