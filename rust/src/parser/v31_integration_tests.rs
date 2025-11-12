//! V3.1 Integration Tests for Document Library and ISDA DSL
//!
//! These tests validate the complete V3.1 parser functionality including:
//! - Document Library verbs (8 verbs)
//! - ISDA Derivative verbs (12 verbs)
//! - Multi-domain workflow integration
//! - V3.1 unified S-expression syntax

use crate::parser::parse_program;
use crate::parser_ast::{Form, Key, Literal, Value, VerbForm};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_catalog_verb() {
        let dsl = r#"(document.catalog
          :document-id "doc-test-001"
          :document-type "CONTRACT"
          :issuer "test-authority"
          :title "Test Document for Phase 4"
          :jurisdiction "US"
          :language "EN")"#;

        let result = parse_program(dsl);
        assert!(
            result.is_ok(),
            "Failed to parse document.catalog: {:?}",
            result.err()
        );

        let forms = result.unwrap();
        assert_eq!(forms.len(), 1);

        match &forms[0] {
            Form::Verb(VerbForm { verb, pairs }) => {
                assert_eq!(verb, "document.catalog");

                // Keys are parsed without the colon prefix
                let doc_id_key = Key::new("document-id");
                let doc_type_key = Key::new("document-type");
                let issuer_key = Key::new("issuer");

                assert!(pairs.contains_key(&doc_id_key));
                assert!(pairs.contains_key(&doc_type_key));
                assert!(pairs.contains_key(&issuer_key));

                // Validate actual values
                if let Some(Value::Literal(Literal::String(doc_id))) = pairs.get(&doc_id_key) {
                    assert_eq!(doc_id, "doc-test-001");
                }
            }
            _ => panic!("Expected verb form"),
        }
    }

    #[test]
    fn test_document_verify_verb() {
        let dsl = r#"(document.verify
          :document-id "doc-test-001"
          :verification-method "DIGITAL_SIGNATURE"
          :verification-result "AUTHENTIC"
          :verified-at "2024-11-10T14:30:00Z")"#;

        let result = parse_program(dsl);
        assert!(result.is_ok(), "Failed to parse document.verify");

        let forms = result.unwrap();
        assert_eq!(forms.len(), 1);

        match &forms[0] {
            Form::Verb(VerbForm { verb, pairs }) => {
                assert_eq!(verb, "document.verify");
                assert_eq!(pairs.len(), 4);
            }
            _ => panic!("Expected verb form"),
        }
    }

    #[test]
    fn test_isda_establish_master_verb() {
        let dsl = r#"(isda.establish_master
          :agreement-id "ISDA-TEST-001"
          :party-a "entity-a"
          :party-b "entity-b"
          :version "2002"
          :governing-law "NY"
          :agreement-date "2024-01-15"
          :multicurrency true)"#;

        let result = parse_program(dsl);
        assert!(result.is_ok(), "Failed to parse isda.establish_master");

        let forms = result.unwrap();
        assert_eq!(forms.len(), 1);

        match &forms[0] {
            Form::Verb(VerbForm { verb, pairs }) => {
                assert_eq!(verb, "isda.establish_master");

                let agreement_id_key = Key::new("agreement-id");
                let multicurrency_key = Key::new("multicurrency");

                // Validate string parameter
                if let Some(Value::Literal(Literal::String(agreement_id))) =
                    pairs.get(&agreement_id_key)
                {
                    assert_eq!(agreement_id, "ISDA-TEST-001");
                }

                // Validate boolean parameter
                if let Some(Value::Literal(Literal::Boolean(multicurrency))) =
                    pairs.get(&multicurrency_key)
                {
                    assert_eq!(*multicurrency, true);
                }
            }
            _ => panic!("Expected verb form"),
        }
    }

    #[test]
    fn test_isda_execute_trade_verb() {
        let dsl = r#"(isda.execute_trade
          :trade-id "TRADE-TEST-001"
          :master-agreement-id "ISDA-TEST-001"
          :product-type "IRS"
          :trade-date "2024-03-15"
          :notional-amount 10000000.0
          :currency "USD")"#;

        let result = parse_program(dsl);
        assert!(result.is_ok(), "Failed to parse isda.execute_trade");

        let forms = result.unwrap();
        assert_eq!(forms.len(), 1);

        match &forms[0] {
            Form::Verb(VerbForm { verb, pairs }) => {
                assert_eq!(verb, "isda.execute_trade");

                let notional_key = Key::new("notional-amount");
                if let Some(Value::Literal(Literal::Number(notional))) = pairs.get(&notional_key) {
                    assert_eq!(*notional, 10000000.0);
                }
            }
            _ => panic!("Expected verb form"),
        }
    }

    #[test]
    fn test_isda_margin_call_verb() {
        let dsl = r#"(isda.margin_call
          :call-id "MC-001"
          :csa-id "CSA-001"
          :call-date "2024-09-15"
          :calling-party "entity-a"
          :called-party "entity-b"
          :exposure-amount 8750000.0
          :call-amount 5000000.0
          :currency "USD")"#;

        let result = parse_program(dsl);
        assert!(result.is_ok(), "Failed to parse isda.margin_call");

        let forms = result.unwrap();
        assert_eq!(forms.len(), 1);

        match &forms[0] {
            Form::Verb(VerbForm { verb, pairs }) => {
                assert_eq!(verb, "isda.margin_call");
                assert_eq!(pairs.len(), 8);
            }
            _ => panic!("Expected verb form"),
        }
    }

    #[test]
    fn test_document_with_extracted_data_map() {
        let dsl = r#"(document.catalog
          :document-id "doc-complex-001"
          :document-type "TRADE_CONFIRMATION"
          :extracted-data {
            :trade-id "TRADE-001"
            :notional-amount 50000000.0
            :currency "USD"
          })"#;

        let result = parse_program(dsl);
        assert!(
            result.is_ok(),
            "Failed to parse document with extracted data"
        );

        let forms = result.unwrap();
        assert_eq!(forms.len(), 1);

        match &forms[0] {
            Form::Verb(VerbForm { verb, pairs }) => {
                assert_eq!(verb, "document.catalog");

                let extracted_data_key = Key::new("extracted-data");
                assert!(pairs.contains_key(&extracted_data_key));

                if let Some(Value::Map(extracted_data)) = pairs.get(&extracted_data_key) {
                    let trade_id_key = Key::new("trade-id");
                    let notional_key = Key::new("notional-amount");
                    let currency_key = Key::new("currency");

                    assert!(extracted_data.contains_key(&trade_id_key));
                    assert!(extracted_data.contains_key(&notional_key));
                    assert!(extracted_data.contains_key(&currency_key));
                }
            }
            _ => panic!("Expected verb form"),
        }
    }

    #[test]
    fn test_isda_with_arrays() {
        let dsl = r#"(isda.establish_csa
          :csa-id "CSA-001"
          :master-agreement-id "ISDA-001"
          :eligible-collateral ["cash_usd" "us_treasury" "uk_gilts"]
          :threshold-party-a 0.0
          :threshold-party-b 5000000.0)"#;

        let result = parse_program(dsl);
        assert!(result.is_ok(), "Failed to parse ISDA verb with arrays");

        let forms = result.unwrap();
        assert_eq!(forms.len(), 1);

        match &forms[0] {
            Form::Verb(VerbForm { verb, pairs }) => {
                assert_eq!(verb, "isda.establish_csa");

                let collateral_key = Key::new("eligible-collateral");
                assert!(pairs.contains_key(&collateral_key));

                if let Some(Value::List(collateral)) = pairs.get(&collateral_key) {
                    assert_eq!(collateral.len(), 3);

                    let cash_usd = Value::Literal(Literal::String("cash_usd".to_string()));
                    let us_treasury = Value::Literal(Literal::String("us_treasury".to_string()));
                    let uk_gilts = Value::Literal(Literal::String("uk_gilts".to_string()));

                    assert!(collateral.contains(&cash_usd));
                    assert!(collateral.contains(&us_treasury));
                    assert!(collateral.contains(&uk_gilts));
                }
            }
            _ => panic!("Expected verb form"),
        }
    }

    #[test]
    fn test_multi_domain_workflow() {
        let dsl = r#"
        ;; V3.1 Multi-domain workflow
        (document.catalog
          :document-id "doc-master-001"
          :document-type "ISDA_MASTER_AGREEMENT"
          :issuer "isda_inc")

        (isda.establish_master
          :agreement-id "ISDA-001"
          :party-a "fund-a"
          :party-b "bank-b"
          :version "2002"
          :governing-law "NY"
          :document-id "doc-master-001")

        (isda.execute_trade
          :trade-id "TRADE-001"
          :master-agreement-id "ISDA-001"
          :product-type "IRS"
          :notional-amount 25000000.0
          :currency "USD")

        (entity
          :id "fund-a"
          :label "Fund"
          :props {
            :legal-name "Alpha Fund LP"
            :jurisdiction "KY"
          })
        "#;

        let result = parse_program(dsl);
        assert!(result.is_ok(), "Failed to parse multi-domain workflow");

        let forms = result.unwrap();

        // Filter out comments to count only verb forms
        let verb_forms: Vec<_> = forms
            .iter()
            .filter_map(|form| match form {
                Form::Verb(verb_form) => Some(verb_form),
                Form::Comment(_) => None,
            })
            .collect();

        assert_eq!(verb_forms.len(), 4);

        // Verify the verb sequence
        let expected_verbs = vec![
            "document.catalog",
            "isda.establish_master",
            "isda.execute_trade",
            "entity",
        ];

        for (i, verb_form) in verb_forms.iter().enumerate() {
            assert_eq!(verb_form.verb, expected_verbs[i]);
        }

        // Validate cross-domain reference
        let isda_master = &verb_forms[1];
        let doc_id_key = Key::new("document-id");
        assert!(isda_master.pairs.contains_key(&doc_id_key));
    }

    #[test]
    fn test_complete_phase4_simple_dsl() {
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
        assert!(
            result.is_ok(),
            "Failed to parse complete Phase 4 simple DSL"
        );

        let forms = result.unwrap();

        // Filter verb forms
        let verb_forms: Vec<_> = forms
            .iter()
            .filter_map(|form| match form {
                Form::Verb(verb_form) => Some(verb_form),
                Form::Comment(_) => None,
            })
            .collect();

        assert_eq!(verb_forms.len(), 5);

        // Verify all verbs in sequence
        let expected_verbs = vec![
            "document.catalog",
            "document.verify",
            "isda.establish_master",
            "isda.execute_trade",
            "entity",
        ];

        for (i, verb_form) in verb_forms.iter().enumerate() {
            assert_eq!(
                verb_form.verb, expected_verbs[i],
                "Verb mismatch at index {}: expected {}, got {}",
                i, expected_verbs[i], verb_form.verb
            );
        }

        // Validate document.catalog has expected parameters
        let doc_catalog = &verb_forms[0];
        assert_eq!(doc_catalog.pairs.len(), 6);

        // Validate ISDA master agreement has expected parameters
        let isda_master = &verb_forms[2];
        assert_eq!(isda_master.pairs.len(), 6);

        // Validate entity has map properties
        let entity = &verb_forms[4];
        let props_key = Key::new("props");
        assert!(entity.pairs.contains_key(&props_key));

        if let Some(Value::Map(props)) = entity.pairs.get(&props_key) {
            assert_eq!(props.len(), 2); // :legal-name and :jurisdiction
        }
    }

    #[test]
    fn test_all_document_verbs() {
        let document_verbs = vec![
            "document.catalog",
            "document.verify",
            "document.extract",
            "document.link",
            "document.use",
            "document.amend",
            "document.expire",
            "document.query",
        ];

        for verb in document_verbs {
            let dsl = format!(r#"({} :test-param "test-value")"#, verb);
            let result = parse_program(&dsl);

            assert!(
                result.is_ok(),
                "Failed to parse {}: {:?}",
                verb,
                result.err()
            );

            let forms = result.unwrap();
            assert_eq!(forms.len(), 1);

            match &forms[0] {
                Form::Verb(VerbForm {
                    verb: parsed_verb, ..
                }) => {
                    assert_eq!(parsed_verb, verb);
                }
                _ => panic!("Expected verb form for {}", verb),
            }
        }
    }

    #[test]
    fn test_all_isda_verbs() {
        let isda_verbs = vec![
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

        for verb in isda_verbs {
            let dsl = format!(r#"({} :test-param "test-value")"#, verb);
            let result = parse_program(&dsl);

            assert!(
                result.is_ok(),
                "Failed to parse {}: {:?}",
                verb,
                result.err()
            );

            let forms = result.unwrap();
            assert_eq!(forms.len(), 1);

            match &forms[0] {
                Form::Verb(VerbForm {
                    verb: parsed_verb, ..
                }) => {
                    assert_eq!(parsed_verb, verb);
                }
                _ => panic!("Expected verb form for {}", verb),
            }
        }
    }

    #[test]
    fn test_error_handling_malformed_syntax() {
        let invalid_dsl = r#"(document.catalog :document-id"#; // Missing closing paren and value

        let result = parse_program(invalid_dsl);
        assert!(result.is_err(), "Should fail to parse malformed syntax");
    }

    #[test]
    fn test_v31_syntax_compliance() {
        // Test various V3.1 syntax patterns
        let test_cases = vec![
            // Basic verb with string param
            r#"(document.catalog :id "test")"#,
            // Verb with number param
            r#"(isda.execute_trade :amount 1000000.0)"#,
            // Verb with boolean param
            r#"(isda.establish_master :active true)"#,
            // Verb with array param
            r#"(test.verb :items ["a" "b" "c"])"#,
            // Verb with map param
            r#"(test.verb :data {:key "value"})"#,
            // Multiple params
            r#"(test.verb :str "test" :num 42.0 :bool false)"#,
        ];

        for (i, dsl) in test_cases.iter().enumerate() {
            let result = parse_program(dsl);
            assert!(
                result.is_ok(),
                "Test case {} failed: {}\nError: {:?}",
                i,
                dsl,
                result.err()
            );

            let forms = result.unwrap();
            assert_eq!(
                forms.len(),
                1,
                "Expected exactly one form for test case {}",
                i
            );

            match &forms[0] {
                Form::Verb(_) => {} // Success
                _ => panic!("Expected verb form for test case {}", i),
            }
        }
    }
}
