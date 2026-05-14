//! Acceptance harness for EOP-VS-LANG-BIND-001 scoped runbook bindings.
//!
//! This harness intentionally sits in `dsl-core` so it is fast and database-free.
//! The current parser surface uses `:as @alias`; these cases map that existing
//! symbol form onto the runbook-binding semantics in the design note:
//! authoring symbols are allowed before finalisation, but executable output must
//! be fully concrete and contain no authoring aliases.
//!
//! These tests exercise the first compiler/finalisation slice.

use dsl_core::ast::{AstNode, Program, Statement};
use dsl_core::compiler::compile_scoped_runbook_bindings;
use dsl_core::ops::Op;
use dsl_core::parser::parse_program;

fn parse(source: &str) -> Program {
    parse_program(source).expect("harness source must parse")
}

fn compile_error_messages(source: &str) -> Vec<String> {
    compile_scoped_runbook_bindings(&parse(source))
        .errors
        .into_iter()
        .map(|error| error.message)
        .collect()
}

fn program_contains_symbol_ref(program: &Program) -> bool {
    program.statements.iter().any(|statement| match statement {
        Statement::VerbCall(call) => call
            .arguments
            .iter()
            .any(|argument| node_contains_symbol_ref(&argument.value)),
        Statement::Comment(_) => false,
    })
}

fn node_contains_symbol_ref(node: &AstNode) -> bool {
    match node {
        AstNode::SymbolRef { .. } => true,
        AstNode::List { items, .. } => items.iter().any(node_contains_symbol_ref),
        AstNode::Map { entries, .. } => entries
            .iter()
            .any(|(_key, value)| node_contains_symbol_ref(value)),
        AstNode::Nested(verb) => verb
            .arguments
            .iter()
            .any(|argument| node_contains_symbol_ref(&argument.value)),
        _ => false,
    }
}

fn op_contains_authoring_binding(op: &Op) -> bool {
    match op {
        Op::EnsureEntity { binding, .. }
        | Op::CreateCase { binding, .. }
        | Op::CreateWorkstream { binding, .. } => binding.is_some(),
        _ => false,
    }
}

#[test]
fn scoped_binding_create_cbu_then_downstream_use_lowers_without_aliases() {
    let source = r#"
        (cbu.create :name "BlackRock" :jurisdiction "GB" :as @cbu)
        (cbu.assign-role :cbu-id @cbu :entity-id "existing-director" :role "DIRECTOR")
    "#;

    let authored = parse(source);
    assert!(
        program_contains_symbol_ref(&authored),
        "authored form should contain authoring aliases before finalisation"
    );

    let compiled = compile_scoped_runbook_bindings(&authored);
    assert!(
        compiled.is_ok(),
        "valid scoped binding program should compile: {:?}",
        compiled.errors
    );

    assert_eq!(compiled.ops.len(), 2);
    assert!(
        compiled
            .ops
            .iter()
            .all(|op| !op_contains_authoring_binding(op)),
        "lowered executable ops must not retain :as authoring bindings: {:?}",
        compiled.ops
    );

    match &compiled.ops[1] {
        Op::LinkRole { cbu, entity, .. } => {
            assert_eq!(cbu.entity_type, "cbu");
            assert_eq!(entity.entity_type, "entity");
        }
        other => panic!("expected downstream role link op, got {other:?}"),
    }
}

#[test]
fn scoped_binding_accepts_dollar_placeholder_aliases() {
    let authored = parse(
        r#"
        (cbu.create :name "BlackRock" :jurisdiction "GB" :as $cbu)
        (cbu.assign-role :cbu-id $cbu :entity-id "existing-director" :role "DIRECTOR")
        "#,
    );

    let compiled = compile_scoped_runbook_bindings(&authored);

    assert!(
        compiled.is_ok(),
        "$ placeholder alias should compile through scoped binding validation: {:?}",
        compiled.errors
    );
}

#[test]
fn scoped_binding_duplicate_alias_is_rejected() {
    let errors = compile_error_messages(
        r#"
        (cbu.create :name "BlackRock" :as @cbu)
        (cbu.create :name "Vanguard" :as @cbu)
    "#,
    );

    assert!(
        errors.iter().any(|message| {
            message.contains("Duplicate binding") || message.contains("@cbu is already defined")
        }),
        "duplicate alias should be rejected with a binding diagnostic: {errors:?}"
    );
}

#[test]
fn scoped_binding_undefined_alias_is_rejected() {
    let errors = compile_error_messages(
        r#"(cbu.assign-role :cbu-id @cbu :entity-id @director :role "DIRECTOR")"#,
    );

    assert!(
        errors.iter().any(|message| {
            message.contains("Undefined binding")
                || message.contains("undefined symbol '@cbu'")
                || message.contains("undefined symbol '@director'")
        }),
        "undefined aliases should be rejected: {errors:?}"
    );
}

#[test]
fn scoped_binding_forward_reference_is_rejected() {
    let errors = compile_error_messages(
        r#"
        (cbu.assign-role :cbu-id @cbu :entity-id "existing-director" :role "DIRECTOR")
        (cbu.create :name "BlackRock" :as @cbu)
    "#,
    );

    assert!(
        errors.iter().any(|message| {
            message.contains("used before it is declared") || message.contains("undefined symbol")
        }),
        "forward references should be rejected in source order: {errors:?}"
    );
}

#[test]
fn scoped_binding_wrong_type_is_rejected() {
    let errors = compile_error_messages(
        r#"
        (cbu.create :name "BlackRock" :as @cbu)
        (cbu.assign-role :cbu-id @cbu :entity-id @cbu :role "DIRECTOR")
    "#,
    );

    assert!(
        errors
            .iter()
            .any(|message| message.contains("Type mismatch") || message.contains("expects")),
        "wrong-type alias use should be rejected: {errors:?}"
    );
}

#[test]
fn scoped_binding_on_non_create_verb_is_rejected() {
    let errors = compile_error_messages(
        r#"
        (cbu.create :name "BlackRock" :as @cbu)
        (cbu.assign-role :cbu-id @cbu :entity-id "existing-director" :role "DIRECTOR" :as @role_link)
    "#,
    );

    assert!(
        errors.iter().any(|message| {
            message.contains("does not declare an entity output")
                || message.contains("Only verbs with")
                || message.contains("non-create")
        }),
        "bindings on non-create verbs should be rejected: {errors:?}"
    );
}
