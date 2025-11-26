//! Monitoring domain verb definitions.
//!
//! Covers periodic reviews, trigger events, ongoing monitoring,
//! risk updates, and case management.

use crate::forth_engine::schema::types::*;

pub static MONITORING_SCHEDULE_REVIEW: VerbDef = VerbDef {
    name: "monitoring.schedule-review",
    domain: "monitoring",
    args: &[
        ArgSpec {
            name: ":cbu-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::Never,
            default: Some(DefaultValue::FromContext(ContextKey::CbuId)),
            validation: &[],
            description: "CBU to schedule review for",
        },
        ArgSpec {
            name: ":review-type",
            sem_type: SemType::Enum(&[
                "PERIODIC", "ANNUAL", "ENHANCED_PERIODIC", "SIMPLIFIED_PERIODIC"
            ]),
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Type of review to schedule",
        },
        ArgSpec {
            name: ":due-date",
            sem_type: SemType::Date,
            required: RequiredRule::Always,
            default: None,
            validation: &[ValidationRule::DateRange { min: Some(DateBound::Today), max: None }],
            description: "When review is due",
        },
        ArgSpec {
            name: ":risk-based-frequency",
            sem_type: SemType::Enum(&["ANNUAL", "BIANNUAL", "QUARTERLY", "MONTHLY"]),
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Frequency based on risk rating",
        },
        ArgSpec {
            name: ":assigned-to",
            sem_type: SemType::String,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Person or team assigned to conduct review",
        },
        ArgSpec {
            name: ":priority",
            sem_type: SemType::Enum(&["LOW", "NORMAL", "HIGH", "URGENT"]),
            required: RequiredRule::Never,
            default: Some(DefaultValue::Str("NORMAL")),
            validation: &[],
            description: "Priority of the review",
        },
        ArgSpec {
            name: ":scope",
            sem_type: SemType::ListOf(&SemType::Enum(&[
                "OWNERSHIP", "CONTROL", "DOCUMENTS", "SCREENING",
                "TRANSACTIONS", "RISK_FACTORS", "FULL"
            ])),
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Scope of review (defaults to FULL)",
        },
        ArgSpec {
            name: ":as",
            sem_type: SemType::Symbol,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Symbol to capture review ID",
        },
    ],
    constraints: &[],
    produces: None,
    crud_asset: "MONITORING_REVIEW",
    description: "Schedule a periodic review for a CBU",
    examples: &[
        r#"(monitoring.schedule-review :review-type "ANNUAL" :due-date "2026-01-15" :as @review)"#,
        r#"(monitoring.schedule-review :review-type "ENHANCED_PERIODIC" :due-date "2025-06-01" :risk-based-frequency "QUARTERLY" :priority "HIGH")"#,
    ],
};

pub static MONITORING_TRIGGER_REVIEW: VerbDef = VerbDef {
    name: "monitoring.trigger-review",
    domain: "monitoring",
    args: &[
        ArgSpec {
            name: ":cbu-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::Never,
            default: Some(DefaultValue::FromContext(ContextKey::CbuId)),
            validation: &[],
            description: "CBU to trigger review for",
        },
        ArgSpec {
            name: ":trigger-type",
            sem_type: SemType::Enum(&[
                "ADVERSE_MEDIA", "SANCTIONS_ALERT", "TRANSACTION_ALERT",
                "OWNERSHIP_CHANGE", "REGULATORY_CHANGE", "CLIENT_REQUEST",
                "INTERNAL_REFERRAL", "SCREENING_HIT", "OTHER"
            ]),
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "What triggered this review",
        },
        ArgSpec {
            name: ":description",
            sem_type: SemType::String,
            required: RequiredRule::Always,
            default: None,
            validation: &[ValidationRule::NotEmpty],
            description: "Description of the trigger event",
        },
        ArgSpec {
            name: ":source",
            sem_type: SemType::String,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Source of the trigger (system, person, etc.)",
        },
        ArgSpec {
            name: ":priority",
            sem_type: SemType::Enum(&["LOW", "NORMAL", "HIGH", "URGENT"]),
            required: RequiredRule::Never,
            default: Some(DefaultValue::Str("HIGH")),
            validation: &[],
            description: "Priority of triggered review",
        },
        ArgSpec {
            name: ":due-date",
            sem_type: SemType::Date,
            required: RequiredRule::Never,
            default: None,
            validation: &[ValidationRule::DateRange { min: Some(DateBound::Today), max: None }],
            description: "When review should be completed",
        },
        ArgSpec {
            name: ":reference-id",
            sem_type: SemType::String,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Reference to triggering event (alert ID, etc.)",
        },
        ArgSpec {
            name: ":as",
            sem_type: SemType::Symbol,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Symbol to capture review ID",
        },
    ],
    constraints: &[],
    produces: None,
    crud_asset: "MONITORING_REVIEW",
    description: "Trigger an ad-hoc review based on an event",
    examples: &[
        r#"(monitoring.trigger-review :trigger-type "ADVERSE_MEDIA" :description "Negative press coverage regarding fraud investigation" :priority "URGENT")"#,
        r#"(monitoring.trigger-review :trigger-type "OWNERSHIP_CHANGE" :description "Major shareholder change detected" :reference-id "ALERT-12345")"#,
    ],
};

pub static MONITORING_UPDATE_RISK: VerbDef = VerbDef {
    name: "monitoring.update-risk",
    domain: "monitoring",
    args: &[
        ArgSpec {
            name: ":cbu-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::Never,
            default: Some(DefaultValue::FromContext(ContextKey::CbuId)),
            validation: &[],
            description: "CBU to update risk for",
        },
        ArgSpec {
            name: ":previous-rating",
            sem_type: SemType::Enum(&["LOW", "MEDIUM", "MEDIUM_HIGH", "HIGH", "VERY_HIGH", "PROHIBITED"]),
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Previous risk rating (for audit trail)",
        },
        ArgSpec {
            name: ":new-rating",
            sem_type: SemType::Enum(&["LOW", "MEDIUM", "MEDIUM_HIGH", "HIGH", "VERY_HIGH", "PROHIBITED"]),
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "New risk rating to assign",
        },
        ArgSpec {
            name: ":reason",
            sem_type: SemType::Enum(&[
                "PERIODIC_REVIEW", "TRIGGER_EVENT", "OWNERSHIP_CHANGE",
                "JURISDICTION_CHANGE", "PRODUCT_CHANGE", "SCREENING_RESULT",
                "TRANSACTION_PATTERN", "REGULATORY_CHANGE", "OTHER"
            ]),
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Reason for risk change",
        },
        ArgSpec {
            name: ":rationale",
            sem_type: SemType::String,
            required: RequiredRule::Always,
            default: None,
            validation: &[ValidationRule::NotEmpty, ValidationRule::Length { min: Some(10), max: Some(2000) }],
            description: "Detailed rationale for the change",
        },
        ArgSpec {
            name: ":effective-date",
            sem_type: SemType::Date,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "When new rating becomes effective",
        },
        ArgSpec {
            name: ":updated-by",
            sem_type: SemType::String,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Person updating the risk",
        },
    ],
    constraints: &[],
    produces: None,
    crud_asset: "RISK_RATING",
    description: "Update risk rating for a CBU",
    examples: &[
        r#"(monitoring.update-risk :new-rating "HIGH" :reason "SCREENING_RESULT" :rationale "New PEP association discovered")"#,
        r#"(monitoring.update-risk :previous-rating "HIGH" :new-rating "MEDIUM" :reason "PERIODIC_REVIEW" :rationale "PEP status ended, 2 years clear")"#,
    ],
};

pub static MONITORING_COMPLETE_REVIEW: VerbDef = VerbDef {
    name: "monitoring.complete-review",
    domain: "monitoring",
    args: &[
        ArgSpec {
            name: ":review-id",
            sem_type: SemType::Symbol,
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Review to complete",
        },
        ArgSpec {
            name: ":outcome",
            sem_type: SemType::Enum(&[
                "NO_CHANGE", "RISK_INCREASED", "RISK_DECREASED",
                "ESCALATED", "EXIT_RECOMMENDED", "ENHANCED_MONITORING"
            ]),
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Outcome of the review",
        },
        ArgSpec {
            name: ":findings",
            sem_type: SemType::String,
            required: RequiredRule::Always,
            default: None,
            validation: &[ValidationRule::NotEmpty],
            description: "Summary of review findings",
        },
        ArgSpec {
            name: ":next-review-date",
            sem_type: SemType::Date,
            required: RequiredRule::Never,
            default: None,
            validation: &[ValidationRule::DateRange { min: Some(DateBound::Today), max: None }],
            description: "When next review should occur",
        },
        ArgSpec {
            name: ":completed-by",
            sem_type: SemType::String,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Person completing the review",
        },
        ArgSpec {
            name: ":actions",
            sem_type: SemType::ListOf(&SemType::String),
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Actions to be taken as result of review",
        },
    ],
    constraints: &[],
    produces: None,
    crud_asset: "MONITORING_REVIEW",
    description: "Complete a monitoring review with findings",
    examples: &[
        r#"(monitoring.complete-review :review-id @review :outcome "NO_CHANGE" :findings "No material changes, risk profile unchanged" :next-review-date "2026-11-26")"#,
        r#"(monitoring.complete-review :review-id @review :outcome "ESCALATED" :findings "Sanctions hit requires senior review" :actions ["Escalate to MLRO" "Suspend trading"])"#,
    ],
};

pub static MONITORING_CLOSE_CASE: VerbDef = VerbDef {
    name: "monitoring.close-case",
    domain: "monitoring",
    args: &[
        ArgSpec {
            name: ":cbu-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::Never,
            default: Some(DefaultValue::FromContext(ContextKey::CbuId)),
            validation: &[],
            description: "CBU to close monitoring for",
        },
        ArgSpec {
            name: ":close-reason",
            sem_type: SemType::Enum(&[
                "ACCOUNT_CLOSED", "CLIENT_EXITED", "RELATIONSHIP_TERMINATED",
                "MERGED_WITH_OTHER", "REGULATORY_ORDER", "OTHER"
            ]),
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Reason for closing monitoring",
        },
        ArgSpec {
            name: ":rationale",
            sem_type: SemType::String,
            required: RequiredRule::Always,
            default: None,
            validation: &[ValidationRule::NotEmpty],
            description: "Detailed rationale for closure",
        },
        ArgSpec {
            name: ":effective-date",
            sem_type: SemType::Date,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "When closure becomes effective",
        },
        ArgSpec {
            name: ":retention-period-years",
            sem_type: SemType::Integer,
            required: RequiredRule::Never,
            default: Some(DefaultValue::Int(7)),
            validation: &[ValidationRule::Range { min: Some(5.0), max: Some(25.0) }],
            description: "How long to retain records (years)",
        },
        ArgSpec {
            name: ":closed-by",
            sem_type: SemType::String,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Person closing the case",
        },
    ],
    constraints: &[],
    produces: None,
    crud_asset: "MONITORING_CASE",
    description: "Close ongoing monitoring for a CBU",
    examples: &[
        r#"(monitoring.close-case :close-reason "CLIENT_EXITED" :rationale "Client requested account closure" :retention-period-years 7)"#,
    ],
};

pub static MONITORING_ADD_ALERT_RULE: VerbDef = VerbDef {
    name: "monitoring.add-alert-rule",
    domain: "monitoring",
    args: &[
        ArgSpec {
            name: ":cbu-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::Never,
            default: Some(DefaultValue::FromContext(ContextKey::CbuId)),
            validation: &[],
            description: "CBU to add alert rule for",
        },
        ArgSpec {
            name: ":rule-type",
            sem_type: SemType::Enum(&[
                "TRANSACTION_VOLUME", "TRANSACTION_VALUE", "JURISDICTION_ACTIVITY",
                "COUNTERPARTY_TYPE", "PATTERN_DEVIATION", "CUSTOM"
            ]),
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Type of alert rule",
        },
        ArgSpec {
            name: ":threshold",
            sem_type: SemType::String,
            required: RequiredRule::Always,
            default: None,
            validation: &[ValidationRule::NotEmpty],
            description: "Threshold condition (e.g., '> 100000 USD')",
        },
        ArgSpec {
            name: ":description",
            sem_type: SemType::String,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Description of what this rule monitors",
        },
        ArgSpec {
            name: ":active",
            sem_type: SemType::Boolean,
            required: RequiredRule::Never,
            default: Some(DefaultValue::Bool(true)),
            validation: &[],
            description: "Whether rule is active",
        },
    ],
    constraints: &[],
    produces: None,
    crud_asset: "MONITORING_ALERT_RULE",
    description: "Add an alert rule for ongoing monitoring",
    examples: &[
        r#"(monitoring.add-alert-rule :rule-type "TRANSACTION_VALUE" :threshold "> 500000 USD" :description "Large transaction alert")"#,
    ],
};

pub static MONITORING_RECORD_ACTIVITY: VerbDef = VerbDef {
    name: "monitoring.record-activity",
    domain: "monitoring",
    args: &[
        ArgSpec {
            name: ":cbu-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::Never,
            default: Some(DefaultValue::FromContext(ContextKey::CbuId)),
            validation: &[],
            description: "CBU to record activity for",
        },
        ArgSpec {
            name: ":activity-type",
            sem_type: SemType::Enum(&[
                "CLIENT_CONTACT", "DOCUMENT_UPDATE", "SCREENING_RUN",
                "TRANSACTION_REVIEW", "RISK_ASSESSMENT", "INTERNAL_NOTE", "OTHER"
            ]),
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Type of activity",
        },
        ArgSpec {
            name: ":description",
            sem_type: SemType::String,
            required: RequiredRule::Always,
            default: None,
            validation: &[ValidationRule::NotEmpty],
            description: "Description of the activity",
        },
        ArgSpec {
            name: ":recorded-by",
            sem_type: SemType::String,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Person recording the activity",
        },
        ArgSpec {
            name: ":reference-id",
            sem_type: SemType::String,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Reference to related record",
        },
    ],
    constraints: &[],
    produces: None,
    crud_asset: "MONITORING_ACTIVITY",
    description: "Record an activity in the monitoring log",
    examples: &[
        r#"(monitoring.record-activity :activity-type "CLIENT_CONTACT" :description "Annual review call with CFO")"#,
    ],
};
