//! Adapter to convert dsl_v2 AST to LSP DocumentState.
//!
//! This module bridges the v2 parser with the LSP's document representation,
//! ensuring a single source of truth for parsing.
//!
//! ## Pipeline Alignment
//!
//! Both LSP and Agent use the SAME pipeline:
//! 1. `parse_program()` - Nom parser → Raw AST
//! 2. `enrich_program()` - YAML lookup config → Enriched AST (optional for LSP)
//! 3. `LspValidator` - EntityGateway resolution → Resolved AST
//!
//! This adapter converts the raw AST to LSP's DocumentState for editor features.

use ob_poc::dsl_v2::{
    ast::{Argument, AstNode, Literal, Program, Span as V2Span, Statement, VerbCall},
    parse_program,
};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range};

use super::document::{DocumentState, ExprKind, ParsedArg, ParsedExpr, SymbolDef, SymbolRef};

/// Parse document using v2 parser and convert to LSP types.
///
/// This uses the SAME parser as the agent pipeline, ensuring consistency.
pub fn parse_with_v2(text: &str) -> (DocumentState, Vec<Diagnostic>) {
    let mut state = DocumentState::new(text.to_string());
    let mut diagnostics = Vec::new();

    match parse_program(text) {
        Ok(program) => {
            state.expressions = convert_program(&program, text);
            extract_symbols(
                &state.expressions,
                &mut state.symbol_defs,
                &mut state.symbol_refs,
            );
        }
        Err(err) => {
            // V2 parse error - convert to diagnostic
            let range = extract_error_range(&err, text);
            diagnostics.push(Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::ERROR),
                code: Some(NumberOrString::String("E000".to_string())),
                source: Some("dsl-lsp".to_string()),
                message: simplify_error_message(&err),
                ..Default::default()
            });
        }
    }

    (state, diagnostics)
}

/// Convert v2 Program to LSP expressions.
fn convert_program(program: &Program, text: &str) -> Vec<ParsedExpr> {
    program
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Statement::VerbCall(vc) => Some(convert_verb_call(vc, text)),
            Statement::Comment(_) => None,
        })
        .collect()
}

/// Convert v2 VerbCall to LSP ParsedExpr.
fn convert_verb_call(vc: &VerbCall, text: &str) -> ParsedExpr {
    let verb_name = format!("{}.{}", vc.domain, vc.verb);
    let range = span_to_range(&vc.span, text);

    // Verb range is approximated from the full span (verb is at start after '(')
    let verb_end = vc.span.start + 1 + vc.domain.len() + 1 + vc.verb.len();
    let verb_range = span_to_range(&V2Span::new(vc.span.start + 1, verb_end), text);

    let mut args: Vec<ParsedArg> = vc
        .arguments
        .iter()
        .map(|arg| convert_argument(arg, text))
        .collect();

    // Add :as binding as a special argument if present
    if let Some(ref binding) = vc.binding {
        // Approximate the :as span from the verb call text
        let binding_text = format!(":as @{}", binding);
        let span_end = vc.span.end.min(text.len());
        let span_start = vc.span.start.min(span_end);
        if let Some(pos) = text
            .get(span_start..span_end)
            .and_then(|s| s.rfind(&binding_text))
        {
            let as_start = span_start + pos;
            let as_end = (as_start + binding_text.len()).min(text.len());
            let as_range = span_to_range(&V2Span::new(as_start, as_end), text);
            args.push(ParsedArg {
                keyword: ":as".to_string(),
                keyword_range: as_range,
                value: Some(Box::new(ParsedExpr {
                    range: as_range,
                    kind: ExprKind::SymbolRef {
                        name: binding.clone(),
                    },
                })),
            });
        } else {
            // Fallback: still add the binding even if we can't find the exact span
            args.push(ParsedArg {
                keyword: ":as".to_string(),
                keyword_range: range, // Use verb call range as fallback
                value: Some(Box::new(ParsedExpr {
                    range,
                    kind: ExprKind::SymbolRef {
                        name: binding.clone(),
                    },
                })),
            });
        }
    }

    ParsedExpr {
        range,
        kind: ExprKind::Call {
            verb_name,
            verb_range,
            args,
        },
    }
}

/// Convert v2 Argument to LSP ParsedArg.
fn convert_argument(arg: &Argument, text: &str) -> ParsedArg {
    let keyword = format!(":{}", arg.key);

    // Use argument span for keyword range (approximation - key is at start of span)
    let key_end = arg.span.start + 1 + arg.key.len(); // +1 for ':'
    let keyword_range = span_to_range(&V2Span::new(arg.span.start, key_end), text);

    // Value span is the rest of the argument span
    let value_span = V2Span::new(key_end + 1, arg.span.end); // +1 for space
    let value = Some(Box::new(convert_node(&arg.value, &value_span, text)));

    ParsedArg {
        keyword,
        keyword_range,
        value,
    }
}

/// Convert v2 AstNode to LSP ParsedExpr.
fn convert_node(node: &AstNode, span: &V2Span, text: &str) -> ParsedExpr {
    let range = span_to_range(span, text);

    let kind = match node {
        AstNode::Literal(lit) => match lit {
            Literal::String(s) => ExprKind::String { value: s.clone() },
            Literal::Integer(n) => ExprKind::Number {
                value: n.to_string(),
            },
            Literal::Decimal(d) => ExprKind::Number {
                value: d.to_string(),
            },
            Literal::Boolean(b) => ExprKind::Identifier {
                value: b.to_string(),
            },
            Literal::Null => ExprKind::Identifier {
                value: "nil".to_string(),
            },
            Literal::Uuid(uuid) => ExprKind::String {
                value: uuid.to_string(),
            },
        },

        AstNode::SymbolRef { name, .. } => ExprKind::SymbolRef { name: name.clone() },

        AstNode::EntityRef { value, .. } => {
            // Display the human-readable value (resolution happens later)
            ExprKind::String {
                value: value.clone(),
            }
        }

        AstNode::List {
            items,
            span: list_span,
        } => {
            let converted: Vec<ParsedExpr> = items
                .iter()
                .map(|v| convert_node(v, list_span, text))
                .collect();
            ExprKind::List { items: converted }
        }

        AstNode::Map { .. } => {
            // Maps are rare in DSL, represent as identifier for now
            ExprKind::Identifier {
                value: "{...}".to_string(),
            }
        }

        AstNode::Nested(vc) => {
            // Recursively convert nested verb call
            return convert_verb_call(vc, text);
        }
    };

    ParsedExpr { range, kind }
}

/// Convert v2 byte-offset Span to LSP line/column Range.
fn span_to_range(span: &V2Span, text: &str) -> Range {
    Range {
        start: offset_to_position(span.start, text),
        end: offset_to_position(span.end, text),
    }
}

/// Convert byte offset to LSP Position (line, character).
fn offset_to_position(offset: usize, text: &str) -> Position {
    let mut line = 0u32;
    let mut col = 0u32;

    for (i, ch) in text.chars().enumerate() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }

    Position {
        line,
        character: col,
    }
}

/// Extract symbol definitions and references from converted expressions.
fn extract_symbols(
    expressions: &[ParsedExpr],
    defs: &mut Vec<SymbolDef>,
    refs: &mut Vec<SymbolRef>,
) {
    for expr in expressions {
        extract_symbols_from_expr(expr, defs, refs);
    }
}

fn extract_symbols_from_expr(
    expr: &ParsedExpr,
    defs: &mut Vec<SymbolDef>,
    refs: &mut Vec<SymbolRef>,
) {
    match &expr.kind {
        ExprKind::Call {
            verb_name, args, ..
        } => {
            // Check for :as @symbol binding
            for arg in args {
                if arg.keyword == ":as" {
                    if let Some(value) = &arg.value {
                        if let ExprKind::SymbolRef { name } = &value.kind {
                            defs.push(SymbolDef {
                                name: name.clone(),
                                range: value.range,
                                defined_by: verb_name.clone(),
                                id_type: infer_id_type(verb_name),
                            });
                        }
                    }
                } else if let Some(value) = &arg.value {
                    extract_symbols_from_expr(value, defs, refs);
                }
            }
        }
        ExprKind::SymbolRef { name } => {
            refs.push(SymbolRef {
                name: name.clone(),
                range: expr.range,
            });
        }
        ExprKind::List { items } => {
            for item in items {
                extract_symbols_from_expr(item, defs, refs);
            }
        }
        _ => {}
    }
}

/// Infer the ID type from the verb that defined the symbol.
fn infer_id_type(verb_name: &str) -> String {
    match verb_name {
        "cbu.ensure" | "cbu.create" => "CbuId".to_string(),
        "entity.create-limited-company"
        | "entity.create-proper-person"
        | "entity.create-partnership"
        | "entity.create-trust" => "EntityId".to_string(),
        "investigation.create" => "InvestigationId".to_string(),
        "document.request" => "DocumentRequestId".to_string(),
        "screening.pep" | "screening.sanctions" => "ScreeningId".to_string(),
        "decision.record" => "DecisionId".to_string(),
        _ => "uuid".to_string(),
    }
}

/// Try to extract a position from nom's verbose error message.
fn extract_error_range(error: &str, text: &str) -> Range {
    // Nom verbose errors often have line numbers like "at line 2:"
    // For now, default to start of document
    let _ = (error, text);
    Range::default()
}

/// Simplify nom's verbose error message for display.
fn simplify_error_message(error: &str) -> String {
    // Take first meaningful line
    if let Some(line) = error.lines().find(|l| !l.trim().is_empty()) {
        if line.len() > 100 {
            format!("{}...", &line[..100])
        } else {
            line.to_string()
        }
    } else {
        "Parse error".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_parse() {
        let input = r#"(cbu.create :name "Test Fund")"#;
        let (state, diags) = parse_with_v2(input);

        assert!(diags.is_empty(), "Expected no errors: {:?}", diags);
        assert_eq!(state.expressions.len(), 1);

        if let ExprKind::Call {
            verb_name, args, ..
        } = &state.expressions[0].kind
        {
            assert_eq!(verb_name, "cbu.create");
            assert_eq!(args.len(), 1);
            assert_eq!(args[0].keyword, ":name");
        } else {
            panic!("Expected Call expression");
        }
    }

    #[test]
    fn test_symbol_binding() {
        let input = r#"(cbu.create :name "Test" :as @mycbu)"#;
        let (state, diags) = parse_with_v2(input);

        assert!(diags.is_empty());
        assert_eq!(state.symbol_defs.len(), 1);
        assert_eq!(state.symbol_defs[0].name, "mycbu");
        assert_eq!(state.symbol_defs[0].defined_by, "cbu.create");
    }

    #[test]
    fn test_symbol_reference() {
        let input = r#"(cbu.attach-entity :cbu-id @fund :entity-id @company)"#;
        let (state, diags) = parse_with_v2(input);

        assert!(diags.is_empty());
        assert_eq!(state.symbol_refs.len(), 2);
        assert!(state.symbol_refs.iter().any(|r| r.name == "fund"));
        assert!(state.symbol_refs.iter().any(|r| r.name == "company"));
    }

    #[test]
    fn test_nested_calls() {
        let input =
            r#"(cbu.create :name "Fund" :roles [(cbu.assign-role :entity-id @e :role "Mgr")])"#;
        let (state, diags) = parse_with_v2(input);

        assert!(diags.is_empty(), "Errors: {:?}", diags);
        assert_eq!(state.expressions.len(), 1);

        // Should have a reference to @e
        assert_eq!(state.symbol_refs.len(), 1);
        assert_eq!(state.symbol_refs[0].name, "e");
    }

    #[test]
    fn test_parse_error() {
        let input = r#"(cbu.create :name"#; // Unclosed
        let (_, diags) = parse_with_v2(input);

        assert!(!diags.is_empty(), "Expected parse error");
    }

    #[test]
    fn test_multiline() {
        let input = r#"
(cbu.create :name "Fund" :as @fund)
(entity.create-limited-company :name "Co" :as @co)
(cbu.attach-entity :cbu-id @fund :entity-id @co :role "Manager")
"#;
        let (state, diags) = parse_with_v2(input);

        assert!(diags.is_empty(), "Errors: {:?}", diags);
        assert_eq!(state.expressions.len(), 3);
        assert_eq!(state.symbol_defs.len(), 2);
        assert_eq!(state.symbol_refs.len(), 2); // @fund and @co in attach-entity
    }
}
