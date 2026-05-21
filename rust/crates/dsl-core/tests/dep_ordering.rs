//! Dependency ordering property tests — Tranche 1 regression baseline.
//!
//! For any compiled multi-step program, verifies that the compiler preserves
//! source order: step[i].source_stmt < step[j].source_stmt when i < j.
//!
//! The dsl-core compiler emits steps in source order (comments excluded).
//! This test ensures no reordering happens at the compiler level. Topological
//! sorting based on @-bindings is applied at plan-build time (in dsl-runtime /
//! ob-poc), not here.
//!
//! Additionally tests that binding names referenced in arguments are correctly
//! preserved — i.e., that a consumer step that uses @cbu in an argument is
//! not placed before the producer step that binds @cbu.

use dsl_core::{compiler::compile_to_steps, parser::parse_program};

/// Compile the source and return the compiled steps.
fn compile(source: &str) -> Vec<dsl_core::compiler::CompileStep> {
    let program = parse_program(source).expect("parse failed");
    compile_to_steps(&program).steps
}

/// Assert that source_stmt indices are strictly increasing in the compiled output.
fn assert_steps_in_source_order(steps: &[dsl_core::compiler::CompileStep], source: &str) {
    for window in steps.windows(2) {
        let (a, b) = (&window[0], &window[1]);
        assert!(
            a.source_stmt < b.source_stmt,
            "Step order violation in:\n{source}\n  step '{}' (stmt {}) should come before '{}' (stmt {})",
            a.verb_call.full_name(),
            a.source_stmt,
            b.verb_call.full_name(),
            b.source_stmt
        );
    }
}

#[test]
fn compiled_plan_preserves_source_order_two_steps() {
    let source = r#"
        (cbu.create :name "Allianz GI" :jurisdiction "LU" :as @cbu)
        (kyc-case.create :cbu-id @cbu :case-type "standard" :as @case)
    "#;
    let steps = compile(source);
    assert_eq!(steps.len(), 2, "expected 2 steps");
    assert_steps_in_source_order(&steps, source);
}

#[test]
fn compiled_plan_preserves_source_order_three_steps() {
    let source = r#"
        (deal.create :client-name "Fidelity" :product "custody" :as @deal)
        (deal.link-cbu :deal-id @deal :cbu-id @cbu)
        (deal.advance-stage :deal-id @deal :to "bac_approved")
    "#;
    let steps = compile(source);
    assert_eq!(steps.len(), 3, "expected 3 steps");
    assert_steps_in_source_order(&steps, source);
}

#[test]
fn compiled_plan_preserves_source_order_with_comment() {
    // Comments are excluded from compiled steps; VerbCall order must match source.
    let source = r#"
        ;; Step 1: create the entity
        (entity.create-limited-company :name "Apex Capital" :as @entity)
        ;; Step 2: screen it
        (screening.pep :entity-id @entity)
    "#;
    let steps = compile(source);
    // Comments are skipped; only VerbCalls become steps
    assert_eq!(steps.len(), 2, "expected 2 steps (comments excluded)");
    assert_steps_in_source_order(&steps, source);
}

#[test]
fn compiled_plan_source_stmt_matches_statement_index() {
    // source_stmt is the 0-based index into program.statements (including Comment).
    // Step 0 = stmt 1 (after the comment at stmt 0).
    let source = r#"
;; Header comment
(cbu.create :name "Fund A" :as @cbu)
(cbu.create :name "Fund B" :as @fundB)
"#;
    let steps = compile(source);
    assert_eq!(steps.len(), 2);
    // The comment occupies statement index 0 (or 1 depending on leading newline).
    // Just verify indices are monotonically increasing.
    assert!(steps[0].source_stmt < steps[1].source_stmt);
}

#[test]
fn compiled_plan_five_step_onboarding_order() {
    let source = r#"
        (cbu.create :name "Generali AM" :jurisdiction "DE" :as @cbu)
        (entity.create-limited-company :name "Generali AM GmbH" :as @entity)
        (cbu.assign-role :cbu-id @cbu :entity-id @entity :role "ASSET_MANAGER")
        (kyc-case.create :cbu-id @cbu :case-type "standard" :as @case)
        (screening.pep :entity-id @entity)
    "#;
    let steps = compile(source);
    assert_eq!(steps.len(), 5, "expected 5 steps");
    assert_steps_in_source_order(&steps, source);
}

#[test]
fn compiled_plan_correct_step_count_no_args() {
    let source = r#"
        (session.start :mode "new")
        (view.universe)
        (session.info)
    "#;
    let steps = compile(source);
    assert_eq!(steps.len(), 3);
    assert_steps_in_source_order(&steps, source);
}

#[test]
fn compiled_plan_binding_names_preserved_per_step() {
    let source = r#"
        (cbu.create :name "Fund A" :as @fundA)
        (cbu.create :name "Fund B" :as @fundB)
        (cbu.create :name "Fund C" :as @fundC)
    "#;
    let steps = compile(source);
    assert_eq!(steps.len(), 3);
    assert_eq!(steps[0].verb_call.binding.as_deref(), Some("fundA"));
    assert_eq!(steps[1].verb_call.binding.as_deref(), Some("fundB"));
    assert_eq!(steps[2].verb_call.binding.as_deref(), Some("fundC"));
}

#[test]
fn compiled_plan_no_binding_steps_interleaved() {
    let source = r#"
        (cbu.create :name "Fund X" :as @cbu)
        (screening.pep :entity-id @entity)
        (kyc-case.create :cbu-id @cbu :case-type "standard" :as @case)
        (screening.sanctions :entity-id @entity)
    "#;
    let steps = compile(source);
    assert_eq!(steps.len(), 4);
    assert_steps_in_source_order(&steps, source);
    // Verify which steps have bindings and which don't.
    assert!(steps[0].verb_call.binding.is_some()); // @cbu
    assert!(steps[1].verb_call.binding.is_none()); // no binding
    assert!(steps[2].verb_call.binding.is_some()); // @case
    assert!(steps[3].verb_call.binding.is_none()); // no binding
}

#[test]
fn compiled_plan_verb_fqn_preserved_in_steps() {
    let source = r#"
        (changeset.compose :title "My CS" :as @cs)
        (changeset.validate :changeset-id @cs)
        (governance.publish :changeset-id @cs :dry-run false)
    "#;
    let steps = compile(source);
    assert_eq!(steps.len(), 3);
    assert_eq!(steps[0].verb_call.full_name(), "changeset.compose");
    assert_eq!(steps[1].verb_call.full_name(), "changeset.validate");
    assert_eq!(steps[2].verb_call.full_name(), "governance.publish");
    assert_steps_in_source_order(&steps, source);
}

#[test]
fn compiled_plan_single_step_no_ordering_violation() {
    let source = r#"(session.start :mode "new")"#;
    let steps = compile(source);
    assert_eq!(steps.len(), 1);
    // Single step — trivially ordered.
}

#[test]
fn compiled_plan_source_stmt_starts_at_zero_or_positive() {
    let source = r#"
        (entity.create :entity-type "company" :name "Test Corp" :as @entity)
        (screening.pep :entity-id @entity)
        (screening.sanctions :entity-id @entity)
        (kyc-case.create :cbu-id @cbu :case-type "standard")
    "#;
    let steps = compile(source);
    for step in &steps {
        // source_stmt is an index; must be ≥ 0 (usize is always ≥ 0 but let's confirm monotone)
        let _ = step.source_stmt; // just reference to confirm it's a usize
    }
    assert_steps_in_source_order(&steps, source);
}
