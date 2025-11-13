//! Enhanced type system support for AST values with semantic analysis and database integration

use super::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Enhanced semantic information for DSL elements
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)]
pub(crate) struct SemanticInfo {
    pub source_location: SourceLocation,
    pub type_info: TypeInfo,
    pub validation_state: ValidationState,
    pub dependencies: Vec<String>,
    pub database_refs: Vec<DatabaseReference>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceLocation {
    pub line: usize,
    pub column: usize,
    pub file: Option<String>,
    pub span: Option<(usize, usize)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)]
pub(crate) struct TypeInfo {
    pub expected_type: DSLType,
    pub inferred_type: Option<DSLType>,
    pub constraints: Vec<TypeConstraint>,
    pub nullable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)]
pub(crate) enum DSLType {
    String {
        max_length: Option<usize>,
    },
    Number {
        min: Option<f64>,
        max: Option<f64>,
    },
    Integer {
        min: Option<i64>,
        max: Option<i64>,
    },
    Boolean,
    Date,
    DateTime,
    UUID,
    EntityReference {
        entity_type: String,
    },
    VocabularyVerb {
        domain: String,
    },
    List {
        element_type: Box<DSLType>,
    },
    Map {
        value_type: Box<DSLType>,
    },
    Union {
        types: Vec<DSLType>,
    },
    Custom {
        name: String,
        schema: HashMap<String, DSLType>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)]
pub(crate) enum TypeConstraint {
    Required,
    Unique,
    MinLength(usize),
    MaxLength(usize),
    Pattern(String),
    InSet(Vec<String>),
    CustomValidator {
        name: String,
        params: HashMap<String, Value>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum ValidationState {
    #[default]
    Pending,
    Valid,
    Invalid {
        errors: Vec<ValidationError>,
    },
    Warning {
        warnings: Vec<ValidationWarning>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationError {
    pub code: String,
    pub message: String,
    pub severity: ErrorSeverity,
    pub location: Option<SourceLocation>,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationWarning {
    pub code: String,
    pub message: String,
    pub location: Option<SourceLocation>,
    pub auto_fix: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ErrorSeverity {
    Error,
    Warning,
    Info,
}

/// Database integration metadata
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)]
pub(crate) struct DatabaseReference {
    pub table: String,
    pub column: Option<String>,
    pub reference_type: DbReferenceType,
    pub uuid: Option<Uuid>,
    pub version: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)]
pub(crate) enum DbReferenceType {
    GrammarRule,
    VocabularyVerb,
    DomainDefinition,
    AttributeDefinition,
    EntityType,
    WorkflowState,
    ValidationRule,
}

/// Enhanced Value type with semantic information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)]
pub(crate) enum EnhancedValue {
    String {
        value: String,
        semantic: Option<SemanticInfo>,
    },
    Number {
        value: f64,
        semantic: Option<SemanticInfo>,
    },
    Integer {
        value: i64,
        semantic: Option<SemanticInfo>,
    },
    Boolean {
        value: bool,
        semantic: Option<SemanticInfo>,
    },
    Date {
        value: chrono::NaiveDate,
        semantic: Option<SemanticInfo>,
    },
    DateTime {
        value: DateTime<Utc>,
        semantic: Option<SemanticInfo>,
    },
    UUID {
        value: Uuid,
        semantic: Option<SemanticInfo>,
    },
    List {
        values: Vec<EnhancedValue>,
        semantic: Option<SemanticInfo>,
    },
    Map {
        values: HashMap<String, EnhancedValue>,
        semantic: Option<SemanticInfo>,
    },
    MultiValue {
        values: Vec<ValueWithSource>,
        semantic: Option<SemanticInfo>,
    },
    Null {
        semantic: Option<SemanticInfo>,
    },
}

/// Grammar rule representation for database integration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrammarRule {
    pub rule_id: Uuid,
    pub rule_name: String,
    pub rule_definition: String,
    pub rule_type: GrammarRuleType,
    pub domain: Option<String>,
    pub version: String,
    pub active: bool,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GrammarRuleType {
    Production,
    Terminal,
    Lexical,
}

/// Vocabulary verb representation for database integration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VocabularyVerb {
    pub vocab_id: Uuid,
    pub domain: String,
    pub verb: String,
    pub category: Option<String>,
    pub description: Option<String>,
    pub parameters: Option<serde_json::Value>,
    pub examples: Option<Vec<String>>,
    pub phase: Option<String>,
    pub active: bool,
    pub version: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// DSL lifecycle state tracking
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)]
pub(crate) struct DSLState {
    pub state_id: Uuid,
    pub dsl_version_id: Uuid,
    pub current_state: LifecycleState,
    pub previous_state: Option<LifecycleState>,
    pub transition_reason: Option<String>,
    pub transition_timestamp: DateTime<Utc>,
    pub metadata: HashMap<String, EnhancedValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[allow(dead_code)]
pub(crate) enum LifecycleState {
    #[default]
    Draft,
    Validating,
    Valid,
    Invalid,
    Compiling,
    Compiled,
    Deploying,
    Active,
    Deprecated,
    Archived,
}

impl Value {
    #[allow(dead_code)]
    pub(crate) fn as_string(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn as_map(&self) -> Option<&PropertyMap> {
        match self {
            Value::Map(m) => Some(m),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Convert to enhanced value with semantic information
    #[allow(dead_code)]
    pub(crate) fn to_enhanced(&self, semantic: Option<SemanticInfo>) -> EnhancedValue {
        match self {
            Value::String(s) => EnhancedValue::String {
                value: s.clone(),
                semantic,
            },
            Value::Number(n) => EnhancedValue::Number {
                value: *n,
                semantic,
            },
            Value::Integer(i) => EnhancedValue::Integer {
                value: *i,
                semantic,
            },
            Value::Boolean(b) => EnhancedValue::Boolean {
                value: *b,
                semantic,
            },
            Value::Date(d) => EnhancedValue::Date {
                value: *d,
                semantic,
            },
            Value::List(list) => EnhancedValue::List {
                values: list.iter().map(|v| v.to_enhanced(None)).collect(),
                semantic,
            },
            Value::Map(map) => {
                let enhanced_map = map
                    .iter()
                    .map(|(k, v)| (k.clone(), v.to_enhanced(None)))
                    .collect();
                EnhancedValue::Map {
                    values: enhanced_map,
                    semantic,
                }
            }
            Value::MultiValue(multi) => EnhancedValue::MultiValue {
                values: multi.clone(),
                semantic,
            },
            Value::Null => EnhancedValue::Null { semantic },
        }
    }
}

impl EnhancedValue {
    /// Extract the underlying value without semantic information
    #[allow(dead_code)]
    pub fn extract_value(&self) -> Value {
        match self {
            EnhancedValue::String { value, .. } => Value::String(value.clone()),
            EnhancedValue::Number { value, .. } => Value::Number(*value),
            EnhancedValue::Integer { value, .. } => Value::Integer(*value),
            EnhancedValue::Boolean { value, .. } => Value::Boolean(*value),
            EnhancedValue::Date { value, .. } => Value::Date(*value),
            EnhancedValue::DateTime { .. } => Value::Null, // No direct mapping in original Value
            EnhancedValue::UUID { .. } => Value::Null,     // No direct mapping in original Value
            EnhancedValue::List { values, .. } => {
                Value::List(values.iter().map(|v| v.extract_value()).collect())
            }
            EnhancedValue::Map { values, .. } => {
                let map = values
                    .iter()
                    .map(|(k, v)| (k.clone(), v.extract_value()))
                    .collect();
                Value::Map(map)
            }
            EnhancedValue::MultiValue { values, .. } => Value::MultiValue(values.clone()),
            EnhancedValue::Null { .. } => Value::Null,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn get_semantic_info(&self) -> Option<&SemanticInfo> {
        match self {
            EnhancedValue::String { semantic, .. }
            | EnhancedValue::Number { semantic, .. }
            | EnhancedValue::Integer { semantic, .. }
            | EnhancedValue::Boolean { semantic, .. }
            | EnhancedValue::Date { semantic, .. }
            | EnhancedValue::DateTime { semantic, .. }
            | EnhancedValue::UUID { semantic, .. }
            | EnhancedValue::List { semantic, .. }
            | EnhancedValue::Map { semantic, .. }
            | EnhancedValue::MultiValue { semantic, .. }
            | EnhancedValue::Null { semantic, .. } => semantic.as_ref(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn set_semantic_info(&mut self, new_semantic: SemanticInfo) {
        let semantic_ref = match self {
            EnhancedValue::String { semantic, .. }
            | EnhancedValue::Number { semantic, .. }
            | EnhancedValue::Integer { semantic, .. }
            | EnhancedValue::Boolean { semantic, .. }
            | EnhancedValue::Date { semantic, .. }
            | EnhancedValue::DateTime { semantic, .. }
            | EnhancedValue::UUID { semantic, .. }
            | EnhancedValue::List { semantic, .. }
            | EnhancedValue::Map { semantic, .. }
            | EnhancedValue::MultiValue { semantic, .. }
            | EnhancedValue::Null { semantic, .. } => semantic,
        };
        *semantic_ref = Some(new_semantic);
    }
}

// duplicate LifecycleState removed during clippy cleanup
