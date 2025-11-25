//! Document domain verb definitions.

use crate::forth_engine::schema::types::*;

pub static DOCUMENT_REQUEST: VerbDef = VerbDef {
    name: "document.request",
    domain: "document",
    args: &[
        ArgSpec {
            name: ":investigation-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::Never,
            default: Some(DefaultValue::FromContext(ContextKey::InvestigationId)),
            validation: &[],
            description: "Investigation this request belongs to",
        },
        ArgSpec {
            name: ":entity-id",
            sem_type: SemType::Symbol,
            required: RequiredRule::UnlessProvided(":cbu-id"),
            default: None,
            validation: &[],
            description: "Entity to request document from",
        },
        ArgSpec {
            name: ":cbu-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::UnlessProvided(":entity-id"),
            default: Some(DefaultValue::FromContext(ContextKey::CbuId)),
            validation: &[],
            description: "CBU to request document for",
        },
        ArgSpec {
            name: ":document-type",
            sem_type: SemType::Ref(RefType::DocumentType),
            required: RequiredRule::Always,
            default: None,
            validation: &[ValidationRule::LookupMustExist],
            description: "Type of document to request",
        },
        ArgSpec {
            name: ":source",
            sem_type: SemType::Enum(&["REGISTRY", "CLIENT", "THIRD_PARTY"]),
            required: RequiredRule::Never,
            default: Some(DefaultValue::Str("CLIENT")),
            validation: &[],
            description: "Where to request document from",
        },
        ArgSpec {
            name: ":priority",
            sem_type: SemType::Enum(&["LOW", "NORMAL", "HIGH", "URGENT"]),
            required: RequiredRule::Never,
            default: Some(DefaultValue::Str("NORMAL")),
            validation: &[],
            description: "Request priority level",
        },
        ArgSpec {
            name: ":due-date",
            sem_type: SemType::Date,
            required: RequiredRule::Never,
            default: None,
            validation: &[ValidationRule::DateRange { min: Some(DateBound::Today), max: None }],
            description: "When document is needed (must be future date)",
        },
    ],
    constraints: &[
        CrossConstraint::AtLeastOne(&[":entity-id", ":cbu-id"]),
    ],
    produces: Some(ProducesSpec {
        capture_as: ContextKey::DocumentRequestId,
        description: "The document request UUID",
    }),
    crud_asset: "DOCUMENT_REQUEST",
    description: "Request a document for KYC investigation",
    examples: &[
        r#"(document.request :entity-id @company :document-type "CERT_OF_INCORP")"#,
        r#"(document.request :entity-id @person :document-type "PASSPORT" :priority "HIGH")"#,
    ],
};

pub static DOCUMENT_RECEIVE: VerbDef = VerbDef {
    name: "document.receive",
    domain: "document",
    args: &[
        ArgSpec {
            name: ":request-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::Never,
            default: Some(DefaultValue::FromContext(ContextKey::DocumentRequestId)),
            validation: &[],
            description: "Document request ID",
        },
        ArgSpec {
            name: ":document-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Received document ID",
        },
        ArgSpec {
            name: ":received-date",
            sem_type: SemType::Date,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Date document was received",
        },
    ],
    constraints: &[],
    produces: None,
    crud_asset: "DOCUMENT",
    description: "Record receipt of a requested document",
    examples: &[
        r#"(document.receive :request-id @req :document-id "doc-uuid")"#,
    ],
};

pub static DOCUMENT_VERIFY: VerbDef = VerbDef {
    name: "document.verify",
    domain: "document",
    args: &[
        ArgSpec {
            name: ":doc-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Document ID to verify",
        },
        ArgSpec {
            name: ":verification-type",
            sem_type: SemType::Enum(&["MANUAL", "AUTOMATED", "THIRD_PARTY"]),
            required: RequiredRule::Never,
            default: Some(DefaultValue::Str("MANUAL")),
            validation: &[],
            description: "Type of verification",
        },
        ArgSpec {
            name: ":status",
            sem_type: SemType::Enum(&["VERIFIED", "REJECTED", "PENDING_REVIEW"]),
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Verification status",
        },
    ],
    constraints: &[],
    produces: None,
    crud_asset: "DOCUMENT",
    description: "Verify a document",
    examples: &[
        r#"(document.verify :doc-id "DOC-001")"#,
    ],
};

pub static DOCUMENT_EXTRACT_ATTRIBUTES: VerbDef = VerbDef {
    name: "document.extract-attributes",
    domain: "document",
    args: &[
        ArgSpec {
            name: ":document-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Document to extract from",
        },
        ArgSpec {
            name: ":document-type",
            sem_type: SemType::Ref(RefType::DocumentType),
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Type of document (determines extractable attributes)",
        },
        ArgSpec {
            name: ":attributes",
            sem_type: SemType::ListOf(&SemType::Ref(RefType::Attribute)),
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Specific attributes to extract (defaults to all for doc type)",
        },
    ],
    constraints: &[],
    produces: None,
    crud_asset: "ATTRIBUTE_VALUE",
    description: "Extract attributes from a document",
    examples: &[
        r#"(document.extract-attributes :document-id "DOC-001" :document-type "UK-PASSPORT")"#,
    ],
};

pub static DOCUMENT_LINK: VerbDef = VerbDef {
    name: "document.link",
    domain: "document",
    args: &[
        ArgSpec {
            name: ":doc-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Document to link",
        },
        ArgSpec {
            name: ":entity-id",
            sem_type: SemType::Symbol,
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Entity to link to",
        },
        ArgSpec {
            name: ":relationship-type",
            sem_type: SemType::Enum(&["PRIMARY", "SUPPORTING", "REFERENCE"]),
            required: RequiredRule::Never,
            default: Some(DefaultValue::Str("PRIMARY")),
            validation: &[],
            description: "Type of relationship",
        },
    ],
    constraints: &[],
    produces: None,
    crud_asset: "DOCUMENT_LINK",
    description: "Link a document to an entity",
    examples: &[
        r#"(document.link :doc-id "DOC-001" :entity-id @company)"#,
    ],
};

pub static DOCUMENT_CATALOG: VerbDef = VerbDef {
    name: "document.catalog",
    domain: "document",
    args: &[
        ArgSpec {
            name: ":doc-id",
            sem_type: SemType::String,
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Document ID",
        },
        ArgSpec {
            name: ":doc-type",
            sem_type: SemType::Ref(RefType::DocumentType),
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Type of document",
        },
    ],
    constraints: &[],
    produces: None,
    crud_asset: "DOCUMENT",
    description: "Catalog a document for processing",
    examples: &[
        r#"(document.catalog :doc-id "DOC-001" :doc-type "UK-PASSPORT")"#,
    ],
};
