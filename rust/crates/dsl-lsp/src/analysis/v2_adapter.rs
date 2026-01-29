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
//!
//! ## Type Mapping
//!
//! | AST Type | LSP ExprKind |
//! |----------|--------------|
//! | `Literal::String` | `String` |
//! | `Literal::Integer` | `Number` |
//! | `Literal::Decimal` | `Number` |
//! | `Literal::Boolean` | `Boolean` |
//! | `Literal::Null` | `Null` |
//! | `Literal::Uuid` | `String` |
//! | `SymbolRef` | `SymbolRef` |
//! | `EntityRef` | `EntityRef` |
//! | `List` | `List` |
//! | `Map` | `Map` |
//! | `Nested` | `Call` |

use ob_poc::dsl_v2::{
    ast::{Argument, AstNode, Literal, Program, Span as V2Span, Statement, VerbCall},
    parse_program,
};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Range};

use super::document::{DocumentState, ExprKind, ParsedArg, ParsedExpr, SymbolDef, SymbolRef};
use crate::encoding::{span_to_range as encoding_span_to_range, PositionEncoding};

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

    // Value uses its own span (from the AstNode)
    let value = Some(Box::new(convert_node(&arg.value, text)));

    ParsedArg {
        keyword,
        keyword_range,
        value,
    }
}

/// Convert v2 AstNode to LSP ParsedExpr.
///
/// Uses each node's own span for accurate LSP positioning.
/// This ensures list items, map entries, and nested structures
/// have correct ranges for hover, go-to-definition, etc.
fn convert_node(node: &AstNode, text: &str) -> ParsedExpr {
    // Use the node's own span, not a parent span
    let node_span = node.span();
    let range = span_to_range(&node_span, text);

    let kind = match node {
        AstNode::Literal(lit, _span) => match lit {
            Literal::String(s) => ExprKind::String { value: s.clone() },
            Literal::Integer(n) => ExprKind::Number {
                value: n.to_string(),
            },
            Literal::Decimal(d) => ExprKind::Number {
                value: d.to_string(),
            },
            Literal::Boolean(b) => ExprKind::Boolean { value: *b },
            Literal::Null => ExprKind::Null,
            Literal::Uuid(uuid) => ExprKind::String {
                value: uuid.to_string(),
            },
        },

        AstNode::SymbolRef { name, .. } => ExprKind::SymbolRef { name: name.clone() },

        AstNode::EntityRef {
            entity_type,
            search_column,
            value,
            resolved_key,
            ..
        } => ExprKind::EntityRef {
            entity_type: entity_type.clone(),
            search_column: search_column.clone(),
            value: value.clone(),
            resolved: resolved_key.is_some(),
        },

        AstNode::List { items, .. } => {
            // Each item uses its own span
            let converted: Vec<ParsedExpr> = items.iter().map(|v| convert_node(v, text)).collect();
            ExprKind::List { items: converted }
        }

        AstNode::Map { entries, .. } => {
            // Each entry value uses its own span
            let converted: Vec<(String, ParsedExpr)> = entries
                .iter()
                .map(|(key, val)| {
                    let value_expr = convert_node(val, text);
                    (key.clone(), value_expr)
                })
                .collect();
            ExprKind::Map { entries: converted }
        }

        AstNode::Nested(vc) => {
            // Recursively convert nested verb call
            return convert_verb_call(vc, text);
        }
    };

    ParsedExpr { range, kind }
}

/// Convert v2 byte-offset Span to LSP line/column Range.
///
/// Uses UTF-16 encoding by default (LSP standard, used by Zed).
fn span_to_range(span: &V2Span, text: &str) -> Range {
    // Default to UTF-16 for Zed compatibility
    encoding_span_to_range(span.start, span.end, text, PositionEncoding::Utf16)
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
        ExprKind::Map { entries } => {
            for (_, value) in entries {
                extract_symbols_from_expr(value, defs, refs);
            }
        }
        _ => {}
    }
}

/// Infer the ID type from the verb registry's `produces` metadata.
///
/// This aligns with the REPL's BindingContext and uses the same source of truth.
fn infer_id_type(verb_name: &str) -> String {
    use ob_poc::dsl_v2::config::ConfigLoader;
    use ob_poc::dsl_v2::RuntimeVerbRegistry;

    // Try to load from registry (cached after first load)
    static REGISTRY: std::sync::OnceLock<Option<RuntimeVerbRegistry>> = std::sync::OnceLock::new();

    let registry = REGISTRY.get_or_init(|| {
        let loader = ConfigLoader::from_env();
        loader
            .load_verbs()
            .ok()
            .map(|config| RuntimeVerbRegistry::from_config(&config))
    });

    if let Some(reg) = registry {
        // Parse domain.verb
        if let Some((domain, verb)) = verb_name.split_once('.') {
            if let Some(produces) = reg.get_produces(domain, verb) {
                // Format as "type" or "type/subtype"
                return match &produces.subtype {
                    Some(sub) => format!("{}/{}", produces.produced_type, sub),
                    None => produces.produced_type.clone(),
                };
            }
        }
    }

    // Fallback for unknown verbs
    "uuid".to_string()
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

    #[test]
    fn test_boolean_conversion() {
        let input = r#"(test.verb :flag true :empty false)"#;
        let (state, diags) = parse_with_v2(input);

        assert!(diags.is_empty());
        if let ExprKind::Call { args, .. } = &state.expressions[0].kind {
            if let Some(val) = &args[0].value {
                assert!(matches!(val.kind, ExprKind::Boolean { value: true }));
            }
            if let Some(val) = &args[1].value {
                assert!(matches!(val.kind, ExprKind::Boolean { value: false }));
            }
        }
    }

    #[test]
    fn test_null_conversion() {
        let input = r#"(test.verb :empty nil)"#;
        let (state, diags) = parse_with_v2(input);

        assert!(diags.is_empty());
        if let ExprKind::Call { args, .. } = &state.expressions[0].kind {
            if let Some(val) = &args[0].value {
                assert!(matches!(val.kind, ExprKind::Null));
            }
        }
    }

    #[test]
    fn test_map_conversion() {
        let input = r#"(test.verb :config {:name "Test" :value 42})"#;
        let (state, diags) = parse_with_v2(input);

        assert!(diags.is_empty());
        if let ExprKind::Call { args, .. } = &state.expressions[0].kind {
            if let Some(val) = &args[0].value {
                if let ExprKind::Map { entries } = &val.kind {
                    assert_eq!(entries.len(), 2);
                    assert!(entries.iter().any(|(k, _)| k == "name"));
                    assert!(entries.iter().any(|(k, _)| k == "value"));
                } else {
                    panic!("Expected Map, got {:?}", val.kind);
                }
            }
        }
    }

    #[test]
    fn test_list_with_symbols() {
        let input = r#"(test.verb :items [@a @b @c])"#;
        let (state, diags) = parse_with_v2(input);

        assert!(diags.is_empty());
        // Should have 3 symbol references
        assert_eq!(state.symbol_refs.len(), 3);
    }
}
