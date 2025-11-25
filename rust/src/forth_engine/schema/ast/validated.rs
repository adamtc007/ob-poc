//! Validated AST types (after schema validation).

use std::collections::HashMap;
use uuid::Uuid;
use chrono::NaiveDate;
use crate::forth_engine::schema::ast::span::Span;
use crate::forth_engine::schema::ast::symbols::SymbolTable;
use crate::forth_engine::schema::types::{VerbDef, RefType};

/// Validated AST (after schema validation passes).
#[derive(Debug, Clone)]
pub struct ValidatedAst {
    pub expressions: Vec<ValidatedExpr>,
    pub symbol_table: SymbolTable,
}

/// Validated expression.
#[derive(Debug, Clone)]
pub struct ValidatedExpr {
    pub span: Span,
    pub kind: ValidatedExprKind,
}

/// Kind of validated expression.
#[derive(Debug, Clone)]
pub enum ValidatedExprKind {
    /// Validated verb call
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
    /// Comment (preserved for formatting)
    Comment(String),
}

/// Typed value after validation.
#[derive(Debug, Clone, PartialEq)]
pub enum TypedValue {
    /// String value
    String(String),
    /// UUID value (parsed and validated)
    Uuid(Uuid),
    /// Integer value
    Integer(i64),
    /// Decimal value
    Decimal(f64),
    /// Date value (parsed and validated)
    Date(NaiveDate),
    /// Boolean value
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

impl ValidatedAst {
    /// Create a new empty validated AST.
    pub fn new() -> Self {
        Self {
            expressions: Vec::new(),
            symbol_table: SymbolTable::new(),
        }
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.expressions.is_empty()
    }

    /// Get number of expressions.
    pub fn len(&self) -> usize {
        self.expressions.len()
    }
}

impl Default for ValidatedAst {
    fn default() -> Self {
        Self::new()
    }
}

impl TypedValue {
    /// Get as string if applicable.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            Self::Enum(s) => Some(s),
            Self::Ref { code, .. } => Some(code),
            _ => None,
        }
    }

    /// Get as UUID if applicable.
    pub fn as_uuid(&self) -> Option<Uuid> {
        match self {
            Self::Uuid(u) => Some(*u),
            Self::Symbol { resolved_id: Some(id), .. } => Some(*id),
            _ => None,
        }
    }

    /// Get as integer if applicable.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Self::Integer(i) => Some(*i),
            _ => None,
        }
    }

    /// Get as decimal if applicable.
    pub fn as_decimal(&self) -> Option<f64> {
        match self {
            Self::Decimal(d) => Some(*d),
            Self::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }

    /// Get as date if applicable.
    pub fn as_date(&self) -> Option<NaiveDate> {
        match self {
            Self::Date(d) => Some(*d),
            _ => None,
        }
    }

    /// Get as boolean if applicable.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// Get symbol name if this is a symbol.
    pub fn as_symbol(&self) -> Option<&str> {
        match self {
            Self::Symbol { name, .. } => Some(name),
            _ => None,
        }
    }

    /// Get ref code if this is a reference.
    pub fn as_ref_code(&self) -> Option<&str> {
        match self {
            Self::Ref { code, .. } => Some(code),
            _ => None,
        }
    }

    /// Get type name for error messages.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::String(_) => "string",
            Self::Uuid(_) => "uuid",
            Self::Integer(_) => "integer",
            Self::Decimal(_) => "decimal",
            Self::Date(_) => "date",
            Self::Boolean(_) => "boolean",
            Self::Symbol { .. } => "symbol",
            Self::Ref { .. } => "reference",
            Self::Enum(_) => "enum",
            Self::List(_) => "list",
            Self::Map(_) => "map",
        }
    }
}
