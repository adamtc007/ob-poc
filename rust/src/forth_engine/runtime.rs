//! Direct AST Runtime for DSL Execution
//!
//! Replaces the Forth stack machine with direct interpretation.
//! S-expressions with named arguments don't need stack threading.

use crate::forth_engine::ast::Expr;
use crate::forth_engine::env::RuntimeEnv;
use crate::forth_engine::errors::EngineError;
use crate::forth_engine::value::{AttributeId, DocumentId, Value};
use std::collections::HashMap;

/// Parsed argument: keyword + value pair
#[derive(Debug, Clone)]
pub struct Arg {
    pub key: String,
    pub value: Value,
}

/// Word function signature - receives args directly, no stack
pub type WordFn = fn(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError>;

/// Word entry with metadata for RAG/agent context
pub struct WordEntry {
    pub name: &'static str,
    pub domain: &'static str,
    pub func: WordFn,
    pub signature: &'static str,
    pub description: &'static str,
    pub examples: &'static [&'static str],
}

/// Direct AST runtime - no stack machine
pub struct Runtime {
    vocab: HashMap<&'static str, WordEntry>,
}

impl Runtime {
    /// Create runtime with registered vocabulary
    pub fn new(words: Vec<WordEntry>) -> Self {
        let mut vocab = HashMap::new();
        for entry in words {
            vocab.insert(entry.name, entry);
        }
        Self { vocab }
    }

    /// Execute a DSL sheet (multiple expressions)
    pub fn execute_sheet(&self, exprs: &[Expr], env: &mut RuntimeEnv) -> Result<(), EngineError> {
        for expr in exprs {
            self.execute_expr(expr, env)?;
        }
        Ok(())
    }

    /// Execute a single expression
    fn execute_expr(&self, expr: &Expr, env: &mut RuntimeEnv) -> Result<(), EngineError> {
        match expr {
            Expr::WordCall { name, args } => {
                let entry = self
                    .vocab
                    .get(name.as_str())
                    .ok_or_else(|| EngineError::UnknownWord(name.clone()))?;

                let parsed_args = self.extract_args(args)?;
                (entry.func)(&parsed_args, env)
            }
            Expr::Comment(_) => Ok(()), // Skip comments
            _ => Err(EngineError::Parse(format!(
                "Top-level expression must be a word call, got: {:?}",
                expr
            ))),
        }
    }

    /// Extract keyword-value pairs from argument list
    fn extract_args(&self, args: &[Expr]) -> Result<Vec<Arg>, EngineError> {
        let mut result = Vec::new();
        let mut iter = args.iter().peekable();

        while let Some(expr) = iter.next() {
            match expr {
                Expr::Keyword(key) => {
                    let value_expr = iter.next().ok_or_else(|| {
                        EngineError::Parse(format!("Keyword {} missing value", key))
                    })?;
                    result.push(Arg {
                        key: key.clone(),
                        value: self.expr_to_value(value_expr)?,
                    });
                }
                // Allow non-keyword args for positional parameters (rare)
                _ => {
                    result.push(Arg {
                        key: format!("_pos_{}", result.len()),
                        value: self.expr_to_value(expr)?,
                    });
                }
            }
        }
        Ok(result)
    }

    /// Convert AST Expr to runtime Value
    fn expr_to_value(&self, expr: &Expr) -> Result<Value, EngineError> {
        match expr {
            Expr::StringLiteral(s) => Ok(Value::Str(s.clone())),
            Expr::IntegerLiteral(i) => Ok(Value::Int(*i)),
            Expr::FloatLiteral(f) => Ok(Value::Float(*f)),
            Expr::BoolLiteral(b) => Ok(Value::Bool(*b)),
            Expr::Keyword(k) => Ok(Value::Keyword(k.clone())),
            Expr::DottedKeyword(parts) => Ok(Value::DottedKeyword(parts.clone())),
            Expr::AttributeRef(id) => Ok(Value::Attr(AttributeId(id.clone()))),
            Expr::DocumentRef(id) => Ok(Value::Doc(DocumentId(id.clone()))),
            Expr::ListLiteral(items) => {
                let values: Result<Vec<_>, _> =
                    items.iter().map(|e| self.expr_to_value(e)).collect();
                Ok(Value::List(values?))
            }
            Expr::MapLiteral(pairs) => {
                let mut converted = Vec::new();
                for (k, v) in pairs {
                    converted.push((k.clone(), self.expr_to_value(v)?));
                }
                Ok(Value::Map(converted))
            }
            Expr::WordCall { .. } => {
                // Nested word calls - for now, error
                // Could support if words return values
                Err(EngineError::Parse(
                    "Nested word calls not yet supported as values".into(),
                ))
            }
            Expr::Comment(_) => Err(EngineError::Parse("Comment cannot be used as value".into())),
        }
    }

    /// Get word entry for RAG context building
    pub fn get_word(&self, name: &str) -> Option<&WordEntry> {
        self.vocab.get(name)
    }

    /// Get all words in a domain for RAG context
    pub fn get_domain_words(&self, domain: &str) -> Vec<&WordEntry> {
        self.vocab.values().filter(|w| w.domain == domain).collect()
    }

    /// Get all domains
    pub fn get_domains(&self) -> Vec<&'static str> {
        let mut domains: Vec<_> = self.vocab.values().map(|w| w.domain).collect();
        domains.sort();
        domains.dedup();
        domains
    }

    /// Get all word names (for validation)
    pub fn get_all_word_names(&self) -> Vec<&'static str> {
        self.vocab.keys().copied().collect()
    }
}

/// Helper trait for argument extraction
pub trait ArgList {
    fn require_string(&self, key: &str) -> Result<String, EngineError>;
    fn get_string(&self, key: &str) -> Option<String>;
    fn require_int(&self, key: &str) -> Result<i64, EngineError>;
    fn get_int(&self, key: &str) -> Option<i64>;
    fn require_uuid(&self, key: &str) -> Result<uuid::Uuid, EngineError>;
    fn get_uuid(&self, key: &str) -> Option<uuid::Uuid>;
    fn get_list(&self, key: &str) -> Option<Vec<Value>>;
    fn get_value(&self, key: &str) -> Option<&Value>;
}

impl ArgList for [Arg] {
    fn require_string(&self, key: &str) -> Result<String, EngineError> {
        self.get_string(key)
            .ok_or_else(|| EngineError::MissingArgument(key.into()))
    }

    fn get_string(&self, key: &str) -> Option<String> {
        self.iter()
            .find(|a| a.key == key)
            .and_then(|a| match &a.value {
                Value::Str(s) => Some(s.clone()),
                _ => None,
            })
    }

    fn require_int(&self, key: &str) -> Result<i64, EngineError> {
        self.get_int(key)
            .ok_or_else(|| EngineError::MissingArgument(key.into()))
    }

    fn get_int(&self, key: &str) -> Option<i64> {
        self.iter()
            .find(|a| a.key == key)
            .and_then(|a| match &a.value {
                Value::Int(i) => Some(*i),
                _ => None,
            })
    }

    fn require_uuid(&self, key: &str) -> Result<uuid::Uuid, EngineError> {
        let s = self.require_string(key)?;
        uuid::Uuid::parse_str(&s)
            .map_err(|e| EngineError::Parse(format!("Invalid UUID for {}: {}", key, e)))
    }

    fn get_uuid(&self, key: &str) -> Option<uuid::Uuid> {
        self.get_string(key)
            .and_then(|s| uuid::Uuid::parse_str(&s).ok())
    }

    fn get_list(&self, key: &str) -> Option<Vec<Value>> {
        self.iter()
            .find(|a| a.key == key)
            .and_then(|a| match &a.value {
                Value::List(items) => Some(items.clone()),
                _ => None,
            })
    }

    fn get_value(&self, key: &str) -> Option<&Value> {
        self.iter().find(|a| a.key == key).map(|a| &a.value)
    }
}
