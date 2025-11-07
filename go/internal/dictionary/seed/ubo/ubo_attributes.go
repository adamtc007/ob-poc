package ubo

import (
	"github.com/google/uuid"

	"dsl-ob-poc/internal/dictionary"
)

// GenerateUBOAttributes creates a comprehensive set of Ultimate Beneficial Ownership (UBO) attributes
// for the DSL-as-State system. These attributes support the full UBO identification, verification,
// and monitoring workflow required for financial services compliance.
func GenerateUBOAttributes() []dictionary.Attribute {
	return []dictionary.Attribute{
		// =============================================================================
		// LEGAL ENTITY ATTRIBUTES
		// =============================================================================
		{
			AttributeID:     uuid.New().String(),
			Name:            "entity.legal_name",
			LongDescription: "Official legal name of the corporate entity as registered",
			GroupID:         "entity_identity",
			Mask:            "STRING",
			Domain:          "UBO",
			Tags:            []string{"ENTITY", "IDENTITY", "LEGAL"},
			Sensitivity:     "MEDIUM",
			Constraints:     []string{"REQUIRED", "MIN_LENGTH:2", "MAX_LENGTH:200"},
			Source: dictionary.SourceMetadata{
				Primary:   "CERTIFICATE_OF_INCORPORATION",
				Secondary: "CORPORATE_REGISTRY",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "ENTITY_REGISTRY",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "entity.jurisdiction",
			LongDescription: "Country of incorporation or establishment (ISO 3166-1 alpha-2)",
			GroupID:         "entity_identity",
			Mask:            "STRING",
			Domain:          "UBO",
			Tags:            []string{"ENTITY", "GEOGRAPHIC", "LEGAL"},
			Constraints:     []string{"REQUIRED", "REGEX:^[A-Z]{2}$"},
			Source: dictionary.SourceMetadata{
				Primary: "CERTIFICATE_OF_INCORPORATION",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "ENTITY_REGISTRY",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "entity.type",
			LongDescription: "Legal form of the entity (corporation, LLC, partnership, trust, etc.)",
			GroupID:         "entity_classification",
			Mask:            "ENUM",
			Domain:          "UBO",
			Tags:            []string{"ENTITY", "CLASSIFICATION"},
			Constraints:     []string{"REQUIRED", "ENUM:CORPORATION,LLC,PARTNERSHIP,TRUST,FOUNDATION"},
			Source: dictionary.SourceMetadata{
				Primary: "CORPORATE_REGISTRY",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "ENTITY_REGISTRY",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "entity.registration_number",
			LongDescription: "Official registration or identification number assigned by jurisdiction",
			GroupID:         "entity_identity",
			Mask:            "STRING",
			Domain:          "UBO",
			Tags:            []string{"ENTITY", "IDENTITY", "REGISTRATION"},
			Constraints:     []string{"OPTIONAL", "MAX_LENGTH:50"},
			Source: dictionary.SourceMetadata{
				Primary: "CORPORATE_REGISTRY",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "ENTITY_REGISTRY",
			},
		},

		// =============================================================================
		// OWNERSHIP LINK ATTRIBUTES
		// =============================================================================
		{
			AttributeID:     uuid.New().String(),
			Name:            "ownership.percentage",
			LongDescription: "Percentage of ownership interest (0.00 to 100.00)",
			GroupID:         "ownership_structure",
			Mask:            "DECIMAL",
			Domain:          "UBO",
			Tags:            []string{"OWNERSHIP", "PERCENTAGE"},
			Constraints:     []string{"REQUIRED", "MIN:0.00", "MAX:100.00", "PRECISION:2"},
			Source: dictionary.SourceMetadata{
				Primary: "SHAREHOLDING_STATEMENT",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "OWNERSHIP_REGISTRY",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "ownership.link_type",
			LongDescription: "Type of ownership relationship between entities",
			GroupID:         "ownership_structure",
			Mask:            "ENUM",
			Domain:          "UBO",
			Tags:            []string{"OWNERSHIP", "CLASSIFICATION"},
			Constraints:     []string{"REQUIRED", "ENUM:DIRECT_SHARE,INDIRECT_SHARE,VOTING_RIGHT,CONTROL_AGREEMENT"},
			Source: dictionary.SourceMetadata{
				Primary: "OWNERSHIP_STRUCTURE_CHART",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "OWNERSHIP_REGISTRY",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "ownership.voting_rights",
			LongDescription: "Percentage of voting rights held (0.00 to 100.00)",
			GroupID:         "ownership_structure",
			Mask:            "DECIMAL",
			Domain:          "UBO",
			Tags:            []string{"OWNERSHIP", "VOTING", "CONTROL"},
			Constraints:     []string{"OPTIONAL", "MIN:0.00", "MAX:100.00", "PRECISION:2"},
			Source: dictionary.SourceMetadata{
				Primary: "VOTING_AGREEMENT",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "OWNERSHIP_REGISTRY",
			},
		},

		// =============================================================================
		// UBO IDENTIFICATION ATTRIBUTES
		// =============================================================================
		{
			AttributeID:     uuid.New().String(),
			Name:            "ubo.natural_proper_person_id",
			LongDescription: "Unique identifier for the natural person who is a UBO",
			GroupID:         "ubo_identification",
			Mask:            "UUID",
			Domain:          "UBO",
			Tags:            []string{"UBO", "IDENTITY", "PROPER_PERSON"},
			Constraints:     []string{"REQUIRED"},
			Source: dictionary.SourceMetadata{
				Primary: "UBO_IDENTIFICATION_PROCESS",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "UBO_REGISTRY",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "ubo.relationship_type",
			LongDescription: "Type of relationship that qualifies person as UBO",
			GroupID:         "ubo_identification",
			Mask:            "ENUM",
			Domain:          "UBO",
			Tags:            []string{"UBO", "RELATIONSHIP"},
			Constraints:     []string{"REQUIRED", "ENUM:DIRECT_OWNERSHIP,INDIRECT_OWNERSHIP,CONTROL_PRONG"},
			Source: dictionary.SourceMetadata{
				Primary: "UBO_ANALYSIS",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "UBO_REGISTRY",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "ubo.ownership_threshold",
			LongDescription: "Ownership threshold percentage used for UBO determination (typically 25%)",
			GroupID:         "ubo_configuration",
			Mask:            "DECIMAL",
			Domain:          "UBO",
			Tags:            []string{"UBO", "THRESHOLD", "CONFIGURATION"},
			Constraints:     []string{"REQUIRED", "MIN:0.01", "MAX:100.00", "PRECISION:2"},
			Source: dictionary.SourceMetadata{
				Primary: "REGULATORY_REQUIREMENTS",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "COMPLIANCE_CONFIGURATION",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "ubo.total_ownership",
			LongDescription: "Total aggregated ownership percentage including direct and indirect",
			GroupID:         "ubo_calculation",
			Mask:            "DECIMAL",
			Domain:          "UBO",
			Tags:            []string{"UBO", "CALCULATED", "OWNERSHIP"},
			Constraints:     []string{"REQUIRED", "MIN:0.00", "MAX:100.00", "PRECISION:4"},
			Source: dictionary.SourceMetadata{
				Primary: "UBO_CALCULATION_ENGINE",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "UBO_REGISTRY",
			},
		},

		// =============================================================================
		// UBO VERIFICATION ATTRIBUTES
		// =============================================================================
		{
			AttributeID:     uuid.New().String(),
			Name:            "ubo.verification_status",
			LongDescription: "Status of UBO identity verification process",
			GroupID:         "ubo_verification",
			Mask:            "ENUM",
			Domain:          "UBO",
			Tags:            []string{"UBO", "VERIFICATION", "STATUS"},
			Constraints:     []string{"REQUIRED", "ENUM:PENDING,IN_PROGRESS,VERIFIED,FAILED,EXPIRED"},
			Source: dictionary.SourceMetadata{
				Primary: "UBO_VERIFICATION_WORKFLOW",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "UBO_REGISTRY",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "ubo.screening_result",
			LongDescription: "Result of sanctions and PEP screening for UBO",
			GroupID:         "ubo_screening",
			Mask:            "ENUM",
			Domain:          "UBO",
			Tags:            []string{"UBO", "SCREENING", "COMPLIANCE"},
			Constraints:     []string{"REQUIRED", "ENUM:CLEARED,FLAGGED,UNDER_REVIEW,BLOCKED"},
			Source: dictionary.SourceMetadata{
				Primary:   "SANCTIONS_DATABASE",
				Secondary: "PEP_DATABASE",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "COMPLIANCE_REGISTRY",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "ubo.pep_status",
			LongDescription: "Politically Exposed Person status of the UBO",
			GroupID:         "ubo_screening",
			Mask:            "ENUM",
			Domain:          "UBO",
			Tags:            []string{"UBO", "PEP", "HIGH_RISK"},
			Constraints:     []string{"REQUIRED", "ENUM:NOT_PEP,DOMESTIC_PEP,FOREIGN_PEP,FAMILY_MEMBER"},
			Source: dictionary.SourceMetadata{
				Primary: "PEP_DATABASE",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "COMPLIANCE_REGISTRY",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "ubo.sanctions_hit",
			LongDescription: "Indicates if UBO appears on any sanctions lists",
			GroupID:         "ubo_screening",
			Mask:            "BOOLEAN",
			Domain:          "UBO",
			Tags:            []string{"UBO", "SANCTIONS", "LEGAL_RISK"},
			Constraints:     []string{"REQUIRED"},
			Source: dictionary.SourceMetadata{
				Primary: "SANCTIONS_DATABASE",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "COMPLIANCE_REGISTRY",
			},
		},

		// =============================================================================
		// UBO RISK ASSESSMENT ATTRIBUTES
		// =============================================================================
		{
			AttributeID:     uuid.New().String(),
			Name:            "ubo.risk_rating",
			LongDescription: "Overall risk rating for the UBO based on multiple factors",
			GroupID:         "ubo_risk_assessment",
			Mask:            "ENUM",
			Domain:          "UBO",
			Tags:            []string{"UBO", "RISK", "ASSESSMENT"},
			Constraints:     []string{"REQUIRED", "ENUM:LOW,MEDIUM,HIGH,VERY_HIGH,PROHIBITED"},
			Source: dictionary.SourceMetadata{
				Primary: "UBO_RISK_ENGINE",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "RISK_REGISTRY",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "ubo.jurisdiction_risk",
			LongDescription: "Risk rating of the UBO's country of residence/nationality",
			GroupID:         "ubo_risk_assessment",
			Mask:            "ENUM",
			Domain:          "UBO",
			Tags:            []string{"UBO", "GEOGRAPHIC", "RISK"},
			Constraints:     []string{"REQUIRED", "ENUM:LOW,MEDIUM,HIGH,VERY_HIGH,PROHIBITED"},
			Source: dictionary.SourceMetadata{
				Primary: "COUNTRY_RISK_DATABASE",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "RISK_REGISTRY",
			},
		},

		// =============================================================================
		// UBO MONITORING ATTRIBUTES
		// =============================================================================
		{
			AttributeID:     uuid.New().String(),
			Name:            "ubo.monitoring_frequency",
			LongDescription: "Frequency of ongoing monitoring for changes in UBO information",
			GroupID:         "ubo_monitoring",
			Mask:            "ENUM",
			Domain:          "UBO",
			Tags:            []string{"UBO", "MONITORING", "FREQUENCY"},
			Constraints:     []string{"REQUIRED", "ENUM:DAILY,WEEKLY,MONTHLY,QUARTERLY,ANNUALLY"},
			Source: dictionary.SourceMetadata{
				Primary: "MONITORING_CONFIGURATION",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "MONITORING_REGISTRY",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "ubo.last_review_date",
			LongDescription: "Date when UBO information was last reviewed and updated",
			GroupID:         "ubo_monitoring",
			Mask:            "DATE",
			Domain:          "UBO",
			Tags:            []string{"UBO", "MONITORING", "TIMESTAMP"},
			Constraints:     []string{"REQUIRED"},
			Source: dictionary.SourceMetadata{
				Primary: "UBO_REVIEW_WORKFLOW",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "UBO_REGISTRY",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "ubo.next_review_due",
			LongDescription: "Date when next UBO review is due",
			GroupID:         "ubo_monitoring",
			Mask:            "DATE",
			Domain:          "UBO",
			Tags:            []string{"UBO", "MONITORING", "SCHEDULING"},
			Constraints:     []string{"REQUIRED"},
			Source: dictionary.SourceMetadata{
				Primary: "SCHEDULING_ENGINE",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "MONITORING_REGISTRY",
			},
		},

		// =============================================================================
		// UBO COMPLIANCE ATTRIBUTES
		// =============================================================================
		{
			AttributeID:     uuid.New().String(),
			Name:            "ubo.compliance_status",
			LongDescription: "Overall compliance status of UBO identification and verification",
			GroupID:         "ubo_compliance",
			Mask:            "ENUM",
			Domain:          "UBO",
			Tags:            []string{"UBO", "COMPLIANCE", "STATUS"},
			Constraints:     []string{"REQUIRED", "ENUM:COMPLIANT,NON_COMPLIANT,UNDER_REVIEW,APPROVED"},
			Source: dictionary.SourceMetadata{
				Primary: "COMPLIANCE_ENGINE",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "COMPLIANCE_REGISTRY",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "ubo.regulatory_threshold",
			LongDescription: "Regulatory threshold percentage applicable to this jurisdiction",
			GroupID:         "ubo_compliance",
			Mask:            "DECIMAL",
			Domain:          "UBO",
			Tags:            []string{"UBO", "REGULATORY", "THRESHOLD"},
			Constraints:     []string{"REQUIRED", "MIN:0.01", "MAX:100.00", "PRECISION:2"},
			Source: dictionary.SourceMetadata{
				Primary: "REGULATORY_DATABASE",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "COMPLIANCE_CONFIGURATION",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "ubo.documentation_complete",
			LongDescription: "Indicates if all required UBO documentation has been collected",
			GroupID:         "ubo_compliance",
			Mask:            "BOOLEAN",
			Domain:          "UBO",
			Tags:            []string{"UBO", "DOCUMENTATION", "COMPLETENESS"},
			Constraints:     []string{"REQUIRED"},
			Source: dictionary.SourceMetadata{
				Primary: "DOCUMENT_COMPLETENESS_CHECK",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "COMPLIANCE_REGISTRY",
			},
		},

		// =============================================================================
		// TRUST-SPECIFIC UBO ATTRIBUTES
		// =============================================================================
		{
			AttributeID:     uuid.New().String(),
			Name:            "trust.settlor_id",
			LongDescription: "Unique identifier for the trust settlor (person who created the trust)",
			GroupID:         "trust_parties",
			Mask:            "UUID",
			Domain:          "UBO",
			Tags:            []string{"TRUST", "SETTLOR", "PARTY"},
			Constraints:     []string{"REQUIRED"},
			Source: dictionary.SourceMetadata{
				Primary: "TRUST_DEED",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "TRUST_REGISTRY",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "trust.trustee_type",
			LongDescription: "Type of trustee (proper person or corporate)",
			GroupID:         "trust_parties",
			Mask:            "ENUM",
			Domain:          "UBO",
			Tags:            []string{"TRUST", "TRUSTEE", "CLASSIFICATION"},
			Constraints:     []string{"REQUIRED", "ENUM:PROPER_PERSON_TRUSTEE,CORPORATE_TRUSTEE,PROFESSIONAL_TRUSTEE"},
			Source: dictionary.SourceMetadata{
				Primary: "TRUST_DEED",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "TRUST_REGISTRY",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "trust.beneficiary_type",
			LongDescription: "Type of beneficiary (named proper person or class)",
			GroupID:         "trust_parties",
			Mask:            "ENUM",
			Domain:          "UBO",
			Tags:            []string{"TRUST", "BENEFICIARY", "CLASSIFICATION"},
			Constraints:     []string{"REQUIRED", "ENUM:NAMED_BENEFICIARY,CLASS_BENEFICIARY,DISCRETIONARY"},
			Source: dictionary.SourceMetadata{
				Primary: "TRUST_DEED",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "TRUST_REGISTRY",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "trust.protector_powers",
			LongDescription: "Powers held by trust protector (comma-separated list)",
			GroupID:         "trust_parties",
			Mask:            "STRING",
			Domain:          "UBO",
			Tags:            []string{"TRUST", "PROTECTOR", "POWERS"},
			Constraints:     []string{"MAX_LENGTH:500"},
			Source: dictionary.SourceMetadata{
				Primary: "TRUST_DEED",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "TRUST_REGISTRY",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "trust.beneficiary_class_definition",
			LongDescription: "Definition of beneficiary class (e.g., 'all grandchildren')",
			GroupID:         "trust_parties",
			Mask:            "STRING",
			Domain:          "UBO",
			Tags:            []string{"TRUST", "BENEFICIARY_CLASS", "DEFINITION"},
			Constraints:     []string{"MAX_LENGTH:200"},
			Source: dictionary.SourceMetadata{
				Primary: "TRUST_DEED",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "TRUST_REGISTRY",
			},
		},

		// =============================================================================
		// PARTNERSHIP-SPECIFIC UBO ATTRIBUTES
		// =============================================================================
		{
			AttributeID:     uuid.New().String(),
			Name:            "partnership.partner_type",
			LongDescription: "Type of partner in the partnership structure",
			GroupID:         "partnership_structure",
			Mask:            "ENUM",
			Domain:          "UBO",
			Tags:            []string{"PARTNERSHIP", "PARTNER", "CLASSIFICATION"},
			Constraints:     []string{"REQUIRED", "ENUM:GENERAL_PARTNER,LIMITED_PARTNER,MANAGING_PARTNER"},
			Source: dictionary.SourceMetadata{
				Primary: "PARTNERSHIP_AGREEMENT",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "PARTNERSHIP_REGISTRY",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "partnership.capital_commitment",
			LongDescription: "Capital commitment amount for limited partner",
			GroupID:         "partnership_structure",
			Mask:            "DECIMAL",
			Domain:          "UBO",
			Tags:            []string{"PARTNERSHIP", "CAPITAL", "FINANCIAL"},
			Constraints:     []string{"MIN:0.00", "PRECISION:2"},
			Source: dictionary.SourceMetadata{
				Primary: "CAPITAL_COMMITMENT_RECORDS",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "PARTNERSHIP_REGISTRY",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "partnership.control_mechanism",
			LongDescription: "Mechanism through which control is exercised over the partnership",
			GroupID:         "partnership_control",
			Mask:            "ENUM",
			Domain:          "UBO",
			Tags:            []string{"PARTNERSHIP", "CONTROL", "MECHANISM"},
			Constraints:     []string{"REQUIRED", "ENUM:MANAGEMENT_AGREEMENT,GP_CONTROL,INVESTMENT_COMMITTEE,VOTING_RIGHTS"},
			Source: dictionary.SourceMetadata{
				Primary: "PARTNERSHIP_AGREEMENT",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "PARTNERSHIP_REGISTRY",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "partnership.prong_type",
			LongDescription: "Prong through which UBO status is achieved (ownership or control)",
			GroupID:         "partnership_ubo",
			Mask:            "ENUM",
			Domain:          "UBO",
			Tags:            []string{"PARTNERSHIP", "UBO", "PRONG"},
			Constraints:     []string{"REQUIRED", "ENUM:OWNERSHIP_PRONG,CONTROL_PRONG,DUAL_PRONG"},
			Source: dictionary.SourceMetadata{
				Primary: "UBO_ANALYSIS_ENGINE",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "UBO_REGISTRY",
			},
		},

		// =============================================================================
		// ENTITY-TYPE-SPECIFIC UBO WORKFLOW ATTRIBUTES
		// =============================================================================
		{
			AttributeID:     uuid.New().String(),
			Name:            "ubo.workflow_type",
			LongDescription: "Type of UBO workflow applied based on entity structure",
			GroupID:         "ubo_workflow",
			Mask:            "ENUM",
			Domain:          "UBO",
			Tags:            []string{"UBO", "WORKFLOW", "TYPE"},
			Constraints:     []string{"REQUIRED", "ENUM:STANDARD_CORPORATE,TRUST_SPECIFIC,PARTNERSHIP_DUAL_PRONG,RECURSIVE_ANALYSIS"},
			Source: dictionary.SourceMetadata{
				Primary: "UBO_WORKFLOW_ENGINE",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "UBO_REGISTRY",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "ubo.regulatory_framework",
			LongDescription: "Regulatory framework applied for UBO identification",
			GroupID:         "ubo_workflow",
			Mask:            "ENUM",
			Domain:          "UBO",
			Tags:            []string{"UBO", "REGULATORY", "FRAMEWORK"},
			Constraints:     []string{"REQUIRED", "ENUM:EU_5MLD,FATF_GUIDANCE,US_CDD,UK_MLR,TRUST_SPECIFIC,PARTNERSHIP_SPECIFIC"},
			Source: dictionary.SourceMetadata{
				Primary: "REGULATORY_REQUIREMENTS",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "COMPLIANCE_REGISTRY",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "ubo.recursive_depth",
			LongDescription: "Maximum depth for recursive UBO analysis of corporate entities",
			GroupID:         "ubo_workflow",
			Mask:            "INTEGER",
			Domain:          "UBO",
			Tags:            []string{"UBO", "RECURSIVE", "DEPTH"},
			Constraints:     []string{"MIN:1", "MAX:10", "DEFAULT:5"},
			Source: dictionary.SourceMetadata{
				Primary: "UBO_CONFIGURATION",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "UBO_REGISTRY",
			},
		},

		// =============================================================================
		// FINCEN CONTROL PRONG ATTRIBUTES (U.S. CDD Rule Compliance)
		// =============================================================================
		{
			AttributeID:     uuid.New().String(),
			Name:            "fincen.control_role_title",
			LongDescription: "Official title of person in FinCEN qualifying control role",
			GroupID:         "fincen_control_prong",
			Mask:            "ENUM",
			Domain:          "UBO",
			Tags:            []string{"FINCEN", "CONTROL", "TITLE", "REGULATORY"},
			Constraints:     []string{"REQUIRED", "ENUM:CEO,CFO,COO,PRESIDENT,GENERAL_PARTNER,MANAGING_MEMBER,SIMILAR_FUNCTIONS"},
			Source: dictionary.SourceMetadata{
				Primary: "ORGANIZATIONAL_CHART",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "UBO_REGISTRY",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "fincen.control_selection_method",
			LongDescription: "Method used to select single proper person under FinCEN Control Prong",
			GroupID:         "fincen_control_prong",
			Mask:            "ENUM",
			Domain:          "UBO",
			Tags:            []string{"FINCEN", "CONTROL", "SELECTION", "METHOD"},
			Constraints:     []string{"REQUIRED", "ENUM:FINCEN_HIERARCHY_RULE,SIMILAR_FUNCTIONS_ANALYSIS,TIE_BREAKER_APPLIED,FALLBACK_RULE"},
			Source: dictionary.SourceMetadata{
				Primary: "UBO_DECISION_ENGINE",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "COMPLIANCE_REGISTRY",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "fincen.control_priority_rank",
			LongDescription: "Priority ranking of control role according to FinCEN hierarchy",
			GroupID:         "fincen_control_prong",
			Mask:            "INTEGER",
			Domain:          "UBO",
			Tags:            []string{"FINCEN", "CONTROL", "PRIORITY", "RANK"},
			Constraints:     []string{"REQUIRED", "MIN:1", "MAX:10"},
			Source: dictionary.SourceMetadata{
				Primary: "FINCEN_HIERARCHY_RULES",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "UBO_REGISTRY",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "fincen.similar_functions_performed",
			LongDescription: "List of functions performed that qualify as 'similar functions' under FinCEN rule",
			GroupID:         "fincen_control_prong",
			Mask:            "STRING",
			Domain:          "UBO",
			Tags:            []string{"FINCEN", "SIMILAR_FUNCTIONS", "CONTROL"},
			Constraints:     []string{"MAX_LENGTH:500"},
			Source: dictionary.SourceMetadata{
				Primary: "JOB_DESCRIPTION_ANALYSIS",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "UBO_REGISTRY",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "fincen.single_individual_selected",
			LongDescription: "Boolean indicating compliance with FinCEN single proper person requirement",
			GroupID:         "fincen_control_prong",
			Mask:            "BOOLEAN",
			Domain:          "UBO",
			Tags:            []string{"FINCEN", "SINGLE_PROPER_PERSON", "COMPLIANCE"},
			Constraints:     []string{"REQUIRED"},
			Source: dictionary.SourceMetadata{
				Primary: "FINCEN_COMPLIANCE_CHECK",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "COMPLIANCE_REGISTRY",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "fincen.decision_rationale",
			LongDescription: "Detailed rationale for Control Prong selection decision",
			GroupID:         "fincen_control_prong",
			Mask:            "STRING",
			Domain:          "UBO",
			Tags:            []string{"FINCEN", "DECISION", "RATIONALE", "AUDIT"},
			Constraints:     []string{"REQUIRED", "MAX_LENGTH:1000"},
			Source: dictionary.SourceMetadata{
				Primary: "UBO_DECISION_ENGINE",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "AUDIT_LOG",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "fincen.regulatory_citation",
			LongDescription: "Specific FinCEN regulatory citation applied",
			GroupID:         "fincen_control_prong",
			Mask:            "ENUM",
			Domain:          "UBO",
			Tags:            []string{"FINCEN", "REGULATORY", "CITATION"},
			Constraints:     []string{"REQUIRED", "ENUM:31_CFR_1010_230,31_CFR_1020_210,31_CFR_1023_210,31_CFR_1024_210"},
			Source: dictionary.SourceMetadata{
				Primary: "REGULATORY_FRAMEWORK",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "COMPLIANCE_REGISTRY",
			},
		},
		{
			AttributeID:     uuid.New().String(),
			Name:            "fincen.has_significant_responsibility",
			LongDescription: "Boolean indicating person has significant responsibility to control, manage, or direct",
			GroupID:         "fincen_control_prong",
			Mask:            "BOOLEAN",
			Domain:          "UBO",
			Tags:            []string{"FINCEN", "SIGNIFICANT_RESPONSIBILITY", "CONTROL"},
			Constraints:     []string{"REQUIRED"},
			Source: dictionary.SourceMetadata{
				Primary: "AUTHORITY_ANALYSIS",
			},
			Sink: dictionary.SinkMetadata{
				Primary: "UBO_REGISTRY",
			},
		},
	}
}
