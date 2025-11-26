//! Attribute domain verb definitions.
//!
//! Covers setting, getting, and validating attributes
//! from the attribute dictionary for entities and CBUs.

use crate::forth_engine::schema::types::*;

pub static ATTRIBUTE_SET: VerbDef = VerbDef {
    name: "attribute.set",
    domain: "attribute",
    args: &[
        ArgSpec {
            name: ":entity-id",
            sem_type: SemType::Symbol,
            required: RequiredRule::UnlessProvided(":cbu-id"),
            default: None,
            validation: &[],
            description: "Entity to set attribute for",
        },
        ArgSpec {
            name: ":cbu-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::UnlessProvided(":entity-id"),
            default: Some(DefaultValue::FromContext(ContextKey::CbuId)),
            validation: &[],
            description: "CBU to set attribute for",
        },
        ArgSpec {
            name: ":attribute",
            sem_type: SemType::Ref(RefType::Attribute),
            required: RequiredRule::Always,
            default: None,
            validation: &[ValidationRule::LookupMustExist],
            description: "Attribute to set (from attribute dictionary)",
        },
        ArgSpec {
            name: ":value",
            sem_type: SemType::String,
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Value to assign to the attribute",
        },
        ArgSpec {
            name: ":source",
            sem_type: SemType::Enum(&["DOCUMENT", "CLIENT_PROVIDED", "THIRD_PARTY", "CALCULATED", "MANUAL"]),
            required: RequiredRule::Never,
            default: Some(DefaultValue::Str("MANUAL")),
            validation: &[],
            description: "Source of the attribute value",
        },
        ArgSpec {
            name: ":source-ref",
            sem_type: SemType::String,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Reference to source document/record",
        },
        ArgSpec {
            name: ":confidence",
            sem_type: SemType::Enum(&["LOW", "MEDIUM", "HIGH", "VERIFIED"]),
            required: RequiredRule::Never,
            default: Some(DefaultValue::Str("MEDIUM")),
            validation: &[],
            description: "Confidence level of the value",
        },
        ArgSpec {
            name: ":effective-date",
            sem_type: SemType::Date,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "When this value becomes effective",
        },
        ArgSpec {
            name: ":expiry-date",
            sem_type: SemType::Date,
            required: RequiredRule::Never,
            default: None,
            validation: &[ValidationRule::DateRange { min: Some(DateBound::Today), max: None }],
            description: "When this value expires",
        },
    ],
    constraints: &[
        CrossConstraint::AtLeastOne(&[":entity-id", ":cbu-id"]),
    ],
    produces: None,
    crud_asset: "ATTRIBUTE_VALUE",
    description: "Set an attribute value for an entity or CBU",
    examples: &[
        r#"(attribute.set :entity-id @person :attribute "DATE_OF_BIRTH" :value "1985-03-15" :source "DOCUMENT" :confidence "VERIFIED")"#,
        r#"(attribute.set :cbu-id @cbu :attribute "ANNUAL_REVENUE" :value "50000000" :source "CLIENT_PROVIDED")"#,
    ],
};

pub static ATTRIBUTE_GET: VerbDef = VerbDef {
    name: "attribute.get",
    domain: "attribute",
    args: &[
        ArgSpec {
            name: ":entity-id",
            sem_type: SemType::Symbol,
            required: RequiredRule::UnlessProvided(":cbu-id"),
            default: None,
            validation: &[],
            description: "Entity to get attribute from",
        },
        ArgSpec {
            name: ":cbu-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::UnlessProvided(":entity-id"),
            default: Some(DefaultValue::FromContext(ContextKey::CbuId)),
            validation: &[],
            description: "CBU to get attribute from",
        },
        ArgSpec {
            name: ":attribute",
            sem_type: SemType::Ref(RefType::Attribute),
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Attribute to retrieve",
        },
        ArgSpec {
            name: ":as-of-date",
            sem_type: SemType::Date,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Get value as of specific date (for versioning)",
        },
    ],
    constraints: &[
        CrossConstraint::AtLeastOne(&[":entity-id", ":cbu-id"]),
    ],
    produces: None,
    crud_asset: "ATTRIBUTE_VALUE",
    description: "Get an attribute value for an entity or CBU",
    examples: &[
        r#"(attribute.get :entity-id @person :attribute "DATE_OF_BIRTH")"#,
        r#"(attribute.get :cbu-id @cbu :attribute "RISK_RATING" :as-of-date "2024-01-01")"#,
    ],
};

pub static ATTRIBUTE_BULK_SET: VerbDef = VerbDef {
    name: "attribute.bulk-set",
    domain: "attribute",
    args: &[
        ArgSpec {
            name: ":entity-id",
            sem_type: SemType::Symbol,
            required: RequiredRule::UnlessProvided(":cbu-id"),
            default: None,
            validation: &[],
            description: "Entity to set attributes for",
        },
        ArgSpec {
            name: ":cbu-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::UnlessProvided(":entity-id"),
            default: Some(DefaultValue::FromContext(ContextKey::CbuId)),
            validation: &[],
            description: "CBU to set attributes for",
        },
        ArgSpec {
            name: ":attributes",
            sem_type: SemType::Map(&[
                ArgSpec {
                    name: ":attr",
                    sem_type: SemType::Ref(RefType::Attribute),
                    required: RequiredRule::Always,
                    default: None,
                    validation: &[],
                    description: "Attribute code",
                },
                ArgSpec {
                    name: ":value",
                    sem_type: SemType::String,
                    required: RequiredRule::Always,
                    default: None,
                    validation: &[],
                    description: "Attribute value",
                },
            ]),
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Map of attribute codes to values",
        },
        ArgSpec {
            name: ":source",
            sem_type: SemType::Enum(&["DOCUMENT", "CLIENT_PROVIDED", "THIRD_PARTY", "CALCULATED", "MANUAL"]),
            required: RequiredRule::Never,
            default: Some(DefaultValue::Str("MANUAL")),
            validation: &[],
            description: "Source for all attributes",
        },
        ArgSpec {
            name: ":source-ref",
            sem_type: SemType::String,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Reference to source document",
        },
    ],
    constraints: &[
        CrossConstraint::AtLeastOne(&[":entity-id", ":cbu-id"]),
    ],
    produces: None,
    crud_asset: "ATTRIBUTE_VALUE",
    description: "Set multiple attributes at once",
    examples: &[
        r#"(attribute.bulk-set :entity-id @person :attributes {"DATE_OF_BIRTH" "1985-03-15" "NATIONALITY" "GB" "TAX_ID" "AB123456C"} :source "DOCUMENT")"#,
    ],
};

pub static ATTRIBUTE_VALIDATE: VerbDef = VerbDef {
    name: "attribute.validate",
    domain: "attribute",
    args: &[
        ArgSpec {
            name: ":entity-id",
            sem_type: SemType::Symbol,
            required: RequiredRule::UnlessProvided(":cbu-id"),
            default: None,
            validation: &[],
            description: "Entity to validate attributes for",
        },
        ArgSpec {
            name: ":cbu-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::UnlessProvided(":entity-id"),
            default: Some(DefaultValue::FromContext(ContextKey::CbuId)),
            validation: &[],
            description: "CBU to validate attributes for",
        },
        ArgSpec {
            name: ":attributes",
            sem_type: SemType::ListOf(&SemType::Ref(RefType::Attribute)),
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Specific attributes to validate (defaults to all)",
        },
        ArgSpec {
            name: ":validation-rules",
            sem_type: SemType::Enum(&["COMPLETENESS", "FORMAT", "CONSISTENCY", "ALL"]),
            required: RequiredRule::Never,
            default: Some(DefaultValue::Str("ALL")),
            validation: &[],
            description: "What validation rules to apply",
        },
    ],
    constraints: &[
        CrossConstraint::AtLeastOne(&[":entity-id", ":cbu-id"]),
    ],
    produces: None,
    crud_asset: "ATTRIBUTE_VALIDATION",
    description: "Validate attributes against dictionary rules",
    examples: &[
        r#"(attribute.validate :entity-id @person :validation-rules "ALL")"#,
        r#"(attribute.validate :cbu-id @cbu :attributes ["JURISDICTION" "CLIENT_TYPE"] :validation-rules "COMPLETENESS")"#,
    ],
};

pub static ATTRIBUTE_CLEAR: VerbDef = VerbDef {
    name: "attribute.clear",
    domain: "attribute",
    args: &[
        ArgSpec {
            name: ":entity-id",
            sem_type: SemType::Symbol,
            required: RequiredRule::UnlessProvided(":cbu-id"),
            default: None,
            validation: &[],
            description: "Entity to clear attribute for",
        },
        ArgSpec {
            name: ":cbu-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::UnlessProvided(":entity-id"),
            default: Some(DefaultValue::FromContext(ContextKey::CbuId)),
            validation: &[],
            description: "CBU to clear attribute for",
        },
        ArgSpec {
            name: ":attribute",
            sem_type: SemType::Ref(RefType::Attribute),
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Attribute to clear",
        },
        ArgSpec {
            name: ":reason",
            sem_type: SemType::String,
            required: RequiredRule::Always,
            default: None,
            validation: &[ValidationRule::NotEmpty],
            description: "Reason for clearing the attribute",
        },
    ],
    constraints: &[
        CrossConstraint::AtLeastOne(&[":entity-id", ":cbu-id"]),
    ],
    produces: None,
    crud_asset: "ATTRIBUTE_VALUE",
    description: "Clear an attribute value (soft delete with reason)",
    examples: &[
        r#"(attribute.clear :entity-id @person :attribute "TAX_ID" :reason "Incorrect value, awaiting correction")"#,
    ],
};

pub static ATTRIBUTE_HISTORY: VerbDef = VerbDef {
    name: "attribute.history",
    domain: "attribute",
    args: &[
        ArgSpec {
            name: ":entity-id",
            sem_type: SemType::Symbol,
            required: RequiredRule::UnlessProvided(":cbu-id"),
            default: None,
            validation: &[],
            description: "Entity to get history for",
        },
        ArgSpec {
            name: ":cbu-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::UnlessProvided(":entity-id"),
            default: Some(DefaultValue::FromContext(ContextKey::CbuId)),
            validation: &[],
            description: "CBU to get history for",
        },
        ArgSpec {
            name: ":attribute",
            sem_type: SemType::Ref(RefType::Attribute),
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Attribute to get history for",
        },
        ArgSpec {
            name: ":from-date",
            sem_type: SemType::Date,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Start of history period",
        },
        ArgSpec {
            name: ":to-date",
            sem_type: SemType::Date,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "End of history period",
        },
    ],
    constraints: &[
        CrossConstraint::AtLeastOne(&[":entity-id", ":cbu-id"]),
    ],
    produces: None,
    crud_asset: "ATTRIBUTE_VALUE",
    description: "Get version history for an attribute",
    examples: &[
        r#"(attribute.history :entity-id @person :attribute "ADDRESS" :from-date "2020-01-01")"#,
    ],
};

pub static ATTRIBUTE_COPY_FROM_DOCUMENT: VerbDef = VerbDef {
    name: "attribute.copy-from-document",
    domain: "attribute",
    args: &[
        ArgSpec {
            name: ":document-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Document to copy attributes from",
        },
        ArgSpec {
            name: ":entity-id",
            sem_type: SemType::Symbol,
            required: RequiredRule::UnlessProvided(":cbu-id"),
            default: None,
            validation: &[],
            description: "Target entity",
        },
        ArgSpec {
            name: ":cbu-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::UnlessProvided(":entity-id"),
            default: Some(DefaultValue::FromContext(ContextKey::CbuId)),
            validation: &[],
            description: "Target CBU",
        },
        ArgSpec {
            name: ":attributes",
            sem_type: SemType::ListOf(&SemType::Ref(RefType::Attribute)),
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Specific attributes to copy (defaults to all extracted)",
        },
        ArgSpec {
            name: ":overwrite",
            sem_type: SemType::Boolean,
            required: RequiredRule::Never,
            default: Some(DefaultValue::Bool(false)),
            validation: &[],
            description: "Whether to overwrite existing values",
        },
    ],
    constraints: &[
        CrossConstraint::AtLeastOne(&[":entity-id", ":cbu-id"]),
    ],
    produces: None,
    crud_asset: "ATTRIBUTE_VALUE",
    description: "Copy extracted attributes from a document to entity/CBU",
    examples: &[
        r#"(attribute.copy-from-document :document-id "DOC-001" :entity-id @person :overwrite true)"#,
        r#"(attribute.copy-from-document :document-id "DOC-002" :cbu-id @cbu :attributes ["JURISDICTION" "COMPANY_NUMBER"])"#,
    ],
};
