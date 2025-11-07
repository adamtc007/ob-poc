// DEPRECATED: This file is marked for deletion as part of multi-domain migration.
//
// Migration Status: Phase 4 - Create Onboarding Domain
// New Location: internal/domains/onboarding/builder.go
// Deprecation Date: 2024-01-XX
// Planned Deletion: After Phase 6 complete and all tests passing
//
// DO NOT MODIFY THIS FILE - It is kept for reference and regression testing only.
// See MIGRATION_DEPRECATION_TRACKER.md for details.
//
// This file contains DSL builder functions for onboarding workflows that will be
// migrated to the new domain-specific builder system.

package dsl

import (
	"context"
	"encoding/json"
	"fmt"
	"regexp"
	"sort"
	"strings"

	"dsl-ob-poc/internal/datastore"
	"dsl-ob-poc/internal/dictionary"
	"dsl-ob-poc/internal/store"
)

// This package is the "Go internal lib" for generating the DSL.

// --- State 1: Create Case ---

func CreateCase(cbuID, naturePurpose string) string {
	var b strings.Builder
	b.WriteString("(case.create\n")
	b.WriteString(fmt.Sprintf("  (cbu.id %q)\n", cbuID))
	b.WriteString(fmt.Sprintf("  (nature-purpose %q)\n", naturePurpose))
	b.WriteString(")")
	return b.String()
}

// --- State 2: Add Products ---

func AddProducts(currentDSL string, products []*store.Product) (string, error) {
	if len(products) == 0 {
		return currentDSL, nil // No change
	}

	productExprs := make([]string, 0, len(products))
	for _, p := range products {
		productExprs = append(productExprs, fmt.Sprintf("%q", p.Name))
	}

	var b strings.Builder
	b.WriteString(currentDSL)
	b.WriteString("\n\n")
	b.WriteString("(products.add ")
	b.WriteString(strings.Join(productExprs, " "))
	b.WriteString(")")

	return b.String(), nil
}

// Simple parser for POC
var productRegex = regexp.MustCompile(`\(products\.add\s+(.*?)\)`)
var naturePurposeRegex = regexp.MustCompile(`\(nature-purpose\s+"(.*?)"\)`)

func ParseProductNames(dsl string) ([]string, error) {
	matches := productRegex.FindStringSubmatch(dsl)
	if len(matches) < 2 {
		return nil, fmt.Errorf("no (products.add ...) block found in DSL")
	}

	namesStr := matches[1] // e.g., "CUSTODY" "FUND_ACCOUNTING"
	namesStr = strings.ReplaceAll(namesStr, "\"", "")
	names := strings.Fields(namesStr) // Use Fields to split on whitespace

	if len(names) == 0 {
		return nil, fmt.Errorf("no product names found in block")
	}
	return names, nil
}

func ParseNaturePurpose(dsl string) (string, error) {
	matches := naturePurposeRegex.FindStringSubmatch(dsl)
	if len(matches) < 2 {
		return "", fmt.Errorf("no (nature-purpose ...) block found in DSL")
	}
	return matches[1], nil
}

// KYCRequirements captures the AI agent output used for DSL generation.
type KYCRequirements struct {
	Documents     []string
	Jurisdictions []string
}

// --- KYC Diff & Reconciliation ---

// KYCDiff represents the changes between two KYC requirement sets.
type KYCDiff struct {
	AddedDocs    []string
	RemovedDocs  []string
	AddedJuris   []string
	RemovedJuris []string
}

// HasChanges returns true if any diff was found.
func (d *KYCDiff) HasChanges() bool {
	return len(d.AddedDocs) > 0 || len(d.RemovedDocs) > 0 || len(d.AddedJuris) > 0 || len(d.RemovedJuris) > 0
}

// calculateDiff computes the delta between two string slices.
func calculateDiff(old, new []string) (added, removed []string) {
	oldSet := make(map[string]bool)
	for _, s := range old {
		oldSet[s] = true
	}

	newSet := make(map[string]bool)
	for _, s := range new {
		newSet[s] = true
		if !oldSet[s] {
			added = append(added, s)
		}
	}

	for _, s := range old {
		if !newSet[s] {
			removed = append(removed, s)
		}
	}
	return
}

// AddOrModifyKYCBlock is the main reconciliation function for KYC.
// It calculates the diff and appends a (kyc.modify ...) block.
func AddOrModifyKYCBlock(currentDSL string, oldReqs, newReqs KYCRequirements) (string, KYCDiff, error) {
	addDocs, remDocs := calculateDiff(oldReqs.Documents, newReqs.Documents)
	addJuris, remJuris := calculateDiff(oldReqs.Jurisdictions, newReqs.Jurisdictions)

	diff := KYCDiff{
		AddedDocs:    addDocs,
		RemovedDocs:  remDocs,
		AddedJuris:   addJuris,
		RemovedJuris: remJuris,
	}

	if !diff.HasChanges() {
		return currentDSL, diff, nil
	}

	var b strings.Builder
	b.WriteString(currentDSL)
	b.WriteString("\n\n")

	// If the original block didn't exist, we create `(kyc.start ...)`
	if len(oldReqs.Documents) == 0 && len(oldReqs.Jurisdictions) == 0 {
		b.WriteString("(kyc.start\n")
		if len(newReqs.Documents) > 0 {
			b.WriteString(writeSExprList("documents", "document", newReqs.Documents))
		}
		if len(newReqs.Jurisdictions) > 0 {
			b.WriteString(writeSExprList("jurisdictions", "jurisdiction", newReqs.Jurisdictions))
		}
		b.WriteString(")")
	} else {
		// Otherwise, we create `(kyc.modify ...)`
		b.WriteString("(kyc.modify\n")
		b.WriteString(writeSExprList("add-documents", "document", addDocs))
		b.WriteString(writeSExprList("remove-documents", "document", remDocs))
		b.WriteString(writeSExprList("add-jurisdictions", "jurisdiction", addJuris))
		b.WriteString(writeSExprList("remove-jurisdictions", "jurisdiction", remJuris))
		b.WriteString(")")
	}

	return b.String(), diff, nil
}

// writeSExprList is a helper to format (list (item "a") (item "b"))
func writeSExprList(listName, itemName string, items []string) string {
	if len(items) == 0 {
		return ""
	}
	var b strings.Builder
	b.WriteString(fmt.Sprintf("  (%s\n", listName))
	docs := append([]string(nil), items...)
	sort.Strings(docs)
	for _, doc := range docs {
		b.WriteString(fmt.Sprintf("    (%s %q)\n", itemName, doc))
	}
	b.WriteString("  )\n")
	return b.String()
}

// --- Parsers ---

var kycBlockRegex = regexp.MustCompile(`(?s)\((kyc\.start|kyc\.modify).*?\((documents|add-documents)\s+(.*?)\).*?\((jurisdictions|add-jurisdictions)\s+(.*?)\)`)
var docRegex = regexp.MustCompile(`\(document\s+"(.*?)"\)`)
var jurisRegex = regexp.MustCompile(`\(jurisdiction\s+"(.*?)"\)`)

// ParseKYCRequirements parses the *current* state of KYC docs and jurisdictions
// from the *entire* DSL history by accumulating all `kyc.start` and `kyc.modify` blocks.
// NOTE: This is a simple POC parser. A real one would walk the S-expression tree.
func ParseKYCRequirements(dsl string) (*KYCRequirements, error) {
	docSet := make(map[string]bool)
	jurisSet := make(map[string]bool)

	// Find all kyc.start blocks
	startMatches := kycBlockRegex.FindAllStringSubmatch(dsl, -1)
	if len(startMatches) == 0 {
		return nil, fmt.Errorf("no (kyc.start ...) or (kyc.modify ...) blocks found")
	}

	for _, block := range startMatches {
		// block[0] is full match, [1] is 'kyc.start' or 'kyc.modify'
		// [2] is 'documents' or 'add-documents'
		// [3] is the content of the documents block
		// [4] is 'jurisdictions' or 'add-jurisdictions'
		// [5] is the content of the jurisdictions block

		// Handle Documents
		docBlockContent := block[3]
		docMatches := docRegex.FindAllStringSubmatch(docBlockContent, -1)
		for _, m := range docMatches {
			docSet[m[1]] = true
		}

		// Handle Jurisdictions
		jurisBlockContent := block[5]
		jurisMatches := jurisRegex.FindAllStringSubmatch(jurisBlockContent, -1)
		for _, m := range jurisMatches {
			jurisSet[m[1]] = true
		}

		// Rudimentary support for remove blocks (POC only)
		if block[1] == "kyc.modify" {
			// This is a naive implementation for a POC
			remDocMatches := regexp.MustCompile(`\(remove-documents\s+(.*?)\)`).FindStringSubmatch(dsl)
			if len(remDocMatches) > 1 {
				innerDocMatches := docRegex.FindAllStringSubmatch(remDocMatches[1], -1)
				for _, m := range innerDocMatches {
					delete(docSet, m[1])
				}
			}
			remJurisMatches := regexp.MustCompile(`\(remove-jurisdictions\s+(.*?)\)`).FindStringSubmatch(dsl)
			if len(remJurisMatches) > 1 {
				innerJurisMatches := jurisRegex.FindAllStringSubmatch(remJurisMatches[1], -1)
				for _, m := range innerJurisMatches {
					delete(jurisSet, m[1])
				}
			}
		}
	}

	reqs := &KYCRequirements{
		Documents:     make([]string, 0, len(docSet)),
		Jurisdictions: make([]string, 0, len(jurisSet)),
	}
	for d := range docSet {
		reqs.Documents = append(reqs.Documents, d)
	}
	for j := range jurisSet {
		reqs.Jurisdictions = append(reqs.Jurisdictions, j)
	}

	return reqs, nil
}

// --- State 3: Discover Services ---

// ServiceDiscoveryPlan holds data for the service discovery step
type ServiceDiscoveryPlan struct {
	ProductServices map[string][]store.Service
}

func AddDiscoveredServices(currentDSL string, plan ServiceDiscoveryPlan) (string, error) {
	var b strings.Builder
	b.WriteString(currentDSL)
	b.WriteString("\n\n")

	// Append (services.discover)
	b.WriteString("(services.discover\n")
	for product, services := range plan.ProductServices {
		b.WriteString(fmt.Sprintf("  (for.product %q\n", product))
		// Use a map to de-duplicate service names
		serviceNames := make(map[string]bool)
		for _, service := range services {
			serviceNames[service.Name] = true
		}
		for serviceName := range serviceNames {
			b.WriteString(fmt.Sprintf("    (service %q)\n", serviceName))
		}
		b.WriteString("  )\n")
	}
	b.WriteString(")")

	return b.String(), nil
}

// Simple parser for POC
var serviceRegex = regexp.MustCompile(`\(service\s+"(.*?)"\)`)

func ParseServiceNames(dsl string) ([]string, error) {
	matches := serviceRegex.FindAllStringSubmatch(dsl, -1)
	if len(matches) == 0 {
		return nil, fmt.Errorf("no (service ...) blocks found in DSL")
	}

	serviceNames := make(map[string]bool) // Use map to de-duplicate
	for _, match := range matches {
		if len(match) >= 2 {
			serviceNames[match[1]] = true
		}
	}

	names := make([]string, 0, len(serviceNames))
	for name := range serviceNames {
		names = append(names, name)
	}

	if len(names) == 0 {
		return nil, fmt.Errorf("no service names found")
	}
	return names, nil
}

// --- State 4: Discover Resources ---

// ResourceDiscoveryPlan holds data for the resource discovery step
// This now uses the rich dictionary.Attribute
type ResourceDiscoveryPlan struct {
	ServiceResources   map[string][]store.ProdResource
	ResourceAttributes map[string][]dictionary.Attribute
}

func AddDiscoveredResources(currentDSL string, plan ResourceDiscoveryPlan) (string, error) {
	var b strings.Builder
	b.WriteString(currentDSL)
	b.WriteString("\n\n")

	// Append (resources.plan)
	b.WriteString("(resources.plan\n")

	// Use a map to find all unique resources
	allResources := make(map[string]store.ProdResource)
	for _, resources := range plan.ServiceResources {
		for _, res := range resources {
			allResources[res.ResourceID] = res
		}
	}

	for _, resource := range allResources {
		b.WriteString(fmt.Sprintf("  (resource.create %q\n", resource.Name))
		b.WriteString(fmt.Sprintf("    (owner %q)\n", resource.Owner))

		attributes := plan.ResourceAttributes[resource.DictionaryGroup]
		for i := range attributes {
			// Use simple variable references: (var (attr-id "uuid"))
			b.WriteString(fmt.Sprintf("    %s\n", VarByAttrIDSimple(attributes[i].AttributeID)))
		}
		b.WriteString("  )\n")
	}
	b.WriteString(")")

	return b.String(), nil
}

func RenderBindings(assign map[string]string) string {
	var b strings.Builder
	b.WriteString("(values.bind\n")
	for id, raw := range assign {
		// raw is a JSON literal; print as string if it's JSON string
		var s string
		if err := json.Unmarshal([]byte(raw), &s); err == nil {
			b.WriteString(fmt.Sprintf(`  (bind (attr-id %q) (value %q))`+"\n", id, s))
		} else {
			b.WriteString(fmt.Sprintf(`  (bind (attr-id %q) (value %s))`+"\n", id, raw))
		}
	}
	b.WriteString(")")
	return b.String()
}

// --- State 6: Populate Attributes ---

// VarByAttrID creates canonical variable form
func VarByAttrID(name, id string) string {
	return fmt.Sprintf(`(var (name %q) (id %q))`, name, id)
}

// VarByAttrIDSimple generates a simple variable reference with just the attribute ID
func VarByAttrIDSimple(id string) string {
	return fmt.Sprintf(`(var (attr-id %q))`, id)
}

// Accept (VAR_<code>) or (VAR_<uuid>) and normalize to canonical form.
// This is a lightweight regex now; Rust/nom will do the real job later.
var reVarSym = regexp.MustCompile(`\(VAR_([A-Za-z0-9\._\-]+)\)`)

func NormalizeVars(dsl string, resolve func(sym string) (attr *dictionary.Attribute, ok bool)) string {
	return reVarSym.ReplaceAllStringFunc(dsl, func(m string) string {
		sym := reVarSym.FindStringSubmatch(m)[1] // code or uuid
		if attr, ok := resolve(sym); ok {
			return VarByAttrIDSimple(attr.AttributeID)
		}
		// leave unknown symbols untouched (keeps idempotence)
		return m
	})
}

// Extract all canonical (var (attr-id "...")) occurrences
var reVarAttrID = regexp.MustCompile(`\(\s*var\s*\(\s*attr-id\s*"([^"]+)"\s*\)\s*\)`)

func ExtractVarAttrIDs(dsl string) []string {
    out := []string{}
    for _, m := range reVarAttrID.FindAllStringSubmatch(dsl, -1) {
        out = append(out, m[1]) // m[1] is the UUID
    }
    return out
}

// Also capture standalone (attr-id "<uuid>") tokens anywhere in the DSL
var reAttrIDToken = regexp.MustCompile(`\(\s*attr-id\s*"([^"]+)"\s*\)`)

// ExtractAttributeIDs returns unique attribute IDs referenced via either canonical var
// or standalone (attr-id ...) tokens.
func ExtractAttributeIDs(dsl string) []string {
    seen := make(map[string]bool)
    ids := []string{}

    for _, id := range ExtractVarAttrIDs(dsl) {
        if !seen[id] {
            seen[id] = true
            ids = append(ids, id)
        }
    }

    for _, m := range reAttrIDToken.FindAllStringSubmatch(dsl, -1) {
        if len(m) >= 2 {
            id := m[1]
            if !seen[id] {
                seen[id] = true
                ids = append(ids, id)
            }
        }
    }

    return ids
}

// AttributeReference represents an attribute variable found in DSL
type AttributeReference struct {
	Name         string // e.g., "onboard.cbu_id"
	AttributeID  string // UUID from dictionary table
	VariableForm string // e.g., "(var (name \"onboard.cbu_id\") (id \"uuid\"))"
}

// AttributeValue represents a populated attribute value
type AttributeValue struct {
	AttributeID string
	Name        string
	Value       string
	SourceInfo  map[string]interface{}
}

// ParseAttributeReferences finds all (var (name "...") (id "...")) references in DSL
func ParseAttributeReferences(dsl string) ([]AttributeReference, error) {
	var refs []AttributeReference
	matches := reVarAttrID.FindAllStringSubmatch(dsl, -1)

	for _, match := range matches {
		if len(match) == 2 {
			id := match[1]
			refs = append(refs, AttributeReference{
				Name:         "", // Empty for simple attr-id format
				AttributeID:  id,
				VariableForm: VarByAttrIDSimple(id),
			})
		}
	}

	return refs, nil
}

// PopulateAttributeValues fetches runtime values for attribute variables
func PopulateAttributeValues(ctx context.Context, ds datastore.DataStore, onboardingID string, refs []AttributeReference) ([]AttributeValue, error) {
	var values []AttributeValue

	for _, ref := range refs {
		// Get attribute definition from dictionary by UUID
		attr, err := ds.GetDictionaryAttributeByID(ctx, ref.AttributeID)
		if err != nil {
			return nil, fmt.Errorf("failed to get attribute %s: %w", ref.AttributeID, err)
		}

		// Fetch runtime value based on source metadata
		value, sourceInfo, err := fetchAttributeValue(ctx, ds, onboardingID, attr)
		if err != nil {
			return nil, fmt.Errorf("failed to fetch value for %s: %w", ref.Name, err)
		}

		// Store the value in attribute_values table using the new interface
		valueJSON, _ := json.Marshal(value)
		err = ds.UpsertAttributeValue(ctx, onboardingID, 0, attr.AttributeID, valueJSON, "populated", sourceInfo)
		if err != nil {
			return nil, fmt.Errorf("failed to store value for %s: %w", ref.Name, err)
		}

		values = append(values, AttributeValue{
			AttributeID: attr.AttributeID,
			Name:        ref.Name,
			Value:       value,
			SourceInfo:  sourceInfo,
		})
	}

	return values, nil
}

// fetchAttributeValue retrieves the actual value based on source metadata
func fetchAttributeValue(ctx context.Context, ds datastore.DataStore, onboardingID string, attr *dictionary.Attribute) (string, map[string]interface{}, error) {
	// For POC, implement simple value fetching based on attribute name
	switch attr.Name {
	case "onboard.cbu_id":
		// Extract CBU ID from onboarding ID (assuming they're the same for POC)
		return onboardingID, map[string]interface{}{"type": "direct", "source": "onboarding_id"}, nil

	case "entity.legal_name":
		// Look up CBU name from cbus table
		cbu, err := ds.GetCBUByName(ctx, onboardingID)
		if err != nil {
			return "", nil, fmt.Errorf("failed to get CBU: %w", err)
		}
		return cbu.Description, map[string]interface{}{"type": "database", "table": "cbus", "field": "description"}, nil

	case "entity.domicile":
		// Extract domicile from nature_purpose for POC
		cbu, err := ds.GetCBUByName(ctx, onboardingID)
		if err != nil {
			return "", nil, fmt.Errorf("failed to get CBU: %w", err)
		}
		// Simple extraction - look for "domiciled in XX"
		domicileRegex := regexp.MustCompile(`domiciled in ([A-Z]{2})`)
		matches := domicileRegex.FindStringSubmatch(cbu.NaturePurpose)
		if len(matches) >= 2 {
			return matches[1], map[string]interface{}{"type": "extracted", "source": "nature_purpose"}, nil
		}
		return "Unknown", map[string]interface{}{"type": "default"}, nil

	default:
		// Default placeholder for unknown attributes
		return fmt.Sprintf("VALUE_%s", attr.Name), map[string]interface{}{"type": "placeholder"}, nil
	}
}

// AddPopulatedAttributes generates final DSL by substituting variables with actual values
func AddPopulatedAttributes(currentDSL string, values []AttributeValue) (string, error) {
	result := currentDSL

	// Replace each variable with its actual value
	for _, val := range values {
		varPattern := fmt.Sprintf("(VAR_%s)", val.Name)
		replacement := fmt.Sprintf("(attr.%s %q)", val.Name, val.Value)
		result = strings.ReplaceAll(result, varPattern, replacement)
	}

	// Add summary block at the end
	var b strings.Builder
	b.WriteString(result)
	b.WriteString("\n\n")
	b.WriteString("(attributes.populated\n")
	for _, val := range values {
		b.WriteString(fmt.Sprintf("  ; %s = %q\n", val.Name, val.Value))
	}
	b.WriteString(")")

	return b.String(), nil
}
