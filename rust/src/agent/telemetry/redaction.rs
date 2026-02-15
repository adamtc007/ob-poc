//! PII-safe hashing and redaction for intent telemetry.

use sha2::{Digest, Sha256};

/// Normalize an utterance for stable hashing: lowercase, trim, collapse whitespace.
pub fn normalize_utterance(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// SHA-256 hex hash of a normalized utterance.
pub fn utterance_hash(normalized: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    hex::encode(hasher.finalize())
}

/// Produce a redacted preview: max 80 chars, no PII masking beyond truncation.
pub fn preview_redacted(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.len() <= 80 {
        Some(trimmed.to_string())
    } else {
        Some(format!("{}...", &trimmed[..77]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_collapses_whitespace() {
        assert_eq!(normalize_utterance("  hello   world  "), "hello world");
    }

    #[test]
    fn test_normalize_lowercases() {
        assert_eq!(
            normalize_utterance("Load Allianz Book"),
            "load allianz book"
        );
    }

    #[test]
    fn test_hash_stable_across_whitespace() {
        let a = utterance_hash(&normalize_utterance("  load allianz  book "));
        let b = utterance_hash(&normalize_utterance("load allianz book"));
        assert_eq!(a, b, "Hash must be stable across whitespace variations");
    }

    #[test]
    fn test_hash_differs_for_different_input() {
        let a = utterance_hash("load allianz book");
        let b = utterance_hash("create a fund");
        assert_ne!(a, b);
    }

    #[test]
    fn test_preview_truncates_at_80() {
        let long = "a".repeat(100);
        let preview = preview_redacted(&long).unwrap();
        assert_eq!(preview.len(), 80); // 77 + "..."
        assert!(preview.ends_with("..."));
    }

    #[test]
    fn test_preview_short_input_unchanged() {
        assert_eq!(preview_redacted("hello").unwrap(), "hello");
    }

    #[test]
    fn test_preview_empty_returns_none() {
        assert!(preview_redacted("").is_none());
        assert!(preview_redacted("   ").is_none());
    }
}
