//! Evaluate rule conditions against context

use super::context::RuleContext;
use super::parser::{Condition, LeafCondition, Operator};
use serde_json::Value;

pub struct RuleEvaluator;

impl RuleEvaluator {
    pub fn new() -> Self {
        Self
    }

    /// Evaluate a condition against the context
    pub fn evaluate(&self, condition: &Condition, context: &RuleContext) -> bool {
        match condition {
            Condition::All { all } => all.iter().all(|c| self.evaluate(c, context)),
            Condition::Any { any } => any.iter().any(|c| self.evaluate(c, context)),
            Condition::Not { not } => !self.evaluate(not, context),
            Condition::Leaf(leaf) => self.evaluate_leaf(leaf, context),
        }
    }

    fn evaluate_leaf(&self, leaf: &LeafCondition, context: &RuleContext) -> bool {
        let value = context.get(&leaf.field);

        match (&leaf.operator, value) {
            // Equals
            (Operator::Equals(expected), Some(actual)) => self.values_equal(expected, actual),
            (Operator::Equals(_), None) => false,

            // Not Equals
            (Operator::NotEquals(expected), Some(actual)) => !self.values_equal(expected, actual),
            (Operator::NotEquals(_), None) => true,

            // In
            (Operator::In(list), Some(actual)) => {
                list.iter().any(|item| self.values_equal(item, actual))
            }
            (Operator::In(_), None) => false,

            // Not In
            (Operator::NotIn(list), Some(actual)) => {
                !list.iter().any(|item| self.values_equal(item, actual))
            }
            (Operator::NotIn(_), None) => true,

            // Contains (string)
            (Operator::Contains(substr), Some(Value::String(s))) => {
                s.to_lowercase().contains(&substr.to_lowercase())
            }
            (Operator::Contains(_), _) => false,

            // StartsWith
            (Operator::StartsWith(prefix), Some(Value::String(s))) => {
                s.to_lowercase().starts_with(&prefix.to_lowercase())
            }
            (Operator::StartsWith(_), _) => false,

            // EndsWith
            (Operator::EndsWith(suffix), Some(Value::String(s))) => {
                s.to_lowercase().ends_with(&suffix.to_lowercase())
            }
            (Operator::EndsWith(_), _) => false,

            // Numeric comparisons
            (Operator::Gt(threshold), Some(actual)) => self
                .get_number(actual)
                .map(|n| n > *threshold)
                .unwrap_or(false),
            (Operator::Gte(threshold), Some(actual)) => self
                .get_number(actual)
                .map(|n| n >= *threshold)
                .unwrap_or(false),
            (Operator::Lt(threshold), Some(actual)) => self
                .get_number(actual)
                .map(|n| n < *threshold)
                .unwrap_or(false),
            (Operator::Lte(threshold), Some(actual)) => self
                .get_number(actual)
                .map(|n| n <= *threshold)
                .unwrap_or(false),
            (Operator::Gt(_) | Operator::Gte(_) | Operator::Lt(_) | Operator::Lte(_), None) => {
                false
            }

            // Null checks
            (Operator::IsNull(expected), v) => {
                (v.is_none() || v == Some(&Value::Null)) == *expected
            }
            (Operator::IsNotNull(expected), v) => {
                (v.is_some() && v != Some(&Value::Null)) == *expected
            }

            // Regex
            (Operator::Matches(pattern), Some(Value::String(s))) => regex::Regex::new(pattern)
                .map(|re| re.is_match(s))
                .unwrap_or(false),
            (Operator::Matches(_), _) => false,
        }
    }

    fn values_equal(&self, a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::String(s1), Value::String(s2)) => s1.to_lowercase() == s2.to_lowercase(),
            (Value::Number(n1), Value::Number(n2)) => n1.as_f64() == n2.as_f64(),
            (Value::Bool(b1), Value::Bool(b2)) => b1 == b2,
            (Value::Null, Value::Null) => true,
            _ => a == b,
        }
    }

    fn get_number(&self, value: &Value) -> Option<f64> {
        match value {
            Value::Number(n) => n.as_f64(),
            Value::String(s) => s.parse().ok(),
            _ => None,
        }
    }
}

impl Default for RuleEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_equals() {
        let evaluator = RuleEvaluator::new();
        let mut ctx = RuleContext::new();
        ctx.set("entity.jurisdiction", "KY");

        let condition = Condition::Leaf(LeafCondition {
            field: "entity.jurisdiction".to_string(),
            operator: Operator::Equals(json!("KY")),
        });

        assert!(evaluator.evaluate(&condition, &ctx));
    }

    #[test]
    fn test_in_operator() {
        let evaluator = RuleEvaluator::new();
        let mut ctx = RuleContext::new();
        ctx.set("entity.jurisdiction", "VG");

        let condition = Condition::Leaf(LeafCondition {
            field: "entity.jurisdiction".to_string(),
            operator: Operator::In(vec![json!("KY"), json!("VG"), json!("BVI")]),
        });

        assert!(evaluator.evaluate(&condition, &ctx));
    }

    #[test]
    fn test_all_condition() {
        let evaluator = RuleEvaluator::new();
        let mut ctx = RuleContext::new();
        ctx.set("entity.type", "trust");
        ctx.set("entity.trust_type", "DISCRETIONARY");

        let condition = Condition::All {
            all: vec![
                Condition::Leaf(LeafCondition {
                    field: "entity.type".to_string(),
                    operator: Operator::Equals(json!("trust")),
                }),
                Condition::Leaf(LeafCondition {
                    field: "entity.trust_type".to_string(),
                    operator: Operator::Equals(json!("DISCRETIONARY")),
                }),
            ],
        };

        assert!(evaluator.evaluate(&condition, &ctx));
    }

    #[test]
    fn test_gte_operator() {
        let evaluator = RuleEvaluator::new();
        let mut ctx = RuleContext::new();
        ctx.set("holding.ownership_percentage", 30.0);

        let condition = Condition::Leaf(LeafCondition {
            field: "holding.ownership_percentage".to_string(),
            operator: Operator::Gte(25.0),
        });

        assert!(evaluator.evaluate(&condition, &ctx));
    }

    #[test]
    fn test_contains_case_insensitive() {
        let evaluator = RuleEvaluator::new();
        let mut ctx = RuleContext::new();
        ctx.set("entity.name", "ABC Nominee Services Ltd");

        let condition = Condition::Leaf(LeafCondition {
            field: "entity.name".to_string(),
            operator: Operator::Contains("nominee".to_string()),
        });

        assert!(evaluator.evaluate(&condition, &ctx));
    }

    #[test]
    fn test_not_condition() {
        let evaluator = RuleEvaluator::new();
        let mut ctx = RuleContext::new();
        ctx.set("entity.jurisdiction", "US");

        let condition = Condition::Not {
            not: Box::new(Condition::Leaf(LeafCondition {
                field: "entity.jurisdiction".to_string(),
                operator: Operator::Equals(json!("KY")),
            })),
        };

        assert!(evaluator.evaluate(&condition, &ctx));
    }

    #[test]
    fn test_any_condition() {
        let evaluator = RuleEvaluator::new();
        let mut ctx = RuleContext::new();
        ctx.set("entity.jurisdiction", "BM");

        let condition = Condition::Any {
            any: vec![
                Condition::Leaf(LeafCondition {
                    field: "entity.jurisdiction".to_string(),
                    operator: Operator::Equals(json!("KY")),
                }),
                Condition::Leaf(LeafCondition {
                    field: "entity.jurisdiction".to_string(),
                    operator: Operator::Equals(json!("BM")),
                }),
            ],
        };

        assert!(evaluator.evaluate(&condition, &ctx));
    }
}
