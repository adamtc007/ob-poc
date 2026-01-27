//! Canonicalizer - normalize parsed expressions to canonical keyword form
//!
//! Transforms ParsedExpr into CanonicalAst which:
//! - Uses fully qualified verb names
//! - All arguments in keyword form
//! - Values normalized to appropriate types
//! - Can be serialized back to s-expr syntax

use super::parser::{ParseError, ParsedExpr, ParsedValue};
use super::registry::VerbRegistry;
use super::tokenizer::Span;
use super::types::ArgShape;
use serde::{Deserialize, Serialize};

/// Canonical AST - normalized form ready for execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalAst {
    /// Fully qualified verb name
    pub verb: String,

    /// Arguments in keyword form
    pub args: Vec<CanonicalArg>,

    /// Source span (for error reporting)
    #[serde(skip)]
    pub span: Span,

    /// Entity references that need resolution
    #[serde(default)]
    pub unresolved_entities: Vec<UnresolvedEntity>,

    /// Binding references used
    #[serde(default)]
    pub binding_refs: Vec<String>,

    /// Binding capture (if :as specified)
    #[serde(default)]
    pub capture_as: Option<String>,
}

/// Canonical argument
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalArg {
    /// Argument name (kebab-case)
    pub name: String,

    /// Canonical value
    pub value: CanonicalValue,

    /// Source span
    #[serde(skip)]
    pub span: Span,
}

/// Canonical value types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CanonicalValue {
    String(String),
    Integer(i64),
    Float(f64),
    Bool(bool),
    Uuid(String),
    EntityRef {
        name: String,
        resolved_id: Option<String>,
    },
    BindingRef(String),
    List(Vec<CanonicalValue>),
    Null,
}

impl CanonicalValue {
    /// Convert to JSON value
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            CanonicalValue::String(s) => serde_json::json!(s),
            CanonicalValue::Integer(i) => serde_json::json!(i),
            CanonicalValue::Float(f) => serde_json::json!(f),
            CanonicalValue::Bool(b) => serde_json::json!(b),
            CanonicalValue::Uuid(u) => serde_json::json!(u),
            CanonicalValue::EntityRef { name, resolved_id } => {
                if let Some(id) = resolved_id {
                    serde_json::json!(id)
                } else {
                    serde_json::json!({ "entity_ref": name })
                }
            }
            CanonicalValue::BindingRef(b) => serde_json::json!({ "binding_ref": b }),
            CanonicalValue::List(items) => {
                serde_json::Value::Array(items.iter().map(|v| v.to_json()).collect())
            }
            CanonicalValue::Null => serde_json::Value::Null,
        }
    }
}

/// Unresolved entity reference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnresolvedEntity {
    /// Entity name from input
    pub name: String,

    /// Argument it was used for
    pub arg_name: String,

    /// Source span
    #[serde(skip)]
    pub span: Span,
}

/// Canonicalize a parsed expression
pub fn canonicalize(
    parsed: &ParsedExpr,
    _registry: &VerbRegistry,
) -> Result<CanonicalAst, ParseError> {
    let spec = &parsed.spec;
    let mut args = Vec::new();
    let mut unresolved_entities = Vec::new();
    let mut binding_refs = Vec::new();
    let mut capture_as = None;

    // Process provided arguments
    for (name, value) in &parsed.args {
        // Handle special :as argument for binding capture
        if name == "as" {
            if let ParsedValue::BindingRef(b, _) = value {
                capture_as = Some(b.clone());
                continue;
            }
        }

        let arg_def = spec.args.get(name);
        let shape = arg_def.map(|d| d.shape.clone()).unwrap_or(ArgShape::Str);

        let (canonical_value, entities, bindings) = canonicalize_value(value, &shape, name)?;

        unresolved_entities.extend(entities);
        binding_refs.extend(bindings);

        args.push(CanonicalArg {
            name: name.clone(),
            value: canonical_value,
            span: value.span(),
        });
    }

    // Add default values for missing optional arguments
    for arg_def in spec.args.optional.iter() {
        if !parsed.args.contains_key(&arg_def.name) {
            if let Some(default) = &arg_def.default {
                let canonical_value = json_to_canonical(default);
                args.push(CanonicalArg {
                    name: arg_def.name.clone(),
                    value: canonical_value,
                    span: Span::default(),
                });
            }
        }
    }

    // Validate required arguments are present
    for arg_def in &spec.args.required {
        if !parsed.args.contains_key(&arg_def.name) {
            return Err(ParseError {
                message: format!("Missing required argument: '{}'", arg_def.name),
                span: parsed.span,
                expected: vec![format!(":{}", arg_def.name)],
                suggestions: vec![],
            });
        }
    }

    Ok(CanonicalAst {
        verb: parsed.verb_fqn.clone(),
        args,
        span: parsed.span,
        unresolved_entities,
        binding_refs,
        capture_as,
    })
}

/// Canonicalize a single value
fn canonicalize_value(
    value: &ParsedValue,
    shape: &ArgShape,
    arg_name: &str,
) -> Result<(CanonicalValue, Vec<UnresolvedEntity>, Vec<String>), ParseError> {
    let mut entities = Vec::new();
    let mut bindings = Vec::new();

    let canonical = match value {
        ParsedValue::String(s, _) => {
            match shape {
                ArgShape::Uuid => CanonicalValue::Uuid(s.clone()),
                ArgShape::Int => {
                    if let Ok(i) = s.parse::<i64>() {
                        CanonicalValue::Integer(i)
                    } else {
                        CanonicalValue::String(s.clone())
                    }
                }
                ArgShape::Bool => match s.to_lowercase().as_str() {
                    "true" | "yes" | "1" => CanonicalValue::Bool(true),
                    "false" | "no" | "0" => CanonicalValue::Bool(false),
                    _ => CanonicalValue::String(s.clone()),
                },
                ArgShape::Enum { values } => {
                    // Normalize enum case
                    let normalized = values
                        .iter()
                        .find(|v| v.to_uppercase() == s.to_uppercase())
                        .cloned()
                        .unwrap_or_else(|| s.clone());
                    CanonicalValue::String(normalized)
                }
                _ => CanonicalValue::String(s.clone()),
            }
        }
        ParsedValue::Integer(i, _) => CanonicalValue::Integer(*i),
        ParsedValue::Float(f, _) => CanonicalValue::Float(*f),
        ParsedValue::Bool(b, _) => CanonicalValue::Bool(*b),
        ParsedValue::EntityRef(name, span) => {
            entities.push(UnresolvedEntity {
                name: name.clone(),
                arg_name: arg_name.to_string(),
                span: *span,
            });
            CanonicalValue::EntityRef {
                name: name.clone(),
                resolved_id: None,
            }
        }
        ParsedValue::BindingRef(name, _) => {
            bindings.push(name.clone());
            CanonicalValue::BindingRef(name.clone())
        }
        ParsedValue::List(items, _) => {
            let item_shape = match shape {
                ArgShape::List { item } => item.as_ref().clone(),
                _ => ArgShape::Str,
            };

            let mut canonical_items = Vec::new();
            for item in items {
                let (cv, ents, binds) = canonicalize_value(item, &item_shape, arg_name)?;
                canonical_items.push(cv);
                entities.extend(ents);
                bindings.extend(binds);
            }
            CanonicalValue::List(canonical_items)
        }
        ParsedValue::Null(_) => CanonicalValue::Null,
    };

    Ok((canonical, entities, bindings))
}

/// Convert JSON value to canonical value
fn json_to_canonical(json: &serde_json::Value) -> CanonicalValue {
    match json {
        serde_json::Value::Null => CanonicalValue::Null,
        serde_json::Value::Bool(b) => CanonicalValue::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                CanonicalValue::Integer(i)
            } else if let Some(f) = n.as_f64() {
                CanonicalValue::Float(f)
            } else {
                CanonicalValue::String(n.to_string())
            }
        }
        serde_json::Value::String(s) => CanonicalValue::String(s.clone()),
        serde_json::Value::Array(arr) => {
            CanonicalValue::List(arr.iter().map(json_to_canonical).collect())
        }
        serde_json::Value::Object(_) => {
            // JSON objects become strings (serialized)
            CanonicalValue::String(json.to_string())
        }
    }
}

impl CanonicalAst {
    /// Convert back to s-expression string
    pub fn to_sexpr(&self) -> String {
        let mut parts = vec![format!("({}", self.verb)];

        for arg in &self.args {
            parts.push(format!(":{}", arg.name));
            parts.push(value_to_sexpr(&arg.value));
        }

        if let Some(capture) = &self.capture_as {
            parts.push(":as".to_string());
            parts.push(format!("@{}", capture));
        }

        parts.push(")".to_string());
        parts.join(" ")
    }

    /// Get argument value by name
    pub fn get_arg(&self, name: &str) -> Option<&CanonicalValue> {
        self.args.iter().find(|a| a.name == name).map(|a| &a.value)
    }

    /// Check if all entity references are resolved
    pub fn is_resolved(&self) -> bool {
        self.unresolved_entities.is_empty()
    }

    /// Get argument as string
    pub fn get_string(&self, name: &str) -> Option<String> {
        match self.get_arg(name)? {
            CanonicalValue::String(s) => Some(s.clone()),
            CanonicalValue::Uuid(u) => Some(u.clone()),
            CanonicalValue::Integer(i) => Some(i.to_string()),
            CanonicalValue::Float(f) => Some(f.to_string()),
            CanonicalValue::Bool(b) => Some(b.to_string()),
            _ => None,
        }
    }

    /// Get argument as integer
    pub fn get_int(&self, name: &str) -> Option<i64> {
        match self.get_arg(name)? {
            CanonicalValue::Integer(i) => Some(*i),
            CanonicalValue::String(s) => s.parse().ok(),
            _ => None,
        }
    }

    /// Get argument as boolean
    pub fn get_bool(&self, name: &str) -> Option<bool> {
        match self.get_arg(name)? {
            CanonicalValue::Bool(b) => Some(*b),
            CanonicalValue::String(s) => match s.to_lowercase().as_str() {
                "true" | "yes" | "1" => Some(true),
                "false" | "no" | "0" => Some(false),
                _ => None,
            },
            _ => None,
        }
    }
}

/// Convert canonical value to s-expr string
fn value_to_sexpr(value: &CanonicalValue) -> String {
    match value {
        CanonicalValue::String(s) => {
            // Quote strings that need it
            if s.contains(' ') || s.contains('"') || s.is_empty() {
                format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
            } else {
                format!("\"{}\"", s)
            }
        }
        CanonicalValue::Integer(i) => i.to_string(),
        CanonicalValue::Float(f) => f.to_string(),
        CanonicalValue::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        CanonicalValue::Uuid(u) => format!("\"{}\"", u),
        CanonicalValue::EntityRef { name, resolved_id } => {
            if let Some(id) = resolved_id {
                format!("\"{}\"", id)
            } else {
                format!("<{}>", name)
            }
        }
        CanonicalValue::BindingRef(b) => format!("@{}", b),
        CanonicalValue::List(items) => {
            let inner: Vec<String> = items.iter().map(value_to_sexpr).collect();
            format!("({})", inner.join(" "))
        }
        CanonicalValue::Null => "nil".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::schema::parser::parse;
    use crate::mcp::schema::registry::VerbRegistry;
    use crate::mcp::schema::types::*;
    use std::collections::HashMap;

    fn test_registry() -> VerbRegistry {
        let mut registry = VerbRegistry::new();

        registry.register(VerbSpec {
            name: "view.drill".to_string(),
            domain: "view".to_string(),
            action: "drill".to_string(),
            aliases: vec!["drill".to_string()],
            args: ArgSchema {
                style: "keyworded".to_string(),
                required: vec![ArgDef {
                    name: "entity".to_string(),
                    shape: ArgShape::EntityRef {
                        allowed_kinds: vec![],
                    },
                    default: None,
                    doc: String::new(),
                    maps_to: None,
                    lookup: None,
                }],
                optional: vec![ArgDef {
                    name: "depth".to_string(),
                    shape: ArgShape::Int,
                    default: Some(serde_json::json!(1)),
                    doc: String::new(),
                    maps_to: None,
                    lookup: None,
                }],
            },
            positional_sugar: vec!["entity".to_string()],
            keyword_aliases: HashMap::new(),
            doc: String::new(),
            tier: "intent".to_string(),
            tags: vec![],
            ..Default::default()
        });

        registry
    }

    #[test]
    fn test_canonicalize_basic() {
        let registry = test_registry();
        let parsed = parse("(view.drill :entity \"Allianz\")", &registry).unwrap();
        let canonical = canonicalize(&parsed, &registry).unwrap();

        assert_eq!(canonical.verb, "view.drill");
        assert!(canonical.args.iter().any(|a| a.name == "entity"));
    }

    #[test]
    fn test_canonicalize_entity_ref() {
        let registry = test_registry();
        let parsed = parse("(drill <Allianz SE>)", &registry).unwrap();
        let canonical = canonicalize(&parsed, &registry).unwrap();

        assert_eq!(canonical.unresolved_entities.len(), 1);
        assert_eq!(canonical.unresolved_entities[0].name, "Allianz SE");
    }

    #[test]
    fn test_roundtrip() {
        let registry = test_registry();
        let input = "(view.drill :entity \"test\" :depth 3)";
        let parsed = parse(input, &registry).unwrap();
        let canonical = canonicalize(&parsed, &registry).unwrap();
        let sexpr = canonical.to_sexpr();

        // Parse the output again
        let reparsed = parse(&sexpr, &registry).unwrap();
        let recanonical = canonicalize(&reparsed, &registry).unwrap();

        assert_eq!(canonical.verb, recanonical.verb);
        assert_eq!(canonical.args.len(), recanonical.args.len());
    }

    #[test]
    fn test_default_values() {
        let registry = test_registry();
        let parsed = parse("(drill :entity \"X\")", &registry).unwrap();
        let canonical = canonicalize(&parsed, &registry).unwrap();

        // Should have depth with default value
        let depth = canonical.get_int("depth");
        assert_eq!(depth, Some(1));
    }
}
