//! Validation tests for the `for-each` template combinator (Tranche 0, sub-phase 0.3).
//!
//! Tests 1–4: parser and resolution validation (no runtime needed).
//! Tests 5–7 (variable-arity instantiation) live in
//!   `bpmn-test-harness/tests/for_each_runtime.rs` because they depend on
//!   `instantiate_pack`, which in turn depends on `bpmn_test_harness`.

use dsl_diagnostics::{DiagnosticBag, INVALID_PARAMETER_NAME, UNKNOWN_TEMPLATE_PARAMETER};
use dsl_resolution::PackRegistry;

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn parse_and_resolve(src: &str) -> (PackRegistry, DiagnosticBag) {
    let (sf, parse_diag) = dsl_parser::parse(src);
    let mut diag = DiagnosticBag::new();
    for d in parse_diag.diagnostics {
        diag.push(d);
    }
    let bag = dsl_ast::AtomBag::from_source_file(sf, &mut diag);
    let mut registry = PackRegistry::new();
    dsl_resolution::resolve(&bag, &mut registry, &mut diag);
    (registry, diag)
}

// ---------------------------------------------------------------------------
// Test 1: for_each_in_template_validates
//
// A pack with for-each parses and resolves without errors.
// ---------------------------------------------------------------------------

#[test]
fn for_each_in_template_validates() {
    let src = r#"
(decision-pack threshold-band-test
  :version "1.0.0"
  :description "Test for-each with list-of-map bands"
  :domain-scope [test]
  :parameters [
    {:name band-gate-name :type symbol      :required true}
    {:name bands          :type list-of-map :required true
     :description "List of {path, upper} band maps"}
  ]
  :template [
    (flow $pre-node -> ,band-gate-name)
    (for-each :var band :in bands
      (flow ,band-gate-name -> ,band.path))
  ]
  :example-utterances ["route by threshold bands"])
"#;
    let (registry, diag) = parse_and_resolve(src);
    assert!(
        !diag.has_errors(),
        "unexpected errors: {:?}",
        diag.errors().map(|d| d.message.clone()).collect::<Vec<_>>()
    );
    assert!(
        registry.lookup("threshold-band-test", "1.0.0").is_some(),
        "pack not found in registry"
    );
}

// ---------------------------------------------------------------------------
// Test 2: for_each_list_param_required
//
// for-each :in must reference a declared parameter; using an undeclared
// parameter is an error (UNKNOWN_TEMPLATE_PARAMETER).
// ---------------------------------------------------------------------------

#[test]
fn for_each_list_param_required() {
    let src = r#"
(decision-pack bad-for-each
  :version "1.0.0"
  :description "for-each referencing undeclared list param"
  :domain-scope [test]
  :parameters [
    {:name gate-name :type symbol :required true}
  ]
  :template [
    (for-each :var band :in undeclared-bands
      (flow ,gate-name -> ,band.path))
  ]
  :example-utterances ["bad pack"])
"#;
    let (_, diag) = parse_and_resolve(src);
    assert!(diag.has_errors(), "expected error for undeclared :in param");
    let has_code = diag
        .errors()
        .any(|d| d.code.as_deref() == Some(UNKNOWN_TEMPLATE_PARAMETER));
    assert!(has_code, "expected UNKNOWN_TEMPLATE_PARAMETER code");
}

// ---------------------------------------------------------------------------
// Test 3: for_each_accessor_dot_valid
//
// `,band.path` and `,band.exit-path` are valid inside a for-each body where
// `band` is the declared loop variable.
// ---------------------------------------------------------------------------

#[test]
fn for_each_accessor_dot_valid() {
    let src = r#"
(decision-pack accessor-test
  :version "1.0.0"
  :description "Test dot accessor forms inside for-each"
  :domain-scope [test]
  :parameters [
    {:name gate-name :type symbol      :required true}
    {:name bands     :type list-of-map :required true}
  ]
  :template [
    (for-each :var band :in bands
      (flow ,gate-name -> ,band.path)
      (flow ,gate-name -> ,band.exit-path))
  ]
  :example-utterances ["accessor dot test"])
"#;
    let (_, diag) = parse_and_resolve(src);
    assert!(
        !diag.has_errors(),
        "unexpected errors for valid dot accessors: {:?}",
        diag.errors().map(|d| d.message.clone()).collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// Test 4: parameter_name_with_dot_is_error
//
// A parameter named `gate.name` is rejected (dots reserved for accessor syntax).
// ---------------------------------------------------------------------------

#[test]
fn parameter_name_with_dot_is_error() {
    let src = r#"
(decision-pack dot-param
  :version "1.0.0"
  :description "Parameter with a dot in its name"
  :domain-scope [test]
  :parameters [
    {:name gate.name :type symbol :required true}
  ]
  :template [
    (flow $pre-node -> ,gate.name)
  ]
  :example-utterances ["dot param test"])
"#;
    let (_, diag) = parse_and_resolve(src);
    assert!(
        diag.has_errors(),
        "expected error for dotted parameter name"
    );
    let has_code = diag
        .errors()
        .any(|d| d.code.as_deref() == Some(INVALID_PARAMETER_NAME));
    assert!(has_code, "expected INVALID_PARAMETER_NAME code");
}
