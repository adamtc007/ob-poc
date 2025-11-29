//! Verb Schema - Typed Argument Definitions
//!
//! This module defines the type system for DSL verb arguments.
//! Each argument has a declared type that determines:
//! 1. What syntax is valid (string, number, UUID, reference)
//! 2. What DB validation is required (existence check, FK lookup)
//!
//! This is the bridge between NOM parsing (syntax) and semantic validation (types + existence).

use crate::dsl_v2::validation::RefType;
use std::collections::HashMap;
use std::sync::LazyLock;

// =============================================================================
// ARGUMENT TYPE SYSTEM
// =============================================================================

/// The type of a verb argument - determines validation rules
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgType {
    /// Plain string - no DB validation
    String,

    /// Integer value
    Integer,

    /// Decimal/float value
    Number,

    /// Boolean (true/false)
    Boolean,

    /// UUID that must exist in a specific table
    /// e.g., :document-id must exist in document_catalog
    UuidRef(RefType),

    /// String code that must exist in a lookup table
    /// e.g., :document-type must exist in document_types.type_code
    CodeRef(RefType),

    /// Symbol reference (@name) - resolved at runtime
    /// Must be bound by a previous :as clause
    Symbol,

    /// Either a Symbol OR a UuidRef - common for :entity-id, :cbu-id
    /// Allows both @entity (symbol) and "uuid-string" (literal)
    SymbolOrUuid(RefType),

    /// Either a Symbol OR a CodeRef
    /// Allows both @doc_type (symbol) and "PASSPORT_GBR" (literal)
    SymbolOrCode(RefType),

    /// List of values of a specific type
    List(Box<ArgType>),

    /// Percentage (0-100 or 0.0-1.0)
    Percentage,

    /// Date in ISO format (YYYY-MM-DD)
    Date,

    /// Nested verb call
    NestedVerb,

    /// Any value - no type checking (escape hatch)
    Any,
}

impl ArgType {
    /// Get the RefType if this arg needs DB validation
    pub fn ref_type(&self) -> Option<RefType> {
        match self {
            ArgType::UuidRef(r)
            | ArgType::CodeRef(r)
            | ArgType::SymbolOrUuid(r)
            | ArgType::SymbolOrCode(r) => Some(*r),
            ArgType::List(inner) => inner.ref_type(),
            _ => None,
        }
    }

    /// Does this type require DB lookup?
    pub fn needs_db_validation(&self) -> bool {
        matches!(
            self,
            ArgType::UuidRef(_)
                | ArgType::CodeRef(_)
                | ArgType::SymbolOrUuid(_)
                | ArgType::SymbolOrCode(_)
        ) || matches!(self, ArgType::List(inner) if inner.needs_db_validation())
    }

    /// Is a symbol reference valid for this type?
    pub fn accepts_symbol(&self) -> bool {
        matches!(
            self,
            ArgType::Symbol | ArgType::SymbolOrUuid(_) | ArgType::SymbolOrCode(_) | ArgType::Any
        )
    }
}

// =============================================================================
// ARGUMENT DEFINITION
// =============================================================================

/// Complete definition of a verb argument
#[derive(Debug, Clone)]
pub struct ArgDef {
    /// Argument name (without colon), e.g., "name", "document-type"
    pub name: &'static str,

    /// The expected type
    pub arg_type: ArgType,

    /// Is this argument required?
    pub required: bool,

    /// Human-readable description
    pub description: &'static str,
}

impl ArgDef {
    pub const fn required(
        name: &'static str,
        arg_type: ArgType,
        description: &'static str,
    ) -> Self {
        Self {
            name,
            arg_type,
            required: true,
            description,
        }
    }

    pub const fn optional(
        name: &'static str,
        arg_type: ArgType,
        description: &'static str,
    ) -> Self {
        Self {
            name,
            arg_type,
            required: false,
            description,
        }
    }
}

// =============================================================================
// VERB SCHEMA
// =============================================================================

/// Complete schema for a verb - defines all valid arguments and their types
#[derive(Debug, Clone)]
pub struct VerbSchema {
    /// Full verb name: "domain.verb"
    pub verb: &'static str,

    /// All argument definitions
    pub args: &'static [ArgDef],

    /// What this verb returns (for :as binding type inference)
    pub returns: Option<RefType>,

    /// Brief description
    pub description: &'static str,
}

impl VerbSchema {
    /// Get argument definition by name
    pub fn get_arg(&self, name: &str) -> Option<&ArgDef> {
        self.args.iter().find(|a| a.name == name)
    }

    /// Get all required arguments
    pub fn required_args(&self) -> impl Iterator<Item = &ArgDef> {
        self.args.iter().filter(|a| a.required)
    }

    /// Get all optional arguments
    pub fn optional_args(&self) -> impl Iterator<Item = &ArgDef> {
        self.args.iter().filter(|a| !a.required)
    }

    /// Get all argument names
    pub fn arg_names(&self) -> impl Iterator<Item = &str> {
        self.args.iter().map(|a| a.name)
    }
}

// =============================================================================
// VERB SCHEMA REGISTRY
// =============================================================================

/// All verb schemas - indexed by "domain.verb"
pub static VERB_SCHEMAS: LazyLock<HashMap<&'static str, &'static VerbSchema>> =
    LazyLock::new(|| {
        let mut map = HashMap::new();
        for schema in ALL_VERB_SCHEMAS.iter() {
            map.insert(schema.verb, schema);
        }
        map
    });

/// Look up a verb schema by domain and verb name
pub fn get_verb_schema(domain: &str, verb: &str) -> Option<&'static VerbSchema> {
    let key = format!("{}.{}", domain, verb);
    // Need to leak the string since VERB_SCHEMAS uses &'static str keys
    // This is fine since we only do lookups, not insertions at runtime
    VERB_SCHEMAS.get(key.as_str()).copied()
}

/// Look up by full verb name
pub fn get_schema(full_verb: &str) -> Option<&'static VerbSchema> {
    VERB_SCHEMAS.get(full_verb).copied()
}

// =============================================================================
// VERB SCHEMA DEFINITIONS
// =============================================================================

static ALL_VERB_SCHEMAS: &[VerbSchema] = &[
    // =========================================================================
    // CBU DOMAIN
    // =========================================================================
    VerbSchema {
        verb: "cbu.create",
        args: &[
            ArgDef::required("name", ArgType::String, "Name of the CBU/client"),
            ArgDef::required(
                "client-type",
                ArgType::String,
                "Type: individual, corporate, fund, trust",
            ),
            ArgDef::optional(
                "jurisdiction",
                ArgType::CodeRef(RefType::Jurisdiction),
                "Jurisdiction code",
            ),
            ArgDef::optional("external-id", ArgType::String, "External system identifier"),
        ],
        returns: Some(RefType::Cbu),
        description: "Create a new Client Business Unit",
    },
    VerbSchema {
        verb: "cbu.get",
        args: &[ArgDef::required(
            "cbu-id",
            ArgType::SymbolOrUuid(RefType::Cbu),
            "CBU identifier",
        )],
        returns: Some(RefType::Cbu),
        description: "Get CBU details",
    },
    VerbSchema {
        verb: "cbu.update",
        args: &[
            ArgDef::required(
                "cbu-id",
                ArgType::SymbolOrUuid(RefType::Cbu),
                "CBU identifier",
            ),
            ArgDef::optional("name", ArgType::String, "Updated name"),
            ArgDef::optional("status", ArgType::String, "Updated status"),
        ],
        returns: Some(RefType::Cbu),
        description: "Update CBU details",
    },
    VerbSchema {
        verb: "cbu.assign-role",
        args: &[
            ArgDef::required(
                "cbu-id",
                ArgType::SymbolOrUuid(RefType::Cbu),
                "CBU identifier",
            ),
            ArgDef::required(
                "entity-id",
                ArgType::SymbolOrUuid(RefType::Entity),
                "Entity to assign",
            ),
            ArgDef::required("role", ArgType::CodeRef(RefType::Role), "Role name"),
            ArgDef::optional(
                "ownership-percentage",
                ArgType::Percentage,
                "Ownership % for UBOs",
            ),
        ],
        returns: None,
        description: "Assign an entity role to a CBU",
    },
    VerbSchema {
        verb: "cbu.remove-role",
        args: &[
            ArgDef::required(
                "cbu-id",
                ArgType::SymbolOrUuid(RefType::Cbu),
                "CBU identifier",
            ),
            ArgDef::required(
                "entity-id",
                ArgType::SymbolOrUuid(RefType::Entity),
                "Entity to remove",
            ),
            ArgDef::required("role", ArgType::CodeRef(RefType::Role), "Role to remove"),
        ],
        returns: None,
        description: "Remove an entity role from a CBU",
    },
    VerbSchema {
        verb: "cbu.list-parties",
        args: &[
            ArgDef::required(
                "cbu-id",
                ArgType::SymbolOrUuid(RefType::Cbu),
                "CBU identifier",
            ),
            ArgDef::optional("role", ArgType::CodeRef(RefType::Role), "Filter by role"),
        ],
        returns: None,
        description: "List all parties associated with a CBU",
    },
    VerbSchema {
        verb: "cbu.get-status",
        args: &[ArgDef::required(
            "cbu-id",
            ArgType::SymbolOrUuid(RefType::Cbu),
            "CBU identifier",
        )],
        returns: None,
        description: "Get CBU onboarding status",
    },
    VerbSchema {
        verb: "cbu.set-risk-rating",
        args: &[
            ArgDef::required(
                "cbu-id",
                ArgType::SymbolOrUuid(RefType::Cbu),
                "CBU identifier",
            ),
            ArgDef::required("rating", ArgType::String, "Risk rating: LOW, MEDIUM, HIGH"),
            ArgDef::optional("rationale", ArgType::String, "Reason for rating"),
        ],
        returns: None,
        description: "Set risk rating for a CBU",
    },
    // =========================================================================
    // ENTITY DOMAIN
    // =========================================================================
    VerbSchema {
        verb: "entity.create",
        args: &[
            ArgDef::required("name", ArgType::String, "Legal name"),
            ArgDef::required("type", ArgType::CodeRef(RefType::EntityType), "Entity type"),
            ArgDef::optional(
                "jurisdiction",
                ArgType::CodeRef(RefType::Jurisdiction),
                "Jurisdiction",
            ),
        ],
        returns: Some(RefType::Entity),
        description: "Create a generic entity (use specific create-* verbs when possible)",
    },
    VerbSchema {
        verb: "entity.create-natural-person",
        args: &[
            ArgDef::required("name", ArgType::String, "Full legal name"),
            ArgDef::optional("first-name", ArgType::String, "First name"),
            ArgDef::optional("last-name", ArgType::String, "Last name"),
            ArgDef::optional("date-of-birth", ArgType::Date, "Date of birth"),
            ArgDef::optional(
                "nationality",
                ArgType::CodeRef(RefType::Jurisdiction),
                "Nationality",
            ),
        ],
        returns: Some(RefType::Entity),
        description: "Create a natural person entity",
    },
    VerbSchema {
        verb: "entity.create-limited-company",
        args: &[
            ArgDef::required("name", ArgType::String, "Company name"),
            ArgDef::optional(
                "jurisdiction",
                ArgType::CodeRef(RefType::Jurisdiction),
                "Incorporation jurisdiction",
            ),
            ArgDef::optional("company-number", ArgType::String, "Registration number"),
            ArgDef::optional("incorporation-date", ArgType::Date, "Date of incorporation"),
        ],
        returns: Some(RefType::Entity),
        description: "Create a limited company entity",
    },
    VerbSchema {
        verb: "entity.create-partnership",
        args: &[
            ArgDef::required("name", ArgType::String, "Partnership name"),
            ArgDef::optional("partnership-type", ArgType::String, "Type: LP, LLP, GP"),
            ArgDef::optional(
                "jurisdiction",
                ArgType::CodeRef(RefType::Jurisdiction),
                "Jurisdiction",
            ),
        ],
        returns: Some(RefType::Entity),
        description: "Create a partnership entity",
    },
    VerbSchema {
        verb: "entity.create-trust",
        args: &[
            ArgDef::required("name", ArgType::String, "Trust name"),
            ArgDef::optional("trust-type", ArgType::String, "Type of trust"),
            ArgDef::optional(
                "jurisdiction",
                ArgType::CodeRef(RefType::Jurisdiction),
                "Governing jurisdiction",
            ),
        ],
        returns: Some(RefType::Entity),
        description: "Create a trust entity",
    },
    VerbSchema {
        verb: "entity.get",
        args: &[ArgDef::required(
            "entity-id",
            ArgType::SymbolOrUuid(RefType::Entity),
            "Entity identifier",
        )],
        returns: Some(RefType::Entity),
        description: "Get entity details",
    },
    VerbSchema {
        verb: "entity.set-attribute",
        args: &[
            ArgDef::required(
                "entity-id",
                ArgType::SymbolOrUuid(RefType::Entity),
                "Entity identifier",
            ),
            ArgDef::required(
                "attribute",
                ArgType::CodeRef(RefType::AttributeId),
                "Attribute code",
            ),
            ArgDef::required("value", ArgType::Any, "Attribute value"),
        ],
        returns: None,
        description: "Set an attribute value on an entity",
    },
    // =========================================================================
    // DOCUMENT DOMAIN
    // =========================================================================
    VerbSchema {
        verb: "document.catalog",
        args: &[
            ArgDef::required(
                "document-type",
                ArgType::CodeRef(RefType::DocumentType),
                "Document type code",
            ),
            ArgDef::required(
                "cbu-id",
                ArgType::SymbolOrUuid(RefType::Cbu),
                "Associated CBU",
            ),
            ArgDef::optional("title", ArgType::String, "Document title/name"),
            ArgDef::optional(
                "entity-id",
                ArgType::SymbolOrUuid(RefType::Entity),
                "Associated entity",
            ),
        ],
        returns: Some(RefType::Document),
        description: "Catalog a document in the system",
    },
    VerbSchema {
        verb: "document.get",
        args: &[ArgDef::required(
            "document-id",
            ArgType::SymbolOrUuid(RefType::Document),
            "Document identifier",
        )],
        returns: Some(RefType::Document),
        description: "Get document details",
    },
    VerbSchema {
        verb: "document.extract",
        args: &[ArgDef::required(
            "document-id",
            ArgType::SymbolOrUuid(RefType::Document),
            "Document to extract from",
        )],
        returns: None,
        description: "Extract attributes from a document",
    },
    VerbSchema {
        verb: "document.link-entity",
        args: &[
            ArgDef::required(
                "document-id",
                ArgType::SymbolOrUuid(RefType::Document),
                "Document identifier",
            ),
            ArgDef::required(
                "entity-id",
                ArgType::SymbolOrUuid(RefType::Entity),
                "Entity to link",
            ),
        ],
        returns: None,
        description: "Link a document to an entity",
    },
    VerbSchema {
        verb: "document.list",
        args: &[
            ArgDef::required(
                "cbu-id",
                ArgType::SymbolOrUuid(RefType::Cbu),
                "CBU to list documents for",
            ),
            ArgDef::optional(
                "document-type",
                ArgType::CodeRef(RefType::DocumentType),
                "Filter by type",
            ),
            ArgDef::optional("status", ArgType::String, "Filter by status"),
        ],
        returns: None,
        description: "List documents for a CBU",
    },
    // =========================================================================
    // SCREENING DOMAIN
    // =========================================================================
    VerbSchema {
        verb: "screening.pep",
        args: &[ArgDef::required(
            "entity-id",
            ArgType::SymbolOrUuid(RefType::Entity),
            "Entity to screen",
        )],
        returns: None,
        description: "Run PEP (Politically Exposed Person) screening",
    },
    VerbSchema {
        verb: "screening.sanctions",
        args: &[ArgDef::required(
            "entity-id",
            ArgType::SymbolOrUuid(RefType::Entity),
            "Entity to screen",
        )],
        returns: None,
        description: "Run sanctions screening",
    },
    VerbSchema {
        verb: "screening.adverse-media",
        args: &[ArgDef::required(
            "entity-id",
            ArgType::SymbolOrUuid(RefType::Entity),
            "Entity to screen",
        )],
        returns: None,
        description: "Run adverse media screening",
    },
    // =========================================================================
    // KYC DOMAIN
    // =========================================================================
    VerbSchema {
        verb: "kyc.run-check",
        args: &[
            ArgDef::required(
                "cbu-id",
                ArgType::SymbolOrUuid(RefType::Cbu),
                "CBU to check",
            ),
            ArgDef::required("check-type", ArgType::String, "Type of check"),
        ],
        returns: None,
        description: "Run a KYC check",
    },
    VerbSchema {
        verb: "kyc.validate-attributes",
        args: &[
            ArgDef::required(
                "cbu-id",
                ArgType::SymbolOrUuid(RefType::Cbu),
                "CBU to validate",
            ),
            ArgDef::optional("all", ArgType::Boolean, "Validate all attributes"),
            ArgDef::optional(
                "attribute",
                ArgType::CodeRef(RefType::AttributeId),
                "Specific attribute to validate",
            ),
        ],
        returns: None,
        description: "Validate attributes for a CBU",
    },
    // =========================================================================
    // UBO DOMAIN
    // =========================================================================
    VerbSchema {
        verb: "ubo.calculate",
        args: &[
            ArgDef::required(
                "cbu-id",
                ArgType::SymbolOrUuid(RefType::Cbu),
                "CBU to calculate UBOs for",
            ),
            ArgDef::optional(
                "threshold",
                ArgType::Percentage,
                "Ownership threshold (default 25%)",
            ),
        ],
        returns: None,
        description: "Calculate ultimate beneficial owners",
    },
];

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verb_schema_lookup() {
        let schema = get_schema("cbu.create");
        assert!(schema.is_some());
        let schema = schema.unwrap();
        assert_eq!(schema.verb, "cbu.create");
        assert!(schema.get_arg("name").is_some());
        assert!(schema.get_arg("name").unwrap().required);
    }

    #[test]
    fn test_arg_type_ref() {
        assert_eq!(
            ArgType::CodeRef(RefType::DocumentType).ref_type(),
            Some(RefType::DocumentType)
        );
        assert_eq!(ArgType::String.ref_type(), None);
        assert!(ArgType::UuidRef(RefType::Entity).needs_db_validation());
        assert!(!ArgType::String.needs_db_validation());
    }

    #[test]
    fn test_symbol_acceptance() {
        assert!(ArgType::Symbol.accepts_symbol());
        assert!(ArgType::SymbolOrUuid(RefType::Cbu).accepts_symbol());
        assert!(!ArgType::String.accepts_symbol());
        assert!(!ArgType::UuidRef(RefType::Document).accepts_symbol());
    }

    #[test]
    fn test_all_schemas_valid() {
        // Ensure all schemas are accessible
        assert!(!VERB_SCHEMAS.is_empty());

        // Check a few key verbs exist
        assert!(get_schema("cbu.create").is_some());
        assert!(get_schema("entity.create-natural-person").is_some());
        assert!(get_schema("document.catalog").is_some());
        assert!(get_schema("screening.pep").is_some());
    }

    #[test]
    fn test_required_args() {
        let schema = get_schema("cbu.assign-role").unwrap();
        let required: Vec<_> = schema.required_args().collect();
        assert_eq!(required.len(), 3); // cbu-id, entity-id, role
    }
}
