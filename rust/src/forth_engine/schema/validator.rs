//! Schema validator for DSL expressions.

use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;
use chrono::{NaiveDate, Local};

use crate::forth_engine::schema::types::*;
use crate::forth_engine::schema::ast::*;
use crate::forth_engine::schema::cache::SchemaCache;
use crate::forth_engine::schema::registry::VERB_REGISTRY;
use crate::forth_engine::schema::validation_errors::*;

/// Schema validator for DSL expressions.
pub struct SchemaValidator {
    schema_cache: Arc<SchemaCache>,
}

/// Result of validating arguments.
struct ValidatedArgs {
    args: HashMap<String, TypedValue>,
    context_injected: Vec<String>,
    defaulted: Vec<String>,
}

impl SchemaValidator {
    /// Create a new validator with a schema cache.
    pub fn new(schema_cache: Arc<SchemaCache>) -> Self {
        Self { schema_cache }
    }

    /// Validate raw AST against verb schemas.
    pub fn validate(
        &self,
        raw: &RawAst,
        context: &ValidationContext,
    ) -> Result<ValidatedAst, ValidationReport> {
        let mut validated_exprs = Vec::new();
        let mut symbol_table = SymbolTable::new();
        let mut errors = Vec::new();

        for expr in &raw.expressions {
            match &expr.kind {
                RawExprKind::Call { name, name_span, args, verb_def } => {
                    // 1. Get verb schema (use attached or lookup)
                    let verb = match verb_def {
                        Some(v) => *v,
                        None => match VERB_REGISTRY.get(name) {
                            Some(v) => v,
                            None => {
                                errors.push(ValidationError {
                                    span: *name_span,
                                    kind: ErrorKind::UnknownVerb {
                                        name: name.clone(),
                                        suggestions: VERB_REGISTRY.suggest(name),
                                    },
                                });
                                continue;
                            }
                        },
                    };

                    // 2. Validate arguments
                    match self.validate_args(verb, args, context, &symbol_table) {
                        Ok(validated_args) => {
                            // 3. Check for symbol definition (:as @name)
                            let defines_symbol = validated_args.args
                                .get(":as")
                                .and_then(|v| match v {
                                    TypedValue::Symbol { name, .. } => Some(name.clone()),
                                    _ => None,
                                });

                            // 4. Update symbol table
                            if let Some(ref sym_name) = defines_symbol {
                                if let Some(produces) = &verb.produces {
                                    if let Err(e) = symbol_table.define(
                                        sym_name,
                                        produces.capture_as,
                                        expr.span,
                                        verb.name,
                                    ) {
                                        errors.push(ValidationError {
                                            span: expr.span,
                                            kind: ErrorKind::SymbolError(e),
                                        });
                                    }
                                }
                            }

                            validated_exprs.push(ValidatedExpr {
                                span: expr.span,
                                kind: ValidatedExprKind::VerbCall {
                                    verb,
                                    args: validated_args.args,
                                    context_injected: validated_args.context_injected,
                                    defaulted: validated_args.defaulted,
                                    defines_symbol,
                                },
                            });
                        }
                        Err(arg_errors) => {
                            errors.extend(arg_errors);
                        }
                    }
                }

                RawExprKind::Comment(c) => {
                    validated_exprs.push(ValidatedExpr {
                        span: expr.span,
                        kind: ValidatedExprKind::Comment(c.clone()),
                    });
                }
            }
        }

        if errors.is_empty() {
            Ok(ValidatedAst {
                expressions: validated_exprs,
                symbol_table,
            })
        } else {
            Err(ValidationReport { errors })
        }
    }

    /// Validate arguments against verb schema.
    fn validate_args(
        &self,
        verb: &'static VerbDef,
        args: &[RawArg],
        context: &ValidationContext,
        symbols: &SymbolTable,
    ) -> Result<ValidatedArgs, Vec<ValidationError>> {
        let mut typed = HashMap::new();
        let mut context_injected = Vec::new();
        let mut defaulted = Vec::new();
        let mut errors = Vec::new();

        // Build map of provided args
        let provided: HashMap<_, _> = args.iter()
            .map(|a| (a.key.as_str(), a))
            .collect();

        // Check each spec
        for spec in verb.args {
            match provided.get(spec.name) {
                // Argument was provided
                Some(arg) => {
                    match self.validate_value(&arg.value, &spec.sem_type, symbols) {
                        Ok(typed_val) => {
                            // Check validation rules
                            for rule in spec.validation {
                                if let Err(msg) = self.check_rule(&typed_val, rule) {
                                    errors.push(ValidationError {
                                        span: arg.value_span,
                                        kind: ErrorKind::ValidationFailed {
                                            arg: spec.name,
                                            rule: format!("{:?}", rule),
                                            message: msg,
                                        },
                                    });
                                }
                            }
                            typed.insert(spec.name.to_string(), typed_val);
                        }
                        Err(msg) => {
                            errors.push(ValidationError {
                                span: arg.value_span,
                                kind: ErrorKind::TypeMismatch {
                                    arg: spec.name,
                                    expected: spec.sem_type.type_name(),
                                    got: msg,
                                },
                            });
                        }
                    }
                }

                // Argument not provided
                None => {
                    // Check if required
                    let is_required = self.is_arg_required(&spec.required, &typed, &provided);

                    if is_required {
                        // Try context injection
                        if let Some(DefaultValue::FromContext(key)) = &spec.default {
                            if let Some(ctx_val) = context.get_context_value(key) {
                                typed.insert(spec.name.to_string(), ctx_val);
                                context_injected.push(spec.name.to_string());
                                continue;
                            }
                        }

                        // Try static default
                        if let Some(default) = &spec.default {
                            if let Some(val) = self.default_to_typed(default) {
                                typed.insert(spec.name.to_string(), val);
                                defaulted.push(spec.name.to_string());
                                continue;
                            }
                        }

                        // Required but not provided
                        errors.push(ValidationError {
                            span: Span::default(),
                            kind: ErrorKind::MissingRequired {
                                arg: spec.name,
                                verb: verb.name,
                                required_because: self.explain_required(&spec.required, &typed),
                            },
                        });
                    } else {
                        // Optional - apply default if available
                        if let Some(default) = &spec.default {
                            if let Some(val) = self.default_to_typed(default) {
                                typed.insert(spec.name.to_string(), val);
                                defaulted.push(spec.name.to_string());
                            }
                        }
                    }
                }
            }
        }

        // Check cross-constraints
        for constraint in verb.constraints {
            if let Err(e) = self.check_constraint(constraint, &typed, &provided) {
                errors.push(e);
            }
        }

        // Check for unknown args
        for arg in args {
            let is_known = verb.args.iter().any(|s| s.name == arg.key) || arg.key == ":as";
            if !is_known {
                errors.push(ValidationError {
                    span: arg.key_span,
                    kind: ErrorKind::UnknownArg {
                        arg: arg.key.clone(),
                        verb: verb.name,
                        suggestions: self.suggest_arg(verb, &arg.key),
                    },
                });
            }
        }

        if errors.is_empty() {
            Ok(ValidatedArgs {
                args: typed,
                context_injected,
                defaulted,
            })
        } else {
            Err(errors)
        }
    }

    /// Validate a value against its semantic type.
    fn validate_value(
        &self,
        raw: &RawValue,
        sem_type: &SemType,
        symbols: &SymbolTable,
    ) -> Result<TypedValue, String> {
        match (sem_type, raw) {
            (SemType::String, RawValue::String(s)) => Ok(TypedValue::String(s.clone())),

            (SemType::Uuid, RawValue::String(s)) => {
                Uuid::parse_str(s)
                    .map(TypedValue::Uuid)
                    .map_err(|_| format!("invalid UUID: '{}'", s))
            }

            (SemType::Integer, RawValue::Int(i)) => Ok(TypedValue::Integer(*i)),

            (SemType::Decimal, RawValue::Float(f)) => Ok(TypedValue::Decimal(*f)),
            (SemType::Decimal, RawValue::Int(i)) => Ok(TypedValue::Decimal(*i as f64)),

            (SemType::Date, RawValue::String(s)) => {
                NaiveDate::parse_from_str(s, "%Y-%m-%d")
                    .map(TypedValue::Date)
                    .map_err(|_| format!("invalid date (expected YYYY-MM-DD): '{}'", s))
            }

            (SemType::Boolean, RawValue::Bool(b)) => Ok(TypedValue::Boolean(*b)),

            (SemType::Ref(ref_type), RawValue::String(code)) => {
                if self.schema_cache.exists(ref_type, code) {
                    Ok(TypedValue::Ref {
                        ref_type: *ref_type,
                        code: code.clone(),
                    })
                } else {
                    let suggestions = self.schema_cache.suggest(ref_type, code);
                    Err(format!(
                        "unknown {}: '{}'. {}",
                        ref_type.name(),
                        code,
                        if suggestions.is_empty() {
                            String::new()
                        } else {
                            format!("Did you mean: {}?", suggestions.join(", "))
                        }
                    ))
                }
            }

            (SemType::Enum(values), RawValue::String(s)) => {
                if values.contains(&s.as_str()) {
                    Ok(TypedValue::Enum(s.clone()))
                } else {
                    Err(format!("must be one of: {:?}", values))
                }
            }

            (SemType::Symbol, RawValue::Symbol(name)) => {
                let resolved_id = symbols.get(name).and_then(|s| s.resolved_id);
                Ok(TypedValue::Symbol {
                    name: name.clone(),
                    resolved_id,
                })
            }

            (SemType::ListOf(inner), RawValue::List(items)) => {
                let typed_items: Result<Vec<_>, _> = items
                    .iter()
                    .map(|item| self.validate_value(item, inner, symbols))
                    .collect();
                typed_items.map(TypedValue::List)
            }

            (SemType::Map(specs), RawValue::Map(pairs)) => {
                let mut typed_map = HashMap::new();
                for (key, value) in pairs {
                    let full_key = format!(":{}", key);
                    if let Some(spec) = specs.iter().find(|s| s.name == full_key) {
                        match self.validate_value(value, &spec.sem_type, symbols) {
                            Ok(typed_val) => { typed_map.insert(key.clone(), typed_val); }
                            Err(e) => return Err(e),
                        }
                    }
                }
                Ok(TypedValue::Map(typed_map))
            }

            _ => Err(format!("expected {}, got {}", sem_type.type_name(), raw.type_name())),
        }
    }

    /// Check a validation rule.
    fn check_rule(&self, value: &TypedValue, rule: &ValidationRule) -> Result<(), String> {
        match rule {
            ValidationRule::LookupMustExist => Ok(()), // Already validated in validate_value

            ValidationRule::Pattern { regex, description } => {
                if let TypedValue::String(s) = value {
                    let re = regex::Regex::new(regex).map_err(|e| e.to_string())?;
                    if re.is_match(s) {
                        Ok(())
                    } else {
                        Err(format!("must match pattern: {}", description))
                    }
                } else {
                    Ok(())
                }
            }

            ValidationRule::Range { min, max } => {
                let num = match value {
                    TypedValue::Integer(i) => *i as f64,
                    TypedValue::Decimal(d) => *d,
                    _ => return Ok(()),
                };

                if let Some(min) = min {
                    if num < *min {
                        return Err(format!("must be >= {}", min));
                    }
                }
                if let Some(max) = max {
                    if num > *max {
                        return Err(format!("must be <= {}", max));
                    }
                }
                Ok(())
            }

            ValidationRule::Length { min, max } => {
                if let TypedValue::String(s) = value {
                    if let Some(min) = min {
                        if s.len() < *min {
                            return Err(format!("must be at least {} characters", min));
                        }
                    }
                    if let Some(max) = max {
                        if s.len() > *max {
                            return Err(format!("must be at most {} characters", max));
                        }
                    }
                }
                Ok(())
            }

            ValidationRule::DateRange { min, max } => {
                if let TypedValue::Date(d) = value {
                    let today = Local::now().date_naive();

                    if let Some(min_bound) = min {
                        let min_date = self.resolve_date_bound(min_bound, today);
                        if let Some(min_date) = min_date {
                            if *d < min_date {
                                return Err(format!("date must be on or after {}", min_date));
                            }
                        }
                    }

                    if let Some(max_bound) = max {
                        let max_date = self.resolve_date_bound(max_bound, today);
                        if let Some(max_date) = max_date {
                            if *d > max_date {
                                return Err(format!("date must be on or before {}", max_date));
                            }
                        }
                    }
                }
                Ok(())
            }

            ValidationRule::NotEmpty => {
                if let TypedValue::String(s) = value {
                    if s.trim().is_empty() {
                        return Err("cannot be empty".to_string());
                    }
                }
                Ok(())
            }

            ValidationRule::Custom(_) => Ok(()), // Custom validation handled elsewhere
        }
    }

    /// Resolve a date bound to an actual date.
    fn resolve_date_bound(&self, bound: &DateBound, today: NaiveDate) -> Option<NaiveDate> {
        match bound {
            DateBound::Literal(s) => NaiveDate::parse_from_str(s, "%Y-%m-%d").ok(),
            DateBound::Today => Some(today),
            DateBound::DaysFromToday(n) => Some(today + chrono::Duration::days(*n as i64)),
        }
    }

    /// Check a cross-constraint.
    fn check_constraint(
        &self,
        constraint: &CrossConstraint,
        typed: &HashMap<String, TypedValue>,
        provided: &HashMap<&str, &RawArg>,
    ) -> Result<(), ValidationError> {
        match constraint {
            CrossConstraint::ExactlyOne(args) => {
                let count = args.iter().filter(|a| provided.contains_key(*a)).count();
                if count != 1 {
                    return Err(ValidationError {
                        span: Span::default(),
                        kind: ErrorKind::ConstraintViolation {
                            constraint: format!("exactly one of {:?} must be provided", args),
                        },
                    });
                }
            }

            CrossConstraint::AtLeastOne(args) => {
                let has_any = args.iter().any(|a| provided.contains_key(*a) || typed.contains_key(*a));
                if !has_any {
                    return Err(ValidationError {
                        span: Span::default(),
                        kind: ErrorKind::ConstraintViolation {
                            constraint: format!("at least one of {:?} must be provided", args),
                        },
                    });
                }
            }

            CrossConstraint::Requires { if_present, then_require } => {
                if provided.contains_key(if_present) && !typed.contains_key(*then_require) {
                    return Err(ValidationError {
                        span: Span::default(),
                        kind: ErrorKind::ConstraintViolation {
                            constraint: format!("'{}' requires '{}'", if_present, then_require),
                        },
                    });
                }
            }

            CrossConstraint::Excludes { if_present, then_forbid } => {
                if provided.contains_key(if_present) && provided.contains_key(then_forbid) {
                    return Err(ValidationError {
                        span: Span::default(),
                        kind: ErrorKind::ConstraintViolation {
                            constraint: format!("'{}' and '{}' cannot both be provided", if_present, then_forbid),
                        },
                    });
                }
            }

            CrossConstraint::ConditionalRequired { if_arg, equals, then_require } => {
                if let Some(val) = typed.get(*if_arg) {
                    if val.as_str() == Some(*equals) && !typed.contains_key(*then_require) {
                        return Err(ValidationError {
                            span: Span::default(),
                            kind: ErrorKind::ConstraintViolation {
                                constraint: format!("'{}' required when {} = '{}'", then_require, if_arg, equals),
                            },
                        });
                    }
                }
            }

            CrossConstraint::LessThan { lesser: _, greater: _ } => {
                // TODO: Implement comparison
            }
        }

        Ok(())
    }

    /// Check if an argument is required based on its rule.
    fn is_arg_required(
        &self,
        rule: &RequiredRule,
        typed: &HashMap<String, TypedValue>,
        provided: &HashMap<&str, &RawArg>,
    ) -> bool {
        match rule {
            RequiredRule::Always => true,
            RequiredRule::Never => false,
            RequiredRule::UnlessProvided(other) => !provided.contains_key(other),
            RequiredRule::IfEquals { arg, value } => {
                typed.get(*arg)
                    .and_then(|v| v.as_str())
                    .map(|s| s == *value)
                    .unwrap_or(false)
            }
            RequiredRule::IfProvided(other) => provided.contains_key(other),
        }
    }

    /// Explain why an argument is required.
    fn explain_required(&self, rule: &RequiredRule, typed: &HashMap<String, TypedValue>) -> String {
        match rule {
            RequiredRule::Always => "always required".to_string(),
            RequiredRule::Never => "optional".to_string(),
            RequiredRule::UnlessProvided(other) => format!("required unless '{}' provided", other),
            RequiredRule::IfEquals { arg, value } => {
                let actual = typed.get(*arg).and_then(|v| v.as_str()).unwrap_or("?");
                format!("required when {} = '{}' (current: '{}')", arg, value, actual)
            }
            RequiredRule::IfProvided(other) => format!("required when '{}' is provided", other),
        }
    }

    /// Convert default value to typed value.
    fn default_to_typed(&self, default: &DefaultValue) -> Option<TypedValue> {
        match default {
            DefaultValue::Str(s) => Some(TypedValue::String(s.to_string())),
            DefaultValue::Int(i) => Some(TypedValue::Integer(*i)),
            DefaultValue::Decimal(d) => Some(TypedValue::Decimal(*d)),
            DefaultValue::Bool(b) => Some(TypedValue::Boolean(*b)),
            DefaultValue::FromContext(_) => None, // Handled separately
        }
    }

    /// Suggest similar argument names.
    fn suggest_arg(&self, verb: &VerbDef, typo: &str) -> Vec<String> {
        verb.args
            .iter()
            .map(|a| a.name)
            .filter(|a| {
                let distance = levenshtein_distance(a, typo);
                distance <= 3 || a.contains(typo) || typo.contains(*a)
            })
            .map(String::from)
            .collect()
    }
}

/// Context for validation (provides runtime values).
#[derive(Debug, Clone, Default)]
pub struct ValidationContext {
    pub cbu_id: Option<Uuid>,
    pub entity_id: Option<Uuid>,
    pub investigation_id: Option<Uuid>,
    pub decision_id: Option<Uuid>,
    pub document_request_id: Option<Uuid>,
    pub screening_id: Option<Uuid>,
}

impl ValidationContext {
    /// Create a new empty context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a context value by key.
    pub fn get_context_value(&self, key: &ContextKey) -> Option<TypedValue> {
        match key {
            ContextKey::CbuId => self.cbu_id.map(TypedValue::Uuid),
            ContextKey::EntityId => self.entity_id.map(TypedValue::Uuid),
            ContextKey::InvestigationId => self.investigation_id.map(TypedValue::Uuid),
            ContextKey::DecisionId => self.decision_id.map(TypedValue::Uuid),
            ContextKey::DocumentRequestId => self.document_request_id.map(TypedValue::Uuid),
            ContextKey::ScreeningId => self.screening_id.map(TypedValue::Uuid),
        }
    }
}

/// Calculate Levenshtein distance.
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_len = a.chars().count();
    let b_len = b.chars().count();

    if a_len == 0 { return b_len; }
    if b_len == 0 { return a_len; }

    let mut matrix = vec![vec![0usize; b_len + 1]; a_len + 1];

    for i in 0..=a_len { matrix[i][0] = i; }
    for j in 0..=b_len { matrix[0][j] = j; }

    for (i, ca) in a.chars().enumerate() {
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            matrix[i + 1][j + 1] = (matrix[i][j + 1] + 1)
                .min(matrix[i + 1][j] + 1)
                .min(matrix[i][j] + cost);
        }
    }

    matrix[a_len][b_len]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_simple_call() {
        let cache = Arc::new(SchemaCache::with_defaults());
        let validator = SchemaValidator::new(cache);
        let context = ValidationContext::new();

        let raw = RawAst {
            expressions: vec![
                RawExpr {
                    span: Span::new(0, 50, 1, 1),
                    kind: RawExprKind::Call {
                        name: "cbu.ensure".to_string(),
                        name_span: Span::new(1, 11, 1, 2),
                        args: vec![
                            RawArg {
                                span: Span::new(12, 40, 1, 13),
                                key: ":cbu-name".to_string(),
                                key_span: Span::new(12, 21, 1, 13),
                                value: RawValue::String("Test Fund".to_string()),
                                value_span: Span::new(22, 33, 1, 23),
                                arg_spec: None,
                            },
                        ],
                        verb_def: None,
                    },
                },
            ],
        };

        let result = validator.validate(&raw, &context);
        assert!(result.is_ok());
    }

    #[test]
    fn test_unknown_verb() {
        let cache = Arc::new(SchemaCache::with_defaults());
        let validator = SchemaValidator::new(cache);
        let context = ValidationContext::new();

        let raw = RawAst {
            expressions: vec![
                RawExpr {
                    span: Span::new(0, 20, 1, 1),
                    kind: RawExprKind::Call {
                        name: "unknown.verb".to_string(),
                        name_span: Span::new(1, 13, 1, 2),
                        args: vec![],
                        verb_def: None,
                    },
                },
            ],
        };

        let result = validator.validate(&raw, &context);
        assert!(result.is_err());
        let report = result.unwrap_err();
        assert!(matches!(report.errors[0].kind, ErrorKind::UnknownVerb { .. }));
    }
}
