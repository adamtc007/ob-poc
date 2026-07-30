//! Symbol substitution helper for DSL sheet execution.
//!
//! `SheetExecutor` (the phased/RunSheet execution orchestrator this module
//! used to house) was entirely dead code — never constructed anywhere — and
//! was removed as part of the session-subsystem dead-code pass (it existed
//! purely to operate on the also-deleted legacy `session::dsl_sheet` types
//! and their unified-but-still-unused `session::unified` counterparts).
//! `substitute_symbols_pure` is the one piece of real, tested logic that
//! survived: substituting `@symbol` references with resolved UUIDs.

use std::collections::HashMap;

use uuid::Uuid;

/// Pure function for symbol substitution (testable without pool)
fn substitute_symbols_pure(
    source: &str,
    symbols: &HashMap<String, Uuid>,
) -> Result<String, String> {
    let mut result = source.to_string();

    // Replace each @symbol with its UUID
    for (symbol, uuid) in symbols {
        let pattern = format!("@{}", symbol);
        result = result.replace(&pattern, &format!("\"{}\"", uuid));
    }

    // Check for remaining unresolved @symbols
    let mut chars = result.chars().peekable();
    let mut pos = 0;
    while let Some(c) = chars.next() {
        if c == '@' {
            if let Some(&next) = chars.peek() {
                if next.is_alphabetic() {
                    let remaining: String = result[pos..].chars().take(30).collect();
                    return Err(format!("Unresolved symbol in: {}", remaining));
                }
            }
        }
        pos += c.len_utf8();
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test symbol substitution without needing a database pool
    /// We test the substitute_symbols function directly since it's pure logic
    #[test]
    fn test_substitute_symbols_basic() {
        let mut symbols = HashMap::new();
        let uuid = Uuid::new_v4();
        symbols.insert("fund".to_string(), uuid);

        let source = "(cbu.assign-role :cbu-id @fund :role \"DIRECTOR\")";
        let result = substitute_symbols_pure(source, &symbols).unwrap();

        assert!(result.contains(&uuid.to_string()));
        assert!(!result.contains("@fund"));
    }

    #[test]
    fn test_substitute_symbols_unresolved() {
        let symbols = HashMap::new(); // Empty - @fund not defined

        let source = "(cbu.assign-role :cbu-id @fund :role \"DIRECTOR\")";
        let result = substitute_symbols_pure(source, &symbols);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unresolved symbol"));
    }

    #[test]
    fn test_substitute_symbols_multiple() {
        let mut symbols = HashMap::new();
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();
        symbols.insert("fund".to_string(), uuid1);
        symbols.insert("director".to_string(), uuid2);

        let source = "(cbu.assign-role :cbu-id @fund :person-id @director)";
        let result = substitute_symbols_pure(source, &symbols).unwrap();

        assert!(result.contains(&uuid1.to_string()));
        assert!(result.contains(&uuid2.to_string()));
        assert!(!result.contains("@fund"));
        assert!(!result.contains("@director"));
    }
}
