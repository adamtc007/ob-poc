/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

module.exports = grammar({
  name: 'dsl',

  extras: $ => [
    /\s/,
    $.comment,
  ],

  rules: {
    source_file: $ => repeat($._expression),

    _expression: $ => choice(
      $.list,
      $.symbol,
      $.keyword,
      $.string,
      $.number,
      $.symbol_ref,
    ),

    // S-expression: (verb-name :arg value ...)
    list: $ => seq(
      '(',
      optional($.symbol),  // verb name
      repeat($._expression),
      ')'
    ),

    // Identifier/verb name
    symbol: $ => /[a-zA-Z_][a-zA-Z0-9_\-\.]*/,

    // Keyword argument: :name
    keyword: $ => seq(':', /[a-zA-Z_][a-zA-Z0-9_\-]*/),

    // String literal
    string: $ => seq(
      '"',
      repeat(choice(
        /[^"\\]+/,
        /\\./
      )),
      '"'
    ),

    // Number (integer or decimal)
    number: $ => /\-?[0-9]+(\.[0-9]+)?/,

    // Symbol reference: @name
    symbol_ref: $ => seq('@', /[a-zA-Z_][a-zA-Z0-9_\-]*/),

    // Comment: ; to end of line
    comment: $ => /;[^\n]*/,
  }
});
