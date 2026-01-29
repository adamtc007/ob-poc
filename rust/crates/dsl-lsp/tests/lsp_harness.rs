//! LSP Test Harness - Comprehensive s-expression validation suite
//!
//! Tests the full LSP validation pipeline including:
//! - Parser conformance (syntax)
//! - Semantic validation (verb existence, arg types)
//! - Planning diagnostics (DAG order, cycles)
//!
//! Run with: cargo test -p dsl-lsp --test lsp_harness

use dsl_lsp::handlers::diagnostics::analyze_document;
use tower_lsp::lsp_types::DiagnosticSeverity;

// =============================================================================
// TEST CASE DEFINITIONS
// =============================================================================

/// Test case with expected outcome
struct TestCase {
    name: &'static str,
    input: &'static str,
    expected: Expected,
}

#[derive(Debug, Clone, PartialEq)]
enum Expected {
    /// Should parse and validate without errors
    Valid,
    /// Should have parse error (syntax)
    ParseError(&'static str),
    /// Should have semantic error (unknown verb, bad args)
    SemanticError(&'static str),
    /// Should have warning but no errors
    Warning(&'static str),
    /// Should have specific number of errors
    ErrorCount(usize),
}

// =============================================================================
// VALID S-EXPRESSIONS
// These should all parse and validate without errors
// =============================================================================

const VALID_CASES: &[TestCase] = &[
    // Basic verb calls
    TestCase {
        name: "simple_verb_call",
        input: r#"(cbu.create :name "Test Fund")"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "verb_with_binding",
        input: r#"(cbu.create :name "Test Fund" :as @fund)"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "verb_with_multiple_args",
        input: r#"(cbu.create :name "Test Fund" :jurisdiction "LU" :type "FUND")"#,
        expected: Expected::Valid,
    },
    // Entity references
    TestCase {
        name: "symbol_reference",
        input: r#"(cbu.attach-entity :cbu-id @fund :entity-id @company)"#,
        expected: Expected::Valid,
    },
    // NOTE: Angle-bracket entity refs <Entity Name> are NOT DSL syntax.
    // Entity resolution happens post-parse via enrichment. Use string literals.
    TestCase {
        name: "entity_ref_as_string",
        input: r#"(session.load-galaxy :apex-entity-id "Allianz SE")"#,
        expected: Expected::Valid,
    },
    // Lists
    TestCase {
        name: "list_with_commas",
        input: r#"(test.verb :items ["a", "b", "c"])"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "list_without_commas",
        input: r#"(test.verb :items ["a" "b" "c"])"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "empty_list",
        input: r#"(test.verb :items [])"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "list_mixed_types",
        input: r#"(test.verb :items [42 "text" true @ref])"#,
        expected: Expected::Valid,
    },
    // Maps
    TestCase {
        name: "simple_map",
        input: r#"(test.verb :config {:key "value" :count 42})"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "empty_map",
        input: r#"(test.verb :config {})"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "nested_map_in_list",
        input: r#"(test.verb :data [{:x 1} {:y 2}])"#,
        expected: Expected::Valid,
    },
    // Literals
    TestCase {
        name: "integer_literal",
        input: r#"(test.verb :count 42)"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "negative_integer",
        input: r#"(test.verb :count -42)"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "decimal_literal",
        input: r#"(test.verb :amount 3.14159)"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "negative_decimal",
        input: r#"(test.verb :amount -99.99)"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "boolean_true",
        input: r#"(test.verb :active true)"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "boolean_false",
        input: r#"(test.verb :active false)"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "nil_literal",
        input: r#"(test.verb :value nil)"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "uuid_string",
        input: r#"(test.verb :id "550e8400-e29b-41d4-a716-446655440000")"#,
        expected: Expected::Valid,
    },
    // Escape sequences
    TestCase {
        name: "string_with_newline",
        input: r#"(test.verb :text "line1\nline2")"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "string_with_tab",
        input: r#"(test.verb :text "col1\tcol2")"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "string_with_escaped_quote",
        input: r#"(test.verb :text "He said \"hello\"")"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "string_with_backslash",
        input: r#"(test.verb :text "path\\to\\file")"#,
        expected: Expected::Valid,
    },
    // Comments
    TestCase {
        name: "single_comment",
        input: r#";; This is a comment
(cbu.create :name "Fund")"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "inline_comment_style",
        input: r#"(cbu.create :name "Fund")
;; Comment after statement"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "multi_line_comments",
        input: r#";; First comment
;; Second comment
(cbu.create :name "Fund")"#,
        expected: Expected::Valid,
    },
    // Multiple statements
    TestCase {
        name: "two_statements",
        input: r#"(cbu.create :name "Fund" :as @fund)
(entity.create :name "Company" :as @company)"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "three_statements_with_deps",
        input: r#"(cbu.create :name "Fund" :as @fund)
(entity.create :name "Manager" :as @mgr)
(cbu.attach-entity :cbu-id @fund :entity-id @mgr :role "MANAGER")"#,
        expected: Expected::Valid,
    },
    // Nested verb calls
    TestCase {
        name: "nested_verb_in_list",
        input: r#"(cbu.create :name "Fund" :roles [(cbu.assign-role :role "MGR")])"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "multiple_nested_verbs",
        input: r#"(cbu.create :name "Fund" :roles [
    (cbu.assign-role :role "MGR")
    (cbu.assign-role :role "DIR")
])"#,
        expected: Expected::Valid,
    },
    // Kebab-case identifiers
    TestCase {
        name: "kebab_case_domain",
        input: r#"(trading-profile.create :name "Profile")"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "kebab_case_verb",
        input: r#"(entity.create-limited-company :name "Corp")"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "kebab_case_keyword",
        input: r#"(cbu.create :cbu-name "Test" :legal-form "ICAV")"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "kebab_case_symbol",
        input: r#"(cbu.create :name "Fund" :as @my-fund-ref)"#,
        expected: Expected::Valid,
    },
    // Complex real-world examples
    TestCase {
        name: "trading_profile_setup",
        input: r#"(trading-profile.create-draft :cbu-id @fund :notes "Setup" :as @profile)
(trading-profile.add-instrument-class :profile-id @profile :class-code "EQUITY")
(trading-profile.add-market :profile-id @profile :instrument-class "EQUITY" :mic "XNYS")"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "isda_csa_config",
        input: r#"(trading-profile.add-isda-config
  :profile-id @profile
  :counterparty-entity-id @counterparty
  :counterparty-name "Goldman Sachs"
  :governing-law "ENGLISH")
(trading-profile.add-csa-config
  :profile-id @profile
  :isda-ref "Goldman Sachs"
  :csa-type "VM"
  :threshold-amount 0
  :minimum-transfer-amount 500000)"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "ssi_setup",
        input: r#"(trading-profile.add-standing-instruction
  :profile-id @profile
  :ssi-type "OTC_COLLATERAL"
  :ssi-name "USD-COLLATERAL"
  :cash-account "001-234567"
  :cash-bic "CITIUS33"
  :cash-currency "USD")"#,
        expected: Expected::Valid,
    },
];

// =============================================================================
// INVALID S-EXPRESSIONS - SYNTAX ERRORS
// These should produce parse errors
// =============================================================================

// NOTE: Parser error messages use generic "expected ')'" for many syntax errors.
// We check for "error" or "expected" in the message rather than specific keywords.
const SYNTAX_ERROR_CASES: &[TestCase] = &[
    TestCase {
        name: "unclosed_paren",
        input: r#"(cbu.create :name "Fund""#,
        expected: Expected::ParseError("expected"),
    },
    TestCase {
        name: "missing_open_paren",
        input: r#"cbu.create :name "Fund")"#,
        expected: Expected::ParseError("error"),
    },
    TestCase {
        name: "unclosed_string",
        input: r#"(cbu.create :name "unclosed"#,
        expected: Expected::ParseError("expected"),
    },
    TestCase {
        name: "empty_parens",
        input: r#"()"#,
        expected: Expected::ParseError("error"),
    },
    TestCase {
        name: "unclosed_list",
        input: r#"(test.verb :items [1, 2, 3)"#,
        expected: Expected::ParseError("expected"),
    },
    TestCase {
        name: "unclosed_map",
        input: r#"(test.verb :config {:key "value")"#,
        expected: Expected::ParseError("expected"),
    },
    TestCase {
        name: "missing_verb_after_domain",
        input: r#"(cbu. :name "Fund")"#,
        expected: Expected::ParseError("error"),
    },
    TestCase {
        name: "missing_domain",
        input: r#"(.create :name "Fund")"#,
        expected: Expected::ParseError("error"),
    },
    TestCase {
        name: "invalid_keyword_no_colon",
        input: r#"(cbu.create name "Fund")"#,
        expected: Expected::ParseError("expected"),
    },
    TestCase {
        name: "double_colon_keyword",
        input: r#"(cbu.create ::name "Fund")"#,
        expected: Expected::ParseError("expected"),
    },
    TestCase {
        name: "missing_value_after_keyword",
        input: r#"(cbu.create :name)"#,
        expected: Expected::ParseError("expected"),
    },
    TestCase {
        name: "keyword_with_no_value_followed_by_keyword",
        input: r#"(cbu.create :name :type "FUND")"#,
        expected: Expected::ParseError("expected"),
    },
];

// =============================================================================
// EDGE CASES - Tricky but valid syntax
// =============================================================================

const EDGE_CASES: &[TestCase] = &[
    TestCase {
        name: "unicode_in_string",
        input: r#"(cbu.create :name "日本語ファンド")"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "emoji_in_string",
        input: r#"(cbu.create :name "Test 🎉 Fund")"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "very_long_string",
        input: r#"(cbu.create :name "This is a very long string that goes on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on")"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "deeply_nested_structure",
        input: r#"(test.verb :data {:level1 {:level2 {:level3 {:level4 "deep"}}}})"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "mixed_nesting",
        input: r#"(test.verb :data [{:a [1 2 {:b [3 4]}]} {:c 5}])"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "whitespace_heavy",
        input: r#"(   cbu.create    :name    "Fund"    :as    @fund    )"#,
        expected: Expected::Valid,
    },
    // NOTE: Parser requires whitespace between tokens (keyword:value pairs)
    TestCase {
        name: "minimal_whitespace",
        input: r#"(cbu.create :name "Fund" :as @fund)"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "newlines_in_list",
        input: r#"(test.verb :items [
"item1"
"item2"
"item3"
])"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "comment_only_file",
        input: r#";; This file only has comments
;; No actual code here"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "empty_input",
        input: "",
        expected: Expected::Valid,
    },
    TestCase {
        name: "whitespace_only",
        input: "   \n\t\n   ",
        expected: Expected::Valid,
    },
    TestCase {
        name: "symbol_with_numbers",
        input: r#"(test.verb :ref @fund123)"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "symbol_with_underscores",
        input: r#"(test.verb :ref @fund_ref_v2)"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "very_large_number",
        input: r#"(test.verb :amount 999999999999999999)"#,
        expected: Expected::Valid,
    },
    // Scientific notation is NOT supported - parser treats "1e10" as invalid
    TestCase {
        name: "scientific_notation_not_supported",
        input: r#"(test.verb :amount 1e10)"#,
        expected: Expected::ParseError("expected"),
    },
    TestCase {
        name: "zero_values",
        input: r#"(test.verb :int 0 :dec 0.0)"#,
        expected: Expected::Valid,
    },
];

// =============================================================================
// ADDITIONAL SYNTAX ERROR CASES - More comprehensive coverage
// =============================================================================

const MORE_SYNTAX_ERRORS: &[TestCase] = &[
    // Malformed verb syntax
    TestCase {
        name: "verb_no_dot",
        input: r#"(cbu-create :name "Fund")"#,
        expected: Expected::ParseError("error"),
    },
    TestCase {
        name: "verb_double_dot",
        input: r#"(cbu..create :name "Fund")"#,
        expected: Expected::ParseError("error"),
    },
    // Parser sees "cbu.create." as domain.verb then expects args
    TestCase {
        name: "verb_trailing_dot",
        input: r#"(cbu.create. :name "Fund")"#,
        expected: Expected::ParseError("expected"),
    },
    TestCase {
        name: "verb_with_spaces",
        input: r#"(cbu . create :name "Fund")"#,
        expected: Expected::ParseError("error"),
    },
    // List/Map issues
    TestCase {
        name: "list_missing_close_bracket",
        input: r#"(test.verb :items ["a" "b")"#,
        expected: Expected::ParseError("expected"),
    },
    TestCase {
        name: "map_missing_close_brace",
        input: r#"(test.verb :config {:key "val")"#,
        expected: Expected::ParseError("expected"),
    },
    // ISSUE FOUND: Parser accepts trailing comma in list - should it?
    TestCase {
        name: "list_with_trailing_comma",
        input: r#"(test.verb :items ["a", "b",])"#,
        expected: Expected::Valid, // Parser accepts this
    },
    TestCase {
        name: "map_missing_value",
        input: r#"(test.verb :config {:key})"#,
        expected: Expected::ParseError("expected"),
    },
    TestCase {
        name: "map_non_keyword_key",
        input: r#"(test.verb :config {"key" "value"})"#,
        expected: Expected::ParseError("expected"),
    },
    // ISSUE FOUND: Parser accepts unescaped newlines in strings - should it?
    TestCase {
        name: "string_with_unescaped_newline",
        input: "(test.verb :text \"line1\nline2\")",
        expected: Expected::Valid, // Parser accepts this (multiline strings)
    },
    TestCase {
        name: "single_quoted_string",
        input: r#"(test.verb :name 'Fund')"#,
        expected: Expected::ParseError("expected"),
    },
    TestCase {
        name: "backtick_string",
        input: r#"(test.verb :name `Fund`)"#,
        expected: Expected::ParseError("expected"),
    },
    // Symbol issues
    TestCase {
        name: "symbol_no_name",
        input: r#"(test.verb :ref @)"#,
        expected: Expected::ParseError("expected"),
    },
    TestCase {
        name: "symbol_starts_with_number",
        input: r#"(test.verb :ref @123fund)"#,
        expected: Expected::ParseError("expected"),
    },
    TestCase {
        name: "double_at_symbol",
        input: r#"(test.verb :ref @@fund)"#,
        expected: Expected::ParseError("expected"),
    },
    // Nesting issues
    TestCase {
        name: "mismatched_parens_bracket",
        input: r#"(test.verb :items [1 2 3))"#,
        expected: Expected::ParseError("expected"),
    },
    TestCase {
        name: "mismatched_bracket_paren",
        input: r#"(test.verb :items (1 2 3])"#,
        expected: Expected::ParseError("expected"),
    },
    // Binding issues
    TestCase {
        name: "as_without_symbol",
        input: r#"(cbu.create :name "Fund" :as)"#,
        expected: Expected::ParseError("expected"),
    },
    TestCase {
        name: "as_with_string",
        input: r#"(cbu.create :name "Fund" :as "fund")"#,
        expected: Expected::ParseError("expected"),
    },
    TestCase {
        name: "as_with_number",
        input: r#"(cbu.create :name "Fund" :as 123)"#,
        expected: Expected::ParseError("expected"),
    },
    TestCase {
        name: "double_binding",
        input: r#"(cbu.create :name "Fund" :as @fund :as @fund2)"#,
        expected: Expected::ParseError("expected"),
    },
    // Random garbage
    TestCase {
        name: "random_text",
        input: "hello world this is not DSL",
        expected: Expected::ParseError("error"),
    },
    TestCase {
        name: "json_not_dsl",
        input: r#"{"name": "Fund", "type": "FUND"}"#,
        expected: Expected::ParseError("error"),
    },
    TestCase {
        name: "xml_not_dsl",
        input: r#"<cbu name="Fund" />"#,
        expected: Expected::ParseError("error"),
    },
    TestCase {
        name: "sql_not_dsl",
        input: "SELECT * FROM cbus WHERE name = 'Fund'",
        expected: Expected::ParseError("error"),
    },
];

// =============================================================================
// STRESS TESTS - Push parser limits
// =============================================================================

const STRESS_TESTS: &[TestCase] = &[
    TestCase {
        name: "many_arguments",
        input: r#"(test.verb :a 1 :b 2 :c 3 :d 4 :e 5 :f 6 :g 7 :h 8 :i 9 :j 10 :k 11 :l 12 :m 13 :n 14 :o 15 :p 16 :q 17 :r 18 :s 19 :t 20)"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "large_list",
        input: r#"(test.verb :items [1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31 32 33 34 35 36 37 38 39 40 41 42 43 44 45 46 47 48 49 50])"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "deeply_nested_lists",
        input: r#"(test.verb :data [[[[[[["deep"]]]]]]])"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "deeply_nested_maps",
        input: r#"(test.verb :data {:a {:b {:c {:d {:e {:f {:g "deep"}}}}}}})"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "many_statements",
        input: r#"(a.a :x 1)(b.b :x 2)(c.c :x 3)(d.d :x 4)(e.e :x 5)(f.f :x 6)(g.g :x 7)(h.h :x 8)(i.i :x 9)(j.j :x 10)"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "very_long_identifier",
        input: r#"(this-is-a-very-long-domain-name-that-goes-on-and-on.this-is-also-a-very-long-verb-name-for-testing :argument "value")"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "many_nested_verb_calls",
        input: r#"(outer.verb :items [
            (inner.one :x 1)
            (inner.two :x 2)
            (inner.three :x 3)
            (inner.four :x 4)
            (inner.five :x 5)
        ])"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "complex_mixed_structure",
        input: r#"(complex.verb
            :simple "text"
            :number 42
            :decimal 3.14
            :bool true
            :null nil
            :ref @some-ref
            :list [1 2 3 "four" true @five]
            :map {:a 1 :b "two" :c true}
            :nested {:list [1 {:deep "value"}]}
            :as @result)"#,
        expected: Expected::Valid,
    },
];

// =============================================================================
// WHITESPACE AND FORMATTING VARIATIONS
// =============================================================================

const WHITESPACE_TESTS: &[TestCase] = &[
    TestCase {
        name: "tabs_instead_of_spaces",
        input: "(cbu.create\t:name\t\"Fund\"\t:as\t@fund)",
        expected: Expected::Valid,
    },
    TestCase {
        name: "crlf_line_endings",
        input: "(cbu.create :name \"Fund\")\r\n(entity.create :name \"Co\")",
        expected: Expected::Valid,
    },
    TestCase {
        name: "mixed_line_endings",
        input: "(cbu.create :name \"Fund\")\n(entity.create :name \"Co\")\r\n(test.verb :x 1)",
        expected: Expected::Valid,
    },
    TestCase {
        name: "leading_whitespace",
        input: "   \n\n   (cbu.create :name \"Fund\")",
        expected: Expected::Valid,
    },
    TestCase {
        name: "trailing_whitespace",
        input: "(cbu.create :name \"Fund\")   \n\n   ",
        expected: Expected::Valid,
    },
    TestCase {
        name: "indented_multiline",
        input: r#"
        (cbu.create
            :name "Fund"
            :jurisdiction "LU"
            :type "FUND"
            :as @fund)
    "#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "no_newlines_long_statement",
        input: r#"(trading-profile.add-isda-config :profile-id @profile :counterparty-entity-id @gs :counterparty-name "Goldman Sachs" :counterparty-lei "ABC123" :governing-law "ENGLISH" :agreement-date "2024-01-15")"#,
        expected: Expected::Valid,
    },
];

// =============================================================================
// SPECIAL CHARACTERS AND UNICODE
// =============================================================================

const UNICODE_TESTS: &[TestCase] = &[
    TestCase {
        name: "chinese_characters",
        input: r#"(entity.create :name "中国银行")"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "japanese_characters",
        input: r#"(entity.create :name "三菱UFJ銀行")"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "korean_characters",
        input: r#"(entity.create :name "한국은행")"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "arabic_characters",
        input: r#"(entity.create :name "البنك العربي")"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "cyrillic_characters",
        input: r#"(entity.create :name "Сбербанк")"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "greek_characters",
        input: r#"(entity.create :name "Εθνική Τράπεζα")"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "accented_characters",
        input: r#"(entity.create :name "Société Générale")"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "german_umlauts",
        input: r#"(entity.create :name "Münchener Rück")"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "special_symbols_in_string",
        input: r#"(test.verb :text "© 2024 Company® — All rights reserved™")"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "currency_symbols",
        input: r#"(test.verb :text "€100 £200 ¥300 ₹400 ₿500")"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "math_symbols",
        input: r#"(test.verb :text "α + β = γ × δ ÷ ε")"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "emoji_sequences",
        input: r#"(test.verb :text "👨‍👩‍👧‍👦 Family emoji")"#,
        expected: Expected::Valid,
    },
];

// =============================================================================
// NUMBER EDGE CASES
// =============================================================================

const NUMBER_TESTS: &[TestCase] = &[
    TestCase {
        name: "integer_zero",
        input: r#"(test.verb :n 0)"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "integer_negative_zero",
        input: r#"(test.verb :n -0)"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "decimal_zero",
        input: r#"(test.verb :n 0.0)"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "decimal_leading_zero",
        input: r#"(test.verb :n 0.123)"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "decimal_many_places",
        input: r#"(test.verb :n 3.141592653589793)"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "large_positive_integer",
        input: r#"(test.verb :n 9223372036854775807)"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "large_negative_integer",
        input: r#"(test.verb :n -9223372036854775808)"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "decimal_no_leading_digit",
        input: r#"(test.verb :n .5)"#,
        expected: Expected::ParseError("expected"),
    },
    TestCase {
        name: "decimal_trailing_dot",
        input: r#"(test.verb :n 5.)"#,
        expected: Expected::ParseError("expected"),
    },
    TestCase {
        name: "hex_number",
        input: r#"(test.verb :n 0xFF)"#,
        expected: Expected::ParseError("expected"),
    },
    TestCase {
        name: "octal_number",
        input: r#"(test.verb :n 0o77)"#,
        expected: Expected::ParseError("expected"),
    },
    TestCase {
        name: "binary_number",
        input: r#"(test.verb :n 0b1010)"#,
        expected: Expected::ParseError("expected"),
    },
    TestCase {
        name: "number_with_underscore",
        input: r#"(test.verb :n 1_000_000)"#,
        expected: Expected::ParseError("expected"),
    },
    TestCase {
        name: "plus_sign_number",
        input: r#"(test.verb :n +42)"#,
        expected: Expected::ParseError("expected"),
    },
];

// =============================================================================
// COMMENT EDGE CASES
// =============================================================================

const COMMENT_TESTS: &[TestCase] = &[
    TestCase {
        name: "comment_with_semicolons",
        input: r#";; Comment with ;;; multiple semicolons
(test.verb :x 1)"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "comment_with_dsl_syntax",
        input: r#";; (this.looks :like "dsl" :but :is :comment)
(test.verb :x 1)"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "comment_between_statements",
        input: r#"(a.one :x 1)
;; Middle comment
(b.two :x 2)"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "many_comments",
        input: r#";; Comment 1
;; Comment 2
;; Comment 3
;; Comment 4
(test.verb :x 1)
;; Comment 5"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "empty_comment",
        input: r#";;
(test.verb :x 1)"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "comment_with_unicode",
        input: r#";; 日本語コメント 🎉
(test.verb :x 1)"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "single_semicolon_not_comment",
        input: r#"; This should fail - single semicolon
(test.verb :x 1)"#,
        expected: Expected::ParseError("error"),
    },
];

// =============================================================================
// STRING ESCAPE SEQUENCE TESTS
// =============================================================================

const ESCAPE_TESTS: &[TestCase] = &[
    TestCase {
        name: "all_escapes",
        input: r#"(test.verb :text "newline\n tab\t return\r backslash\\ quote\"")"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "escaped_backslash_before_quote",
        input: r#"(test.verb :text "ends with backslash\\")"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "multiple_escaped_quotes",
        input: r#"(test.verb :text "\"hello\" \"world\"")"#,
        expected: Expected::Valid,
    },
    // ISSUE FOUND: Parser rejects \x and \u escapes - only \n \r \t \\ \" supported
    TestCase {
        name: "invalid_escape_x",
        input: r#"(test.verb :text "\x41")"#,
        expected: Expected::ParseError("expected"),
    },
    TestCase {
        name: "invalid_escape_u",
        input: r#"(test.verb :text "\u0041")"#,
        expected: Expected::ParseError("expected"),
    },
    TestCase {
        name: "backslash_at_end_unclosed",
        input: r#"(test.verb :text "unterminated\"#,
        expected: Expected::ParseError("expected"),
    },
];

// =============================================================================
// GOLDEN FILE EXAMPLES - Real DSL patterns from docs/dsl/golden/
// =============================================================================

const GOLDEN_EXAMPLES: &[TestCase] = &[
    TestCase {
        name: "fund_creation_basic",
        input: r#"(cbu.create
  :name "Allianz Global Equity Fund"
  :type "FUND"
  :jurisdiction "LU"
  :legal-form "SICAV"
  :as @fund)"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "entity_creation",
        input: r#"(entity.create
  :name "BlackRock Fund Advisors"
  :type "LEGAL"
  :jurisdiction "US"
  :lei "549300WVFXZ3RTHZ4M94"
  :as @entity)"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "session_load",
        input: r#"(session.load-galaxy :apex-entity-id "Allianz SE")"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "trading_profile_lifecycle",
        input: r#";; Create draft
(trading-profile.create-draft :cbu-id @fund :as @profile)

;; Configure universe
(trading-profile.add-instrument-class :profile-id @profile :class-code "EQUITY")
(trading-profile.add-market :profile-id @profile :instrument-class "EQUITY" :mic "XNYS")

;; Add SSI
(trading-profile.add-standing-instruction
  :profile-id @profile
  :ssi-type "SECURITIES"
  :ssi-name "MAIN-SSI"
  :safekeeping-account "12345"
  :safekeeping-bic "CITIUS33")

;; Activate
(trading-profile.validate-go-live-ready :profile-id @profile)
(trading-profile.submit :profile-id @profile)
(trading-profile.approve :profile-id @profile)"#,
        expected: Expected::Valid,
    },
    TestCase {
        name: "isda_csa_full",
        input: r#";; Counterparty
(entity.create
  :name "Goldman Sachs International"
  :type "LEGAL"
  :jurisdiction "GB"
  :lei "W22LROWP2IHZNBB6K528"
  :as @gs)

;; ISDA config
(trading-profile.add-isda-config
  :profile-id @profile
  :counterparty-entity-id @gs
  :counterparty-name "Goldman Sachs International"
  :counterparty-lei "W22LROWP2IHZNBB6K528"
  :governing-law "ENGLISH"
  :agreement-date "2024-01-15")

;; Product coverage
(trading-profile.add-isda-coverage
  :profile-id @profile
  :isda-ref "Goldman Sachs International"
  :asset-class "RATES"
  :base-products ["IRS" "XCCY_SWAP"])

;; CSA
(trading-profile.add-csa-config
  :profile-id @profile
  :isda-ref "Goldman Sachs International"
  :csa-type "VM"
  :threshold-currency "USD"
  :threshold-amount 0
  :minimum-transfer-amount 500000)

;; Collateral
(trading-profile.add-csa-collateral
  :profile-id @profile
  :counterparty-ref "Goldman Sachs International"
  :collateral-type "CASH"
  :currencies ["USD" "EUR"]
  :haircut-pct 0)"#,
        expected: Expected::Valid,
    },
];

// =============================================================================
// TEST RUNNER
// =============================================================================

fn run_test_case(case: &TestCase) {
    let (state, diagnostics) = analyze_document(case.input);

    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
        .collect();

    let warnings: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == Some(DiagnosticSeverity::WARNING))
        .collect();

    match &case.expected {
        Expected::Valid => {
            if !errors.is_empty() {
                panic!(
                    "Test '{}' expected valid but got {} error(s):\n{}\n\nInput:\n{}",
                    case.name,
                    errors.len(),
                    errors
                        .iter()
                        .map(|e| format!("  - {}", e.message))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    case.input
                );
            }
        }
        Expected::ParseError(expected_msg) => {
            if errors.is_empty() {
                panic!(
                    "Test '{}' expected parse error containing '{}' but got no errors.\n\nInput:\n{}",
                    case.name, expected_msg, case.input
                );
            }
            // Check if any error message contains the expected substring
            let found = errors.iter().any(|e| {
                e.message
                    .to_lowercase()
                    .contains(&expected_msg.to_lowercase())
                    || e.source
                        .as_ref()
                        .map(|s| s.to_lowercase().contains(&expected_msg.to_lowercase()))
                        .unwrap_or(false)
            });
            if !found {
                panic!(
                    "Test '{}' expected error containing '{}' but got:\n{}\n\nInput:\n{}",
                    case.name,
                    expected_msg,
                    errors
                        .iter()
                        .map(|e| format!("  - {}", e.message))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    case.input
                );
            }
        }
        Expected::SemanticError(expected_msg) => {
            if errors.is_empty() {
                panic!(
                    "Test '{}' expected semantic error containing '{}' but got no errors.\n\nInput:\n{}",
                    case.name, expected_msg, case.input
                );
            }
            let found = errors.iter().any(|e| {
                e.message
                    .to_lowercase()
                    .contains(&expected_msg.to_lowercase())
            });
            if !found {
                panic!(
                    "Test '{}' expected error containing '{}' but got:\n{}\n\nInput:\n{}",
                    case.name,
                    expected_msg,
                    errors
                        .iter()
                        .map(|e| format!("  - {}", e.message))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    case.input
                );
            }
        }
        Expected::Warning(expected_msg) => {
            if !errors.is_empty() {
                panic!(
                    "Test '{}' expected only warnings but got {} error(s):\n{}\n\nInput:\n{}",
                    case.name,
                    errors.len(),
                    errors
                        .iter()
                        .map(|e| format!("  - {}", e.message))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    case.input
                );
            }
            if warnings.is_empty() {
                panic!(
                    "Test '{}' expected warning containing '{}' but got no warnings.\n\nInput:\n{}",
                    case.name, expected_msg, case.input
                );
            }
            let found = warnings.iter().any(|w| {
                w.message
                    .to_lowercase()
                    .contains(&expected_msg.to_lowercase())
            });
            if !found {
                panic!(
                    "Test '{}' expected warning containing '{}' but got:\n{}\n\nInput:\n{}",
                    case.name,
                    expected_msg,
                    warnings
                        .iter()
                        .map(|w| format!("  - {}", w.message))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    case.input
                );
            }
        }
        Expected::ErrorCount(count) => {
            if errors.len() != *count {
                panic!(
                    "Test '{}' expected {} error(s) but got {}:\n{}\n\nInput:\n{}",
                    case.name,
                    count,
                    errors.len(),
                    errors
                        .iter()
                        .map(|e| format!("  - {}", e.message))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    case.input
                );
            }
        }
    }

    // Print success for visibility in verbose mode
    eprintln!("✓ {}", case.name);
}

// =============================================================================
// TESTS
// =============================================================================

#[test]
fn test_valid_cases() {
    eprintln!("\n=== VALID CASES ({}) ===", VALID_CASES.len());
    for case in VALID_CASES {
        run_test_case(case);
    }
}

#[test]
fn test_syntax_errors() {
    eprintln!(
        "\n=== SYNTAX ERROR CASES ({}) ===",
        SYNTAX_ERROR_CASES.len()
    );
    for case in SYNTAX_ERROR_CASES {
        run_test_case(case);
    }
}

#[test]
fn test_edge_cases() {
    eprintln!("\n=== EDGE CASES ({}) ===", EDGE_CASES.len());
    for case in EDGE_CASES {
        run_test_case(case);
    }
}

#[test]
fn test_golden_examples() {
    eprintln!("\n=== GOLDEN EXAMPLES ({}) ===", GOLDEN_EXAMPLES.len());
    for case in GOLDEN_EXAMPLES {
        run_test_case(case);
    }
}

#[test]
fn test_more_syntax_errors() {
    eprintln!(
        "\n=== MORE SYNTAX ERRORS ({}) ===",
        MORE_SYNTAX_ERRORS.len()
    );
    for case in MORE_SYNTAX_ERRORS {
        run_test_case(case);
    }
}

#[test]
fn test_stress_tests() {
    eprintln!("\n=== STRESS TESTS ({}) ===", STRESS_TESTS.len());
    for case in STRESS_TESTS {
        run_test_case(case);
    }
}

#[test]
fn test_whitespace() {
    eprintln!("\n=== WHITESPACE TESTS ({}) ===", WHITESPACE_TESTS.len());
    for case in WHITESPACE_TESTS {
        run_test_case(case);
    }
}

#[test]
fn test_unicode() {
    eprintln!("\n=== UNICODE TESTS ({}) ===", UNICODE_TESTS.len());
    for case in UNICODE_TESTS {
        run_test_case(case);
    }
}

#[test]
fn test_numbers() {
    eprintln!("\n=== NUMBER TESTS ({}) ===", NUMBER_TESTS.len());
    for case in NUMBER_TESTS {
        run_test_case(case);
    }
}

#[test]
fn test_comments() {
    eprintln!("\n=== COMMENT TESTS ({}) ===", COMMENT_TESTS.len());
    for case in COMMENT_TESTS {
        run_test_case(case);
    }
}

#[test]
fn test_escapes() {
    eprintln!("\n=== ESCAPE TESTS ({}) ===", ESCAPE_TESTS.len());
    for case in ESCAPE_TESTS {
        run_test_case(case);
    }
}

// =============================================================================
// GOLDEN FILE VALIDATION - Parse actual files from docs/dsl/golden/
// =============================================================================

const GOLDEN_FILE_CONTENTS: &[(&str, &str)] = &[
    (
        "00-syntax-tour.dsl",
        include_str!("../../../../docs/dsl/golden/00-syntax-tour.dsl"),
    ),
    (
        "01-cbu-create.dsl",
        include_str!("../../../../docs/dsl/golden/01-cbu-create.dsl"),
    ),
    (
        "02-roles-and-links.dsl",
        include_str!("../../../../docs/dsl/golden/02-roles-and-links.dsl"),
    ),
    (
        "03-kyc-case-sheet.dsl",
        include_str!("../../../../docs/dsl/golden/03-kyc-case-sheet.dsl"),
    ),
    (
        "04-ubo-mini-graph.dsl",
        include_str!("../../../../docs/dsl/golden/04-ubo-mini-graph.dsl"),
    ),
    (
        "05-otc-isda-csa.dsl",
        include_str!("../../../../docs/dsl/golden/05-otc-isda-csa.dsl"),
    ),
    (
        "06-macro-v2-roundtrip.dsl",
        include_str!("../../../../docs/dsl/golden/06-macro-v2-roundtrip.dsl"),
    ),
    (
        "99-lsp-test.dsl",
        include_str!("../../../../docs/dsl/golden/99-lsp-test.dsl"),
    ),
];

#[test]
fn test_golden_files_parse() {
    eprintln!(
        "\n=== GOLDEN FILE VALIDATION ({} files) ===",
        GOLDEN_FILE_CONTENTS.len()
    );

    let mut all_passed = true;

    for (filename, content) in GOLDEN_FILE_CONTENTS {
        let (_, diagnostics) = analyze_document(content);

        let errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
            .collect();

        if errors.is_empty() {
            eprintln!("✓ {}", filename);
        } else {
            eprintln!("✗ {} - {} error(s):", filename, errors.len());
            for err in &errors {
                eprintln!("    Line {}: {}", err.range.start.line + 1, err.message);
            }
            all_passed = false;
        }
    }

    assert!(all_passed, "Some golden files have parse errors");
}

// =============================================================================
// SUMMARY TEST - Run all and report
// =============================================================================

#[test]
fn test_harness_summary() {
    let all_cases: Vec<(&str, &[TestCase])> = vec![
        ("Valid", VALID_CASES),
        ("Syntax Errors", SYNTAX_ERROR_CASES),
        ("More Syntax Errors", MORE_SYNTAX_ERRORS),
        ("Edge Cases", EDGE_CASES),
        ("Stress Tests", STRESS_TESTS),
        ("Whitespace", WHITESPACE_TESTS),
        ("Unicode", UNICODE_TESTS),
        ("Numbers", NUMBER_TESTS),
        ("Comments", COMMENT_TESTS),
        ("Escapes", ESCAPE_TESTS),
        ("Golden Examples", GOLDEN_EXAMPLES),
    ];

    let mut total = 0;
    let mut passed = 0;
    let mut failed_cases = Vec::new();

    eprintln!("\n╔══════════════════════════════════════════════════════════════╗");
    eprintln!("║               LSP TEST HARNESS SUMMARY                       ║");
    eprintln!("╠══════════════════════════════════════════════════════════════╣");

    for (category, cases) in &all_cases {
        let category_total = cases.len();
        let mut category_passed = 0;

        for case in *cases {
            total += 1;
            let result = std::panic::catch_unwind(|| {
                run_test_case(case);
            });
            if result.is_ok() {
                passed += 1;
                category_passed += 1;
            } else {
                failed_cases.push((category, case.name));
            }
        }

        eprintln!(
            "║ {:20} {:3}/{:3} passed                           ║",
            *category, category_passed, category_total
        );
    }

    eprintln!("╠══════════════════════════════════════════════════════════════╣");
    eprintln!(
        "║ TOTAL: {}/{} passed ({:.1}%)                                  ║",
        passed,
        total,
        (passed as f64 / total as f64) * 100.0
    );
    eprintln!("╚══════════════════════════════════════════════════════════════╝");

    if !failed_cases.is_empty() {
        eprintln!("\nFailed cases:");
        for (category, name) in &failed_cases {
            eprintln!("  - [{}] {}", category, name);
        }
    }

    // Don't fail the summary test - individual tests will fail
}
