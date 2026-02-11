//! Rule Expression Evaluator — pure Rust, no DB dependency
//!
//! Evaluates structured JSON rule conditions against an evaluation context.
//! Same evaluator used for:
//! 1. Rule `when_expr` conditions during eligibility evaluation
//! 2. `service_availability.*_constraints` evaluation
//! 3. Ruleset publish-time validation (field references exist in dictionary)
//!
//! Implements the boundary-aware merge strategy from the architecture paper (§6.3).

use std::collections::HashMap;

use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::api::booking_principal_types::{
    CandidateStatus, Condition, Effect, Gate, GateSeverity, Operator, Rule, Ruleset,
    RulesetBoundary,
};

// ============================================================================
// Evaluation Context
// ============================================================================

/// Flat key→value map assembled from client profile, classifications,
/// deal context, principal/offering/relationship facts.
pub type EvalContext = HashMap<String, JsonValue>;

/// Outcome of evaluating a single rule
#[derive(Debug, Clone)]
pub struct RuleOutcome {
    pub rule_id: Uuid,
    pub rule_name: String,
    pub kind: String,
    pub boundary: RulesetBoundary,
    pub effect: Effect,
    pub explain_text: Option<String>,
    pub evaluated_facts: HashMap<String, JsonValue>,
}

/// Merged candidate result after all rules are applied
#[derive(Debug, Clone)]
pub struct MergedCandidate {
    pub principal_id: Uuid,
    pub status: CandidateStatus,
    pub gates: Vec<Gate>,
    pub contract_packs: Vec<ContractSelection>,
    pub deny_reasons: Vec<String>,
    pub rule_hits: Vec<RuleOutcome>,
}

/// Contract pack selected by a rule
#[derive(Debug, Clone)]
pub struct ContractSelection {
    pub contract_pack_code: String,
    pub template_types: Vec<String>,
    pub source_rule_id: Uuid,
}

// ============================================================================
// Condition Evaluation — recursive tree walk
// ============================================================================

/// Evaluate a condition tree against an evaluation context.
/// Returns true if the condition matches.
pub fn evaluate_condition(condition: &Condition, ctx: &EvalContext) -> bool {
    match condition {
        Condition::All(children) => children.iter().all(|c| evaluate_condition(c, ctx)),
        Condition::Any(children) => children.iter().any(|c| evaluate_condition(c, ctx)),
        Condition::Not(inner) => !evaluate_condition(inner, ctx),
        Condition::Field { field, op, value } => evaluate_field(field, op, value, ctx),
    }
}

/// Evaluate a single field comparison
fn evaluate_field(field: &str, op: &Operator, expected: &JsonValue, ctx: &EvalContext) -> bool {
    let actual = match ctx.get(field) {
        Some(v) => v,
        None => {
            // Field not present — only `exists` operator can match absent fields
            return matches!(op, Operator::Exists) && expected == &JsonValue::Bool(false);
        }
    };

    match op {
        Operator::Eq => actual == expected,
        Operator::Neq => actual != expected,
        Operator::In => {
            // actual value should be IN the expected array
            if let JsonValue::Array(arr) = expected {
                arr.contains(actual)
            } else {
                false
            }
        }
        Operator::NotIn => {
            if let JsonValue::Array(arr) = expected {
                !arr.contains(actual)
            } else {
                true
            }
        }
        Operator::Contains => {
            // actual array should contain expected value
            if let JsonValue::Array(arr) = actual {
                arr.contains(expected)
            } else {
                false
            }
        }
        Operator::Exists => {
            // Expected is a boolean: true = must exist, false = must not exist
            match expected {
                JsonValue::Bool(true) => true, // field exists (we already checked above)
                JsonValue::Bool(false) => false, // field should not exist but does
                _ => true,
            }
        }
        Operator::Gt => compare_numbers(actual, expected, |a, b| a > b),
        Operator::Gte => compare_numbers(actual, expected, |a, b| a >= b),
        Operator::Lt => compare_numbers(actual, expected, |a, b| a < b),
        Operator::Lte => compare_numbers(actual, expected, |a, b| a <= b),
    }
}

/// Compare two JSON values as f64 numbers
fn compare_numbers(actual: &JsonValue, expected: &JsonValue, cmp: fn(f64, f64) -> bool) -> bool {
    let a = actual.as_f64();
    let b = expected.as_f64();
    match (a, b) {
        (Some(av), Some(bv)) => cmp(av, bv),
        _ => false,
    }
}

// ============================================================================
// Rule Evaluation — evaluate rules within a ruleset
// ============================================================================

/// Evaluate all rules in a ruleset against the context.
/// Returns outcomes for rules whose conditions matched.
pub fn evaluate_rules(ruleset: &Ruleset, rules: &[Rule], ctx: &EvalContext) -> Vec<RuleOutcome> {
    let boundary = match RulesetBoundary::from_str_val(&ruleset.ruleset_boundary) {
        Some(b) => b,
        None => return Vec::new(),
    };

    let mut outcomes = Vec::new();

    for rule in rules {
        // Parse condition from JSON
        let condition: Condition = match serde_json::from_value(rule.when_expr.clone()) {
            Ok(c) => c,
            Err(_) => continue, // Skip malformed conditions
        };

        if !evaluate_condition(&condition, ctx) {
            continue; // Condition didn't match
        }

        // Parse effect
        let effect: Effect = match serde_json::from_value(rule.then_effect.clone()) {
            Ok(e) => e,
            Err(_) => continue,
        };

        // Collect which facts were actually referenced
        let evaluated_facts = collect_referenced_facts(&condition, ctx);

        outcomes.push(RuleOutcome {
            rule_id: rule.rule_id,
            rule_name: rule.name.clone(),
            kind: rule.kind.clone(),
            boundary: boundary.clone(),
            effect,
            explain_text: rule.explain.clone(),
            evaluated_facts,
        });
    }

    outcomes
}

/// Collect the actual values of fields referenced in a condition tree
fn collect_referenced_facts(
    condition: &Condition,
    ctx: &EvalContext,
) -> HashMap<String, JsonValue> {
    let mut facts = HashMap::new();
    collect_facts_recursive(condition, ctx, &mut facts);
    facts
}

fn collect_facts_recursive(
    condition: &Condition,
    ctx: &EvalContext,
    facts: &mut HashMap<String, JsonValue>,
) {
    match condition {
        Condition::All(children) | Condition::Any(children) => {
            for c in children {
                collect_facts_recursive(c, ctx, facts);
            }
        }
        Condition::Not(inner) => {
            collect_facts_recursive(inner, ctx, facts);
        }
        Condition::Field { field, .. } => {
            if let Some(v) = ctx.get(field) {
                facts.insert(field.clone(), v.clone());
            }
        }
    }
}

// ============================================================================
// Boundary-Aware Merge Strategy
// ============================================================================

/// Merge all rule outcomes for a single candidate principal.
///
/// Pipeline:
/// 1. Deny classification: regulatory deny → HardDeny; commercial/operational → ConditionalDeny
/// 2. Gate accumulation: all require_gate outcomes collected
/// 3. Allow/constraint merge: global > offering > principal priority
pub fn merge_outcomes_for_candidate(
    principal_id: Uuid,
    outcomes: &[RuleOutcome],
) -> MergedCandidate {
    let mut gates = Vec::new();
    let mut contract_packs = Vec::new();
    let mut deny_reasons = Vec::new();
    let mut hard_deny = false;
    let mut conditional_deny: Option<(RulesetBoundary, String, Vec<Uuid>)> = None;
    let mut hard_deny_info: Option<(String, Option<String>, Vec<Uuid>)> = None;

    for outcome in outcomes {
        match &outcome.effect {
            Effect::Deny {
                reason_code,
                reason,
            } => match outcome.boundary {
                RulesetBoundary::Regulatory => {
                    hard_deny = true;
                    let blocking = hard_deny_info.get_or_insert_with(|| {
                        (reason.clone(), Some(reason_code.clone()), Vec::new())
                    });
                    blocking.2.push(outcome.rule_id);
                    deny_reasons.push(format!("[regulatory] {}", reason));
                }
                RulesetBoundary::Commercial | RulesetBoundary::Operational => {
                    if conditional_deny.is_none() {
                        conditional_deny = Some((
                            outcome.boundary.clone(),
                            reason.clone(),
                            vec![outcome.rule_id],
                        ));
                    } else if let Some(ref mut cd) = conditional_deny {
                        cd.2.push(outcome.rule_id);
                    }
                    deny_reasons.push(format!("[{}] {}", outcome.boundary.as_str(), reason));
                }
            },
            Effect::RequireGate { gate, severity } => {
                gates.push(Gate {
                    gate_code: gate.clone(),
                    gate_name: gate.clone(),
                    boundary: outcome.boundary.clone(),
                    severity: severity.clone(),
                    source_rule_id: outcome.rule_id,
                    source_ruleset_id: Uuid::nil(), // Caller should enrich
                });
            }
            Effect::SelectContract {
                contract_pack_code,
                template_types,
            } => {
                contract_packs.push(ContractSelection {
                    contract_pack_code: contract_pack_code.clone(),
                    template_types: template_types.clone(),
                    source_rule_id: outcome.rule_id,
                });
            }
            Effect::Allow | Effect::ConstrainPrincipal { .. } => {
                // Allow and constraints don't directly affect candidate status
            }
        }
    }

    // Determine final status
    let status = if hard_deny {
        let (reason, regulation_ref, blocking_rules) =
            hard_deny_info.unwrap_or_else(|| ("Regulatory denial".to_string(), None, Vec::new()));
        CandidateStatus::HardDeny {
            reason,
            regulation_ref,
            blocking_rules,
        }
    } else if let Some((boundary, reason, blocking_rules)) = conditional_deny {
        let override_gate = Some(Gate {
            gate_code: format!("{}_override", boundary.as_str()),
            gate_name: format!("{} override approval", boundary.as_str()),
            boundary: boundary.clone(),
            severity: GateSeverity::Blocking,
            source_rule_id: blocking_rules.first().copied().unwrap_or(Uuid::nil()),
            source_ruleset_id: Uuid::nil(),
        });
        CandidateStatus::ConditionalDeny {
            boundary,
            reason,
            override_gate,
            blocking_rules,
        }
    } else if !gates.is_empty() {
        CandidateStatus::EligibleWithGates {
            gates: gates.clone(),
        }
    } else {
        CandidateStatus::Eligible
    };

    MergedCandidate {
        principal_id,
        status,
        gates,
        contract_packs,
        deny_reasons,
        rule_hits: outcomes.to_vec(),
    }
}

// ============================================================================
// Field Dictionary Validation (for ruleset.publish)
// ============================================================================

/// Validate that all field references in a condition tree exist in the dictionary.
/// Returns a list of unknown field keys.
pub fn validate_field_references(
    condition: &Condition,
    known_fields: &HashMap<String, String>, // field_key → field_type
) -> Vec<String> {
    let mut unknown = Vec::new();
    validate_fields_recursive(condition, known_fields, &mut unknown);
    unknown
}

fn validate_fields_recursive(
    condition: &Condition,
    known_fields: &HashMap<String, String>,
    unknown: &mut Vec<String>,
) {
    match condition {
        Condition::All(children) | Condition::Any(children) => {
            for c in children {
                validate_fields_recursive(c, known_fields, unknown);
            }
        }
        Condition::Not(inner) => {
            validate_fields_recursive(inner, known_fields, unknown);
        }
        Condition::Field { field, .. } => {
            if !known_fields.contains_key(field) {
                unknown.push(field.clone());
            }
        }
    }
}

/// Validate operator/type compatibility for a condition tree.
/// Returns warnings for incompatible operator + field_type combinations.
pub fn validate_operator_compatibility(
    condition: &Condition,
    known_fields: &HashMap<String, String>,
) -> Vec<String> {
    let mut warnings = Vec::new();
    validate_ops_recursive(condition, known_fields, &mut warnings);
    warnings
}

fn validate_ops_recursive(
    condition: &Condition,
    known_fields: &HashMap<String, String>,
    warnings: &mut Vec<String>,
) {
    match condition {
        Condition::All(children) | Condition::Any(children) => {
            for c in children {
                validate_ops_recursive(c, known_fields, warnings);
            }
        }
        Condition::Not(inner) => {
            validate_ops_recursive(inner, known_fields, warnings);
        }
        Condition::Field { field, op, .. } => {
            if let Some(field_type) = known_fields.get(field) {
                let invalid = match field_type.as_str() {
                    "string" => matches!(
                        op,
                        Operator::Gt | Operator::Gte | Operator::Lt | Operator::Lte
                    ),
                    "boolean" => !matches!(op, Operator::Eq | Operator::Neq | Operator::Exists),
                    "string_array" => matches!(
                        op,
                        Operator::Gt | Operator::Gte | Operator::Lt | Operator::Lte
                    ),
                    _ => false,
                };
                if invalid {
                    warnings.push(format!(
                        "Operator {:?} not valid for field '{}' (type: {})",
                        op, field, field_type
                    ));
                }
            }
        }
    }
}

// ============================================================================
// Context Assembly Helpers
// ============================================================================

/// Build evaluation context from client profile fields
pub fn build_client_context(
    segment: &str,
    domicile_country: &str,
    entity_types: &[String],
    risk_flags: Option<&serde_json::Value>,
    classifications: &[(String, String)], // (scheme, value) pairs
) -> EvalContext {
    let mut ctx = EvalContext::new();

    ctx.insert("client.segment".into(), JsonValue::String(segment.into()));
    ctx.insert(
        "client.domicile_country".into(),
        JsonValue::String(domicile_country.into()),
    );
    ctx.insert(
        "client.entity_types".into(),
        JsonValue::Array(
            entity_types
                .iter()
                .map(|s| JsonValue::String(s.clone()))
                .collect(),
        ),
    );

    // Flatten risk flags
    if let Some(JsonValue::Object(flags)) = risk_flags {
        for (key, val) in flags {
            ctx.insert(format!("client.risk_flags.{}", key), val.clone());
        }
    }

    // Flatten classifications
    for (scheme, value) in classifications {
        ctx.insert(
            format!("client.classification.{}", scheme),
            JsonValue::String(value.clone()),
        );
    }

    ctx
}

/// Add principal context fields
pub fn add_principal_context(
    ctx: &mut EvalContext,
    principal_code: &str,
    country_code: &str,
    region_code: Option<&str>,
    regulatory_regimes: &[String],
) {
    ctx.insert(
        "principal.code".into(),
        JsonValue::String(principal_code.into()),
    );
    ctx.insert(
        "principal.location.country".into(),
        JsonValue::String(country_code.into()),
    );
    if let Some(region) = region_code {
        ctx.insert(
            "principal.location.region".into(),
            JsonValue::String(region.into()),
        );
    }
    ctx.insert(
        "principal.location.regulatory_regimes".into(),
        JsonValue::Array(
            regulatory_regimes
                .iter()
                .map(|s| JsonValue::String(s.clone()))
                .collect(),
        ),
    );
}

/// Add offering context fields
pub fn add_offering_context(
    ctx: &mut EvalContext,
    product_code: &str,
    product_family: Option<&str>,
) {
    ctx.insert(
        "offering.code".into(),
        JsonValue::String(product_code.into()),
    );
    if let Some(family) = product_family {
        ctx.insert(
            "offering.product_family".into(),
            JsonValue::String(family.into()),
        );
    }
}

/// Add deal context fields
pub fn add_deal_context(
    ctx: &mut EvalContext,
    market_countries: Option<&[String]>,
    instrument_types: Option<&[String]>,
    trading_venues: Option<&[String]>,
) {
    if let Some(markets) = market_countries {
        ctx.insert(
            "deal.market_countries".into(),
            JsonValue::Array(
                markets
                    .iter()
                    .map(|s| JsonValue::String(s.clone()))
                    .collect(),
            ),
        );
    }
    if let Some(instruments) = instrument_types {
        ctx.insert(
            "deal.instrument_types".into(),
            JsonValue::Array(
                instruments
                    .iter()
                    .map(|s| JsonValue::String(s.clone()))
                    .collect(),
            ),
        );
    }
    if let Some(venues) = trading_venues {
        ctx.insert(
            "deal.trading_venues".into(),
            JsonValue::Array(
                venues
                    .iter()
                    .map(|s| JsonValue::String(s.clone()))
                    .collect(),
            ),
        );
    }
}

/// Add relationship context fields
pub fn add_relationship_context(
    ctx: &mut EvalContext,
    has_relationship: bool,
    status: Option<&str>,
    offering_codes: &[String],
) {
    ctx.insert(
        "relationship.exists".into(),
        JsonValue::Bool(has_relationship),
    );
    if let Some(s) = status {
        ctx.insert("relationship.status".into(), JsonValue::String(s.into()));
    }
    ctx.insert(
        "relationship.offerings".into(),
        JsonValue::Array(
            offering_codes
                .iter()
                .map(|s| JsonValue::String(s.clone()))
                .collect(),
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_context() -> EvalContext {
        let mut ctx = EvalContext::new();
        ctx.insert("client.segment".into(), json!("pension"));
        ctx.insert("client.domicile_country".into(), json!("LU"));
        ctx.insert("client.entity_types".into(), json!(["fund", "sicav"]));
        ctx.insert(
            "client.classification.mifid_ii".into(),
            json!("professional_client"),
        );
        ctx.insert("client.risk_flags.sanctions".into(), json!(false));
        ctx.insert("client.risk_flags.pep".into(), json!(false));
        ctx
    }

    #[test]
    fn test_eq_operator() {
        let ctx = sample_context();
        let cond = Condition::Field {
            field: "client.segment".into(),
            op: Operator::Eq,
            value: json!("pension"),
        };
        assert!(evaluate_condition(&cond, &ctx));
    }

    #[test]
    fn test_neq_operator() {
        let ctx = sample_context();
        let cond = Condition::Field {
            field: "client.segment".into(),
            op: Operator::Neq,
            value: json!("retail"),
        };
        assert!(evaluate_condition(&cond, &ctx));
    }

    #[test]
    fn test_in_operator() {
        let ctx = sample_context();
        let cond = Condition::Field {
            field: "client.segment".into(),
            op: Operator::In,
            value: json!(["pension", "hedge_fund"]),
        };
        assert!(evaluate_condition(&cond, &ctx));
    }

    #[test]
    fn test_not_in_operator() {
        let ctx = sample_context();
        let cond = Condition::Field {
            field: "client.segment".into(),
            op: Operator::NotIn,
            value: json!(["retail", "sovereign"]),
        };
        assert!(evaluate_condition(&cond, &ctx));
    }

    #[test]
    fn test_contains_operator() {
        let ctx = sample_context();
        let cond = Condition::Field {
            field: "client.entity_types".into(),
            op: Operator::Contains,
            value: json!("fund"),
        };
        assert!(evaluate_condition(&cond, &ctx));
    }

    #[test]
    fn test_exists_operator() {
        let ctx = sample_context();
        let cond = Condition::Field {
            field: "client.segment".into(),
            op: Operator::Exists,
            value: json!(true),
        };
        assert!(evaluate_condition(&cond, &ctx));

        let missing_cond = Condition::Field {
            field: "nonexistent.field".into(),
            op: Operator::Exists,
            value: json!(false),
        };
        assert!(evaluate_condition(&missing_cond, &ctx));
    }

    #[test]
    fn test_all_combinator() {
        let ctx = sample_context();
        let cond = Condition::All(vec![
            Condition::Field {
                field: "client.segment".into(),
                op: Operator::Eq,
                value: json!("pension"),
            },
            Condition::Field {
                field: "client.domicile_country".into(),
                op: Operator::Eq,
                value: json!("LU"),
            },
        ]);
        assert!(evaluate_condition(&cond, &ctx));
    }

    #[test]
    fn test_any_combinator() {
        let ctx = sample_context();
        let cond = Condition::Any(vec![
            Condition::Field {
                field: "client.segment".into(),
                op: Operator::Eq,
                value: json!("retail"),
            },
            Condition::Field {
                field: "client.domicile_country".into(),
                op: Operator::Eq,
                value: json!("LU"),
            },
        ]);
        assert!(evaluate_condition(&cond, &ctx));
    }

    #[test]
    fn test_not_combinator() {
        let ctx = sample_context();
        let cond = Condition::Not(Box::new(Condition::Field {
            field: "client.risk_flags.sanctions".into(),
            op: Operator::Eq,
            value: json!(true),
        }));
        assert!(evaluate_condition(&cond, &ctx));
    }

    #[test]
    fn test_merge_eligible() {
        let outcomes = vec![]; // No rule hits = eligible
        let result = merge_outcomes_for_candidate(Uuid::new_v4(), &outcomes);
        assert!(matches!(result.status, CandidateStatus::Eligible));
    }

    #[test]
    fn test_merge_hard_deny() {
        let outcomes = vec![RuleOutcome {
            rule_id: Uuid::new_v4(),
            rule_name: "Sanctions check".into(),
            kind: "deny".into(),
            boundary: RulesetBoundary::Regulatory,
            effect: Effect::Deny {
                reason_code: "SANCTIONS_HIT".into(),
                reason: "Client domicile under sanctions".into(),
            },
            explain_text: None,
            evaluated_facts: HashMap::new(),
        }];
        let result = merge_outcomes_for_candidate(Uuid::new_v4(), &outcomes);
        assert!(matches!(result.status, CandidateStatus::HardDeny { .. }));
    }

    #[test]
    fn test_merge_conditional_deny() {
        let outcomes = vec![RuleOutcome {
            rule_id: Uuid::new_v4(),
            rule_name: "Market entry".into(),
            kind: "deny".into(),
            boundary: RulesetBoundary::Commercial,
            effect: Effect::Deny {
                reason_code: "NOT_OFFERED".into(),
                reason: "Not offered for EU pensions".into(),
            },
            explain_text: None,
            evaluated_facts: HashMap::new(),
        }];
        let result = merge_outcomes_for_candidate(Uuid::new_v4(), &outcomes);
        assert!(matches!(
            result.status,
            CandidateStatus::ConditionalDeny { .. }
        ));
    }

    #[test]
    fn test_merge_eligible_with_gates() {
        let outcomes = vec![RuleOutcome {
            rule_id: Uuid::new_v4(),
            rule_name: "Credit approval".into(),
            kind: "require_gate".into(),
            boundary: RulesetBoundary::Commercial,
            effect: Effect::RequireGate {
                gate: "credit_approval".into(),
                severity: GateSeverity::Blocking,
            },
            explain_text: None,
            evaluated_facts: HashMap::new(),
        }];
        let result = merge_outcomes_for_candidate(Uuid::new_v4(), &outcomes);
        assert!(matches!(
            result.status,
            CandidateStatus::EligibleWithGates { .. }
        ));
    }

    #[test]
    fn test_validate_field_references() {
        let mut known = HashMap::new();
        known.insert("client.segment".into(), "string".into());
        known.insert("client.domicile_country".into(), "string".into());

        let cond = Condition::All(vec![
            Condition::Field {
                field: "client.segment".into(),
                op: Operator::Eq,
                value: json!("pension"),
            },
            Condition::Field {
                field: "unknown.field".into(),
                op: Operator::Eq,
                value: json!("x"),
            },
        ]);

        let unknown = validate_field_references(&cond, &known);
        assert_eq!(unknown, vec!["unknown.field"]);
    }

    #[test]
    fn test_validate_operator_compatibility() {
        let mut known = HashMap::new();
        known.insert("client.risk_flags.sanctions".into(), "boolean".into());

        let cond = Condition::Field {
            field: "client.risk_flags.sanctions".into(),
            op: Operator::Gt,
            value: json!(true),
        };

        let warnings = validate_operator_compatibility(&cond, &known);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("not valid"));
    }
}
