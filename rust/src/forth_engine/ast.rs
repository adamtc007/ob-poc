//! AST (Abstract Syntax Tree) for the DSL Forth Engine.

use crate::forth_engine::errors::EngineError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslSheet {
    pub id: String,
    pub domain: String,
    pub version: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    WordCall { name: String, args: Vec<Expr> },
    StringLiteral(String),
    IntegerLiteral(i64),
    FloatLiteral(f64),
    BoolLiteral(bool),
    Keyword(String),            // :case-id, :case-type, etc.
    DottedKeyword(Vec<String>), // :customer.id -> ["customer", "id"]
    AttributeRef(String),       // @attr{uuid}
    DocumentRef(String),        // @doc{uuid}
    ListLiteral(Vec<Expr>),
    MapLiteral(Vec<(String, Expr)>), // {:key value :key2 value2}
    Comment(String),            // ;; comment text
}

pub trait DslParser {
    fn parse(&self, input: &str) -> Result<Vec<Expr>, EngineError>;
}
