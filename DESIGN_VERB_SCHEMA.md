# Design: Formal Verb Schema System

**Created:** 2025-11-25  
**Status:** DESIGN SPECIFICATION  
**Priority:** P0 — Production Readiness  
**Scope:** Verb schemas, validation pipeline, typed AST, error reporting  

---

## Executive Summary

The DSL requires formal verb schemas to be production-ready. Currently, argument validation happens at runtime (in CrudExecutor), producing errors with no line numbers and poor developer experience.

**This design introduces:**
1. **Formal VerbDef schemas** — Declarative argument specifications
2. **Schema Validator phase** — Validates Raw AST against schemas before execution
3. **Typed AST** — ValidatedAST with resolved types and symbols
4. **Rich error reporting** — Line numbers, suggestions, "did you mean?"
5. **Single source of truth** — Schemas drive validation, LSP, LLM context, and docs

---

## Part 1: Problem Statement

### Current Flow (Broken)

```
DSL Source
    │
    ▼
┌─────────────┐
│ Nom Parser  │  ← Only checks syntax (balanced parens, valid tokens)
└─────────────┘
    │
    ▼
┌─────────────┐
│ Raw AST     │  ← Untyped: Call { name: String, args: Vec<Arg> }
└─────────────┘
    │
    ▼
┌─────────────┐
│ Word Fn     │  ← No validation, just emits CRUD
└─────────────┘
    │
    ▼
┌─────────────┐
│CrudExecutor │  ← FINALLY validates: "cbu-id required" → RUNTIME ERROR
└─────────────┘
                   ↑
                   TOO LATE!
                   • No line numbers
                   • No suggestions
                   • Hard to debug
                   • LLM can't learn rules
```

### What We Need

```
DSL Source
    │
    ▼
┌─────────────┐
│ Nom Parser  │  Phase 1: Syntax
└─────────────┘
    │
    ▼
┌─────────────┐
│ Raw AST     │  With source spans
└─────────────┘
    │
    ▼
┌──────────────────────────────────────────────────┐
│           SCHEMA VALIDATOR (Phase 2)             │  ← NEW
│                                                  │
│  • Lookup VerbDef by name                        │
│  • Validate each arg against ArgSpec            │
│  • Check types (SemType)                        │
│  • Validate against lookup tables (RefTypes)    │
│  • Check required/optional/conditional          │
│  • Apply defaults and context injection         │
│  • Check cross-argument constraints             │
│  • Build symbol table for @references           │
│                                                  │
│  Output: ValidatedAST OR rich errors            │
└──────────────────────────────────────────────────┘
    │                    │
    │ success            │ errors
    ▼                    ▼
┌─────────────┐    ┌─────────────────────────────┐
│ValidatedAST │    │ error[E008]: unknown role   │
└─────────────┘    │   --> file.dsl:12:45        │
    │              │ 12| ... :role "Investmanger" │
    ▼              │                     ^        │
┌─────────────┐    │ hint: did you mean           │
│  Execute    │    │   "InvestmentManager"?       │
└─────────────┘    └─────────────────────────────┘
```

---

## Part 2: Core Type Definitions

### 2.1 Semantic Types

```rust
// rust/src/dsl_runtime/schema/types.rs

use std::collections::HashMap;
use uuid::Uuid;
use chrono::NaiveDate;

/// Semantic type for argument values
#[derive(Debug, Clone, PartialEq)]
pub enum SemType {
    // =========== Primitives ===========
    String,
    Uuid,
    Integer,
    Decimal,
    Date,
    Boolean,
    
    // =========== Reference Types ===========
    /// Reference to lookup table — triggers picklist in LSP
    Ref(RefType),
    
    // =========== Enumeration ===========
    /// Fixed set of allowed values
    Enum(&'static [&'static str]),
    
    // =========== Symbol ===========
    /// Reference to session symbol (@name)
    Symbol,
    
    // =========== Composite ===========
    /// List of values
    List(Box<SemType>),
    
    /// Nested map structure
    Map(&'static [ArgSpec]),
    
    // =========== Union ===========
    /// One of several types
    OneOf(&'static [SemType]),
}

/// Reference types that map to lookup tables
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefType {
    /// document_types.type_code
    DocumentType,
    
    /// attribute_dictionary.attr_id  
    Attribute,
    
    /// roles.name
    Role,
    
    /// entity_types.type_code
    EntityType,
    
    /// jurisdictions.iso_code
    Jurisdiction,
    
    /// screening_lists.list_code
    ScreeningList,
    
    /// currencies.iso_code
    Currency,
}

impl RefType {
    /// Human-readable name for error messages
    pub fn name(&self) -> &'static str {
        match self {
            Self::DocumentType => "document type",
            Self::Attribute => "attribute",
            Self::Role => "role",
            Self::EntityType => "entity type",
            Self::Jurisdiction => "jurisdiction",
            Self::ScreeningList => "screening list",
            Self::Currency => "currency",
        }
    }
    
    /// Table and column for lookups
    pub fn table_info(&self) -> (&'static str, &'static str, &'static str) {
        // (table, code_column, display_column)
        match self {
            Self::DocumentType => ("document_types", "type_code", "type_name"),
            Self::Attribute => ("attribute_dictionary", "attr_id", "attr_name"),
            Self::Role => ("roles", "name", "description"),
            Self::EntityType => ("entity_types", "type_code", "type_name"),
            Self::Jurisdiction => ("jurisdictions", "iso_code", "name"),
            Self::ScreeningList => ("screening_lists", "list_code", "list_name"),
            Self::Currency => ("currencies", "iso_code", "name"),
        }
    }
}
```

### 2.2 Argument Specification

```rust
/// Specification for a single argument
#[derive(Debug, Clone)]
pub struct ArgSpec {
    /// Keyword name (e.g., ":cbu-id")
    pub name: &'static str,
    
    /// Semantic type — drives validation and LSP completions
    pub sem_type: SemType,
    
    /// When is this argument required?
    pub required: RequiredRule,
    
    /// Default value if not provided
    pub default: Option<DefaultValue>,
    
    /// Additional validation rules
    pub validation: &'static [ValidationRule],
    
    /// Human description for LSP hover and docs
    pub description: &'static str,
}

/// Rules for when an argument is required
#[derive(Debug, Clone, PartialEq)]
pub enum RequiredRule {
    /// Always required
    Always,
    
    /// Never required (optional)
    Never,
    
    /// Required unless another arg is provided
    UnlessProvided(&'static str),
    
    /// Required if another arg equals a specific value
    IfEquals {
        arg: &'static str,
        value: &'static str,
    },
    
    /// Required if another arg is provided
    IfProvided(&'static str),
}

/// Default values for optional arguments
#[derive(Debug, Clone)]
pub enum DefaultValue {
    /// Static string value
    Str(&'static str),
    
    /// Static integer value
    Int(i64),
    
    /// Static decimal value
    Decimal(f64),
    
    /// Static boolean value
    Bool(bool),
    
    /// Inject from runtime context (e.g., env.cbu_id)
    FromContext(ContextKey),
}

/// Keys for context injection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextKey {
    CbuId,
    EntityId,
    InvestigationId,
    DecisionId,
    DocumentRequestId,
    ScreeningId,
}

impl ContextKey {
    pub fn env_field(&self) -> &'static str {
        match self {
            Self::CbuId => "cbu_id",
            Self::EntityId => "entity_id",
            Self::InvestigationId => "investigation_id",
            Self::DecisionId => "decision_id",
            Self::DocumentRequestId => "document_request_id",
            Self::ScreeningId => "screening_id",
        }
    }
}
```

### 2.3 Validation Rules

```rust
/// Additional validation rules beyond type checking
#[derive(Debug, Clone)]
pub enum ValidationRule {
    /// Value must exist in lookup table (already implied by Ref type, but explicit)
    LookupMustExist,
    
    /// String must match regex pattern
    Pattern {
        regex: &'static str,
        description: &'static str,
    },
    
    /// Numeric value must be in range
    Range {
        min: Option<f64>,
        max: Option<f64>,
    },
    
    /// String length constraints
    Length {
        min: Option<usize>,
        max: Option<usize>,
    },
    
    /// Date constraints
    DateRange {
        min: Option<DateBound>,
        max: Option<DateBound>,
    },
    
    /// Value must not be empty
    NotEmpty,
    
    /// Custom validation function name (for complex cases)
    Custom(&'static str),
}

#[derive(Debug, Clone)]
pub enum DateBound {
    /// Literal date
    Literal(&'static str),
    
    /// Dynamic: today
    Today,
    
    /// Dynamic: N days from today
    DaysFromToday(i32),
}
```

### 2.4 Cross-Argument Constraints

```rust
/// Constraints that involve multiple arguments
#[derive(Debug, Clone)]
pub enum CrossConstraint {
    /// Exactly one of these must be provided
    ExactlyOne(&'static [&'static str]),
    
    /// At least one of these must be provided
    AtLeastOne(&'static [&'static str]),
    
    /// If A is provided, B is required
    Requires {
        if_present: &'static str,
        then_require: &'static str,
    },
    
    /// If A is provided, B is forbidden
    Excludes {
        if_present: &'static str,
        then_forbid: &'static str,
    },
    
    /// If A equals X, then B is required
    ConditionalRequired {
        if_arg: &'static str,
        equals: &'static str,
        then_require: &'static str,
    },
    
    /// A must be less than B (for dates, numbers)
    LessThan {
        lesser: &'static str,
        greater: &'static str,
    },
}
```

### 2.5 Verb Definition

```rust
/// Complete definition of a DSL verb
#[derive(Debug, Clone)]
pub struct VerbDef {
    /// Verb name (e.g., "cbu.attach-entity")
    pub name: &'static str,
    
    /// Domain for grouping (e.g., "cbu", "entity", "kyc")
    pub domain: &'static str,
    
    /// Argument specifications
    pub args: &'static [ArgSpec],
    
    /// Cross-argument constraints
    pub constraints: &'static [CrossConstraint],
    
    /// What this verb produces (for context capture)
    pub produces: Option<ProducesSpec>,
    
    /// CRUD asset type this verb generates
    pub crud_asset: &'static str,
    
    /// Human description
    pub description: &'static str,
    
    /// Usage examples
    pub examples: &'static [&'static str],
}

/// What a verb produces for context capture
#[derive(Debug, Clone)]
pub struct ProducesSpec {
    /// Context key to capture result into
    pub capture_as: ContextKey,
    
    /// Description of what's produced
    pub description: &'static str,
}
```

---

## Part 3: Verb Registry

### 3.1 Static Verb Definitions

```rust
// rust/src/dsl_runtime/schema/verbs/cbu.rs

use super::*;

pub static CBU_ENSURE: VerbDef = VerbDef {
    name: "cbu.ensure",
    domain: "cbu",
    args: &[
        ArgSpec {
            name: ":cbu-name",
            sem_type: SemType::String,
            required: RequiredRule::Always,
            default: None,
            validation: &[
                ValidationRule::NotEmpty,
                ValidationRule::Length { min: Some(1), max: Some(255) },
            ],
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
        description: "The CBU's UUID",
    }),
    crud_asset: "CBU",
    description: "Create or update a CBU (idempotent via name)",
    examples: &[
        r#"(cbu.ensure :cbu-name "Meridian Global Fund" :jurisdiction "LU" :as @cbu)"#,
        r#"(cbu.ensure :cbu-name "Test Fund" :client-type "UCITS")"#,
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
            required: RequiredRule::IfEquals {
                arg: ":role",
                value: "BeneficialOwner",
            },
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
```

### 3.2 Entity Verbs

```rust
// rust/src/dsl_runtime/schema/verbs/entity.rs

pub static ENTITY_CREATE_LIMITED_COMPANY: VerbDef = VerbDef {
    name: "entity.create-limited-company",
    domain: "entity",
    args: &[
        ArgSpec {
            name: ":name",
            sem_type: SemType::String,
            required: RequiredRule::Always,
            default: None,
            validation: &[
                ValidationRule::NotEmpty,
                ValidationRule::Length { min: Some(1), max: Some(255) },
            ],
            description: "Company name",
        },
        ArgSpec {
            name: ":jurisdiction",
            sem_type: SemType::Ref(RefType::Jurisdiction),
            required: RequiredRule::Always,
            default: None,
            validation: &[ValidationRule::LookupMustExist],
            description: "Country of incorporation (ISO code)",
        },
        ArgSpec {
            name: ":company-number",
            sem_type: SemType::String,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Official company registration number",
        },
        ArgSpec {
            name: ":incorporation-date",
            sem_type: SemType::Date,
            required: RequiredRule::Never,
            default: None,
            validation: &[ValidationRule::DateRange { 
                min: None, 
                max: Some(DateBound::Today) 
            }],
            description: "Date of incorporation (cannot be in future)",
        },
        ArgSpec {
            name: ":registered-office",
            sem_type: SemType::String,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Registered office address",
        },
        ArgSpec {
            name: ":share-capital",
            sem_type: SemType::Decimal,
            required: RequiredRule::Never,
            default: None,
            validation: &[ValidationRule::Range { min: Some(0.0), max: None }],
            description: "Share capital amount",
        },
        ArgSpec {
            name: ":currency",
            sem_type: SemType::Ref(RefType::Currency),
            required: RequiredRule::IfProvided(":share-capital"),
            default: None,
            validation: &[],
            description: "Currency for share capital",
        },
        ArgSpec {
            name: ":as",
            sem_type: SemType::Symbol,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Symbol name to capture entity ID",
        },
    ],
    constraints: &[],
    produces: Some(ProducesSpec {
        capture_as: ContextKey::EntityId,
        description: "The entity's UUID",
    }),
    crud_asset: "LIMITED_COMPANY",
    description: "Create a limited company entity",
    examples: &[
        r#"(entity.create-limited-company :name "Aviva Investors Ltd" :jurisdiction "GB" :as @aviva)"#,
        r#"(entity.create-limited-company :name "ManCo S.à r.l." :jurisdiction "LU" :company-number "B123456")"#,
    ],
};

pub static ENTITY_CREATE_PROPER_PERSON: VerbDef = VerbDef {
    name: "entity.create-proper-person",
    domain: "entity",
    args: &[
        ArgSpec {
            name: ":first-name",
            sem_type: SemType::String,
            required: RequiredRule::Always,
            default: None,
            validation: &[ValidationRule::NotEmpty],
            description: "First name",
        },
        ArgSpec {
            name: ":last-name",
            sem_type: SemType::String,
            required: RequiredRule::Always,
            default: None,
            validation: &[ValidationRule::NotEmpty],
            description: "Last name / surname",
        },
        ArgSpec {
            name: ":middle-name",
            sem_type: SemType::String,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Middle name(s)",
        },
        ArgSpec {
            name: ":date-of-birth",
            sem_type: SemType::Date,
            required: RequiredRule::Never,
            default: None,
            validation: &[ValidationRule::DateRange { 
                min: Some(DateBound::Literal("1900-01-01")), 
                max: Some(DateBound::Today) 
            }],
            description: "Date of birth",
        },
        ArgSpec {
            name: ":nationality",
            sem_type: SemType::Ref(RefType::Jurisdiction),
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Nationality (country code)",
        },
        ArgSpec {
            name: ":tax-id",
            sem_type: SemType::String,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Tax identification number",
        },
        ArgSpec {
            name: ":as",
            sem_type: SemType::Symbol,
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Symbol name to capture entity ID",
        },
    ],
    constraints: &[],
    produces: Some(ProducesSpec {
        capture_as: ContextKey::EntityId,
        description: "The person entity's UUID",
    }),
    crud_asset: "PROPER_PERSON",
    description: "Create a natural person entity",
    examples: &[
        r#"(entity.create-proper-person :first-name "John" :last-name "Smith" :as @john)"#,
        r#"(entity.create-proper-person :first-name "Chen" :last-name "Wei" :nationality "SG" :date-of-birth "1968-04-12")"#,
    ],
};
```

### 3.3 Document Verbs

```rust
// rust/src/dsl_runtime/schema/verbs/document.rs

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
            validation: &[ValidationRule::DateRange { 
                min: Some(DateBound::Today), 
                max: None 
            }],
            description: "When document is needed (must be future date)",
        },
    ],
    constraints: &[
        CrossConstraint::AtLeastOne(&[":entity-id", ":cbu-id"]),
    ],
    produces: Some(ProducesSpec {
        capture_as: ContextKey::DocumentRequestId,
        description: "The document request's UUID",
    }),
    crud_asset: "DOCUMENT_REQUEST",
    description: "Request a document for KYC investigation",
    examples: &[
        r#"(document.request :entity-id @company :document-type "CERT_OF_INCORP")"#,
        r#"(document.request :entity-id @person :document-type "PASSPORT" :priority "HIGH")"#,
    ],
};
```

### 3.4 KYC/Risk Verbs

```rust
// rust/src/dsl_runtime/schema/verbs/kyc.rs

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
            validation: &[ValidationRule::DateRange { 
                min: Some(DateBound::Today), 
                max: None 
            }],
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
        description: "The investigation's UUID",
    }),
    crud_asset: "INVESTIGATION",
    description: "Create a new KYC investigation for a CBU",
    examples: &[
        r#"(investigation.create :investigation-type "ENHANCED_DUE_DILIGENCE" :as @inv)"#,
        r#"(investigation.create :cbu-id @cbu :investigation-type "STANDARD" :deadline "2024-03-01")"#,
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
        ArgSpec {
            name: ":factors",
            sem_type: SemType::List(Box::new(SemType::Map(&[
                ArgSpec {
                    name: ":factor",
                    sem_type: SemType::String,
                    required: RequiredRule::Always,
                    default: None,
                    validation: &[],
                    description: "Risk factor name",
                },
                ArgSpec {
                    name: ":rating",
                    sem_type: SemType::Enum(&["LOW", "MEDIUM", "HIGH"]),
                    required: RequiredRule::Always,
                    default: None,
                    validation: &[],
                    description: "Factor rating",
                },
                ArgSpec {
                    name: ":weight",
                    sem_type: SemType::Decimal,
                    required: RequiredRule::Never,
                    default: None,
                    validation: &[ValidationRule::Range { min: Some(0.0), max: Some(1.0) }],
                    description: "Factor weight (0-1)",
                },
            ]))),
            required: RequiredRule::Never,
            default: None,
            validation: &[],
            description: "Individual risk factors",
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
```

### 3.5 Verb Registry

```rust
// rust/src/dsl_runtime/schema/registry.rs

use std::collections::HashMap;
use once_cell::sync::Lazy;

pub struct VerbRegistry {
    verbs: HashMap<&'static str, &'static VerbDef>,
    by_domain: HashMap<&'static str, Vec<&'static VerbDef>>,
}

impl VerbRegistry {
    pub fn new() -> Self {
        let mut verbs = HashMap::new();
        let mut by_domain: HashMap<&'static str, Vec<&'static VerbDef>> = HashMap::new();
        
        // Register all verbs
        let all_verbs: &[&'static VerbDef] = &[
            // CBU domain
            &cbu::CBU_ENSURE,
            &cbu::CBU_CREATE,
            &cbu::CBU_ATTACH_ENTITY,
            &cbu::CBU_DETACH_ENTITY,
            &cbu::CBU_LIST_ENTITIES,
            
            // Entity domain
            &entity::ENTITY_CREATE_LIMITED_COMPANY,
            &entity::ENTITY_CREATE_PROPER_PERSON,
            &entity::ENTITY_CREATE_PARTNERSHIP,
            &entity::ENTITY_CREATE_TRUST,
            &entity::ENTITY_ENSURE_OWNERSHIP,
            
            // Document domain
            &document::DOCUMENT_REQUEST,
            &document::DOCUMENT_RECEIVE,
            &document::DOCUMENT_VERIFY,
            &document::DOCUMENT_EXTRACT_ATTRIBUTES,
            
            // KYC domain
            &kyc::INVESTIGATION_CREATE,
            &kyc::INVESTIGATION_UPDATE_STATUS,
            &kyc::INVESTIGATION_COMPLETE,
            &kyc::RISK_ASSESS_CBU,
            &kyc::RISK_ASSESS_ENTITY,
            &kyc::RISK_SET_RATING,
            &kyc::RISK_ADD_FLAG,
            
            // Screening domain
            &screening::SCREENING_PEP,
            &screening::SCREENING_SANCTIONS,
            &screening::SCREENING_ADVERSE_MEDIA,
            &screening::SCREENING_RECORD_RESULT,
            &screening::SCREENING_RESOLVE,
            
            // Decision domain
            &decision::DECISION_RECORD,
            &decision::DECISION_ADD_CONDITION,
            &decision::DECISION_SATISFY_CONDITION,
            
            // Monitoring domain
            &monitoring::MONITORING_SETUP,
            &monitoring::MONITORING_SCHEDULE_REVIEW,
            &monitoring::MONITORING_RECORD_EVENT,
        ];
        
        for verb in all_verbs {
            verbs.insert(verb.name, *verb);
            by_domain.entry(verb.domain).or_default().push(*verb);
        }
        
        Self { verbs, by_domain }
    }
    
    pub fn get(&self, name: &str) -> Option<&'static VerbDef> {
        self.verbs.get(name).copied()
    }
    
    pub fn get_by_domain(&self, domain: &str) -> &[&'static VerbDef] {
        self.by_domain.get(domain).map(|v| v.as_slice()).unwrap_or(&[])
    }
    
    pub fn all(&self) -> impl Iterator<Item = &'static VerbDef> + '_ {
        self.verbs.values().copied()
    }
    
    pub fn domains(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.by_domain.keys().copied()
    }
    
    /// Suggest similar verbs for typo correction
    pub fn suggest(&self, name: &str) -> Vec<&'static str> {
        let mut suggestions: Vec<_> = self.verbs
            .keys()
            .filter(|k| {
                levenshtein_distance(k, name) <= 3 || k.contains(name) || name.contains(*k)
            })
            .copied()
            .collect();
        suggestions.sort_by_key(|k| levenshtein_distance(k, name));
        suggestions.truncate(3);
        suggestions
    }
}

/// Global verb registry
pub static VERB_REGISTRY: Lazy<VerbRegistry> = Lazy::new(VerbRegistry::new);
```

---

## Part 4: AST Types

### 4.1 Raw AST (From Parser)

```rust
// rust/src/dsl_runtime/ast/raw.rs

/// Source location for error reporting
#[derive(Debug, Clone, Copy, Default)]
pub struct Span {
    pub start: usize,  // Byte offset
    pub end: usize,
    pub line: u32,
    pub column: u32,
}

/// Raw AST from parser (unvalidated)
#[derive(Debug, Clone)]
pub struct RawAst {
    pub expressions: Vec<RawExpr>,
}

#[derive(Debug, Clone)]
pub struct RawExpr {
    pub span: Span,
    pub kind: RawExprKind,
}

#[derive(Debug, Clone)]
pub enum RawExprKind {
    Call {
        name: String,
        name_span: Span,
        args: Vec<RawArg>,
    },
    Comment(String),
}

#[derive(Debug, Clone)]
pub struct RawArg {
    pub span: Span,
    pub key: String,
    pub key_span: Span,
    pub value: RawValue,
    pub value_span: Span,
}

#[derive(Debug, Clone)]
pub enum RawValue {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Symbol(String),        // @name
    Keyword(String),       // :keyword (for nested structures)
    List(Vec<RawValue>),
    Map(Vec<(String, RawValue)>),
}
```

### 4.2 Validated AST (After Schema Validation)

```rust
// rust/src/dsl_runtime/ast/validated.rs

/// Validated AST (after schema validation)
#[derive(Debug, Clone)]
pub struct ValidatedAst {
    pub expressions: Vec<ValidatedExpr>,
    pub symbol_table: SymbolTable,
}

#[derive(Debug, Clone)]
pub struct ValidatedExpr {
    pub span: Span,
    pub kind: ValidatedExprKind,
}

#[derive(Debug, Clone)]
pub enum ValidatedExprKind {
    VerbCall {
        /// Reference to the verb schema
        verb: &'static VerbDef,
        
        /// Validated and typed arguments
        args: HashMap<String, TypedValue>,
        
        /// Arguments that were injected from context
        context_injected: Vec<String>,
        
        /// Arguments that used default values
        defaulted: Vec<String>,
        
        /// Symbol defined by this call (from :as)
        defines_symbol: Option<String>,
    },
    Comment(String),
}

/// Typed value (after validation)
#[derive(Debug, Clone)]
pub enum TypedValue {
    String(String),
    Uuid(Uuid),
    Integer(i64),
    Decimal(f64),
    Date(NaiveDate),
    Boolean(bool),
    
    /// Symbol reference with optional resolved ID
    Symbol {
        name: String,
        resolved_id: Option<Uuid>,
    },
    
    /// Validated reference to lookup table
    Ref {
        ref_type: RefType,
        code: String,
    },
    
    /// Validated enum value
    Enum(String),
    
    /// List of typed values
    List(Vec<TypedValue>),
    
    /// Map of typed values
    Map(HashMap<String, TypedValue>),
}

impl TypedValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            Self::Enum(s) => Some(s),
            Self::Ref { code, .. } => Some(code),
            _ => None,
        }
    }
    
    pub fn as_uuid(&self) -> Option<Uuid> {
        match self {
            Self::Uuid(u) => Some(*u),
            Self::Symbol { resolved_id: Some(id), .. } => Some(*id),
            _ => None,
        }
    }
}
```

### 4.3 Symbol Table

```rust
// rust/src/dsl_runtime/ast/symbols.rs

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct SymbolTable {
    symbols: HashMap<String, SymbolInfo>,
}

#[derive(Debug, Clone)]
pub struct SymbolInfo {
    /// What type of ID this symbol holds
    pub id_type: ContextKey,
    
    /// Where it was defined
    pub defined_at: Span,
    
    /// Which verb defined it
    pub defined_by: &'static str,
    
    /// Resolved UUID (known after execution)
    pub resolved_id: Option<Uuid>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self { symbols: HashMap::new() }
    }
    
    pub fn define(
        &mut self,
        name: &str,
        id_type: ContextKey,
        span: Span,
        verb_name: &'static str,
    ) -> Result<(), SymbolError> {
        if let Some(existing) = self.symbols.get(name) {
            return Err(SymbolError::AlreadyDefined {
                name: name.to_string(),
                first_defined: existing.defined_at,
                second_defined: span,
            });
        }
        
        self.symbols.insert(name.to_string(), SymbolInfo {
            id_type,
            defined_at: span,
            defined_by: verb_name,
            resolved_id: None,
        });
        
        Ok(())
    }
    
    pub fn get(&self, name: &str) -> Option<&SymbolInfo> {
        self.symbols.get(name)
    }
    
    pub fn resolve(&mut self, name: &str, id: Uuid) {
        if let Some(info) = self.symbols.get_mut(name) {
            info.resolved_id = Some(id);
        }
    }
    
    pub fn all_names(&self) -> Vec<&str> {
        self.symbols.keys().map(|s| s.as_str()).collect()
    }
}

#[derive(Debug)]
pub enum SymbolError {
    AlreadyDefined {
        name: String,
        first_defined: Span,
        second_defined: Span,
    },
}
```

---

## Part 5: Schema Validator

### 5.1 Validator Implementation

```rust
// rust/src/dsl_runtime/validator.rs

use std::sync::Arc;

pub struct SchemaValidator {
    schema_cache: Arc<SchemaCache>,
}

impl SchemaValidator {
    pub fn new(schema_cache: Arc<SchemaCache>) -> Self {
        Self { schema_cache }
    }
    
    /// Validate raw AST against verb schemas
    pub fn validate(
        &self,
        raw: &RawAst,
        env: &RuntimeEnv,
    ) -> Result<ValidatedAst, ValidationReport> {
        let mut validated_exprs = Vec::new();
        let mut symbol_table = SymbolTable::new();
        let mut errors = Vec::new();
        
        for expr in &raw.expressions {
            match &expr.kind {
                RawExprKind::Call { name, name_span, args } => {
                    // 1. Lookup verb schema
                    let verb = match VERB_REGISTRY.get(name) {
                        Some(v) => v,
                        None => {
                            errors.push(ValidationError {
                                span: *name_span,
                                kind: ErrorKind::UnknownVerb {
                                    name: name.clone(),
                                    suggestions: VERB_REGISTRY.suggest(name),
                                },
                            });
                            continue;
                        }
                    };
                    
                    // 2. Validate arguments
                    match self.validate_args(verb, args, env, &symbol_table) {
                        Ok(validated_args) => {
                            // 3. Check for symbol definition (:as @name)
                            let defines_symbol = validated_args.args
                                .get(":as")
                                .and_then(|v| match v {
                                    TypedValue::Symbol { name, .. } => Some(name.clone()),
                                    _ => None,
                                });
                            
                            // 4. Update symbol table
                            if let Some(ref sym_name) = defines_symbol {
                                if let Some(produces) = &verb.produces {
                                    if let Err(e) = symbol_table.define(
                                        sym_name,
                                        produces.capture_as,
                                        expr.span,
                                        verb.name,
                                    ) {
                                        errors.push(ValidationError {
                                            span: expr.span,
                                            kind: ErrorKind::SymbolError(e),
                                        });
                                    }
                                }
                            }
                            
                            validated_exprs.push(ValidatedExpr {
                                span: expr.span,
                                kind: ValidatedExprKind::VerbCall {
                                    verb,
                                    args: validated_args.args,
                                    context_injected: validated_args.context_injected,
                                    defaulted: validated_args.defaulted,
                                    defines_symbol,
                                },
                            });
                        }
                        Err(arg_errors) => {
                            errors.extend(arg_errors);
                        }
                    }
                }
                
                RawExprKind::Comment(c) => {
                    validated_exprs.push(ValidatedExpr {
                        span: expr.span,
                        kind: ValidatedExprKind::Comment(c.clone()),
                    });
                }
            }
        }
        
        if errors.is_empty() {
            Ok(ValidatedAst {
                expressions: validated_exprs,
                symbol_table,
            })
        } else {
            Err(ValidationReport { errors })
        }
    }
    
    fn validate_args(
        &self,
        verb: &'static VerbDef,
        args: &[RawArg],
        env: &RuntimeEnv,
        symbols: &SymbolTable,
    ) -> Result<ValidatedArgs, Vec<ValidationError>> {
        let mut typed = HashMap::new();
        let mut context_injected = Vec::new();
        let mut defaulted = Vec::new();
        let mut errors = Vec::new();
        
        // Build map of provided args
        let provided: HashMap<_, _> = args.iter()
            .map(|a| (a.key.as_str(), a))
            .collect();
        
        // Check each spec
        for spec in verb.args {
            match provided.get(spec.name) {
                // Argument was provided
                Some(arg) => {
                    match self.validate_value(&arg.value, &spec.sem_type, symbols) {
                        Ok(typed_val) => {
                            // Check validation rules
                            for rule in spec.validation {
                                if let Err(msg) = self.check_rule(&typed_val, rule) {
                                    errors.push(ValidationError {
                                        span: arg.value_span,
                                        kind: ErrorKind::ValidationFailed {
                                            arg: spec.name,
                                            rule: format!("{:?}", rule),
                                            message: msg,
                                        },
                                    });
                                }
                            }
                            typed.insert(spec.name.to_string(), typed_val);
                        }
                        Err(msg) => {
                            errors.push(ValidationError {
                                span: arg.value_span,
                                kind: ErrorKind::TypeMismatch {
                                    arg: spec.name,
                                    expected: format!("{:?}", spec.sem_type),
                                    got: msg,
                                },
                            });
                        }
                    }
                }
                
                // Argument not provided
                None => {
                    // Check if required
                    let is_required = match &spec.required {
                        RequiredRule::Always => true,
                        RequiredRule::Never => false,
                        RequiredRule::UnlessProvided(other) => !provided.contains_key(other),
                        RequiredRule::IfEquals { arg, value } => {
                            typed.get(*arg)
                                .and_then(|v| v.as_str())
                                .map(|s| s == *value)
                                .unwrap_or(false)
                        }
                        RequiredRule::IfProvided(other) => provided.contains_key(other),
                    };
                    
                    if is_required {
                        // Try context injection
                        if let Some(DefaultValue::FromContext(key)) = &spec.default {
                            if let Some(ctx_val) = env.get_context_typed(key) {
                                typed.insert(spec.name.to_string(), ctx_val);
                                context_injected.push(spec.name.to_string());
                                continue;
                            }
                        }
                        
                        // Try static default
                        if let Some(default) = &spec.default {
                            if let Some(val) = default.to_typed_value() {
                                typed.insert(spec.name.to_string(), val);
                                defaulted.push(spec.name.to_string());
                                continue;
                            }
                        }
                        
                        // Required but not provided
                        errors.push(ValidationError {
                            span: Span::default(),
                            kind: ErrorKind::MissingRequired {
                                arg: spec.name,
                                verb: verb.name,
                                required_because: self.explain_required_rule(&spec.required, &typed),
                            },
                        });
                    } else {
                        // Optional - apply default if available
                        if let Some(default) = &spec.default {
                            if let Some(val) = default.to_typed_value() {
                                typed.insert(spec.name.to_string(), val);
                                defaulted.push(spec.name.to_string());
                            }
                        }
                    }
                }
            }
        }
        
        // Check cross-constraints
        for constraint in verb.constraints {
            if let Err(e) = self.check_constraint(constraint, &typed, &provided) {
                errors.push(e);
            }
        }
        
        // Check for unknown args
        for arg in args {
            if !verb.args.iter().any(|s| s.name == arg.key) && arg.key != ":as" {
                errors.push(ValidationError {
                    span: arg.key_span,
                    kind: ErrorKind::UnknownArg {
                        arg: arg.key.clone(),
                        verb: verb.name,
                        suggestions: self.suggest_arg(verb, &arg.key),
                    },
                });
            }
        }
        
        if errors.is_empty() {
            Ok(ValidatedArgs {
                args: typed,
                context_injected,
                defaulted,
            })
        } else {
            Err(errors)
        }
    }
    
    fn validate_value(
        &self,
        raw: &RawValue,
        sem_type: &SemType,
        symbols: &SymbolTable,
    ) -> Result<TypedValue, String> {
        match (sem_type, raw) {
            (SemType::String, RawValue::String(s)) => Ok(TypedValue::String(s.clone())),
            
            (SemType::Uuid, RawValue::String(s)) => {
                Uuid::parse_str(s)
                    .map(TypedValue::Uuid)
                    .map_err(|_| format!("invalid UUID: '{}'", s))
            }
            
            (SemType::Integer, RawValue::Int(i)) => Ok(TypedValue::Integer(*i)),
            
            (SemType::Decimal, RawValue::Float(f)) => Ok(TypedValue::Decimal(*f)),
            (SemType::Decimal, RawValue::Int(i)) => Ok(TypedValue::Decimal(*i as f64)),
            
            (SemType::Date, RawValue::String(s)) => {
                NaiveDate::parse_from_str(s, "%Y-%m-%d")
                    .map(TypedValue::Date)
                    .map_err(|_| format!("invalid date (expected YYYY-MM-DD): '{}'", s))
            }
            
            (SemType::Boolean, RawValue::Bool(b)) => Ok(TypedValue::Boolean(*b)),
            
            (SemType::Ref(ref_type), RawValue::String(code)) => {
                if self.schema_cache.exists(ref_type, code) {
                    Ok(TypedValue::Ref {
                        ref_type: *ref_type,
                        code: code.clone(),
                    })
                } else {
                    let suggestions = self.schema_cache.suggest(ref_type, code);
                    Err(format!(
                        "unknown {}: '{}'. {}",
                        ref_type.name(),
                        code,
                        if suggestions.is_empty() {
                            String::new()
                        } else {
                            format!("Did you mean: {}?", suggestions.join(", "))
                        }
                    ))
                }
            }
            
            (SemType::Enum(values), RawValue::String(s)) => {
                if values.contains(&s.as_str()) {
                    Ok(TypedValue::Enum(s.clone()))
                } else {
                    Err(format!("must be one of: {:?}", values))
                }
            }
            
            (SemType::Symbol, RawValue::Symbol(name)) => {
                // Check if symbol is defined (for forward references)
                let resolved_id = symbols.get(name).and_then(|s| s.resolved_id);
                Ok(TypedValue::Symbol {
                    name: name.clone(),
                    resolved_id,
                })
            }
            
            (SemType::List(inner), RawValue::List(items)) => {
                let typed_items: Result<Vec<_>, _> = items
                    .iter()
                    .map(|item| self.validate_value(item, inner, symbols))
                    .collect();
                typed_items.map(TypedValue::List)
            }
            
            (SemType::Map(specs), RawValue::Map(pairs)) => {
                // Validate nested structure
                let mut typed_map = HashMap::new();
                for (key, value) in pairs {
                    if let Some(spec) = specs.iter().find(|s| s.name == format!(":{}", key)) {
                        match self.validate_value(value, &spec.sem_type, symbols) {
                            Ok(typed_val) => { typed_map.insert(key.clone(), typed_val); }
                            Err(e) => return Err(e),
                        }
                    }
                }
                Ok(TypedValue::Map(typed_map))
            }
            
            _ => Err(format!("expected {:?}, got {:?}", sem_type, raw)),
        }
    }
    
    fn check_rule(&self, value: &TypedValue, rule: &ValidationRule) -> Result<(), String> {
        match rule {
            ValidationRule::LookupMustExist => {
                // Already validated in validate_value for Ref types
                Ok(())
            }
            
            ValidationRule::Pattern { regex, description } => {
                if let TypedValue::String(s) = value {
                    let re = regex::Regex::new(regex).unwrap();
                    if re.is_match(s) {
                        Ok(())
                    } else {
                        Err(format!("must match pattern: {}", description))
                    }
                } else {
                    Ok(())
                }
            }
            
            ValidationRule::Range { min, max } => {
                let num = match value {
                    TypedValue::Integer(i) => *i as f64,
                    TypedValue::Decimal(d) => *d,
                    _ => return Ok(()),
                };
                
                if let Some(min) = min {
                    if num < *min {
                        return Err(format!("must be >= {}", min));
                    }
                }
                if let Some(max) = max {
                    if num > *max {
                        return Err(format!("must be <= {}", max));
                    }
                }
                Ok(())
            }
            
            ValidationRule::Length { min, max } => {
                if let TypedValue::String(s) = value {
                    if let Some(min) = min {
                        if s.len() < *min {
                            return Err(format!("must be at least {} characters", min));
                        }
                    }
                    if let Some(max) = max {
                        if s.len() > *max {
                            return Err(format!("must be at most {} characters", max));
                        }
                    }
                }
                Ok(())
            }
            
            ValidationRule::DateRange { min, max } => {
                if let TypedValue::Date(d) = value {
                    let today = chrono::Local::now().date_naive();
                    
                    if let Some(min_bound) = min {
                        let min_date = match min_bound {
                            DateBound::Literal(s) => NaiveDate::parse_from_str(s, "%Y-%m-%d").ok(),
                            DateBound::Today => Some(today),
                            DateBound::DaysFromToday(n) => Some(today + chrono::Duration::days(*n as i64)),
                        };
                        if let Some(min_date) = min_date {
                            if *d < min_date {
                                return Err(format!("date must be on or after {}", min_date));
                            }
                        }
                    }
                    
                    if let Some(max_bound) = max {
                        let max_date = match max_bound {
                            DateBound::Literal(s) => NaiveDate::parse_from_str(s, "%Y-%m-%d").ok(),
                            DateBound::Today => Some(today),
                            DateBound::DaysFromToday(n) => Some(today + chrono::Duration::days(*n as i64)),
                        };
                        if let Some(max_date) = max_date {
                            if *d > max_date {
                                return Err(format!("date must be on or before {}", max_date));
                            }
                        }
                    }
                }
                Ok(())
            }
            
            ValidationRule::NotEmpty => {
                if let TypedValue::String(s) = value {
                    if s.trim().is_empty() {
                        return Err("cannot be empty".to_string());
                    }
                }
                Ok(())
            }
            
            ValidationRule::Custom(_) => {
                // Custom validation handled elsewhere
                Ok(())
            }
        }
    }
    
    fn check_constraint(
        &self,
        constraint: &CrossConstraint,
        typed: &HashMap<String, TypedValue>,
        provided: &HashMap<&str, &RawArg>,
    ) -> Result<(), ValidationError> {
        match constraint {
            CrossConstraint::ExactlyOne(args) => {
                let count = args.iter().filter(|a| provided.contains_key(*a)).count();
                if count != 1 {
                    return Err(ValidationError {
                        span: Span::default(),
                        kind: ErrorKind::ConstraintViolation {
                            constraint: format!("exactly one of {:?} must be provided", args),
                        },
                    });
                }
            }
            
            CrossConstraint::AtLeastOne(args) => {
                let has_any = args.iter().any(|a| provided.contains_key(*a) || typed.contains_key(*a));
                if !has_any {
                    return Err(ValidationError {
                        span: Span::default(),
                        kind: ErrorKind::ConstraintViolation {
                            constraint: format!("at least one of {:?} must be provided", args),
                        },
                    });
                }
            }
            
            CrossConstraint::Requires { if_present, then_require } => {
                if provided.contains_key(if_present) && !typed.contains_key(*then_require) {
                    return Err(ValidationError {
                        span: Span::default(),
                        kind: ErrorKind::ConstraintViolation {
                            constraint: format!("'{}' requires '{}'", if_present, then_require),
                        },
                    });
                }
            }
            
            CrossConstraint::Excludes { if_present, then_forbid } => {
                if provided.contains_key(if_present) && provided.contains_key(then_forbid) {
                    return Err(ValidationError {
                        span: Span::default(),
                        kind: ErrorKind::ConstraintViolation {
                            constraint: format!("'{}' and '{}' cannot both be provided", if_present, then_forbid),
                        },
                    });
                }
            }
            
            CrossConstraint::ConditionalRequired { if_arg, equals, then_require } => {
                if let Some(val) = typed.get(*if_arg) {
                    if val.as_str() == Some(*equals) && !typed.contains_key(*then_require) {
                        return Err(ValidationError {
                            span: Span::default(),
                            kind: ErrorKind::ConstraintViolation {
                                constraint: format!("'{}' required when {} = '{}'", then_require, if_arg, equals),
                            },
                        });
                    }
                }
            }
            
            CrossConstraint::LessThan { lesser, greater } => {
                // Implement comparison for dates/numbers
            }
        }
        
        Ok(())
    }
    
    fn suggest_arg(&self, verb: &VerbDef, typo: &str) -> Vec<String> {
        verb.args
            .iter()
            .map(|a| a.name)
            .filter(|a| levenshtein_distance(a, typo) <= 3)
            .map(String::from)
            .collect()
    }
}

struct ValidatedArgs {
    args: HashMap<String, TypedValue>,
    context_injected: Vec<String>,
    defaulted: Vec<String>,
}
```

---

## Part 6: Error Reporting

### 6.1 Error Types

```rust
// rust/src/dsl_runtime/errors.rs

#[derive(Debug)]
pub struct ValidationReport {
    pub errors: Vec<ValidationError>,
}

#[derive(Debug)]
pub struct ValidationError {
    pub span: Span,
    pub kind: ErrorKind,
}

#[derive(Debug)]
pub enum ErrorKind {
    // Verb errors
    UnknownVerb {
        name: String,
        suggestions: Vec<&'static str>,
    },
    
    // Argument errors
    UnknownArg {
        arg: String,
        verb: &'static str,
        suggestions: Vec<String>,
    },
    MissingRequired {
        arg: &'static str,
        verb: &'static str,
        required_because: String,
    },
    
    // Type errors
    TypeMismatch {
        arg: &'static str,
        expected: String,
        got: String,
    },
    
    // Validation errors
    ValidationFailed {
        arg: &'static str,
        rule: String,
        message: String,
    },
    
    // Constraint errors
    ConstraintViolation {
        constraint: String,
    },
    
    // Symbol errors
    UndefinedSymbol {
        name: String,
        defined_symbols: Vec<String>,
    },
    SymbolError(SymbolError),
}

impl ErrorKind {
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnknownVerb { .. } => "E001",
            Self::UnknownArg { .. } => "E002",
            Self::MissingRequired { .. } => "E003",
            Self::TypeMismatch { .. } => "E004",
            Self::ValidationFailed { .. } => "E005",
            Self::ConstraintViolation { .. } => "E006",
            Self::UndefinedSymbol { .. } => "E007",
            Self::SymbolError(_) => "E008",
        }
    }
    
    pub fn message(&self) -> String {
        match self {
            Self::UnknownVerb { name, .. } => 
                format!("unknown verb '{}'", name),
            Self::UnknownArg { arg, verb, .. } => 
                format!("unknown argument '{}' for verb '{}'", arg, verb),
            Self::MissingRequired { arg, verb, required_because } => 
                format!("missing required argument '{}' for '{}' ({})", arg, verb, required_because),
            Self::TypeMismatch { arg, expected, got } => 
                format!("'{}': expected {}, got {}", arg, expected, got),
            Self::ValidationFailed { arg, message, .. } => 
                format!("'{}': {}", arg, message),
            Self::ConstraintViolation { constraint } => 
                format!("constraint violated: {}", constraint),
            Self::UndefinedSymbol { name, .. } => 
                format!("undefined symbol '@{}'", name),
            Self::SymbolError(e) => format!("{:?}", e),
        }
    }
    
    pub fn hint(&self) -> Option<String> {
        match self {
            Self::UnknownVerb { suggestions, .. } if !suggestions.is_empty() => 
                Some(format!("did you mean: {}?", suggestions.join(", "))),
            Self::UnknownArg { suggestions, .. } if !suggestions.is_empty() => 
                Some(format!("did you mean: {}?", suggestions.join(", "))),
            Self::UndefinedSymbol { defined_symbols, .. } if !defined_symbols.is_empty() => 
                Some(format!("defined symbols: {}", defined_symbols.join(", "))),
            _ => None,
        }
    }
}
```

### 6.2 Pretty Printing

```rust
impl ValidationReport {
    pub fn format(&self, source: &str, filename: &str) -> String {
        let mut out = String::new();
        
        for err in &self.errors {
            let (line, col) = span_to_line_col(source, err.span);
            let line_text = get_source_line(source, line);
            
            // Error header
            out += &format!(
                "\x1b[1;31merror[{}]\x1b[0m: {}\n",
                err.kind.code(),
                err.kind.message()
            );
            
            // Location
            out += &format!(
                "  \x1b[1;34m-->\x1b[0m {}:{}:{}\n",
                filename,
                line + 1,
                col + 1
            );
            
            // Source context
            out += &format!("   \x1b[1;34m|\x1b[0m\n");
            out += &format!(
                "\x1b[1;34m{:3}|\x1b[0m {}\n",
                line + 1,
                line_text
            );
            out += &format!(
                "   \x1b[1;34m|\x1b[0m {}\x1b[1;31m^\x1b[0m\n",
                " ".repeat(col)
            );
            
            // Hint
            if let Some(hint) = err.kind.hint() {
                out += &format!(
                    "   \x1b[1;34m= \x1b[0m\x1b[1mhint\x1b[0m: {}\n",
                    hint
                );
            }
            
            out += "\n";
        }
        
        // Summary
        out += &format!(
            "\x1b[1;31merror\x1b[0m: aborting due to {} previous error{}\n",
            self.errors.len(),
            if self.errors.len() == 1 { "" } else { "s" }
        );
        
        out
    }
}
```

### 6.3 Example Output

```
error[E008]: unknown role: 'Investmanager'
  --> kyc_session.dsl:12:45
   |
12 | (cbu.attach-entity :entity-id @company :role "Investmanager")
   |                                              ^
   = hint: did you mean: InvestmentManager?

error[E003]: missing required argument ':ownership-percent' for 'cbu.attach-entity' (required when :role = "BeneficialOwner")
  --> kyc_session.dsl:18:1
   |
18 | (cbu.attach-entity :entity-id @person :role "BeneficialOwner")
   | ^
   = hint: add :ownership-percent with the ownership percentage (0.0-100.0)

error[E007]: undefined symbol '@companyx'
  --> kyc_session.dsl:25:27
   |
25 | (cbu.attach-entity :entity-id @companyx :role "Custodian")
   |                               ^
   = hint: defined symbols: @cbu, @company, @person

error: aborting due to 3 previous errors
```

---

## Part 7: LLM Context Export

### 7.1 Export for RAG/Prompt Context

```rust
impl VerbDef {
    /// Export verb definition for LLM context
    pub fn to_llm_context(&self) -> String {
        let mut out = format!("## {}\n\n", self.name);
        out += &format!("{}\n\n", self.description);
        
        out += "### Arguments\n\n";
        for arg in self.args {
            let req = match &arg.required {
                RequiredRule::Always => "**required**",
                RequiredRule::Never => "optional",
                RequiredRule::UnlessProvided(other) => &format!("required unless `{}` provided", other),
                RequiredRule::IfEquals { arg, value } => &format!("required if `{} = \"{}\"`", arg, value),
                RequiredRule::IfProvided(other) => &format!("required if `{}` provided", other),
            };
            
            let type_str = match &arg.sem_type {
                SemType::String => "STRING",
                SemType::Uuid => "UUID",
                SemType::Integer => "INTEGER",
                SemType::Decimal => "DECIMAL",
                SemType::Date => "DATE (YYYY-MM-DD)",
                SemType::Boolean => "BOOLEAN",
                SemType::Ref(r) => &format!("{}_REF", r.name().to_uppercase().replace(" ", "_")),
                SemType::Enum(values) => &format!("one of {:?}", values),
                SemType::Symbol => "SYMBOL (@name)",
                _ => "COMPLEX",
            };
            
            out += &format!("- `{}` ({}) [{}]\n", arg.name, type_str, req);
            out += &format!("  - {}\n", arg.description);
            
            if let Some(default) = &arg.default {
                out += &format!("  - Default: {:?}\n", default);
            }
        }
        
        if !self.constraints.is_empty() {
            out += "\n### Constraints\n\n";
            for c in self.constraints {
                out += &format!("- {:?}\n", c);
            }
        }
        
        out += "\n### Examples\n\n```clojure\n";
        for ex in self.examples {
            out += &format!("{}\n", ex);
        }
        out += "```\n";
        
        out
    }
}

impl VerbRegistry {
    /// Export all verbs for LLM context
    pub fn to_llm_context(&self) -> String {
        let mut out = String::from("# DSL Verb Reference\n\n");
        
        for domain in self.domains() {
            out += &format!("# {} Domain\n\n", domain.to_uppercase());
            
            for verb in self.get_by_domain(domain) {
                out += &verb.to_llm_context();
                out += "\n---\n\n";
            }
        }
        
        out
    }
}
```

---

## Part 8: Directory Structure

```
rust/src/dsl_runtime/
├── mod.rs
├── schema/
│   ├── mod.rs
│   ├── types.rs              # SemType, ArgSpec, RequiredRule, etc.
│   ├── registry.rs           # VerbRegistry, VERB_REGISTRY
│   └── verbs/
│       ├── mod.rs
│       ├── cbu.rs            # CBU_ENSURE, CBU_ATTACH_ENTITY, etc.
│       ├── entity.rs         # ENTITY_CREATE_*, etc.
│       ├── document.rs       # DOCUMENT_REQUEST, etc.
│       ├── kyc.rs            # INVESTIGATION_*, RISK_*, etc.
│       ├── screening.rs      # SCREENING_*, etc.
│       ├── decision.rs       # DECISION_*, etc.
│       └── monitoring.rs     # MONITORING_*, etc.
├── ast/
│   ├── mod.rs
│   ├── raw.rs                # RawAst, RawExpr, RawValue
│   ├── validated.rs          # ValidatedAst, TypedValue
│   └── symbols.rs            # SymbolTable, SymbolInfo
├── validator.rs              # SchemaValidator
├── errors.rs                 # ValidationReport, ErrorKind
├── parser.rs                 # Nom parser (existing, updated for spans)
├── vocabulary.rs             # Runtime word lookup
└── words/                    # Implementation functions
    ├── mod.rs
    └── ...
```

---

## Part 9: Implementation Phases

### Phase 1: Schema Foundation
- [ ] Define `SemType`, `ArgSpec`, `VerbDef` structs
- [ ] Create verb definitions for core verbs (CBU, Entity)
- [ ] Build `VerbRegistry`

### Phase 2: AST Updates
- [ ] Add `Span` tracking to parser
- [ ] Define `RawAst` with spans
- [ ] Define `ValidatedAst` and `TypedValue`
- [ ] Implement `SymbolTable`

### Phase 3: Schema Validator
- [ ] Implement `SchemaValidator`
- [ ] Type checking for all `SemType` variants
- [ ] Validation rules (`Range`, `Pattern`, etc.)
- [ ] Cross-constraint checking
- [ ] Context injection

### Phase 4: Error Reporting
- [ ] Rich error types with suggestions
- [ ] Pretty-printed output with line numbers
- [ ] LSP diagnostic conversion

### Phase 5: Integration
- [ ] Wire validator into pipeline (after parse, before execute)
- [ ] Update executor to use `ValidatedAst`
- [ ] Remove redundant validation from `CrudExecutor`

### Phase 6: Complete Verb Schemas
- [ ] Document domain verbs
- [ ] KYC domain verbs
- [ ] Screening domain verbs
- [ ] Decision domain verbs
- [ ] Monitoring domain verbs

### Phase 7: Tooling
- [ ] LLM context export
- [ ] Documentation generation
- [ ] LSP integration

---

## Summary

| Component | Purpose |
|-----------|---------|
| **VerbDef** | Declarative specification of verb name, args, constraints |
| **ArgSpec** | Name, type, required rule, default, validation |
| **SemType** | Semantic types including Ref (lookup) and Enum |
| **SchemaValidator** | Validates RawAst against schemas |
| **ValidatedAst** | Typed AST after validation |
| **SymbolTable** | Tracks @symbol definitions and references |
| **ValidationReport** | Rich errors with line numbers and suggestions |

**This makes the DSL production-ready:**
- ✅ Parse-time validation
- ✅ Line/column error reporting
- ✅ Typo suggestions
- ✅ Required/optional enforcement
- ✅ Lookup table validation
- ✅ LLM-friendly schema export
- ✅ LSP diagnostic integration


---

## Part 10: Parser-Schema Integration (EBNF Binding)

This section covers how the nom parser connects to verb schemas for early validation.

### 10.1 Current EBNF (Syntax Only)

```ebnf
(* Current: Parser only checks structure, not semantics *)
program     = { expression } ;
expression  = "(" call ")" | comment ;
call        = verb-name { arg } ;
verb-name   = identifier { "." identifier } ;
arg         = keyword value ;
keyword     = ":" identifier ;
value       = string | number | symbol | list | map ;
symbol      = "@" identifier ;
```

**Problem:** Parser accepts any keyword for any verb. Invalid args discovered at runtime.

### 10.2 Extended EBNF (Schema-Aware)

```ebnf
(* Extended: Parser validates against VerbDef during parse *)
program     = { expression } ;
expression  = "(" call ")" | comment ;

(* Verb call with schema-driven arg validation *)
call        = verb-name { typed-arg } ;
verb-name   = VERB_REGISTRY.lookup(identifier { "." identifier }) ;

(* Each arg validated against ArgSpec from VerbDef *)
typed-arg   = keyword typed-value ;
keyword     = ":" identifier ;  (* Must match ArgSpec.name *)

(* Value type determined by ArgSpec.sem_type *)
typed-value = string-value      (* SemType::String *)
            | uuid-value        (* SemType::Uuid *)
            | integer-value     (* SemType::Integer *)
            | decimal-value     (* SemType::Decimal *)
            | date-value        (* SemType::Date *)
            | boolean-value     (* SemType::Boolean *)
            | ref-value         (* SemType::Ref(RefType) *)
            | enum-value        (* SemType::Enum([...]) *)
            | symbol-value      (* SemType::Symbol *)
            | list-value        (* SemType::List(inner) *)
            | map-value         (* SemType::Map(specs) *)
            ;

(* Reference types - validated against SchemaCache lookup tables *)
ref-value   = SCHEMA_CACHE.validate(RefType, string) ;
             (* DocumentType → document_types.type_code *)
             (* Attribute    → attribute_dictionary.attr_id *)
             (* Role         → roles.name *)
             (* EntityType   → entity_types.type_code *)
             (* Jurisdiction → jurisdictions.iso_code *)
```

### 10.3 Parser Implementation (Nom + Schema)

```rust
// rust/src/dsl_runtime/parser.rs

use nom::{IResult, combinator::*, sequence::*, multi::*, branch::*};
use crate::schema::{VERB_REGISTRY, VerbDef, ArgSpec, SemType};

/// Parse result with source span
#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub value: T,
    pub span: Span,
}

/// Parse a complete DSL program
pub fn parse_program(input: &str) -> IResult<&str, RawAst> {
    let (remaining, expressions) = many0(parse_expression)(input)?;
    Ok((remaining, RawAst { expressions }))
}

/// Parse a verb call - NOW WITH SCHEMA LOOKUP
pub fn parse_call(input: &str) -> IResult<&str, RawExpr> {
    let start = input.as_ptr() as usize;
    
    // Parse verb name
    let (input, _) = char('(')(input)?;
    let (input, _) = multispace0(input)?;
    let (input, verb_name) = parse_verb_name(input)?;
    
    // *** SCHEMA LOOKUP AT PARSE TIME ***
    let verb_def: Option<&'static VerbDef> = VERB_REGISTRY.get(&verb_name.value);
    
    // Parse arguments (schema-aware if verb found)
    let (input, args) = parse_args(input, verb_def)?;
    
    let (input, _) = multispace0(input)?;
    let (input, _) = char(')')(input)?;
    
    let end = input.as_ptr() as usize;
    let span = Span::from_offsets(start, end);
    
    Ok((input, RawExpr {
        span,
        kind: RawExprKind::Call {
            name: verb_name.value,
            name_span: verb_name.span,
            args,
            verb_def,  // ← Attached for later validation
        },
    }))
}

/// Parse args with optional schema validation
fn parse_args(
    input: &str, 
    verb_def: Option<&'static VerbDef>
) -> IResult<&str, Vec<RawArg>> {
    let mut args = Vec::new();
    let mut remaining = input;
    
    loop {
        let (input, _) = multispace0(remaining)?;
        
        // Try to parse keyword
        match parse_keyword(input) {
            Ok((input, keyword)) => {
                // If we have a verb schema, validate keyword exists
                let arg_spec: Option<&ArgSpec> = verb_def.and_then(|v| {
                    v.args.iter().find(|a| a.name == keyword.value)
                });
                
                // Parse value (schema-aware for type checking)
                let (input, _) = multispace0(input)?;
                let (input, value) = parse_value(input, arg_spec)?;
                
                args.push(RawArg {
                    span: Span::merge(&keyword.span, &value.span),
                    key: keyword.value,
                    key_span: keyword.span,
                    value: value.value,
                    value_span: value.span,
                    arg_spec,  // ← Attached for validation
                });
                
                remaining = input;
            }
            Err(_) => break,
        }
    }
    
    Ok((remaining, args))
}

/// Parse value with optional type hint from ArgSpec
fn parse_value(
    input: &str,
    arg_spec: Option<&ArgSpec>,
) -> IResult<&str, Spanned<RawValue>> {
    // Use sem_type hint if available for better parsing
    match arg_spec.map(|s| &s.sem_type) {
        Some(SemType::Date) => {
            // Parse as date string with format validation
            parse_date_string(input)
        }
        Some(SemType::Uuid) => {
            // Parse as UUID string with format validation  
            parse_uuid_string(input)
        }
        Some(SemType::Symbol) => {
            // Expect @symbol
            parse_symbol(input)
        }
        Some(SemType::Ref(ref_type)) => {
            // Parse as string, will validate against SchemaCache later
            parse_ref_string(input, *ref_type)
        }
        Some(SemType::Enum(values)) => {
            // Parse as string, check against allowed values
            parse_enum_string(input, values)
        }
        _ => {
            // Generic value parsing
            alt((
                parse_string,
                parse_number,
                parse_symbol,
                parse_list,
                parse_map,
            ))(input)
        }
    }
}
```

### 10.4 Raw AST with Schema Annotations

```rust
// rust/src/dsl_runtime/ast/raw.rs

/// Raw expression from parser (includes schema references)
#[derive(Debug, Clone)]
pub enum RawExprKind {
    Call {
        name: String,
        name_span: Span,
        args: Vec<RawArg>,
        /// Schema attached at parse time (None if unknown verb)
        verb_def: Option<&'static VerbDef>,
    },
    Comment(String),
}

/// Raw argument with optional schema annotation
#[derive(Debug, Clone)]
pub struct RawArg {
    pub span: Span,
    pub key: String,
    pub key_span: Span,
    pub value: RawValue,
    pub value_span: Span,
    /// ArgSpec attached at parse time (None if unknown arg)
    pub arg_spec: Option<&'static ArgSpec>,
}
```

### 10.5 Binding Resolution Phases

```
┌─────────────────────────────────────────────────────────────────┐
│                    BINDING RESOLUTION PHASES                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  PHASE 1: LEXICAL (Parser)                                      │
│  ─────────────────────────                                       │
│  • Tokenize source                                               │
│  • Build parse tree                                              │
│  • Attach source spans                                           │
│  • Lookup VerbDef by name → attach to AST                       │
│  • Lookup ArgSpec by keyword → attach to AST                    │
│                                                                  │
│  Bound: verb-name → VerbDef, keyword → ArgSpec                  │
│  Errors: Unknown verb, unknown argument (with suggestions)      │
│                                                                  │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  PHASE 2: SEMANTIC VALIDATION (SchemaValidator)                 │
│  ───────────────────────────────────────────────                 │
│  For each Call with VerbDef:                                    │
│                                                                  │
│  2a. TYPE CHECKING                                               │
│      • String → SemType::String ✓                               │
│      • "2024-01-15" → SemType::Date (parse & validate format)   │
│      • "550e8400..." → SemType::Uuid (parse & validate format)  │
│      • 25.5 → SemType::Decimal ✓                                │
│      • @symbol → SemType::Symbol ✓                              │
│                                                                  │
│  2b. REFERENCE VALIDATION (RefTypes against SchemaCache)        │
│      • :document-type "CERT_OF_INCORP"                          │
│        → SchemaCache.document_types.contains("CERT_OF_INCORP")  │
│        → ✓ Valid | ✗ Error + suggestions                        │
│                                                                  │
│      • :role "InvestmentManager"                                 │
│        → SchemaCache.roles.contains("InvestmentManager")        │
│        → ✓ Valid | ✗ Error + suggestions                        │
│                                                                  │
│      • :attribute "CBU.LEGAL_NAME"                               │
│        → SchemaCache.attributes.contains("CBU.LEGAL_NAME")      │
│        → ✓ Valid | ✗ Error + suggestions                        │
│                                                                  │
│  2c. REQUIRED/OPTIONAL CHECKING                                  │
│      • RequiredRule::Always → must be present                   │
│      • RequiredRule::IfEquals{:role, "BeneficialOwner"}         │
│        → :ownership-percent required if role is UBO             │
│      • RequiredRule::UnlessProvided(":entity-id")               │
│        → :cbu-id required unless entity-id provided             │
│                                                                  │
│  2d. CROSS-CONSTRAINT CHECKING                                   │
│      • CrossConstraint::AtLeastOne([":entity-id", ":cbu-id"])   │
│      • CrossConstraint::Excludes{if: ":a", then: ":b"}          │
│                                                                  │
│  2e. SYMBOL TABLE CONSTRUCTION                                   │
│      • (... :as @cbu) → SymbolTable.define("cbu", CbuId)        │
│      • @company → SymbolTable.exists("company")?                │
│                                                                  │
│  Bound: values → TypedValue, refs → validated codes             │
│  Errors: Type mismatch, unknown ref, missing required,          │
│          undefined symbol (all with line:column)                │
│                                                                  │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  PHASE 3: CONTEXT INJECTION (ValidatedAST construction)         │
│  ──────────────────────────────────────────────────              │
│  • DefaultValue::FromContext(ContextKey::CbuId)                 │
│    → Inject env.cbu_id if :cbu-id not provided                  │
│  • Track which args were injected vs explicit                   │
│                                                                  │
│  Bound: missing optional args → defaults/context values         │
│                                                                  │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  PHASE 4: EXECUTION (CrudExecutor)                              │
│  ─────────────────────────────────                               │
│  • @symbol → SymbolTable.resolve() → actual UUID                │
│  • RefType code → LookupService.resolve() → UUID                │
│    "CERT_OF_INCORP" → 550e8400-e29b-41d4-a716-446655440000     │
│  • Execute SQL with resolved UUIDs                              │
│                                                                  │
│  Bound: codes → UUIDs, symbols → UUIDs                          │
│  Errors: Only DB errors (constraints, FK violations)            │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 10.6 When Each ID Type Binds

| ID Type | Example | Phase | Validation | Resolution |
|---------|---------|-------|------------|------------|
| **Verb name** | `cbu.attach-entity` | 1 Lexical | VERB_REGISTRY.get() | → VerbDef |
| **Keyword** | `:role` | 1 Lexical | VerbDef.args.find() | → ArgSpec |
| **Document type** | `"CERT_OF_INCORP"` | 2 Semantic | SchemaCache.document_types | → code (UUID at Phase 4) |
| **Attribute ID** | `"CBU.LEGAL_NAME"` | 2 Semantic | SchemaCache.attributes | → attr_id (UUID at Phase 4) |
| **Role** | `"InvestmentManager"` | 2 Semantic | SchemaCache.roles | → code (UUID at Phase 4) |
| **Entity type** | `"LIMITED_COMPANY"` | 2 Semantic | SchemaCache.entity_types | → code |
| **Jurisdiction** | `"LU"` | 2 Semantic | SchemaCache.jurisdictions | → iso_code |
| **Symbol** | `@company` | 2 Semantic | SymbolTable.exists() | → UUID at Phase 4 |
| **Literal UUID** | `"550e8400..."` | 2 Semantic | Format validation only | → UUID |

---

## Part 11: LSP RefType Completions

This section covers IDE autocomplete for reference types (document-id, attribute-id, entity-id, etc.).

### 11.1 SchemaCache Structure

```rust
// rust/src/dsl_runtime/schema_cache.rs

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Cached lookup tables for validation and LSP completions
pub struct SchemaCache {
    /// Document types: type_code → DisplayInfo
    pub document_types: HashMap<String, LookupEntry>,
    
    /// Attributes: attr_id → DisplayInfo  
    pub attributes: HashMap<String, LookupEntry>,
    
    /// Roles: role_name → DisplayInfo
    pub roles: HashMap<String, LookupEntry>,
    
    /// Entity types: type_code → DisplayInfo
    pub entity_types: HashMap<String, LookupEntry>,
    
    /// Jurisdictions: iso_code → DisplayInfo
    pub jurisdictions: HashMap<String, LookupEntry>,
    
    /// Screening lists: list_code → DisplayInfo
    pub screening_lists: HashMap<String, LookupEntry>,
    
    /// Currencies: iso_code → DisplayInfo
    pub currencies: HashMap<String, LookupEntry>,
}

/// Entry for LSP completion display
#[derive(Debug, Clone)]
pub struct LookupEntry {
    /// Code to insert into DSL (e.g., "CERT_OF_INCORP")
    pub code: String,
    
    /// Human-readable display name (e.g., "Certificate of Incorporation")
    pub display_name: String,
    
    /// Category for grouping (e.g., "Corporate", "Personal")
    pub category: Option<String>,
    
    /// Description for hover/docs
    pub description: Option<String>,
    
    /// Related attributes (for document types)
    pub extractable_attributes: Option<Vec<String>>,
}

impl SchemaCache {
    /// Load from database
    pub async fn load(pool: &PgPool) -> Result<Self> {
        let document_types = Self::load_document_types(pool).await?;
        let attributes = Self::load_attributes(pool).await?;
        let roles = Self::load_roles(pool).await?;
        let entity_types = Self::load_entity_types(pool).await?;
        let jurisdictions = Self::load_jurisdictions(pool).await?;
        let screening_lists = Self::load_screening_lists(pool).await?;
        let currencies = Self::load_currencies(pool).await?;
        
        Ok(Self {
            document_types,
            attributes,
            roles,
            entity_types,
            jurisdictions,
            screening_lists,
            currencies,
        })
    }
    
    async fn load_document_types(pool: &PgPool) -> Result<HashMap<String, LookupEntry>> {
        let rows = sqlx::query!(r#"
            SELECT 
                dt.type_code,
                dt.type_name,
                dt.category,
                dt.description,
                COALESCE(
                    array_agg(DISTINCT ad.attr_name) FILTER (WHERE ad.attr_name IS NOT NULL),
                    '{}'
                ) as extractable_attributes
            FROM document_types dt
            LEFT JOIN document_type_attributes dta ON dt.id = dta.document_type_id
            LEFT JOIN attribute_dictionary ad ON dta.attribute_id = ad.id
            GROUP BY dt.id, dt.type_code, dt.type_name, dt.category, dt.description
        "#)
        .fetch_all(pool)
        .await?;
        
        Ok(rows.into_iter().map(|r| {
            (r.type_code.clone(), LookupEntry {
                code: r.type_code,
                display_name: r.type_name,
                category: r.category,
                description: r.description,
                extractable_attributes: Some(r.extractable_attributes),
            })
        }).collect())
    }
    
    /// Check if a code exists for a RefType
    pub fn exists(&self, ref_type: &RefType, code: &str) -> bool {
        match ref_type {
            RefType::DocumentType => self.document_types.contains_key(code),
            RefType::Attribute => self.attributes.contains_key(code),
            RefType::Role => self.roles.contains_key(code),
            RefType::EntityType => self.entity_types.contains_key(code),
            RefType::Jurisdiction => self.jurisdictions.contains_key(code),
            RefType::ScreeningList => self.screening_lists.contains_key(code),
            RefType::Currency => self.currencies.contains_key(code),
        }
    }
    
    /// Get suggestions for typo correction
    pub fn suggest(&self, ref_type: &RefType, typo: &str) -> Vec<String> {
        let entries = match ref_type {
            RefType::DocumentType => &self.document_types,
            RefType::Attribute => &self.attributes,
            RefType::Role => &self.roles,
            RefType::EntityType => &self.entity_types,
            RefType::Jurisdiction => &self.jurisdictions,
            RefType::ScreeningList => &self.screening_lists,
            RefType::Currency => &self.currencies,
        };
        
        let mut suggestions: Vec<_> = entries.keys()
            .filter(|k| {
                levenshtein_distance(k, typo) <= 3 
                    || k.to_lowercase().contains(&typo.to_lowercase())
            })
            .cloned()
            .collect();
        
        suggestions.sort_by_key(|k| levenshtein_distance(k, typo));
        suggestions.truncate(5);
        suggestions
    }
    
    /// Get all entries for LSP completion
    pub fn get_completions(&self, ref_type: &RefType) -> Vec<&LookupEntry> {
        match ref_type {
            RefType::DocumentType => self.document_types.values().collect(),
            RefType::Attribute => self.attributes.values().collect(),
            RefType::Role => self.roles.values().collect(),
            RefType::EntityType => self.entity_types.values().collect(),
            RefType::Jurisdiction => self.jurisdictions.values().collect(),
            RefType::ScreeningList => self.screening_lists.values().collect(),
            RefType::Currency => self.currencies.values().collect(),
        }
    }
    
    /// Filter completions by prefix (for incremental typing)
    pub fn get_filtered_completions(
        &self, 
        ref_type: &RefType, 
        prefix: &str
    ) -> Vec<&LookupEntry> {
        self.get_completions(ref_type)
            .into_iter()
            .filter(|e| {
                e.code.to_lowercase().starts_with(&prefix.to_lowercase())
                    || e.display_name.to_lowercase().contains(&prefix.to_lowercase())
            })
            .collect()
    }
}
```

### 11.2 LSP Completion Handler

```rust
// rust/src/lsp/completions.rs

use tower_lsp::lsp_types::*;

impl LanguageServer for DslLanguageServer {
    async fn completion(
        &self,
        params: CompletionParams,
    ) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        
        // Get document and parse context
        let doc = self.documents.get(&uri)?;
        let context = self.get_completion_context(&doc, position)?;
        
        let items = match context {
            // After opening paren - complete verb names
            CompletionContext::VerbName { prefix } => {
                self.complete_verbs(&prefix)
            }
            
            // After verb name - complete keywords
            CompletionContext::Keyword { verb_def, prefix } => {
                self.complete_keywords(verb_def, &prefix)
            }
            
            // After keyword with RefType - complete from SchemaCache
            CompletionContext::RefValue { ref_type, prefix } => {
                self.complete_ref_values(ref_type, &prefix).await
            }
            
            // After keyword with Enum - complete enum values
            CompletionContext::EnumValue { values, prefix } => {
                self.complete_enum_values(values, &prefix)
            }
            
            // After :as keyword - suggest symbol name
            CompletionContext::SymbolDef { prefix } => {
                self.suggest_symbol_name(&prefix)
            }
            
            // Symbol reference - complete from symbol table
            CompletionContext::SymbolRef { prefix } => {
                self.complete_symbols(&prefix)
            }
            
            _ => vec![],
        };
        
        Ok(Some(CompletionResponse::Array(items)))
    }
    
    /// Complete RefType values (document types, attributes, roles, etc.)
    async fn complete_ref_values(
        &self,
        ref_type: RefType,
        prefix: &str,
    ) -> Vec<CompletionItem> {
        let cache = self.schema_cache.read().await;
        
        cache.get_filtered_completions(&ref_type, prefix)
            .into_iter()
            .map(|entry| {
                CompletionItem {
                    label: entry.display_name.clone(),
                    kind: Some(CompletionItemKind::ENUM_MEMBER),
                    
                    // Show category and description
                    detail: entry.category.clone(),
                    documentation: entry.description.as_ref().map(|d| {
                        Documentation::MarkupContent(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: self.format_ref_docs(entry),
                        })
                    }),
                    
                    // Insert the CODE, not display name
                    insert_text: Some(format!("\"{}\"", entry.code)),
                    insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                    
                    // Filter by both code and display name
                    filter_text: Some(format!("{} {}", entry.code, entry.display_name)),
                    
                    // Sort by category then name
                    sort_text: Some(format!(
                        "{}-{}", 
                        entry.category.as_deref().unwrap_or("zzz"),
                        entry.display_name
                    )),
                    
                    ..Default::default()
                }
            })
            .collect()
    }
    
    /// Format documentation for RefType completion
    fn format_ref_docs(&self, entry: &LookupEntry) -> String {
        let mut doc = format!("**{}**\n\n", entry.display_name);
        doc += &format!("Code: `{}`\n\n", entry.code);
        
        if let Some(desc) = &entry.description {
            doc += &format!("{}\n\n", desc);
        }
        
        // For document types, show extractable attributes
        if let Some(attrs) = &entry.extractable_attributes {
            if !attrs.is_empty() {
                doc += "**Extractable Attributes:**\n";
                for attr in attrs {
                    doc += &format!("- `{}`\n", attr);
                }
            }
        }
        
        doc
    }
}
```

### 11.3 LSP Completion Examples

#### Document Type Completion

```
User types: (document.request :document-type "C|
                                              ↑ cursor

┌─────────────────────────────────────────────────────────────────┐
│ 📄 Certificate of Incorporation       [Corporate]              │
│    Code: CERT_OF_INCORP                                         │
│    Extractable: COMPANY_NAME, INCORP_DATE, JURISDICTION...     │
├─────────────────────────────────────────────────────────────────┤
│ 📄 Certificate of Good Standing       [Corporate]              │
│    Code: CERT_GOOD_STANDING                                     │
├─────────────────────────────────────────────────────────────────┤
│ 📄 Commercial Register Extract        [Corporate]              │
│    Code: COMM_REG_EXTRACT                                       │
└─────────────────────────────────────────────────────────────────┘

User selects "Certificate of Incorporation"
Result: (document.request :document-type "CERT_OF_INCORP"|
```

#### Attribute ID Completion

```
User types: (document.extract-attribute :attr-id "CBU.|
                                                      ↑ cursor

┌─────────────────────────────────────────────────────────────────┐
│ 📋 CBU.LEGAL_NAME                    [CBU Core]                 │
│    Legal name of the Client Business Unit                       │
├─────────────────────────────────────────────────────────────────┤
│ 📋 CBU.JURISDICTION                  [CBU Core]                 │
│    Jurisdiction of registration                                 │
├─────────────────────────────────────────────────────────────────┤
│ 📋 CBU.NATURE_PURPOSE                [CBU Core]                 │
│    Nature and purpose of business                               │
├─────────────────────────────────────────────────────────────────┤
│ 📋 CBU.CLIENT_TYPE                   [CBU Classification]       │
│    Type of client structure (UCITS, AIFM, etc.)                │
└─────────────────────────────────────────────────────────────────┘

User selects "CBU.LEGAL_NAME"
Result: (document.extract-attribute :attr-id "CBU.LEGAL_NAME"|
```

#### Role Completion

```
User types: (cbu.attach-entity :entity-id @company :role "|
                                                          ↑ cursor

┌─────────────────────────────────────────────────────────────────┐
│ 👤 Investment Manager               [Management]                │
│    Manages investments for the fund                             │
├─────────────────────────────────────────────────────────────────┤
│ 👤 Beneficial Owner                 [Ownership]                 │
│    Ultimate beneficial owner (>25% ownership)                   │
│    ⚠️ Requires: :ownership-percent                              │
├─────────────────────────────────────────────────────────────────┤
│ 👤 Director                         [Governance]                │
│    Member of board of directors                                 │
├─────────────────────────────────────────────────────────────────┤
│ 👤 Authorized Signatory             [Operations]                │
│    Authorized to sign on behalf of entity                       │
└─────────────────────────────────────────────────────────────────┘
```

### 11.4 Error Diagnostics for Invalid RefTypes

```rust
// rust/src/lsp/diagnostics.rs

impl DslLanguageServer {
    pub fn validate_document(&self, doc: &TextDocument) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        
        // Parse and validate
        match self.parse_and_validate(&doc.text) {
            Ok(_) => {}
            Err(report) => {
                for error in report.errors {
                    diagnostics.push(self.error_to_diagnostic(&error, &doc.text));
                }
            }
        }
        
        diagnostics
    }
    
    fn error_to_diagnostic(&self, error: &ValidationError, source: &str) -> Diagnostic {
        let range = span_to_range(&error.span, source);
        
        match &error.kind {
            ErrorKind::UnknownRef { ref_type, code, suggestions } => {
                Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: Some(NumberOrString::String("E010".to_string())),
                    source: Some("dsl".to_string()),
                    message: format!(
                        "Unknown {}: '{}'", 
                        ref_type.name(), 
                        code
                    ),
                    related_information: if suggestions.is_empty() {
                        None
                    } else {
                        Some(vec![DiagnosticRelatedInformation {
                            location: Location {
                                uri: doc.uri.clone(),
                                range,
                            },
                            message: format!("Did you mean: {}?", suggestions.join(", ")),
                        }])
                    },
                    ..Default::default()
                }
            }
            // ... other error kinds
        }
    }
}
```

### 11.5 Validation Error Display

```
┌─────────────────────────────────────────────────────────────────┐
│ file: kyc_session.dsl                                           │
├─────────────────────────────────────────────────────────────────┤
│  8 │ (document.request                                          │
│  9 │   :entity-id @company                                      │
│ 10 │   :document-type "CERT_INCORP")                            │
│    │                   ~~~~~~~~~~~~                              │
│    │ error[E010]: Unknown document type: 'CERT_INCORP'          │
│    │ hint: Did you mean: CERT_OF_INCORP, CERT_OF_GOOD_STANDING? │
│    │                                                             │
│ 15 │ (cbu.attach-entity                                          │
│ 16 │   :entity-id @person                                        │
│ 17 │   :role "BeneficialOwner")                                  │
│    │ error[E003]: Missing required argument ':ownership-percent' │
│    │ note: required when :role = "BeneficialOwner"              │
│    │                                                             │
│ 22 │ (risk.set-rating :rating "SUPER_HIGH")                      │
│    │                          ~~~~~~~~~~~~                       │
│    │ error[E011]: Invalid enum value: 'SUPER_HIGH'              │
│    │ allowed: LOW, MEDIUM, MEDIUM_HIGH, HIGH, VERY_HIGH         │
└─────────────────────────────────────────────────────────────────┘
```

---

## Part 12: Rust Verb Implementation Pattern

This section defines the pattern for implementing verbs with formal schema bindings.

### 12.1 Verb Module Structure

```rust
// rust/src/dsl_runtime/verbs/cbu.rs

use crate::schema::{VerbDef, ArgSpec, SemType, RefType, RequiredRule, DefaultValue, ContextKey};

// ═══════════════════════════════════════════════════════════════
// SCHEMA DEFINITION (Declarative)
// ═══════════════════════════════════════════════════════════════

pub static CBU_ATTACH_ENTITY: VerbDef = VerbDef {
    name: "cbu.attach-entity",
    domain: "cbu",
    crud_asset: "CBU_ENTITY_ROLE",
    description: "Attach an entity to a CBU with a specific role",
    
    args: &[
        ArgSpec {
            name: ":cbu-id",
            sem_type: SemType::Uuid,
            required: RequiredRule::Never,
            default: Some(DefaultValue::FromContext(ContextKey::CbuId)),
            validation: &[],
            description: "CBU to attach to (defaults to current context)",
        },
        ArgSpec {
            name: ":entity-id",
            sem_type: SemType::Symbol,  // @symbol reference
            required: RequiredRule::Always,
            default: None,
            validation: &[],
            description: "Entity to attach (reference to previously created entity)",
        },
        ArgSpec {
            name: ":role",
            sem_type: SemType::Ref(RefType::Role),  // ← Lookup from roles table
            required: RequiredRule::Always,
            default: None,
            validation: &[ValidationRule::LookupMustExist],
            description: "Role this entity plays in the CBU",
        },
        ArgSpec {
            name: ":ownership-percent",
            sem_type: SemType::Decimal,
            required: RequiredRule::IfEquals {
                arg: ":role",
                value: "BeneficialOwner",
            },
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
    
    examples: &[
        r#"(cbu.attach-entity :entity-id @company :role "InvestmentManager")"#,
        r#"(cbu.attach-entity :entity-id @person :role "BeneficialOwner" :ownership-percent 25.0)"#,
    ],
};

// ═══════════════════════════════════════════════════════════════
// IMPLEMENTATION (Uses ValidatedAST - types already checked!)
// ═══════════════════════════════════════════════════════════════

pub fn execute_cbu_attach_entity(
    call: &ValidatedVerbCall,  // Already validated against CBU_ATTACH_ENTITY
    env: &mut RuntimeEnv,
    executor: &mut CrudExecutor,
) -> Result<()> {
    // Extract typed values - GUARANTEED to exist and be correct type
    // because SchemaValidator already checked against ArgSpec
    
    let cbu_id = call.args.get(":cbu-id")
        .expect("validated: cbu-id present (required or context-injected)")
        .as_uuid()
        .expect("validated: cbu-id is UUID");
    
    let entity_id = call.args.get(":entity-id")
        .expect("validated: entity-id required")
        .as_symbol()
        .expect("validated: entity-id is Symbol");
    
    let role = call.args.get(":role")
        .expect("validated: role required")
        .as_ref_code()
        .expect("validated: role is Ref(Role)");
    
    // Optional fields - may or may not be present
    let ownership_percent = call.args.get(":ownership-percent")
        .map(|v| v.as_decimal().expect("validated: decimal"));
    
    let effective_date = call.args.get(":effective-date")
        .map(|v| v.as_date().expect("validated: date"));
    
    // Resolve symbol to actual UUID
    let entity_uuid = env.symbol_table
        .resolve(&entity_id)
        .ok_or_else(|| anyhow!("Symbol @{} not resolved", entity_id))?;
    
    // Resolve role code to UUID via LookupService
    let role_id = executor.lookup_service
        .resolve_role(&role)
        .await?;
    
    // Build CRUD operation
    executor.execute_insert(
        "cbu_entity_roles",
        &[
            ("cbu_id", Value::Uuid(cbu_id)),
            ("entity_id", Value::Uuid(entity_uuid)),
            ("role_id", Value::Uuid(role_id)),
            ("ownership_percent", ownership_percent.map(Value::Decimal).unwrap_or(Value::Null)),
            ("effective_date", effective_date.map(Value::Date).unwrap_or(Value::Null)),
        ],
    ).await
}
```

### 12.2 Word Registration

```rust
// rust/src/dsl_runtime/vocabulary.rs

use crate::verbs::{cbu, entity, document, kyc, screening, decision};

/// Register all words with their schemas and implementations
pub fn register_all_words(engine: &mut DslEngine) {
    // CBU domain
    engine.register_word(
        &cbu::CBU_ENSURE,
        cbu::execute_cbu_ensure,
    );
    engine.register_word(
        &cbu::CBU_ATTACH_ENTITY,
        cbu::execute_cbu_attach_entity,
    );
    engine.register_word(
        &cbu::CBU_DETACH_ENTITY,
        cbu::execute_cbu_detach_entity,
    );
    
    // Entity domain
    engine.register_word(
        &entity::ENTITY_CREATE_LIMITED_COMPANY,
        entity::execute_entity_create_limited_company,
    );
    engine.register_word(
        &entity::ENTITY_CREATE_PROPER_PERSON,
        entity::execute_entity_create_proper_person,
    );
    
    // Document domain
    engine.register_word(
        &document::DOCUMENT_REQUEST,
        document::execute_document_request,
    );
    engine.register_word(
        &document::DOCUMENT_EXTRACT_ATTRIBUTE,
        document::execute_document_extract_attribute,
    );
    
    // ... etc
}
```

### 12.3 Engine Execution Flow

```rust
// rust/src/dsl_runtime/engine.rs

pub struct DslEngine {
    words: HashMap<&'static str, RegisteredWord>,
    schema_cache: Arc<SchemaCache>,
    validator: SchemaValidator,
}

struct RegisteredWord {
    schema: &'static VerbDef,
    execute: fn(&ValidatedVerbCall, &mut RuntimeEnv, &mut CrudExecutor) -> Result<()>,
}

impl DslEngine {
    pub fn register_word(
        &mut self,
        schema: &'static VerbDef,
        execute: fn(&ValidatedVerbCall, &mut RuntimeEnv, &mut CrudExecutor) -> Result<()>,
    ) {
        self.words.insert(schema.name, RegisteredWord { schema, execute });
    }
    
    /// Execute DSL source with full validation pipeline
    pub async fn execute(&self, source: &str, env: &mut RuntimeEnv) -> Result<ExecutionResult> {
        // Phase 1: Parse (syntax)
        let raw_ast = parse_program(source)
            .map_err(|e| anyhow!("Parse error: {:?}", e))?;
        
        // Phase 2: Validate (semantics)
        let validated_ast = self.validator.validate(&raw_ast, env, &self.schema_cache)
            .map_err(|report| {
                // Pretty-print errors
                anyhow!("{}", report.format(source, "input.dsl"))
            })?;
        
        // Phase 3: Execute (runtime)
        let mut executor = CrudExecutor::new(&self.schema_cache);
        
        for expr in &validated_ast.expressions {
            match &expr.kind {
                ValidatedExprKind::VerbCall { verb, args, defines_symbol, .. } => {
                    // Get registered word
                    let word = self.words.get(verb.name)
                        .expect("verb exists: validated");
                    
                    // Create validated call struct
                    let call = ValidatedVerbCall {
                        verb,
                        args: args.clone(),
                    };
                    
                    // Execute
                    (word.execute)(&call, env, &mut executor)?;
                    
                    // Capture symbol if :as was provided
                    if let Some(sym_name) = defines_symbol {
                        if let Some(produced_id) = executor.last_produced_id() {
                            env.symbol_table.resolve(sym_name, produced_id);
                        }
                    }
                }
                ValidatedExprKind::Comment(_) => {}
            }
        }
        
        Ok(ExecutionResult {
            crud_count: executor.operation_count(),
            symbols: env.symbol_table.clone(),
        })
    }
}
```

---

## Summary: Complete Pipeline

```
┌─────────────────────────────────────────────────────────────────┐
│                        COMPLETE PIPELINE                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  DSL Source                                                      │
│  ──────────                                                      │
│  (cbu.attach-entity                                              │
│    :entity-id @company                                           │
│    :role "InvestmentManager")                                    │
│                                                                  │
│       │                                                          │
│       ▼                                                          │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │ PHASE 1: PARSER (Nom)                                      │  │
│  │ • Tokenize → AST                                           │  │
│  │ • Lookup VerbDef: "cbu.attach-entity" → CBU_ATTACH_ENTITY │  │
│  │ • Lookup ArgSpec: ":role" → ArgSpec{sem_type: Ref(Role)}  │  │
│  │ • Attach spans for error reporting                         │  │
│  └────────────────────────────────────────────────────────────┘  │
│       │                                                          │
│       ▼                                                          │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │ PHASE 2: SCHEMA VALIDATOR                                   │  │
│  │ • Type check: "InvestmentManager" vs SemType::Ref(Role)   │  │
│  │ • Lookup: SchemaCache.roles.exists("InvestmentManager")?  │  │
│  │   → ✓ Found | ✗ Error + "Did you mean...?"                │  │
│  │ • Required check: :entity-id required ✓                    │  │
│  │ • Conditional: :ownership-percent needed if UBO? No ✓      │  │
│  │ • Context inject: :cbu-id from env.cbu_id                 │  │
│  │ • Symbol check: @company defined? ✓                        │  │
│  └────────────────────────────────────────────────────────────┘  │
│       │                                                          │
│       ▼                                                          │
│  ValidatedAST                                                    │
│  ────────────                                                    │
│  VerbCall {                                                      │
│    verb: &CBU_ATTACH_ENTITY,                                    │
│    args: {                                                       │
│      ":cbu-id": TypedValue::Uuid(env.cbu_id),  // injected     │
│      ":entity-id": TypedValue::Symbol("company"),               │
│      ":role": TypedValue::Ref(Role, "InvestmentManager"),       │
│    }                                                             │
│  }                                                               │
│                                                                  │
│       │                                                          │
│       ▼                                                          │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │ PHASE 3: EXECUTOR                                           │  │
│  │ • Resolve @company → UUID from SymbolTable                 │  │
│  │ • Resolve "InvestmentManager" → UUID via LookupService     │  │
│  │ • INSERT INTO cbu_entity_roles (...)                       │  │
│  └────────────────────────────────────────────────────────────┘  │
│                                                                  │
│  LSP (runs Phase 1+2 continuously)                              │
│  ───                                                             │
│  • Autocomplete: After :role " → show roles from SchemaCache   │
│  • Diagnostics: Unknown role → red squiggle + suggestion        │
│  • Hover: :role → "Role this entity plays in the CBU"          │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Updated Implementation Phases

### Phase 1: Schema Foundation ✓ (in document)
- [x] SemType, ArgSpec, VerbDef definitions
- [x] RefType enum with table mappings
- [x] RequiredRule, CrossConstraint, ValidationRule

### Phase 2: Parser-Schema Integration (NEW)
- [ ] Update nom parser to lookup VerbDef at parse time
- [ ] Attach VerbDef and ArgSpec to RawAST nodes
- [ ] Add Span tracking throughout

### Phase 3: SchemaCache (NEW)
- [ ] Define SchemaCache struct with all lookup tables
- [ ] Implement database loading for document_types, attributes, roles, etc.
- [ ] Add exists(), suggest(), get_completions() methods

### Phase 4: Schema Validator ✓ (in document)
- [ ] Implement full validation against VerbDef
- [ ] RefType validation against SchemaCache
- [ ] Cross-constraint checking
- [ ] Symbol table construction

### Phase 5: LSP RefType Completions (NEW)
- [ ] Implement completion handler for RefTypes
- [ ] Add filtered completions with prefix matching
- [ ] Format completion items with display_name, category, docs
- [ ] Add hover documentation for RefTypes

### Phase 6: Verb Implementation Pattern (NEW)
- [ ] Refactor existing word functions to use ValidatedVerbCall
- [ ] Remove redundant validation from implementations
- [ ] Add static VerbDef for each verb
- [ ] Update engine registration

### Phase 7: Complete All Verb Schemas
- [ ] CBU domain verbs
- [ ] Entity domain verbs
- [ ] Document domain verbs (with attribute mappings)
- [ ] KYC domain verbs
- [ ] Screening domain verbs
- [ ] Decision domain verbs
- [ ] Monitoring domain verbs

### Phase 8: Error Reporting & Diagnostics ✓ (in document)
- [ ] Pretty-printed errors with line numbers
- [ ] LSP diagnostic conversion
- [ ] Typo suggestions

### Phase 9: LLM Context Export ✓ (in document)
- [ ] VerbDef.to_llm_context()
- [ ] Full registry export

