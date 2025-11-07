package cli

import (
	"context"
	"flag"
	"fmt"
	"log"
	"strings"

	"dsl-ob-poc/internal/agent"
	"dsl-ob-poc/internal/datastore"
	"dsl-ob-poc/internal/dsl"
	"dsl-ob-poc/internal/shared-dsl/session"
	"dsl-ob-poc/internal/store"
)

// RunDiscoverKYC handles the 'discover-kyc' command (new Step 3 powered by the AI agent).
func RunDiscoverKYC(ctx context.Context, ds datastore.DataStore, ai *agent.Agent, args []string) error {
	fs := flag.NewFlagSet("discover-kyc", flag.ExitOnError)
	cbuID := fs.String("cbu", "", "The CBU ID of the case to discover (required)")
	if err := fs.Parse(args); err != nil {
		return fmt.Errorf("failed to parse flags: %w", err)
	}

	if *cbuID == "" {
		fs.Usage()
		return fmt.Errorf("error: --cbu flag is required")
	}

	if ai == nil {
		return fmt.Errorf("ai agent is not configured; set GEMINI_API_KEY and try again")
	}

	log.Printf("Starting KYC discovery (Agent Step 3) for CBU: %s", *cbuID)

	// 1. Get the current onboarding session (for validation)
	_, err := ds.GetOnboardingSession(ctx, *cbuID)
	if err != nil {
		return fmt.Errorf("failed to get onboarding session for CBU %s: %w", *cbuID, err)
	}

	// 2. Get the latest DSL with state information
	currentDSLState, err := ds.GetLatestDSLWithState(ctx, *cbuID)
	if err != nil {
		return err
	}

	currentDSL := currentDSLState.DSLText

	// 2. Parse the DSL for the inputs needed by the agent
	naturePurpose, err := dsl.ParseNaturePurpose(currentDSL)
	if err != nil {
		return fmt.Errorf("failed to parse nature-purpose from DSL: %w", err)
	}
	productNames, err := dsl.ParseProductNames(currentDSL)
	if err != nil {
		return fmt.Errorf("failed to parse products from DSL: %w", err)
	}

	// 3. Parse *existing* KYC requirements to perform a diff
	// This makes the step idempotent and reconcilable.
	existingReqs, err := dsl.ParseKYCRequirements(currentDSL)
	if err != nil {
		log.Printf("Note: No existing KYC block found, will create a new one.")
		existingReqs = &dsl.KYCRequirements{} // Set to empty
	} else {
		log.Printf("Found %d existing documents and %d jurisdictions.", len(existingReqs.Documents), len(existingReqs.Jurisdictions))
	}

	log.Printf("Found Nature/Purpose: %q", naturePurpose)
	log.Printf("Found Products: %v", productNames)

	// 4. Call the AI Agent to determine the *new desired* KYC requirements
	log.Println("Calling AI Agent (Gemini) to determine KYC requirementds...")
	newReqs, err := ai.CallKYCAgent(ctx, naturePurpose, productNames)
	if err != nil {
		return fmt.Errorf("ai agent failed: %w", err)
	}
	log.Printf("Agent response received. Desired docs: %v, Jurisdictions: %v", newReqs.Documents, newReqs.Jurisdictions)

	// 5. Calculate the "delta" and generate the new DSL
	// This is the core reconciliation logic for KYC.
	// We pass both the old and new requirements to the DSL generator.
	newDSLFragment, diff, err := dsl.AddOrModifyKYCBlock("", *existingReqs, *newReqs)
	if err != nil {
		return fmt.Errorf("failed to generate new DSL: %w", err)
	}

	if !diff.HasChanges() {
		log.Println("✅ KYC requirements are already up-to-date. No changes needed.")
		fmt.Println("KYC requirements are already up-to-date.")
		return nil
	}

	log.Printf("KYC Diff: +Docs[%s] -Docs[%s] +Juris[%s] -Juris[%s]",
		strings.Join(diff.AddedDocs, ","),
		strings.Join(diff.RemovedDocs, ","),
		strings.Join(diff.AddedJuris, ","),
		strings.Join(diff.RemovedJuris, ","))

	// 6. Create DSL session manager and accumulate DSL (single source of truth)
	sessionMgr := session.NewManager()
	dslSession := sessionMgr.GetOrCreate(*cbuID, "onboarding")

	// Accumulate current DSL
	err = dslSession.AccumulateDSL(currentDSL)
	if err != nil {
		return fmt.Errorf("failed to accumulate current DSL: %w", err)
	}

	// Accumulate KYC discovery DSL
	err = dslSession.AccumulateDSL(newDSLFragment)
	if err != nil {
		return fmt.Errorf("failed to accumulate KYC DSL: %w", err)
	}

	// Get final DSL from state manager and save to database
	finalDSL := dslSession.GetDSL()
	versionID, err := ds.InsertDSLWithState(ctx, *cbuID, finalDSL, store.StateKYCDiscovered)
	if err != nil {
		return fmt.Errorf("failed to save new DSL version: %w", err)
	}

	// 7. Update onboarding session state
	err = ds.UpdateOnboardingState(ctx, *cbuID, store.StateKYCDiscovered, versionID)
	if err != nil {
		return fmt.Errorf("failed to update onboarding state: %w", err)
	}

	fmt.Printf("🔍 Updated case from %s to %s\n", currentDSLState.OnboardingState, store.StateKYCDiscovered)
	fmt.Printf("📝 DSL version: %s\n", versionID)
	fmt.Println("---")
	fmt.Println(finalDSL)
	fmt.Println("---")

	return nil
}
