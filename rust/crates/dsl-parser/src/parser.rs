//! Hand-written recursive-descent parser for the unified DSL v0.1.
//!
//! Produces a `SourceFile` (sequence of top-level `RawAtom`s) from a token
//! stream produced by `Token::lexer`. The parser is tolerant: on error it
//! emits a `Diagnostic` and skips tokens to recover at the next top-level
//! atom boundary (an `OpenParen` not nested inside another atom).
//!
//! Grammar (informal):
//!
//! ```text
//! source-file  ::= atom*
//! atom         ::= '(' symbol name? slot* ')'
//! name         ::= symbol          ; only if it does NOT start with ':' (keywords are separate tokens)
//! slot         ::= keyword value
//! value        ::= atom | list | map | qualified-name | symbol | string | int | float | bool
//!                | slot-ref | template-subst | template-splice | insertion-marker
//! list         ::= '[' value* ']'
//! map          ::= '{' (keyword value)* '}'
//! qualified-name ::= symbol '/' symbol    ; two symbols separated by '/'
//! flow-arrow   ::= '->'                   ; sugar: treated as a positional slot value separator
//! ```
//!
//! Arrow (`->`) sugar for flows: in a flow atom `(flow src -> tgt)` the `->` is
//! silently consumed; `src` and `tgt` are parsed as positional values for the
//! `source` and `target` slot names respectively (no explicit `:source`/`:target`
//! keywords needed on the wire, but the parser synthesises them).

use logos::Logos;

use dsl_diagnostics::{Diagnostic, DiagnosticBag};

use crate::lexer::Token;
use crate::raw_ast::{RawAtom, RawValue, SourceFile};

// ---------------------------------------------------------------------------
// TokenStream — thin wrapper around the logos iterator with one-token lookahead
// ---------------------------------------------------------------------------

struct TokenStream<'src> {
    inner: logos::Lexer<'src, Token>,
    peeked: Option<Option<Result<Token, ()>>>,
}

impl<'src> TokenStream<'src> {
    fn new(src: &'src str) -> Self {
        Self {
            inner: Token::lexer(src),
            peeked: None,
        }
    }

    /// Advance and return the next token (or `None` at EOF, `Some(Err(()))` on
    /// lex error).
    fn next(&mut self) -> Option<Result<Token, ()>> {
        if let Some(tok) = self.peeked.take() {
            tok
        } else {
            self.inner.next()
        }
    }

    /// Peek at the next token without consuming it.
    fn peek(&mut self) -> Option<&Result<Token, ()>> {
        if self.peeked.is_none() {
            self.peeked = Some(self.inner.next());
        }
        self.peeked.as_ref().and_then(|x| x.as_ref())
    }

    /// Consume the next token and return it only if it satisfies the predicate.
    /// If the predicate fails, the token is pushed back (via peeked).
    fn expect_ok<F, T>(&mut self, f: F) -> Option<T>
    where
        F: FnOnce(&Token) -> Option<T>,
    {
        match self.peek() {
            Some(Ok(tok)) => {
                let result = f(tok);
                if result.is_some() {
                    self.next(); // consume
                }
                result
            }
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct Parser<'src> {
    stream: TokenStream<'src>,
    diagnostics: DiagnosticBag,
}

impl<'src> Parser<'src> {
    fn new(src: &'src str) -> Self {
        Self {
            stream: TokenStream::new(src),
            diagnostics: DiagnosticBag::new(),
        }
    }

    fn parse_source_file(mut self) -> (SourceFile, DiagnosticBag) {
        let mut atoms = Vec::new();
        loop {
            match self.stream.peek() {
                None => break,
                Some(Err(())) => {
                    // Lex error — skip and continue
                    self.stream.next();
                    self.diagnostics.push(Diagnostic::error("Unexpected character"));
                }
                Some(Ok(Token::OpenParen)) => {
                    match self.parse_atom() {
                        Some(atom) => atoms.push(atom),
                        None => {
                            // Error already emitted; skip to next OpenParen
                            self.recover_to_next_top_level();
                        }
                    }
                }
                Some(Ok(_)) => {
                    // Unexpected token at top level — skip it
                    self.stream.next();
                    self.diagnostics
                        .push(Diagnostic::error("Expected '(' at top level"));
                }
            }
        }
        (SourceFile { atoms }, self.diagnostics)
    }

    /// Skip tokens until we either reach a top-level `(` or EOF.
    fn recover_to_next_top_level(&mut self) {
        loop {
            match self.stream.peek() {
                None => break,
                Some(Ok(Token::OpenParen)) => break,
                _ => {
                    self.stream.next();
                }
            }
        }
    }

    /// Parse `(kind [name] slot* ')`.
    fn parse_atom(&mut self) -> Option<RawAtom> {
        // Consume the '('
        self.stream.next();

        // First token must be a Symbol → the atom kind
        let kind = match self.stream.next() {
            Some(Ok(Token::Symbol(s))) => s,
            other => {
                self.diagnostics.push(Diagnostic::error(format!(
                    "Expected atom kind symbol after '(', got {:?}",
                    other
                )));
                return None;
            }
        };

        // Optional name: next token is a Symbol that is NOT a Keyword
        let name = self.stream.expect_ok(|tok| {
            if let Token::Symbol(s) = tok {
                Some(s.clone())
            } else {
                None
            }
        });

        // For the "flow" kind, handle the `->` sugar specially.
        // `(flow src -> tgt :slot val)` is parsed as:
        //   kind="flow", slots=[("source", Symbol(src)), ("target", Symbol(tgt)), ...]
        let is_flow = kind == "flow";

        let mut slots: Vec<(String, RawValue)> = Vec::new();

        // Track whether we've seen the first positional value for flow sugar
        let mut flow_state = if is_flow && name.is_none() {
            // No name was consumed, so first slot is a positional source
            FlowParseState::ExpectingSource
        } else if is_flow {
            // Name was consumed as source
            slots.push(("source".to_owned(), RawValue::Symbol(name.as_ref().unwrap().clone())));
            FlowParseState::ExpectingArrowOrTarget
        } else {
            FlowParseState::NotFlow
        };

        loop {
            match self.stream.peek() {
                None => {
                    self.diagnostics
                        .push(Diagnostic::error("Unexpected EOF inside atom"));
                    return None;
                }
                Some(Ok(Token::CloseParen)) => {
                    self.stream.next();
                    break;
                }
                Some(Ok(Token::Arrow)) => {
                    self.stream.next();
                    match flow_state {
                        FlowParseState::ExpectingArrowOrTarget => {
                            flow_state = FlowParseState::ExpectingTarget;
                        }
                        _ => {
                            self.diagnostics.push(Diagnostic::error(
                                "Unexpected '->' outside flow source→target position",
                            ));
                        }
                    }
                }
                Some(Ok(Token::Keyword(_))) => {
                    // Normal `:slot value` pair
                    let slot_name = match self.stream.next() {
                        Some(Ok(Token::Keyword(k))) => k,
                        _ => unreachable!(),
                    };
                    match self.parse_value() {
                        Some(val) => slots.push((slot_name, val)),
                        None => {
                            self.diagnostics.push(Diagnostic::error(format!(
                                "Expected value for slot ':{}'",
                                slot_name
                            )));
                        }
                    }
                }
                _ => {
                    // Positional value — only valid in flow sugar positions
                    match self.parse_value() {
                        Some(val) => {
                            match flow_state {
                                FlowParseState::ExpectingSource => {
                                    slots.push(("source".to_owned(), val));
                                    flow_state = FlowParseState::ExpectingArrowOrTarget;
                                }
                                FlowParseState::ExpectingTarget => {
                                    slots.push(("target".to_owned(), val));
                                    flow_state = FlowParseState::Done;
                                }
                                FlowParseState::Done | FlowParseState::NotFlow => {
                                    // Positional value outside flow context: treat as
                                    // a bare value slot with an empty key (parse error)
                                    self.diagnostics.push(Diagnostic::error(
                                        "Positional value in non-flow atom context; expected ':slot'",
                                    ));
                                    slots.push(("".to_owned(), val));
                                }
                                FlowParseState::ExpectingArrowOrTarget => {
                                    // Source was set, no arrow seen — treat next value as target
                                    slots.push(("target".to_owned(), val));
                                    flow_state = FlowParseState::Done;
                                }
                            }
                        }
                        None => {
                            // parse_value already emitted a diagnostic; skip
                        }
                    }
                }
            }
        }

        // For pure flow atoms (no name consumed before flow state machine started),
        // ensure we return the right name
        let final_name = if is_flow {
            None // flow atoms don't have a standalone name; source/target are slots
        } else {
            name
        };

        Some(RawAtom {
            kind,
            name: final_name,
            slots,
            span: None,
        })
    }

    /// Parse a single value (slot RHS or list element).
    fn parse_value(&mut self) -> Option<RawValue> {
        match self.stream.peek() {
            None => {
                self.diagnostics
                    .push(Diagnostic::error("Expected value but got EOF"));
                None
            }
            Some(Err(())) => {
                self.stream.next();
                self.diagnostics
                    .push(Diagnostic::error("Lex error in value position"));
                None
            }
            Some(Ok(tok)) => match tok {
                Token::OpenParen => {
                    // Nested atom
                    self.parse_atom().map(RawValue::Atom)
                }
                Token::OpenBracket => {
                    self.stream.next();
                    self.parse_list()
                }
                Token::OpenBrace => {
                    self.stream.next();
                    self.parse_map()
                }
                Token::StringLit(_) => {
                    if let Some(Ok(Token::StringLit(s))) = self.stream.next() {
                        Some(RawValue::StringLit(s))
                    } else {
                        unreachable!()
                    }
                }
                Token::IntLit(_) => {
                    if let Some(Ok(Token::IntLit(i))) = self.stream.next() {
                        Some(RawValue::IntLit(i))
                    } else {
                        unreachable!()
                    }
                }
                Token::FloatLit(_) => {
                    if let Some(Ok(Token::FloatLit(f))) = self.stream.next() {
                        Some(RawValue::FloatLit(f))
                    } else {
                        unreachable!()
                    }
                }
                Token::BoolLit(_) => {
                    if let Some(Ok(Token::BoolLit(b))) = self.stream.next() {
                        Some(RawValue::BoolLit(b))
                    } else {
                        unreachable!()
                    }
                }
                Token::TemplateSubst(_) => {
                    if let Some(Ok(Token::TemplateSubst(s))) = self.stream.next() {
                        Some(RawValue::TemplateSubst(s))
                    } else {
                        unreachable!()
                    }
                }
                Token::TemplateSplice(_) => {
                    if let Some(Ok(Token::TemplateSplice(s))) = self.stream.next() {
                        Some(RawValue::TemplateSplice(s))
                    } else {
                        unreachable!()
                    }
                }
                Token::InsertionMarker(_) => {
                    if let Some(Ok(Token::InsertionMarker(s))) = self.stream.next() {
                        Some(RawValue::InsertionMarker(s))
                    } else {
                        unreachable!()
                    }
                }
                Token::Symbol(_) => {
                    // Could be a plain symbol or a qualified name `pack/atom`
                    if let Some(Ok(Token::Symbol(s))) = self.stream.next() {
                        // Check for `/` in the symbol (qualified name formed by
                        // the surface syntax `pack-name/atom-name` — logos will
                        // tokenize this as a single symbol since `/` is part of
                        // the capture group if embedded, but actually logos won't
                        // match `/` in our Symbol regex. So qualified names
                        // `pack/atom` arrive as two Symbols with a `/` Error token
                        // between them). We handle the split-token form here by
                        // peeking for an error token that is `/` followed by another Symbol.
                        // In practice, since `/` is not in the Symbol regex, the lexer
                        // will emit an error token for `/`. We peek to handle that.
                        //
                        // Simpler approach: peek for Error token that came from `/`
                        // by checking the raw source. Since we don't have position
                        // info in the token itself, we use a different strategy:
                        // peek the next two tokens. If next is Error and after that
                        // is Symbol, we treat it as a qualified name.
                        //
                        // Note: `logos::skip` is not used for `/` so it produces Err(()).
                        // We handle it specially here.
                        if matches!(self.stream.peek(), Some(Err(()))) {
                            // Speculatively: peek two ahead to see if it's Symbol
                            // This requires consuming the error token and peeking
                            // at what follows. We consume the Err here and check.
                            self.stream.next(); // consume the Err (which is `/`)
                            if let Some(value) = self.stream.expect_ok(|tok| {
                                if let Token::Symbol(atom) = tok {
                                    Some(atom.clone())
                                } else {
                                    None
                                }
                            }) {
                                Some(RawValue::QualifiedName {
                                    pack: s,
                                    atom: value,
                                })
                            } else {
                                // Was an error but not a qualified name — just return
                                // the original symbol (the error and possible token
                                // are already consumed)
                                Some(RawValue::Symbol(s))
                            }
                        } else {
                            Some(RawValue::Symbol(s))
                        }
                    } else {
                        unreachable!()
                    }
                }
                // These tokens cannot start a value
                Token::CloseParen
                | Token::CloseBracket
                | Token::CloseBrace
                | Token::Keyword(_)
                | Token::Arrow
                | Token::Comment
                | Token::Whitespace => {
                    // Don't consume — let the caller handle the unexpected token
                    None
                }
            },
        }
    }

    /// Parse `value* ']'`
    fn parse_list(&mut self) -> Option<RawValue> {
        let mut items = Vec::new();
        loop {
            match self.stream.peek() {
                None => {
                    self.diagnostics
                        .push(Diagnostic::error("Unexpected EOF inside list"));
                    return None;
                }
                Some(Ok(Token::CloseBracket)) => {
                    self.stream.next();
                    break;
                }
                _ => {
                    match self.parse_value() {
                        Some(v) => items.push(v),
                        None => {
                            // Skip to close bracket
                            loop {
                                match self.stream.peek() {
                                    None | Some(Ok(Token::CloseBracket)) => break,
                                    _ => {
                                        self.stream.next();
                                    }
                                }
                            }
                            break;
                        }
                    }
                }
            }
        }
        Some(RawValue::List(items))
    }

    /// Parse `(keyword value)* '}'`
    fn parse_map(&mut self) -> Option<RawValue> {
        let mut pairs = Vec::new();
        loop {
            match self.stream.peek() {
                None => {
                    self.diagnostics
                        .push(Diagnostic::error("Unexpected EOF inside map"));
                    return None;
                }
                Some(Ok(Token::CloseBrace)) => {
                    self.stream.next();
                    break;
                }
                Some(Ok(Token::Keyword(_))) => {
                    let key = match self.stream.next() {
                        Some(Ok(Token::Keyword(k))) => k,
                        _ => unreachable!(),
                    };
                    match self.parse_value() {
                        Some(val) => pairs.push((key, val)),
                        None => {
                            self.diagnostics.push(Diagnostic::error(format!(
                                "Expected value for map key ':{}' ",
                                key
                            )));
                            break;
                        }
                    }
                }
                _ => {
                    self.stream.next();
                    self.diagnostics
                        .push(Diagnostic::error("Expected ':key' in map"));
                }
            }
        }
        Some(RawValue::Map(pairs))
    }
}

/// State machine for parsing flow atoms with `->` sugar.
#[derive(Debug, Clone, PartialEq)]
enum FlowParseState {
    NotFlow,
    ExpectingSource,
    ExpectingArrowOrTarget,
    ExpectingTarget,
    Done,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parse a DSL source string into a `SourceFile`.
///
/// The parser is tolerant: parse errors produce diagnostics but do not abort
/// the parse. The returned `DiagnosticBag` should be inspected after calling
/// this function.
pub fn parse(src: &str) -> (SourceFile, DiagnosticBag) {
    Parser::new(src).parse_source_file()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(src: &str) -> SourceFile {
        let (sf, diag) = parse(src);
        if diag.has_errors() {
            let msgs: Vec<_> = diag.errors().map(|d| d.message.as_str()).collect();
            panic!("Parse errors: {:?}", msgs);
        }
        sf
    }

    #[test]
    fn parse_gateway_atom() {
        let sf = parse_ok("(gateway activation-gate :kind exclusive)");
        assert_eq!(sf.atoms.len(), 1);
        let atom = &sf.atoms[0];
        assert_eq!(atom.kind, "gateway");
        assert_eq!(atom.name, Some("activation-gate".to_owned()));
        assert_eq!(atom.slots.len(), 1);
        assert_eq!(atom.slots[0].0, "kind");
        assert_eq!(atom.slots[0].1, RawValue::Symbol("exclusive".to_owned()));
    }

    #[test]
    fn parse_flow_with_arrow_and_template_forms() {
        let sf = parse_ok("(flow $pre-node -> ,gate-name)");
        assert_eq!(sf.atoms.len(), 1);
        let atom = &sf.atoms[0];
        assert_eq!(atom.kind, "flow");
        assert_eq!(atom.name, None);
        // Slots: source=$pre-node, target=,gate-name
        let source = atom.slots.iter().find(|(k, _)| k == "source").unwrap();
        let target = atom.slots.iter().find(|(k, _)| k == "target").unwrap();
        assert_eq!(source.1, RawValue::InsertionMarker("pre-node".to_owned()));
        assert_eq!(target.1, RawValue::TemplateSubst("gate-name".to_owned()));
    }

    #[test]
    fn parse_list_value() {
        let sf = parse_ok("(node foo :items [a b c])");
        let atom = &sf.atoms[0];
        let items_slot = atom.slots.iter().find(|(k, _)| k == "items").unwrap();
        match &items_slot.1 {
            RawValue::List(items) => {
                assert_eq!(items.len(), 3);
                assert_eq!(items[0], RawValue::Symbol("a".to_owned()));
                assert_eq!(items[1], RawValue::Symbol("b".to_owned()));
                assert_eq!(items[2], RawValue::Symbol("c".to_owned()));
            }
            other => panic!("Expected List, got {:?}", other),
        }
    }

    #[test]
    fn parse_map_value() {
        let sf = parse_ok(r#"(entity bar :meta {:key "value"})"#);
        let atom = &sf.atoms[0];
        let meta_slot = atom.slots.iter().find(|(k, _)| k == "meta").unwrap();
        match &meta_slot.1 {
            RawValue::Map(pairs) => {
                assert_eq!(pairs.len(), 1);
                assert_eq!(pairs[0].0, "key");
                assert_eq!(pairs[0].1, RawValue::StringLit("value".to_owned()));
            }
            other => panic!("Expected Map, got {:?}", other),
        }
    }

    #[test]
    fn parse_multiple_atoms() {
        let src = r#"
            (node start :label "Start")
            (node end :label "End")
            (flow start -> end)
        "#;
        let sf = parse_ok(src);
        assert_eq!(sf.atoms.len(), 3);
        assert_eq!(sf.atoms[0].kind, "node");
        assert_eq!(sf.atoms[1].kind, "node");
        assert_eq!(sf.atoms[2].kind, "flow");
    }

    #[test]
    fn parse_tolerates_errors_and_continues() {
        // Junk before a valid atom — should recover
        let (sf, diag) = parse("not-an-atom (gateway g :kind exclusive)");
        // We should have gotten the gateway atom
        assert_eq!(sf.atoms.len(), 1);
        assert_eq!(sf.atoms[0].kind, "gateway");
        assert!(diag.has_errors(), "should have error for 'not-an-atom'");
    }

    #[test]
    fn parse_nested_atom_as_value() {
        let src = "(node outer :child (node inner :x 1))";
        let sf = parse_ok(src);
        let child_slot = sf.atoms[0].slots.iter().find(|(k, _)| k == "child").unwrap();
        match &child_slot.1 {
            RawValue::Atom(inner) => {
                assert_eq!(inner.kind, "node");
                assert_eq!(inner.name, Some("inner".to_owned()));
            }
            other => panic!("Expected nested Atom, got {:?}", other),
        }
    }

    #[test]
    fn parse_bool_and_int_literals() {
        let sf = parse_ok("(node n :active true :count 42)");
        let active = sf.atoms[0].slots.iter().find(|(k, _)| k == "active").unwrap();
        let count = sf.atoms[0].slots.iter().find(|(k, _)| k == "count").unwrap();
        assert_eq!(active.1, RawValue::BoolLit(true));
        assert_eq!(count.1, RawValue::IntLit(42));
    }
}
