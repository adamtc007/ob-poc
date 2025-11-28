//! Diagnostics handler for the DSL Language Server.
//!
//! Parses DSL documents and reports errors.

use tower_lsp::lsp_types::*;

use crate::analysis::document::{
    DocumentState, ExprKind, ParsedArg, ParsedExpr, SymbolDef, SymbolRef,
};

use ob_poc::dsl_v2::{find_verb, STANDARD_VERBS};

/// Analyze a document and return state + diagnostics.
pub fn analyze_document(text: &str) -> (DocumentState, Vec<Diagnostic>) {
    let mut state = DocumentState::new(text.to_string());
    let mut diagnostics = Vec::new();

    // Parse the document
    let (expressions, parse_errors) = parse_document(text);
    state.expressions = expressions;

    // Add parse errors as diagnostics
    for err in parse_errors {
        diagnostics.push(Diagnostic {
            range: err.range,
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String("E001".to_string())),
            source: Some("dsl-lsp".to_string()),
            message: err.message,
            ..Default::default()
        });
    }

    // Extract symbols
    extract_symbols(
        &state.expressions,
        &mut state.symbol_defs,
        &mut state.symbol_refs,
    );

    // Validate expressions
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

/// Parse error info.
struct ParseError {
    range: Range,
    message: String,
}

/// Parse the document into expressions.
fn parse_document(text: &str) -> (Vec<ParsedExpr>, Vec<ParseError>) {
    let mut expressions = Vec::new();
    let mut errors = Vec::new();

    let chars: Vec<char> = text.chars().collect();
    let mut pos = 0;

    while pos < chars.len() {
        // Skip whitespace
        while pos < chars.len() && chars[pos].is_whitespace() {
            pos += 1;
        }

        if pos >= chars.len() {
            break;
        }

        // Skip comments
        if pos + 1 < chars.len() && chars[pos] == ';' && chars[pos + 1] == ';' {
            while pos < chars.len() && chars[pos] != '\n' {
                pos += 1;
            }
            continue;
        }

        // Parse expression
        if chars[pos] == '(' {
            match parse_expression(&chars, &mut pos, text) {
                Ok(expr) => expressions.push(expr),
                Err(e) => errors.push(e),
            }
        } else {
            // Unexpected character
            let position = offset_to_position(pos, text);
            errors.push(ParseError {
                range: Range {
                    start: position,
                    end: Position {
                        line: position.line,
                        character: position.character + 1,
                    },
                },
                message: format!("unexpected character '{}'", chars[pos]),
            });
            pos += 1;
        }
    }

    (expressions, errors)
}

/// Parse a single expression starting at '('.
fn parse_expression(chars: &[char], pos: &mut usize, text: &str) -> Result<ParsedExpr, ParseError> {
    let start_pos = *pos;
    let start_position = offset_to_position(start_pos, text);

    // Consume '('
    *pos += 1;

    // Skip whitespace
    skip_whitespace(chars, pos);

    // Parse verb name (domain.verb)
    let verb_start = *pos;
    let verb_start_position = offset_to_position(verb_start, text);

    while *pos < chars.len()
        && !chars[*pos].is_whitespace()
        && chars[*pos] != ')'
        && chars[*pos] != '('
    {
        *pos += 1;
    }

    let verb_name: String = chars[verb_start..*pos].iter().collect();
    let verb_end_position = offset_to_position(*pos, text);
    let verb_range = Range {
        start: verb_start_position,
        end: verb_end_position,
    };

    if verb_name.is_empty() {
        return Err(ParseError {
            range: Range {
                start: start_position,
                end: verb_end_position,
            },
            message: "expected verb name".to_string(),
        });
    }

    // Parse arguments
    let mut args = Vec::new();

    loop {
        skip_whitespace(chars, pos);

        if *pos >= chars.len() {
            return Err(ParseError {
                range: Range {
                    start: start_position,
                    end: offset_to_position(*pos, text),
                },
                message: "unclosed expression, expected ')'".to_string(),
            });
        }

        if chars[*pos] == ')' {
            *pos += 1;
            break;
        }

        // Parse keyword
        if chars[*pos] == ':' {
            let kw_start = *pos;
            let kw_start_pos = offset_to_position(kw_start, text);
            *pos += 1;

            while *pos < chars.len() && !chars[*pos].is_whitespace() && chars[*pos] != ')' {
                *pos += 1;
            }

            let keyword: String = chars[kw_start..*pos].iter().collect();
            let kw_end_pos = offset_to_position(*pos, text);
            let keyword_range = Range {
                start: kw_start_pos,
                end: kw_end_pos,
            };

            skip_whitespace(chars, pos);

            // Parse value
            let value = if *pos < chars.len() && chars[*pos] != ')' && chars[*pos] != ':' {
                Some(Box::new(parse_value(chars, pos, text)?))
            } else {
                None
            };

            args.push(ParsedArg {
                keyword,
                keyword_range,
                value,
            });
        } else if chars[*pos] == '(' {
            // Nested expression
            let nested = parse_expression(chars, pos, text)?;
            // Store as a value - not typical but handles nested calls
            args.push(ParsedArg {
                keyword: String::new(),
                keyword_range: Range::default(),
                value: Some(Box::new(nested)),
            });
        } else {
            // Unexpected
            let ch = chars[*pos];
            *pos += 1;
            return Err(ParseError {
                range: Range {
                    start: offset_to_position(*pos - 1, text),
                    end: offset_to_position(*pos, text),
                },
                message: format!("expected keyword starting with ':', got '{}'", ch),
            });
        }
    }

    let end_position = offset_to_position(*pos, text);

    Ok(ParsedExpr {
        range: Range {
            start: start_position,
            end: end_position,
        },
        kind: ExprKind::Call {
            verb_name,
            verb_range,
            args,
        },
    })
}

/// Parse a value (string, number, symbol ref, list, or nested expression).
fn parse_value(chars: &[char], pos: &mut usize, text: &str) -> Result<ParsedExpr, ParseError> {
    let start = *pos;
    let start_position = offset_to_position(start, text);

    if chars[*pos] == '"' {
        // String literal
        *pos += 1;
        let str_start = *pos;

        while *pos < chars.len() && chars[*pos] != '"' {
            if chars[*pos] == '\\' && *pos + 1 < chars.len() {
                *pos += 2;
            } else {
                *pos += 1;
            }
        }

        if *pos >= chars.len() {
            return Err(ParseError {
                range: Range {
                    start: start_position,
                    end: offset_to_position(*pos, text),
                },
                message: "unclosed string literal".to_string(),
            });
        }

        let value: String = chars[str_start..*pos].iter().collect();
        *pos += 1; // consume closing "

        Ok(ParsedExpr {
            range: Range {
                start: start_position,
                end: offset_to_position(*pos, text),
            },
            kind: ExprKind::String { value },
        })
    } else if chars[*pos] == '@' {
        // Symbol reference
        *pos += 1;
        let sym_start = *pos;

        while *pos < chars.len()
            && (chars[*pos].is_alphanumeric() || chars[*pos] == '_' || chars[*pos] == '-')
        {
            *pos += 1;
        }

        let name: String = chars[sym_start..*pos].iter().collect();

        Ok(ParsedExpr {
            range: Range {
                start: start_position,
                end: offset_to_position(*pos, text),
            },
            kind: ExprKind::SymbolRef { name },
        })
    } else if chars[*pos] == '[' {
        // List
        *pos += 1;
        let mut items = Vec::new();

        loop {
            skip_whitespace(chars, pos);

            if *pos >= chars.len() {
                return Err(ParseError {
                    range: Range {
                        start: start_position,
                        end: offset_to_position(*pos, text),
                    },
                    message: "unclosed list, expected ']'".to_string(),
                });
            }

            if chars[*pos] == ']' {
                *pos += 1;
                break;
            }

            // Skip commas
            if chars[*pos] == ',' {
                *pos += 1;
                continue;
            }

            items.push(parse_value(chars, pos, text)?);
        }

        Ok(ParsedExpr {
            range: Range {
                start: start_position,
                end: offset_to_position(*pos, text),
            },
            kind: ExprKind::List { items },
        })
    } else if chars[*pos] == '(' {
        // Nested expression
        parse_expression(chars, pos, text)
    } else if chars[*pos].is_ascii_digit()
        || (chars[*pos] == '-' && *pos + 1 < chars.len() && chars[*pos + 1].is_ascii_digit())
    {
        // Number
        let num_start = *pos;
        if chars[*pos] == '-' {
            *pos += 1;
        }
        while *pos < chars.len() && (chars[*pos].is_ascii_digit() || chars[*pos] == '.') {
            *pos += 1;
        }
        let value: String = chars[num_start..*pos].iter().collect();

        Ok(ParsedExpr {
            range: Range {
                start: start_position,
                end: offset_to_position(*pos, text),
            },
            kind: ExprKind::Number { value },
        })
    } else if chars[*pos].is_alphabetic() {
        // Identifier (true, false, nil, or bare word)
        let id_start = *pos;
        while *pos < chars.len()
            && (chars[*pos].is_alphanumeric() || chars[*pos] == '_' || chars[*pos] == '-')
        {
            *pos += 1;
        }
        let value: String = chars[id_start..*pos].iter().collect();

        Ok(ParsedExpr {
            range: Range {
                start: start_position,
                end: offset_to_position(*pos, text),
            },
            kind: ExprKind::Identifier { value },
        })
    } else {
        Err(ParseError {
            range: Range {
                start: start_position,
                end: offset_to_position(*pos + 1, text),
            },
            message: format!("unexpected character '{}' in value", chars[*pos]),
        })
    }
}

fn skip_whitespace(chars: &[char], pos: &mut usize) {
    while *pos < chars.len() && chars[*pos].is_whitespace() {
        *pos += 1;
    }
}

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

/// Extract symbol definitions and references from expressions.
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
                                id_type: "uuid".to_string(),
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
        let verb = match find_verb(parts[0], parts[1]) {
            Some(v) => v,
            None => {
                // Suggest similar verbs
                let suggestions: Vec<String> = STANDARD_VERBS
                    .iter()
                    .filter(|v| {
                        v.domain == parts[0]
                            || v.verb.contains(parts[1])
                            || format!("{}.{}", v.domain, v.verb).contains(verb_name)
                    })
                    .take(3)
                    .map(|v| format!("{}.{}", v.domain, v.verb))
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
            .required_args
            .iter()
            .chain(verb.optional_args.iter())
            .copied()
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

        for required_arg in verb.required_args {
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
