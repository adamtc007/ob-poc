//! S-expression lexer for the unified DSL v0.1.
//!
//! Uses the `logos` crate for efficient tokenisation. The lexer is driven
//! by `Token::lexer(src)` which returns an iterator of `Result<Token, ()>`.
//!
//! Key token forms:
//! - `(` `)` `[` `]` `{` `}` — structural delimiters
//! - `:keyword` — keyword slots (the `:` is consumed; `Keyword` carries the bare name)
//! - `"string"` — string literal (quotes stripped, basic escape handling)
//! - `123` / `-42` — integer literals
//! - `3.14` — float literals
//! - `true` / `false` — boolean literals
//! - `,symbol` — template substitution
//! - `,@symbol` — template splice
//! - `$symbol` — insertion marker
//! - `->` — flow arrow sugar
//! - `;...` — line comment (skipped)
//! - whitespace — skipped

use logos::Logos;

/// The full token vocabulary for the unified DSL v0.1 lexer.
#[derive(Logos, Debug, Clone, PartialEq)]
pub enum Token {
    // -----------------------------------------------------------------------
    // Structural delimiters
    // -----------------------------------------------------------------------
    #[token("(")]
    OpenParen,

    #[token(")")]
    CloseParen,

    #[token("[")]
    OpenBracket,

    #[token("]")]
    CloseBracket,

    #[token("{")]
    OpenBrace,

    #[token("}")]
    CloseBrace,

    // -----------------------------------------------------------------------
    // Flow arrow sugar
    // -----------------------------------------------------------------------
    #[token("->")]
    Arrow,

    // -----------------------------------------------------------------------
    // Template / insertion forms  (must come before Symbol to win priority)
    // -----------------------------------------------------------------------

    /// `,@symbol` — splice: expands a list-valued template parameter in-place.
    #[regex(r",@[a-zA-Z_][a-zA-Z0-9_\-]*", |lex| lex.slice()[2..].to_owned())]
    TemplateSplice(String),

    /// `,symbol` or `,var.field` — substitution: inserts a single template
    /// parameter value or accesses a field of a loop variable inside a
    /// `(for-each ...)` body.  The `.` separator distinguishes loop-variable
    /// field access (`,band.upper`) from plain parameter names (`,gate-name`).
    /// Parameter names must not contain dots — dots are reserved for this form.
    #[regex(r",[a-zA-Z_][a-zA-Z0-9_\-\.]*", |lex| lex.slice()[1..].to_owned())]
    TemplateSubst(String),

    /// `$symbol` — insertion marker for pre/post-node positions.
    #[regex(r"\$[a-zA-Z_][a-zA-Z0-9_\-]*", |lex| lex.slice()[1..].to_owned())]
    InsertionMarker(String),

    // -----------------------------------------------------------------------
    // Keywords (`:name` — the colon is part of the token syntax but stripped)
    // -----------------------------------------------------------------------
    #[regex(r":[a-zA-Z_][a-zA-Z0-9_\-]*", |lex| lex.slice()[1..].to_owned())]
    Keyword(String),

    // -----------------------------------------------------------------------
    // Literals
    // -----------------------------------------------------------------------

    /// `true` or `false` boolean literals (must be before Symbol rule).
    #[token("true", |_| true)]
    #[token("false", |_| false)]
    BoolLit(bool),

    /// Floating-point literal. Requires a decimal point.
    #[regex(r"-?[0-9]+\.[0-9]+", |lex| lex.slice().parse::<f64>().ok())]
    FloatLit(f64),

    /// Integer literal (decimal, possibly negative).
    #[regex(r"-?[0-9]+", |lex| lex.slice().parse::<i64>().ok())]
    IntLit(i64),

    /// Double-quoted string literal. The surrounding quotes are stripped.
    /// Escape sequences `\\`, `\"`, `\n`, `\t` are handled by the callback.
    #[regex(r#""(?:[^"\\]|\\.)*""#, lex_string)]
    StringLit(String),

    // -----------------------------------------------------------------------
    // Symbol — unquoted identifier (dashes and dots allowed, must come last)
    // -----------------------------------------------------------------------
    /// Unquoted identifier: starts with a letter or underscore, may contain
    /// letters, digits, dashes, underscores, and dots.
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_\-\.]*", |lex| lex.slice().to_owned())]
    Symbol(String),

    // -----------------------------------------------------------------------
    // Skipped forms
    // -----------------------------------------------------------------------

    /// Line comment starting with `;`.
    #[regex(r";[^\n]*", logos::skip)]
    Comment,

    /// Whitespace (spaces, tabs, newlines).
    #[regex(r"[ \t\r\n]+", logos::skip)]
    Whitespace,
}

/// Callback that strips surrounding quotes from a string token and handles
/// basic escape sequences: `\\` → `\`, `\"` → `"`, `\n` → newline,
/// `\t` → tab.
fn lex_string(lex: &mut logos::Lexer<Token>) -> Option<String> {
    let raw = lex.slice();
    // Strip the surrounding double-quotes
    let inner = &raw[1..raw.len() - 1];
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('\\') => out.push('\\'),
                Some('"') => out.push('"'),
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => break,
            }
        } else {
            out.push(c);
        }
    }
    Some(out)
}

/// Lex the given source string and collect all tokens, discarding position
/// information. Returns `Ok(token)` for recognised tokens and `Err(())` for
/// unrecognised byte sequences.
pub fn lex(src: &str) -> Vec<Result<Token, ()>> {
    Token::lexer(src).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(src: &str) -> Vec<Token> {
        Token::lexer(src)
            .filter_map(|t| t.ok())
            .collect()
    }

    #[test]
    fn lex_delimiters() {
        let toks = tokens("( ) [ ] { }");
        assert_eq!(
            toks,
            vec![
                Token::OpenParen,
                Token::CloseParen,
                Token::OpenBracket,
                Token::CloseBracket,
                Token::OpenBrace,
                Token::CloseBrace,
            ]
        );
    }

    #[test]
    fn lex_keyword() {
        let toks = tokens(":kind");
        assert_eq!(toks, vec![Token::Keyword("kind".to_owned())]);
    }

    #[test]
    fn lex_symbol() {
        let toks = tokens("activation-gate");
        assert_eq!(toks, vec![Token::Symbol("activation-gate".to_owned())]);
    }

    #[test]
    fn lex_string_literal() {
        let toks = tokens(r#""hello world""#);
        assert_eq!(toks, vec![Token::StringLit("hello world".to_owned())]);
    }

    #[test]
    fn lex_string_with_escapes() {
        let toks = tokens(r#""line1\nline2""#);
        assert_eq!(toks, vec![Token::StringLit("line1\nline2".to_owned())]);
    }

    #[test]
    fn lex_integer() {
        let toks = tokens("42 -7");
        assert_eq!(toks, vec![Token::IntLit(42), Token::IntLit(-7)]);
    }

    #[test]
    fn lex_float() {
        let toks = tokens("3.14");
        assert_eq!(toks, vec![Token::FloatLit(3.14)]);
    }

    #[test]
    fn lex_bool() {
        let toks = tokens("true false");
        assert_eq!(toks, vec![Token::BoolLit(true), Token::BoolLit(false)]);
    }

    #[test]
    fn lex_template_subst() {
        let toks = tokens(",gate-name");
        assert_eq!(toks, vec![Token::TemplateSubst("gate-name".to_owned())]);
    }

    #[test]
    fn lex_template_subst_with_dot() {
        // Dot accessor form for for-each loop variables: ,band.upper, ,jp.path
        let toks = tokens(",band.upper");
        assert_eq!(toks, vec![Token::TemplateSubst("band.upper".to_owned())]);

        let toks2 = tokens(",jp.path");
        assert_eq!(toks2, vec![Token::TemplateSubst("jp.path".to_owned())]);

        // Verify that plain params without dots still work
        let toks3 = tokens(",gate-name");
        assert_eq!(toks3, vec![Token::TemplateSubst("gate-name".to_owned())]);
    }

    #[test]
    fn lex_template_splice() {
        let toks = tokens(",@conditions");
        assert_eq!(toks, vec![Token::TemplateSplice("conditions".to_owned())]);
    }

    #[test]
    fn lex_insertion_marker() {
        let toks = tokens("$pre-node");
        assert_eq!(toks, vec![Token::InsertionMarker("pre-node".to_owned())]);
    }

    #[test]
    fn lex_arrow() {
        let toks = tokens("->");
        assert_eq!(toks, vec![Token::Arrow]);
    }

    #[test]
    fn lex_comment_skipped() {
        let toks = tokens("; this is a comment\n(foo)");
        assert_eq!(toks, vec![Token::OpenParen, Token::Symbol("foo".to_owned()), Token::CloseParen]);
    }

    #[test]
    fn lex_full_atom() {
        let toks = tokens("(gateway activation-gate :kind exclusive)");
        assert_eq!(
            toks,
            vec![
                Token::OpenParen,
                Token::Symbol("gateway".to_owned()),
                Token::Symbol("activation-gate".to_owned()),
                Token::Keyword("kind".to_owned()),
                Token::Symbol("exclusive".to_owned()),
                Token::CloseParen,
            ]
        );
    }
}
