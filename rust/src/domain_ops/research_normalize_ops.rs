//! Research Generic Normalize Operations
//!
//! Plugin handler for research payload normalization:
//! - Canonical JSON generation with deterministic key ordering
//! - SHA-256 content hashing for deduplication
//! - Optional persistence to research_normalized_payloads table
//!
//! This operation takes raw JSON payloads from research sources,
//! canonicalizes them (sorted keys, trimmed strings), and produces
//! a content hash for deduplication and audit.

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

use super::helpers::{extract_string, extract_string_opt};
use super::CustomOperation;

#[cfg(feature = "database")]
use sqlx::PgPool;

// =============================================================================
// RESULT TYPE
// =============================================================================

/// Result of normalizing a research payload.
///
/// Contains the canonical JSON, its SHA-256 hash, and metadata
/// about the normalization operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizeResult {
    /// Name of the research source (e.g., "gleif", "companies-house")
    pub source_name: String,
    /// Canonicalized JSON payload (sorted keys, trimmed strings)
    pub canonical_json: serde_json::Value,
    /// SHA-256 hex digest of the canonical JSON string
    pub content_hash: String,
    /// Number of top-level fields in the canonical JSON
    pub field_count: usize,
    /// ISO 8601 timestamp of when normalization occurred
    pub normalized_at: String,
}

// =============================================================================
// CANONICALIZATION
// =============================================================================

/// Recursively canonicalize a JSON value:
/// - Object keys are sorted alphabetically
/// - String values are trimmed of leading/trailing whitespace
/// - Arrays are canonicalized element-wise
/// - Other types (numbers, booleans, null) are unchanged
fn canonicalize_json(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut sorted: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            for key in keys {
                sorted.insert(key.clone(), canonicalize_json(&map[key]));
            }
            serde_json::Value::Object(sorted)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(canonicalize_json).collect())
        }
        serde_json::Value::String(s) => serde_json::Value::String(s.trim().to_string()),
        other => other.clone(),
    }
}

// =============================================================================
// NORMALIZE OP
// =============================================================================

/// Normalize a research payload into canonical JSON with SHA-256 hash.
///
/// Rationale: Requires custom canonicalization (recursive key sorting,
/// string trimming) and SHA-256 hashing that cannot be expressed as CRUD.
/// Optionally persists to the normalized payloads table for deduplication.
#[register_custom_op]
pub struct ResearchGenericNormalizeOp;

fn normalize_payload_impl(source_name: String, payload_raw: &str) -> Result<NormalizeResult> {
    let parsed: serde_json::Value = serde_json::from_str(payload_raw)
        .map_err(|e| anyhow::anyhow!("Invalid JSON payload: {}", e))?;
    let canonical = canonicalize_json(&parsed);
    let canonical_str = serde_json::to_string(&canonical)?;

    let mut hasher = Sha256::new();
    hasher.update(canonical_str.as_bytes());
    let hash_bytes = hasher.finalize();
    let content_hash = hex::encode(hash_bytes);

    let field_count = match &canonical {
        serde_json::Value::Object(map) => map.len(),
        _ => 0,
    };

    let normalized_at = chrono::Utc::now().to_rfc3339();

    Ok(NormalizeResult {
        source_name,
        canonical_json: canonical,
        content_hash,
        field_count,
        normalized_at,
    })
}

#[async_trait]
impl CustomOperation for ResearchGenericNormalizeOp {
    fn domain(&self) -> &'static str {
        "research.generic"
    }

    fn verb(&self) -> &'static str {
        "normalize"
    }

    fn rationale(&self) -> &'static str {
        "Requires custom canonicalization logic (recursive key sorting, string trimming) and SHA-256 content hashing for deduplication"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let source_name = extract_string(verb_call, "source-name")?;
        let payload_raw = extract_string(verb_call, "payload")?;
        let _schema_name = extract_string_opt(verb_call, "schema-name");
        let result = normalize_payload_impl(source_name, &payload_raw)?;

        let payload_id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO "ob-poc".research_normalized_payloads
                (payload_id, source_name, content_hash, canonical_payload, normalized_at)
            VALUES ($1, $2, $3, $4, NOW())
            ON CONFLICT (content_hash) DO NOTHING"#,
        )
        .bind(payload_id)
        .bind(&result.source_name)
        .bind(&result.content_hash)
        .bind(&result.canonical_json)
        .execute(pool)
        .await?;

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        let source_name = extract_string(verb_call, "source-name")?;
        let payload_raw = extract_string(verb_call, "payload")?;
        let result = normalize_payload_impl(source_name, &payload_raw)?;

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut sem_os_core::execution::VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<sem_os_core::execution::VerbExecutionOutcome> {
        use super::helpers::{json_extract_string, json_extract_string_opt};

        let source_name = json_extract_string(args, "source-name")?;
        let payload_raw = json_extract_string(args, "payload")?;
        let _schema_name = json_extract_string_opt(args, "schema-name");
        let result = normalize_payload_impl(source_name, &payload_raw)?;

        let payload_id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO "ob-poc".research_normalized_payloads
                (payload_id, source_name, content_hash, canonical_payload, normalized_at)
            VALUES ($1, $2, $3, $4, NOW())
            ON CONFLICT (content_hash) DO NOTHING"#,
        )
        .bind(payload_id)
        .bind(&result.source_name)
        .bind(&result.content_hash)
        .bind(&result.canonical_json)
        .execute(pool)
        .await?;

        Ok(sem_os_core::execution::VerbExecutionOutcome::Record(
            serde_json::to_value(result)?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_metadata() {
        let op = ResearchGenericNormalizeOp;
        assert_eq!(op.domain(), "research.generic");
        assert_eq!(op.verb(), "normalize");
        assert!(!op.rationale().is_empty());
    }

    #[test]
    fn test_canonicalize_sorts_keys() {
        let input: serde_json::Value = serde_json::from_str(r#"{"b": 1, "a": 2, "c": 3}"#).unwrap();
        let canonical = canonicalize_json(&input);
        let output = serde_json::to_string(&canonical).unwrap();
        assert_eq!(output, r#"{"a":2,"b":1,"c":3}"#);
    }

    #[test]
    fn test_canonicalize_trims_strings() {
        let input: serde_json::Value =
            serde_json::from_str(r#"{"name": "  hello  ", "city": " London "}"#).unwrap();
        let canonical = canonicalize_json(&input);

        assert_eq!(
            canonical["name"],
            serde_json::Value::String("hello".to_string())
        );
        assert_eq!(
            canonical["city"],
            serde_json::Value::String("London".to_string())
        );
    }

    #[test]
    fn test_canonicalize_nested_objects() {
        let input: serde_json::Value =
            serde_json::from_str(r#"{"z": {"b": 1, "a": 2}, "a": {"d": 3, "c": 4}}"#).unwrap();
        let canonical = canonicalize_json(&input);
        let output = serde_json::to_string(&canonical).unwrap();
        // Outer keys sorted: a before z; inner keys sorted too
        assert_eq!(output, r#"{"a":{"c":4,"d":3},"z":{"a":2,"b":1}}"#);
    }

    #[test]
    fn test_canonicalize_arrays() {
        let input: serde_json::Value =
            serde_json::from_str(r#"{"items": [{"b": 1, "a": 2}, {"d": " x ", "c": 4}]}"#).unwrap();
        let canonical = canonicalize_json(&input);
        let output = serde_json::to_string(&canonical).unwrap();
        assert_eq!(output, r#"{"items":[{"a":2,"b":1},{"c":4,"d":"x"}]}"#);
    }

    #[test]
    fn test_canonicalize_preserves_non_string_types() {
        let input: serde_json::Value =
            serde_json::from_str(r#"{"flag": true, "count": 42, "empty": null, "ratio": 3.14}"#)
                .unwrap();
        let canonical = canonicalize_json(&input);
        assert_eq!(canonical["flag"], serde_json::Value::Bool(true));
        assert_eq!(canonical["count"], serde_json::json!(42));
        assert_eq!(canonical["empty"], serde_json::Value::Null);
        assert_eq!(canonical["ratio"], serde_json::json!(314.0_f64 / 100.0));
    }

    #[test]
    fn test_same_payload_same_hash() {
        // Two JSON strings with different key orderings should produce the same hash
        let json_a = r#"{"name": "Allianz", "country": "DE", "lei": "ABC123"}"#;
        let json_b = r#"{"lei": "ABC123", "name": "Allianz", "country": "DE"}"#;

        let parsed_a: serde_json::Value = serde_json::from_str(json_a).unwrap();
        let parsed_b: serde_json::Value = serde_json::from_str(json_b).unwrap();

        let canonical_a = canonicalize_json(&parsed_a);
        let canonical_b = canonicalize_json(&parsed_b);

        let str_a = serde_json::to_string(&canonical_a).unwrap();
        let str_b = serde_json::to_string(&canonical_b).unwrap();

        // Canonical forms must be identical
        assert_eq!(str_a, str_b);

        // Hashes must be identical
        let hash_a = {
            let mut h = Sha256::new();
            h.update(str_a.as_bytes());
            hex::encode(h.finalize())
        };
        let hash_b = {
            let mut h = Sha256::new();
            h.update(str_b.as_bytes());
            hex::encode(h.finalize())
        };

        assert_eq!(hash_a, hash_b);
    }

    #[test]
    fn test_different_payload_different_hash() {
        let json_a = r#"{"name": "Allianz"}"#;
        let json_b = r#"{"name": "BlackRock"}"#;

        let canonical_a = canonicalize_json(&serde_json::from_str(json_a).unwrap());
        let canonical_b = canonicalize_json(&serde_json::from_str(json_b).unwrap());

        let hash_a = {
            let mut h = Sha256::new();
            h.update(serde_json::to_string(&canonical_a).unwrap().as_bytes());
            hex::encode(h.finalize())
        };
        let hash_b = {
            let mut h = Sha256::new();
            h.update(serde_json::to_string(&canonical_b).unwrap().as_bytes());
            hex::encode(h.finalize())
        };

        assert_ne!(hash_a, hash_b);
    }
}
