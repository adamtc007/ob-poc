//! Variable Substitution for Macro Expansion
//!
//! Handles substitution of `${...}` variables in macro expansion templates.
//!
//! ## Variable Syntax
//!
//! - `${arg.<name>}` - Argument value (for non-enum types)
//! - `${arg.<name>.internal}` - Enum's internal token (REQUIRED for enums)
//! - `${scope.<field>}` - Scope context (e.g., `${scope.client_id}`)
//! - `${session.<path>}` - Session state (e.g., `${session.current_structure}`)

use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use thiserror::Error;

use super::schema::MacroArg;

/// Errors during variable substitution
#[derive(Debug, Error)]
pub enum VariableError {
    #[error("Unknown variable root '{root}' in ${{{full}}}. Allowed: arg, scope, session")]
    UnknownRoot { root: String, full: String },

    #[error("Missing argument: {0}")]
    MissingArg(String),

    #[error(
        "Argument '{arg}' is an enum but used as ${{arg.{arg}}} instead of ${{arg.{arg}.internal}}"
    )]
    EnumWithoutInternal { arg: String },

    #[error("Argument '{arg}' is not an enum, cannot use .internal suffix")]
    InternalOnNonEnum { arg: String },

    #[error("Unknown enum key '{key}' for argument '{arg}'")]
    UnknownEnumKey { arg: String, key: String },

    #[error("Missing scope field: {0}")]
    MissingScopeField(String),

    #[error("Missing session field: {0}")]
    MissingSessionField(String),

    #[error("Invalid variable syntax: ${{{0}}}")]
    InvalidSyntax(String),
}

/// Context for variable substitution
#[derive(Debug, Clone, Default)]
pub struct VariableContext {
    /// Argument values (arg name → value as string)
    pub args: HashMap<String, ArgValue>,

    /// Scope context (field name → value)
    pub scope: HashMap<String, String>,

    /// Session state (path → value as JSON)
    pub session: HashMap<String, Value>,
}

/// Argument value with optional enum internal mapping
#[derive(Debug, Clone)]
pub struct ArgValue {
    /// The raw value (UI key for enums, literal for others)
    pub value: String,

    /// For enums: the internal token (e.g., "pe" → "private-equity")
    pub internal: Option<String>,

    /// Whether this is an enum type
    pub is_enum: bool,
}

impl ArgValue {
    /// Create a non-enum argument value
    pub fn literal(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            internal: None,
            is_enum: false,
        }
    }

    /// Create an enum argument value
    pub fn enum_value(key: impl Into<String>, internal: impl Into<String>) -> Self {
        Self {
            value: key.into(),
            internal: Some(internal.into()),
            is_enum: true,
        }
    }
}

impl VariableContext {
    /// Create empty context
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an argument value
    pub fn with_arg(mut self, name: impl Into<String>, value: ArgValue) -> Self {
        self.args.insert(name.into(), value);
        self
    }

    /// Add a scope field
    pub fn with_scope(mut self, field: impl Into<String>, value: impl Into<String>) -> Self {
        self.scope.insert(field.into(), value.into());
        self
    }

    /// Add a session value
    pub fn with_session(mut self, path: impl Into<String>, value: Value) -> Self {
        self.session.insert(path.into(), value);
        self
    }

    /// Build context from macro args and provided values
    pub fn from_macro_args(
        args_spec: &HashMap<String, MacroArg>,
        provided_args: &HashMap<String, String>,
    ) -> Result<Self, VariableError> {
        let mut ctx = Self::new();

        for (name, spec) in args_spec {
            if let Some(value) = provided_args.get(name) {
                if spec.is_enum() {
                    // Look up internal token for enum value
                    let internal = spec.internal_for_key(value).ok_or_else(|| {
                        VariableError::UnknownEnumKey {
                            arg: name.clone(),
                            key: value.clone(),
                        }
                    })?;
                    ctx.args
                        .insert(name.clone(), ArgValue::enum_value(value, internal));
                } else {
                    ctx.args
                        .insert(name.clone(), ArgValue::literal(value.clone()));
                }
            } else if let Some(default) = spec.default_internal() {
                // Use default internal value for enum
                let key = spec.default_enum_key().unwrap_or("");
                ctx.args
                    .insert(name.clone(), ArgValue::enum_value(key, default));
            } else if let Some(default) = &spec.default {
                // Use default value for non-enum
                let value_str = match default {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    _ => default.to_string(),
                };
                ctx.args.insert(name.clone(), ArgValue::literal(value_str));
            }
        }

        Ok(ctx)
    }

    /// Get args as a simple string map (for condition evaluation)
    pub fn args_map(&self) -> HashMap<String, String> {
        self.args
            .iter()
            .map(|(k, v)| {
                // Use internal value for enums, raw value otherwise
                let value = v.internal.as_ref().unwrap_or(&v.value).clone();
                (k.clone(), value)
            })
            .collect()
    }

    /// Get scope as a simple string map (for condition evaluation)
    pub fn scope_map(&self) -> HashMap<String, String> {
        self.scope.clone()
    }

    /// Bind a loop variable for foreach iteration
    pub fn bind_loop_var(&mut self, var_name: &str, value: &str, index: usize) {
        // Bind the main variable
        self.args
            .insert(var_name.to_string(), ArgValue::literal(value));

        // Also bind as scope for nested access patterns
        self.scope.insert(var_name.to_string(), value.to_string());

        // Bind index for iteration tracking
        self.scope
            .insert(format!("{}.index", var_name), index.to_string());
    }
}

/// Substitute all variables in a template string
///
/// # Example
///
/// ```ignore
/// let ctx = VariableContext::new()
///     .with_arg("name", ArgValue::literal("Acme Fund"))
///     .with_arg("structure_type", ArgValue::enum_value("pe", "private-equity"))
///     .with_scope("client_id", "uuid-123");
///
/// let result = substitute_variables(
///     "(cbu.create :name \"${arg.name}\" :kind ${arg.structure_type.internal} :client ${scope.client_id})",
///     &ctx
/// )?;
/// // → "(cbu.create :name \"Acme Fund\" :kind private-equity :client uuid-123)"
/// ```
pub fn substitute_variables(
    template: &str,
    ctx: &VariableContext,
) -> Result<String, VariableError> {
    // Match ${...} patterns
    let var_regex = Regex::new(r"\$\{([^}]+)\}").unwrap();

    let mut result = template.to_string();

    // Find all variables and collect substitutions
    let substitutions: Result<Vec<_>, _> = var_regex
        .captures_iter(template)
        .map(|cap| {
            let full_match = cap.get(0).unwrap().as_str();
            let var_content = &cap[1];
            resolve_variable(var_content, ctx).map(|value| (full_match.to_string(), value))
        })
        .collect();

    // Apply substitutions (or return first error)
    for (pattern, value) in substitutions? {
        result = result.replace(&pattern, &value);
    }

    Ok(result)
}

/// Resolve a single variable (without ${} wrapper)
fn resolve_variable(var: &str, ctx: &VariableContext) -> Result<String, VariableError> {
    let parts: Vec<&str> = var.split('.').collect();

    match parts.first() {
        Some(&"arg") => resolve_arg_variable(&parts[1..], ctx),
        Some(&"scope") => resolve_scope_variable(&parts[1..], ctx),
        Some(&"session") => resolve_session_variable(&parts[1..], ctx),
        Some(other) => Err(VariableError::UnknownRoot {
            root: other.to_string(),
            full: var.to_string(),
        }),
        None => Err(VariableError::InvalidSyntax(var.to_string())),
    }
}

/// Resolve ${arg.<name>} or ${arg.<name>.internal}
fn resolve_arg_variable(parts: &[&str], ctx: &VariableContext) -> Result<String, VariableError> {
    if parts.is_empty() {
        return Err(VariableError::InvalidSyntax("arg.".to_string()));
    }

    let arg_name = parts[0];
    let arg_value = ctx
        .args
        .get(arg_name)
        .ok_or_else(|| VariableError::MissingArg(arg_name.to_string()))?;

    // Check for .internal suffix
    if parts.len() > 1 && parts[1] == "internal" {
        if !arg_value.is_enum {
            return Err(VariableError::InternalOnNonEnum {
                arg: arg_name.to_string(),
            });
        }
        arg_value
            .internal
            .clone()
            .ok_or_else(|| VariableError::MissingArg(format!("{}.internal", arg_name)))
    } else {
        // For enums used without .internal, this is usually an error
        // But we allow it for cases where the UI key is needed
        if arg_value.is_enum && parts.len() == 1 {
            // Return the UI key, but this might be a lint warning
            Ok(arg_value.value.clone())
        } else {
            Ok(arg_value.value.clone())
        }
    }
}

/// Resolve ${scope.<field>}
fn resolve_scope_variable(parts: &[&str], ctx: &VariableContext) -> Result<String, VariableError> {
    if parts.is_empty() {
        return Err(VariableError::InvalidSyntax("scope.".to_string()));
    }

    let field = parts[0];
    ctx.scope
        .get(field)
        .cloned()
        .ok_or_else(|| VariableError::MissingScopeField(field.to_string()))
}

/// Resolve ${session.<path>}
fn resolve_session_variable(
    parts: &[&str],
    ctx: &VariableContext,
) -> Result<String, VariableError> {
    if parts.is_empty() {
        return Err(VariableError::InvalidSyntax("session.".to_string()));
    }

    // Join remaining parts as path (e.g., "current_structure" or "client.id")
    let path = parts.join(".");

    ctx.session
        .get(&path)
        .map(|v| match v {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => "null".to_string(),
            _ => v.to_string(),
        })
        .ok_or(VariableError::MissingSessionField(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_substitute_arg_literal() {
        let ctx = VariableContext::new().with_arg("name", ArgValue::literal("Acme Fund"));

        let result = substitute_variables("(cbu.create :name \"${arg.name}\")", &ctx).unwrap();
        assert_eq!(result, "(cbu.create :name \"Acme Fund\")");
    }

    #[test]
    fn test_substitute_arg_enum_internal() {
        let ctx =
            VariableContext::new().with_arg("type", ArgValue::enum_value("pe", "private-equity"));

        let result = substitute_variables("(cbu.create :kind ${arg.type.internal})", &ctx).unwrap();
        assert_eq!(result, "(cbu.create :kind private-equity)");
    }

    #[test]
    fn test_substitute_scope() {
        let ctx = VariableContext::new().with_scope("client_id", "uuid-123-456");

        let result =
            substitute_variables("(cbu.create :client_id ${scope.client_id})", &ctx).unwrap();
        assert_eq!(result, "(cbu.create :client_id uuid-123-456)");
    }

    #[test]
    fn test_substitute_session() {
        let ctx = VariableContext::new()
            .with_session("current_structure", Value::String("uuid-struct-1".into()));

        let result = substitute_variables(
            "(kyc-case.create :cbu_id ${session.current_structure})",
            &ctx,
        )
        .unwrap();
        assert_eq!(result, "(kyc-case.create :cbu_id uuid-struct-1)");
    }

    #[test]
    fn test_substitute_multiple() {
        let ctx = VariableContext::new()
            .with_arg("name", ArgValue::literal("Acme Fund"))
            .with_arg("type", ArgValue::enum_value("pe", "private-equity"))
            .with_scope("client_id", "uuid-client");

        let template =
            "(cbu.create :name \"${arg.name}\" :kind ${arg.type.internal} :client ${scope.client_id})";
        let result = substitute_variables(template, &ctx).unwrap();
        assert_eq!(
            result,
            "(cbu.create :name \"Acme Fund\" :kind private-equity :client uuid-client)"
        );
    }

    #[test]
    fn test_error_missing_arg() {
        let ctx = VariableContext::new();

        let result = substitute_variables("${arg.missing}", &ctx);
        assert!(matches!(result, Err(VariableError::MissingArg(_))));
    }

    #[test]
    fn test_error_internal_on_non_enum() {
        let ctx = VariableContext::new().with_arg("name", ArgValue::literal("test"));

        let result = substitute_variables("${arg.name.internal}", &ctx);
        assert!(matches!(
            result,
            Err(VariableError::InternalOnNonEnum { .. })
        ));
    }

    #[test]
    fn test_error_unknown_root() {
        let ctx = VariableContext::new();

        let result = substitute_variables("${unknown.field}", &ctx);
        assert!(matches!(result, Err(VariableError::UnknownRoot { .. })));
    }
}
