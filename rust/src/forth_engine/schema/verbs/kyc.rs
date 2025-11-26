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

