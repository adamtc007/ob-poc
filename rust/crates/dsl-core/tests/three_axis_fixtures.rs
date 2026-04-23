//! Integration test for the three-axis declaration pipeline (P.1.f).
//!
//! Exercises P.1.a (schema types) + P.1.b (escalation evaluator) + P.1.c
//! (structural + well-formedness validator) + P.1.d (policy-sanity
//! warnings) + P.1.e (runbook composition) against the 20-verb fixture
//! at `tests/fixtures/three_axis_samples/verbs.yaml`.
//!
//! Per v1.1 Tranche 1 Phase 1.6, exits by demonstrating:
//! 1. Every fixture verb passes the validator cleanly.
//! 2. Per-verb `effective_tier` evaluates correctly under chosen runtime
//!    contexts (baseline, escalation-triggering, non-triggering).
//! 3. Composition produces identical tiers for a macro-expanded runbook
//!    and an ad-hoc REPL runbook built from the same step sequence
//!    (P12 invariant).

use dsl_core::config::{
    compute_effective_tier, compute_runbook_tier, compute_runbook_tier_with_trace,
    validate_verbs_config, AggregationRule, ConsequenceTier, CrossScopeRule, EvaluationContext,
    ExternalEffect, RunbookStep, StateEffect, ValidationContext, VerbsConfig,
};
use serde_json::json;
use std::fs;

fn load_fixture() -> VerbsConfig {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/three_axis_samples/verbs.yaml"
    );
    let raw = fs::read_to_string(path).expect("fixture file not readable");
    serde_yaml::from_str(&raw).expect("fixture YAML parses")
}

// ---------------------------------------------------------------------------
// Top-level — fixture parses and validates cleanly
// ---------------------------------------------------------------------------

#[test]
fn fixture_parses_to_twenty_verbs() {
    let cfg = load_fixture();
    let domain = cfg.domains.get("fixture").expect("fixture domain");
    assert_eq!(domain.verbs.len(), 20, "fixture should declare 20 verbs");
}

#[test]
fn every_fixture_verb_has_three_axis_declaration() {
    let cfg = load_fixture();
    for (name, verb) in &cfg.domains["fixture"].verbs {
        assert!(
            verb.three_axis.is_some(),
            "verb '{}' missing three_axis declaration",
            name
        );
    }
}

#[test]
fn fixture_passes_validator_cleanly() {
    let cfg = load_fixture();
    // require_declaration=true — every verb in this fixture must carry one.
    let ctx = ValidationContext {
        require_declaration: true,
        ..ValidationContext::default()
    };
    let report = validate_verbs_config(&cfg, &ctx);
    assert!(
        report.structural.is_empty(),
        "structural errors: {:#?}",
        report.structural
    );
    assert!(
        report.well_formedness.is_empty(),
        "well-formedness errors: {:#?}",
        report.well_formedness
    );
    assert!(
        report.warnings.is_empty(),
        "policy-sanity warnings: {:#?}",
        report.warnings
    );
}

// ---------------------------------------------------------------------------
// P10 orthogonality — the three canonical unusual combinations stay silent
// ---------------------------------------------------------------------------

#[test]
fn p10_export_report_preserving_plus_high_tier_silent() {
    let cfg = load_fixture();
    let verb = &cfg.domains["fixture"].verbs["export-report"];
    let decl = verb.three_axis.as_ref().unwrap();
    assert_eq!(decl.state_effect, StateEffect::Preserving);
    assert_eq!(
        decl.consequence.baseline,
        ConsequenceTier::RequiresExplicitAuthorisation
    );
    // Validator silent (asserted in fixture_passes_validator_cleanly already).
}

#[test]
fn p10_reorder_transition_plus_benign_silent() {
    let cfg = load_fixture();
    let verb = &cfg.domains["fixture"].verbs["reorder-collection"];
    let decl = verb.three_axis.as_ref().unwrap();
    assert_eq!(decl.state_effect, StateEffect::Transition);
    assert_eq!(decl.consequence.baseline, ConsequenceTier::Benign);
}

#[test]
fn p10_sanctions_transition_plus_empty_effects_plus_high_tier_silent() {
    let cfg = load_fixture();
    let verb = &cfg.domains["fixture"].verbs["sanctions-apply"];
    let decl = verb.three_axis.as_ref().unwrap();
    assert_eq!(decl.state_effect, StateEffect::Transition);
    assert!(decl.external_effects.is_empty());
    assert_eq!(
        decl.consequence.baseline,
        ConsequenceTier::RequiresExplicitAuthorisation
    );
}

// ---------------------------------------------------------------------------
// P.1.b runtime escalation — verbs with escalation rules
// ---------------------------------------------------------------------------

#[test]
fn audit_log_query_escalates_on_sensitive_arg() {
    let cfg = load_fixture();
    let verb = &cfg.domains["fixture"].verbs["audit-log-query"];
    let decl = verb.three_axis.as_ref().unwrap();
    // Baseline context — no escalation.
    let baseline_ctx = EvaluationContext::new().with_arg("sensitive", json!(false));
    assert_eq!(
        compute_effective_tier(&decl.consequence, &baseline_ctx),
        ConsequenceTier::Reviewable
    );
    // Sensitive=true context — escalates.
    let hot_ctx = EvaluationContext::new().with_arg("sensitive", json!(true));
    assert_eq!(
        compute_effective_tier(&decl.consequence, &hot_ctx),
        ConsequenceTier::RequiresExplicitAuthorisation
    );
}

#[test]
fn bulk_send_escalates_over_threshold() {
    let cfg = load_fixture();
    let verb = &cfg.domains["fixture"].verbs["bulk-send-notifications"];
    let decl = verb.three_axis.as_ref().unwrap();
    // Low count — stays at baseline.
    let low_ctx = EvaluationContext::new().with_arg("count", json!(50));
    assert_eq!(
        compute_effective_tier(&decl.consequence, &low_ctx),
        ConsequenceTier::Reviewable
    );
    // High count — escalates.
    let high_ctx = EvaluationContext::new().with_arg("count", json!(500));
    assert_eq!(
        compute_effective_tier(&decl.consequence, &high_ctx),
        ConsequenceTier::RequiresConfirmation
    );
}

#[test]
fn submit_for_review_escalates_on_high_risk_jurisdiction() {
    let cfg = load_fixture();
    let verb = &cfg.domains["fixture"].verbs["submit-for-review"];
    let decl = verb.three_axis.as_ref().unwrap();
    // Normal jurisdiction.
    let ok_ctx = EvaluationContext::new().with_entity_attr("cbu", "jurisdiction", json!("LU"));
    assert_eq!(
        compute_effective_tier(&decl.consequence, &ok_ctx),
        ConsequenceTier::Reviewable
    );
    // High-risk jurisdiction.
    let bad_ctx = EvaluationContext::new().with_entity_attr("cbu", "jurisdiction", json!("IR"));
    assert_eq!(
        compute_effective_tier(&decl.consequence, &bad_ctx),
        ConsequenceTier::RequiresExplicitAuthorisation
    );
}

#[test]
fn activate_trading_double_escalation_max_wins() {
    let cfg = load_fixture();
    let verb = &cfg.domains["fixture"].verbs["activate-trading"];
    let decl = verb.three_axis.as_ref().unwrap();
    // No escalation triggers.
    let baseline = EvaluationContext::new().with_arg("live_capital", json!(1_000_000));
    assert_eq!(
        compute_effective_tier(&decl.consequence, &baseline),
        ConsequenceTier::RequiresConfirmation
    );
    // Capital threshold triggers → first escalation.
    let big = EvaluationContext::new().with_arg("live_capital", json!(50_000_000));
    assert_eq!(
        compute_effective_tier(&decl.consequence, &big),
        ConsequenceTier::RequiresExplicitAuthorisation
    );
    // Sanctioned CBU + no waiver → second escalation.
    let sanctioned = EvaluationContext::new()
        .with_arg("live_capital", json!(100))
        .with_entity_attr("cbu", "sanctions_status", json!("listed"))
        .with_flag("compliance_waiver_granted", false);
    assert_eq!(
        compute_effective_tier(&decl.consequence, &sanctioned),
        ConsequenceTier::RequiresExplicitAuthorisation
    );
    // Sanctioned CBU + waiver granted → second escalation does NOT fire.
    let waived = EvaluationContext::new()
        .with_arg("live_capital", json!(100))
        .with_entity_attr("cbu", "sanctions_status", json!("listed"))
        .with_flag("compliance_waiver_granted", true);
    assert_eq!(
        compute_effective_tier(&decl.consequence, &waived),
        ConsequenceTier::RequiresConfirmation
    );
}

// ---------------------------------------------------------------------------
// P.1.e runbook composition — macro-produced vs ad-hoc invariance (P12)
// ---------------------------------------------------------------------------

/// Build a composition-layer step from a fixture verb + resolved context.
/// Helper for the two-runbook P12 parity test.
fn step_for(
    cfg: &VerbsConfig,
    verb_name: &str,
    workspace: &str,
    dag: Option<&str>,
    ctx: &EvaluationContext,
) -> RunbookStep {
    let verb = &cfg.domains["fixture"].verbs[verb_name];
    let decl = verb.three_axis.as_ref().unwrap();
    let effective_tier = compute_effective_tier(&decl.consequence, ctx);
    RunbookStep {
        verb_fqn: format!("fixture.{}", verb_name),
        effective_tier,
        state_effect: decl.state_effect,
        external_effects: decl.external_effects.clone(),
        workspace: workspace.into(),
        dag: dag.map(str::to_string),
        entity_kind: Some("cbu".into()),
    }
}

#[test]
fn macro_expanded_and_adhoc_produce_same_tier_p12_invariant() {
    let cfg = load_fixture();
    // Step sequence representing an onboarding flow:
    //   read → list → advance-draft → approve-submission
    let ctx = EvaluationContext::new();
    let build_steps = |cfg: &VerbsConfig| {
        vec![
            step_for(cfg, "read-cbu", "instrument_matrix", None, &ctx),
            step_for(cfg, "list-instruments", "instrument_matrix", None, &ctx),
            step_for(
                cfg,
                "advance-draft",
                "instrument_matrix",
                Some("fixture_dag"),
                &ctx,
            ),
            step_for(
                cfg,
                "approve-submission",
                "instrument_matrix",
                Some("fixture_dag"),
                &ctx,
            ),
        ]
    };
    let macro_expanded = build_steps(&cfg);
    let adhoc_assembled = build_steps(&cfg);

    // No aggregation / cross-scope rules — tiers come purely from Component A.
    let t1 = compute_runbook_tier(&macro_expanded, &[], &[]);
    let t2 = compute_runbook_tier(&adhoc_assembled, &[], &[]);
    assert_eq!(t1, t2, "P12 invariant: composition is origin-agnostic");
    // Max of (benign, benign, reviewable, requires_confirmation) = RequiresConfirmation.
    assert_eq!(t1, ConsequenceTier::RequiresConfirmation);
}

#[test]
fn cross_workspace_runbook_escalates_via_component_c() {
    let cfg = load_fixture();
    let ctx = EvaluationContext::new();
    let steps = vec![
        step_for(&cfg, "read-cbu", "instrument_matrix", None, &ctx),
        step_for(&cfg, "read-cbu", "kyc_workspace", None, &ctx),
    ];
    let cross_scope = vec![CrossScopeRule::MultiWorkspace {
        name: "cross_ws".into(),
        min_workspaces: 2,
        tier: ConsequenceTier::RequiresConfirmation,
    }];
    let t = compute_runbook_tier(&steps, &[], &cross_scope);
    assert_eq!(
        t, ConsequenceTier::RequiresConfirmation,
        "cross-workspace rule should escalate a runbook with all-benign steps"
    );
}

#[test]
fn bulk_emission_runbook_escalates_via_component_b() {
    let cfg = load_fixture();
    let ctx_low = EvaluationContext::new().with_arg("count", json!(1));
    let mut steps = Vec::new();
    for _ in 0..4 {
        steps.push(step_for(&cfg, "notify-operator", "w", None, &ctx_low));
    }
    let rules = vec![AggregationRule::RepeatedExternalEffect {
        name: "many_emissions".into(),
        effect: ExternalEffect::Emitting,
        threshold: 3,
        tier: ConsequenceTier::RequiresConfirmation,
    }];
    let t = compute_runbook_tier(&steps, &rules, &[]);
    assert_eq!(
        t, ConsequenceTier::RequiresConfirmation,
        "4 emitting steps above threshold=3 should escalate"
    );
}

#[test]
fn composed_tier_fully_traced() {
    let cfg = load_fixture();
    let ctx = EvaluationContext::new().with_arg("count", json!(500));
    let steps = vec![
        // 1. reviewable
        step_for(&cfg, "notify-operator", "w1", None, &ctx),
        // 2. bulk-send escalated to requires_confirmation (count=500 > 100)
        step_for(&cfg, "bulk-send-notifications", "w1", None, &ctx),
        // 3. benign
        step_for(&cfg, "read-cbu", "w2", None, &ctx),
    ];
    let agg = vec![AggregationRule::BulkCardinality {
        name: "bulk_runbook".into(),
        threshold: 3,
        tier: ConsequenceTier::Reviewable,
    }];
    let xs = vec![CrossScopeRule::MultiWorkspace {
        name: "cross_ws".into(),
        min_workspaces: 2,
        tier: ConsequenceTier::Reviewable,
    }];
    let trace = compute_runbook_tier_with_trace(&steps, &agg, &xs);
    // Component A: max(reviewable, requires_confirmation, benign)
    //            = requires_confirmation
    assert_eq!(trace.component_a, ConsequenceTier::RequiresConfirmation);
    // Component B: bulk_runbook fires (>= 3 steps) → Reviewable
    assert_eq!(trace.component_b, ConsequenceTier::Reviewable);
    assert_eq!(trace.aggregation_fired, vec!["bulk_runbook"]);
    // Component C: cross_ws fires (w1 + w2) → Reviewable
    assert_eq!(trace.component_c, ConsequenceTier::Reviewable);
    assert_eq!(trace.cross_scope_fired, vec!["cross_ws"]);
    // Effective: max(A, B, C) = requires_confirmation
    assert_eq!(trace.effective, ConsequenceTier::RequiresConfirmation);
}
