//! Sanitize user input before logging
//! Removes potential PII, client names, account numbers

use once_cell::sync::Lazy;
use regex::Regex;
use sha2::{Digest, Sha256};

// Common patterns to redact
static ACCOUNT_NUMBER: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b\d{8,12}\b").unwrap());
static EMAIL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b").unwrap());
static PHONE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b\+?[\d\s\-\(\)]{10,}\b").unwrap());

/// Sanitize user input for logging
/// Returns (sanitized_text, hash_of_original)
pub fn sanitize_input(input: &str, known_entities: &[&str]) -> (String, String) {
    let mut sanitized = input.to_string();

    // Replace known entity names with [ENTITY]
    for entity in known_entities {
        if entity.len() >= 3 {
            // Only replace meaningful names
            let pattern = regex::escape(entity);
            if let Ok(re) = Regex::new(&format!(r"(?i)\b{}\b", pattern)) {
                sanitized = re.replace_all(&sanitized, "[ENTITY]").to_string();
            }
        }
    }

    // Replace potential account numbers
    sanitized = ACCOUNT_NUMBER
        .replace_all(&sanitized, "[ACCOUNT]")
        .to_string();

    // Replace emails
    sanitized = EMAIL.replace_all(&sanitized, "[EMAIL]").to_string();

    // Replace phone numbers
    sanitized = PHONE.replace_all(&sanitized, "[PHONE]").to_string();

    // Hash for dedup (hash original, not sanitized)
    let hash = compute_hash(input);

    (sanitized, hash)
}

fn compute_hash(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    hex::encode(&result[..8]) // First 8 bytes = 16 hex chars
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_entities() {
        let (sanitized, _) = sanitize_input(
            "show me the Acme Corp ownership",
            &["Acme Corp", "BigBank Ltd"],
        );
        assert_eq!(sanitized, "show me the [ENTITY] ownership");
    }

    #[test]
    fn test_sanitize_account_numbers() {
        let (sanitized, _) = sanitize_input("look up account 12345678901", &[]);
        assert_eq!(sanitized, "look up account [ACCOUNT]");
    }

    #[test]
    fn test_sanitize_email() {
        let (sanitized, _) = sanitize_input("send to user@example.com", &[]);
        assert_eq!(sanitized, "send to [EMAIL]");
    }

    #[test]
    fn test_hash_consistency() {
        let (_, hash1) = sanitize_input("test input", &[]);
        let (_, hash2) = sanitize_input("test input", &[]);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_differs() {
        let (_, hash1) = sanitize_input("test input 1", &[]);
        let (_, hash2) = sanitize_input("test input 2", &[]);
        assert_ne!(hash1, hash2);
    }
}
