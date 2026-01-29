//! Completion context detection.
//!
//! This module analyzes the text before the cursor position to determine
//! what kind of completion should be offered. It tracks:
//! - S-expression nesting `(...)`
//! - List nesting `[...]`
//! - Map nesting `{...}`
//! - String boundaries `"..."`
//! - Keyword arguments `:`
//! - Symbol references `@`

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
    /// Completing a symbol reference (after @) - for existing symbols
    /// Includes optional verb/keyword context for type-aware ranking
    SymbolRef {
        prefix: String,
        /// The verb being called (if inside an s-expression)
        verb_name: Option<String>,
        /// The keyword this symbol is a value for (e.g., "cbu-id", "entity-id")
        keyword: Option<String>,
    },
    /// Completing an entity lookup that will be inserted as @symbol (after keyword + @)
    EntityAsSymbol {
        verb_name: String,
        keyword: String,
        prefix: String,
    },
    /// No specific completion context
    None,
}

/// Get all text from the start of the document up to the cursor position.
/// This is necessary for proper multiline s-expression context detection.
fn get_text_up_to_position(doc: &DocumentState, position: Position) -> String {
    let mut result = String::new();

    for (line_num, line) in doc.text.lines().enumerate() {
        if line_num < position.line as usize {
            // Add full line plus newline
            result.push_str(line);
            result.push('\n');
        } else if line_num == position.line as usize {
            // Add partial line up to cursor column
            let col = position.character as usize;
            if col <= line.len() {
                result.push_str(&line[..col]);
            } else {
                result.push_str(line);
            }
            break;
        }
    }

    result
}

/// Detect the completion context at a position.
pub fn detect_completion_context(doc: &DocumentState, position: Position) -> CompletionContext {
    // Get all text up to the cursor position for proper multiline context
    let prefix = get_text_up_to_position(doc, position);

    tracing::debug!(
        "Context detection: line={}, col={}, prefix len={}",
        position.line,
        position.character,
        prefix.len()
    );

    // Check for @ symbol - could be existing symbol ref OR entity lookup for keyword
    if let Some(at_pos) = prefix.rfind('@') {
        let after_at = &prefix[at_pos + 1..];
        if after_at
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            // Check if we're after a keyword that expects an entity lookup
            let before_at = &prefix[..at_pos];
            let (verb_name, keyword) = parse_sexp_context(before_at);

            tracing::debug!(
                "@ context: before_at='{}', verb={:?}, keyword={:?}",
                before_at,
                verb_name,
                keyword
            );

            if let (Some(ref verb), Some(ref kw)) = (&verb_name, &keyword) {
                // Use dynamic registry lookup instead of hardcoded list
                if is_entity_keyword_for_verb(verb, kw) {
                    return CompletionContext::EntityAsSymbol {
                        verb_name: verb.clone(),
                        keyword: kw.clone(),
                        prefix: after_at.to_string(),
                    };
                }
            }

            // Otherwise, it's a regular symbol reference with optional context
            return CompletionContext::SymbolRef {
                prefix: after_at.to_string(),
                verb_name,
                keyword,
            };
        }
    }

    // Find the enclosing s-expression
    let (verb_name, current_keyword) = parse_sexp_context(&prefix);

    match (verb_name, current_keyword) {
        // After open paren or word prefix - complete verb names
        (None, None) => {
            let word_prefix = extract_word_prefix(&prefix);
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
            let value_prefix = extract_value_prefix(&prefix);
            let in_string = is_in_string(&prefix);
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
///
/// Tracks three levels of nesting:
/// - `paren_depth` for `(...)` - S-expressions
/// - `bracket_depth` for `[...]` - Lists
/// - `brace_depth` for `{...}` - Maps
///
/// Keywords (`:name`) are only recognized at the top level of an s-expression,
/// not inside lists or maps.
fn parse_sexp_context(prefix: &str) -> (Option<String>, Option<String>) {
    let mut paren_depth = 0i32;
    let mut bracket_depth = 0i32;
    let mut brace_depth = 0i32;
    let mut verb_name: Option<String> = None;
    let mut current_keyword: Option<String> = None;
    let mut in_string = false;
    let mut token_start: Option<usize> = None;
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
                paren_depth += 1;
                // Reset context for new s-expression
                verb_name = None;
                current_keyword = None;
                last_keyword = None;
                token_start = None;
            }
            ')' => {
                paren_depth = (paren_depth - 1).max(0);
                token_start = None;
            }
            '[' => {
                bracket_depth += 1;
                token_start = None;
            }
            ']' => {
                bracket_depth = (bracket_depth - 1).max(0);
                token_start = None;
            }
            '{' => {
                brace_depth += 1;
                token_start = None;
            }
            '}' => {
                brace_depth = (brace_depth - 1).max(0);
                token_start = None;
            }
            ':' if paren_depth > 0 && bracket_depth == 0 && brace_depth == 0 => {
                // Start of keyword at top level of s-expression
                token_start = Some(i);
            }
            ':' if brace_depth > 0 => {
                // Map key - track but don't set as verb keyword
                token_start = Some(i);
            }
            ' ' | '\t' | '\n' => {
                if let Some(start) = token_start {
                    let token: String = chars[start..i].iter().collect();
                    if let Some(stripped) = token.strip_prefix(':') {
                        // Only set as keyword if at top level (not in list or map)
                        if bracket_depth == 0 && brace_depth == 0 {
                            last_keyword = Some(stripped.to_string());
                            current_keyword = None; // Reset - we're after the keyword now
                        }
                    } else if verb_name.is_none()
                        && paren_depth > 0
                        && bracket_depth == 0
                        && brace_depth == 0
                    {
                        verb_name = Some(token);
                    }
                }
                token_start = None;

                // If we just finished a keyword at top level, set it as current
                if last_keyword.is_some() && bracket_depth == 0 && brace_depth == 0 {
                    current_keyword = last_keyword.take();
                }
            }
            _ => {
                if token_start.is_none() && paren_depth > 0 {
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

/// Check if a keyword expects an entity reference by looking up the verb registry.
///
/// This replaces the hardcoded `is_entity_keyword()` function with dynamic lookup.
/// A keyword is an entity keyword if the verb's arg definition has a `lookup` config.
fn is_entity_keyword_for_verb(verb_name: &str, keyword: &str) -> bool {
    use ob_poc::dsl_v2::find_unified_verb;

    let parts: Vec<&str> = verb_name.split('.').collect();
    if parts.len() != 2 {
        return false;
    }

    if let Some(verb) = find_unified_verb(parts[0], parts[1]) {
        for arg in &verb.args {
            if arg.name == keyword {
                return arg.lookup.is_some();
            }
        }
    }

    false
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
        // After :as @ - should complete with existing symbols
        let doc = make_doc("(cbu.ensure :name \"Test\" :as @fu");
        let ctx = detect_completion_context(
            &doc,
            Position {
                line: 0,
                character: 32,
            },
        );
        match ctx {
            CompletionContext::SymbolRef { prefix, .. } => assert_eq!(prefix, "fu"),
            other => panic!("Expected SymbolRef context, got {:?}", other),
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

    #[test]
    fn test_context_inside_list() {
        // Inside a list, keywords should NOT be detected as verb keywords
        let doc = make_doc("(test.verb :items [:a :b ");
        let ctx = detect_completion_context(
            &doc,
            Position {
                line: 0,
                character: 25,
            },
        );
        // Should NOT interpret :a or :b as verb keywords
        match ctx {
            CompletionContext::KeywordValue { keyword, .. } => {
                // The keyword should still be "items" from the outer context
                assert_eq!(keyword, "items");
            }
            other => panic!("Expected KeywordValue context, got {:?}", other),
        }
    }

    #[test]
    fn test_context_inside_map() {
        // Inside a map, keywords are map keys, not verb keywords
        let doc = make_doc("(test.verb :config {:name \"Test\" :val ");
        let ctx = detect_completion_context(
            &doc,
            Position {
                line: 0,
                character: 38,
            },
        );
        // Should interpret "config" as the verb keyword, not ":val"
        match ctx {
            CompletionContext::KeywordValue { keyword, .. } => {
                assert_eq!(keyword, "config");
            }
            other => panic!("Expected KeywordValue context, got {:?}", other),
        }
    }

    #[test]
    fn test_nested_brackets_and_braces() {
        // Complex nesting: list containing map
        let doc = make_doc("(test.verb :data [{:x 1} {:y ");
        let ctx = detect_completion_context(
            &doc,
            Position {
                line: 0,
                character: 29,
            },
        );
        match ctx {
            CompletionContext::KeywordValue { keyword, .. } => {
                // Should still recognize "data" as the outer keyword
                assert_eq!(keyword, "data");
            }
            other => panic!("Expected KeywordValue context, got {:?}", other),
        }
    }
}
