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
    BoolLiteral(bool),
    Keyword(String), // :case-id, :case-type, etc.
    AttributeRef(String),
    DocumentRef(String),
}

pub trait DslParser {
    fn parse(&self, input: &str) -> Result<Vec<Expr>, EngineError>;
}
