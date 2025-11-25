//! Raw AST types from parser (before validation).

use crate::forth_engine::schema::ast::span::Span;
use crate::forth_engine::schema::types::{VerbDef, ArgSpec};

/// Raw AST from parser (unvalidated).
#[derive(Debug, Clone)]
pub struct RawAst {
    pub expressions: Vec<RawExpr>,
}

/// Raw expression with source span.
#[derive(Debug, Clone)]
pub struct RawExpr {
    pub span: Span,
    pub kind: RawExprKind,
}

/// Kind of raw expression.
#[derive(Debug, Clone)]
pub enum RawExprKind {
    /// Verb call with arguments
    Call {
        name: String,
        name_span: Span,
        args: Vec<RawArg>,
        /// Verb definition attached at parse time (None if unknown verb)
        verb_def: Option<&'static VerbDef>,
    },
    /// Comment
    Comment(String),
}

/// Raw argument from parser.
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

/// Raw value from parser (untyped).
#[derive(Debug, Clone, PartialEq)]
pub enum RawValue {
    /// String literal
    String(String),
    /// Integer literal
    Int(i64),
    /// Float literal
    Float(f64),
    /// Boolean literal
    Bool(bool),
    /// Symbol reference (@name)
    Symbol(String),
    /// Keyword (:keyword)
    Keyword(String),
    /// List of values
    List(Vec<RawValue>),
    /// Map of key-value pairs
    Map(Vec<(String, RawValue)>),
}

impl RawAst {
    /// Create a new empty AST.
    pub fn new() -> Self {
        Self { expressions: Vec::new() }
    }

    /// Add an expression.
    pub fn push(&mut self, expr: RawExpr) {
        self.expressions.push(expr);
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

impl Default for RawAst {
    fn default() -> Self {
        Self::new()
    }
}

impl RawExpr {
    /// Create a new call expression.
    pub fn call(
        name: String, 
        name_span: Span, 
        args: Vec<RawArg>, 
        span: Span,
        verb_def: Option<&'static VerbDef>,
    ) -> Self {
        Self {
            span,
            kind: RawExprKind::Call { name, name_span, args, verb_def },
        }
    }

    /// Create a comment expression.
    pub fn comment(text: String, span: Span) -> Self {
        Self {
            span,
            kind: RawExprKind::Comment(text),
        }
    }

    /// Check if this is a call.
    pub fn is_call(&self) -> bool {
        matches!(self.kind, RawExprKind::Call { .. })
    }

    /// Get the verb name if this is a call.
    pub fn verb_name(&self) -> Option<&str> {
        match &self.kind {
            RawExprKind::Call { name, .. } => Some(name),
            _ => None,
        }
    }
}

impl RawValue {
    /// Get as string if this is a string value.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            RawValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// Get as symbol name if this is a symbol.
    pub fn as_symbol(&self) -> Option<&str> {
        match self {
            RawValue::Symbol(s) => Some(s),
            _ => None,
        }
    }

    /// Get as integer if this is an int.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            RawValue::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// Get as float if this is a float (or int coerced).
    pub fn as_float(&self) -> Option<f64> {
        match self {
            RawValue::Float(f) => Some(*f),
            RawValue::Int(i) => Some(*i as f64),
            _ => None,
        }
    }

    /// Get as bool if this is a boolean.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            RawValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Get type name for error messages.
    pub fn type_name(&self) -> &'static str {
        match self {
            RawValue::String(_) => "string",
            RawValue::Int(_) => "integer",
            RawValue::Float(_) => "float",
            RawValue::Bool(_) => "boolean",
            RawValue::Symbol(_) => "symbol",
            RawValue::Keyword(_) => "keyword",
            RawValue::List(_) => "list",
            RawValue::Map(_) => "map",
        }
    }
}
