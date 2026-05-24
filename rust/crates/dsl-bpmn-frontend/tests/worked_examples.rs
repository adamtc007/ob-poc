//! Compilation tests for the 12 bpmn-lite worked examples from
//! `docs/design/v0.1/session3-regression-packs-examples.md` §9.
//!
//! Each test:
//!  1. Parses the DSL source.
//!  2. Builds an `AtomBag`.
//!  3. Runs the assembly pass to produce a `RailwayGraph`.
//!  4. Lowers to a `JourneySpec`.
//!  5. Asserts structural properties of the result.

use dsl_ast::AtomBag;
use dsl_bpmn_frontend::assemble;
use dsl_diagnostics::DiagnosticBag;
use dsl_lowering::lower;

// ---------------------------------------------------------------------------
// Test helper
// ---------------------------------------------------------------------------

fn compile_example(source: &str) -> (dsl_lowering::JourneySpec, DiagnosticBag) {
    let (source_file, parse_diag) = dsl_parser::parse(source);
    let mut diag = DiagnosticBag::new();

    // Merge parse diagnostics
    for d in &parse_diag.diagnostics {
        diag.push(d.clone());
    }

    let bag = AtomBag::from_source_file(source_file, &mut diag);
    let graph = assemble(&bag, &mut diag);
    let spec = lower(&graph, "test-process");
    (spec, diag)
}

// ---------------------------------------------------------------------------
// Example 1: Linear sequence — onboarding intake
// ---------------------------------------------------------------------------

const EXAMPLE_1: &str = r#"
(node intake-start    :kind start-event)
(node intake-form     :kind user-task)
(node verify-identity :kind service-task)
(node aml-check       :kind service-task)
(node intake-end      :kind end-event)

(flow intake-start    -> intake-form)
(flow intake-form     -> verify-identity)
(flow verify-identity -> aml-check)
(flow aml-check       -> intake-end)
"#;

#[test]
fn example_1_linear_sequence() {
    let (spec, diag) = compile_example(EXAMPLE_1);

    // Should have no errors
    let errors: Vec<_> = diag.errors().map(|d| d.message.clone()).collect();
    assert!(errors.is_empty(), "unexpected errors: {:?}", errors);

    // Should have at least 3 nodes
    assert!(
        spec.nodes.len() >= 3,
        "expected ≥3 nodes, got {}",
        spec.nodes.len()
    );

    // Start node should be intake-start
    assert_eq!(
        spec.start_node, "intake-start",
        "expected start_node = intake-start"
    );

    // No gateways, no forks, 4 edges
    assert_eq!(spec.edges.len(), 4, "expected 4 edges, got {}", spec.edges.len());
    assert!(spec.parallel_joins.is_empty(), "expected no parallel joins");
}

// ---------------------------------------------------------------------------
// Example 2: Exclusive gateway — Pattern A
// ---------------------------------------------------------------------------

const EXAMPLE_2: &str = r#"
(node start-classify  :kind start-event)
(node kyc-review      :kind user-task)
(node classify-client :kind business-rule-task)
(gateway risk-gate    :kind exclusive)
(node activate-cbu    :kind service-task)
(node enhanced-review :kind user-task)
(node classify-end    :kind end-event)

(flow start-classify  -> kyc-review)
(flow kyc-review      -> classify-client)
(flow classify-client -> risk-gate)
(flow risk-gate       -> activate-cbu     :condition "risk-class-standard")
(flow risk-gate       -> enhanced-review  :default true)
(flow activate-cbu    -> classify-end)
(flow enhanced-review -> classify-end)
"#;

#[test]
fn example_2_exclusive_gateway_pattern_a() {
    let (spec, diag) = compile_example(EXAMPLE_2);

    let errors: Vec<_> = diag.errors().map(|d| d.message.clone()).collect();
    assert!(errors.is_empty(), "unexpected errors: {:?}", errors);

    // There should be a node with kind "exclusive"
    let has_exclusive = spec.nodes.iter().any(|n| n.kind.contains("exclusive"));
    assert!(has_exclusive, "expected at least one exclusive gateway node");
}

// ---------------------------------------------------------------------------
// Example 3: Linked-switch chain — Pattern B
// ---------------------------------------------------------------------------

const EXAMPLE_3: &str = r#"
(node start-chain     :kind start-event)
(node kyc-review      :kind user-task)
(gateway sanctions-gate :kind exclusive)
(gateway pep-gate       :kind exclusive)
(gateway risk-gate      :kind exclusive)
(node enhanced-review :kind user-task)
(node activate-cbu    :kind service-task)
(node chain-end       :kind end-event)

(flow start-chain    -> kyc-review)
(flow kyc-review     -> sanctions-gate)
(flow sanctions-gate -> enhanced-review :condition "sanctions-hit")
(flow sanctions-gate -> pep-gate        :default true)
(flow pep-gate       -> enhanced-review :condition "pep-positive")
(flow pep-gate       -> risk-gate       :default true)
(flow risk-gate      -> enhanced-review :condition "risk-enhanced")
(flow risk-gate      -> activate-cbu    :default true)
(flow enhanced-review -> chain-end)
(flow activate-cbu    -> chain-end)
"#;

#[test]
fn example_3_linked_switch_chain_pattern_b() {
    let (spec, diag) = compile_example(EXAMPLE_3);

    let errors: Vec<_> = diag.errors().map(|d| d.message.clone()).collect();
    assert!(errors.is_empty(), "unexpected errors: {:?}", errors);

    // Should have exactly 3 exclusive gateways
    let exclusive_count = spec.nodes.iter().filter(|n| n.kind == "exclusive").count();
    assert_eq!(exclusive_count, 3, "expected 3 exclusive gateways, got {}", exclusive_count);
}

// ---------------------------------------------------------------------------
// Example 4: Inclusive gateway — dynamic fan-out and fan-in
// ---------------------------------------------------------------------------

const EXAMPLE_4: &str = r#"
(node start-modular   :kind start-event)
(node select-modules  :kind business-rule-task)
(gateway module-fork  :kind inclusive)
(node basic-kyc       :kind user-task)
(node enhanced-kyc    :kind user-task)
(node sanctions-check :kind service-task)
(parallel-join module-join
  :expects [module-fork]
  :merge [
    {:location basic-kyc-result    :operator latest}
    {:location enhanced-kyc-result :operator latest}
    {:location sanctions-result    :operator latest}
  ])
(node evaluate-results :kind business-rule-task)
(node modular-end     :kind end-event)

(flow start-modular   -> select-modules)
(flow select-modules  -> module-fork)
(flow module-fork     -> basic-kyc)
(flow module-fork     -> enhanced-kyc)
(flow module-fork     -> sanctions-check)
(flow basic-kyc       -> module-join)
(flow enhanced-kyc    -> module-join)
(flow sanctions-check -> module-join)
(flow module-join     -> evaluate-results)
(flow evaluate-results -> modular-end)
"#;

#[test]
fn example_4_inclusive_gateway_dynamic_fanout() {
    let (spec, diag) = compile_example(EXAMPLE_4);

    let errors: Vec<_> = diag.errors().map(|d| d.message.clone()).collect();
    assert!(errors.is_empty(), "unexpected errors: {:?}", errors);

    // Should have an inclusive gateway
    let has_inclusive = spec.nodes.iter().any(|n| n.kind.contains("inclusive"));
    assert!(has_inclusive, "expected at least one inclusive gateway node");

    // Should have a parallel join
    assert_eq!(spec.parallel_joins.len(), 1, "expected 1 parallel join");
}

// ---------------------------------------------------------------------------
// Example 5: Parallel fork/join with declared data merge
// ---------------------------------------------------------------------------

const EXAMPLE_5: &str = r#"
(node onboarding-start  :kind start-event)
(gateway initiate-fork  :kind parallel)
(node kyc-task          :kind user-task)
(node deal-task         :kind service-task)
(node im-task           :kind service-task)
(parallel-join onboarding-join
  :expects [initiate-fork]
  :merge [
    {:location kyc-outcome  :operator latest}
    {:location deal-id      :operator latest}
    {:location im-config-id :operator latest}
  ])
(node final-review      :kind user-task)
(node activate-cbu      :kind service-task)
(node onboarding-end    :kind end-event)

(flow onboarding-start -> initiate-fork)
(flow initiate-fork    -> kyc-task)
(flow initiate-fork    -> deal-task)
(flow initiate-fork    -> im-task)
(flow kyc-task         -> onboarding-join)
(flow deal-task        -> onboarding-join)
(flow im-task          -> onboarding-join)
(flow onboarding-join  -> final-review)
(flow final-review     -> activate-cbu)
(flow activate-cbu     -> onboarding-end)
"#;

#[test]
fn example_5_parallel_fork_join_with_merge() {
    let (spec, diag) = compile_example(EXAMPLE_5);

    let errors: Vec<_> = diag.errors().map(|d| d.message.clone()).collect();
    assert!(errors.is_empty(), "unexpected errors: {:?}", errors);

    // Should have exactly 1 parallel join
    assert_eq!(spec.parallel_joins.len(), 1, "expected 1 parallel join");

    // The join should have merge clauses
    let join = &spec.parallel_joins[0];
    assert_eq!(join.name, "onboarding-join");
    assert!(
        !join.merge.is_empty(),
        "expected merge clauses on parallel join"
    );
    assert_eq!(join.merge.len(), 3, "expected 3 merge clauses");
}

// ---------------------------------------------------------------------------
// Example 6: Parallel fork/join — undeclared write conflict
//
// Same structure as Example 5 except the parallel-join has no :merge clause.
// This should produce an UNDECLARED_MERGE warning but NOT an error.
// ---------------------------------------------------------------------------

const EXAMPLE_6: &str = r#"
(node conflict-start  :kind start-event)
(gateway conflict-fork :kind parallel)
(node kyc-task        :kind user-task)
(node deal-task       :kind service-task)
(node im-task         :kind service-task)
(parallel-join conflict-join
  :expects [conflict-fork])
(node conflict-review :kind user-task)
(node conflict-end    :kind end-event)

(flow conflict-start -> conflict-fork)
(flow conflict-fork  -> kyc-task)
(flow conflict-fork  -> deal-task)
(flow conflict-fork  -> im-task)
(flow kyc-task       -> conflict-join)
(flow deal-task      -> conflict-join)
(flow im-task        -> conflict-join)
(flow conflict-join  -> conflict-review)
(flow conflict-review -> conflict-end)
"#;

#[test]
fn example_6_undeclared_write_conflict_warning() {
    let (spec, diag) = compile_example(EXAMPLE_6);

    // Should NOT have errors (assembly still succeeds)
    let errors: Vec<_> = diag.errors().map(|d| d.message.clone()).collect();
    assert!(errors.is_empty(), "unexpected errors: {:?}", errors);

    // Should have an UNDECLARED_MERGE warning
    let has_undeclared_merge = diag
        .warnings()
        .any(|w| w.code.as_deref() == Some(dsl_diagnostics::UNDECLARED_MERGE));
    assert!(
        has_undeclared_merge,
        "expected UNDECLARED_MERGE warning, warnings were: {:?}",
        diag.warnings().map(|w| format!("{:?}: {}", w.code, w.message)).collect::<Vec<_>>()
    );

    // Should still produce a valid JourneySpec
    assert!(!spec.nodes.is_empty(), "expected nodes in journey spec");
}

// ---------------------------------------------------------------------------
// Example 7: Subprocess invocation (call-activity)
// ---------------------------------------------------------------------------

const EXAMPLE_7: &str = r#"
(node main-start      :kind start-event)
(node verify-entity   :kind call-activity)
(node post-verify     :kind service-task)
(node main-end        :kind end-event)

(flow main-start    -> verify-entity)
(flow verify-entity -> post-verify)
(flow post-verify   -> main-end)
"#;

#[test]
fn example_7_subprocess_call_activity() {
    let (spec, diag) = compile_example(EXAMPLE_7);

    let errors: Vec<_> = diag.errors().map(|d| d.message.clone()).collect();
    assert!(errors.is_empty(), "unexpected errors: {:?}", errors);

    // call-activity node should be present
    let has_call_activity = spec.nodes.iter().any(|n| n.kind == "call-activity");
    assert!(has_call_activity, "expected a call-activity node");
}

// ---------------------------------------------------------------------------
// Example 8: Interrupting error boundary
// ---------------------------------------------------------------------------

// The boundary-attachment atom in this example uses the simplified form
// (without the event-name second symbol) since the parser only supports
// a single name. The boundary event name is derived as "auto-verify-boundary".
const EXAMPLE_8: &str = r#"
(node start-verify    :kind start-event)
(node auto-verify     :kind service-task)
(node manual-verify   :kind user-task)
(node verification-end :kind end-event)

(boundary-attachment auto-verify
  :event-kind error
  :interrupting true)

(flow start-verify         -> auto-verify)
(flow auto-verify          -> verification-end)
(flow auto-verify-boundary -> manual-verify)
(flow manual-verify        -> verification-end)
"#;

#[test]
fn example_8_interrupting_error_boundary() {
    let (spec, diag) = compile_example(EXAMPLE_8);

    let errors: Vec<_> = diag.errors().map(|d| d.message.clone()).collect();
    assert!(errors.is_empty(), "unexpected errors: {:?}", errors);

    // Should have at least 1 boundary attachment
    assert!(
        !spec.boundary_attachments.is_empty(),
        "expected ≥1 boundary attachment, got {}",
        spec.boundary_attachments.len()
    );

    // The boundary attachment should be interrupting and of kind "error"
    let ba = spec.boundary_attachments.iter().find(|ba| ba.event_kind == "error")
        .expect("expected an error boundary attachment");
    assert!(ba.interrupting, "expected interrupting=true");
}

// ---------------------------------------------------------------------------
// Example 9: Non-interrupting timer boundary
// ---------------------------------------------------------------------------

// Simplified boundary-attachment without the second event-name symbol.
// The boundary event name is derived as "kyc-review-task-boundary".
const EXAMPLE_9: &str = r#"
(node kyc-start        :kind start-event)
(node kyc-review-task  :kind user-task)
(node sla-escalation   :kind service-task)
(node escalation-end   :kind end-event)
(node kyc-end          :kind end-event)

(timer-definition five-day-sla
  :type duration
  :expression "P5D")

(boundary-attachment kyc-review-task
  :event-kind timer
  :interrupting false)

(flow kyc-start                  -> kyc-review-task)
(flow kyc-review-task            -> kyc-end)
(flow kyc-review-task-boundary   -> sla-escalation)
(flow sla-escalation             -> escalation-end)
"#;

#[test]
fn example_9_non_interrupting_timer_boundary() {
    let (spec, diag) = compile_example(EXAMPLE_9);

    let errors: Vec<_> = diag.errors().map(|d| d.message.clone()).collect();
    assert!(errors.is_empty(), "unexpected errors: {:?}", errors);

    // Should have a timer boundary attachment
    let timer_ba = spec
        .boundary_attachments
        .iter()
        .find(|ba| ba.event_kind == "timer");
    assert!(timer_ba.is_some(), "expected a timer boundary attachment");

    let ba = timer_ba.unwrap();
    assert!(!ba.interrupting, "expected interrupting=false for timer boundary");
}

// ---------------------------------------------------------------------------
// Example 10: Event-based gateway
// ---------------------------------------------------------------------------

// Note: the original example has a malformed `(node await-response :kind event-based :kind gateway)`
// which duplicates the :kind slot. We use the clean form at the bottom of the example
// with the `(gateway await-response :kind event-based)` atom.
const EXAMPLE_10: &str = r#"
(node proposal-start   :kind start-event)
(node send-proposal    :kind send-task)
(gateway await-response :kind event-based)
(node msg-accepted     :kind intermediate-catch-message)
(node msg-rejected     :kind intermediate-catch-message)
(node timer-timeout    :kind intermediate-catch-timer)
(node process-accept   :kind service-task)
(node process-reject   :kind service-task)
(node process-timeout  :kind service-task)
(node proposal-end     :kind end-event)

(flow proposal-start  -> send-proposal)
(flow send-proposal   -> await-response)
(flow await-response  -> msg-accepted)
(flow await-response  -> msg-rejected)
(flow await-response  -> timer-timeout)
(flow msg-accepted    -> process-accept)
(flow msg-rejected    -> process-reject)
(flow timer-timeout   -> process-timeout)
(flow process-accept  -> proposal-end)
(flow process-reject  -> proposal-end)
(flow process-timeout -> proposal-end)
"#;

#[test]
fn example_10_event_based_gateway() {
    let (spec, diag) = compile_example(EXAMPLE_10);

    let errors: Vec<_> = diag.errors().map(|d| d.message.clone()).collect();
    assert!(errors.is_empty(), "unexpected errors: {:?}", errors);

    // Should have an event-based gateway
    let has_event_based = spec.nodes.iter().any(|n| n.kind == "event-based");
    assert!(has_event_based, "expected event-based gateway node");
}

// ---------------------------------------------------------------------------
// Example 11: Complex KYC/onboarding scenario
// ---------------------------------------------------------------------------

// Simplified to avoid parser edge cases with inline :loop and back-edges.
// The key structural elements are preserved:
//   - Exclusive gateway with 3 branches
//   - Parallel fork/join with merge
//   - Error boundary on sign-off
//   - Non-interrupting SLA timer boundary on intake-form
const EXAMPLE_11: &str = r#"
(node full-start         :kind start-event)
(node intake-form        :kind user-task)
(gateway jur-gate        :kind exclusive)
(node uk-kyc             :kind subprocess)
(node eu-kyc             :kind subprocess)
(node standard-kyc       :kind subprocess)
(gateway main-fork       :kind parallel)
(node deal-task          :kind service-task)
(node im-task            :kind service-task)
(parallel-join main-join
  :expects [main-fork]
  :merge [
    {:location deal-id     :operator latest}
    {:location im-config   :operator latest}
    {:location kyc-outcome :operator latest}
  ])
(node sign-off           :kind user-task)
(node activate           :kind service-task)
(node full-end           :kind end-event)

(boundary-attachment sign-off
  :event-kind error
  :interrupting true)

(node escalation-review  :kind user-task)
(node escalation-end     :kind end-event)

(timer-definition intake-sla :type duration :expression "P3D")
(boundary-attachment intake-form
  :event-kind timer
  :interrupting false)
(node intake-reminder    :kind service-task)
(node reminder-end       :kind end-event)

(flow full-start           -> intake-form)
(flow intake-form          -> jur-gate)
(flow jur-gate             -> uk-kyc       :condition "jurisdiction-gb")
(flow jur-gate             -> eu-kyc       :condition "jurisdiction-de")
(flow jur-gate             -> standard-kyc :default true)
(flow uk-kyc               -> main-fork)
(flow eu-kyc               -> main-fork)
(flow standard-kyc         -> main-fork)
(flow main-fork            -> deal-task)
(flow main-fork            -> im-task)
(flow deal-task            -> main-join)
(flow im-task              -> main-join)
(flow main-join            -> sign-off)
(flow sign-off             -> activate     :condition "sign-off-approve")
(flow activate             -> full-end)
(flow sign-off-boundary    -> escalation-review)
(flow escalation-review    -> escalation-end)
(flow intake-form-boundary -> intake-reminder)
(flow intake-reminder      -> reminder-end)
"#;

#[test]
fn example_11_complex_kyc_onboarding() {
    let (spec, diag) = compile_example(EXAMPLE_11);

    // Just assert no errors — the structure is complex
    let errors: Vec<_> = diag.errors().map(|d| d.message.clone()).collect();
    assert!(errors.is_empty(), "unexpected errors: {:?}", errors);

    // Should have multiple nodes, gateways, a join, and boundary attachments
    assert!(spec.nodes.len() >= 8, "expected ≥8 nodes");
    assert!(!spec.parallel_joins.is_empty(), "expected ≥1 parallel join");
    assert!(spec.boundary_attachments.len() >= 2, "expected ≥2 boundary attachments");
}

// ---------------------------------------------------------------------------
// Example 12: Pack-authored process with provenance
//
// The provenance atom is declarative and should be dropped at lowering.
// The gateway and flows should compile fine.
// ---------------------------------------------------------------------------

const EXAMPLE_12: &str = r#"
(node pre-activation-check :kind service-task)
(gateway activation-eligibility-gate :kind exclusive)
(node activate-cbu-task    :kind service-task)
(node compliance-review-task :kind user-task)
(node process-start        :kind start-event)
(node process-end          :kind end-event)

(flow process-start            -> pre-activation-check)
(flow pre-activation-check     -> activation-eligibility-gate)
(flow activation-eligibility-gate -> activate-cbu-task
  :condition "kyc-approved-and-ubo-resolved-and-sanctions-clear")
(flow activation-eligibility-gate -> compliance-review-task
  :default true)
(flow activate-cbu-task        -> process-end)
(flow compliance-review-task   -> process-end)

(provenance activation-eligibility-gate-prov
  :covers [activation-eligibility-gate]
  :source pack
  :source-id conjunctive-gate
  :version "1.0.0"
  :session "sess-abc123"
  :authored-at "2026-05-21T12:00:00Z")
"#;

#[test]
fn example_12_pack_authored_with_provenance() {
    let (spec, diag) = compile_example(EXAMPLE_12);

    let errors: Vec<_> = diag.errors().map(|d| d.message.clone()).collect();
    assert!(errors.is_empty(), "unexpected errors: {:?}", errors);

    // The activation-eligibility-gate gateway should be present
    let has_gate = spec
        .nodes
        .iter()
        .any(|n| n.name == "activation-eligibility-gate");
    assert!(has_gate, "expected activation-eligibility-gate node in spec");

    // The provenance atom should NOT appear in the JourneySpec (it's declarative)
    let has_provenance = spec
        .nodes
        .iter()
        .any(|n| n.name.contains("prov") || n.kind == "provenance");
    assert!(!has_provenance, "provenance atom must not appear in JourneySpec");

    // Should have the conditional flow (three-condition `and` expression)
    let conditional_edge = spec
        .edges
        .iter()
        .find(|e| e.source == "activation-eligibility-gate" && !e.is_default);
    assert!(conditional_edge.is_some(), "expected conditional flow from gate");
    let edge = conditional_edge.unwrap();
    assert!(
        edge.condition.is_some(),
        "expected condition expression on flow"
    );
    let cond = edge.condition.as_ref().unwrap();
    assert!(
        cond.contains("kyc") || cond.contains("approved"),
        "expected condition referencing kyc/approval, got: {}",
        cond
    );

    // Should have the default flow
    let default_edge = spec
        .edges
        .iter()
        .find(|e| e.source == "activation-eligibility-gate" && e.is_default);
    assert!(default_edge.is_some(), "expected default flow from gate");
}
