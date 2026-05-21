//! @-slot binding assertion tests — Tranche 1 regression baseline.
//!
//! Verifies that every major @-slot binding type is captured correctly in the
//! parsed `VerbCall.binding` field. These are structural assertions, not
//! snapshot tests — they are intentionally more brittle so that any parser
//! change that silently drops a binding is caught immediately.

use dsl_core::{ast::Statement, parser::parse_program};

/// Helper: parse source and extract the first VerbCall.
fn first_verb(source: &str) -> dsl_core::VerbCall {
    let program = parse_program(source).expect("parse failed");
    for stmt in program.statements {
        if let Statement::VerbCall(vc) = stmt {
            return vc;
        }
    }
    panic!("No VerbCall found in: {source}");
}

// =============================================================================
// @cbu binding
// =============================================================================

#[test]
fn at_cbu_binding_captured() {
    let source = r#"(cbu.create :name "Allianz GI" :jurisdiction "LU" :as @cbu)"#;
    let vc = first_verb(source);
    assert_eq!(
        vc.binding.as_deref(),
        Some("cbu"),
        "expected @cbu binding on cbu.create"
    );
}

#[test]
fn at_cbu_binding_full_name() {
    let source = r#"(cbu.create :name "Generali Fund" :jurisdiction "DE" :as @generali_fund)"#;
    let vc = first_verb(source);
    assert_eq!(
        vc.binding.as_deref(),
        Some("generali_fund"),
        "expected @generali_fund binding"
    );
}

// =============================================================================
// @entity binding
// =============================================================================

#[test]
fn at_entity_binding_captured() {
    let source =
        r#"(entity.create :entity-type "company" :name "HSBC Holdings plc" :as @entity)"#;
    let vc = first_verb(source);
    assert_eq!(
        vc.binding.as_deref(),
        Some("entity"),
        "expected @entity binding on entity.create"
    );
}

#[test]
fn at_person_binding_captured() {
    let source = r#"(entity.create-proper-person :first-name "John" :last-name "Smith" :as @person)"#;
    let vc = first_verb(source);
    assert_eq!(
        vc.binding.as_deref(),
        Some("person"),
        "expected @person binding on entity.create-proper-person"
    );
}

#[test]
fn at_company_binding_captured() {
    let source =
        r#"(entity.create-limited-company :name "Apex Capital Ltd" :jurisdiction "KY" :as @company)"#;
    let vc = first_verb(source);
    assert_eq!(
        vc.binding.as_deref(),
        Some("company"),
        "expected @company binding on entity.create-limited-company"
    );
}

// =============================================================================
// @case / @kyc-case binding
// =============================================================================

#[test]
fn at_case_binding_captured() {
    let source = r#"(kyc-case.create :cbu-id @cbu :case-type "standard" :as @case)"#;
    let vc = first_verb(source);
    assert_eq!(
        vc.binding.as_deref(),
        Some("case"),
        "expected @case binding on kyc-case.create"
    );
}

#[test]
fn at_kyc_binding_hyphenated() {
    let source = r#"(kyc-case.create :cbu-id @cbu :case-type "enhanced" :as @kyc-case)"#;
    let vc = first_verb(source);
    assert_eq!(
        vc.binding.as_deref(),
        Some("kyc-case"),
        "expected @kyc-case binding with hyphen in name"
    );
}

// =============================================================================
// @deal binding
// =============================================================================

#[test]
fn at_deal_binding_captured() {
    let source =
        r#"(deal.create :client-name "Fidelity International" :product "custody" :as @deal)"#;
    let vc = first_verb(source);
    assert_eq!(
        vc.binding.as_deref(),
        Some("deal"),
        "expected @deal binding on deal.create"
    );
}

// =============================================================================
// @changeset binding
// =============================================================================

#[test]
fn at_changeset_binding_captured() {
    let source = r#"(changeset.compose :title "My changeset" :as @cs)"#;
    let vc = first_verb(source);
    assert_eq!(
        vc.binding.as_deref(),
        Some("cs"),
        "expected @cs binding on changeset.compose"
    );
}

// =============================================================================
// No binding (verb without :as)
// =============================================================================

#[test]
fn no_binding_when_absent() {
    let source = r#"(session.start :mode "new")"#;
    let vc = first_verb(source);
    assert!(
        vc.binding.is_none(),
        "expected no binding on session.start without :as"
    );
}

#[test]
fn no_binding_on_screening_verb() {
    let source = r#"(screening.pep :entity-id @entity)"#;
    let vc = first_verb(source);
    assert!(
        vc.binding.is_none(),
        "expected no binding on screening.pep without :as"
    );
}

// =============================================================================
// $-sigil binding (alternative to @)
// =============================================================================

#[test]
fn dollar_sigil_binding_captured() {
    let source = r#"(cbu.create :name "Dollar Fund" :jurisdiction "US" :as $cbu)"#;
    let vc = first_verb(source);
    assert_eq!(
        vc.binding.as_deref(),
        Some("cbu"),
        "expected binding captured from $ sigil just like @ sigil"
    );
}

// =============================================================================
// @-symbol in argument values (not bindings)
// =============================================================================

#[test]
fn at_symbol_in_arg_is_not_binding() {
    let source = r#"(cbu.assign-role :cbu-id @cbu :entity-id @entity :role "DIRECTOR")"#;
    let vc = first_verb(source);
    // The :cbu-id and :entity-id args use @cbu and @entity as symbol refs,
    // but the verb itself has no :as binding.
    assert!(
        vc.binding.is_none(),
        "symbol refs in args should not produce a verb binding"
    );
    // Verify both args captured their symbol refs
    let cbu_arg = vc.arguments.iter().find(|a| a.key == "cbu-id").unwrap();
    let entity_arg = vc.arguments.iter().find(|a| a.key == "entity-id").unwrap();
    assert_eq!(cbu_arg.value.as_symbol(), Some("cbu"));
    assert_eq!(entity_arg.value.as_symbol(), Some("entity"));
}

// =============================================================================
// Multi-statement: bindings on correct statements only
// =============================================================================

#[test]
fn bindings_on_correct_statements_in_multi_step() {
    let source = r#"
        (cbu.create :name "Allianz" :jurisdiction "LU" :as @cbu)
        (kyc-case.create :cbu-id @cbu :case-type "standard" :as @case)
        (screening.pep :entity-id @entity)
    "#;
    let program = parse_program(source).expect("parse failed");
    let verb_calls: Vec<_> = program
        .statements
        .iter()
        .filter_map(|s| {
            if let Statement::VerbCall(vc) = s {
                Some(vc)
            } else {
                None
            }
        })
        .collect();

    assert_eq!(verb_calls.len(), 3, "expected 3 verb calls");
    assert_eq!(
        verb_calls[0].binding.as_deref(),
        Some("cbu"),
        "first step should bind @cbu"
    );
    assert_eq!(
        verb_calls[1].binding.as_deref(),
        Some("case"),
        "second step should bind @case"
    );
    assert!(
        verb_calls[2].binding.is_none(),
        "third step has no :as binding"
    );
}

// =============================================================================
// Binding name with underscores
// =============================================================================

#[test]
fn binding_with_underscores() {
    let source = r#"(cbu.create :name "My Fund" :as @my_fund)"#;
    let vc = first_verb(source);
    assert_eq!(
        vc.binding.as_deref(),
        Some("my_fund"),
        "binding name with underscores should be captured verbatim"
    );
}

// =============================================================================
// Domain / verb fields preserved alongside binding
// =============================================================================

#[test]
fn binding_does_not_corrupt_domain_or_verb() {
    let source = r#"(entity.create-proper-person :first-name "Hans" :last-name "Müller" :as @director)"#;
    let vc = first_verb(source);
    assert_eq!(vc.domain, "entity");
    assert_eq!(vc.verb, "create-proper-person");
    assert_eq!(vc.binding.as_deref(), Some("director"));
}
