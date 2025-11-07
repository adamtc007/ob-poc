package cli

import (
	"context"
	"flag"
	"fmt"
	"strings"

	"dsl-ob-poc/internal/datastore"
	"dsl-ob-poc/internal/dsl"
	"dsl-ob-poc/internal/shared-dsl/session"
	"dsl-ob-poc/internal/store"
)

// RunAddProducts handles the 'add-products' command.
func RunAddProducts(ctx context.Context, ds datastore.DataStore, args []string) error {
	fs := flag.NewFlagSet("add-products", flag.ExitOnError)
	cbuID := fs.String("cbu", "", "The CBU ID of the case to update (required)")
	productsStr := fs.String("products", "", "Comma-separated list of products (required)")
	if err := fs.Parse(args); err != nil {
		return fmt.Errorf("failed to parse flags: %w", err)
	}

	if *cbuID == "" || *productsStr == "" {
		fs.Usage()
		return fmt.Errorf("error: --cbu and --products flags are required")
	}

	productNames := strings.Split(*productsStr, ",")
	if len(productNames) == 0 {
		return fmt.Errorf("error: no products provided")
	}

	// 1. Validate products against the catalog
	validProducts := make([]*store.Product, 0, len(productNames))
	for _, name := range productNames {
		p, err := ds.GetProductByName(ctx, strings.TrimSpace(name))
		if err != nil {
			return fmt.Errorf("validation failed: %w", err)
		}
		validProducts = append(validProducts, p)
	}
	fmt.Printf("Validated %d products against catalog.\n", len(validProducts))

	// 2. Get the current onboarding session (for validation)
	_, err := ds.GetOnboardingSession(ctx, *cbuID)
	if err != nil {
		return fmt.Errorf("failed to get onboarding session for CBU %s: %w", *cbuID, err)
	}

	// 3. Get the *current* state of the DSL from the DB with state information
	currentDSLState, err := ds.GetLatestDSLWithState(ctx, *cbuID)
	if err != nil {
		return fmt.Errorf("failed to get current case for CBU %s: %w", *cbuID, err)
	}

	// 4. Create DSL session manager and accumulate DSL (single source of truth)
	sessionMgr := session.NewManager()
	dslSession := sessionMgr.GetOrCreate(*cbuID, "onboarding")

	// Accumulate current DSL
	err = dslSession.AccumulateDSL(currentDSLState.DSLText)
	if err != nil {
		return fmt.Errorf("failed to accumulate current DSL: %w", err)
	}

	// Generate product addition DSL and accumulate
	productDSL, err := dsl.AddProducts("", validProducts) // Generate only the product part
	if err != nil {
		return fmt.Errorf("failed to generate product DSL: %w", err)
	}

	err = dslSession.AccumulateDSL(productDSL)
	if err != nil {
		return fmt.Errorf("failed to accumulate product DSL: %w", err)
	}

	// 5. Get final DSL from state manager and save to database
	finalDSL := dslSession.GetDSL()
	versionID, err := ds.InsertDSLWithState(ctx, *cbuID, finalDSL, store.StateProductsAdded)
	if err != nil {
		return fmt.Errorf("failed to save updated case: %w", err)
	}

	// 6. Update onboarding session state
	err = ds.UpdateOnboardingState(ctx, *cbuID, store.StateProductsAdded, versionID)
	if err != nil {
		return fmt.Errorf("failed to update onboarding state: %w", err)
	}

	fmt.Printf("🚀 Updated case from %s to %s\n", currentDSLState.OnboardingState, store.StateProductsAdded)
	fmt.Printf("📝 DSL version: %s\n", versionID)
	fmt.Println("---")
	fmt.Println(finalDSL)
	fmt.Println("---")

	return nil
}
