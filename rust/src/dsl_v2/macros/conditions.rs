//! Condition evaluation for macro expansion
//!
//! Evaluates `when:` conditions against expansion context.

use super::schema::WhenCondition;
use std::collections::HashMap;

/// Context for condition evaluation
pub struct ConditionContext<'a> {
    /// Argument values (e.g., "structure-type" -> "ucits")
    pub args: &'a HashMap<String, String>,
    /// Session/scope values (e.g., "scope.jurisdiction" -> "LU")
    pub scope: &'a HashMap<String, String>,
}

impl<'a> ConditionContext<'a> {
    /// Create a new condition context
    pub fn new(args: &'a HashMap<String, String>, scope: &'a HashMap<String, String>) -> Self {
        Self { args, scope }
    }

    /// Get a value by key, checking args first, then scope
    pub fn get(&self, key: &str) -> Option<&str> {
        self.args
            .get(key)
            .or_else(|| self.scope.get(key))
            .map(|s| s.as_str())
    }
}

/// Result of condition evaluation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConditionResult {
    True,
    False,
    /// Condition references unknown variable
    Unknown(String),
}

impl ConditionResult {
    pub fn is_true(&self) -> bool {
        matches!(self, ConditionResult::True)
    }

    pub fn is_false(&self) -> bool {
        matches!(self, ConditionResult::False)
    }
}

/// Evaluate a condition against the given context
pub fn evaluate_condition(condition: &WhenCondition, ctx: &ConditionContext) -> ConditionResult {
    match condition {
        WhenCondition::Simple(expr) => evaluate_simple_condition(expr, ctx),
        WhenCondition::Not(not) => evaluate_not(&not.not, ctx),
        WhenCondition::AnyOf(any) => evaluate_any_of(&any.any_of, ctx),
        WhenCondition::AllOf(all) => evaluate_all_of(&all.all_of, ctx),
    }
}

/// Evaluate a simple string condition like "structure-type = ucits"
fn evaluate_simple_condition(expr: &str, ctx: &ConditionContext) -> ConditionResult {
    let expr = expr.trim();

    // Try different operators
    if let Some((lhs, rhs)) = expr.split_once("!=") {
        return evaluate_inequality(lhs.trim(), rhs.trim(), ctx);
    }

    if let Some((lhs, rhs)) = expr.split_once('=') {
        return evaluate_equality(lhs.trim(), rhs.trim(), ctx);
    }

    if let Some((lhs, rhs)) = expr.split_once(" in ") {
        return evaluate_membership(lhs.trim(), rhs.trim(), ctx);
    }

    if let Some((lhs, rhs)) = expr.split_once(" not in ") {
        let result = evaluate_membership(lhs.trim(), rhs.trim(), ctx);
        return negate_result(result);
    }

    // Single variable - check if it's truthy (non-empty)
    match ctx.get(expr) {
        Some(val) => {
            if val.is_empty() || val == "false" || val == "0" {
                ConditionResult::False
            } else {
                ConditionResult::True
            }
        }
        None => ConditionResult::Unknown(expr.to_string()),
    }
}

/// Evaluate equality: "var = value"
fn evaluate_equality(lhs: &str, rhs: &str, ctx: &ConditionContext) -> ConditionResult {
    match ctx.get(lhs) {
        Some(val) => {
            if val == rhs {
                ConditionResult::True
            } else {
                ConditionResult::False
            }
        }
        None => ConditionResult::Unknown(lhs.to_string()),
    }
}

/// Evaluate inequality: "var != value"
fn evaluate_inequality(lhs: &str, rhs: &str, ctx: &ConditionContext) -> ConditionResult {
    match ctx.get(lhs) {
        Some(val) => {
            if val != rhs {
                ConditionResult::True
            } else {
                ConditionResult::False
            }
        }
        None => ConditionResult::Unknown(lhs.to_string()),
    }
}

/// Evaluate membership: "var in [a, b, c]"
fn evaluate_membership(lhs: &str, rhs: &str, ctx: &ConditionContext) -> ConditionResult {
    let value = match ctx.get(lhs) {
        Some(v) => v,
        None => return ConditionResult::Unknown(lhs.to_string()),
    };

    // Parse the list: "[a, b, c]" or "a, b, c"
    let list_str = rhs.trim_start_matches('[').trim_end_matches(']');
    let items: Vec<&str> = list_str.split(',').map(|s| s.trim()).collect();

    if items.contains(&value) {
        ConditionResult::True
    } else {
        ConditionResult::False
    }
}

/// Evaluate NOT condition
fn evaluate_not(condition: &WhenCondition, ctx: &ConditionContext) -> ConditionResult {
    negate_result(evaluate_condition(condition, ctx))
}

/// Evaluate ANY-OF condition (OR)
fn evaluate_any_of(conditions: &[WhenCondition], ctx: &ConditionContext) -> ConditionResult {
    let mut has_unknown = false;

    for cond in conditions {
        match evaluate_condition(cond, ctx) {
            ConditionResult::True => return ConditionResult::True,
            ConditionResult::Unknown(var) => {
                has_unknown = true;
                // Continue checking - another branch might be true
                let _ = var;
            }
            ConditionResult::False => {}
        }
    }

    if has_unknown {
        // If none were true but some were unknown, we can't determine
        ConditionResult::Unknown("any-of had unknown conditions".to_string())
    } else {
        ConditionResult::False
    }
}

/// Evaluate ALL-OF condition (AND)
fn evaluate_all_of(conditions: &[WhenCondition], ctx: &ConditionContext) -> ConditionResult {
    for cond in conditions {
        match evaluate_condition(cond, ctx) {
            ConditionResult::False => return ConditionResult::False,
            ConditionResult::Unknown(var) => return ConditionResult::Unknown(var),
            ConditionResult::True => {}
        }
    }
    ConditionResult::True
}

/// Negate a condition result
fn negate_result(result: ConditionResult) -> ConditionResult {
    match result {
        ConditionResult::True => ConditionResult::False,
        ConditionResult::False => ConditionResult::True,
        ConditionResult::Unknown(var) => ConditionResult::Unknown(var),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ctx(
        args: &[(&str, &str)],
        scope: &[(&str, &str)],
    ) -> (HashMap<String, String>, HashMap<String, String>) {
        let args_map: HashMap<String, String> = args
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let scope_map: HashMap<String, String> = scope
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        (args_map, scope_map)
    }

    #[test]
    fn test_simple_equality() {
        let (args, scope) = make_ctx(&[("structure-type", "ucits")], &[]);
        let ctx = ConditionContext::new(&args, &scope);

        let cond = WhenCondition::Simple("structure-type = ucits".to_string());
        assert_eq!(evaluate_condition(&cond, &ctx), ConditionResult::True);

        let cond = WhenCondition::Simple("structure-type = aif".to_string());
        assert_eq!(evaluate_condition(&cond, &ctx), ConditionResult::False);
    }

    #[test]
    fn test_simple_inequality() {
        let (args, scope) = make_ctx(&[("jurisdiction", "LU")], &[]);
        let ctx = ConditionContext::new(&args, &scope);

        let cond = WhenCondition::Simple("jurisdiction != IE".to_string());
        assert_eq!(evaluate_condition(&cond, &ctx), ConditionResult::True);

        let cond = WhenCondition::Simple("jurisdiction != LU".to_string());
        assert_eq!(evaluate_condition(&cond, &ctx), ConditionResult::False);
    }

    #[test]
    fn test_membership() {
        let (args, scope) = make_ctx(&[("jurisdiction", "LU")], &[]);
        let ctx = ConditionContext::new(&args, &scope);

        let cond = WhenCondition::Simple("jurisdiction in [LU, IE, DE]".to_string());
        assert_eq!(evaluate_condition(&cond, &ctx), ConditionResult::True);

        let cond = WhenCondition::Simple("jurisdiction in [US, UK]".to_string());
        assert_eq!(evaluate_condition(&cond, &ctx), ConditionResult::False);
    }

    #[test]
    fn test_not_condition() {
        let (args, scope) = make_ctx(&[("structure-type", "ucits")], &[]);
        let ctx = ConditionContext::new(&args, &scope);

        let cond = WhenCondition::Not(NotCondition {
            not: Box::new(WhenCondition::Simple("structure-type = aif".to_string())),
        });
        assert_eq!(evaluate_condition(&cond, &ctx), ConditionResult::True);
    }

    #[test]
    fn test_any_of() {
        let (args, scope) = make_ctx(&[("jurisdiction", "LU")], &[]);
        let ctx = ConditionContext::new(&args, &scope);

        let cond = WhenCondition::AnyOf(AnyOfCondition {
            any_of: vec![
                WhenCondition::Simple("jurisdiction = IE".to_string()),
                WhenCondition::Simple("jurisdiction = LU".to_string()),
            ],
        });
        assert_eq!(evaluate_condition(&cond, &ctx), ConditionResult::True);

        let cond = WhenCondition::AnyOf(AnyOfCondition {
            any_of: vec![
                WhenCondition::Simple("jurisdiction = IE".to_string()),
                WhenCondition::Simple("jurisdiction = UK".to_string()),
            ],
        });
        assert_eq!(evaluate_condition(&cond, &ctx), ConditionResult::False);
    }

    #[test]
    fn test_all_of() {
        let (args, scope) = make_ctx(&[("jurisdiction", "LU"), ("structure-type", "ucits")], &[]);
        let ctx = ConditionContext::new(&args, &scope);

        let cond = WhenCondition::AllOf(AllOfCondition {
            all_of: vec![
                WhenCondition::Simple("jurisdiction = LU".to_string()),
                WhenCondition::Simple("structure-type = ucits".to_string()),
            ],
        });
        assert_eq!(evaluate_condition(&cond, &ctx), ConditionResult::True);

        let cond = WhenCondition::AllOf(AllOfCondition {
            all_of: vec![
                WhenCondition::Simple("jurisdiction = LU".to_string()),
                WhenCondition::Simple("structure-type = aif".to_string()),
            ],
        });
        assert_eq!(evaluate_condition(&cond, &ctx), ConditionResult::False);
    }

    #[test]
    fn test_unknown_variable() {
        let (args, scope) = make_ctx(&[], &[]);
        let ctx = ConditionContext::new(&args, &scope);

        let cond = WhenCondition::Simple("unknown-var = value".to_string());
        assert!(matches!(
            evaluate_condition(&cond, &ctx),
            ConditionResult::Unknown(_)
        ));
    }

    #[test]
    fn test_scope_fallback() {
        let (args, scope) = make_ctx(&[], &[("scope.jurisdiction", "LU")]);
        let ctx = ConditionContext::new(&args, &scope);

        let cond = WhenCondition::Simple("scope.jurisdiction = LU".to_string());
        assert_eq!(evaluate_condition(&cond, &ctx), ConditionResult::True);
    }
}
