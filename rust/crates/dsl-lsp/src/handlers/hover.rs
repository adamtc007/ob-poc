//! Hover handler for the DSL Language Server.

use tower_lsp::lsp_types::*;

use crate::analysis::document::{ExprKind, ParsedArg};
use crate::analysis::DocumentState;

use ob_poc::dsl_v2::{find_unified_verb, UnifiedVerbDef};

/// Get hover information at position.
pub fn get_hover(doc: &DocumentState, position: Position) -> Option<Hover> {
    // Check if we're hovering over a verb or keyword argument
    for expr in &doc.expressions {
        if let ExprKind::Call {
            verb_name,
            verb_range,
            args,
        } = &expr.kind
        {
            // First check if hovering over verb name
            if position_in_range(position, verb_range) {
                let parts: Vec<&str> = verb_name.split('.').collect();
                if parts.len() == 2 {
                    if let Some(verb) = find_unified_verb(parts[0], parts[1]) {
                        return Some(Hover {
                            contents: HoverContents::Markup(MarkupContent {
                                kind: MarkupKind::Markdown,
                                value: format_verb_hover(verb),
                            }),
                            range: Some(*verb_range),
                        });
                    } else {
                        // Unknown verb - show error hover
                        let suggestion = find_similar_verb(verb_name);
                        let message = if let Some(similar) = suggestion {
                            format!(
                                "**Unknown verb** `{}`\n\nDid you mean `{}`?",
                                verb_name, similar
                            )
                        } else {
                            format!("**Unknown verb** `{}`", verb_name)
                        };
                        return Some(Hover {
                            contents: HoverContents::Markup(MarkupContent {
                                kind: MarkupKind::Markdown,
                                value: message,
                            }),
                            range: Some(*verb_range),
                        });
                    }
                }
            }

            // Check if hovering over a keyword argument
            if let Some(hover) = hover_on_keyword(verb_name, args, position) {
                return Some(hover);
            }
        }
    }

    // Check if we're hovering over a symbol definition
    for sym_def in &doc.symbol_defs {
        if position_in_range(position, &sym_def.range) {
            // Count references
            let ref_count = doc
                .symbol_refs
                .iter()
                .filter(|r| r.name == sym_def.name)
                .count();

            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!(
                        "**Symbol** `@{}`\n\nDefined by: `{}`\nType: `{}`\nUsed {} time{}",
                        sym_def.name,
                        sym_def.defined_by,
                        sym_def.id_type,
                        ref_count,
                        if ref_count == 1 { "" } else { "s" }
                    ),
                }),
                range: Some(sym_def.range),
            });
        }
    }

    // Check symbol references
    for sym_ref in &doc.symbol_refs {
        if position_in_range(position, &sym_ref.range) {
            // Find definition
            let def = doc.symbol_defs.iter().find(|d| d.name == sym_ref.name);
            let info = if let Some(d) = def {
                format!("Defined by: `{}`\nType: `{}`", d.defined_by, d.id_type)
            } else {
                // Undefined symbol - suggest similar
                let suggestion = find_similar_symbol(&sym_ref.name, doc);
                if let Some(similar) = suggestion {
                    format!("⚠️ Undefined symbol\n\nDid you mean `@{}`?", similar)
                } else {
                    "⚠️ Undefined symbol (no binding in file)".to_string()
                }
            };

            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("**Symbol Reference** `@{}`\n\n{}", sym_ref.name, info),
                }),
                range: Some(sym_ref.range),
            });
        }
    }

    None
}

/// Hover on a keyword argument - show type, required status, and description.
fn hover_on_keyword(verb_name: &str, args: &[ParsedArg], position: Position) -> Option<Hover> {
    for arg in args {
        if position_in_range(position, &arg.keyword_range) {
            // Get verb definition to find argument info
            let parts: Vec<&str> = verb_name.split('.').collect();
            if parts.len() == 2 {
                if let Some(verb) = find_unified_verb(parts[0], parts[1]) {
                    // Find matching argument definition
                    let keyword_name = arg.keyword.strip_prefix(':').unwrap_or(&arg.keyword);
                    if let Some(arg_def) = verb.args.iter().find(|a| a.name == keyword_name) {
                        let required = if arg_def.required {
                            "**required**"
                        } else {
                            "optional"
                        };

                        let lookup_info = if let Some(ref lookup) = arg_def.lookup {
                            let entity_type = lookup.entity_type.as_deref().unwrap_or("entity");
                            let search_key = format!("{:?}", lookup.search_key);
                            format!("\n\nLookup: `{}` by `{}`", entity_type, search_key)
                        } else {
                            String::new()
                        };

                        let desc = if arg_def.description.is_empty() {
                            String::new()
                        } else {
                            format!("\n\n{}", arg_def.description)
                        };

                        return Some(Hover {
                            contents: HoverContents::Markup(MarkupContent {
                                kind: MarkupKind::Markdown,
                                value: format!(
                                    "**`:{}** ({})\n\nType: `{}`{}{}",
                                    keyword_name, required, arg_def.arg_type, desc, lookup_info
                                ),
                            }),
                            range: Some(arg.keyword_range),
                        });
                    } else {
                        // Unknown keyword for this verb - suggest similar
                        let suggestion = find_similar_keyword(keyword_name, &verb.args);
                        let message = if let Some(similar) = suggestion {
                            format!(
                                "**Unknown argument** `:{}`\n\nDid you mean `:{}` for `{}`?",
                                keyword_name, similar, verb_name
                            )
                        } else {
                            format!(
                                "**Unknown argument** `:{}`\n\n`{}` does not accept this argument",
                                keyword_name, verb_name
                            )
                        };
                        return Some(Hover {
                            contents: HoverContents::Markup(MarkupContent {
                                kind: MarkupKind::Markdown,
                                value: message,
                            }),
                            range: Some(arg.keyword_range),
                        });
                    }
                }
            }
        }
    }
    None
}

// =============================================================================
// "Did You Mean" Suggestions
// =============================================================================

use ob_poc::dsl_v2::{registry, ArgDef};

/// Find similar verb name using levenshtein distance.
fn find_similar_verb(name: &str) -> Option<String> {
    let reg = registry();

    reg.all_verbs()
        .map(|v| (v.full_name(), levenshtein(name, &v.full_name())))
        .filter(|(_, dist)| *dist <= 4)
        .min_by_key(|(_, dist)| *dist)
        .map(|(name, _)| name)
}

/// Find similar symbol name from document definitions.
fn find_similar_symbol(name: &str, doc: &DocumentState) -> Option<String> {
    doc.symbol_defs
        .iter()
        .map(|d| (&d.name, levenshtein(name, &d.name)))
        .filter(|(_, dist)| *dist <= 2)
        .min_by_key(|(_, dist)| *dist)
        .map(|(name, _)| name.clone())
}

/// Find similar keyword from verb arguments.
fn find_similar_keyword(keyword: &str, args: &[ArgDef]) -> Option<String> {
    args.iter()
        .map(|a| (&a.name, levenshtein(keyword, &a.name)))
        .filter(|(_, dist)| *dist <= 3)
        .min_by_key(|(_, dist)| *dist)
        .map(|(name, _)| name.clone())
}

/// Simple Levenshtein distance for suggestions.
#[allow(clippy::needless_range_loop)]
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut dp = vec![vec![0; b.len() + 1]; a.len() + 1];

    for i in 0..=a.len() {
        dp[i][0] = i;
    }
    for j in 0..=b.len() {
        dp[0][j] = j;
    }

    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }

    dp[a.len()][b.len()]
}

// =============================================================================
// Hover Formatting
// =============================================================================

fn format_verb_hover(verb: &UnifiedVerbDef) -> String {
    let mut parts = Vec::new();

    parts.push(format!("**{}.{}**", verb.domain, verb.verb));
    parts.push(String::new());
    parts.push(verb.description.clone());
    parts.push(String::new());

    let required = verb.required_arg_names();
    if !required.is_empty() {
        parts.push("**Required arguments:**".to_string());
        for arg in required {
            parts.push(format!("- `:{}`", arg));
        }
        parts.push(String::new());
    }

    let optional = verb.optional_arg_names();
    if !optional.is_empty() {
        parts.push("**Optional arguments:**".to_string());
        for arg in optional {
            parts.push(format!("- `:{}`", arg));
        }
    }

    parts.join("\n")
}

fn position_in_range(position: Position, range: &Range) -> bool {
    if position.line < range.start.line || position.line > range.end.line {
        return false;
    }
    if position.line == range.start.line && position.character < range.start.character {
        return false;
    }
    if position.line == range.end.line && position.character > range.end.character {
        return false;
    }
    true
}
