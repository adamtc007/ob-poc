//! Hover handler for the DSL Language Server.

use tower_lsp::lsp_types::*;

use crate::analysis::DocumentState;

use ob_poc::dsl_v2::{find_verb, VerbDef};

/// Get hover information at position.
pub fn get_hover(doc: &DocumentState, position: Position) -> Option<Hover> {
    // Check if we're hovering over a verb
    for expr in &doc.expressions {
        if let crate::analysis::document::ExprKind::Call {
            verb_name,
            verb_range,
            ..
        } = &expr.kind
        {
            if position_in_range(position, verb_range) {
                let parts: Vec<&str> = verb_name.split('.').collect();
                if parts.len() == 2 {
                    if let Some(verb) = find_verb(parts[0], parts[1]) {
                        return Some(Hover {
                            contents: HoverContents::Markup(MarkupContent {
                                kind: MarkupKind::Markdown,
                                value: format_verb_hover(verb),
                            }),
                            range: Some(*verb_range),
                        });
                    }
                }
            }
        }
    }

    // Check if we're hovering over a symbol
    for sym_def in &doc.symbol_defs {
        if position_in_range(position, &sym_def.range) {
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!(
                        "**Symbol** `@{}`\n\nDefined by: `{}`\nType: `{}`",
                        sym_def.name, sym_def.defined_by, sym_def.id_type
                    ),
                }),
                range: Some(sym_def.range),
            });
        }
    }

    for sym_ref in &doc.symbol_refs {
        if position_in_range(position, &sym_ref.range) {
            // Find definition
            let def = doc.symbol_defs.iter().find(|d| d.name == sym_ref.name);
            let info = if let Some(d) = def {
                format!("Defined by: `{}`\nType: `{}`", d.defined_by, d.id_type)
            } else {
                "⚠️ Undefined symbol".to_string()
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

fn format_verb_hover(verb: &VerbDef) -> String {
    let mut parts = Vec::new();

    parts.push(format!("**{}.{}**", verb.domain, verb.verb));
    parts.push(String::new());
    parts.push(verb.description.to_string());
    parts.push(String::new());

    if !verb.required_args.is_empty() {
        parts.push("**Required arguments:**".to_string());
        for arg in verb.required_args {
            parts.push(format!("- `:{}`", arg));
        }
        parts.push(String::new());
    }

    if !verb.optional_args.is_empty() {
        parts.push("**Optional arguments:**".to_string());
        for arg in verb.optional_args {
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
