//! Failure Classifier
//!
//! Classifies errors by type and determines remediation path.
//! Generates stable fingerprints for deduplication.

use crate::events::ErrorSnapshot;
use sha2::{Digest, Sha256};

use super::types::{ErrorType, RemediationPath};

/// Fingerprint version for future migration
const FINGERPRINT_VERSION: u8 = 1;

/// Failure classifier - categorizes errors and generates fingerprints
pub struct FailureClassifier {
    /// Known transient error patterns
    transient_patterns: Vec<&'static str>,
    /// Known schema drift patterns
    schema_patterns: Vec<&'static str>,
    /// Known enum drift patterns
    enum_patterns: Vec<&'static str>,
}

impl Default for FailureClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl FailureClassifier {
    pub fn new() -> Self {
        Self {
            transient_patterns: vec![
                "timeout",
                "timed out",
                "connection reset",
                "connection refused",
                "temporarily unavailable",
                "service unavailable",
                "503",
                "504",
                "rate limit",
                "too many requests",
                "429",
                "pool exhausted",
                "no available connections",
            ],
            schema_patterns: vec![
                "missing field",
                "unknown field",
                "expected object",
                "expected array",
                "expected string",
                "expected number",
                "invalid type",
                "schema mismatch",
            ],
            enum_patterns: vec![
                "unknown variant",
                "invalid variant",
                "expected one of",
                "enum",
                "not a valid",
            ],
        }
    }

    /// Classify an error snapshot into error type and remediation path
    pub fn classify_snapshot(
        &self,
        verb: &str,
        error: &ErrorSnapshot,
    ) -> (ErrorType, RemediationPath) {
        let message_lower = error.message.to_lowercase();

        // Check for transient errors first (runtime retry)
        if self.matches_patterns(&message_lower, &self.transient_patterns) {
            let error_type = self.classify_transient(&message_lower);
            return (error_type, RemediationPath::Runtime);
        }

        // Check for enum drift
        if self.matches_patterns(&message_lower, &self.enum_patterns) {
            return (ErrorType::EnumDrift, RemediationPath::Code);
        }

        // Check for schema drift
        if self.matches_patterns(&message_lower, &self.schema_patterns) {
            return (ErrorType::SchemaDrift, RemediationPath::Code);
        }

        // Check for panics
        if message_lower.contains("panic") || message_lower.contains("unwrap") {
            return (ErrorType::HandlerPanic, RemediationPath::Code);
        }

        // Check for parse errors
        if message_lower.contains("parse error")
            || message_lower.contains("parsing failed")
            || message_lower.contains("invalid json")
            || message_lower.contains("invalid xml")
        {
            return (ErrorType::ParseError, RemediationPath::Code);
        }

        // Check for DSL parse errors
        if message_lower.contains("dsl") && message_lower.contains("parse") {
            return (ErrorType::DslParseError, RemediationPath::Code);
        }

        // Check for API changes
        if message_lower.contains("404") || message_lower.contains("not found") {
            // Could be endpoint moved or just not found
            if verb.contains("fetch") || verb.contains("lookup") {
                return (ErrorType::ApiEndpointMoved, RemediationPath::Code);
            }
            return (ErrorType::ValidationFailed, RemediationPath::LogOnly);
        }

        // Check for auth issues
        if message_lower.contains("401")
            || message_lower.contains("403")
            || message_lower.contains("unauthorized")
            || message_lower.contains("forbidden")
        {
            return (ErrorType::ApiAuthChanged, RemediationPath::Code);
        }

        // Check for validation failures
        if message_lower.contains("validation")
            || message_lower.contains("constraint")
            || message_lower.contains("required field")
        {
            return (ErrorType::ValidationFailed, RemediationPath::LogOnly);
        }

        // Default: handler error requiring investigation
        (ErrorType::HandlerError, RemediationPath::Code)
    }

    /// Compute fingerprint for an error
    ///
    /// Returns (fingerprint, discriminator, version)
    /// Fingerprint format: v{version}:{error_type}:{verb}:{source}:{hash}
    pub fn compute_fingerprint_snapshot(
        &self,
        verb: &str,
        error_type: ErrorType,
        error: &ErrorSnapshot,
    ) -> (String, String, u8) {
        // Extract source from verb if present (e.g., "research.fetch-entity" with source="gleif")
        let source = self.extract_source(verb, &error.message);

        // Build discriminator from error details
        let discriminator = self.build_discriminator(error_type, error);

        // Hash the discriminator for the fingerprint
        let hash = self.hash_discriminator(&discriminator);

        let fingerprint = format!(
            "v{}:{}:{}:{}:{}",
            FINGERPRINT_VERSION,
            error_type.as_str(),
            verb,
            source.as_deref().unwrap_or("internal"),
            &hash[..12] // Use first 12 chars of hash
        );

        (fingerprint, discriminator, FINGERPRINT_VERSION)
    }

    /// Suggest an action based on error type
    pub fn suggest_action(&self, error_type: ErrorType) -> Option<String> {
        match error_type {
            ErrorType::Timeout | ErrorType::RateLimited => {
                Some("Consider adding exponential backoff retry".to_string())
            }
            ErrorType::ConnectionReset | ErrorType::ServiceUnavailable => {
                Some("Check external service health, may be temporary".to_string())
            }
            ErrorType::PoolExhausted => {
                Some("Consider increasing connection pool size".to_string())
            }
            ErrorType::EnumDrift => {
                Some("External API returned unknown enum value - update Rust enum".to_string())
            }
            ErrorType::SchemaDrift => {
                Some("External API schema changed - update serde structs".to_string())
            }
            ErrorType::ParseError => {
                Some("Response parsing failed - check format expectations".to_string())
            }
            ErrorType::HandlerPanic => {
                Some("Handler panicked - add error handling for edge case".to_string())
            }
            ErrorType::HandlerError => {
                Some("Handler returned error - investigate root cause".to_string())
            }
            ErrorType::DslParseError => {
                Some("DSL parsing failed - check DSL syntax or generator".to_string())
            }
            ErrorType::ApiEndpointMoved => {
                Some("API endpoint not found - check if API was updated".to_string())
            }
            ErrorType::ApiAuthChanged => {
                Some("Authentication failed - check API keys and permissions".to_string())
            }
            ErrorType::ValidationFailed => None, // Usually user input issue
            ErrorType::Unknown => Some("Unknown error - investigate logs".to_string()),
        }
    }

    // =========================================================================
    // PRIVATE HELPERS
    // =========================================================================

    fn matches_patterns(&self, message: &str, patterns: &[&str]) -> bool {
        patterns.iter().any(|p| message.contains(p))
    }

    fn classify_transient(&self, message: &str) -> ErrorType {
        if message.contains("timeout") || message.contains("timed out") {
            ErrorType::Timeout
        } else if message.contains("rate limit") || message.contains("429") {
            ErrorType::RateLimited
        } else if message.contains("connection reset") {
            ErrorType::ConnectionReset
        } else if message.contains("pool") {
            ErrorType::PoolExhausted
        } else {
            ErrorType::ServiceUnavailable
        }
    }

    fn extract_source(&self, verb: &str, message: &str) -> Option<String> {
        // Check for known sources in verb name
        let known_sources = ["gleif", "lbr", "bods", "brave", "anthropic", "openai"];

        for source in known_sources {
            if verb.to_lowercase().contains(source) {
                return Some(source.to_string());
            }
            if message.to_lowercase().contains(source) {
                return Some(source.to_string());
            }
        }

        // Check for URL patterns
        if message.contains("api.gleif.org") {
            return Some("gleif".to_string());
        }
        if message.contains("lbr.lu") {
            return Some("lbr".to_string());
        }

        None
    }

    fn build_discriminator(&self, error_type: ErrorType, error: &ErrorSnapshot) -> String {
        match error_type {
            // For schema/enum drift, include the specific field/variant
            ErrorType::EnumDrift | ErrorType::SchemaDrift => {
                // Try to extract the specific field name
                self.extract_field_name(&error.message)
                    .unwrap_or_else(|| error.message.chars().take(100).collect())
            }
            // For parse errors, include structure info
            ErrorType::ParseError => self
                .extract_parse_context(&error.message)
                .unwrap_or_else(|| error.message.chars().take(100).collect()),
            // For other errors, use normalized message
            _ => self.normalize_message(&error.message),
        }
    }

    fn extract_field_name(&self, message: &str) -> Option<String> {
        // Look for patterns like "missing field `foo`" or "unknown field `bar`"
        let patterns = [
            (r"field `([^`]+)`", 1),
            (r"field '([^']+)'", 1),
            (r"variant `([^`]+)`", 1),
        ];

        for (pattern, _) in patterns {
            if let Some(captures) = regex::Regex::new(pattern)
                .ok()
                .and_then(|re| re.captures(message))
            {
                if let Some(field) = captures.get(1) {
                    return Some(field.as_str().to_string());
                }
            }
        }

        None
    }

    fn extract_parse_context(&self, message: &str) -> Option<String> {
        // Extract line/column info if present
        let line_col = regex::Regex::new(r"line (\d+), column (\d+)")
            .ok()
            .and_then(|re| re.captures(message))
            .map(|c| format!("line{}:col{}", &c[1], &c[2]));

        // Extract expected type if present
        let expected = regex::Regex::new(r"expected (\w+)")
            .ok()
            .and_then(|re| re.captures(message))
            .map(|c| c[1].to_string());

        match (line_col, expected) {
            (Some(loc), Some(exp)) => Some(format!("{}:{}", exp, loc)),
            (Some(loc), None) => Some(loc),
            (None, Some(exp)) => Some(exp),
            (None, None) => None,
        }
    }

    fn normalize_message(&self, message: &str) -> String {
        // Remove UUIDs
        let uuid_re = regex::Regex::new(
            r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}",
        )
        .unwrap();
        let normalized = uuid_re.replace_all(message, "<UUID>");

        // Remove numbers that look like IDs
        let id_re = regex::Regex::new(r"\b\d{6,}\b").unwrap();
        let normalized = id_re.replace_all(&normalized, "<ID>");

        // Remove timestamps
        let ts_re = regex::Regex::new(r"\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}").unwrap();
        let normalized = ts_re.replace_all(&normalized, "<TIMESTAMP>");

        // Truncate
        normalized.chars().take(200).collect()
    }

    fn hash_discriminator(&self, discriminator: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(discriminator.as_bytes());
        let result = hasher.finalize();
        hex::encode(result)
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_snapshot(message: &str) -> ErrorSnapshot {
        ErrorSnapshot {
            error_type: "TestError".to_string(),
            message: message.to_string(),
            source_id: None,
            http_status: None,
            verb: "test.verb".to_string(),
        }
    }

    #[test]
    fn test_classify_timeout() {
        let classifier = FailureClassifier::new();
        let error = make_snapshot("Request timed out after 30s");

        let (error_type, path) = classifier.classify_snapshot("gleif.fetch", &error);

        assert_eq!(error_type, ErrorType::Timeout);
        assert_eq!(path, RemediationPath::Runtime);
    }

    #[test]
    fn test_classify_rate_limit() {
        let classifier = FailureClassifier::new();
        let error = make_snapshot("Rate limit exceeded: 429 Too Many Requests");

        let (error_type, path) = classifier.classify_snapshot("api.call", &error);

        assert_eq!(error_type, ErrorType::RateLimited);
        assert_eq!(path, RemediationPath::Runtime);
    }

    #[test]
    fn test_classify_enum_drift() {
        let classifier = FailureClassifier::new();
        let error =
            make_snapshot("unknown variant `NEW_STATUS`, expected one of `ACTIVE`, `INACTIVE`");

        let (error_type, path) = classifier.classify_snapshot("gleif.parse", &error);

        assert_eq!(error_type, ErrorType::EnumDrift);
        assert_eq!(path, RemediationPath::Code);
    }

    #[test]
    fn test_classify_schema_drift() {
        let classifier = FailureClassifier::new();
        let error = make_snapshot("missing field `newRequiredField` at line 5");

        let (error_type, path) = classifier.classify_snapshot("api.parse", &error);

        assert_eq!(error_type, ErrorType::SchemaDrift);
        assert_eq!(path, RemediationPath::Code);
    }

    #[test]
    fn test_classify_panic() {
        let classifier = FailureClassifier::new();
        let error =
            make_snapshot("thread 'main' panicked at 'called `unwrap()` on a `None` value'");

        let (error_type, path) = classifier.classify_snapshot("handler.process", &error);

        assert_eq!(error_type, ErrorType::HandlerPanic);
        assert_eq!(path, RemediationPath::Code);
    }

    #[test]
    fn test_fingerprint_stable() {
        let classifier = FailureClassifier::new();
        let error = make_snapshot("unknown variant `NEW_STATUS`");

        let (fp1, _, _) =
            classifier.compute_fingerprint_snapshot("gleif.parse", ErrorType::EnumDrift, &error);
        let (fp2, _, _) =
            classifier.compute_fingerprint_snapshot("gleif.parse", ErrorType::EnumDrift, &error);

        assert_eq!(fp1, fp2, "Fingerprint should be stable");
    }

    #[test]
    fn test_fingerprint_format() {
        let classifier = FailureClassifier::new();
        let error = make_snapshot("timeout connecting to api.gleif.org");

        let (fp, _, version) =
            classifier.compute_fingerprint_snapshot("gleif.fetch", ErrorType::Timeout, &error);

        assert!(fp.starts_with("v1:"), "Should start with version");
        assert!(fp.contains("TIMEOUT"), "Should contain error type");
        assert!(fp.contains("gleif.fetch"), "Should contain verb");
        assert_eq!(version, 1);
    }

    #[test]
    fn test_fingerprint_different_for_different_errors() {
        let classifier = FailureClassifier::new();

        let error1 = make_snapshot("missing field `foo`");
        let error2 = make_snapshot("missing field `bar`");

        let (fp1, _, _) =
            classifier.compute_fingerprint_snapshot("api.parse", ErrorType::SchemaDrift, &error1);
        let (fp2, _, _) =
            classifier.compute_fingerprint_snapshot("api.parse", ErrorType::SchemaDrift, &error2);

        assert_ne!(
            fp1, fp2,
            "Different errors should have different fingerprints"
        );
    }

    #[test]
    fn test_suggest_action() {
        let classifier = FailureClassifier::new();

        assert!(classifier
            .suggest_action(ErrorType::Timeout)
            .unwrap()
            .contains("backoff"));
        assert!(classifier
            .suggest_action(ErrorType::EnumDrift)
            .unwrap()
            .contains("enum"));
        assert!(classifier
            .suggest_action(ErrorType::ValidationFailed)
            .is_none());
    }

    #[test]
    fn test_normalize_removes_uuids() {
        let classifier = FailureClassifier::new();
        let message = "Entity 550e8400-e29b-41d4-a716-446655440000 not found";

        let normalized = classifier.normalize_message(message);

        assert!(normalized.contains("<UUID>"));
        assert!(!normalized.contains("550e8400"));
    }

    #[test]
    fn test_extract_source_from_verb() {
        let classifier = FailureClassifier::new();

        let source = classifier.extract_source("gleif.fetch-entity", "some error");
        assert_eq!(source, Some("gleif".to_string()));

        let source = classifier.extract_source("bods.import", "some error");
        assert_eq!(source, Some("bods".to_string()));
    }
}
