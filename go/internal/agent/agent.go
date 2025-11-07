package agent

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"strings"

	"dsl-ob-poc/internal/dsl"

	"github.com/google/generative-ai-go/genai"
	"google.golang.org/api/option"
)

// Agent wraps the Gemini client and model used for KYC discovery.
type Agent struct {
	client *genai.Client
	model  *genai.GenerativeModel
}

// KYCResponse is the structured JSON we expect from the LLM.
type KYCResponse struct {
	RequiredDocuments []string `json:"required_documents"`
	Jurisdictions     []string `json:"jurisdictions"`
}

// NewAgent initializes the Gemini or other client. If the API key is empty,
// the caller receives a nil Agent and no error so that commands can
// decide how to handle missing configuration.
func NewAgent(ctx context.Context, apiKey string) (*Agent, error) {
	if apiKey == "" {
		return nil, nil
	}

	client, err := genai.NewClient(ctx, option.WithAPIKey(apiKey))
	if err != nil {
		return nil, fmt.Errorf("failed to create genai client: %w", err)
	}

	model := client.GenerativeModel("gemini-2.5-flash-preview-09-2025")
	model.SafetySettings = []*genai.SafetySetting{
		{
			Category:  genai.HarmCategoryHarassment,
			Threshold: genai.HarmBlockNone,
		},
		{
			Category:  genai.HarmCategoryHateSpeech,
			Threshold: genai.HarmBlockNone,
		},
		{
			Category:  genai.HarmCategorySexuallyExplicit,
			Threshold: genai.HarmBlockNone,
		},
		{
			Category:  genai.HarmCategoryDangerousContent,
			Threshold: genai.HarmBlockNone,
		},
	}

	return &Agent{
		client: client,
		model:  model,
	}, nil
}

// Close releases underlying resources.
func (a *Agent) Close() {
	if a == nil || a.client == nil {
		return
	}
	if err := a.client.Close(); err != nil {
		log.Printf("warning: failed to close Gemini client: %v", err)
	}
}

// CallKYCAgent is the core "agentic" function.
func (a *Agent) CallKYCAgent(ctx context.Context, naturePurpose string, products []string) (*dsl.KYCRequirements, error) {
	if a == nil || a.model == nil {
		return nil, fmt.Errorf("ai agent is not initialized")
	}

	systemPrompt := `You are an expert KYC/AML Compliance Officer for a major global bank.
Your job is to analyze a new client's "nature and purpose" and their "requested products" to determine the *minimum* required KYC documents and all relevant jurisdictions.

RULES:
1.  Analyze the "nature and purpose" for entity type and domicile (e.g., "UCITS fund domiciled in LU" -> Domicile is "LU").
2.  Analyze the products for regulatory impact (e.g., "TRANSFER_AGENT" implies AML checks on investors).
3.  Respond ONLY with a single, minified JSON object. Do not include markdown ticks, "json", or any other conversational text.
4.  The JSON format MUST be: {"required_documents": ["doc1", "doc2"], "jurisdictions": ["jur1", "jur2"]}

EXAMPLES:
- Input: "UCITS equity fund domiciled in LU", Products: ["CUSTODY"]
- Output: {"required_documents":["CertificateOfIncorporation","ArticlesOfAssociation","W8BEN-E"],"jurisdictions":["LU"]}
- Input: "US-based hedge fund", Products: ["TRANSFER_AGENT", "CUSTODY"]
- Output: {"required_documents":["CertificateOfLimitedPartnership","PartnershipAgreement","W9","AMLPolicy"],"jurisdictions":["US"]}
`

	var userPrompt string
	if len(products) == 0 {
		userPrompt = fmt.Sprintf(
			`Nature and Purpose: %q, Products: []`,
			naturePurpose,
		)
	} else {
		// Quote each product individually so the placeholder needn't be wrapped in quotes.
		quoted := make([]string, len(products))
		for i, p := range products {
			quoted[i] = fmt.Sprintf("%q", p)
		}
		productPayload := strings.Join(quoted, ", ")
		userPrompt = fmt.Sprintf(
			`Nature and Purpose: %q, Products: [%s]`,
			naturePurpose,
			productPayload,
		)
	}

	a.model.SystemInstruction = &genai.Content{Parts: []genai.Part{genai.Text(systemPrompt)}}

	resp, err := a.model.GenerateContent(ctx, genai.Text(userPrompt))
	if err != nil {
		return nil, fmt.Errorf("failed to generate content: %w", err)
	}

	if len(resp.Candidates) == 0 || resp.Candidates[0] == nil || len(resp.Candidates[0].Content.Parts) == 0 {
		return nil, fmt.Errorf("no response from agent: %v", resp)
	}

	part := resp.Candidates[0].Content.Parts[0]
	textPart, ok := part.(genai.Text)
	if !ok {
		return nil, fmt.Errorf("unexpected response type from agent: %T", part)
	}

	log.Printf("AI Agent Raw Response: %s", textPart)

	var kycResp KYCResponse
	if uErr := json.Unmarshal([]byte(textPart), &kycResp); uErr != nil {
		return nil, fmt.Errorf("failed to parse agent's JSON response: %w (response was: %s)", uErr, textPart)
	}

	return &dsl.KYCRequirements{
		Jurisdictions: kycResp.Jurisdictions,
		Documents:     kycResp.RequiredDocuments,
	}, nil
}
