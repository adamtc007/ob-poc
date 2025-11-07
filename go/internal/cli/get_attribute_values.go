package cli

import (
	"context"
	"flag"
	"fmt"
	"log"
	"regexp"

	"dsl-ob-poc/internal/datastore"
	"dsl-ob-poc/internal/dictionary"
	"dsl-ob-poc/internal/dsl"
	"dsl-ob-poc/internal/shared-dsl/session"
)

// looksLikeUUID checks if a string looks like a UUID
func looksLikeUUID(s string) bool {
	uuidRegex := regexp.MustCompile(`^[a-fA-F0-9]{8}-[a-fA-F0-9]{4}-[a-fA-F0-9]{4}-[a-fA-F0-9]{4}-[a-fA-F0-9]{12}$`)
	return uuidRegex.MatchString(s)
}

// RunGetAttributeValues implements the get-attribute-values command
func RunGetAttributeValues(ctx context.Context, ds datastore.DataStore, args []string) error {
	fs := flag.NewFlagSet("get-attribute-values", flag.ExitOnError)
	cbuID := fs.String("cbu", "", "The CBU ID of the case to process (required)")

	if parseErr := fs.Parse(args); parseErr != nil {
		return fmt.Errorf("failed to parse flags: %w", parseErr)
	}

	if *cbuID == "" {
		return fmt.Errorf("--cbu flag is required")
	}

	log.Printf("Getting attribute values for CBU: %s", *cbuID)

	// 1) Get latest DSL + version
	latest, err := ds.GetLatestDSL(ctx, *cbuID)
	if err != nil {
		return fmt.Errorf("failed to get latest DSL: %w", err)
	}

	// For POC, use version = 1
	version := 1

	// 2) Normalize any shorthand vars (needs a resolver using the dictionary)
	norm := dsl.NormalizeVars(latest, func(sym string) (attr *dictionary.Attribute, ok bool) {
		a, _ := ds.GetDictionaryAttributeByName(ctx, sym)
		if a != nil {
			return a, true
		}
		// accept raw UUIDs in symbol too - create a minimal attribute for UUIDs
		if looksLikeUUID(sym) {
			return &dictionary.Attribute{
				AttributeID: sym,
				Name:        sym, // Use UUID as name for raw UUIDs
			}, true
		}
		return nil, false
	})

	// 3) Extract canonical var attr-ids
	ids := dsl.ExtractVarAttrIDs(norm)
	log.Printf("Found %d attribute variables to resolve", len(ids))

	// 4) Resolve & persist
	assignments := map[string]string{}
	for _, attrID := range ids {
		val, prov, state, resolveErr := ds.ResolveValueFor(ctx, *cbuID, attrID)
		if resolveErr != nil {
			return fmt.Errorf("failed to resolve value for %s: %w", attrID, resolveErr)
		}

		if upsertErr := ds.UpsertAttributeValue(ctx, *cbuID, version, attrID, val, state, prov); upsertErr != nil {
			return fmt.Errorf("failed to store value for %s: %w", attrID, upsertErr)
		}

		if state == "resolved" {
			assignments[attrID] = string(val)
			log.Printf("✅ Resolved %s = %s", attrID, string(val))
		} else {
			log.Printf("⏳ Pending resolution for %s (state: %s)", attrID, state)
		}
	}

	// 5) Create session manager and accumulate DSL (single source of truth)
	sessionMgr := session.NewManager()
	sess := sessionMgr.GetOrCreate(*cbuID, "onboarding")

	// Accumulate current DSL
	err = sess.AccumulateDSL(norm)
	if err != nil {
		return fmt.Errorf("failed to accumulate current DSL: %w", err)
	}

	// 6) Generate and accumulate binding DSL through state manager
	bind := dsl.RenderBindings(assignments)
	err = sess.AccumulateDSL(bind)
	if err != nil {
		return fmt.Errorf("failed to accumulate binding DSL: %w", err)
	}

	// 7) Get final DSL from state manager and save to database
	finalDSL := sess.GetDSL()
	versionID, err := ds.InsertDSL(ctx, *cbuID, finalDSL)
	if err != nil {
		return fmt.Errorf("failed to save final DSL: %w", err)
	}

	log.Printf("✅ Attribute values resolved and stored!")
	log.Printf("📊 Resolved %d/%d attributes", len(assignments), len(ids))
	log.Printf("💾 Final DSL saved as version: %s", versionID)

	fmt.Println("\nGenerated bindings:")
	fmt.Println(bind)

	return nil
}
