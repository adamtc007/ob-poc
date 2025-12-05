//! Completion context detection.

use tower_lsp::lsp_types::Position;

use super::document::DocumentState;

/// Context for completion.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum CompletionContext {
    /// Completing a verb name (after open paren)
    VerbName { prefix: String },
    /// Completing a keyword name (after colon)
    Keyword { verb_name: String, prefix: String },
    /// Completing a keyword value
    KeywordValue {
        verb_name: String,
        keyword: String,
        prefix: String,
        in_string: bool,
    },
    /// Completing a symbol reference (after @)
    SymbolRef { prefix: String },
    /// No specific completion context
    None,
}

/// Detect the completion context at a position.
pub fn detect_completion_context(doc: &DocumentState, position: Position) -> CompletionContext {
    let line = match doc.get_line(position.line) {
        Some(l) => l,
        None => {
            tracing::debug!("No line at position {}", position.line);
            return CompletionContext::None;
        }
    };

    let col = position.character as usize;
    let prefix = if col <= line.len() {
        &line[..col]
    } else {
        line
    };

    tracing::debug!(
        "Context detection: line={}, col={}, prefix='{}'",
        position.line,
        col,
        prefix
    );

    // Check for symbol reference: @
    if let Some(at_pos) = prefix.rfind('@') {
        let after_at = &prefix[at_pos + 1..];
        if after_at
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return CompletionContext::SymbolRef {
                prefix: after_at.to_string(),
            };
        }
    }

    // Find the enclosing s-expression
    let (verb_name, current_keyword) = parse_sexp_context(prefix);

    match (verb_name, current_keyword) {
        // After open paren or word prefix - complete verb names
        (None, None) => {
            let word_prefix = extract_word_prefix(prefix);
            CompletionContext::VerbName {
                prefix: word_prefix,
            }
        }

        // After keyword colon - complete keyword names
        (Some(verb), None) if prefix.trim_end().ends_with(':') => CompletionContext::Keyword {
            verb_name: verb,
            prefix: String::new(),
        },

        // Typing keyword name
        (Some(verb), None) => {
            if let Some(kw_start) = prefix.rfind(':') {
                let kw_prefix = &prefix[kw_start + 1..];
                if kw_prefix.chars().all(|c| c.is_alphanumeric() || c == '-') {
                    return CompletionContext::Keyword {
                        verb_name: verb,
                        prefix: kw_prefix.to_string(),
                    };
                }
            }
            CompletionContext::None
        }

        // After keyword - complete value
        (Some(verb), Some(keyword)) => {
            let value_prefix = extract_value_prefix(prefix);
            let in_string = is_in_string(prefix);
            CompletionContext::KeywordValue {
                verb_name: verb,
                keyword,
                prefix: value_prefix,
                in_string,
            }
        }

        // No verb but have keyword - invalid s-expr, ignore
        (None, Some(_)) => CompletionContext::None,
    }
}

/// Parse s-expression context to find current verb and keyword.
fn parse_sexp_context(prefix: &str) -> (Option<String>, Option<String>) {
    let mut depth = 0;
    let mut verb_name: Option<String> = None;
    let mut current_keyword: Option<String> = None;
    let mut in_string = false;
    let mut token_start = None;
    let mut last_keyword: Option<String> = None;

    let chars: Vec<char> = prefix.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if in_string {
            if c == '"' && (i == 0 || chars[i - 1] != '\\') {
                in_string = false;
            }
            i += 1;
            continue;
        }

        match c {
            '"' => {
                in_string = true;
                token_start = None;
            }
            '(' => {
                depth += 1;
                verb_name = None;
                current_keyword = None;
                last_keyword = None;
                token_start = None;
            }
            ')' => {
                depth -= 1;
                if depth < 0 {
                    depth = 0;
                }
                token_start = None;
            }
            ':' if depth > 0 => {
                // Start of keyword
                token_start = Some(i);
            }
            ' ' | '\t' | '\n' => {
                if let Some(start) = token_start {
                    let token: String = chars[start..i].iter().collect();
                    if let Some(stripped) = token.strip_prefix(':') {
                        last_keyword = Some(stripped.to_string());
                        current_keyword = None; // Reset - we're after the keyword now
                    } else if verb_name.is_none() && depth > 0 {
                        verb_name = Some(token);
                    }
                }
                token_start = None;

                // If we just finished a keyword, set it as current
                if last_keyword.is_some() {
                    current_keyword = last_keyword.take();
                }
            }
            _ => {
                if token_start.is_none() && depth > 0 {
                    token_start = Some(i);
                }
            }
        }
        i += 1;
    }

    // Handle token at end of prefix - this is the token being typed
    // Don't set verb_name for incomplete tokens - we want verb completion
    if let Some(start) = token_start {
        let token: String = chars[start..].iter().collect();
        if token.strip_prefix(':').is_some() {
            // We're typing a keyword - don't set current_keyword yet
            current_keyword = None;
        }
        // Note: we intentionally don't set verb_name here for incomplete tokens
        // This allows verb completion to work when still typing the verb
    }

    (verb_name, current_keyword)
}

/// Extract word prefix for verb completion.
fn extract_word_prefix(prefix: &str) -> String {
    let mut result = String::new();
    for c in prefix.chars().rev() {
        if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' {
            result.insert(0, c);
        } else {
            break;
        }
    }
    result
}

/// Extract value prefix for keyword value completion.
fn extract_value_prefix(prefix: &str) -> String {
    // Find the last string start or space
    if let Some(quote_pos) = prefix.rfind('"') {
        return prefix[quote_pos + 1..].to_string();
    }

    // Otherwise, extract non-space chars from end
    let mut result = String::new();
    for c in prefix.chars().rev() {
        if c.is_whitespace() {
            break;
        }
        result.insert(0, c);
    }
    result
}

/// Check if cursor is inside a string.
fn is_in_string(prefix: &str) -> bool {
    let mut in_string = false;
    let chars: Vec<char> = prefix.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c == '"' && (i == 0 || chars[i - 1] != '\\') {
            in_string = !in_string;
        }
    }
    in_string
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_doc(text: &str) -> DocumentState {
        DocumentState::new(text.to_string())
    }

    #[test]
    fn test_verb_completion() {
        let doc = make_doc("(cbu");
        let ctx = detect_completion_context(
            &doc,
            Position {
                line: 0,
                character: 4,
            },
        );
        match ctx {
            CompletionContext::VerbName { prefix } => assert_eq!(prefix, "cbu"),
            _ => panic!("Expected VerbName context"),
        }
    }

    #[test]
    fn test_keyword_completion() {
        let doc = make_doc("(cbu.ensure :cbu");
        let ctx = detect_completion_context(
            &doc,
            Position {
                line: 0,
                character: 16,
            },
        );
        match ctx {
            CompletionContext::Keyword { verb_name, prefix } => {
                assert_eq!(verb_name, "cbu.ensure");
                assert_eq!(prefix, "cbu");
            }
            _ => panic!("Expected Keyword context"),
        }
    }

    #[test]
    fn test_symbol_ref_completion() {
        let doc = make_doc("(cbu.attach-entity :entity-id @co");
        let ctx = detect_completion_context(
            &doc,
            Position {
                line: 0,
                character: 33,
            },
        );
        match ctx {
            CompletionContext::SymbolRef { prefix } => assert_eq!(prefix, "co"),
            _ => panic!("Expected SymbolRef context"),
        }
    }

    #[test]
    fn test_keyword_value_completion_cbu_id() {
        // After `:cbu-id ` - should complete CBU ID values
        let doc = make_doc("(cbu.assign-role :cbu-id ");
        let ctx = detect_completion_context(
            &doc,
            Position {
                line: 0,
                character: 25,
            },
        );
        match ctx {
            CompletionContext::KeywordValue {
                verb_name,
                keyword,
                prefix,
                ..
            } => {
                assert_eq!(verb_name, "cbu.assign-role");
                assert_eq!(keyword, "cbu-id");
                assert_eq!(prefix, "");
            }
            other => panic!("Expected KeywordValue context, got {:?}", other),
        }
    }

    #[test]
    fn test_keyword_value_completion_with_partial() {
        // Typing after `:cbu-id "Ap` - should complete with prefix
        let doc = make_doc("(cbu.assign-role :cbu-id \"Ap");
        let ctx = detect_completion_context(
            &doc,
            Position {
                line: 0,
                character: 28,
            },
        );
        match ctx {
            CompletionContext::KeywordValue {
                verb_name,
                keyword,
                prefix,
                in_string,
            } => {
                assert_eq!(verb_name, "cbu.assign-role");
                assert_eq!(keyword, "cbu-id");
                assert_eq!(prefix, "Ap");
                assert!(in_string);
            }
            other => panic!("Expected KeywordValue context, got {:?}", other),
        }
    }

    #[test]
    fn test_multiline_keyword_value() {
        // Test with comment line before, simulating real file
        let doc = make_doc(";; Test CBU completion\n(cbu.assign-role :cbu-id \"Ap");
        let ctx = detect_completion_context(
            &doc,
            Position {
                line: 1,
                character: 28,
            },
        );
        println!("Context: {:?}", ctx);
        match ctx {
            CompletionContext::KeywordValue {
                verb_name,
                keyword,
                prefix,
                in_string,
            } => {
                assert_eq!(verb_name, "cbu.assign-role");
                assert_eq!(keyword, "cbu-id");
                assert_eq!(prefix, "Ap");
                assert!(in_string);
            }
            other => panic!("Expected KeywordValue context, got {:?}", other),
        }
    }
}
