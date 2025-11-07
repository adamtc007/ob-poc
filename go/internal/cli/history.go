package cli

import (
	"context"
	"flag"
	"fmt"
	"log"
	"time"

	"dsl-ob-poc/internal/datastore"
)

// RunHistory handles the 'history' command.
func RunHistory(ctx context.Context, ds datastore.DataStore, args []string) error {
	fs := flag.NewFlagSet("history", flag.ExitOnError)
	cbuID := fs.String("cbu", "", "The CBU ID of the case to view (required)")
	if err := fs.Parse(args); err != nil {
		return fmt.Errorf("failed to parse flags: %w", err)
	}

	if *cbuID == "" {
		fs.Usage()
		return fmt.Errorf("error: --cbu flag is required")
	}

	log.Printf("Fetching DSL history for CBU: %s", *cbuID)

	// 1. Get onboarding session information
	session, err := ds.GetOnboardingSession(ctx, *cbuID)
	if err != nil {
		log.Printf("Warning: Could not get onboarding session: %v", err)
		session = nil
	}

	// 2. Get all DSL versions with state information from the store
	historyWithState, err := ds.GetDSLHistoryWithState(ctx, *cbuID)
	if err != nil {
		return err
	}

	// 3. Print the full history with state information
	fmt.Printf("\n--- DSL State Evolution for CBU: %s ---\n", *cbuID)
	if session != nil {
		fmt.Printf("📋 Onboarding Session: %s\n", session.OnboardingID)
		fmt.Printf("🎯 Current State: %s (Version %d)\n", session.CurrentState, session.CurrentVersion)
		fmt.Printf("📅 Session Created: %s\n", session.CreatedAt.Format(time.RFC3339))
		fmt.Printf("🕐 Last Updated: %s\n", session.UpdatedAt.Format(time.RFC3339))
	}
	fmt.Printf("📚 Found %d versions.\n\n", len(historyWithState))

	for _, version := range historyWithState {
		fmt.Printf("===========================================\n")
		fmt.Printf("📄 Version %d | State: %s\n", version.VersionNumber, version.OnboardingState)
		fmt.Printf("🆔 Version ID: %s\n", version.VersionID)
		fmt.Printf("📅 Created At: %s\n", version.CreatedAt.Format(time.RFC3339))
		fmt.Printf("-------------------------------------------\n")
		fmt.Println(version.DSLText)
		fmt.Printf("===========================================\n\n")
	}

	return nil
}
