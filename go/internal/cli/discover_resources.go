package cli

import (
	"context"
	"flag"
	"fmt"
	"log"

	"dsl-ob-poc/internal/datastore"
	"dsl-ob-poc/internal/dictionary"
	"dsl-ob-poc/internal/dsl"
	"dsl-ob-poc/internal/shared-dsl/session"
	"dsl-ob-poc/internal/store"
)

// RunDiscoverResources handles the 'discover-resources' command (Step 5).
func RunDiscoverResources(ctx context.Context, ds datastore.DataStore, args []string) error {
	fs := flag.NewFlagSet("discover-resources", flag.ExitOnError)
	cbuID := fs.String("cbu", "", "The CBU ID of the case to discover (required)")
	if err := fs.Parse(args); err != nil {
		return fmt.Errorf("failed to parse flags: %w", err)
	}

	if *cbuID == "" {
		fs.Usage()
		return fmt.Errorf("error: --cbu flag is required")
	}

	log.Printf("Starting resource discovery (Step 5) for CBU: %s", *cbuID)

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

	// 2. Parse *service* names from the DSL
	serviceNames, err := dsl.ParseServiceNames(currentDSL)
	if err != nil {
		return fmt.Errorf("failed to parse services from DSL: %w. Run 'discover-services' first", err)
	}
	log.Printf("Found %d services in DSL: %v", len(serviceNames), serviceNames)

	// 3. Discover all resources and attributes from the catalog
	serviceResourceMap := make(map[string][]store.ProdResource)
	dictionaryAttributeMap := make(map[string][]dictionary.Attribute)

	allResources := make(map[string]store.ProdResource)

	for _, serviceName := range serviceNames {
		service, getErr := ds.GetServiceByName(ctx, serviceName)
		if getErr != nil {
			return getErr
		}

		resources, getErr := ds.GetResourcesForService(ctx, service.ServiceID)
		if getErr != nil {
			return getErr
		}
		serviceResourceMap[service.Name] = resources

		for _, resource := range resources {
			// Add to unique map
			allResources[resource.ResourceID] = resource

			// If resource has a dictionary group, get its attributes
			if resource.DictionaryGroup != "" {
				// Only fetch if we haven't already
				if _, ok := dictionaryAttributeMap[resource.DictionaryGroup]; !ok {
					dictAttributes, attrErr := ds.GetAttributesForDictionaryGroup(ctx, resource.DictionaryGroup)
					if attrErr != nil {
						return attrErr
					}
					dictionaryAttributeMap[resource.DictionaryGroup] = dictAttributes
				}
			}
		}
	}
	log.Printf("Discovery complete: found %d unique resources.", len(allResources))

	// 4. Create DSL session manager and accumulate DSL (single source of truth)
	sessionMgr := session.NewManager()
	dslSession := sessionMgr.GetOrCreate(*cbuID, "onboarding")

	// Accumulate current DSL
	err = dslSession.AccumulateDSL(currentDSL)
	if err != nil {
		return fmt.Errorf("failed to accumulate current DSL: %w", err)
	}

	// 5. Generate the resources discovery DSL fragment
	plan := dsl.ResourceDiscoveryPlan{
		ServiceResources:   serviceResourceMap,
		ResourceAttributes: dictionaryAttributeMap,
	}

	resourcesDSL, err := dsl.AddDiscoveredResources("", plan)
	if err != nil {
		return fmt.Errorf("failed to generate resources DSL: %w", err)
	}

	// Accumulate resources DSL through state manager
	err = dslSession.AccumulateDSL(resourcesDSL)
	if err != nil {
		return fmt.Errorf("failed to accumulate resources DSL: %w", err)
	}

	// 6. Get final DSL from state manager and save to database
	finalDSL := dslSession.GetDSL()
	versionID, err := ds.InsertDSLWithState(ctx, *cbuID, finalDSL, store.StateResourcesDiscovered)
	if err != nil {
		return fmt.Errorf("failed to save new DSL version: %w", err)
	}

	// 7. Update onboarding session state
	err = ds.UpdateOnboardingState(ctx, *cbuID, store.StateResourcesDiscovered, versionID)
	if err != nil {
		return fmt.Errorf("failed to update onboarding state: %w", err)
	}

	fmt.Printf("🔍 Updated case from %s to %s\n", currentDSLState.OnboardingState, store.StateResourcesDiscovered)
	fmt.Printf("📝 DSL version: %s\n", versionID)
	fmt.Println("---")
	fmt.Println(finalDSL)
	fmt.Println("---")

	return nil
}
