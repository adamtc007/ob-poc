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
        CompletionContext::VerbName { prefix } => complete_verb_names(&prefix),
        CompletionContext::Keyword { verb_name, prefix } => complete_keywords(&verb_name, &prefix),
        CompletionContext::KeywordValue {
            verb_name: _,
            keyword,
            prefix,
            in_string,
        } => complete_keyword_values(&keyword, &prefix, in_string, entity_client).await,
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

/// Complete keyword values - uses EntityGateway for all lookups.
async fn complete_keyword_values(
    keyword: &str,
    prefix: &str,
    in_string: bool,
    entity_client: Option<EntityLookupClient>,
) -> Vec<CompletionItem> {
    tracing::debug!(
        "complete_keyword_values: keyword={}, prefix={}, in_string={}, has_client={}",
        keyword,
        prefix,
        in_string,
        entity_client.is_some()
    );

    // Map keyword to EntityGateway nickname
    let nickname = keyword_to_nickname(keyword);

    if let Some(nickname) = nickname {
        if let Some(mut client) = entity_client {
            match client.search(nickname, prefix, 15).await {
                Ok(results) => {
                    tracing::debug!("{} lookup returned {} results", nickname, results.len());
                    if !results.is_empty() {
                        return results
                            .into_iter()
                            .enumerate()
                            .map(|(i, m)| {
                                // Always insert the token (UUID for entities, code for enums)
                                // EntityGateway returns the correct value as token for each type
                                let insert = if in_string {
                                    m.id.clone()
                                } else {
                                    format!("\"{}\"", m.id)
                                };

                                let is_uuid = m.id.len() == 36 && m.id.contains('-');

                                CompletionItem {
                                    label: m.display.clone(),
                                    kind: Some(if is_uuid {
                                        CompletionItemKind::REFERENCE
                                    } else {
                                        CompletionItemKind::ENUM_MEMBER
                                    }),
                                    detail: Some(format!("{:.0}% match", m.score * 100.0)),
                                    documentation: if is_uuid {
                                        Some(Documentation::String(format!("ID: {}", m.id)))
                                    } else {
                                        Some(Documentation::String(format!("Code: {}", m.id)))
                                    },
                                    insert_text: Some(insert),
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

/// Map DSL keyword names to EntityGateway nicknames.
fn keyword_to_nickname(keyword: &str) -> Option<&'static str> {
    match keyword {
        // Entity ID lookups
        "cbu-id" => Some("cbu"),
        "entity-id"
        | "owner-entity-id"
        | "owned-entity-id"
        | "ubo-person-id"
        | "subject-entity-id"
        | "investor-entity-id"
        | "commercial-client-entity-id" => Some("entity"),

        // Reference data lookups
        "role" => Some("role"),
        "jurisdiction" => Some("jurisdiction"),
        "currency" | "cash-currency" => Some("currency"),
        "client-type" => Some("client_type"),
        "case-type" => Some("case_type"),
        "screening-type" => Some("screening_type"),
        "risk-rating" => Some("risk_rating"),
        "settlement-type" => Some("settlement_type"),
        "ssi-type" | "type" => Some("ssi_type"),
        "product-code" | "product" => Some("product"),
        "instrument-class" => Some("instrument_class"),
        "market" => Some("market"),

        _ => None,
    }
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

    // Note: Verb/keyword completion tests require DSL_CONFIG_DIR to be set
    // pointing to the config directory. These are tested in integration tests.

    #[test]
    fn test_keyword_to_nickname() {
        assert_eq!(keyword_to_nickname("cbu-id"), Some("cbu"));
        assert_eq!(keyword_to_nickname("role"), Some("role"));
        assert_eq!(keyword_to_nickname("jurisdiction"), Some("jurisdiction"));
        assert_eq!(keyword_to_nickname("currency"), Some("currency"));
        assert_eq!(keyword_to_nickname("cash-currency"), Some("currency"));
        assert_eq!(keyword_to_nickname("client-type"), Some("client_type"));
        assert_eq!(keyword_to_nickname("unknown-field"), None);
    }

    #[test]
    fn test_is_id_keyword() {
        assert!(is_id_keyword("cbu-id"));
        assert!(is_id_keyword("entity-id"));
        assert!(is_id_keyword("owner-entity-id"));
        assert!(!is_id_keyword("role"));
        assert!(!is_id_keyword("jurisdiction"));
        assert!(!is_id_keyword("client-type"));
    }
}
