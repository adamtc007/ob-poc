//! Policy-based PII Redactor
//!
//! Redacts sensitive information from error context while preserving
//! enough structure for debugging schema/enum drift issues.

use super::types::ErrorType;
use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

// =============================================================================
// REDACTION PATTERNS
// =============================================================================

/// Email pattern
static EMAIL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap());

/// Phone number patterns (various formats)
static PHONE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\+?\d{1,3}[-.\s]?)?\(?\d{3}\)?[-.\s]?\d{3}[-.\s]?\d{4}").unwrap()
});

/// Credit card pattern (simplified)
static CARD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b\d{4}[-\s]?\d{4}[-\s]?\d{4}[-\s]?\d{4}\b").unwrap());

/// SSN/National ID patterns
static SSN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b\d{3}[-\s]?\d{2}[-\s]?\d{4}\b").unwrap());

/// UUID pattern
static UUID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}")
        .unwrap()
});

/// LEI pattern (20 alphanumeric characters)
static LEI_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b[A-Z0-9]{20}\b").unwrap());

// =============================================================================
// REDACTION MODE
// =============================================================================

/// Redaction mode controlling how much information is preserved
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedactionMode {
    /// Keep keys/types, redact PII strings but preserve short identifiers
    /// Used for schema/enum drift where we need to see the structure
    StructuralOnly,

    /// Redact all string values except very short ones (likely enums)
    /// Used for most errors where we don't need the full data
    Full,
}

impl RedactionMode {
    /// Get the appropriate mode for an error type
    pub fn for_error_type(error_type: ErrorType) -> Self {
        match error_type {
            // For schema/parse errors, we need structure to debug
            ErrorType::EnumDrift | ErrorType::SchemaDrift | ErrorType::ParseError => {
                Self::StructuralOnly
            }
            // For everything else, be more aggressive
            _ => Self::Full,
        }
    }
}

// =============================================================================
// REDACTOR
// =============================================================================

/// Policy-based PII redactor
#[derive(Debug, Clone)]
pub struct Redactor {
    /// Maximum length for strings to preserve in Full mode
    max_preserve_len: usize,

    /// Patterns that indicate a value should be preserved (likely enum/identifier)
    safe_patterns: Vec<Regex>,
}

impl Default for Redactor {
    fn default() -> Self {
        Self::new()
    }
}

impl Redactor {
    pub fn new() -> Self {
        Self {
            max_preserve_len: 30, // Short strings are likely identifiers/enums
            safe_patterns: vec![
                // Status-like values
                Regex::new(r"^[A-Z][A-Z_]{2,20}$").unwrap(),
                // ISO country codes
                Regex::new(r"^[A-Z]{2,3}$").unwrap(),
                // Currency codes
                Regex::new(r"^[A-Z]{3}$").unwrap(),
                // Boolean-like
                Regex::new(r"^(true|false|yes|no|active|inactive)$").unwrap(),
            ],
        }
    }

    /// Redact a JSON value based on error type
    pub fn redact_for_error(&self, value: &Value, error_type: ErrorType) -> Value {
        let mode = RedactionMode::for_error_type(error_type);
        self.redact(value, mode)
    }

    /// Redact a JSON value with explicit mode
    pub fn redact(&self, value: &Value, mode: RedactionMode) -> Value {
        match value {
            Value::Null => Value::Null,
            Value::Bool(b) => Value::Bool(*b),
            Value::Number(n) => {
                // Preserve numbers in structural mode, redact large ones in full mode
                if mode == RedactionMode::StructuralOnly {
                    Value::Number(n.clone())
                } else {
                    // Keep small integers (likely IDs/codes), redact large ones
                    if let Some(i) = n.as_i64() {
                        if i.abs() < 1000 {
                            Value::Number(n.clone())
                        } else {
                            Value::String("<NUMBER>".to_string())
                        }
                    } else {
                        Value::String("<NUMBER>".to_string())
                    }
                }
            }
            Value::String(s) => self.redact_string(s, mode),
            Value::Array(arr) => Value::Array(arr.iter().map(|v| self.redact(v, mode)).collect()),
            Value::Object(obj) => {
                let mut result = serde_json::Map::new();
                for (key, val) in obj {
                    // Always preserve keys (important for schema debugging)
                    result.insert(key.clone(), self.redact(val, mode));
                }
                Value::Object(result)
            }
        }
    }

    /// Redact a single string value
    fn redact_string(&self, s: &str, mode: RedactionMode) -> Value {
        // First, redact identifiers (UUIDs, LEIs) - these can look like phone numbers
        let with_identifiers_redacted = self.redact_identifiers(s);

        // Then check for and redact PII patterns
        let with_pii_redacted = if self.contains_pii(&with_identifiers_redacted) {
            self.redact_pii(&with_identifiers_redacted)
        } else {
            with_identifiers_redacted
        };

        match mode {
            RedactionMode::StructuralOnly => {
                // Preserve the value with identifiers/PII redacted
                Value::String(with_pii_redacted)
            }
            RedactionMode::Full => {
                // Check if it's safe to preserve (after redaction)
                if self.is_safe_value(&with_pii_redacted) {
                    Value::String(with_pii_redacted)
                } else if with_pii_redacted.len() <= self.max_preserve_len
                    && with_pii_redacted.chars().all(|c| {
                        c.is_alphanumeric() || c == '_' || c == '-' || c == '<' || c == '>'
                    })
                {
                    // Short alphanumeric strings (or redaction placeholders) are likely identifiers
                    Value::String(with_pii_redacted)
                } else {
                    Value::String("<REDACTED>".to_string())
                }
            }
        }
    }

    /// Check if a string contains obvious PII
    fn contains_pii(&self, s: &str) -> bool {
        EMAIL_RE.is_match(s) || PHONE_RE.is_match(s) || CARD_RE.is_match(s) || SSN_RE.is_match(s)
    }

    /// Redact PII patterns from a string
    fn redact_pii(&self, s: &str) -> String {
        let mut result = s.to_string();
        result = EMAIL_RE.replace_all(&result, "<EMAIL>").to_string();
        result = PHONE_RE.replace_all(&result, "<PHONE>").to_string();
        result = CARD_RE.replace_all(&result, "<CARD>").to_string();
        result = SSN_RE.replace_all(&result, "<SSN>").to_string();
        result
    }

    /// Redact identifiers (UUIDs, LEIs) but keep structure
    fn redact_identifiers(&self, s: &str) -> String {
        let mut result = s.to_string();
        result = UUID_RE.replace_all(&result, "<UUID>").to_string();
        result = LEI_RE.replace_all(&result, "<LEI>").to_string();
        result
    }

    /// Check if a value is safe to preserve (likely an enum/code)
    fn is_safe_value(&self, s: &str) -> bool {
        self.safe_patterns.iter().any(|p| p.is_match(s))
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_redact_email() {
        let redactor = Redactor::new();
        let value = json!({"email": "user@example.com", "name": "John"});

        let redacted = redactor.redact(&value, RedactionMode::Full);

        let obj = redacted.as_object().unwrap();
        assert!(obj["email"].as_str().unwrap().contains("<EMAIL>"));
    }

    #[test]
    fn test_redact_phone() {
        let redactor = Redactor::new();
        let value = json!({"phone": "+1-555-123-4567"});

        let redacted = redactor.redact(&value, RedactionMode::Full);

        let obj = redacted.as_object().unwrap();
        assert!(obj["phone"].as_str().unwrap().contains("<PHONE>"));
    }

    #[test]
    fn test_preserve_short_strings() {
        let redactor = Redactor::new();
        let value = json!({"status": "ACTIVE", "country": "US"});

        let redacted = redactor.redact(&value, RedactionMode::Full);

        let obj = redacted.as_object().unwrap();
        assert_eq!(obj["status"].as_str().unwrap(), "ACTIVE");
        assert_eq!(obj["country"].as_str().unwrap(), "US");
    }

    #[test]
    fn test_redact_long_strings() {
        let redactor = Redactor::new();
        let value = json!({"description": "This is a very long description that should be redacted because it might contain sensitive information"});

        let redacted = redactor.redact(&value, RedactionMode::Full);

        let obj = redacted.as_object().unwrap();
        assert_eq!(obj["description"].as_str().unwrap(), "<REDACTED>");
    }

    #[test]
    fn test_structural_preserves_values() {
        let redactor = Redactor::new();
        let value = json!({
            "field": "some value here",
            "nested": {
                "data": "more data"
            }
        });

        let redacted = redactor.redact(&value, RedactionMode::StructuralOnly);

        let obj = redacted.as_object().unwrap();
        // In structural mode, we preserve values (just redact identifiers)
        assert_eq!(obj["field"].as_str().unwrap(), "some value here");
    }

    #[test]
    fn test_structural_redacts_uuids() {
        let redactor = Redactor::new();
        let value = json!({
            "entity_id": "550e8400-e29b-41d4-a716-446655440000"
        });

        let redacted = redactor.redact(&value, RedactionMode::StructuralOnly);

        let obj = redacted.as_object().unwrap();
        let result = obj["entity_id"].as_str().unwrap();
        assert!(
            result.contains("<UUID>"),
            "Expected <UUID>, got: {}",
            result
        );
    }

    #[test]
    fn test_preserve_keys() {
        let redactor = Redactor::new();
        let value = json!({
            "sensitive_field_name": "secret value"
        });

        let redacted = redactor.redact(&value, RedactionMode::Full);

        // Keys should always be preserved
        let obj = redacted.as_object().unwrap();
        assert!(obj.contains_key("sensitive_field_name"));
    }

    #[test]
    fn test_redact_arrays() {
        let redactor = Redactor::new();
        let value = json!({
            "items": ["short", "also_short", "this is a very long string that should be redacted"]
        });

        let redacted = redactor.redact(&value, RedactionMode::Full);

        let obj = redacted.as_object().unwrap();
        let items = obj["items"].as_array().unwrap();
        assert_eq!(items[0].as_str().unwrap(), "short");
        assert_eq!(items[1].as_str().unwrap(), "also_short");
        assert_eq!(items[2].as_str().unwrap(), "<REDACTED>");
    }

    #[test]
    fn test_mode_for_error_type() {
        assert_eq!(
            RedactionMode::for_error_type(ErrorType::EnumDrift),
            RedactionMode::StructuralOnly
        );
        assert_eq!(
            RedactionMode::for_error_type(ErrorType::SchemaDrift),
            RedactionMode::StructuralOnly
        );
        assert_eq!(
            RedactionMode::for_error_type(ErrorType::Timeout),
            RedactionMode::Full
        );
        assert_eq!(
            RedactionMode::for_error_type(ErrorType::HandlerPanic),
            RedactionMode::Full
        );
    }

    #[test]
    fn test_preserve_booleans() {
        let redactor = Redactor::new();
        let value = json!({"active": true, "deleted": false});

        let redacted = redactor.redact(&value, RedactionMode::Full);

        let obj = redacted.as_object().unwrap();
        assert_eq!(obj["active"].as_bool().unwrap(), true);
        assert_eq!(obj["deleted"].as_bool().unwrap(), false);
    }

    #[test]
    fn test_preserve_small_numbers() {
        let redactor = Redactor::new();
        let value = json!({"count": 5, "large": 123456789});

        let redacted = redactor.redact(&value, RedactionMode::Full);

        let obj = redacted.as_object().unwrap();
        assert_eq!(obj["count"].as_i64().unwrap(), 5);
        assert_eq!(obj["large"].as_str().unwrap(), "<NUMBER>");
    }

    #[test]
    fn test_nested_redaction() {
        let redactor = Redactor::new();
        let value = json!({
            "user": {
                "email": "test@example.com",
                "profile": {
                    "status": "VERIFIED"
                }
            }
        });

        let redacted = redactor.redact(&value, RedactionMode::Full);

        let user = redacted["user"].as_object().unwrap();
        assert!(user["email"].as_str().unwrap().contains("<EMAIL>"));
        assert_eq!(user["profile"]["status"].as_str().unwrap(), "VERIFIED");
    }
}
