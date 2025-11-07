package seed

import (
	"github.com/google/uuid"

	"dsl-ob-poc/internal/dictionary"
)

// GenerateKYCAttributes creates a comprehensive set of KYC and onboarding attributes
func GenerateKYCAttributes() []dictionary.Attribute {
	return []dictionary.Attribute{
		{
			AttributeID:     uuid.New().String(),
			Name:            "investor.legal_name",
			LongDescription: "Legal full name of the investor",
			GroupID:         "investor_identity",
			Mask:            "STRING",
			Domain:          "KYC",
			Tags:            []string{"PII", "IDENTITY"},
			Sensitivity:     "HIGH",
			Constraints:     []string{"REQUIRED", "MIN_LENGTH:2", "MAX_LENGTH:100"},
			Source: dictionary.SourceMetadata{
				Primary: "INVESTOR_DOCUMENT",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "INVESTOR_PROFILE",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "investor.type",
			LongDescription: "Type of investor (proper person, corporate, institutional)",
			GroupID:         "investor_classification",
			Mask:            "ENUM",
			Domain:          "KYC",
			Tags:            []string{"CLASSIFICATION"},
			Constraints:     []string{"REQUIRED", "ENUM:PROPER_PERSON,CORPORATE,INSTITUTIONAL"},
			Source: dictionary.SourceMetadata{
				Primary: "INVESTOR_REGISTRATION",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "INVESTOR_PROFILE",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "investor.nationality",
			LongDescription: "Nationality of the investor (ISO 3166-1 alpha-2 country code)",
			GroupID:         "investor_identity",
			Mask:            "STRING",
			Domain:          "KYC",
			Tags:            []string{"GEOGRAPHIC"},
			Constraints:     []string{"REQUIRED", "REGEX:^[A-Z]{2}$"},
			Source: dictionary.SourceMetadata{
				Primary: "PASSPORT",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "INVESTOR_PROFILE",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "kyc.risk_rating",
			LongDescription: "Risk assessment rating for KYC compliance",
			GroupID:         "risk_assessment",
			Mask:            "ENUM",
			Domain:          "KYC",
			Tags:            []string{"COMPLIANCE", "RISK"},
			Constraints:     []string{"REQUIRED", "ENUM:LOW,MEDIUM,HIGH"},
			Source: dictionary.SourceMetadata{
				Primary:   "KYC_ASSESSMENT",
				Secondary: "RISK_ENGINE",
			},
			Derivation: &dictionary.DerivationRule{
				Type:               dictionary.DerivationTypeCalculated,
				SourceAttributeIDs: []string{"document_verification", "background_check", "financial_history"},
				Transformation: &dictionary.TransformationRule{
					Type:   "RISK_CALCULATION",
					Params: map[string]string{"method": "weighted_average"},
				},
			},
			Sink: dictionary.SinkMetadata{
				Primary: "COMPLIANCE_REPORT",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "document.type",
			LongDescription: "Type of identification document submitted",
			GroupID:         "document_verification",
			Mask:            "ENUM",
			Domain:          "KYC",
			Tags:            []string{"PII", "IDENTITY"},
			Constraints:     []string{"REQUIRED", "ENUM:PASSPORT,DRIVER_LICENSE,NATIONAL_ID,RESIDENCE_PERMIT"},
			Source: dictionary.SourceMetadata{
				Primary: "DOCUMENT_UPLOAD",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "DOCUMENT_REGISTRY",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "investor.domicile",
			LongDescription: "Country of legal residence (ISO 3166-1 alpha-2 country code)",
			GroupID:         "investor_identity",
			Mask:            "STRING",
			Domain:          "KYC",
			Tags:            []string{"GEOGRAPHIC", "TAX_JURISDICTION"},
			Constraints:     []string{"REQUIRED", "REGEX:^[A-Z]{2}$"},
			Source: dictionary.SourceMetadata{
				Primary: "PROOF_OF_ADDRESS",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "INVESTOR_PROFILE",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "kyc.status",
			LongDescription: "Current status of KYC verification process",
			GroupID:         "compliance",
			Mask:            "ENUM",
			Domain:          "KYC",
			Tags:            []string{"COMPLIANCE", "WORKFLOW"},
			Constraints:     []string{"REQUIRED", "ENUM:PENDING,IN_PROGRESS,VERIFIED,REJECTED"},
			Source: dictionary.SourceMetadata{
				Primary: "KYC_WORKFLOW",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "COMPLIANCE_REPORT",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "pep.status",
			LongDescription: "Politically Exposed Person (PEP) status",
			GroupID:         "risk_assessment",
			Mask:            "BOOLEAN",
			Domain:          "KYC",
			Tags:            []string{"COMPLIANCE", "HIGH_RISK"},
			Constraints:     []string{"REQUIRED"},
			Source: dictionary.SourceMetadata{
				Primary: "PEP_DATABASE_LOOKUP",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "COMPLIANCE_REPORT",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "sanctions.check",
			LongDescription: "International sanctions screening result",
			GroupID:         "risk_assessment",
			Mask:            "ENUM",
			Domain:          "KYC",
			Tags:            []string{"COMPLIANCE", "LEGAL_RISK"},
			Constraints:     []string{"REQUIRED", "ENUM:CLEARED,FLAGGED,UNDER_REVIEW"},
			Source: dictionary.SourceMetadata{
				Primary: "SANCTIONS_DATABASE",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "COMPLIANCE_REPORT",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "tax.identification_number",
			LongDescription: "Tax identification number or equivalent",
			GroupID:         "tax_compliance",
			Mask:            "STRING",
			Domain:          "KYC",
			Tags:            []string{"TAX_JURISDICTION", "FINANCIAL"},
			Sensitivity:     "HIGH",
			Constraints:     []string{"OPTIONAL", "MAX_LENGTH:50"},
			Source: dictionary.SourceMetadata{
				Primary: "TAX_DOCUMENT",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "TAX_REGISTRY",
			},
		},
	}
}
