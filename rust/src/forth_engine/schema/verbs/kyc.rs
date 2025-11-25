//! KYC domain verb definitions.

use crate::forth_engine::schema::types::*;

pub static INVESTIGATION_CREATE: VerbDef = VerbDef {
    name: "investigation.create",
    domain: "kyc",
    args: &[
        ArgSpec {
            name: ":cbu-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::Never,
            default: Some(DefaultValue::FromContext(ContextKey::CbuId)),
            validation: &[],
            description: "CBU this investigation is for",
        },
        ArgSpec {
            name: ":investigation-type",
            sem_type: SemType::Enum(&[
                "STANDARD", "ENHANCED_DUE_DILIGENCE", "SIMPLIFIED", "PERIODIC_REVIEW"
            ]),
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Type of KYC investigation",
        },
        ArgSpec {
            name: ":risk-rating",
            sem_type: SemType::Enum(&["LOW", "MEDIUM", "MEDIUM_HIGH", "HIGH", "VERY_HIGH"]),
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Initial risk rating",
        },
        ArgSpec {
            name: ":ubo-threshold",
            sem_type: SemType::Decimal,
            required: RequiredRule::Never,
            default: Some(DefaultValue::Decimal(25.0)),
            validation: &[ValidationRule::Range { min: Some(0.0), max: Some(100.0) }],
            description: "Ownership percentage threshold for UBO identification",
        },
        ArgSpec {
            name: ":deadline",
            sem_type: SemType::Date,
            required: RequiredRule::Never,
            default: None,
            validation: &[ValidationRule::DateRange { min: Some(DateBound::Today), max: None }],
            description: "Investigation completion deadline",
        },
        ArgSpec {
            name: ":as",
            sem_type: SemType::Symbol,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Symbol name to capture investigation ID",
        },
    ],
    constraints: &[],
    produces: Some(ProducesSpec {
        capture_as: ContextKey::InvestigationId,
        description: "The investigation UUID",
    }),
    crud_asset: "INVESTIGATION",
    description: "Create a new KYC investigation for a CBU",
    examples: &[
        r#"(investigation.create :investigation-type "ENHANCED_DUE_DILIGENCE" :as @inv)"#,
    ],
};

pub static INVESTIGATION_UPDATE_STATUS: VerbDef = VerbDef {
    name: "investigation.update-status",
    domain: "kyc",
    args: &[
        ArgSpec {
            name: ":investigation-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::Never,
            default: Some(DefaultValue::FromContext(ContextKey::InvestigationId)),
            validation: &[],
            description: "Investigation ID",
        },
        ArgSpec {
            name: ":status",
            sem_type: SemType::Enum(&[
                "PENDING", "IN_PROGRESS", "COLLECTING_DOCUMENTS", 
                "UNDER_REVIEW", "ESCALATED", "APPROVED", "REJECTED", "CLOSED"
            ]),
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "New status",
        },
    ],
    constraints: &[],
    produces: None,
    crud_asset: "INVESTIGATION",
    description: "Update investigation status",
    examples: &[
        r#"(investigation.update-status :investigation-id @inv :status "COLLECTING_DOCUMENTS")"#,
    ],
};

pub static INVESTIGATION_COMPLETE: VerbDef = VerbDef {
    name: "investigation.complete",
    domain: "kyc",
    args: &[
        ArgSpec {
            name: ":investigation-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::Never,
            default: Some(DefaultValue::FromContext(ContextKey::InvestigationId)),
            validation: &[],
            description: "Investigation ID",
        },
        ArgSpec {
            name: ":outcome",
            sem_type: SemType::Enum(&["APPROVED", "REJECTED", "CONDITIONALLY_APPROVED"]),
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Investigation outcome",
        },
        ArgSpec {
            name: ":rationale",
            sem_type: SemType::String,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Reason for outcome",
        },
    ],
    constraints: &[],
    produces: None,
    crud_asset: "INVESTIGATION",
    description: "Complete an investigation with outcome",
    examples: &[
        r#"(investigation.complete :investigation-id @inv :outcome "APPROVED")"#,
    ],
};

pub static RISK_ASSESS_CBU: VerbDef = VerbDef {
    name: "risk.assess-cbu",
    domain: "kyc",
    args: &[
        ArgSpec {
            name: ":cbu-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::Never,
            default: Some(DefaultValue::FromContext(ContextKey::CbuId)),
            validation: &[],
            description: "CBU to assess",
        },
        ArgSpec {
            name: ":investigation-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::Never,
            default: Some(DefaultValue::FromContext(ContextKey::InvestigationId)),
            validation: &[],
            description: "Associated investigation",
        },
        ArgSpec {
            name: ":methodology",
            sem_type: SemType::Enum(&["FACTOR_WEIGHTED", "HIGHEST_RISK", "CUMULATIVE"]),
            required: RequiredRule::Never,
            default: Some(DefaultValue::Str("FACTOR_WEIGHTED")),
            validation: &[],
            description: "Risk assessment methodology",
        },
    ],
    constraints: &[],
    produces: None,
    crud_asset: "RISK_ASSESSMENT_CBU",
    description: "Perform risk assessment on a CBU",
    examples: &[
        r#"(risk.assess-cbu :methodology "FACTOR_WEIGHTED")"#,
    ],
};

pub static RISK_SET_RATING: VerbDef = VerbDef {
    name: "risk.set-rating",
    domain: "kyc",
    args: &[
        ArgSpec {
            name: ":cbu-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::Never,
            default: Some(DefaultValue::FromContext(ContextKey::CbuId)),
            validation: &[],
            description: "CBU to rate",
        },
        ArgSpec {
            name: ":rating",
            sem_type: SemType::Enum(&["LOW", "MEDIUM", "MEDIUM_HIGH", "HIGH", "VERY_HIGH", "PROHIBITED"]),
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Risk rating to assign",
        },
        ArgSpec {
            name: ":rationale",
            sem_type: SemType::String,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Explanation for the rating",
        },
    ],
    constraints: &[],
    produces: None,
    crud_asset: "RISK_RATING",
    description: "Set the risk rating for a CBU",
    examples: &[
        r#"(risk.set-rating :rating "HIGH" :rationale "PEP exposure")"#,
    ],
};

pub static SCREENING_PEP: VerbDef = VerbDef {
    name: "screening.pep",
    domain: "screening",
    args: &[
        ArgSpec {
            name: ":entity-id",
            sem_type: SemType::Symbol,
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Entity to screen",
        },
        ArgSpec {
            name: ":screening-provider",
            sem_type: SemType::Enum(&["REFINITIV", "DOW_JONES", "LEXISNEXIS", "INTERNAL"]),
            required: RequiredRule::Never,
            default: Some(DefaultValue::Str("REFINITIV")),
            validation: &[],
            description: "Screening data provider",
        },
    ],
    constraints: &[],
    produces: Some(ProducesSpec {
        capture_as: ContextKey::ScreeningId,
        description: "The screening result UUID",
    }),
    crud_asset: "SCREENING_RESULT",
    description: "Screen entity for PEP (Politically Exposed Person) status",
    examples: &[
        r#"(screening.pep :entity-id @person)"#,
    ],
};

pub static SCREENING_SANCTIONS: VerbDef = VerbDef {
    name: "screening.sanctions",
    domain: "screening",
    args: &[
        ArgSpec {
            name: ":entity-id",
            sem_type: SemType::Symbol,
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Entity to screen",
        },
        ArgSpec {
            name: ":lists",
            sem_type: SemType::ListOf(&SemType::Ref(RefType::ScreeningList)),
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Specific sanctions lists to check",
        },
    ],
    constraints: &[],
    produces: Some(ProducesSpec {
        capture_as: ContextKey::ScreeningId,
        description: "The screening result UUID",
    }),
    crud_asset: "SCREENING_RESULT",
    description: "Screen entity against sanctions lists",
    examples: &[
        r#"(screening.sanctions :entity-id @company)"#,
    ],
};

pub static DECISION_RECORD: VerbDef = VerbDef {
    name: "decision.record",
    domain: "decision",
    args: &[
        ArgSpec {
            name: ":investigation-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::Never,
            default: Some(DefaultValue::FromContext(ContextKey::InvestigationId)),
            validation: &[],
            description: "Investigation this decision relates to",
        },
        ArgSpec {
            name: ":decision-type",
            sem_type: SemType::Enum(&["APPROVE", "REJECT", "ESCALATE", "DEFER", "CONDITIONAL_APPROVE"]),
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Type of decision",
        },
        ArgSpec {
            name: ":rationale",
            sem_type: SemType::String,
            required: RequiredRule::Always,
            default: None,
            validation: &[ValidationRule::NotEmpty],
            description: "Reason for the decision",
        },
        ArgSpec {
            name: ":decided-by",
            sem_type: SemType::String,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Person/system making the decision",
        },
        ArgSpec {
            name: ":as",
            sem_type: SemType::Symbol,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Symbol to capture decision ID",
        },
    ],
    constraints: &[],
    produces: Some(ProducesSpec {
        capture_as: ContextKey::DecisionId,
        description: "The decision UUID",
    }),
    crud_asset: "DECISION",
    description: "Record a decision for an investigation",
    examples: &[
        r#"(decision.record :decision-type "APPROVE" :rationale "All requirements met" :as @decision)"#,
    ],
};
