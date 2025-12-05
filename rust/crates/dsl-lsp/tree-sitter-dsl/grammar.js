/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

module.exports = grammar({
  name: "dsl",

  extras: ($) => [/\s/, $.comment],

  rules: {
    source_file: ($) => repeat($.list),

    _expression: ($) =>
      choice($.list, $.keyword, $.string, $.number, $.symbol_ref),

    // S-expression: (verb-name :arg value ...)
    list: ($) =>
      seq(
        "(",
        optional($.verb_name), // verb name
        repeat($._expression),
        ")",
      ),

    // Verb name (only appears as first element of list)
    verb_name: ($) => /[a-zA-Z_][a-zA-Z0-9_\-\.]*/,

    // Identifier/verb name
    symbol: ($) => /[a-zA-Z_][a-zA-Z0-9_\-\.]*/,

    // Keyword argument: :name
    keyword: ($) => seq(":", /[a-zA-Z_][a-zA-Z0-9_\-]*/),

    // String literal
    string: ($) => seq('"', repeat(choice(/[^"\\]+/, /\\./)), '"'),

    // Number (integer or decimal)
    number: ($) => /\-?[0-9]+(\.[0-9]+)?/,

    // Symbol reference: @name
    symbol_ref: ($) => seq("@", /[a-zA-Z_][a-zA-Z0-9_\-]*/),

    // Comment: ; to end of line
    comment: ($) => /;[^\n]*/,
  },
});
