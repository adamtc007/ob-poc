//! Tests ensuring LSP and v2 parsers produce equivalent results.
//!
//! These tests verify that the unified parser (v2) handles all DSL
//! syntax that the LSP needs to support.

use ob_poc::dsl_v2::parse_program;

const VALID_INPUTS: &[&str] = &[
    // Basic verb calls
    r#"(cbu.create :name "Fund")"#,
    r#"(cbu.create :name "Fund" :jurisdiction "LU")"#,
    r#"(cbu.create :name "Fund" :as @cbu)"#,
    r#"(entity.create-limited-company :name "Person")"#,
    // Numbers and booleans
    r#"(test.verb :count 42 :active true)"#,
    r#"(test.verb :amount -3.14 :enabled false)"#,
    // Lists
    r#"(test.verb :items ["a", "b", "c"])"#,
    r#"(test.verb :items ["a" "b" "c"])"#,
    // Nested calls
    r#"(cbu.create :name "Fund" :roles [(cbu.assign-role :entity-id @e :role "Mgr")])"#,
    // Multiple statements
    r#"
    (cbu.create :name "Fund" :as @fund)
    (entity.create-limited-company :name "Co" :as @co)
    (cbu.attach-entity :cbu-id @fund :entity-id @co :role "Manager")
    "#,
    // Comments
    r#"
    ;; This is a comment
    (cbu.create :name "Fund")
    "#,
    // Symbol references
    r#"(cbu.attach-entity :cbu-id @fund :entity-id @company)"#,
    // Dotted keywords
    r#"(test.verb :address.city "London")"#,
    // Escape sequences in strings
    r#"(test.verb :text "line1\nline2")"#,
    // Multiple nested calls
    r#"(cbu.create :name "Fund" :roles [
        (cbu.assign-role :entity-id @a :role "Manager")
        (cbu.assign-role :entity-id @b :role "Director")
    ])"#,
];

const INVALID_INPUTS: &[&str] = &[
    r#"(cbu.create :name"#,           // Unclosed paren
    r#"cbu.create :name "Fund")"#,    // Missing open paren
    r#"(cbu.create :name "unclosed"#, // Unclosed string
    r#"()"#,                          // Empty call (no verb)
];

#[test]
fn valid_inputs_parse_successfully_with_v2() {
    for input in VALID_INPUTS {
        let result = parse_program(input);
        assert!(
            result.is_ok(),
            "V2 parser should accept:\n{}\nError: {:?}",
            input,
            result.err()
        );
    }
}

#[test]
fn invalid_inputs_rejected_by_v2() {
    for input in INVALID_INPUTS {
        let result = parse_program(input);
        assert!(
            result.is_err(),
            "V2 parser should reject:\n{}\nBut got: {:?}",
            input,
            result.ok()
        );
    }
}

#[test]
fn v2_parser_captures_verb_span() {
    let input = r#"(cbu.create :name "Fund")"#;
    let program = parse_program(input).unwrap();

    if let ob_poc::dsl_v2::ast::Statement::VerbCall(vc) = &program.statements[0] {
        // verb_span should cover "cbu.create"
        let verb_text = &input[vc.verb_span.start..vc.verb_span.end];
        assert_eq!(verb_text, "cbu.create");
    } else {
        panic!("Expected VerbCall");
    }
}

#[test]
fn v2_parser_captures_argument_spans() {
    let input = r#"(cbu.create :name "Fund")"#;
    let program = parse_program(input).unwrap();

    if let ob_poc::dsl_v2::ast::Statement::VerbCall(vc) = &program.statements[0] {
        assert_eq!(vc.arguments.len(), 1);
        let arg = &vc.arguments[0];

        // key_span should cover ":name"
        let key_text = &input[arg.key_span.start..arg.key_span.end];
        assert_eq!(key_text, ":name");

        // value_span should cover "\"Fund\""
        let value_text = &input[arg.value_span.start..arg.value_span.end];
        assert_eq!(value_text, "\"Fund\"");
    } else {
        panic!("Expected VerbCall");
    }
}

#[test]
fn v2_parser_captures_as_binding_span() {
    let input = r#"(cbu.create :name "Fund" :as @mycbu)"#;
    let program = parse_program(input).unwrap();

    if let ob_poc::dsl_v2::ast::Statement::VerbCall(vc) = &program.statements[0] {
        assert_eq!(vc.as_binding, Some("mycbu".to_string()));
        assert!(vc.as_binding_span.is_some());

        let span = vc.as_binding_span.unwrap();
        let binding_text = &input[span.start..span.end];
        assert_eq!(binding_text, ":as @mycbu");
    } else {
        panic!("Expected VerbCall");
    }
}

#[test]
fn v2_parser_handles_nested_calls() {
    let input = r#"(cbu.create :name "Fund" :roles [(cbu.assign-role :entity-id @e :role "Mgr")])"#;
    let program = parse_program(input).unwrap();

    if let ob_poc::dsl_v2::ast::Statement::VerbCall(vc) = &program.statements[0] {
        let roles_arg = vc.arguments.iter().find(|a| a.key.canonical() == "roles");
        assert!(roles_arg.is_some());

        if let ob_poc::dsl_v2::ast::Value::List(items) = &roles_arg.unwrap().value {
            assert_eq!(items.len(), 1);
            assert!(matches!(
                items[0],
                ob_poc::dsl_v2::ast::Value::NestedCall(_)
            ));
        } else {
            panic!("Expected List for :roles");
        }
    } else {
        panic!("Expected VerbCall");
    }
}
