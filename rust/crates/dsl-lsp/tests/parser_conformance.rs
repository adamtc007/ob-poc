//! Tests ensuring LSP and v2 parsers produce equivalent results.
//!
//! These tests verify that the unified parser (v2) handles all DSL
//! syntax that the LSP needs to support.
//!
//! The parser produces a raw AST where:
//! - All string values are `Literal::String`
//! - Symbol references (`@name`) are `SymbolRef`
//! - Entity references are NOT identified yet (that's the enrichment pass)

use ob_poc::dsl_v2::ast::{AstNode, Literal, Statement};
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
    // Escape sequences in strings
    r#"(test.verb :text "line1\nline2")"#,
    // Multiple nested calls
    r#"(cbu.create :name "Fund" :roles [
        (cbu.assign-role :entity-id @a :role "Manager")
        (cbu.assign-role :entity-id @b :role "Director")
    ])"#,
    // UUIDs in strings (parsed as Literal::Uuid)
    r#"(test.verb :id "550e8400-e29b-41d4-a716-446655440000")"#,
    // Nil values
    r#"(test.verb :value nil)"#,
    // Maps
    r#"(test.verb :config {:key "value" :count 42})"#,
    // Empty list
    r#"(test.verb :items [])"#,
    // Mixed list types
    r#"(test.verb :mixed [42 "text" true @ref])"#,
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
fn v2_parser_captures_span() {
    let input = r#"(cbu.create :name "Fund")"#;
    let program = parse_program(input).unwrap();

    if let Statement::VerbCall(vc) = &program.statements[0] {
        // span should cover the entire verb call
        assert_eq!(vc.span.start, 0);
        assert_eq!(vc.span.end, input.len());

        // Verify domain and verb parsed correctly
        assert_eq!(vc.domain, "cbu");
        assert_eq!(vc.verb, "create");
    } else {
        panic!("Expected VerbCall");
    }
}

#[test]
fn v2_parser_captures_argument_spans() {
    let input = r#"(cbu.create :name "Fund")"#;
    let program = parse_program(input).unwrap();

    if let Statement::VerbCall(vc) = &program.statements[0] {
        assert_eq!(vc.arguments.len(), 1);
        let arg = &vc.arguments[0];

        // Key should be "name" (without colon)
        assert_eq!(arg.key, "name");

        // Argument span should be within the verb call
        assert!(arg.span.start > 0);
        assert!(arg.span.end <= input.len());

        // Value should be a string literal
        assert_eq!(arg.value.as_string(), Some("Fund"));
    } else {
        panic!("Expected VerbCall");
    }
}

#[test]
fn v2_parser_captures_binding() {
    let input = r#"(cbu.create :name "Fund" :as @mycbu)"#;
    let program = parse_program(input).unwrap();

    if let Statement::VerbCall(vc) = &program.statements[0] {
        assert_eq!(vc.binding, Some("mycbu".to_string()));
    } else {
        panic!("Expected VerbCall");
    }
}

#[test]
fn v2_parser_handles_nested_calls() {
    let input = r#"(cbu.create :name "Fund" :roles [(cbu.assign-role :entity-id @e :role "Mgr")])"#;
    let program = parse_program(input).unwrap();

    if let Statement::VerbCall(vc) = &program.statements[0] {
        let roles_arg = vc.arguments.iter().find(|a| a.key == "roles");
        assert!(roles_arg.is_some(), "Should have :roles argument");

        if let AstNode::List { items, .. } = &roles_arg.unwrap().value {
            assert_eq!(items.len(), 1);
            assert!(
                matches!(items[0], AstNode::Nested(_)),
                "Expected Nested verb call in list"
            );
        } else {
            panic!("Expected List for :roles");
        }
    } else {
        panic!("Expected VerbCall");
    }
}

#[test]
fn v2_parser_handles_symbol_refs() {
    let input = r#"(cbu.attach-entity :cbu-id @fund :entity-id @company)"#;
    let program = parse_program(input).unwrap();

    if let Statement::VerbCall(vc) = &program.statements[0] {
        // Check @fund is a SymbolRef
        let cbu_arg = vc.arguments.iter().find(|a| a.key == "cbu-id").unwrap();
        assert!(cbu_arg.value.is_symbol_ref());
        assert_eq!(cbu_arg.value.as_symbol(), Some("fund"));

        // Check @company is a SymbolRef
        let entity_arg = vc.arguments.iter().find(|a| a.key == "entity-id").unwrap();
        assert!(entity_arg.value.is_symbol_ref());
        assert_eq!(entity_arg.value.as_symbol(), Some("company"));
    } else {
        panic!("Expected VerbCall");
    }
}

#[test]
fn v2_parser_handles_literals() {
    let input = r#"(test.verb :str "hello" :int 42 :dec 3.14 :bool true :null nil)"#;
    let program = parse_program(input).unwrap();

    if let Statement::VerbCall(vc) = &program.statements[0] {
        // String
        let str_arg = vc.arguments.iter().find(|a| a.key == "str").unwrap();
        assert_eq!(str_arg.value.as_string(), Some("hello"));

        // Integer
        let int_arg = vc.arguments.iter().find(|a| a.key == "int").unwrap();
        assert_eq!(int_arg.value.as_integer(), Some(42));

        // Decimal
        let dec_arg = vc.arguments.iter().find(|a| a.key == "dec").unwrap();
        assert!(dec_arg.value.as_decimal().is_some());

        // Boolean
        let bool_arg = vc.arguments.iter().find(|a| a.key == "bool").unwrap();
        assert_eq!(bool_arg.value.as_boolean(), Some(true));

        // Null
        let null_arg = vc.arguments.iter().find(|a| a.key == "null").unwrap();
        assert!(matches!(null_arg.value, AstNode::Literal(Literal::Null)));
    } else {
        panic!("Expected VerbCall");
    }
}

#[test]
fn v2_parser_handles_uuid_strings() {
    let input = r#"(test.verb :id "550e8400-e29b-41d4-a716-446655440000")"#;
    let program = parse_program(input).unwrap();

    if let Statement::VerbCall(vc) = &program.statements[0] {
        let id_arg = vc.arguments.iter().find(|a| a.key == "id").unwrap();
        // UUID strings should be parsed as Literal::Uuid
        if let AstNode::Literal(Literal::Uuid(uuid)) = &id_arg.value {
            assert_eq!(uuid.to_string(), "550e8400-e29b-41d4-a716-446655440000");
        } else {
            panic!("Expected Uuid literal, got {:?}", id_arg.value);
        }
    } else {
        panic!("Expected VerbCall");
    }
}

#[test]
fn v2_parser_handles_multiple_statements() {
    let input = r#"
        (cbu.create :name "Fund" :as @fund)
        (entity.create-limited-company :name "Co" :as @co)
    "#;
    let program = parse_program(input).unwrap();

    // Filter out comments
    let verb_calls: Vec<_> = program
        .statements
        .iter()
        .filter(|s| matches!(s, Statement::VerbCall(_)))
        .collect();

    assert_eq!(verb_calls.len(), 2);
}

#[test]
fn v2_parser_handles_comments() {
    let input = r#"
        ;; This is a comment
        (cbu.create :name "Fund")
    "#;
    let program = parse_program(input).unwrap();

    let has_comment = program
        .statements
        .iter()
        .any(|s| matches!(s, Statement::Comment(_)));
    let has_verb = program
        .statements
        .iter()
        .any(|s| matches!(s, Statement::VerbCall(_)));

    assert!(has_comment, "Should have comment");
    assert!(has_verb, "Should have verb call");
}

#[test]
fn v2_parser_handles_maps() {
    let input = r#"(test.verb :config {:name "Test" :count 42})"#;
    let program = parse_program(input).unwrap();

    if let Statement::VerbCall(vc) = &program.statements[0] {
        let config_arg = vc.arguments.iter().find(|a| a.key == "config").unwrap();
        if let AstNode::Map { entries, .. } = &config_arg.value {
            assert_eq!(entries.len(), 2);

            let name_entry = entries.iter().find(|(k, _)| k == "name");
            assert!(name_entry.is_some());
            assert_eq!(name_entry.unwrap().1.as_string(), Some("Test"));
        } else {
            panic!("Expected Map for :config");
        }
    } else {
        panic!("Expected VerbCall");
    }
}

#[test]
fn v2_parser_handles_empty_list() {
    let input = r#"(test.verb :items [])"#;
    let program = parse_program(input).unwrap();

    if let Statement::VerbCall(vc) = &program.statements[0] {
        let items_arg = vc.arguments.iter().find(|a| a.key == "items").unwrap();
        if let AstNode::List { items, .. } = &items_arg.value {
            assert!(items.is_empty());
        } else {
            panic!("Expected List for :items");
        }
    } else {
        panic!("Expected VerbCall");
    }
}

#[test]
fn v2_parser_preserves_string_content() {
    // Test that strings with special content are preserved
    let input = r#"(test.verb :text "Hello \"World\" with\nnewline")"#;
    let program = parse_program(input).unwrap();

    if let Statement::VerbCall(vc) = &program.statements[0] {
        let text_arg = vc.arguments.iter().find(|a| a.key == "text").unwrap();
        let text = text_arg.value.as_string().unwrap();
        assert!(text.contains("World"));
        assert!(text.contains('\n'));
    } else {
        panic!("Expected VerbCall");
    }
}
