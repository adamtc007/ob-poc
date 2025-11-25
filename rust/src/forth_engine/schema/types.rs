//! Core type definitions for the Verb Schema System.

use serde::{Deserialize, Serialize};

/// Semantic type for argument values.
#[derive(Debug, Clone, PartialEq)]
pub enum SemType {
    String,
    Uuid,
    Integer,
    Decimal,
    Date,
    Boolean,
    Ref(RefType),
    Enum(&'static [&'static str]),
    Symbol,
    /// List of values - uses reference to avoid Box in static
    ListOf(&'static SemType),
    /// Nested map structure with field specifications
    Map(&'static [ArgSpec]),
    /// One of several types
    OneOf(&'static [SemType]),
}

/// Reference types that map to lookup tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RefType {
    DocumentType,
    Attribute,
    Role,
    EntityType,
    Jurisdiction,
    ScreeningList,
    Currency,
}

impl RefType {
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

    pub fn table_info(&self) -> (&'static str, &'static str, &'static str) {
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

/// Specification for a single verb argument.
#[derive(Debug, Clone, PartialEq)]
pub struct ArgSpec {
    pub name: &'static str,
    pub sem_type: SemType,
    pub required: RequiredRule,
    pub default: Option<DefaultValue>,
    pub validation: &'static [ValidationRule],
    pub description: &'static str,
}

/// Rules for when an argument is required.
#[derive(Debug, Clone, PartialEq)]
pub enum RequiredRule {
    Always,
    Never,
    UnlessProvided(&'static str),
    IfEquals { arg: &'static str, value: &'static str },
    IfProvided(&'static str),
}

/// Default values for optional arguments.
#[derive(Debug, Clone, PartialEq)]
pub enum DefaultValue {
    Str(&'static str),
    Int(i64),
    Decimal(f64),
    Bool(bool),
    FromContext(ContextKey),
}

/// Keys for context injection from RuntimeEnv.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

/// Additional validation rules.
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationRule {
    LookupMustExist,
    Pattern { regex: &'static str, description: &'static str },
    Range { min: Option<f64>, max: Option<f64> },
    Length { min: Option<usize>, max: Option<usize> },
    DateRange { min: Option<DateBound>, max: Option<DateBound> },
    NotEmpty,
    Custom(&'static str),
}

/// Bounds for date validation.
#[derive(Debug, Clone, PartialEq)]
pub enum DateBound {
    Literal(&'static str),
    Today,
    DaysFromToday(i32),
}

/// Cross-argument constraints.
#[derive(Debug, Clone, PartialEq)]
pub enum CrossConstraint {
    ExactlyOne(&'static [&'static str]),
    AtLeastOne(&'static [&'static str]),
    Requires { if_present: &'static str, then_require: &'static str },
    Excludes { if_present: &'static str, then_forbid: &'static str },
    ConditionalRequired { if_arg: &'static str, equals: &'static str, then_require: &'static str },
    LessThan { lesser: &'static str, greater: &'static str },
}

/// Complete definition of a DSL verb.
#[derive(Debug, Clone, PartialEq)]
pub struct VerbDef {
    pub name: &'static str,
    pub domain: &'static str,
    pub args: &'static [ArgSpec],
    pub constraints: &'static [CrossConstraint],
    pub produces: Option<ProducesSpec>,
    pub crud_asset: &'static str,
    pub description: &'static str,
    pub examples: &'static [&'static str],
}

/// What a verb produces for context capture.
#[derive(Debug, Clone, PartialEq)]
pub struct ProducesSpec {
    pub capture_as: ContextKey,
    pub description: &'static str,
}

impl SemType {
    pub fn type_name(&self) -> String {
        match self {
            SemType::String => "STRING".to_string(),
            SemType::Uuid => "UUID".to_string(),
            SemType::Integer => "INTEGER".to_string(),
            SemType::Decimal => "DECIMAL".to_string(),
            SemType::Date => "DATE (YYYY-MM-DD)".to_string(),
            SemType::Boolean => "BOOLEAN".to_string(),
            SemType::Ref(r) => format!("{}_REF", r.name().to_uppercase().replace(' ', "_")),
            SemType::Enum(values) => format!("one of {:?}", values),
            SemType::Symbol => "SYMBOL (@name)".to_string(),
            SemType::ListOf(inner) => format!("LIST<{}>", inner.type_name()),
            SemType::Map(_) => "MAP".to_string(),
            SemType::OneOf(types) => {
                let names: Vec<_> = types.iter().map(|t| t.type_name()).collect();
                format!("one of [{}]", names.join(", "))
            }
        }
    }
}

impl VerbDef {
    pub fn get_arg(&self, name: &str) -> Option<&ArgSpec> {
        self.args.iter().find(|a| a.name == name)
    }

    pub fn required_args(&self) -> Vec<&'static str> {
        self.args
            .iter()
            .filter(|a| matches!(a.required, RequiredRule::Always))
            .map(|a| a.name)
            .collect()
    }

    pub fn optional_args(&self) -> Vec<&'static str> {
        self.args
            .iter()
            .filter(|a| !matches!(a.required, RequiredRule::Always))
            .map(|a| a.name)
            .collect()
    }
}
