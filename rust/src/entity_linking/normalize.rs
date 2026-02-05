//! Text normalization for entity matching
//!
//! Provides production-grade normalization for entity names and aliases:
//! - Unicode NFKC normalization
//! - Lowercase conversion
//! - Punctuation stripping (except digits)
//! - Whitespace collapsing
//! - Optional legal suffix removal

use unicode_normalization::UnicodeNormalization;

/// Common legal suffixes to optionally strip during normalization
const LEGAL_SUFFIXES: &[&str] = &[
    "inc",
    "incorporated",
    "corp",
    "corporation",
    "llc",
    "ltd",
    "limited",
    "plc",
    "sa",
    "ag",
    "gmbh",
    "co",
    "company",
    "lp",
    "llp",
    "nv",
    "bv",
    "sarl",
    "sas",
    "se",
    "kg",
    "ohg",
    "pty",
    "pte",
];

/// Normalize entity text for matching.
///
/// Performs:
/// - Unicode NFKC fold
/// - Lowercase conversion
/// - Strip punctuation (replace with space)
/// - Collapse whitespace
/// - Optionally strip legal suffixes
///
/// # Examples
///
/// ```
/// use ob_poc::entity_linking::normalize::normalize_entity_text;
///
/// assert_eq!(normalize_entity_text("Apple, Inc.", true), "apple");
/// assert_eq!(normalize_entity_text("Apple, Inc.", false), "apple inc");
/// assert_eq!(normalize_entity_text("Goldman Sachs & Co.", true), "goldman sachs");
/// ```
pub fn normalize_entity_text(s: &str, strip_legal_suffixes: bool) -> String {
    // Unicode NFKC normalization
    let folded: String = s.nfkc().collect();

    // Replace non-alphanumeric with space, lowercase
    let stripped: String = folded
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect();

    // Split into tokens
    let tokens: Vec<&str> = stripped.split_whitespace().collect();

    // Optionally filter legal suffixes
    let filtered: Vec<&str> = if strip_legal_suffixes {
        tokens.into_iter().filter(|t| !is_legal_suffix(t)).collect()
    } else {
        tokens
    };

    filtered.join(" ")
}

/// Check if a token is a common legal suffix
fn is_legal_suffix(token: &str) -> bool {
    LEGAL_SUFFIXES.contains(&token)
}

/// Tokenize text for overlap matching.
///
/// Returns normalized tokens suitable for index lookup.
pub fn tokenize(s: &str) -> Vec<String> {
    normalize_entity_text(s, false)
        .split_whitespace()
        .map(|s| s.to_string())
        .collect()
}

/// Tokenize without legal suffix stripping (for alias matching)
pub fn tokenize_preserving_suffixes(s: &str) -> Vec<String> {
    normalize_entity_text(s, false)
        .split_whitespace()
        .map(|s| s.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_with_suffix_strip() {
        assert_eq!(normalize_entity_text("Apple, Inc.", true), "apple");
        assert_eq!(
            normalize_entity_text("Ford Motor Company", true),
            "ford motor"
        );
        assert_eq!(
            normalize_entity_text("Goldman Sachs & Co.", true),
            "goldman sachs"
        );
        assert_eq!(
            normalize_entity_text("Microsoft Corporation", true),
            "microsoft"
        );
    }

    #[test]
    fn test_normalize_without_suffix_strip() {
        assert_eq!(normalize_entity_text("Apple, Inc.", false), "apple inc");
        assert_eq!(
            normalize_entity_text("Ford Motor Company", false),
            "ford motor company"
        );
    }

    #[test]
    fn test_unicode_normalization() {
        // Full-width characters are converted to ASCII by NFKC
        assert_eq!(normalize_entity_text("Ａｐｐｌｅ", false), "apple");
        // Accented characters are preserved (NFKC doesn't strip diacritics)
        // This is intentional - "société" and "societe" are different searches
        assert_eq!(
            normalize_entity_text("Société Générale", false),
            "société générale"
        );
    }

    #[test]
    fn test_tokenize() {
        let tokens = tokenize("Goldman Sachs Group Inc.");
        assert_eq!(tokens, vec!["goldman", "sachs", "group", "inc"]);
    }

    #[test]
    fn test_whitespace_collapse() {
        assert_eq!(normalize_entity_text("  Apple   Inc  ", false), "apple inc");
    }

    #[test]
    fn test_punctuation_handling() {
        assert_eq!(normalize_entity_text("AT&T Inc.", false), "at t inc");
        assert_eq!(
            normalize_entity_text("Johnson & Johnson", false),
            "johnson johnson"
        );
    }
}
