//! Completion handler for the DSL Language Server.

use tower_lsp::lsp_types::*;

use crate::analysis::{detect_completion_context, CompletionContext, DocumentState, SymbolTable};

use ob_poc::dsl_v2::{find_unified_verb, registry};

/// Generate completions based on cursor position.
pub fn get_completions(
    doc: &DocumentState,
    position: Position,
    symbols: &SymbolTable,
) -> Vec<CompletionItem> {
    let context = detect_completion_context(doc, position);

    match context {
        CompletionContext::VerbName { prefix } => complete_verb_names(&prefix),
        CompletionContext::Keyword { verb_name, prefix } => complete_keywords(&verb_name, &prefix),
        CompletionContext::KeywordValue {
            verb_name: _,
            keyword: _,
            prefix: _,
            in_string: _,
        } => {
            // V2 doesn't have rich type info for value completion
            vec![]
        }
        CompletionContext::SymbolRef { prefix } => complete_symbols(&prefix, symbols),
        CompletionContext::None => vec![],
    }
}

/// Complete verb names.
fn complete_verb_names(prefix: &str) -> Vec<CompletionItem> {
    let prefix_lower = prefix.to_lowercase();
    let reg = registry();

    reg.all_verbs()
        .filter(|verb| {
            let full_name = verb.full_name();
            full_name.to_lowercase().contains(&prefix_lower)
        })
        .map(|verb| {
            let full_name = verb.full_name();
            let required: Vec<_> = verb
                .required_arg_names()
                .iter()
                .map(|s| format!(":{}", s))
                .collect();
            let detail = if required.is_empty() {
                format!("[{}]", verb.domain)
            } else {
                format!("[{}] requires: {}", verb.domain, required.join(", "))
            };

            CompletionItem {
                label: full_name.clone(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some(detail),
                documentation: Some(Documentation::String(verb.description.clone())),
                insert_text: Some(full_name.clone()),
                insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                sort_text: Some(format!("0-{}-{}", verb.domain, verb.verb)),
                ..Default::default()
            }
        })
        .collect()
}

/// Complete keyword arguments for a verb.
fn complete_keywords(verb_name: &str, prefix: &str) -> Vec<CompletionItem> {
    // Parse domain.verb
    let parts: Vec<&str> = verb_name.split('.').collect();
    if parts.len() != 2 {
        return vec![];
    }

    let verb = match find_unified_verb(parts[0], parts[1]) {
        Some(v) => v,
        None => return vec![],
    };

    let prefix_lower = prefix.to_lowercase();
    let mut completions = Vec::new();

    // Required args
    for arg in verb.required_arg_names() {
        if arg.to_lowercase().contains(&prefix_lower) {
            completions.push(CompletionItem {
                label: format!(":{}", arg),
                kind: Some(CompletionItemKind::PROPERTY),
                detail: Some("(required)".to_string()),
                insert_text: Some(format!(":{} ", arg)),
                insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                sort_text: Some(format!("0-{}", arg)),
                ..Default::default()
            });
        }
    }

    // Optional args
    for arg in verb.optional_arg_names() {
        if arg.to_lowercase().contains(&prefix_lower) {
            completions.push(CompletionItem {
                label: format!(":{}", arg),
                kind: Some(CompletionItemKind::PROPERTY),
                detail: Some("(optional)".to_string()),
                insert_text: Some(format!(":{} ", arg)),
                insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                sort_text: Some(format!("1-{}", arg)),
                ..Default::default()
            });
        }
    }

    // Always offer :as for symbol binding
    if "as".contains(&prefix_lower) {
        completions.push(CompletionItem {
            label: ":as".to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("bind result to @symbol".to_string()),
            insert_text: Some(":as @".to_string()),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            sort_text: Some("2-as".to_string()),
            ..Default::default()
        });
    }

    completions
}

/// Complete symbol references.
fn complete_symbols(prefix: &str, symbols: &SymbolTable) -> Vec<CompletionItem> {
    let prefix_lower = prefix.to_lowercase();

    symbols
        .all()
        .filter(|(name, _)| name.to_lowercase().starts_with(&prefix_lower))
        .map(|(name, info)| CompletionItem {
            label: format!("@{}", name),
            kind: Some(CompletionItemKind::VARIABLE),
            detail: Some(format!("{} from {}", info.id_type, info.defined_by)),
            documentation: Some(Documentation::String(format!(
                "Defined at line {}",
                info.definition.range.start.line + 1
            ))),
            insert_text: Some(format!("@{}", name)),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            ..Default::default()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verb_completions() {
        let completions = complete_verb_names("cbu");
        assert!(!completions.is_empty());
        assert!(completions.iter().any(|c| c.label.starts_with("cbu.")));
    }

    #[test]
    fn test_keyword_completions() {
        let completions = complete_keywords("cbu.create", "");
        assert!(!completions.is_empty());
    }
}
