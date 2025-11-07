package cli

import (
	"context"
	"flag"
	"fmt"
	"log"

	"dsl-ob-poc/internal/datastore"
	"dsl-ob-poc/internal/dsl"
	"dsl-ob-poc/internal/shared-dsl/session"
)

// RunPopulateAttributes implements the 6th state of the onboarding DSL:
// Populates attribute values from runtime sources and generates final DSL.
func RunPopulateAttributes(ctx context.Context, ds datastore.DataStore, args []string) error {
	fs := flag.NewFlagSet("populate-attributes", flag.ExitOnError)
	cbuID := fs.String("cbu", "", "The CBU ID of the case to populate (required)")

	if parseErr := fs.Parse(args); parseErr != nil {
		return fmt.Errorf("failed to parse flags: %w", parseErr)
	}

	if *cbuID == "" {
		return fmt.Errorf("--cbu flag is required")
	}

	log.Printf("Starting attribute population (Step 6) for CBU: %s", *cbuID)

	// Get the current DSL state
	currentDSL, err := ds.GetLatestDSL(ctx, *cbuID)
	if err != nil {
		return fmt.Errorf("failed to get current DSL: %w", err)
	}

	// Parse all attribute references from the DSL
	attributeRefs, err := dsl.ParseAttributeReferences(currentDSL)
	if err != nil {
		return fmt.Errorf("failed to parse attribute references: %w", err)
	}

	log.Printf("Found %d attribute references to populate", len(attributeRefs))

	// Populate attribute values using the DataStore interface
	populatedValues, err := dsl.PopulateAttributeValues(ctx, ds, *cbuID, attributeRefs)
	if err != nil {
		return fmt.Errorf("failed to populate attribute values: %w", err)
	}
	log.Printf("Successfully populated %d attribute values", len(populatedValues))

	// Create DSL session manager and accumulate DSL (single source of truth)
	sessionMgr := session.NewManager()
	dslSession := sessionMgr.GetOrCreate(*cbuID, "onboarding")

	// Accumulate current DSL
	err = dslSession.AccumulateDSL(currentDSL)
	if err != nil {
		return fmt.Errorf("failed to accumulate current DSL: %w", err)
	}

	// Generate populated attributes DSL fragment
	populatedFragment, err := dsl.AddPopulatedAttributes("", populatedValues)
	if err != nil {
		return fmt.Errorf("failed to generate populated attributes: %w", err)
	}

	// Accumulate populated attributes through state manager
	err = dslSession.AccumulateDSL(populatedFragment)
	if err != nil {
		return fmt.Errorf("failed to accumulate populated attributes: %w", err)
	}

	// Get final DSL from state manager and save to database
	finalDSL := dslSession.GetDSL()
	versionID, err := ds.InsertDSL(ctx, *cbuID, finalDSL)
	if err != nil {
		return fmt.Errorf("failed to save final DSL: %w", err)
	}

	log.Printf("✅ Attribute population completed successfully!")
	log.Printf("📊 Populated %d attributes", len(populatedValues))
	log.Printf("💾 Final DSL saved as version: %s", versionID)

	return nil
}
