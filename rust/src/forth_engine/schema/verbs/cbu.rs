//! CBU domain verb definitions.

use crate::forth_engine::schema::types::*;

pub static CBU_ENSURE: VerbDef = VerbDef {
    name: "cbu.ensure",
    domain: "cbu",
    args: &[
        ArgSpec {
            name: ":cbu-name",
            sem_type: SemType::String,
            required: RequiredRule::Always,
            default: None,
            validation: &[ValidationRule::NotEmpty, ValidationRule::Length { min: Some(1), max: Some(255) }],
            description: "Name of the CBU (Client Business Unit)",
        },
        ArgSpec {
            name: ":jurisdiction",
            sem_type: SemType::Ref(RefType::Jurisdiction),
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Jurisdiction (country) of registration",
        },
        ArgSpec {
            name: ":nature-purpose",
            sem_type: SemType::String,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Nature and purpose of the business arrangement",
        },
        ArgSpec {
            name: ":client-type",
            sem_type: SemType::Enum(&[
                "UCITS", "AIFM", "SICAV", "FCP", "SIF", "RAIF",
                "PENSION_FUND", "SOVEREIGN_WEALTH", "CORPORATE", "INDIVIDUAL"
            ]),
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Type of client structure",
        },
        ArgSpec {
            name: ":as",
            sem_type: SemType::Symbol,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Symbol name to capture CBU ID for later reference",
        },
    ],
    constraints: &[],
    produces: Some(ProducesSpec {
        capture_as: ContextKey::CbuId,
        description: "The CBU UUID",
    }),
    crud_asset: "CBU",
    description: "Create or update a CBU (idempotent via name)",
    examples: &[
        r#"(cbu.ensure :cbu-name "Meridian Global Fund" :jurisdiction "LU" :as @cbu)"#,
        r#"(cbu.ensure :cbu-name "Test Fund" :client-type "UCITS")"#,
    ],
};

pub static CBU_CREATE: VerbDef = VerbDef {
    name: "cbu.create",
    domain: "cbu",
    args: &[
        ArgSpec {
            name: ":cbu-name",
            sem_type: SemType::String,
            required: RequiredRule::Always,
            default: None,
            validation: &[ValidationRule::NotEmpty],
            description: "Name of the CBU",
        },
        ArgSpec {
            name: ":client-type",
            sem_type: SemType::Enum(&[
                "UCITS", "AIFM", "SICAV", "FCP", "SIF", "RAIF",
                "PENSION_FUND", "SOVEREIGN_WEALTH", "CORPORATE", "INDIVIDUAL",
                "CORP", "FUND", "HEDGE_FUND"
            ]),
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Type of client structure",
        },
        ArgSpec {
            name: ":jurisdiction",
            sem_type: SemType::Ref(RefType::Jurisdiction),
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Jurisdiction of registration",
        },
        ArgSpec {
            name: ":nature-purpose",
            sem_type: SemType::String,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Nature and purpose of the business",
        },
        ArgSpec {
            name: ":description",
            sem_type: SemType::String,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Additional description",
        },
        ArgSpec {
            name: ":as",
            sem_type: SemType::Symbol,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Symbol to capture CBU ID",
        },
    ],
    constraints: &[],
    produces: Some(ProducesSpec {
        capture_as: ContextKey::CbuId,
        description: "The created CBU UUID",
    }),
    crud_asset: "CBU",
    description: "Create a new Client Business Unit",
    examples: &[
        r#"(cbu.create :cbu-name "AcmeFund" :client-type "HEDGE_FUND" :jurisdiction "GB")"#,
    ],
};

pub static CBU_ATTACH_ENTITY: VerbDef = VerbDef {
    name: "cbu.attach-entity",
    domain: "cbu",
    args: &[
        ArgSpec {
            name: ":cbu-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::Never,
            default: Some(DefaultValue::FromContext(ContextKey::CbuId)),
            validation: &[],
            description: "CBU to attach entity to (defaults to current context)",
        },
        ArgSpec {
            name: ":entity-id",
            sem_type: SemType::Symbol,
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Entity to attach (reference to previously created entity)",
        },
        ArgSpec {
            name: ":role",
            sem_type: SemType::Ref(RefType::Role),
            required: RequiredRule::Always,
            default: None,
            validation: &[ValidationRule::LookupMustExist],
            description: "Role this entity plays in the CBU",
        },
        ArgSpec {
            name: ":ownership-percent",
            sem_type: SemType::Decimal,
            required: RequiredRule::IfEquals { arg: ":role", value: "BeneficialOwner" },
            default: None,
            validation: &[ValidationRule::Range { min: Some(0.0), max: Some(100.0) }],
            description: "Ownership percentage (required for UBO roles)",
        },
        ArgSpec {
            name: ":effective-date",
            sem_type: SemType::Date,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "When this relationship became effective",
        },
    ],
    constraints: &[],
    produces: None,
    crud_asset: "CBU_ENTITY_ROLE",
    description: "Attach an existing entity to a CBU with a specific role",
    examples: &[
        r#"(cbu.attach-entity :entity-id @company :role "InvestmentManager")"#,
        r#"(cbu.attach-entity :entity-id @person :role "BeneficialOwner" :ownership-percent 25.0)"#,
    ],
};

pub static CBU_DETACH_ENTITY: VerbDef = VerbDef {
    name: "cbu.detach-entity",
    domain: "cbu",
    args: &[
        ArgSpec {
            name: ":cbu-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::Never,
            default: Some(DefaultValue::FromContext(ContextKey::CbuId)),
            validation: &[],
            description: "CBU to detach entity from",
        },
        ArgSpec {
            name: ":entity-id",
            sem_type: SemType::Symbol,
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Entity to detach",
        },
        ArgSpec {
            name: ":role",
            sem_type: SemType::Ref(RefType::Role),
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Specific role to detach (optional)",
        },
    ],
    constraints: &[],
    produces: None,
    crud_asset: "CBU_ENTITY_ROLE",
    description: "Detach an entity from a CBU",
    examples: &[
        r#"(cbu.detach-entity :cbu-id @cbu :entity-id @company)"#,
    ],
};

pub static CBU_LIST_ENTITIES: VerbDef = VerbDef {
    name: "cbu.list-entities",
    domain: "cbu",
    args: &[
        ArgSpec {
            name: ":cbu-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::Never,
            default: Some(DefaultValue::FromContext(ContextKey::CbuId)),
            validation: &[],
            description: "CBU to list entities for",
        },
        ArgSpec {
            name: ":role",
            sem_type: SemType::Ref(RefType::Role),
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Filter by role",
        },
    ],
    constraints: &[],
    produces: None,
    crud_asset: "CBU_ENTITY_ROLE",
    description: "List all entities attached to a CBU",
    examples: &[
        r#"(cbu.list-entities :cbu-id @cbu)"#,
        r#"(cbu.list-entities :cbu-id @cbu :role "BeneficialOwner")"#,
    ],
};

pub static CBU_READ: VerbDef = VerbDef {
    name: "cbu.read",
    domain: "cbu",
    args: &[
        ArgSpec {
            name: ":cbu-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "CBU ID to read",
        },
    ],
    constraints: &[],
    produces: None,
    crud_asset: "CBU",
    description: "Read a CBU by ID",
    examples: &[
        r#"(cbu.read :cbu-id "550e8400-e29b-41d4-a716-446655440000")"#,
    ],
};

pub static CBU_UPDATE: VerbDef = VerbDef {
    name: "cbu.update",
    domain: "cbu",
    args: &[
        ArgSpec {
            name: ":cbu-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "CBU ID to update",
        },
        ArgSpec {
            name: ":name",
            sem_type: SemType::String,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "New name",
        },
        ArgSpec {
            name: ":status",
            sem_type: SemType::Enum(&["DRAFT", "ACTIVE", "SUSPENDED", "CLOSED"]),
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "New status",
        },
    ],
    constraints: &[],
    produces: None,
    crud_asset: "CBU",
    description: "Update a CBU",
    examples: &[
        r#"(cbu.update :cbu-id "..." :name "NewName")"#,
    ],
};

pub static CBU_DELETE: VerbDef = VerbDef {
    name: "cbu.delete",
    domain: "cbu",
    args: &[
        ArgSpec {
            name: ":cbu-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "CBU ID to delete",
        },
    ],
    constraints: &[],
    produces: None,
    crud_asset: "CBU",
    description: "Delete a CBU",
    examples: &[
        r#"(cbu.delete :cbu-id "...")"#,
    ],
};

pub static CBU_FINALIZE: VerbDef = VerbDef {
    name: "cbu.finalize",
    domain: "cbu",
    args: &[
        ArgSpec {
            name: ":cbu-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::Never,
            default: Some(DefaultValue::FromContext(ContextKey::CbuId)),
            validation: &[],
            description: "CBU ID to finalize",
        },
        ArgSpec {
            name: ":status",
            sem_type: SemType::Enum(&["ACTIVE", "APPROVED", "PENDING_REVIEW"]),
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Final status",
        },
    ],
    constraints: &[],
    produces: None,
    crud_asset: "CBU",
    description: "Finalize a CBU with a status",
    examples: &[
        r#"(cbu.finalize :cbu-id "..." :status "ACTIVE")"#,
    ],
};
