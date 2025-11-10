//! V3.1 DSL Parser Module
//!
//! Pure V3.1 implementation with unified S-expression syntax for multi-domain workflows.
//! Supports Document Library and ISDA domain verbs with AttributeID-as-Type pattern.

pub mod debug_test;
pub mod idiomatic_parser;
pub mod phase5_integration_test;
pub mod semantic_agent_integration_test;
pub mod v31_integration_tests;

use crate::{Form, Program};
use nom::error::VerboseError;

// Re-export the main parsing functions for V3.1
pub use idiomatic_parser::{
    parse_form, parse_identifier, parse_program as parse_program_internal, parse_string_literal,
    parse_value, parse_verb_form,
};

/// Main V3.1 DSL parsing function
///
/// Parses complete DSL programs with unified (verb :key value) syntax
/// Supports all domains: Document, ISDA, KYC, UBO, Onboarding, Compliance, Graph
pub fn parse_program(input: &str) -> Result<Program, VerboseError<&str>> {
    idiomatic_parser::parse_program(input)
}

#[cfg(test)]
mod v31_tests {
    use super::*;
    use crate::{Form, Key, Literal, Value, VerbForm};

    #[test]
    fn test_v31_document_catalog() {
        let dsl = r#"(document.catalog :document-id "doc-001" :document-type "CONTRACT")"#;

        let result = parse_program(dsl);
        assert!(
            result.is_ok(),
            "Failed to parse document.catalog: {:?}",
            result.err()
        );

        let forms = result.unwrap();
        assert_eq!(forms.len(), 1);

        match &forms[0] {
            Form::Verb(VerbForm { verb, .. }) => {
                assert_eq!(verb, "document.catalog");
            }
            _ => panic!("Expected verb form"),
        }
    }

    #[test]
    fn test_v31_isda_establish_master() {
        let dsl = r#"(isda.establish_master :agreement-id "ISDA-001" :party-a "entity-a" :party-b "entity-b")"#;

        let result = parse_program(dsl);
        assert!(
            result.is_ok(),
            "Failed to parse isda.establish_master: {:?}",
            result.err()
        );

        let forms = result.unwrap();
        assert_eq!(forms.len(), 1);

        match &forms[0] {
            Form::Verb(VerbForm { verb, .. }) => {
                assert_eq!(verb, "isda.establish_master");
            }
            _ => panic!("Expected verb form"),
        }
    }

    #[test]
    fn test_v31_entity_with_map() {
        let dsl = r#"(entity :id "test-001" :label "Company" :props {:legal-name "Test Corp"})"#;

        let result = parse_program(dsl);
        assert!(
            result.is_ok(),
            "Failed to parse entity with map: {:?}",
            result.err()
        );

        let forms = result.unwrap();
        assert_eq!(forms.len(), 1);

        match &forms[0] {
            Form::Verb(VerbForm { verb, pairs }) => {
                assert_eq!(verb, "entity");
                assert_eq!(pairs.len(), 3); // :id, :label, :props
            }
            _ => panic!("Expected verb form"),
        }
    }

    #[test]
    fn test_v31_comments() {
        let dsl = r#"
        ;; V3.1 DSL with comments
        (entity :id "test" :label "Company")
        "#;

        let result = parse_program(dsl);
        assert!(
            result.is_ok(),
            "Failed to parse DSL with comments: {:?}",
            result.err()
        );

        let forms = result.unwrap();
        assert_eq!(forms.len(), 2); // comment + verb

        match (&forms[0], &forms[1]) {
            (Form::Comment(_), Form::Verb(VerbForm { verb, .. })) => {
                assert_eq!(verb, "entity");
            }
            _ => panic!("Expected comment then verb form"),
        }
    }

    #[test]
    fn test_v31_multi_verb_sequence() {
        let dsl = r#"
        (document.catalog :document-id "doc-001" :document-type "CONTRACT")
        (isda.establish_master :agreement-id "ISDA-001" :version "2002")
        (entity :id "test-001" :label "Company")
        "#;

        let result = parse_program(dsl);
        assert!(
            result.is_ok(),
            "Failed to parse multi-verb sequence: {:?}",
            result.err()
        );

        let forms = result.unwrap();
        let verb_forms: Vec<_> = forms
            .iter()
            .filter_map(|f| match f {
                Form::Verb(vf) => Some(vf),
                _ => None,
            })
            .collect();

        assert_eq!(verb_forms.len(), 3);
        assert_eq!(verb_forms[0].verb, "document.catalog");
        assert_eq!(verb_forms[1].verb, "isda.establish_master");
        assert_eq!(verb_forms[2].verb, "entity");
    }
}
