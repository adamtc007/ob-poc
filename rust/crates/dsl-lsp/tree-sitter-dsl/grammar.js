/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

/**
 * Tree-sitter grammar for the OB-POC DSL
 *
 * This grammar is aligned with:
 * - EBNF specification: docs/dsl-grammar.ebnf
 * - NOM parser: rust/crates/dsl-core/src/parser.rs
 *
 * Key syntax elements:
 * - S-expressions: (domain.verb :key value ...)
 * - Comments: ;; (double semicolon, Lisp-style)
 * - Booleans: true, false
 * - Null: nil
 * - Lists: [item1 item2] or [item1, item2]
 * - Maps: {:key value :key2 value2}
 * - Symbol refs: @name
 * - Strings: "..." with escape sequences
 * - Numbers: 42, -17, 3.14
 */

module.exports = grammar({
  name: "dsl",

  extras: ($) => [/\s/, $.comment],

  rules: {
    source_file: ($) => repeat($._statement),

    _statement: ($) => choice($.list, $.comment),

    _expression: ($) =>
      choice(
        $.list,
        $.binding, // :as @symbol binding (must be before keyword)
        $.keyword,
        $.string,
        $.number,
        $.boolean,
        $.null_literal,
        $.symbol_ref,
        $.array,
        $.map,
      ),

    // S-expression: (verb-name :arg value ...)
    list: ($) => seq("(", optional($.verb_name), repeat($._expression), ")"),

    // Binding: :as @symbol-name (special syntax for symbol definitions)
    // This is distinct from regular keyword arguments
    binding: ($) => seq(":as", $.symbol_ref),

    // Verb name: domain.verb (only appears as first element of list)
    // Supports kebab-case: cbu.create, entity.create-limited-company
    verb_name: ($) => /[a-zA-Z_][a-zA-Z0-9_\-]*\.[a-zA-Z_][a-zA-Z0-9_\-]*/,

    // Keyword argument: :name, :cbu-id, :first-name
    keyword: ($) => seq(":", /[a-zA-Z_][a-zA-Z0-9_\-]*/),

    // String literal with escape sequences: "hello \"world\""
    string: ($) => seq('"', repeat(choice(/[^"\\]+/, /\\./)), '"'),

    // Number (integer or decimal, with optional negative): 42, -17, 3.14
    number: ($) => /\-?[0-9]+(\.[0-9]+)?/,

    // Boolean literals: true, false
    boolean: ($) => choice("true", "false"),

    // Null literal: nil
    null_literal: ($) => "nil",

    // Symbol reference: @name, @my-cbu, @fund_a
    symbol_ref: ($) => seq("@", /[a-zA-Z_][a-zA-Z0-9_\-]*/),

    // Array/List literal: [expr expr] or [expr, expr]
    // Supports both space-separated and comma-separated items
    array: ($) =>
      seq(
        "[",
        optional(seq($._expression, repeat(seq(optional(","), $._expression)))),
        "]",
      ),

    // Map literal: {:key value :key2 value2}
    map: ($) => seq("{", repeat(seq($.keyword, $._expression)), "}"),

    // Comment: ;; to end of line (double semicolon, Lisp-style)
    // IMPORTANT: DSL uses ;; NOT single ;
    comment: ($) => /;;[^\n]*/,
  },
});
