//! DAG / compiler golden snapshot tests — Tranche 1 regression baseline.
//!
//! 20 representative multi-step programs. For each, we snapshot the full
//! `CompiledSteps` structure so any change in how the compiler walks the
//! AST or preserves binding information is caught immediately.
//!
//! The dsl-core compiler (`compile_to_steps`) is deliberately thin — it
//! emits one `CompileStep` per `VerbCall`, carrying the parsed VerbCall and
//! source statement index. Dependency ordering lives at plan-build time in
//! the ob-poc crate. These tests therefore focus on verifying:
//!
//!   1. Step count matches the number of VerbCall statements.
//!   2. Each step carries the correct verb FQN and binding.
//!   3. Source statement indices are correct.
//!
//! Snapshots use `assert_debug_snapshot!` because `CompileStep` and
//! `CompiledSteps` do not implement `serde::Serialize`.

use dsl_core::{compiler::compile_to_steps, parser::parse_program};

/// Parse source and compile to steps.  Panics if parsing fails.
fn compile(source: &str) -> dsl_core::compiler::CompiledSteps {
    let program = parse_program(source).expect("parse failed");
    compile_to_steps(&program)
}

// =============================================================================
// Linear chains via @-binding (A produces @x → B consumes @x)
// =============================================================================

#[test]
fn dag_create_cbu_then_load() {
    let source = r#"
        (cbu.create :name "Allianz GI" :jurisdiction "LU" :as @cbu)
        (session.load-cbu :name "Allianz GI")
    "#;
    let compiled = compile(source);
    insta::assert_debug_snapshot!("dag_create_cbu_then_load", compiled);
}

#[test]
fn dag_create_entity_assign_to_cbu() {
    let source = r#"
        (entity.create :entity-type "company" :name "Apex Capital Ltd" :as @entity)
        (cbu.assign-role :cbu-id @cbu :entity-id @entity :role "DIRECTOR")
    "#;
    let compiled = compile(source);
    insta::assert_debug_snapshot!("dag_create_entity_assign_to_cbu", compiled);
}

#[test]
fn dag_cbu_then_kyc_case() {
    let source = r#"
        (cbu.create :name "BlackRock EMEA" :jurisdiction "IE" :as @cbu)
        (kyc-case.create :cbu-id @cbu :case-type "standard" :as @case)
    "#;
    let compiled = compile(source);
    insta::assert_debug_snapshot!("dag_cbu_then_kyc_case", compiled);
}

#[test]
fn dag_changeset_compose_validate_publish() {
    let source = r#"
        (changeset.compose :title "Add passport attribute" :as @cs)
        (changeset.validate :changeset-id @cs)
        (governance.publish :changeset-id @cs :dry-run false)
    "#;
    let compiled = compile(source);
    insta::assert_debug_snapshot!("dag_changeset_compose_validate_publish", compiled);
}

#[test]
fn dag_deal_create_link_advance() {
    let source = r#"
        (deal.create :client-name "Fidelity International" :product "custody" :as @deal)
        (deal.link-cbu :deal-id @deal :cbu-id @cbu)
        (deal.advance-stage :deal-id @deal :to "bac_approved")
    "#;
    let compiled = compile(source);
    insta::assert_debug_snapshot!("dag_deal_create_link_advance", compiled);
}

// =============================================================================
// Parallel-safe steps (no @-binding dependency)
// =============================================================================

#[test]
fn dag_two_independent_screenings() {
    let source = r#"
        (screening.pep :entity-id @director)
        (screening.sanctions :entity-id @director)
    "#;
    let compiled = compile(source);
    insta::assert_debug_snapshot!("dag_two_independent_screenings", compiled);
}

#[test]
fn dag_independent_entity_creates() {
    let source = r#"
        (entity.create-proper-person :first-name "John" :last-name "Smith" :as @person1)
        (entity.create-proper-person :first-name "Jane" :last-name "Doe" :as @person2)
        (entity.create-limited-company :name "Apex Capital Ltd" :as @company)
    "#;
    let compiled = compile(source);
    insta::assert_debug_snapshot!("dag_independent_entity_creates", compiled);
}

// =============================================================================
// Multi-step onboarding flows
// =============================================================================

#[test]
fn dag_full_onboarding_flow() {
    let source = r#"
        (cbu.create :name "Generali Asset Mgmt" :jurisdiction "DE" :as @cbu)
        (entity.create-limited-company :name "Generali AM GmbH" :as @entity)
        (cbu.assign-role :cbu-id @cbu :entity-id @entity :role "ASSET_MANAGER")
        (kyc-case.create :cbu-id @cbu :case-type "standard" :as @case)
        (screening.pep :entity-id @entity)
        (screening.sanctions :entity-id @entity)
    "#;
    let compiled = compile(source);
    insta::assert_debug_snapshot!("dag_full_onboarding_flow", compiled);
}

#[test]
fn dag_kyc_evidence_and_advance() {
    let source = r#"
        (kyc-case.create :cbu-id @cbu :case-type "enhanced" :as @case)
        (document.solicit :cbu-id @cbu :doc-types ["AML_POLICY" "FATCA_CERT"])
        (kyc-case.advance :case-id @case :to-stage "review")
    "#;
    let compiled = compile(source);
    insta::assert_debug_snapshot!("dag_kyc_evidence_and_advance", compiled);
}

#[test]
fn dag_trading_profile_setup() {
    let source = r#"
        (trading-profile.set-instrument :cbu-id @cbu :instrument-type "equity" :venue "LSE" :enabled true)
        (trading-profile.set-instrument :cbu-id @cbu :instrument-type "bond" :venue "LuxSE" :enabled true)
        (trading-profile.enable :cbu-id @cbu :instrument "fx" :enabled false)
    "#;
    let compiled = compile(source);
    insta::assert_debug_snapshot!("dag_trading_profile_setup", compiled);
}

// =============================================================================
// KYC + deal cross-domain programs
// =============================================================================

#[test]
fn dag_deal_and_kyc_parallel() {
    let source = r#"
        (deal.create :client-name "Invesco Ltd" :product "fund-admin" :as @deal)
        (kyc-case.create :cbu-id @cbu :case-type "standard" :as @case)
        (deal.link-cbu :deal-id @deal :cbu-id @cbu)
        (kyc-case.advance :case-id @case :to-stage "approval")
    "#;
    let compiled = compile(source);
    insta::assert_debug_snapshot!("dag_deal_and_kyc_parallel", compiled);
}

#[test]
fn dag_rate_card_negotiation() {
    let source = r#"
        (deal.create :client-name "BNP Paribas AM" :product "custody" :as @deal)
        (deal.set-rate-card :deal-id @deal :rate-card-id @rc :effective-from "2026-01-01")
        (deal.close :deal-id @deal :reason "completed")
    "#;
    let compiled = compile(source);
    insta::assert_debug_snapshot!("dag_rate_card_negotiation", compiled);
}

// =============================================================================
// Programs with @-slot bindings flowing multiple hops
// =============================================================================

#[test]
fn dag_entity_then_screening_then_case() {
    let source = r#"
        (entity.create-proper-person :first-name "Hans" :last-name "Müller" :as @director)
        (screening.pep :entity-id @director)
        (screening.adverse-media :entity-id @director :depth "full")
        (kyc-case.create :cbu-id @cbu :case-type "standard" :as @case)
    "#;
    let compiled = compile(source);
    insta::assert_debug_snapshot!("dag_entity_then_screening_then_case", compiled);
}

#[test]
fn dag_attribute_governance_pipeline() {
    let source = r#"
        (changeset.compose :title "Add risk attributes Q2 2026" :as @cs)
        (attribute.define :id "attr.risk.country_risk" :display-name "Country Risk" :category "risk" :value-type "string")
        (attribute.define-internal :id "attr.system.risk_ts" :display-name "Risk Timestamp" :category "resource" :value-type "datetime")
        (changeset.validate :changeset-id @cs)
        (governance.publish :changeset-id @cs :dry-run false)
    "#;
    let compiled = compile(source);
    insta::assert_debug_snapshot!("dag_attribute_governance_pipeline", compiled);
}

// =============================================================================
// Edge cases
// =============================================================================

#[test]
fn dag_single_step() {
    let source = r#"(session.start :mode "new")"#;
    let compiled = compile(source);
    insta::assert_debug_snapshot!("dag_single_step", compiled);
}

#[test]
fn dag_comment_then_verb() {
    let source = r#"
        ;; Kick off onboarding for Allianz GI LU
        (cbu.create :name "Allianz GI LU" :jurisdiction "LU" :as @cbu)
    "#;
    let compiled = compile(source);
    insta::assert_debug_snapshot!("dag_comment_then_verb", compiled);
}

#[test]
fn dag_multiple_bindings_different_slots() {
    let source = r#"
        (cbu.create :name "Fund A" :jurisdiction "LU" :as @fundA)
        (cbu.create :name "Fund B" :jurisdiction "IE" :as @fundB)
        (cbu.create :name "Fund C" :jurisdiction "DE" :as @fundC)
    "#;
    let compiled = compile(source);
    insta::assert_debug_snapshot!("dag_multiple_bindings_different_slots", compiled);
}

#[test]
fn dag_screening_full_suite() {
    let source = r#"
        (screening.pep :entity-id @entity :provider "worldcheck")
        (screening.sanctions :entity-id @entity)
        (screening.adverse-media :entity-id @entity :depth "full")
        (screening.refresh :entity-id @entity :force true)
    "#;
    let compiled = compile(source);
    insta::assert_debug_snapshot!("dag_screening_full_suite", compiled);
}

#[test]
fn dag_registry_introspection_flow() {
    let source = r#"
        (registry.list :kind "attribute" :limit 50)
        (registry.discover-dsl :domain "cbu" :verb "create")
    "#;
    let compiled = compile(source);
    insta::assert_debug_snapshot!("dag_registry_introspection_flow", compiled);
}

#[test]
fn dag_session_navigation_sequence() {
    let source = r#"
        (session.start :mode "new")
        (session.load-cbu :name "Allianz GI")
        (view.universe)
        (view.cbu :mode "trading")
    "#;
    let compiled = compile(source);
    insta::assert_debug_snapshot!("dag_session_navigation_sequence", compiled);
}
