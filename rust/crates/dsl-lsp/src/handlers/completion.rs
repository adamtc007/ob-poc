//! Completion handler for the DSL Language Server.

use tower_lsp::lsp_types::*;

use crate::analysis::{detect_completion_context, CompletionContext, DocumentState, SymbolTable};

// Import from main crate - these will be available once compiled with ob-poc
use ob_poc::forth_engine::schema::registry::VERB_REGISTRY;
use ob_poc::forth_engine::schema::cache::SchemaCache;
use ob_poc::forth_engine::schema::types::{RefType, SemType, RequiredRule};

/// Generate completions based on cursor position.
pub fn get_completions(
    doc: &DocumentState,
    position: Position,
    symbols: &SymbolTable,
) -> Vec<CompletionItem> {
    let context = detect_completion_context(doc, position);
    
    match context {
        CompletionContext::VerbName { prefix } => {
            complete_verb_names(&prefix)
        }
        CompletionContext::Keyword { verb_name, prefix } => {
            complete_keywords(&verb_name, &prefix)
        }
        CompletionContext::KeywordValue { verb_name, keyword, prefix, in_string } => {
            complete_keyword_value(&verb_name, &keyword, &prefix, in_string, symbols)
        }
        CompletionContext::SymbolRef { prefix } => {
            complete_symbols(&prefix, symbols)
        }
        CompletionContext::None => vec![],
    }
}

/// Complete verb names.
fn complete_verb_names(prefix: &str) -> Vec<CompletionItem> {
    let prefix_lower = prefix.to_lowercase();
    
    VERB_REGISTRY
        .all()
        .filter(|verb| {
            verb.name.to_lowercase().starts_with(&prefix_lower)
                || verb.name.to_lowercase().contains(&prefix_lower)
        })
        .map(|verb| {
            let required_args: Vec<_> = verb.required_args();
            let detail = if required_args.is_empty() {
                format!("[{}]", verb.domain)
            } else {
                format!("[{}] requires: {}", verb.domain, required_args.join(", "))
            };

            CompletionItem {
                label: verb.name.to_string(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some(detail),
                documentation: Some(Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!(
                        "{}\n\n**Examples:**\n```clojure\n{}\n```",
                        verb.description,
                        verb.examples.join("\n")
                    ),
                })),
                insert_text: Some(verb.name.to_string()),
                insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                sort_text: Some(format!("0-{}-{}", verb.domain, verb.name)),
                ..Default::default()
            }
        })
        .collect()
}

/// Complete keyword arguments for a verb.
fn complete_keywords(verb_name: &str, prefix: &str) -> Vec<CompletionItem> {
    let verb = match VERB_REGISTRY.get(verb_name) {
        Some(v) => v,
        None => return vec![],
    };

    let prefix_lower = prefix.to_lowercase();

    verb.args
        .iter()
        .filter(|arg| {
            let name_without_colon = &arg.name[1..]; // Remove leading ':'
            name_without_colon.to_lowercase().starts_with(&prefix_lower)
                || name_without_colon.to_lowercase().contains(&prefix_lower)
        })
        .map(|arg| {
            let required_marker = match &arg.required {
                RequiredRule::Always => " (required)",
                RequiredRule::Never => "",
                _ => " (conditional)",
            };

            CompletionItem {
                label: arg.name.to_string(),
                kind: Some(CompletionItemKind::PROPERTY),
                detail: Some(format!("{}{}", arg.sem_type.type_name(), required_marker)),
                documentation: Some(Documentation::String(arg.description.to_string())),
                insert_text: Some(format!("{} ", arg.name)),
                insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                sort_text: Some(match &arg.required {
                    RequiredRule::Always => format!("0-{}", arg.name),
                    _ => format!("1-{}", arg.name),
                }),
                ..Default::default()
            }
        })
        .collect()
}

/// Complete keyword values based on semantic type.
fn complete_keyword_value(
    verb_name: &str,
    keyword: &str,
    prefix: &str,
    in_string: bool,
    symbols: &SymbolTable,
) -> Vec<CompletionItem> {
    let verb = match VERB_REGISTRY.get(verb_name) {
        Some(v) => v,
        None => return vec![],
    };

    let keyword_with_colon = if keyword.starts_with(':') {
        keyword.to_string()
    } else {
        format!(":{}", keyword)
    };

    let arg = match verb.args.iter().find(|a| a.name == keyword_with_colon) {
        Some(a) => a,
        None => return vec![],
    };

    match &arg.sem_type {
        // Reference types - use schema cache
        SemType::Ref(ref_type) => {
            complete_ref_type(ref_type, prefix, in_string)
        }

        // Enum types - fixed values
        SemType::Enum(values) => {
            complete_enum_values(values, prefix, in_string)
        }

        // Symbol references
        SemType::Symbol => {
            complete_symbols(prefix, symbols)
        }

        // Other types - no completions
        _ => vec![],
    }
}

/// Complete reference type values from schema cache.
fn complete_ref_type(ref_type: &RefType, prefix: &str, in_string: bool) -> Vec<CompletionItem> {
    let cache = SchemaCache::with_defaults();
    let prefix_lower = prefix.to_lowercase();

    cache
        .get_completions(ref_type)
        .into_iter()
        .filter(|entry| {
            entry.code.to_lowercase().contains(&prefix_lower)
                || entry.display_name.to_lowercase().contains(&prefix_lower)
        })
        .map(|entry| {
            let icon = match ref_type {
                RefType::DocumentType => "📄",
                RefType::Attribute => "📋",
                RefType::Role => "👤",
                RefType::EntityType => "🏢",
                RefType::Jurisdiction => "🌍",
                RefType::ScreeningList => "📜",
                RefType::Currency => "💰",
            };

            let insert_text = if in_string {
                entry.code.clone()
            } else {
                format!("\"{}\"", entry.code)
            };

            CompletionItem {
                label: format!("{} {}", icon, entry.display_name),
                kind: Some(CompletionItemKind::VALUE),
                detail: Some(entry.code.clone()),
                documentation: entry.description.as_ref().map(|d| {
                    Documentation::String(d.clone())
                }),
                insert_text: Some(insert_text),
                insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                filter_text: Some(format!("{} {}", entry.display_name, entry.code)),
                sort_text: Some(format!(
                    "{}-{}",
                    entry.category.as_deref().unwrap_or("zzz"),
                    entry.display_name
                )),
                label_details: entry.category.as_ref().map(|cat| {
                    CompletionItemLabelDetails {
                        description: Some(cat.clone()),
                        ..Default::default()
                    }
                }),
                ..Default::default()
            }
        })
        .collect()
}

/// Complete enum values.
fn complete_enum_values(values: &[&str], prefix: &str, in_string: bool) -> Vec<CompletionItem> {
    let prefix_lower = prefix.to_lowercase();

    values
        .iter()
        .filter(|v| v.to_lowercase().contains(&prefix_lower))
        .map(|v| {
            let insert_text = if in_string {
                v.to_string()
            } else {
                format!("\"{}\"", v)
            };

            CompletionItem {
                label: v.to_string(),
                kind: Some(CompletionItemKind::ENUM_MEMBER),
                insert_text: Some(insert_text),
                insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                ..Default::default()
            }
        })
        .collect()
}

/// Complete symbol references.
fn complete_symbols(prefix: &str, symbols: &SymbolTable) -> Vec<CompletionItem> {
    let prefix_lower = prefix.to_lowercase();

    symbols
        .all()
        .filter(|(name, _)| name.to_lowercase().starts_with(&prefix_lower))
        .map(|(name, info)| {
            CompletionItem {
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
            }
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
        assert!(completions.iter().any(|c| c.label == "cbu.ensure"));
    }

    #[test]
    fn test_keyword_completions() {
        let completions = complete_keywords("cbu.ensure", "cbu");
        assert!(!completions.is_empty());
        assert!(completions.iter().any(|c| c.label == ":cbu-name"));
    }

    #[test]
    fn test_enum_completions() {
        let completions = complete_enum_values(
            &["LOW", "MEDIUM", "HIGH"],
            "med",
            false,
        );
        assert_eq!(completions.len(), 1);
        assert_eq!(completions[0].label, "MEDIUM");
    }
}
