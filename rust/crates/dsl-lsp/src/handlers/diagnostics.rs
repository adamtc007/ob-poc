//! Diagnostics handler for the DSL Language Server.
//!
//! Parses DSL documents and reports errors.

use tower_lsp::lsp_types::*;

use crate::analysis::document::{
    DocumentState, ExprKind, ParsedArg, ParsedExpr, SymbolDef, SymbolRef,
};

use ob_poc::forth_engine::schema::registry::VERB_REGISTRY;
use ob_poc::forth_engine::schema::types::RequiredRule;

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
    extract_symbols(&state.expressions, &mut state.symbol_defs, &mut state.symbol_refs);

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
                            state.symbol_defs
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

/// Parse a DSL document into expressions.
fn parse_document(text: &str) -> (Vec<ParsedExpr>, Vec<ParseError>) {
    let mut expressions = Vec::new();
    let mut errors = Vec::new();
    let mut pos = 0;
    let chars: Vec<char> = text.chars().collect();

    while pos < chars.len() {
        // Skip whitespace
        while pos < chars.len() && chars[pos].is_whitespace() {
            pos += 1;
        }

        if pos >= chars.len() {
            break;
        }

        // Comment
        if chars[pos] == ';' {
            let start_pos = pos;
            let start = offset_to_position(text, pos);
            while pos < chars.len() && chars[pos] != '\n' {
                pos += 1;
            }
            let end = offset_to_position(text, pos);
            let comment_text: String = chars[start_pos..pos].iter().collect();
            expressions.push(ParsedExpr {
                kind: ExprKind::Comment { text: comment_text },
                range: Range { start, end },
                children: vec![],
            });
            continue;
        }

        // S-expression
        if chars[pos] == '(' {
            match parse_sexp(&chars, &mut pos, text) {
                Ok(expr) => expressions.push(expr),
                Err(err) => errors.push(err),
            }
            continue;
        }

        // Unexpected character
        let start = offset_to_position(text, pos);
        errors.push(ParseError {
            range: Range {
                start,
                end: Position {
                    line: start.line,
                    character: start.character + 1,
                },
            },
            message: format!("unexpected character: '{}'", chars[pos]),
        });
        pos += 1;
    }

    (expressions, errors)
}

/// Parse an s-expression.
fn parse_sexp(chars: &[char], pos: &mut usize, text: &str) -> Result<ParsedExpr, ParseError> {
    let start = offset_to_position(text, *pos);
    *pos += 1; // Skip '('

    // Skip whitespace
    while *pos < chars.len() && chars[*pos].is_whitespace() {
        *pos += 1;
    }

    // Parse verb name
    let verb_start = offset_to_position(text, *pos);
    let mut verb_name = String::new();
    while *pos < chars.len() && is_symbol_char(chars[*pos]) {
        verb_name.push(chars[*pos]);
        *pos += 1;
    }
    let verb_end = offset_to_position(text, *pos);

    // Parse arguments
    let mut args = Vec::new();
    let mut children = Vec::new();

    while *pos < chars.len() {
        // Skip whitespace
        while *pos < chars.len() && chars[*pos].is_whitespace() {
            *pos += 1;
        }

        if *pos >= chars.len() {
            return Err(ParseError {
                range: Range { start, end: offset_to_position(text, *pos) },
                message: "unclosed s-expression".to_string(),
            });
        }

        // End of s-expression
        if chars[*pos] == ')' {
            *pos += 1;
            break;
        }

        // Keyword argument
        if chars[*pos] == ':' {
            let _kw_start = *pos;
            let kw_start_pos = offset_to_position(text, *pos);
            *pos += 1;
            let mut keyword = String::from(":");
            while *pos < chars.len() && is_symbol_char(chars[*pos]) {
                keyword.push(chars[*pos]);
                *pos += 1;
            }
            let kw_end_pos = offset_to_position(text, *pos);

            // Skip whitespace before value
            while *pos < chars.len() && chars[*pos].is_whitespace() {
                *pos += 1;
            }

            // Parse value
            let (value, value_range) = if *pos < chars.len() && chars[*pos] != ':' && chars[*pos] != ')' {
                parse_value(chars, pos, text)?
            } else {
                (None, None)
            };

            args.push(ParsedArg {
                keyword: keyword.clone(),
                keyword_range: Range { start: kw_start_pos, end: kw_end_pos },
                value,
                value_range,
            });

            // Check for :as @symbol pattern
            if keyword == ":as" {
                if let Some(val) = args.last().and_then(|a| a.value.as_ref()) {
                    if let ExprKind::SymbolRef { name } = &val.kind {
                        children.push(ParsedExpr {
                            kind: ExprKind::SymbolDef { name: name.clone() },
                            range: val.range,
                            children: vec![],
                        });
                    }
                }
            }
            continue;
        }

        // Nested s-expression
        if chars[*pos] == '(' {
            let nested = parse_sexp(chars, pos, text)?;
            children.push(nested);
            continue;
        }

        // Other value (shouldn't happen in well-formed DSL)
        let (value, _) = parse_value(chars, pos, text)?;
        if let Some(v) = value {
            children.push(*v);
        }
    }

    let end = offset_to_position(text, *pos);

    Ok(ParsedExpr {
        kind: ExprKind::Call {
            verb_name,
            verb_range: Range { start: verb_start, end: verb_end },
            args,
        },
        range: Range { start, end },
        children,
    })
}

/// Parse a value (string, number, symbol ref, etc.).
fn parse_value(
    chars: &[char],
    pos: &mut usize,
    text: &str,
) -> Result<(Option<Box<ParsedExpr>>, Option<Range>), ParseError> {
    let start = offset_to_position(text, *pos);

    // String
    if chars[*pos] == '"' {
        *pos += 1;
        let mut value = String::new();
        while *pos < chars.len() && chars[*pos] != '"' {
            if chars[*pos] == '\\' && *pos + 1 < chars.len() {
                value.push(chars[*pos + 1]);
                *pos += 2;
            } else {
                value.push(chars[*pos]);
                *pos += 1;
            }
        }
        if *pos < chars.len() {
            *pos += 1; // Skip closing quote
        }
        let end = offset_to_position(text, *pos);
        let range = Range { start, end };
        return Ok((
            Some(Box::new(ParsedExpr {
                kind: ExprKind::String { value },
                range,
                children: vec![],
            })),
            Some(range),
        ));
    }

    // Symbol reference
    if chars[*pos] == '@' {
        *pos += 1;
        let mut name = String::new();
        while *pos < chars.len() && is_symbol_char(chars[*pos]) {
            name.push(chars[*pos]);
            *pos += 1;
        }
        let end = offset_to_position(text, *pos);
        let range = Range { start, end };
        return Ok((
            Some(Box::new(ParsedExpr {
                kind: ExprKind::SymbolRef { name },
                range,
                children: vec![],
            })),
            Some(range),
        ));
    }

    // Number or symbol
    let mut value = String::new();
    while *pos < chars.len() && !chars[*pos].is_whitespace() && chars[*pos] != ')' && chars[*pos] != ':' {
        value.push(chars[*pos]);
        *pos += 1;
    }
    let end = offset_to_position(text, *pos);
    let range = Range { start, end };

    if value.is_empty() {
        return Ok((None, None));
    }

    // Check if it's a number
    if value.parse::<f64>().is_ok() {
        return Ok((
            Some(Box::new(ParsedExpr {
                kind: ExprKind::Number { value },
                range,
                children: vec![],
            })),
            Some(range),
        ));
    }

    Ok((None, Some(range)))
}

/// Check if a character is valid in a symbol/keyword name.
fn is_symbol_char(c: char) -> bool {
    c.is_alphanumeric() || c == '-' || c == '_' || c == '.'
}

/// Convert byte offset to LSP position.
fn offset_to_position(text: &str, offset: usize) -> Position {
    let mut line = 0u32;
    let mut col = 0u32;
    for (i, c) in text.chars().enumerate() {
        if i >= offset {
            break;
        }
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    Position { line, character: col }
}

/// Extract symbol definitions and references from expressions.
fn extract_symbols(
    expressions: &[ParsedExpr],
    defs: &mut Vec<SymbolDef>,
    refs: &mut Vec<SymbolRef>,
) {
    for expr in expressions {
        if let ExprKind::Call { verb_name, args, .. } = &expr.kind {
            // Check for :as @symbol
            for arg in args {
                if arg.keyword == ":as" {
                    if let Some(ref val) = arg.value {
                        if let ExprKind::SymbolRef { name } = &val.kind {
                            defs.push(SymbolDef {
                                name: name.clone(),
                                range: val.range,
                                verb_name: verb_name.clone(),
                                line: val.range.start.line,
                            });
                        }
                    }
                }

                // Check for symbol refs in values
                if let Some(ref val) = arg.value {
                    if let ExprKind::SymbolRef { name } = &val.kind {
                        if arg.keyword != ":as" {
                            refs.push(SymbolRef {
                                name: name.clone(),
                                range: val.range,
                                line: val.range.start.line,
                            });
                        }
                    }
                }
            }
        }

        // Recurse into children
        extract_symbols(&expr.children, defs, refs);
    }
}

/// Validate an expression against verb schema.
#[allow(clippy::only_used_in_recursion)]
fn validate_expression(
    expr: &ParsedExpr,
    symbol_defs: &[SymbolDef],
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let ExprKind::Call { verb_name, verb_range, args } = &expr.kind {
        // Check verb exists
        let verb = match VERB_REGISTRY.get(verb_name) {
            Some(v) => v,
            None => {
                let suggestions = VERB_REGISTRY.suggest(verb_name);
                let message = if suggestions.is_empty() {
                    format!("unknown verb '{}'", verb_name)
                } else {
                    format!("unknown verb '{}'. Did you mean: {}?", verb_name, suggestions.join(", "))
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
        for arg in args {
            let is_known = verb.args.iter().any(|a| a.name == arg.keyword) || arg.keyword == ":as";
            if !is_known {
                let suggestions: Vec<_> = verb.args
                    .iter()
                    .filter(|a| {
                        let dist = levenshtein(a.name, &arg.keyword);
                        dist <= 3 || a.name.contains(&arg.keyword[1..])
                    })
                    .map(|a| a.name)
                    .collect();

                let message = if suggestions.is_empty() {
                    format!("unknown argument '{}' for verb '{}'", arg.keyword, verb_name)
                } else {
                    format!(
                        "unknown argument '{}' for verb '{}'. Did you mean: {}?",
                        arg.keyword, verb_name, suggestions.join(", ")
                    )
                };

                diagnostics.push(Diagnostic {
                    range: arg.keyword_range,
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: Some(NumberOrString::String("E002".to_string())),
                    source: Some("dsl-lsp".to_string()),
                    message,
                    ..Default::default()
                });
            }
        }

        // Check for missing required arguments
        let provided: std::collections::HashSet<_> = args.iter().map(|a| a.keyword.as_str()).collect();

        for spec in verb.args {
            let is_required = match &spec.required {
                RequiredRule::Always => true,
                RequiredRule::Never => false,
                RequiredRule::UnlessProvided(other) => !provided.contains(other),
                RequiredRule::IfProvided(other) => provided.contains(other),
                RequiredRule::IfEquals { arg, value } => {
                    // Check if arg equals value
                    args.iter()
                        .find(|a| a.keyword == *arg)
                        .and_then(|a| a.value.as_ref())
                        .map(|v| {
                            if let ExprKind::String { value: s } = &v.kind {
                                s == *value
                            } else {
                                false
                            }
                        })
                        .unwrap_or(false)
                }
            };

            if is_required && !provided.contains(spec.name) {
                diagnostics.push(Diagnostic {
                    range: expr.range,
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: Some(NumberOrString::String("E003".to_string())),
                    source: Some("dsl-lsp".to_string()),
                    message: format!("missing required argument '{}' for '{}'", spec.name, verb_name),
                    ..Default::default()
                });
            }
        }
    }

    // Validate children
    for child in &expr.children {
        validate_expression(child, symbol_defs, diagnostics);
    }
}

/// Simple Levenshtein distance.
fn levenshtein(a: &str, b: &str) -> usize {
    let a_len = a.chars().count();
    let b_len = b.chars().count();

    if a_len == 0 { return b_len; }
    if b_len == 0 { return a_len; }

    let mut matrix = vec![vec![0usize; b_len + 1]; a_len + 1];

    (0..=a_len).for_each(|i| matrix[i][0] = i);
    (0..=b_len).for_each(|j| matrix[0][j] = j);

    for (i, ca) in a.chars().enumerate() {
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            matrix[i + 1][j + 1] = (matrix[i][j + 1] + 1)
                .min(matrix[i + 1][j] + 1)
                .min(matrix[i][j] + cost);
        }
    }

    matrix[a_len][b_len]
}
