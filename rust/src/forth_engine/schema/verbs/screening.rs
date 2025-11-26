//! Screening domain verb definitions.
//!
//! Covers PEP screening, sanctions screening, adverse media,
//! hit resolution, and batch screening operations.

use crate::forth_engine::schema::types::*;

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
            description: "Entity to screen for PEP status",
        },
        ArgSpec {
            name: ":screening-provider",
            sem_type: SemType::Enum(&["REFINITIV", "DOW_JONES", "LEXISNEXIS", "INTERNAL"]),
            required: RequiredRule::Never,
            default: Some(DefaultValue::Str("REFINITIV")),
            validation: &[],
            description: "Screening data provider",
        },
        ArgSpec {
            name: ":match-threshold",
            sem_type: SemType::Decimal,
            required: RequiredRule::Never,
            default: Some(DefaultValue::Decimal(85.0)),
            validation: &[ValidationRule::Range { min: Some(0.0), max: Some(100.0) }],
            description: "Minimum match score threshold (0-100)",
        },
        ArgSpec {
            name: ":as",
            sem_type: SemType::Symbol,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Symbol to capture screening result ID",
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
        r#"(screening.pep :entity-id @person :as @pep-result)"#,
        r#"(screening.pep :entity-id @director :screening-provider "DOW_JONES" :match-threshold 90.0)"#,
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
            description: "Entity to screen against sanctions lists",
        },
        ArgSpec {
            name: ":lists",
            sem_type: SemType::ListOf(&SemType::Ref(RefType::ScreeningList)),
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Specific sanctions lists to check (defaults to all)",
        },
        ArgSpec {
            name: ":screening-provider",
            sem_type: SemType::Enum(&["REFINITIV", "DOW_JONES", "LEXISNEXIS", "INTERNAL"]),
            required: RequiredRule::Never,
            default: Some(DefaultValue::Str("REFINITIV")),
            validation: &[],
            description: "Screening data provider",
        },
        ArgSpec {
            name: ":match-threshold",
            sem_type: SemType::Decimal,
            required: RequiredRule::Never,
            default: Some(DefaultValue::Decimal(85.0)),
            validation: &[ValidationRule::Range { min: Some(0.0), max: Some(100.0) }],
            description: "Minimum match score threshold",
        },
        ArgSpec {
            name: ":as",
            sem_type: SemType::Symbol,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Symbol to capture screening result ID",
        },
    ],
    constraints: &[],
    produces: Some(ProducesSpec {
        capture_as: ContextKey::ScreeningId,
        description: "The screening result UUID",
    }),
    crud_asset: "SCREENING_RESULT",
    description: "Screen entity against sanctions lists (OFAC, EU, UN, etc.)",
    examples: &[
        r#"(screening.sanctions :entity-id @company :as @sanc-result)"#,
        r#"(screening.sanctions :entity-id @person :lists ["OFAC_SDN" "EU_SANCTIONS"])"#,
    ],
};

pub static SCREENING_ADVERSE_MEDIA: VerbDef = VerbDef {
    name: "screening.adverse-media",
    domain: "screening",
    args: &[
        ArgSpec {
            name: ":entity-id",
            sem_type: SemType::Symbol,
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Entity to screen for adverse media",
        },
        ArgSpec {
            name: ":categories",
            sem_type: SemType::ListOf(&SemType::Enum(&[
                "FINANCIAL_CRIME", "FRAUD", "CORRUPTION", "MONEY_LAUNDERING",
                "TAX_EVASION", "TERRORISM", "ORGANIZED_CRIME", "REGULATORY",
                "ENVIRONMENTAL", "HUMAN_RIGHTS", "OTHER"
            ])),
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Categories of adverse media to search",
        },
        ArgSpec {
            name: ":screening-provider",
            sem_type: SemType::Enum(&["REFINITIV", "DOW_JONES", "LEXISNEXIS", "INTERNAL"]),
            required: RequiredRule::Never,
            default: Some(DefaultValue::Str("REFINITIV")),
            validation: &[],
            description: "Screening data provider",
        },
        ArgSpec {
            name: ":lookback-months",
            sem_type: SemType::Integer,
            required: RequiredRule::Never,
            default: Some(DefaultValue::Int(36)),
            validation: &[ValidationRule::Range { min: Some(1.0), max: Some(120.0) }],
            description: "How many months back to search",
        },
        ArgSpec {
            name: ":as",
            sem_type: SemType::Symbol,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Symbol to capture screening result ID",
        },
    ],
    constraints: &[],
    produces: Some(ProducesSpec {
        capture_as: ContextKey::ScreeningId,
        description: "The screening result UUID",
    }),
    crud_asset: "SCREENING_RESULT",
    description: "Screen entity for adverse media coverage",
    examples: &[
        r#"(screening.adverse-media :entity-id @company :as @media-result)"#,
        r#"(screening.adverse-media :entity-id @person :categories ["FINANCIAL_CRIME" "FRAUD"] :lookback-months 60)"#,
    ],
};

pub static SCREENING_RESOLVE_HIT: VerbDef = VerbDef {
    name: "screening.resolve-hit",
    domain: "screening",
    args: &[
        ArgSpec {
            name: ":screening-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::Never,
            default: Some(DefaultValue::FromContext(ContextKey::ScreeningId)),
            validation: &[],
            description: "Screening result containing the hit",
        },
        ArgSpec {
            name: ":hit-id",
            sem_type: SemType::String,
            required: RequiredRule::Always,
            default: None,
            validation: &[ValidationRule::NotEmpty],
            description: "Identifier of the specific hit to resolve",
        },
        ArgSpec {
            name: ":resolution",
            sem_type: SemType::Enum(&["TRUE_MATCH", "FALSE_POSITIVE", "INCONCLUSIVE", "ESCALATE"]),
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Resolution decision for the hit",
        },
        ArgSpec {
            name: ":rationale",
            sem_type: SemType::String,
            required: RequiredRule::Always,
            default: None,
            validation: &[ValidationRule::NotEmpty, ValidationRule::Length { min: Some(10), max: Some(2000) }],
            description: "Detailed rationale for the resolution",
        },
        ArgSpec {
            name: ":resolved-by",
            sem_type: SemType::String,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Person or system resolving the hit",
        },
        ArgSpec {
            name: ":evidence-refs",
            sem_type: SemType::ListOf(&SemType::String),
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "References to supporting evidence",
        },
    ],
    constraints: &[],
    produces: None,
    crud_asset: "SCREENING_HIT_RESOLUTION",
    description: "Resolve a screening hit with documented rationale",
    examples: &[
        r#"(screening.resolve-hit :hit-id "HIT-001" :resolution "FALSE_POSITIVE" :rationale "Different date of birth and nationality")"#,
        r#"(screening.resolve-hit :screening-id @sanc-result :hit-id "HIT-002" :resolution "TRUE_MATCH" :rationale "Confirmed identity match via passport")"#,
    ],
};

pub static SCREENING_DISMISS_HIT: VerbDef = VerbDef {
    name: "screening.dismiss-hit",
    domain: "screening",
    args: &[
        ArgSpec {
            name: ":screening-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::Never,
            default: Some(DefaultValue::FromContext(ContextKey::ScreeningId)),
            validation: &[],
            description: "Screening result containing the hit",
        },
        ArgSpec {
            name: ":hit-id",
            sem_type: SemType::String,
            required: RequiredRule::Always,
            default: None,
            validation: &[ValidationRule::NotEmpty],
            description: "Identifier of the hit to dismiss",
        },
        ArgSpec {
            name: ":reason",
            sem_type: SemType::Enum(&[
                "NAME_ONLY_MATCH", "DIFFERENT_DOB", "DIFFERENT_NATIONALITY",
                "DIFFERENT_JURISDICTION", "DECEASED", "DELISTED", "OTHER"
            ]),
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Reason for dismissal",
        },
        ArgSpec {
            name: ":notes",
            sem_type: SemType::String,
            required: RequiredRule::IfEquals { arg: ":reason", value: "OTHER" },
            default: None,
            validation: &[],
            description: "Additional notes (required if reason is OTHER)",
        },
    ],
    constraints: &[],
    produces: None,
    crud_asset: "SCREENING_HIT_RESOLUTION",
    description: "Dismiss a screening hit as false positive",
    examples: &[
        r#"(screening.dismiss-hit :hit-id "HIT-003" :reason "DIFFERENT_DOB")"#,
        r#"(screening.dismiss-hit :hit-id "HIT-004" :reason "OTHER" :notes "Entity confirmed dissolved in 2019")"#,
    ],
};

pub static SCREENING_BATCH: VerbDef = VerbDef {
    name: "screening.batch",
    domain: "screening",
    args: &[
        ArgSpec {
            name: ":entity-ids",
            sem_type: SemType::ListOf(&SemType::Symbol),
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "List of entities to screen",
        },
        ArgSpec {
            name: ":screen-types",
            sem_type: SemType::ListOf(&SemType::Enum(&["PEP", "SANCTIONS", "ADVERSE_MEDIA"])),
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Types of screening to perform",
        },
        ArgSpec {
            name: ":screening-provider",
            sem_type: SemType::Enum(&["REFINITIV", "DOW_JONES", "LEXISNEXIS", "INTERNAL"]),
            required: RequiredRule::Never,
            default: Some(DefaultValue::Str("REFINITIV")),
            validation: &[],
            description: "Screening data provider",
        },
        ArgSpec {
            name: ":match-threshold",
            sem_type: SemType::Decimal,
            required: RequiredRule::Never,
            default: Some(DefaultValue::Decimal(85.0)),
            validation: &[ValidationRule::Range { min: Some(0.0), max: Some(100.0) }],
            description: "Minimum match score threshold",
        },
    ],
    constraints: &[],
    produces: None,
    crud_asset: "SCREENING_BATCH",
    description: "Perform batch screening on multiple entities",
    examples: &[
        r#"(screening.batch :entity-ids [@person1 @person2 @company] :screen-types ["PEP" "SANCTIONS"])"#,
    ],
};

pub static SCREENING_REFRESH: VerbDef = VerbDef {
    name: "screening.refresh",
    domain: "screening",
    args: &[
        ArgSpec {
            name: ":entity-id",
            sem_type: SemType::Symbol,
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Entity to re-screen",
        },
        ArgSpec {
            name: ":screen-types",
            sem_type: SemType::ListOf(&SemType::Enum(&["PEP", "SANCTIONS", "ADVERSE_MEDIA"])),
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Types of screening to refresh (defaults to all previous)",
        },
        ArgSpec {
            name: ":reason",
            sem_type: SemType::Enum(&["PERIODIC_REVIEW", "TRIGGER_EVENT", "MANUAL_REQUEST", "REGULATORY"]),
            required: RequiredRule::Never,
            default: Some(DefaultValue::Str("PERIODIC_REVIEW")),
            validation: &[],
            description: "Reason for refresh",
        },
    ],
    constraints: &[],
    produces: Some(ProducesSpec {
        capture_as: ContextKey::ScreeningId,
        description: "The new screening result UUID",
    }),
    crud_asset: "SCREENING_RESULT",
    description: "Refresh screening for an entity",
    examples: &[
        r#"(screening.refresh :entity-id @person :reason "PERIODIC_REVIEW")"#,
    ],
};
