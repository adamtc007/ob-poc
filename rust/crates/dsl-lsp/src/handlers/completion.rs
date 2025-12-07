//! Completion handler for the DSL Language Server.

use tower_lsp::lsp_types::*;

use crate::analysis::{detect_completion_context, CompletionContext, DocumentState, SymbolTable};
use crate::entity_client::EntityLookupClient;

use ob_poc::dsl_v2::{find_unified_verb, registry};

/// Generate completions based on cursor position.
pub async fn get_completions(
    doc: &DocumentState,
    position: Position,
    symbols: &SymbolTable,
    entity_client: Option<EntityLookupClient>,
) -> Vec<CompletionItem> {
    let context = detect_completion_context(doc, position);

    tracing::debug!(
        "Completion context: {:?}, entity_client: {}",
        context,
        entity_client.is_some()
    );

    match context {
        CompletionContext::VerbName { prefix } => complete_verb_names(&prefix, position),
        CompletionContext::Keyword { verb_name, prefix } => complete_keywords(&verb_name, &prefix),
        CompletionContext::KeywordValue {
            verb_name,
            keyword,
            prefix,
            in_string,
        } => {
            complete_keyword_values(
                &verb_name,
                &keyword,
                &prefix,
                in_string,
                position,
                entity_client,
            )
            .await
        }
        CompletionContext::SymbolRef { prefix } => complete_symbols(&prefix, symbols),
        CompletionContext::EntityAsSymbol {
            verb_name,
            keyword,
            prefix,
        } => {
            complete_entity_as_symbol(&verb_name, &keyword, &prefix, position, entity_client).await
        }
        CompletionContext::None => vec![],
    }
}

/// Complete verb names - progressively narrows as user types.
/// e.g., "cbu" -> all cbu.* verbs, "cbu.e" -> cbu.ensure, etc.
fn complete_verb_names(prefix: &str, position: Position) -> Vec<CompletionItem> {
    let prefix_lower = prefix.to_lowercase();
    let reg = registry();

    // Calculate range to replace the prefix
    let prefix_len = prefix.len() as u32;
    let start_char = position.character.saturating_sub(prefix_len);
    let range = Range {
        start: Position {
            line: position.line,
            character: start_char,
        },
        end: position,
    };

    reg.all_verbs()
        .filter(|verb| {
            let full_name = verb.full_name().to_lowercase();
            // Use starts_with for progressive narrowing
            full_name.starts_with(&prefix_lower)
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
                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                    range,
                    new_text: full_name.clone(),
                })),
                insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                filter_text: Some(full_name.clone()),
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

/// Complete keyword values - uses EntityGateway for all lookups.
/// Uses text_edit to replace any existing prefix (including quotes) with bare token.
///
/// Looks up the LookupConfig from the verb registry to determine the entity_type
/// for the EntityGateway search, making this fully dynamic based on verbs.yaml.
async fn complete_keyword_values(
    verb_name: &str,
    keyword: &str,
    prefix: &str,
    in_string: bool,
    position: Position,
    entity_client: Option<EntityLookupClient>,
) -> Vec<CompletionItem> {
    tracing::debug!(
        "complete_keyword_values: verb={}, keyword={}, prefix={}, in_string={}, has_client={}",
        verb_name,
        keyword,
        prefix,
        in_string,
        entity_client.is_some()
    );

    // Look up the entity_type from the verb's arg LookupConfig
    let nickname = get_lookup_entity_type(verb_name, keyword);

    if let Some(nickname) = nickname {
        if let Some(mut client) = entity_client {
            match client.search(&nickname, prefix, 15).await {
                Ok(results) => {
                    tracing::debug!("{} lookup returned {} results", nickname, results.len());
                    if !results.is_empty() {
                        // Calculate range to replace:
                        // - If in_string, we need to replace from opening quote to cursor
                        // - prefix.len() is chars typed after the quote (or after space)
                        // - Add 1 for the opening quote if in_string
                        let prefix_len = prefix.len() as u32;
                        let extra = if in_string { 1 } else { 0 }; // for opening quote
                        let start_char = position.character.saturating_sub(prefix_len + extra);

                        let range = Range {
                            start: Position {
                                line: position.line,
                                character: start_char,
                            },
                            end: position,
                        };

                        return results
                            .into_iter()
                            .enumerate()
                            .map(|(i, m)| {
                                // All tokens are inserted as bare identifiers (no quotes)
                                // Reference data: DIRECTOR, LU, FUND_ACCOUNTING
                                let new_text = m.id.clone();

                                CompletionItem {
                                    label: m.display.clone(),
                                    kind: Some(CompletionItemKind::CONSTANT),
                                    detail: Some(format!("Code: {}", m.id)),
                                    documentation: Some(Documentation::String(format!(
                                        "Insert: {}",
                                        m.id
                                    ))),
                                    // Use text_edit to replace prefix (and quote if present) with bare token
                                    text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                                        range,
                                        new_text,
                                    })),
                                    insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                                    filter_text: Some(m.display.clone()),
                                    sort_text: Some(format!("{:03}", i)),
                                    ..Default::default()
                                }
                            })
                            .collect();
                    }
                }
                Err(e) => {
                    tracing::warn!("{} lookup failed: {}", nickname, e);
                }
            }
        }
    }

    // No results from EntityGateway
    vec![]
}

/// Complete entity lookup when user types @ after an entity keyword (e.g., :entity-id @Ap)
///
/// Inserts `@KEY` as a bare token. The parser/linter resolves the key to UUID.
///
/// - filterText: `@{display_name}` - allows filtering as user types `@Apex`
/// - textEdit: replaces from @ to cursor with `@KEY`
/// - label: display name shown in completion menu
async fn complete_entity_as_symbol(
    verb_name: &str,
    keyword: &str,
    prefix: &str,
    position: Position,
    entity_client: Option<EntityLookupClient>,
) -> Vec<CompletionItem> {
    tracing::debug!(
        "complete_entity_as_symbol: verb={}, keyword={}, prefix={}, position={:?}",
        verb_name,
        keyword,
        prefix,
        position
    );

    // Look up the entity_type from the verb's arg LookupConfig
    let nickname = get_lookup_entity_type(verb_name, keyword);

    if let Some(nickname) = nickname {
        if let Some(mut client) = entity_client {
            match client.search(&nickname, prefix, 15).await {
                Ok(results) => {
                    tracing::debug!("{} lookup returned {} results", nickname, results.len());
                    if !results.is_empty() {
                        // Calculate the range to replace: from @ to cursor position
                        // prefix.len() is the number of chars after @
                        // We need to go back prefix.len() + 1 (for the @) from cursor
                        let prefix_len = prefix.len() as u32;
                        let at_char = position.character.saturating_sub(prefix_len + 1);

                        let range = Range {
                            start: Position {
                                line: position.line,
                                character: at_char,
                            },
                            end: position,
                        };

                        return results
                            .into_iter()
                            .enumerate()
                            .map(|(i, m)| {
                                // Insert @KEY as bare token - parser/linter resolves to UUID
                                // m.id is the key (e.g., search_name, display_name) from return_key config
                                let new_text = format!("@{}", m.id);

                                // filterText includes @ so user can filter by typing @Apex
                                let filter = format!("@{}", m.display);

                                CompletionItem {
                                    label: m.display.clone(),
                                    kind: Some(CompletionItemKind::REFERENCE),
                                    detail: Some(format!("{:.0}% match", m.score * 100.0)),
                                    documentation: Some(Documentation::String(format!(
                                        "Key: {}",
                                        m.id
                                    ))),
                                    // Use textEdit to replace @prefix with @KEY
                                    text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                                        range,
                                        new_text,
                                    })),
                                    insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                                    // Include @ in filterText so filtering works as user types @Apex
                                    filter_text: Some(filter),
                                    sort_text: Some(format!("{:03}", i)),
                                    ..Default::default()
                                }
                            })
                            .collect();
                    }
                }
                Err(e) => {
                    tracing::warn!("{} lookup failed: {}", nickname, e);
                }
            }
        }
    }

    vec![]
}

/// Get the entity_type from the verb's arg LookupConfig.
///
/// Looks up the verb in the registry, finds the arg by keyword name,
/// and returns the entity_type from its LookupConfig if present.
///
/// This replaces the hardcoded keyword_to_nickname mapping with
/// a dynamic lookup from verbs.yaml configuration.
fn get_lookup_entity_type(verb_name: &str, keyword: &str) -> Option<String> {
    // Parse domain.verb
    let parts: Vec<&str> = verb_name.split('.').collect();
    if parts.len() != 2 {
        tracing::debug!(
            "get_lookup_entity_type: invalid verb_name format: {}",
            verb_name
        );
        return None;
    }

    let verb = find_unified_verb(parts[0], parts[1])?;

    // Find the arg matching this keyword
    for arg in &verb.args {
        if arg.name == keyword {
            if let Some(ref lookup) = arg.lookup {
                if let Some(ref entity_type) = lookup.entity_type {
                    tracing::debug!(
                        "get_lookup_entity_type: {}:{} -> entity_type={}",
                        verb_name,
                        keyword,
                        entity_type
                    );
                    return Some(entity_type.clone());
                }
            }
        }
    }

    tracing::debug!(
        "get_lookup_entity_type: no lookup config for {}:{}",
        verb_name,
        keyword
    );
    None
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
    // Note: All completion tests require DSL_CONFIG_DIR to be set
    // pointing to the config directory. These are tested in integration tests.
    //
    // The get_lookup_entity_type function now dynamically looks up entity_type
    // from the verb registry based on verbs.yaml configuration, so tests
    // need the full config loaded.
}
