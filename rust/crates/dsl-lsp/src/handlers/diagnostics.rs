//! Diagnostics handler for the DSL Language Server.
//!
//! Uses the v2 parser for parsing and provides semantic validation.

use tower_lsp::lsp_types::*;

use crate::analysis::document::{DocumentState, ExprKind, ParsedExpr, SymbolDef};
use crate::analysis::parse_with_v2;

use ob_poc::dsl_v2::{find_unified_verb, registry};

/// Analyze a document and return state + diagnostics.
pub fn analyze_document(text: &str) -> (DocumentState, Vec<Diagnostic>) {
    // Use v2 parser via adapter
    let (state, mut diagnostics) = parse_with_v2(text);

    // Validate expressions against verb schema
    for expr in &state.expressions {
        validate_expression(expr, &state.symbol_defs, &mut diagnostics);
    }

    // Check for undefined symbol references
    for sym_ref in &state.symbol_refs {
        if !state.symbol_defs.iter().any(|d| d.name == sym_ref.name) {
            diagnostics.push(Diagnostic {
                range: sym_ref.range,
                severity: Some(DiagnosticSeverity::ERROR),
                code: Some(NumberOrString::String("E007".to_string())),
                source: Some("dsl-lsp".to_string()),
                message: format!("undefined symbol '@{}'", sym_ref.name),
                related_information: if !state.symbol_defs.is_empty() {
                    Some(vec![DiagnosticRelatedInformation {
                        location: Location {
                            uri: Url::parse("file:///").unwrap(),
                            range: Range::default(),
                        },
                        message: format!(
                            "defined symbols: {}",
                            state
                                .symbol_defs
                                .iter()
                                .map(|d| format!("@{}", d.name))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                    }])
                } else {
                    None
                },
                ..Default::default()
            });
        }
    }

    (state, diagnostics)
}

/// Validate an expression against verb schema.
fn validate_expression(
    expr: &ParsedExpr,
    symbol_defs: &[SymbolDef],
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let ExprKind::Call {
        verb_name,
        verb_range,
        args,
    } = &expr.kind
    {
        // Parse domain.verb
        let parts: Vec<&str> = verb_name.split('.').collect();
        if parts.len() != 2 {
            diagnostics.push(Diagnostic {
                range: *verb_range,
                severity: Some(DiagnosticSeverity::ERROR),
                code: Some(NumberOrString::String("E001".to_string())),
                source: Some("dsl-lsp".to_string()),
                message: format!(
                    "invalid verb format '{}', expected 'domain.verb'",
                    verb_name
                ),
                ..Default::default()
            });
            return;
        }

        // Check verb exists
        let verb = match find_unified_verb(parts[0], parts[1]) {
            Some(v) => v,
            None => {
                // Suggest similar verbs
                let reg = registry();
                let suggestions: Vec<String> = reg
                    .all_verbs()
                    .filter(|v| {
                        v.domain == parts[0]
                            || v.verb.contains(parts[1])
                            || v.full_name().contains(verb_name)
                    })
                    .take(3)
                    .map(|v| v.full_name())
                    .collect();

                let message = if suggestions.is_empty() {
                    format!("unknown verb '{}'", verb_name)
                } else {
                    format!(
                        "unknown verb '{}'. Did you mean: {}?",
                        verb_name,
                        suggestions.join(", ")
                    )
                };

                diagnostics.push(Diagnostic {
                    range: *verb_range,
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: Some(NumberOrString::String("E001".to_string())),
                    source: Some("dsl-lsp".to_string()),
                    message,
                    ..Default::default()
                });
                return;
            }
        };

        // Check for unknown arguments
        let all_known_args: Vec<&str> = verb
            .required_arg_names()
            .into_iter()
            .chain(verb.optional_arg_names())
            .collect();

        for arg in args {
            if arg.keyword.is_empty() {
                continue; // Skip nested expressions without keyword
            }

            let arg_name = arg.keyword.trim_start_matches(':');
            let is_known = all_known_args.contains(&arg_name) || arg_name == "as";

            if !is_known {
                diagnostics.push(Diagnostic {
                    range: arg.keyword_range,
                    severity: Some(DiagnosticSeverity::WARNING),
                    code: Some(NumberOrString::String("E002".to_string())),
                    source: Some("dsl-lsp".to_string()),
                    message: format!(
                        "unknown argument '{}' for verb '{}'",
                        arg.keyword, verb_name
                    ),
                    ..Default::default()
                });
            }
        }

        // Check for missing required arguments
        let provided: std::collections::HashSet<&str> = args
            .iter()
            .map(|a| a.keyword.trim_start_matches(':'))
            .collect();

        for required_arg in verb.required_arg_names() {
            if !provided.contains(required_arg) {
                diagnostics.push(Diagnostic {
                    range: expr.range,
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: Some(NumberOrString::String("E003".to_string())),
                    source: Some("dsl-lsp".to_string()),
                    message: format!(
                        "missing required argument '{}' for '{}'",
                        required_arg, verb_name
                    ),
                    ..Default::default()
                });
            }
        }

        // Recursively validate nested expressions
        for arg in args {
            if let Some(value) = &arg.value {
                validate_nested_expr(value, symbol_defs, diagnostics);
            }
        }
    }
}

fn validate_nested_expr(
    expr: &ParsedExpr,
    symbol_defs: &[SymbolDef],
    diagnostics: &mut Vec<Diagnostic>,
) {
    match &expr.kind {
        ExprKind::Call { .. } => {
            validate_expression(expr, symbol_defs, diagnostics);
        }
        ExprKind::List { items } => {
            for item in items {
                validate_nested_expr(item, symbol_defs, diagnostics);
            }
        }
        _ => {}
    }
}
