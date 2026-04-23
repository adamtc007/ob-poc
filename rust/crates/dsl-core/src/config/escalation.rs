//! Escalation DSL runtime evaluator (v1.1 P11 — pilot P.1.b).
//!
//! Given a `ConsequenceDeclaration` (baseline tier + escalation rules) and a
//! runtime `EvaluationContext` (argument values, entity attributes, context
//! flags), compute the **effective tier** per v1.1 P11:
//!
//! ```text
//! matched = [rule.tier for rule in rules if rule.matches]
//! effective = baseline if matched is empty else max(baseline, max(matched))
//! ```
//!
//! Monotonic: rules can only raise tier, never lower (type system enforces —
//! `ConsequenceTier` is `PartialOrd`, and `compute_effective_tier` folds with
//! `max`).
//!
//! Predicate evaluation is a pure function over the input context. No DB
//! access, no HTTP, no wall clock — satisfies P3 (DB-free catalogue mode)
//! for validator use; runtime use is equally deterministic given the context
//! snapshot.

use crate::config::types::{
    ConsequenceDeclaration, ConsequenceTier, EscalationPredicate, EscalationRule,
};
use std::collections::HashMap;

/// Evaluation context — the three argument kinds referenced by the
/// restricted DSL per v1.1 R6.
#[derive(Debug, Clone, Default)]
pub struct EvaluationContext {
    /// Verb argument values, keyed by argument name.
    pub args: HashMap<String, serde_json::Value>,
    /// Entity attributes, keyed by `entity_kind` → attr_name → value.
    /// Example: `entities["cbu"]["sanctions_status"] = "listed"`.
    pub entities: HashMap<String, HashMap<String, serde_json::Value>>,
    /// Named boolean context flags (session state).
    pub context_flags: HashMap<String, bool>,
}

impl EvaluationContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_arg(mut self, name: impl Into<String>, value: serde_json::Value) -> Self {
        self.args.insert(name.into(), value);
        self
    }

    pub fn with_entity_attr(
        mut self,
        entity_kind: impl Into<String>,
        attr: impl Into<String>,
        value: serde_json::Value,
    ) -> Self {
        self.entities
            .entry(entity_kind.into())
            .or_default()
            .insert(attr.into(), value);
        self
    }

    pub fn with_flag(mut self, flag: impl Into<String>, value: bool) -> Self {
        self.context_flags.insert(flag.into(), value);
        self
    }
}

/// Evaluate a predicate against the context. Missing references (unknown
/// arg / attr / flag) evaluate to **false** — the conservative direction,
/// consistent with P11's "rules can only raise tier": a missing reference
/// cannot cause a spurious escalation.
pub fn evaluate_predicate(pred: &EscalationPredicate, ctx: &EvaluationContext) -> bool {
    match pred {
        EscalationPredicate::ArgEq { arg, value } => {
            ctx.args.get(arg).is_some_and(|v| v == value)
        }
        EscalationPredicate::ArgIn { arg, values } => {
            ctx.args.get(arg).is_some_and(|v| values.iter().any(|w| w == v))
        }
        EscalationPredicate::ArgGt { arg, value } => {
            ctx.args.get(arg).and_then(as_f64).is_some_and(|n| n > *value)
        }
        EscalationPredicate::ArgGte { arg, value } => {
            ctx.args.get(arg).and_then(as_f64).is_some_and(|n| n >= *value)
        }
        EscalationPredicate::ArgLt { arg, value } => {
            ctx.args.get(arg).and_then(as_f64).is_some_and(|n| n < *value)
        }
        EscalationPredicate::ArgLte { arg, value } => {
            ctx.args.get(arg).and_then(as_f64).is_some_and(|n| n <= *value)
        }
        EscalationPredicate::EntityAttrEq {
            entity_kind,
            attr,
            value,
        } => ctx
            .entities
            .get(entity_kind)
            .and_then(|attrs| attrs.get(attr))
            .is_some_and(|v| v == value),
        EscalationPredicate::EntityAttrIn {
            entity_kind,
            attr,
            values,
        } => ctx
            .entities
            .get(entity_kind)
            .and_then(|attrs| attrs.get(attr))
            .is_some_and(|v| values.iter().any(|w| w == v)),
        EscalationPredicate::ContextFlag { flag } => {
            ctx.context_flags.get(flag).copied().unwrap_or(false)
        }
        EscalationPredicate::And { preds } => {
            preds.iter().all(|p| evaluate_predicate(p, ctx))
        }
        EscalationPredicate::Or { preds } => {
            preds.iter().any(|p| evaluate_predicate(p, ctx))
        }
        EscalationPredicate::Not { pred } => !evaluate_predicate(pred, ctx),
    }
}

/// Compute effective tier per P11:
/// `effective = max(baseline, max(matching_rule.tier))`.
pub fn compute_effective_tier(
    decl: &ConsequenceDeclaration,
    ctx: &EvaluationContext,
) -> ConsequenceTier {
    decl.escalation
        .iter()
        .filter(|rule| evaluate_predicate(&rule.when, ctx))
        .map(|rule| rule.tier)
        .fold(decl.baseline, |acc, tier| acc.max(tier))
}

/// Like [`compute_effective_tier`] but also returns the names of the rules
/// that fired, for UX transparency (v1.1 Open Question 15 — "effective-tier
/// UX transparency: honest UX shows the escalation chain").
pub fn compute_effective_tier_with_trace<'a>(
    decl: &'a ConsequenceDeclaration,
    ctx: &EvaluationContext,
) -> (ConsequenceTier, Vec<&'a EscalationRule>) {
    let fired: Vec<_> = decl
        .escalation
        .iter()
        .filter(|rule| evaluate_predicate(&rule.when, ctx))
        .collect();
    let tier = fired
        .iter()
        .map(|rule| rule.tier)
        .fold(decl.baseline, |acc, tier| acc.max(tier));
    (tier, fired)
}

/// Coerce a JSON value to f64 for numeric-threshold predicates. Returns
/// `None` for non-numeric values → predicate evaluates false (conservative).
fn as_f64(v: &serde_json::Value) -> Option<f64> {
    v.as_f64()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::{ConsequenceTier, EscalationPredicate, EscalationRule};
    use serde_json::json;

    fn decl(baseline: ConsequenceTier, rules: Vec<EscalationRule>) -> ConsequenceDeclaration {
        ConsequenceDeclaration {
            baseline,
            escalation: rules,
        }
    }

    fn rule(
        name: &str,
        pred: EscalationPredicate,
        tier: ConsequenceTier,
    ) -> EscalationRule {
        EscalationRule {
            name: name.into(),
            when: pred,
            tier,
            reason: None,
        }
    }

    #[test]
    fn baseline_returned_when_no_rules_match() {
        let d = decl(
            ConsequenceTier::Benign,
            vec![rule(
                "sanctions",
                EscalationPredicate::EntityAttrEq {
                    entity_kind: "cbu".into(),
                    attr: "sanctions_status".into(),
                    value: json!("listed"),
                },
                ConsequenceTier::RequiresExplicitAuthorisation,
            )],
        );
        let ctx = EvaluationContext::new(); // empty — rule doesn't match
        assert_eq!(compute_effective_tier(&d, &ctx), ConsequenceTier::Benign);
    }

    #[test]
    fn matching_rule_raises_tier() {
        let d = decl(
            ConsequenceTier::Benign,
            vec![rule(
                "bulk",
                EscalationPredicate::ArgGt {
                    arg: "count".into(),
                    value: 100.0,
                },
                ConsequenceTier::RequiresConfirmation,
            )],
        );
        let ctx = EvaluationContext::new().with_arg("count", json!(500));
        assert_eq!(
            compute_effective_tier(&d, &ctx),
            ConsequenceTier::RequiresConfirmation
        );
    }

    #[test]
    fn monotonic_never_lowers_below_baseline() {
        // Baseline higher than all rules — effective stays at baseline.
        let d = decl(
            ConsequenceTier::RequiresExplicitAuthorisation,
            vec![rule(
                "low_rule",
                EscalationPredicate::ArgEq {
                    arg: "mode".into(),
                    value: json!("safe"),
                },
                ConsequenceTier::Benign,
            )],
        );
        let ctx = EvaluationContext::new().with_arg("mode", json!("safe"));
        assert_eq!(
            compute_effective_tier(&d, &ctx),
            ConsequenceTier::RequiresExplicitAuthorisation
        );
    }

    #[test]
    fn max_of_multiple_matching_rules() {
        let d = decl(
            ConsequenceTier::Reviewable,
            vec![
                rule(
                    "a",
                    EscalationPredicate::ContextFlag { flag: "f1".into() },
                    ConsequenceTier::RequiresConfirmation,
                ),
                rule(
                    "b",
                    EscalationPredicate::ContextFlag { flag: "f2".into() },
                    ConsequenceTier::RequiresExplicitAuthorisation,
                ),
                rule(
                    "c",
                    EscalationPredicate::ContextFlag { flag: "f3".into() },
                    ConsequenceTier::Benign, // never wins — max preserves monotonicity
                ),
            ],
        );
        let ctx = EvaluationContext::new()
            .with_flag("f1", true)
            .with_flag("f2", true)
            .with_flag("f3", true);
        assert_eq!(
            compute_effective_tier(&d, &ctx),
            ConsequenceTier::RequiresExplicitAuthorisation
        );
    }

    #[test]
    fn missing_arg_evaluates_false_conservatively() {
        let d = decl(
            ConsequenceTier::Benign,
            vec![rule(
                "sanctions",
                EscalationPredicate::ArgEq {
                    arg: "sanctions".into(),
                    value: json!(true),
                },
                ConsequenceTier::RequiresExplicitAuthorisation,
            )],
        );
        // Context doesn't mention `sanctions` — rule must evaluate false.
        let ctx = EvaluationContext::new();
        assert_eq!(compute_effective_tier(&d, &ctx), ConsequenceTier::Benign);
    }

    #[test]
    fn entity_attr_in_set() {
        let d = decl(
            ConsequenceTier::Benign,
            vec![rule(
                "high_risk_jur",
                EscalationPredicate::EntityAttrIn {
                    entity_kind: "cbu".into(),
                    attr: "jurisdiction".into(),
                    values: vec![json!("IR"), json!("KP"), json!("RU")],
                },
                ConsequenceTier::RequiresExplicitAuthorisation,
            )],
        );
        let ctx =
            EvaluationContext::new().with_entity_attr("cbu", "jurisdiction", json!("IR"));
        assert_eq!(
            compute_effective_tier(&d, &ctx),
            ConsequenceTier::RequiresExplicitAuthorisation
        );
    }

    #[test]
    fn boolean_and_combinator() {
        let d = decl(
            ConsequenceTier::Benign,
            vec![rule(
                "bulk_and_non_sim",
                EscalationPredicate::And {
                    preds: vec![
                        EscalationPredicate::ArgGt {
                            arg: "count".into(),
                            value: 100.0,
                        },
                        EscalationPredicate::Not {
                            pred: Box::new(EscalationPredicate::ContextFlag {
                                flag: "simulation_mode".into(),
                            }),
                        },
                    ],
                },
                ConsequenceTier::RequiresConfirmation,
            )],
        );
        // 150 items, not in simulation → rule fires.
        let ctx = EvaluationContext::new()
            .with_arg("count", json!(150))
            .with_flag("simulation_mode", false);
        assert_eq!(
            compute_effective_tier(&d, &ctx),
            ConsequenceTier::RequiresConfirmation
        );

        // Same count but simulation → rule doesn't fire (Not flips to false, AND flips to false).
        let ctx_sim = EvaluationContext::new()
            .with_arg("count", json!(150))
            .with_flag("simulation_mode", true);
        assert_eq!(
            compute_effective_tier(&d, &ctx_sim),
            ConsequenceTier::Benign
        );
    }

    #[test]
    fn trace_returns_fired_rules_in_declaration_order() {
        let d = decl(
            ConsequenceTier::Benign,
            vec![
                rule(
                    "first",
                    EscalationPredicate::ArgEq {
                        arg: "a".into(),
                        value: json!(1),
                    },
                    ConsequenceTier::Reviewable,
                ),
                rule(
                    "second",
                    EscalationPredicate::ArgEq {
                        arg: "b".into(),
                        value: json!(2),
                    },
                    ConsequenceTier::RequiresConfirmation,
                ),
            ],
        );
        let ctx = EvaluationContext::new()
            .with_arg("a", json!(1))
            .with_arg("b", json!(2));
        let (tier, fired) = compute_effective_tier_with_trace(&d, &ctx);
        assert_eq!(tier, ConsequenceTier::RequiresConfirmation);
        assert_eq!(fired.len(), 2);
        assert_eq!(fired[0].name, "first");
        assert_eq!(fired[1].name, "second");
    }

    #[test]
    fn numeric_threshold_on_json_int_and_float() {
        let d = decl(
            ConsequenceTier::Benign,
            vec![rule(
                "over_10",
                EscalationPredicate::ArgGte {
                    arg: "n".into(),
                    value: 10.0,
                },
                ConsequenceTier::Reviewable,
            )],
        );
        // Integer JSON value.
        let ctx_int = EvaluationContext::new().with_arg("n", json!(15));
        assert_eq!(
            compute_effective_tier(&d, &ctx_int),
            ConsequenceTier::Reviewable
        );
        // Float JSON value.
        let ctx_flt = EvaluationContext::new().with_arg("n", json!(10.5));
        assert_eq!(
            compute_effective_tier(&d, &ctx_flt),
            ConsequenceTier::Reviewable
        );
        // Just under.
        let ctx_under = EvaluationContext::new().with_arg("n", json!(9.999));
        assert_eq!(
            compute_effective_tier(&d, &ctx_under),
            ConsequenceTier::Benign
        );
    }

    #[test]
    fn non_numeric_arg_fails_numeric_predicate_conservatively() {
        let d = decl(
            ConsequenceTier::Benign,
            vec![rule(
                "gt",
                EscalationPredicate::ArgGt {
                    arg: "x".into(),
                    value: 0.0,
                },
                ConsequenceTier::Reviewable,
            )],
        );
        // x is a string — numeric comparison evaluates false.
        let ctx = EvaluationContext::new().with_arg("x", json!("hello"));
        assert_eq!(compute_effective_tier(&d, &ctx), ConsequenceTier::Benign);
    }
}
