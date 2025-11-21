//! Enhanced type system support for AST values with semantic analysis and database integration

use super::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceLocation {
    pub line: usize,
    pub column: usize,
    pub file: Option<String>,
    pub span: Option<(usize, usize)>,
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

impl Value {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Boolean(b) => Some(*b),
            _ => None,
        }
    }
}
