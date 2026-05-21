//! AST golden snapshot tests — Tranche 1 regression baseline.
//!
//! 50 representative verb calls across all major domains. These snapshots must
//! remain stable throughout the Tranche 3 verb reshape; any drift here signals
//! a behavioural change in the parser or AST types.
//!
//! Run once to create snapshots:
//!   INSTA_UPDATE=new cargo test -p dsl-core --test ast_golden
//!
//! Snapshots are stored in tests/snapshots/ and checked into git.

use dsl_core::parser::parse_program;

// =============================================================================
// CBU domain — 5 verbs
// =============================================================================

#[test]
fn ast_cbu_create() {
    let source = r#"(cbu.create :name "Allianz Global Investors" :jurisdiction "LU")"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_cbu_create", program);
}

#[test]
fn ast_cbu_create_with_binding() {
    let source = r#"(cbu.create :name "BlackRock EMEA Fund" :jurisdiction "IE" :as @cbu)"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_cbu_create_with_binding", program);
}

#[test]
fn ast_cbu_assign_role() {
    let source = r#"(cbu.assign-role :cbu-id @cbu :entity-id @entity :role "DIRECTOR")"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_cbu_assign_role", program);
}

#[test]
fn ast_cbu_load() {
    let source = r#"(session.load-cbu :name "Allianz GI" :as @scope)"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_cbu_load", program);
}

#[test]
fn ast_cbu_update_status() {
    let source = r#"(cbu.update-status :cbu-id @cbu :status "active")"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_cbu_update_status", program);
}

// =============================================================================
// KYC domain — 5 verbs
// =============================================================================

#[test]
fn ast_kyc_case_create() {
    let source = r#"(kyc-case.create :cbu-id @cbu :case-type "standard" :as @case)"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_kyc_case_create", program);
}

#[test]
fn ast_kyc_case_advance() {
    let source = r#"(kyc-case.advance :case-id @case :to-stage "screening")"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_kyc_case_advance", program);
}

#[test]
fn ast_kyc_case_close() {
    let source = r#"(kyc-case.close :case-id @case :outcome "approved" :rationale "Full documentation received")"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_kyc_case_close", program);
}

#[test]
fn ast_kyc_coverage_check() {
    let source = r#"(kyc-coverage.check :cbu-id @cbu :target "full_kyc")"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_kyc_coverage_check", program);
}

#[test]
fn ast_kyc_tollgate_evaluate() {
    let source = r#"(kyc-tollgate.evaluate :case-id @case :strict true)"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_kyc_tollgate_evaluate", program);
}

// =============================================================================
// Deal domain — 5 verbs
// =============================================================================

#[test]
fn ast_deal_create() {
    let source = r#"(deal.create :client-name "Fidelity International" :product "custody" :as @deal)"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_deal_create", program);
}

#[test]
fn ast_deal_link_cbu() {
    let source = r#"(deal.link-cbu :deal-id @deal :cbu-id @cbu)"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_deal_link_cbu", program);
}

#[test]
fn ast_deal_advance_stage() {
    let source = r#"(deal.advance-stage :deal-id @deal :to "bac_approved")"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_deal_advance_stage", program);
}

#[test]
fn ast_deal_set_rate_card() {
    let source = r#"(deal.set-rate-card :deal-id @deal :rate-card-id @rc :effective-from "2026-01-01")"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_deal_set_rate_card", program);
}

#[test]
fn ast_deal_close() {
    let source = r#"(deal.close :deal-id @deal :reason "completed")"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_deal_close", program);
}

// =============================================================================
// Screening domain — 5 verbs
// =============================================================================

#[test]
fn ast_screening_pep() {
    let source = r#"(screening.pep :entity-id @entity :provider "worldcheck")"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_screening_pep", program);
}

#[test]
fn ast_screening_sanctions() {
    let source = r#"(screening.sanctions :entity-id @entity)"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_screening_sanctions", program);
}

#[test]
fn ast_screening_adverse_media() {
    let source = r#"(screening.adverse-media :entity-id @entity :depth "full")"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_screening_adverse_media", program);
}

#[test]
fn ast_screening_refresh() {
    let source = r#"(screening.refresh :entity-id @entity :force true)"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_screening_refresh", program);
}

#[test]
fn ast_screening_batch() {
    let source =
        r#"(screening.batch :entity-ids ["id-1" "id-2" "id-3"] :types ["pep" "sanctions"])"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_screening_batch", program);
}

// =============================================================================
// Entity domain — 5 verbs
// =============================================================================

#[test]
fn ast_entity_create() {
    let source = r#"(entity.create :entity-type "company" :name "HSBC Holdings plc" :as @entity)"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_entity_create", program);
}

#[test]
fn ast_entity_create_proper_person() {
    let source = r#"(entity.create-proper-person :first-name "John" :last-name "Smith" :nationality "GB" :as @person)"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_entity_create_proper_person", program);
}

#[test]
fn ast_entity_create_limited_company() {
    let source = r#"(entity.create-limited-company :name "Apex Capital Ltd" :jurisdiction "KY" :as @company)"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_entity_create_limited_company", program);
}

#[test]
fn ast_entity_update() {
    let source = r#"(entity.update :entity-id @entity :field "address" :value "123 Main St")"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_entity_update", program);
}

#[test]
fn ast_entity_link() {
    let source = r#"(entity.link :source-id @entity :target-id @cbu :relationship "ASSET_OWNER")"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_entity_link", program);
}

// =============================================================================
// Session / navigation verbs — 5 verbs
// =============================================================================

#[test]
fn ast_session_start() {
    let source = r#"(session.start :mode "new")"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_session_start", program);
}

#[test]
fn ast_session_info() {
    let source = r#"(session.info)"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_session_info", program);
}

#[test]
fn ast_view_universe() {
    let source = r#"(view.universe)"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_view_universe", program);
}

#[test]
fn ast_view_cbu() {
    let source = r#"(view.cbu :mode "ubo")"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_view_cbu", program);
}

#[test]
fn ast_session_undo() {
    let source = r#"(session.undo)"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_session_undo", program);
}

// =============================================================================
// Governance / registry verbs — 5 verbs
// =============================================================================

#[test]
fn ast_changeset_compose() {
    let source = r#"(changeset.compose :title "Add KYC attributes" :rationale "Phase 3 expansion" :as @cs)"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_changeset_compose", program);
}

#[test]
fn ast_changeset_validate() {
    let source = r#"(changeset.validate :changeset-id @cs)"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_changeset_validate", program);
}

#[test]
fn ast_governance_publish() {
    let source = r#"(governance.publish :changeset-id @cs :dry-run false)"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_governance_publish", program);
}

#[test]
fn ast_registry_discover() {
    let source = r#"(registry.discover-dsl :domain "cbu" :verb "create")"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_registry_discover", program);
}

#[test]
fn ast_registry_list() {
    let source = r#"(registry.list :kind "attribute" :limit 50)"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_registry_list", program);
}

// =============================================================================
// Attribute verbs — 5 verbs
// =============================================================================

#[test]
fn ast_attribute_define() {
    let source = r#"(attribute.define :id "attr.identity.passport_number" :display-name "Passport Number" :category "identity" :value-type "string")"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_attribute_define", program);
}

#[test]
fn ast_attribute_define_internal() {
    let source = r#"(attribute.define-internal :id "attr.system.last_sync" :display-name "Last Sync" :category "resource" :value-type "datetime")"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_attribute_define_internal", program);
}

#[test]
fn ast_attribute_update_internal() {
    let source = r#"(attribute.update-internal :id "attr.system.last_sync" :field "description" :value "Timestamp of last external sync")"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_attribute_update_internal", program);
}

#[test]
fn ast_attribute_define_derived() {
    let source = r#"(attribute.define-derived :id "attr.derived.kyc_score" :display-name "KYC Risk Score" :derivation-fqn "derivation.kyc_score_v1")"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_attribute_define_derived", program);
}

#[test]
fn ast_attribute_bridge_to_semos() {
    let source = r#"(attribute.bridge-to-semos :store-id @attr :force false)"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_attribute_bridge_to_semos", program);
}

// =============================================================================
// Pattern variations — 5 cases
// =============================================================================

#[test]
fn ast_pattern_multi_arg_symbol_chain() {
    // Multi-arg with @-bindings flowing from previous steps
    let source = r#"(trading-profile.set-instrument :cbu-id @cbu :instrument-type "equity" :venue "LSE" :enabled true)"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_pattern_multi_arg_symbol_chain", program);
}

#[test]
fn ast_pattern_numeric_args() {
    // Numeric (integer and decimal) argument values
    let source = r#"(billing.set-rate :cbu-id @cbu :basis-points 25 :minimum-fee 150.00 :currency "GBP")"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_pattern_numeric_args", program);
}

#[test]
fn ast_pattern_boolean_arg() {
    // Boolean flags
    let source = r#"(trading-profile.enable :cbu-id @cbu :instrument "bond" :enabled false :override true)"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_pattern_boolean_arg", program);
}

#[test]
fn ast_pattern_list_arg() {
    // List-valued argument
    let source = r#"(document.solicit :cbu-id @cbu :doc-types ["AML_POLICY" "FATCA_CERT" "CRS_FORM"] :deadline "2026-06-30")"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_pattern_list_arg", program);
}

#[test]
fn ast_pattern_map_arg() {
    // Map-valued argument
    let source = r#"(cbu.create :name "Test Fund" :metadata {:region "EMEA" :tier "premium"})"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_pattern_map_arg", program);
}

// =============================================================================
// Edge cases — 5 cases
// =============================================================================

#[test]
fn ast_edge_comment_only() {
    // Program consisting only of a comment
    let source = r#";; This is a planning comment for the operator"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_edge_comment_only", program);
}

#[test]
fn ast_edge_empty_program() {
    // Empty source produces empty program
    let source = "";
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_edge_empty_program", program);
}

#[test]
fn ast_edge_verb_no_args() {
    // Verb call with zero arguments
    let source = r#"(session.info)"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_edge_verb_no_args", program);
}

#[test]
fn ast_edge_whitespace_variations() {
    // Irregular whitespace including tabs and multiple spaces
    let source = "(cbu.create\t:name\t\t\"Whitespace Fund\"\n  :jurisdiction   \"LU\")";
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_edge_whitespace_variations", program);
}

#[test]
fn ast_edge_nil_value() {
    // nil literal as argument value
    let source = r#"(cbu.update :cbu-id @cbu :parent-id nil)"#;
    let program = parse_program(source).expect("parse failed");
    insta::assert_debug_snapshot!("ast_edge_nil_value", program);
}
