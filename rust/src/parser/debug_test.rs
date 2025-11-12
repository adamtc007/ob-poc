//! Debug test to examine actual parser output for V3.1 alignment
//!
//! This test helps us understand what the parser is actually producing
//! so we can align V3.1 EBNF, DSL examples, and NOM parser.

use crate::parser::parse_program;
use crate::parser_ast::{Form, Key, Literal, Value, VerbForm};;

#[cfg(test)]
mod debug_tests {
    use super::*;

    #[test]
    fn debug_simple_document_verb() {
        let dsl = r#"(document.catalog :document-id "test-001")"#;

        let result = parse_program(dsl);
        println!("=== DOCUMENT.CATALOG DEBUG ===");
        println!("Input: {}", dsl);
        println!("Result: {:#?}", result);

        if let Ok(forms) = &result {
            println!("Forms count: {}", forms.len());
            for (i, form) in forms.iter().enumerate() {
                println!("Form {}: {:#?}", i, form);
            }
        }
    }

    #[test]
    fn debug_simple_isda_verb() {
        let dsl = r#"(isda.establish_master :agreement-id "ISDA-001")"#;

        let result = parse_program(dsl);
        println!("=== ISDA.ESTABLISH_MASTER DEBUG ===");
        println!("Input: {}", dsl);
        println!("Result: {:#?}", result);

        if let Ok(forms) = &result {
            println!("Forms count: {}", forms.len());
            for (i, form) in forms.iter().enumerate() {
                println!("Form {}: {:#?}", i, form);
            }
        }
    }

    #[test]
    fn debug_existing_entity_verb() {
        let dsl = r#"(entity :id "test-001" :label "Company")"#;

        let result = parse_program(dsl);
        println!("=== ENTITY DEBUG (Known Working) ===");
        println!("Input: {}", dsl);
        println!("Result: {:#?}", result);

        if let Ok(forms) = &result {
            println!("Forms count: {}", forms.len());
            for (i, form) in forms.iter().enumerate() {
                println!("Form {}: {:#?}", i, form);

                match form {
                    Form::Verb(VerbForm { verb, pairs }) => {
                        println!("  Verb: {}", verb);
                        println!("  Pairs count: {}", pairs.len());
                        for (key, value) in pairs {
                            println!("    Key: {:?}", key);
                            println!("    Value: {:?}", value);
                        }
                    }
                    Form::Comment(comment) => {
                        println!("  Comment: {}", comment);
                    }
                }
            }
        }
    }

    #[test]
    fn debug_key_creation() {
        println!("=== KEY CREATION DEBUG ===");

        let key1 = Key::new(":document-id");
        let key2 = Key::new("document-id");
        let key3 = Key::new("document-id");

        println!("Key::new(':document-id'): {:?}", key1);
        println!("Key::new('document-id'): {:?}", key2);
        println!("Key::from_parts(['document-id']): {:?}", key3);
    }

    #[test]
    fn debug_map_syntax() {
        let dsl = r#"(entity :id "test" :props {:name "Test Corp"})"#;

        let result = parse_program(dsl);
        println!("=== MAP SYNTAX DEBUG ===");
        println!("Input: {}", dsl);
        println!("Result: {:#?}", result);

        if let Ok(forms) = &result {
            if let Some(Form::Verb(VerbForm { pairs, .. })) = forms.first() {
                let props_key = Key::new(":props");
                println!("Looking for key: {:?}", props_key);
                println!("Available keys: {:?}", pairs.keys().collect::<Vec<_>>());

                if let Some(props_value) = pairs.get(&props_key) {
                    println!("Props value: {:?}", props_value);
                } else {
                    println!("Props key not found!");
                }
            }
        }
    }

    #[test]
    fn debug_all_new_verbs() {
        let verbs_to_test = vec![
            // Document verbs
            "document.catalog",
            "document.verify",
            "document.extract",
            "document.link",
            "document.use",
            "document.amend",
            "document.expire",
            "document.query",
            // ISDA verbs
            "isda.establish_master",
            "isda.establish_csa",
            "isda.execute_trade",
            "isda.margin_call",
            "isda.post_collateral",
            "isda.value_portfolio",
            "isda.declare_termination_event",
            "isda.close_out",
            "isda.amend_agreement",
            "isda.novate_trade",
            "isda.dispute",
            "isda.manage_netting_set",
        ];

        println!("=== TESTING ALL NEW VERBS ===");

        for verb in verbs_to_test {
            let dsl = format!(r#"({} :test-param "test-value")"#, verb);
            let result = parse_program(&dsl);

            match result {
                Ok(forms) => {
                    if forms.len() > 0 {
                        match &forms[0] {
                            Form::Verb(VerbForm {
                                verb: parsed_verb, ..
                            }) => {
                                if parsed_verb == verb {
                                    println!("✅ {} - PARSED CORRECTLY", verb);
                                } else {
                                    println!("❌ {} - VERB MISMATCH: got {}", verb, parsed_verb);
                                }
                            }
                            _ => println!("❌ {} - NOT A VERB FORM", verb),
                        }
                    } else {
                        println!("❌ {} - NO FORMS PARSED", verb);
                    }
                }
                Err(err) => {
                    println!("❌ {} - PARSE ERROR: {:?}", verb, err);
                }
            }
        }
    }

    #[test]
    fn debug_simple_array() {
        let dsl = r#"(test.verb :items ["a" "b" "c"])"#;

        let result = parse_program(dsl);
        println!("=== ARRAY SYNTAX DEBUG ===");
        println!("Input: {}", dsl);
        println!("Result: {:#?}", result);

        if let Ok(forms) = &result {
            println!("Forms count: {}", forms.len());
            for (i, form) in forms.iter().enumerate() {
                println!("Form {}: {:#?}", i, form);
            }
        }
    }

    #[test]
    fn debug_array_parsing_step_by_step() {
        use crate::parser::idiomatic_parser::{parse_list_value, parse_value};

        println!("=== STEP BY STEP ARRAY PARSING DEBUG ===");

        // Test 1: Direct array parsing
        let array_input = r#"["a" "b" "c"]"#;
        let result1 = parse_list_value(array_input);
        println!("Direct array '{}': {:?}", array_input, result1);

        // Test 2: Array as value parsing
        let result2 = parse_value(array_input);
        println!("Array as value '{}': {:?}", array_input, result2);

        // Test 3: Array with commas
        let array_comma = r#"["a", "b", "c"]"#;
        let result3 = parse_list_value(array_comma);
        println!("Array with commas '{}': {:?}", array_comma, result3);

        // Test 4: Simple verb with array
        let simple_verb = r#"(test.verb :items ["simple"])"#;
        let result4 = parse_program(simple_verb);
        println!("Simple verb with array '{}': {:?}", simple_verb, result4);

        // Test 5: The failing ISDA example
        let isda_example = r#"(isda.establish_csa :eligible-collateral ["cash_usd"])"#;
        let result5 = parse_program(isda_example);
        println!("ISDA example '{}': {:?}", isda_example, result5);
    }

    #[test]
    fn debug_key_parsing_issue() {
        let dsl = r#"(test.verb :customer-id "test" :multi.part.key "value")"#;

        let result = parse_program(dsl);
        println!("=== KEY PARSING DEBUG ===");
        println!("Input: {}", dsl);
        println!("Result: {:#?}", result);

        if let Ok(forms) = &result {
            if let Some(Form::Verb(VerbForm { pairs, .. })) = forms.first() {
                println!("All keys in pairs:");
                for (key, value) in pairs {
                    println!(
                        "  Key: {:?} (parts: {:?}) -> Value: {:?}",
                        key, key.parts, value
                    );
                }

                let customer_id_key = Key::new("customer-id");
                let multi_part_key = Key::new("multi.part.key");

                println!("Expected customer_id_key: {:?}", customer_id_key);
                println!("Expected multi_part_key: {:?}", multi_part_key);

                println!(
                    "Contains customer_id_key: {}",
                    pairs.contains_key(&customer_id_key)
                );
                println!(
                    "Contains multi_part_key: {}",
                    pairs.contains_key(&multi_part_key)
                );
            }
        }
    }
}

#[test]
fn debug_complete_phase4_simple_dsl() {
    let dsl = r#"
    ;; Phase 4 Simple Test DSL
    ;; Basic validation of document and ISDA verbs for parser testing

    ;; Simple document cataloging test
    (document.catalog
      :document-id "doc-test-001"
      :document-type "CONTRACT"
      :issuer "test-authority"
      :title "Test Document for Phase 4"
      :jurisdiction "US"
      :language "EN")

    ;; Simple document verification test
    (document.verify
      :document-id "doc-test-001"
      :verification-method "DIGITAL_SIGNATURE"
      :verification-result "AUTHENTIC")

    ;; Simple ISDA master agreement test
    (isda.establish_master
      :agreement-id "ISDA-TEST-001"
      :party-a "entity-a"
      :party-b "entity-b"
      :version "2002"
      :governing-law "NY"
      :agreement-date "2024-01-15")

    ;; Simple ISDA trade execution test
    (isda.execute_trade
      :trade-id "TRADE-TEST-001"
      :master-agreement-id "ISDA-TEST-001"
      :product-type "IRS"
      :trade-date "2024-03-15"
      :notional-amount 10000000.0
      :currency "USD")

    ;; Test basic entity creation (existing verb)
    (entity
      :id "test-entity-001"
      :label "Company"
      :props {
        :legal-name "Test Company Inc"
        :jurisdiction "DE"
      })
    "#;

    let result = parse_program(dsl);
    println!("=== COMPLETE PHASE 4 SIMPLE DSL DEBUG ===");
    println!("Input DSL: {}", dsl);
    println!("Result: {:#?}", result);

    if let Ok(forms) = &result {
        println!("Total forms count: {}", forms.len());
        for (i, form) in forms.iter().enumerate() {
            match form {
                Form::Verb(VerbForm { verb, pairs }) => {
                    println!("Form {}: VERB '{}' with {} pairs", i, verb, pairs.len());
                }
                Form::Comment(comment) => {
                    println!("Form {}: COMMENT '{}'", i, comment.trim());
                }
            }
        }

        // Filter verb forms
        let verb_forms: Vec<_> = forms
            .iter()
            .filter_map(|form| match form {
                Form::Verb(verb_form) => Some(verb_form),
                Form::Comment(_) => None,
            })
            .collect();

        println!("Verb forms count: {}", verb_forms.len());
        for (i, verb_form) in verb_forms.iter().enumerate() {
            println!("  Verb {}: {}", i, verb_form.verb);
        }
    }
}
