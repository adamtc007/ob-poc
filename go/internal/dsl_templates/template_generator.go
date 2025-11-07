package dsl_templates

import (
	"fmt"
	"strings"
	"time"

	"github.com/google/uuid"
)

// DSLTemplateGenerator provides flexible DSL generation for different domains
type DSLTemplateGenerator struct {
	domainTemplates map[string]map[string]func(map[string]interface{}) string
}

// NewDSLTemplateGenerator creates a new template generator with predefined domain templates
func NewDSLTemplateGenerator() *DSLTemplateGenerator {
	return &DSLTemplateGenerator{
		domainTemplates: map[string]map[string]func(map[string]interface{}) string{
			"investor": {
				"CREATED":             generateInvestorCreatedDSL,
				"KYC_STARTED":         generateInvestorKYCStartedDSL,
				"KYC_COMPLETED":       generateInvestorKYCCompletedDSL,
				"DOCUMENTS_COLLECTED": generateInvestorDocumentsCollectedDSL,
			},
			"hedge-fund": {
				"FUND_CREATED":         generateHedgeFundCreatedDSL,
				"SUBSCRIPTION_STARTED": generateHedgeFundSubscriptionStartedDSL,
				"RISK_ASSESSMENT":      generateHedgeFundRiskAssessmentDSL,
			},
			"trust": {
				"CREATED":               generateTrustCreatedDSL,
				"ASSETS_TRANSFERRED":    generateTrustAssetsTransferredDSL,
				"BENEFICIARIES_DEFINED": generateTrustBeneficiariesDefinedDSL,
			},
		},
	}
}

// GenerateDSL creates a domain and state-specific DSL based on provided parameters
func (g *DSLTemplateGenerator) GenerateDSL(domain, state string, params map[string]interface{}) (string, error) {
	domainTemplates, exists := g.domainTemplates[domain]
	if !exists {
		return "", fmt.Errorf("unsupported domain: %s", domain)
	}

	templateFunc, exists := domainTemplates[state]
	if !exists {
		return "", fmt.Errorf("unsupported state %s for domain %s", state, domain)
	}

	// Ensure required params are present
	if err := validateParams(domain, state, params); err != nil {
		return "", err
	}

	return templateFunc(params), nil
}

// validateParams ensures required parameters are present for a specific domain and state
func validateParams(domain, state string, params map[string]interface{}) error {
	switch domain {
	case "investor":
		switch state {
		case "CREATED":
			requiredKeys := []string{"name", "type"}
			return checkRequiredKeys(params, requiredKeys)
		case "KYC_STARTED":
			requiredKeys := []string{"document", "jurisdiction"}
			return checkRequiredKeys(params, requiredKeys)
		}
	case "hedge-fund":
		switch state {
		case "FUND_CREATED":
			requiredKeys := []string{"name", "strategy"}
			return checkRequiredKeys(params, requiredKeys)
		case "SUBSCRIPTION_STARTED":
			requiredKeys := []string{"amount", "currency"}
			return checkRequiredKeys(params, requiredKeys)
		}
	case "trust":
		switch state {
		case "CREATED":
			requiredKeys := []string{"type", "grantor"}
			return checkRequiredKeys(params, requiredKeys)
		case "ASSETS_TRANSFERRED":
			requiredKeys := []string{"asset", "value"}
			return checkRequiredKeys(params, requiredKeys)
		}
	}
	return nil
}

// checkRequiredKeys verifies that all required keys are present in the params map
func checkRequiredKeys(params map[string]interface{}, requiredKeys []string) error {
	for _, key := range requiredKeys {
		if _, exists := params[key]; !exists {
			return fmt.Errorf("missing required parameter: %s", key)
		}
	}
	return nil
}

// Utility function to generate a new UUID
func generateUUID() string {
	return uuid.New().String()
}

// Investor Domain Templates
func generateInvestorCreatedDSL(params map[string]interface{}) string {
	return fmt.Sprintf(`(investor.create
		(investor.id "%s")
		(name "%s")
		(type "%s")
		(created-at "%s")
	)`, generateUUID(), params["name"], params["type"], time.Now().Format(time.RFC3339))
}

func generateInvestorKYCStartedDSL(params map[string]interface{}) string {
	return fmt.Sprintf(`(kyc.start
		(document "%s")
		(jurisdiction "%s")
		(started-at "%s")
	)`, params["document"], params["jurisdiction"], time.Now().Format(time.RFC3339))
}

func generateInvestorKYCCompletedDSL(params map[string]interface{}) string {
	return fmt.Sprintf(`(kyc.complete
		(status "APPROVED")
		(risk-rating "%s")
		(completed-at "%s")
	)`, params["risk_rating"], time.Now().Format(time.RFC3339))
}

func generateInvestorDocumentsCollectedDSL(params map[string]interface{}) string {
	documents := params["documents"].([]string)
	return fmt.Sprintf(`(documents.collect
		(type "%s")
		(documents (%s))
		(collected-at "%s")
	)`, params["type"], strings.Join(documents, " "), time.Now().Format(time.RFC3339))
}

// Hedge Fund Domain Templates
func generateHedgeFundCreatedDSL(params map[string]interface{}) string {
	return fmt.Sprintf(`(fund.create
		(fund.id "%s")
		(name "%s")
		(strategy "%s")
		(created-at "%s")
	)`, generateUUID(), params["name"], params["strategy"], time.Now().Format(time.RFC3339))
}

func generateHedgeFundSubscriptionStartedDSL(params map[string]interface{}) string {
	return fmt.Sprintf(`(subscription.start
		(amount "%s")
		(currency "%s")
		(initiated-at "%s")
	)`, params["amount"], params["currency"], time.Now().Format(time.RFC3339))
}

func generateHedgeFundRiskAssessmentDSL(params map[string]interface{}) string {
	return fmt.Sprintf(`(risk.assess
		(category "%s")
		(score "%s")
		(mitigations (%s))
		(assessed-at "%s")
	)`, params["category"], params["score"], strings.Join(params["mitigations"].([]string), " "), time.Now().Format(time.RFC3339))
}

// Trust Domain Templates
func generateTrustCreatedDSL(params map[string]interface{}) string {
	return fmt.Sprintf(`(trust.create
		(trust.id "%s")
		(type "%s")
		(grantor "%s")
		(created-at "%s")
	)`, generateUUID(), params["type"], params["grantor"], time.Now().Format(time.RFC3339))
}

func generateTrustAssetsTransferredDSL(params map[string]interface{}) string {
	return fmt.Sprintf(`(assets.transfer
		(asset "%s")
		(value "%s")
		(transferred-at "%s")
	)`, params["asset"], params["value"], time.Now().Format(time.RFC3339))
}

func generateTrustBeneficiariesDefinedDSL(params map[string]interface{}) string {
	primaryBeneficiaries := params["primary"].([]string)
	contingentBeneficiaries := params["contingent"].([]string)
	return fmt.Sprintf(`(beneficiaries.define
		(primary (%s))
		(contingent (%s))
		(defined-at "%s")
	)`,
		strings.Join(primaryBeneficiaries, " "),
		strings.Join(contingentBeneficiaries, " "),
		time.Now().Format(time.RFC3339))
}
