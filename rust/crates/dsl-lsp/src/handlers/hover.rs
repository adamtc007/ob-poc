//! Hover handler for the DSL Language Server.

use tower_lsp::lsp_types::*;

use crate::analysis::DocumentState;

use ob_poc::forth_engine::schema::registry::VERB_REGISTRY;
use ob_poc::forth_engine::schema::types::RequiredRule;

/// Get hover information at a position.
pub fn get_hover(doc: &DocumentState, position: Position) -> Option<Hover> {
    let line = doc.get_line(position.line)?;
    let col = position.character as usize;

    // Find the word at cursor
    let (word, word_range) = find_word_at_position(line, col, position.line)?;

    // Check if it's a verb
    if let Some(verb) = VERB_REGISTRY.get(&word) {
        return Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format_verb_hover(verb),
            }),
            range: Some(word_range),
        });
    }

    // Check if it's a keyword (starts with :)
    if word.starts_with(':') {
        // Find the enclosing verb to get keyword info
        if let Some((verb_name, _)) = doc.find_call_at_position(position) {
            if let Some(verb) = VERB_REGISTRY.get(verb_name) {
                if let Some(arg) = verb.args.iter().find(|a| a.name == word) {
                    return Some(Hover {
                        contents: HoverContents::Markup(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: format_arg_hover(arg, verb.name),
                        }),
                        range: Some(word_range),
                    });
                }
            }
        }
    }

    // Check if it's a symbol reference (@name)
    if let Some(symbol_name) = word.strip_prefix('@') {
        
        if let Some(def) = doc.get_symbol_def(symbol_name) {
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!(
                        "**Symbol** `@{}`\n\nDefined by `{}` at line {}",
                        symbol_name,
                        def.verb_name,
                        def.line + 1
                    ),
                }),
                range: Some(word_range),
            });
        }
    }

    None
}

/// Find the word at a position in a line.
fn find_word_at_position(line: &str, col: usize, line_num: u32) -> Option<(String, Range)> {
    if col > line.len() {
        return None;
    }

    let chars: Vec<char> = line.chars().collect();
    
    // Find word boundaries
    let mut start = col;
    while start > 0 {
        let c = chars.get(start - 1)?;
        if !is_word_char(*c) {
            break;
        }
        start -= 1;
    }

    let mut end = col;
    while end < chars.len() {
        let c = chars.get(end)?;
        if !is_word_char(*c) {
            break;
        }
        end += 1;
    }

    if start == end {
        return None;
    }

    let word: String = chars[start..end].iter().collect();
    let range = Range {
        start: Position {
            line: line_num,
            character: start as u32,
        },
        end: Position {
            line: line_num,
            character: end as u32,
        },
    };

    Some((word, range))
}

/// Check if a character is part of a word.
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == ':' || c == '@'
}

/// Format hover content for a verb.
fn format_verb_hover(verb: &ob_poc::forth_engine::schema::types::VerbDef) -> String {
    let mut parts = vec![
        format!("## {}", verb.name),
        String::new(),
        verb.description.to_string(),
        String::new(),
        "### Arguments".to_string(),
        String::new(),
    ];

    for arg in verb.args {
        let required = match &arg.required {
            RequiredRule::Always => "**required**",
            RequiredRule::Never => "optional",
            RequiredRule::UnlessProvided(other) => &format!("required unless `{}` provided", other),
            RequiredRule::IfEquals { arg, value } => &format!("required if `{} = \"{}\"`", arg, value),
            RequiredRule::IfProvided(other) => &format!("required if `{}` provided", other),
        };

        parts.push(format!(
            "- `{}` ({}) [{}]",
            arg.name,
            arg.sem_type.type_name(),
            required
        ));
        parts.push(format!("  - {}", arg.description));
    }

    if !verb.examples.is_empty() {
        parts.push(String::new());
        parts.push("### Examples".to_string());
        parts.push(String::new());
        parts.push("```clojure".to_string());
        for ex in verb.examples {
            parts.push(ex.to_string());
        }
        parts.push("```".to_string());
    }

    parts.join("\n")
}

/// Format hover content for an argument.
fn format_arg_hover(
    arg: &ob_poc::forth_engine::schema::types::ArgSpec,
    verb_name: &str,
) -> String {
    let required = match &arg.required {
        RequiredRule::Always => "**required**",
        RequiredRule::Never => "optional",
        RequiredRule::UnlessProvided(other) => &format!("required unless `{}` provided", other),
        RequiredRule::IfEquals { arg: a, value } => &format!("required if `{} = \"{}\"`", a, value),
        RequiredRule::IfProvided(other) => &format!("required if `{}` provided", other),
    };

    format!(
        "**{}** for `{}`\n\n**Type:** {}\n\n**Required:** {}\n\n{}",
        arg.name,
        verb_name,
        arg.sem_type.type_name(),
        required,
        arg.description
    )
}
