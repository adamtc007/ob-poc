package cli

import (
	"context"
	"flag"
	"fmt"

	"dsl-ob-poc/internal/datastore"
	"dsl-ob-poc/internal/dsl"
	"dsl-ob-poc/internal/shared-dsl/session"
	"dsl-ob-poc/internal/store"
)

// RunCreate handles the 'create' command.
func RunCreate(ctx context.Context, ds datastore.DataStore, args []string) error {
	fs := flag.NewFlagSet("create", flag.ExitOnError)
	cbuID := fs.String("cbu", "", "The CBU ID for the new case (required)")
	if err := fs.Parse(args); err != nil {
		return fmt.Errorf("failed to parse flags: %w", err)
	}

	if *cbuID == "" {
		fs.Usage()
		return fmt.Errorf("error: --cbu flag is required")
	}

	// FIXED: Use database instead of hardcoded mock data
	// First check if CBU exists in database
	existingCBU, err := ds.GetCBUByName(ctx, *cbuID)
	if err != nil {
		// CBU doesn't exist, create it with default values
		// In production, this should prompt for description and nature_purpose
		cbuUUID, createErr := ds.CreateCBU(ctx, *cbuID, "Auto-created CBU", "Default onboarding case")
		if createErr != nil {
			return fmt.Errorf("failed to create CBU in database: %w", createErr)
		}
		// Retrieve the newly created CBU
		existingCBU, err = ds.GetCBUByID(ctx, cbuUUID)
		if err != nil {
			return fmt.Errorf("failed to retrieve newly created CBU: %w", err)
		}
	}

	// Create onboarding session in database
	dbSession, err := ds.CreateOnboardingSession(ctx, existingCBU.CBUID)
	if err != nil {
		return fmt.Errorf("failed to create onboarding session: %w", err)
	}

	// Create DSL session manager and accumulate DSL (single source of truth)
	sessionMgr := session.NewManager()
	sess := sessionMgr.GetOrCreate(existingCBU.CBUID, "onboarding")

	// Generate the initial "CREATE" DSL through builder using database CBU data
	newDSL := dsl.CreateCase(existingCBU.CBUID, existingCBU.NaturePurpose)

	// Accumulate DSL through state manager
	err = sess.AccumulateDSL(newDSL)
	if err != nil {
		return fmt.Errorf("failed to accumulate DSL: %w", err)
	}

	// Get final DSL from state manager and save to database
	finalDSL := sess.GetDSL()
	versionID, err := ds.InsertDSLWithState(ctx, existingCBU.CBUID, finalDSL, store.StateCreated)
	if err != nil {
		return fmt.Errorf("failed to save new case: %w", err)
	}

	// Update onboarding session with the new DSL version
	err = ds.UpdateOnboardingState(ctx, existingCBU.CBUID, store.StateCreated, versionID)
	if err != nil {
		return fmt.Errorf("failed to update onboarding state: %w", err)
	}

	fmt.Printf("✅ Created new case with onboarding session: %s\n", dbSession.OnboardingID)
	fmt.Printf("📝 DSL version (v%d) in state %s: %s\n", dbSession.CurrentVersion, store.StateCreated, versionID)
	fmt.Println("---")
	fmt.Println(finalDSL)
	fmt.Println("---")

	return nil
}
